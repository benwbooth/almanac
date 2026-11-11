//! Where mail is kept on disk: messages in an SQLite database
//! (`mail.sqlite`), attachments as ordinary files in a folder per message
//! (`attachments/<message id>/<file name>`). Everything else (accounts,
//! calendars, contacts, settings) stays in `data.json`.
//!
//! Only what lists and sorting need is kept in memory (sender, subject, date,
//! flags, a short preview); the text, formatted text and invitations stay in
//! the database and are read when a message is opened. Searching the text uses
//! the database's full-text index. Saving writes only the rows that changed.

use crate::data::{data_dir, Message};
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// What each saved row looked like (a fingerprint per message id).
static SAVED: Mutex<Option<HashMap<i64, u64>>> = Mutex::new(None);
/// For messages in memory that are also saved: their folder, read and flagged
/// as saved (so the counts read from the database can be put right for
/// changes not saved yet).
static STATE: Mutex<Option<HashMap<i64, (String, bool, bool)>>> = Mutex::new(None);
/// Goes up with each save (counts read from the database are kept until then).
pub static SAVES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Raised when the search index must be built again from the stored mail.
const SEARCH_VERSION: &str = "2";

// A connection to the database per thread, kept for the whole run: the
// window's queries never wait behind the mail checks' (the database lets
// several read at once, and one write while they do).
thread_local! {
    static CONN: std::cell::RefCell<Option<(PathBuf, Connection)>> = const { std::cell::RefCell::new(None) };
}
/// Setting the database up (the first time, or after an update) is done by
/// one thread; the others wait for it.
static SETTING_UP: Mutex<()> = Mutex::new(());

fn with_conn<T>(path: &Path, f: impl FnOnce(&mut Connection) -> Result<T, String>) -> Result<T, String> {
    CONN.with(|cell| {
        let mut held = cell.borrow_mut();
        if held.as_ref().map(|(p, _)| p != path).unwrap_or(true) {
            let c = {
                let _one = SETTING_UP.lock().unwrap();
                open_at(path)?
            };
            *held = Some((path.to_path_buf(), c));
        }
        f(&mut held.as_mut().unwrap().1)
    })
}

/// Sender, recipients and copies, for searching.
fn people(m: &Message) -> String {
    format!("{} {} {} {}", m.from_name, m.from_email, m.to, m.cc)
}

pub fn db_path() -> PathBuf {
    data_dir().join("mail.sqlite")
}

pub fn attachments_root() -> PathBuf {
    data_dir().join("attachments")
}

pub fn attachment_dir(id: i64) -> PathBuf {
    attachments_root().join(id.to_string())
}

/// A file name that is safe on every system.
pub fn safe_name(name: &str) -> String {
    let n: String = name.chars().map(|c| if "/\\:*?\"<>|".contains(c) || c.is_control() { '_' } else { c }).collect();
    let n = n.trim().trim_matches('.').to_string();
    if n.is_empty() {
        "attachment".into()
    } else {
        n
    }
}

/// The stored copy of one attachment, if it's been saved already.
pub fn attachment_file(id: i64, name: &str) -> Option<PathBuf> {
    let p = attachment_dir(id).join(safe_name(name));
    p.is_file().then_some(p)
}

/// Keeps one attachment as a file; returns where it is.
pub fn store_attachment(id: i64, name: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    let dir = attachment_dir(id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let p = dir.join(safe_name(name));
    let tmp = dir.join(format!(".{}.part", safe_name(name)));
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &p).map_err(|e| e.to_string())?;
    Ok(p)
}

/// Saves every attachment found in a whole original message.
pub fn store_attachments_from(id: i64, raw: &[u8], names: &[String]) {
    for name in names {
        if attachment_file(id, name).is_none() {
            if let Some((bytes, _)) = crate::mime::extract_attachment(raw, name) {
                let _ = store_attachment(id, name, &bytes);
            }
        }
    }
}

fn open_at(path: &Path) -> Result<Connection, String> {
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let c = Connection::open(path).map_err(|e| e.to_string())?;
    // (Another thread writing: waited for, not an error.)
    let _ = c.busy_timeout(std::time::Duration::from_secs(15));
    c.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         -- (A row replaced, as messages with their text are saved, counts as
         -- deleted too, so the folder counts' triggers see it go.)
         PRAGMA recursive_triggers=ON;
         CREATE TABLE IF NOT EXISTS messages(
             id INTEGER PRIMARY KEY,
             folder TEXT NOT NULL,
             source TEXT NOT NULL DEFAULT '',
             uid INTEGER NOT NULL DEFAULT 0,
             date INTEGER NOT NULL DEFAULT 0,
             from_name TEXT NOT NULL DEFAULT '',
             from_email TEXT NOT NULL DEFAULT '',
             to_addr TEXT NOT NULL DEFAULT '',
             cc TEXT NOT NULL DEFAULT '',
             subject TEXT NOT NULL DEFAULT '',
             body TEXT NOT NULL DEFAULT '',
             html TEXT NOT NULL DEFAULT '',
             invite TEXT NOT NULL DEFAULT '',
             read INTEGER NOT NULL DEFAULT 0,
             flagged INTEGER NOT NULL DEFAULT 0,
             attachments TEXT NOT NULL DEFAULT '[]',
             raw_ref TEXT NOT NULL DEFAULT '',
             remind_at INTEGER NOT NULL DEFAULT 0,
             message_id TEXT NOT NULL DEFAULT '',
             partial INTEGER NOT NULL DEFAULT 0,
             in_reply_to TEXT NOT NULL DEFAULT '',
             refs TEXT NOT NULL DEFAULT '',
             reply_to TEXT NOT NULL DEFAULT ''
         );
         CREATE INDEX IF NOT EXISTS messages_by_folder ON messages(folder, date);
         CREATE INDEX IF NOT EXISTS messages_by_server ON messages(source, folder, uid);
         CREATE INDEX IF NOT EXISTS messages_by_message_id ON messages(message_id);
         CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
         -- Full-text search over subject, people and text (words, any language).
         CREATE VIRTUAL TABLE IF NOT EXISTS search USING fts5(subject, people, body,
             content='', contentless_delete=1, tokenize='unicode61 remove_diacritics 2');",
    )
    .map_err(|e| e.to_string())?;
    // Files made by earlier versions lack the newer columns.
    for col in ["in_reply_to", "refs", "reply_to", "preview"] {
        let _ = c.execute_batch(&format!("ALTER TABLE messages ADD COLUMN {col} TEXT NOT NULL DEFAULT ''"));
    }
    let _ = c.execute_batch("ALTER TABLE messages ADD COLUMN flag_color INTEGER NOT NULL DEFAULT 0");
    // Flag colours other than red, in a small table of their own: read at start
    // in a moment (a column of the messages would mean remaking the list's index
    // or reading scattered rows of a file of gigabytes).
    c.execute_batch("CREATE TABLE IF NOT EXISTS flag_colors(id INTEGER PRIMARY KEY, color INTEGER NOT NULL)").map_err(|e| e.to_string())?;
    // The junk filter: tokens seen in junk and in good mail, and which way
    // each message was learned (so a change of mind takes it back).
    c.execute_batch(
        "CREATE TABLE IF NOT EXISTS junk_tokens(token TEXT PRIMARY KEY, spam INTEGER NOT NULL, ham INTEGER NOT NULL) WITHOUT ROWID;
         CREATE TABLE IF NOT EXISTS junk_learned(id INTEGER PRIMARY KEY, spam INTEGER NOT NULL);",
    )
    .map_err(|e| e.to_string())?;
    // Mailing lists' headers (how to unsubscribe), kept as mail arrives.
    c.execute_batch("CREATE TABLE IF NOT EXISTS list_heads(id INTEGER PRIMARY KEY, heads TEXT NOT NULL)").map_err(|e| e.to_string())?;
    // (Mail stored before then wasn't looked at: its headers are read when it's opened.)
    c.execute_batch("INSERT OR IGNORE INTO meta(key, value) SELECT 'list_heads_after', CAST(COALESCE(MAX(id), 0) AS TEXT) FROM messages").map_err(|e| e.to_string())?;
    // Mail stored before there was a search index (or by an older Almanac
    // since it was built): index all of it once.
    let built: Option<String> = c.query_row("SELECT value FROM meta WHERE key = 'search'", [], |r| r.get(0)).ok();
    if built.as_deref() != Some(SEARCH_VERSION) {
        c.execute_batch(
            "BEGIN;
             DELETE FROM search;
             INSERT INTO search(rowid, subject, people, body)
                 SELECT id, subject, from_name || ' ' || from_email || ' ' || to_addr || ' ' || cc, body FROM messages;
             COMMIT;",
        )
        .map_err(|e| e.to_string())?;
        c.execute("INSERT OR REPLACE INTO meta(key, value) VALUES('search', ?1)", params![SEARCH_VERSION]).map_err(|e| e.to_string())?;
    }
    // The message list's fields in an index of their own, every message with its
    // preview line stored: starting up reads just that (a few tens of MB), not
    // the rows that also hold all the text and formatting (gigabytes for a big
    // mailbox, which made starting slow). Done once for older files.
    let listed: Option<String> = c.query_row("SELECT value FROM meta WHERE key = 'list'", [], |r| r.get(0)).ok();
    // (Version 2 also held the flag colour; reading that from the rows of the
    // few flagged messages instead spares remaking the index, which takes
    // minutes on a big mailbox: the version 1 index serves as it is.)
    if listed.as_deref() != Some("1") && listed.as_deref() != Some("2") {
        let missing: Vec<(i64, String)> = {
            let mut st = c.prepare("SELECT id, substr(body, 1, 1200) FROM messages WHERE preview = '' AND body <> ''").map_err(|e| e.to_string())?;
            let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).map_err(|e| e.to_string())?;
            rows.filter_map(Result::ok).collect()
        };
        let tx = c.unchecked_transaction().map_err(|e| e.to_string())?;
        {
            let mut up = tx.prepare("UPDATE messages SET preview = ?1 WHERE id = ?2").map_err(|e| e.to_string())?;
            for (id, body) in &missing {
                up.execute(params![crate::data::strip_preview(body), id]).map_err(|e| e.to_string())?;
            }
        }
        // (Made again when the list gains a field.)
        tx.execute_batch(&format!("DROP INDEX IF EXISTS messages_for_list; CREATE INDEX messages_for_list ON messages({LIST_COLUMNS});"))
            .map_err(|e| e.to_string())?;
        tx.execute("INSERT OR REPLACE INTO meta(key, value) VALUES('list', '1')", []).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
    }
    // Mail whose "plain text" was really HTML (tags shown in the message list's
    // preview): its text made from the HTML, once.
    let fixed: Option<String> = c.query_row("SELECT value FROM meta WHERE key = 'html_text'", [], |r| r.get(0)).ok();
    if fixed.as_deref() != Some("1") {
        let wrong: Vec<(i64, String, String)> = {
            let mut st = c
                .prepare("SELECT id, body, html FROM messages WHERE ltrim(body) LIKE '<%' OR ltrim(preview) LIKE '<%'")
                .map_err(|e| e.to_string())?;
            let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))).map_err(|e| e.to_string())?;
            rows.filter_map(Result::ok).filter(|(_, body, _)| crate::mime::looks_like_html(body)).collect()
        };
        let tx = c.unchecked_transaction().map_err(|e| e.to_string())?;
        {
            let mut up = tx.prepare("UPDATE messages SET body = ?1, preview = ?2 WHERE id = ?3").map_err(|e| e.to_string())?;
            for (id, body, html) in &wrong {
                let text = crate::mime::html_to_text(if html.is_empty() { body } else { html });
                up.execute(params![text, crate::data::strip_preview(&text), id]).map_err(|e| e.to_string())?;
            }
        }
        tx.execute("INSERT OR REPLACE INTO meta(key, value) VALUES('html_text', '1')", []).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
    }
    // Each folder's counts (all, unread, flagged) in a small table the
    // database keeps right itself as messages are added, changed or deleted:
    // the sidebar reads a few rows, however much mail there is. Counted once.
    // (Version 2: counted again, as rows replaced weren't seen going before.)
    let counted: Option<String> = c.query_row("SELECT value FROM meta WHERE key = 'counts'", [], |r| r.get(0)).ok();
    if counted.as_deref() != Some("2") {
        let _t = crate::Timed::new("counting the mail once (for the sidebar)");
        c.execute_batch(
            "BEGIN;
             DROP TRIGGER IF EXISTS counts_add;
             DROP TRIGGER IF EXISTS counts_drop;
             DROP TRIGGER IF EXISTS counts_change;
             DROP TABLE IF EXISTS folder_counts;
             CREATE TABLE folder_counts(folder TEXT PRIMARY KEY, total INTEGER NOT NULL DEFAULT 0,
                                        unread INTEGER NOT NULL DEFAULT 0, flagged INTEGER NOT NULL DEFAULT 0);
             INSERT INTO folder_counts(folder, total, unread, flagged)
                 SELECT folder, COUNT(*), SUM(read = 0), SUM(flagged = 1) FROM messages INDEXED BY messages_for_list GROUP BY folder;
             CREATE TRIGGER counts_add AFTER INSERT ON messages BEGIN
                 INSERT INTO folder_counts(folder) VALUES (NEW.folder) ON CONFLICT(folder) DO NOTHING;
                 UPDATE folder_counts SET total = total + 1, unread = unread + (NEW.read = 0), flagged = flagged + (NEW.flagged = 1)
                     WHERE folder = NEW.folder;
             END;
             CREATE TRIGGER counts_drop AFTER DELETE ON messages BEGIN
                 UPDATE folder_counts SET total = total - 1, unread = unread - (OLD.read = 0), flagged = flagged - (OLD.flagged = 1)
                     WHERE folder = OLD.folder;
             END;
             CREATE TRIGGER counts_change AFTER UPDATE OF folder, read, flagged ON messages BEGIN
                 UPDATE folder_counts SET total = total - 1, unread = unread - (OLD.read = 0), flagged = flagged - (OLD.flagged = 1)
                     WHERE folder = OLD.folder;
                 INSERT INTO folder_counts(folder) VALUES (NEW.folder) ON CONFLICT(folder) DO NOTHING;
                 UPDATE folder_counts SET total = total + 1, unread = unread + (NEW.read = 0), flagged = flagged + (NEW.flagged = 1)
                     WHERE folder = NEW.folder;
             END;
             INSERT OR REPLACE INTO meta(key, value) VALUES('counts', '2');
             COMMIT;",
        )
        .map_err(|e| e.to_string())?;
    }
    convs_ready(&c)?;
    // Each folder's messages by date in the small table too: pages of a long
    // list without conversations read from there (a few ms anywhere in it).
    let _ = c.execute_batch("CREATE INDEX IF NOT EXISTS msg_threads_by_date ON msg_threads(folder, date, id)");
    Ok(c)
}

