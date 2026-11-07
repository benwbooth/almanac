//! Live accounts: IMAP for mail, SMTP for sending, CalDAV for calendars and
//! CardDAV for contacts. Passwords live in the system keychain (Secret
//! Service on Linux, Keychain on macOS, Credential Manager on Windows).

use crate::data::*;
use crate::dav::Dav;
use crate::{ics, mime, tz};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

/// Internal keychain name (kept from before the app was named Almanac, so
/// saved passwords keep working).
const KEYCHAIN_SERVICE: &str = "craft-pim";
/// How many of the newest messages to keep per folder.
const MAIL_PER_FOLDER: u32 = 400;
/// Only the first part of very large messages is downloaded for reading.
const BODY_BYTES: u32 = 400_000;
/// Messages up to this size are downloaded whole, so their attachments can
/// be kept as files straight away; bigger ones get theirs when opened.
const WHOLE_BYTES: u32 = 8_000_000;
/// The background download of a whole account takes less: messages up to
/// this size whole, bigger ones just their start (the text; attachments come
/// when a message is opened). Mail servers limit how much an app may download
/// a day (Gmail's runs out fast on a big mailbox, then it slows to a trickle).
const BG_WHOLE_BYTES: u32 = 150_000;
/// Messages whose text is downloaded per server round trip.
const TEXT_BATCH: usize = 20;
/// The longest a new-mail or background download waits, in all, for messages
/// someone opened to come first.
const WAIT_FOR_OPENED: std::time::Duration = std::time::Duration::from_secs(10);
/// How many of each mailbox's newest messages a regular check refreshes the
/// read / flagged state of.
const RECENT_FLAGS: usize = 2000;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LiveAccount {
    pub id: String,
    /// Shown in the sidebar, e.g. "Work" or the address
    pub name: String,
    pub email: String,
    /// Name put on sent mail
    pub full_name: String,
    pub username: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub caldav_url: String,
    pub carddav_url: String,
    #[serde(default)]
    pub last_sync: String,
    #[serde(default)]
    pub last_error: String,
    /// "" (IMAP/SMTP + CalDAV/CardDAV with a password), "google" or "microsoft"
    #[serde(default)]
    pub provider: String,
    /// App (client) ID and secret for Google / Microsoft sign-in
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub client_secret: String,
    /// Microsoft directory ("common" for personal and work accounts)
    #[serde(default)]
    pub tenant: String,
    /// How mail comes in: "" (IMAP), "pop" (POP3: downloaded into this
    /// computer's Inbox; the incoming server and port are imap_host/imap_port)
    /// or "ews" (a Microsoft Exchange server; imap_host is its EWS address, or
    /// empty to find it by Autodiscover; mail is sent through it too)
    #[serde(default)]
    pub incoming: String,
}

impl LiveAccount {
    pub fn is_pop(&self) -> bool {
        self.incoming == "pop"
    }
    pub fn is_ews(&self) -> bool {
        self.incoming == "ews"
    }
}

/// An access token for a Google / Microsoft account.
pub fn token(a: &LiveAccount, r: crate::oauth::Resource) -> Result<String, String> {
    if crate::offline() {
        return Err("offline (ALMANAC_OFFLINE)".into());
    }
    crate::oauth::access_token(&a.id, &a.provider, &a.client_id, &a.client_secret, &a.tenant, r)
}

impl LiveAccount {
    pub fn source(&self) -> String {
        format!("live:{}", self.id)
    }
}

/// Known server settings for common providers, from the address's domain.
pub fn preset(email: &str) -> LiveAccount {
    let domain = email.rsplit('@').next().unwrap_or("").to_lowercase();
    let mut a = LiveAccount {
        email: email.to_string(),
        username: email.to_string(),
        imap_port: 993,
        smtp_port: 465,
        ..Default::default()
    };
    let (imap, smtp, smtp_port, cal, card): (&str, &str, u16, &str, &str) = match domain.as_str() {
        "gmail.com" | "googlemail.com" => ("imap.gmail.com", "smtp.gmail.com", 465, "https://apidata.googleusercontent.com/caldav/v2/", "https://www.googleapis.com/carddav/v1/principals/"),
        "icloud.com" | "me.com" | "mac.com" => ("imap.mail.me.com", "smtp.mail.me.com", 587, "https://caldav.icloud.com/", "https://contacts.icloud.com/"),
        "fastmail.com" | "fastmail.fm" | "fastmail.net" => ("imap.fastmail.com", "smtp.fastmail.com", 465, "https://caldav.fastmail.com/dav/", "https://carddav.fastmail.com/dav/"),
        "outlook.com" | "hotmail.com" | "live.com" | "msn.com" => ("outlook.office365.com", "smtp.office365.com", 587, "", ""),
        "yahoo.com" | "ymail.com" => ("imap.mail.yahoo.com", "smtp.mail.yahoo.com", 465, "https://caldav.calendar.yahoo.com/", "https://carddav.address.yahoo.com/"),
        "aol.com" => ("imap.aol.com", "smtp.aol.com", 465, "https://caldav.aol.com/", ""),
        "gmx.com" | "gmx.net" | "gmx.de" => ("imap.gmx.net", "mail.gmx.net", 587, "", ""),
        "posteo.de" | "posteo.net" => ("posteo.de", "posteo.de", 465, "https://posteo.de:8443/", "https://posteo.de:8843/"),
        "mailbox.org" => ("imap.mailbox.org", "smtp.mailbox.org", 465, "https://dav.mailbox.org/", "https://dav.mailbox.org/"),
        "zoho.com" | "zohomail.com" => ("imap.zoho.com", "smtp.zoho.com", 465, "https://calendar.zoho.com/", "https://contacts.zoho.com/"),
        "zoho.eu" | "zohomail.eu" => ("imap.zoho.eu", "smtp.zoho.eu", 465, "https://calendar.zoho.eu/", "https://contacts.zoho.eu/"),
        "mail.com" | "email.com" | "usa.com" | "post.com" => ("imap.mail.com", "smtp.mail.com", 587, "", ""),
        "web.de" => ("imap.web.de", "smtp.web.de", 587, "", ""),
        "yandex.com" | "yandex.ru" | "ya.ru" => ("imap.yandex.com", "smtp.yandex.com", 465, "https://caldav.yandex.ru/", "https://carddav.yandex.ru/"),
        // Proton Mail is end-to-end encrypted: its free Bridge app (proton.me/mail/bridge)
        // runs on this computer and offers it over IMAP/SMTP, with its own password.
        "proton.me" | "protonmail.com" | "protonmail.ch" | "pm.me" => ("127.0.0.1", "127.0.0.1", 1025, "", ""),
        _ => ("", "", 465, "", ""),
    };
    if imap.is_empty() {
        a.imap_host = format!("imap.{domain}");
        a.smtp_host = format!("smtp.{domain}");
        a.caldav_url = format!("https://{domain}/");
        a.carddav_url = format!("https://{domain}/");
    } else {
        a.imap_host = imap.into();
        a.smtp_host = smtp.into();
        a.smtp_port = smtp_port;
        if is_local(imap) {
            a.imap_port = 1143;
        }
        a.caldav_url = cal.into();
        a.carddav_url = card.into();
    }
    a.name = email.to_string();
    a
}

/// The mail providers offered when adding an account (Apple Mail's "Choose a
/// Mail account provider", with more of the services people use). `how`:
/// "oauth" (the provider's own sign-in page), "ews" (Exchange), "bridge"
/// (Proton Mail, through its Bridge app), "form" (address and password; the
/// servers come from `preset`), or "none" (can't be used by mail apps).
pub fn providers_json() -> serde_json::Value {
    serde_json::json!([
        {"id": "icloud", "name": "iCloud", "detail": "Mail, calendars and contacts", "glyph": "", "tint": "#3a8ef6", "how": "form", "domain": "icloud.com",
         "help": "iCloud wants an app-specific password: at account.apple.com › Sign-In and Security › App-Specific Passwords, make one and use it here.",
         "link": "https://account.apple.com/account/manage"},
        {"id": "google", "name": "Google", "detail": "Gmail, Google Calendar and Contacts", "glyph": "G", "tint": "#4285f4", "how": "oauth", "oauth": "google"},
        {"id": "microsoft", "name": "Outlook.com", "detail": "Outlook, Hotmail, Live and Microsoft 365", "glyph": "M", "tint": "#0078d4", "how": "oauth", "oauth": "microsoft"},
        {"id": "exchange", "name": "Microsoft Exchange", "detail": "A work or school Exchange server", "glyph": "Ex", "tint": "#0a5aa4", "how": "ews",
         "help": "Use your work or school address and password; Almanac finds the Exchange server from the address (or type its EWS address under Server settings)."},
        {"id": "yahoo", "name": "Yahoo", "detail": "Yahoo Mail, calendars and contacts", "glyph": "Y!", "tint": "#6001d2", "how": "oauth", "oauth": "yahoo"},
        {"id": "aol", "name": "AOL", "detail": "AOL Mail and calendars", "glyph": "Aol", "tint": "#2f2f2f", "how": "oauth", "oauth": "aol"},
        {"id": "proton", "name": "Proton Mail", "detail": "Through Proton Mail Bridge", "glyph": "P", "tint": "#6d4aff", "how": "bridge", "domain": "proton.me",
         "servers": {"imapHost": "127.0.0.1", "imapPort": 1143, "smtpHost": "127.0.0.1", "smtpPort": 1025, "caldavUrl": "", "carddavUrl": ""}},
        {"id": "fastmail", "name": "Fastmail", "detail": "Mail, calendars and contacts", "glyph": "F", "tint": "#0067b9", "how": "form", "domain": "fastmail.com",
         "help": "Fastmail wants an app password: Settings › Privacy & Security › Manage app passwords (allow mail, calendars and contacts).",
         "link": "https://app.fastmail.com/settings/security/apppasswords"},
        {"id": "zoho", "name": "Zoho Mail", "detail": "Mail, calendars and contacts", "glyph": "Z", "tint": "#e42527", "how": "form", "domain": "zoho.com",
         "help": "Turn on IMAP access in Zoho Mail (Settings › Mail Accounts › IMAP Access). With two-step sign-in, make an app-specific password at accounts.zoho.com.",
         "link": "https://accounts.zoho.com/home#security/app_password"},
        {"id": "gmx", "name": "GMX", "detail": "GMX Mail", "glyph": "GMX", "tint": "#1c449b", "how": "form", "domain": "gmx.com",
         "help": "Turn on IMAP access in GMX first (E-mail › Settings › POP3 & IMAP)."},
        {"id": "mailcom", "name": "mail.com", "detail": "mail.com Mail", "glyph": "m", "tint": "#004788", "how": "form", "domain": "mail.com",
         "help": "Turn on IMAP access in mail.com first (E-mail › Settings › POP3 & IMAP)."},
        {"id": "yandex", "name": "Yandex Mail", "detail": "Mail, calendars and contacts", "glyph": "Я", "tint": "#fc3f1d", "how": "form", "domain": "yandex.com",
         "help": "Turn on mail apps in Yandex Mail (Settings › Mail clients), then make an app password at id.yandex.com › Security.",
         "link": "https://id.yandex.com/security/app-passwords"},
        {"id": "mailboxorg", "name": "mailbox.org", "detail": "Mail, calendars and contacts", "glyph": "mb", "tint": "#3b8a3e", "how": "form", "domain": "mailbox.org",
         "help": "Use your mailbox.org password (or an app password if two-factor sign-in is on)."},
        {"id": "posteo", "name": "Posteo", "detail": "Mail, calendars and contacts", "glyph": "Po", "tint": "#8bbb2a", "how": "form", "domain": "posteo.de",
         "help": "Use your Posteo address and password."},
        {"id": "tuta", "name": "Tuta", "detail": "Only works in Tuta's own apps", "glyph": "T", "tint": "#850122", "how": "none",
         "help": "Tuta (Tutanota) keeps mail encrypted in a way only its own apps can read, and offers no IMAP or Bridge, so no other mail app can add it."},
        {"id": "hey", "name": "HEY", "detail": "Only works in HEY's own apps", "glyph": "H", "tint": "#5522fa", "how": "none",
         "help": "HEY doesn't let other mail apps connect (no IMAP), so it can only be used in HEY's own apps."}
    ])
}

/// Proton Mail Bridge running on this computer (it listens on 127.0.0.1:1143).
pub fn bridge_running() -> bool {
    std::net::TcpStream::connect_timeout(&std::net::SocketAddr::from(([127, 0, 0, 1], 1143)), std::time::Duration::from_millis(250)).is_ok()
}

// ---------------------------------------------------------------- passwords

static SESSION_PASSWORDS: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

/// Stores the password in the system keychain. If there is no keychain,
/// it is kept only until the app closes. Returns a note for the person.
pub fn save_password(id: &str, password: &str) -> Option<String> {
    let saved = keyring::Entry::new(KEYCHAIN_SERVICE, id).and_then(|e| e.set_password(password));
    let mut map = SESSION_PASSWORDS.lock().unwrap();
    map.get_or_insert_with(HashMap::new).insert(id.to_string(), password.to_string());
    saved.err().map(|e| format!("No system keychain ({e}); the password is kept until the app closes."))
}

pub fn password(id: &str) -> Option<String> {
    if let Some(p) = SESSION_PASSWORDS.lock().unwrap().as_ref().and_then(|m| m.get(id)) {
        return Some(p.clone());
    }
    keyring::Entry::new(KEYCHAIN_SERVICE, id).ok()?.get_password().ok()
}

pub fn forget_password(id: &str) {
    if let Ok(e) = keyring::Entry::new(KEYCHAIN_SERVICE, id) {
        let _ = e.delete_credential();
    }
    if let Some(m) = SESSION_PASSWORDS.lock().unwrap().as_mut() {
        m.remove(id);
    }
}

pub fn need_password(a: &LiveAccount) -> Result<String, String> {
    password(&a.id).ok_or_else(|| format!("{}: password missing, sign in again in Accounts", a.name))
}

// ---------------------------------------------------------------------- IMAP

type Session = imap::Session<imap::Connection>;

/// A mail server on this computer (Proton Mail Bridge and similar): its
/// certificate is its own, made on this computer, so it can't be checked
/// against the public ones; the connection never leaves the computer.
pub fn is_local(host: &str) -> bool {
    matches!(host.trim(), "127.0.0.1" | "localhost" | "::1")
}

