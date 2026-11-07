//! Time zones: event times are kept as wall-clock times in the event's own
//! IANA zone (or "floating" when there is none) and turned into this
//! computer's local time only for display, so repeats stay right across
//! daylight-saving changes and travel.

use chrono::{Local, LocalResult, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use std::str::FromStr;

/// Windows / Outlook zone names that show up in invitations.
const WINDOWS_ZONES: &[(&str, &str)] = &[
    ("Dateline Standard Time", "Etc/GMT+12"),
    ("Hawaiian Standard Time", "Pacific/Honolulu"),
    ("Alaskan Standard Time", "America/Anchorage"),
    ("Pacific Standard Time", "America/Los_Angeles"),
    ("Mountain Standard Time", "America/Denver"),
    ("US Mountain Standard Time", "America/Phoenix"),
    ("Central Standard Time", "America/Chicago"),
    ("Canada Central Standard Time", "America/Regina"),
    ("Central America Standard Time", "America/Guatemala"),
    ("Mexico Standard Time", "America/Mexico_City"),
    ("Eastern Standard Time", "America/New_York"),
    ("US Eastern Standard Time", "America/Indiana/Indianapolis"),
    ("Atlantic Standard Time", "America/Halifax"),
    ("Newfoundland Standard Time", "America/St_Johns"),
    ("E. South America Standard Time", "America/Sao_Paulo"),
    ("Argentina Standard Time", "America/Argentina/Buenos_Aires"),
    ("SA Pacific Standard Time", "America/Bogota"),
    ("Pacific SA Standard Time", "America/Santiago"),
    ("UTC", "UTC"),
    ("Coordinated Universal Time", "UTC"),
    ("GMT Standard Time", "Europe/London"),
    ("Greenwich Standard Time", "Atlantic/Reykjavik"),
    ("W. Europe Standard Time", "Europe/Berlin"),
    ("Central Europe Standard Time", "Europe/Budapest"),
    ("Central European Standard Time", "Europe/Warsaw"),
    ("Romance Standard Time", "Europe/Paris"),
    ("E. Europe Standard Time", "Europe/Chisinau"),
    ("FLE Standard Time", "Europe/Kiev"),
    ("GTB Standard Time", "Europe/Bucharest"),
    ("Russian Standard Time", "Europe/Moscow"),
    ("Turkey Standard Time", "Europe/Istanbul"),
    ("Israel Standard Time", "Asia/Jerusalem"),
    ("Egypt Standard Time", "Africa/Cairo"),
    ("South Africa Standard Time", "Africa/Johannesburg"),
    ("W. Central Africa Standard Time", "Africa/Lagos"),
    ("E. Africa Standard Time", "Africa/Nairobi"),
    ("Arabian Standard Time", "Asia/Dubai"),
    ("Arab Standard Time", "Asia/Riyadh"),
    ("Iran Standard Time", "Asia/Tehran"),
    ("Pakistan Standard Time", "Asia/Karachi"),
    ("India Standard Time", "Asia/Kolkata"),
    ("Nepal Standard Time", "Asia/Kathmandu"),
    ("Bangladesh Standard Time", "Asia/Dhaka"),
    ("SE Asia Standard Time", "Asia/Bangkok"),
    ("China Standard Time", "Asia/Shanghai"),
    ("Singapore Standard Time", "Asia/Singapore"),
    ("Taipei Standard Time", "Asia/Taipei"),
    ("Tokyo Standard Time", "Asia/Tokyo"),
    ("Korea Standard Time", "Asia/Seoul"),
    ("AUS Eastern Standard Time", "Australia/Sydney"),
    ("E. Australia Standard Time", "Australia/Brisbane"),
    ("Cen. Australia Standard Time", "Australia/Adelaide"),
    ("AUS Central Standard Time", "Australia/Darwin"),
    ("W. Australia Standard Time", "Australia/Perth"),
    ("Tasmania Standard Time", "Australia/Hobart"),
    ("New Zealand Standard Time", "Pacific/Auckland"),
];

/// Reads a zone name the way calendars write it: IANA names, Windows names,
/// and prefixed forms like "/mozilla.org/20050126_1/Europe/Berlin".
pub fn parse_tz(name: &str) -> Option<Tz> {
    let n = name.trim().trim_matches('"');
    if n.is_empty() || n.eq_ignore_ascii_case("floating") {
        return None;
    }
    if n == "Z" || n.eq_ignore_ascii_case("utc") || n.eq_ignore_ascii_case("gmt") {
        return Some(Tz::UTC);
    }
    if let Ok(tz) = Tz::from_str(n) {
        return Some(tz);
    }
    if let Some((_, iana)) = WINDOWS_ZONES.iter().find(|(w, _)| w.eq_ignore_ascii_case(n)) {
        return Tz::from_str(iana).ok();
    }
    // Try the last two or three path parts ("…/America/Argentina/Buenos_Aires").
    let parts: Vec<&str> = n.split('/').filter(|p| !p.is_empty()).collect();
    for take in [3usize, 2, 1] {
        if parts.len() >= take {
            let cand = parts[parts.len() - take..].join("/");
            if let Ok(tz) = Tz::from_str(&cand) {
                return Some(tz);
            }
        }
    }
    None
}

/// Canonical name for storage ("" = floating).
pub fn canonical(name: &str) -> String {
    parse_tz(name).map(|t| t.name().to_string()).unwrap_or_default()
}

/// This computer's zone, e.g. "Europe/Berlin" (falls back to "UTC").
pub fn system_zone() -> String {
    iana_time_zone::get_timezone()
        .ok()
        .and_then(|n| parse_tz(&n))
        .map(|t| t.name().to_string())
        .unwrap_or_else(|| "UTC".into())
}

fn resolve<T: TimeZone>(r: LocalResult<chrono::DateTime<T>>, tz: &T, wall: NaiveDateTime) -> chrono::DateTime<T> {
    match r {
        LocalResult::Single(d) => d,
        LocalResult::Ambiguous(a, _) => a,
        // In a spring-forward gap: move forward by the gap like most calendars do.
        LocalResult::None => tz
            .from_local_datetime(&(wall + chrono::Duration::hours(1)))
            .earliest()
            .unwrap_or_else(|| tz.from_utc_datetime(&wall)),
    }
}

/// Wall time in `zone` → this computer's local wall time.
pub fn to_local(wall: NaiveDateTime, zone: &str) -> NaiveDateTime {
    match parse_tz(zone) {
        None => wall,
        Some(tz) => resolve(tz.from_local_datetime(&wall), &tz, wall)
            .with_timezone(&Local)
            .naive_local(),
    }
}

/// This computer's local wall time → wall time in `zone`.
pub fn from_local(local: NaiveDateTime, zone: &str) -> NaiveDateTime {
    match parse_tz(zone) {
        None => local,
        Some(tz) => resolve(Local.from_local_datetime(&local), &Local, local)
            .with_timezone(&tz)
            .naive_local(),
    }
}

/// UTC time → wall time in `zone` (floating zones get local time).
pub fn utc_to_wall(utc: NaiveDateTime, zone: &str) -> NaiveDateTime {
    match parse_tz(zone) {
        None => Utc.from_utc_datetime(&utc).with_timezone(&Local).naive_local(),
        Some(tz) => tz.from_utc_datetime(&utc).naive_local(),
    }
}

/// Wall time in `zone` → UTC.
pub fn wall_to_utc(wall: NaiveDateTime, zone: &str) -> NaiveDateTime {
    match parse_tz(zone) {
        None => resolve(Local.from_local_datetime(&wall), &Local, wall).naive_utc(),
        Some(tz) => resolve(tz.from_local_datetime(&wall), &tz, wall).naive_utc(),
    }
}

/// All zone names for the event editor.
pub fn all_zones() -> Vec<&'static str> {
    chrono_tz::TZ_VARIANTS.iter().map(|t| t.name()).collect()
}