/// A message's conversation, from its headers: the first message it refers
/// to (References, else In-Reply-To), else its own Message-ID.
const THREAD_KEY: &str = "lower(CASE
    WHEN instr(refs, '<') > 0 THEN substr(refs, instr(refs, '<') + 1, instr(substr(refs, instr(refs, '<')), '>') - 2)
    WHEN instr(in_reply_to, '<') > 0 THEN substr(in_reply_to, instr(in_reply_to, '<') + 1, instr(substr(in_reply_to, instr(in_reply_to, '<')), '>') - 2)
    WHEN instr(message_id, '<') > 0 THEN substr(message_id, instr(message_id, '<') + 1, instr(substr(message_id, instr(message_id, '<')), '>') - 2)
    WHEN trim(message_id) <> '' THEN trim(message_id)
    ELSE '#' || date || '|' || from_email || '|' || subject END)";

/// Each message's conversation (folder, thread key, date, read) in a small
/// table of its own, and each folder's conversations (newest message, how
/// many, how many unread) in another, both kept right by the database itself,
/// so the message list can be read a page at a time anywhere with
/// conversations on. Here only what's instant: the tables and the triggers
/// that keep them right from now on. Filling them in for the mail already
/// here is `build_conversations`'s, in the background, a little at a time.
fn convs_ready(c: &Connection) -> Result<(), String> {
    let state: Option<String> = c.query_row("SELECT value FROM meta WHERE key = 'convs'", [], |r| r.get(0)).ok();
    // (Triggers of the version before this one failed saves: made again.)
    let fixed: Option<String> = c.query_row("SELECT value FROM meta WHERE key = 'threads_triggers'", [], |r| r.get(0)).ok();
    if (state.as_deref() == Some("2") || state.as_deref() == Some("building")) && fixed.as_deref() == Some("2") {
        return Ok(());
    }
    if state.as_deref() == Some("2") || state.as_deref() == Some("building") {
        c.execute_batch(&format!("BEGIN; {THREADS_TRIGGERS} INSERT OR REPLACE INTO meta(key, value) VALUES('threads_triggers', '2'); COMMIT;"))
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    let has_tk: bool = c.prepare("SELECT tk FROM messages LIMIT 0").is_ok();
    if !has_tk {
        c.execute_batch(&format!("ALTER TABLE messages ADD COLUMN tk TEXT GENERATED ALWAYS AS ({THREAD_KEY}) VIRTUAL")).map_err(|e| e.to_string())?;
    }
    c.execute_batch(&format!(
        "BEGIN;
         -- (The first version's: an index over the messages, made in one go.)
         DROP TRIGGER IF EXISTS convs_add;
         DROP TRIGGER IF EXISTS convs_drop;
         DROP TRIGGER IF EXISTS convs_change;
         DROP INDEX IF EXISTS messages_by_thread;
         DROP TABLE IF EXISTS convs;
         CREATE TABLE IF NOT EXISTS msg_threads(id INTEGER PRIMARY KEY, folder TEXT NOT NULL, tk TEXT NOT NULL,
                                                date INTEGER NOT NULL, read INTEGER NOT NULL);
         CREATE INDEX IF NOT EXISTS msg_threads_by_conv ON msg_threads(folder, tk, date);
{THREADS_TRIGGERS}
         INSERT OR REPLACE INTO meta(key, value) VALUES('threads_triggers', '2');
         INSERT OR REPLACE INTO meta(key, value) VALUES('convs', 'building');
         COMMIT;"
    ))
    .map_err(|e| e.to_string())
}

/// The triggers that keep each message's conversation (msg_threads) right.
const THREADS_TRIGGERS: &str = "
         DROP TRIGGER IF EXISTS threads_add;
         DROP TRIGGER IF EXISTS threads_drop;
         DROP TRIGGER IF EXISTS threads_change;
         -- (Delete, then insert: inside a trigger an outer statement's own
         -- conflict handling would win over OR REPLACE, and saving failed.)
         CREATE TRIGGER threads_add AFTER INSERT ON messages BEGIN
             DELETE FROM msg_threads WHERE id = NEW.id;
             INSERT INTO msg_threads VALUES (NEW.id, NEW.folder, NEW.tk, NEW.date, NEW.read);
         END;
         CREATE TRIGGER threads_drop AFTER DELETE ON messages BEGIN
             DELETE FROM msg_threads WHERE id = OLD.id;
         END;
         CREATE TRIGGER threads_change AFTER UPDATE OF folder, read, date, refs, in_reply_to, message_id ON messages BEGIN
             DELETE FROM msg_threads WHERE id = NEW.id;
             INSERT INTO msg_threads VALUES (NEW.id, NEW.folder, NEW.tk, NEW.date, NEW.read);
         END;
";

/// Whether each folder's conversations are all worked out (so the list can be
/// read a page at a time with conversations on).
pub fn conversations_ready() -> bool {
    static READY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if READY.load(std::sync::atomic::Ordering::Relaxed) {
        return true;
    }
    let path = db_path();
    if !path.exists() {
        return false;
    }
    let ok = with_conn(&path, |c| Ok(c.query_row("SELECT value FROM meta WHERE key = 'convs'", [], |r| r.get::<_, String>(0)).ok().as_deref() == Some("2")))
        .unwrap_or(false);
    if ok {
        READY.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    ok
}

/// Takes hidden parts (preview lines, comments) out of the text of mail
/// already here whose text came from its HTML (and so out of the search),
/// once, in the background: a few hundred messages at a time with pauses
/// between, carrying on where it stopped after a restart.
pub fn drop_hidden_text() {
    let path = db_path();
    if !path.exists() {
        return;
    }
    let state: Option<String> = with_conn(&path, |c| Ok(c.query_row("SELECT value FROM meta WHERE key = 'hidden_text_2'", [], |r| r.get(0)).ok())).unwrap_or(None);
    if state.as_deref() == Some("done") {
        return;
    }
    let _t = crate::Timed::new("taking hidden text out of the search (once, in the background)");
    let started = std::time::Instant::now();
    let mut last: i64 = state.and_then(|s| s.parse().ok()).unwrap_or(0);
    let (mut looked, mut changed) = (0usize, 0usize);
    loop {
        let step = with_conn(&path, |c| {
            let rows: Vec<(i64, String, String, String, String, String)> = {
                let mut st = c
                    .prepare_cached(
                        "SELECT id, body, html, preview, subject, from_name || ' ' || from_email || ' ' || to_addr || ' ' || cc
                         FROM messages WHERE id > ?1 AND html <> '' ORDER BY id LIMIT 300",
                    )
                    .map_err(|e| e.to_string())?;
                let rows = st
                    .query_map(params![last], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))
                    .map_err(|e| e.to_string())?;
                rows.flatten().collect()
            };
            let Some(end) = rows.last().map(|r| r.0) else { return Ok(None) };
            let mut fixes = Vec::new();
            for (id, body, html, preview, subject, people) in &rows {
                let shown = crate::mime::html_text(html, true);
                // (Also text left empty by the first, too eager version of this.)
                if shown != *body && (crate::mime::html_text(html, false) == *body || (body.trim().is_empty() && !shown.trim().is_empty())) {
                    let was = crate::data::strip_preview(body);
                    let preview = if *preview == was { crate::data::strip_preview(&shown) } else { preview.clone() };
                    fixes.push((*id, shown, preview, subject.clone(), people.clone()));
                }
            }
            let tx = c.transaction().map_err(|e| e.to_string())?;
            {
                let mut up = tx.prepare_cached("UPDATE messages SET body = ?2, preview = ?3 WHERE id = ?1").map_err(|e| e.to_string())?;
                let mut unindex = tx.prepare_cached("DELETE FROM search WHERE rowid = ?1").map_err(|e| e.to_string())?;
                let mut index = tx.prepare_cached("INSERT INTO search(rowid, subject, people, body) VALUES(?1, ?2, ?3, ?4)").map_err(|e| e.to_string())?;
                for (id, body, preview, subject, people) in &fixes {
                    up.execute(params![id, body, preview]).map_err(|e| e.to_string())?;
                    unindex.execute(params![id]).map_err(|e| e.to_string())?;
                    index.execute(params![id, subject, people, body]).map_err(|e| e.to_string())?;
                }
            }
            tx.execute("INSERT OR REPLACE INTO meta(key, value) VALUES('hidden_text_2', ?1)", params![end.to_string()]).map_err(|e| e.to_string())?;
            tx.commit().map_err(|e| e.to_string())?;
            Ok(Some((end, rows.len(), fixes.len())))
        });
        match step {
            Ok(Some((end, n, f))) => {
                last = end;
                looked += n;
                changed += f;
                if looked % 30_000 < 300 {
                    eprintln!("almanac: hidden text: {looked} looked at, {changed} changed ({}s)", started.elapsed().as_secs());
                }
            }
            Ok(None) => break,
            Err(e) => {
                eprintln!("almanac: taking hidden text out: {e}");
                return;
            }
        }
        // (Mail checks and searches go first.)
        std::thread::sleep(std::time::Duration::from_millis(40));
    }
    let _ = with_conn(&path, |c| c.execute("INSERT OR REPLACE INTO meta(key, value) VALUES('hidden_text_2', 'done')", []).map_err(|e| e.to_string()));
    eprintln!("almanac: hidden text taken out of the search: {looked} looked at, {changed} changed, in {}s", started.elapsed().as_secs());
}

/// Whether messages can be looked up by their conversation's key (the index
/// `index_thread_keys` makes): a message's conversation is then found in ms,
/// not by reading every message's References (seconds on a big mailbox).
pub fn thread_keys_ready() -> bool {
    THREAD_KEYS.load(std::sync::atomic::Ordering::Relaxed) && conversations_ready()
}
pub(crate) static THREAD_KEYS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Makes the index of conversations by key (once; a second or so on a few
/// hundred thousand messages), after the conversations are worked out. Run on
/// a thread of its own.
pub fn index_thread_keys() {
    let path = db_path();
    if !path.exists() || !conversations_ready() {
        return;
    }
    let done = with_conn(&path, |c| {
        let there: bool = c
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'msg_threads_by_tk'", [], |r| r.get::<_, i64>(0))
            .map_err(|e| e.to_string())?
            > 0;
        if !there {
            let _t = crate::Timed::new("indexing conversations by key (once, in the background)");
            c.execute_batch("CREATE INDEX IF NOT EXISTS msg_threads_by_tk ON msg_threads(tk, date)").map_err(|e| e.to_string())?;
        }
        Ok(())
    });
    match done {
        Ok(()) => THREAD_KEYS.store(true, std::sync::atomic::Ordering::Relaxed),
        Err(e) => eprintln!("almanac: indexing conversations: {e}"),
    }
}

/// The messages of the conversations with these keys (lower case), from
/// `since` on, oldest first, at most `limit`.
pub fn by_thread_keys(keys: &[String], since: i64, limit: i64) -> Vec<Message> {
    if keys.is_empty() {
        return Vec::new();
    }
    let mut args: Vec<Arg> = vec![Arg::Integer(since)];
    let marks: Vec<String> = keys
        .iter()
        .map(|k| {
            args.push(Arg::Text(k.to_lowercase()));
            format!("?{}", args.len())
        })
        .collect();
    let path = db_path();
    if !path.exists() {
        return Vec::new();
    }
    // (Ids from the small table, then those messages by id: `rows` reads the
    // whole list's index.)
    let ids: Vec<i64> = with_conn(&path, |c| {
        let sql = format!(
            "SELECT id FROM msg_threads INDEXED BY msg_threads_by_tk WHERE tk IN ({}) AND date >= ?1 ORDER BY date ASC LIMIT {limit}",
            marks.join(", ")
        );
        let mut st = c.prepare_cached(&sql).map_err(|e| e.to_string())?;
        let found = st.query_map(rusqlite::params_from_iter(args.iter()), |r| r.get(0)).map_err(|e| e.to_string())?;
        Ok(found.filter_map(Result::ok).collect())
    })
    .unwrap_or_default();
    by_ids(&ids)
}

/// Works out the conversations of the mail already here (once), a couple of
/// thousand messages at a time so nothing else waits on the database for
/// long; then each folder's conversations, in one short step. Run on a
/// thread of its own; returns at once when it's done already.
pub fn build_conversations() {
    let path = db_path();
    if !path.exists() || conversations_ready() {
        return;
    }
    let _t = crate::Timed::new("working out the mail's conversations (in the background)");
    let mut last: i64 = with_conn(&path, |c| c.query_row("SELECT COALESCE(MAX(id), 0) FROM msg_threads", [], |r| r.get(0)).map_err(|e| e.to_string())).unwrap_or(0);
    // (Messages before the newest in the table but missing from it: a first
    // run that stopped part way, so it starts over from the beginning.)
    if last > 0 {
        let gap: i64 = with_conn(&path, |c| {
            c.query_row("SELECT COUNT(*) FROM messages WHERE id <= ?1 AND id NOT IN (SELECT id FROM msg_threads)", params![last], |r| r.get(0))
                .map_err(|e| e.to_string())
        })
        .unwrap_or(0);
        if gap > 0 {
            last = 0;
        }
    }
    loop {
        let step = with_conn(&path, |c| {
            let upto: Option<i64> =
                c.query_row("SELECT id FROM messages WHERE id > ?1 ORDER BY id LIMIT 1 OFFSET 1999", params![last], |r| r.get(0)).ok();
            let upto = match upto {
                Some(u) => u,
                None => c.query_row("SELECT COALESCE(MAX(id), ?1) FROM messages", params![last], |r| r.get(0)).map_err(|e| e.to_string())?,
            };
            // (Rows a trigger put in meanwhile are newer: kept.)
            c.execute(
                "INSERT OR IGNORE INTO msg_threads SELECT id, folder, tk, date, read FROM messages WHERE id > ?1 AND id <= ?2",
                params![last, upto],
            )
            .map_err(|e| e.to_string())?;
            Ok(upto)
        });
        match step {
            Ok(upto) if upto > last => last = upto,
            Ok(_) => break,
            Err(e) => {
                eprintln!("almanac: working out conversations: {e}");
                return;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let redo = |x: &str| {
        format!(
            "DELETE FROM convs WHERE folder = {x}.folder AND tkey = {x}.tk;
             INSERT INTO convs(folder, tkey, latest, top_id, n, unread)
                 SELECT folder, tk, MAX(date), id, COUNT(*), TOTAL(read = 0) FROM msg_threads INDEXED BY msg_threads_by_conv
                 WHERE folder = {x}.folder AND tk = {x}.tk GROUP BY folder, tk;"
        )
    };
    let done = with_conn(&path, |c| {
        c.execute_batch(&format!(
            "BEGIN IMMEDIATE;
             DROP TABLE IF EXISTS convs;
             CREATE TABLE convs(folder TEXT NOT NULL, tkey TEXT NOT NULL, latest INTEGER NOT NULL, top_id INTEGER NOT NULL,
                                n INTEGER NOT NULL, unread INTEGER NOT NULL, PRIMARY KEY(folder, tkey)) WITHOUT ROWID;
             INSERT INTO convs(folder, tkey, latest, top_id, n, unread)
                 SELECT folder, tk, MAX(date), id, COUNT(*), TOTAL(read = 0) FROM msg_threads INDEXED BY msg_threads_by_conv GROUP BY folder, tk;
             CREATE INDEX convs_by_latest ON convs(folder, latest, top_id);
             DROP TRIGGER IF EXISTS convs_add;
             DROP TRIGGER IF EXISTS convs_drop;
             DROP TRIGGER IF EXISTS convs_change;
             CREATE TRIGGER convs_add AFTER INSERT ON msg_threads BEGIN {} END;
             CREATE TRIGGER convs_drop AFTER DELETE ON msg_threads BEGIN {} END;
             CREATE TRIGGER convs_change AFTER UPDATE ON msg_threads BEGIN {} {} END;
             INSERT OR REPLACE INTO meta(key, value) VALUES('convs', '2');
             COMMIT;",
            redo("NEW"),
            redo("OLD"),
            redo("OLD"),
            redo("NEW")
        ))
        .map_err(|e| e.to_string())
    });
    if let Err(e) = done {
        eprintln!("almanac: working out conversations: {e}");
    }
}

/// A page of a mailbox's conversations, newest first: (folder, thread key)
/// of each, skipping `offset`. `unread_or`: only those with unread mail (or
/// the conversation of that message, kept while it's open).
pub fn conv_page(folders: &[String], unread_or: Option<i64>, ascending: bool, offset: i64, limit: i64) -> Vec<(String, String)> {
    let path = db_path();
    if !path.exists() || folders.is_empty() {
        return Vec::new();
    }
    with_conn(&path, |c| {
        let (filter, args) = conv_filter(c, folders, unread_or);
        let dir = if ascending { "ASC" } else { "DESC" };
        let sql = format!("SELECT folder, tkey FROM convs WHERE {filter} ORDER BY latest {dir}, top_id {dir} LIMIT {limit} OFFSET {offset}");
        let mut st = c.prepare_cached(&sql).map_err(|e| e.to_string())?;
        let rows = st.query_map(rusqlite::params_from_iter(args.iter()), |r| Ok((r.get(0)?, r.get(1)?))).map_err(|e| e.to_string())?;
        Ok(rows.filter_map(Result::ok).collect())
    })
    .unwrap_or_default()
}

/// How many conversations a mailbox has (and with unread mail).
pub fn conv_count(folders: &[String], unread_or: Option<i64>) -> (i64, i64) {
    let path = db_path();
    if !path.exists() || folders.is_empty() {
        return (0, 0);
    }
    with_conn(&path, |c| {
        let (filter, args) = conv_filter(c, folders, unread_or);
        let n: i64 = c.query_row(&format!("SELECT COUNT(*) FROM convs WHERE {filter}"), rusqlite::params_from_iter(args.iter()), |r| r.get(0)).map_err(|e| e.to_string())?;
        let (filter, args) = conv_filter(c, folders, None);
        let u: i64 = c
            .query_row(&format!("SELECT COUNT(*) FROM convs WHERE {filter} AND unread > 0"), rusqlite::params_from_iter(args.iter()), |r| r.get(0))
            .map_err(|e| e.to_string())?;
        Ok((n, u))
    })
    .unwrap_or((0, 0))
}

/// Where message `id`'s conversation is in a mailbox's list (how many come
/// before it), or None when it's not in it.
pub fn conv_position(folders: &[String], unread_or: Option<i64>, ascending: bool, id: i64) -> Option<i64> {
    let path = db_path();
    if !path.exists() || folders.is_empty() {
        return None;
    }
    with_conn(&path, |c| {
        let (folder, tk): (String, String) = c.query_row("SELECT folder, tk FROM msg_threads WHERE id = ?1", params![id], |r| Ok((r.get(0)?, r.get(1)?))).map_err(|e| e.to_string())?;
        if !folders.contains(&folder) {
            return Err("not here".into());
        }
        let (latest, top): (i64, i64) = c
            .query_row("SELECT latest, top_id FROM convs WHERE folder = ?1 AND tkey = ?2", params![folder, tk], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| e.to_string())?;
        let (filter, mut args) = conv_filter(c, folders, unread_or);
        args.push(Arg::Integer(latest));
        args.push(Arg::Integer(top));
        let (a, b) = (args.len() - 1, args.len());
        c.query_row(
            &if ascending {
                format!("SELECT COUNT(*) FROM convs WHERE {filter} AND (latest < ?{a} OR (latest = ?{a} AND top_id < ?{b}))")
            } else {
                format!("SELECT COUNT(*) FROM convs WHERE {filter} AND (latest > ?{a} OR (latest = ?{a} AND top_id > ?{b}))")
            },
            rusqlite::params_from_iter(args.iter()),
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())
    })
    .ok()
}

fn conv_filter(c: &Connection, folders: &[String], unread_or: Option<i64>) -> (String, Vec<Arg>) {
    let mut args: Vec<Arg> = folders.iter().map(|f| Arg::Text(f.clone())).collect();
    let marks = (1..=folders.len()).map(|i| format!("?{i}")).collect::<Vec<_>>().join(",");
    let mut filter = format!("folder IN ({marks})");
    if let Some(keep) = unread_or {
        let kept: Option<(String, String)> = c.query_row("SELECT folder, tk FROM msg_threads WHERE id = ?1", params![keep], |r| Ok((r.get(0)?, r.get(1)?))).ok();
        match kept {
            Some((f, t)) => {
                args.push(Arg::Text(f));
                args.push(Arg::Text(t));
                filter = format!("{filter} AND (unread > 0 OR (folder = ?{} AND tkey = ?{}))", args.len() - 1, args.len());
            }
            None => filter = format!("{filter} AND unread > 0"),
        }
    }
    (filter, args)
}

/// A list's order as SQL: over messages, and over conversations (convs `c`,
/// by their newest message).
pub fn list_orders(sort_by: &str, ascending: bool) -> (String, String) {
    let dir = if ascending { "ASC" } else { "DESC" };
    let key = match sort_by {
        "from" => "lower(CASE WHEN from_name = '' THEN from_email ELSE from_name END)",
        "subject" => "lower(subject)",
        "to" => "lower(to_addr)",
        "attachments" => "(attachments <> '[]')",
        "flags" => "flagged",
        "unread" => "(read = 0)",
        _ => "",
    };
    if key.is_empty() {
        return (format!("date {dir}, id {dir}"), format!("c.latest {dir}, c.top_id {dir}"));
    }
    // (Over conversations: the newest message's key, that message looked up
    // by its row; see filtered_convs.)
    let conv_key = if sort_by == "unread" { "(c.unread > 0)".to_string() } else { format!("{key} /*s.k*/") };
    (format!("{key} {dir}, date DESC, id DESC"), format!("{conv_key} {dir}, c.latest DESC, c.top_id DESC"))
}

/// The conversations with a message the filter matches (SQL over messages),
/// with `s.k` to sort them by when the order needs it.
fn filtered_convs(filter: &str, order: &str) -> String {
    let join = if order.contains("/*s.k*/") { "JOIN messages ON messages.id = c.top_id" } else { "" };
    format!(
        "convs c {join} WHERE (c.folder, c.tkey) IN (SELECT t.folder, t.tk FROM msg_threads t WHERE t.id IN
             (SELECT id FROM messages INDEXED BY messages_for_list WHERE {filter}))"
    )
}

/// A search of mailboxes, by date: driven from the full-text index (`expr`,
/// see `match_expr`) and the small table of each message's folder, date and
/// conversation, never the whole list's index (tens of ms on a mailbox of
/// hundreds of thousands, where reading every row took a second).
/// `unread_or`: only unread messages (or that one). Gives the rows in order
/// (message ids, or conversations (folder, thread key) with `threaded`) and
/// how many of them have unread mail.
pub enum SearchRows {
    Messages(Vec<i64>),
    Conversations(Vec<(String, String)>),
}

pub fn search_rows(folders: &[String], expr: &str, extra: &[i64], unread_or: Option<i64>, threaded: bool, ascending: bool) -> (SearchRows, i64) {
    let empty = || if threaded { SearchRows::Conversations(Vec::new()) } else { SearchRows::Messages(Vec::new()) };
    let path = db_path();
    if !path.exists() || folders.is_empty() || expr.is_empty() {
        return (empty(), 0);
    }
    with_conn(&path, |c| {
        let (sql, args) = search_rows_sql(folders, expr, extra, unread_or, threaded, ascending);
        let mut st = c.prepare_cached(&sql).map_err(|e| e.to_string())?;
        let mut unread = 0i64;
        if threaded {
            let rows = st
                .query_map(rusqlite::params_from_iter(args.iter()), |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?)))
                .map_err(|e| e.to_string())?;
            let mut out = Vec::new();
            for (f, t, u) in rows.filter_map(Result::ok) {
                unread += (u > 0) as i64;
                out.push((f, t));
            }
            Ok((SearchRows::Conversations(out), unread))
        } else {
            let rows = st.query_map(rusqlite::params_from_iter(args.iter()), |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))).map_err(|e| e.to_string())?;
            let mut out = Vec::new();
            for (id, read) in rows.filter_map(Result::ok) {
                unread += (read == 0) as i64;
                out.push(id);
            }
            Ok((SearchRows::Messages(out), unread))
        }
    })
    .unwrap_or_else(|_| (empty(), 0))
}

fn search_rows_sql(folders: &[String], expr: &str, extra: &[i64], unread_or: Option<i64>, threaded: bool, ascending: bool) -> (String, Vec<Arg>) {
    let mut args: Vec<Arg> = vec![Arg::Text(expr.to_string())];
    let marks: Vec<String> = folders
        .iter()
        .map(|f| {
            args.push(Arg::Text(f.clone()));
            format!("?{}", args.len())
        })
        .collect();
    // (With those the server's search found too: their text may not be here.)
    let words = if extra.is_empty() {
        "t.id IN (SELECT rowid FROM search WHERE search MATCH ?1)".to_string()
    } else {
        args.push(Arg::Text(serde_json::to_string(extra).unwrap_or_else(|_| "[]".into())));
        format!("(t.id IN (SELECT rowid FROM search WHERE search MATCH ?1) OR t.id IN (SELECT value FROM json_each(?{})))", args.len())
    };
    let mut found = format!("{words} AND t.folder IN ({})", marks.join(","));
    if let Some(k) = unread_or {
        args.push(Arg::Integer(k));
        found.push_str(&format!(" AND (t.read = 0 OR t.id = ?{})", args.len()));
    }
    let dir = if ascending { "ASC" } else { "DESC" };
    let sql = if threaded {
        format!(
            "SELECT c.folder, c.tkey, c.unread FROM convs c WHERE (c.folder, c.tkey) IN (SELECT t.folder, t.tk FROM msg_threads t WHERE {found})
             ORDER BY c.latest {dir}, c.top_id {dir}"
        )
    } else {
        format!("SELECT t.id, t.read FROM msg_threads t WHERE {found} ORDER BY t.date {dir}, t.id {dir}")
    };
    (sql, args)
}

/// A page of the conversations with a message the filter matches.
pub fn filtered_conv_page(filter: &str, args: &[Arg], order: &str, offset: i64, limit: i64) -> Vec<(String, String)> {
    let path = db_path();
    if !path.exists() {
        return Vec::new();
    }
    with_conn(&path, |c| {
        let sql = format!("SELECT c.folder, c.tkey FROM {} ORDER BY {order} LIMIT {limit} OFFSET {offset}", filtered_convs(filter, order));
        let mut st = c.prepare_cached(&sql).map_err(|e| e.to_string())?;
        let rows = st.query_map(rusqlite::params_from_iter(args.iter()), |r| Ok((r.get(0)?, r.get(1)?))).map_err(|e| e.to_string())?;
        Ok(rows.filter_map(Result::ok).collect())
    })
    .unwrap_or_default()
}

/// How many conversations have a message the filter matches (and with unread mail).
pub fn filtered_conv_count(filter: &str, args: &[Arg]) -> (i64, i64) {
    let path = db_path();
    if !path.exists() {
        return (0, 0);
    }
    with_conn(&path, |c| {
        let n: i64 = c
            .query_row(&format!("SELECT COUNT(*) FROM {}", filtered_convs(filter, "")), rusqlite::params_from_iter(args.iter()), |r| r.get(0))
            .map_err(|e| e.to_string())?;
        let u: i64 = c
            .query_row(&format!("SELECT COUNT(*) FROM {} AND c.unread > 0", filtered_convs(filter, "")), rusqlite::params_from_iter(args.iter()), |r| r.get(0))
            .map_err(|e| e.to_string())?;
        Ok((n, u))
    })
    .unwrap_or((0, 0))
}

/// The row of message `id`'s conversation among those the filter matches.
pub fn filtered_conv_position(filter: &str, args: &[Arg], order: &str, id: i64) -> Option<i64> {
    let path = db_path();
    if !path.exists() {
        return None;
    }
    with_conn(&path, |c| {
        let (folder, tk): (String, String) = c.query_row("SELECT folder, tk FROM msg_threads WHERE id = ?1", params![id], |r| Ok((r.get(0)?, r.get(1)?))).map_err(|e| e.to_string())?;
        let mut a = args.to_vec();
        a.push(Arg::Text(folder));
        a.push(Arg::Text(tk));
        let (f, t) = (a.len() - 1, a.len());
        c.query_row(
            &format!(
                "SELECT n FROM (SELECT c.folder, c.tkey, ROW_NUMBER() OVER (ORDER BY {order}) - 1 AS n FROM {}) WHERE folder = ?{f} AND tkey = ?{t}",
                filtered_convs(filter, order)
            ),
            rusqlite::params_from_iter(a.iter()),
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())
    })
    .ok()
}

/// The row of message `id` among the messages the filter matches.
pub fn filtered_position(filter: &str, args: &[Arg], order: &str, id: i64) -> Option<i64> {
    let path = db_path();
    if !path.exists() {
        return None;
    }
    with_conn(&path, |c| {
        let mut a = args.to_vec();
        a.push(Arg::Integer(id));
        let k = a.len();
        c.query_row(
            &format!("SELECT n FROM (SELECT id, ROW_NUMBER() OVER (ORDER BY {order}) - 1 AS n FROM messages INDEXED BY messages_for_list WHERE {filter}) WHERE id = ?{k}"),
            rusqlite::params_from_iter(a.iter()),
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())
    })
    .ok()
}

/// A page of a mailbox's messages by date, from the small table (ids, in order),
/// and how many there are (with `counted`). `unread_or`: only unread (or that one).
pub fn date_page(folders: &[String], unread_or: Option<i64>, ascending: bool, offset: i64, limit: i64, counted: bool) -> (Vec<i64>, i64) {
    let path = db_path();
    if !path.exists() || folders.is_empty() {
        return (Vec::new(), 0);
    }
    with_conn(&path, |c| {
        let mut args: Vec<Arg> = folders.iter().map(|f| Arg::Text(f.clone())).collect();
        let marks = (1..=folders.len()).map(|i| format!("?{i}")).collect::<Vec<_>>().join(",");
        let mut filter = format!("folder IN ({marks})");
        if let Some(k) = unread_or {
            args.push(Arg::Integer(k));
            filter = format!("{filter} AND (read = 0 OR id = ?{})", args.len());
        }
        let dir = if ascending { "ASC" } else { "DESC" };
        let sql = format!("SELECT id FROM msg_threads INDEXED BY msg_threads_by_date WHERE {filter} ORDER BY date {dir}, id {dir} LIMIT {limit} OFFSET {offset}");
        let mut st = c.prepare_cached(&sql).map_err(|e| e.to_string())?;
        let ids: Vec<i64> = st.query_map(rusqlite::params_from_iter(args.iter()), |r| r.get(0)).map_err(|e| e.to_string())?.filter_map(Result::ok).collect();
        drop(st);
        let total = if counted {
            c.query_row(&format!("SELECT COUNT(*) FROM msg_threads INDEXED BY msg_threads_by_date WHERE {filter}"), rusqlite::params_from_iter(args.iter()), |r| r.get(0))
                .map_err(|e| e.to_string())?
        } else {
            -1
        };
        Ok((ids, total))
    })
    .unwrap_or((Vec::new(), 0))
}

/// The text of these messages (those that have it here), by id: (plain text, HTML).
pub fn bodies(ids: &[i64]) -> HashMap<i64, (String, String)> {
    let path = db_path();
    if !path.exists() || ids.is_empty() {
        return HashMap::new();
    }
    with_conn(&path, |c| {
        let marks = (1..=ids.len()).map(|i| format!("?{i}")).collect::<Vec<_>>().join(",");
        let mut st = c
            .prepare(&format!("SELECT id, body, html FROM messages WHERE id IN ({marks}) AND (body <> '' OR html <> '')"))
            .map_err(|e| e.to_string())?;
        let rows = st
            .query_map(rusqlite::params_from_iter(ids.iter()), |r| Ok((r.get::<_, i64>(0)?, (r.get::<_, String>(1)?, r.get::<_, String>(2)?))))
            .map_err(|e| e.to_string())?;
        Ok(rows.flatten().collect())
    })
    .unwrap_or_default()
}

/// Which of these messages have their whole text here (not only headers).
pub fn whole_here(ids: &[i64]) -> std::collections::HashSet<i64> {
    let path = db_path();
    if !path.exists() || ids.is_empty() {
        return Default::default();
    }
    with_conn(&path, |c| {
        let mut out = std::collections::HashSet::new();
        for chunk in ids.chunks(500) {
            let marks = (1..=chunk.len()).map(|i| format!("?{i}")).collect::<Vec<_>>().join(",");
            let mut st = c.prepare(&format!("SELECT id FROM messages WHERE id IN ({marks}) AND partial = 0")).map_err(|e| e.to_string())?;
            out.extend(st.query_map(rusqlite::params_from_iter(chunk.iter()), |r| r.get::<_, i64>(0)).map_err(|e| e.to_string())?.flatten());
        }
        Ok(out)
    })
    .unwrap_or_default()
}

/// Messages by id, in the order given.
pub fn by_ids(ids: &[i64]) -> Vec<Message> {
    let path = db_path();
    if !path.exists() || ids.is_empty() {
        return Vec::new();
    }
    with_conn(&path, |c| {
        let marks = (1..=ids.len()).map(|i| format!("?{i}")).collect::<Vec<_>>().join(",");
        let sql = format!("SELECT {ROW_COLS} FROM messages WHERE id IN ({marks})");
        let mut st = c.prepare(&sql).map_err(|e| e.to_string())?;
        let found = st.query_map(rusqlite::params_from_iter(ids.iter()), row_message).map_err(|e| e.to_string())?;
        let mut list: Vec<Message> = found.filter_map(Result::ok).collect();
        drop(st);
        colors_of(c, &mut list);
        let at: std::collections::HashMap<i64, usize> = ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
        list.sort_by_key(|m| at.get(&m.id).copied().unwrap_or(usize::MAX));
        Ok(list)
    })
    .unwrap_or_default()
}

/// The ids of the messages `filter` matches, in `order`.
pub fn row_ids(filter: &str, args: &[Arg], order: &str) -> Vec<i64> {
    let path = db_path();
    if !path.exists() {
        return Vec::new();
    }
    with_conn(&path, |c| {
        let sql = format!("SELECT id FROM messages INDEXED BY messages_for_list WHERE {filter} ORDER BY {order}");
        let mut st = c.prepare_cached(&sql).map_err(|e| e.to_string())?;
        let rows = st.query_map(rusqlite::params_from_iter(args.iter()), |r| r.get(0)).map_err(|e| e.to_string())?;
        Ok(rows.filter_map(Result::ok).collect())
    })
    .unwrap_or_default()
}

/// Message `id`'s conversation (folder, thread key).
pub fn conv_of(id: i64) -> Option<(String, String)> {
    let path = db_path();
    if !path.exists() {
        return None;
    }
    with_conn(&path, |c| c.query_row("SELECT folder, tk FROM msg_threads WHERE id = ?1", params![id], |r| Ok((r.get(0)?, r.get(1)?))).map_err(|e| e.to_string()))
        .ok()
}

/// The messages of one conversation in one folder, newest first.
pub fn conv_messages(folder: &str, tkey: &str) -> Vec<Message> {
    let path = db_path();
    if !path.exists() {
        return Vec::new();
    }
    with_conn(&path, |c| {
        let sql = format!(
            "SELECT {ROW_COLS} FROM messages WHERE id IN (SELECT id FROM msg_threads INDEXED BY msg_threads_by_conv WHERE folder = ?1 AND tk = ?2)
             ORDER BY date DESC, id DESC"
        );
        let mut st = c.prepare_cached(&sql).map_err(|e| e.to_string())?;
        let found = st.query_map(params![folder, tkey], row_message).map_err(|e| e.to_string())?;
        let mut list: Vec<Message> = found.filter_map(Result::ok).collect();
        drop(st);
        colors_of(c, &mut list);
        Ok(list)
    })
    .unwrap_or_default()
}

/// The text, formatted text and invitation of one message.
/// A message's mailing-list headers, when kept: Some("") for mail known not
/// to be from a list, None when not known. `synced`: the message came in by
/// a server check (whose headers are kept as they arrive).
pub fn list_heads(id: i64, synced: bool) -> Option<String> {
    list_heads_at(&db_path(), id, synced)
}

fn list_heads_at(path: &Path, id: i64, synced: bool) -> Option<String> {
    if !path.exists() {
        return None;
    }
    with_conn(path, |c| {
        if let Ok(h) = c.query_row("SELECT heads FROM list_heads WHERE id = ?1", params![id], |r| r.get(0)) {
            return Ok(Some(h));
        }
        // Stored since the headers have been kept: not from a list.
        let after: i64 = c.query_row("SELECT CAST(value AS INTEGER) FROM meta WHERE key = 'list_heads_after'", [], |r| r.get(0)).unwrap_or(i64::MAX);
        Ok(if synced && id > after { Some(String::new()) } else { None })
    })
    .ok()
    .flatten()
}

/// Keeps a message's mailing-list headers (read later from its source).
pub fn keep_list_heads(id: i64, heads: &str) {
    let path = db_path();
    if !path.exists() {
        return;
    }
    let _ = with_conn(&path, |c| c.execute("INSERT OR REPLACE INTO list_heads(id, heads) VALUES(?1, ?2)", params![id, heads]).map_err(|e| e.to_string()));
}

/// The junk filter as saved (junk_tokens; message counts in meta).
pub fn junk_model() -> Option<crate::junk::Model> {
    let path = db_path();
    if !path.exists() {
        return None;
    }
    with_conn(&path, |c| {
        let n = |k: &str| -> u32 { c.query_row("SELECT CAST(value AS INTEGER) FROM meta WHERE key = ?1", params![k], |r| r.get(0)).unwrap_or(0) };
        let mut m = crate::junk::Model { spam_docs: n("junk_spam_docs"), ham_docs: n("junk_ham_docs"), ..Default::default() };
        let mut st = c.prepare("SELECT token, spam, ham FROM junk_tokens").map_err(|e| e.to_string())?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?, r.get::<_, u32>(2)?))).map_err(|e| e.to_string())?;
        for (t, a, b) in rows.flatten() {
            m.counts.insert(t, (a, b));
        }
        Ok(m)
    })
    .ok()
}