/// Plain connections upgraded to encrypted ones (STARTTLS): the standard
/// port and the Proton Mail Bridge's.
fn imap_starttls(port: u16) -> bool {
    port == 143 || port == 1143
}

/// An encrypted connection to a mail server with time limits: a server (or
/// network) that stops answering mid-way gives an error after two minutes
/// instead of holding every later mail check up for ever.
fn tls_client(host: &str, port: u16) -> Result<imap::Client<imap::Connection>, String> {
    let conn: imap::Connection = Box::new(tls_stream(host, port)?);
    let mut client = imap::Client::new(conn);
    client.read_greeting().map_err(|e| format!("{host} didn't answer: {e}"))?;
    Ok(client)
}

/// Connections Almanac has open to one mail server at most: Gmail refuses
/// more than 15 per account ("Too many simultaneous connections"), counting
/// phones and other mail apps, so Almanac leaves room. More wait their turn.
const MAX_CONNECTIONS: usize = 8;
static OPEN: (std::sync::Mutex<Option<HashMap<String, usize>>>, std::sync::Condvar) = (std::sync::Mutex::new(None), std::sync::Condvar::new());

/// A place among a server's connections, given back when the connection closes.
#[derive(Debug)]
pub struct Place(String);
impl Place {
    fn take(server: &str) -> Place {
        let (lock, freed) = &OPEN;
        let mut open = lock.lock().unwrap();
        while open.get_or_insert_with(HashMap::new).get(server).copied().unwrap_or(0) >= MAX_CONNECTIONS {
            open = freed.wait(open).unwrap();
        }
        *open.get_or_insert_with(HashMap::new).entry(server.to_string()).or_default() += 1;
        Place(server.to_string())
    }
}
impl Drop for Place {
    fn drop(&mut self) {
        let (lock, freed) = &OPEN;
        if let Some(n) = lock.lock().unwrap().get_or_insert_with(HashMap::new).get_mut(&self.0) {
            *n = n.saturating_sub(1);
        }
        freed.notify_all();
    }
}

/// An encrypted connection holding its place (see MAX_CONNECTIONS).
#[derive(Debug)]
pub struct Limited {
    inner: rustls_connector::TlsStream<std::net::TcpStream>,
    _place: Place,
}
impl std::io::Read for Limited {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}
impl std::io::Write for Limited {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}
impl imap::extensions::idle::SetReadTimeout for Limited {
    fn set_read_timeout(&mut self, timeout: Option<std::time::Duration>) -> imap::Result<()> {
        self.inner.get_ref().set_read_timeout(timeout).map_err(imap::Error::Io)
    }
}

/// The encrypted connection under `tls_client` (its greeting not yet read).
fn tls_stream(host: &str, port: u16) -> Result<crate::imap_pipe::Stream, String> {
    let place = Place::take(&host.to_lowercase());
    let inner = tls_stream_open(host, port)?;
    Ok(Limited { inner, _place: place })
}

fn tls_stream_open(host: &str, port: u16) -> Result<rustls_connector::TlsStream<std::net::TcpStream>, String> {
    if crate::offline() {
        return Err("offline (ALMANAC_OFFLINE)".into());
    }
    use std::net::ToSocketAddrs;
    use std::time::Duration;
    let mut last = format!("Could not reach {host}");
    let mut tcp = None;
    for addr in (host, port).to_socket_addrs().map_err(|e| format!("Could not reach {host}: {e}"))? {
        match std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(20)) {
            Ok(t) => {
                tcp = Some(t);
                break;
            }
            Err(e) => last = format!("Could not reach {host}: {e}"),
        }
    }
    let tcp = tcp.ok_or(last)?;
    let _ = tcp.set_read_timeout(Some(Duration::from_secs(120)));
    let _ = tcp.set_write_timeout(Some(Duration::from_secs(120)));
    let connector = rustls_connector::RustlsConnectorConfig::new_with_native_certs()
        .map_err(|e| format!("No certificates to check {host} with: {e}"))?
        .connector_with_no_client_auth();
    connector.connect(host, tcp).map_err(|e| format!("Could not reach {host} securely: {e}"))
}

fn imap_connect(a: &LiveAccount) -> Result<Session, String> {
    if crate::offline() {
        return Err("offline (ALMANAC_OFFLINE)".into());
    }
    if !a.provider.is_empty() {
        let token = token(a, crate::oauth::Resource::Mail)?;
        let client = tls_client(&a.imap_host, a.imap_port)?;
        let auth = crate::oauth::XOAuth2 { user: a.email.clone(), token };
        return client
            .authenticate("XOAUTH2", &auth)
            .map_err(|(e, _)| format!("{} did not accept the sign-in: {e}", a.imap_host));
    }
    let pass = need_password(a)?;
    if !imap_starttls(a.imap_port) && !is_local(&a.imap_host) {
        return tls_client(&a.imap_host, a.imap_port)?
            .login(&a.username, &pass)
            .map_err(|(e, _)| format!("{} did not accept the sign-in: {e}", a.imap_host));
    }
    let mode = if imap_starttls(a.imap_port) { imap::ConnectionMode::StartTls } else { imap::ConnectionMode::Tls };
    let client = imap::ClientBuilder::new(a.imap_host.as_str(), a.imap_port)
        .mode(mode)
        .tls_kind(imap::TlsKind::Rust)
        .danger_skip_tls_verify(is_local(&a.imap_host))
        .connect()
        .map_err(|e| {
            if is_local(&a.imap_host) {
                format!("Could not reach the mail app on this computer ({}:{}). If this is Proton Mail, is Proton Mail Bridge running? ({e})", a.imap_host, a.imap_port)
            } else {
                format!("Could not reach {}: {e}", a.imap_host)
            }
        })?;
    client
        .login(&a.username, &pass)
        .map_err(|(e, _)| format!("{} did not accept the sign-in: {e}", a.imap_host))
}

fn folder_kind(name: &str, attrs: &[String]) -> &'static str {
    for at in attrs {
        match at.trim_start_matches('\\').to_lowercase().as_str() {
            "sent" => return "sent",
            "drafts" => return "drafts",
            "trash" => return "trash",
            "junk" => return "junk",
            "archive" | "all" => return "archive",
            _ => {}
        }
    }
    if name.eq_ignore_ascii_case("INBOX") {
        return "inbox";
    }
    guess_kind(name)
}

