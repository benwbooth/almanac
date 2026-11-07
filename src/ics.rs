//! iCalendar / vCard helpers: repeat rules (RFC 5545), reading and writing
//! VEVENTs with their time zones, and reading and writing vCards.

use crate::tz;
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, Weekday};

// ------------------------------------------------------------------ repeats

fn weekday(s: &str) -> Option<Weekday> {
    Some(match s.to_uppercase().as_str() {
        "MO" => Weekday::Mon,
        "TU" => Weekday::Tue,
        "WE" => Weekday::Wed,
        "TH" => Weekday::Thu,
        "FR" => Weekday::Fri,
        "SA" => Weekday::Sat,
        "SU" => Weekday::Sun,
        _ => return None,
    })
}

/// "2TU" → (2, Tue); "-1FR" → (-1, Fri); "MO" → (0, Mon)
fn byday_item(s: &str) -> Option<(i32, Weekday)> {
    let s = s.trim();
    let split = s.find(|c: char| c.is_ascii_alphabetic())?;
    let n: i32 = if split == 0 { 0 } else { s[..split].parse().ok()? };
    Some((n, weekday(&s[split..])?))
}

fn days_in_month(y: i32, m: u32) -> u32 {
    let next = if m == 12 { NaiveDate::from_ymd_opt(y + 1, 1, 1) } else { NaiveDate::from_ymd_opt(y, m + 1, 1) };
    next.map(|d| (d - Duration::days(1)).day()).unwrap_or(28)
}

/// Days of month (y, m) matching BYDAY items like "2TU", "-1FR", "MO".
fn month_bydays(y: i32, m: u32, items: &[(i32, Weekday)]) -> Vec<u32> {
    let dim = days_in_month(y, m);
    let mut out = Vec::new();
    for &(n, wd) in items {
        let matching: Vec<u32> = (1..=dim)
            .filter(|d| NaiveDate::from_ymd_opt(y, m, *d).map(|x| x.weekday() == wd).unwrap_or(false))
            .collect();
        if n == 0 {
            out.extend(matching);
        } else if n > 0 {
            if let Some(d) = matching.get((n - 1) as usize) {
                out.push(*d);
            }
        } else if let Some(d) = matching.len().checked_sub((-n) as usize).and_then(|i| matching.get(i)) {
            out.push(*d);
        }
    }
    out
}

fn apply_setpos(mut days: Vec<u32>, setpos: &[i32]) -> Vec<u32> {
    days.sort();
    days.dedup();
    if setpos.is_empty() {
        return days;
    }
    let mut out = Vec::new();
    for &p in setpos {
        let idx = if p > 0 { (p - 1) as isize } else { days.len() as isize + p as isize };
        if idx >= 0 {
            if let Some(d) = days.get(idx as usize) {
                out.push(*d);
            }
        }
    }
    out.sort();
    out
}

#[derive(Default)]
struct Rule {
    freq: String,
    interval: i64,
    count: Option<usize>,
    until: Option<NaiveDateTime>,
    byday: Vec<(i32, Weekday)>,
    bymonthday: Vec<i32>,
    bymonth: Vec<u32>,
    bysetpos: Vec<i32>,
}

fn parse_rule(rrule: &str, zone: &str) -> Rule {
    let mut r = Rule { interval: 1, ..Default::default() };
    for part in rrule.split(';') {
        let mut kv = part.splitn(2, '=');
        let k = kv.next().unwrap_or("").trim().to_uppercase();
        let v = kv.next().unwrap_or("").trim();
        match k.as_str() {
            "FREQ" => r.freq = v.to_uppercase(),
            "INTERVAL" => r.interval = v.parse().unwrap_or(1).max(1),
            "COUNT" => r.count = v.parse().ok(),
            "UNTIL" => {
                r.until = parse_ical_time(v, None).map(|t| {
                    let wall = if t.utc { tz::utc_to_wall(t.time, zone) } else { t.time };
                    if t.date_only { wall + Duration::days(1) - Duration::seconds(1) } else { wall }
                })
            }
            "BYDAY" => r.byday = v.split(',').filter_map(byday_item).collect(),
            "BYMONTHDAY" => r.bymonthday = v.split(',').filter_map(|x| x.trim().parse().ok()).collect(),
            "BYMONTH" => r.bymonth = v.split(',').filter_map(|x| x.trim().parse().ok()).collect(),
            "BYSETPOS" => r.bysetpos = v.split(',').filter_map(|x| x.trim().parse().ok()).collect(),
            _ => {}
        }
    }
    r
}

