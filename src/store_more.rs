//! More of the Store bridge: the outbox (Undo Send / Send Later),
//! reminders, calendar alerts and notifications, mail rules, smart
//! mailboxes, VIPs, signatures, attachments, invitations, smart groups,
//! duplicate cards, printing and Google / Microsoft sign-in.

use super::*;
use crate::live::{LiveAccount, MailOp};

/// A desktop notification with Almanac's icon. Clicking it brings Almanac
/// forward with `target` open ({"mail": id}, {"inbox": true}, {"event": id,
/// "start": …}, {"calendar": true}). `sticky`: it stays until closed
/// (calendar alerts, as in Apple Calendar).
fn notify_with(title: &str, body: &str, sticky: bool, thread: cxx_qt::CxxQtThread<qobject::Store>, target: Value) {
    let (t, b) = (title.to_string(), body.to_string());
    std::thread::spawn(move || {
        let mut n = notify_rust::Notification::new();
        n.appname("Almanac").summary(&t).body(&b);
        let icon = crate::icon_path("almanac");
        if icon.exists() {
            n.icon(&icon.display().to_string());
        }
        n.action("default", "Open");
        if sticky {
            n.timeout(notify_rust::Timeout::Never);
            #[cfg(all(unix, not(target_os = "macos")))]
            n.urgency(notify_rust::Urgency::Critical);
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        if let Ok(shown) = n.show() {
            // (Waits here, on this thread, until it's clicked or closed.)
            shown.wait_for_action(|action| {
                if action == "default" {
                    let mut target = target;
                    // (Each click a new value, so the same one twice still opens.)
                    target["at"] = json!(chrono::Utc::now().timestamp_millis());
                    let _ = thread.queue(move |mut store| store.as_mut().set_open_request(QString::from(target.to_string().as_str())));
                }
            });
        }
        #[cfg(not(all(unix, not(target_os = "macos"))))]
        {
            let _ = (thread, target);
            let _ = n.show();
        }
    });
}

/// Pages of a PDF shown in a message (at most this many; Open shows all).
const PDF_PAGES: i32 = 50;

/// Where a PDF attachment's pages are kept as pictures.
fn pdf_pages_dir(id: i64, name: &str) -> std::path::PathBuf {
    crate::mailstore::attachment_dir(id).join(".pages").join(crate::mailstore::safe_name(name))
}

/// PDFs opened in messages this session (message id, name): the rest show
/// closed, as a line with their name.
static PDFS_OPEN: std::sync::Mutex<Vec<(i64, String)>> = std::sync::Mutex::new(Vec::new());

fn pdf_open(id: i64, name: &str) -> bool {
    PDFS_OPEN.lock().map(|o| o.iter().any(|(i, n)| *i == id && n == name)).unwrap_or(false)
}

/// How many pages a PDF has: known once its pages were made, otherwise counted
/// from its page objects (0 when they're packed away where they can't be seen).
fn pdf_page_count(id: i64, name: &str, bytes: &[u8]) -> i64 {
    if let Some(n) = std::fs::read_to_string(pdf_pages_dir(id, name).join("done")).ok().and_then(|t| t.trim().parse().ok()) {
        return n;
    }
    let mut n = 0;
    let mut i = 0;
    while let Some(at) = bytes[i..].windows(5).position(|w| w == b"/Type") {
        let mut j = i + at + 5;
        while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\r' || bytes[j] == b'\n') {
            j += 1;
        }
        if bytes[j..].starts_with(b"/Page") && !bytes[j..].starts_with(b"/Pages") {
            n += 1;
        }
        i = j;
    }
    n
}

/// A PDF attachment's pages as pictures (base64 PNGs) and how many it has,
/// once they've been made.
fn pdf_pages(id: i64, name: &str) -> Option<(Vec<String>, i64)> {
    use base64::Engine;
    let dir = pdf_pages_dir(id, name);
    let total: i64 = std::fs::read_to_string(dir.join("done")).ok()?.trim().parse().ok()?;
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "png")).collect();
    files.sort();
    let pages = files.iter().filter_map(|f| std::fs::read(f).ok()).map(|b| base64::engine::general_purpose::STANDARD.encode(b)).collect();
    Some((pages, total))
}

/// Makes a PDF's page pictures (once), a page per turn of the window's event
/// loop (Qt's PDF reader works on the main thread only); the message redraws
/// when they're all there.
fn make_pdf_pages(thread: cxx_qt::CxxQtThread<qobject::Store>, id: i64, name: String, file: std::path::PathBuf) {
    static MAKING: std::sync::Mutex<Vec<(i64, String)>> = std::sync::Mutex::new(Vec::new());
    {
        let mut making = MAKING.lock().unwrap();
        if making.contains(&(id, name.clone())) {
            return;
        }
        making.push((id, name.clone()));
    }
    let _ = std::fs::create_dir_all(pdf_pages_dir(id, &name));
    pdf_page_step(thread, id, name, file, 0);
}

fn pdf_page_step(thread: cxx_qt::CxxQtThread<qobject::Store>, id: i64, name: String, file: std::path::PathBuf, index: i32) {
    let next = thread.clone();
    let _ = thread.queue(move |store| {
        let dir = pdf_pages_dir(id, &name);
        let out = dir.join(format!("page-{:03}.png", index + 1));
        let total = qobject::craft_pdf_page(
            &QString::from(file.display().to_string().as_str()),
            index,
            &QString::from(out.display().to_string().as_str()),
            1400,
        );
        if index + 1 < total.min(PDF_PAGES) {
            pdf_page_step(next, id, name, file, index + 1);
        } else {
            let _ = std::fs::write(dir.join("done"), total.to_string());
            store.changed();
        }
    });
}

fn downloads_dir() -> std::path::PathBuf {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(std::path::PathBuf::from).unwrap_or_default();
    let d = home.join("Downloads");
    if d.is_dir() {
        d
    } else {
        crate::data::data_dir()
    }
}

fn unique(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let clean: String = name.chars().map(|c| if "/\\:*?\"<>|".contains(c) { '_' } else { c }).collect();
    let mut p = dir.join(&clean);
    let mut n = 1;
    while p.exists() {
        let (stem, ext) = match clean.rsplit_once('.') {
            Some((s, e)) => (s.to_string(), format!(".{e}")),
            None => (clean.clone(), String::new()),
        };
        p = dir.join(format!("{stem} ({n}){ext}"));
        n += 1;
    }
    p
}

fn file_url(p: &std::path::Path) -> String {
    let s = p.display().to_string().replace('\\', "/");
    if s.starts_with('/') {
        format!("file://{s}")
    } else {
        format!("file:///{s}")
    }
}

impl qobject::Store {
    // ---------------------------------------------------------------- tick

    /// Called every few seconds by the window.
    pub(super) fn tick(mut self: Pin<&mut Self>) {
        let _timed = crate::Timed::new("tick");
        let due = if crate::offline() { Vec::new() } else { self.as_mut().db_mut().take_due_outgoing() };
        let undo = self.undo_id;
        if undo != 0 && !self.db.outbox.iter().any(|o| o.id == undo) {
            self.as_mut().set_undo_id(0);
        }
        if !due.is_empty() {
            for o in due {
                self.as_mut().send_now(o);
            }
            self.as_mut().changed();
        }
        let now = chrono::Local::now().timestamp();
        if now - self.last_alert_check >= 20 {
            self.as_mut().rust_mut().get_mut().last_alert_check = now;
            let reminders = self.as_mut().db_mut().due_reminders();
            let alerts = self.as_mut().db_mut().due_alerts();
            let (nm, na) = (self.db.settings.notify_mail, self.db.settings.notify_alerts);
            for (id, from, subject) in &reminders {
                if nm {
                    notify_with(&format!("Reminder: {from}"), subject, false, self.qt_thread(), json!({"mail": id}));
                }
            }
            // Snoozed alerts that are due again.
            let again: Vec<(String, String, i64, String)> =
                self.snoozed_alerts.iter().filter(|s| s.0 <= now).map(|s| (s.1.clone(), s.2.clone(), 0, String::new())).collect();
            self.as_mut().rust_mut().get_mut().snoozed_alerts.retain(|s| s.0 > now);
            for (title, when, id, start) in alerts.iter().chain(again.iter()) {
                self.as_mut().show_alert(title, when, *id, start, na);
            }
            if !reminders.is_empty() || !alerts.is_empty() {
                self.changed();
            }
        }
    }

    fn send_now(mut self: Pin<&mut Self>, o: crate::extras::Outgoing) {
        let _timed = crate::Timed::new("send_now");
        let account = o.payload.get("account").and_then(Value::as_str).unwrap_or("").to_string();
        let Some(acct) = self.db.live_account(&account).cloned() else {
            self.as_mut().db_mut().store_outgoing(&o.payload, "sent");
            self.as_mut().set_status_text(QString::from("Saved in On My Computer › Sent (no account chosen, not sent)."));
            return;
        };
        let sent_path = self.db.live_kind_path(&account, "sent").map(|(_, p)| p);
        let m = crate::live::OutMsg::from_json(&o.payload);
        let payload = o.payload.clone();
        self.background(
            move || Ok(crate::live::send_msg(&acct, &m, sent_path.as_deref())),
            move |mut store, r: Result<(), String>| {
                match r {
                    Ok(()) => {
                        store.as_mut().db_mut().store_sent_live(&account, &payload);
                        store.as_mut().set_status_text(QString::from("Message sent."));
                    }
                    Err(e) => {
                        // Keep it in the Outbox and try again later.
                        let at = chrono::Local::now().timestamp() + 300;
                        store.as_mut().db_mut().queue_outgoing(payload, at, true);
                        store.as_mut().set_status_text(QString::from(format!("{e} — kept in the Outbox, will retry.").as_str()));
                    }
                }
                store.changed();
            },
        );
    }

    pub(super) fn undo_send(mut self: Pin<&mut Self>) -> QString {
        let _timed = crate::Timed::new("undo_send");
        let id = self.undo_id;
        self.as_mut().set_undo_id(0);
        match self.as_mut().db_mut().cancel_outgoing(id) {
            Some(o) => {
                self.as_mut().set_status_text(QString::from("Sending undone."));
                self.changed();
                q(&o.payload)
            }
            None => QString::default(),
        }
    }

    pub(super) fn outbox(&self) -> QString {
        let _timed = crate::Timed::new("outbox");
        q(&self.db.outbox_json())
    }

