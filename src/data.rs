//! The app's data: accounts, mail folders, messages, calendars, events,
//! contacts and groups. Kept in memory; messages are saved in an SQLite
//! database and their attachments as files (see mailstore.rs), the rest in
//! one JSON file.

use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime, Timelike};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;

pub const SOURCE_LOCAL: &str = "local";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub email: String,
    pub source: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub account: String,
    pub name: String,
    /// inbox, drafts, sent, junk, trash, archive or other
    pub kind: String,
    /// Folder name on the mail server (live accounts)
    #[serde(default)]
    pub path: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Hash)]
pub struct Message {
    pub id: i64,
    /// The start of the text, as the list shows it (kept in memory; the text
    /// itself is in the database).
    #[serde(default)]
    pub preview: String,
    pub folder: String,
    pub from_name: String,
    pub from_email: String,
    pub to: String,
    #[serde(default)]
    pub cc: String,
    pub subject: String,
    pub body: String,
    /// Unix seconds
    pub date: i64,
    pub read: bool,
    pub flagged: bool,
    /// The flag's colour, as in Apple Mail: 0 red, 1 orange, 2 yellow, 3 green,
    /// 4 blue, 5 purple, 6 gray (kept on the server as Apple does, see live.rs).
    #[serde(default)]
    pub flag_color: u8,
    #[serde(default)]
    pub source: String,
    /// File names of attachments
    #[serde(default)]
    pub attachments: Vec<String>,
    /// IMAP UID in its folder (live accounts; 0 = not known yet)
    #[serde(default)]
    pub uid: u32,
    /// Formatted (HTML) version of the body, when the sender sent one
    #[serde(default)]
    pub html: String,
    /// Calendar invitation inside the message (iCalendar text)
    #[serde(default)]
    pub invite: String,
    /// Where the full original can be read again for attachments:
    /// "mbox:<file>:<offset>:<length>", "mailspring:<message id>" or "" (server / none)
    #[serde(default)]
    pub raw_ref: String,
    /// "Remind Me": unix seconds when the message comes back to the top
    #[serde(default)]
    pub remind_at: i64,
    /// Message-ID header, used for replies
    #[serde(default)]
    pub message_id: String,
    /// Only the headers are here so far; the text is still being
    /// downloaded in the background (live accounts).
    #[serde(default)]
    pub partial: bool,
    /// The messages this one answers (Message-IDs), for conversations
    #[serde(default)]
    pub in_reply_to: String,
    #[serde(default)]
    pub references: String,
    /// Reply-To address, when replies should go somewhere else
    #[serde(default)]
    pub reply_to: String,
    /// List-Id, List-Unsubscribe and List-Unsubscribe-Post as received (header
    /// lines; "" for mail not from a list, or not read yet). Kept in a table
    /// of their own; not read back with the list.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub list_heads: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Calendar {
    pub id: String,
    pub name: String,
    pub color: String,
    pub visible: bool,
    pub source: String,
    /// CalDAV collection URL (live accounts)
    #[serde(default)]
    pub url: String,
    /// Live account id
    #[serde(default)]
    pub account: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Event {
    pub id: i64,
    pub calendar: String,
    pub title: String,
    /// Wall time in `tz` ("YYYY-MM-DDTHH:MM"); local time when `tz` is empty
    pub start: String,
    pub end: String,
    pub all_day: bool,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub notes: String,
    /// iCalendar RRULE value (without "RRULE:"), empty when not repeating
    #[serde(default)]
    pub rrule: String,
    #[serde(default)]
    pub source: String,
    /// IANA time zone of `start`/`end`; empty = floating (same clock time everywhere)
    #[serde(default)]
    pub tz: String,
    /// Skipped occurrences (wall times in `tz`)
    #[serde(default)]
    pub exdates: Vec<String>,
    /// iCalendar UID, server resource, change tag and original text (live accounts)
    #[serde(default)]
    pub uid: String,
    #[serde(default)]
    pub href: String,
    #[serde(default)]
    pub etag: String,
    #[serde(default)]
    pub raw: String,
    /// Alerts: minutes before the start (all-day: before midnight)
    #[serde(default)]
    pub alarms: Vec<i64>,
    /// People invited, and who organises it
    #[serde(default)]
    pub attendees: Vec<Attendee>,
    #[serde(default)]
    pub organizer: String,
    /// The notes as sent, formatted (HTML), for reading; `notes` stays the
    /// plain text that's written back. Gone once the notes are changed here.
    #[serde(default)]
    pub notes_html: String,
    /// A video call's link (Google Meet, Teams…), given apart from the notes
    #[serde(default)]
    pub url: String,
    /// For one changed occurrence of a series: which occurrence it replaces
    /// (wall time in `tz`); the series has the same `uid`.
    #[serde(default)]
    pub recurrence_id: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Attendee {
    pub email: String,
    #[serde(default)]
    pub name: String,
    /// NEEDS-ACTION, ACCEPTED, TENTATIVE or DECLINED
    #[serde(default)]
    pub status: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Labeled {
    pub label: String,
    pub value: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Contact {
    pub id: i64,
    pub first: String,
    pub last: String,
    #[serde(default)]
    pub company: String,
    #[serde(default)]
    pub job_title: String,
    #[serde(default)]
    pub emails: Vec<Labeled>,
    #[serde(default)]
    pub phones: Vec<Labeled>,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub birthday: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub groups: Vec<String>,
    #[serde(default)]
    pub source: String,
    /// vCard UID, server resource, change tag and original text (live accounts)
    #[serde(default)]
    pub uid: String,
    #[serde(default)]
    pub href: String,
    #[serde(default)]
    pub etag: String,
    #[serde(default)]
    pub raw: String,
    #[serde(default)]
    pub nickname: String,
    #[serde(default)]
    pub urls: Vec<Labeled>,
    /// Picture file on this computer ("" = initials)
    #[serde(default)]
    pub photo: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub source: String,
    /// CardDAV address book URL (live accounts)
    #[serde(default)]
    pub url: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SourceSettings {
    pub id: String,
    pub auto_import: bool,
    #[serde(default)]
    pub last_import: String,
}

/// What changed in a mail account since the last sync (live accounts):
/// new messages, read/flag changes, and messages gone from the server.
#[derive(Debug, Clone)]
pub struct MailDelta {
    /// When the sync began asking the server (made with the delta, before any
    /// flags are read): read/flag changes made here and sent to the server
    /// after this aren't in it, so they win over what it says.
    pub started: std::time::Instant,
    pub new: Vec<Message>,
    /// (folder, uid, read, flagged) for messages already here
    /// (folder, uid, read, flagged, flag colour; NO_COLOR: as it was)
    pub flags: Vec<(String, u32, bool, bool, u8)>,
    /// (folder, uid) no longer on the server
    pub gone: Vec<(String, u32)>,
    /// Mailboxes read through, as they were (see `FolderMark`)
    pub marks: Vec<(String, FolderMark)>,
}

impl Default for MailDelta {
    fn default() -> Self {
        MailDelta { started: std::time::Instant::now(), new: Vec::new(), flags: Vec::new(), gone: Vec::new(), marks: Vec::new() }
    }
}

/// A read/flag change made here that's on its way to the server (see
/// `Db::flag_guards`): `pushed` is when the server took it (None: not yet).
#[derive(Clone, Debug)]
pub struct FlagGuard {
    pub generation: u64,
    pub changed: std::time::Instant,
    pub pushed: Option<std::time::Instant>,
}

/// How long a change the server never confirmed (no answer at all) keeps
/// winning over what a sync says.
const FLAG_GUARD_MAX: std::time::Duration = std::time::Duration::from_secs(600);

/// A mailbox as the server described it at the last full look (UIDVALIDITY,
/// UIDNEXT, message count and CONDSTORE's HIGHESTMODSEQ). When none of them
/// changed, nothing in it did: the check skips it, as Thunderbird does,
/// instead of listing tens of thousands of messages again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FolderMark {
    pub validity: u32,
    pub uid_next: u32,
    pub exists: u32,
    pub modseq: u64,
}

/// The flag colours, in Apple Mail's order.
pub const FLAG_NAMES: [&str; 7] = ["Red", "Orange", "Yellow", "Green", "Blue", "Purple", "Gray"];
/// "The server didn't say": a flag's colour stays as it was.
pub const NO_COLOR: u8 = u8::MAX;

/// The mail as read at start (see `Db::read_mail`).
pub struct MailRead {
    pub messages: Vec<Message>,
    pub moved: bool,
    pub restyled: bool,
    pub conversations: std::collections::HashMap<i64, String>,
}

/// Everything one importer brings in. Replaces earlier imports from the same source.
#[derive(Default)]
pub struct Imported {
    /// Live mail synced as changes (instead of replacing the account's mail)
    pub mail_delta: Option<MailDelta>,
    pub source: String,
    pub accounts: Vec<Account>,
    pub folders: Vec<Folder>,
    pub messages: Vec<Message>,
    pub calendars: Vec<Calendar>,
    pub events: Vec<Event>,
    pub contacts: Vec<Contact>,
    pub groups: Vec<Group>,
    /// Live accounts: the server is the truth, so earlier read/flag
    /// changes are not carried over.
    pub server_truth: bool,
    /// Parts that could not be read this time ("mail", "calendars",
    /// "contacts"); what was there before is kept.
    pub failed: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Db {
    pub accounts: Vec<Account>,
    pub folders: Vec<Folder>,
    /// Kept in mail.sqlite, not in data.json (read from data.json only once,
    /// to move mail saved by earlier versions).
    #[serde(default, skip_serializing)]
    pub messages: Vec<Message>,
    pub calendars: Vec<Calendar>,
    pub events: Vec<Event>,
    pub contacts: Vec<Contact>,
    pub groups: Vec<Group>,
    #[serde(default)]
    pub sources: Vec<SourceSettings>,
    pub next_id: i64,
    /// "system", "light" or "dark"
    #[serde(default)]
    pub theme: String,
    /// Live mail / calendar / contact accounts (passwords are in the system keychain)
    #[serde(default)]
    pub live: Vec<crate::live::LiveAccount>,
    /// Each live mailbox at the last check (by folder id)
    #[serde(default)]
    pub folder_marks: std::collections::HashMap<String, FolderMark>,
    /// Read/flag changes made here, by (source, folder, uid), until a sync
    /// that began after the server took them: one that began before (the
    /// usual case for a check already running when a message is opened)
    /// still has the old flags and would otherwise undo them, so a message
    /// opened would go back to unread until opened again.
    #[serde(skip)]
    pub flag_guards: std::collections::HashMap<(String, String, u32), FlagGuard>,
    #[serde(skip)]
    pub flag_guard_generation: u64,
    #[serde(default)]
    pub rules: Vec<crate::extras::MailRule>,
    #[serde(default)]
    pub smart_mailboxes: Vec<crate::extras::SmartMailbox>,
    /// VIP senders (email addresses, lower case)
    #[serde(default)]
    pub vips: Vec<String>,
    /// Blocked senders: their mail goes straight to Trash
    #[serde(default)]
    pub blocked: Vec<String>,
    /// Muted conversations (thread keys)
    #[serde(default)]
    pub muted: Vec<String>,
    /// Signatures per account id ("" = On My Computer)
    #[serde(default)]
    pub signatures: Vec<Labeled>,
    #[serde(default)]
    pub smart_groups: Vec<crate::extras::SmartGroup>,
    /// Messages waiting to be sent ("Send Later" and the undo-send delay)
    #[serde(default)]
    pub outbox: Vec<crate::extras::Outgoing>,
    /// Alerts already shown ("<event id>@<time>")
    #[serde(default)]
    pub fired_alerts: Vec<String>,
    /// The contact card that is "me"
    #[serde(default)]
    pub my_card: i64,
    /// The setup guide has been finished (or skipped)
    #[serde(default)]
    pub onboarded: bool,
    /// Version of how message formatting is kept (2: with the sender's styles)
    #[serde(default)]
    pub html_version: u32,
    /// Holds the made-up examples ("Try with sample data")
    #[serde(default)]
    pub is_sample: bool,
    /// The mail is in the database and read as it's needed (the list a page
    /// at a time, counts and conversations by query): `messages` holds only
    /// messages just changed or arrived, until they're saved. Off for sample
    /// mail, checks and tests, which keep all of it in memory.
    #[serde(skip)]
    pub backed: bool,
    /// The sidebar's counts read from the database, kept until the next save.
    #[serde(skip)]
    pub counts_cache: std::sync::Arc<std::sync::Mutex<Option<(u64, MailCounts)>>>,
    /// Calendars subscribed to by web address (read-only, refreshed on sync)
    #[serde(default)]
    pub subscriptions: Vec<Subscription>,
    #[serde(default)]
    pub settings: Settings,
    /// Names and colours chosen here for calendars that come from a server
    /// (calendar id -> (name, colour)); kept over each sync.
    #[serde(default)]
    pub calendar_looks: std::collections::HashMap<String, (String, String)>,
    /// Conversation of each message, worked out again only when mail changes
    #[serde(skip)]
    pub conv_cache: std::sync::Arc<std::sync::Mutex<Option<(u64, std::collections::HashMap<i64, String>)>>>,
    /// Goes up with every change (the app's "revision"): cached results made
    /// at an older one are made again.
    #[serde(skip)]
    pub generation: u64,
    /// Mail in the database: when to look there for reminders next (the
    /// earliest one set, or in a while).
    #[serde(skip)]
    pub next_reminder_look: i64,
    /// The message list's grouped and sorted rows (indexes into `messages`)
    /// for the last mailbox / search / sort asked for, at a generation.
    #[serde(skip)]
    pub page_cache: std::sync::Arc<std::sync::Mutex<Option<(String, u64, usize, std::sync::Arc<PageRows>)>>>,
    /// Everyone in the mail (for recipient suggestions), worked out again only when mail changes
    #[serde(skip)]
    pub people_cache: std::sync::Arc<std::sync::Mutex<Option<(u64, Vec<Person>)>>>,
}

/// A mailbox's list rows before paging: each row's messages (indexes into
/// `Db::messages`, newest first), sorted; with the number of rows and of unread ones.
/// A live account's messages still waiting for their text, from the database
/// (inbox first, newest first), for the download on the sync's thread.
    /// The preview of a message whose text just arrived: from its plain text,
    /// else its HTML, else an invisible mark. Never empty: an empty preview
    /// means "text not fetched yet", and messages with no text of their own
    /// (photos with an empty line of HTML) were fetched again on every check
    /// for new mail, ahead of the new mail itself.
    pub fn fetched_preview(body: &str, html: &str) -> String {
        let p = strip_preview(body);
        if !p.trim().is_empty() {
            return p;
        }
        let h = strip_preview(&crate::mime::html_to_text(html));
        if !h.trim().is_empty() {
            return h;
        }
        "\u{200B}".into()
    }

pub fn partial_wanted(source: &str, folders: &[Folder]) -> Vec<crate::live::Wanted> {
    let filled = crate::mailstore::fill_from_copies(source);
    if filled > 0 {
        eprintln!("almanac: {filled} message(s) filled in from their copies in other folders (not downloaded again)");
    }
    let rank = |k: &str| ["inbox", "sent", "drafts", "archive", "other", "junk", "trash"].iter().position(|x| *x == k).unwrap_or(4);
    let by_id: std::collections::HashMap<&str, &Folder> = folders.iter().map(|f| (f.id.as_str(), f)).collect();
    let mut list: Vec<(usize, i64, crate::live::Wanted, String)> = crate::mailstore::partial_ids_with_message_id(source)
        .into_iter()
        .filter_map(|(id, folder, uid, date, mid)| {
            let f = by_id.get(folder.as_str())?;
            Some((rank(&f.kind), -date, (id, f.path.clone(), uid), mid))
        })
        .collect();
    list.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
    // One copy of each message (new mail lands in several Gmail labels at once):
    // the others are filled in from it next time (fill_from_copies).
    let mut seen = std::collections::HashSet::new();
    list.into_iter().filter(|x| x.3.is_empty() || seen.insert(x.3.clone())).map(|x| x.2).collect()
}

/// Message counts read from the database: by folder, (all, unread, flagged
/// by colour).
#[derive(Clone, Debug, Default)]
pub struct MailCounts {
    pub by_folder: std::collections::HashMap<String, (i64, i64, [i64; 7])>,
    /// Messages from VIPs (outside Junk and Trash)
    pub vip: i64,
}

#[derive(Debug, Default)]
pub struct PageRows {
    pub groups: Vec<Vec<usize>>,
    pub all: usize,
    pub unread: usize,
}

/// Someone met in the mail: as sender, recipient or copied.
#[derive(Clone, Debug, Default)]
pub struct Person {
    pub email: String,
    pub name: String,
    /// Messages they're in
    pub seen: u32,
    /// Messages sent to them from here (counts most: people you write to)
    pub written: u32,
    /// Newest message they're in, seconds
    pub last: i64,
    /// Newest message sent to them from here, seconds (0: never)
    pub last_written: i64,
}

/// "Ann Lee <ann@x.com>, bob@y.org" → [(name, email)]
pub fn split_addresses(list: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut part = String::new();
    let mut quoted = false;
    let mut angle = false;
    let push = |p: &str, out: &mut Vec<(String, String)>| {
        let p = p.trim();
        if p.is_empty() {
            return;
        }
        let (name, email) = match (p.rfind('<'), p.rfind('>')) {
            (Some(a), Some(b)) if b > a => (p[..a].trim().trim_matches('"').trim().to_string(), p[a + 1..b].trim().to_string()),
            _ => (String::new(), p.trim_matches('"').to_string()),
        };
        if email.contains('@') && !email.contains(' ') {
            out.push((name, email));
        }
    };
    for ch in list.chars() {
        match ch {
            '"' => quoted = !quoted,
            '<' => angle = true,
            '>' => angle = false,
            ',' | ';' if !quoted && !angle => {
                push(&part, &mut out);
                part.clear();
                continue;
            }
            _ => {}
        }
        part.push(ch);
    }
    push(&part, &mut out);
    out
}

/// Preferences (the Settings page).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Minutes between checks for new mail and changes (0 = only by hand)
    pub check_minutes: i64,
    pub notify_mail: bool,
    pub notify_alerts: bool,
    /// Seconds to take back a sent message (0 = send straight away)
    pub undo_seconds: i64,
    pub mark_read_on_open: bool,
    pub show_formatted: bool,
    /// Dark mode: show messages on a dark background (those that set their
    /// own colours still get a light page, as in Apple Mail).
    pub dark_messages: bool,
    /// Dark mode: dark logos on see-through pictures made readable (single-
    /// colour ones lightened, others given a light backdrop).
    pub readable_logos: bool,
    /// Load pictures from the internet in messages without asking.
    pub load_remote: bool,
    /// Senders (lower-case addresses) whose remote content always loads.
    pub remote_senders: Vec<String>,
    /// Message list order: date, from, subject, to, attachments, flags or unread
    pub sort_by: String,
    /// Oldest / A first instead of newest / Z first
    pub sort_ascending: bool,
    /// Turn Markdown typed in new messages into formatting
    pub markdown_compose: bool,
    /// Ask before sending a message without a subject (Settings › Mail).
    pub warn_no_subject: bool,
    /// Ask before sending when an attachment is mentioned but none is attached.
    pub warn_missing_attachment: bool,
    /// Attachments too big for the mail account: "ask" to upload them to cloud
    /// storage and send links, "always" upload them, or "never".
    pub cloud_attachments: String,
    /// Lists left with Unsubscribe (their List-Id, or the sender's address).
    pub unsubscribed: Vec<String>,
    /// What the junk filter does with mail it thinks is junk: "mark" (says so
    /// on the message and in the list), "move" (to Junk) or "off".
    pub junk_filter: String,
    /// Where big attachments go: "account" (the mail account's own Google
    /// Drive or OneDrive), "dropbox", "box" or "nextcloud".
    pub cloud_store: String,
    /// The Nextcloud server and user Almanac signed in to (its app password
    /// is in the keychain).
    pub cloud_nextcloud_server: String,
    pub cloud_nextcloud_user: String,
    /// Underline misspelled words while writing
    pub spell_check: bool,
    /// …and grammar problems too
    pub grammar_check: bool,
    /// Words taught to the spelling checker ("Learn Spelling")
    pub spelling_words: Vec<String>,
    /// Keep a copy of every message on this computer (as Apple Mail and
    /// Thunderbird do); off: only the newest messages of each mailbox.
    pub download_all_mail: bool,
    /// "standard" (list beside the message) or "classic" (list above it)
    pub mail_layout: String,
    pub conversations: bool,
    /// mail, calendar or contacts
    pub start_section: String,
    /// -1 = as the system does, 0 = Sunday, 1 = Monday, 6 = Saturday
    pub week_start: i64,
    pub default_calendar: String,
    /// Minutes before; None = no alert
    pub default_alert: Option<i64>,
    pub event_minutes: i64,
    pub time_zone_support: bool,
    pub contacts_sort_first: bool,
    pub contacts_last_first: bool,
    pub calendar_view: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            check_minutes: 5,
            notify_mail: true,
            notify_alerts: true,
            undo_seconds: 8,
            mark_read_on_open: true,
            show_formatted: true,
            dark_messages: true,
            readable_logos: true,
            load_remote: false,
            remote_senders: Vec::new(),
            spell_check: true,
            grammar_check: true,
            spelling_words: Vec::new(),
            sort_by: "date".into(),
            sort_ascending: false,
            markdown_compose: true,
            warn_no_subject: true,
            warn_missing_attachment: true,
            cloud_attachments: "ask".into(),
            unsubscribed: Vec::new(),
            junk_filter: "mark".into(),
            cloud_store: "account".into(),
            cloud_nextcloud_server: String::new(),
            cloud_nextcloud_user: String::new(),
            download_all_mail: true,
            mail_layout: "standard".into(),
            conversations: true,
            start_section: "mail".into(),
            week_start: -1,
            default_calendar: String::new(),
            default_alert: None,
            event_minutes: 60,
            time_zone_support: true,
            contacts_sort_first: false,
            contacts_last_first: false,
            calendar_view: "week".into(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Subscription {
    pub calendar: String,
    pub url: String,
    #[serde(default)]
    pub last_sync: String,
}

pub const BIRTHDAYS: &str = "birthdays";

/// The files behind the sample messages' attachments (made when first shown).
pub fn sample_attachment(name: &str) -> Option<Vec<u8>> {
    let picture = |top: &str, bottom: &str, sun: &str| {
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="640" height="400" viewBox="0 0 640 400"><defs><linearGradient id="s" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{top}"/><stop offset="1" stop-color="{bottom}"/></linearGradient></defs><rect width="640" height="400" fill="url(#s)"/><circle cx="470" cy="150" r="54" fill="{sun}" opacity="0.9"/><path d="M0 300 L140 210 L260 280 L380 190 L520 270 L640 220 L640 400 L0 400 Z" fill="#2b3a2e" opacity="0.85"/></svg>"##
        )
        .into_bytes()
    };
    match name {
        "Sunset.jpg" => Some(picture("#ff9a5a", "#7a3ca8", "#ffd27a")),
        "Lake.jpg" => Some(picture("#9fd8f5", "#1e5d8c", "#ffffff")),
        "Trail.jpg" => Some(picture("#cfe8b0", "#3d6b35", "#fff3b0")),
        "Agenda.pdf" => Some(
            b"%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n\
3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 595 420]/Contents 4 0 R/Resources<</Font<</F1 5 0 R>>>>>>endobj\n\
4 0 obj<</Length 230>>stream\nBT /F1 26 Tf 50 350 Td (Project kickoff - agenda) Tj ET\nBT /F1 15 Tf 50 300 Td (1. Goals and timeline) Tj ET\n\
BT /F1 15 Tf 50 270 Td (2. Budget) Tj ET\nBT /F1 15 Tf 50 240 Td (3. Who owns the launch checklist) Tj ET\nBT /F1 15 Tf 50 210 Td (4. Questions) Tj ET\nendstream endobj\n\
5 0 obj<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>endobj\ntrailer<</Root 1 0 R>>\n%%EOF\n"
                .to_vec(),
        ),
        _ => None,
    }
}

/// Subject without "Re:", "Fwd:" and similar, for grouping conversations.
pub fn thread_key(subject: &str) -> String {
    let mut s = subject.trim();
    loop {
        let lower = s.to_lowercase();
        let mut cut = None;
        for p in ["re:", "fwd:", "fw:", "aw:", "sv:", "wg:", "re :", "tr:"] {
            if lower.starts_with(p) {
                cut = Some(p.len());
                break;
            }
        }
        match cut {
            Some(n) => s = s[n..].trim_start(),
            None => break,
        }
    }
    s.to_lowercase()
}


/// The email addresses in a To/Cc/From text, lower case.
fn addresses(s: &str) -> Vec<String> {
    s.split(',')
        .filter_map(|p| {
            let p = p.trim();
            let a = match (p.find('<'), p.rfind('>')) {
                (Some(i), Some(j)) if j > i => &p[i + 1..j],
                _ => p,
            };
            a.contains('@').then(|| a.trim().to_lowercase())
        })
        .collect()
}

/// Which conversation each message belongs to (message id → conversation
/// key), worked out as mail apps do: messages are linked through their
/// Message-ID / In-Reply-To / References headers. A reply ("Re: …") that
/// lacks those headers joins an earlier message with the same subject that
/// shares a person with it. Different mail that merely has the same subject
/// ("Your order", "Invoice") stays apart.
/// What a list read a page at a time lists (see `Db::window_plan`).
#[derive(Clone, Debug)]
pub struct WindowPlan {
    pub folders: Vec<String>,
    /// Any other list (a search, VIPs, Flagged, another order): its messages
    /// as SQL (see `Db::list_filter`), the folders then unused.
    pub filter: Option<(String, Vec<crate::mailstore::Arg>)>,
    pub sort_by: String,
    pub ascending: bool,
    /// The mail's generation (Db::generation): a list worked out whole is kept
    /// until the mail changes.
    pub generation: u64,
    pub threaded: bool,
    /// Only rows with unread mail (or the one with this message).
    pub unread_or: Option<i64>,
    pub show_folder: bool,
}

/// Reads rows `offset`… of a list (on any thread): the messages of each row
/// (a conversation, or one message), and with `counted`, how many rows there
/// are and how many have unread mail.
pub fn fetch_window(plan: &WindowPlan, offset: usize, count: usize, counted: bool) -> (Vec<Vec<Message>>, i64, i64) {
    if plan.filter.is_some() {
        let whole = whole_list(plan);
        let end = (offset + count).min(whole.rows.len());
        let groups = whole.rows.get(offset..end).unwrap_or(&[]).iter().map(|r| r.messages()).collect();
        let (total, unread) = if counted { (whole.rows.len() as i64, whole.unread) } else { (-1, -1) };
        return (groups, total, unread);
    }
    if plan.threaded {
        let (total, unread) = if counted { crate::mailstore::conv_count(&plan.folders, plan.unread_or) } else { (-1, -1) };
        let groups = crate::mailstore::conv_page(&plan.folders, plan.unread_or, plan.ascending, offset as i64, count as i64)
            .into_iter()
            .map(|(folder, tkey)| crate::mailstore::conv_messages(&folder, &tkey))
            .collect();
        return (groups, total, unread);
    }
    // (From the small table once it's ready: a few ms anywhere in the list.)
    if crate::mailstore::conversations_ready() {
        let (ids, total) = crate::mailstore::date_page(&plan.folders, plan.unread_or, plan.ascending, offset as i64, count as i64, counted);
        let unread = if counted { crate::mailstore::date_page(&plan.folders, Some(-1), false, 0, 0, true).1 } else { -1 };
        let groups = crate::mailstore::by_ids(&ids).into_iter().map(|m| vec![m]).collect();
        return (groups, total, unread);
    }
    let marks = (1..=plan.folders.len()).map(|i| format!("?{i}")).collect::<Vec<_>>().join(",");
    let mut args: Vec<crate::mailstore::Arg> = plan.folders.iter().map(|f| crate::mailstore::Arg::Text(f.clone())).collect();
    let base = format!("folder IN ({marks})");
    let unread = if counted { crate::mailstore::count(&format!("{base} AND read = 0"), &args) } else { -1 };
    let filter = match plan.unread_or {
        Some(keep) => {
            args.push(crate::mailstore::Arg::Integer(keep));
            format!("{base} AND (read = 0 OR id = ?{})", args.len())
        }
        None => base,
    };
    let total = if counted { crate::mailstore::count(&filter, &args) } else { -1 };
    let order = if plan.ascending { "date ASC, id ASC" } else { "date DESC, id DESC" };
    let groups = crate::mailstore::rows(&filter, &args, order, count as i64, offset as i64).into_iter().map(|m| vec![m]).collect();
    (groups, total, unread)
}

/// One row of a list worked out whole: a message, or a conversation (folder, thread key).
#[derive(Clone, Debug)]
pub enum ListItem {
    Message(i64),
    Conversation(String, String),
}

impl ListItem {
    fn messages(&self) -> Vec<Message> {
        match self {
            ListItem::Message(id) => crate::mailstore::by_id(*id).into_iter().collect(),
            ListItem::Conversation(f, t) => crate::mailstore::conv_messages(f, t),
        }
    }
}

/// A list (a search, VIPs, another order…) worked out whole, once: its rows in
/// order and how many have unread mail. Kept until the mail changes or another
/// list is asked for, so each page after the first is only its rows' reading.
pub struct WholeList {
    pub rows: Vec<ListItem>,
    pub unread: i64,
}

fn whole_list(plan: &WindowPlan) -> std::sync::Arc<WholeList> {
    use std::sync::{Arc, Mutex};
    static CACHE: Mutex<Option<(String, Arc<WholeList>)>> = Mutex::new(None);
    let Some((filter, args)) = &plan.filter else { return Arc::new(WholeList { rows: Vec::new(), unread: 0 }) };
    let key = format!("{}|{}|{:?}|{}|{}|{}", plan.threaded, filter, args, plan.sort_by, plan.ascending, plan.generation);
    // (Worked out under the lock: two pages asked for at once wait for one reading.)
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((k, w)) = cache.as_ref() {
        if *k == key {
            return w.clone();
        }
    }
    let _t = crate::Timed::new("working out a list whole");
    let (msg_order, conv_order) = crate::mailstore::list_orders(&plan.sort_by, plan.ascending);
    let whole = if plan.threaded {
        let rows = crate::mailstore::filtered_conv_page(filter, args, &conv_order, 0, i64::MAX)
            .into_iter()
            .map(|(f, t)| ListItem::Conversation(f, t))
            .collect();
        let (_, unread) = crate::mailstore::filtered_conv_count(filter, args);
        WholeList { rows, unread }
    } else {
        let rows = crate::mailstore::row_ids(filter, args, &msg_order).into_iter().map(ListItem::Message).collect();
        let unread = crate::mailstore::count(&format!("{filter} AND read = 0"), args);
        WholeList { rows, unread }
    };
    let whole = Arc::new(whole);
    *cache = Some((key, whole.clone()));
    whole
}

/// The row of message `id` in a list read a page at a time (see
/// `Db::window_plan`), or -1 when it isn't in it (on any thread). `msg`: its
/// folder and date.
pub fn position_in(plan: &WindowPlan, id: i64, msg: Option<(String, i64)>) -> i64 {
    if plan.filter.is_some() {
        let whole = whole_list(plan);
        let conv = if plan.threaded { crate::mailstore::conv_of(id) } else { None };
        return whole
            .rows
            .iter()
            .position(|r| match r {
                ListItem::Message(m) => *m == id,
                ListItem::Conversation(f, t) => conv.as_ref().is_some_and(|(cf, ct)| cf == f && ct == t),
            })
            .map(|p| p as i64)
            .unwrap_or(-1);
    }
    if plan.threaded {
        if !crate::mailstore::conversations_ready() {
            return -1;
        }
        return crate::mailstore::conv_position(&plan.folders, plan.unread_or, plan.ascending, id).unwrap_or(-1);
    }
    let Some((folder, date)) = msg else { return -1 };
    if !plan.folders.contains(&folder) {
        return -1;
    }
    let marks = (1..=plan.folders.len()).map(|i| format!("?{i}")).collect::<Vec<_>>().join(",");
    let mut args: Vec<crate::mailstore::Arg> = plan.folders.iter().map(|f| crate::mailstore::Arg::Text(f.clone())).collect();
    let mut filter = format!("folder IN ({marks})");
    if plan.unread_or.is_some() {
        args.push(crate::mailstore::Arg::Integer(id));
        filter = format!("{filter} AND (read = 0 OR id = ?{})", args.len());
    }
    args.push(crate::mailstore::Arg::Integer(date));
    args.push(crate::mailstore::Arg::Integer(id));
    let (a, b) = (args.len() - 1, args.len());
    let cmp = if plan.ascending { "<" } else { ">" };
    crate::mailstore::count(&format!("{filter} AND (date {cmp} ?{a} OR (date = ?{a} AND id {cmp} ?{b}))"), &args)
}

pub fn conversation_keys(msgs: &[Message]) -> std::collections::HashMap<i64, String> {
    use std::collections::HashMap;
    // Message-IDs as numbers (hashed as they're read, lower case, without
    // copying them): a mailbox of a few hundred thousand messages is grouped in
    // a fraction of a second.
    fn hash_lower(s: &str) -> u64 {
        // FNV-1a over the lower-cased bytes.
        let mut h: u64 = 0xcbf29ce484222325;
        for b in s.bytes() {
            h ^= b.to_ascii_lowercase() as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }
    /// The Message-IDs in a header, as they're written there.
    fn each_id<'s>(s: &'s str, mut f: impl FnMut(&'s str)) {
        let s = s.trim();
        if s.contains('<') {
            for part in s.split('<').skip(1) {
                let id = part.split('>').next().unwrap_or("").trim();
                if !id.is_empty() {
                    f(id);
                }
            }
        } else {
            for id in s.split_whitespace() {
                f(id);
            }
        }
    }
    struct Ids<'a> {
        index: HashMap<u64, u32>,
        names: Vec<std::borrow::Cow<'a, str>>,
        hashes: Vec<u64>,
        parent: Vec<u32>,
    }
    impl<'a> Ids<'a> {
        fn of(&mut self, name: std::borrow::Cow<'a, str>) -> u32 {
            let h = hash_lower(&name);
            if let Some(&i) = self.index.get(&h) {
                return i;
            }
            let i = self.names.len() as u32;
            self.names.push(name);
            self.hashes.push(h);
            self.parent.push(i);
            self.index.insert(h, i);
            i
        }
        fn find(&mut self, mut x: u32) -> u32 {
            while self.parent[x as usize] != x {
                let up = self.parent[x as usize];
                self.parent[x as usize] = self.parent[up as usize];
                x = up;
            }
            x
        }
        fn union(&mut self, a: u32, b: u32) {
            let (ra, rb) = (self.find(a), self.find(b));
            if ra != rb {
                // The same one stays the root whichever order they come in, so the key is stable.
                if self.hashes[ra as usize] < self.hashes[rb as usize] {
                    self.parent[rb as usize] = ra;
                } else {
                    self.parent[ra as usize] = rb;
                }
            }
        }
    }
    let n = msgs.len();
    let mut ids = Ids { index: HashMap::with_capacity(n * 2), names: Vec::with_capacity(n * 2), hashes: Vec::with_capacity(n * 2), parent: Vec::with_capacity(n * 2) };
    let mut node: Vec<u32> = Vec::with_capacity(n);
    for m in msgs {
        let mut own: Option<&str> = None;
        each_id(&m.message_id, |id| {
            if own.is_none() {
                own = Some(id)
            }
        });
        let own = match own {
            Some(id) => ids.of(std::borrow::Cow::Borrowed(id)),
            None => ids.of(std::borrow::Cow::Owned(format!("#{}", m.id))),
        };
        let mut refs: Vec<&str> = Vec::new();
        each_id(&m.in_reply_to, |id| refs.push(id));
        each_id(&m.references, |id| refs.push(id));
        for r in refs {
            let r = ids.of(std::borrow::Cow::Borrowed(r));
            ids.union(own, r);
        }
        node.push(own);
    }
    // Replies without reference headers (rare): same subject and a shared
    // person. The people are only looked at for those.
    let people = |m: &Message| -> Vec<String> { addresses(&m.from_email).into_iter().chain(addresses(&m.to)).chain(addresses(&m.cc)).collect() };
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| msgs[i].date);
    let mut by_subject: HashMap<String, Vec<usize>> = HashMap::new();
    for i in order {
        let m = &msgs[i];
        let subj = thread_key(&m.subject);
        if subj.is_empty() {
            continue;
        }
        if m.in_reply_to.is_empty() && m.references.is_empty() && subj != m.subject.trim().to_lowercase() {
            if let Some(list) = by_subject.get(&subj) {
                let mine = people(m);
                if let Some(&other) = list.iter().find(|&&j| people(&msgs[j]).iter().any(|p| mine.contains(p))) {
                    ids.union(node[i], node[other]);
                }
            }
        }
        by_subject.entry(subj).or_default().push(i);
    }
    let mut keys: HashMap<u32, String> = HashMap::new();
    let mut out = HashMap::with_capacity(n);
    for (m, x) in msgs.iter().zip(node) {
        let root = ids.find(x);
        let key = keys.entry(root).or_insert_with(|| ids.names[root as usize].to_lowercase()).clone();
        out.insert(m.id, key);
    }
    out
}

/// Where the app keeps its data (and the folder it used before it was named Almanac).
#[cfg_attr(test, allow(dead_code))]
fn data_dirs() -> (PathBuf, PathBuf) {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    if cfg!(target_os = "windows") {
        let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or(home);
        (base.join("Almanac"), base.join("CraftPIM"))
    } else if cfg!(target_os = "macos") {
        let base = home.join("Library/Application Support");
        (base.join("Almanac"), base.join("CraftPIM"))
    } else {
        let base = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".local/share"));
        (base.join("almanac"), base.join("craft-pim"))
    }
}

pub fn data_dir() -> PathBuf {
    // Tests never read or write the real mail.
    // (A copy named for a timing check, never the real folder.)
    #[cfg(test)]
    return std::env::var_os("ALMANAC_TEST_DATA").map(PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join(format!("almanac-test-data-{}", std::process::id())));
    #[cfg(not(test))]
    {
    let (dir, old) = data_dirs();
    // First start after the rename: move the old folder over.
    if !dir.exists() && old.join("data.json").exists() {
        if std::fs::rename(&old, &dir).is_err() {
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::copy(old.join("data.json"), dir.join("data.json"));
        }
    }
    dir
    }
}

fn data_file() -> PathBuf {
    data_dir().join("data.json")
}

pub fn fmt_dt(dt: NaiveDateTime) -> String {
    dt.format("%Y-%m-%dT%H:%M").to_string()
}

pub fn parse_dt(s: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M")
        .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S"))
        .ok()
        .or_else(|| {
            NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .ok()
                .and_then(|d| d.and_hms_opt(0, 0, 0))
        })
}

pub fn strip_preview(body: &str) -> String {
    if crate::mime::looks_like_html(body) {
        return strip_preview(&crate::mime::html_to_text(body));
    }
    let mut out = String::new();
    for line in body.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('>') {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(l);
        if out.len() > 220 {
            break;
        }
    }
    out.chars().take(200).collect()
}

fn lc(s: &str) -> String {
    s.to_lowercase()
}

impl Db {
    /// Everything but the mail (quick: settings, accounts, calendars,
    /// contacts), and mail kept in data.json by earlier versions if any.
    pub fn load_without_mail() -> Option<(Db, Vec<Message>)> {
        let text = std::fs::read_to_string(data_file()).ok()?;
        let mut db = serde_json::from_str::<Db>(&text).ok()?;
        let legacy = std::mem::take(&mut db.messages);
        db.ensure_birthdays_calendar();
        Some((db, legacy))
    }

    /// The mail (the slow part of starting, done in the background): the
    /// messages, whether they came from data.json (moved into the database),
    /// and each message's conversation, worked out ahead.
    pub fn read_mail(legacy: Vec<Message>, html_version: u32) -> MailRead {
        let mut messages = crate::mailstore::load();
        let moved = messages.is_empty() && !legacy.is_empty();
        if moved {
            let _ = std::fs::copy(data_file(), data_dir().join("data.json.before-sqlite"));
            messages = legacy;
        }
        let restyled = html_version < 3;
        if restyled {
            for m in messages.iter_mut().filter(|m| m.source.starts_with("live:") && m.uid > 0) {
                if (html_version < 2 && !m.html.is_empty()) || (m.html.is_empty() && !m.attachments.is_empty()) {
                    m.partial = true;
                }
            }
        }
        let conversations = {
            let _t = crate::Timed::new("grouping conversations (ahead)");
            conversation_keys(&messages)
        };
        MailRead { messages, moved, restyled, conversations }
    }

    /// The mail read in the background, put in place.
    pub fn take_mail(&mut self, read: MailRead) -> bool {
        self.messages = read.messages;
        *self.conv_cache.lock().unwrap() = Some((self.messages_fingerprint(), read.conversations));
        if read.restyled {
            self.html_version = 3;
        }
        self.generation += 1;
        self.strip_sample() || read.moved || read.restyled
    }

    pub fn load() -> Db {
        if let Ok(text) = std::fs::read_to_string(data_file()) {
            let parsed = {
                let _t = crate::Timed::new("reading data.json");
                serde_json::from_str::<Db>(&text)
            };
            if let Ok(mut db) = parsed {
                let from_json = std::mem::take(&mut db.messages);
                db.messages = crate::mailstore::load();
                // Earlier versions kept mail in data.json: move it into the
                // database once (keeping a copy of the old file).
                let moved = db.messages.is_empty() && !from_json.is_empty();
                if moved {
                    let _ = std::fs::copy(data_file(), data_dir().join("data.json.before-sqlite"));
                    db.messages = from_json;
                }
                db.ensure_birthdays_calendar();
                let restyle = db.html_version < 3;
                if restyle {
                    for m in db.messages.iter_mut().filter(|m| m.source.starts_with("live:") && m.uid > 0) {
                        // v2: styles kept. v3: emails with many pictures of their own keep
                        // their formatting (it was dropped as too big).
                        if (db.html_version < 2 && !m.html.is_empty()) || (m.html.is_empty() && !m.attachments.is_empty()) {
                            m.partial = true;
                        }
                    }
                    db.html_version = 3;
                }
                if db.strip_sample() || moved || restyle {
                    db.save();
                }
                return db;
            }
        }
        let db = Db::empty();
        db.save();
        db
    }

    /// A fresh start: "On My Computer" with empty mailboxes and calendars.
    pub fn empty() -> Db {
        let mut db = Db::default();
        db.accounts.push(Account { id: "local".into(), name: "On My Computer".into(), email: String::new(), source: SOURCE_LOCAL.into() });
        for kind in ["inbox", "drafts", "sent", "junk", "trash", "archive"] {
            db.folders.push(Folder { id: format!("local/{kind}"), account: "local".into(), name: kind_name(kind).into(), kind: kind.into(), ..Default::default() });
        }
        for (id, name, color) in [("home", "Home", "#2f7cf6"), ("work", "Work", "#e8453c")] {
            db.calendars.push(Calendar { id: id.into(), name: name.into(), color: color.into(), visible: true, source: SOURCE_LOCAL.into(), ..Default::default() });
        }
        db.ensure_birthdays_calendar();
        db.next_id = 1000;
        db
    }

    /// Earlier versions started with made-up examples. Removes them (and
    /// only them): items that came with the sample all have small ids.
    pub fn strip_sample(&mut self) -> bool {
        if self.backed {
            return false;
        }
        // Only the examples older versions put there on their own; sample
        // data chosen in the setup guide stays.
        let looks_sample = !self.onboarded
            && self.messages.iter().any(|m| m.source == SOURCE_LOCAL && m.from_email == "riley@example.org" && m.subject == "Project kickoff on Thursday");
        if !looks_sample {
            return false;
        }
        let limit = Db::sample().next_id;
        let local = |src: &str| src == SOURCE_LOCAL || src.is_empty();
        self.messages.retain(|m| !(local(&m.source) && m.id <= limit));
        self.events.retain(|e| !(local(&e.source) && e.id <= limit));
        self.contacts.retain(|c| !(local(&c.source) && c.id <= limit));
        self.groups.retain(|g| !(local(&g.source) && ["friends", "coworkers", "family"].contains(&g.id.as_str())));
        self.calendars.retain(|c| !(c.source == SOURCE_LOCAL && c.id == "family"));
        if let Some(a) = self.accounts.iter_mut().find(|a| a.id == "local") {
            if a.email == "me@example.com" {
                a.email.clear();
            }
        }
        self.is_sample = false;
        self.next_id = self.next_id.max(1000);
        true
    }

    /// A message's text, formatted text and invitation: from memory if it's
    /// there (just arrived, or sample mail), otherwise from the database.
    pub fn text_of(&self, id: i64) -> (String, String, String) {
        if let Some(m) = self.messages.iter().find(|m| m.id == id).filter(|m| crate::mailstore::has_text(m)) {
            return (m.body.clone(), m.html.clone(), m.invite.clone());
        }
        crate::mailstore::text(id).unwrap_or_default()
    }

    /// After saving: the text lives in the database now, so memory keeps only
    /// a preview of it.
    pub fn forget_saved_text(&mut self) {
        for m in self.messages.iter_mut().filter(|m| crate::mailstore::has_text(m)) {
            if m.preview.is_empty() {
                m.preview = strip_preview(&m.body);
            }
            m.body = String::new();
            m.html = String::new();
            m.invite = String::new();
        }
        // (Kept list headers too: read from the database when needed.)
        for m in self.messages.iter_mut().filter(|m| !m.list_heads.is_empty()) {
            m.list_heads = String::new();
        }
    }

    /// What the list shows under the subject.
    fn preview_of(&self, m: &Message) -> String {
        if m.preview.is_empty() { strip_preview(&m.body) } else { m.preview.clone() }
    }

    /// Messages whose text matches `query` (the database's search index; the
    /// text in memory when there's no database).
    fn text_hits(&self, query: &str) -> Option<std::collections::HashSet<i64>> {
        crate::mailstore::search(query)
    }

    /// Saves everything; false when the mail couldn't be saved.
    pub fn save(&self) -> bool {
        let dir = data_dir();
        let _ = std::fs::create_dir_all(&dir);
        let mail_saved = crate::mailstore::save(&self.messages);
        if let Ok(text) = serde_json::to_string(self) {
            let tmp = dir.join("data.json.tmp");
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(&tmp, data_file());
            }
        }
        mail_saved
    }

    /// After saving (mail in the database): the messages in memory are let
    /// go; what's shown is read from the database from now on.
    pub fn release_saved(&mut self) {
        if !self.backed || self.messages.is_empty() {
            return;
        }
        let ids: Vec<i64> = self.messages.iter().map(|m| m.id).collect();
        crate::mailstore::forget(&ids);
        self.messages.clear();
    }

    // ------------------------------------------------ mail in the database

    /// A message: from memory if it's there (just changed or arrived), else
    /// from the database (unless it's waiting to be deleted there).
    pub fn get(&self, id: i64) -> Option<Message> {
        if let Some(m) = self.messages.iter().find(|m| m.id == id) {
            return Some(m.clone());
        }
        if self.backed && !crate::mailstore::is_tracked(id) {
            return crate::mailstore::by_id(id);
        }
        None
    }

    /// Every message of a selection (newest first), from the database too.
    /// The messages in a mailbox from one sender (found with a search for
    /// their address, then matched exactly).
    pub fn messages_from(&self, selection: &str, email: &str) -> Vec<Message> {
        let e = email.trim().to_lowercase();
        if e.is_empty() {
            return Vec::new();
        }
        self.query_messages(selection, &e, None, "date", false, 100_000).0.into_iter().filter(|m| m.from_email.to_lowercase() == e).collect()
    }

    pub fn messages_of(&self, selection: &str) -> Vec<Message> {
        self.query_messages(selection, "", None, "date", false, 50_000_000).0
    }

    /// Folder ids of a kind, for queries.
    fn folders_of_kind(&self, kinds: &[&str]) -> Vec<String> {
        self.folders.iter().filter(|f| kinds.contains(&f.kind.as_str())).map(|f| f.id.clone()).collect()
    }

    /// A mailbox selection as SQL over the list's columns (None: worked out
    /// here, for smart mailboxes).
    fn selection_sql(&self, selection: &str) -> Option<(String, Vec<crate::mailstore::Arg>)> {
        use crate::mailstore::Arg;
        let mut args: Vec<Arg> = Vec::new();
        let mut in_list = |ids: Vec<String>, args: &mut Vec<Arg>| -> String {
            if ids.is_empty() {
                return "0".into();
            }
            let marks: Vec<String> = ids.into_iter().map(|id| {
                args.push(Arg::Text(id));
                format!("?{}", args.len())
            }).collect();
            marks.join(", ")
        };
        let not_in = |ids: Vec<String>, args: &mut Vec<Arg>, in_list: &mut dyn FnMut(Vec<String>, &mut Vec<Arg>) -> String| -> String {
            if ids.is_empty() { "1".into() } else { format!("folder NOT IN ({})", in_list(ids, args)) }
        };
        let sql = if selection.starts_with("smart:") {
            return None;
        } else if let Some(kind) = selection.strip_prefix("kind:") {
            match kind {
                "vip" => {
                    let vips: Vec<String> = self.vips.iter().map(|v| v.to_lowercase()).collect();
                    let who = in_list(vips, &mut args);
                    let skip = not_in(self.folders_of_kind(&["trash", "junk"]), &mut args, &mut in_list);
                    format!("lower(from_email) IN ({who}) AND {skip}")
                }
                "flagged" => format!("flagged = 1 AND {}", not_in(self.folders_of_kind(&["trash"]), &mut args, &mut in_list)),
                k if k.starts_with("flag") => {
                    let c: i64 = k[4..].parse().unwrap_or(0);
                    let skip = not_in(self.folders_of_kind(&["trash"]), &mut args, &mut in_list);
                    if c == 0 {
                        format!("flagged = 1 AND {skip} AND id NOT IN (SELECT id FROM flag_colors)")
                    } else {
                        format!("flagged = 1 AND {skip} AND id IN (SELECT id FROM flag_colors WHERE color = {c})")
                    }
                }
                "unread" => format!("read = 0 AND folder IN ({})", in_list(self.folders_of_kind(&["inbox"]), &mut args)),
                k => format!("folder IN ({})", in_list(self.folders_of_kind(&[k]), &mut args)),
            }
        } else if let Some(f) = selection.strip_prefix("folder:") {
            args.push(Arg::Text(f.to_string()));
            "folder = ?1".to_string()
        } else {
            "0".to_string()
        };
        Some((sql, args))
    }

    /// The messages of a selection matching a search (and, with
    /// `unread_or`, unread or that one), in the order asked for, the first
    /// `limit`: from the database, with those in memory in their place. Also
    /// whether there are more.
    /// A list's messages as SQL over the list's columns (the mailbox or kind,
    /// the search, only unread or the one open), or None for a smart mailbox
    /// (its conditions are checked here, not in SQL).
    fn list_filter(&self, selection: &str, query: &str, unread_or: Option<i64>) -> Option<(String, Vec<crate::mailstore::Arg>)> {
        use crate::mailstore::Arg;
        let (base, mut args) = self.selection_sql(selection)?;
        let mut sql = format!("({base})");
        if !query.trim().is_empty() {
            args.push(Arg::Text(format!("%{}%", query.trim())));
            let n = args.len();
            let mut q = format!(" AND (subject LIKE ?{n} OR from_name LIKE ?{n} OR from_email LIKE ?{n} OR to_addr LIKE ?{n}");
            let words = crate::mailstore::match_expr(query);
            if !words.is_empty() {
                args.push(Arg::Text(words));
                q.push_str(&format!(" OR id IN (SELECT rowid FROM search WHERE search MATCH ?{})", args.len()));
            }
            q.push(')');
            sql.push_str(&q);
        }
        if let Some(k) = unread_or {
            args.push(Arg::Integer(k));
            sql.push_str(&format!(" AND (read = 0 OR id = ?{})", args.len()));
        }
        Some((sql, args))
    }

    fn query_messages(&self, selection: &str, query: &str, unread_or: Option<i64>, sort_by: &str, ascending: bool, limit: usize) -> (Vec<Message>, bool) {
        use crate::mailstore::Arg;
        let kinds = self.folder_kinds();
        let q = lc(query.trim());
        let hits = if q.is_empty() { None } else { self.text_hits(query) };
        let fits = |m: &Message| {
            self.message_in_k(m, selection, Some(&kinds))
                && (q.is_empty()
                    || lc(&m.subject).contains(&q)
                    || lc(&m.from_name).contains(&q)
                    || lc(&m.from_email).contains(&q)
                    || lc(&m.to).contains(&q)
                    || hits.as_ref().map(|h| h.contains(&m.id)).unwrap_or_else(|| lc(&m.body).contains(&q)))
                && unread_or.is_none_or(|k| !m.read || m.id == k)
        };
        let here: std::collections::HashSet<i64> = self.messages.iter().map(|m| m.id).collect();
        // (Taken out of memory but not yet deleted in the database: left out.)
        let deleting: std::collections::HashSet<i64> = crate::mailstore::tracked_ids().into_iter().filter(|id| !here.contains(id)).collect();
        let dir = if ascending { "ASC" } else { "DESC" };
        let order = match sort_by {
            "from" => format!("lower(CASE WHEN from_name = '' THEN from_email ELSE from_name END) {dir}, date DESC"),
            "subject" => format!("lower(subject) {dir}, date DESC"),
            "to" => format!("lower(to_addr) {dir}, date DESC"),
            "attachments" => format!("(attachments <> '[]') {dir}, date DESC"),
            "flags" => format!("flagged {dir}, date DESC"),
            "unread" => format!("(read = 0) {dir}, date DESC"),
            _ => format!("date {dir}, id {dir}"),
        };
        let mut extra = String::new();
        let mut extra_args: Vec<Arg> = Vec::new();
        if !q.is_empty() {
            let words = crate::mailstore::match_expr(query);
            extra_args.push(Arg::Text(format!("%{}%", query.trim())));
            let n = "?{N}";
            extra = format!(" AND (subject LIKE {n} OR from_name LIKE {n} OR from_email LIKE {n} OR to_addr LIKE {n}");
            if !words.is_empty() {
                extra_args.push(Arg::Text(words));
                extra.push_str(" OR id IN (SELECT rowid FROM search WHERE search MATCH ?{M})");
            }
            extra.push(')');
        }
        if let Some(k) = unread_or {
            extra_args.push(Arg::Integer(k));
            extra.push_str(" AND (read = 0 OR id = ?{K})");
        }
        let with_extra = |base: String, mut args: Vec<Arg>| -> (String, Vec<Arg>) {
            let start = args.len();
            let mut sql = format!("({base}){extra}");
            let mut i = start;
            for (mark, _) in [("?{N}", ()), ("?{M}", ()), ("?{K}", ())] {
                if sql.contains(mark) {
                    i += 1;
                    sql = sql.replace(mark, &format!("?{i}"));
                }
            }
            args.extend(extra_args.iter().cloned());
            (sql, args)
        };
        let mut out: Vec<Message>;
        let mut more;
        match self.selection_sql(selection) {
            Some((base, args)) => {
                let (filter, args) = with_extra(base, args);
                let want = limit + self.messages.len() + 1;
                out = crate::mailstore::rows(&filter, &args, &order, want as i64, 0);
                more = out.len() >= want;
                out.retain(|m| !here.contains(&m.id) && !deleting.contains(&m.id));
            }
            None => {
                // A smart mailbox: the newest messages read a stretch at a time
                // and checked here.
                let (filter, args) = with_extra("1".into(), Vec::new());
                out = Vec::new();
                more = false;
                let mut offset = 0i64;
                while out.len() <= limit && offset < 200_000 {
                    let chunk = crate::mailstore::rows(&filter, &args, &order, 2000, offset);
                    let n = chunk.len();
                    out.extend(chunk.into_iter().filter(|m| !here.contains(&m.id) && !deleting.contains(&m.id) && fits(m)));
                    offset += n as i64;
                    if n < 2000 {
                        break;
                    }
                    more = true;
                }
            }
        }
        out.extend(self.messages.iter().filter(|m| fits(m)).cloned());
        let key = |m: &Message| -> (i64, String) {
            match sort_by {
                "from" => (0, lc(if m.from_name.is_empty() { &m.from_email } else { &m.from_name })),
                "subject" => (0, lc(&m.subject)),
                "to" => (0, lc(&m.to)),
                "attachments" => (!m.attachments.is_empty() as i64, String::new()),
                "flags" => (m.flagged as i64, String::new()),
                "unread" => (!m.read as i64, String::new()),
                _ => (m.date, String::new()),
            }
        };
        out.sort_by(|a, b| {
            let k = if ascending { key(a).cmp(&key(b)) } else { key(b).cmp(&key(a)) };
            k.then(b.date.cmp(&a.date)).then(b.id.cmp(&a.id))
        });
        if out.len() > limit {
            more = true;
            out.truncate(limit);
        }
        (out, more)
    }

    /// The list a page at a time, from the database (see `message_page`).
    #[allow(clippy::too_many_arguments)]
    fn message_page_backed(&self, selection: &str, query: &str, threaded: bool, sort_by: &str, ascending: bool, unread_only: bool, keep: i64, limit: usize) -> Value {
        let unread_or = unread_only.then_some(keep);
        let mut want = (limit.max(50) * if threaded { 2 } else { 1 }) + 1;
        let (msgs, more, groups) = loop {
            let (msgs, more) = self.query_messages(selection, query, unread_or, sort_by, ascending, want);
            let groups: Vec<Vec<usize>> = if threaded {
                let keys = conversation_keys(&msgs);
                let mut order: Vec<Vec<usize>> = Vec::new();
                let mut at: std::collections::HashMap<&String, usize> = Default::default();
                for (i, m) in msgs.iter().enumerate() {
                    match keys.get(&m.id) {
                        Some(k) if !k.is_empty() => match at.get(k) {
                            Some(&g) => order[g].push(i),
                            None => {
                                at.insert(k, order.len());
                                order.push(vec![i]);
                            }
                        },
                        _ => order.push(vec![i]),
                    }
                }
                order
            } else {
                (0..msgs.len()).map(|i| vec![i]).collect()
            };
            if groups.len() > limit || !more || want > limit * 32 + 64 {
                break (msgs, more, groups);
            }
            want *= 2;
        };
        // The totals (messages; a conversation's count is how many of its messages are on the page).
        // (Mailboxes and kinds of mailbox: from the counts the database keeps.)
        let counted: Option<Vec<String>> = if !query.trim().is_empty() {
            None
        } else if let Some(f) = selection.strip_prefix("folder:") {
            Some(vec![f.to_string()])
        } else if let Some(k) = selection.strip_prefix("kind:").filter(|k| !["vip", "flagged", "unread"].contains(k) && !k.starts_with("flag")) {
            Some(self.folders_of_kind(&[k]))
        } else {
            None
        };
        let (all, unread) = match (counted, self.selection_sql(selection)) {
            (Some(folders), _) => {
                let c = self.mail_counts();
                let pick = |i: usize| folders.iter().filter_map(|f| c.by_folder.get(f)).map(|v| if i == 0 { v.0 } else { v.1 }).sum::<i64>().max(0) as usize;
                if unread_only {
                    (pick(1), pick(1))
                } else {
                    (pick(0), pick(1))
                }
            }
            (None, Some((base, args))) if query.trim().is_empty() => {
                let all = crate::mailstore::count(&base, &args);
                let unread = crate::mailstore::count(&format!("({base}) AND read = 0"), &args);
                (all as usize, unread as usize)
            }
            _ => (groups.len(), groups.iter().filter(|g| g.iter().any(|&i| !msgs[i].read)).count()),
        };
        let shown = groups.len().min(limit.max(1));
        let mut rows_groups: Vec<&Vec<usize>> = groups.iter().take(shown).collect();
        // The open message, if it's further down.
        if keep > 0 && !rows_groups.iter().any(|g| g.iter().any(|&i| msgs[i].id == keep)) {
            if let Some(g) = groups.iter().find(|g| g.iter().any(|&i| msgs[i].id == keep)) {
                rows_groups.push(g);
            }
        }
        let show_folder = selection.starts_with("kind:") && self.accounts.len() > 1;
        let out: Vec<Value> = rows_groups
            .iter()
            .map(|g| {
                let group: Vec<&Message> = g.iter().map(|&i| &msgs[i]).collect();
                let mut row = self.list_row(&group, show_folder);
                if threaded {
                    row["count"] = json!(group.len());
                    row["ids"] = json!(group.iter().map(|x| x.id).collect::<Vec<_>>());
                }
                row
            })
            .collect();
        // How many rows the whole list has, so its scroll bar is right from
        // the start and End reaches the oldest. Without conversations that's
        // the message count; with them, the message count scaled by how many
        // conversations the messages read so far made (an estimate until all
        // are read). With more to read it's always past what's shown, so
        // scrolling asks for them.
        let total = if !more {
            groups.len().max(out.len())
        } else {
            let estimate = if threaded && !msgs.is_empty() {
                (all as f64 * groups.len() as f64 / msgs.len() as f64).round() as usize
            } else {
                all
            };
            estimate.max(groups.len()).max(out.len() + 1)
        };
        // (`all` counts messages here, not conversations.)
        json!({"rows": out, "total": total, "all": all.max(out.len()), "unread": unread, "countsMessages": true})
    }

    /// Counts by folder from the database (kept until the next save), with the
    /// changes in memory not saved yet put right.
    fn mail_counts(&self) -> MailCounts {
        let saves = crate::mailstore::SAVES.load(std::sync::atomic::Ordering::SeqCst);
        let mut c = {
            let mut cache = self.counts_cache.lock().unwrap();
            match cache.as_ref() {
                Some((n, c)) if *n == saves => c.clone(),
                _ => {
                    let _t = crate::Timed::new("counting mail");
                    let mut c = MailCounts::default();
                    for (folder, total, unread, flagged) in crate::mailstore::folder_counts() {
                        c.by_folder.insert(folder, (total, unread, [flagged, 0, 0, 0, 0, 0, 0]));
                    }
                    for (folder, color) in crate::mailstore::colored_flags() {
                        if let Some(e) = c.by_folder.get_mut(&folder) {
                            e.2[0] -= 1;
                            e.2[color as usize] += 1;
                        }
                    }
                    if !self.vips.is_empty() {
                        c.vip = self.selection_sql("kind:vip").map(|(s, a)| crate::mailstore::count(&s, &a)).unwrap_or(0);
                    }
                    *cache = Some((saves, c.clone()));
                    c
                }
            }
        };
        let mut add = |folder: &str, read: bool, flagged: bool, color: u8, step: i64| {
            let e = c.by_folder.entry(folder.to_string()).or_insert((0, 0, [0; 7]));
            e.0 += step;
            if !read {
                e.1 += step;
            }
            if flagged {
                e.2[(color as usize).min(6)] += step;
            }
        };
        let here: std::collections::HashSet<i64> = self.messages.iter().map(|m| m.id).collect();
        for m in &self.messages {
            if let Some((f, r, fl)) = crate::mailstore::saved_state(m.id) {
                // (Its colour as saved isn't known: taken as the same.)
                add(&f, r, fl, m.flag_color, -1);
            }
            add(&m.folder, m.read, m.flagged, m.flag_color, 1);
        }
        for id in crate::mailstore::tracked_ids().into_iter().filter(|id| !here.contains(id)) {
            if let Some((f, r, fl)) = crate::mailstore::saved_state(id) {
                add(&f, r, fl, 0, -1);
            }
        }
        c
    }

    /// The sidebar's counts from the database (see `mail_sidebar`).
    fn mail_sidebar_backed(&self) -> Value {
        let c = self.mail_counts();
        let kinds = self.folder_kinds();
        let sum = |pick: &dyn Fn(&str, &(i64, i64, [i64; 7])) -> i64| -> i64 { c.by_folder.iter().map(|(f, v)| pick(f, v)).sum() };
        let kind_of = |f: &str| kinds.get(f).copied().unwrap_or("other");
        let smart = [
            ("kind:inbox", "Inbox", "inbox"),
            ("kind:vip", "VIPs", "star"),
            ("kind:flagged", "Flagged", "flag"),
            ("kind:drafts", "Drafts", "draft"),
            ("kind:sent", "Sent", "send"),
            ("kind:junk", "Junk", "junk"),
            ("kind:trash", "Trash", "trash"),
            ("kind:archive", "Archive", "archive"),
        ];
        let mut by_color = [0i64; 7];
        for (f, v) in &c.by_folder {
            if kind_of(f) != "trash" {
                for (i, n) in v.2.iter().enumerate() {
                    by_color[i] += n;
                }
            }
        }
        let colors: Vec<Value> = by_color
            .iter()
            .enumerate()
            .filter(|(_, n)| **n > 0)
            .map(|(i, n)| json!({"id": format!("kind:flag{i}"), "color": i, "name": FLAG_NAMES[i], "badge": n}))
            .collect();
        let smart: Vec<Value> = smart
            .iter()
            .map(|(id, name, icon)| {
                let k = &id[5..];
                let badge = match k {
                    "vip" => c.vip,
                    "flagged" => by_color.iter().sum(),
                    "drafts" => sum(&|f, v| if kind_of(f) == "drafts" { v.0 } else { 0 }),
                    k => sum(&|f, v| if kind_of(f) == k { v.1 } else { 0 }),
                };
                let mut v = json!({"id": id, "name": name, "icon": icon, "badge": badge});
                if *id == "kind:flagged" {
                    v["colors"] = json!(colors);
                }
                v
            })
            .collect();
        let accounts: Vec<Value> = self
            .accounts
            .iter()
            .map(|a| {
                let folders: Vec<Value> = self
                    .folders
                    .iter()
                    .filter(|f| f.account == a.id)
                    .map(|f| {
                        let unread = c.by_folder.get(&f.id).map(|v| v.1).unwrap_or(0);
                        json!({"id": format!("folder:{}", f.id), "name": f.name, "kind": f.kind, "icon": kind_icon(&f.kind), "badge": unread})
                    })
                    .collect();
                json!({"id": a.id, "name": a.name, "email": a.email, "source": a.source, "folders": folders})
            })
            .collect();
        // (Smart mailboxes are worked out message by message: no count.)
        let smart_boxes: Vec<Value> = self
            .smart_mailboxes
            .iter()
            .map(|s| json!({"id": format!("smart:{}", s.id), "name": s.name, "icon": "gear", "badge": 0, "smartId": s.id}))
            .collect();
        json!({"smart": smart, "smartBoxes": smart_boxes, "accounts": accounts, "outbox": self.outbox.len(), "vipCount": self.vips.len()})
    }

    /// A message's conversation (within a selection), from the database: its
    /// earlier messages (by the Message-IDs it refers to) and the later ones
    /// replying to any of them.
    fn thread_backed(&self, selection: &str, id: i64) -> Value {
        let Some(m) = self.get(id) else { return json!([]) };
        let ids_in = |s: &str| -> Vec<String> {
            let mut out = Vec::new();
            for part in s.split('<').skip(1) {
                let id = part.split('>').next().unwrap_or("").trim();
                if !id.is_empty() {
                    out.push(id.to_string());
                }
            }
            out
        };
        use crate::mailstore::Arg;
        // The open conversation is looked up again on every change to the mail
        // (several a second while mail arrives); the full search is slow on a big
        // mailbox (seconds, with the window frozen meanwhile: the list stuttered).
        // So what it found is kept, and later only its messages (by id) and any
        // newer ones are read.
        static CACHE: std::sync::Mutex<Option<(String, i64, Vec<String>, Vec<i64>, i64)>> = std::sync::Mutex::new(None);
        let like = |known: &[String], from: i64| -> (String, Vec<Arg>) {
            let mut args: Vec<Arg> = vec![Arg::Integer(from)];
            let mut ors = Vec::new();
            for k in known {
                args.push(Arg::Text(format!("%{k}%")));
                ors.push(format!("in_reply_to LIKE ?{0} OR refs LIKE ?{0}", args.len()));
            }
            (format!("date >= ?1 AND ({})", ors.join(" OR ")), args)
        };
        let search_t = crate::Timed::new("thread: search");
        let cached = CACHE.lock().unwrap().clone().filter(|c| c.0 == selection && c.1 == id);
        let mut cands: Vec<Message>;
        let known: Vec<String>;
        if let Some((_, _, k, members, newest)) = cached {
            known = k;
            cands = if members.is_empty() {
                Vec::new()
            } else {
                let args: Vec<Arg> = members.iter().map(|x| Arg::Integer(*x)).collect();
                let marks: Vec<String> = (1..=members.len()).map(|n| format!("?{n}")).collect();
                crate::mailstore::rows(&format!("id IN ({})", marks.join(", ")), &args, "date ASC", members.len() as i64 + 1, 0)
            };
            if !known.is_empty() {
                let (filter, args) = like(&known, newest);
                cands.extend(crate::mailstore::rows(&filter, &args, "date ASC", 500, 0));
            }
        } else {
            let mut k: Vec<String> = ids_in(&m.references);
            k.extend(ids_in(&m.in_reply_to));
            cands = crate::mailstore::by_message_ids(&k);
            // Replies can only be newer than the oldest message of the thread found so
            // far. With none found (its start isn't here, as with GitHub notifications,
            // which all point at a pull request's first message nobody got), there's no
            // such bound: limiting to this message's own date found it alone.
            let oldest = if cands.is_empty() { 0 } else { cands.iter().map(|x| x.date).min().unwrap_or(m.date).min(m.date) };
            // Replies are found by the thread's first message (every reply's
            // References starts with it) and by this one's own id (direct replies):
            // two patterns, not one for each message of the thread (up to 40 made
            // this search take seconds on a big mailbox).
            let mut look: Vec<String> = Vec::new();
            if let Some(root) = k.first().cloned() {
                look.push(root);
            }
            if !m.message_id.is_empty() {
                look.push(m.message_id.trim().trim_matches(['<', '>']).to_string());
            }
            look.dedup();
            if !look.is_empty() {
                let (filter, args) = like(&look, oldest);
                cands.extend(crate::mailstore::rows(&filter, &args, "date ASC", 500, 0));
            }
            known = look;
        }
        drop(search_t);
        {
            let mut ids: Vec<i64> = cands.iter().map(|x| x.id).collect();
            ids.sort();
            ids.dedup();
            let newest = cands.iter().map(|x| x.date).max().unwrap_or(m.date).max(m.date);
            *CACHE.lock().unwrap() = Some((selection.to_string(), id, known, ids, newest));
        }
        let _keys_t = crate::Timed::new("thread: grouping");
        cands.push(m.clone());
        for r in self.messages.iter() {
            cands.push(r.clone());
        }
        // In memory wins; each message once.
        let mut seen = std::collections::HashSet::new();
        let here: std::collections::HashSet<i64> = self.messages.iter().map(|x| x.id).collect();
        cands.retain(|x| (here.contains(&x.id) || !self.messages.iter().any(|r| r.id == x.id)) && seen.insert(x.id));
        let keys = conversation_keys(&cands);
        let key = keys.get(&id).cloned().unwrap_or_default();
        let kinds = self.folder_kinds();
        let mut list: Vec<&Message> = cands.iter().filter(|x| x.id == id || (!key.is_empty() && keys.get(&x.id) == Some(&key) && self.message_in_k(x, selection, Some(&kinds)))).collect();
        list.sort_by_key(|x| x.date);
        drop(_keys_t);
        let _json_t = crate::Timed::new("thread: messages");
        Value::Array(list.iter().map(|x| self.message_json(x)).collect())
    }

    pub fn new_id(&mut self) -> i64 {
        self.next_id += 1;
        self.next_id
    }

    // ------------------------------------------------------------------ mail

    fn folder(&self, id: &str) -> Option<&Folder> {
        self.folders.iter().find(|f| f.id == id)
    }

    pub fn folder_kind(&self, id: &str) -> &str {
        self.folder(id).map(|f| f.kind.as_str()).unwrap_or("other")
    }

    fn message_in(&self, m: &Message, selection: &str) -> bool {
        self.message_in_k(m, selection, None)
    }

    /// Each folder's kind, for looking up many messages quickly.
    pub fn folder_kinds(&self) -> std::collections::HashMap<&str, &str> {
        self.folders.iter().map(|f| (f.id.as_str(), f.kind.as_str())).collect()
    }

    /// `message_in`, with the folder kinds looked up in `kinds` when given.
    fn message_in_k(&self, m: &Message, selection: &str, kinds: Option<&std::collections::HashMap<&str, &str>>) -> bool {
        let kind_of = |f: &str| -> &str {
            match kinds {
                Some(k) => k.get(f).copied().unwrap_or("other"),
                None => self.folder_kind(f),
            }
        };
        if let Some(id) = selection.strip_prefix("smart:") {
            return self.in_smart_mailbox(m, id);
        }
        if let Some(kind) = selection.strip_prefix("kind:") {
            match kind {
                "vip" => self.is_vip(&m.from_email) && !["trash", "junk"].contains(&kind_of(&m.folder)),
                "flagged" => m.flagged && kind_of(&m.folder) != "trash",
                // One colour of flag (the Flagged mailbox's sub-mailboxes).
                k if k.starts_with("flag") => {
                    m.flagged && kind_of(&m.folder) != "trash" && k[4..].parse::<u8>().map(|c| c == m.flag_color).unwrap_or(false)
                }
                "unread" => !m.read && kind_of(&m.folder) == "inbox",
                k => kind_of(&m.folder) == k,
            }
        } else if let Some(f) = selection.strip_prefix("folder:") {
            m.folder == f
        } else {
            false
        }
    }

    pub fn mail_sidebar(&self) -> Value {
        if self.backed {
            return self.mail_sidebar_backed();
        }
        let smart = [
            ("kind:inbox", "Inbox", "inbox"),
            ("kind:vip", "VIPs", "star"),
            ("kind:flagged", "Flagged", "flag"),
            ("kind:drafts", "Drafts", "draft"),
            ("kind:sent", "Sent", "send"),
            ("kind:junk", "Junk", "junk"),
            ("kind:trash", "Trash", "trash"),
            ("kind:archive", "Archive", "archive"),
        ];
        // Every count in one pass over the mail (it can be hundreds of thousands).
        let kinds = self.folder_kinds();
        let mut counts = vec![0usize; smart.len()];
        let mut folder_unread: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        let total_for = |id: &str| id == "kind:flagged" || id == "kind:drafts" || id == "kind:vip";
        for m in &self.messages {
            if !m.read {
                *folder_unread.entry(m.folder.as_str()).or_default() += 1;
            }
            for (i, (id, _, _)) in smart.iter().enumerate() {
                if (!m.read || total_for(id)) && self.message_in_k(m, id, Some(&kinds)) {
                    counts[i] += 1;
                }
            }
        }
        // Flagged, by colour (those in use).
        let mut by_color = [0usize; 7];
        for m in self.messages.iter().filter(|m| m.flagged && kinds.get(m.folder.as_str()).copied() != Some("trash")) {
            by_color[(m.flag_color as usize).min(6)] += 1;
        }
        let colors: Vec<Value> = by_color
            .iter()
            .enumerate()
            .filter(|(_, n)| **n > 0)
            .map(|(c, n)| json!({"id": format!("kind:flag{c}"), "color": c, "name": FLAG_NAMES[c], "badge": n}))
            .collect();
        let smart: Vec<Value> = smart
            .iter()
            .zip(counts)
            .map(|((id, name, icon), count)| {
                let mut v = json!({"id": id, "name": name, "icon": icon, "badge": count});
                if *id == "kind:flagged" {
                    v["colors"] = json!(colors);
                }
                v
            })
            .collect();
        let accounts: Vec<Value> = self
            .accounts
            .iter()
            .map(|a| {
                let folders: Vec<Value> = self
                    .folders
                    .iter()
                    .filter(|f| f.account == a.id)
                    .map(|f| {
                        let unread = folder_unread.get(f.id.as_str()).copied().unwrap_or(0);
                        json!({"id": format!("folder:{}", f.id), "name": f.name, "kind": f.kind,
                               "icon": kind_icon(&f.kind), "badge": unread})
                    })
                    .collect();
                json!({"id": a.id, "name": a.name, "email": a.email, "source": a.source, "folders": folders})
            })
            .collect();
        let smart_boxes: Vec<Value> = self
            .smart_mailboxes
            .iter()
            .map(|s| {
                let sel = format!("smart:{}", s.id);
                let n = self.messages.iter().filter(|m| self.message_in(m, &sel)).count();
                json!({"id": sel, "name": s.name, "icon": "gear", "badge": n, "smartId": s.id})
            })
            .collect();
        json!({"smart": smart, "smartBoxes": smart_boxes, "accounts": accounts,
               "outbox": self.outbox.len(), "vipCount": self.vips.len()})
    }

    /// Runs `f` with the conversation of every message (see conversation_keys).
    /// Which messages there are (their headers never change once here; a
    /// changed draft comes back as a new message): quick to check each time.
    fn messages_fingerprint(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.messages.len().hash(&mut h);
        self.messages.iter().fold((0u64, 0i64), |(sum, max), m| (sum.wrapping_add(m.id as u64), max.max(m.id))).hash(&mut h);
        h.finish()
    }

    pub fn with_conversations<T>(&self, f: impl FnOnce(&std::collections::HashMap<i64, String>) -> T) -> T {
        let fp = self.messages_fingerprint();
        let mut cache = self.conv_cache.lock().unwrap();
        if cache.as_ref().map(|(k, _)| *k) != Some(fp) {
            let _t = crate::Timed::new("grouping conversations");
            *cache = Some((fp, conversation_keys(&self.messages)));
        }
        f(&cache.as_ref().unwrap().1)
    }

    pub fn conversation_of(&self, id: i64) -> String {
        if self.backed {
            // (From the messages of its conversation found in the database.)
            let thread = self.thread_backed("", id);
            let ids: Vec<i64> = thread.as_array().map(|l| l.iter().filter_map(|m| m["id"].as_i64()).collect()).unwrap_or_default();
            let msgs: Vec<Message> = ids.iter().filter_map(|i| self.get(*i)).collect();
            return conversation_keys(&msgs).get(&id).cloned().unwrap_or_else(|| format!("#{id}"));
        }
        self.with_conversations(|c| c.get(&id).cloned().unwrap_or_else(|| format!("#{id}")))
    }

    /// Message list; with `threaded`, one row per conversation (latest message
    /// shown, with how many messages it has).
    pub fn messages(&self, selection: &str, query: &str, threaded: bool) -> Value {
        if self.backed {
            let page = self.message_page_backed(selection, query, threaded, "date", false, false, 0, 500);
            return page["rows"].clone();
        }
        let flat = self.messages_flat(selection, query);
        if !threaded {
            return flat;
        }
        let Value::Array(items) = flat else { return flat };
        let mut order: Vec<String> = Vec::new();
        let mut groups: std::collections::HashMap<String, Vec<Value>> = Default::default();
        let keys = self.with_conversations(|c| items.iter().map(|it| it["id"].as_i64().and_then(|id| c.get(&id).cloned()).unwrap_or_default()).collect::<Vec<_>>());
        for (it, key) in items.into_iter().zip(keys) {
            if !groups.contains_key(&key) {
                order.push(key.clone());
            }
            groups.entry(key).or_default().push(it);
        }
        Value::Array(
            order
                .into_iter()
                .map(|k| {
                    let g = groups.remove(&k).unwrap_or_default();
                    let mut head = g[0].clone();
                    let ids: Vec<i64> = g.iter().filter_map(|m| m["id"].as_i64()).collect();
                    head["count"] = json!(g.len());
                    head["ids"] = json!(ids);
                    head["read"] = json!(g.iter().all(|m| m["read"].as_bool().unwrap_or(true)));
                    head["flagged"] = json!(g.iter().any(|m| m["flagged"].as_bool().unwrap_or(false)));
                    head["flagColor"] = g.iter().find(|m| m["flagged"].as_bool().unwrap_or(false)).map(|m| m["flagColor"].clone()).unwrap_or(json!(0));
                    head["attachments"] = json!(g.iter().any(|m| m["attachments"].as_bool().unwrap_or(false)));
                    head
                })
                .collect(),
        )
    }

    /// The whole conversation a message belongs to (within the same mailbox
    /// selection), oldest first.
    pub fn thread(&self, selection: &str, id: i64) -> Value {
        if self.backed {
            return self.thread_backed(selection, id);
        }
        let Some(m) = self.messages.iter().find(|m| m.id == id) else { return json!([]) };
        let key = self.conversation_of(m.id);
        let kinds = self.folder_kinds();
        let mut list: Vec<&Message> = self.with_conversations(|c| {
            self.messages
                .iter()
                .filter(|x| x.id == id || (c.get(&x.id) == Some(&key) && self.message_in_k(x, selection, Some(&kinds))))
                .collect()
        });
        list.sort_by_key(|m| m.date);
        Value::Array(list.iter().map(|m| self.message(m.id)).collect())
    }

    pub fn mail_folders(&self) -> Value {
        Value::Array(
            self.folders
                .iter()
                .map(|f| {
                    let account = self.accounts.iter().find(|a| a.id == f.account).map(|a| a.name.clone()).unwrap_or_default();
                    json!({"id": f.id, "name": f.name, "account": account, "kind": f.kind})
                })
                .collect(),
        )
    }

    pub fn move_to_folder(&mut self, id: i64, folder: &str) {
        if self.folder(folder).is_none() {
            return;
        }
        if let Some(m) = self.msg_mut(id) {
            if m.folder != folder {
                m.uid = 0;
            }
            m.folder = folder.to_string();
        }
    }

    pub fn mark_all_read(&mut self, selection: &str) {
        if self.backed {
            let (unread, _) = self.query_messages(selection, "", Some(0), "date", false, 1_000_000);
            for m in unread.iter().filter(|m| !m.read) {
                self.set_read(m.id, true);
            }
            return;
        }
        let ids: Vec<i64> = self
            .messages
            .iter()
            .filter(|m| !m.read && self.message_in(m, selection))
            .map(|m| m.id)
            .collect();
        for id in ids {
            self.set_read(id, true);
        }
    }

    /// The messages of a mailbox selection matching a search, newest first.
    fn matching(&self, selection: &str, query: &str) -> Vec<&Message> {
        let q = lc(query.trim());
        let kinds = self.folder_kinds();
        let hits = if q.is_empty() { None } else { self.text_hits(query) };
        let mut list: Vec<&Message> = self
            .messages
            .iter()
            .filter(|m| self.message_in_k(m, selection, Some(&kinds)))
            .filter(|m| {
                q.is_empty()
                    || lc(&m.subject).contains(&q)
                    || lc(&m.from_name).contains(&q)
                    || lc(&m.from_email).contains(&q)
                    || lc(&m.to).contains(&q)
                    || hits.as_ref().map(|h| h.contains(&m.id)).unwrap_or_else(|| lc(&m.body).contains(&q))
            })
            .collect();
        list.sort_by(|a, b| b.date.cmp(&a.date));
        list
    }

    /// One row of the message list (a message, or with `group` a conversation
    /// shown by its newest message).
    fn list_row(&self, group: &[&Message], show_folder: bool) -> Value {
        let m = group[0];
        let folder = self.folder(&m.folder);
        let account = folder.and_then(|f| self.accounts.iter().find(|a| a.id == f.account)).map(|a| a.name.clone()).unwrap_or_default();
        json!({
            "id": m.id,
            "fromName": if m.from_name.is_empty() { &m.from_email } else { &m.from_name },
            "fromEmail": m.from_email,
            "to": m.to,
            "subject": if m.subject.is_empty() { "(No Subject)" } else { &m.subject },
            "preview": self.preview_of(m),
            "date": m.date * 1000,
            "read": group.iter().all(|x| x.read),
            "flagged": group.iter().any(|x| x.flagged),
            "flagColor": group.iter().find(|x| x.flagged).map(|x| x.flag_color).unwrap_or(0),
            "attachments": group.iter().any(|x| !x.attachments.is_empty()),
            "vip": self.is_vip(&m.from_email),
            "muted": self.is_muted(m),
            "remind": m.remind_at > 0,
            "account": if show_folder { account } else { String::new() },
        })
    }

    /// The message list, a page at a time: the first `limit` rows of a mailbox
    /// selection, sorted and filtered (so big mailboxes stay quick), with the
    /// totals. The row of `keep` is always included (the open message).
    #[allow(clippy::too_many_arguments)]
    /// The folders a selection lists, when the list can be read a page at a time
    /// anywhere (a mailbox, or a kind of mailbox, by date, no search). None otherwise.
    fn window_folders(&self, selection: &str, query: &str, sort_by: &str) -> Option<Vec<String>> {
        if !self.backed || !query.trim().is_empty() || !(sort_by == "date" || sort_by.is_empty()) {
            return None;
        }
        if let Some(f) = selection.strip_prefix("folder:") {
            return Some(vec![f.to_string()]);
        }
        let k = selection.strip_prefix("kind:")?;
        if ["vip", "flagged", "unread"].contains(&k) || k.starts_with("flag") {
            return None;
        }
        Some(self.folders_of_kind(&[k]))
    }

    /// How to read a list a page at a time (see `list_window`), or None when
    /// it can't be (a search, another order, a smart mailbox).
    #[allow(clippy::too_many_arguments)]
    pub fn window_plan(&self, selection: &str, query: &str, threaded: bool, sort_by: &str, ascending: bool, unread_only: bool, keep: i64) -> Option<WindowPlan> {
        if !self.backed {
            return None;
        }
        // (Conversations not all worked out yet, the first time: read as before meanwhile.)
        if threaded && !crate::mailstore::conversations_ready() {
            return None;
        }
        let show_folder = selection.starts_with("kind:") && self.accounts.len() > 1;
        let unread_or = unread_only.then_some(keep);
        let sort = if sort_by.is_empty() { "date".to_string() } else { sort_by.to_string() };
        if let Some(folders) = self.window_folders(selection, query, sort_by) {
            return Some(WindowPlan { folders, filter: None, sort_by: sort, ascending, generation: self.generation, threaded, unread_or, show_folder });
        }
        // Anything else SQL can say (not a smart mailbox's conditions).
        let filter = self.list_filter(selection, query, unread_or)?;
        Some(WindowPlan { folders: Vec::new(), filter: Some(filter), sort_by: sort, ascending, generation: self.generation, threaded, unread_or, show_folder })
    }

    /// The list's rows from what `fetch_window` read.
    pub fn window_rows(&self, plan: &WindowPlan, groups: &[Vec<Message>]) -> Vec<Value> {
        groups
            .iter()
            .filter(|g| !g.is_empty())
            .map(|g| {
                let group: Vec<&Message> = g.iter().collect();
                let mut row = self.list_row(&group, plan.show_folder);
                if plan.threaded {
                    row["count"] = json!(group.len());
                    row["ids"] = json!(group.iter().map(|x| x.id).collect::<Vec<_>>());
                }
                row
            })
            .collect()
    }

    /// Rows `offset`… of the message list (newest first), and how many there
    /// are in all: read a page at a time, however far down. With conversations
    /// on, each row is a conversation of the mailbox. None when this list
    /// can't be read that way.
    #[allow(clippy::too_many_arguments)]
    pub fn list_window(&self, selection: &str, query: &str, threaded: bool, sort_by: &str, ascending: bool, unread_only: bool, keep: i64, offset: usize, count: usize) -> Option<Value> {
        let plan = self.window_plan(selection, query, threaded, sort_by, ascending, unread_only, keep)?;
        let (groups, total, unread) = fetch_window(&plan, offset, count, true);
        Some(json!({"rows": self.window_rows(&plan, &groups), "total": total, "unread": unread, "offset": offset}))
    }

    /// The row of message `id` in that list (how many rows come before it), or
    /// -1 when it isn't in it or the list can't be read a page at a time.
    #[allow(clippy::too_many_arguments)]
    pub fn list_position(&self, selection: &str, query: &str, threaded: bool, sort_by: &str, ascending: bool, unread_only: bool, id: i64) -> i64 {
        let Some(plan) = self.window_plan(selection, query, threaded, sort_by, ascending, unread_only, id) else { return -1 };
        position_in(&plan, id, self.get(id).map(|m| (m.folder.clone(), m.date)))
    }

    /// How many messages of a mailbox come before message `id` in the list
    /// (by date): a row number it's at or before, even with conversations on.
    /// -1 when it's not in that list, or can't be told (a search, another order).
    pub fn rows_before(&self, selection: &str, query: &str, sort_by: &str, ascending: bool, unread_only: bool, id: i64) -> i64 {
        if !self.backed || !query.trim().is_empty() || (sort_by != "date" && !sort_by.is_empty()) {
            return -1;
        }
        let Some(m) = self.get(id) else { return -1 };
        let Some((base, mut args)) = self.selection_sql(selection) else { return -1 };
        let unread = if unread_only { " AND read = 0" } else { "" };
        args.push(crate::mailstore::Arg::Integer(id));
        if crate::mailstore::count(&format!("({base}){unread} AND id = ?{}", args.len()), &args) == 0 {
            return -1;
        }
        args.pop();
        args.push(crate::mailstore::Arg::Integer(m.date));
        let n = args.len();
        let cmp = if ascending { "<" } else { ">" };
        crate::mailstore::count(&format!("({base}){unread} AND date {cmp} ?{n}"), &args)
    }

    pub fn message_page(&self, selection: &str, query: &str, threaded: bool, sort_by: &str, ascending: bool, unread_only: bool, keep: i64, limit: usize) -> Value {
        if self.backed {
            return self.message_page_backed(selection, query, threaded, sort_by, ascending, unread_only, keep, limit);
        }
        // The grouped and sorted rows are worked out once per change of mail
        // (or of mailbox, search, sort), then paged and filtered from there.
        let key = format!("{selection}\u{1}{query}\u{1}{threaded}\u{1}{sort_by}\u{1}{ascending}");
        let rows = {
            let mut cache = self.page_cache.lock().unwrap();
            match cache.as_ref() {
                Some((k, g, n, rows)) if *k == key && *g == self.generation && *n == self.messages.len() => rows.clone(),
                _ => {
                    let rows = std::sync::Arc::new(self.page_rows(selection, query, threaded, sort_by, ascending));
                    *cache = Some((key, self.generation, self.messages.len(), rows.clone()));
                    rows
                }
            }
        };
        let msgs = &self.messages;
        let mut groups: Vec<&Vec<usize>> = rows.groups.iter().collect();
        if unread_only {
            groups.retain(|g| g.iter().any(|&i| !msgs[i].read || msgs[i].id == keep));
        }
        let total = groups.len();
        let mut n = limit.min(total);
        if let Some(k) = groups.iter().position(|g| g.iter().any(|&i| msgs[i].id == keep)) {
            n = n.max(k + 1);
        }
        let show_folder = selection.starts_with("kind:") && self.accounts.len() > 1;
        let out: Vec<Value> = groups[..n]
            .iter()
            .map(|g| {
                let group: Vec<&Message> = g.iter().map(|&i| &msgs[i]).collect();
                let mut row = self.list_row(&group, show_folder);
                if threaded {
                    row["count"] = json!(group.len());
                    row["ids"] = json!(group.iter().map(|x| x.id).collect::<Vec<_>>());
                }
                row
            })
            .collect();
        json!({"rows": out, "total": total, "all": rows.all, "unread": rows.unread})
    }

    /// A mailbox's rows, grouped into conversations (when `threaded`) and sorted.
    fn page_rows(&self, selection: &str, query: &str, threaded: bool, sort_by: &str, ascending: bool) -> PageRows {
        let base = self.messages.as_ptr() as usize;
        let size = std::mem::size_of::<Message>().max(1);
        let index_of = |m: &Message| (m as *const Message as usize - base) / size;
        let list = {
            let _t = crate::Timed::new("finding the mailbox's messages");
            self.matching(selection, query)
        };
        let mut groups: Vec<Vec<&Message>> = if threaded {
            let keys = self.with_conversations(|c| list.iter().map(|m| c.get(&m.id).cloned().unwrap_or_default()).collect::<Vec<_>>());
            let mut order: Vec<Vec<&Message>> = Vec::new();
            let mut at: std::collections::HashMap<String, usize> = Default::default();
            for (m, key) in list.iter().zip(keys) {
                match at.get(&key) {
                    Some(&i) if !key.is_empty() => order[i].push(m),
                    _ => {
                        at.insert(key, order.len());
                        order.push(vec![m]);
                    }
                }
            }
            order
        } else {
            list.iter().map(|m| vec![*m]).collect()
        };
        let all = groups.len();
        let unread = groups.iter().filter(|g| g.iter().any(|m| !m.read)).count();
        // Sorted (newest first is how they come; ties stay newest first).
        let bare = |s: &str| {
            let mut s = s.trim().to_lowercase();
            loop {
                let before = s.len();
                for p in ["re:", "fwd:", "fw:", "aw:", "sv:", "wg:", "tr:"] {
                    if let Some(rest) = s.strip_prefix(p) {
                        s = rest.trim_start().to_string();
                    }
                }
                if s.len() == before {
                    return s;
                }
            }
        };
        if !(sort_by == "date" && !ascending) {
            let key = |g: &Vec<&Message>| -> (i64, String) {
                let m = g[0];
                match sort_by {
                    "from" => (0, lc(if m.from_name.is_empty() { &m.from_email } else { &m.from_name })),
                    "subject" => (0, bare(&m.subject)),
                    "to" => (0, lc(&m.to)),
                    "attachments" => (g.iter().any(|x| !x.attachments.is_empty()) as i64, String::new()),
                    "flags" => (g.iter().any(|x| x.flagged) as i64, String::new()),
                    "unread" => (g.iter().any(|x| !x.read) as i64, String::new()),
                    _ => (m.date, String::new()),
                }
            };
            let mut keyed: Vec<((i64, String), Vec<&Message>)> = groups.into_iter().map(|g| (key(&g), g)).collect();
            keyed.sort_by(|a, b| if ascending { a.0.cmp(&b.0) } else { b.0.cmp(&a.0) });
            groups = keyed.into_iter().map(|(_, g)| g).collect();
        }
        PageRows { groups: groups.iter().map(|g| g.iter().map(|m| index_of(m)).collect()).collect(), all, unread }
    }

    fn messages_flat(&self, selection: &str, query: &str) -> Value {
        let q = lc(query.trim());
        let hits = if q.is_empty() { None } else { self.text_hits(query) };
        let mut list: Vec<&Message> = self
            .messages
            .iter()
            .filter(|m| self.message_in(m, selection))
            .filter(|m| {
                q.is_empty()
                    || lc(&m.subject).contains(&q)
                    || lc(&m.from_name).contains(&q)
                    || lc(&m.from_email).contains(&q)
                    || lc(&m.to).contains(&q)
                    || hits.as_ref().map(|h| h.contains(&m.id)).unwrap_or_else(|| lc(&m.body).contains(&q))
            })
            .collect();
        list.sort_by(|a, b| b.date.cmp(&a.date));
        let show_folder = selection.starts_with("kind:") && self.accounts.len() > 1;
        Value::Array(
            list.iter()
                .map(|m| {
                    let folder = self.folder(&m.folder);
                    let account = folder
                        .and_then(|f| self.accounts.iter().find(|a| a.id == f.account))
                        .map(|a| a.name.clone())
                        .unwrap_or_default();
                    json!({
                        "id": m.id,
                        "fromName": if m.from_name.is_empty() { &m.from_email } else { &m.from_name },
                        "fromEmail": m.from_email,
                        "to": m.to,
                        "subject": if m.subject.is_empty() { "(No Subject)" } else { &m.subject },
                        "preview": self.preview_of(m),
                        "date": m.date * 1000,
                        "read": m.read,
                        "flagged": m.flagged,
                        "flagColor": m.flag_color,
                        "attachments": !m.attachments.is_empty(),
                        "vip": self.is_vip(&m.from_email),
                        "muted": self.is_muted(m),
                        "remind": m.remind_at > 0,
                        "account": if show_folder { account } else { String::new() },
                    })
                })
                .collect(),
        )
    }

    pub fn message(&self, id: i64) -> Value {
        match self.get(id) {
            Some(m) => self.message_json(&m),
            None => Value::Null,
        }
    }

    fn message_json(&self, m: &Message) -> Value {
        match Some(m) {
            Some(m) => {
                let folder = self.folder(&m.folder);
                // The text comes from the database (memory keeps only a preview).
                let (body, html, invite) = self.text_of(m.id);
                json!({
                    "id": m.id,
                    "fromName": m.from_name,
                    "fromEmail": m.from_email,
                    "to": m.to,
                    "cc": m.cc,
                    "subject": m.subject,
                    // The heart of it, for a closed message in a conversation.
                    "gist": crate::gist::gist(&body, &html),
                    "body": body,
                    "date": m.date * 1000,
                    "read": m.read,
                    "flagged": m.flagged,
                    "flagColor": m.flag_color,
                    "folder": folder.map(|f| f.name.clone()).unwrap_or_default(),
                    "kind": folder.map(|f| f.kind.clone()).unwrap_or_default(),
                    "source": m.source,
                    "attachments": m.attachments,
                    "html": html,
                    "invite": invite,
                    "canOpen": !m.raw_ref.is_empty() || (m.uid > 0 && m.source.starts_with("live:")),
                    "vip": self.is_vip(&m.from_email),
                    "blocked": self.blocked.contains(&m.from_email.to_lowercase()),
                    "muted": self.is_muted(m),
                    "remindAt": m.remind_at * 1000,
                    "messageId": m.message_id,
                    "partial": m.partial,
                    "replyTo": m.reply_to,
                })
            }
            None => Value::Null,
        }
    }

    /// Messages of a live account still waiting for their text: inbox
    /// first, newest first.
    pub fn partial_messages(&self, source: &str) -> Vec<crate::live::Wanted> {
        let rank = |k: &str| ["inbox", "sent", "drafts", "archive", "other", "junk", "trash"].iter().position(|x| *x == k).unwrap_or(4);
        if self.backed {
            let here: std::collections::HashSet<i64> = self.messages.iter().map(|m| m.id).collect();
            let mut list: Vec<(usize, i64, crate::live::Wanted)> = crate::mailstore::partial_ids(source)
                .into_iter()
                .filter(|(id, ..)| !here.contains(id))
                .chain(self.messages.iter().filter(|m| m.source == source && m.partial && m.uid > 0 && m.preview.is_empty()).map(|m| (m.id, m.folder.clone(), m.uid, m.date)))
                .filter_map(|(id, folder, uid, date)| {
                    let f = self.folder(&folder)?;
                    Some((rank(&f.kind), -date, (id, f.path.clone(), uid)))
                })
                .collect();
            list.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
            return list.into_iter().map(|x| x.2).collect();
        }
        let mut list: Vec<(usize, i64, crate::live::Wanted)> = self
            .messages
            .iter()
            // (Those with their start already here are finished when opened.)
            .filter(|m| m.source == source && m.partial && m.uid > 0 && m.preview.is_empty())
            .filter_map(|m| {
                let f = self.folder(&m.folder)?;
                Some((rank(&f.kind), -m.date, (m.id, f.path.clone(), m.uid)))
            })
            .collect();
        list.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
        list.into_iter().map(|x| x.2).collect()
    }

    /// Puts downloaded text into messages that only had their headers.
    pub fn fill_texts(&mut self, texts: Vec<crate::live::Text>) -> bool {
        let _t = crate::Timed::new("filling in message text");
        let mut any = false;
        for t in texts {
            if let Some(m) = self.msg_mut(t.id).filter(|m| m.partial) {
                m.preview = fetched_preview(&t.body, &t.html);
                m.body = t.body;
                m.html = t.html;
                m.invite = t.invite;
                if t.complete {
                    m.attachments = t.attachments;
                    m.partial = false;
                } else {
                    // Just its start: readable (and searchable) now; the rest, and any
                    // attachments further on, come when it's opened.
                    for a in t.attachments {
                        if !m.attachments.contains(&a) {
                            m.attachments.push(a);
                        }
                    }
                }
                any = true;
            }
        }
        any
    }

    /// Adds messages read on request (older mail, server search results),
    /// skipping any already here. Returns the new ids.
    /// What a long background pass over a whole account found (it can run for
    /// hours): new mail, read / flagged changes and mail gone from the server,
    /// for folders that still exist. The folder list is left to the regular
    /// checks, which see the server as it is now.
    pub fn apply_late_mail(&mut self, src: &str, imp: Imported) -> Vec<i64> {
        let _t = crate::Timed::new("applying older mail");
        let Some(delta) = imp.mail_delta else { return vec![] };
        let folders: std::collections::HashSet<String> = self.folders.iter().map(|f| f.id.clone()).collect();
        let gone: std::collections::HashSet<(String, u32)> = delta.gone.into_iter().filter(|(f, _)| folders.contains(f)).collect();
        for (f, m) in delta.marks {
            if folders.contains(&f) {
                self.folder_marks.insert(f, m);
            }
        }
        if !gone.is_empty() {
            self.messages.retain(|m| m.source != src || !gone.contains(&(m.folder.clone(), m.uid)));
            if self.backed {
                let list: Vec<(String, u32)> = gone.iter().cloned().collect();
                crate::mailstore::remove(src, crate::mailstore::Gone::Uids(&list));
            }
        }
        let mut index: std::collections::HashMap<(String, u32), usize> = std::collections::HashMap::new();
        for (i, m) in self.messages.iter().enumerate().filter(|(_, m)| m.source == src && m.uid > 0) {
            index.insert((m.folder.clone(), m.uid), i);
        }
        let mut elsewhere = Vec::new();
        let flags = self.unguarded_flags(src, delta.started, delta.flags);
        for (folder, uid, read, flagged, color) in flags {
            if let Some(&i) = index.get(&(folder.clone(), uid)) {
                self.messages[i].read = read;
                self.messages[i].flagged = flagged;
                if color != NO_COLOR {
                    self.messages[i].flag_color = color;
                }
            } else if self.backed {
                elsewhere.push((folder, uid, read, flagged, color));
            }
        }
        // (Those not in memory: changed in the database directly.)
        crate::mailstore::update_flags(src, &elsewhere);
        let new: Vec<Message> = delta.new.into_iter().map(|mut m| { m.source = src.to_string(); m }).collect();
        self.add_messages(new)
    }

    pub fn add_messages(&mut self, msgs: Vec<Message>) -> Vec<i64> {
        let _t = crate::Timed::new("adding messages");
        let mut new_ids = Vec::new();
        let mut here: std::collections::HashSet<(String, u32)> =
            self.messages.iter().filter(|x| x.uid > 0).map(|x| (x.folder.clone(), x.uid)).collect();
        // Only into folders that are (still) here: one deleted meanwhile (a Gmail
        // label removed, say) doesn't come back through late-arriving mail.
        let folders: std::collections::HashSet<String> = self.folders.iter().map(|f| f.id.clone()).collect();
        for mut m in msgs {
            if !folders.is_empty() && !folders.contains(&m.folder) {
                continue;
            }
            if m.uid > 0 && !here.insert((m.folder.clone(), m.uid)) {
                continue;
            }
            // (Already in the database.)
            if self.backed && m.uid > 0 && crate::mailstore::id_at(&m.source, &m.folder, m.uid).is_some() {
                continue;
            }
            // Moved here by Almanac (number not known yet): this is it, so the
            // stand-in goes.
            if m.uid > 0 && !m.message_id.is_empty() {
                let (src, folder, mid) = (m.source.clone(), m.folder.clone(), m.message_id.clone());
                self.messages.retain(|x| !(x.uid == 0 && x.source == src && x.folder == folder && x.message_id == mid));
                if self.backed {
                    crate::mailstore::drop_stand_ins(&src, &folder, &mid);
                }
            }
            m.id = self.new_id();
            new_ids.push(m.id);
            self.messages.push(m);
        }
        new_ids
    }

    /// The live-account folders a mailbox selection shows (folder id, server
    /// path, account), for loading older mail and searching on the server.
    pub fn live_folders_for(&self, selection: &str) -> Vec<(String, String, crate::live::LiveAccount)> {
        self.folders
            .iter()
            .filter(|f| {
                if let Some(id) = selection.strip_prefix("folder:") {
                    f.id == id
                } else if let Some(kind) = selection.strip_prefix("kind:") {
                    f.kind == kind
                } else {
                    false
                }
            })
            .filter_map(|f| {
                let source = self.accounts.iter().find(|a| a.id == f.account).map(|a| a.source.clone())?;
                let acct = self.live_for_source(&source)?;
                Some((f.id.clone(), f.path.clone(), acct))
            })
            .collect()
    }

    /// UIDs already here for each folder of a live account (for syncing changes).
    pub fn known_uids(&self, source: &str) -> std::collections::HashMap<String, Vec<u32>> {
        let mut out: std::collections::HashMap<String, Vec<u32>> =
            if self.backed { crate::mailstore::known_uids(source) } else { Default::default() };
        if self.backed {
            // (Those in memory and not saved yet too; each once.)
            for m in self.messages.iter().filter(|m| m.source == source && m.uid > 0) {
                let l = out.entry(m.folder.clone()).or_default();
                if !l.contains(&m.uid) {
                    l.push(m.uid);
                }
            }
            return out;
        }
        for m in self.messages.iter().filter(|m| m.source == source && m.uid > 0) {
            out.entry(m.folder.clone()).or_default().push(m.uid);
        }
        out
    }

    /// The lowest UID held for a folder (older mail is below it).
    pub fn lowest_uid(&self, folder: &str) -> u32 {
        if self.backed {
            let source = folder.split(":mail/").next().unwrap_or("");
            let here = self.messages.iter().filter(|m| m.folder == folder && m.uid > 0).map(|m| m.uid).min();
            let saved = crate::mailstore::lowest_uid(source, folder);
            return match (here, saved) {
                (Some(h), s) if s > 0 => h.min(s),
                (Some(h), _) => h,
                (None, s) => s,
            };
        }
        self.messages.iter().filter(|m| m.folder == folder && m.uid > 0).map(|m| m.uid).min().unwrap_or(0)
    }

    /// A message to change: from memory, or read from the database into memory
    /// (saved back when the changes are, then let go).
    pub fn msg_mut(&mut self, id: i64) -> Option<&mut Message> {
        if let Some(i) = self.messages.iter().position(|m| m.id == id) {
            return self.messages.get_mut(i);
        }
        if self.backed && !crate::mailstore::is_tracked(id) {
            let m = crate::mailstore::by_id(id)?;
            crate::mailstore::note_loaded(&m);
            self.messages.push(m);
            return self.messages.last_mut();
        }
        None
    }

    /// Takes a message out (deleted in the database when next saved).
    pub fn remove_message(&mut self, id: i64) {
        if self.msg_mut(id).is_some() {
            self.messages.retain(|m| m.id != id);
        }
    }

    /// Notes that message `id`'s read/flag state was changed here and is being
    /// sent to the server: syncs that began before the server took it leave it
    /// alone. Returns the change's number for `flag_pushed`.
    pub fn guard_flags(&mut self, id: i64) -> Option<u64> {
        let m = self.get(id)?;
        if m.uid == 0 {
            return None;
        }
        let key = (m.source.clone(), m.folder.clone(), m.uid);
        self.flag_guard_generation += 1;
        let generation = self.flag_guard_generation;
        self.flag_guards.insert(key, FlagGuard { generation, changed: std::time::Instant::now(), pushed: None });
        Some(generation)
    }

    /// The server has answered change `generation` (taken or refused): syncs
    /// that begin from now on see what it has.
    pub fn flag_pushed(&mut self, generation: u64) {
        let now = std::time::Instant::now();
        for g in self.flag_guards.values_mut().filter(|g| g.generation == generation) {
            g.pushed = Some(now);
        }
    }

    /// A sync's read/flag rows without those for messages changed here that
    /// the sync (begun at `started`) can't have seen yet. Guards such a sync
    /// is past are dropped.
    fn unguarded_flags(&mut self, src: &str, started: std::time::Instant, rows: Vec<(String, u32, bool, bool, u8)>) -> Vec<(String, u32, bool, bool, u8)> {
        let stale = |g: &FlagGuard| match g.pushed {
            Some(p) => p <= started,
            None => g.changed.elapsed() > FLAG_GUARD_MAX,
        };
        self.flag_guards.retain(|k, g| k.0 != src || !stale(g));
        if self.flag_guards.is_empty() {
            return rows;
        }
        let guards = &self.flag_guards;
        rows.into_iter().filter(|(folder, uid, ..)| !guards.contains_key(&(src.to_string(), folder.clone(), *uid))).collect()
    }

    pub fn set_read(&mut self, id: i64, read: bool) -> bool {
        match self.msg_mut(id) {
            Some(m) if m.read != read => {
                m.read = read;
                true
            }
            _ => false,
        }
    }

    pub fn toggle_flag(&mut self, id: i64) {
        if let Some(m) = self.msg_mut(id) {
            m.flagged = !m.flagged;
        }
    }

    /// Flagged in a colour (None: not flagged).
    pub fn set_flag(&mut self, id: i64, color: Option<u8>) {
        if let Some(m) = self.msg_mut(id) {
            m.flagged = color.is_some();
            if let Some(c) = color {
                m.flag_color = c.min(6);
            }
        }
    }

    /// Folder of the given kind in the same account as `folder_id`, made if missing.
    fn sibling_folder(&mut self, folder_id: &str, kind: &str) -> String {
        let account = self
            .folder(folder_id)
            .map(|f| f.account.clone())
            .unwrap_or_else(|| "local".into());
        if let Some(f) = self
            .folders
            .iter()
            .find(|f| f.account == account && f.kind == kind)
        {
            return f.id.clone();
        }
        let id = format!("{account}/{kind}");
        self.folders.push(Folder {
            id: id.clone(),
            account,
            name: kind_name(kind).into(),
            kind: kind.into(),
            ..Default::default()
        });
        id
    }

    pub fn move_to(&mut self, id: i64, kind: &str) {
        let Some(folder) = self.get(id).map(|m| m.folder.clone())
        else {
            return;
        };
        let target = self.sibling_folder(&folder, kind);
        if let Some(m) = self.msg_mut(id) {
            if m.folder != target {
                // Its number on the server was the old folder's: unknown in the
                // new one until the next check finds it there (meanwhile nothing
                // is done to it on the server, rather than to another message
                // with that number).
                m.uid = 0;
            }
            m.folder = target;
        }
    }

    pub fn delete_message(&mut self, id: i64) {
        let Some(m) = self.get(id) else {
            return;
        };
        if self.folder_kind(&m.folder) == "trash" {
            self.remove_message(id);
        } else {
            self.move_to(id, "trash");
        }
    }

    fn local_folder(&mut self, kind: &str) -> String {
        if !self.accounts.iter().any(|a| a.id == "local") {
            self.accounts.insert(
                0,
                Account {
                    id: "local".into(),
                    name: "On My Computer".into(),
                    email: "me@example.com".into(),
                    source: SOURCE_LOCAL.into(),
                },
            );
        }
        self.sibling_folder("local/inbox", kind)
    }

    /// Stores a written message. `kind` is "sent" or "drafts".
    pub fn store_outgoing(&mut self, v: &Value, kind: &str) -> i64 {
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let folder = self.local_folder(kind);
        if let Some(old) = v.get("draftId").and_then(Value::as_i64) {
            self.remove_message(old);
        }
        let id = self.new_id();
        let me = self
            .accounts
            .iter()
            .find(|a| a.id == "local")
            .map(|a| a.email.clone())
            .unwrap_or_default();
        self.messages.push(Message {
            id,
            folder,
            from_name: "Me".into(),
            from_email: me,
            to: s("to"),
            cc: s("cc"),
            subject: s("subject"),
            body: s("body"),
            date: Local::now().timestamp(),
            read: true,
            flagged: false,
            source: SOURCE_LOCAL.into(),
            attachments: Vec::new(),
            ..Default::default()
        });
        id
    }

    // -------------------------------------------------------------- calendar

    pub fn calendars_json(&self) -> Value {
        Value::Array(
            self.calendars
                .iter()
                .map(|c| json!({"id": c.id, "name": c.name, "color": c.color, "visible": c.visible, "source": c.source,
                                "readOnly": c.source.starts_with("sub:") || c.id == BIRTHDAYS, "subscription": c.source.starts_with("sub:"), "url": c.url}))
                .collect(),
        )
    }

    pub fn set_calendar_visible(&mut self, id: &str, visible: bool) {
        if let Some(c) = self.calendars.iter_mut().find(|c| c.id == id) {
            c.visible = visible;
        }
    }

    /// The Birthdays calendar shows every contact's birthday each year.
    fn birthday_events(&self) -> Vec<Event> {
        if !self.calendars.iter().any(|c| c.id == BIRTHDAYS) {
            return Vec::new();
        }
        self.contacts
            .iter()
            .filter_map(|c| {
                let d = NaiveDate::parse_from_str(c.birthday.trim(), "%Y-%m-%d").ok()?;
                let s = d.and_hms_opt(0, 0, 0)?;
                Some(Event {
                    id: -c.id,
                    calendar: BIRTHDAYS.into(),
                    title: format!("{}'s Birthday", display_name(c)),
                    start: fmt_dt(s),
                    end: fmt_dt(s + Duration::days(1)),
                    all_day: true,
                    rrule: "FREQ=YEARLY".into(),
                    source: SOURCE_LOCAL.into(),
                    ..Default::default()
                })
            })
            .collect()
    }

    pub fn ensure_birthdays_calendar(&mut self) {
        if !self.calendars.iter().any(|c| c.id == BIRTHDAYS) {
            self.calendars.push(Calendar {
                id: BIRTHDAYS.into(),
                name: "Birthdays".into(),
                color: "#9b59d0".into(),
                visible: true,
                source: SOURCE_LOCAL.into(),
                ..Default::default()
            });
        }
    }

    pub fn add_calendar(&mut self, name: &str, color: &str) -> String {
        let id = format!("local-cal-{}", self.new_id());
        self.calendars.push(Calendar {
            id: id.clone(),
            name: if name.trim().is_empty() { "Untitled".into() } else { name.trim().into() },
            color: color.into(),
            visible: true,
            source: SOURCE_LOCAL.into(),
            ..Default::default()
        });
        id
    }

    pub fn update_calendar(&mut self, id: &str, name: &str, color: &str) {
        if let Some(c) = self.calendars.iter_mut().find(|c| c.id == id) {
            if !name.trim().is_empty() {
                c.name = name.trim().into();
            }
            if !color.is_empty() {
                c.color = color.into();
            }
            if c.source != SOURCE_LOCAL {
                let look = (c.name.clone(), c.color.clone());
                self.calendar_looks.insert(id.to_string(), look);
            }
        }
    }

    pub fn delete_calendar(&mut self, id: &str) {
        if id == BIRTHDAYS {
            return;
        }
        self.calendars.retain(|c| c.id != id);
        self.events.retain(|e| e.calendar != id);
        self.subscriptions.retain(|s| s.calendar != id);
    }

    /// Occurrences of `e` overlapping local [from, to): (local start, local end, wall start).
    fn expand(e: &Event, from: NaiveDateTime, to: NaiveDateTime) -> Vec<(NaiveDateTime, NaiveDateTime, NaiveDateTime)> {
        let (Some(s), Some(en)) = (parse_dt(&e.start), parse_dt(&e.end)) else { return vec![] };
        let dur = (en - s).max(Duration::zero());
        // All-day events keep their dates everywhere; timed ones follow their zone.
        let zone = if e.all_day { "" } else { e.tz.as_str() };
        let wall_from = crate::tz::from_local(from, zone) - Duration::days(1) - dur;
        let wall_to = crate::tz::from_local(to, zone) + Duration::days(1);
        crate::ics::occurrences(s, &e.rrule, wall_from, wall_to, &e.exdates, zone)
            .into_iter()
            .filter_map(|occ| {
                let ls = crate::tz::to_local(occ, zone);
                let le = crate::tz::to_local(occ + dur, zone);
                let overlaps = ls < to && (le > from || (le == ls && ls >= from));
                overlaps.then_some((ls, le, occ))
            })
            .collect()
    }

    fn event_json(&self, e: &Event, cal: &Calendar, ls: NaiveDateTime, le: NaiveDateTime, wall: NaiveDateTime) -> Value {
        json!({
            "id": e.id,
            "title": e.title,
            "start": fmt_dt(ls),
            "end": fmt_dt(le),
            "allDay": e.all_day,
            "location": e.location,
            "notes": e.notes,
            "calendar": e.calendar,
            "color": cal.color,
            "repeats": !e.rrule.is_empty(),
            "seriesStart": e.start,
            "seriesEnd": e.end,
            "occurrence": fmt_dt(wall),
            "rrule": e.rrule,
            "tz": e.tz,
            "virtual": e.id < 0,
            "contactId": -e.id,
            "live": !cal.account.is_empty(),
            "readOnly": cal.source.starts_with("sub:"),
            "alarms": e.alarms,
            "attendees": e.attendees,
            "organizer": e.organizer,
            "notesHtml": e.notes_html,
            "url": e.url,
            "override": !e.recurrence_id.is_empty(),
        })
    }

    /// Events whose title, place or notes match, nearest to today first.
    pub fn search_events(&self, query: &str) -> Value {
        let q = lc(query.trim());
        if q.is_empty() {
            return json!([]);
        }
        let now = Local::now().naive_local();
        let birthdays = self.birthday_events();
        let mut hits: Vec<(i64, Value)> = Vec::new();
        for e in self.events.iter().chain(birthdays.iter()) {
            if !(lc(&e.title).contains(&q) || lc(&e.location).contains(&q) || lc(&e.notes).contains(&q)) {
                continue;
            }
            let Some(cal) = self.calendars.iter().find(|c| c.id == e.calendar) else { continue };
            // Next time it happens, or when it last happened.
            let next = Self::expand(e, now, now + Duration::days(800)).into_iter().next();
            let hit = next.or_else(|| {
                let (Some(s), Some(en)) = (parse_dt(&e.start), parse_dt(&e.end)) else { return None };
                let zone = if e.all_day { "" } else { e.tz.as_str() };
                Some((crate::tz::to_local(s, zone), crate::tz::to_local(en, zone), s))
            });
            let Some((ls, le, wall)) = hit else { continue };
            hits.push(((ls - now).num_minutes().abs(), self.event_json(e, cal, ls, le, wall)));
        }
        hits.sort_by_key(|h| h.0);
        Value::Array(hits.into_iter().take(50).map(|h| h.1).collect())
    }

    /// Events (with repeats expanded) overlapping local [from, to).
    pub fn events(&self, from: &str, to: &str) -> Value {
        let (Some(from), Some(to)) = (parse_dt(from), parse_dt(to)) else {
            return json!([]);
        };
        let mut out: Vec<(NaiveDateTime, bool, Value)> = Vec::new();
        let birthdays = self.birthday_events();
        for e in self.events.iter().chain(birthdays.iter()) {
            let Some(cal) = self.calendars.iter().find(|c| c.id == e.calendar) else { continue };
            if !cal.visible {
                continue;
            }
            for (ls, le, wall) in Self::expand(e, from, to) {
                out.push((ls, e.all_day, self.event_json(e, cal, ls, le, wall)));
            }
        }
        out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        Value::Array(out.into_iter().map(|(_, _, v)| v).collect())
    }

    /// Saves an event from the editor. Times arrive as wall times in the
    /// chosen zone ("tz", empty = floating). Returns the event id.
    pub fn save_event(&mut self, v: &Value) -> i64 {
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let b = |k: &str| v.get(k).and_then(Value::as_bool).unwrap_or(false);
        let id = v.get("id").and_then(Value::as_i64).unwrap_or(0);
        let mut ev = self.events.iter().find(|e| e.id == id && id != 0).cloned().unwrap_or_else(|| Event {
            source: SOURCE_LOCAL.into(),
            ..Default::default()
        });
        ev.calendar = s("calendar");
        ev.title = if s("title").trim().is_empty() { "New Event".into() } else { s("title") };
        ev.start = s("start");
        ev.end = s("end");
        ev.all_day = b("allDay");
        ev.location = s("location");
        // Formatted notes (from the editor): kept only when there's formatting in them.
        if v.get("notesHtml").is_some() {
            let html = notes_body(&s("notesHtml"));
            ev.notes_html = if s("notes").trim().is_empty() || !has_formatting(&html) { String::new() } else { html };
        } else if ev.notes != s("notes") {
            ev.notes_html.clear();
        }
        ev.notes = s("notes");
        if ev.rrule != s("rrule") {
            ev.exdates.clear();
        }
        ev.rrule = s("rrule");
        ev.tz = if ev.all_day { String::new() } else { crate::tz::canonical(&s("tz")) };
        if let Some(a) = v.get("alarms").and_then(Value::as_array) {
            ev.alarms = a.iter().filter_map(Value::as_i64).collect();
        }
        if let Some(a) = v.get("attendees").and_then(Value::as_array) {
            let old = ev.attendees.clone();
            ev.attendees = a
                .iter()
                .filter_map(|x| x.as_str().map(str::trim).filter(|x| x.contains('@')).map(String::from))
                .map(|email| {
                    let (name, addr) = crate::mime::split_address(&email);
                    old.iter().find(|o| o.email.eq_ignore_ascii_case(&addr)).cloned().unwrap_or(Attendee { email: addr, name, status: "NEEDS-ACTION".into() })
                })
                .collect();
            if !ev.attendees.is_empty() && ev.organizer.is_empty() {
                ev.organizer = s("organizer");
            }
        }
        if let (Some(st), Some(en)) = (parse_dt(&ev.start), parse_dt(&ev.end)) {
            if en < st {
                ev.end = fmt_dt(st + Duration::hours(1));
            }
        }
        if !self.calendars.iter().any(|c| c.id == ev.calendar && c.id != BIRTHDAYS) {
            ev.calendar = self
                .calendars
                .iter()
                .find(|c| c.id != BIRTHDAYS)
                .map(|c| c.id.clone())
                .unwrap_or_default();
        }
        // Moving between server calendars makes it a new resource there.
        if let Some(old) = self.events.iter().find(|e| e.id == id && id != 0) {
            if old.calendar != ev.calendar {
                ev.href.clear();
                ev.etag.clear();
            }
        }
        if ev.uid.is_empty() {
            ev.uid = format!("{}-{}@craft-pim", Local::now().timestamp_millis(), self.next_id + 1);
        }
        if let Some(old) = self.events.iter_mut().find(|e| e.id == id && id != 0) {
            *old = ev.clone();
            id
        } else {
            ev.id = self.new_id();
            let id = ev.id;
            self.events.push(ev);
            id
        }
    }

    /// Saves a change to one occurrence of a series only ("this event")
    /// or to it and everything after ("future"). `v` is the editor's data,
    /// `occurrence` the wall time of the occurrence that was opened.
    /// Returns the ids to write to a server.
    pub fn save_event_scoped(&mut self, v: &Value, occurrence: &str, scope: &str) -> Vec<i64> {
        let id = v.get("id").and_then(Value::as_i64).unwrap_or(0);
        let Some(master) = self.events.iter().find(|e| e.id == id).cloned() else { return vec![self.save_event(v)] };
        if master.rrule.is_empty() || occurrence.is_empty() || scope == "all" {
            return vec![self.save_event(v)];
        }
        if scope == "this" {
            // A changed occurrence: same UID, RECURRENCE-ID = the original time.
            let mut o = v.clone();
            o["id"] = json!(0);
            o["rrule"] = json!("");
            let oid = self.save_event(&o);
            if let Some(e) = self.events.iter_mut().find(|e| e.id == oid) {
                e.uid = master.uid.clone();
                e.recurrence_id = occurrence.to_string();
                e.href = master.href.clone();
                e.etag = master.etag.clone();
                e.raw = master.raw.clone();
                e.source = master.source.clone();
                e.exdates.clear();
            }
            self.skip_occurrence(id, occurrence);
            return vec![oid];
        }
        // "future": the old series stops; a new one starts here.
        self.end_series_before(id, occurrence);
        let mut n = v.clone();
        n["id"] = json!(0);
        let nid = self.save_event(&n);
        if let Some(e) = self.events.iter_mut().find(|e| e.id == nid) {
            e.uid = format!("{}-{}@craft-pim", Local::now().timestamp_millis(), nid);
            e.href.clear();
            e.etag.clear();
            e.raw.clear();
            e.recurrence_id.clear();
        }
        vec![id, nid]
    }

    /// Drag to move / resize. Times are this computer's local times; for a
    /// repeating event `scope` is "this" or "all".
    pub fn move_event(&mut self, id: i64, occurrence: &str, new_start: &str, new_end: &str, scope: &str) -> Vec<i64> {
        let Some(e) = self.events.iter().find(|e| e.id == id).cloned() else { return vec![] };
        let zone = if e.all_day { String::new() } else { e.tz.clone() };
        let (Some(ns), Some(ne)) = (parse_dt(new_start), parse_dt(new_end)) else { return vec![] };
        let ws = crate::tz::from_local(ns, &zone);
        let we = crate::tz::from_local(ne, &zone);
        let mut v = json!({
            "id": e.id, "calendar": e.calendar, "title": e.title, "allDay": e.all_day, "location": e.location,
            "notes": e.notes, "rrule": e.rrule, "tz": e.tz, "alarms": e.alarms,
            "start": fmt_dt(ws), "end": fmt_dt(we),
        });
        if !e.rrule.is_empty() && scope != "this" {
            // Shift the whole series by how far this occurrence moved.
            if let (Some(occ), Some(s0), Some(e0)) = (parse_dt(occurrence), parse_dt(&e.start), parse_dt(&e.end)) {
                let d_start = ws - occ;
                let occ_len = e0 - s0;
                let new_len = we - ws;
                v["start"] = json!(fmt_dt(s0 + d_start));
                v["end"] = json!(fmt_dt(s0 + d_start + if new_len.num_minutes() > 0 { new_len } else { occ_len }));
            }
            return vec![self.save_event(&v)];
        }
        self.save_event_scoped(&v, occurrence, if e.rrule.is_empty() { "all" } else { "this" })
    }

    /// The series an event belongs to and its changed occurrences.
    pub fn series(&self, id: i64) -> Option<(Event, Vec<Event>)> {
        let e = self.events.iter().find(|e| e.id == id)?;
        let master = if e.recurrence_id.is_empty() {
            e.clone()
        } else {
            self.events
                .iter()
                .find(|m| m.uid == e.uid && m.calendar == e.calendar && m.recurrence_id.is_empty())
                .cloned()
                .unwrap_or_else(|| e.clone())
        };
        let overrides = self
            .events
            .iter()
            .filter(|o| o.uid == master.uid && o.calendar == master.calendar && !o.recurrence_id.is_empty() && o.id != master.id)
            .cloned()
            .collect();
        Some((master, overrides))
    }

    pub fn event_by_id(&self, id: i64) -> Option<&Event> {
        self.events.iter().find(|e| e.id == id)
    }

    pub fn event_mut(&mut self, id: i64) -> Option<&mut Event> {
        self.events.iter_mut().find(|e| e.id == id)
    }

    pub fn calendar(&self, id: &str) -> Option<&Calendar> {
        self.calendars.iter().find(|c| c.id == id)
    }

    /// Removes an event; returns it so a server copy can be deleted too.
    pub fn delete_event(&mut self, id: i64) -> Option<Event> {
        let pos = self.events.iter().position(|e| e.id == id)?;
        Some(self.events.remove(pos))
    }

    /// Skips one occurrence of a repeating event ("Delete Only This Event").
    pub fn skip_occurrence(&mut self, id: i64, wall: &str) {
        if let Some(e) = self.events.iter_mut().find(|e| e.id == id) {
            if !e.exdates.iter().any(|x| x == wall) {
                e.exdates.push(wall.to_string());
            }
        }
    }

    /// Ends a repeating event before the given occurrence ("Delete All Future Events").
    pub fn end_series_before(&mut self, id: i64, wall: &str) {
        let Some(t) = parse_dt(wall) else { return };
        if let Some(e) = self.events.iter_mut().find(|e| e.id == id) {
            let until = t - Duration::seconds(1);
            let utc = crate::tz::wall_to_utc(until, &e.tz);
            let mut parts: Vec<String> = e
                .rrule
                .split(';')
                .filter(|p| {
                    let k = p.split('=').next().unwrap_or("").to_uppercase();
                    k != "UNTIL" && k != "COUNT" && !p.is_empty()
                })
                .map(String::from)
                .collect();
            parts.push(format!("UNTIL={}", utc.format("%Y%m%dT%H%M%SZ")));
            e.rrule = parts.join(";");
        }
    }

    // -------------------------------------------------------------- contacts

    pub fn groups_json(&self) -> Value {
        let mut list = vec![json!({"id": "", "name": "All Contacts", "count": self.contacts.len()})];
        for g in &self.groups {
            let n = self.contacts.iter().filter(|c| c.groups.contains(&g.id)).count();
            list.push(json!({"id": g.id, "name": g.name, "count": n, "source": g.source}));
        }
        for g in &self.smart_groups {
            let n = self.contacts.iter().filter(|c| self.in_smart_group(c, &g.id)).count();
            list.push(json!({"id": g.id, "name": g.name, "count": n, "smart": true,
                             "matchAll": g.match_all, "conditions": g.conditions}));
        }
        Value::Array(list)
    }

    pub fn contacts(&self, group: &str, query: &str) -> Value {
        let q = lc(query.trim());
        let mut list: Vec<&Contact> = self
            .contacts
            .iter()
            .filter(|c| {
                group.is_empty()
                    || (group.starts_with("smart-") && self.in_smart_group(c, group))
                    || c.groups.iter().any(|g| g == group)
            })
            .filter(|c| {
                q.is_empty()
                    || lc(&display_name(c)).contains(&q)
                    || lc(&c.company).contains(&q)
                    || c.emails.iter().any(|e| lc(&e.value).contains(&q))
                    || c.phones.iter().any(|p| p.value.contains(&q))
            })
            .collect();
        let by_first = self.settings.contacts_sort_first;
        let last_first = self.settings.contacts_last_first;
        list.sort_by_key(|c| if by_first { lc(&display_name(c)) } else { sort_key(c) });
        Value::Array(
            list.iter()
                .map(|c| {
                    let key = if by_first { lc(&display_name(c)) } else { sort_key(c) };
                    let section = key
                        .chars()
                        .next()
                        .filter(|ch| ch.is_alphabetic())
                        .map(|ch| ch.to_uppercase().to_string())
                        .unwrap_or_else(|| "#".into());
                    let shown = if last_first && !c.last.is_empty() { format!("{}, {}", c.last, c.first) } else { display_name(c) };
                    json!({"id": c.id, "name": shown, "first": c.first, "last": c.last, "lastFirst": last_first, "boldFirst": by_first,
                           "company": c.company, "section": section, "photo": c.photo, "isMe": self.my_card == c.id})
                })
                .collect(),
        )
    }

    pub fn contact(&self, id: i64) -> Value {
        match self.contacts.iter().find(|c| c.id == id) {
            Some(c) => {
                let mut v = serde_json::to_value(c).unwrap_or(Value::Null);
                v["name"] = json!(display_name(c));
                v["isMe"] = json!(self.my_card == c.id);
                v["groupNames"] = json!(c
                    .groups
                    .iter()
                    .filter_map(|g| self.groups.iter().find(|x| &x.id == g).map(|x| x.name.clone()))
                    .collect::<Vec<_>>());
                v
            }
            None => Value::Null,
        }
    }

    pub fn save_contact(&mut self, v: &Value) -> i64 {
        let mut c: Contact = serde_json::from_value(v.clone()).unwrap_or_default();
        c.emails.retain(|e| !e.value.trim().is_empty());
        c.phones.retain(|e| !e.value.trim().is_empty());
        if let Some(old) = self.contacts.iter_mut().find(|x| x.id == c.id && c.id != 0) {
            c.source = old.source.clone();
            *old = c.clone();
            c.id
        } else {
            c.id = self.new_id();
            c.source = SOURCE_LOCAL.into();
            let id = c.id;
            self.contacts.push(c);
            id
        }
    }

    pub fn delete_contact(&mut self, id: i64) {
        self.contacts.retain(|c| c.id != id);
    }

    pub fn add_group(&mut self, name: &str) -> String {
        let id = format!("local-group-{}", self.new_id());
        self.groups.push(Group {
            id: id.clone(),
            name: name.to_string(),
            source: SOURCE_LOCAL.into(),
            ..Default::default()
        });
        id
    }

    /// Everyone in the mail, from senders, recipients and copies, with how
    /// often and how lately they appear (kept until mail is added or removed).
    pub fn with_people<T>(&self, f: impl FnOnce(&[Person]) -> T) -> T {
        let fp = if self.backed {
            // (Worked out again after a while of saves, not at every one.)
            crate::mailstore::SAVES.load(std::sync::atomic::Ordering::SeqCst) / 50
        } else {
            (self.messages.len() as u64) << 32 ^ self.messages.iter().map(|m| m.id as u64).max().unwrap_or(0)
        };
        let mut cache = self.people_cache.lock().unwrap();
        if cache.as_ref().map(|(k, _)| *k) != Some(fp) {
            let sent: std::collections::HashSet<&str> = self.folders.iter().filter(|f| f.kind == "sent").map(|f| f.id.as_str()).collect();
            let mut by: std::collections::HashMap<String, Person> = std::collections::HashMap::new();
            let mut note = |name: &str, email: &str, date: i64, written: bool| {
                let key = email.to_ascii_lowercase();
                let p = by.entry(key).or_insert_with(|| Person { email: email.to_string(), ..Default::default() });
                p.seen += 1;
                if written {
                    p.written += 1;
                    p.last_written = p.last_written.max(date);
                }
                // The newest name they went by.
                if !name.is_empty() && !name.contains('@') && date >= p.last {
                    p.name = name.to_string();
                }
                p.last = p.last.max(date);
            };
            let mut each = |folder: &str, from_name: &str, from_email: &str, to: &str, cc: &str, date: i64| {
                let mine = sent.contains(folder);
                if !from_email.is_empty() && !mine {
                    note(from_name, from_email, date, false);
                }
                for (name, email) in split_addresses(to).into_iter().chain(split_addresses(cc)) {
                    note(&name, &email, date, mine);
                }
            };
            for m in &self.messages {
                each(&m.folder, &m.from_name, &m.from_email, &m.to, &m.cc, m.date);
            }
            // (Mail in the database: the newest 100,000 messages are plenty to know who's who.)
            if self.backed {
                crate::mailstore::each_person_row(100_000, |f, n, e, t, c, d| each(f, n, e, t, c, d));
            }
            *cache = Some((fp, by.into_values().collect()));
        }
        f(&cache.as_ref().unwrap().1)
    }

    /// Recipient suggestions while typing an address: contacts and everyone in
    /// the mail history, the people you wrote to most recently first, then
    /// those who wrote to you most recently. Matches the start of the address
    /// or of any word of the name.
    pub fn email_suggestions(&self, prefix: &str) -> Value {
        let p = lc(prefix.trim());
        if p.is_empty() {
            return json!([]);
        }
        let matches = |name: &str, email: &str| {
            lc(email).starts_with(&p) || lc(name).starts_with(&p) || lc(name).split(|c: char| !c.is_alphanumeric()).any(|w| w.starts_with(&p))
        };
        let mine: std::collections::HashSet<String> = self.accounts.iter().map(|a| a.email.to_ascii_lowercase()).collect();
        self.with_people(|people| {
            let when: std::collections::HashMap<String, (i64, i64)> =
                people.iter().map(|x| (x.email.to_ascii_lowercase(), (x.last_written, x.last))).collect();
            // (last written to, last seen, name, address)
            let mut found: Vec<(i64, i64, String, String)> = Vec::new();
            let mut taken = std::collections::HashSet::new();
            for c in &self.contacts {
                let name = display_name(c);
                for e in &c.emails {
                    let key = e.value.to_ascii_lowercase();
                    if matches(&name, &e.value) && taken.insert(key.clone()) {
                        let (w, l) = when.get(&key).copied().unwrap_or((0, 0));
                        found.push((w, l, name.clone(), e.value.clone()));
                    }
                }
            }
            for x in people {
                let key = x.email.to_ascii_lowercase();
                if matches(&x.name, &x.email) && taken.insert(key.clone()) {
                    // Your own addresses are in nearly every message you get, so
                    // they rank only by when you wrote to yourself.
                    let seen = if mine.contains(&key) { 0 } else { x.last };
                    found.push((x.last_written, seen, x.name.clone(), x.email.clone()));
                }
            }
            // Your own addresses, even when the mail never mentions them.
            for a in &self.accounts {
                let key = a.email.to_ascii_lowercase();
                // (An account's name is a label like "Gmail", not yours.)
                if key.contains('@') && matches("", &a.email) && taken.insert(key) {
                    found.push((0, 0, String::new(), a.email.clone()));
                }
            }
            found.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(a.2.to_lowercase().cmp(&b.2.to_lowercase())));
            Value::Array(found.into_iter().take(12).map(|(_, _, name, email)| json!({"name": name, "email": email})).collect())
        })
    }

    // --------------------------------------------------------------- imports

    pub fn source_settings(&mut self, id: &str) -> &mut SourceSettings {
        if !self.sources.iter().any(|s| s.id == id) {
            self.sources.push(SourceSettings {
                id: id.into(),
                auto_import: false,
                last_import: String::new(),
            });
        }
        self.sources.iter_mut().find(|s| s.id == id).unwrap()
    }

    pub fn counts_for(&self, source: &str) -> Value {
        json!({
            "messages": self.message_count(source),
            "events": self.events.iter().filter(|e| e.source == source).count(),
            "contacts": self.contacts.iter().filter(|c| c.source == source).count(),
        })
    }

    /// Replaces everything that came from `imp.source` earlier with `imp`.
    /// Items seen before keep their ids so open selections stay put.
    pub fn apply_import(&mut self, mut imp: Imported) -> Vec<i64> {
        let _t = crate::Timed::new("applying a mail check");
        let mut new_ids: Vec<i64> = Vec::new();
        use std::collections::HashMap;
        let src = imp.source.clone();
        let keep_mail = imp.failed.iter().any(|f| f == "mail");
        let keep_cals = imp.failed.iter().any(|f| f == "calendars");
        let keep_cards = imp.failed.iter().any(|f| f == "contacts");
        if keep_mail {
            imp.accounts.clear();
            imp.folders.clear();
            imp.messages.clear();
        }
        if keep_cals {
            imp.calendars.clear();
            imp.events.clear();
        }
        if keep_cards {
            imp.contacts.clear();
            imp.groups.clear();
        }
        let msg_key = |m: &Message| if m.uid > 0 { format!("{}#{}", m.folder, m.uid) } else { format!("{}#{}#{}", m.folder, m.date, m.subject) };
        let ev_key = |e: &Event| format!("{}#{}#{}#{}", e.calendar, e.uid, e.start, e.title);
        let card_key = |c: &Contact| if !c.href.is_empty() { c.href.clone() } else { format!("{}#{}#{}", c.uid, c.first, c.last) };
        // Keep read/flag changes made in this app for imported (not live) mail.
        let mut old_msgs: HashMap<String, (i64, bool, bool)> = HashMap::new();
        // "Remind Me" times set in this app (the server doesn't know them).
        let mut old_remind: HashMap<String, i64> = HashMap::new();
        // Text already downloaded, for messages that arrive as headers only.
        let mut old_text: HashMap<String, Message> = HashMap::new();
        // Mail of a source that comes whole each time (an import): all of it in
        // memory for this (it's replaced).
        if self.backed && imp.mail_delta.is_none() && !keep_mail {
            let here: std::collections::HashSet<i64> = self.messages.iter().map(|m| m.id).collect();
            let all = crate::mailstore::rows("source = ?1", &[crate::mailstore::Arg::Text(src.clone())], "date DESC", i64::MAX, 0);
            for m in all.into_iter().filter(|m| !here.contains(&m.id) && !crate::mailstore::is_tracked(m.id)) {
                crate::mailstore::note_loaded(&m);
                self.messages.push(m);
            }
        }
        // (Had mail before: what's new now is announced.)
        let had_mail = self.message_count(&src) > 0;
        for m in self.messages.iter().filter(|m| m.source == src) {
            old_msgs.insert(msg_key(m), (m.id, m.read, m.flagged));
            if m.remind_at > 0 {
                old_remind.insert(msg_key(m), m.remind_at);
            }
            if !m.partial {
                old_text.insert(msg_key(m), m.clone());
            }
        }
        let old_events: HashMap<String, i64> = self.events.iter().filter(|e| e.source == src).map(|e| (ev_key(e), e.id)).collect();
        let old_cards: HashMap<String, i64> = self.contacts.iter().filter(|c| c.source == src).map(|c| (card_key(c), c.id)).collect();
        let old_vis: HashMap<String, bool> = self
            .calendars
            .iter()
            .filter(|c| c.source == src)
            .map(|c| (c.id.clone(), c.visible))
            .collect();

        if !keep_mail {
            self.accounts.retain(|a| a.source != src);
            let src_accounts: Vec<String> = imp.accounts.iter().map(|a| a.id.clone()).collect();
            self.folders
                .retain(|f| !src_accounts.contains(&f.account) && !f.id.starts_with(&format!("{src}:")));
            // Older mail loaded on request ("Load Older Messages") lies below
            // the newest-messages window a sync reads: keep it.
            let mut window: HashMap<String, u32> = HashMap::new();
            if imp.server_truth {
                for m in imp.messages.iter().filter(|m| m.uid > 0) {
                    let e = window.entry(m.folder.clone()).or_insert(m.uid);
                    *e = (*e).min(m.uid);
                }
            }
            if let Some(delta) = imp.mail_delta.take() {
                // Changes only: everything else (downloaded text, reminders,
                // attachments) stays as it is.
                let folders: std::collections::HashSet<String> = imp.folders.iter().map(|f| f.id.clone()).collect();
                let gone: std::collections::HashSet<(String, u32)> = delta.gone.into_iter().collect();
                // (Mailboxes gone from the server lose theirs.)
                self.folder_marks.retain(|f, _| !f.starts_with(&format!("{src}:")) || folders.contains(f));
                self.folder_marks.extend(delta.marks);
                self.messages.retain(|m| m.source != src || m.uid == 0 && folders.contains(&m.folder) || (folders.contains(&m.folder) && !gone.contains(&(m.folder.clone(), m.uid))));
                if self.backed {
                    // (And those not in memory, in the database.)
                    let keep: Vec<String> = folders.iter().cloned().collect();
                    crate::mailstore::remove(&src, crate::mailstore::Gone::FoldersNotIn(&keep));
                    let list: Vec<(String, u32)> = gone.iter().cloned().collect();
                    crate::mailstore::remove(&src, crate::mailstore::Gone::Uids(&list));
                }
                let mut index: HashMap<(String, u32), usize> = HashMap::new();
                for (i, m) in self.messages.iter().enumerate().filter(|(_, m)| m.source == src && m.uid > 0) {
                    index.insert((m.folder.clone(), m.uid), i);
                }
                let mut elsewhere = Vec::new();
                let flags = self.unguarded_flags(&src, delta.started, delta.flags);
                for (folder, uid, read, flagged, color) in flags {
                    if let Some(&i) = index.get(&(folder.clone(), uid)) {
                        self.messages[i].read = read;
                        self.messages[i].flagged = flagged;
                        if color != NO_COLOR {
                            self.messages[i].flag_color = color;
                        }
                    } else if self.backed {
                        elsewhere.push((folder, uid, read, flagged, color));
                    }
                }
                crate::mailstore::update_flags(&src, &elsewhere);
                for mut m in delta.new {
                    if index.contains_key(&(m.folder.clone(), m.uid)) {
                        continue;
                    }
                    if self.backed && crate::mailstore::id_at(&src, &m.folder, m.uid).is_some() {
                        continue;
                    }
                    m.source = src.clone();
                    m.id = self.new_id();
                    new_ids.push(m.id);
                    index.insert((m.folder.clone(), m.uid), self.messages.len());
                    self.messages.push(m);
                }
                imp.messages.clear();
            } else {
                let older: Vec<Message> = self
                    .messages
                    .iter()
                    .filter(|m| m.source == src && m.uid > 0 && window.get(&m.folder).map(|low| m.uid < *low).unwrap_or(false))
                    .cloned()
                    .collect();
                self.messages.retain(|m| m.source != src);
                self.messages.extend(older);
            }
        }
        if !keep_cals {
            self.calendars.retain(|c| c.source != src);
            self.events.retain(|e| e.source != src);
        }
        if !keep_cards {
            self.contacts.retain(|c| c.source != src);
            self.groups.retain(|g| g.source != src);
        }

        for m in imp.messages.iter_mut() {
            m.source = src.clone();
            if m.partial {
                if let Some(o) = old_text.get(&msg_key(m)) {
                    m.body = o.body.clone();
                    m.html = o.html.clone();
                    m.invite = o.invite.clone();
                    m.attachments = o.attachments.clone();
                    m.raw_ref = o.raw_ref.clone();
                    m.partial = false;
                }
            }
            if let Some(at) = old_remind.get(&msg_key(m)) {
                m.remind_at = *at;
            }
            match old_msgs.get(&msg_key(m)) {
                Some((id, r, f)) => {
                    m.id = *id;
                    if !imp.server_truth {
                        m.read = *r;
                        m.flagged = *f;
                    }
                }
                None => {
                    m.id = self.new_id();
                    new_ids.push(m.id);
                }
            }
        }
        for e in imp.events.iter_mut() {
            e.source = src.clone();
            e.id = old_events.get(&ev_key(e)).copied().unwrap_or_else(|| {
                self.next_id += 1;
                self.next_id
            });
        }
        for c in imp.contacts.iter_mut() {
            c.source = src.clone();
            c.id = old_cards.get(&card_key(c)).copied().unwrap_or_else(|| {
                self.next_id += 1;
                self.next_id
            });
        }
        for c in imp.calendars.iter_mut() {
            c.source = src.clone();
            if let Some(v) = old_vis.get(&c.id) {
                c.visible = *v;
            }
            // A name or colour chosen here wins over the server's.
            if let Some((name, color)) = self.calendar_looks.get(&c.id) {
                c.name = name.clone();
                c.color = color.clone();
            }
        }
        for g in imp.groups.iter_mut() {
            g.source = src.clone();
        }
        self.accounts.extend(imp.accounts);
        self.folders.extend(imp.folders);
        self.messages.extend(imp.messages);
        self.calendars.extend(imp.calendars);
        self.events.extend(imp.events);
        self.contacts.extend(imp.contacts);
        self.groups.extend(imp.groups);
        let now = Local::now().format("%Y-%m-%d %H:%M").to_string();
        self.source_settings(&src).last_import = now;
        // On the very first read everything is "new"; only later arrivals count.
        if old_msgs.is_empty() && !had_mail {
            new_ids.clear();
        }
        new_ids
    }

    // ---------------------------------------------------------- sample data

    pub fn sample() -> Db {
        let mut db = Db { is_sample: true, ..Default::default() };
        db.accounts.push(Account {
            id: "local".into(),
            name: "On My Computer".into(),
            email: "me@example.com".into(),
            source: SOURCE_LOCAL.into(),
        });
        for kind in ["inbox", "drafts", "sent", "junk", "trash", "archive"] {
            db.folders.push(Folder {
                id: format!("local/{kind}"),
                account: "local".into(),
                name: kind_name(kind).into(),
                kind: kind.into(),
                ..Default::default()
            });
        }
        let now = Local::now();
        let mails: [(&str, &str, &str, &str, i64, bool, bool, &str); 12] = [
            ("Riley Park", "riley@example.org", "Project kickoff on Thursday",
             "Hi,\n\nJust a reminder that the project kickoff is on Thursday at 10:00 in the large meeting room. Please bring your notes from last week and any questions you have about the timeline.\n\nI've attached the agenda in the shared folder.\n\nSee you there,\nRiley", 1, false, true, "inbox"),
            ("Sam Lee", "sam@example.net", "Weekend hike?",
             "Hey! The weather looks great for Saturday. Want to do the lake loop trail? We could start around 8 and be back by lunch.\n\nLet me know,\nSam", 3, false, false, "inbox"),
            ("Library Notices", "notices@library.example", "Your reserved book is ready",
             "Hello,\n\nThe item you reserved is now available for pickup at the front desk. It will be held for 7 days.\n\nThank you.", 6, true, false, "inbox"),
            ("Jordan Diaz", "jordan@example.com", "Re: Draft budget",
             "Thanks for sending this over. I made a few changes to the travel section and left comments on the second page. Overall it looks good to me.\n\n> Here's the first draft of the budget.\n\nJordan", 20, true, true, "inbox"),
            ("Alex Kim", "alex@example.org", "Photos from the trip",
             "Finally sorted through all the photos! I put the best ones in an album. The sunset ones came out really well.\n\nAlex", 27, true, false, "inbox"),
            ("Morgan Wu", "morgan@example.net", "Lunch next week",
             "Are you free for lunch on Tuesday or Wednesday next week? There's a new noodle place near the office I've been wanting to try.", 50, true, false, "inbox"),
            ("Taylor Brooks", "taylor@example.com", "Quarterly report numbers",
             "Hi team,\n\nThe quarterly numbers are in. Sales were up 8% compared to last quarter, mostly thanks to the spring campaign. Full details in the report.\n\nBest,\nTaylor", 75, true, false, "inbox"),
            ("Casey Rivera", "casey@example.org", "Book club: next pick",
             "Our next book club pick is a short mystery novel. Meeting is in three weeks at my place. Snacks welcome!", 130, true, false, "inbox"),
            ("Prize Desk", "winner@spam.example", "You may have won!!!",
             "Click here to claim your prize. This is definitely not junk.", 30, false, false, "junk"),
            ("Me", "me@example.com", "Notes for the meeting",
             "Points to bring up:\n- timeline\n- budget\n- who owns the launch checklist", 8, true, false, "drafts"),
            ("Me", "me@example.com", "Re: Lunch next week",
             "Wednesday works for me! See you at 12:30.", 48, true, false, "sent"),
            ("Drew Patel", "drew@example.com", "Old newsletter",
             "This month's newsletter: updates from the garden club and the spring plant swap.", 400, true, false, "archive"),
        ];
        for (name, email, subject, body, hours_ago, read, flagged, kind) in mails {
            let id = db.new_id();
            let to = if kind == "sent" || kind == "drafts" { "morgan@example.net" } else { "me@example.com" };
            db.messages.push(Message {
                id,
                folder: format!("local/{kind}"),
                from_name: name.into(),
                from_email: email.into(),
                to: to.into(),
                cc: String::new(),
                subject: subject.into(),
                body: body.into(),
                date: (now - Duration::hours(hours_ago) - Duration::minutes(hours_ago * 7 % 60)).timestamp(),
                read,
                flagged,
                source: SOURCE_LOCAL.into(),
                attachments: Vec::new(),
                ..Default::default()
            });
        }
        // Flags in a few colours (red, then blue, green …), as Apple Mail's.
        for (n, m) in db.messages.iter_mut().filter(|m| m.flagged).enumerate() {
            m.flag_color = [0, 4, 3, 1, 5][n % 5];
        }

        // A designed newsletter (its own styles, a 600-pixel layout, a
        // background picture and a remote picture), as many real emails are.
        {
            let bg = "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSI2MDAiIGhlaWdodD0iMjAwIj48ZGVmcz48bGluZWFyR3JhZGllbnQgaWQ9ImciIHgxPSIwIiB5MT0iMCIgeDI9IjEiIHkyPSIxIj48c3RvcCBvZmZzZXQ9IjAiIHN0b3AtY29sb3I9IiMyZjdjZjYiLz48c3RvcCBvZmZzZXQ9IjEiIHN0b3AtY29sb3I9IiM5YjU5ZDAiLz48L2xpbmVhckdyYWRpZW50PjwvZGVmcz48cmVjdCB3aWR0aD0iNjAwIiBoZWlnaHQ9IjIwMCIgZmlsbD0idXJsKCNnKSIvPjwvc3ZnPg==";
            let html = format!(
                r##"<html><head><style>
.wrap {{ width: 600px; margin: 0 auto; font-family: Georgia, serif; }}
.hero {{ background-image: url('{bg}'); background-size: cover; color: #fff; padding: 48px 32px; border-radius: 12px 12px 0 0; }}
.hero h1 {{ margin: 0; font-size: 30px; }}
.cols td {{ width: 50%; vertical-align: top; padding: 16px; background: #f4f1ea; color: #333; }}
.btn {{ display: inline-block; background: #e8453c; color: #fff !important; padding: 10px 18px; border-radius: 20px; text-decoration: none; }}
</style></head><body style="background:#e9e4da">
<table class="wrap" cellpadding="0" cellspacing="0" bgcolor="#ffffff"><tr><td class="hero"><h1>Garden Club · Spring</h1><p>Plant swap, workshops and a tomato contest.</p></td></tr>
<tr><td><img src="https://example.com/garden/banner.jpg" width="600" alt="Banner"></td></tr>
<tr><td><table class="cols" width="100%"><tr><td><b>Plant swap</b><br>Saturday 10–2 at the community hall. Bring cuttings and seeds.</td>
<td><b>Workshops</b><br>Composting basics and spring pruning, every Tuesday evening.</td></tr></table></td></tr>
<tr><td style="padding:16px 32px 28px"><a class="btn" href="https://example.com/garden/signup">Sign up</a></td></tr></table></body></html>"##
            );
            let id = db.new_id();
            db.messages.push(Message {
                id,
                folder: "local/inbox".into(),
                from_name: "Garden Club".into(),
                from_email: "news@garden.example".into(),
                to: "me@example.com".into(),
                subject: "Spring newsletter: plant swap and workshops".into(),
                body: "Garden Club · Spring\n\nPlant swap Saturday 10–2 at the community hall. Workshops every Tuesday evening.".into(),
                html: crate::mime::clean_html(&html),
                date: (now - Duration::hours(9)).timestamp(),
                read: true,
                source: SOURCE_LOCAL.into(),
                ..Default::default()
            });
        }

        for (id, name, color) in [
            ("home", "Home", "#2f7cf6"),
            ("work", "Work", "#e8453c"),
            ("family", "Family", "#34a853"),
            ("birthdays", "Birthdays", "#9b59d0"),
        ] {
            db.calendars.push(Calendar {
                id: id.into(),
                name: name.into(),
                color: color.into(),
                visible: true,
                source: SOURCE_LOCAL.into(),
                ..Default::default()
            });
        }
        let today = now.date_naive();
        let monday = today - Duration::days(today.weekday().num_days_from_monday() as i64);
        let at = |d: NaiveDate, h: u32, m: u32| d.and_hms_opt(h, m, 0).unwrap();
        let events: Vec<(&str, &str, NaiveDateTime, NaiveDateTime, bool, &str, &str)> = vec![
            ("work", "Team stand-up", at(monday, 9, 30), at(monday, 9, 45), false, "Room 2", "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR"),
            ("work", "Project kickoff", at(monday + Duration::days(3), 10, 0), at(monday + Duration::days(3), 11, 30), false, "Large meeting room", ""),
            ("work", "Design review", at(today + Duration::days(1), 14, 0), at(today + Duration::days(1), 15, 0), false, "Video call", ""),
            ("home", "Dentist", at(today + Duration::days(2), 8, 15), at(today + Duration::days(2), 9, 0), false, "Main St Clinic", ""),
            ("home", "Yoga", at(monday + Duration::days(1), 18, 0), at(monday + Duration::days(1), 19, 0), false, "Studio", "FREQ=WEEKLY"),
            ("family", "Dinner with family", at(monday + Duration::days(5), 18, 30), at(monday + Duration::days(5), 21, 0), false, "", ""),
            ("family", "Lake hike", at(monday + Duration::days(5), 8, 0), at(monday + Duration::days(5), 12, 0), false, "Lake loop trail", ""),
            ("home", "Trip to the coast", at(today + Duration::days(9), 0, 0), at(today + Duration::days(12), 0, 0), true, "", ""),
            ("work", "Quarterly planning", at(today - Duration::days(6), 13, 0), at(today - Duration::days(6), 16, 0), false, "Board room", ""),
            ("home", "Pay rent", at(today.with_day(1).unwrap(), 0, 0), at(today.with_day(1).unwrap() + Duration::days(1), 0, 0), true, "", "FREQ=MONTHLY"),
            ("work", "Lunch with Morgan", at(today + Duration::days(7), 12, 30), at(today + Duration::days(7), 13, 30), false, "Noodle place", ""),
        ];
        for (cal, title, s, e, all_day, loc, rrule) in events {
            let id = db.new_id();
            db.events.push(Event {
                id,
                calendar: cal.into(),
                title: title.into(),
                start: fmt_dt(s),
                end: fmt_dt(e),
                all_day,
                location: loc.into(),
                notes: String::new(),
                rrule: rrule.into(),
                source: SOURCE_LOCAL.into(),
                ..Default::default()
            });
        }
        // A video call with notes as calendar services send them (formatted, with links).
        if let Some(e) = db.events.iter_mut().find(|e| e.title == "Design review") {
            e.location = "https://example.zoom.us/j/81234567890?pwd=abc123".into();
            e.notes = "<p>Hi all, let's go over the <b>new list layout</b> before it ships.</p><p>&amp;#128205;: Room 4B</p>\
                       <p><b>Join Zoom Meeting</b><br><a href=\"https://example.zoom.us/j/81234567890?pwd=abc123\">https://example.zoom.us/j/81234567890?pwd=abc123</a></p>\
                       <p>Meeting ID: 812 3456 7890<br>Passcode: abc123</p>\
                       <ul><li>Mock-ups: https://example.com/designs/list</li><li>Questions to <span style=\"color:#000000\">riley@example.org</span></li></ul>"
                .into();
            e.alarms = vec![10];
            e.attendees = vec![
                Attendee { email: "riley@example.org".into(), name: "Riley Park".into(), status: "ACCEPTED".into() },
                Attendee { email: "sam@example.net".into(), name: "Sam Lee".into(), status: "TENTATIVE".into() },
            ];
            e.organizer = "mailto:riley@example.org".into();
        }
        // A few sample attachments and a reply so conversations show.
        for m in db.messages.iter_mut() {
            if m.subject == "Quarterly report numbers" {
                m.html = "<p>Hi team,</p><p>The quarterly numbers are in. Sales were <b>up 8%</b> compared to last quarter, mostly thanks to the spring campaign.</p>\
                          <table border='1' cellpadding='6' cellspacing='0'><tr><th>Region</th><th>Change</th></tr><tr><td>North</td><td>+11%</td></tr>\
                          <tr><td>South</td><td>+6%</td></tr></table><p>Full details in the report.</p><p>Best,<br>Taylor</p>"
                    .into();
            }
            match m.subject.as_str() {
                "Project kickoff on Thursday" => {
                    m.attachments = vec!["Agenda.pdf".into()];
                    // Several people on To and Cc, as a kickoff invite has.
                    m.to = "Me <me@example.com>, Alex Kim <alex@example.org>, Jordan Diaz <jordan@example.com>, Morgan Wu <morgan@example.net>".into();
                    m.cc = "Taylor Brooks <taylor@example.com>, Casey Rivera <casey@example.org>, Drew Patel <drew@example.com>".into();
                }
                "Photos from the trip" => m.attachments = vec!["Sunset.jpg".into(), "Lake.jpg".into(), "Trail.jpg".into()],
                "Quarterly report numbers" => m.attachments = vec!["Q3 Report.xlsx".into()],
                _ => {}
            }
        }
        let id = db.new_id();
        db.messages.push(Message {
            id,
            folder: "local/inbox".into(),
            from_name: "Sam Lee".into(),
            from_email: "sam@example.net".into(),
            to: "me@example.com".into(),
            subject: "Re: Weekend hike?".into(),
            body: "Also, I can bring snacks and the trail map. Do you still have the big thermos?\n\nSam".into(),
            date: (now - Duration::minutes(25)).timestamp(),
            read: false,
            source: SOURCE_LOCAL.into(),
            ..Default::default()
        });

        // A meeting invitation.
        let friday = monday + Duration::days(4);
        let invite = format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Example//EN\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:offsite-planning@example.com\r\n\
             DTSTART:{d}T150000\r\nDTEND:{d}T160000\r\nSUMMARY:Offsite planning\r\nLOCATION:Room 4\r\n\
             ORGANIZER;CN=Taylor Brooks:mailto:taylor@example.com\r\nATTENDEE;PARTSTAT=NEEDS-ACTION;RSVP=TRUE:mailto:me@example.com\r\n\
             END:VEVENT\r\nEND:VCALENDAR\r\n",
            d = friday.format("%Y%m%d")
        );
        let id = db.new_id();
        db.messages.push(Message {
            id,
            folder: "local/inbox".into(),
            from_name: "Taylor Brooks".into(),
            from_email: "taylor@example.com".into(),
            to: "me@example.com".into(),
            subject: "Invitation: Offsite planning".into(),
            body: "Let's plan the team offsite. Please accept if you can make it.\n\nTaylor".into(),
            date: (now - Duration::hours(2)).timestamp(),
            read: true,
            source: SOURCE_LOCAL.into(),
            invite,
            ..Default::default()
        });

        for (id, name) in [("friends", "Friends"), ("coworkers", "Coworkers"), ("family", "Family")] {
            db.groups.push(Group { id: id.into(), name: name.into(), source: SOURCE_LOCAL.into() , ..Default::default() });
        }
        let people: [(&str, &str, &str, &str, &str, &str, &str); 12] = [
            ("Alex", "Kim", "", "alex@example.org", "+1 555 0101", "friends", "1990-04-12"),
            ("Casey", "Rivera", "Book Club", "casey@example.org", "+1 555 0102", "friends", ""),
            ("Drew", "Patel", "Garden Club", "drew@example.com", "+1 555 0103", "", ""),
            ("Jordan", "Diaz", "Northwind Ltd", "jordan@example.com", "+1 555 0104", "coworkers", ""),
            ("Morgan", "Wu", "Northwind Ltd", "morgan@example.net", "+1 555 0105", "coworkers", ""),
            ("Riley", "Park", "Northwind Ltd", "riley@example.org", "+1 555 0106", "coworkers", "1987-09-30"),
            ("Sam", "Lee", "", "sam@example.net", "+1 555 0107", "friends", ""),
            ("Taylor", "Brooks", "Northwind Ltd", "taylor@example.com", "+1 555 0108", "coworkers", ""),
            ("Jamie", "Lee", "", "jamie@example.net", "+1 555 0109", "family", ""),
            ("Robin", "Lee", "", "robin@example.net", "+1 555 0110", "family", ""),
            ("Avery", "Nguyen", "City Library", "avery@library.example", "+1 555 0111", "", ""),
            ("Quinn", "Foster", "Foster Bakery", "quinn@example.com", "+1 555 0112", "friends", ""),
        ];
        for (first, last, company, email, phone, group, bday) in people {
            let id = db.new_id();
            db.contacts.push(Contact {
                id,
                first: first.into(),
                last: last.into(),
                company: company.into(),
                job_title: String::new(),
                emails: vec![Labeled { label: "home".into(), value: email.into() }],
                phones: vec![Labeled { label: "mobile".into(), value: phone.into() }],
                address: if company.is_empty() { String::new() } else { "12 Example Street\nSpringfield".into() },
                birthday: bday.into(),
                notes: String::new(),
                groups: if group.is_empty() { vec![] } else { vec![group.into()] },
                source: SOURCE_LOCAL.into(),
                ..Default::default()
            });
        }
        // Sam's birthday falls later this week so the Birthdays calendar shows.
        let soon = today + Duration::days(4);
        if let Some(sam) = db.contacts.iter_mut().find(|c| c.first == "Sam") {
            sam.birthday = format!("1991-{:02}-{:02}", soon.month(), soon.day());
        }
        let _ = now.hour();
        db
    }

    // ------------------------------------------------------ live accounts

    pub fn live_account(&self, id: &str) -> Option<&crate::live::LiveAccount> {
        self.live.iter().find(|a| a.id == id)
    }

    /// The live account a data source ("live:<id>") belongs to.
    pub fn live_for_source(&self, source: &str) -> Option<crate::live::LiveAccount> {
        let id = source.strip_prefix("live:")?;
        self.live_account(id).cloned()
    }

    /// For a message on a live account: (account, server folder, UID).
    pub fn live_message(&self, id: i64) -> Option<(crate::live::LiveAccount, String, u32)> {
        let m = self.get(id)?;
        let acct = self.live_for_source(&m.source)?;
        let path = self.folder(&m.folder)?.path.clone();
        Some((acct, path, m.uid))
    }

    /// Server path of the folder of `kind` in the same account as message `id`.
    pub fn live_folder_of_kind(&self, id: i64, kind: &str) -> Option<String> {
        let m = self.get(id)?;
        let account = &self.folder(&m.folder)?.account;
        self.folders.iter().find(|f| &f.account == account && f.kind == kind).map(|f| f.path.clone())
    }

    pub fn folder_path(&self, folder_id: &str) -> Option<String> {
        self.folder(folder_id).map(|f| f.path.clone()).filter(|p| !p.is_empty())
    }

    /// Server path of a folder kind for a live account id.
    pub fn live_kind_path(&self, account_id: &str, kind: &str) -> Option<(String, String)> {
        let acct = format!("live:{account_id}:mail");
        self.folders
            .iter()
            .find(|f| f.account == acct && f.kind == kind)
            .map(|f| (f.id.clone(), f.path.clone()))
    }

    pub fn upsert_live(&mut self, a: crate::live::LiveAccount) {
        match self.live.iter_mut().find(|x| x.id == a.id) {
            Some(x) => *x = a,
            None => self.live.push(a),
        }
    }

    pub fn remove_live(&mut self, id: &str) {
        let src = format!("live:{id}");
        self.live.retain(|a| a.id != id);
        self.accounts.retain(|a| a.source != src);
        self.folders.retain(|f| !f.account.starts_with(&format!("{src}:")));
        self.messages.retain(|m| m.source != src);
        if self.backed {
            crate::mailstore::remove(&src, crate::mailstore::Gone::All);
        }
        self.calendars.retain(|c| c.source != src);
        self.events.retain(|e| e.source != src);
        self.contacts.retain(|c| c.source != src);
        self.groups.retain(|g| g.source != src);
    }

    pub fn live_json(&self) -> Value {
        Value::Array(
            self.live
                .iter()
                .map(|a| {
                    let src = a.source();
                    json!({
                        "id": a.id, "name": a.name, "email": a.email, "fullName": a.full_name,
                        "username": a.username, "imapHost": a.imap_host, "imapPort": a.imap_port,
                        "smtpHost": a.smtp_host, "smtpPort": a.smtp_port, "caldavUrl": a.caldav_url,
                        "carddavUrl": a.carddav_url, "lastSync": a.last_sync, "lastError": a.last_error,
                        "provider": a.provider, "clientId": a.client_id, "clientSecret": a.client_secret, "tenant": a.tenant,
                        // Google Drive allowed at the last sign-in: "yes", "no", or "" (not known).
                        "driveAccess": if a.provider == "google" { crate::oauth::load_extra(&format!("{}:drive", a.id)) } else { String::new() },
                        "messages": self.message_count(&src),
                        "events": self.events.iter().filter(|e| e.source == src).count(),
                        "contacts": self.contacts.iter().filter(|c| c.source == src).count(),
                    })
                })
                .collect(),
        )
    }

    /// Accounts that can send: live ones plus "On My Computer".
    pub fn senders_json(&self) -> Value {
        let mut out: Vec<Value> = self
            .live
            .iter()
            .filter(|a| !a.smtp_host.is_empty())
            .map(|a| json!({"id": a.id, "label": format!("{} <{}>", if a.full_name.is_empty() { &a.name } else { &a.full_name }, a.email)}))
            .collect();
        out.push(json!({"id": "", "label": "On My Computer (not sent)"}));
        Value::Array(out)
    }

    /// Which live account a message belongs to ("" if none).
    pub fn message_account(&self, id: i64) -> String {
        self.get(id).and_then(|m| m.source.strip_prefix("live:").map(String::from)).unwrap_or_default()
    }

    /// How many messages came from a source.
    pub fn message_count(&self, source: &str) -> usize {
        if self.backed {
            let prefix = format!("{source}:");
            return self.mail_counts().by_folder.iter().filter(|(f, _)| f.starts_with(&prefix)).map(|(_, v)| v.0.max(0) as usize).sum();
        }
        self.messages.iter().filter(|m| m.source == source).count()
    }

    /// Records a message just sent from a live account in its Sent folder.
    pub fn store_sent_live(&mut self, account_id: &str, v: &Value) {
        let Some((folder, _)) = self.live_kind_path(account_id, "sent") else {
            self.store_outgoing(v, "sent");
            return;
        };
        let Some(a) = self.live_account(account_id).cloned() else { return };
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let id = self.new_id();
        self.messages.push(Message {
            id,
            folder,
            from_name: if a.full_name.is_empty() { a.name.clone() } else { a.full_name.clone() },
            from_email: a.email.clone(),
            to: s("to"),
            cc: s("cc"),
            subject: s("subject"),
            body: s("body"),
            date: Local::now().timestamp(),
            read: true,
            source: a.source(),
            ..Default::default()
        });
    }

    // ------------------------------------------------------------ vCards

    /// vCard 3.0 text for the contacts in a group ("" = everyone).
    pub fn export_vcards(&self, group: &str) -> String {
        let esc = |s: &str| {
            s.replace('\\', "\\\\").replace('\n', "\\n").replace(',', "\\,").replace(';', "\\;")
        };
        let mut out = String::new();
        for c in self.contacts.iter().filter(|c| group.is_empty() || c.groups.iter().any(|g| g == group)) {
            out.push_str("BEGIN:VCARD\r\nVERSION:3.0\r\n");
            out.push_str(&format!("N:{};{};;;\r\n", esc(&c.last), esc(&c.first)));
            out.push_str(&format!("FN:{}\r\n", esc(&display_name(c))));
            if !c.company.is_empty() {
                out.push_str(&format!("ORG:{}\r\n", esc(&c.company)));
            }
            if !c.job_title.is_empty() {
                out.push_str(&format!("TITLE:{}\r\n", esc(&c.job_title)));
            }
            for e in &c.emails {
                out.push_str(&format!("EMAIL;TYPE={}:{}\r\n", esc(&e.label), esc(&e.value)));
            }
            for p in &c.phones {
                out.push_str(&format!("TEL;TYPE={}:{}\r\n", esc(&p.label), esc(&p.value)));
            }
            if !c.address.is_empty() {
                out.push_str(&format!("ADR;TYPE=home:;;{};;;;\r\n", esc(&c.address)));
            }
            if !c.birthday.is_empty() {
                out.push_str(&format!("BDAY:{}\r\n", c.birthday));
            }
            if !c.notes.is_empty() {
                out.push_str(&format!("NOTE:{}\r\n", esc(&c.notes)));
            }
            out.push_str("END:VCARD\r\n");
        }
        out
    }

    /// Adds every card in a .vcf file; returns how many were added.
    pub fn import_vcards(&mut self, text: &str, group: &str) -> usize {
        let mut n = 0;
        for chunk in text.split("BEGIN:VCARD").skip(1) {
            let v = crate::ics::parse_vcard(&format!("BEGIN:VCARD{chunk}"));
            if v.first.is_empty() && v.last.is_empty() && v.emails.is_empty() && v.org.is_empty() {
                continue;
            }
            let id = self.new_id();
            self.contacts.push(Contact {
                id,
                first: v.first,
                last: v.last,
                company: v.org,
                job_title: v.title,
                emails: v.emails.into_iter().map(|(label, value)| Labeled { label, value }).collect(),
                phones: v.phones.into_iter().map(|(label, value)| Labeled { label, value }).collect(),
                address: v.address,
                birthday: v.birthday,
                notes: v.notes,
                groups: if group.is_empty() || !self.groups.iter().any(|g| g.id == group) { vec![] } else { vec![group.into()] },
                source: SOURCE_LOCAL.into(),
                ..Default::default()
            });
            n += 1;
        }
        n
    }
}

/// Events (a series and its changed occurrences) from one calendar object.
pub fn events_from_resource(text: &str, calendar: &str, source: &str) -> Vec<Event> {
    let vevents = crate::ics::parse_vevents(text);
    let mut out: Vec<Event> = Vec::new();
    for e in &vevents {
        let rid = e.recurrence_id.map(fmt_dt).unwrap_or_default();
        out.push(Event {
            calendar: calendar.into(),
            title: if e.title.is_empty() { "(No title)".into() } else { e.title.clone() },
            start: fmt_dt(e.start.unwrap_or_default()),
            end: fmt_dt(e.end.unwrap_or_default()),
            all_day: e.all_day,
            location: e.location.clone(),
            notes: e.notes.clone(),
            rrule: if rid.is_empty() { e.rrule.clone() } else { String::new() },
            source: source.into(),
            tz: e.zone.clone(),
            exdates: if rid.is_empty() { e.exdates.clone() } else { vec![] },
            uid: e.uid.clone(),
            alarms: e.alarms.clone(),
            attendees: e.attendees.clone(),
            organizer: e.organizer.clone(),
            notes_html: e.notes_html.clone(),
            url: e.url.clone(),
            recurrence_id: rid,
            ..Default::default()
        });
    }
    // The series skips the dates that were changed on their own.
    let changed: Vec<(String, String)> = out.iter().filter(|e| !e.recurrence_id.is_empty()).map(|e| (e.uid.clone(), e.recurrence_id.clone())).collect();
    for (uid, rid) in changed {
        if let Some(m) = out.iter_mut().find(|m| m.uid == uid && m.recurrence_id.is_empty() && !m.rrule.is_empty()) {
            if !m.exdates.contains(&rid) {
                m.exdates.push(rid);
            }
        }
    }
    out
}

/// A contact from a vCard (its picture is saved in the app's folder).
pub fn contact_from_vcard(v: crate::ics::VCard, groups: Vec<String>, source: &str) -> Contact {
    let photo = if v.photo.is_empty() { String::new() } else { crate::ics::store_photo(&v.photo) };
    Contact {
        first: v.first,
        last: v.last,
        company: v.org,
        job_title: v.title,
        emails: v.emails.into_iter().map(|(label, value)| Labeled { label, value }).collect(),
        phones: v.phones.into_iter().map(|(label, value)| Labeled { label, value }).collect(),
        urls: v.urls.into_iter().map(|(label, value)| Labeled { label, value }).collect(),
        nickname: v.nickname,
        address: v.address,
        birthday: v.birthday,
        notes: v.notes,
        groups,
        source: source.into(),
        uid: v.uid,
        photo,
        ..Default::default()
    }
}

pub fn display_name(c: &Contact) -> String {
    let n = format!("{} {}", c.first, c.last).trim().to_string();
    if !n.is_empty() {
        n
    } else if !c.company.is_empty() {
        c.company.clone()
    } else {
        c.emails.first().map(|e| e.value.clone()).unwrap_or_else(|| "No Name".into())
    }
}

fn sort_key(c: &Contact) -> String {
    let k = if !c.last.is_empty() {
        format!("{} {}", c.last, c.first)
    } else {
        display_name(c)
    };
    lc(k.trim_start_matches(|ch: char| !ch.is_alphanumeric()))
}

pub fn kind_name(kind: &str) -> &'static str {
    match kind {
        "inbox" => "Inbox",
        "drafts" => "Drafts",
        "sent" => "Sent",
        "junk" => "Junk",
        "trash" => "Trash",
        "archive" => "Archive",
        _ => "Folder",
    }
}

pub fn kind_icon(kind: &str) -> &'static str {
    match kind {
        "inbox" => "inbox",
        "drafts" => "draft",
        "sent" => "send",
        "junk" => "junk",
        "trash" => "trash",
        "archive" => "archive",
        _ => "folder",
    }
}

/// Guess what kind of folder a name is.
pub fn guess_kind(name: &str) -> &'static str {
    let n = lc(name);
    let last = n.rsplit('/').next().unwrap_or(&n).trim().to_string();
    match last.as_str() {
        "inbox" => "inbox",
        "sent" | "sent mail" | "sent items" | "sent messages" => "sent",
        "drafts" | "draft" => "drafts",
        "trash" | "deleted" | "deleted items" | "deleted messages" | "bin" => "trash",
        "spam" | "junk" | "junk e-mail" | "junk email" | "bulk mail" => "junk",
        "archive" | "archives" | "all mail" => "archive",
        _ => "other",
    }
}


/// What's inside an editor page's <body> (its head, styles and page wrapper gone).
pub fn notes_body(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let Some(open) = lower.find("<body").and_then(|b| lower[b..].find('>').map(|x| b + x + 1)) else { return html.trim().to_string() };
    let close = lower.rfind("</body>").filter(|&c| c >= open).unwrap_or(html.len());
    html[open..close].trim().to_string()
}

/// Whether HTML has formatting beyond plain paragraphs and line breaks.
pub fn has_formatting(html: &str) -> bool {
    let lower = html.to_ascii_lowercase();
    ["<span", "<a ", "<b>", "<i>", "<u>", "<s>", "<ul", "<ol", "<table", "<h1", "<h2", "<h3", "<h4", "<blockquote", "<hr", "<img", "<pre", "<code", "align=", "margin-left:18px", "font-weight", "font-style", "text-decoration", "color:"]
        .iter()
        .any(|t| lower.contains(t))
}

#[cfg(test)]
mod conversation_tests {
    use super::*;

    fn msg(id: i64, subject: &str, from: &str, to: &str, mid: &str, irt: &str, refs: &str, date: i64) -> Message {
        Message { id, subject: subject.into(), from_email: from.into(), to: to.into(), message_id: mid.into(), in_reply_to: irt.into(), references: refs.into(), date, ..Default::default() }
    }

    #[test]
    fn groups_by_headers_not_just_subject() {
        let list = vec![
            // Two unrelated "Invoice" mails from different senders
            msg(1, "Invoice", "shop@a.com", "me@x.com", "<i1@a.com>", "", "", 10),
            msg(2, "Invoice", "bills@b.com", "me@x.com", "<i2@b.com>", "", "", 20),
            // A proper thread: reply and reply-to-reply
            msg(3, "Lunch?", "sam@x.com", "me@x.com", "<l1@x.com>", "", "", 30),
            msg(4, "Re: Lunch?", "me@x.com", "sam@x.com", "<l2@x.com>", "<l1@x.com>", "<l1@x.com>", 40),
            msg(5, "Re: Re: Lunch?", "sam@x.com", "me@x.com", "<l3@x.com>", "<l2@x.com>", "<l1@x.com> <l2@x.com>", 50),
            // A reply from a client that leaves out the headers: joins by subject and person
            msg(6, "Re: Lunch?", "sam@x.com", "Me <me@x.com>", "<l4@x.com>", "", "", 60),
            // Same subject, a "Re:", but nobody in common: stays apart
            msg(7, "Re: Lunch?", "pat@y.com", "kim@y.com", "<o1@y.com>", "", "", 70),
        ];
        let k = conversation_keys(&list);
        assert_ne!(k[&1], k[&2], "different invoices must not merge");
        assert_eq!(k[&3], k[&4]);
        assert_eq!(k[&4], k[&5]);
        assert_eq!(k[&3], k[&6]);
        assert_ne!(k[&3], k[&7]);
    }
}

#[cfg(test)]
mod older_mail_tests {
    use super::*;

    fn m(uid: u32, subject: &str) -> Message {
        Message { folder: "live:a:mail/INBOX".into(), source: "live:a".into(), uid, subject: subject.into(), date: uid as i64, ..Default::default() }
    }

    fn sync_of(uids: std::ops::RangeInclusive<u32>) -> Imported {
        Imported {
            source: "live:a".into(),
            accounts: vec![Account { id: "live:a:mail".into(), name: "A".into(), email: "a@x.com".into(), source: "live:a".into() }],
            folders: vec![Folder { id: "live:a:mail/INBOX".into(), account: "live:a:mail".into(), name: "Inbox".into(), kind: "inbox".into(), path: "INBOX".into() }],
            messages: uids.map(|u| m(u, "new")).collect(),
            server_truth: true,
            ..Default::default()
        }
    }

    #[test]
    fn sync_keeps_older_mail_and_load_older_skips_duplicates() {
        let mut db = Db::empty();
        db.apply_import(sync_of(6..=10));
        // "Load Older Messages" brings in 1..6; 6 is already here.
        let added = db.add_messages((1..=6).map(|u| m(u, "old")).collect());
        assert_eq!(added.len(), 5, "uid 6 was already here");
        let old_ids: Vec<i64> = db.messages.iter().filter(|x| x.uid <= 5).map(|x| x.id).collect();
        // The next regular sync reads only the newest window (now 7..11): older mail stays, with the same ids.
        db.apply_import(sync_of(7..=11));
        let mut uids: Vec<u32> = db.messages.iter().map(|x| x.uid).collect();
        uids.sort();
        // 6 is now just below the newest window, so it's kept too.
        assert_eq!(uids, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11], "older ones kept, no duplicates");
        let kept: Vec<i64> = db.messages.iter().filter(|x| x.uid <= 5).map(|x| x.id).collect();
        assert_eq!(kept.len(), old_ids.len());
        assert!(old_ids.iter().all(|id| kept.contains(id)));
    }
}

#[cfg(test)]
mod delta_tests {
    use super::*;

    fn delta_import(delta: MailDelta) -> Imported {
        Imported {
            source: "live:a".into(),
            accounts: vec![Account { id: "live:a:mail".into(), name: "A".into(), email: "a@x.com".into(), source: "live:a".into() }],
            folders: vec![Folder { id: "live:a:mail/INBOX".into(), account: "live:a:mail".into(), name: "Inbox".into(), kind: "inbox".into(), path: "INBOX".into() }],
            mail_delta: Some(delta),
            server_truth: true,
            ..Default::default()
        }
    }
    fn msg(uid: u32) -> Message {
        Message { folder: "live:a:mail/INBOX".into(), source: "live:a".into(), uid, subject: format!("m{uid}"), date: uid as i64, partial: true, ..Default::default() }
    }

    #[test]
    fn a_sync_begun_before_a_message_was_opened_does_not_mark_it_unread_again() {
        let mut db = Db::empty();
        db.apply_import(delta_import(MailDelta { new: (1..=2).map(msg).collect(), ..Default::default() }));
        let id1 = db.messages.iter().find(|m| m.uid == 1).unwrap().id;
        let unread = |uid: u32| vec![("live:a:mail/INBOX".to_string(), uid, false, false, NO_COLOR)];
        // A check starts, then the message is opened (marked read, sent to the server).
        let running = MailDelta { flags: [unread(1), unread(2)].concat(), ..Default::default() };
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert!(db.set_read(id1, true));
        let g = db.guard_flags(id1).unwrap();
        // The check finishes with the flags from before: the message stays read.
        db.apply_import(delta_import(running.clone()));
        assert!(db.get(id1).unwrap().read, "still read while the change is on its way");
        // The server takes it; a check that began before that still can't undo it.
        db.flag_pushed(g);
        db.apply_import(delta_import(running));
        assert!(db.get(id1).unwrap().read, "still read: that check began before the server had it");
        // A check begun after the server took it is the truth again (here:
        // marked unread on another device).
        std::thread::sleep(std::time::Duration::from_millis(5));
        db.apply_import(delta_import(MailDelta { flags: unread(1), ..Default::default() }));
        assert!(!db.get(id1).unwrap().read, "later server changes still come through");
        assert!(db.flag_guards.is_empty());
    }

    #[test]
    fn syncing_changes_keeps_what_is_here() {
        let mut db = Db::empty();
        let first = db.apply_import(delta_import(MailDelta { new: (1..=3).map(msg).collect(), ..Default::default() }));
        assert!(first.is_empty(), "the very first sync doesn't count as new mail");
        assert_eq!(db.messages.iter().filter(|m| m.source == "live:a").count(), 3);
        // Text downloaded and a reminder set in the meantime.
        for m in db.messages.iter_mut().filter(|m| m.uid == 2) {
            m.body = "downloaded".into();
            m.partial = false;
            m.remind_at = 99;
        }
        let id2 = db.messages.iter().find(|m| m.uid == 2).unwrap().id;
        // Next sync: 1 deleted on the server, 2 read and flagged elsewhere, 4 is new.
        let new = db.apply_import(delta_import(MailDelta {
            new: vec![msg(4)],
            flags: vec![("live:a:mail/INBOX".into(), 2, true, true, 4), ("live:a:mail/INBOX".into(), 3, false, false, NO_COLOR)],
            gone: vec![("live:a:mail/INBOX".into(), 1)],
            marks: vec![("live:a:mail/INBOX".into(), FolderMark { validity: 1, uid_next: 5, exists: 3, modseq: 77 })],
            ..Default::default()
        }));
        assert_eq!(new.len(), 1, "only the new message counts as new");
        assert_eq!(db.folder_marks.get("live:a:mail/INBOX").map(|m| m.modseq), Some(77), "the mailbox's counters are kept");
        let mut uids: Vec<u32> = db.messages.iter().filter(|m| m.source == "live:a").map(|m| m.uid).collect();
        uids.sort();
        assert_eq!(uids, vec![2, 3, 4]);
        let m2 = db.messages.iter().find(|m| m.uid == 2).unwrap();
        assert_eq!(m2.id, id2, "same message, same id");
        assert!(m2.read && m2.flagged);
        assert_eq!(m2.body, "downloaded");
        assert_eq!(m2.remind_at, 99);
        assert_eq!(db.known_uids("live:a")["live:a:mail/INBOX"].len(), 3);
    }
}

#[cfg(test)]
mod page_tests {
    use super::*;

    fn big(n: u32) -> Db {
        let mut db = Db::empty();
        db.apply_import(Imported {
            source: "live:a".into(),
            accounts: vec![Account { id: "live:a:mail".into(), name: "A".into(), email: "a@x.com".into(), source: "live:a".into() }],
            folders: vec![Folder { id: "live:a:mail/INBOX".into(), account: "live:a:mail".into(), name: "Inbox".into(), kind: "inbox".into(), path: "INBOX".into() }],
            server_truth: true,
            ..Default::default()
        });
        let msgs = (1..=n)
            .map(|u| Message {
                folder: "live:a:mail/INBOX".into(),
                source: "live:a".into(),
                uid: u,
                subject: format!("Message {u}"),
                message_id: format!("<{u}@x>"),
                date: 1_136_073_600 + u as i64 * 3600, // from 2006 on
                read: u % 7 != 0,
                ..Default::default()
            })
            .collect();
        db.add_messages(msgs);
        db
    }

    #[test]
    fn pages_reach_back_to_the_oldest_and_stay_quick() {
        let db = big(100_000);
        let started = std::time::Instant::now();
        let page = db.message_page("kind:inbox", "", true, "date", false, false, -1, 300);
        let took = started.elapsed();
        assert_eq!(page["total"], 100_000);
        assert_eq!(page["rows"].as_array().unwrap().len(), 300);
        assert_eq!(page["rows"][0]["subject"], "Message 100000", "newest first");
        // All of it (End): the last row is the oldest, from 2006.
        let all = db.message_page("kind:inbox", "", true, "date", false, false, -1, 100_000);
        assert_eq!(all["rows"][99_999]["subject"], "Message 1");
        assert!(took.as_millis() < 2000, "a page took {took:?}");
    }

    #[test]
    fn the_open_message_is_always_in_the_page() {
        let db = big(1000);
        let oldest = db.messages.iter().find(|m| m.uid == 1).unwrap().id;
        let page = db.message_page("kind:inbox", "", false, "date", false, false, oldest, 50);
        let rows = page["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 1000, "extends down to the open message");
        assert_eq!(rows.last().unwrap()["id"], oldest);
    }

    #[test]
    fn unread_only_and_sorting() {
        let db = big(70);
        let page = db.message_page("kind:inbox", "", false, "date", false, true, -1, 300);
        assert_eq!(page["total"], 10, "uids 7, 14, … 70 are unread");
        assert_eq!(page["unread"], 10);
        assert_eq!(page["all"], 70);
        let up = db.message_page("kind:inbox", "", false, "date", true, false, -1, 1);
        assert_eq!(up["rows"][0]["subject"], "Message 1", "oldest first");
    }
}

#[cfg(test)]
mod suggestion_tests {
    use super::*;

    #[test]
    fn addresses_split_with_names_and_quotes() {
        let v = split_addresses("\"Lee, Ann\" <ann@x.com>, bob@y.org; Cy <cy@z.net>");
        assert_eq!(v, vec![("Lee, Ann".into(), "ann@x.com".into()), ("".into(), "bob@y.org".into()), ("Cy".into(), "cy@z.net".into())]);
    }

    #[test]
    fn suggestions_come_from_the_whole_mail_history() {
        let mut db = Db::empty();
        db.apply_import(Imported {
            source: "live:a".into(),
            accounts: vec![Account { id: "live:a:mail".into(), name: "A".into(), email: "me@x.com".into(), source: "live:a".into() }],
            folders: vec![
                Folder { id: "f/in".into(), account: "live:a:mail".into(), name: "Inbox".into(), kind: "inbox".into(), path: "INBOX".into() },
                Folder { id: "f/sent".into(), account: "live:a:mail".into(), name: "Sent".into(), kind: "sent".into(), path: "Sent".into() },
            ],
            server_truth: true,
            ..Default::default()
        });
        let m = |folder: &str, uid: u32, from: (&str, &str), to: &str, cc: &str, date: i64| Message {
            folder: folder.into(), source: "live:a".into(), uid, from_name: from.0.into(), from_email: from.1.into(), to: to.into(), cc: cc.into(), date, ..Default::default()
        };
        let now = chrono::Utc::now().timestamp();
        db.add_messages(vec![
            m("f/in", 1, ("Jeffrey Kumagai", "jeff@band.org"), "me@x.com", "", now - 86_400 * 4000),
            m("f/sent", 2, ("Me", "me@x.com"), "Jenny Park <jenny@school.edu>", "", now - 86_400),
            m("f/sent", 3, ("Me", "me@x.com"), "jenny@school.edu", "Jeffrey Kumagai <jeff@band.org>", now - 86_400 * 2),
            m("f/in", 4, ("Newsletter", "news@shop.com"), "me@x.com", "", now),
        ]);
        let v = db.email_suggestions("je");
        let emails: Vec<&str> = v.as_array().unwrap().iter().map(|x| x["email"].as_str().unwrap()).collect();
        assert_eq!(emails, vec!["jenny@school.edu", "jeff@band.org"], "people written to most first");
        assert_eq!(db.email_suggestions("kuma")[0]["name"], "Jeffrey Kumagai", "matches a word of the name");
        let mine = db.email_suggestions("me@");
        let mine: Vec<&str> = mine.as_array().unwrap().iter().map(|x| x["email"].as_str().unwrap()).collect();
        assert_eq!(mine, vec!["me@x.com"], "my own address too");
    }

    #[test]
    fn my_own_address_doesnt_crowd_out_others() {
        let mut db = Db::empty();
        db.apply_import(Imported {
            source: "live:a".into(),
            accounts: vec![Account { id: "live:a:mail".into(), name: "Gmail".into(), email: "ben@x.com".into(), source: "live:a".into() }],
            folders: vec![Folder { id: "f/in".into(), account: "live:a:mail".into(), name: "Inbox".into(), kind: "inbox".into(), path: "INBOX".into() }],
            server_truth: true,
            ..Default::default()
        });
        let now = chrono::Utc::now().timestamp();
        db.add_messages(vec![
            Message { folder: "f/in".into(), source: "live:a".into(), uid: 1, from_name: "Beth".into(), from_email: "beth@y.org".into(), to: "ben@x.com".into(), date: now - 86_400 * 30, ..Default::default() },
            Message { folder: "f/in".into(), source: "live:a".into(), uid: 2, from_name: "Shop".into(), from_email: "shop@z.com".into(), to: "ben@x.com".into(), date: now, ..Default::default() },
        ]);
        let v = db.email_suggestions("be");
        let emails: Vec<&str> = v.as_array().unwrap().iter().map(|x| x["email"].as_str().unwrap()).collect();
        assert_eq!(emails, vec!["beth@y.org", "ben@x.com"]);
    }
}

#[cfg(test)]
mod speed_tests {
    use super::*;
    use std::time::Instant;

    /// A mailbox shaped like a big real one: 165,000 messages in 60 folders,
    /// with text, most of it threaded by subject.
    pub fn big_account() -> Db {
        let mut db = Db::empty();
        let folders: Vec<Folder> = (0..60)
            .map(|i| Folder {
                id: format!("live:a:mail/F{i}"),
                account: "live:a:mail".into(),
                name: format!("Folder {i}"),
                kind: match i { 0 => "inbox", 1 => "sent", 2 => "archive", 3 => "trash", 4 => "junk", _ => "other" }.into(),
                path: format!("F{i}"),
            })
            .collect();
        db.apply_import(Imported {
            source: "live:a".into(),
            accounts: vec![Account { id: "live:a:mail".into(), name: "A".into(), email: "me@x.com".into(), source: "live:a".into() }],
            folders,
            server_truth: true,
            ..Default::default()
        });
        let body = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(40);
        let html = format!("<html><body><table><tr><td>{}</td></tr></table></body></html>", body.repeat(4));
        let msgs = (1..=165_000u32)
            .map(|u| Message {
                folder: format!("live:a:mail/F{}", if u % 2 == 0 { 0 } else { u % 60 }),
                source: "live:a".into(),
                uid: u,
                subject: format!("Topic {}", u / 4),
                from_name: format!("Person {}", u % 900),
                from_email: format!("p{}@example.com", u % 900),
                to: "Me <me@x.com>".into(),
                message_id: format!("<{u}@x>"),
                date: 1_100_000_000 + u as i64 * 3600,
                read: u % 9 != 0,
                body: body.clone(),
                html: html.clone(),
                ..Default::default()
            })
            .collect();
        db.add_messages(msgs);
        db
    }

    #[test]
    #[ignore] // a measurement: cargo test --release speed -- --ignored --nocapture
    fn what_a_click_costs() {
        let mut db = big_account();
        let t = |label: &str, f: &mut dyn FnMut()| {
            let s = Instant::now();
            f();
            eprintln!("{label:32} {:>8.1} ms", s.elapsed().as_secs_f64() * 1000.0);
        };
        let mut saved = std::collections::HashMap::new();
        let path = crate::mailstore::db_path();
        let _ = std::fs::remove_file(&path);
        crate::mailstore::save_for_test(&path, &db.messages, &mut saved);
        db.forget_saved_text();
        crate::mailstore::save_for_test(&path, &db.messages, &mut saved);
        let id = db.messages[1000].id;
        t("mark read", &mut || { db.set_read(id, true); });
        t("save changes (sqlite)", &mut || crate::mailstore::save_for_test(&path, &db.messages, &mut saved));
        t("data.json", &mut || { let _ = serde_json::to_string(&db); });
        t("message list page", &mut || { db.message_page("kind:inbox", "", true, "date", false, false, id, 300); });
        t("message list page again", &mut || { db.message_page("kind:inbox", "", true, "date", false, false, id, 300); });
        t("open conversation", &mut || { db.thread("kind:inbox", id); });
        t("sidebar counts", &mut || { db.mail_sidebar(); });
        t("folder list", &mut || { db.mail_folders(); });
        let _ = std::fs::remove_file(&path);
    }
}

#[cfg(test)]
mod real_copy_tests {
    use super::*;

    /// Looks at one message of a COPY of a real mailbox:
    /// ALMANAC_COPY=/dir/with/mail.sqlite+data.json ALMANAC_ID=123 cargo test real_copy -- --ignored --nocapture
    #[test]
    #[ignore]
    fn open_one_message_from_a_copy() {
        let Ok(from) = std::env::var("ALMANAC_COPY") else { return };
        let id: i64 = std::env::var("ALMANAC_ID").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
        let dir = data_dir();
        let _ = std::fs::create_dir_all(&dir);
        for f in ["mail.sqlite", "data.json"] {
            std::fs::copy(std::path::Path::new(&from).join(f), dir.join(f)).unwrap();
        }
        let t = std::time::Instant::now();
        let db = Db::load();
        eprintln!("loaded {} messages in {:?}", db.messages.len(), t.elapsed());
        let m = db.messages.iter().find(|m| m.id == id).expect("no such message");
        eprintln!("folder {} subject {:?} partial {} preview {:?}", m.folder, m.subject, m.partial, &m.preview.chars().take(60).collect::<String>());
        for sel in ["kind:inbox", &format!("folder:{}", m.folder)] {
            let t = std::time::Instant::now();
            let th = db.thread(sel, id);
            let list = th.as_array().unwrap();
            eprintln!("thread in {sel}: {} messages in {:?}", list.len(), t.elapsed());
            for x in list.iter().take(5) {
                eprintln!("  id {} html {} body {} partial {}", x["id"], x["html"].as_str().map(|s| s.len()).unwrap_or(0), x["body"].as_str().map(|s| s.len()).unwrap_or(0), x["partial"]);
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// What every change costs the window's thread on a COPY of a real mailbox:
    /// ALMANAC_COPY=/dir/with/mail.sqlite+data.json cargo test change_costs -- --ignored --nocapture
    #[test]
    #[ignore]
    fn change_costs_on_a_copy() {
        let Ok(from) = std::env::var("ALMANAC_COPY") else { return };
        let dir = data_dir();
        let _ = std::fs::create_dir_all(&dir);
        for f in ["mail.sqlite", "data.json"] {
            std::fs::copy(std::path::Path::new(&from).join(f), dir.join(f)).unwrap();
        }
        let t = std::time::Instant::now();
        let _ = Db::load();
        eprintln!("first start (with any one-time upgrade): {:?}", t.elapsed());
        let t = std::time::Instant::now();
        let mut db = Db::load();
        eprintln!("loaded {} messages in {:?}", db.messages.len(), t.elapsed());
        let t = std::time::Instant::now();
        let _ = crate::mailstore::load();
        eprintln!("  of which the message list: {:?}", t.elapsed());
        let time = |what: &str, f: &mut dyn FnMut()| {
            let t = std::time::Instant::now();
            f();
            eprintln!("{what}: {:?}", t.elapsed());
        };
        let id = db.messages.iter().max_by_key(|m| m.date).map(|m| m.id).unwrap_or(0);
        time("mail_sidebar", &mut || { let _ = db.mail_sidebar(); });
        time("mail_folders", &mut || { let _ = db.mail_folders(); });
        time("message_page inbox", &mut || { let _ = db.message_page("kind:inbox", "", true, "date", false, false, id, 200); });
        db.generation += 1;
        time("message_page inbox after a change", &mut || { let _ = db.message_page("kind:inbox", "", true, "date", false, false, id, 200); });
        time("thread", &mut || { let _ = db.thread("kind:inbox", id); });
        time("save (first)", &mut || { db.save(); });
        time("save (nothing changed)", &mut || { db.save(); });
        time("data.json text", &mut || { let _ = serde_json::to_string(&db).unwrap(); });
        time("known_uids", &mut || { let _ = db.known_uids(&db.live.first().map(|a| format!("live:{}", a.id)).unwrap_or_default()); });
        // A regular check that found nothing new: every message's read / flagged state.
        if let Some(acct) = db.live.first().cloned() {
            let src = format!("live:{}", acct.id);
            let imp = Imported {
                source: src.clone(),
                accounts: db.accounts.iter().filter(|a| a.source == src).cloned().collect(),
                folders: db.folders.iter().filter(|f| f.account.starts_with(&src)).cloned().collect(),
                server_truth: true,
                failed: vec!["calendars".into(), "contacts".into()],
                mail_delta: Some(MailDelta {
                    flags: db.messages.iter().filter(|m| m.source == src && m.uid > 0).map(|m| (m.folder.clone(), m.uid, m.read, m.flagged, m.flag_color)).collect(),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let mut once = Some(imp);
            time("apply_import (a check with nothing new)", &mut || { db.apply_import(once.take().unwrap()); });
            time("partial_messages", &mut || { let _ = db.partial_messages(&src); });
            time("save after the check", &mut || { db.save(); });
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod recency_tests {
    use super::*;

    #[test]
    fn most_recently_written_to_comes_first() {
        let mut db = Db::empty();
        db.folders.push(Folder { id: "local/sent".into(), account: "local".into(), name: "Sent".into(), kind: "sent".into(), path: String::new() });
        db.folders.push(Folder { id: "local/inbox".into(), account: "local".into(), name: "Inbox".into(), kind: "inbox".into(), path: String::new() });
        let msg = |id: i64, folder: &str, from: &str, to: &str, date: i64| Message { id, folder: folder.into(), from_email: from.into(), to: to.into(), date, ..Default::default() };
        db.messages = vec![
            msg(1, "local/sent", "me@x.com", "Sam Old <sam.old@x.com>", 100),
            msg(2, "local/sent", "me@x.com", "Sam New <sam.new@x.com>", 300),
            // Wrote to me lately, but I haven't written to them.
            msg(3, "local/inbox", "sam.reader@x.com", "me@x.com", 500),
        ];
        db.contacts.push(Contact { first: "Sam".into(), last: "Book".into(), emails: vec![Labeled { label: "home".into(), value: "sam.book@x.com".into() }], ..Default::default() });
        let got: Vec<String> = db.email_suggestions("sam").as_array().unwrap().iter().map(|x| x["email"].as_str().unwrap().to_string()).collect();
        assert_eq!(got, vec!["sam.new@x.com", "sam.old@x.com", "sam.reader@x.com", "sam.book@x.com"]);
    }
}

#[cfg(test)]
mod late_mail_tests {
    use super::*;

    fn sync(labels: &[&str]) -> Imported {
        Imported {
            source: "live:a".into(),
            accounts: vec![Account { id: "live:a:mail".into(), name: "A".into(), email: "a@x.com".into(), source: "live:a".into() }],
            folders: labels.iter().map(|l| Folder { id: format!("live:a:mail/{l}"), account: "live:a:mail".into(), name: l.to_string(), kind: "other".into(), path: l.to_string() }).collect(),
            server_truth: true,
            mail_delta: Some(MailDelta::default()),
            ..Default::default()
        }
    }

    #[test]
    fn a_deleted_label_stays_deleted() {
        let mut db = Db::empty();
        db.apply_import(sync(&["INBOX", "Old Label"]));
        // The label is deleted on the server; the next regular check removes it…
        db.apply_import(sync(&["INBOX"]));
        assert!(!db.folders.iter().any(|f| f.name == "Old Label"));
        // …and the long background pass, which started before, doesn't bring it back.
        let mut late = sync(&["INBOX", "Old Label"]);
        late.failed = vec!["calendars".into(), "contacts".into()];
        late.mail_delta = Some(MailDelta {
            new: vec![Message { folder: "live:a:mail/Old Label".into(), uid: 7, subject: "old".into(), ..Default::default() },
                      Message { folder: "live:a:mail/INBOX".into(), uid: 8, subject: "kept".into(), ..Default::default() }],
            ..Default::default()
        });
        db.apply_late_mail("live:a", late);
        assert!(!db.folders.iter().any(|f| f.name == "Old Label"), "folder stays gone");
        let subjects: Vec<&str> = db.messages.iter().map(|m| m.subject.as_str()).collect();
        assert_eq!(subjects, vec!["kept"], "only mail for folders still here");
    }

    #[test]
    fn a_deleted_label_goes_as_soon_as_the_list_is_read() {
        let mut db = Db::empty();
        let mut first = sync(&["INBOX", "Old Label"]);
        first.mail_delta = Some(MailDelta {
            new: vec![Message { folder: "live:a:mail/Old Label".into(), uid: 7, subject: "old".into(), ..Default::default() },
                      Message { folder: "live:a:mail/INBOX".into(), uid: 8, subject: "kept".into(), ..Default::default() }],
            ..Default::default()
        });
        db.apply_import(first);
        assert_eq!(db.folders.iter().filter(|f| f.account == "live:a:mail").count(), 2);
        // The next check's list (read first, in one go) no longer has it.
        let mut next = sync(&["INBOX"]);
        next.mail_delta = Some(MailDelta::default());
        db.apply_import(next);
        assert!(!db.folders.iter().any(|f| f.name == "Old Label"));
        assert!(db.folders.iter().any(|f| f.id == "live:a:mail/INBOX"));
        let subjects: Vec<&str> = db.messages.iter().map(|m| m.subject.as_str()).collect();
        assert_eq!(subjects, vec!["kept"]);
    }
}

#[cfg(test)]
mod backed_tests {
    use super::*;

    /// The tests here share one database file: one at a time.
    static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// A message waiting for its text in one Gmail label is filled in from its
    /// copy already downloaded in another (All Mail), not fetched again.
    #[test]
    fn copies_in_other_folders_are_filled_not_downloaded() {
        let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let _ = std::fs::create_dir_all(data_dir());
        let src = "live:dup";
        crate::mailstore::remove(src, crate::mailstore::Gone::All);
        let mut db = Db { backed: true, ..Default::default() };
        db.accounts.push(Account { id: "live:dup:mail".into(), name: "Dup".into(), email: "me@dup.example".into(), source: src.into() });
        for (id, kind, path) in [("live:dup:mail/[Gmail]/All Mail", "archive", "[Gmail]/All Mail"), ("live:dup:mail/[Gmail]/Important", "other", "[Gmail]/Important")] {
            db.folders.push(Folder { id: id.into(), account: "live:dup:mail".into(), name: path.into(), kind: kind.into(), path: path.into() });
        }
        db.next_id = 800_000;
        let (a, b) = (db.new_id(), db.new_id());
        db.messages.push(Message { id: a, folder: "live:dup:mail/[Gmail]/All Mail".into(), source: src.into(), uid: 1, date: 1_760_000_000,
            subject: "Receipt".into(), body: "Thanks for your order.".into(), message_id: "<order-1@shop.example>".into(), ..Default::default() });
        db.messages.push(Message { id: b, folder: "live:dup:mail/[Gmail]/Important".into(), source: src.into(), uid: 9, date: 1_760_000_000,
            subject: "Receipt".into(), partial: true, message_id: "<order-1@shop.example>".into(), ..Default::default() });
        assert!(db.save());
        db.release_saved();
        // (On the sync's own thread, as the app does it: it can take a while.)
        let wanted = partial_wanted(src, &db.folders);
        assert!(wanted.is_empty(), "the copy is filled from All Mail, not downloaded: {wanted:?}");
        assert_eq!(db.thread(&format!("folder:live:dup:mail/[Gmail]/Important"), b)[0]["body"], "Thanks for your order.");
        assert!(crate::mailstore::search("order").is_some_and(|hits| hits.contains(&b)), "searchable too");
        crate::mailstore::remove(src, crate::mailstore::Gone::All);
    }

    /// A message moved by Almanac (deleted to Trash) loses its old folder's
    /// server number, so nothing done to it later lands on another message
    /// with that number; when the server shows it in Trash, that row takes its place.
    #[test]
    fn a_moved_message_never_keeps_its_old_number() {
        let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let _ = std::fs::create_dir_all(data_dir());
        let src = "live:mv";
        crate::mailstore::remove(src, crate::mailstore::Gone::All);
        let mut db = Db { backed: true, ..Default::default() };
        db.accounts.push(Account { id: "live:mv:mail".into(), name: "Mv".into(), email: "me@mv.example".into(), source: src.into() });
        for (id, kind, path) in [("live:mv:mail/INBOX", "inbox", "INBOX"), ("live:mv:mail/[Gmail]/Trash", "trash", "[Gmail]/Trash")] {
            db.folders.push(Folder { id: id.into(), account: "live:mv:mail".into(), name: path.into(), kind: kind.into(), path: path.into() });
        }
        db.next_id = 810_000;
        let a = db.new_id();
        db.messages.push(Message { id: a, folder: "live:mv:mail/INBOX".into(), source: src.into(), uid: 79375, date: 1_760_000_000,
            subject: "Halloween Updates".into(), message_id: "<h@school.example>".into(), ..Default::default() });
        assert!(db.save());
        db.delete_message(a);
        let m = db.get(a).unwrap();
        assert_eq!((m.folder.as_str(), m.uid), ("live:mv:mail/[Gmail]/Trash", 0), "INBOX's number isn't Trash's");
        assert!(db.save());
        db.release_saved();
        // The next check finds it in Trash under its own number.
        let found = Message { folder: "live:mv:mail/[Gmail]/Trash".into(), source: src.into(), uid: 32291, date: 1_760_000_000,
            subject: "Halloween Updates".into(), message_id: "<h@school.example>".into(), ..Default::default() };
        let new = db.add_messages(vec![found]);
        assert_eq!(new.len(), 1);
        assert!(db.save());
        db.release_saved();
        let rows = db.messages_of("folder:live:mv:mail/[Gmail]/Trash");
        assert_eq!(rows.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![32291], "one row, with Trash's number: {rows:?}");
        crate::mailstore::remove(src, crate::mailstore::Gone::All);
    }

    /// A conversation whose first message isn't here (GitHub notifications all
    /// point at a pull request's opening message): opening the newest shows all.
    #[test]
    fn a_thread_without_its_first_message_shows_whole() {
        let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let _ = std::fs::create_dir_all(data_dir());
        let src = "live:gh";
        let inbox = "live:gh:mail/INBOX";
        crate::mailstore::remove(src, crate::mailstore::Gone::All);
        let mut db = Db { backed: true, ..Default::default() };
        db.accounts.push(Account { id: "live:gh:mail".into(), name: "GH".into(), email: "me@gh.example".into(), source: src.into() });
        db.folders.push(Folder { id: inbox.into(), account: "live:gh:mail".into(), name: "Inbox".into(), kind: "inbox".into(), path: "INBOX".into() });
        db.next_id = 700_000;
        let mut newest = 0;
        for n in 1..=6u32 {
            let id = db.new_id();
            newest = id;
            db.messages.push(Message {
                id,
                folder: inbox.into(),
                source: src.into(),
                uid: n,
                date: 1_750_000_000 + n as i64 * 600,
                from_name: format!("Reviewer {n}"),
                from_email: "notifications@github.example".into(),
                subject: "Re: [org/repo] Fix the thing (PR #1)".into(),
                body: format!("Comment {n}"),
                message_id: format!("<org/repo/pull/1/c{n}@github.example>"),
                in_reply_to: "<org/repo/pull/1@github.example>".into(),
                references: "<org/repo/pull/1@github.example>".into(),
                ..Default::default()
            });
        }
        assert!(db.save());
        db.release_saved();
        let thread = db.thread(&format!("folder:{inbox}"), newest);
        assert_eq!(thread.as_array().unwrap().len(), 6, "every reply, not just the newest");
        crate::mailstore::remove(src, crate::mailstore::Gone::All);
    }

    /// Mail in the database, read as needed: lists, conversations, counts,
    /// changes (saved and not yet), changes from the server, search.
    #[test]
    fn mail_read_from_the_database_as_needed() {
        let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let _ = std::fs::create_dir_all(data_dir());
        let src = "live:bt";
        let inbox = "live:bt:mail/INBOX";
        let trash = "live:bt:mail/Trash";
        crate::mailstore::remove(src, crate::mailstore::Gone::All);
        let mut db = Db { backed: true, ..Default::default() };
        db.accounts.push(Account { id: "live:bt:mail".into(), name: "BT".into(), email: "me@bt.example".into(), source: src.into() });
        for (id, kind) in [(inbox, "inbox"), (trash, "trash")] {
            db.folders.push(Folder { id: id.into(), account: "live:bt:mail".into(), name: kind.into(), kind: kind.into(), path: kind.into() });
        }
        db.next_id = 500_000;
        for uid in 1..=30u32 {
            let id = db.new_id();
            db.messages.push(Message {
                id,
                folder: inbox.into(),
                source: src.into(),
                uid,
                date: 1_700_000_000 + uid as i64 * 60,
                from_name: format!("Sender {uid}"),
                from_email: format!("s{uid}@example.org"),
                subject: format!("Subject {uid}"),
                body: format!("Body of message {uid}"),
                read: uid % 3 != 0,
                flagged: uid == 5,
                flag_color: if uid == 5 { 4 } else { 0 },
                message_id: format!("<m{uid}@example.org>"),
                in_reply_to: if uid == 20 { "<m10@example.org>".into() } else { String::new() },
                ..Default::default()
            });
        }
        let id_of = |db: &Db, uid: u32| crate::mailstore::id_at(src, inbox, uid).or_else(|| db.messages.iter().find(|m| m.uid == uid).map(|m| m.id)).unwrap();
        assert!(db.save());
        // Read while its text is still in memory (saved whole, replacing the row):
        // the counts see it too. (Read back unread after.)
        let six = id_of(&db, 6);
        db.set_read(six, true);
        assert!(db.save());
        assert_eq!(db.mail_sidebar()["accounts"][0]["folders"][0]["badge"], 9, "replaced rows counted right");
        db.set_read(six, false);
        assert!(db.save());
        db.release_saved();
        assert!(db.messages.is_empty(), "nothing kept in memory after saving");

        // A page of the list, newest first, with how many there are.
        let sel = format!("folder:{inbox}");
        let page = db.message_page(&sel, "", false, "date", false, false, 0, 10);
        let rows = page["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 10);
        assert_eq!(rows[0]["subject"], "Subject 30");
        assert_eq!(page["all"], 30);
        assert_eq!(page["total"], 30, "the whole list's length, not just past this page");
        assert_eq!(page["unread"], 10);
        let flagged = db.message_page(&sel, "", false, "date", false, false, 0, 50);
        let five = flagged["rows"].as_array().unwrap().iter().find(|r| r["subject"] == "Subject 5").unwrap().clone();
        assert_eq!(five["flagColor"], 4, "flag colour kept");

        // Conversations: the reply grouped with what it answers; the thread opened.
        let first = db.message_page(&sel, "", true, "date", false, false, 0, 10);
        assert_eq!(first["total"], 29, "30 messages, one reply with what it answers: 29 conversations");
        let threaded = db.message_page(&sel, "", true, "date", false, false, 0, 50);
        let twenty = threaded["rows"].as_array().unwrap().iter().find(|r| r["subject"] == "Subject 20").unwrap().clone();
        assert_eq!(twenty["count"], 2);
        let thread = db.thread(&sel, id_of(&db, 20));
        let subjects: Vec<&str> = thread.as_array().unwrap().iter().map(|m| m["subject"].as_str().unwrap()).collect();
        assert_eq!(subjects, vec!["Subject 10", "Subject 20"]);

        // Counts: right straight after a change (before it's saved), and after.
        let badge = |db: &Db| {
            let s = db.mail_sidebar();
            s["accounts"][0]["folders"].as_array().unwrap().iter().find(|f| f["id"] == format!("folder:{inbox}")).unwrap()["badge"].as_i64().unwrap()
        };
        assert_eq!(badge(&db), 10);
        assert!(db.set_read(id_of(&db, 3), true));
        assert_eq!(db.messages.len(), 1, "just that one read into memory to change");
        assert_eq!(badge(&db), 9);
        assert!(db.save());
        db.release_saved();
        assert_eq!(badge(&db), 9);

        // Changes from the server for messages not in memory.
        let imp = Imported {
            source: src.into(),
            mail_delta: Some(MailDelta {
                flags: vec![(inbox.into(), 1, false, true, 2)],
                gone: vec![(inbox.into(), 4)],
                new: vec![Message { folder: inbox.into(), uid: 31, date: 1_700_009_999, subject: "Subject 31".into(), read: false, ..Default::default() }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let new = db.apply_late_mail(src, imp);
        assert_eq!(new.len(), 1);
        assert!(db.get(id_of(&db, 1)).is_some_and(|m| !m.read && m.flagged && m.flag_color == 2));
        assert!(crate::mailstore::id_at(src, inbox, 4).is_none(), "gone from the database");
        assert!(db.save());
        db.release_saved();
        let known = db.known_uids(src);
        assert!(known[inbox].contains(&31) && !known[inbox].contains(&4));
        let page = db.message_page(&sel, "", false, "date", false, false, 0, 5);
        assert_eq!(page["rows"][0]["subject"], "Subject 31");

        // Search: subject and text.
        let found = db.message_page(&sel, "Subject 17", false, "date", false, false, 0, 50);
        assert!(found["rows"].as_array().unwrap().iter().any(|r| r["subject"] == "Subject 17"));
        let found = db.message_page(&sel, "message 18", false, "date", false, false, 0, 50);
        assert!(found["rows"].as_array().unwrap().iter().any(|r| r["subject"] == "Subject 18"), "found by its text");

        // Moving to the Trash, and marking a whole mailbox read.
        let two = id_of(&db, 2);
        db.delete_message(two);
        assert!(db.save());
        db.release_saved();
        // (In Trash, its INBOX number is gone: unknown until the server says.)
        assert_eq!(db.get(two).map(|m| (m.folder.clone(), m.uid)), Some((trash.to_string(), 0)));
        assert!(crate::mailstore::id_at(src, trash, 2).is_none());
        db.mark_all_read(&sel);
        assert!(db.save());
        db.release_saved();
        assert_eq!(badge(&db), 0);
        crate::mailstore::remove(src, crate::mailstore::Gone::All);
    }
}

#[cfg(test)]
mod window_timings {
    /// On a copy of real mail (XDG_DATA_HOME pointing at it): how long reading
    /// the list a page at a time takes, top, middle and bottom.
    /// XDG_DATA_HOME=<copy> cargo test window_timings -- --ignored --nocapture
    #[test]
    #[ignore]
    fn window_timings() {
        if std::env::var_os("ALMANAC_WINDOW_TEST").is_none() {
            return;
        }
        let t = std::time::Instant::now();
        let mut db = super::Db::load();
        db.backed = true;
        eprintln!("load (with the one-time grouping): {:?} backed={} folders={}", t.elapsed(), db.backed, db.folders.len());
        let cases: [(&str, &str, &str, bool); 6] = [
            ("kind:inbox", "", "date", false),
            ("kind:inbox", "invoice", "date", false),
            ("kind:inbox", "", "subject", false),
            ("kind:inbox", "", "date", true),
            ("kind:flagged", "", "date", false),
            ("kind:vip", "", "date", false),
        ];
        for (sel, q, sort, asc) in cases {
            for threaded in [true, false] {
                let t = std::time::Instant::now();
                let Some(first) = db.list_window(sel, q, threaded, sort, asc, false, 0, 0, 100) else {
                    eprintln!("{sel} q={q:?} sort={sort} asc={asc} threaded={threaded}: not paged");
                    continue;
                };
                let total = first["total"].as_i64().unwrap();
                let first_t = t.elapsed();
                let at = (total / 2).max(0) as usize;
                let t = std::time::Instant::now();
                let w = db.list_window(sel, q, threaded, sort, asc, false, 0, at, 100).unwrap();
                let mid_t = t.elapsed();
                let rows = w["rows"].as_array().unwrap();
                let (pos, pos_t) = match rows.first().and_then(|r| r["id"].as_i64()) {
                    Some(id) => {
                        let t = std::time::Instant::now();
                        (db.list_position(sel, q, threaded, sort, asc, false, id), t.elapsed())
                    }
                    None => (-1, std::time::Duration::ZERO),
                };
                eprintln!("{sel} q={q:?} sort={sort} asc={asc} threaded={threaded}: total {total}, first {first_t:?}, middle {mid_t:?}, position {pos} (expected {at}) {pos_t:?}");
            }
        }
    }
}

#[cfg(test)]
mod notes_tests {
    use super::*;

    #[test]
    fn formatted_notes_are_kept_and_plain_ones_are_not() {
        let mut d = Db::default();
        let page = |body: &str| format!("<!DOCTYPE HTML><html><head><style>p {{}}</style></head><body style=\"font-size:14px\">\n{body}</body></html>");
        let id = d.save_event(&serde_json::json!({"title": "A", "start": "2026-01-05T10:00", "end": "2026-01-05T11:00",
            "notes": "Bring the plan", "notesHtml": page("<p>Bring the <span style=\" font-weight:700;\">plan</span></p>")}));
        let e = d.events.iter().find(|e| e.id == id).unwrap();
        assert_eq!(e.notes_html, "<p>Bring the <span style=\" font-weight:700;\">plan</span></p>");
        let id = d.save_event(&serde_json::json!({"id": id, "title": "A", "start": "2026-01-05T10:00", "end": "2026-01-05T11:00",
            "notes": "Just words", "notesHtml": page("<p style=\" margin-top:0px;\">Just words</p>")}));
        assert_eq!(d.events.iter().find(|e| e.id == id).unwrap().notes_html, "");
    }
}

#[cfg(test)]
mod fetched_text_tests {
    use super::*;

    /// A message whose text is only an empty line of HTML (photos sent with no
    /// words) isn't asked for again once fetched.
    #[test]
    fn a_message_without_words_is_fetched_once() {
        let mut db = Db::empty();
        db.folders.push(Folder { id: "live:x:mail/INBOX".into(), account: "live:x:mail".into(), name: "Inbox".into(), kind: "inbox".into(), path: "INBOX".into() });
        db.messages.push(Message { id: 9001, folder: "live:x:mail/INBOX".into(), source: "live:x".into(), uid: 7, partial: true, subject: "Ergo pics".into(), ..Default::default() });
        assert_eq!(db.partial_messages("live:x").len(), 1, "waiting for its text");
        db.fill_texts(vec![crate::live::Text { id: 9001, body: String::new(), html: "<div dir=\"ltr\"><br></div>".into(), invite: String::new(), attachments: vec!["photo.jpg".into()], complete: false }]);
        assert!(db.partial_messages("live:x").is_empty(), "fetched: not asked for again");
        assert!(db.messages.iter().any(|m| m.id == 9001 && !m.preview.is_empty() && m.partial), "kept partial (the rest comes when opened), preview set");
    }
}
