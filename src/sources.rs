//! Readers for data kept by other mail apps on this computer:
//! Thunderbird profiles (mbox folders, address books, local calendars) and
//! Mailspring (its sync engine's SQLite database).
//!
//! Imports are read-only: nothing is written back to those apps.

use crate::data::*;
use crate::{ics, mime};
use chrono::{TimeZone, Utc};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const MAX_PER_FOLDER: usize = 400;
const TAIL_BYTES: u64 = 12 * 1024 * 1024;

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_default()
}

fn appdata() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(PathBuf::from)
}

/// Copies an SQLite file (and its write-ahead log) somewhere private so it
/// can be read while the other app is running.
fn open_copy(path: &Path) -> Option<(rusqlite::Connection, PathBuf)> {
    if !path.exists() {
        return None;
    }
    let dir = std::env::temp_dir().join(format!("almanac-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).ok()?;
    let name = path.file_name()?.to_string_lossy().to_string();
    let dest = dir.join(&name);
    std::fs::copy(path, &dest).ok()?;
    for ext in ["-wal", "-shm"] {
        let side = PathBuf::from(format!("{}{}", path.display(), ext));
        let dside = PathBuf::from(format!("{}{}", dest.display(), ext));
        let _ = std::fs::remove_file(&dside);
        if side.exists() && ext == "-wal" {
            let _ = std::fs::copy(&side, &dside);
        }
    }
    let conn = rusqlite::Connection::open(&dest).ok()?;
    Some((conn, dest))
}

fn cleanup(dest: &Path) {
    let _ = std::fs::remove_file(dest);
    let _ = std::fs::remove_file(format!("{}-wal", dest.display()));
    let _ = std::fs::remove_file(format!("{}-shm", dest.display()));
}

fn color_for(i: usize) -> String {
    const COLORS: [&str; 8] = ["#2f7cf6", "#e8453c", "#34a853", "#f5a623", "#9b59d0", "#1abc9c", "#e67e22", "#7f8c8d"];
    COLORS[i % COLORS.len()].to_string()
}

// =================================================================== Thunderbird

pub fn thunderbird_profile() -> Option<PathBuf> {
    let h = home();
    let mut bases = vec![
        h.join(".thunderbird"),
        h.join(".var/app/org.mozilla.Thunderbird/.thunderbird"),
        h.join("snap/thunderbird/common/.thunderbird"),
        h.join("Library/Thunderbird"),
    ];
    if let Some(a) = appdata() {
        bases.insert(0, a.join("Thunderbird"));
    }
    for base in bases {
        let ini = base.join("profiles.ini");
        let Ok(text) = std::fs::read_to_string(&ini) else { continue };
        let mut install_default: Option<String> = None;
        let mut profiles: Vec<(String, bool, bool)> = Vec::new(); // path, relative, default
        let mut section = String::new();
        let (mut path, mut rel, mut def) = (String::new(), true, false);
        let flush = |section: &str, path: &mut String, rel: bool, def: bool, profiles: &mut Vec<(String, bool, bool)>| {
            if section.starts_with("Profile") && !path.is_empty() {
                profiles.push((std::mem::take(path), rel, def));
            }
        };
        for line in text.lines().map(str::trim) {
            if line.starts_with('[') {
                flush(&section, &mut path, rel, def, &mut profiles);
                section = line.trim_matches(|c| c == '[' || c == ']').to_string();
                path.clear();
                rel = true;
                def = false;
            } else if let Some((k, v)) = line.split_once('=') {
                match k {
                    "Path" => path = v.to_string(),
                    "IsRelative" => rel = v == "1",
                    "Default" if section.starts_with("Install") => install_default = Some(v.to_string()),
                    "Default" => def = v == "1",
                    _ => {}
                }
            }
        }
        flush(&section, &mut path, rel, def, &mut profiles);
        let resolve = |p: &str, rel: bool| if rel { base.join(p) } else { PathBuf::from(p) };
        if let Some(d) = install_default {
            let p = base.join(&d);
            if p.exists() {
                return Some(p);
            }
        }
        if let Some((p, r, _)) = profiles.iter().find(|x| x.2) {
            return Some(resolve(p, *r));
        }
        if let Some((p, r, _)) = profiles.first() {
            return Some(resolve(p, *r));
        }
    }
    None
}

/// user_pref("key", value); lines of prefs.js
fn read_prefs(profile: &Path) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let Ok(text) = std::fs::read_to_string(profile.join("prefs.js")) else { return out };
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix("user_pref(\"") else { continue };
        let Some(kend) = rest.find("\",") else { continue };
        let key = rest[..kend].to_string();
        let val = rest[kend + 2..].trim().trim_end_matches(';').trim().trim_end_matches(')').trim();
        let val = if val.starts_with('"') && val.ends_with('"') && val.len() >= 2 {
            val[1..val.len() - 1].replace("\\\"", "\"").replace("\\\\", "\\")
        } else {
            val.to_string()
        };
        out.insert(key, val);
    }
    out
}