/// Start times (wall times in the event's zone) of an event that begins at
/// `start` and repeats by `rrule`, at or after `from` and before `to`.
/// `exdates` are skipped occurrences ("YYYY-MM-DDTHH:MM" wall times).
pub fn occurrences(
    start: NaiveDateTime,
    rrule: &str,
    from: NaiveDateTime,
    to: NaiveDateTime,
    exdates: &[String],
    zone: &str,
) -> Vec<NaiveDateTime> {
    let skip = |t: &NaiveDateTime| exdates.iter().any(|x| x == &crate::data::fmt_dt(*t));
    if rrule.trim().is_empty() {
        return if skip(&start) { vec![] } else { vec![start] };
    }
    let r = parse_rule(rrule, zone);
    let time = start.time();
    let mut out = Vec::new();
    let mut n = 0usize;
    // Returns false when the series is over.
    let mut push = |t: NaiveDateTime, out: &mut Vec<NaiveDateTime>| -> bool {
        if t < start {
            return true;
        }
        if let Some(u) = r.until {
            if t > u {
                return false;
            }
        }
        if let Some(c) = r.count {
            if n >= c {
                return false;
            }
        }
        if t >= to {
            return false;
        }
        n += 1;
        if t >= from && !skip(&t) {
            out.push(t);
        }
        true
    };
    let month_days = |y: i32, m: u32| -> Vec<u32> {
        let dim = days_in_month(y, m) as i32;
        let mut days: Vec<u32> = Vec::new();
        if !r.byday.is_empty() {
            days = month_bydays(y, m, &r.byday);
            if !r.bymonthday.is_empty() {
                days.retain(|d| r.bymonthday.iter().any(|md| (if *md < 0 { dim + md + 1 } else { *md }) == *d as i32));
            }
        } else if !r.bymonthday.is_empty() {
            for md in &r.bymonthday {
                let d = if *md < 0 { dim + md + 1 } else { *md };
                if d >= 1 && d <= dim {
                    days.push(d as u32);
                }
            }
        } else if start.day() as i32 <= dim {
            days.push(start.day());
        }
        apply_setpos(days, &r.bysetpos)
    };
    let mut guard = 0;
    match r.freq.as_str() {
        "DAILY" => {
            let mut t = start;
            while guard < 40000 {
                let ok_day = r.byday.is_empty() || r.byday.iter().any(|(_, wd)| *wd == t.weekday());
                let ok_month = r.bymonth.is_empty() || r.bymonth.contains(&t.month());
                if ok_day && ok_month && !push(t, &mut out) {
                    break;
                }
                if t >= to {
                    break;
                }
                t += Duration::days(r.interval);
                guard += 1;
            }
        }
        "WEEKLY" => {
            let days: Vec<Weekday> = if r.byday.is_empty() { vec![start.weekday()] } else { r.byday.iter().map(|x| x.1).collect() };
            let week0 = start.date() - Duration::days(start.weekday().num_days_from_monday() as i64);
            let mut w = 0i64;
            'outer: while guard < 6000 {
                let ws = week0 + Duration::weeks(w * r.interval);
                if ws.and_time(time) >= to + Duration::days(7) {
                    break;
                }
                let mut ds: Vec<NaiveDate> = days.iter().map(|d| ws + Duration::days(d.num_days_from_monday() as i64)).collect();
                ds.sort();
                ds.dedup();
                for d in ds {
                    if !r.bymonth.is_empty() && !r.bymonth.contains(&d.month()) {
                        continue;
                    }
                    if !push(d.and_time(time), &mut out) {
                        break 'outer;
                    }
                }
                w += 1;
                guard += 1;
            }
        }
        "MONTHLY" => {
            let mut m = 0i64;
            'outer: while guard < 3000 {
                let total = start.month0() as i64 + m * r.interval;
                let y = start.year() + total.div_euclid(12) as i32;
                let mo = total.rem_euclid(12) as u32 + 1;
                if NaiveDate::from_ymd_opt(y, mo, 1).map(|d| d.and_time(time) >= to).unwrap_or(true) {
                    break;
                }
                if r.bymonth.is_empty() || r.bymonth.contains(&mo) {
                    for d in month_days(y, mo) {
                        if let Some(date) = NaiveDate::from_ymd_opt(y, mo, d) {
                            if !push(date.and_time(time), &mut out) {
                                break 'outer;
                            }
                        }
                    }
                }
                m += 1;
                guard += 1;
            }
        }
        "YEARLY" => {
            let months: Vec<u32> = if r.bymonth.is_empty() { vec![start.month()] } else { let mut v = r.bymonth.clone(); v.sort(); v };
            let mut k = 0i64;
            'outer: while guard < 600 {
                let y = start.year() + (k * r.interval) as i32;
                if NaiveDate::from_ymd_opt(y, 1, 1).map(|d| d.and_time(time) >= to).unwrap_or(true) {
                    break;
                }
                for &mo in &months {
                    let days = if r.byday.is_empty() && r.bymonthday.is_empty() {
                        // Same day each year (29 Feb only in leap years).
                        if start.day() <= days_in_month(y, mo) { vec![start.day()] } else { vec![] }
                    } else {
                        month_days(y, mo)
                    };
                    for d in days {
                        if let Some(date) = NaiveDate::from_ymd_opt(y, mo, d) {
                            if !push(date.and_time(time), &mut out) {
                                break 'outer;
                            }
                        }
                    }
                }
                k += 1;
                guard += 1;
            }
        }
        _ => {
            if !skip(&start) {
                out.push(start)
            }
        }
    }
    out
}

// ---------------------------------------------------------------- times

pub struct IcalTime {
    /// Wall time as written (UTC when `utc`)
    pub time: NaiveDateTime,
    pub date_only: bool,
    pub utc: bool,
}