/// IMAP folder names use "modified UTF-7"; turn "&AOQ-" style parts into text.
fn decode_mutf7(s: &str) -> String {
    use base64::Engine;
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let Some(end) = after.find('-') else {
            out.push_str(&rest[i..]);
            return out;
        };
        let chunk = &after[..end];
        if chunk.is_empty() {
            out.push('&');
        } else {
            let b64 = chunk.replace(',', "/");
            if let Ok(bytes) = base64::engine::general_purpose::STANDARD_NO_PAD.decode(b64.trim_end_matches('=')) {
                let units: Vec<u16> = bytes.chunks(2).filter(|c| c.len() == 2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
                out.push_str(&String::from_utf16_lossy(&units));
            }
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Reads folders and the headers of the newest messages of each (sender,
/// subject, date, flags), which is quick; the text follows in the
/// background (`fetch_texts`).
///
/// With `batch`, new messages are handed over a few hundred at a time as they
/// arrive (for downloading a whole account, which can take hours) instead of
/// all at the end.
/// POP accounts have no folders or flags on the server: what's done to their
/// mail stays on this computer.
fn local_only<T: Default>(a: &LiveAccount) -> Option<Result<T, String>> {
    // (Exchange: mail comes whole at sync, so there's no text to fetch later;
    // drafts stay on this computer.)
    (a.is_pop() || a.is_ews()).then(|| Ok(T::default()))
}

pub fn sync_mail(
    a: &LiveAccount,
    imp: &mut Imported,
    known: &HashMap<String, Vec<u32>>,
    marks: &HashMap<String, crate::data::FolderMark>,
    all: bool,
    progress: &dyn Fn(String),
    batch: Option<&dyn Fn(Vec<Message>)>,
    // The background pass over the whole account (else a regular check).
    deep: bool,
) -> Result<(), String> {
    if a.is_pop() {
        return crate::pop::sync(a, imp, known, all, progress);
    }
    if a.is_ews() {
        return crate::ews::sync(a, imp, known, all, progress);
    }
    if !deep && quick_check(a, imp, known, marks, progress, batch)? {
        return Ok(());
    }
    let mut delta = crate::data::MailDelta::default();
    progress(format!("{}: signing in to the mail server…", a.name));
    let mut s = imap_connect(a)?;
    progress(format!("{}: looking for mailboxes that changed…", a.name));
    // Change counters (HIGHESTMODSEQ), so unchanged mailboxes can be skipped
    // and only changed flags fetched.
    let caps = s.capabilities().ok();
    let has = |c: &str| caps.as_ref().is_some_and(|k| k.has_str(c));
    let condstore = has("CONDSTORE") && has("ENABLE") && s.run_command_and_check_ok("ENABLE CONDSTORE").is_ok();
    // The mailboxes, with (LIST-STATUS) each one's counters in the same answer,
    // so unchanged ones aren't even opened: one round trip for the lot.
    let mut names: Vec<(String, String, Vec<String>)> = Vec::new(); // path, delimiter, attributes
    let mut status: HashMap<String, crate::data::FolderMark> = HashMap::new();
    let listed = if condstore && has("LIST-STATUS") {
        s.list_status(Some(""), Some("\"*\""), "(MESSAGES UIDNEXT UIDVALIDITY HIGHESTMODSEQ)").ok()
    } else {
        None
    };
    match listed {
        Some(l) => {
            for (n, mb) in l.iter() {
                if let Some(mb) = mb {
                    let m = crate::data::FolderMark { validity: mb.uid_validity.unwrap_or(0), uid_next: mb.uid_next.unwrap_or(0), exists: mb.exists, modseq: mb.highest_mod_seq.unwrap_or(0) };
                    status.insert(n.name().to_string(), m);
                }
                names.push((n.name().to_string(), n.delimiter().unwrap_or("/").to_string(), n.attributes().iter().map(|x| format!("{x:?}")).collect()));
            }
        }
        None => {
            for n in s.list(Some(""), Some("*")).map_err(|e| e.to_string())?.iter() {
                names.push((n.name().to_string(), n.delimiter().unwrap_or("/").to_string(), n.attributes().iter().map(|x| format!("{x:?}")).collect()));
            }
        }
    }
    let account = format!("{}:mail", a.source());
    imp.accounts.push(Account { id: account.clone(), name: a.name.clone(), email: a.email.clone(), source: a.source() });
    let mut folders = mailbox_list(names);
    if deep {
        // Whole account: the Inbox and Sent first, mailboxes that repeat the
        // others (Gmail's All Mail) and Junk / Trash last.
        let deep = |k: &str| match k {
            "inbox" => 0,
            "sent" => 1,
            "archive" => 4,
            "junk" | "trash" => 5,
            _ => 2,
        };
        folders.sort_by_key(|f| deep(&f.2));
    }
    // (Counted, so the progress bar shows how far through the mailboxes it is.)
    let folder_count = folders.len();
    for (folder_no, (path, pretty, kind)) in folders.into_iter().enumerate() {
        let folder_id = format!("{account}/{path}");
        imp.folders.push(Folder { id: folder_id.clone(), account: account.clone(), name: pretty.clone(), kind: kind.clone(), path: path.clone() });
        progress(counted(&format!("{}: reading {pretty}", a.name), folder_no, folder_count));
        let began = std::time::Instant::now();
        let here: Vec<u32> = known.get(&folder_id).cloned().unwrap_or_default();
        // As it was at the last look (from the mailbox list's counters): skipped.
        if let Some(now) = status.get(&path).filter(|now| unchanged(marks.get(&folder_id), now, here.len(), deep)) {
            delta.marks.push((folder_id, *now));
            continue;
        }
        let mb = match s.examine(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let mark = crate::data::FolderMark {
            validity: mb.uid_validity.unwrap_or(0),
            uid_next: mb.uid_next.unwrap_or(0),
            exists: mb.exists,
            modseq: if condstore { mb.highest_mod_seq.unwrap_or(0) } else { 0 },
        };
        // The last look, if it can be compared with this one.
        let before = marks.get(&folder_id).copied().filter(|b| mark.validity > 0 && mark.modseq > 0 && b.validity == mark.validity && b.modseq > 0);
        // Only what changed: new messages' headers, read/flag changes, and
        // messages deleted on the server. With "keep all mail", every message
        // is kept; otherwise the newest ones of each mailbox.
        let same_list = before.is_some_and(|b| same_messages(&b, &mark, here.len(), deep));
        if unchanged(before.as_ref(), &mark, here.len(), deep) {
            delta.marks.push((folder_id, mark));
            continue;
        }
        let mut whole = true;
        let mut on_server: Vec<u32> = if mb.exists == 0 {
            Vec::new()
        } else if same_list {
            here.clone()
        } else {
            match s.uid_search("ALL") {
                Ok(u) => u.into_iter().collect(),
                Err(e) => {
                    progress(format!("{}: {pretty}: {e}", a.name));
                    continue;
                }
            }
        };
        on_server.sort_unstable();
        let server_set: std::collections::HashSet<u32> = on_server.iter().copied().collect();
        let here_set: std::collections::HashSet<u32> = here.iter().copied().collect();
        for uid in &here {
            if !server_set.contains(uid) {
                delta.gone.push((folder_id.clone(), *uid));
            }
        }
        let window: std::collections::HashSet<u32> = if all {
            server_set.clone()
        } else {
            on_server.iter().rev().take(MAIL_PER_FOLDER as usize).copied().collect()
        };
        let new: Vec<u32> = on_server.iter().copied().filter(|u| !here_set.contains(u) && window.contains(u)).collect();
        // Newest first, so recent mail shows up soonest.
        let mut got = 0usize;
        for chunk in new.rchunks(500) {
            if new.len() > 200 {
                progress(counted(&format!("{}: {pretty}: getting the message list", a.name), got, new.len()));
            }
            match fetch_headers(&mut s, chunk, &folder_id, &a.source()) {
                Ok(m) => match batch {
                    Some(hand_over) => hand_over(m),
                    None => delta.new.extend(m),
                },
                Err(e) => {
                    whole = false;
                    progress(format!("{}: {pretty}: {e}", a.name))
                }
            }
            got += chunk.len();
        }
        // Read / flagged state of what's already here.
        // (A regular check looks at the newest ones; the background pass over the
        // whole account at all of them. Thousands of round trips otherwise kept a
        // check for new mail busy for minutes on a big mailbox.)
        let flag_rows = |f: &imap::types::Fetches, delta: &mut crate::data::MailDelta| {
            for x in f.iter() {
                let flags: Vec<String> = x.flags().iter().map(|f| format!("{f:?}")).collect();
                if let Some(uid) = x.uid.filter(|u| here_set.contains(u)) {
                    delta.flags.push((folder_id.clone(), uid, flags.iter().any(|f| f == "Seen"), flags.iter().any(|f| f == "Flagged"), flag_color(&flags)));
                }
            }
        };
        if let Some(b) = before {
            // Only the flags changed since the last look, however big the mailbox.
            if mb.exists > 0 && b.modseq != mark.modseq {
                match s.uid_fetch("1:*", format!("(UID FLAGS) (CHANGEDSINCE {})", b.modseq)) {
                    Ok(f) => flag_rows(&f, &mut delta),
                    Err(e) => {
                        whole = false;
                        progress(format!("{}: {pretty}: {e}", a.name))
                    }
                }
            }
        } else if !here.is_empty() {
            // In one command (each one can take seconds when the server is
            // busy): all of them when they'll be compared by counter from now
            // on, or for the whole-account pass; else the newest few thousand.
            let mut still: Vec<u32> = here.iter().copied().filter(|u| server_set.contains(u)).collect();
            still.sort_unstable();
            let from = if mark.modseq > 0 || deep { 1 } else { still.len().checked_sub(RECENT_FLAGS).map_or(1, |i| still[i]) };
            match s.uid_fetch(format!("{from}:*"), "(UID FLAGS)") {
                Ok(f) => flag_rows(&f, &mut delta),
                Err(e) => {
                    whole = false;
                    progress(format!("{}: {pretty}: {e}", a.name))
                }
            }
        }
        if whole && mark.modseq > 0 {
            delta.marks.push((folder_id, mark));
        }
        let took = began.elapsed().as_secs_f32();
        if took > 2.0 {
            eprintln!("almanac: checking {pretty} ({} messages) took {took:.1}s", mb.exists);
        }
    }
    imp.mail_delta = Some(delta);
    let _ = s.logout();
    Ok(())
}

/// The mailboxes that hold mail, (path, name shown, kind): the Inbox first,
/// then the special ones, then the rest by name.
fn mailbox_list(names: Vec<(String, String, Vec<String>)>) -> Vec<(String, String, String)> {
    let mut folders: Vec<(String, String, String)> = Vec::new();
    for (path, delim, attrs) in names {
        // (However the server spells it: \Noselect, \NoSelect, \NonExistent.)
        if attrs.iter().any(|x| {
            let x = x.to_lowercase();
            x.contains("noselect") || x.contains("nonexistent")
        }) {
            continue;
        }
        let display = decode_mutf7(&path).replace(delim.as_str(), "/");
        let kind = folder_kind(&display, &attrs);
        let pretty = if kind == "inbox" { "Inbox".to_string() } else { display.trim_start_matches("[Gmail]/").trim_start_matches("[Google Mail]/").to_string() };
        folders.push((path, pretty, kind.to_string()));
    }
    let rank = |k: &str| ["inbox", "drafts", "sent", "archive", "junk", "trash"].iter().position(|x| *x == k).unwrap_or(9);
    folders.sort_by(|x, y| rank(&x.2).cmp(&rank(&y.2)).then(x.1.to_lowercase().cmp(&y.1.to_lowercase())));
    folders
}

/// Accounts whose Inbox is being watched (see `watch_inbox`).
static WATCHING: std::sync::Mutex<Option<std::collections::HashSet<String>>> = std::sync::Mutex::new(None);

/// Watches an account's Inbox over a connection of its own (IMAP IDLE, as
/// phones do): when the server says mail has arrived, `on_new` runs (a check
/// for new mail). Waits again after each; connects again after a dropped
/// connection, a little later each time. Once per account.
pub fn watch_inbox(a: LiveAccount, on_new: impl Fn() + Send + 'static) {
    if crate::offline() || !WATCHING.lock().unwrap().get_or_insert_with(Default::default).insert(a.id.clone()) {
        return;
    }
    std::thread::spawn(move || {
        let mut pause = 30u64;
        loop {
            let watched = (|| -> Result<(), String> {
                let mut s = imap_connect(&a)?;
                s.examine("INBOX").map_err(|e| e.to_string())?;
                pause = 30;
                loop {
                    // (Servers end an idle connection after 30 minutes: started again before that.)
                    let mut idle = s.idle();
                    idle.timeout(std::time::Duration::from_secs(20 * 60));
                    let outcome = idle
                        .wait_while(|r| !matches!(r, imap::types::UnsolicitedResponse::Exists(_) | imap::types::UnsolicitedResponse::Recent(_)))
                        .map_err(|e| e.to_string())?;
                    if matches!(outcome, imap::extensions::idle::WaitOutcome::MailboxChanged) {
                        on_new();
                    }
                }
            })();
            if let Err(e) = watched {
                eprintln!("almanac: watching {}'s Inbox: {e} (again in {pause} s)", a.name);
            }
            std::thread::sleep(std::time::Duration::from_secs(pause));
            pause = (pause * 2).min(600);
        }
    });
}

/// The regular check for new mail with every command for the changed
/// mailboxes sent at once (see `imap_pipe`): on a slowed-down Gmail account
/// two or three waits of ten seconds or so instead of several per mailbox.
/// Needs a server with LIST-STATUS and CONDSTORE; Ok(false) otherwise, before
/// anything is changed, and the regular way is taken.
fn quick_check(
    a: &LiveAccount,
    imp: &mut Imported,
    known: &HashMap<String, Vec<u32>>,
    marks: &HashMap<String, crate::data::FolderMark>,
    progress: &dyn Fn(String),
    batch: Option<&dyn Fn(Vec<Message>)>,
) -> Result<bool, String> {
    use crate::imap_pipe::{quote, Auth, Pipe};
    if imap_starttls(a.imap_port) || is_local(&a.imap_host) {
        return Ok(false);
    }
    let auth = if a.provider.is_empty() {
        Auth::Password { user: a.username.clone(), pass: need_password(a)? }
    } else {
        Auth::OAuth { user: a.email.clone(), token: token(a, crate::oauth::Resource::Mail)? }
    };
    progress(format!("{}: signing in to the mail server…", a.name));
    let stream = match tls_stream(&a.imap_host, a.imap_port) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("almanac: quick check: {e}");
            return Ok(false);
        }
    };
    let mut pipe = match Pipe::sign_in(stream, &auth) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("almanac: quick check: {e}");
            return Ok(false);
        }
    };
    progress(format!("{}: looking for mailboxes that changed…", a.name));
    let listed = pipe.batch(&["ENABLE CONDSTORE".into(), "LIST \"\" \"*\" RETURN (STATUS (MESSAGES UIDNEXT UIDVALIDITY HIGHESTMODSEQ))".into()])?;
    if !listed[0].ok || !listed[1].ok || listed[1].statuses.is_empty() {
        eprintln!("almanac: quick check: no mailbox counters ({} / {}, {} counters)", listed[0].info, listed[1].info, listed[1].statuses.len());
        pipe.logout();
        return Ok(false);
    }
    let status: HashMap<String, crate::data::FolderMark> = listed[1]
        .statuses
        .iter()
        .map(|(name, exists, uid_next, validity, modseq)| (name.clone(), crate::data::FolderMark { validity: *validity, uid_next: *uid_next, exists: *exists, modseq: *modseq }))
        .collect();
    let account = format!("{}:mail", a.source());
    // (Mailboxes without counters can't be opened: Gmail's "[Gmail]" holder.)
    let folders: Vec<(String, String, String)> = mailbox_list(listed[1].lists.clone()).into_iter().filter(|f| status.contains_key(&f.0)).collect();
    // What each changed mailbox needs: since the last look (new messages'
    // headers, changed flags), or, never looked at in full, all of its list.
    struct Plan {
        path: String,
        id: String,
        pretty: String,
        before: Option<crate::data::FolderMark>,
        now: crate::data::FolderMark,
        first: usize,
        new_at: Option<usize>,
        flags_at: Option<usize>,
        search_at: Option<usize>,
    }
    let mut delta = crate::data::MailDelta::default();
    let mut plans: Vec<Plan> = Vec::new();
    let mut cmds: Vec<String> = Vec::new();
    for (path, pretty, _) in &folders {
        let id = format!("{account}/{path}");
        let here = known.get(&id).map_or(0, |h| h.len());
        let now = status[path];
        let before = marks.get(&id).copied().filter(|b| now.validity > 0 && now.modseq > 0 && b.validity == now.validity && b.modseq > 0);
        if unchanged(before.as_ref(), &now, here, false) {
            delta.marks.push((id, now));
            continue;
        }
        // (Thousands of new messages: listed in full like a new mailbox.)
        let before = before.filter(|b| now.uid_next >= b.uid_next && now.uid_next - b.uid_next <= 2000);
        let first = cmds.len();
        cmds.push(format!("EXAMINE {}", quote(path)));
        let (mut new_at, mut flags_at, mut search_at) = (None, None, None);
        match before {
            Some(b) => {
                if now.uid_next > b.uid_next {
                    cmds.push(format!("UID FETCH {}:* (UID FLAGS INTERNALDATE BODY.PEEK[HEADER])", b.uid_next));
                    new_at = Some(cmds.len() - 1);
                }
                if now.modseq != b.modseq && now.exists > 0 {
                    cmds.push(format!("UID FETCH 1:* (UID FLAGS) (CHANGEDSINCE {})", b.modseq));
                    flags_at = Some(cmds.len() - 1);
                }
            }
            None if now.exists > 0 => {
                cmds.push("UID SEARCH ALL".into());
                search_at = Some(cmds.len() - 1);
                if here > 0 {
                    cmds.push("UID FETCH 1:* (UID FLAGS)".into());
                    flags_at = Some(cmds.len() - 1);
                }
            }
            None => {}
        }
        plans.push(Plan { path: path.clone(), id, pretty: pretty.clone(), before, now, first, new_at, flags_at, search_at });
    }
    if !plans.is_empty() {
        let names: Vec<&str> = plans.iter().map(|p| p.pretty.as_str()).collect();
        let what = if names.len() <= 3 { names.join(", ") } else { format!("{} mailboxes", names.len()) };
        progress(format!("{}: reading {what}…", a.name));
    }
    let replies = if cmds.is_empty() { Vec::new() } else { pipe.batch(&cmds)? };
    // What takes one more go (all of it together): the list of messages of a
    // mailbox some went from (its count short of what's known plus what's
    // new), and the headers of new messages found by listing.
    let mut recount: Vec<usize> = Vec::new();
    let mut to_fetch: Vec<(usize, Vec<u32>)> = Vec::new();
    let mut fresh: Vec<Vec<Message>> = Vec::new();
    let mut whole: Vec<bool> = vec![true; plans.len()];
    for (k, p) in plans.iter().enumerate() {
        fresh.push(Vec::new());
        let here: std::collections::HashSet<u32> = known.get(&p.id).map(|h| h.iter().copied().collect()).unwrap_or_default();
        if !replies[p.first].ok || [p.new_at, p.flags_at, p.search_at].iter().flatten().any(|i| !replies[*i].ok) {
            whole[k] = false;
            progress(format!("{}: {}: {}", a.name, p.pretty, replies[p.first].info));
            continue;
        }
        let on_server: Option<std::collections::HashSet<u32>> = p.search_at.map(|i| replies[i].search.iter().copied().collect());
        if let Some(i) = p.flags_at {
            for f in replies[i].fetches.iter().filter(|f| here.contains(&f.uid)) {
                delta.flags.push((p.id.clone(), f.uid, f.flags.iter().any(|x| x == "Seen"), f.flags.iter().any(|x| x == "Flagged"), flag_color(&f.flags)));
            }
        }
        match (p.before, on_server) {
            (Some(b), _) => {
                let mut added = 0u32;
                if let Some(i) = p.new_at {
                    for f in replies[i].fetches.iter().filter(|f| f.uid >= b.uid_next) {
                        added += 1;
                        if here.contains(&f.uid) {
                            continue;
                        }
                        if let Some(m) = f.header.as_deref().and_then(|h| header_parts(f.uid, &f.flags, f.internal_date, h, &p.id, &a.source())) {
                            fresh[k].push(m);
                        }
                    }
                }
                if p.now.exists != b.exists + added {
                    recount.push(k);
                }
            }
            (None, on_server) => {
                let on_server = on_server.unwrap_or_default();
                for uid in &here {
                    if !on_server.contains(uid) {
                        delta.gone.push((p.id.clone(), *uid));
                    }
                }
                let mut new: Vec<u32> = on_server.iter().copied().filter(|u| !here.contains(u)).collect();
                new.sort_unstable();
                let keep = new.len().saturating_sub(MAIL_PER_FOLDER as usize);
                new.drain(..keep);
                if !new.is_empty() {
                    to_fetch.push((k, new));
                }
            }
        }
    }
    if !recount.is_empty() || !to_fetch.is_empty() {
        let mut more: Vec<String> = Vec::new();
        // (index into `more` of the mailbox's first answer, how many)
        let mut at: Vec<(usize, usize, usize, bool)> = Vec::new(); // plan, first, count, recount?
        for k in &recount {
            at.push((*k, more.len(), 2, true));
            more.push(format!("EXAMINE {}", quote(&plans[*k].path)));
            more.push("UID SEARCH ALL".into());
        }
        for (k, uids) in &to_fetch {
            let first = more.len();
            more.push(format!("EXAMINE {}", quote(&plans[*k].path)));
            for chunk in uids.chunks(200) {
                more.push(format!("UID FETCH {} (UID FLAGS INTERNALDATE BODY.PEEK[HEADER])", chunk.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(",")));
            }
            at.push((*k, first, more.len() - first, false));
        }
        let r = pipe.batch(&more)?;
        for (k, first, count, is_recount) in at {
            let answers = &r[first..first + count];
            if answers.iter().any(|x| !x.ok) {
                whole[k] = false;
                continue;
            }
            let p = &plans[k];
            if is_recount {
                let on_server: std::collections::HashSet<u32> = answers[1].search.iter().copied().collect();
                for uid in known.get(&p.id).into_iter().flatten() {
                    if !on_server.contains(uid) {
                        delta.gone.push((p.id.clone(), *uid));
                    }
                }
            } else {
                for f in answers[1..].iter().flat_map(|x| x.fetches.iter()) {
                    if let Some(m) = f.header.as_deref().and_then(|h| header_parts(f.uid, &f.flags, f.internal_date, h, &p.id, &a.source())) {
                        fresh[k].push(m);
                    }
                }
            }
        }
    }
    pipe.logout();
    // All read: now it counts.
    imp.accounts.push(Account { id: account.clone(), name: a.name.clone(), email: a.email.clone(), source: a.source() });
    for (path, pretty, kind) in folders {
        imp.folders.push(Folder { id: format!("{account}/{path}"), account: account.clone(), name: pretty, kind, path });
    }
    for (k, msgs) in fresh.into_iter().enumerate() {
        if whole[k] {
            delta.marks.push((plans[k].id.clone(), plans[k].now));
        }
        match batch {
            Some(hand_over) if !msgs.is_empty() => hand_over(msgs),
            _ => delta.new.extend(msgs),
        }
    }
    imp.mail_delta = Some(delta);
    Ok(true)
}

/// Nothing added (UIDNEXT) and so nothing removed (the count) since the last
/// look: the same messages. (The whole-account pass also needs all of them here.)
fn same_messages(before: &crate::data::FolderMark, now: &crate::data::FolderMark, here: usize, deep: bool) -> bool {
    now.uid_next > 0 && before.validity == now.validity && before.uid_next == now.uid_next && before.exists == now.exists && (!deep || here >= now.exists as usize)
}

/// The same messages with the same flags (CONDSTORE's counter unchanged):
/// nothing to fetch.
fn unchanged(before: Option<&crate::data::FolderMark>, now: &crate::data::FolderMark, here: usize, deep: bool) -> bool {
    before.is_some_and(|b| b.modseq > 0 && b.modseq == now.modseq && same_messages(b, now, here, deep))
}

/// Apple Mail's flag colours on the server: keywords $MailFlagBit0..2 hold the
/// colour's number in binary (0 red … 6 gray).
const FLAG_BITS: [&str; 3] = ["$MailFlagBit0", "$MailFlagBit1", "$MailFlagBit2"];

/// A message's flag colour from its server flags (NO_COLOR when none is set,
/// which is red, Apple's default, for a flagged message).
fn flag_color(flags: &[String]) -> u8 {
    let mut c = 0u8;
    let mut any = false;
    for (i, k) in FLAG_BITS.iter().enumerate() {
        if flags.iter().any(|f| f.contains(k)) {
            c |= 1 << i;
            any = true;
        }
    }
    if any || flags.iter().any(|f| f == "Flagged") { c.min(6) } else { crate::data::NO_COLOR }
}

/// A message from its headers only (the text follows later).
fn header_message(f: &imap::types::Fetch, folder_id: &str, source: &str) -> Option<Message> {
    let flags: Vec<String> = f.flags().iter().map(|x| format!("{x:?}")).collect();
    header_parts(f.uid.unwrap_or(0), &flags, f.internal_date().map(|d| d.timestamp()), f.header()?, folder_id, source)
}

fn header_parts(uid: u32, flags: &[String], internal_date: Option<i64>, head: &[u8], folder_id: &str, source: &str) -> Option<Message> {
    let mut raw = head.to_vec();
    raw.extend_from_slice(b"\r\n");
    let p = mime::parse(&raw);
    if flags.iter().any(|x| x == "Deleted") {
        return None;
    }
    let date = if p.date != 0 { p.date } else { internal_date.unwrap_or(0) };
    Some(Message {
        folder: folder_id.to_string(),
        from_name: p.from_name,
        from_email: p.from_email,
        to: p.to,
        cc: p.cc,
        subject: p.subject,
        date,
        read: flags.iter().any(|x| x == "Seen"),
        flagged: flags.iter().any(|x| x == "Flagged"),
        flag_color: match flag_color(flags) {
            crate::data::NO_COLOR => 0,
            c => c,
        },
        source: source.to_string(),
        message_id: p.message_id,
        in_reply_to: p.in_reply_to,
        references: p.references,
        reply_to: p.reply_to,
        uid,
        partial: true,
        ..Default::default()
    })
}

fn fetch_headers(s: &mut Session, uids: &[u32], folder_id: &str, source: &str) -> Result<Vec<Message>, String> {
    let mut out = Vec::new();
    for chunk in uids.chunks(200) {
        let set = chunk.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(",");
        let fetched = s.uid_fetch(&set, "(UID FLAGS INTERNALDATE BODY.PEEK[HEADER])").map_err(|e| e.to_string())?;
        out.extend(fetched.iter().filter_map(|f| header_message(f, folder_id, source)));
    }
    Ok(out)
}

/// The next `count` older messages of a folder (UIDs below `below`), as
/// headers; also says whether there are more still.
pub fn fetch_older(a: &LiveAccount, path: &str, folder_id: &str, below: u32, count: usize) -> Result<(Vec<Message>, bool), String> {
    if let Some(r) = local_only(a) {
        return r;
    }
    let mut s = imap_connect(a)?;
    s.examine(path).map_err(|e| e.to_string())?;
    let mut uids: Vec<u32> = if below > 1 {
        s.uid_search(format!("UID 1:{}", below - 1)).map_err(|e| e.to_string())?.into_iter().filter(|u| *u < below).collect()
    } else {
        Vec::new()
    };
    uids.sort_unstable();
    let more = uids.len() > count;
    let take = &uids[uids.len().saturating_sub(count)..];
    let r = fetch_headers(&mut s, take, folder_id, &a.source());
    let _ = s.logout();
    r.map(|m| (m, more))
}

/// Messages in a folder that contain the words, searched on the server
/// (finds mail older than what's been downloaded). Newest 200.
pub fn search_server(a: &LiveAccount, path: &str, folder_id: &str, query: &str) -> Result<Vec<Message>, String> {
    if let Some(r) = local_only(a) {
        return r;
    }
    let mut s = imap_connect(a)?;
    s.examine(path).map_err(|e| e.to_string())?;
    let q = query.replace(['"', '\\'], " ");
    let cmd = if q.is_ascii() { format!("TEXT \"{q}\"") } else { format!("CHARSET UTF-8 TEXT \"{q}\"") };
    let mut uids: Vec<u32> = s.uid_search(cmd).map_err(|e| e.to_string())?.into_iter().collect();
    uids.sort_unstable();
    let take = &uids[uids.len().saturating_sub(200)..];
    let r = fetch_headers(&mut s, take, folder_id, &a.source());
    let _ = s.logout();
    r
}

/// A message whose text is wanted: (message id, server folder, UID).
pub type Wanted = (i64, String, u32);

/// The downloaded text of one message.
pub struct Text {
    pub id: i64,
    pub body: String,
    pub html: String,
    pub invite: String,
    pub attachments: Vec<String>,
    /// The whole message (false: just its start, from the background download).
    pub complete: bool,
}

/// Which download a text fetch belongs to: a message someone opened (first in
/// line), the newest mail after a check, or the long one through a whole account
/// (older mail). A newer download of the same kind makes an older one stop; the
/// kinds don't stop each other.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Lane {
    Opened,
    Recent,
    Whole,
}

