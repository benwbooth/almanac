//! Calendars and contacts for Google (Calendar API, People API) and
//! Microsoft (Graph: Outlook.com, Microsoft 365). Mail for both goes through
//! IMAP/SMTP with the same sign-in (see live.rs).

use crate::data::*;
use crate::live::LiveAccount;
use crate::tz;
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use serde_json::{json, Value};

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(std::time::Duration::from_secs(90)))
        .build()
        .into()
}

fn enc(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// One JSON request. Returns the parsed body (Null for empty bodies).
fn api(token: &str, method: &str, url: &str, body: Option<&Value>, extra: &[(&str, &str)]) -> Result<Value, String> {
    let mut b = ureq::http::Request::builder()
        .method(method)
        .uri(url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/json");
    for (k, v) in extra {
        b = b.header(*k, *v);
    }
    let text = body.map(|v| v.to_string()).unwrap_or_default();
    if body.is_some() {
        b = b.header("Content-Type", "application/json");
    }
    let req = b.body(text).map_err(|e| e.to_string())?;
    let mut resp = agent().run(req).map_err(|e| format!("{url}: {e}"))?;
    let status = resp.status().as_u16();
    let out = resp.body_mut().read_to_string().unwrap_or_default();
    let v: Value = if out.trim().is_empty() { Value::Null } else { serde_json::from_str(&out).unwrap_or(Value::Null) };
    if status >= 400 {
        let msg = v
            .pointer("/error/message")
            .and_then(Value::as_str)
            .map(String::from)
            .unwrap_or_else(|| format!("HTTP {status}"));
        return Err(msg);
    }
    Ok(v)
}

/// All items of a paged list (Google "nextPageToken" or Graph "@odata.nextLink").
fn list_all(token: &str, url: &str, items_key: &str, extra: &[(&str, &str)]) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    let mut next = Some(url.to_string());
    let mut guard = 0;
    while let Some(u) = next.take() {
        let v = api(token, "GET", &u, None, extra)?;
        if let Some(a) = v.get(items_key).and_then(Value::as_array) {
            out.extend(a.iter().cloned());
        }
        if let Some(link) = v.get("@odata.nextLink").and_then(Value::as_str) {
            next = Some(link.to_string());
        } else if let Some(tok) = v.get("nextPageToken").and_then(Value::as_str) {
            let base = url.split("&pageToken=").next().unwrap_or(url);
            next = Some(format!("{base}{}pageToken={}", if base.contains('?') { "&" } else { "?" }, enc(tok)));
        }
        guard += 1;
        if guard > 200 {
            break;
        }
    }
    Ok(out)
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

fn utc_rfc3339(t: NaiveDateTime) -> String {
    t.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// An RFC 3339 time ("2026-11-03T09:00:00-08:00") as UTC.
fn parse_rfc3339(v: &str) -> Option<NaiveDateTime> {
    DateTime::parse_from_rfc3339(v).ok().map(|d| d.with_timezone(&Utc).naive_utc())
}

// =================================================================== Google

const GCAL: &str = "https://www.googleapis.com/calendar/v3";
const PEOPLE: &str = "https://people.googleapis.com/v1";

fn g_time(v: &Value, zone: &str) -> Option<(NaiveDateTime, bool)> {
    if let Some(d) = v.get("date").and_then(Value::as_str) {
        return parse_dt(d).map(|t| (t, true));
    }
    let utc = parse_rfc3339(v.get("dateTime")?.as_str()?)?;
    Some((tz::utc_to_wall(utc, zone), false))
}

fn g_status(s: &str) -> String {
    match s {
        "accepted" => "ACCEPTED",
        "tentative" => "TENTATIVE",
        "declined" => "DECLINED",
        _ => "NEEDS-ACTION",
    }
    .into()
}

pub fn google_sync_calendars(a: &LiveAccount, token: &str, imp: &mut Imported, progress: &dyn Fn(String)) -> Result<(), String> {
    let cals = list_all(token, &format!("{GCAL}/users/me/calendarList?maxResults=250"), "items", &[])?;
    let from = utc_rfc3339(Utc::now().naive_utc() - Duration::days(400));
    let to = utc_rfc3339(Utc::now().naive_utc() + Duration::days(800));
    for c in cals {
        let cid = s(&c, "id");
        let name = if s(&c, "summaryOverride").is_empty() { s(&c, "summary") } else { s(&c, "summaryOverride") };
        progress(format!("{}: reading calendar {name}…", a.name));
        let cal_zone = tz::canonical(&s(&c, "timeZone"));
        let defaults: Vec<i64> = c
            .get("defaultReminders")
            .and_then(Value::as_array)
            .map(|r| r.iter().filter_map(|x| x.get("minutes").and_then(Value::as_i64)).collect())
            .unwrap_or_default();
        let cal_id = format!("{}:google:{cid}", a.source());
        imp.calendars.push(Calendar {
            id: cal_id.clone(),
            name,
            color: s(&c, "backgroundColor"),
            visible: c.get("selected").and_then(Value::as_bool).unwrap_or(true),
            source: a.source(),
            url: format!("google:{cid}"),
            account: a.id.clone(),
        });
        let url = format!("{GCAL}/calendars/{}/events?maxResults=2500&singleEvents=false&timeMin={}&timeMax={}", enc(&cid), enc(&from), enc(&to));
        let items = match list_all(token, &url, "items", &[]) {
            Ok(i) => i,
            Err(e) => {
                progress(format!("{}: {e}", a.name));
                continue;
            }
        };
        let mut evs: Vec<Event> = Vec::new();
        let mut cancelled: Vec<(String, String)> = Vec::new(); // (master id, wall)
        for it in items {
            let zone = {
                let z = it.pointer("/start/timeZone").and_then(Value::as_str).unwrap_or("");
                if z.is_empty() { cal_zone.clone() } else { tz::canonical(z) }
            };
            let master_id = s(&it, "recurringEventId");
            let orig = it.get("originalStartTime").and_then(|o| g_time(o, &zone)).map(|(t, _)| fmt_dt(t));
            if s(&it, "status") == "cancelled" {
                if let (false, Some(w)) = (master_id.is_empty(), orig) {
                    cancelled.push((master_id, w));
                }
                continue;
            }
            let Some((start, all_day)) = it.get("start").and_then(|t| g_time(t, &zone)) else { continue };
            let end = it.get("end").and_then(|t| g_time(t, &zone)).map(|(t, _)| t).unwrap_or(start + Duration::hours(1));
            let mut rrule = String::new();
            let mut exdates = Vec::new();
            for r in it.get("recurrence").and_then(Value::as_array).cloned().unwrap_or_default() {
                let line = r.as_str().unwrap_or("");
                if let Some(x) = line.strip_prefix("RRULE:") {
                    rrule = x.to_string();
                } else if line.starts_with("EXDATE") {
                    let fake = format!("BEGIN:VEVENT\nDTSTART:20000101T000000Z\n{line}\nEND:VEVENT\n");
                    if let Some(ev) = crate::ics::parse_vevents(&fake).into_iter().next() {
                        for x in ev.exdates {
                            if let Some(t) = parse_dt(&x) {
                                exdates.push(fmt_dt(tz::utc_to_wall(t, if zone.is_empty() { "UTC" } else { &zone })));
                            }
                        }
                    }
                }
            }
            let reminders = it.get("reminders").cloned().unwrap_or(Value::Null);
            let alarms = if reminders.get("useDefault").and_then(Value::as_bool).unwrap_or(true) {
                defaults.clone()
            } else {
                reminders
                    .get("overrides")
                    .and_then(Value::as_array)
                    .map(|r| r.iter().filter_map(|x| x.get("minutes").and_then(Value::as_i64)).collect())
                    .unwrap_or_default()
            };
            let attendees = it
                .get("attendees")
                .and_then(Value::as_array)
                .map(|l| {
                    l.iter()
                        .map(|x| Attendee { email: s(x, "email"), name: s(x, "displayName"), status: g_status(&s(x, "responseStatus")) })
                        .collect()
                })
                .unwrap_or_default();
            evs.push(Event {
                calendar: cal_id.clone(),
                title: if s(&it, "summary").is_empty() { "(No title)".into() } else { s(&it, "summary") },
                start: fmt_dt(start),
                end: fmt_dt(end),
                all_day,
                location: s(&it, "location"),
                notes: crate::mime::html_to_text(&s(&it, "description")),
                notes_html: if s(&it, "description").contains('<') { s(&it, "description") } else { String::new() },
                // Google Meet's link (or another video call added to the event).
                url: if s(&it, "hangoutLink").is_empty() {
                    it.pointer("/conferenceData/entryPoints")
                        .and_then(Value::as_array)
                        .and_then(|l| l.iter().find(|p| s(p, "entryPointType") == "video"))
                        .map(|p| s(p, "uri"))
                        .unwrap_or_default()
                } else {
                    s(&it, "hangoutLink")
                },
                rrule,
                source: a.source(),
                tz: if all_day { String::new() } else { zone.clone() },
                exdates,
                uid: if master_id.is_empty() { s(&it, "id") } else { master_id.clone() },
                href: s(&it, "id"),
                etag: s(&it, "etag"),
                alarms,
                attendees,
                organizer: it.pointer("/organizer/email").and_then(Value::as_str).unwrap_or("").to_string(),
                recurrence_id: if master_id.is_empty() { String::new() } else { orig.unwrap_or_default() },
                ..Default::default()
            });
        }
        // Skipped and changed dates are left out of the series.
        let changed: Vec<(String, String)> = evs.iter().filter(|e| !e.recurrence_id.is_empty()).map(|e| (e.uid.clone(), e.recurrence_id.clone())).collect();
        for (mid, w) in cancelled.into_iter().chain(changed) {
            if let Some(m) = evs.iter_mut().find(|m| m.uid == mid && m.recurrence_id.is_empty()) {
                if !m.exdates.contains(&w) {
                    m.exdates.push(w);
                }
            }
        }
        imp.events.extend(evs);
    }
    Ok(())
}

fn g_event_body(e: &Event, with_recurrence: bool) -> Value {
    let zone = if e.tz.is_empty() { tz::system_zone() } else { e.tz.clone() };
    let t = |w: &str| -> Value {
        if e.all_day {
            json!({"date": w.get(..10).unwrap_or(w)})
        } else {
            json!({"dateTime": format!("{}:00", w), "timeZone": zone})
        }
    };
    let mut v = json!({
        "summary": e.title,
        "location": e.location,
        // (Google shows simple HTML in descriptions.)
        "description": if e.notes_html.is_empty() { &e.notes } else { &e.notes_html },
        "start": t(&e.start),
        "end": t(&e.end),
        "reminders": {"useDefault": false, "overrides": e.alarms.iter().map(|m| json!({"method": "popup", "minutes": m})).collect::<Vec<_>>()},
    });
    if !e.attendees.is_empty() {
        v["attendees"] = json!(e.attendees.iter().map(|a| json!({"email": a.email, "displayName": a.name})).collect::<Vec<_>>());
    }
    if with_recurrence && !e.rrule.is_empty() {
        let mut rec = vec![format!("RRULE:{}", e.rrule)];
        for x in &e.exdates {
            if let Some(t) = parse_dt(x) {
                rec.push(if e.all_day {
                    format!("EXDATE;VALUE=DATE:{}", t.format("%Y%m%d"))
                } else {
                    format!("EXDATE;TZID={zone}:{}", t.format("%Y%m%dT%H%M%S"))
                });
            }
        }
        v["recurrence"] = json!(rec);
    }
    v
}

/// The id of one occurrence of a Google series (to change just that one).
fn g_instance_id(token: &str, cal: &str, master: &Event, wall: &str) -> Result<String, String> {
    let t = parse_dt(wall).ok_or("bad time")?;
    let zone = if master.all_day { "" } else { master.tz.as_str() };
    let utc = tz::wall_to_utc(t, zone);
    let url = format!(
        "{GCAL}/calendars/{}/events/{}/instances?timeMin={}&timeMax={}&maxResults=5",
        enc(cal),
        enc(&master.href),
        enc(&utc_rfc3339(utc - Duration::minutes(1))),
        enc(&utc_rfc3339(utc + Duration::days(1)))
    );
    let v = api(token, "GET", &url, None, &[])?;
    v.get("items")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .map(|x| s(x, "id"))
        .filter(|x| !x.is_empty())
        .ok_or_else(|| "That occurrence was not found on the server".into())
}

pub fn google_push_event(token: &str, cal: &str, master: &Event, overrides: &[Event]) -> Result<(String, String), String> {
    let base = format!("{GCAL}/calendars/{}/events", enc(cal));
    let body = g_event_body(master, true);
    let r = if master.href.is_empty() {
        api(token, "POST", &format!("{base}?sendUpdates=all"), Some(&body), &[])?
    } else {
        api(token, "PUT", &format!("{base}/{}?sendUpdates=all", enc(&master.href)), Some(&body), &[])?
    };
    let mut m = master.clone();
    m.href = s(&r, "id");
    for o in overrides {
        let id = if !o.href.is_empty() && o.href != master.href { o.href.clone() } else { g_instance_id(token, cal, &m, &o.recurrence_id)? };
        api(token, "PUT", &format!("{base}/{}?sendUpdates=all", enc(&id)), Some(&g_event_body(o, false)), &[])?;
    }
    Ok((s(&r, "id"), s(&r, "etag")))
}

pub fn google_delete_event(token: &str, cal: &str, id: &str) -> Result<(), String> {
    api(token, "DELETE", &format!("{GCAL}/calendars/{}/events/{}?sendUpdates=all", enc(cal), enc(id)), None, &[]).map(|_| ())
}

pub fn google_sync_contacts(a: &LiveAccount, token: &str, imp: &mut Imported, progress: &dyn Fn(String)) -> Result<(), String> {
    progress(format!("{}: reading contacts…", a.name));
    let fields = "names,nicknames,emailAddresses,phoneNumbers,organizations,addresses,birthdays,biographies,urls,photos";
    let people = list_all(token, &format!("{PEOPLE}/people/me/connections?pageSize=1000&personFields={fields}"), "connections", &[])?;
    let group_id = format!("{}:google-contacts", a.source());
    imp.groups.push(Group { id: group_id.clone(), name: format!("Contacts ({})", a.name), source: a.source(), url: "google:contacts".into() });
    for p in people {
        let first_of = |k: &str| p.get(k).and_then(Value::as_array).and_then(|a| a.first()).cloned().unwrap_or(Value::Null);
        let name = first_of("names");
        let org = first_of("organizations");
        let labeled = |k: &str| -> Vec<Labeled> {
            p.get(k)
                .and_then(Value::as_array)
                .map(|a| a.iter().map(|x| Labeled { label: if s(x, "type").is_empty() { "other".into() } else { s(x, "type") }, value: s(x, "value") }).collect())
                .unwrap_or_default()
        };
        let bday = first_of("birthdays");
        let birthday = bday
            .get("date")
            .and_then(|d| {
                let (y, m, dd) = (d.get("year")?.as_i64().unwrap_or(1604), d.get("month")?.as_i64()?, d.get("day")?.as_i64()?);
                Some(format!("{y:04}-{m:02}-{dd:02}"))
            })
            .unwrap_or_default();
        let photo_url = p
            .get("photos")
            .and_then(Value::as_array)
            .and_then(|a| a.iter().find(|x| !x.get("default").and_then(Value::as_bool).unwrap_or(false)))
            .map(|x| s(x, "url"))
            .unwrap_or_default();
        let photo = if photo_url.is_empty() {
            String::new()
        } else {
            agent()
                .get(&photo_url)
                .call()
                .ok()
                .and_then(|mut r| r.body_mut().read_to_vec().ok())
                .map(|b| crate::ics::store_photo(&b))
                .unwrap_or_default()
        };
        imp.contacts.push(Contact {
            first: s(&name, "givenName"),
            last: s(&name, "familyName"),
            nickname: s(&first_of("nicknames"), "value"),
            company: s(&org, "name"),
            job_title: s(&org, "title"),
            emails: labeled("emailAddresses"),
            phones: labeled("phoneNumbers"),
            urls: labeled("urls"),
            address: s(&first_of("addresses"), "formattedValue"),
            birthday,
            notes: s(&first_of("biographies"), "value"),
            groups: vec![group_id.clone()],
            source: a.source(),
            href: s(&p, "resourceName"),
            etag: s(&p, "etag"),
            photo,
            ..Default::default()
        });
    }
    Ok(())
}

fn g_person(c: &Contact) -> Value {
    let mut v = json!({
        "names": [{"givenName": c.first, "familyName": c.last}],
        "nicknames": if c.nickname.is_empty() { json!([]) } else { json!([{"value": c.nickname}]) },
        "emailAddresses": c.emails.iter().map(|e| json!({"value": e.value, "type": e.label})).collect::<Vec<_>>(),
        "phoneNumbers": c.phones.iter().map(|e| json!({"value": e.value, "type": e.label})).collect::<Vec<_>>(),
        "urls": c.urls.iter().map(|e| json!({"value": e.value, "type": e.label})).collect::<Vec<_>>(),
        "organizations": if c.company.is_empty() && c.job_title.is_empty() { json!([]) } else { json!([{"name": c.company, "title": c.job_title}]) },
        "addresses": if c.address.is_empty() { json!([]) } else { json!([{"streetAddress": c.address}]) },
        "biographies": if c.notes.is_empty() { json!([]) } else { json!([{"value": c.notes, "contentType": "TEXT_PLAIN"}]) },
    });
    if c.birthday.len() >= 10 {
        let y: i64 = c.birthday[0..4].parse().unwrap_or(0);
        let m: i64 = c.birthday[5..7].parse().unwrap_or(1);
        let d: i64 = c.birthday[8..10].parse().unwrap_or(1);
        v["birthdays"] = json!([{"date": if y > 1604 { json!({"year": y, "month": m, "day": d}) } else { json!({"month": m, "day": d}) }}]);
    }
    v
}

pub fn google_push_contact(token: &str, c: &Contact) -> Result<(String, String), String> {
    let mut body = g_person(c);
    let r = if c.href.is_empty() {
        api(token, "POST", &format!("{PEOPLE}/people:createContact"), Some(&body), &[])?
    } else {
        body["etag"] = json!(c.etag);
        let fields = "names,nicknames,emailAddresses,phoneNumbers,organizations,addresses,birthdays,biographies,urls";
        api(token, "PATCH", &format!("{PEOPLE}/{}:updateContact?updatePersonFields={fields}", c.href), Some(&body), &[])?
    };
    Ok((s(&r, "resourceName"), s(&r, "etag")))
}

pub fn google_delete_contact(token: &str, href: &str) -> Result<(), String> {
    api(token, "DELETE", &format!("{PEOPLE}/{href}:deleteContact"), None, &[]).map(|_| ())
}

// ================================================================ Microsoft

const GRAPH: &str = "https://graph.microsoft.com/v1.0";
const UTC_PREF: (&str, &str) = ("Prefer", "outlook.timezone=\"UTC\"");

const DAYS: [(&str, &str); 7] = [
    ("monday", "MO"),
    ("tuesday", "TU"),
    ("wednesday", "WE"),
    ("thursday", "TH"),
    ("friday", "FR"),
    ("saturday", "SA"),
    ("sunday", "SU"),
];
const INDEX: [(&str, i32); 5] = [("first", 1), ("second", 2), ("third", 3), ("fourth", 4), ("last", -1)];

/// Graph recurrence → RRULE.
pub fn graph_to_rrule(r: &Value) -> String {
    let p = r.get("pattern").cloned().unwrap_or(Value::Null);
    let interval = p.get("interval").and_then(Value::as_i64).unwrap_or(1).max(1);
    let days: Vec<&str> = p
        .get("daysOfWeek")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|d| d.as_str()).filter_map(|d| DAYS.iter().find(|x| x.0 == d.to_lowercase()).map(|x| x.1)).collect())
        .unwrap_or_default();
    let idx = INDEX.iter().find(|x| x.0 == s(&p, "index")).map(|x| x.1).unwrap_or(1);
    let byday_n = days.iter().map(|d| format!("{idx}{d}")).collect::<Vec<_>>().join(",");
    let mut rule = match s(&p, "type").as_str() {
        "daily" => format!("FREQ=DAILY;INTERVAL={interval}"),
        "weekly" => format!("FREQ=WEEKLY;INTERVAL={interval};BYDAY={}", days.join(",")),
        "absoluteMonthly" => format!("FREQ=MONTHLY;INTERVAL={interval};BYMONTHDAY={}", p.get("dayOfMonth").and_then(Value::as_i64).unwrap_or(1)),
        "relativeMonthly" => format!("FREQ=MONTHLY;INTERVAL={interval};BYDAY={byday_n}"),
        "absoluteYearly" => format!(
            "FREQ=YEARLY;INTERVAL={interval};BYMONTH={};BYMONTHDAY={}",
            p.get("month").and_then(Value::as_i64).unwrap_or(1),
            p.get("dayOfMonth").and_then(Value::as_i64).unwrap_or(1)
        ),
        "relativeYearly" => format!("FREQ=YEARLY;INTERVAL={interval};BYMONTH={};BYDAY={byday_n}", p.get("month").and_then(Value::as_i64).unwrap_or(1)),
        _ => return String::new(),
    };
    let range = r.get("range").cloned().unwrap_or(Value::Null);
    match s(&range, "type").as_str() {
        "endDate" => rule.push_str(&format!(";UNTIL={}", s(&range, "endDate").replace('-', ""))),
        "numbered" => rule.push_str(&format!(";COUNT={}", range.get("numberOfOccurrences").and_then(Value::as_i64).unwrap_or(1))),
        _ => {}
    }
    rule
}

/// RRULE → Graph recurrence (start date needed for the range).
pub fn rrule_to_graph(rule: &str, start: &str, zone: &str) -> Value {
    let mut freq = "";
    let mut interval = 1;
    let mut byday: Vec<String> = vec![];
    let mut monthday: Option<i64> = None;
    let mut month: Option<i64> = None;
    let mut count: Option<i64> = None;
    let mut until = String::new();
    for part in rule.split(';') {
        let (k, v) = part.split_once('=').unwrap_or((part, ""));
        match k.to_uppercase().as_str() {
            "FREQ" => freq = v,
            "INTERVAL" => interval = v.parse().unwrap_or(1),
            "BYDAY" => byday = v.split(',').map(String::from).collect(),
            "BYMONTHDAY" => monthday = v.split(',').next().and_then(|x| x.parse().ok()),
            "BYMONTH" => month = v.split(',').next().and_then(|x| x.parse().ok()),
            "COUNT" => count = v.parse().ok(),
            "UNTIL" => until = v.get(..8).map(|d| format!("{}-{}-{}", &d[0..4], &d[4..6], &d[6..8])).unwrap_or_default(),
            _ => {}
        }
    }
    let day_name = |d: &str| DAYS.iter().find(|x| d.ends_with(x.1)).map(|x| x.0).unwrap_or("monday");
    let ord = byday.first().map(|d| d.trim_end_matches(|c: char| c.is_ascii_alphabetic()).parse::<i32>().unwrap_or(0)).unwrap_or(0);
    let index = INDEX.iter().find(|x| x.1 == ord).map(|x| x.0).unwrap_or("first");
    let start_date = start.get(..10).unwrap_or(start).to_string();
    let sd = parse_dt(start);
    let pattern = match freq.to_uppercase().as_str() {
        "DAILY" => json!({"type": "daily", "interval": interval}),
        "WEEKLY" => {
            let days: Vec<&str> = if byday.is_empty() {
                vec![sd.map(|d| DAYS[chrono::Datelike::weekday(&d).num_days_from_monday() as usize].0).unwrap_or("monday")]
            } else {
                byday.iter().map(|d| day_name(d)).collect()
            };
            json!({"type": "weekly", "interval": interval, "daysOfWeek": days, "firstDayOfWeek": "sunday"})
        }
        "MONTHLY" if !byday.is_empty() && ord != 0 => json!({"type": "relativeMonthly", "interval": interval, "daysOfWeek": byday.iter().map(|d| day_name(d)).collect::<Vec<_>>(), "index": index}),
        "MONTHLY" => json!({"type": "absoluteMonthly", "interval": interval, "dayOfMonth": monthday.unwrap_or_else(|| sd.map(|d| chrono::Datelike::day(&d) as i64).unwrap_or(1))}),
        "YEARLY" if !byday.is_empty() && ord != 0 => json!({"type": "relativeYearly", "interval": interval, "month": month.unwrap_or(1), "daysOfWeek": byday.iter().map(|d| day_name(d)).collect::<Vec<_>>(), "index": index}),
        _ => json!({"type": "absoluteYearly", "interval": interval,
                     "month": month.unwrap_or_else(|| sd.map(|d| chrono::Datelike::month(&d) as i64).unwrap_or(1)),
                     "dayOfMonth": monthday.unwrap_or_else(|| sd.map(|d| chrono::Datelike::day(&d) as i64).unwrap_or(1))}),
    };
    let range = if !until.is_empty() {
        json!({"type": "endDate", "startDate": start_date, "endDate": until, "recurrenceTimeZone": zone})
    } else if let Some(c) = count {
        json!({"type": "numbered", "startDate": start_date, "numberOfOccurrences": c, "recurrenceTimeZone": zone})
    } else {
        json!({"type": "noEnd", "startDate": start_date, "recurrenceTimeZone": zone})
    };
    json!({"pattern": pattern, "range": range})
}

fn m_time(v: &Value, zone: &str, all_day: bool) -> Option<NaiveDateTime> {
    let t = s(v, "dateTime");
    let t = t.get(..19).unwrap_or(&t);
    let utc = NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M:%S").ok()?;
    if all_day {
        // All-day times come back as midnight in UTC; keep the date.
        return Some(utc);
    }
    Some(tz::utc_to_wall(utc, zone))
}

fn m_status(r: &str) -> String {
    match r {
        "accepted" | "organizer" => "ACCEPTED",
        "tentativelyAccepted" => "TENTATIVE",
        "declined" => "DECLINED",
        _ => "NEEDS-ACTION",
    }
    .into()
}

fn m_event(it: &Value, cal_id: &str, a: &LiveAccount) -> Option<Event> {
    let all_day = it.get("isAllDay").and_then(Value::as_bool).unwrap_or(false);
    let zone = if all_day { String::new() } else { tz::canonical(&s(it, "originalStartTimeZone")) };
    let zone = if zone.is_empty() && !all_day { "UTC".to_string() } else { zone };
    let start = m_time(it.get("start")?, &zone, all_day)?;
    let end = it.get("end").and_then(|e| m_time(e, &zone, all_day)).unwrap_or(start + Duration::hours(1));
    let body = it.pointer("/body/content").and_then(Value::as_str).unwrap_or("");
    let notes = if it.pointer("/body/contentType").and_then(Value::as_str) == Some("html") { crate::mime::html_to_text(body) } else { body.to_string() };
    let attendees = it
        .get("attendees")
        .and_then(Value::as_array)
        .map(|l| {
            l.iter()
                .map(|x| Attendee {
                    email: x.pointer("/emailAddress/address").and_then(Value::as_str).unwrap_or("").into(),
                    name: x.pointer("/emailAddress/name").and_then(Value::as_str).unwrap_or("").into(),
                    status: m_status(x.pointer("/status/response").and_then(Value::as_str).unwrap_or("")),
                })
                .collect()
        })
        .unwrap_or_default();
    let alarms = if it.get("isReminderOn").and_then(Value::as_bool).unwrap_or(false) {
        vec![it.get("reminderMinutesBeforeStart").and_then(Value::as_i64).unwrap_or(15)]
    } else {
        vec![]
    };
    Some(Event {
        calendar: cal_id.into(),
        title: if s(it, "subject").is_empty() { "(No title)".into() } else { s(it, "subject") },
        start: fmt_dt(start),
        end: fmt_dt(end),
        all_day,
        location: it.pointer("/location/displayName").and_then(Value::as_str).unwrap_or("").into(),
        notes: notes.trim().to_string(),
        rrule: it.get("recurrence").filter(|r| !r.is_null()).map(graph_to_rrule).unwrap_or_default(),
        source: a.source(),
        tz: zone,
        uid: s(it, "id"),
        href: s(it, "id"),
        etag: s(it, "@odata.etag"),
        alarms,
        attendees,
        organizer: it.pointer("/organizer/emailAddress/address").and_then(Value::as_str).unwrap_or("").into(),
        notes_html: if it.pointer("/body/contentType").and_then(Value::as_str) == Some("html") { body.to_string() } else { String::new() },
        // A Teams (or other) meeting's link.
        url: it.pointer("/onlineMeeting/joinUrl").and_then(Value::as_str).or_else(|| it.get("onlineMeetingUrl").and_then(Value::as_str)).unwrap_or("").into(),
        ..Default::default()
    })
}

pub fn graph_sync_calendars(a: &LiveAccount, token: &str, imp: &mut Imported, progress: &dyn Fn(String)) -> Result<(), String> {
    let cals = list_all(token, &format!("{GRAPH}/me/calendars?$top=100"), "value", &[])?;
    let from = utc_rfc3339(Utc::now().naive_utc() - Duration::days(400));
    let to = utc_rfc3339(Utc::now().naive_utc() + Duration::days(800));
    for (i, c) in cals.iter().enumerate() {
        let cid = s(c, "id");
        let name = s(c, "name");
        progress(format!("{}: reading calendar {name}…", a.name));
        let cal_id = format!("{}:graph:{cid}", a.source());
        let color = s(c, "hexColor");
        imp.calendars.push(Calendar {
            id: cal_id.clone(),
            name,
            color: if color.is_empty() { ["#2f7cf6", "#e8453c", "#34a853", "#f5a623", "#9b59d0", "#1abc9c"][i % 6].into() } else { color },
            visible: true,
            source: a.source(),
            url: format!("graph:{cid}"),
            account: a.id.clone(),
        });
        // Single events and series (each series once).
        let items = list_all(token, &format!("{GRAPH}/me/calendars/{cid}/events?$top=200"), "value", &[UTC_PREF])?;
        let mut evs: Vec<Event> = items.iter().filter_map(|it| m_event(it, &cal_id, a)).collect();
        // Changed occurrences of series in the time window.
        let view = list_all(
            token,
            &format!("{GRAPH}/me/calendars/{cid}/calendarView?startDateTime={}&endDateTime={}&$top=500", enc(&from), enc(&to)),
            "value",
            &[UTC_PREF],
        )
        .unwrap_or_default();
        for it in view.iter().filter(|x| s(x, "type") == "exception") {
            let master = s(it, "seriesMasterId");
            let Some(mut e) = m_event(it, &cal_id, a) else { continue };
            let zone = evs.iter().find(|m| m.uid == master).map(|m| m.tz.clone()).unwrap_or(e.tz.clone());
            let orig = it
                .get("originalStart")
                .and_then(Value::as_str)
                .and_then(parse_rfc3339)
                .map(|u| fmt_dt(if e.all_day { u } else { tz::utc_to_wall(u, &zone) }))
                .unwrap_or_default();
            e.uid = master.clone();
            e.recurrence_id = orig.clone();
            e.rrule.clear();
            if let Some(m) = evs.iter_mut().find(|m| m.uid == master && m.recurrence_id.is_empty()) {
                if !m.exdates.contains(&orig) {
                    m.exdates.push(orig);
                }
            }
            evs.push(e);
        }
        imp.events.extend(evs);
    }
    Ok(())
}

fn m_event_body(e: &Event, with_recurrence: bool) -> Value {
    let zone = if e.all_day || e.tz.is_empty() { tz::system_zone() } else { e.tz.clone() };
    let t = |w: &str| json!({"dateTime": format!("{}:00", if e.all_day { format!("{}T00:00", w.get(..10).unwrap_or(w)) } else { w.to_string() }), "timeZone": zone});
    let mut v = json!({
        "subject": e.title,
        "body": if e.notes_html.is_empty() { json!({"contentType": "text", "content": e.notes}) } else { json!({"contentType": "html", "content": e.notes_html}) },
        "location": {"displayName": e.location},
        "start": t(&e.start),
        "end": t(&e.end),
        "isAllDay": e.all_day,
        "isReminderOn": !e.alarms.is_empty(),
        "reminderMinutesBeforeStart": e.alarms.first().copied().unwrap_or(15),
    });
    if !e.attendees.is_empty() {
        v["attendees"] = json!(e
            .attendees
            .iter()
            .map(|a| json!({"emailAddress": {"address": a.email, "name": a.name}, "type": "required"}))
            .collect::<Vec<_>>());
    }
    if with_recurrence && !e.rrule.is_empty() {
        v["recurrence"] = rrule_to_graph(&e.rrule, &e.start, &zone);
    }
    v
}

fn m_instance_id(token: &str, master: &Event, wall: &str) -> Result<String, String> {
    let t = parse_dt(wall).ok_or("bad time")?;
    let utc = tz::wall_to_utc(t, if master.all_day { "" } else { &master.tz });
    let url = format!(
        "{GRAPH}/me/events/{}/instances?startDateTime={}&endDateTime={}",
        master.href,
        enc(&utc_rfc3339(utc - Duration::minutes(1))),
        enc(&utc_rfc3339(utc + Duration::days(1)))
    );
    let v = api(token, "GET", &url, None, &[UTC_PREF])?;
    v.get("value")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .map(|x| s(x, "id"))
        .filter(|x| !x.is_empty())
        .ok_or_else(|| "That occurrence was not found on the server".into())
}

pub fn graph_push_event(token: &str, cal: &str, master: &Event, overrides: &[Event]) -> Result<(String, String), String> {
    let body = m_event_body(master, true);
    let r = if master.href.is_empty() {
        api(token, "POST", &format!("{GRAPH}/me/calendars/{cal}/events"), Some(&body), &[])?
    } else {
        api(token, "PATCH", &format!("{GRAPH}/me/events/{}", master.href), Some(&body), &[])?
    };
    let mut m = master.clone();
    m.href = s(&r, "id");
    for o in overrides {
        let id = if !o.href.is_empty() && o.href != master.href { o.href.clone() } else { m_instance_id(token, &m, &o.recurrence_id)? };
        api(token, "PATCH", &format!("{GRAPH}/me/events/{id}"), Some(&m_event_body(o, false)), &[])?;
    }
    Ok((s(&r, "id"), s(&r, "@odata.etag")))
}

pub fn graph_delete(token: &str, path: &str) -> Result<(), String> {
    api(token, "DELETE", &format!("{GRAPH}/me/{path}"), None, &[]).map(|_| ())
}

pub fn graph_sync_contacts(a: &LiveAccount, token: &str, imp: &mut Imported, progress: &dyn Fn(String)) -> Result<(), String> {
    progress(format!("{}: reading contacts…", a.name));
    let list = list_all(token, &format!("{GRAPH}/me/contacts?$top=500"), "value", &[])?;
    let group_id = format!("{}:graph-contacts", a.source());
    imp.groups.push(Group { id: group_id.clone(), name: format!("Contacts ({})", a.name), source: a.source(), url: "graph:contacts".into() });
    for p in list {
        let mut phones: Vec<Labeled> = Vec::new();
        if !s(&p, "mobilePhone").is_empty() {
            phones.push(Labeled { label: "mobile".into(), value: s(&p, "mobilePhone") });
        }
        for (k, label) in [("homePhones", "home"), ("businessPhones", "work")] {
            for x in p.get(k).and_then(Value::as_array).cloned().unwrap_or_default() {
                if let Some(v) = x.as_str() {
                    phones.push(Labeled { label: label.into(), value: v.into() });
                }
            }
        }
        let addr = p.get("homeAddress").cloned().unwrap_or(Value::Null);
        let address = ["street", "city", "state", "postalCode", "countryOrRegion"]
            .iter()
            .map(|k| s(&addr, k))
            .filter(|x| !x.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        let homepage = s(&p, "businessHomePage");
        imp.contacts.push(Contact {
            first: s(&p, "givenName"),
            last: s(&p, "surname"),
            nickname: s(&p, "nickName"),
            company: s(&p, "companyName"),
            job_title: s(&p, "jobTitle"),
            emails: p
                .get("emailAddresses")
                .and_then(Value::as_array)
                .map(|a| a.iter().map(|e| Labeled { label: "email".into(), value: s(e, "address") }).collect())
                .unwrap_or_default(),
            phones,
            urls: if homepage.is_empty() { vec![] } else { vec![Labeled { label: "homepage".into(), value: homepage }] },
            address,
            birthday: s(&p, "birthday").get(..10).unwrap_or("").to_string(),
            notes: s(&p, "personalNotes"),
            groups: vec![group_id.clone()],
            source: a.source(),
            href: s(&p, "id"),
            etag: s(&p, "@odata.etag"),
            ..Default::default()
        });
    }
    Ok(())
}

pub fn graph_push_contact(token: &str, c: &Contact) -> Result<(String, String), String> {
    let mobile = c.phones.iter().find(|p| p.label == "mobile").map(|p| p.value.clone()).unwrap_or_default();
    let home: Vec<String> = c.phones.iter().filter(|p| p.label == "home").map(|p| p.value.clone()).collect();
    let work: Vec<String> = c.phones.iter().filter(|p| p.label != "home" && p.label != "mobile").map(|p| p.value.clone()).collect();
    let mut body = json!({
        "givenName": c.first, "surname": c.last, "nickName": c.nickname, "companyName": c.company,
        "jobTitle": c.job_title, "personalNotes": c.notes, "mobilePhone": mobile, "homePhones": home,
        "businessPhones": work,
        "emailAddresses": c.emails.iter().map(|e| json!({"address": e.value, "name": display_name(c)})).collect::<Vec<_>>(),
        "homeAddress": {"street": c.address},
        "businessHomePage": c.urls.first().map(|u| u.value.clone()).unwrap_or_default(),
    });
    if c.birthday.len() >= 10 {
        body["birthday"] = json!(format!("{}T00:00:00Z", &c.birthday[..10]));
    }
    let r = if c.href.is_empty() {
        api(token, "POST", &format!("{GRAPH}/me/contacts"), Some(&body), &[])?
    } else {
        api(token, "PATCH", &format!("{GRAPH}/me/contacts/{}", c.href), Some(&body), &[])?
    };
    Ok((s(&r, "id"), s(&r, "@odata.etag")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_recurrence_round_trip() {
        let g = json!({"pattern": {"type": "relativeMonthly", "interval": 1, "daysOfWeek": ["tuesday"], "index": "second"},
                       "range": {"type": "numbered", "startDate": "2026-01-13", "numberOfOccurrences": 5}});
        let r = graph_to_rrule(&g);
        assert_eq!(r, "FREQ=MONTHLY;INTERVAL=1;BYDAY=2TU;COUNT=5");
        let back = rrule_to_graph(&r, "2026-01-13T10:00", "Europe/Berlin");
        assert_eq!(back["pattern"]["type"], "relativeMonthly");
        assert_eq!(back["pattern"]["index"], "second");
        assert_eq!(back["range"]["numberOfOccurrences"], 5);
    }
}