/// Reads "20261103T090000Z", "20261103T090000" or "20261103".
pub fn parse_ical_time(v: &str, _tzid: Option<&str>) -> Option<IcalTime> {
    let v = v.trim();
    if v.len() == 8 {
        let d = NaiveDate::parse_from_str(v, "%Y%m%d").ok()?;
        return Some(IcalTime { time: d.and_hms_opt(0, 0, 0)?, date_only: true, utc: false });
    }
    if let Some(z) = v.strip_suffix('Z') {
        let t = NaiveDateTime::parse_from_str(z, "%Y%m%dT%H%M%S").ok()?;
        return Some(IcalTime { time: t, date_only: false, utc: true });
    }
    let t = NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M%S")
        .or_else(|_| NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M"))
        .ok()?;
    Some(IcalTime { time: t, date_only: false, utc: false })
}

/// A date-time property → (wall time in `zone`, zone, all-day)
fn read_dt(params: &str, value: &str) -> Option<(NaiveDateTime, String, bool)> {
    let t = parse_ical_time(value, None)?;
    if t.date_only {
        return Some((t.time, String::new(), true));
    }
    if t.utc {
        return Some((t.time, "UTC".into(), false));
    }
    let zone = param(params, "TZID").map(|z| tz::canonical(&z)).unwrap_or_default();
    Some((t.time, zone, false))
}

// ---------------------------------------------------------- content lines

/// Unfolded content lines: (NAME, params, value)
pub fn content_lines(text: &str) -> Vec<(String, String, String)> {
    unfold(text)
        .into_iter()
        .filter_map(|l| {
            let colon = find_value_colon(&l)?;
            let (head, value) = (&l[..colon], &l[colon + 1..]);
            let mut hp = head.splitn(2, ';');
            let name = hp.next()?.to_uppercase();
            let name = name.rsplit('.').next().unwrap_or(&name).to_string();
            Some((name, hp.next().unwrap_or("").to_string(), value.to_string()))
        })
        .collect()
}

fn unfold(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for raw in text.split('\n') {
        let raw = raw.trim_end_matches('\r');
        if (raw.starts_with(' ') || raw.starts_with('\t')) && !lines.is_empty() {
            lines.last_mut().unwrap().push_str(&raw[1..]);
        } else {
            lines.push(raw.to_string());
        }
    }
    lines
}

fn find_value_colon(l: &str) -> Option<usize> {
    let mut in_quote = false;
    for (i, c) in l.char_indices() {
        match c {
            '"' => in_quote = !in_quote,
            ':' if !in_quote => return Some(i),
            _ => {}
        }
    }
    None
}

pub fn unescape(v: &str) -> String {
    v.replace("\\n", "\n")
        .replace("\\N", "\n")
        .replace("\\,", ",")
        .replace("\\;", ";")
        .replace("\\\\", "\\")
}

pub fn escape(v: &str) -> String {
    v.replace('\\', "\\\\").replace('\n', "\\n").replace(',', "\\,").replace(';', "\\;")
}

/// Folds a content line at 75 octets.
fn fold(line: &str) -> String {
    let mut out = String::new();
    let mut count = 0;
    for ch in line.chars() {
        let len = ch.len_utf8();
        if count + len > 75 {
            out.push_str("\r\n ");
            count = 1;
        }
        out.push(ch);
        count += len;
    }
    out.push_str("\r\n");
    out
}

pub fn param(params: &str, key: &str) -> Option<String> {
    params.split(';').find_map(|p| {
        let mut kv = p.splitn(2, '=');
        let k = kv.next()?.trim();
        if k.eq_ignore_ascii_case(key) {
            Some(kv.next().unwrap_or("").trim_matches('"').to_string())
        } else {
            None
        }
    })
}

// ------------------------------------------------------------------ VEVENT

#[derive(Default, Debug, Clone)]
pub struct IcsEvent {
    pub uid: String,
    pub title: String,
    /// Wall time in `zone`
    pub start: Option<NaiveDateTime>,
    pub end: Option<NaiveDateTime>,
    pub zone: String,
    pub all_day: bool,
    pub location: String,
    pub notes: String,
    pub rrule: String,
    /// Skipped occurrences, as wall times in `zone`
    pub exdates: Vec<String>,
    /// For a changed single occurrence: which one it replaces (wall time in `zone`)
    pub recurrence_id: Option<NaiveDateTime>,
    /// Alerts, minutes before the start
    pub alarms: Vec<i64>,
    pub attendees: Vec<crate::data::Attendee>,
    pub organizer: String,
    /// X-ALT-DESC in HTML (Outlook, Teams): the notes formatted
    pub notes_html: String,
    /// URL, or a video call's (Google's and Teams' own properties)
    pub url: String,
    pub sequence: u32,
    pub status: String,
}

fn mail_addr(v: &str) -> String {
    let v = v.trim();
    v.strip_prefix("mailto:").or_else(|| v.strip_prefix("MAILTO:")).unwrap_or(v).to_string()
}

/// METHOD of a calendar object (REQUEST, REPLY, CANCEL…), "" for plain events.
pub fn calendar_method(text: &str) -> String {
    content_lines(text)
        .into_iter()
        .find(|(n, _, _)| n == "METHOD")
        .map(|(_, _, v)| v.trim().to_uppercase())
        .unwrap_or_default()
}

pub fn parse_vevents(text: &str) -> Vec<IcsEvent> {
    let mut out = Vec::new();
    let mut cur: Option<IcsEvent> = None;
    let mut duration: Option<Duration> = None;
    let mut end_zone = String::new();
    let mut depth_other = 0; // inside VALARM etc.
    let mut in_alarm = false;
    for (name, params, value) in content_lines(text) {
        match name.as_str() {
            "BEGIN" if value.eq_ignore_ascii_case("VEVENT") => {
                cur = Some(IcsEvent::default());
                duration = None;
                end_zone.clear();
            }
            "BEGIN" if cur.is_some() => {
                depth_other += 1;
                in_alarm = depth_other == 1 && value.eq_ignore_ascii_case("VALARM");
            }
            "END" if value.eq_ignore_ascii_case("VEVENT") => {
                depth_other = 0;
                in_alarm = false;
                if let Some(mut e) = cur.take() {
                    if let (Some(en), false) = (e.end, end_zone.is_empty() || end_zone == e.zone) {
                        // DTEND written in another zone: convert to the start's zone.
                        e.end = Some(tz::utc_to_wall(tz::wall_to_utc(en, &end_zone), &e.zone));
                        if e.zone.is_empty() {
                            e.end = Some(tz::to_local(en, &end_zone));
                        }
                    }
                    if e.end.is_none() {
                        e.end = e.start.map(|s| {
                            s + duration.unwrap_or(if e.all_day { Duration::days(1) } else { Duration::zero() })
                        });
                    }
                    if e.start.is_some() {
                        out.push(e);
                    }
                }
            }
            "END" if cur.is_some() && depth_other > 0 => {
                depth_other -= 1;
                if depth_other == 0 {
                    in_alarm = false;
                }
            }
            _ => {
                let Some(e) = cur.as_mut() else { continue };
                if in_alarm {
                    if name == "TRIGGER" && param(&params, "VALUE").map(|v| !v.eq_ignore_ascii_case("DATE-TIME")).unwrap_or(true) {
                        if let Some(d) = parse_duration(&value) {
                            let mins = -d.num_minutes();
                            if !e.alarms.contains(&mins) {
                                e.alarms.push(mins);
                            }
                        }
                    }
                    continue;
                }
                if depth_other > 0 {
                    continue;
                }
                match name.as_str() {
                    "UID" => e.uid = value.trim().to_string(),
                    "SUMMARY" => e.title = unescape(&value),
                    "LOCATION" => e.location = unescape(&value),
                    "DESCRIPTION" => e.notes = unescape(&value),
                    "X-ALT-DESC" if param(&params, "FMTTYPE").is_some_and(|t| t.eq_ignore_ascii_case("text/html")) => e.notes_html = unescape(&value),
                    "URL" | "X-GOOGLE-CONFERENCE" | "X-MICROSOFT-SKYPETEAMSMEETINGURL" | "X-MICROSOFT-ONLINEMEETINGCONFLINK" if e.url.is_empty() => {
                        e.url = value.trim().to_string()
                    }
                    "RRULE" => e.rrule = value.trim().to_string(),
                    "SEQUENCE" => e.sequence = value.trim().parse().unwrap_or(0),
                    "STATUS" => e.status = value.trim().to_uppercase(),
                    "ORGANIZER" => e.organizer = mail_addr(&value),
                    "ATTENDEE" => e.attendees.push(crate::data::Attendee {
                        email: mail_addr(&value),
                        name: param(&params, "CN").unwrap_or_default(),
                        status: param(&params, "PARTSTAT").unwrap_or_else(|| "NEEDS-ACTION".into()).to_uppercase(),
                    }),
                    "DTSTART" => {
                        if let Some((t, zone, all_day)) = read_dt(&params, &value) {
                            e.start = Some(t);
                            e.zone = zone;
                            e.all_day = all_day;
                        }
                    }
                    "DTEND" => {
                        if let Some((t, zone, _)) = read_dt(&params, &value) {
                            e.end = Some(t);
                            end_zone = zone;
                        }
                    }
                    "DURATION" => duration = parse_duration(&value),
                    "RECURRENCE-ID" => {
                        if let Some((t, zone, _)) = read_dt(&params, &value) {
                            e.recurrence_id = Some(t);
                            e.exdates.push(format!("@{zone}@{}", crate::data::fmt_dt(t)));
                        }
                    }
                    "EXDATE" => {
                        for v in value.split(',') {
                            if let Some((t, zone, _)) = read_dt(&params, v) {
                                e.exdates.push(format!("@{zone}@{}", crate::data::fmt_dt(t)));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    // EXDATE / RECURRENCE-ID values were kept with their own zone; express
    // them in the event's zone now that it is known.
    for e in out.iter_mut() {
        let zone = e.zone.clone();
        e.exdates = e
            .exdates
            .iter()
            .filter_map(|x| {
                let mut parts = x.splitn(3, '@').skip(1);
                let z = parts.next()?;
                let t = crate::data::parse_dt(parts.next()?)?;
                let wall = if z == zone || z.is_empty() || zone.is_empty() { t } else { tz::utc_to_wall(tz::wall_to_utc(t, z), &zone) };
                Some(crate::data::fmt_dt(wall))
            })
            .collect();
        if e.recurrence_id.is_some() {
            // The RECURRENCE-ID value (in the event's zone) is the replaced occurrence.
            e.recurrence_id = e.exdates.last().and_then(|x| crate::data::parse_dt(x));
            e.exdates.clear();
        }
    }
    out
}

fn parse_duration(v: &str) -> Option<Duration> {
    let neg = v.trim().starts_with('-');
    let mut total = Duration::zero();
    let mut num = String::new();
    for c in v.chars() {
        if c.is_ascii_digit() {
            num.push(c);
        } else {
            let n: i64 = num.parse().unwrap_or(0);
            num.clear();
            total += match c {
                'W' => Duration::weeks(n),
                'D' => Duration::days(n),
                'H' => Duration::hours(n),
                'M' => Duration::minutes(n),
                'S' => Duration::seconds(n),
                _ => Duration::zero(),
            };
        }
    }
    Some(if neg { -total } else { total })
}

pub struct EventFields<'a> {
    pub uid: &'a str,
    pub title: &'a str,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub zone: &'a str,
    pub all_day: bool,
    pub location: &'a str,
    pub notes: &'a str,
    /// The notes formatted (empty: plain only)
    pub notes_html: &'a str,
    pub rrule: &'a str,
    pub exdates: &'a [String],
    pub alarms: &'a [i64],
    pub attendees: &'a [crate::data::Attendee],
    pub organizer: &'a str,
    /// Set for one changed occurrence of a series
    pub recurrence_id: Option<NaiveDateTime>,
}

fn dt_lines(name: &str, t: NaiveDateTime, zone: &str, all_day: bool) -> String {
    if all_day {
        format!("{name};VALUE=DATE:{}", t.format("%Y%m%d"))
    } else if zone == "UTC" {
        format!("{name}:{}", t.format("%Y%m%dT%H%M%SZ"))
    } else if zone.is_empty() {
        format!("{name}:{}", t.format("%Y%m%dT%H%M%S"))
    } else {
        format!("{name};TZID={zone}:{}", t.format("%Y%m%dT%H%M%S"))
    }
}

fn event_lines(f: &EventFields) -> Vec<String> {
    let mut l = vec![
        format!("UID:{}", f.uid),
        format!("DTSTAMP:{}", chrono::Utc::now().format("%Y%m%dT%H%M%SZ")),
    ];
    if let Some(r) = f.recurrence_id {
        l.push(dt_lines("RECURRENCE-ID", r, f.zone, f.all_day));
    }
    l.push(dt_lines("DTSTART", f.start, f.zone, f.all_day));
    l.push(dt_lines("DTEND", f.end, f.zone, f.all_day));
    l.push(format!("SUMMARY:{}", escape(f.title)));
    if !f.location.is_empty() {
        l.push(format!("LOCATION:{}", escape(f.location)));
    }
    if !f.notes.is_empty() {
        l.push(format!("DESCRIPTION:{}", escape(f.notes)));
    }
    if !f.notes_html.is_empty() {
        l.push(format!("X-ALT-DESC;FMTTYPE=text/html:{}", escape(f.notes_html)));
    }
    if !f.rrule.is_empty() && f.recurrence_id.is_none() {
        l.push(format!("RRULE:{}", f.rrule));
    }
    if f.recurrence_id.is_none() {
        for x in f.exdates {
            if let Some(t) = crate::data::parse_dt(x) {
                l.push(dt_lines("EXDATE", t, f.zone, f.all_day));
            }
        }
    }
    if !f.organizer.is_empty() {
        l.push(format!("ORGANIZER:mailto:{}", f.organizer));
    }
    for a in f.attendees {
        let cn = if a.name.is_empty() { String::new() } else { format!(";CN=\"{}\"", a.name.replace('"', "")) };
        let st = if a.status.is_empty() { "NEEDS-ACTION" } else { a.status.as_str() };
        l.push(format!("ATTENDEE{cn};ROLE=REQ-PARTICIPANT;PARTSTAT={st};RSVP=TRUE:mailto:{}", a.email));
    }
    for m in f.alarms {
        l.push("BEGIN:VALARM".into());
        l.push("ACTION:DISPLAY".into());
        l.push(format!("DESCRIPTION:{}", escape(f.title)));
        l.push(if *m == 0 { "TRIGGER:PT0S".into() } else if *m < 0 { format!("TRIGGER:PT{}M", -m) } else { format!("TRIGGER:-PT{m}M") });
        l.push("END:VALARM".into());
    }
    l
}

fn vevent_block(f: &EventFields, extra: &[String]) -> String {
    let mut out = String::from("BEGIN:VEVENT\r\n");
    for x in event_lines(f) {
        out.push_str(&fold(&x));
    }
    for x in extra {
        out.push_str(&fold(x));
    }
    out.push_str("END:VEVENT\r\n");
    out
}

/// A whole new calendar object for an event (and its changed occurrences).
pub fn new_vcalendar(f: &EventFields, overrides: &[EventFields]) -> String {
    let mut out = String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Almanac//EN\r\nCALSCALE:GREGORIAN\r\n");
    out.push_str(&vevent_block(f, &[]));
    for o in overrides {
        out.push_str(&vevent_block(o, &[]));
    }
    out.push_str("END:VCALENDAR\r\n");
    out
}

/// An invitation message (iMIP): METHOD REQUEST, REPLY or CANCEL.
/// For REPLY, `f.attendees` should hold just the person replying.
pub fn imip(method: &str, f: &EventFields) -> String {
    let mut out = format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Almanac//EN\r\nMETHOD:{method}\r\n");
    let extra = if method == "CANCEL" { vec!["STATUS:CANCELLED".to_string()] } else { vec![] };
    out.push_str(&vevent_block(f, &extra));
    out.push_str("END:VCALENDAR\r\n");
    out
}

/// Rewrites the series in an existing calendar object, keeping everything
/// this app doesn't edit (time zone data, unknown properties). Changed
/// occurrences are replaced by `overrides`.
pub fn update_vcalendar(raw: &str, f: &EventFields, overrides: &[EventFields]) -> String {
    if raw.trim().is_empty() {
        return new_vcalendar(f, overrides);
    }
    const OWNED: [&str; 15] = [
        "UID", "DTSTAMP", "DTSTART", "DTEND", "DURATION", "SUMMARY", "LOCATION", "DESCRIPTION", "RRULE", "EXDATE",
        "ATTENDEE", "ORGANIZER", "RECURRENCE-ID", "SEQUENCE", "METHOD",
    ];
    let prop = |l: &str| l.to_uppercase().split(|c| c == ':' || c == ';').next().unwrap_or("").to_string();
    let mut out = String::new();
    // Lines of the VEVENT being read, with their nesting depth (0 = the event itself).
    let mut event: Option<Vec<(usize, String)>> = None;
    let mut depth = 0usize;
    let mut done = false;
    let mut overrides_written = false;
    for l in unfold(raw) {
        if l.trim().is_empty() {
            continue;
        }
        let name = prop(&l);
        let value_upper = l.to_uppercase();
        match event.as_mut() {
            None => {
                if value_upper.starts_with("BEGIN:VEVENT") {
                    event = Some(Vec::new());
                    depth = 0;
                } else if value_upper.starts_with("END:VCALENDAR") {
                    if !overrides_written {
                        for o in overrides {
                            out.push_str(&vevent_block(o, &[]));
                        }
                        overrides_written = true;
                    }
                    out.push_str(&fold(&l));
                } else if name != "METHOD" {
                    out.push_str(&fold(&l));
                }
            }
            Some(lines) => {
                if value_upper.starts_with("END:VEVENT") && depth == 0 {
                    let is_master = !lines.iter().any(|(d, x)| *d == 0 && prop(x) == "RECURRENCE-ID");
                    if is_master && !done {
                        done = true;
                        // Keep unknown lines, but not our own fields or old alarms.
                        let mut keep: Vec<String> = Vec::new();
                        let mut skip_depth: Option<usize> = None;
                        for (d, x) in lines.iter() {
                            let p = prop(x);
                            if let Some(sd) = skip_depth {
                                if p == "END" && *d == sd {
                                    skip_depth = None;
                                }
                                continue;
                            }
                            if p == "BEGIN" && x.to_uppercase().contains("VALARM") {
                                skip_depth = Some(*d);
                                continue;
                            }
                            if *d == 0 && OWNED.contains(&p.as_str()) {
                                continue;
                            }
                            keep.push(x.clone());
                        }
                        out.push_str(&vevent_block(f, &keep));
                    }
                    // Old changed occurrences are replaced by `overrides`.
                    event = None;
                    continue;
                }
                if name == "BEGIN" {
                    depth += 1;
                    lines.push((depth, l));
                } else if name == "END" {
                    lines.push((depth, l));
                    depth = depth.saturating_sub(1);
                } else {
                    lines.push((depth, l));
                }
            }
        }
    }
    if !done {
        return new_vcalendar(f, overrides);
    }
    out
}

// ------------------------------------------------------------------- vCard

#[derive(Default, Debug)]
pub struct VCard {
    pub uid: String,
    pub first: String,
    pub last: String,
    pub full: String,
    pub org: String,
    pub title: String,
    pub emails: Vec<(String, String)>,
    pub phones: Vec<(String, String)>,
    pub address: String,
    pub birthday: String,
    pub notes: String,
    pub nickname: String,
    pub urls: Vec<(String, String)>,
    /// Picture bytes (JPEG/PNG) when the card has one inline
    pub photo: Vec<u8>,
}

fn b64(v: &str) -> Vec<u8> {
    use base64::Engine;
    let clean: String = v.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '+' || *c == '/').collect();
    base64::engine::general_purpose::STANDARD_NO_PAD.decode(clean).unwrap_or_default()
}

pub fn parse_vcard(text: &str) -> VCard {
    let mut c = VCard::default();
    for (name, params, value) in content_lines(text) {
        let label = param(&params, "TYPE")
            .map(|t| {
                t.split(',')
                    .map(|x| x.trim().to_lowercase())
                    .find(|x| !x.is_empty() && x != "pref" && x != "internet" && x != "voice")
                    .unwrap_or_default()
            })
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| "other".into());
        match name.as_str() {
            "UID" => c.uid = value.trim().to_string(),
            "FN" => c.full = unescape(&value),
            "N" => {
                let parts: Vec<&str> = value.split(';').collect();
                c.last = unescape(parts.first().copied().unwrap_or(""));
                c.first = unescape(parts.get(1).copied().unwrap_or(""));
            }
            "ORG" => c.org = unescape(value.split(';').next().unwrap_or("")),
            "TITLE" => c.title = unescape(&value),
            "NICKNAME" => c.nickname = unescape(&value),
            "URL" => c.urls.push((if label == "other" { "homepage".into() } else { label }, unescape(&value))),
            "EMAIL" => c.emails.push((label, unescape(&value))),
            "TEL" => c.phones.push((if label == "cell" { "mobile".into() } else { label }, unescape(value.trim_start_matches("tel:")))),
            "ADR" => {
                let parts: Vec<String> = value.split(';').map(unescape).filter(|p| !p.trim().is_empty()).collect();
                if c.address.is_empty() {
                    c.address = parts.join("\n");
                }
            }
            "BDAY" => {
                let v = value.replace('-', "");
                let v = v.trim_start_matches("--");
                if v.len() >= 8 {
                    c.birthday = format!("{}-{}-{}", &v[0..4], &v[4..6], &v[6..8]);
                }
            }
            "NOTE" => c.notes = unescape(&value),
            "PHOTO" => {
                let enc = param(&params, "ENCODING").unwrap_or_default().to_lowercase();
                if enc == "b" || enc == "base64" {
                    c.photo = b64(&value);
                } else if let Some(rest) = value.strip_prefix("data:") {
                    if let Some(i) = rest.find("base64,") {
                        c.photo = b64(&rest[i + 7..]);
                    }
                }
            }
            _ => {}
        }
    }
    if c.first.is_empty() && c.last.is_empty() && !c.full.is_empty() {
        let mut it = c.full.rsplitn(2, ' ');
        let last = it.next().unwrap_or("").to_string();
        match it.next() {
            Some(first) => {
                c.first = first.to_string();
                c.last = last;
            }
            None => c.first = last,
        }
    }
    c
}

/// Saves card picture bytes in the app's data folder; returns the file path.
pub fn store_photo(bytes: &[u8]) -> String {
    if bytes.len() < 16 {
        return String::new();
    }
    let ext = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) { "png" } else { "jpg" };
    let mut h: u64 = 1469598103934665603;
    for b in bytes {
        h = (h ^ *b as u64).wrapping_mul(1099511628211);
    }
    let dir = crate::data::data_dir().join("photos");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("{h:016x}.{ext}"));
    if !path.exists() {
        let _ = std::fs::write(&path, bytes);
    }
    path.display().to_string()
}

/// Lines for the fields this app edits.
fn card_lines(c: &crate::data::Contact, uid: &str) -> Vec<String> {
    let mut l = vec![
        format!("UID:{uid}"),
        format!("N:{};{};;;", escape(&c.last), escape(&c.first)),
        format!("FN:{}", escape(&crate::data::display_name(c))),
    ];
    if !c.nickname.is_empty() {
        l.push(format!("NICKNAME:{}", escape(&c.nickname)));
    }
    if !c.company.is_empty() {
        l.push(format!("ORG:{}", escape(&c.company)));
    }
    if !c.job_title.is_empty() {
        l.push(format!("TITLE:{}", escape(&c.job_title)));
    }
    for e in &c.emails {
        l.push(format!("EMAIL;TYPE={}:{}", e.label.to_uppercase(), escape(&e.value)));
    }
    for p in &c.phones {
        let t = if p.label == "mobile" { "CELL".to_string() } else { p.label.to_uppercase() };
        l.push(format!("TEL;TYPE={t}:{}", escape(&p.value)));
    }
    for u in &c.urls {
        l.push(format!("URL;TYPE={}:{}", u.label.to_uppercase(), u.value));
    }
    if !c.address.is_empty() {
        l.push(format!("ADR;TYPE=HOME:;;{};;;;", escape(&c.address)));
    }
    if !c.birthday.is_empty() {
        l.push(format!("BDAY:{}", c.birthday));
    }
    if !c.notes.is_empty() {
        l.push(format!("NOTE:{}", escape(&c.notes)));
    }
    if !c.photo.is_empty() {
        if let Ok(bytes) = std::fs::read(&c.photo) {
            use base64::Engine;
            let kind = if c.photo.ends_with(".png") { "PNG" } else { "JPEG" };
            l.push(format!("PHOTO;ENCODING=b;TYPE={kind}:{}", base64::engine::general_purpose::STANDARD.encode(bytes)));
        }
    }
    l
}

/// A vCard 3.0 for a contact, keeping unknown lines from `raw` (extra
/// fields) when it came from a server.
pub fn write_vcard(c: &crate::data::Contact, uid: &str, raw: &str) -> String {
    const OWNED: [&str; 14] = ["UID", "N", "FN", "ORG", "TITLE", "EMAIL", "TEL", "ADR", "BDAY", "NOTE", "REV", "NICKNAME", "URL", "PHOTO"];
    let mut out = String::from("BEGIN:VCARD\r\nVERSION:3.0\r\n");
    for l in card_lines(c, uid) {
        out.push_str(&fold(&l));
    }
    for l in unfold(raw) {
        let upper = l.to_uppercase();
        let name = upper.split(|ch| ch == ':' || ch == ';').next().unwrap_or("");
        let name = name.rsplit('.').next().unwrap_or(name);
        if l.trim().is_empty() || OWNED.contains(&name) || ["BEGIN", "END", "VERSION", "PRODID"].contains(&name) {
            continue;
        }
        out.push_str(&fold(&l));
    }
    out.push_str(&fold(&format!("REV:{}", chrono::Utc::now().format("%Y%m%dT%H%M%SZ"))));
    out.push_str("END:VCARD\r\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dt(s: &str) -> NaiveDateTime {
        crate::data::parse_dt(s).unwrap()
    }

    #[test]
    fn second_tuesday_monthly() {
        let occ = occurrences(dt("2026-01-13T10:00"), "FREQ=MONTHLY;BYDAY=2TU", dt("2026-01-01T00:00"), dt("2026-05-01T00:00"), &[], "");
        let got: Vec<String> = occ.iter().map(|t| crate::data::fmt_dt(*t)).collect();
        assert_eq!(got, ["2026-01-13T10:00", "2026-02-10T10:00", "2026-03-10T10:00", "2026-04-14T10:00"]);
    }

    #[test]
    fn last_weekday_of_month() {
        let occ = occurrences(dt("2026-01-30T09:00"), "FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1", dt("2026-01-01T00:00"), dt("2026-04-01T00:00"), &[], "");
        let got: Vec<String> = occ.iter().map(|t| crate::data::fmt_dt(*t)).collect();
        assert_eq!(got, ["2026-01-30T09:00", "2026-02-27T09:00", "2026-03-31T09:00"]);
    }

    #[test]
    fn weekly_with_exdate_and_count() {
        let ex = vec!["2026-01-12T09:00".to_string()];
        let occ = occurrences(dt("2026-01-05T09:00"), "FREQ=WEEKLY;COUNT=3", dt("2026-01-01T00:00"), dt("2026-03-01T00:00"), &ex, "");
        let got: Vec<String> = occ.iter().map(|t| crate::data::fmt_dt(*t)).collect();
        assert_eq!(got, ["2026-01-05T09:00", "2026-01-19T09:00"]);
    }

    #[test]
    fn dst_stays_on_wall_time() {
        // A 09:00 New York meeting is 14:00 UTC in winter and 13:00 UTC in summer.
        let w = dt("2026-03-02T09:00");
        assert_eq!(crate::data::fmt_dt(tz::wall_to_utc(w, "America/New_York")), "2026-03-02T14:00");
        let s = dt("2026-03-16T09:00");
        assert_eq!(crate::data::fmt_dt(tz::wall_to_utc(s, "America/New_York")), "2026-03-16T13:00");
    }

    #[test]
    fn reads_tzid_and_exdate() {
        let ics = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:a\r\nDTSTART;TZID=Europe/Berlin:20260105T090000\r\nDTEND;TZID=Europe/Berlin:20260105T100000\r\nRRULE:FREQ=WEEKLY\r\nEXDATE:20260112T080000Z\r\nSUMMARY:Stand-up\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let e = &parse_vevents(ics)[0];
        assert_eq!(e.zone, "Europe/Berlin");
        assert_eq!(e.exdates, vec!["2026-01-12T09:00".to_string()]);
    }

    #[test]
    fn reads_formatted_notes_and_call_links() {
        let ics = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:a\r\nDTSTART:20260105T090000Z\r\nSUMMARY:Sync\r\nDESCRIPTION:Join here\r\n\
                   X-ALT-DESC;FMTTYPE=text/html:<p>Join <a href=\"https://teams.microsoft.com/l/x\">here</a></p>\r\n\
                   X-MICROSOFT-SKYPETEAMSMEETINGURL:https://teams.microsoft.com/l/x\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let e = &parse_vevents(ics)[0];
        assert_eq!(e.notes, "Join here", "the plain text stays the notes (written back as is)");
        assert!(e.notes_html.contains("<a href="));
        assert_eq!(e.url, "https://teams.microsoft.com/l/x");
    }

    #[test]
    fn update_keeps_alarm() {
        let raw = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:a\r\nDTSTART:20260105T090000Z\r\nSUMMARY:Old\r\nBEGIN:VALARM\r\nTRIGGER:-PT15M\r\nACTION:DISPLAY\r\nEND:VALARM\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let f = EventFields { uid: "a", title: "New", start: dt("2026-01-05T10:00"), end: dt("2026-01-05T11:00"), zone: "UTC", all_day: false, location: "", notes: "", notes_html: "", rrule: "", exdates: &[], alarms: &[15], attendees: &[], organizer: "", recurrence_id: None };
        let out = update_vcalendar(raw, &f, &[]);
        assert!(out.contains("SUMMARY:New"));
        assert!(!out.contains("SUMMARY:Old"));
        assert!(out.contains("TRIGGER:-PT15M"));
    }
}