/// Saves the junk filter: the tokens a lesson touched, the counts, and which
/// way each message was learned (id → junk?).
pub fn save_junk(model: &crate::junk::Model, touched: &std::collections::HashSet<String>, learned: &[(i64, Option<bool>)]) {
    let path = db_path();
    if !path.exists() {
        return;
    }
    let _ = with_conn(&path, |c| {
        let tx = c.transaction().map_err(|e| e.to_string())?;
        {
            let mut put = tx.prepare_cached("INSERT OR REPLACE INTO junk_tokens(token, spam, ham) VALUES(?1, ?2, ?3)").map_err(|e| e.to_string())?;
            let mut drop = tx.prepare_cached("DELETE FROM junk_tokens WHERE token = ?1").map_err(|e| e.to_string())?;
            for t in touched {
                match model.counts.get(t) {
                    Some(&(a, b)) if a + b > 0 => put.execute(params![t, a, b]),
                    _ => drop.execute(params![t]),
                }
                .map_err(|e| e.to_string())?;
            }
            let mut learn = tx.prepare_cached("INSERT OR REPLACE INTO junk_learned(id, spam) VALUES(?1, ?2)").map_err(|e| e.to_string())?;
            let mut unlearn = tx.prepare_cached("DELETE FROM junk_learned WHERE id = ?1").map_err(|e| e.to_string())?;
            for (id, way) in learned {
                match way {
                    Some(spam) => learn.execute(params![id, spam]),
                    None => unlearn.execute(params![id]),
                }
                .map_err(|e| e.to_string())?;
            }
            let mut meta = tx.prepare_cached("INSERT OR REPLACE INTO meta(key, value) VALUES(?1, ?2)").map_err(|e| e.to_string())?;
            meta.execute(params!["junk_spam_docs", model.spam_docs.to_string()]).map_err(|e| e.to_string())?;
            meta.execute(params!["junk_ham_docs", model.ham_docs.to_string()]).map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())
    });
}