    pub(super) fn cancel_outbox(mut self: Pin<&mut Self>, id: i64) -> QString {
        let _timed = crate::Timed::new("cancel_outbox");
        let r = self.as_mut().db_mut().cancel_outgoing(id).map(|o| o.payload).unwrap_or(Value::Null);
        self.changed();
        q(&r)
    }

    // ------------------------------------------------- new mail and rules

    /// Runs rules on newly arrived mail, makes their changes on the server
    /// and shows a notification.
    pub(super) fn after_new_mail(mut self: Pin<&mut Self>, new_ids: Vec<i64>) {
        let _timed = crate::Timed::new("after_new_mail");
        if new_ids.is_empty() {
            return;
        }
        let targets: std::collections::HashMap<i64, (LiveAccount, String, u32)> =
            new_ids.iter().filter_map(|id| self.db.live_message(*id).map(|t| (*id, t))).collect();
        let ops = self.as_mut().db_mut().apply_rules(&new_ids);
        self.as_mut().push_rule_ops(ops, targets);
        // Notification for new unread mail in the inbox (not muted).
        let fresh: Vec<(String, String, i64)> = new_ids
            .iter()
            .filter_map(|id| self.db.get(*id))
            .filter(|m| !m.read && !self.db.is_muted(m) && self.db.folders.iter().any(|f| f.id == m.folder && f.kind == "inbox"))
            .map(|m| (if m.from_name.is_empty() { m.from_email.clone() } else { m.from_name.clone() }, m.subject.clone(), m.id))
            .collect();
        if !fresh.is_empty() {
            let n = *self.new_mail() + fresh.len() as i32;
            self.as_mut().set_new_mail(n);
        }
        match if self.db.settings.notify_mail { fresh.len() } else { 0 } {
            0 => {}
            // (Clicked: that message open; several: the Inbox.)
            1 => notify_with(&fresh[0].0, &fresh[0].1, false, self.qt_thread(), json!({"mail": fresh[0].2})),
            n => notify_with(
                &format!("{n} new messages"),
                &fresh.iter().take(3).map(|x| x.0.clone()).collect::<Vec<_>>().join(", "),
                false,
                self.qt_thread(),
                json!({"inbox": true}),
            ),
        }
    }

    fn push_rule_ops(mut self: Pin<&mut Self>, ops: Vec<(i64, String, String)>, targets: std::collections::HashMap<i64, (LiveAccount, String, u32)>) {
        let _timed = crate::Timed::new("push_rule_ops");
        for (id, kind, value) in ops {
            let Some(t) = targets.get(&id).cloned() else { continue };
            let op = match kind.as_str() {
                "flag" => Some(MailOp::Flagged(true)),
                "read" => Some(MailOp::Seen(true)),
                "move" => self.db.folder_path(&value).map(MailOp::Move),
                "junk" => self.db.live_folder_of_kind(id, "junk").map(MailOp::Move),
                "delete" => self.db.live_folder_of_kind(id, "trash").map(MailOp::Move),
                _ => None,
            };
            if let Some(op) = op {
                self.as_mut().push_mail(Some(t), op);
            }
        }
    }

    pub(super) fn apply_rules_now(mut self: Pin<&mut Self>) {
        let _timed = crate::Timed::new("apply_rules_now");
        let ids: Vec<i64> = if self.db.backed {
            self.db.messages_of("kind:inbox").iter().map(|m| m.id).collect()
        } else {
            self.db.messages.iter().map(|m| m.id).collect()
        };
        let targets: std::collections::HashMap<i64, (LiveAccount, String, u32)> =
            ids.iter().filter_map(|id| self.db.live_message(*id).map(|t| (*id, t))).collect();
        let ops = self.as_mut().db_mut().apply_rules_to_inbox();
        let n = ops.len();
        self.as_mut().push_rule_ops(ops, targets);
        self.as_mut().set_status_text(QString::from(format!("Rules made {n} changes.").as_str()));
        self.changed();
    }

    pub(super) fn rules(&self) -> QString {
        let _timed = crate::Timed::new("rules");
        q(&self.db.rules_json())
    }

    pub(super) fn save_rule(mut self: Pin<&mut Self>, json: &QString) {
        let _timed = crate::Timed::new("save_rule");
        self.as_mut().db_mut().save_rule(&parse(json));
        self.changed();
    }

    pub(super) fn delete_rule(mut self: Pin<&mut Self>, id: &QString) {
        let _timed = crate::Timed::new("delete_rule");
        self.as_mut().db_mut().delete_rule(&id.to_string());
        self.changed();
    }

    // ---------------------------------------------------- smart mailboxes

    pub(super) fn smart_mailbox(&self, id: &QString) -> QString {
        let _timed = crate::Timed::new("smart_mailbox");
        let id = id.to_string();
        let id = id.strip_prefix("smart:").unwrap_or(&id);
        q(&self.db.smart_mailboxes.iter().find(|s| s.id == id).map(|s| serde_json::to_value(s).unwrap_or(Value::Null)).unwrap_or(Value::Null))
    }

    pub(super) fn save_smart_mailbox(mut self: Pin<&mut Self>, json: &QString) -> QString {
        let _timed = crate::Timed::new("save_smart_mailbox");
        let id = self.as_mut().db_mut().save_smart_mailbox(&parse(json));
        self.changed();
        QString::from(format!("smart:{id}").as_str())
    }

    pub(super) fn delete_smart_mailbox(mut self: Pin<&mut Self>, id: &QString) {
        let _timed = crate::Timed::new("delete_smart_mailbox");
        let id = id.to_string();
        self.as_mut().db_mut().delete_smart_mailbox(id.strip_prefix("smart:").unwrap_or(&id));
        self.changed();
    }

    // ------------------------------------------------- VIPs, blocks, etc.

    pub(super) fn toggle_vip(mut self: Pin<&mut Self>, email: &QString) {
        let _timed = crate::Timed::new("toggle_vip");
        self.as_mut().db_mut().toggle_vip(&email.to_string());
        self.changed();
    }

    pub(super) fn toggle_blocked(mut self: Pin<&mut Self>, email: &QString) {
        let _timed = crate::Timed::new("toggle_blocked");
        self.as_mut().db_mut().toggle_blocked(&email.to_string());
        self.changed();
    }

    pub(super) fn toggle_mute(mut self: Pin<&mut Self>, id: i64) {
        let _timed = crate::Timed::new("toggle_mute");
        self.as_mut().db_mut().toggle_muted(id);
        self.changed();
    }

    pub(super) fn remind_me(mut self: Pin<&mut Self>, id: i64, at_ms: i64) {
        let _timed = crate::Timed::new("remind_me");
        self.as_mut().db_mut().remind_me(id, at_ms / 1000);
        self.as_mut().set_status_text(QString::from(if at_ms > 0 { "You'll be reminded about this message." } else { "Reminder removed." }));
        self.changed();
    }

    pub(super) fn signature(&self, account: &QString) -> QString {
        let _timed = crate::Timed::new("signature");
        QString::from(self.db.signature(&account.to_string()).as_str())
    }

    pub(super) fn set_signature(mut self: Pin<&mut Self>, account: &QString, text: &QString) {
        let _timed = crate::Timed::new("set_signature");
        self.as_mut().db_mut().set_signature(&account.to_string(), &text.to_string());
        self.changed();
    }

    // --------------------------------------------------------- attachments

    /// Opens (or saves to Downloads) one attachment of a message.
    /// Saves one attachment where the person chose (`dest`: a file, or a folder
    /// to put it in under its own name).
    pub(super) fn save_attachment_as(mut self: Pin<&mut Self>, id: i64, name: &QString, dest: &QString) {
        let _timed = crate::Timed::new("save_attachment_as");
        let name = name.to_string();
        let Some(m) = self.db.get(id) else { return };
        let live = self.db.live_message(id);
        let dest = super::url_to_path(&dest.to_string());
        self.as_mut().set_status_text(QString::from(format!("Saving {name}…").as_str()));
        let wanted = name.clone();
        self.background(
            move || -> Result<std::path::PathBuf, String> {
                let stored = attachment_path(&m, live, &wanted)?;
                let p = if dest.is_dir() { unique(&dest, &wanted) } else { dest };
                std::fs::copy(&stored, &p).map_err(|e| format!("Couldn't save {wanted}: {e}"))?;
                Ok(p)
            },
            move |mut store, p| store.as_mut().set_status_text(QString::from(format!("Saved to {}", p.display()).as_str())),
        );
    }