fn read_tail(path: &Path) -> Option<(Vec<u8>, bool, u64)> {
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let start = len.saturating_sub(TAIL_BYTES);
    f.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    Some((buf, start > 0, start))
}

/// Splits mbox text into messages (without the "From " separator line).
fn mbox_messages(buf: &[u8], partial: bool) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    if !partial && buf.starts_with(b"From ") {
        starts.push(0);
    }
    let mut i = 0;
    while i + 6 < buf.len() {
        if buf[i] == b'\n' && &buf[i + 1..i + 6] == b"From " {
            starts.push(i + 1);
        }
        i += 1;
    }
    let mut out = Vec::new();
    for (n, &s) in starts.iter().enumerate() {
        let e = starts.get(n + 1).copied().unwrap_or(buf.len());
        let chunk = &buf[s..e];
        let body_start = chunk.iter().position(|&c| c == b'\n').map(|p| p + 1).unwrap_or(chunk.len());
        out.push(&chunk[body_start..]);
    }
    out
}

fn walk_mbox(dir: &Path, rel: &str, out: &mut Vec<(PathBuf, String)>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if p.is_dir() {
            if let Some(base) = name.strip_suffix(".sbd") {
                let sub = if rel.is_empty() { base.to_string() } else { format!("{rel}/{base}") };
                walk_mbox(&p, &sub, out);
            }
        } else if !name.contains('.') || PathBuf::from(format!("{}.msf", p.display())).exists() {
            if name.ends_with(".msf") || name.ends_with(".dat") || name.ends_with(".json") {
                continue;
            }
            let full = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
            out.push((p, full));
        }
    }
}

fn pretty_folder(rel: &str) -> String {
    let last = rel.rsplit('/').next().unwrap_or(rel);
    if last.eq_ignore_ascii_case("inbox") {
        "Inbox".into()
    } else {
        rel.trim_start_matches("[Gmail]/").trim_start_matches("[Google Mail]/").to_string()
    }
}

