//! Mail rules, smart mailboxes, VIPs, blocked senders, muted conversations,
//! smart groups, duplicate cards, the outbox (Send Later / Undo Send),
//! reminders and calendar alerts.

use crate::data::*;
use chrono::{Duration, Local};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Condition {
    /// Mail: from, to, cc, subject, body, any, account, unread, flagged,
    /// attachments, days (received within N days), vip.
    /// Contacts: name, company, email, phone, address, notes, group, birthday_month.
    pub field: String,
    /// contains, not_contains, is, begins, ends
    #[serde(default)]
    pub op: String,
    #[serde(default)]
    pub value: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RuleAction {
    /// move, flag, read, delete, junk
    pub kind: String,
    /// Folder id for "move"
    #[serde(default)]
    pub value: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MailRule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub match_all: bool,
    pub conditions: Vec<Condition>,
    pub actions: Vec<RuleAction>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SmartMailbox {
    pub id: String,
    pub name: String,
    pub match_all: bool,
    pub conditions: Vec<Condition>,
    #[serde(default)]
    pub include_trash: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SmartGroup {
    pub id: String,
    pub name: String,
    pub match_all: bool,
    pub conditions: Vec<Condition>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Outgoing {
    pub id: i64,
    /// The compose payload (to, cc, subject, body, html, attachments, account…)
    pub payload: Value,
    /// Unix seconds when it goes out
    pub send_at: i64,
    /// True for "Send Later" (shown in the Outbox); false for the undo delay
    #[serde(default)]
    pub scheduled: bool,
}

fn text_match(hay: &str, c: &Condition) -> bool {
    let h = hay.to_lowercase();
    let v = c.value.to_lowercase();
    match c.op.as_str() {
        "not_contains" => !h.contains(&v),
        "is" => h.trim() == v.trim(),
        "begins" => h.starts_with(&v),
        "ends" => h.ends_with(&v),
        _ => h.contains(&v),
    }
}


impl Db {
    // --------------------------------------------------------- conditions

    pub fn message_matches(&self, m: &Message, c: &Condition) -> bool {
        match c.field.as_str() {
            "from" => text_match(&format!("{} {}", m.from_name, m.from_email), c),
            "to" => text_match(&m.to, c),
            "cc" => text_match(&m.cc, c),
            "subject" => text_match(&m.subject, c),
            "body" => text_match(&self.text_of(m.id).0, c),
            "any" => text_match(&format!("{} {} {} {} {}", m.from_name, m.from_email, m.to, m.subject, self.text_of(m.id).0), c),
            "account" => {
                let acct = self.folders.iter().find(|f| f.id == m.folder).map(|f| f.account.clone()).unwrap_or_default();
                let name = self.accounts.iter().find(|a| a.id == acct).map(|a| a.name.clone()).unwrap_or_default();
                text_match(&name, c)
            }
            "unread" => !m.read,
            "flagged" => m.flagged,
            "attachments" => !m.attachments.is_empty(),
            "vip" => self.is_vip(&m.from_email),
            "days" => {
                let days: i64 = c.value.trim().parse().unwrap_or(7);
                m.date >= (Local::now() - Duration::days(days)).timestamp()
            }
            _ => false,
        }
    }

    fn all_or_any<T>(&self, item: &T, conds: &[Condition], all: bool, f: impl Fn(&T, &Condition) -> bool) -> bool {
        if conds.is_empty() {
            return false;
        }
        if all {
            conds.iter().all(|c| f(item, c))
        } else {
            conds.iter().any(|c| f(item, c))
        }
    }

    pub fn in_smart_mailbox(&self, m: &Message, id: &str) -> bool {
        let Some(sm) = self.smart_mailboxes.iter().find(|s| s.id == id) else { return false };
        if !sm.include_trash && self.folders.iter().any(|f| f.id == m.folder && f.kind == "trash") {
            return false;
        }
        self.all_or_any(m, &sm.conditions, sm.match_all, |m, c| self.message_matches(m, c))
    }

    pub fn contact_matches(&self, ct: &Contact, c: &Condition) -> bool {
        match c.field.as_str() {
            "name" => text_match(&format!("{} {} {}", ct.first, ct.last, ct.nickname), c),
            "company" => text_match(&format!("{} {}", ct.company, ct.job_title), c),
            "email" => ct.emails.iter().any(|e| text_match(&e.value, c)),
            "phone" => ct.phones.iter().any(|p| text_match(&p.value, c)),
            "address" => text_match(&ct.address, c),
            "notes" => text_match(&ct.notes, c),
            "group" => ct.groups.iter().any(|g| {
                let name = self.groups.iter().find(|x| &x.id == g).map(|x| x.name.clone()).unwrap_or_default();
                text_match(&name, c)
            }),
            "birthday_month" => {
                let month = ct.birthday.get(5..7).and_then(|m| m.parse::<u32>().ok()).unwrap_or(0);
                c.value.trim().parse::<u32>().map(|v| v == month).unwrap_or(false)
            }
            _ => false,
        }
    }

    pub fn in_smart_group(&self, ct: &Contact, id: &str) -> bool {
        let Some(g) = self.smart_groups.iter().find(|s| s.id == id) else { return false };
        self.all_or_any(ct, &g.conditions, g.match_all, |x, c| self.contact_matches(x, c))
    }

    // ------------------------------------------------------- VIPs & blocks

    pub fn is_vip(&self, email: &str) -> bool {
        if self.vips.is_empty() {
            return false;
        }
        let e = email.trim().to_lowercase();
        !e.is_empty() && self.vips.iter().any(|v| v == &e)
    }

    pub fn toggle_vip(&mut self, email: &str) {
        let e = email.trim().to_lowercase();
        if e.is_empty() {
            return;
        }
        if self.vips.contains(&e) {
            self.vips.retain(|v| v != &e);
        } else {
            self.vips.push(e);
        }
    }

    pub fn toggle_blocked(&mut self, email: &str) {
        let e = email.trim().to_lowercase();
        if e.is_empty() {
            return;
        }
        if self.blocked.contains(&e) {
            self.blocked.retain(|v| v != &e);
        } else {
            self.blocked.push(e);
        }
    }

    pub fn toggle_muted(&mut self, message_id: i64) {
        let Some(m) = self.get(message_id) else { return };
        // (Mail in the database: muted by the message's own Message-ID, which
        // the conversation's later messages refer to.)
        let key = if self.backed && !m.message_id.trim().is_empty() {
            m.message_id.trim().trim_matches(['<', '>']).to_string()
        } else {
            self.conversation_of(m.id)
        };
        if self.muted.contains(&key) {
            self.muted.retain(|k| k != &key);
        } else {
            self.muted.push(key);
        }
    }

    pub fn is_muted(&self, m: &Message) -> bool {
        if self.muted.is_empty() {
            return false;
        }
        if self.backed {
            // Its own Message-ID or one it refers to is muted (no reading the
            // whole conversation for each row of the list).
            let mut ids: Vec<&str> = Vec::new();
            for s in [&m.message_id, &m.in_reply_to, &m.references] {
                for part in s.split('<').skip(1) {
                    ids.push(part.split('>').next().unwrap_or("").trim());
                }
                if !s.contains('<') && !s.trim().is_empty() {
                    ids.push(s.trim());
                }
            }
            return ids.iter().any(|id| !id.is_empty() && self.muted.iter().any(|k| k.eq_ignore_ascii_case(id)));
        }
        self.muted.contains(&self.conversation_of(m.id))
    }

    // --------------------------------------------------------------- rules

    /// What rules (and blocked senders / muted threads) do to newly
    /// arrived inbox messages. Returns the changes so live accounts can
    /// make them on the server too: (message id, action kind, value).
    pub fn apply_rules(&mut self, new_ids: &[i64]) -> Vec<(i64, String, String)> {
        let mut out = Vec::new();
        for &id in new_ids {
            let Some(m) = self.get(id) else { continue };
            let in_inbox = self.folders.iter().any(|f| f.id == m.folder && f.kind == "inbox");
            if !in_inbox {
                continue;
            }
            if self.blocked.contains(&m.from_email.to_lowercase()) {
                out.push((id, "delete".into(), String::new()));
                continue;
            }
            if self.is_muted(&m) {
                out.push((id, "read".into(), String::new()));
            }
            for r in self.rules.iter().filter(|r| r.enabled) {
                if self.all_or_any(&m, &r.conditions, r.match_all, |m, c| self.message_matches(m, c)) {
                    for a in &r.actions {
                        out.push((id, a.kind.clone(), a.value.clone()));
                    }
                }
            }
        }
        // Make the local changes now.
        for (id, kind, value) in &out {
            match kind.as_str() {
                "flag" => {
                    if let Some(m) = self.msg_mut(*id) {
                        m.flagged = true;
                    }
                }
                "read" => {
                    self.set_read(*id, true);
                }
                "move" => self.move_to_folder(*id, value),
                "junk" => self.move_to(*id, "junk"),
                "delete" => self.move_to(*id, "trash"),
                _ => {}
            }
        }
        out
    }

    pub fn rules_json(&self) -> Value {
        serde_json::to_value(&self.rules).unwrap_or(json!([]))
    }

    pub fn save_rule(&mut self, v: &Value) {
        let mut r: MailRule = serde_json::from_value(v.clone()).unwrap_or_default();
        if r.id.is_empty() {
            r.id = format!("rule{}", self.new_id());
        }
        if r.name.trim().is_empty() {
            r.name = "Rule".into();
        }
        match self.rules.iter_mut().find(|x| x.id == r.id) {
            Some(x) => *x = r,
            None => self.rules.push(r),
        }
    }

    pub fn delete_rule(&mut self, id: &str) {
        self.rules.retain(|r| r.id != id);
    }

    /// Runs every rule on the whole inbox now ("Apply Rules").
    pub fn apply_rules_to_inbox(&mut self) -> Vec<(i64, String, String)> {
        if self.backed {
            let ids: Vec<i64> = self.messages_of("kind:inbox").into_iter().map(|m| m.id).collect();
            return self.apply_rules(&ids);
        }
        let ids: Vec<i64> = self
            .messages
            .iter()
            .filter(|m| self.folders.iter().any(|f| f.id == m.folder && f.kind == "inbox"))
            .map(|m| m.id)
            .collect();
        self.apply_rules(&ids)
    }

    // ------------------------------------------------------ smart things

    pub fn save_smart_mailbox(&mut self, v: &Value) -> String {
        let mut s: SmartMailbox = serde_json::from_value(v.clone()).unwrap_or_default();
        if s.id.is_empty() {
            s.id = format!("sm{}", self.new_id());
        }
        if s.name.trim().is_empty() {
            s.name = "Smart Mailbox".into();
        }
        let id = s.id.clone();
        match self.smart_mailboxes.iter_mut().find(|x| x.id == s.id) {
            Some(x) => *x = s,
            None => self.smart_mailboxes.push(s),
        }
        id
    }

    pub fn delete_smart_mailbox(&mut self, id: &str) {
        self.smart_mailboxes.retain(|s| s.id != id);
    }

    pub fn save_smart_group(&mut self, v: &Value) -> String {
        let mut s: SmartGroup = serde_json::from_value(v.clone()).unwrap_or_default();
        if s.id.is_empty() {
            s.id = format!("smart-{}", self.new_id());
        }
        if s.name.trim().is_empty() {
            s.name = "Smart Group".into();
        }
        let id = s.id.clone();
        match self.smart_groups.iter_mut().find(|x| x.id == s.id) {
            Some(x) => *x = s,
            None => self.smart_groups.push(s),
        }
        id
    }

    pub fn delete_smart_group(&mut self, id: &str) {
        self.smart_groups.retain(|s| s.id != id);
    }

    pub fn delete_group(&mut self, id: &str) {
        self.groups.retain(|g| g.id != id);
        for c in self.contacts.iter_mut() {
            c.groups.retain(|g| g != id);
        }
    }

    pub fn rename_group(&mut self, id: &str, name: &str) {
        if let Some(g) = self.groups.iter_mut().find(|g| g.id == id) {
            if !name.trim().is_empty() {
                g.name = name.trim().into();
            }
        }
    }

    // ---------------------------------------------------------- duplicates

    /// Sets of cards that look like the same person (same name, or same
    /// email address).
    pub fn duplicates(&self) -> Vec<Vec<i64>> {
        let mut groups: Vec<Vec<i64>> = Vec::new();
        let mut seen: std::collections::HashMap<String, usize> = Default::default();
        for c in &self.contacts {
            let mut keys: Vec<String> = Vec::new();
            let name = format!("{} {}", c.first.trim(), c.last.trim()).trim().to_lowercase();
            if name.len() > 2 {
                keys.push(format!("n:{name}"));
            }
            for e in &c.emails {
                let v = e.value.trim().to_lowercase();
                if !v.is_empty() {
                    keys.push(format!("e:{v}"));
                }
            }
            let hit = keys.iter().find_map(|k| seen.get(k).copied());
            let gi = match hit {
                Some(i) => {
                    groups[i].push(c.id);
                    i
                }
                None => {
                    groups.push(vec![c.id]);
                    groups.len() - 1
                }
            };
            for k in keys {
                seen.entry(k).or_insert(gi);
            }
        }
        groups.into_iter().filter(|g| g.len() > 1).collect()
    }

    pub fn duplicates_json(&self) -> Value {
        Value::Array(
            self.duplicates()
                .into_iter()
                .map(|ids| {
                    let names: Vec<String> = ids
                        .iter()
                        .filter_map(|id| self.contacts.iter().find(|c| c.id == *id).map(display_name))
                        .collect();
                    json!({"ids": ids, "names": names})
                })
                .collect(),
        )
    }

    /// Merges cards into the first one; returns the removed cards (so
    /// server copies can be deleted).
    pub fn merge_contacts(&mut self, ids: &[i64]) -> Vec<Contact> {
        if ids.len() < 2 {
            return vec![];
        }
        let Some(pos) = self.contacts.iter().position(|c| c.id == ids[0]) else { return vec![] };
        let mut keep = self.contacts[pos].clone();
        let mut removed = Vec::new();
        for id in &ids[1..] {
            let Some(o) = self.contacts.iter().find(|c| c.id == *id).cloned() else { continue };
            let fill = |a: &mut String, b: &str| {
                if a.trim().is_empty() {
                    *a = b.to_string();
                }
            };
            fill(&mut keep.first, &o.first);
            fill(&mut keep.last, &o.last);
            fill(&mut keep.company, &o.company);
            fill(&mut keep.job_title, &o.job_title);
            fill(&mut keep.address, &o.address);
            fill(&mut keep.birthday, &o.birthday);
            fill(&mut keep.nickname, &o.nickname);
            fill(&mut keep.photo, &o.photo);
            if !o.notes.trim().is_empty() && !keep.notes.contains(o.notes.trim()) {
                keep.notes = format!("{}\n{}", keep.notes, o.notes).trim().to_string();
            }
            for e in &o.emails {
                if !keep.emails.iter().any(|x| x.value.eq_ignore_ascii_case(&e.value)) {
                    keep.emails.push(e.clone());
                }
            }
            for p in &o.phones {
                let digits = |s: &str| s.chars().filter(|c| c.is_ascii_digit()).collect::<String>();
                if !keep.phones.iter().any(|x| digits(&x.value) == digits(&p.value)) {
                    keep.phones.push(p.clone());
                }
            }
            for u in &o.urls {
                if !keep.urls.iter().any(|x| x.value == u.value) {
                    keep.urls.push(u.clone());
                }
            }
            for g in &o.groups {
                if !keep.groups.contains(g) {
                    keep.groups.push(g.clone());
                }
            }
            removed.push(o);
        }
        self.contacts.retain(|c| !ids[1..].contains(&c.id));
        if let Some(c) = self.contacts.iter_mut().find(|c| c.id == ids[0]) {
            *c = keep;
        }
        removed
    }

    // --------------------------------------------------------- signatures

    pub fn signature(&self, account: &str) -> String {
        self.signatures.iter().find(|s| s.label == account).map(|s| s.value.clone()).unwrap_or_default()
    }

    pub fn set_signature(&mut self, account: &str, text: &str) {
        match self.signatures.iter_mut().find(|s| s.label == account) {
            Some(s) => s.value = text.to_string(),
            None => self.signatures.push(Labeled { label: account.into(), value: text.into() }),
        }
    }

    // ------------------------------------------------------------- outbox

    pub fn queue_outgoing(&mut self, payload: Value, send_at: i64, scheduled: bool) -> i64 {
        let id = self.new_id();
        self.outbox.push(Outgoing { id, payload, send_at, scheduled });
        id
    }

    /// Takes out what is due now.
    pub fn take_due_outgoing(&mut self) -> Vec<Outgoing> {
        let now = Local::now().timestamp();
        let (due, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut self.outbox).into_iter().partition(|o| o.send_at <= now);
        self.outbox = rest;
        due
    }

    pub fn cancel_outgoing(&mut self, id: i64) -> Option<Outgoing> {
        let pos = self.outbox.iter().position(|o| o.id == id)?;
        Some(self.outbox.remove(pos))
    }

    pub fn outbox_json(&self) -> Value {
        Value::Array(
            self.outbox
                .iter()
                .map(|o| {
                    json!({"id": o.id, "sendAt": o.send_at * 1000, "scheduled": o.scheduled,
                           "to": o.payload["to"], "subject": o.payload["subject"]})
                })
                .collect(),
        )
    }

    // ----------------------------------------------------------- reminders

    pub fn remind_me(&mut self, id: i64, at: i64) {
        if at > 0 {
            self.next_reminder_look = self.next_reminder_look.min(at);
        }
        if let Some(m) = self.msg_mut(id) {
            m.remind_at = at;
        }
    }

    /// Messages whose reminder time has come: they become unread and
    /// rise to the top. Returns (id, sender, subject) for a notification.
    pub fn due_reminders(&mut self) -> Vec<(i64, String, String)> {
        let now = Local::now().timestamp();
        let mut out = Vec::new();
        // (Mail in the database: those due are read into memory first, in the
        // background, see `reminders_in_database`.)
        for m in self.messages.iter_mut().filter(|m| m.remind_at > 0 && m.remind_at <= now) {
            m.remind_at = 0;
            m.read = false;
            m.date = now;
            out.push((m.id, if m.from_name.is_empty() { m.from_email.clone() } else { m.from_name.clone() }, m.subject.clone()));
        }
        out
    }

    // -------------------------------------------------------------- alerts

    /// Calendar alerts that are due now and not shown yet: (title, when
    /// text, event id, start of this occurrence).
    /// Whether the mail in the database should be looked at for reminders now
    /// (then not again for ten minutes, or until `reminders_found` says when).
    pub fn reminders_to_look_for(&mut self) -> bool {
        let now = Local::now().timestamp();
        if !self.backed || now < self.next_reminder_look {
            return false;
        }
        self.next_reminder_look = now + 600;
        true
    }

    /// Mail in the database with a reminder due (ids), and when the next one is
    /// (on any thread: a scan of the mail, a fraction of a second on a big mailbox).
    pub fn reminders_in_database() -> (Vec<i64>, Option<i64>) {
        let now = Local::now().timestamp();
        let due = crate::mailstore::rows("remind_at > 0 AND remind_at <= ?1", &[crate::mailstore::Arg::Integer(now)], "date DESC", 1000, 0);
        let next = crate::mailstore::rows("remind_at > ?1", &[crate::mailstore::Arg::Integer(now)], "remind_at ASC", 1, 0);
        (due.into_iter().map(|m| m.id).collect(), next.first().map(|m| m.remind_at))
    }

    /// What `reminders_in_database` found: those due into memory (shown at the
    /// next look), and the next one looked for then (or in ten minutes).
    pub fn reminders_found(&mut self, due: Vec<i64>, next: Option<i64>) -> bool {
        let now = Local::now().timestamp();
        for id in &due {
            self.msg_mut(*id);
        }
        self.next_reminder_look = next.unwrap_or(i64::MAX).min(now + 600);
        !due.is_empty()
    }

    pub fn due_alerts(&mut self) -> Vec<(String, String, i64, String)> {
        let now = Local::now().naive_local();
        let from = now - Duration::days(2);
        let to = now + Duration::days(8);
        let Value::Array(list) = self.events(&fmt_dt(from), &fmt_dt(to)) else { return vec![] };
        let mut out = Vec::new();
        for e in list {
            let id = e["id"].as_i64().unwrap_or(0);
            let Some(ev) = self.events.iter().find(|x| x.id == id) else { continue };
            if ev.alarms.is_empty() {
                continue;
            }
            let Some(start) = parse_dt(e["start"].as_str().unwrap_or("")) else { continue };
            for &mins in &ev.alarms {
                let at = start - Duration::minutes(mins);
                let key = format!("{id}@{}@{mins}", fmt_dt(start));
                // Due within the last 10 minutes and not shown before.
                if at <= now && at > now - Duration::minutes(10) && !self.fired_alerts.contains(&key) {
                    self.fired_alerts.push(key);
                    let when = alert_when(start, now, ev.all_day);
                    let place = if ev.location.is_empty() { String::new() } else { format!(" · {}", ev.location) };
                    out.push((ev.title.clone(), format!("{when}{place}"), id, fmt_dt(start)));
                }
            }
        }
        if self.fired_alerts.len() > 500 {
            let n = self.fired_alerts.len() - 500;
            self.fired_alerts.drain(..n);
        }
        out
    }
}

/// An alert's "when" text, saying which day the event is on when it isn't
/// today (a reminder set a day or more ahead): "Tomorrow at 08:15",
/// "Wed 11 Nov", "at 14:00 (in 30 min)", "Today".
pub fn alert_when(start: chrono::NaiveDateTime, now: chrono::NaiveDateTime, all_day: bool) -> String {
    let days = (start.date() - now.date()).num_days();
    let day = match days {
        ..=0 => String::new(),
        1 => "Tomorrow".to_string(),
        2..=6 => start.format("%A").to_string(),
        _ => start.format("%a %-d %b").to_string(),
    };
    if all_day {
        return if day.is_empty() { "Today".to_string() } else { day };
    }
    let time = start.format("%H:%M");
    let left = (start - now).num_minutes().max(0);
    if left == 0 {
        return format!("Now ({time})");
    }
    let at = if day.is_empty() { format!("At {time}") } else { format!("{day} at {time}") };
    format!("{at} (in {})", span(left))
}

/// A wait in minutes as its two largest parts: "45 min", "2 h 5 min",
/// "1 day 3 h", "2 weeks 1 day", "1 month 2 weeks" (a month is 30 days).
pub fn span(minutes: i64) -> String {
    let units: [(i64, &str, &str); 5] =
        [(30 * 1440, "month", "months"), (7 * 1440, "week", "weeks"), (1440, "day", "days"), (60, "h", "h"), (1, "min", "min")];
    let mut rest = minutes.max(0);
    let mut parts = Vec::new();
    for (size, one, many) in units {
        let n = rest / size;
        if n > 0 {
            parts.push(format!("{n} {}", if n == 1 { one } else { many }));
            rest -= n * size;
        } else if !parts.is_empty() {
            // Only next to each other: "1 day 5 min" would read oddly.
            break;
        }
        if parts.len() == 2 {
            break;
        }
    }
    if parts.is_empty() { "0 min".into() } else { parts.join(" ") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alert_says_which_day() {
        let dt = |s: &str| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M").unwrap();
        let now = dt("2026-11-09T08:15");
        assert_eq!(alert_when(dt("2026-11-10T08:15"), now, false), "Tomorrow at 08:15 (in 1 day)");
        assert_eq!(alert_when(dt("2026-11-10T08:14"), now, false), "Tomorrow at 08:14 (in 23 h 59 min)");
        assert_eq!(span(1439), "23 h 59 min");
        assert_eq!(span(1440 + 180), "1 day 3 h");
        assert_eq!(span(1440 + 5), "1 day");
        assert_eq!(span(15 * 1440), "2 weeks 1 day");
        assert_eq!(span(45 * 1440), "1 month 2 weeks");
        assert_eq!(span(45), "45 min");
        assert_eq!(alert_when(dt("2026-11-09T08:45"), now, false), "At 08:45 (in 30 min)");
        assert_eq!(alert_when(dt("2026-11-09T10:15"), now, false), "At 10:15 (in 2 h)");
        assert_eq!(alert_when(dt("2026-11-09T08:15"), now, false), "Now (08:15)");
        assert_eq!(alert_when(dt("2026-11-11T00:00"), now, true), "Wednesday");
        assert_eq!(alert_when(dt("2026-11-10T00:00"), dt("2026-11-09T22:00"), true), "Tomorrow");
        assert_eq!(alert_when(dt("2026-11-09T00:00"), now, true), "Today");
        assert_eq!(alert_when(dt("2026-11-20T09:00"), now, false), "Fri 20 Nov at 09:00 (in 1 week 4 days)");
    }

    #[test]
    fn rule_moves_and_flags_new_mail() {
        let mut db = Db::sample();
        let id = db.messages.iter().find(|m| m.subject == "Weekend hike?").unwrap().id;
        db.rules.push(MailRule {
            id: "r".into(),
            name: "Sam".into(),
            enabled: true,
            match_all: true,
            conditions: vec![Condition { field: "from".into(), op: "contains".into(), value: "sam@".into() }],
            actions: vec![RuleAction { kind: "flag".into(), value: String::new() }, RuleAction { kind: "move".into(), value: "local/archive".into() }],
        });
        let ops = db.apply_rules(&[id]);
        assert_eq!(ops.len(), 2);
        let m = db.messages.iter().find(|m| m.id == id).unwrap();
        assert!(m.flagged);
        assert_eq!(m.folder, "local/archive");
    }

    #[test]
    fn finds_and_merges_duplicates() {
        let mut db = Db::sample();
        let mut copy = db.contacts.iter().find(|c| c.first == "Alex").unwrap().clone();
        copy.id = 9999;
        copy.phones = vec![Labeled { label: "work".into(), value: "+1 555 9999".into() }];
        db.contacts.push(copy);
        let sets = db.duplicates();
        assert_eq!(sets.len(), 1);
        db.merge_contacts(&sets[0]);
        let alex: Vec<&Contact> = db.contacts.iter().filter(|c| c.first == "Alex").collect();
        assert_eq!(alex.len(), 1);
        assert_eq!(alex[0].phones.len(), 2);
    }

    #[test]
    fn changed_occurrence_round_trip() {
        let ics = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:s\r\nDTSTART;TZID=Europe/Berlin:20260105T090000\r\nDTEND;TZID=Europe/Berlin:20260105T093000\r\nRRULE:FREQ=WEEKLY\r\nSUMMARY:Sync\r\nBEGIN:VALARM\r\nTRIGGER:-PT10M\r\nACTION:DISPLAY\r\nEND:VALARM\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:s\r\nRECURRENCE-ID;TZID=Europe/Berlin:20260112T090000\r\nDTSTART;TZID=Europe/Berlin:20260112T110000\r\nDTEND;TZID=Europe/Berlin:20260112T113000\r\nSUMMARY:Sync (moved)\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let evs = events_from_resource(ics, "c", "local");
        assert_eq!(evs.len(), 2);
        let master = evs.iter().find(|e| e.recurrence_id.is_empty()).unwrap();
        assert_eq!(master.alarms, vec![10]);
        assert_eq!(master.exdates, vec!["2026-01-12T09:00".to_string()]);
        let ov = evs.iter().find(|e| !e.recurrence_id.is_empty()).unwrap();
        assert_eq!(ov.recurrence_id, "2026-01-12T09:00");
        let text = crate::live::resource_text(master, &[ov.clone()]).unwrap();
        assert!(text.contains("RECURRENCE-ID;TZID=Europe/Berlin:20260112T090000"));
        assert!(text.contains("TRIGGER:-PT10M"));
        assert_eq!(text.matches("BEGIN:VEVENT").count(), 2);
    }
}