    /// The Downloads folder (where Save dialogs start), as a file:// address.
    pub(super) fn print_attachment(mut self: Pin<&mut Self>, id: i64, name: &QString) {
        let _timed = crate::Timed::new("print_attachment");
        let Some(path) = crate::mailstore::attachment_file(id, &name.to_string()) else {
            self.as_mut().set_status_text(QString::from("That attachment hasn't been downloaded yet."));
            return;
        };
        // A word on screen first: the print dialog takes a moment to come up.
        self.as_mut().set_status_text(QString::from("Opening the print dialog…"));
        let (file, title) = (QString::from(path.display().to_string().as_str()), name.clone());
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(60));
            let _ = thread.queue(move |mut store| {
                let text = if qobject::craft_print_pdf_file(&file, &title) { "Sent to the printer." } else { "" };
                store.as_mut().set_status_text(QString::from(text));
            });
        });
    }

    pub(super) fn attachment_url(&self, id: i64, name: &QString) -> QString {
        let _timed = crate::Timed::new("attachment_url");
        match crate::mailstore::attachment_file(id, &name.to_string()) {
            Some(p) => QString::from(file_url(&p).as_str()),
            None => QString::default(),
        }
    }

    pub(super) fn downloads_folder(&self) -> QString {
        let _timed = crate::Timed::new("downloads_folder");
        QString::from(file_url(&downloads_dir()).as_str())
    }

    pub(super) fn open_attachment(mut self: Pin<&mut Self>, id: i64, name: &QString, save: bool) {
        let _timed = crate::Timed::new("open_attachment");
        let name = name.to_string();
        let Some(m) = self.db.get(id) else { return };
        let live = self.db.live_message(id);
        self.as_mut().set_status_text(QString::from(format!("Getting {name}…").as_str()));
        let wanted = name.clone();
        self.background(
            move || -> Result<std::path::PathBuf, String> {
                // Attachments are kept as files: fetched once, then opened from there.
                let stored = attachment_path(&m, live, &wanted)?;
                if !save {
                    return Ok(stored);
                }
                let dir = downloads_dir();
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let p = unique(&dir, &wanted);
                std::fs::copy(&stored, &p).map_err(|e| e.to_string())?;
                Ok(p)
            },
            move |mut store, p| {
                if save {
                    store.as_mut().set_status_text(QString::from(format!("Saved to {}", p.display()).as_str()));
                } else {
                    store.as_mut().set_status_text(QString::from(format!("Opening {name}").as_str()));
                    store.as_mut().set_open_url(QString::from(file_url(&p).as_str()));
                }
            },
        );
    }

    // --------------------------------------------------------- invitations

    pub(super) fn my_addresses(&self) -> Vec<String> {
        let _timed = crate::Timed::new("my_addresses");
        let mut v: Vec<String> = self.db.live.iter().map(|a| a.email.to_lowercase()).collect();
        v.extend(self.db.accounts.iter().map(|a| a.email.to_lowercase()));
        v
    }

    pub(super) fn invite_info(&self, id: i64) -> QString {
        let _timed = crate::Timed::new("invite_info");
        let Some(mut m) = self.db.get(id) else { return q(&Value::Null) };
        m.invite = self.db.text_of(id).2;
        if m.invite.is_empty() {
            return q(&Value::Null);
        }
        let method = crate::ics::calendar_method(&m.invite);
        let Some(e) = crate::ics::parse_vevents(&m.invite).into_iter().find(|e| e.recurrence_id.is_none()).or_else(|| crate::ics::parse_vevents(&m.invite).into_iter().next()) else {
            return q(&Value::Null);
        };
        let zone = if e.all_day { String::new() } else { e.zone.clone() };
        let ls = e.start.map(|t| crate::data::fmt_dt(crate::tz::to_local(t, &zone))).unwrap_or_default();
        let le = e.end.map(|t| crate::data::fmt_dt(crate::tz::to_local(t, &zone))).unwrap_or_default();
        let mine = self.my_addresses();
        let existing = self.db.events.iter().find(|x| x.uid == e.uid && x.recurrence_id.is_empty());
        let my_status = existing
            .and_then(|x| x.attendees.iter().find(|a| mine.contains(&a.email.to_lowercase())).map(|a| a.status.clone()))
            .unwrap_or_default();
        let calendars: Vec<Value> = self
            .db
            .calendars
            .iter()
            .filter(|c| c.id != crate::data::BIRTHDAYS)
            .map(|c| json!({"id": c.id, "name": c.name, "color": c.color}))
            .collect();
        q(&json!({
            "method": method, "title": e.title, "start": ls, "end": le, "allDay": e.all_day,
            "location": e.location, "organizer": e.organizer, "repeats": !e.rrule.is_empty(),
            "attendees": e.attendees.len(), "inCalendar": existing.is_some(), "myStatus": my_status,
            "calendar": existing.map(|x| x.calendar.clone()).unwrap_or_default(), "calendars": calendars,
        }))
    }

    /// Accept / Maybe / Decline: puts the event in a calendar (or removes a
    /// cancelled one) and emails the answer to the organiser.
    pub(super) fn respond_invite(mut self: Pin<&mut Self>, id: i64, status: &QString, calendar: &QString) {
        let _timed = crate::Timed::new("respond_invite");
        let status = status.to_string().to_uppercase();
        let Some(mut m) = self.db.get(id) else { return };
        m.invite = self.db.text_of(id).2;
        let method = crate::ics::calendar_method(&m.invite);
        let mine = self.my_addresses();
        let acct_id = self.db.message_account(id);
        let acct = self.db.live_account(&acct_id).cloned().or_else(|| self.db.live.first().cloned());
        let my_email = acct.as_ref().map(|a| a.email.to_lowercase()).unwrap_or_default();
        let source_uid = crate::ics::parse_vevents(&m.invite).first().map(|e| e.uid.clone()).unwrap_or_default();
        if method == "CANCEL" {
            let ids: Vec<i64> = self.db.events.iter().filter(|e| e.uid == source_uid && e.recurrence_id.is_empty()).map(|e| e.id).collect();
            for i in ids {
                self.as_mut().delete_event(i);
            }
            self.as_mut().set_status_text(QString::from("Removed the cancelled event."));
            return;
        }
        let cal_id = calendar.to_string();
        let source = self.db.calendar(&cal_id).map(|c| c.source.clone()).unwrap_or_else(|| crate::data::SOURCE_LOCAL.into());
        let mut evs = crate::data::events_from_resource(&m.invite, &cal_id, &source);
        for e in evs.iter_mut() {
            let mut found = false;
            for a in e.attendees.iter_mut() {
                if mine.contains(&a.email.to_lowercase()) {
                    a.status = status.clone();
                    found = true;
                }
            }
            if !found && !my_email.is_empty() {
                e.attendees.push(crate::data::Attendee { email: my_email.clone(), name: String::new(), status: status.clone() });
            }
        }
        // Replace an earlier copy of the same invitation.
        let old: Vec<i64> = self.db.events.iter().filter(|e| e.uid == source_uid).map(|e| e.id).collect();
        let keep_ref = self.db.events.iter().find(|e| e.uid == source_uid && e.recurrence_id.is_empty()).map(|e| (e.href.clone(), e.etag.clone(), e.raw.clone(), e.calendar.clone()));
        self.as_mut().db_mut().events.retain(|e| !old.contains(&e.id));
        let mut first = 0;
        if status != "DECLINED" {
            for mut e in evs {
                if let Some((h, t, r, c)) = &keep_ref {
                    if c == &cal_id {
                        e.href = h.clone();
                        e.etag = t.clone();
                        e.raw = r.clone();
                    }
                }
                e.id = self.as_mut().db_mut().new_id();
                if first == 0 {
                    first = e.id;
                }
                self.as_mut().db_mut().events.push(e);
            }
            if first != 0 {
                self.as_mut().push_event(first);
            }
        }
        // The reply email.
        if let Some(a) = acct {
            let mut reply = crate::data::events_from_resource(&m.invite, &cal_id, &source).into_iter().next().unwrap_or_default();
            reply.attendees = vec![crate::data::Attendee { email: a.email.clone(), name: a.full_name.clone(), status: status.clone() }];
            self.as_mut().background(move || crate::live::send_invitation(&a, &reply, "REPLY"), |_, _| {});
        }
        let word = match status.as_str() {
            "ACCEPTED" => "Accepted",
            "DECLINED" => "Declined",
            _ => "Answered Maybe",
        };
        self.as_mut().set_status_text(QString::from(word));
        self.changed();
    }

    // ------------------------------------------------------------ contacts

    pub(super) fn save_smart_group(mut self: Pin<&mut Self>, json: &QString) -> QString {
        let _timed = crate::Timed::new("save_smart_group");
        let id = self.as_mut().db_mut().save_smart_group(&parse(json));
        self.changed();
        QString::from(id.as_str())
    }

    pub(super) fn delete_smart_group(mut self: Pin<&mut Self>, id: &QString) {
        let _timed = crate::Timed::new("delete_smart_group");
        self.as_mut().db_mut().delete_smart_group(&id.to_string());
        self.changed();
    }

    pub(super) fn delete_group(mut self: Pin<&mut Self>, id: &QString) {
        let _timed = crate::Timed::new("delete_group");
        self.as_mut().db_mut().delete_group(&id.to_string());
        self.changed();
    }

    pub(super) fn rename_group(mut self: Pin<&mut Self>, id: &QString, name: &QString) {
        let _timed = crate::Timed::new("rename_group");
        self.as_mut().db_mut().rename_group(&id.to_string(), &name.to_string());
        self.changed();
    }

    pub(super) fn duplicates(&self) -> QString {
        let _timed = crate::Timed::new("duplicates");
        q(&self.db.duplicates_json())
    }

    pub(super) fn merge_contacts(mut self: Pin<&mut Self>, ids_json: &QString) {
        let _timed = crate::Timed::new("merge_contacts");
        let ids: Vec<i64> = serde_json::from_str(&ids_json.to_string()).unwrap_or_default();
        let removed = self.as_mut().db_mut().merge_contacts(&ids);
        for c in removed.into_iter().filter(|c| !c.href.is_empty()) {
            if let Some(acct) = self.db.live_for_source(&c.source) {
                self.as_mut().background(move || crate::live::delete_contact_remote(&acct, &c), |_, _| {});
            }
        }
        if let Some(keep) = ids.first().and_then(|id| self.db.contacts.iter().find(|c| c.id == *id)).cloned() {
            let v = serde_json::to_value(&keep).unwrap_or(Value::Null);
            self.as_mut().save_contact(&QString::from(v.to_string().as_str()));
        }
        self.as_mut().set_status_text(QString::from("Cards merged."));
        self.changed();
    }

    pub(super) fn set_my_card(mut self: Pin<&mut Self>, id: i64) {
        let _timed = crate::Timed::new("set_my_card");
        self.as_mut().db_mut().my_card = id;
        self.changed();
    }

    pub(super) fn set_contact_photo(mut self: Pin<&mut Self>, id: i64, url: &QString) {
        let _timed = crate::Timed::new("set_contact_photo");
        let path = if url.to_string().is_empty() {
            String::new()
        } else {
            match std::fs::read(url_to_path(&url.to_string())) {
                Ok(b) => crate::ics::store_photo(&b),
                Err(e) => {
                    self.as_mut().set_status_text(QString::from(format!("Could not read the picture: {e}").as_str()));
                    return;
                }
            }
        };
        let Some(mut c) = self.db.contacts.iter().find(|c| c.id == id).cloned() else { return };
        c.photo = path;
        let v = serde_json::to_value(&c).unwrap_or(Value::Null);
        self.as_mut().save_contact(&QString::from(v.to_string().as_str()));
    }

    // ------------------------------------------------------------ printing

    /// The system print dialog, then prints.
    pub(super) fn print_html(mut self: Pin<&mut Self>, title: &QString, html: &QString) {
        let _timed = crate::Timed::new("print_html");
        if qobject::craft_print_dialog(html, title) {
            self.as_mut().set_status_text(QString::from("Sent to the printer."));
        }
    }

    /// Saves the page as a PDF and opens it.
    pub(super) fn export_pdf(mut self: Pin<&mut Self>, title: &QString, html: &QString) {
        let _timed = crate::Timed::new("export_pdf");
        let name: String = title.to_string().chars().filter(|c| c.is_alphanumeric() || *c == ' ').take(40).collect();
        let dir = std::env::temp_dir().join("almanac-print");
        let _ = std::fs::create_dir_all(&dir);
        let path = unique(&dir, &format!("{}.pdf", if name.trim().is_empty() { "Print" } else { name.trim() }));
        if qobject::craft_print_pdf(html, &QString::from(path.display().to_string().as_str())) {
            self.as_mut().set_status_text(QString::from("Opening the print-out…"));
            self.as_mut().set_open_url(QString::from(file_url(&path).as_str()));
        } else {
            self.as_mut().set_status_text(QString::from("Could not make the print-out."));
        }
    }

    // ------------------------------------------------- Google / Microsoft

    pub(super) fn sign_in_with(mut self: Pin<&mut Self>, provider: &QString, client_id: &QString, client_secret: &QString, tenant: &QString) {
        let _timed = crate::Timed::new("sign_in_with");
        let provider = provider.to_string();
        let (mut cid, mut secret, tenant) = (client_id.to_string().trim().to_string(), client_secret.to_string().trim().to_string(), tenant.to_string().trim().to_string());
        // The app's own registration when it has one; no ID to type.
        if cid.is_empty() {
            if let Some((i, sec)) = crate::oauth::builtin_client(&provider) {
                cid = i;
                secret = sec;
            }
        }
        if cid.is_empty() {
            self.as_mut().set_status_text(QString::from("Set up sign-in for this provider first (Sign-in Setup)."));
            return;
        }
        self.as_mut().set_signing_in(QString::from(provider.as_str()));
        self.as_mut().set_status_text(QString::from("Opening the sign-in page in your browser…"));
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let t2 = thread.clone();
            let open = move |url: String| {
                let _ = t2.queue(move |mut store| store.as_mut().set_open_url(QString::from(url.as_str())));
            };
            let result = crate::oauth::sign_in(&provider, &cid, &secret, &tenant, &open);
            let _ = thread.queue(move |mut store| match result {
                // Cancelled, or replaced by a newer sign-in: leave its state alone.
                Err(e) if e == crate::oauth::SIGN_IN_STOPPED => {}
                Err(e) => {
                    store.as_mut().set_signing_in(QString::default());
                    store.as_mut().set_status_text(QString::from(e.as_str()))
                }
                Ok(si) => {
                    store.as_mut().set_signing_in(QString::default());
                    store.add_signed_in(provider, cid, secret, tenant, si);
                }
            });
        });
    }

    /// Adds (or refreshes) the account a finished sign-in belongs to.
    pub(super) fn add_signed_in(mut self: Pin<&mut Self>, provider: String, cid: String, secret: String, tenant: String, si: crate::oauth::SignIn) {
        let _timed = crate::Timed::new("add_signed_in");
        let mut store = self.as_mut();
        // Signing in again with the same account (e.g. Google's weekly
        // sign-out in Testing mode) refreshes it instead of adding a copy.
        let existing = store
            .db
            .live
            .iter()
            .find(|a| a.provider == provider && a.email.eq_ignore_ascii_case(&si.email))
            .cloned();
        // Google lets each permission be unticked on its page: whether Drive
        // (for big attachments) was allowed is kept, for Accounts to say.
        let drive_note = |id: &str| {
            if provider == crate::oauth::GOOGLE && !si.scope.is_empty() {
                let ok = si.scope.contains(crate::oauth::scopes(crate::oauth::GOOGLE, &crate::oauth::Resource::Drive));
                let _ = crate::oauth::save_extra(&format!("{id}:drive"), if ok { "yes" } else { "no" });
            }
        };
        let no_drive = provider == crate::oauth::GOOGLE && !si.scope.is_empty()
            && !si.scope.contains(crate::oauth::scopes(crate::oauth::GOOGLE, &crate::oauth::Resource::Drive));
        if let Some(mut a) = existing {
            let _ = crate::live::save_password(&format!("{}:refresh", a.id), &si.refresh_token);
            drive_note(&a.id);
            crate::oauth::forget_tokens(&a.id);
            a.client_id = cid;
            a.client_secret = secret;
            a.tenant = tenant;
            store.as_mut().db_mut().upsert_live(a.clone());
            store.as_mut().set_status_text(QString::from(
                if no_drive {
                    format!("Signed in again as {}, but Google Drive access wasn't allowed: sign in again and tick the Google Drive box on Google's page to send big attachments as links.", si.email)
                } else {
                    format!("Signed in again as {}. Getting mail, calendars and contacts…", si.email)
                }
                .as_str(),
            ));
            store.as_mut().changed();
            store.start_jobs(vec![], vec![a]);
            return;
        }
        let id = format!("acct{}", chrono::Utc::now().timestamp_millis());
        let _ = crate::live::save_password(&format!("{id}:refresh"), &si.refresh_token);
        drive_note(&id);
        let (imap, smtp, smtp_port) = crate::oauth::mail_servers(&provider);
        let acct = LiveAccount {
            id,
            name: si.email.clone(),
            email: si.email.clone(),
            username: si.email.clone(),
            imap_host: imap.into(),
            imap_port: 993,
            smtp_host: smtp.into(),
            smtp_port,
            provider: provider.clone(),
            client_id: cid,
            client_secret: secret,
            tenant,
            ..Default::default()
        };
        store.as_mut().db_mut().upsert_live(acct.clone());
        store.as_mut().set_status_text(QString::from(format!("Signed in as {}. Getting mail, calendars and contacts…", si.email).as_str()));
        store.as_mut().changed();
        store.start_jobs(vec![], vec![acct]);
    }

    // ------------------------------------------------------- setup guide

    pub(super) fn finish_setup(mut self: Pin<&mut Self>) {
        let _timed = crate::Timed::new("finish_setup");
        self.as_mut().db_mut().onboarded = true;
        self.as_mut().set_onboarded(true);
        self.changed();
    }

    /// An event's alert: the desktop notification (it stays until closed; when
    /// notifications are on) and the alert card in the window (Snooze, Dismiss).
    /// The same for real alerts and the test one.
    fn show_alert(mut self: Pin<&mut Self>, title: &str, when: &str, id: i64, start: &str, notify: bool) {
        if notify {
            let target = if id != 0 { json!({"event": id, "start": start}) } else { json!({"calendar": true}) };
            notify_with(title, when, true, self.qt_thread(), target);
        }
        self.as_mut().rust_mut().get_mut().open_alerts.push((title.to_string(), when.to_string()));
        let j = json!(self.open_alerts.iter().map(|(t, w)| json!({"title": t, "when": w})).collect::<Vec<_>>()).to_string();
        self.as_mut().set_alerts_json(QString::from(j.as_str()));
    }

    /// Checks a signed message's signature off the window's thread: its
    /// original (kept next to its attachments once fetched, so it's fetched
    /// once), then gpg. The result is published in signatureJson.
    pub(super) fn check_signature(&self, id: i64) {
        let Some(m) = self.db.get(id) else { return };
        let (body, html, _) = self.db.text_of(id);
        if !crate::pgp::looks_signed(&m.attachments, &body, &html) {
            return;
        }
        let live = self.db.live_message(id);
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let kept = crate::mailstore::attachment_dir(id).join("original.eml");
            let raw = match std::fs::read(&kept) {
                Ok(r) => Ok(r),
                Err(_) => original_message(&m, live).inspect(|r| {
                    let _ = std::fs::create_dir_all(crate::mailstore::attachment_dir(id));
                    let _ = std::fs::write(&kept, r);
                }),
            };
            let result = match raw {
                Ok(raw) => match crate::pgp::verify(&raw) {
                    Some(c) => serde_json::to_value(c).unwrap_or_default(),
                    None => return,
                },
                Err(e) => json!({"state": "error", "detail": format!("Couldn't get the original message: {e}"), "trust": "unknown", "signer": "", "key": ""}),
            };
            let mut result = result;
            result["id"] = json!(id);
            let text = result.to_string();
            let _ = thread.queue(move |store| store.set_signature_json(QString::from(text.as_str())));
        });
    }

    /// Decrypts an encrypted message off the window's thread (gpg-agent asks
    /// for the passphrase); the text goes to the page only, never to disk.
    pub(super) fn decrypt_message(&self, id: i64, passphrase: &QString) {
        let pass = passphrase.to_string();
        let Some(m) = self.db.get(id) else { return };
        let (body, html, _) = self.db.text_of(id);
        if !crate::pgp::looks_encrypted(&m.attachments, &body, &html) {
            return;
        }
        let live = self.db.live_message(id);
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            // (The original, still encrypted, is kept like a signed one's.)
            let kept = crate::mailstore::attachment_dir(id).join("original.eml");
            let raw = match std::fs::read(&kept) {
                Ok(r) => Ok(r),
                Err(_) => original_message(&m, live).inspect(|r| {
                    let _ = std::fs::create_dir_all(crate::mailstore::attachment_dir(id));
                    let _ = std::fs::write(&kept, r);
                }),
            };
            let mut result = match raw {
                Ok(raw) => match crate::pgp::decrypt(&raw, (!pass.is_empty()).then_some(pass.as_str())) {
                    Some(d) => serde_json::to_value(d).unwrap_or_default(),
                    None => return,
                },
                Err(e) => json!({"state": "error", "detail": format!("Couldn't get the original message: {e}")}),
            };
            result["id"] = json!(id);
            let text = result.to_string();
            let _ = thread.queue(move |store| store.set_decrypted_json(QString::from(text.as_str())));
        });
    }

    pub(super) fn pgp_ready(&self, from: &QString, recipients: &QString) -> QString {
        let _timed = crate::Timed::new("pgp_ready");
        if !crate::pgp::available() {
            return q(&json!({"gpg": false, "canSign": false, "missing": []}));
        }
        let me = crate::pgp::addresses(&from.to_string()).into_iter().next().unwrap_or_default();
        let to = crate::pgp::addresses(&recipients.to_string());
        q(&json!({"gpg": true, "canSign": !me.is_empty() && crate::pgp::can_sign(&me), "missing": crate::pgp::missing_keys(&to)}))
    }

    pub(super) fn pgp_keys(&self) -> QString {
        let _timed = crate::Timed::new("pgp_keys");
        let keys: Vec<Value> = self
            .db
            .live
            .iter()
            .filter(|a| !a.smtp_host.is_empty() && !a.email.is_empty())
            .map(|a| json!({"email": a.email, "name": if a.full_name.is_empty() { &a.name } else { &a.full_name }, "fingerprint": crate::pgp::own_key(&a.email).unwrap_or_default()}))
            .collect();
        q(&json!({"gpg": crate::pgp::available(), "keys": keys}))
    }

    /// Runs a key job in the background; while it runs and when it's done,
    /// pgpStatusJson says so (and the status line), and the lists are read again.
    fn pgp_job(&self, op: &str, email: &str, busy_text: &str, job: impl FnOnce() -> Result<String, String> + Send + 'static) {
        let thread = self.qt_thread();
        let (op, email) = (op.to_string(), email.to_string());
        let busy = json!({"op": op, "email": email, "busy": true, "ok": false, "text": busy_text}).to_string();
        let _ = thread.queue(move |mut store| store.as_mut().set_pgp_status_json(QString::from(busy.as_str())));
        std::thread::spawn(move || {
            let (ok, text) = match job() {
                Ok(t) => (true, t),
                Err(e) => (false, e),
            };
            let status = json!({"op": op, "email": email, "busy": false, "ok": ok, "text": text}).to_string();
            let line = if ok { text } else { format!("OpenPGP: {text}") };
            let _ = thread.queue(move |mut store| {
                store.as_mut().set_pgp_status_json(QString::from(status.as_str()));
                store.as_mut().set_status_text(QString::from(line.as_str()));
                store.changed();
            });
        });
    }

    pub(super) fn pgp_create_key(&self, name: &QString, email: &QString, passphrase: &QString) {
        let (n, e, p) = (name.to_string(), email.to_string(), passphrase.to_string());
        self.pgp_job("create", &e.clone(), "Creating your key…", move || {
            crate::pgp::create_key(&n, &e, &p).map(|f| format!("Made a key for {e}: {}", f[f.len().saturating_sub(16)..].as_bytes().chunks(4).map(|c| String::from_utf8_lossy(c).to_string()).collect::<Vec<_>>().join(" ")))
        });
    }

    pub(super) fn pgp_unlock(&self, email: &QString, passphrase: &QString) {
        let (e, p) = (email.to_string(), passphrase.to_string());
        self.pgp_job("unlock", &e.clone(), "Unlocking your key…", move || crate::pgp::unlock(&e, &p).map(|_| "Key unlocked".to_string()));
    }

    pub(super) fn file_size(&self, path: &QString) -> i64 {
        std::fs::metadata(path.to_string()).map(|m| m.len() as i64).unwrap_or(0)
    }

    pub(super) fn cloud_stores(&self) -> QString {
        let s = &self.db.settings;
        let connected = |k: &str| crate::live::password(k).is_some_and(|p| !p.is_empty());
        let client = |p: &str| crate::oauth::builtin_client(p).map(|(id, _)| id).unwrap_or_default();
        q(&json!({
            "store": if s.cloud_store.is_empty() { "account" } else { &s.cloud_store },
            "dropbox": {"connected": connected("storage-dropbox:refresh"), "clientId": client(crate::oauth::DROPBOX)},
            "box": {"connected": connected("storage-box:refresh"), "clientId": client(crate::oauth::BOX)},
            "nextcloud": {"connected": connected("storage-nextcloud:password"), "server": s.cloud_nextcloud_server, "user": s.cloud_nextcloud_user},
            "port": crate::oauth::STORAGE_PORT,
        }))
    }

    pub(super) fn cloud_connect(&self, provider: &QString, a: &QString, b: &QString) {
        let (provider, a, b) = (provider.to_string(), a.to_string().trim().to_string(), b.to_string().trim().to_string());
        let thread = self.qt_thread();
        let t2 = thread.clone();
        std::thread::spawn(move || {
            let open = move |url: String| {
                let _ = t2.queue(move |mut store| store.as_mut().set_open_url(QString::from(url.as_str())));
            };
            let name = match provider.as_str() { "dropbox" => "Dropbox", "box" => "Box", _ => "Nextcloud" };
            let result: Result<Option<(String, String)>, String> = if provider == "nextcloud" {
                crate::cloudfiles::nextcloud_login(&a, &open).map(|(server, user, pass)| {
                    let _ = crate::live::save_password("storage-nextcloud:password", &pass);
                    Some((server, user))
                })
            } else {
                let _ = crate::oauth::save_client(&provider, &a, &b);
                crate::oauth::sign_in(&provider, &a, &b, "", &open).map(|si| {
                    let _ = crate::live::save_password(&format!("storage-{provider}:refresh"), &si.refresh_token);
                    crate::oauth::forget_tokens(&format!("storage-{provider}"));
                    None
                })
            };
            let _ = thread.queue(move |mut store| {
                match result {
                    Ok(nc) => {
                        if let Some((server, user)) = nc {
                            store.as_mut().db_mut().settings.cloud_nextcloud_server = server;
                            store.as_mut().db_mut().settings.cloud_nextcloud_user = user;
                        }
                        store.as_mut().db_mut().settings.cloud_store = provider.clone();
                        store.as_mut().set_status_text(QString::from(format!("Connected to {name}: big attachments go there now.").as_str()));
                    }
                    Err(e) => store.as_mut().set_status_text(QString::from(format!("{name}: {e}").as_str())),
                }
                store.changed();
            });
        });
    }

    pub(super) fn cloud_disconnect(mut self: Pin<&mut Self>, provider: &QString) {
        let provider = provider.to_string();
        let key = if provider == "nextcloud" { "storage-nextcloud:password".to_string() } else { format!("storage-{provider}:refresh") };
        crate::live::forget_password(&key);
        crate::oauth::forget_tokens(&format!("storage-{provider}"));
        if self.db.settings.cloud_store == provider {
            self.as_mut().db_mut().settings.cloud_store = "account".into();
        }
        self.changed();
    }

    /// The service big attachments go to from `provider`'s accounts (its name
    /// for the offer), "" for none.
    fn cloud_store_name(&self, provider: &str) -> &'static str {
        match self.db.settings.cloud_store.as_str() {
            "dropbox" => "Dropbox",
            "box" => "Box",
            "nextcloud" => "Nextcloud",
            _ => crate::cloudfiles::store_name(provider),
        }
    }

    pub(super) fn cloud_offer(&self, account: &QString, bytes: i64) -> QString {
        let provider = self.db.live_account(&account.to_string()).map(|a| a.provider.clone()).unwrap_or_default();
        let limit = crate::cloudfiles::mail_limit(&provider);
        q(&json!({
            "over": crate::cloudfiles::as_sent(bytes.max(0) as u64) > limit,
            "limitMb": limit / 1024 / 1024,
            "store": self.cloud_store_name(&provider),
            "mode": self.db.settings.cloud_attachments,
        }))
    }

    pub(super) fn cloud_upload(&self, account: &QString, path: &QString) {
        let path = path.to_string();
        let thread = self.qt_thread();
        // One entry of cloudUploadsJson changed (on the window's thread).
        fn report(thread: &cxx_qt::CxxQtThread<qobject::Store>, path: &str, entry: Value) {
            let path = path.to_string();
            let _ = thread.queue(move |mut store| {
                let mut all: Value = serde_json::from_str(&store.cloud_uploads_json().to_string()).unwrap_or_else(|_| json!({}));
                all[path.as_str()] = entry;
                store.as_mut().set_cloud_uploads_json(QString::from(all.to_string().as_str()));
            });
        }
        let name = std::path::Path::new(&path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let choice = self.db.settings.cloud_store.clone();
        let (nc_server, nc_user) = (self.db.settings.cloud_nextcloud_server.clone(), self.db.settings.cloud_nextcloud_user.clone());
        let a = self.db.live_account(&account.to_string()).cloned();
        if a.is_none() && !["dropbox", "box", "nextcloud"].contains(&choice.as_str()) {
            report(&thread, &path, json!({"state": "error", "name": name, "size": size, "error": "Big attachments can be sent as links from Google and Microsoft accounts, or through Dropbox, Box or Nextcloud (Settings › Mail)"}));
            return;
        }
        report(&thread, &path, json!({"state": "uploading", "name": name, "size": size, "sent": 0, "total": size}));
        std::thread::spawn(move || {
            let result = (|| {
                // The service chosen in Settings › Mail, or the account's own.
                let store: Box<dyn crate::cloudfiles::CloudStore + Send> = match choice.as_str() {
                    "dropbox" | "box" => {
                        let (id, secret) = crate::oauth::builtin_client(&choice).ok_or("Connect it again in Settings › Mail")?;
                        let token = crate::oauth::access_token(&format!("storage-{choice}"), &choice, &id, &secret, "", crate::oauth::Resource::Data)
                            .map_err(|_| format!("Almanac isn't signed in to {} any more: Connect it again in Settings › Mail", if choice == "box" { "Box" } else { "Dropbox" }))?;
                        if choice == "box" { Box::new(crate::cloudfiles::BoxCom::new(token)) } else { Box::new(crate::cloudfiles::Dropbox::new(token)) }
                    }
                    "nextcloud" => {
                        let pass = crate::live::password("storage-nextcloud:password").filter(|p| !p.is_empty()).ok_or("Connect Nextcloud again in Settings › Mail")?;
                        Box::new(crate::cloudfiles::Nextcloud { server: nc_server, user: nc_user, app_password: pass })
                    }
                    _ => {
                        let a = a.ok_or("No mail account to send from")?;
                        // (A stand-in Drive for tests needs no sign-in.)
                        let token = if std::env::var("ALMANAC_DRIVE_TEST_BASE").is_ok() || std::env::var("ALMANAC_ONEDRIVE_TEST_BASE").is_ok() { "test".to_string() } else { crate::live::token(&a, crate::oauth::Resource::Drive)? };
                        crate::cloudfiles::store_for(&a.provider, token).ok_or("Big attachments can be sent as links from Google and Microsoft accounts, or through Dropbox, Box or Nextcloud (Settings › Mail)")?
                    }
                };
                let last = std::cell::Cell::new(std::time::Instant::now());
                store.upload(std::path::Path::new(&path), &|sent, total| {
                    // (The window hears about it a few times a second at most.)
                    if sent < total && last.get().elapsed() < std::time::Duration::from_millis(250) {
                        return;
                    }
                    last.set(std::time::Instant::now());
                    report(&thread, &path, json!({"state": "uploading", "name": name, "size": size, "sent": sent, "total": total}));
                })
            })();
            match result {
                Ok(s) => report(&thread, &path, json!({"state": "done", "name": s.name, "size": s.size, "sent": s.size, "total": s.size, "url": s.url, "provider": s.provider})),
                Err(e) => report(&thread, &path, json!({"state": "error", "name": name, "size": size, "error": e})),
            }
        });
    }

    pub(super) fn pgp_sign_ready(&self, email: &QString) -> bool {
        crate::pgp::ready_to_sign(&email.to_string())
    }

    pub(super) fn pgp_import_keys(&self, file_url: &QString) {
        let u = file_url.to_string();
        let path = std::path::PathBuf::from(u.strip_prefix("file://").unwrap_or(&u));
        self.pgp_job("import", "", "Importing…", move || crate::pgp::import_keys(&path));
    }

    pub(super) fn pgp_publish_key(&self, fingerprint: &QString) {
        let f = fingerprint.to_string();
        self.pgp_job("publish", "", "Sending your key to keys.openpgp.org…", move || crate::pgp::publish(&f).map(|_| "Sent your key to keys.openpgp.org: they'll email you to confirm before anyone can find it".to_string()));
    }

    pub(super) fn pgp_look_up(&self, email: &QString) {
        let e = email.to_string().trim().to_string();
        self.pgp_job("lookup", &e.clone(), "Looking up a key…", move || crate::pgp::look_up(&e));
    }

    /// A made-up event's alert ("Test Event", in 5 minutes), shown as real ones
    /// are (always notified, so it can be tried with notifications off too).
    pub(super) fn test_alert(self: Pin<&mut Self>) {
        let at = chrono::Local::now() + chrono::Duration::minutes(5);
        let when = format!("In 5 minutes, at {}", at.format("%-I:%M %p"));
        self.show_alert("Test Event", &when, 0, "", true);
    }

    /// A made-up new-mail notification, as real ones are (clicked: the Inbox).
    pub(super) fn test_mail_notification(self: Pin<&mut Self>) {
        notify_with("Test Sender", "This is how new mail is announced", false, self.qt_thread(), json!({"inbox": true}));
    }

    /// Fills "On My Computer" with made-up examples to look around.
    pub(super) fn use_sample_data(mut self: Pin<&mut Self>) {
        let _timed = crate::Timed::new("use_sample_data");
        let keep_live = std::mem::take(&mut self.as_mut().db_mut().live);
        let keep_sources = std::mem::take(&mut self.as_mut().db_mut().sources);
        let keep_settings = self.db.settings.clone();
        let theme = self.db.theme.clone();
        let mut other: Vec<crate::data::Message> = self.db.messages.iter().filter(|m| m.source != crate::data::SOURCE_LOCAL).cloned().collect();
        let mut sample = Db::sample();
        sample.live = keep_live;
        sample.sources = keep_sources;
        sample.settings = keep_settings;
        sample.theme = theme;
        sample.onboarded = true;
        // (Mail in the database stays there; the examples join it.)
        sample.backed = self.db.backed;
        sample.messages.append(&mut other);
        *self.as_mut().db_mut() = sample;
        self.as_mut().set_onboarded(true);
        self.changed();
    }

    /// Removes the made-up examples again.
    pub(super) fn clear_sample_data(mut self: Pin<&mut Self>) {
        let _timed = crate::Timed::new("clear_sample_data");
        let db = self.as_mut().db_mut();
        let limit = Db::sample().next_id;
        let local = |s: &str| s == crate::data::SOURCE_LOCAL || s.is_empty();
        db.messages.retain(|m| !(local(&m.source) && m.id <= limit));
        if db.backed {
            let examples = crate::mailstore::rows(
                "(source = ?1 OR source = '') AND id <= ?2",
                &[crate::mailstore::Arg::Text(crate::data::SOURCE_LOCAL.into()), crate::mailstore::Arg::Integer(limit)],
                "date DESC",
                10_000,
                0,
            );
            for m in examples {
                db.remove_message(m.id);
            }
        }
        db.events.retain(|e| !(local(&e.source) && e.id <= limit));
        db.contacts.retain(|c| !(local(&c.source) && c.id <= limit));
        db.groups.retain(|g| !(local(&g.source) && ["friends", "coworkers", "family"].contains(&g.id.as_str())));
        db.calendars.retain(|c| c.id != "family");
        db.is_sample = false;
        db.next_id = db.next_id.max(1000);
        self.as_mut().set_status_text(QString::from("Sample data removed."));
        self.changed();
    }

    // ---------------------------------------------------------- settings

    pub(super) fn settings(&self) -> QString {
        let _timed = crate::Timed::new("settings");
        let mut v = serde_json::to_value(&self.db.settings).unwrap_or(Value::Null);
        v["vipCount"] = json!(self.db.vips.len());
        q(&v)
    }

    pub(super) fn dismiss_alert(mut self: Pin<&mut Self>, index: i32, snooze_minutes: i32) {
        let _timed = crate::Timed::new("dismiss_alert");
        let i = index as usize;
        if i >= self.open_alerts.len() {
            return;
        }
        let (title, when) = self.as_mut().rust_mut().get_mut().open_alerts.remove(i);
        if snooze_minutes > 0 {
            let at = chrono::Local::now().timestamp() + snooze_minutes as i64 * 60;
            self.as_mut().rust_mut().get_mut().snoozed_alerts.push((at, title, when));
        }
        let j = json!(self.open_alerts.iter().map(|(t, w)| json!({"title": t, "when": w})).collect::<Vec<_>>()).to_string();
        self.as_mut().set_alerts_json(QString::from(j.as_str()));
    }

    pub(super) fn set_setting(mut self: Pin<&mut Self>, key: &QString, value_json: &QString) {
        let _timed = crate::Timed::new("set_setting");
        let mut v = serde_json::to_value(&self.db.settings).unwrap_or(Value::Null);
        let val: Value = serde_json::from_str(&value_json.to_string()).unwrap_or(Value::Null);
        v[key.to_string()] = val;
        if let Ok(s) = serde_json::from_value(v) {
            self.as_mut().db_mut().settings = s;
            self.changed();
        }
    }

    pub(super) fn blocked_and_vips(&self) -> QString {
        let _timed = crate::Timed::new("blocked_and_vips");
        q(&json!({"vips": self.db.vips, "blocked": self.db.blocked}))
    }

    // ------------------------------------------------- calendar subscriptions

    /// Subscribes to a calendar on the web (a .ics address); read-only.
    pub(super) fn add_subscription(mut self: Pin<&mut Self>, name: &QString, url: &QString, color: &QString) {
        let _timed = crate::Timed::new("add_subscription");
        let url = url.to_string().trim().replacen("webcal://", "https://", 1);
        if url.is_empty() {
            return;
        }
        let id = format!("sub-{}", self.as_mut().db_mut().new_id());
        let n = name.to_string();
        let db = self.as_mut().db_mut();
        db.calendars.push(crate::data::Calendar {
            id: id.clone(),
            name: if n.trim().is_empty() { "Subscription".into() } else { n.trim().into() },
            color: if color.to_string().is_empty() { "#7f8c8d".into() } else { color.to_string() },
            visible: true,
            source: format!("sub:{id}"),
            url: url.clone(),
            ..Default::default()
        });
        db.subscriptions.push(crate::data::Subscription { calendar: id, url, last_sync: String::new() });
        self.as_mut().changed();
        self.sync_subscriptions();
    }

    /// Downloads every subscribed calendar again.
    pub(super) fn sync_subscriptions(self: Pin<&mut Self>) {
        let _timed = crate::Timed::new("sync_subscriptions");
        let subs = self.db.subscriptions.clone();
        let cals: Vec<crate::data::Calendar> = subs.iter().filter_map(|s| self.db.calendar(&s.calendar).cloned()).collect();
        self.background(
            move || {
                let agent: ureq::Agent = ureq::Agent::config_builder()
                    .http_status_as_error(false)
                    .timeout_global(Some(std::time::Duration::from_secs(60)))
                    .build()
                    .into();
                let mut out = Vec::new();
                let mut failed = Vec::new();
                for (s, cal) in subs.iter().zip(cals) {
                    let url = s.url.replacen("webcal://", "https://", 1);
                    let text = agent.get(&url).call().ok().and_then(|mut r| r.body_mut().read_to_string().ok()).unwrap_or_default();
                    if !text.contains("BEGIN:VCALENDAR") {
                        failed.push(cal.name.clone());
                        continue;
                    }
                    let mut events = Vec::new();
                    // A feed can hold many events; read them one VEVENT group (UID) at a time.
                    for e in crate::data::events_from_resource(&text, &cal.id, &cal.source) {
                        events.push(e);
                    }
                    out.push(crate::data::Imported { source: cal.source.clone(), calendars: vec![cal], events, ..Default::default() });
                }
                Ok((out, failed))
            },
            |mut store, (list, failed): (Vec<crate::data::Imported>, Vec<String>)| {
                for imp in list {
                    store.as_mut().db_mut().apply_import(imp);
                }
                // Quiet when all is well (this runs every few minutes); say which failed.
                if !failed.is_empty() {
                    store.as_mut().set_status_text(QString::from(format!("Couldn't update the subscribed calendar {}: check its web address.", failed.join(", ")).as_str()));
                }
                store.changed();
            },
        );
    }

    // ---------------------------------------------------------- forwarding

    /// Copies a message's attachments to temporary files so a forward
    /// can include them; the result arrives in `attachmentsReady`.
    pub(super) fn fetch_attachments(mut self: Pin<&mut Self>, id: i64) {
        let _timed = crate::Timed::new("fetch_attachments");
        let Some(m) = self.db.get(id) else { return };
        if m.attachments.is_empty() {
            return;
        }
        let live = self.db.live_message(id);
        self.as_mut().set_status_text(QString::from("Adding the attachments…"));
        self.background(
            move || {
                let mut files = Vec::new();
                for name in &m.attachments {
                    if let Ok(p) = attachment_path(&m, live.clone(), name) {
                        files.push(p.display().to_string());
                    }
                }
                Ok(files)
            },
            move |mut store, files: Vec<String>| {
                if files.is_empty() {
                    store.as_mut().set_status_text(QString::from("The attachments aren't stored for this message."));
                }
                store.as_mut().set_attachments_ready(QString::from(json!({"id": id, "files": files}).to_string().as_str()));
            },
        );
    }

    pub(super) fn card_file(&self, id: i64) -> QString {
        let _timed = crate::Timed::new("card_file");
        let Some(c) = self.db.contacts.iter().find(|c| c.id == id) else { return QString::default() };
        let name = crate::data::display_name(c);
        let dir = std::env::temp_dir().join("almanac-cards");
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join(format!("{}.vcf", name.chars().filter(|ch| ch.is_alphanumeric() || *ch == ' ').collect::<String>().trim()));
        let uid = if c.uid.is_empty() { format!("card-{id}") } else { c.uid.clone() };
        match std::fs::write(&p, crate::ics::write_vcard(c, &uid, &c.raw)) {
            Ok(_) => QString::from(p.display().to_string().as_str()),
            Err(_) => QString::default(),
        }
    }

    pub(super) fn sign_in_providers(&self) -> QString {
        let _timed = crate::Timed::new("sign_in_providers");
        q(&crate::oauth::providers_json())
    }

    pub(super) fn sign_in_setup(&self) -> QString {
        let _timed = crate::Timed::new("sign_in_setup");
        q(&crate::oauth::setup_json())
    }

    /// Saves the app's registration with a provider once, for every account.
    pub(super) fn save_oauth_client(mut self: Pin<&mut Self>, provider: &QString, client_id: &QString, client_secret: &QString) {
        let _timed = crate::Timed::new("save_oauth_client");
        match crate::oauth::save_client(&provider.to_string(), &client_id.to_string(), &client_secret.to_string()) {
            Ok(()) => self.as_mut().set_status_text(QString::from("Saved. Sign-in is now one click.")),
            Err(e) => self.as_mut().set_status_text(QString::from(format!("Could not save: {e}").as_str())),
        }
        self.changed();
    }

    /// A code copied from the provider's page.
    pub(super) fn cancel_sign_in(mut self: Pin<&mut Self>) {
        let _timed = crate::Timed::new("cancel_sign_in");
        crate::oauth::cancel_sign_in();
        crate::browser::cancel();
        self.as_mut().set_signing_in(QString::default());
        self.as_mut().set_status_text(QString::from("Sign-in cancelled."));
    }

    pub(super) fn submit_sign_in_code(mut self: Pin<&mut Self>, code: &QString) {
        let _timed = crate::Timed::new("submit_sign_in_code");
        crate::oauth::paste_code(&code.to_string());
        self.as_mut().set_status_text(QString::from("Finishing sign-in…"));
    }

    // ------------------------------------------- automatic registration

    pub(super) fn setup_tools(&self) -> QString {
        let _timed = crate::Timed::new("setup_tools");
        q(&crate::autoreg::tools_json())
    }

    /// Microsoft: registers Almanac by itself, saves the ID and signs in.
    /// Google: makes the project and turns on its services, then opens the
    /// one page Google wants clicked by hand.
    pub(super) fn auto_register(mut self: Pin<&mut Self>, provider: &QString) {
        let _timed = crate::Timed::new("auto_register");
        let provider = provider.to_string();
        if !self.registering.to_string().is_empty() {
            return;
        }
        self.as_mut().set_registering(QString::from(provider.as_str()));
        let known_project = crate::oauth::load_extra("google_project");
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let t2 = thread.clone();
            let progress = move |m: String| {
                let _ = t2.queue(move |mut s| s.as_mut().set_status_text(QString::from(m.as_str())));
            };
            if provider == crate::oauth::MICROSOFT {
                let r = crate::autoreg::register_microsoft(&progress);
                let _ = thread.queue(move |mut store| {
                    store.as_mut().set_registering(QString::default());
                    match r {
                        Ok(id) => {
                            let _ = crate::oauth::save_client(crate::oauth::MICROSOFT, &id, "");
                            store.as_mut().set_status_text(QString::from("Almanac is registered with Microsoft. Opening the sign-in page…"));
                            store.as_mut().changed();
                            let p = QString::from(crate::oauth::MICROSOFT);
                            let empty = QString::default();
                            store.sign_in_with(&p, &empty, &empty, &QString::from("common"));
                        }
                        Err(e) => store.as_mut().set_status_text(QString::from(e.as_str())),
                    }
                });
            } else {
                let r = crate::autoreg::prepare_google(&known_project, &progress);
                let _ = thread.queue(move |mut store| {
                    store.as_mut().set_registering(QString::default());
                    match r {
                        Ok(project) => {
                            let (consent, _) = crate::autoreg::google_pages(&project);
                            let _ = crate::oauth::save_extra("google_project", &project);
                            store.as_mut().set_google_project(QString::from(project.as_str()));
                            store.as_mut().set_status_text(QString::from(
                                "Project ready. Finish the two short pages Google opens, then load the downloaded file.",
                            ));
                            store.as_mut().set_open_url(QString::from(consent.as_str()));
                        }
                        Err(e) => store.as_mut().set_status_text(QString::from(e.as_str())),
                    }
                });
            }
        });
    }

    pub(super) fn arg(&self, name: &QString) -> QString {
        let _timed = crate::Timed::new("arg");
        QString::from(crate::arg_value(&name.to_string()).unwrap_or_default().as_str())
    }

    pub(super) fn set_pdf_open(&self, id: i64, name: &QString, open: bool) {
        let name = name.to_string();
        if let Ok(mut o) = PDFS_OPEN.lock() {
            o.retain(|(i, n)| !(*i == id && *n == name));
            if open {
                o.push((id, name));
            }
        }
    }

    pub(super) fn attachment_previews(&self, id: i64) -> QString {
        let _timed = crate::Timed::new("attachment_previews");
        use base64::Engine;
        let Some(m) = self.db.get(id) else { return q(&json!([])) };
        let mut out = Vec::new();
        let mut missing = Vec::new();
        for name in &m.attachments {
            if !previewable(name) {
                continue;
            }
            let mut file = crate::mailstore::attachment_file(id, name);
            if file.is_none() && self.db.is_sample {
                if let Some(bytes) = crate::data::sample_attachment(name) {
                    file = crate::mailstore::store_attachment(id, name, &bytes).ok();
                }
            }
            match file {
                Some(p) => {
                    let too_big = std::fs::metadata(&p).map(|x| x.len() > 25_000_000).unwrap_or(true);
                    if too_big {
                        continue;
                    }
                    if let Ok(bytes) = std::fs::read(&p) {
                        match sniff_type(&bytes) {
                            // PDFs: their pages as pictures, in a box of the message's own
                            // that scrolls (its scroll bars show while it's scrolled,
                            // which the browser's PDF viewer's don't).
                            // Closed (the default): just its name, size and pages, nothing made.
                            Some("application/pdf") if !pdf_open(id, name) => out.push(json!({
                                "name": name, "mime": "application/pdf", "data": "", "collapsed": true, "size": bytes.len(),
                                "total": pdf_page_count(id, name, &bytes),
                            })),
                            Some("application/pdf") => match pdf_pages(id, name) {
                                Some((pages, total)) => out.push(json!({"name": name, "mime": "application/pdf", "data": "", "pages": pages, "total": total, "size": bytes.len()})),
                                None => {
                                    // (Its line shows meanwhile; the pages follow.)
                                    out.push(json!({"name": name, "mime": "application/pdf", "data": "", "pages": [], "making": true,
                                                    "size": bytes.len(), "total": pdf_page_count(id, name, &bytes)}));
                                    make_pdf_pages(self.qt_thread(), id, name.clone(), p.clone());
                                }
                            },
                            Some(mime) => out.push(json!({"name": name, "mime": mime, "data": base64::engine::general_purpose::STANDARD.encode(&bytes)})),
                            None => {}
                        }
                    }
                }
                None => {
                    // Still to come (the message redraws when it's here).
                    out.push(json!({"name": name, "pending": true}));
                    missing.push(name.clone())
                }
            }
        }
        if !missing.is_empty() {
            // Fetch them once in the background; the message redraws when they're here.
            static ASKED: std::sync::Mutex<Vec<i64>> = std::sync::Mutex::new(Vec::new());
            let first = {
                let mut asked = ASKED.lock().unwrap();
                let f = !asked.contains(&id);
                if f {
                    asked.push(id);
                }
                f
            };
            if first {
                let live = self.db.live_message(id);
                let thread = self.qt_thread();
                std::thread::spawn(move || {
                    let mut got = false;
                    for name in &missing {
                        got |= attachment_path(&m, live.clone(), name).is_ok();
                    }
                    if got {
                        let _ = thread.queue(|store| store.changed());
                    }
                });
            }
        }
        q(&Value::Array(out))
    }

    pub(super) fn is_server_mailbox(&self, selection: &QString) -> bool {
        let _timed = crate::Timed::new("is_server_mailbox");
        !self.db.live_folders_for(&selection.to_string()).is_empty()
    }

    pub(super) fn load_older(self: Pin<&mut Self>, selection: &QString) {
        let _timed = crate::Timed::new("load_older");
        let sel = selection.to_string();
        let targets: Vec<(String, String, LiveAccount, u32)> = self
            .db
            .live_folders_for(&sel)
            .into_iter()
            .map(|(fid, path, a)| {
                let low = self.db.lowest_uid(&fid);
                (fid, path, a, low)
            })
            .collect();
        self.fetch_more_mail(targets, String::new());
    }

    pub(super) fn search_on_server(self: Pin<&mut Self>, selection: &QString, query: &QString) {
        let _timed = crate::Timed::new("search_on_server");
        let q = query.to_string();
        if q.trim().is_empty() {
            return;
        }
        let targets: Vec<(String, String, LiveAccount, u32)> = self.db.live_folders_for(&selection.to_string()).into_iter().map(|(f, p, a)| (f, p, a, 0)).collect();
        self.fetch_more_mail(targets, q);
    }

    /// Older mail (no query) or server search results (a query): headers
    /// first, shown at once, then the text of each message.
    fn fetch_more_mail(mut self: Pin<&mut Self>, targets: Vec<(String, String, LiveAccount, u32)>, query: String) {
        let _timed = crate::Timed::new("fetch_more_mail");
        if targets.is_empty() {
            self.as_mut().set_status_text(QString::from("This mailbox isn't on a mail server, so everything is already here."));
            return;
        }
        self.as_mut().set_status_text(QString::from(if query.is_empty() { "Loading older messages…" } else { "Searching on the server…" }));
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let mut total = 0usize;
            let mut more_left = false;
            for (fid, path, acct, low) in targets {
                let got = if query.is_empty() {
                    let below = if low == 0 { u32::MAX } else { low };
                    crate::live::fetch_older(&acct, &path, &fid, below, 200)
                } else {
                    crate::live::search_server(&acct, &path, &fid, &query).map(|m| (m, false))
                };
                let (msgs, more) = match got {
                    Ok(x) => x,
                    Err(e) => {
                        let _ = thread.queue(move |mut s| s.as_mut().set_status_text(QString::from(format!("{}: {e}", acct.name).as_str())));
                        continue;
                    }
                };
                more_left |= more;
                let (tx, rx) = std::sync::mpsc::channel();
                let p2 = path.clone();
                let _ = thread.queue(move |mut store| {
                    let ids = store.as_mut().db_mut().add_messages(msgs);
                    let wanted: Vec<crate::live::Wanted> = ids
                        .iter()
                        .filter_map(|id| store.db.get(*id).map(|m| (m.id, p2.clone(), m.uid)))
                        .collect();
                    let _ = tx.send(wanted);
                    store.changed();
                });
                let wanted = rx.recv_timeout(std::time::Duration::from_secs(60)).unwrap_or_default();
                total += wanted.len();
                let t = thread.clone();
                let deliver = move |texts: Vec<crate::live::Text>| {
                    let _ = t.queue(move |mut store| {
                        if store.as_mut().db_mut().fill_texts(texts) {
                            store.changed();
                        }
                    });
                };
                let _ = crate::live::fetch_texts(&acct, &wanted, crate::live::Lane::Opened, &deliver, &|_| {});
            }
            let msg = if !query.is_empty() {
                match total {
                    0 => "Nothing more found on the server.".to_string(),
                    n => format!("Found {n} more on the server."),
                }
            } else if total == 0 {
                "There's no older mail in this mailbox.".to_string()
            } else {
                format!("Loaded {total} older messages{}", if more_left { "." } else { " (that's all of them)." })
            };
            let _ = thread.queue(move |mut s| s.as_mut().set_status_text(QString::from(msg.as_str())));
        });
    }

    pub(super) fn browser_name(&self) -> QString {
        let _timed = crate::Timed::new("browser_name");
        QString::from(crate::browser::find_browser().map(|(n, _)| n).unwrap_or_default().as_str())
    }

    /// Opens Almanac's own browser window and does the Google Cloud setup
    /// there, then signs in in that same window.
    pub(super) fn set_up_in_browser(mut self: Pin<&mut Self>, provider: &QString, own_browser: bool) {
        let _timed = crate::Timed::new("set_up_in_browser");
        let provider = provider.to_string();
        if provider != crate::oauth::GOOGLE || !self.registering.to_string().is_empty() {
            return;
        }
        self.as_mut().set_registering(QString::from(provider.as_str()));
        let known_project = crate::oauth::load_extra("google_project");
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let t2 = thread.clone();
            let progress = move |m: String| {
                let _ = t2.queue(move |mut s| s.as_mut().set_status_text(QString::from(m.as_str())));
            };
            let setup = match crate::browser::setup_google(&known_project, own_browser, &progress) {
                Ok(s) => s,
                Err(e) => {
                    let _ = thread.queue(move |mut store| {
                        store.as_mut().set_registering(QString::default());
                        store.as_mut().set_status_text(QString::from(e.as_str()));
                        store.as_mut().changed();
                    });
                    return;
                }
            };
            let _ = crate::oauth::save_client(crate::oauth::GOOGLE, &setup.client_id, &setup.client_secret);
            let project = setup.project.clone();
            let _ = thread.queue(move |mut store| {
                store.as_mut().set_google_project(QString::from(project.as_str()));
                store.as_mut().set_signing_in(QString::from(crate::oauth::GOOGLE));
                store.as_mut().set_status_text(QString::from("Almanac is registered with Google. Finish signing in in the browser window."));
                store.as_mut().changed();
            });
            // Sign in in the same window, so it's the account just added as a test user.
            let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            crate::browser::coach_sign_in(setup.page.clone(), setup.email.clone(), done.clone());
            let page = setup.page.clone();
            let t3 = thread.clone();
            let open = move |url: String| {
                if page.call("Page.navigate", serde_json::json!({"url": url})).is_err() {
                    let _ = t3.queue(move |mut s| s.as_mut().set_open_url(QString::from(url.as_str())));
                }
            };
            let r = crate::oauth::sign_in(crate::oauth::GOOGLE, &setup.client_id, &setup.client_secret, "common", &open);
            done.store(true, std::sync::atomic::Ordering::SeqCst);
            let (cid, secret) = (setup.client_id.clone(), setup.client_secret.clone());
            let _ = thread.queue(move |mut store| {
                store.as_mut().set_registering(QString::default());
                match r {
                    Err(e) if e == crate::oauth::SIGN_IN_STOPPED => {}
                    Err(e) => {
                        store.as_mut().set_signing_in(QString::default());
                        store.as_mut().set_status_text(QString::from(e.as_str()));
                    }
                    Ok(si) => {
                        store.as_mut().set_signing_in(QString::default());
                        store.add_signed_in(crate::oauth::GOOGLE.to_string(), cid, secret, "common".to_string(), si);
                    }
                }
            });
        });
    }

    pub(super) fn ai_prompt(&self, provider: &QString) -> QString {
        let _timed = crate::Timed::new("ai_prompt");
        QString::from(crate::autoreg::ai_prompt(&provider.to_string()).as_str())
    }

    /// Finds the IDs in pasted text or a downloaded file and saves them.
    pub(super) fn use_pasted_registration(mut self: Pin<&mut Self>, provider: &QString, text: &QString) -> bool {
        let _timed = crate::Timed::new("use_pasted_registration");
        let p = provider.to_string();
        match crate::autoreg::extract_ids(&p, &text.to_string()) {
            Some((id, secret)) => {
                let _ = crate::oauth::save_client(&p, &id, &secret);
                self.as_mut().set_status_text(QString::from("Found the app ID and saved it."));
                self.changed();
                true
            }
            None => {
                self.as_mut().set_status_text(QString::from("Couldn't find an app ID in that text."));
                false
            }
        }
    }

    pub(super) fn read_text_file(&self, url: &QString) -> QString {
        let _timed = crate::Timed::new("read_text_file");
        let path = url_to_path(&url.to_string());
        QString::from(std::fs::read_to_string(path).unwrap_or_default().as_str())
    }
}