/// Which way the filter learned these messages (id → junk?).
pub fn junk_learned(ids: &[i64]) -> HashMap<i64, bool> {
    let path = db_path();
    if !path.exists() || ids.is_empty() {
        return HashMap::new();
    }
    with_conn(&path, |c| {
        let mut st = c.prepare_cached("SELECT spam FROM junk_learned WHERE id = ?1").map_err(|e| e.to_string())?;
        Ok(ids.iter().filter_map(|id| st.query_row(params![id], |r| r.get::<_, bool>(0)).ok().map(|b| (*id, b))).collect())
    })
    .unwrap_or_default()
}

/// Messages to learn from at first: (id, from a junk mailbox?) — every one
/// in Junk / Spam, and up to `good` from the inboxes (the newest, and some
/// from across the years).
pub fn junk_examples(junk_folders: &[String], inbox_folders: &[String], good: i64) -> Vec<(i64, bool)> {
    let path = db_path();
    if !path.exists() {
        return Vec::new();
    }
    with_conn(&path, |c| {
        let mut out = Vec::new();
        for f in junk_folders {
            let mut st = c.prepare_cached("SELECT id FROM messages WHERE folder = ?1").map_err(|e| e.to_string())?;
            out.extend(st.query_map(params![f], |r| r.get::<_, i64>(0)).map_err(|e| e.to_string())?.flatten().map(|id| (id, true)));
        }
        // (Half the newest, half from across the years; per inbox, as most
        // people's mail is in one.)
        let each = (good / 2).max(1);
        for f in inbox_folders {
            let mut st = c
                .prepare_cached(
                    "SELECT id FROM (SELECT id FROM messages WHERE folder = ?1 ORDER BY date DESC LIMIT ?2)
                     UNION SELECT id FROM (SELECT id FROM messages WHERE folder = ?1 AND id % 17 = 3 LIMIT ?2)",
                )
                .map_err(|e| e.to_string())?;
            out.extend(st.query_map(params![f, each], |r| r.get::<_, i64>(0)).map_err(|e| e.to_string())?.flatten().map(|id| (id, false)));
        }
        Ok(out)
    })
    .unwrap_or_default()
}