/// Bumped by every download of each kind, so an older one of that kind stops.
static RECENT_ROUND: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static WHOLE_ROUND: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// Messages someone has opened being fetched right now: the other downloads
/// wait (between their pieces) until they're here, so they come first.
static OPENING: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// The most one background fetch asks for at once (so an opened message never
/// waits long behind it).
/// Fetched in pieces of at most this much, one command each: with Gmail
/// answering every command ~10 s late when it's slowing an account down, fewer,
/// bigger pieces get far more done (it was 3 MB).
const PIECE_BYTES: u64 = 12_000_000;
/// Connections the long download of a whole account uses at once (Thunderbird
/// uses up to 5 with Gmail; Gmail allows 15).
const WHOLE_CONNECTIONS: usize = 2;

/// Gmail lets an account download about 2.5 GB a day over IMAP (every mail app
/// together) and past that answers every command ~10 seconds late, so even a
/// check for new mail takes many minutes. The long background download stops
/// for the day at this much (new mail counts too; opened messages don't),
/// leaving room for the rest. (It was 1 GB, which made a big mailbox take
/// many days; Thunderbird has no limit of its own.)
const GMAIL_DAILY_BYTES: u64 = 2_200_000_000;

fn is_gmail(a: &LiveAccount) -> bool {
    let h = a.imap_host.to_lowercase();
    h.ends_with("gmail.com") || h.ends_with("googlemail.com")
}

/// What the background download took today, by account (on disk, so starting
/// again doesn't reset it), after adding `add`.
static DOWNLOADED: std::sync::Mutex<Option<(String, HashMap<String, u64>)>> = std::sync::Mutex::new(None);
fn downloaded_today(account: &str, add: u64) -> u64 {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let path = crate::data::data_dir().join("downloaded-today.json");
    let mut g = DOWNLOADED.lock().unwrap();
    if g.as_ref().is_none_or(|(d, _)| *d != today) {
        let saved: Option<(String, HashMap<String, u64>)> = std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str(&s).ok());
        *g = Some(match saved {
            Some((d, m)) if d == today => (d, m),
            _ => (today, HashMap::new()),
        });
    }
    let Some((_, m)) = g.as_mut() else { return 0 };
    let n = m.entry(account.to_string()).or_default();
    *n += add;
    let total = *n;
    if add > 0 {
        let _ = std::fs::write(&path, serde_json::to_string(&*g).unwrap_or_default());
    }
    total
}

