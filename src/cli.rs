//! Calendar answers for other programs (a desktop's menu bar clock, scripts),
//! without opening the window:
//!
//! - `almanac --events FROM TO` prints the events overlapping local
//!   [FROM, TO) as JSON (repeats expanded, hidden calendars left out). FROM
//!   and TO are `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM`. Read-only: it only reads
//!   data.json, which the app replaces in one go when it saves (watch that
//!   file, named in the answer's `dataFile`, to know when to ask again).
//!   With `--sample`, the made-up example calendar instead.
//! - `almanac --open-event ID START` shows an event (START: the occurrence's
//!   start as `--events` gave it) and `almanac --new-event YYYY-MM-DD[THH:MM]`
//!   opens the new-event editor on that day: handed to the running Almanac if
//!   there is one, else Almanac starts with it.
//! - `almanac --compose 'mailto:a@b.org?cc=…&bcc=…&subject=…&body=…&attach=/path'`
//!   (or just `almanac mailto:…`, as a desktop's mailto: handler is run) opens
//!   a new message filled in from the link (RFC 6068). `attach=` / `attachment=`
//!   (a path or a file:// URI, repeatable) adds files. Handed over the same way.

use serde_json::{json, Value};

/// What `--open-event` / `--new-event` asked to open, in the form the window's
/// `openRequest` takes ({event, start} / {newEvent}).
pub fn launch_request() -> Option<Value> {
    let args: Vec<String> = std::env::args().collect();
    let after = |name: &str, n: usize| args.iter().position(|a| a == name).and_then(|i| args.get(i + n)).cloned();
    if let (Some(id), Some(start)) = (after("--open-event", 1), after("--open-event", 2)) {
        let id: i64 = id.parse().ok()?;
        crate::data::parse_dt(&start)?;
        return Some(json!({ "event": id, "start": start, "at": now_ms() }));
    }
    if let Some(day) = after("--new-event", 1) {
        crate::data::parse_dt(&day)?;
        return Some(json!({ "newEvent": day, "at": now_ms() }));
    }
    let link = after("--compose", 1).or_else(|| args.iter().skip(1).find(|a| a.to_ascii_lowercase().starts_with("mailto:")).cloned());
    if let Some(link) = link {
        return Some(json!({ "compose": compose_from_mailto(&link), "at": now_ms() }));
    }
    None
}

/// A mailto: link (RFC 6068) as ComposeSheet.openWith's fields:
/// {to, cc, bcc, subject, body, attachments}. Addresses are comma separated;
/// attachments keep only files that exist.
pub fn compose_from_mailto(link: &str) -> Value {
    let rest = match link.get(..7) {
        Some(p) if p.eq_ignore_ascii_case("mailto:") => &link[7..],
        _ => link,
    };
    let (to, query) = rest.split_once('?').unwrap_or((rest, ""));
    let mut to: Vec<String> = split_addrs(&pct_decode(to));
    let (mut cc, mut bcc, mut subject, mut body, mut files) = (Vec::new(), Vec::new(), String::new(), String::new(), Vec::new());
    for kv in query.split('&').filter(|kv| !kv.is_empty()) {
        let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
        let v = pct_decode(v);
        match pct_decode(k).to_ascii_lowercase().as_str() {
            "to" => to.extend(split_addrs(&v)),
            "cc" => cc.extend(split_addrs(&v)),
            "bcc" => bcc.extend(split_addrs(&v)),
            "subject" => subject = v,
            // Line breaks come as %0D%0A; the editor wants \n.
            "body" => body = v.replace("\r\n", "\n"),
            "attach" | "attachment" => {
                let path = v.strip_prefix("file://").map(pct_decode).unwrap_or(v);
                if std::path::Path::new(&path).is_file() {
                    files.push(path);
                } else {
                    eprintln!("almanac --compose: no such file to attach: {path}");
                }
            }
            _ => {}
        }
    }
    json!({ "to": to.join(", "), "cc": cc.join(", "), "bcc": bcc.join(", "), "subject": subject, "body": body, "attachments": files })
}

fn split_addrs(s: &str) -> Vec<String> {
    s.split(',').map(str::trim).filter(|a| !a.is_empty()).map(String::from).collect()
}