pub fn text(id: i64) -> Option<(String, String, String)> {
    text_at(&db_path(), id)
}

fn text_at(path: &Path, id: i64) -> Option<(String, String, String)> {
    if !path.exists() {
        return None;
    }
    with_conn(path, |c| {
        c.query_row("SELECT body, html, invite FROM messages WHERE id = ?1", params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(|e| e.to_string())
    })
    .ok()
}

/// The messages whose subject, people or text contain every word of `query`
/// (words starting with what's typed). None when there's no database (or the
/// query has no words).
pub fn search(query: &str) -> Option<std::collections::HashSet<i64>> {
    search_at(&db_path(), query)
}

fn search_at(path: &Path, query: &str) -> Option<std::collections::HashSet<i64>> {
    if !path.exists() {
        return None;
    }
    let words: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{}\"*", w.replace('"', "")))
        .collect();
    if words.is_empty() {
        return None;
    }
    with_conn(path, |c| {
        let mut st = c.prepare_cached("SELECT rowid FROM search WHERE search MATCH ?1").map_err(|e| e.to_string())?;
        let ids = st.query_map(params![words.join(" ")], |r| r.get::<_, i64>(0)).map_err(|e| e.to_string())?;
        Ok(ids.filter_map(Result::ok).collect())
    })
    .ok()
}

/// Tells whether a message's header fields changed since it was last saved.
/// (Its text is written whenever it's in memory: it's only there between
/// arriving and being saved.)
fn fingerprint(m: &Message) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (m.id, &m.folder, &m.source, m.uid, m.date, &m.from_name, &m.from_email, &m.to, &m.cc, &m.subject).hash(&mut h);
    (m.read, m.flagged, &m.attachments, &m.raw_ref, m.remind_at, &m.message_id, m.partial, &m.in_reply_to, &m.references, &m.reply_to, &m.preview).hash(&mut h);
    m.flag_color.hash(&mut h);
    h.finish()
}

/// Whether a message carries its text in memory (to be written, then dropped).
pub fn has_text(m: &Message) -> bool {
    !m.body.is_empty() || !m.html.is_empty() || !m.invite.is_empty()
}

/// What the message list needs (and the order of the index that holds it).
const LIST_COLUMNS: &str = "date, id, folder, source, uid, from_name, from_email, to_addr, cc, subject, preview, read, flagged, \
                            attachments, raw_ref, remind_at, message_id, partial, in_reply_to, refs, reply_to";

fn load_from(path: &Path) -> Result<Vec<Message>, String> {
    let c = open_at(path)?;
    // From the list's index alone (see open_at).
    let mut st = c
        .prepare(
            "SELECT id, folder, source, uid, date, from_name, from_email, to_addr, cc, subject, preview,
                    read, flagged, attachments, raw_ref, remind_at, message_id, partial, in_reply_to, refs, reply_to
             FROM messages INDEXED BY messages_for_list ORDER BY date",
        )
        .map_err(|e| e.to_string())?;
    let rows = st
        .query_map([], |r| {
            let attachments: String = r.get(13)?;
            Ok(Message {
                id: r.get(0)?,
                folder: r.get(1)?,
                source: r.get(2)?,
                uid: r.get::<_, i64>(3)? as u32,
                date: r.get(4)?,
                from_name: r.get(5)?,
                from_email: r.get(6)?,
                to: r.get(7)?,
                cc: r.get(8)?,
                subject: r.get(9)?,
                preview: r.get(10)?,
                read: r.get(11)?,
                flagged: r.get(12)?,
                attachments: serde_json::from_str(&attachments).unwrap_or_default(),
                raw_ref: r.get(14)?,
                remind_at: r.get(15)?,
                message_id: r.get(16)?,
                partial: r.get(17)?,
                in_reply_to: r.get(18)?,
                references: r.get(19)?,
                reply_to: r.get(20)?,
                ..Default::default()
            })
        })
        .map_err(|e| e.to_string())?;
    let mut all: Vec<Message> = rows.filter_map(Result::ok).collect();
    let mut colors: HashMap<i64, u8> = HashMap::new();
    let mut st = c.prepare("SELECT id, color FROM flag_colors").map_err(|e| e.to_string())?;
    for r in st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))).map_err(|e| e.to_string())?.flatten() {
        colors.insert(r.0, r.1.clamp(0, 6) as u8);
    }
    for m in all.iter_mut().filter(|m| m.flagged) {
        m.flag_color = colors.get(&m.id).copied().unwrap_or(0);
    }
    Ok(all)
}