pub fn import_thunderbird(profile: &Path, progress: &dyn Fn(String)) -> Result<Imported, String> {
    let src = "thunderbird";
    let mut imp = Imported { source: src.into(), ..Default::default() };
    let prefs = read_prefs(profile);

    // ---- mail
    let mut servers: Vec<(String, String, String)> = Vec::new(); // dir, name, email
    for (k, v) in &prefs {
        if let Some(key) = k.strip_prefix("mail.server.").and_then(|r| r.strip_suffix(".directory-rel")) {
            let dir = v.replace("[ProfD]", "");
            let name = prefs.get(&format!("mail.server.{key}.name")).cloned().unwrap_or_else(|| dir.clone());
            let user = prefs.get(&format!("mail.server.{key}.userName")).cloned().unwrap_or_default();
            servers.push((dir, name, user));
        }
    }
    if servers.is_empty() {
        for top in ["ImapMail", "Mail"] {
            if let Ok(rd) = std::fs::read_dir(profile.join(top)) {
                for e in rd.flatten().filter(|e| e.path().is_dir()) {
                    let n = e.file_name().to_string_lossy().to_string();
                    servers.push((format!("{top}/{n}"), n, String::new()));
                }
            }
        }
    }
    servers.sort();
    for (dir, name, user) in servers {
        let path = profile.join(&dir);
        if !path.is_dir() {
            continue;
        }
        let mut files = Vec::new();
        walk_mbox(&path, "", &mut files);
        if files.is_empty() {
            continue;
        }
        let account = format!("{src}:{dir}");
        imp.accounts.push(Account {
            id: account.clone(),
            name: name.clone(),
            email: user,
            source: src.into(),
        });
        for (file, rel) in files {
            progress(format!("Reading Thunderbird mail: {name} / {rel}"));
            let folder_id = format!("{account}/{rel}");
            let kind = guess_kind(&rel);
            imp.folders.push(Folder {
                id: folder_id.clone(),
                account: account.clone(),
                name: pretty_folder(&rel),
                kind: kind.into(),
                ..Default::default()
            });
            let Some((buf, partial, file_start)) = read_tail(&file) else { continue };
            let msgs = mbox_messages(&buf, partial);
            let skip = msgs.len().saturating_sub(MAX_PER_FOLDER * 2);
            let mut got = Vec::new();
            for raw in msgs.into_iter().skip(skip) {
                // Where this message sits in the mbox file, to read attachments later.
                let offset = file_start + (raw.as_ptr() as usize - buf.as_ptr() as usize) as u64;
                let raw_ref = format!("mbox:{}:{}:{}", file.display(), offset, raw.len());
                let p = mime::parse(raw);
                let status = p.mozilla_status.unwrap_or(0);
                if status & 0x0008 != 0 {
                    continue; // deleted
                }
                got.push(Message {
                    id: 0,
                    folder: folder_id.clone(),
                    from_name: p.from_name,
                    from_email: p.from_email,
                    to: p.to,
                    cc: p.cc,
                    subject: p.subject,
                    body: p.body,
                    date: p.date,
                    read: status & 0x0001 != 0 || p.mozilla_status.is_none(),
                    flagged: status & 0x0004 != 0,
                    source: src.into(),
                    attachments: p.attachments,
                    html: p.html,
                    invite: p.invite,
                    message_id: p.message_id,
                    in_reply_to: p.in_reply_to,
                    references: p.references,
                    reply_to: p.reply_to,
                    raw_ref,
                    ..Default::default()
                });
            }
            let skip = got.len().saturating_sub(MAX_PER_FOLDER);
            imp.messages.extend(got.into_iter().skip(skip));
        }
    }

    // ---- calendars
    progress("Reading Thunderbird calendars…".into());
    let mut cal_names: HashMap<String, (String, String, bool)> = HashMap::new();
    for (k, v) in &prefs {
        if let Some(rest) = k.strip_prefix("calendar.registry.") {
            if let Some((id, field)) = rest.split_once('.') {
                let e = cal_names.entry(id.to_string()).or_insert((String::new(), String::new(), false));
                match field {
                    "name" => e.0 = v.clone(),
                    "color" => e.1 = v.clone(),
                    "disabled" => e.2 = v == "true",
                    _ => {}
                }
            }
        }
    }
    let mut seen_cals: Vec<String> = Vec::new();
    for file in ["calendar-data/local.sqlite", "calendar-data/cache.sqlite"] {
        let Some((conn, dest)) = open_copy(&profile.join(file)) else { continue };
        let props = |item: &str, cal: &str, key: &str| -> String {
            conn.query_row(
                "SELECT value FROM cal_properties WHERE item_id=?1 AND cal_id=?2 AND key=?3",
                [item, cal, key],
                |r| r.get::<_, String>(0),
            )
            .unwrap_or_default()
        };
        // Repeat rule and skipped dates (EXDATE lines) of a series.
        let recurrence = |item: &str, cal: &str| -> (String, Vec<String>) {
            let mut rule = String::new();
            let mut ex = Vec::new();
            if let Ok(mut st) = conn.prepare("SELECT icalString FROM cal_recurrence WHERE item_id=?1 AND cal_id=?2") {
                if let Ok(rows) = st.query_map([item, cal], |r| r.get::<_, String>(0)) {
                    for s in rows.flatten() {
                        let s = s.trim();
                        if let Some(r) = s.strip_prefix("RRULE:") {
                            if rule.is_empty() {
                                rule = r.trim().to_string();
                            }
                        } else if s.starts_with("EXDATE") {
                            ex.push(s.to_string());
                        }
                    }
                }
            }
            (rule, ex)
        };
        // Thunderbird keeps times as UTC microseconds plus the zone name.
        let wall = |us: i64, zone: &str, all_day: bool| {
            let utc = Utc
                .timestamp_opt(us.div_euclid(1_000_000), 0)
                .single()
                .map(|d| d.naive_utc())
                .unwrap_or_default();
            if all_day || zone == "floating" || zone.is_empty() {
                utc
            } else {
                crate::tz::utc_to_wall(utc, zone)
            }
        };
        type Row = (String, String, String, i64, i64, String, i64, Option<i64>);
        let rows: Vec<Row> = conn
            .prepare(
                "SELECT cal_id, id, COALESCE(title,''), event_start, COALESCE(event_end, event_start), \
                 COALESCE(event_stzone,''), COALESCE(flags,0), recurrence_id FROM cal_events",
            )
            .and_then(|mut st| {
                st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)))
                    .map(|it| it.flatten().collect())
            })
            .unwrap_or_default();
        // Changed single occurrences: (series id, cal) → their original times.
        let mut changed: HashMap<(String, String), Vec<i64>> = HashMap::new();
        for r in rows.iter().filter(|r| r.7.is_some()) {
            changed.entry((r.1.clone(), r.0.clone())).or_default().push(r.7.unwrap_or(0));
        }
        for (cal, id, title, s, e, stzone, flags, rid) in rows {
            let all_day = flags & 8 != 0;
            let zone = if all_day || stzone == "floating" { String::new() } else { crate::tz::canonical(&stzone) };
            let zone_ref = if zone.is_empty() && !all_day && stzone != "floating" { "UTC" } else { zone.as_str() };
            let zone_store = if zone_ref == "UTC" && zone.is_empty() { "UTC".to_string() } else { zone.clone() };
            let (rule, ex_lines) = if rid.is_none() { recurrence(&id, &cal) } else { (String::new(), vec![]) };
            let mut exdates: Vec<String> = Vec::new();
            for l in ex_lines {
                // "EXDATE;TZID=…:20260112T090000" or "EXDATE:20260112T080000Z"
                let fake = format!("BEGIN:VEVENT\nDTSTART:20000101T000000Z\n{l}\nEND:VEVENT\n");
                if let Some(ev) = ics::parse_vevents(&fake).into_iter().next() {
                    for x in ev.exdates {
                        if let Some(t) = parse_dt(&x) {
                            exdates.push(fmt_dt(crate::tz::utc_to_wall(t, zone_ref)));
                        }
                    }
                }
            }
            if let Some(times) = changed.get(&(id.clone(), cal.clone())) {
                if rid.is_none() {
                    for us in times {
                        exdates.push(fmt_dt(wall(*us, zone_ref, all_day)));
                    }
                }
            }
            let start_wall = wall(s, zone_ref, all_day);
            let end_wall = wall(e, zone_ref, all_day);
            let cal_id = format!("{src}:{cal}");
            if !seen_cals.contains(&cal_id) {
                let (name, color, disabled) = cal_names.get(&cal).cloned().unwrap_or_default();
                imp.calendars.push(Calendar {
                    id: cal_id.clone(),
                    name: if name.is_empty() { "Thunderbird".into() } else { name },
                    color: if color.is_empty() { color_for(seen_cals.len() + 3) } else { color },
                    visible: !disabled,
                    source: src.into(),
                    ..Default::default()
                });
                seen_cals.push(cal_id.clone());
            }
            imp.events.push(Event {
                id: 0,
                calendar: cal_id,
                title,
                start: fmt_dt(start_wall),
                end: fmt_dt(end_wall),
                all_day,
                location: props(&id, &cal, "LOCATION"),
                notes: props(&id, &cal, "DESCRIPTION"),
                rrule: rule,
                source: src.into(),
                tz: zone_store,
                exdates,
                uid: id.clone(),
                ..Default::default()
            });
        }
        drop(conn);
        cleanup(&dest);
    }

    // ---- address books
    progress("Reading Thunderbird address books…".into());
    let mut books: Vec<(String, String)> = Vec::new(); // file, name
    for (k, v) in &prefs {
        if let Some(key) = k.strip_prefix("ldap_2.servers.").and_then(|r| r.strip_suffix(".filename")) {
            if v.ends_with(".sqlite") {
                let name = prefs.get(&format!("ldap_2.servers.{key}.description")).cloned().unwrap_or_else(|| v.clone());
                books.push((v.clone(), name));
            }
        }
    }
    for (f, n) in [("abook.sqlite", "Personal Address Book"), ("history.sqlite", "Collected Addresses")] {
        if !books.iter().any(|b| b.0 == f) {
            books.push((f.into(), n.into()));
        }
    }
    books.sort();
    for (file, name) in books {
        let Some((conn, dest)) = open_copy(&profile.join(&file)) else { continue };
        let group_id = format!("{src}:{file}");
        let mut cards: HashMap<String, HashMap<String, String>> = HashMap::new();
        if let Ok(mut st) = conn.prepare("SELECT card, name, value FROM properties") {
            if let Ok(rows) = st.query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2).unwrap_or_default()))
            }) {
                for (card, k, v) in rows.flatten() {
                    cards.entry(card).or_default().insert(k, v);
                }
            }
        }
        drop(conn);
        cleanup(&dest);
        if cards.is_empty() {
            continue;
        }
        imp.groups.push(Group { id: group_id.clone(), name, source: src.into() , ..Default::default() });
        let mut keys: Vec<_> = cards.keys().cloned().collect();
        keys.sort();
        for key in keys.into_iter().take(3000) {
            let p = &cards[&key];
            let g = |k: &str| p.get(k).cloned().unwrap_or_default();
            let mut c = Contact { groups: vec![group_id.clone()], source: src.into(), ..Default::default() };
            if let Some(vc) = p.get("_vCard").filter(|v| v.contains("BEGIN:VCARD")) {
                let v = ics::parse_vcard(vc);
                c.first = v.first;
                c.last = v.last;
                c.company = v.org;
                c.job_title = v.title;
                c.emails = v.emails.into_iter().map(|(label, value)| Labeled { label, value }).collect();
                c.phones = v.phones.into_iter().map(|(label, value)| Labeled { label, value }).collect();
                c.address = v.address;
                c.birthday = v.birthday;
                c.notes = v.notes;
                if c.first.is_empty() && c.last.is_empty() {
                    c.first = g("DisplayName");
                }
            } else {
                c.first = g("FirstName");
                c.last = g("LastName");
                if c.first.is_empty() && c.last.is_empty() {
                    c.first = g("DisplayName");
                }
                c.company = g("Company");
                c.job_title = g("JobTitle");
                for (k, label) in [("PrimaryEmail", "home"), ("SecondEmail", "other")] {
                    let v = g(k);
                    if !v.is_empty() {
                        c.emails.push(Labeled { label: label.into(), value: v });
                    }
                }
                for (k, label) in [("CellularNumber", "mobile"), ("HomePhone", "home"), ("WorkPhone", "work")] {
                    let v = g(k);
                    if !v.is_empty() {
                        c.phones.push(Labeled { label: label.into(), value: v });
                    }
                }
                c.address = [g("HomeAddress"), g("HomeCity"), g("HomeZipCode"), g("HomeCountry")]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n");
                c.notes = g("Notes");
            }
            if c.first.is_empty() && c.last.is_empty() && c.emails.is_empty() && c.company.is_empty() {
                continue;
            }
            imp.contacts.push(c);
        }
    }
    Ok(imp)
}