/// %XX decoding (a '+' stays a '+': mailto: links don't use it for spaces).
fn pct_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 3 <= b.len() {
            if let Some(x) = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(x);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Handles `--events`: true when it did (the program should then stop).
pub fn run() -> bool {
    let args: Vec<String> = std::env::args().collect();
    let Some(i) = args.iter().position(|a| a == "--events") else { return false };
    let (from, to) = (args.get(i + 1).cloned().unwrap_or_default(), args.get(i + 2).cloned().unwrap_or_default());
    match events_json(&from, &to) {
        Ok(v) => println!("{v}"),
        Err(e) => {
            eprintln!("almanac --events: {e}");
            println!("{}", json!({ "error": e }));
            std::process::exit(2);
        }
    }
    true
}

fn events_json(from: &str, to: &str) -> Result<Value, String> {
    use crate::data::{parse_dt, Db};
    if parse_dt(from).is_none() || parse_dt(to).is_none() {
        return Err("usage: almanac --events FROM TO (YYYY-MM-DD or YYYY-MM-DDTHH:MM)".into());
    }
    // `--sample`: the made-up example calendar (for screenshots and tests).
    if std::env::args().any(|a| a == "--sample") {
        return Ok(answer(&Db::sample(), from, to, ""));
    }
    let file = crate::data::data_dir().join("data.json");
    // Never set up (or no data yet): no calendars, not an error.
    let db = if file.exists() {
        Db::load_without_mail().map(|(db, _)| db).ok_or_else(|| format!("couldn't read {}", file.display()))?
    } else {
        Db::default()
    };
    Ok(answer(&db, from, to, &file.display().to_string()))
}

/// The answer for `--events`: calendars and events, without notes (they can
/// be long and private; the window shows them).
fn answer(db: &crate::data::Db, from: &str, to: &str, data_file: &str) -> Value {
    let keep = ["id", "title", "start", "end", "allDay", "location", "calendar", "color", "repeats", "occurrence", "virtual", "readOnly", "url"];
    let events: Vec<Value> = match db.events(from, to) {
        Value::Array(list) => list
            .into_iter()
            .map(|e| Value::Object(e.as_object().map(|o| o.iter().filter(|(k, _)| keep.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default()))
            .collect(),
        _ => vec![],
    };
    let calendars: Vec<Value> = db.calendars.iter().map(|c| json!({ "id": c.id, "name": c.name, "color": c.color, "visible": c.visible })).collect();
    json!({ "dataFile": data_file, "from": from, "to": to, "calendars": calendars, "events": events })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_answer_has_no_notes_and_expands_repeats() {
        let db = crate::data::Db::sample();
        let today = chrono::Local::now().date_naive();
        let (from, to) = ((today - chrono::Duration::days(40)).to_string(), (today + chrono::Duration::days(40)).to_string());
        let v = answer(&db, &from, &to, "/x/data.json");
        let events = v["events"].as_array().unwrap();
        assert!(!events.is_empty());
        assert!(events.iter().all(|e| e.get("notes").is_none() && e.get("title").is_some() && e.get("color").is_some()));
        assert!(!v["calendars"].as_array().unwrap().is_empty());
        assert_eq!(v["dataFile"], "/x/data.json");
        // The weekday stand-up comes back once per weekday.
        assert!(events.iter().filter(|e| e["title"] == "Team stand-up").count() > 20);
    }

    #[test]
    fn empty_db_is_no_calendars() {
        let v = answer(&crate::data::Db::default(), "2026-01-01", "2026-02-01", "f");
        assert_eq!(v["events"].as_array().unwrap().len(), 0);
        assert_eq!(v["calendars"].as_array().unwrap().len(), 0);
    }
}

#[cfg(test)]
mod compose_tests {
    use super::compose_from_mailto;

    #[test]
    fn mailto_fields() {
        let dir = std::env::temp_dir().join("almanac-compose-test");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a b.pdf");
        std::fs::write(&f, b"x").unwrap();
        let link = format!(
            "MAILTO:a@b.org,c%20d@e.org?cc=x@y.org&bcc=z@w.org,q@r.org&subject=Hi%20%26%20bye%2B&body=L1%0D%0AL2&attach={}&attachment=file://{}&attach=/nope/missing",
            f.display().to_string().replace(' ', "%20"),
            f.display().to_string().replace(' ', "%20"));
        let v = compose_from_mailto(&link);
        assert_eq!(v["to"], "a@b.org, c d@e.org");
        assert_eq!(v["cc"], "x@y.org");
        assert_eq!(v["bcc"], "z@w.org, q@r.org");
        assert_eq!(v["subject"], "Hi & bye+");
        assert_eq!(v["body"], "L1\nL2");
        assert_eq!(v["attachments"].as_array().unwrap().len(), 2);
        assert_eq!(v["attachments"][0], f.display().to_string());
        let v = compose_from_mailto("mailto:?to=solo@x.org&subject=%E2%9C%93%");
        assert_eq!(v["to"], "solo@x.org");
        assert_eq!(v["subject"], "\u{2713}%");
    }
}