fn save_to(path: &Path, messages: &[Message], saved: &mut HashMap<i64, u64>) -> Result<Vec<i64>, String> {
    let now: HashMap<i64, u64> = messages.iter().map(|m| (m.id, fingerprint(m))).collect();
    let changed: Vec<&Message> = messages.iter().filter(|m| has_text(m) || !m.list_heads.is_empty() || saved.get(&m.id) != now.get(&m.id)).collect();
    let gone: Vec<i64> = saved.keys().filter(|id| !now.contains_key(id)).copied().collect();
    if changed.is_empty() && gone.is_empty() {
        return Ok(vec![]);
    }
    with_conn(path, |c| {
        let tx = c.transaction().map_err(|e| e.to_string())?;
        {
            let mut full = tx
                .prepare_cached(
                    "INSERT OR REPLACE INTO messages(id, folder, source, uid, date, from_name, from_email, to_addr, cc, subject, body, html,
                         invite, read, flagged, attachments, raw_ref, remind_at, message_id, partial, in_reply_to, refs, reply_to, preview, flag_color)
                     VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)",
                )
                .map_err(|e| e.to_string())?;
            // Header fields only: the stored text stays as it is.
            let mut head = tx
                .prepare_cached(
                    "INSERT INTO messages(id, folder, source, uid, date, from_name, from_email, to_addr, cc, subject,
                         read, flagged, attachments, raw_ref, remind_at, message_id, partial, in_reply_to, refs, reply_to, preview, flag_color)
                     VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)
                     ON CONFLICT(id) DO UPDATE SET folder = excluded.folder, source = excluded.source, uid = excluded.uid,
                         date = excluded.date, from_name = excluded.from_name, from_email = excluded.from_email,
                         to_addr = excluded.to_addr, cc = excluded.cc, subject = excluded.subject, read = excluded.read,
                         flagged = excluded.flagged, attachments = excluded.attachments, raw_ref = excluded.raw_ref,
                         remind_at = excluded.remind_at, message_id = excluded.message_id, partial = excluded.partial,
                         in_reply_to = excluded.in_reply_to, refs = excluded.refs, reply_to = excluded.reply_to, flag_color = excluded.flag_color,
                         preview = CASE WHEN excluded.preview <> '' THEN excluded.preview ELSE messages.preview END",
                )
                .map_err(|e| e.to_string())?;
            let mut unindex = tx.prepare_cached("DELETE FROM search WHERE rowid = ?1").map_err(|e| e.to_string())?;
            let mut index = tx.prepare_cached("INSERT INTO search(rowid, subject, people, body) VALUES(?1, ?2, ?3, ?4)").map_err(|e| e.to_string())?;
            for m in &changed {
                let attachments = serde_json::to_string(&m.attachments).unwrap_or_else(|_| "[]".into());
                let fresh = !saved.contains_key(&m.id);
                if has_text(m) {
                    let preview = if m.preview.is_empty() { crate::data::strip_preview(&m.body) } else { m.preview.clone() };
                    full.execute(params![
                        m.id, m.folder, m.source, m.uid as i64, m.date, m.from_name, m.from_email, m.to, m.cc, m.subject,
                        m.body, m.html, m.invite, m.read, m.flagged, attachments, m.raw_ref, m.remind_at, m.message_id,
                        m.partial, m.in_reply_to, m.references, m.reply_to, preview, m.flag_color,
                    ])
                    .map_err(|e| e.to_string())?;
                    unindex.execute(params![m.id]).map_err(|e| e.to_string())?;
                    index.execute(params![m.id, m.subject, people(m), m.body]).map_err(|e| e.to_string())?;
                } else {
                    head.execute(params![
                        m.id, m.folder, m.source, m.uid as i64, m.date, m.from_name, m.from_email, m.to, m.cc, m.subject,
                        m.read, m.flagged, attachments, m.raw_ref, m.remind_at, m.message_id, m.partial, m.in_reply_to,
                        m.references, m.reply_to, m.preview, m.flag_color,
                    ])
                    .map_err(|e| e.to_string())?;
                    if fresh {
                        index.execute(params![m.id, m.subject, people(m), ""]).map_err(|e| e.to_string())?;
                    }
                }
            }
            let mut color = tx.prepare_cached("INSERT OR REPLACE INTO flag_colors(id, color) VALUES(?1, ?2)").map_err(|e| e.to_string())?;
            let mut uncolor = tx.prepare_cached("DELETE FROM flag_colors WHERE id = ?1").map_err(|e| e.to_string())?;
            for m in &changed {
                if m.flagged && m.flag_color != 0 {
                    color.execute(params![m.id, m.flag_color]).map_err(|e| e.to_string())?;
                } else {
                    uncolor.execute(params![m.id]).map_err(|e| e.to_string())?;
                }
            }
            let mut heads = tx.prepare_cached("INSERT OR REPLACE INTO list_heads(id, heads) VALUES(?1, ?2)").map_err(|e| e.to_string())?;
            for m in changed.iter().filter(|m| !m.list_heads.is_empty()) {
                heads.execute(params![m.id, m.list_heads]).map_err(|e| e.to_string())?;
            }
            let mut unhead = tx.prepare_cached("DELETE FROM list_heads WHERE id = ?1").map_err(|e| e.to_string())?;
            let mut del = tx.prepare_cached("DELETE FROM messages WHERE id = ?1").map_err(|e| e.to_string())?;
            for id in &gone {
                del.execute(params![id]).map_err(|e| e.to_string())?;
                unindex.execute(params![id]).map_err(|e| e.to_string())?;
                uncolor.execute(params![id]).map_err(|e| e.to_string())?;
                unhead.execute(params![id]).map_err(|e| e.to_string())?;
            }
        }
        tx.commit().map_err(|e| e.to_string())
    })?;
    *saved = now;
    Ok(gone)
}

#[cfg(test)]
pub fn save_for_test(path: &Path, messages: &[Message], saved: &mut HashMap<i64, u64>) {
    save_to(path, messages, saved).unwrap();
}

/// Reads every message (at start-up).
pub fn load() -> Vec<Message> {
    let list = {
        let _t = crate::Timed::new("reading the message list");
        load_from(&db_path()).unwrap_or_default()
    };
    let _t = crate::Timed::new("noting what's saved");
    *SAVED.lock().unwrap() = Some(list.iter().map(|m| (m.id, fingerprint(m))).collect());
    list
}

/// Writes what changed since the last save; deleted messages also lose
/// their attachment files.
pub fn save(messages: &[Message]) -> bool {
    let mut guard = SAVED.lock().unwrap();
    let saved = guard.get_or_insert_with(HashMap::new);
    // No mail at all where there was plenty: something went wrong (mail not
    // read yet?), not thousands of messages deleted at once. Nothing is saved.
    if messages.is_empty() && saved.len() > 1000 {
        eprintln!("almanac: not saving an empty mailbox over {} messages", saved.len());
        return false;
    }
    match save_to(&db_path(), messages, saved) {
        Ok(gone) => {
            let mut state = STATE.lock().unwrap();
            let state = state.get_or_insert_with(HashMap::new);
            for id in &gone {
                state.remove(id);
            }
            for m in messages {
                state.insert(m.id, (m.folder.clone(), m.read, m.flagged));
            }
            for id in gone {
                let _ = std::fs::remove_dir_all(attachment_dir(id));
            }
            SAVES.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            true
        }
        Err(e) => {
            eprintln!("almanac: couldn't save mail: {e}");
            false
        }
    }
}

// ------------------------------------------------- reading on demand
//
// The mail isn't all read into memory: the list, the counts and lookups ask
// the database for what's needed (a page of a mailbox, one message), from the
// list's index (see open_at). Memory holds only messages just changed or
// arrived, until they're saved (see Db::backed).

/// The list's columns, in the order `row_message` reads them.
const ROW_COLS: &str = "id, folder, source, uid, date, from_name, from_email, to_addr, cc, subject, preview, \
                        read, flagged, attachments, raw_ref, remind_at, message_id, partial, in_reply_to, refs, reply_to";

fn row_message(r: &rusqlite::Row) -> rusqlite::Result<Message> {
    let attachments: String = r.get(13)?;
    Ok(Message {
        id: r.get(0)?,
        folder: r.get(1)?,
        source: r.get(2)?,
        uid: r.get::<_, i64>(3)? as u32,
        date: r.get(4)?,
        from_name: r.get(5)?,
        from_email: r.get(6)?,
        to: r.get(7)?,
        cc: r.get(8)?,
        subject: r.get(9)?,
        preview: r.get(10)?,
        read: r.get(11)?,
        flagged: r.get(12)?,
        attachments: serde_json::from_str(&attachments).unwrap_or_default(),
        raw_ref: r.get(14)?,
        remind_at: r.get(15)?,
        message_id: r.get(16)?,
        partial: r.get(17)?,
        in_reply_to: r.get(18)?,
        references: r.get(19)?,
        reply_to: r.get(20)?,
        ..Default::default()
    })
}

/// A value for a query's `?` (text or a number).
pub type Arg = rusqlite::types::Value;

/// Flag colours of the given messages (those not red).
fn colors_of(c: &Connection, msgs: &mut [Message]) {
    if !msgs.iter().any(|m| m.flagged) {
        return;
    }
    let Ok(mut st) = c.prepare_cached("SELECT color FROM flag_colors WHERE id = ?1") else { return };
    for m in msgs.iter_mut().filter(|m| m.flagged) {
        m.flag_color = st.query_row(params![m.id], |r| r.get::<_, i64>(0)).map(|c| c.clamp(0, 6) as u8).unwrap_or(0);
    }
}

/// A page of messages: those matching `filter` (SQL over the list's columns,
/// with `args` for its ?1, ?2…), in `order`.
pub fn rows(filter: &str, args: &[Arg], order: &str, limit: i64, offset: i64) -> Vec<Message> {
    let path = db_path();
    if !path.exists() {
        return Vec::new();
    }
    with_conn(&path, |c| {
        let sql = format!("SELECT {ROW_COLS} FROM messages INDEXED BY messages_for_list WHERE {filter} ORDER BY {order} LIMIT {limit} OFFSET {offset}");
        let mut st = c.prepare_cached(&sql).map_err(|e| e.to_string())?;
        let found = st.query_map(rusqlite::params_from_iter(args.iter()), row_message).map_err(|e| e.to_string())?;
        let mut list: Vec<Message> = found.filter_map(Result::ok).collect();
        drop(st);
        colors_of(c, &mut list);
        Ok(list)
    })
    .unwrap_or_default()
}

/// How many messages match `filter`.
pub fn count(filter: &str, args: &[Arg]) -> i64 {
    let path = db_path();
    if !path.exists() {
        return 0;
    }
    with_conn(&path, |c| {
        let sql = format!("SELECT COUNT(*) FROM messages INDEXED BY messages_for_list WHERE {filter}");
        c.query_row(&sql, rusqlite::params_from_iter(args.iter()), |r| r.get(0)).map_err(|e| e.to_string())
    })
    .unwrap_or(0)
}

/// One message (its list fields; the text is read with `text`).
pub fn by_id(id: i64) -> Option<Message> {
    let path = db_path();
    if !path.exists() {
        return None;
    }
    with_conn(&path, |c| {
        // Its date from the row (it comes before the text), then the rest from the list's index.
        let date: i64 = c.query_row("SELECT date FROM messages WHERE id = ?1", params![id], |r| r.get(0)).map_err(|e| e.to_string())?;
        let sql = format!("SELECT {ROW_COLS} FROM messages INDEXED BY messages_for_list WHERE date = ?1 AND id = ?2");
        let mut m = c.query_row(&sql, params![date, id], row_message).map_err(|e| e.to_string())?;
        colors_of(c, std::slice::from_mut(&mut m));
        Ok(m)
    })
    .ok()
}

/// Rows moved into `folder` by Almanac whose server number isn't known yet
/// (uid 0), for a message just found there on the server: removed.
pub fn drop_stand_ins(source: &str, folder: &str, message_id: &str) {
    let path = db_path();
    if !path.exists() {
        return;
    }
    let _ = with_conn(&path, |c| {
        let ids: Vec<i64> = {
            let mut st = c.prepare("SELECT id FROM messages WHERE source = ?1 AND folder = ?2 AND message_id = ?3 AND uid = 0").map_err(|e| e.to_string())?;
            let rows = st.query_map(params![source, folder, message_id], |r| r.get(0)).map_err(|e| e.to_string())?;
            rows.filter_map(Result::ok).collect()
        };
        for id in ids {
            let _ = c.execute("DELETE FROM search WHERE rowid = ?1", params![id]);
            c.execute("DELETE FROM messages WHERE id = ?1", params![id]).map_err(|e| e.to_string())?;
        }
        Ok(())
    });
}