/// Downloads the text of the given messages, a batch at a time, handing
/// each batch to `deliver` as soon as it arrives. Messages small enough are
/// downloaded whole and their attachments kept as files.
pub fn fetch_texts(
    a: &LiveAccount,
    wanted: &[Wanted],
    lane: Lane,
    deliver: &(dyn Fn(Vec<Text>) + Sync),
    progress: &(dyn Fn(String) + Sync),
) -> Result<(), String> {
    if let Some(r) = local_only(a) {
        return r;
    }
    use std::sync::atomic::{AtomicUsize, Ordering};
    let round = match lane {
        Lane::Opened => 0,
        Lane::Recent => RECENT_ROUND.fetch_add(1, Ordering::SeqCst) + 1,
        Lane::Whole => WHOLE_ROUND.fetch_add(1, Ordering::SeqCst) + 1,
    };
    // Opened by someone: ahead of the other downloads until done.
    struct Opening;
    impl Drop for Opening {
        fn drop(&mut self) {
            OPENING.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let _opening = if lane == Lane::Opened { OPENING.fetch_add(1, Ordering::SeqCst); Some(Opening) } else { None };
    // Opened by someone: everything asked at once, so it's here after one
    // wait (a slowed-down Gmail answers each round trip ~10 seconds late).
    if lane == Lane::Opened {
        match opened_quick(a, wanted, deliver) {
            Ok(()) => return Ok(()),
            Err(e) => eprintln!("almanac: opening a message the quick way: {e}"),
        }
    }
    // Gmail's daily allowance already used: said straight away (finding out
    // after connecting took a minute of "0%" on a slowed-down account).
    if lane == Lane::Whole && is_gmail(a) && downloaded_today(&a.id, 0) >= GMAIL_DAILY_BYTES {
        progress(format!("{}: older mail continues tomorrow (Gmail allows only so much downloading a day)", a.name));
        return Ok(());
    }
    let label = match lane {
        Lane::Whole => format!("{}: downloading older mail in the background", a.name),
        _ => format!("{}: downloading new mail", a.name),
    };
    // The long download counts the whole account (what's already here as done),
    // not just what's left, which read "0% of 60,736" each time it went on.
    let left = wanted.len();
    let (base, total) = if lane == Lane::Whole {
        let all = crate::mailstore::count("source = ?1 AND uid > 0", &[crate::mailstore::Arg::Text(a.source())]).max(0) as usize;
        (all.saturating_sub(left), all.max(left))
    } else {
        (0, left)
    };
    let done = AtomicUsize::new(base);
    if lane != Lane::Opened && left > TEXT_BATCH {
        progress(counted(&label, base, total));
    }
    let job = Job { a, lane, round, deliver, progress, done: &done, total, label: &label, waited_ms: std::sync::atomic::AtomicU64::new(0) };
    if lane == Lane::Whole && left > 200 {
        // Several connections at once, each with its share (the newest first in each).
        std::thread::scope(|scope| {
            for k in 0..WHOLE_CONNECTIONS {
                let part: Vec<Wanted> = wanted.iter().skip(k).step_by(WHOLE_CONNECTIONS).cloned().collect();
                let job = &job;
                scope.spawn(move || job.run(&part));
            }
        });
        return Ok(());
    }
    job.run(wanted)
}

/// Messages someone opened, over one connection with the commands sent
/// together: the mailbox opened and the messages (with their sizes) fetched in
/// one go, up to BODY_BYTES each; those bigger, but not too big to keep whole,
/// in a second go for their attachments (their text is shown meanwhile).
fn opened_quick(a: &LiveAccount, wanted: &[Wanted], deliver: &(dyn Fn(Vec<Text>) + Sync)) -> Result<(), String> {
    use crate::imap_pipe::{quote, Auth, Pipe};
    if imap_starttls(a.imap_port) || is_local(&a.imap_host) || wanted.is_empty() {
        return Err("not for this server".into());
    }
    let auth = if a.provider.is_empty() {
        Auth::Password { user: a.username.clone(), pass: need_password(a)? }
    } else {
        Auth::OAuth { user: a.email.clone(), token: token(a, crate::oauth::Resource::Mail)? }
    };
    let mut pipe = Pipe::sign_in(tls_stream(&a.imap_host, a.imap_port)?, &auth)?;
    // Folder by folder, in the order asked.
    let mut order: Vec<String> = Vec::new();
    let mut by_folder: HashMap<String, Vec<(i64, u32)>> = HashMap::new();
    for (id, path, uid) in wanted {
        if !by_folder.contains_key(path) {
            order.push(path.clone());
        }
        by_folder.entry(path.clone()).or_default().push((*id, *uid));
    }
    let text_of = |raw: &[u8], id: i64, whole: bool| {
        let p = mime::parse(raw);
        if whole && !p.attachments.is_empty() {
            crate::mailstore::store_attachments_from(id, raw, &p.attachments);
        }
        Text { id, body: p.body, html: p.html, invite: p.invite, attachments: p.attachments, complete: whole }
    };
    for path in order {
        let items = &by_folder[&path];
        let ids: HashMap<u32, i64> = items.iter().map(|(id, uid)| (*uid, *id)).collect();
        let set = items.iter().map(|(_, u)| u.to_string()).collect::<Vec<_>>().join(",");
        let r = pipe.batch(&[format!("EXAMINE {}", quote(&path)), format!("UID FETCH {set} (UID RFC822.SIZE BODY.PEEK[]<0.{BODY_BYTES}>)")])?;
        if !r[0].ok || !r[1].ok {
            return Err(format!("{path}: {} {}", r[0].info, r[1].info));
        }
        let mut texts = Vec::new();
        let mut rest: Vec<u32> = Vec::new();
        for f in &r[1].fetches {
            let (Some(&id), Some(raw)) = (ids.get(&f.uid), f.header.as_deref()) else { continue };
            let size = f.size.unwrap_or(raw.len() as u32);
            let whole = size as usize <= raw.len();
            texts.push(text_of(raw, id, whole));
            if !whole && size <= WHOLE_BYTES {
                rest.push(f.uid);
            }
        }
        deliver(texts);
        // The bigger ones whole, for their attachments.
        if !rest.is_empty() {
            let set = rest.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(",");
            let r = pipe.batch(&[format!("UID FETCH {set} (UID BODY.PEEK[])")])?;
            let texts: Vec<Text> = r[0].fetches.iter().filter_map(|f| Some(text_of(f.header.as_deref()?, *ids.get(&f.uid)?, true))).collect();
            deliver(texts);
        }
    }
    pipe.logout();
    Ok(())
}

/// One connection's share of a text download.
struct Job<'a> {
    a: &'a LiveAccount,
    lane: Lane,
    round: u64,
    deliver: &'a (dyn Fn(Vec<Text>) + Sync),
    progress: &'a (dyn Fn(String) + Sync),
    done: &'a std::sync::atomic::AtomicUsize,
    total: usize,
    label: &'a str,
    /// How long this download has waited for opened messages, in total (ms).
    waited_ms: std::sync::atomic::AtomicU64,
}

impl Job<'_> {
    /// Still wanted (not overtaken by a newer download of the same kind).
    fn current(&self) -> bool {
        use std::sync::atomic::Ordering;
        match self.lane {
            Lane::Opened => true,
            Lane::Recent => RECENT_ROUND.load(Ordering::SeqCst) == self.round,
            Lane::Whole => WHOLE_ROUND.load(Ordering::SeqCst) == self.round,
        }
    }

    /// Messages someone opened go first, but only for a while: waiting up to 90
    /// seconds before every piece held new mail back for many minutes ("0 of
    /// 500" for ages) while a slow server answered for an opened conversation.
    /// Now at most WAIT_FOR_OPENED in all per download.
    fn wait_turn(&self) {
        use std::sync::atomic::Ordering;
        if self.lane == Lane::Opened {
            return;
        }
        let budget = WAIT_FOR_OPENED.as_millis() as u64;
        let started = std::time::Instant::now();
        while OPENING.load(Ordering::SeqCst) > 0 && self.waited_ms.load(Ordering::SeqCst) + (started.elapsed().as_millis() as u64) < budget {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let waited = started.elapsed().as_millis() as u64;
        if waited >= 500 {
            let total = self.waited_ms.fetch_add(waited, Ordering::SeqCst) + waited;
            eprintln!("almanac: {}: waited {:.1} s for opened messages ({:.1} s in all)", self.label, waited as f64 / 1000.0, total as f64 / 1000.0);
        }
    }

    /// The download over one connection. The long one carries on after a
    /// dropped connection or a server error (connecting again, a few times).
    fn run(&self, wanted: &[Wanted]) -> Result<(), String> {
        let mut left: Vec<Wanted> = wanted.to_vec();
        let mut tries = 0;
        loop {
            match self.run_once(&left) {
                Ok(()) => return Ok(()),
                Err((e, rest)) => {
                    tries += 1;
                    if self.lane != Lane::Whole || tries > 5 || !self.current() || rest.is_empty() {
                        return Err(e);
                    }
                    left = rest;
                    std::thread::sleep(std::time::Duration::from_secs(5 * tries));
                }
            }
        }
    }

    /// Err: what went wrong, and what's still to fetch.
    fn run_once(&self, wanted: &[Wanted]) -> Result<(), (String, Vec<Wanted>)> {
        use std::sync::atomic::Ordering;
        let mut s = imap_connect(self.a).map_err(|e| (e, wanted.to_vec()))?;
        // Folder by folder, keeping the order asked for (inbox and newest first).
        let mut order: Vec<String> = Vec::new();
        let mut by_folder: HashMap<String, Vec<(i64, u32)>> = HashMap::new();
        for (id, path, uid) in wanted {
            if !by_folder.contains_key(path) {
                order.push(path.clone());
            }
            by_folder.entry(path.clone()).or_default().push((*id, *uid));
        }
        let (whole_limit, start_bytes) = if self.lane == Lane::Whole { (BG_WHOLE_BYTES, BG_WHOLE_BYTES) } else { (WHOLE_BYTES, BODY_BYTES) };
        // (Big batches: each starts with a command for the messages' sizes.)
        let batch = if self.lane == Lane::Whole { 400 } else if self.lane == Lane::Recent { 100 } else { TEXT_BATCH };
        let mut finished: std::collections::HashSet<i64> = std::collections::HashSet::new();
        let rest = |finished: &std::collections::HashSet<i64>| wanted.iter().filter(|w| !finished.contains(&w.0)).cloned().collect::<Vec<_>>();
        for path in order {
            if !self.current() {
                break;
            }
            if s.examine(&path).is_err() {
                continue;
            }
            let items = &by_folder[&path];
            for chunk in items.chunks(batch) {
                if !self.current() {
                    break;
                }
                let ids: HashMap<u32, i64> = chunk.iter().map(|(id, uid)| (*uid, *id)).collect();
                let set = chunk.iter().map(|(_, u)| u.to_string()).collect::<Vec<_>>().join(",");
                // Sizes first, to choose between whole messages and just the start.
                let sizes: HashMap<u32, u32> = match s.uid_fetch(&set, "(UID RFC822.SIZE)") {
                    Ok(f) => f.iter().filter_map(|x| Some((x.uid?, x.size.unwrap_or(0)))).collect(),
                    Err(e) => return Err((e.to_string(), rest(&finished))),
                };
                let (small, big): (Vec<u32>, Vec<u32>) = chunk.iter().map(|(_, u)| *u).partition(|u| sizes.get(u).copied().unwrap_or(0) <= whole_limit);
                let mut texts = Vec::new();
                // In pieces of at most PIECE_BYTES (a single bigger message on its own).
                let mut pieces: Vec<(Vec<u32>, String, bool)> = Vec::new();
                for (uids, query, whole) in [(small, "(UID BODY.PEEK[])".to_string(), true), (big, format!("(UID BODY.PEEK[]<0.{start_bytes}>)"), false)] {
                    let mut piece: Vec<u32> = Vec::new();
                    let mut bytes = 0u64;
                    for u in uids {
                        let cost = if whole { sizes.get(&u).copied().unwrap_or(0) as u64 } else { start_bytes as u64 };
                        if !piece.is_empty() && bytes + cost > PIECE_BYTES {
                            pieces.push((std::mem::take(&mut piece), query.clone(), whole));
                            bytes = 0;
                        }
                        piece.push(u);
                        bytes += cost;
                    }
                    if !piece.is_empty() {
                        pieces.push((piece, query.clone(), whole));
                    }
                }
                let mut counted_so_far = 0usize;
                for (uids, query, whole) in pieces {
                    self.wait_turn();
                    if !self.current() {
                        break;
                    }
                    // Gmail's daily allowance: the rest tomorrow.
                    if self.lane == Lane::Whole && is_gmail(self.a) && downloaded_today(&self.a.id, 0) >= GMAIL_DAILY_BYTES {
                        (self.deliver)(texts);
                        (self.progress)(format!("{}: older mail continues tomorrow (Gmail allows only so much downloading a day)", self.a.name));
                        let _ = s.logout();
                        return Ok(());
                    }
                    let set = uids.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(",");
                    let piece_started = std::time::Instant::now();
                    let fetched = match s.uid_fetch(&set, &query) {
                        Ok(f) => f,
                        Err(e) => {
                            (self.deliver)(texts);
                            return Err((e.to_string(), rest(&finished)));
                        }
                    };
                    if self.lane != Lane::Opened {
                        downloaded_today(&self.a.id, fetched.iter().map(|f| f.body().map_or(0, |b| b.len() as u64)).sum());
                    }
                    for f in fetched.iter() {
                        let (Some(uid), Some(raw)) = (f.uid, f.body()) else { continue };
                        let Some(&id) = ids.get(&uid) else { continue };
                        let p = mime::parse(raw);
                        if whole && !p.attachments.is_empty() {
                            crate::mailstore::store_attachments_from(id, raw, &p.attachments);
                        }
                        texts.push(Text { id, body: p.body, html: p.html, invite: p.invite, attachments: p.attachments, complete: whole });
                    }
                    // Slow answers are noted (to see where time goes), and the count
                    // moves on with each piece, not only at the end of a batch.
                    let took = piece_started.elapsed();
                    if took.as_secs() >= 10 {
                        let bytes: usize = fetched.iter().map(|f| f.body().map_or(0, |b| b.len())).sum();
                        eprintln!("almanac: {}: {} message(s), {} KB, took {:.1} s", self.label, uids.len(), bytes / 1024, took.as_secs_f32());
                    }
                    let done = self.done.fetch_add(uids.len(), Ordering::SeqCst) + uids.len();
                    counted_so_far += uids.len();
                    if self.lane != Lane::Opened && self.total > TEXT_BATCH {
                        (self.progress)(counted(self.label, done.min(self.total), self.total));
                    }
                    // Kept as each piece arrives, not at the end of the batch.
                    (self.deliver)(std::mem::take(&mut texts));
                }
                for (id, _) in chunk {
                    finished.insert(*id);
                }
                // (Any of the batch not fetched, e.g. gone from the server, still counts as done.)
                if chunk.len() > counted_so_far {
                    let extra = chunk.len() - counted_so_far;
                    let done = self.done.fetch_add(extra, Ordering::SeqCst) + extra;
                    if self.lane != Lane::Opened && self.total > TEXT_BATCH {
                        (self.progress)(counted(self.label, done.min(self.total), self.total));
                    }
                }
                (self.deliver)(texts);
            }
        }
        let _ = s.logout();
        Ok(())
    }
}

/// A progress report with a count, "label (done of total)…"; the app shows
/// it with a progress bar (see `parse_counted`).
pub fn counted(label: &str, done: usize, total: usize) -> String {
    format!("{label} ({done} of {total})…")
}

/// The (done, total) of a report made by `counted`.
pub fn parse_counted(s: &str) -> Option<(usize, usize)> {
    let inner = s.strip_suffix(")…")?;
    let (_, nums) = inner.rsplit_once(" (")?;
    let (done, total) = nums.split_once(" of ")?;
    Some((done.parse().ok()?, total.parse().ok()?))
}

#[cfg(test)]
mod flag_color_tests {
    #[test]
    fn apple_flag_colours_from_server_keywords() {
        let f = |v: &[&str]| super::flag_color(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(f(&["Flagged"]), 0, "red");
        assert_eq!(f(&["Flagged", "Custom(\"$MailFlagBit0\")"]), 1, "orange");
        assert_eq!(f(&["Flagged", "Custom(\"$MailFlagBit2\")"]), 4, "blue");
        assert_eq!(f(&["Flagged", "Custom(\"$MailFlagBit1\")", "Custom(\"$MailFlagBit2\")"]), 6, "gray");
        assert_eq!(f(&["Seen"]), crate::data::NO_COLOR, "no flag: colour left as it was");
    }
}

#[cfg(test)]
mod budget_tests {
    #[test]
    fn the_daily_download_adds_up_and_is_kept() {
        let _ = std::fs::create_dir_all(crate::data::data_dir());
        let before = super::downloaded_today("budget-test", 0);
        assert_eq!(super::downloaded_today("budget-test", 500), before + 500);
        assert_eq!(super::downloaded_today("other", 0), 0, "each account has its own");
        // Read back from disk (as after starting again).
        *super::DOWNLOADED.lock().unwrap() = None;
        assert_eq!(super::downloaded_today("budget-test", 0), before + 500);
    }
}

#[cfg(test)]
mod connection_tests {
    /// Each step of talking to the real account's mail server, timed
    /// (read-only): ALMANAC_REAL_DATA=~/.local/share/almanac cargo test step_times -- --ignored --nocapture
    #[test]
    #[ignore]
    fn step_times() {
        let Some(dir) = std::env::var_os("ALMANAC_REAL_DATA").map(std::path::PathBuf::from) else { return };
        let db: crate::data::Db = serde_json::from_str(&std::fs::read_to_string(dir.join("data.json")).unwrap()).unwrap();
        let a = db.live.iter().find(|a| !a.imap_host.is_empty()).unwrap().clone();
        let t = std::time::Instant::now();
        let lap = |what: &str| eprintln!("{what}: {:?}", t.elapsed());
        let token = super::token(&a, crate::oauth::Resource::Mail).unwrap();
        lap("token");
        let client = super::tls_client(&a.imap_host, a.imap_port).unwrap();
        lap("connected");
        drop(client);
        // The same by hand: when do the server's answers really arrive?
        {
            use std::io::{Read, Write};
            let tcp = std::net::TcpStream::connect((a.imap_host.as_str(), a.imap_port)).unwrap();
            let connector = rustls_connector::RustlsConnectorConfig::new_with_native_certs().unwrap().connector_with_no_client_auth();
            let mut tls = connector.connect(&a.imap_host, tcp).unwrap();
            let mut buf = [0u8; 65536];
            let mut until = |tls: &mut rustls_connector::TlsStream<std::net::TcpStream>, tag: &str, what: &str| {
                let mut got = String::new();
                while !got.contains(tag) {
                    let n = tls.read(&mut buf).unwrap();
                    if n == 0 { break }
                    got.push_str(&String::from_utf8_lossy(&buf[..n]));
                    eprintln!("  {what}: {n} bytes at {:?}", t.elapsed());
                }
            };
            until(&mut tls, "\r\n", "greeting");
            let auth = format!("user={}\x01auth=Bearer {}\x01\x01", a.email, super::token(&a, crate::oauth::Resource::Mail).unwrap());
            use base64::Engine;
            write!(tls, "a1 AUTHENTICATE XOAUTH2 {}\r\n", base64::engine::general_purpose::STANDARD.encode(auth)).unwrap();
            tls.flush().unwrap();
            until(&mut tls, "a1 ", "sign-in");
            write!(tls, "a2 NOOP\r\n").unwrap();
            tls.flush().unwrap();
            until(&mut tls, "a2 ", "noop");
            write!(tls, "a3 NOOP\r\n").unwrap();
            tls.flush().unwrap();
            until(&mut tls, "a3 ", "noop");
            // Three at once (pipelined): one wait, or three?
            write!(tls, "b1 NOOP\r\nb2 EXAMINE INBOX\r\nb3 NOOP\r\n").unwrap();
            tls.flush().unwrap();
            until(&mut tls, "b3 ", "three at once");
            if std::env::var("ALMANAC_STEPS_ONLY_RAW").is_ok() {
                return;
            }
        }
        let client = super::tls_client(&a.imap_host, a.imap_port).unwrap();
        let mut s = client.authenticate("XOAUTH2", &crate::oauth::XOAuth2 { user: a.email.clone(), token }).map_err(|(e, _)| e).unwrap();
        lap("signed in");
        s.noop().unwrap();
        lap("noop");
        s.examine("[Mailspring]/Snoozed").ok();
        lap("examine empty");
        s.noop().unwrap();
        lap("noop");
        s.capabilities().unwrap();
        lap("capabilities");
        s.examine("INBOX").unwrap();
        lap("examine inbox");
        s.uid_fetch("1:*", "(UID FLAGS)").map(|f| f.len()).ok();
        lap("all flags of the inbox");
        let _ = s.logout();
    }

    /// A check of the real account (read-only: it only looks and reads
    /// headers; nothing is saved), timed without and then with the mailboxes'
    /// change counters (tests never find the real data themselves; it's named):
    /// ALMANAC_REAL_DATA=~/.local/share/almanac cargo test unchanged_mailboxes -- --ignored --nocapture
    /// Waiting on the real account's Inbox (IMAP IDLE, read-only) for 20
    /// seconds: ALMANAC_REAL_DATA=~/.local/share/almanac cargo test idle_waits -- --ignored --nocapture
    #[test]
    #[ignore]
    fn idle_waits() {
        let Some(dir) = std::env::var_os("ALMANAC_REAL_DATA").map(std::path::PathBuf::from) else { return };
        let db: crate::data::Db = serde_json::from_str(&std::fs::read_to_string(dir.join("data.json")).unwrap()).unwrap();
        let a = db.live.iter().find(|a| !a.imap_host.is_empty()).unwrap().clone();
        let t = std::time::Instant::now();
        let mut s = super::imap_connect(&a).unwrap();
        s.examine("INBOX").unwrap();
        eprintln!("inbox open at {:?}", t.elapsed());
        let mut idle = s.idle();
        idle.timeout(std::time::Duration::from_secs(20)).keepalive(false);
        let outcome = idle.wait_while(|r| { eprintln!("  told: {r:?}"); false }).unwrap();
        eprintln!("idle ended {outcome:?} at {:?}", t.elapsed());
    }

    /// Opening a message on the real account (read-only: BODY.PEEK leaves it
    /// unread; attachments go to the test folder), timed:
    /// ALMANAC_REAL_DATA=~/.local/share/almanac cargo test opening_a_message_times -- --ignored --nocapture
    #[test]
    #[ignore]
    fn opening_a_message_times() {
        let Some(dir) = std::env::var_os("ALMANAC_REAL_DATA").map(std::path::PathBuf::from) else { return };
        let db: crate::data::Db = serde_json::from_str(&std::fs::read_to_string(dir.join("data.json")).unwrap()).unwrap();
        let a = db.live.iter().find(|a| !a.imap_host.is_empty()).unwrap().clone();
        let c = rusqlite::Connection::open_with_flags(dir.join("mail.sqlite"), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let uid: u32 = c.query_row("SELECT MAX(uid) FROM messages WHERE folder = ?1", [format!("{}:mail/INBOX", a.source())], |r| r.get::<_, i64>(0)).unwrap() as u32;
        let _ = std::fs::create_dir_all(crate::data::data_dir());
        let t = std::time::Instant::now();
        let got = std::sync::Mutex::new(Vec::new());
        let deliver = |texts: Vec<super::Text>| {
            for x in texts {
                eprintln!("  text at {:?}: {} chars, {} html, complete {}", t.elapsed(), x.body.len(), x.html.len(), x.complete);
                got.lock().unwrap().push(x.complete);
            }
        };
        super::fetch_texts(&a, &[(1, "INBOX".into(), uid)], super::Lane::Opened, &deliver, &|_| {}).unwrap();
        eprintln!("opened in {:?}", t.elapsed());
        assert!(!got.lock().unwrap().is_empty());
    }

    #[test]
    #[ignore]
    fn quick_check_times() {
        let Some(dir) = std::env::var_os("ALMANAC_REAL_DATA").map(std::path::PathBuf::from) else { return };
        let db: crate::data::Db = serde_json::from_str(&std::fs::read_to_string(dir.join("data.json")).unwrap()).unwrap();
        let a = db.live.iter().find(|a| !a.imap_host.is_empty()).unwrap().clone();
        let c = rusqlite::Connection::open_with_flags(dir.join("mail.sqlite"), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let mut known: std::collections::HashMap<String, Vec<u32>> = Default::default();
        let mut q = c.prepare("SELECT folder, uid FROM messages WHERE source = ?1 AND uid > 0").unwrap();
        for r in q.query_map([a.source()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?))).unwrap() {
            let (f, u) = r.unwrap();
            known.entry(f).or_default().push(u);
        }
        let say = |m: String| eprintln!("  {m}");
        let mut marks = db.folder_marks.clone();
        for round in 0..2 {
            let mut imp = crate::data::Imported::default();
            let t = std::time::Instant::now();
            let done = super::quick_check(&a, &mut imp, &known, &marks, &say, None).expect("checks");
            let d = imp.mail_delta.unwrap_or_default();
            eprintln!("quick check {round}: done {done} in {:?}: {} new, {} flag rows, {} gone, {} mailboxes marked, {} folders", t.elapsed(), d.new.len(), d.flags.len(), d.gone.len(), d.marks.len(), imp.folders.len());
            marks.extend(d.marks);
        }
    }

    #[test]
    #[ignore]
    fn unchanged_mailboxes_are_skipped() {
        let Some(dir) = std::env::var_os("ALMANAC_REAL_DATA").map(std::path::PathBuf::from) else { return };
        let json = std::fs::read_to_string(dir.join("data.json")).expect("data.json");
        let db: crate::data::Db = serde_json::from_str(&json).expect("reads");
        let a = db.live.iter().find(|a| !a.imap_host.is_empty()).expect("a live account").clone();
        let path = dir.join("mail.sqlite");
        let c = rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).expect("opens");
        let mut known: std::collections::HashMap<String, Vec<u32>> = Default::default();
        let mut q = c.prepare("SELECT folder, uid FROM messages WHERE source = ?1 AND uid > 0").unwrap();
        for r in q.query_map([a.source()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?))).unwrap() {
            let (f, u) = r.unwrap();
            known.entry(f).or_default().push(u);
        }
        let quiet = |_: String| {};
        let run = |marks: &std::collections::HashMap<String, crate::data::FolderMark>| {
            let mut imp = crate::data::Imported::default();
            let t = std::time::Instant::now();
            super::sync_mail(&a, &mut imp, &known, marks, false, &quiet, None, false).expect("checks");
            let d = imp.mail_delta.unwrap();
            eprintln!("check took {:?}: {} new, {} flag rows, {} gone, {} mailboxes marked", t.elapsed(), d.new.len(), d.flags.len(), d.gone.len(), d.marks.len());
            d.marks.into_iter().collect::<std::collections::HashMap<_, _>>()
        };
        let marks = run(&db.folder_marks);
        let again = run(&marks);
        assert_eq!(again.len(), marks.len());
    }

    /// Reaches a real mail server and reads its greeting (no sign-in):
    /// ALMANAC_NET_TEST=1 cargo test reaches_a_server -- --ignored
    #[test]
    #[ignore]
    fn reaches_a_server_with_time_limits() {
        if std::env::var("ALMANAC_NET_TEST").is_err() {
            return;
        }
        let t = std::time::Instant::now();
        let client = super::tls_client("imap.gmail.com", 993).expect("connects");
        drop(client);
        eprintln!("connected in {:?}", t.elapsed());
        assert!(super::tls_client("imap.gmail.com.invalid", 993).is_err());
    }

    /// Every provider offered when adding an account: its mail server answers
    /// at the address and port Almanac uses (no sign-in).
    #[test]
    #[ignore]
    fn every_offered_provider_answers() {
        if std::env::var("ALMANAC_NET_TEST").is_err() {
            return;
        }
        let mut failed = Vec::new();
        for p in super::providers_json().as_array().unwrap() {
            let Some(domain) = p["domain"].as_str() else { continue };
            if p["how"] == "bridge" {
                continue;
            }
            let a = super::preset(&format!("someone@{domain}"));
            let t = std::time::Instant::now();
            match super::tls_client(&a.imap_host, a.imap_port) {
                Ok(_) => eprintln!("{:<12} {}:{} answered in {:?}", p["name"].as_str().unwrap(), a.imap_host, a.imap_port, t.elapsed()),
                Err(e) => failed.push(format!("{}: {e}", p["name"])),
            }
        }
        for host in ["imap.gmail.com", "outlook.office365.com", "imap.mail.yahoo.com", "imap.aol.com"] {
            match super::tls_client(host, 993) {
                Ok(_) => eprintln!("{host}:993 answered (sign-in providers)"),
                Err(e) => failed.push(format!("{host}: {e}")),
            }
        }
        assert!(failed.is_empty(), "{failed:#?}");
    }
}

#[cfg(test)]
mod counted_tests {
    #[test]
    fn round_trip() {
        let s = super::counted("Work: downloading messages", 40, 1200);
        assert_eq!(s, "Work: downloading messages (40 of 1200)…");
        assert_eq!(super::parse_counted(&s), Some((40, 1200)));
        assert_eq!(super::parse_counted("Work: reading Inbox…"), None);
        assert_eq!(super::parse_counted("Work (old): up to date"), None);
    }
}

/// A change to one message on the server.
pub enum MailOp {
    Seen(bool),
    Flagged(bool),
    /// Flagged in a colour (Apple Mail's), or not flagged.
    FlagColor(Option<u8>),
    /// Move to another folder (server path)
    Move(String),
    /// Delete for good
    Expunge,
}

pub fn mail_op(a: &LiveAccount, folder_path: &str, uid: u32, op: MailOp) -> Result<(), String> {
    if a.is_ews() {
        return crate::ews::op(a, folder_path, &[uid], op);
    }
    if let Some(r) = local_only(a) {
        return r;
    }
    if uid == 0 {
        return Err("This message has not been synced yet".into());
    }
    mail_op_many(a, folder_path, &[uid], op)
}

/// The same change to many messages of one folder, in one go.
pub fn mail_op_many(a: &LiveAccount, folder_path: &str, uids: &[u32], op: MailOp) -> Result<(), String> {
    if a.is_ews() {
        return crate::ews::op(a, folder_path, uids, op);
    }
    if let Some(r) = local_only(a) {
        return r;
    }
    let uids: Vec<String> = uids.iter().filter(|u| **u > 0).map(|u| u.to_string()).collect();
    if uids.is_empty() {
        return Ok(());
    }
    let mut s = imap_connect(a)?;
    s.select(folder_path).map_err(|e| e.to_string())?;
    let set = uids.join(",");
    let r = match op {
        MailOp::Seen(on) => s.uid_store(&set, if on { "+FLAGS (\\Seen)" } else { "-FLAGS (\\Seen)" }).map(|_| ()),
        MailOp::Flagged(on) => s.uid_store(&set, if on { "+FLAGS (\\Flagged)" } else { "-FLAGS (\\Flagged)" }).map(|_| ()),
        MailOp::FlagColor(color) => {
            let on = color.is_some();
            if let Err(e) = s.uid_store(&set, if on { "+FLAGS (\\Flagged)" } else { "-FLAGS (\\Flagged)" }) {
                return Err(e.to_string());
            }
            // The colour as Apple Mail keeps it: three keywords, one bit each (a
            // server that doesn't keep keywords still has the flag itself).
            let bits = color.unwrap_or(0);
            let (set_on, set_off): (Vec<&str>, Vec<&str>) = FLAG_BITS.iter().enumerate().fold((vec![], vec![]), |(mut a, mut b), (i, k)| {
                if on && bits & (1 << i) != 0 { a.push(*k) } else { b.push(*k) }
                (a, b)
            });
            if !set_off.is_empty() {
                let _ = s.uid_store(&set, format!("-FLAGS ({})", set_off.join(" ")));
            }
            if !set_on.is_empty() {
                let _ = s.uid_store(&set, format!("+FLAGS ({})", set_on.join(" ")));
            }
            Ok(())
        }
        MailOp::Move(to) => s.uid_mv(&set, &to).or_else(|_| {
            s.uid_copy(&set, &to)?;
            s.uid_store(&set, "+FLAGS (\\Deleted)")?;
            s.expunge().map(|_| ())
        }),
        MailOp::Expunge => s.uid_store(&set, "+FLAGS (\\Deleted)").and_then(|_| s.expunge().map(|_| ())),
    };
    let _ = s.logout();
    r.map_err(|e| e.to_string())
}

/// A message to send.
#[derive(Default, Clone)]
pub struct OutMsg {
    pub to: String,
    pub cc: String,
    pub bcc: String,
    pub subject: String,
    pub body: String,
    /// Formatted version (HTML); empty for plain text only
    pub html: String,
    /// Files on this computer to attach
    pub attachments: Vec<String>,
    /// A calendar part: (METHOD, iCalendar text)
    pub calendar: Option<(String, String)>,
    pub in_reply_to: String,
    /// A fixed Message-ID (drafts keep theirs, so the server copy can be replaced)
    pub message_id: String,
    /// A draft: may have no recipients yet
    pub draft: bool,
}

impl OutMsg {
    pub fn from_json(v: &serde_json::Value) -> OutMsg {
        let s = |k: &str| v.get(k).and_then(serde_json::Value::as_str).unwrap_or("").to_string();
        OutMsg {
            to: s("to"),
            cc: s("cc"),
            bcc: s("bcc"),
            subject: s("subject"),
            body: s("body"),
            html: s("html"),
            attachments: v
                .get("attachments")
                .and_then(serde_json::Value::as_array)
                .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default(),
            calendar: None,
            in_reply_to: s("inReplyTo"),
            message_id: String::new(),
            draft: false,
        }
    }
}

fn content_type_for(path: &str) -> &'static str {
    let p = path.to_lowercase();
    for (ext, t) in [
        (".pdf", "application/pdf"),
        (".png", "image/png"),
        (".jpg", "image/jpeg"),
        (".jpeg", "image/jpeg"),
        (".gif", "image/gif"),
        (".txt", "text/plain"),
        (".ics", "text/calendar"),
        (".vcf", "text/vcard"),
        (".zip", "application/zip"),
        (".doc", "application/msword"),
        (".docx", "application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
        (".xlsx", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        (".pptx", "application/vnd.openxmlformats-officedocument.presentationml.presentation"),
    ] {
        if p.ends_with(ext) {
            return t;
        }
    }
    "application/octet-stream"
}

/// Builds the full message (plain + formatted text, attachments, invitation).
pub fn build_message(a: &LiveAccount, m: &OutMsg) -> Result<lettre::Message, String> {
    use lettre::message::{header::ContentType, Attachment, Mailbox, Mailboxes, MultiPart, SinglePart};
    let from: Mailbox = format!("{} <{}>", if a.full_name.is_empty() { &a.name } else { &a.full_name }, a.email)
        .parse()
        .or_else(|_| a.email.parse())
        .map_err(|e| format!("Bad sender address: {e}"))?;
    let mut b = lettre::Message::builder().from(from).subject(m.subject.clone()).date_now();
    let mut any_to = false;
    for (field, list) in [("to", &m.to), ("cc", &m.cc), ("bcc", &m.bcc)] {
        if list.trim().is_empty() {
            continue;
        }
        let parsed: Mailboxes = list.trim().trim_end_matches(',').parse().map_err(|e| format!("Check the {} addresses: {e}", field.to_uppercase()))?;
        for mb in parsed {
            any_to = true;
            b = match field {
                "to" => b.to(mb),
                "cc" => b.cc(mb),
                _ => b.bcc(mb),
            };
        }
    }
    if !any_to {
        if !m.draft {
            return Err("Add someone to send it to.".into());
        }
        // A draft with nobody in To yet: give it an envelope of its own.
        let me: lettre::Address = a.email.parse().map_err(|e| format!("Bad sender address: {e}"))?;
        b = b.envelope(lettre::address::Envelope::new(Some(me.clone()), vec![me]).map_err(|e| e.to_string())?);
    }
    if !m.message_id.is_empty() {
        b = b.message_id(Some(m.message_id.clone()));
    }
    if !m.in_reply_to.is_empty() {
        b = b.in_reply_to(m.in_reply_to.clone()).references(m.in_reply_to.clone());
    }
    let text = SinglePart::builder().header(ContentType::TEXT_PLAIN).body(m.body.clone());
    let mut alt = if m.html.is_empty() {
        None
    } else {
        Some(MultiPart::alternative().singlepart(text.clone()).singlepart(SinglePart::builder().header(ContentType::TEXT_HTML).body(m.html.clone())))
    };
    if let Some((method, ics)) = &m.calendar {
        let ct = ContentType::parse(&format!("text/calendar; method={method}; charset=UTF-8")).map_err(|e| e.to_string())?;
        let part = SinglePart::builder().header(ct).body(ics.clone());
        alt = Some(match alt {
            Some(a) => a.singlepart(part),
            None => MultiPart::alternative().singlepart(text.clone()).singlepart(part),
        });
    }
    let msg = if m.attachments.is_empty() {
        match alt {
            Some(a) => b.multipart(a),
            None => b.singlepart(text),
        }
    } else {
        let mut mixed = match alt {
            Some(a) => MultiPart::mixed().multipart(a),
            None => MultiPart::mixed().singlepart(text),
        };
        for path in &m.attachments {
            let bytes = std::fs::read(path).map_err(|e| format!("Could not read {path}: {e}"))?;
            let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "file".into());
            let ct = ContentType::parse(content_type_for(path)).map_err(|e| e.to_string())?;
            mixed = mixed.singlepart(Attachment::new(name).body(bytes, ct));
        }
        b.multipart(mixed)
    };
    msg.map_err(|e| e.to_string())
}

fn smtp(a: &LiveAccount) -> Result<lettre::SmtpTransport, String> {
    if crate::offline() {
        return Err("offline (ALMANAC_OFFLINE)".into());
    }
    use lettre::transport::smtp::authentication::{Credentials, Mechanism};
    let builder = if is_local(&a.smtp_host) {
        // A server on this computer (Proton Mail Bridge): its own certificate.
        use lettre::transport::smtp::client::{Tls, TlsParameters};
        let params = TlsParameters::builder(a.smtp_host.clone()).dangerous_accept_invalid_certs(true).dangerous_accept_invalid_hostnames(true).build().map_err(|e| e.to_string())?;
        lettre::SmtpTransport::builder_dangerous(&a.smtp_host).tls(if a.smtp_port == 465 { Tls::Wrapper(params) } else { Tls::Required(params) })
    } else if a.smtp_port == 465 {
        lettre::SmtpTransport::relay(&a.smtp_host).map_err(|e| e.to_string())?
    } else {
        lettre::SmtpTransport::starttls_relay(&a.smtp_host).map_err(|e| e.to_string())?
    }
    .port(a.smtp_port)
    .timeout(Some(std::time::Duration::from_secs(90)));
    Ok(if a.provider.is_empty() {
        builder.credentials(Credentials::new(a.username.clone(), need_password(a)?)).build()
    } else {
        let token = token(a, crate::oauth::Resource::Mail)?;
        builder
            .credentials(Credentials::new(a.email.clone(), token))
            .authentication(vec![Mechanism::Xoauth2])
            .build()
    })
}

/// Sends a message with SMTP and keeps a copy in the Sent folder.
pub fn send_msg(a: &LiveAccount, m: &OutMsg, sent_path: Option<&str>) -> Result<(), String> {
    use lettre::Transport;
    let msg = build_message(a, m)?;
    if a.is_ews() {
        // Exchange sends it and keeps the copy in Sent Items itself.
        return crate::ews::send(a, &msg.formatted());
    }
    smtp(a)?.send(&msg).map_err(|e| format!("Sending failed: {e}"))?;
    // Gmail and Outlook file sent mail themselves; others need a copy.
    if let Some(path) = sent_path {
        if a.provider.is_empty() && !a.smtp_host.contains("gmail") && !a.smtp_host.contains("office365") {
            if let Ok(mut s) = imap_connect(a) {
                let _ = s.append(path, &msg.formatted()).flag(imap::types::Flag::Seen).finish();
                let _ = s.logout();
            }
        }
    }
    Ok(())
}

/// Saves a draft on the server.
/// Saves a draft on the server, replacing the earlier copy of the same
/// draft (same Message-ID) so drafts don't pile up.
pub fn save_draft_msg(a: &LiveAccount, drafts_path: &str, m: &OutMsg) -> Result<(), String> {
    if let Some(r) = local_only(a) {
        return r;
    }
    let mut m = m.clone();
    m.draft = true;
    let msg = build_message(a, &m)?;
    let mut s = imap_connect(a)?;
    if !m.message_id.is_empty() {
        let _ = remove_by_message_id(&mut s, drafts_path, &m.message_id);
    }
    let r = s.append(drafts_path, &msg.formatted()).flag(imap::types::Flag::Draft).flag(imap::types::Flag::Seen).finish();
    let _ = s.logout();
    r.map(|_| ()).map_err(|e| e.to_string())
}

/// Removes a draft from the server once it's been sent (or thrown away).
pub fn delete_draft(a: &LiveAccount, drafts_path: &str, message_id: &str) -> Result<(), String> {
    if let Some(r) = local_only(a) {
        return r;
    }
    if message_id.is_empty() {
        return Ok(());
    }
    let mut s = imap_connect(a)?;
    let r = remove_by_message_id(&mut s, drafts_path, message_id);
    let _ = s.logout();
    r
}

fn remove_by_message_id(s: &mut Session, folder: &str, message_id: &str) -> Result<(), String> {
    s.select(folder).map_err(|e| e.to_string())?;
    let id = message_id.replace('"', "");
    let uids = s.uid_search(format!("HEADER Message-ID \"{id}\"")).map_err(|e| e.to_string())?;
    if uids.is_empty() {
        return Ok(());
    }
    let set = uids.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(",");
    s.uid_store(&set, "+FLAGS (\\Deleted)").map_err(|e| e.to_string())?;
    // Only these messages (UID EXPUNGE) when the server can; otherwise all marked ones.
    if s.uid_expunge(&set).is_err() {
        s.expunge().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// The whole original message from the server (for attachments).
pub fn fetch_full(a: &LiveAccount, folder_path: &str, uid: u32) -> Result<Vec<u8>, String> {
    let mut s = imap_connect(a)?;
    s.examine(folder_path).map_err(|e| e.to_string())?;
    let f = s.uid_fetch(uid.to_string(), "BODY.PEEK[]").map_err(|e| e.to_string())?;
    let raw = f.iter().next().and_then(|x| x.body().map(|b| b.to_vec())).ok_or("The message is no longer on the server")?;
    let _ = s.logout();
    Ok(raw)
}

/// Emails an invitation, update, cancellation or reply for an event.
pub fn send_invitation(a: &LiveAccount, e: &Event, method: &str) -> Result<(), String> {
    let mut ev = e.clone();
    let (to, verb) = match method {
        "REPLY" => {
            // Only my own answer goes back to the organiser.
            ev.attendees.retain(|x| x.email.eq_ignore_ascii_case(&a.email));
            let st = ev.attendees.first().map(|x| x.status.clone()).unwrap_or_default();
            let verb = match st.as_str() {
                "ACCEPTED" => "Accepted",
                "DECLINED" => "Declined",
                _ => "Tentative",
            };
            (ev.organizer.clone(), verb.to_string())
        }
        "CANCEL" => (ev.attendees.iter().map(|x| x.email.clone()).collect::<Vec<_>>().join(", "), "Canceled".to_string()),
        _ => (ev.attendees.iter().map(|x| x.email.clone()).collect::<Vec<_>>().join(", "), "Invitation".to_string()),
    };
    if to.trim().is_empty() {
        return Ok(());
    }
    let f = event_fields(&ev).ok_or("Bad event time")?;
    let ics = ics::imip(method, &f);
    let when = parse_dt(&ev.start).map(|t| t.format("%A %e %B %Y %H:%M").to_string()).unwrap_or_default();
    let body = format!(
        "{verb}: {}\n{when}{}\n{}",
        ev.title,
        if ev.tz.is_empty() { String::new() } else { format!(" ({})", ev.tz) },
        if ev.location.is_empty() { String::new() } else { format!("Where: {}\n", ev.location) }
    );
    let m = OutMsg { to, subject: format!("{verb}: {} @ {when}", ev.title), body, calendar: Some((method.to_string(), ics)), ..Default::default() };
    send_msg(a, &m, None)
}

// ------------------------------------------------------- calendars & contacts

fn sync_calendars(a: &LiveAccount, dav: &Dav, imp: &mut Imported, progress: &dyn Fn(String)) -> Result<(), String> {
    let cals = dav.calendars(&a.caldav_url)?;
    let from = (Utc::now() - Duration::days(400)).format("%Y%m%dT000000Z").to_string();
    let to = (Utc::now() + Duration::days(800)).format("%Y%m%dT000000Z").to_string();
    for (i, c) in cals.iter().enumerate() {
        progress(format!("{}: reading calendar {}…", a.name, c.name));
        let cal_id = format!("{}:{}", a.source(), c.url);
        imp.calendars.push(Calendar {
            id: cal_id.clone(),
            name: c.name.clone(),
            color: if c.color.is_empty() { ["#2f7cf6", "#e8453c", "#34a853", "#f5a623", "#9b59d0", "#1abc9c"][i % 6].into() } else { c.color.clone() },
            visible: true,
            source: a.source(),
            url: c.url.clone(),
            account: a.id.clone(),
        });
        for item in dav.events(&c.url, &from, &to)? {
            let mut evs = events_from_resource(&item.data, &cal_id, &a.source());
            for e in evs.iter_mut() {
                // A series and its changed occurrences share one server resource.
                e.href = item.href.clone();
                e.etag = item.etag.clone();
                e.raw = item.data.clone();
            }
            imp.events.extend(evs);
        }
    }
    Ok(())
}

fn sync_contacts(a: &LiveAccount, dav: &Dav, imp: &mut Imported, progress: &dyn Fn(String)) -> Result<(), String> {
    for book in dav.address_books(&a.carddav_url)? {
        progress(format!("{}: reading contacts {}…", a.name, book.name));
        let group_id = format!("{}:{}", a.source(), book.url);
        imp.groups.push(Group { id: group_id.clone(), name: format!("{} ({})", book.name, a.name), source: a.source(), url: book.url.clone() });
        for item in dav.cards(&book.url)? {
            let mut c = contact_from_vcard(ics::parse_vcard(&item.data), vec![group_id.clone()], &a.source());
            c.href = item.href;
            c.etag = item.etag;
            c.raw = item.data;
            imp.contacts.push(c);
        }
    }
    Ok(())
}

/// Reads everything from one account. Parts that fail are reported but
/// don't stop the others.
/// A regular check of an account. `fresh` gets new messages a mailbox at a
/// time (Inbox first), so they show up without waiting for the whole check.
pub fn sync(
    a: &LiveAccount,
    known: &HashMap<String, Vec<u32>>,
    marks: &HashMap<String, crate::data::FolderMark>,
    all: bool,
    progress: &dyn Fn(String),
    fresh: Option<&dyn Fn(Vec<Message>)>,
) -> (Imported, Vec<String>) {
    let mut imp = Imported { source: a.source(), server_truth: true, ..Default::default() };
    let mut errors = Vec::new();
    if !a.imap_host.is_empty() {
        if let Err(e) = sync_mail(a, &mut imp, known, marks, all, progress, fresh, false) {
            errors.push(format!("Mail: {e}"));
        }
    }
    if a.provider == crate::oauth::GOOGLE || a.provider == crate::oauth::MICROSOFT {
        match token(a, crate::oauth::Resource::Data) {
            Err(e) => errors.push(format!("Calendars: {e}")),
            Ok(tok) => {
                let (cal, cards) = if a.provider == crate::oauth::GOOGLE {
                    (crate::cloud::google_sync_calendars(a, &tok, &mut imp, progress), crate::cloud::google_sync_contacts(a, &tok, &mut imp, progress))
                } else {
                    (crate::cloud::graph_sync_calendars(a, &tok, &mut imp, progress), crate::cloud::graph_sync_contacts(a, &tok, &mut imp, progress))
                };
                if let Err(e) = cal {
                    errors.push(format!("Calendars: {e}"));
                }
                if let Err(e) = cards {
                    errors.push(format!("Contacts: {e}"));
                }
            }
        }
    } else if !a.caldav_url.is_empty() || !a.carddav_url.is_empty() {
        match need_password(a) {
            Err(e) => errors.push(e),
            Ok(pass) => {
                let dav = Dav::new(&a.username, &pass);
                if !a.caldav_url.is_empty() {
                    if let Err(e) = sync_calendars(a, &dav, &mut imp, progress) {
                        errors.push(format!("Calendars: {e}"));
                    }
                }
                if !a.carddav_url.is_empty() {
                    if let Err(e) = sync_contacts(a, &dav, &mut imp, progress) {
                        errors.push(format!("Contacts: {e}"));
                    }
                }
            }
        }
    }
    (imp, errors)
}

/// Checks the sign-in quickly (mail first, then calendars).
pub fn test(a: &LiveAccount) -> Result<(), String> {
    if !a.provider.is_empty() {
        return token(a, crate::oauth::Resource::Data).map(|_| ());
    }
    if a.is_pop() {
        return crate::pop::test(a);
    }
    if a.is_ews() {
        return crate::ews::test(a);
    }
    if !a.imap_host.is_empty() {
        let mut s = imap_connect(a)?;
        let _ = s.logout();
        return Ok(());
    }
    let pass = need_password(a)?;
    let dav = Dav::new(&a.username, &pass);
    if !a.caldav_url.is_empty() {
        dav.calendars(&a.caldav_url).map(|_| ())
    } else {
        dav.address_books(&a.carddav_url).map(|_| ())
    }
}

// ------------------------------------------------------- writing changes

pub fn event_fields<'a>(e: &'a Event) -> Option<ics::EventFields<'a>> {
    Some(ics::EventFields {
        uid: &e.uid,
        title: &e.title,
        start: parse_dt(&e.start)?,
        end: parse_dt(&e.end)?,
        zone: if e.all_day { "" } else { &e.tz },
        all_day: e.all_day,
        location: &e.location,
        notes: &e.notes,
        notes_html: &e.notes_html,
        rrule: &e.rrule,
        exdates: &e.exdates,
        alarms: &e.alarms,
        attendees: &e.attendees,
        organizer: &e.organizer,
        recurrence_id: parse_dt(&e.recurrence_id),
    })
}

/// The calendar text for a series plus its changed occurrences.
pub fn resource_text(master: &Event, overrides: &[Event]) -> Option<String> {
    let f = event_fields(master)?;
    let o: Vec<ics::EventFields> = overrides.iter().filter_map(event_fields).collect();
    Some(if master.raw.is_empty() { ics::new_vcalendar(&f, &o) } else { ics::update_vcalendar(&master.raw, &f, &o) })
}

/// Creates or updates an event on the server; returns (href, etag).
pub fn put_event(a: &LiveAccount, cal_url: &str, e: &Event, overrides: &[Event]) -> Result<(String, String), String> {
    let pass = need_password(a)?;
    let dav = Dav::new(&a.username, &pass);
    let data = resource_text(e, overrides).ok_or("Bad event time")?;
    let href = if e.href.is_empty() {
        crate::dav::join(&format!("{}/", cal_url.trim_end_matches('/')), &format!("{}.ics", e.uid.replace(['/', '@', ' '], "-")))
    } else {
        e.href.clone()
    };
    let etag = dav.put(&href, &e.etag, &data, "text/calendar; charset=utf-8")?;
    Ok((href, etag))
}

/// Writes a series (and its changed occurrences) to wherever its calendar
/// lives. Returns (server id or address, change tag, calendar text).
pub fn push_event(a: &LiveAccount, cal: &Calendar, master: &Event, overrides: &[Event]) -> Result<(String, String, String), String> {
    if let Some(cid) = cal.url.strip_prefix("google:") {
        let tok = token(a, crate::oauth::Resource::Data)?;
        let (h, e) = crate::cloud::google_push_event(&tok, cid, master, overrides)?;
        return Ok((h, e, String::new()));
    }
    if let Some(cid) = cal.url.strip_prefix("graph:") {
        let tok = token(a, crate::oauth::Resource::Data)?;
        let (h, e) = crate::cloud::graph_push_event(&tok, cid, master, overrides)?;
        return Ok((h, e, String::new()));
    }
    let raw = resource_text(master, overrides).ok_or("Bad event time")?;
    let (href, etag) = put_event(a, &cal.url, master, overrides)?;
    Ok((href, etag, raw))
}

pub fn delete_event_remote(a: &LiveAccount, cal: &Calendar, e: &Event) -> Result<(), String> {
    if let Some(cid) = cal.url.strip_prefix("google:") {
        return crate::cloud::google_delete_event(&token(a, crate::oauth::Resource::Data)?, cid, &e.href);
    }
    if cal.url.starts_with("graph:") {
        return crate::cloud::graph_delete(&token(a, crate::oauth::Resource::Data)?, &format!("events/{}", e.href));
    }
    delete_remote(a, &e.href, &e.etag)
}

/// Writes a contact to its address book; returns (server id or address, change tag).
pub fn push_contact(a: &LiveAccount, book_url: &str, c: &Contact) -> Result<(String, String), String> {
    if book_url.starts_with("google:") {
        return crate::cloud::google_push_contact(&token(a, crate::oauth::Resource::Data)?, c);
    }
    if book_url.starts_with("graph:") {
        return crate::cloud::graph_push_contact(&token(a, crate::oauth::Resource::Data)?, c);
    }
    put_contact(a, book_url, c)
}

pub fn delete_contact_remote(a: &LiveAccount, c: &Contact) -> Result<(), String> {
    match a.provider.as_str() {
        crate::oauth::GOOGLE => crate::cloud::google_delete_contact(&token(a, crate::oauth::Resource::Data)?, &c.href),
        crate::oauth::MICROSOFT => crate::cloud::graph_delete(&token(a, crate::oauth::Resource::Data)?, &format!("contacts/{}", c.href)),
        _ => delete_remote(a, &c.href, &c.etag),
    }
}

pub fn delete_remote(a: &LiveAccount, href: &str, etag: &str) -> Result<(), String> {
    let pass = need_password(a)?;
    Dav::new(&a.username, &pass).delete(href, etag)
}

/// Creates or updates a contact card on the server; returns (href, etag).
pub fn put_contact(a: &LiveAccount, book_url: &str, c: &Contact) -> Result<(String, String), String> {
    let pass = need_password(a)?;
    let dav = Dav::new(&a.username, &pass);
    let uid = if c.uid.is_empty() { format!("{}-{}@craft-pim", Utc::now().timestamp_millis(), c.id) } else { c.uid.clone() };
    let data = ics::write_vcard(c, &uid, &c.raw);
    let href = if c.href.is_empty() {
        crate::dav::join(&format!("{}/", book_url.trim_end_matches('/')), &format!("{}.vcf", uid.replace(['/', '@', ' '], "-")))
    } else {
        c.href.clone()
    };
    let etag = dav.put(&href, &c.etag, &data, "text/vcard; charset=utf-8")?;
    Ok((href, etag))
}

#[allow(dead_code)]
pub fn zone_note() -> String {
    tz::system_zone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_offered_provider_has_what_it_needs() {
        let list = providers_json();
        for p in list.as_array().unwrap() {
            let how = p["how"].as_str().unwrap();
            assert!(["oauth", "ews", "bridge", "form", "none"].contains(&how), "{p}");
            if how == "oauth" {
                assert!(crate::oauth::PROVIDERS.iter().any(|(id, _, _)| *id == p["oauth"].as_str().unwrap()), "{p}");
            }
            // Each address-and-password provider fills in its own servers.
            if how == "form" {
                let a = preset(&format!("someone@{}", p["domain"].as_str().unwrap()));
                // (Unknown domains get guessed servers, with https://<domain>/ for calendars.)
                assert_ne!(a.caldav_url, format!("https://{}/", p["domain"].as_str().unwrap()), "{} has servers of its own", p["name"]);
            }
        }
        assert_eq!(preset("me@proton.me").imap_port, 1143);
        assert_eq!(preset("me@mail.com").imap_host, "imap.mail.com");
        assert_eq!(preset("me@yandex.ru").smtp_host, "smtp.yandex.com");
    }

    /// Talks to a real CalDAV/CardDAV server when CRAFT_DAV_TEST_URL is set
    /// (e.g. a local Radicale with a calendar and an address book).
    #[test]
    fn dav_round_trip_against_server() {
        let Ok(base) = std::env::var("CRAFT_DAV_TEST_URL") else { return };
        let dav = Dav::new("tester", "pw");
        let cals = dav.calendars(&base).expect("find calendars");
        assert!(!cals.is_empty(), "no calendars found");
        let cal = &cals[0];
        let ev = Event {
            uid: "rt-1@craft".into(),
            title: "Round trip".into(),
            start: "2026-11-10T09:00".into(),
            end: "2026-11-10T10:00".into(),
            tz: "Europe/Berlin".into(),
            rrule: "FREQ=WEEKLY;COUNT=3".into(),
            alarms: vec![15],
            attendees: vec![Attendee { email: "a@example.com".into(), name: "A".into(), status: "NEEDS-ACTION".into() }],
            organizer: "me@example.com".into(),
            ..Default::default()
        };
        let href = crate::dav::join(&format!("{}/", cal.url.trim_end_matches('/')), "rt-1.ics");
        dav.put(&href, "", &resource_text(&ev, &[]).unwrap(), "text/calendar; charset=utf-8").expect("create");
        let items = dav.events(&cal.url, "20261101T000000Z", "20261201T000000Z").expect("read");
        let item = items.iter().find(|i| i.href == href).expect("event on server");
        let got = events_from_resource(&item.data, "c", "live:x");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].title, "Round trip");
        assert_eq!(got[0].tz, "Europe/Berlin");
        assert_eq!(got[0].start, "2026-11-10T09:00");
        assert_eq!(got[0].alarms, vec![15]);
        assert_eq!(got[0].attendees.len(), 1);

        // Change the series and move just its second occurrence.
        let mut master = got[0].clone();
        master.title = "Changed".into();
        master.raw = item.data.clone();
        master.exdates = vec!["2026-11-17T09:00".into()];
        let ov = Event {
            recurrence_id: "2026-11-17T09:00".into(),
            start: "2026-11-17T14:00".into(),
            end: "2026-11-17T15:00".into(),
            rrule: String::new(),
            exdates: vec![],
            title: "Moved one".into(),
            ..master.clone()
        };
        let etag2 = dav.put(&href, &item.etag, &resource_text(&master, &[ov]).unwrap(), "text/calendar; charset=utf-8").expect("update");
        // An out-of-date change tag must be refused.
        assert!(dav.put(&href, &item.etag, &resource_text(&master, &[]).unwrap(), "text/calendar").is_err(), "stale update accepted");
        let items = dav.events(&cal.url, "20261101T000000Z", "20261201T000000Z").expect("read again");
        let item = items.iter().find(|i| i.href == href).expect("still there");
        let got = events_from_resource(&item.data, "c", "live:x");
        assert_eq!(got.len(), 2);
        let m = got.iter().find(|e| e.recurrence_id.is_empty()).unwrap();
        assert_eq!(m.title, "Changed");
        assert!(m.exdates.contains(&"2026-11-17T09:00".to_string()));
        let o = got.iter().find(|e| !e.recurrence_id.is_empty()).unwrap();
        assert_eq!(o.start, "2026-11-17T14:00");
        let tag = if etag2.is_empty() { item.etag.clone() } else { etag2 };
        dav.delete(&href, &tag).expect("delete");
        let items = dav.events(&cal.url, "20261101T000000Z", "20261201T000000Z").expect("read after delete");
        assert!(!items.iter().any(|i| i.href == href));

        // Contacts
        let books = dav.address_books(&base).expect("find address books");
        assert!(!books.is_empty(), "no address books found");
        let c = Contact {
            first: "Ada".into(),
            last: "Lovelace".into(),
            nickname: "Countess".into(),
            emails: vec![Labeled { label: "home".into(), value: "ada@example.com".into() }],
            phones: vec![Labeled { label: "mobile".into(), value: "+44 20 0000".into() }],
            urls: vec![Labeled { label: "homepage".into(), value: "https://example.com".into() }],
            birthday: "1815-12-10".into(),
            ..Default::default()
        };
        let chref = crate::dav::join(&format!("{}/", books[0].url.trim_end_matches('/')), "ada.vcf");
        dav.put(&chref, "", &ics::write_vcard(&c, "ada-1", ""), "text/vcard; charset=utf-8").expect("create card");
        let cards = dav.cards(&books[0].url).expect("read cards");
        let card = cards.iter().find(|i| i.href == chref).expect("card on server");
        let back = contact_from_vcard(ics::parse_vcard(&card.data), vec![], "live:x");
        assert_eq!(back.last, "Lovelace");
        assert_eq!(back.nickname, "Countess");
        assert_eq!(back.birthday, "1815-12-10");
        assert_eq!(back.emails[0].value, "ada@example.com");
        assert_eq!(back.urls.len(), 1);
        dav.delete(&chref, &card.etag).expect("delete card");
    }

    #[test]
    fn drafts_keep_their_message_id_and_need_no_recipient() {
        let a = LiveAccount { email: "me@example.com".into(), name: "Me".into(), ..Default::default() };
        let mut m = OutMsg { subject: "Half written".into(), body: "Hello".into(), message_id: "<d1.abc@almanac.local>".into(), draft: true, ..Default::default() };
        let text = String::from_utf8(build_message(&a, &m).expect("draft without To builds").formatted()).unwrap();
        assert!(text.contains("Message-ID: <d1.abc@almanac.local>"), "{text}");
        assert!(!text.contains("\nTo:"), "no made-up recipient: {text}");
        m.draft = false;
        assert!(build_message(&a, &m).is_err(), "sending still needs someone to send to");
    }

    #[test]
    fn folder_names() {
        assert_eq!(super::decode_mutf7("Entw&APw-rfe"), "Entwürfe");
        assert_eq!(super::decode_mutf7("A&-B"), "A&B");
    }
}