/// Attachments shown inside the message (pictures and PDFs), by name.
fn previewable(name: &str) -> bool {
    let n = name.to_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".svg", ".pdf"].iter().any(|e| n.ends_with(e))
}

/// What a file really is, from its first bytes (names can lie).
fn sniff_type(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(b"%PDF") {
        Some("application/pdf")
    } else if b.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("image/png")
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if b.starts_with(b"GIF8") {
        Some("image/gif")
    } else if b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("image/webp")
    } else if b.starts_with(b"BM") {
        Some("image/bmp")
    } else if String::from_utf8_lossy(&b[..b.len().min(512)]).contains("<svg") {
        Some("image/svg+xml")
    } else {
        None
    }
}

/// Where one attachment is kept (attachments/<message id>/<name>). The
/// first time, it's fetched from wherever the message came from (the mail
/// server, Thunderbird's mailbox file, Mailspring's files) and saved there.
fn attachment_path(m: &crate::data::Message, live: Option<(LiveAccount, String, u32)>, wanted: &str) -> Result<std::path::PathBuf, String> {
    if let Some(p) = crate::mailstore::attachment_file(m.id, wanted) {
        return Ok(p);
    }
    if let Some(rest) = m.raw_ref.strip_prefix("msfiles:") {
        let mut lines = rest.lines();
        let dir = std::path::PathBuf::from(lines.next().unwrap_or(""));
        let fid = lines.filter_map(|l| l.split_once('\t')).find(|(n, _)| *n == wanted).map(|(_, i)| i.to_string()).ok_or("Attachment not found")?;
        let p = dir.join(&fid[0..2]).join(&fid[2..4]).join(&fid).join(wanted);
        let bytes = std::fs::read(&p).map_err(|_| "Mailspring hasn't downloaded this attachment yet".to_string())?;
        return crate::mailstore::store_attachment(m.id, wanted, &bytes);
    }
    // Keep all of the message's attachments while the original is at hand.
    let raw = original_message(m, live)?;
    crate::mailstore::store_attachments_from(m.id, &raw, &m.attachments);
    crate::mailstore::attachment_file(m.id, wanted).ok_or_else(|| "Attachment not found in the message".into())
}