/// The message on the server at (source, folder, uid), if it's here.
pub fn id_at(source: &str, folder: &str, uid: u32) -> Option<i64> {
    let path = db_path();
    if !path.exists() {
        return None;
    }
    with_conn(&path, |c| {
        c.query_row("SELECT id FROM messages WHERE source = ?1 AND folder = ?2 AND uid = ?3", params![source, folder, uid as i64], |r| r.get(0))
            .map_err(|e| e.to_string())
    })
    .ok()
}

/// Each folder's server UIDs of a live account's messages here.
/// A folder's messages here by their UID on the server (UID → id).
pub fn folder_uid_ids(source: &str, folder: &str) -> HashMap<u32, i64> {
    let path = db_path();
    if !path.exists() {
        return HashMap::new();
    }
    with_conn(&path, |c| {
        let mut st = c.prepare_cached("SELECT uid, id FROM messages INDEXED BY messages_by_server WHERE source = ?1 AND folder = ?2 AND uid > 0").map_err(|e| e.to_string())?;
        let rows = st.query_map(params![source, folder], |r| Ok((r.get::<_, i64>(0)? as u32, r.get::<_, i64>(1)?))).map_err(|e| e.to_string())?;
        Ok(rows.flatten().collect())
    })
    .unwrap_or_default()
}

pub fn known_uids(source: &str) -> HashMap<String, Vec<u32>> {
    let path = db_path();
    let mut out: HashMap<String, Vec<u32>> = HashMap::new();
    if !path.exists() {
        return out;
    }
    let _ = with_conn(&path, |c| {
        let mut st = c.prepare_cached("SELECT folder, uid FROM messages WHERE source = ?1 AND uid > 0").map_err(|e| e.to_string())?;
        let rows = st.query_map(params![source], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u32))).map_err(|e| e.to_string())?;
        for (f, u) in rows.flatten() {
            out.entry(f).or_default().push(u);
        }
        Ok(())
    });
    out
}

/// Message counts by folder: (folder, read, flagged) → how many.
/// Each folder's (all, unread, flagged) counts, from the table the database
/// keeps (see open_at).
pub fn folder_counts() -> Vec<(String, i64, i64, i64)> {
    let path = db_path();
    if !path.exists() {
        return Vec::new();
    }
    with_conn(&path, |c| {
        let mut st = c.prepare_cached("SELECT folder, total, unread, flagged FROM folder_counts WHERE total > 0").map_err(|e| e.to_string())?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).map_err(|e| e.to_string())?;
        Ok(rows.filter_map(Result::ok).collect())
    })
    .unwrap_or_default()
}

/// Flagged messages that have a colour other than red: (folder, colour).
pub fn colored_flags() -> Vec<(String, u8)> {
    let path = db_path();
    if !path.exists() {
        return Vec::new();
    }
    with_conn(&path, |c| {
        let mut st = c
            .prepare_cached("SELECT (SELECT folder FROM messages WHERE id = f.id), f.color FROM flag_colors f WHERE (SELECT flagged FROM messages WHERE id = f.id) = 1")
            .map_err(|e| e.to_string())?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get::<_, i64>(1)?.clamp(0, 6) as u8))).map_err(|e| e.to_string())?;
        Ok(rows.filter_map(Result::ok).collect())
    })
    .unwrap_or_default()
}