// ==================================================================== Mailspring

pub fn mailspring_dir() -> Option<PathBuf> {
    let h = home();
    let mut dirs = vec![
        h.join(".config/Mailspring"),
        h.join(".var/app/com.getmailspring.Mailspring/config/Mailspring"),
        h.join("snap/mailspring/common/.config/Mailspring"),
        h.join("Library/Application Support/Mailspring"),
    ];
    if let Some(a) = appdata() {
        dirs.insert(0, a.join("Mailspring"));
    }
    dirs.into_iter().find(|d| d.join("edgehill.db").exists())
}

fn find_accounts(v: &Value, out: &mut Vec<(String, String, String)>) {
    match v {
        Value::Object(map) => {
            if let (Some(id), Some(email)) = (
                map.get("id").and_then(Value::as_str),
                map.get("emailAddress").and_then(Value::as_str),
            ) {
                let label = map.get("label").and_then(Value::as_str).unwrap_or(email);
                if !out.iter().any(|a| a.0 == id) {
                    out.push((id.into(), label.into(), email.into()));
                }
            }
            for x in map.values() {
                find_accounts(x, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| find_accounts(x, out)),
        _ => {}
    }
}

fn role_kind(role: &str, path: &str) -> &'static str {
    match role {
        "inbox" => "inbox",
        "sent" => "sent",
        "drafts" => "drafts",
        "trash" => "trash",
        "spam" => "junk",
        "all" | "archive" => "archive",
        _ => guess_kind(path),
    }
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

fn people(v: &Value, k: &str) -> String {
    v.get(k)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|p| {
                    let (n, e) = (s(p, "name"), s(p, "email"));
                    if n.is_empty() { e } else { format!("{n} <{e}>") }
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

fn find_vcard(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if s.contains("BEGIN:VCARD") => Some(s.clone()),
        Value::Object(m) => m.values().find_map(find_vcard),
        Value::Array(a) => a.iter().find_map(find_vcard),
        _ => None,
    }
}

pub fn import_mailspring(dir: &Path, progress: &dyn Fn(String)) -> Result<Imported, String> {
    let src = "mailspring";
    let mut imp = Imported { source: src.into(), ..Default::default() };
    let (conn, dest) = open_copy(&dir.join("edgehill.db")).ok_or("Could not open Mailspring's database")?;

    let mut accts = Vec::new();
    if let Ok(text) = std::fs::read_to_string(dir.join("config.json")) {
        if let Ok(v) = serde_json::from_str::<Value>(&text) {
            find_accounts(&v, &mut accts);
        }
    }
    let account_id = |a: &str| format!("{src}:{a}");
    let mut known_accounts: Vec<String> = Vec::new();
    let mut ensure_account = |imp: &mut Imported, a: &str| {
        if known_accounts.iter().any(|x| x == a) {
            return;
        }
        known_accounts.push(a.to_string());
        let (name, email) = accts
            .iter()
            .find(|x| x.0 == a)
            .map(|x| (x.1.clone(), x.2.clone()))
            .unwrap_or_else(|| ("Mailspring".into(), String::new()));
        imp.accounts.push(Account { id: account_id(a), name, email, source: src.into() });
    };

    // ---- folders and labels
    progress("Reading Mailspring folders…".into());
    let mut folder_kind: HashMap<String, (String, String)> = HashMap::new(); // id -> (kind, accountId)
    for table in ["Folder", "Label"] {
        let rows: Vec<(String, String, String)> = conn
            .prepare(&format!("SELECT id, accountId, data FROM {table}"))
            .and_then(|mut st| {
                st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, String>(2).unwrap_or_default())))
                    .map(|it| it.flatten().collect())
            })
            .unwrap_or_default();
        for (id, acct, data) in rows {
            let v: Value = serde_json::from_str(&data).unwrap_or(Value::Null);
            let path = s(&v, "path");
            let role = s(&v, "role");
            let kind = role_kind(&role, &path);
            folder_kind.insert(id.clone(), (kind.into(), acct.clone()));
            if table == "Label" && kind == "other" {
                continue;
            }
            ensure_account(&mut imp, &acct);
            let name = if kind == "inbox" { "Inbox".to_string() } else { pretty_folder(&path) };
            imp.folders.push(Folder { id: format!("{src}:{id}"), account: account_id(&acct), name, kind: kind.into() , ..Default::default() });
        }
    }

    // ---- messages
    progress("Reading Mailspring messages…".into());
    let rows: Vec<(String, String)> = conn
        .prepare("SELECT id, data FROM Message ORDER BY date DESC LIMIT 4000")
        .and_then(|mut st| {
            st.query_map([], |r| Ok((r.get(0)?, r.get::<_, String>(1).unwrap_or_default())))
                .map(|it| it.flatten().collect())
        })
        .unwrap_or_default();
    let mut body_q = conn.prepare("SELECT value FROM MessageBody WHERE id=?1").ok();
    let mut per_folder: HashMap<String, usize> = HashMap::new();
    for (id, data) in rows {
        let v: Value = serde_json::from_str(&data).unwrap_or(Value::Null);
        let mut folder = v.get("folder").map(|f| s(f, "id")).unwrap_or_default();
        let fkind = folder_kind.get(&folder).map(|x| x.0.clone()).unwrap_or_default();
        if fkind == "archive" || fkind == "other" || fkind.is_empty() {
            if let Some(labels) = v.get("labels").and_then(Value::as_array) {
                for l in labels {
                    let lid = s(l, "id");
                    if let Some((k, _)) = folder_kind.get(&lid) {
                        if ["inbox", "sent", "drafts", "trash", "junk"].contains(&k.as_str()) {
                            folder = lid;
                            break;
                        }
                    }
                }
            }
        }
        if folder.is_empty() || !folder_kind.contains_key(&folder) {
            continue;
        }
        let n = per_folder.entry(folder.clone()).or_insert(0);
        *n += 1;
        if *n > MAX_PER_FOLDER * 2 {
            continue;
        }
        let html: String = body_q
            .as_mut()
            .and_then(|q| q.query_row([&id], |r| r.get::<_, String>(0)).ok())
            .unwrap_or_default();
        let clean = if html.contains('<') { mime::clean_html(&html) } else { String::new() };
        // Mailspring keeps attachments as files: files/ab/cd/<file id>/<name>
        let files: Vec<(String, String)> = v
            .get("files")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(|f| (s(f, "filename"), s(f, "id"))).filter(|(n, i)| !n.is_empty() && i.len() > 4).collect())
            .unwrap_or_default();
        let raw_ref = if files.is_empty() {
            String::new()
        } else {
            let mut r = format!("msfiles:{}", dir.join("files").display());
            for (n, i) in &files {
                r.push_str(&format!("\n{n}\t{i}"));
            }
            r
        };
        let body = if html.is_empty() {
            s(&v, "snippet")
        } else if html.contains('<') {
            mime::html_to_text(&html)
        } else {
            html
        };
        let (from_name, from_email) = v
            .get("from")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .map(|p| (s(p, "name"), s(p, "email")))
            .unwrap_or_default();
        imp.messages.push(Message {
            id: 0,
            folder: format!("{src}:{folder}"),
            from_name,
            from_email,
            to: people(&v, "to"),
            cc: people(&v, "cc"),
            subject: s(&v, "subject"),
            body: body.chars().take(60_000).collect(),
            date: v.get("date").and_then(Value::as_i64).unwrap_or(0),
            read: !v.get("unread").and_then(Value::as_bool).unwrap_or(false),
            flagged: v.get("starred").and_then(Value::as_bool).unwrap_or(false),
            source: src.into(),
            attachments: v
                .get("files")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter(|f| f.get("contentId").map_or(true, |c| c.is_null()))
                        .map(|f| s(f, "filename"))
                        .filter(|n| !n.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
            html: if clean.len() > 400_000 { String::new() } else { clean },
            raw_ref,
            ..Default::default()
        });
    }
    drop(body_q);

    // ---- calendars and events
    progress("Reading Mailspring calendars…".into());
    let cals: Vec<(String, String)> = conn
        .prepare("SELECT id, data FROM Calendar")
        .and_then(|mut st| {
            st.query_map([], |r| Ok((r.get(0)?, r.get::<_, String>(1).unwrap_or_default())))
                .map(|it| it.flatten().collect())
        })
        .unwrap_or_default();
    for (i, (id, data)) in cals.iter().enumerate() {
        let v: Value = serde_json::from_str(data).unwrap_or(Value::Null);
        let name = s(&v, "name");
        imp.calendars.push(Calendar {
            id: format!("{src}:{id}"),
            name: if name.is_empty() { "Mailspring".into() } else { name },
            color: color_for(i + 1),
            visible: true,
            source: src.into(),
            ..Default::default()
        });
    }
    let evs: Vec<(String, String)> = conn
        .prepare("SELECT calendarId, data FROM Event")
        .and_then(|mut st| {
            st.query_map([], |r| Ok((r.get::<_, String>(0).unwrap_or_default(), r.get::<_, String>(1).unwrap_or_default())))
                .map(|it| it.flatten().collect())
        })
        .unwrap_or_default();
    for (cal, data) in evs {
        let v: Value = serde_json::from_str(&data).unwrap_or(Value::Null);
        let text = s(&v, "ics");
        let cal_id = format!("{src}:{cal}");
        if !imp.calendars.iter().any(|c| c.id == cal_id) {
            imp.calendars.push(Calendar {
                id: cal_id.clone(),
                name: "Mailspring".into(),
                color: color_for(imp.calendars.len() + 1),
                visible: true,
                source: src.into(),
                ..Default::default()
            });
        }
        let vevents = ics::parse_vevents(&text);
        let overrides: Vec<String> = vevents
            .iter()
            .filter_map(|e| e.recurrence_id.map(fmt_dt))
            .collect();
        for e in vevents {
            let is_override = e.recurrence_id.is_some();
            let mut exdates = e.exdates;
            if !is_override {
                exdates.extend(overrides.iter().cloned());
            }
            imp.events.push(Event {
                id: 0,
                calendar: cal_id.clone(),
                title: if e.title.is_empty() { "(No title)".into() } else { e.title },
                start: fmt_dt(e.start.unwrap_or_default()),
                end: fmt_dt(e.end.unwrap_or_default()),
                all_day: e.all_day,
                location: e.location,
                notes: e.notes,
                rrule: if is_override { String::new() } else { e.rrule },
                source: src.into(),
                tz: e.zone,
                exdates: if is_override { vec![] } else { exdates },
                uid: e.uid,
                ..Default::default()
            });
        }
    }

    // ---- contacts
    progress("Reading Mailspring contacts…".into());
    let contacts: Vec<(String, String)> = conn
        .prepare("SELECT accountId, data FROM Contact ORDER BY refs DESC LIMIT 3000")
        .or_else(|_| conn.prepare("SELECT accountId, data FROM Contact LIMIT 3000"))
        .and_then(|mut st| {
            st.query_map([], |r| Ok((r.get::<_, String>(0).unwrap_or_default(), r.get::<_, String>(1).unwrap_or_default())))
                .map(|it| it.flatten().collect())
        })
        .unwrap_or_default();
    for (acct, data) in contacts {
        let v: Value = serde_json::from_str(&data).unwrap_or(Value::Null);
        let group_id = format!("{src}:contacts:{acct}");
        if !imp.groups.iter().any(|g| g.id == group_id) {
            let label = accts.iter().find(|a| a.0 == acct).map(|a| a.2.clone()).unwrap_or_else(|| "Mailspring".into());
            imp.groups.push(Group { id: group_id.clone(), name: label, source: src.into() , ..Default::default() });
        }
        let mut c = Contact { groups: vec![group_id], source: src.into(), ..Default::default() };
        if let Some(vc) = find_vcard(&v) {
            let card = ics::parse_vcard(&vc);
            c.first = card.first;
            c.last = card.last;
            c.company = card.org;
            c.job_title = card.title;
            c.emails = card.emails.into_iter().map(|(label, value)| Labeled { label, value }).collect();
            c.phones = card.phones.into_iter().map(|(label, value)| Labeled { label, value }).collect();
            c.address = card.address;
            c.birthday = card.birthday;
            c.notes = card.notes;
        } else {
            let name = s(&v, "name");
            let mut it = name.rsplitn(2, ' ');
            let last = it.next().unwrap_or("").to_string();
            match it.next() {
                Some(first) => {
                    c.first = first.into();
                    c.last = last;
                }
                None => c.first = last,
            }
        }
        let email = s(&v, "email");
        if !email.is_empty() && !c.emails.iter().any(|e| e.value == email) {
            c.emails.insert(0, Labeled { label: "email".into(), value: email });
        }
        if c.first.is_empty() && c.last.is_empty() && c.emails.is_empty() {
            continue;
        }
        imp.contacts.push(c);
    }
    drop(conn);
    cleanup(&dest);
    Ok(imp)
}