/// The whole original message (Thunderbird mailbox file or mail server).
fn original_message(m: &crate::data::Message, live: Option<(LiveAccount, String, u32)>) -> Result<Vec<u8>, String> {
    let raw: Vec<u8> = if let Some(rest) = m.raw_ref.strip_prefix("mbox:") {
        let mut parts = rest.rsplitn(3, ':');
        let len: usize = parts.next().and_then(|x| x.parse().ok()).ok_or("bad ref")?;
        let off: u64 = parts.next().and_then(|x| x.parse().ok()).ok_or("bad ref")?;
        let path = parts.next().ok_or("bad ref")?;
        use std::io::{Read, Seek, SeekFrom};
        let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
        f.seek(SeekFrom::Start(off)).map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; len];
        f.read_exact(&mut buf).map_err(|e| e.to_string())?;
        buf
    } else if let Some((acct, path, uid)) = live {
        crate::live::fetch_full(&acct, &path, uid)?
    } else {
        return Err("This message has no stored attachment.".into());
    };
    Ok(raw)
}

#[cfg(test)]
mod notify_tests {
    /// The desktop takes a notification as Almanac sends them (icon, Open
    /// action, staying until closed); shown for 3 seconds, then closed:
    /// ALMANAC_NOTIFY_TEST=1 cargo test desktop_takes_notifications -- --ignored --nocapture
    #[test]
    #[ignore]
    fn desktop_takes_notifications() {
        if std::env::var("ALMANAC_NOTIFY_TEST").is_err() {
            return;
        }
        let icon = std::path::PathBuf::from(std::env::var("HOME").unwrap()).join(".local/share/almanac/icons/almanac.svg");
        let mut n = notify_rust::Notification::new();
        n.appname("Almanac").summary("Almanac test (closes by itself)").body("Checking notifications work").icon(&icon.display().to_string());
        n.action("default", "Open").timeout(notify_rust::Timeout::Never).urgency(notify_rust::Urgency::Critical);
        let shown = n.show().expect("the desktop took it");
        eprintln!("shown, id {}", shown.id());
        std::thread::sleep(std::time::Duration::from_secs(3));
        shown.close();
    }
}