/// The full-text search's words for a query ("" when it has none).
pub fn match_expr(query: &str) -> String {
    query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{}\"*", w.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The full-text words for a search of mailboxes typed a letter at a time:
/// as `match_expr`, but a single letter beside longer words is left out until
/// it's more ("meeting t": every word starting with t took half a second, to
/// narrow what "meeting" found to almost the same).
pub fn search_expr(query: &str) -> String {
    let words: Vec<&str> = query.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let longer = words.iter().any(|w| w.chars().count() > 1);
    words
        .into_iter()
        .filter(|w| !longer || w.chars().count() > 1)
        .map(|w| format!("\"{}\"*", w.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Read / flagged changes found on the server, for messages not in memory:
/// written straight to the database.
pub fn update_flags(source: &str, changes: &[(String, u32, bool, bool, u8)]) {
    let path = db_path();
    if changes.is_empty() || !path.exists() {
        return;
    }
    let _ = with_conn(&path, |c| {
        let tx = c.transaction().map_err(|e| e.to_string())?;
        {
            let mut find = tx.prepare_cached("SELECT id, read, flagged FROM messages WHERE source = ?1 AND folder = ?2 AND uid = ?3").map_err(|e| e.to_string())?;
            let mut set = tx.prepare_cached("UPDATE messages SET read = ?2, flagged = ?3 WHERE id = ?1").map_err(|e| e.to_string())?;
            let mut color = tx.prepare_cached("INSERT OR REPLACE INTO flag_colors(id, color) VALUES(?1, ?2)").map_err(|e| e.to_string())?;
            let mut uncolor = tx.prepare_cached("DELETE FROM flag_colors WHERE id = ?1").map_err(|e| e.to_string())?;
            for (folder, uid, read, flagged, c) in changes {
                let Ok((id, was_read, was_flagged)) = find.query_row(params![source, folder, *uid as i64], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, bool>(1)?, r.get::<_, bool>(2)?))) else {
                    continue;
                };
                if was_read != *read || was_flagged != *flagged {
                    set.execute(params![id, read, flagged]).map_err(|e| e.to_string())?;
                }
                if !flagged || *c == 0 {
                    uncolor.execute(params![id]).map_err(|e| e.to_string())?;
                } else if *c != crate::data::NO_COLOR {
                    color.execute(params![id, *c as i64]).map_err(|e| e.to_string())?;
                }
            }
        }
        tx.commit().map_err(|e| e.to_string())
    });
}

/// Messages gone from the server (or from a mailbox or account that's gone),
/// for those not in memory: deleted from the database with their attachments.
/// `which` picks them: (folder, uid) pairs of `source`, or every message of
/// `source` outside the `keep` folders, or all of `source`.
pub enum Gone<'a> {
    Uids(&'a [(String, u32)]),
    FoldersNotIn(&'a [String]),
    All,
}
pub fn remove(source: &str, which: Gone) -> Vec<i64> {
    let path = db_path();
    if !path.exists() {
        return Vec::new();
    }
    let ids: Vec<i64> = with_conn(&path, |c| {
        let mut ids = Vec::new();
        match which {
            Gone::Uids(list) => {
                let mut st = c.prepare_cached("SELECT id FROM messages WHERE source = ?1 AND folder = ?2 AND uid = ?3").map_err(|e| e.to_string())?;
                for (f, u) in list {
                    if let Ok(id) = st.query_row(params![source, f, *u as i64], |r| r.get::<_, i64>(0)) {
                        ids.push(id);
                    }
                }
            }
            Gone::FoldersNotIn(keep) => {
                // (The account's folders from the small counts table, not from every message.)
                let mut st = c.prepare_cached("SELECT folder FROM folder_counts WHERE substr(folder, 1, length(?1) + 1) = ?1 || ':' AND total > 0").map_err(|e| e.to_string())?;
                let folders: Vec<String> = st.query_map(params![source], |r| r.get(0)).map_err(|e| e.to_string())?.filter_map(Result::ok).collect();
                let mut of = c.prepare_cached("SELECT id FROM messages WHERE source = ?1 AND folder = ?2").map_err(|e| e.to_string())?;
                for f in folders.iter().filter(|f| !keep.contains(f)) {
                    ids.extend(of.query_map(params![source, f], |r| r.get::<_, i64>(0)).map_err(|e| e.to_string())?.filter_map(Result::ok));
                }
            }
            Gone::All => {
                let mut st = c.prepare_cached("SELECT id FROM messages WHERE source = ?1").map_err(|e| e.to_string())?;
                ids.extend(st.query_map(params![source], |r| r.get::<_, i64>(0)).map_err(|e| e.to_string())?.filter_map(Result::ok));
            }
        }
        if ids.is_empty() {
            return Ok(ids);
        }
        let tx = c.transaction().map_err(|e| e.to_string())?;
        {
            let mut del = tx.prepare_cached("DELETE FROM messages WHERE id = ?1").map_err(|e| e.to_string())?;
            let mut unindex = tx.prepare_cached("DELETE FROM search WHERE rowid = ?1").map_err(|e| e.to_string())?;
            let mut uncolor = tx.prepare_cached("DELETE FROM flag_colors WHERE id = ?1").map_err(|e| e.to_string())?;
            for id in &ids {
                del.execute(params![id]).map_err(|e| e.to_string())?;
                unindex.execute(params![id]).map_err(|e| e.to_string())?;
                uncolor.execute(params![id]).map_err(|e| e.to_string())?;
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(ids)
    })
    .unwrap_or_default();
    if let Some(saved) = SAVED.lock().unwrap().as_mut() {
        for id in &ids {
            saved.remove(id);
        }
    }
    for id in &ids {
        let _ = std::fs::remove_dir_all(attachment_dir(*id));
    }
    ids
}

/// A message read from the database into memory (to be changed): noted as
/// saved, so only real changes are written back.
pub fn note_loaded(m: &Message) {
    SAVED.lock().unwrap().get_or_insert_with(HashMap::new).insert(m.id, fingerprint(m));
    STATE.lock().unwrap().get_or_insert_with(HashMap::new).insert(m.id, (m.folder.clone(), m.read, m.flagged));
}

/// Whether a message is tracked (read into memory, or saved from it, and not
/// let go since): one not in memory any more is waiting to be deleted.
pub fn is_tracked(id: i64) -> bool {
    SAVED.lock().unwrap().as_ref().is_some_and(|s| s.contains_key(&id))
}

/// Every tracked message (see `is_tracked`).
pub fn tracked_ids() -> std::collections::HashSet<i64> {
    SAVED.lock().unwrap().as_ref().map(|s| s.keys().copied().collect()).unwrap_or_default()
}

/// A tracked message's folder, read and flagged as saved.
pub fn saved_state(id: i64) -> Option<(String, bool, bool)> {
    STATE.lock().unwrap().as_ref().and_then(|s| s.get(&id).cloned())
}

/// Downloaded text stored straight into the database, on the download's own
/// thread (handed to the window's thread, 100 messages and ~10 MB at a time
/// froze the window while it was saved). Only messages still waiting for it
/// are filled. Returns the ids filled.
pub fn store_texts(texts: &[crate::live::Text]) -> Vec<i64> {
    let path = db_path();
    if !path.exists() || texts.is_empty() {
        return Vec::new();
    }
    let _t = crate::Timed::new("storing downloaded text");
    with_conn(&path, |c| {
        let tx = c.transaction().map_err(|e| e.to_string())?;
        let mut done = Vec::new();
        {
            let mut old = tx.prepare_cached("SELECT attachments FROM messages WHERE id = ?1 AND partial = 1").map_err(|e| e.to_string())?;
            let mut set = tx
                .prepare_cached("UPDATE messages SET body = ?2, html = ?3, invite = ?4, preview = ?5, attachments = ?6, partial = ?7 WHERE id = ?1")
                .map_err(|e| e.to_string())?;
            let mut unindex = tx.prepare_cached("DELETE FROM search WHERE rowid = ?1").map_err(|e| e.to_string())?;
            let mut index = tx
                .prepare_cached(
                    "INSERT INTO search(rowid, subject, people, body)
                     SELECT id, subject, from_name || ' ' || from_email || ' ' || to_addr || ' ' || cc, body FROM messages WHERE id = ?1",
                )
                .map_err(|e| e.to_string())?;
            for t in texts {
                let Ok(was) = old.query_row(params![t.id], |r| r.get::<_, String>(0)) else { continue };
                // Whole: its attachments as found; just its start: those known so far too.
                let mut atts: Vec<String> = if t.complete { Vec::new() } else { serde_json::from_str(&was).unwrap_or_default() };
                for a in &t.attachments {
                    if !atts.contains(a) {
                        atts.push(a.clone());
                    }
                }
                let atts = serde_json::to_string(&atts).unwrap_or_else(|_| "[]".into());
                let preview = crate::data::fetched_preview(&t.body, &t.html);
                set.execute(params![t.id, t.body, t.html, t.invite, preview, atts, !t.complete]).map_err(|e| e.to_string())?;
                let _ = unindex.execute(params![t.id]);
                index.execute(params![t.id]).map_err(|e| e.to_string())?;
                done.push(t.id);
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(done)
    })
    .unwrap_or_default()
}

/// Messages waiting for their text that are already here in another folder
/// (Gmail shows one message in every label it has: Inbox, All Mail, Important,
/// …), filled in from that copy instead of being downloaded again, as
/// Thunderbird does for Gmail. Returns how many were filled.
pub fn fill_from_copies(source: &str) -> usize {
    let path = db_path();
    if !path.exists() {
        return 0;
    }
    let _t = crate::Timed::new("filling in messages from their copies");
    with_conn(&path, |c| {
        let pairs: Vec<(i64, i64)> = {
            // (Read from the list's slim index: the rows themselves, with all
            // the mail's text, took ~20 s to scan on every check.)
            let mut st = c
                .prepare_cached(
                    "SELECT p.id, (SELECT q.id FROM messages q WHERE q.message_id = p.message_id AND q.id <> p.id AND q.preview <> '' LIMIT 1)
                     FROM messages p INDEXED BY messages_for_list WHERE p.source = ?1 AND p.partial = 1 AND p.uid > 0 AND p.preview = '' AND p.message_id <> ''",
                )
                .map_err(|e| e.to_string())?;
            let rows = st.query_map(params![source], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?))).map_err(|e| e.to_string())?;
            rows.filter_map(Result::ok).filter_map(|(a, b)| b.map(|b| (a, b))).collect()
        };
        if pairs.is_empty() {
            return Ok(0);
        }
        let tx = c.transaction().map_err(|e| e.to_string())?;
        {
            // (Still partial when the copy had attachments: they're fetched for
            // this one when it's opened.)
            let mut copy = tx
                .prepare_cached(
                    "UPDATE messages SET (body, html, invite, preview, attachments, partial) =
                         (SELECT body, html, invite, preview, attachments, CASE WHEN partial = 0 AND attachments = '[]' THEN 0 ELSE 1 END
                          FROM messages WHERE id = ?2)
                     WHERE id = ?1",
                )
                .map_err(|e| e.to_string())?;
            let mut unindex = tx.prepare_cached("DELETE FROM search WHERE rowid = ?1").map_err(|e| e.to_string())?;
            let mut index = tx
                .prepare_cached(
                    "INSERT INTO search(rowid, subject, people, body)
                     SELECT id, subject, from_name || ' ' || from_email || ' ' || to_addr || ' ' || cc, body FROM messages WHERE id = ?1",
                )
                .map_err(|e| e.to_string())?;
            for (to, from) in &pairs {
                copy.execute(params![to, from]).map_err(|e| e.to_string())?;
                let _ = unindex.execute(params![to]);
                index.execute(params![to]).map_err(|e| e.to_string())?;
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(pairs.len())
    })
    .unwrap_or(0)
}

/// A live account's messages still waiting for their text: (id, folder, uid, date).
pub fn partial_ids(source: &str) -> Vec<(i64, String, u32, i64)> {
    partial_ids_with_message_id(source).into_iter().map(|(id, f, u, d, _)| (id, f, u, d)).collect()
}

/// The same, with each one's Message-ID (to fetch only one of its copies).
pub fn partial_ids_with_message_id(source: &str) -> Vec<(i64, String, u32, i64, String)> {
    let path = db_path();
    if !path.exists() {
        return Vec::new();
    }
    with_conn(&path, |c| {
        let mut st = c
            .prepare_cached("SELECT id, folder, uid, date, message_id FROM messages INDEXED BY messages_for_list WHERE source = ?1 AND partial = 1 AND uid > 0 AND preview = ''")
            .map_err(|e| e.to_string())?;
        let rows = st.query_map(params![source], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? as u32, r.get(3)?, r.get(4)?))).map_err(|e| e.to_string())?;
        Ok(rows.filter_map(Result::ok).collect())
    })
    .unwrap_or_default()
}

/// The lowest server UID held for a folder (older mail is below it).
pub fn lowest_uid(source: &str, folder: &str) -> u32 {
    let path = db_path();
    if !path.exists() {
        return 0;
    }
    with_conn(&path, |c| {
        c.query_row("SELECT MIN(uid) FROM messages WHERE source = ?1 AND folder = ?2 AND uid > 0", params![source, folder], |r| r.get::<_, Option<i64>>(0))
            .map(|v| v.unwrap_or(0) as u32)
            .map_err(|e| e.to_string())
    })
    .unwrap_or(0)
}

/// Messages by their Message-ID (a conversation's earlier messages).
pub fn by_message_ids(ids: &[String]) -> Vec<Message> {
    let path = db_path();
    if ids.is_empty() || !path.exists() {
        return Vec::new();
    }
    let found: Vec<i64> = with_conn(&path, |c| {
        // (Kept as written, usually <in angle brackets>: both looked for.)
        let mut st = c.prepare_cached("SELECT id FROM messages WHERE message_id IN (?1, ?2)").map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for mid in ids {
            let bare = mid.trim().trim_matches(['<', '>']);
            out.extend(st.query_map(params![bare, format!("<{bare}>")], |r| r.get::<_, i64>(0)).map_err(|e| e.to_string())?.filter_map(Result::ok));
        }
        Ok(out)
    })
    .unwrap_or_default();
    found.into_iter().filter_map(by_id).collect()
}

/// Messages let go from memory after saving: no longer tracked (so they
/// aren't taken for deleted).
pub fn forget(ids: &[i64]) {
    if let Some(saved) = SAVED.lock().unwrap().as_mut() {
        for id in ids {
            saved.remove(id);
        }
    }
    if let Some(state) = STATE.lock().unwrap().as_mut() {
        for id in ids {
            state.remove(id);
        }
    }
}

/// Everyone in the mail (for address suggestions): `f` with each message's
/// folder, sender and recipients and date, newest first, up to `limit`.
pub fn each_person_row(limit: i64, mut f: impl FnMut(&str, &str, &str, &str, &str, i64)) {
    let path = db_path();
    if !path.exists() {
        return;
    }
    let _ = with_conn(&path, |c| {
        let mut st = c
            .prepare_cached("SELECT folder, from_name, from_email, to_addr, cc, date FROM messages INDEXED BY messages_for_list ORDER BY date DESC LIMIT ?1")
            .map_err(|e| e.to_string())?;
        let mut rows = st.query(params![limit]).map_err(|e| e.to_string())?;
        while let Ok(Some(r)) = rows.next() {
            let get = |i: usize| r.get_ref(i).ok().and_then(|v| v.as_str().ok()).unwrap_or("").to_string();
            f(&get(0), &get(1), &get(2), &get(3), &get(4), r.get(5).unwrap_or(0));
        }
        Ok(())
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_only_changes_and_reads_back() {
        let dir = std::env::temp_dir().join(format!("almanac-mailstore-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("mail.sqlite");
        let mut msgs: Vec<Message> = (1..=3)
            .map(|i| Message {
                id: i,
                folder: "live:a:mail/INBOX".into(),
                subject: format!("Hello {i}"),
                body: "Text ünïcode".into(),
                date: 1_700_000_000 + i,
                uid: i as u32,
                attachments: vec!["report.pdf".into()],
                partial: i == 3,
                flagged: i == 2,
                flag_color: if i == 2 { 4 } else { 0 },
                ..Default::default()
            })
            .collect();
        let mut saved = HashMap::new();
        save_to(&path, &msgs, &mut saved).unwrap();
        let back = load_from(&path).unwrap();
        assert_eq!(back.len(), 3);
        assert_eq!(back.iter().map(|m| (m.flagged, m.flag_color)).collect::<Vec<_>>(), vec![(false, 0), (true, 4), (false, 0)], "the flag's colour is kept");
        // Memory gets a preview; the text stays in the database.
        assert!(back[0].body.is_empty());
        assert_eq!(back[0].preview, "Text ünïcode");
        assert_eq!(text_at(&path, 1).unwrap().0, "Text ünïcode");
        assert_eq!(back[0].attachments, vec!["report.pdf".to_string()]);
        assert!(back[2].partial && !back[0].partial);
        // Found by a word of its text (accents or not), and by a word's start.
        assert_eq!(search_at(&path, "unicode").unwrap().len(), 3);
        assert!(search_at(&path, "Hell").unwrap().contains(&2));
        assert!(search_at(&path, "nowhere").unwrap().is_empty());

        // Text dropped from memory, then a header change: the stored text stays.
        for m in msgs.iter_mut() {
            m.preview = "Text ünïcode".into();
            m.body.clear();
        }
        save_to(&path, &msgs, &mut saved).unwrap();
        // One changed, one removed: only those touch the database.
        msgs[0].read = true;
        msgs.remove(1);
        let gone = save_to(&path, &msgs, &mut saved).unwrap();
        assert_eq!(gone, vec![2]);
        let back = load_from(&path).unwrap();
        assert_eq!(back.len(), 2);
        assert!(back.iter().find(|m| m.id == 1).unwrap().read);
        assert_eq!(text_at(&path, 1).unwrap().0, "Text ünïcode", "a header change keeps the text");
        assert!(!search_at(&path, "Hello").unwrap().contains(&2), "a removed message leaves the search index");
        // A list's headers: kept as it arrives (and dropped with it). Mail
        // stored before they were kept isn't known; mail checked since is.
        let mut list = Message { id: 9, folder: "live:a:mail/INBOX".into(), list_heads: "List-Unsubscribe: <https://x.example/u>\r\n".into(), ..Default::default() };
        assert_eq!(list_heads_at(&path, 1, true).as_deref(), Some(""), "a new file: all mail checked since");
        with_conn(&path, |c| c.execute("UPDATE meta SET value = '8' WHERE key = 'list_heads_after'", []).map_err(|e| e.to_string())).unwrap();
        assert_eq!(list_heads_at(&path, 1, true), None, "stored before");
        msgs.push(list.clone());
        msgs.push(Message { id: 10, folder: "live:a:mail/INBOX".into(), ..Default::default() });
        save_to(&path, &msgs, &mut saved).unwrap();
        assert_eq!(list_heads_at(&path, 9, true).as_deref(), Some(list.list_heads.as_str()));
        assert_eq!(list_heads_at(&path, 10, true).as_deref(), Some(""), "checked since: not from a list");
        assert_eq!(list_heads_at(&path, 10, false), None, "imported: looked at when opened");
        list.list_heads.clear();
        msgs.retain(|m| m.id != 9 && m.id != 10);
        save_to(&path, &msgs, &mut saved).unwrap();
        assert_eq!(list_heads_at(&path, 9, false), None, "gone with its message");
        // Nothing changed: nothing written.
        assert!(save_to(&path, &msgs, &mut saved).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// How long the list takes to read at start from the real file (read-only):
    /// ALMANAC_REAL_DATA=~/.local/share/almanac cargo test --release list_read_time -- --ignored --nocapture
    #[test]
    #[ignore]
    fn list_read_time() {
        let Some(dir) = std::env::var_os("ALMANAC_REAL_DATA").map(std::path::PathBuf::from) else { return };
        let c = rusqlite::Connection::open_with_flags(dir.join("mail.sqlite"), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let t = std::time::Instant::now();
        let n: i64 = c.query_row("SELECT count(*) FROM (SELECT id, folder, source, uid, date, from_name, from_email, to_addr, cc, subject, preview, read, flagged, attachments, raw_ref, remind_at, message_id, partial, in_reply_to, refs, reply_to FROM messages INDEXED BY messages_for_list)", [], |r| r.get(0)).unwrap();
        eprintln!("list of {n}: {:?}", t.elapsed());
        let t = std::time::Instant::now();
        let f: i64 = c.query_row("SELECT count(*) FROM flag_colors", [], |r| r.get(0)).unwrap_or(0);
        eprintln!("flag colours of {f}: {:?}", t.elapsed());
    }

    #[test]
    fn attachment_names_are_safe() {
        assert_eq!(safe_name("../../etc/passwd"), "_.._etc_passwd");
        assert_eq!(safe_name("a:b?.pdf"), "a_b_.pdf");
        assert_eq!(safe_name(""), "attachment");
    }
}
