//! Big attachments sent as links, as Apple Mail's Mail Drop does: the file is
//! uploaded to the person's cloud storage, shared with "anyone with the link
//! can view", and the message carries the link instead. Each service is a
//! small implementation of `CloudStore` (Google Drive here; OneDrive,
//! Dropbox, Box and Nextcloud alike).

use serde_json::{json, Value};
use std::io::Read;

/// A file uploaded and shared.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct Shared {
    pub name: String,
    pub size: u64,
    /// Where anyone with the link can see (and download) it.
    pub url: String,
    /// "Google Drive", "OneDrive", …
    pub provider: String,
    /// The service's own id for it (to remove it again).
    pub id: String,
}

/// A cloud storage service that can take a file and share it by link.
pub trait CloudStore {
    fn name(&self) -> &'static str;
    /// Uploads `path` (in pieces that can be resumed), reporting (sent, total)
    /// as it goes, then shares it with anyone who has the link.
    fn upload(&self, path: &std::path::Path, progress: &dyn Fn(u64, u64)) -> Result<Shared, String>;
    /// Removes a file it uploaded.
    fn delete(&self, id: &str) -> Result<(), String>;
}

/// The cloud storage that goes with a kind of mail account ("" for none yet).
pub fn store_name(provider: &str) -> &'static str {
    match provider {
        "google" => "Google Drive",
        "microsoft" => "OneDrive",
        _ => "",
    }
}

/// That storage, signed in with the account's token.
pub fn store_for(provider: &str, token: String) -> Option<Box<dyn CloudStore + Send>> {
    match provider {
        "google" => Some(Box::new(GoogleDrive::new(token))),
        "microsoft" => Some(Box::new(OneDrive::new(token))),
        _ => None,
    }
}

/// The largest message each kind of mail account takes, attachments included
/// (as sent: about a third bigger than the files themselves).
pub fn mail_limit(provider: &str) -> u64 {
    match provider {
        "microsoft" => 20 * 1024 * 1024,
        _ => 25 * 1024 * 1024,
    }
}

/// How big files get once in a message (base64: 4 bytes for every 3, plus line breaks).
pub fn as_sent(bytes: u64) -> u64 {
    bytes / 3 * 4 + bytes / 57 * 2 + 4096
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        // (Drive answers a piece with 308 "Resume Incomplete": not a redirect.)
        .max_redirects(0)
        .max_redirects_will_error(false)
        .timeout_global(Some(std::time::Duration::from_secs(600)))
        .build()
        .into()
}

/// Pieces of 10 MB (Google wants multiples of 256 KB, OneDrive of 320 KB).
const CHUNK: usize = 10 * 1024 * 1024;

/// A file's next piece (up to CHUNK bytes) read into `buf`: how many bytes.
fn read_piece(file: &mut std::fs::File, buf: &mut [u8]) -> Result<usize, String> {
    let mut n = 0;
    while n < buf.len() {
        let r = file.read(&mut buf[n..]).map_err(|e| e.to_string())?;
        if r == 0 {
            break;
        }
        n += r;
    }
    Ok(n)
}

fn json_of(resp: &mut ureq::http::Response<ureq::Body>) -> Value {
    serde_json::from_str(&resp.body_mut().read_to_string().unwrap_or_default()).unwrap_or(Value::Null)
}

/// OneDrive (Microsoft accounts, through Microsoft Graph): files go in an
/// "Almanac" folder, shared with "anyone with the link can view".
pub struct OneDrive {
    pub token: String,
    pub base: String,
}

impl OneDrive {
    pub fn new(token: String) -> OneDrive {
        let base = std::env::var("ALMANAC_ONEDRIVE_TEST_BASE").unwrap_or_else(|_| "https://graph.microsoft.com/v1.0".into());
        OneDrive { token, base }
    }

    fn call(&self, method: &str, url: &str, body: Option<Value>) -> Result<Value, String> {
        let mut b = ureq::http::Request::builder().method(method).uri(url).header("Authorization", format!("Bearer {}", self.token));
        if body.is_some() {
            b = b.header("Content-Type", "application/json");
        }
        let req = b.body(body.map(|v| v.to_string()).unwrap_or_default()).map_err(|e| e.to_string())?;
        let mut resp = agent().run(req).map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        let v = json_of(&mut resp);
        if status >= 400 {
            return Err(graph_error(status, &v));
        }
        Ok(v)
    }
}

/// What a OneDrive (Graph) error means for the person.
fn graph_error(status: u16, v: &Value) -> String {
    let code = v.pointer("/error/code").and_then(Value::as_str).unwrap_or("");
    let msg = v.pointer("/error/message").and_then(Value::as_str).unwrap_or("");
    if status == 401 || code == "accessDenied" && msg.to_lowercase().contains("scope") {
        return "Almanac isn't allowed to use your OneDrive yet: press Sign In Again in Accounts to allow it".into();
    }
    if code == "quotaLimitReached" || code == "insufficientStorage" {
        return "Your OneDrive is full".into();
    }
    if code == "accessDenied" || code == "notAllowed" {
        return "Your organization doesn't allow sharing files with anyone who has the link".into();
    }
    if msg.is_empty() { format!("OneDrive: HTTP {status}") } else { format!("OneDrive: {msg}") }
}

/// A name safe in a OneDrive path.
fn path_part(name: &str) -> String {
    let mut out = String::new();
    for b in name.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~ ()".contains(&b) {
            if b == b' ' { out.push_str("%20") } else { out.push(b as char) }
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

impl CloudStore for OneDrive {
    fn name(&self) -> &'static str {
        "OneDrive"
    }

    fn upload(&self, path: &std::path::Path, progress: &dyn Fn(u64, u64)) -> Result<Shared, String> {
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "file".into());
        let mut file = std::fs::File::open(path).map_err(|e| format!("Could not read {name}: {e}"))?;
        let size = file.metadata().map_err(|e| e.to_string())?.len();
        // An upload session (a file of the same name gets a new name, not replaced).
        let session = self.call(
            "POST",
            &format!("{}/me/drive/root:/Almanac/{}:/createUploadSession", self.base, path_part(&name)),
            Some(json!({"item": {"@microsoft.graph.conflictBehavior": "rename", "name": name}})),
        )?;
        let upload_url = session.get("uploadUrl").and_then(Value::as_str).ok_or("OneDrive didn't start the upload")?.to_string();
        let mut buf = vec![0u8; CHUNK];
        let mut sent: u64 = 0;
        progress(0, size);
        let item = loop {
            let n = read_piece(&mut file, &mut buf)?;
            // (The upload address carries its own permission: no sign-in header.)
            let put = ureq::http::Request::builder()
                .method("PUT")
                .uri(&upload_url)
                .header("Content-Range", format!("bytes {}-{}/{}", sent, sent + n.max(1) as u64 - 1, size))
                .body(buf[..n].to_vec())
                .map_err(|e| e.to_string())?;
            let mut resp = agent().run(put).map_err(|e| e.to_string())?;
            let status = resp.status().as_u16();
            let v = json_of(&mut resp);
            sent += n as u64;
            progress(sent.min(size), size);
            match status {
                202 => continue,
                200 | 201 => break v,
                _ => return Err(graph_error(status, &v)),
            }
        };
        let id = item.get("id").and_then(Value::as_str).ok_or("OneDrive didn't say where the file went")?.to_string();
        let link = self.call("POST", &format!("{}/me/drive/items/{id}/createLink", self.base), Some(json!({"type": "view", "scope": "anonymous"})))?;
        let url = link.pointer("/link/webUrl").and_then(Value::as_str).ok_or("OneDrive gave no link")?.to_string();
        Ok(Shared { name, size, url, provider: self.name().into(), id })
    }

    fn delete(&self, id: &str) -> Result<(), String> {
        self.call("DELETE", &format!("{}/me/drive/items/{id}", self.base), None).map(|_| ())
    }
}

/// Google Drive (the drive.file permission: only files Almanac makes).
pub struct GoogleDrive {
    pub token: String,
    /// The API's address (a test server in the tests).
    pub base: String,
}

impl GoogleDrive {
    pub fn new(token: String) -> GoogleDrive {
        let base = std::env::var("ALMANAC_DRIVE_TEST_BASE").unwrap_or_else(|_| "https://www.googleapis.com".into());
        GoogleDrive { token, base }
    }

    fn json(&self, method: &str, url: &str, body: Option<Value>) -> Result<Value, String> {
        let mut b = ureq::http::Request::builder()
            .method(method)
            .uri(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/json");
        if body.is_some() {
            b = b.header("Content-Type", "application/json; charset=UTF-8");
        }
        let req = b.body(body.map(|v| v.to_string()).unwrap_or_default()).map_err(|e| e.to_string())?;
        let mut resp = agent().run(req).map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        let text = resp.body_mut().read_to_string().unwrap_or_default();
        let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        if status >= 400 {
            return Err(drive_error(status, &v));
        }
        Ok(v)
    }
}

/// What a Drive error means for the person.
fn drive_error(status: u16, v: &Value) -> String {
    let msg = v.pointer("/error/message").and_then(Value::as_str).unwrap_or("").to_string();
    let reason = v.pointer("/error/errors/0/reason").and_then(Value::as_str).unwrap_or("");
    if msg.contains("has not been used in project") || msg.contains("is disabled") || reason == "accessNotConfigured" {
        return "The Google Drive API isn't turned on for your Google sign-in project: Sign-in Setup can turn it on".into();
    }
    if status == 401 || status == 403 && (reason == "insufficientPermissions" || msg.contains("insufficient")) {
        return "Almanac isn't allowed to use your Google Drive yet: sign in again in Accounts to allow it".into();
    }
    if reason == "storageQuotaExceeded" {
        return "Your Google Drive is full".into();
    }
    if msg.is_empty() { format!("Google Drive: HTTP {status}") } else { format!("Google Drive: {msg}") }
}

impl CloudStore for GoogleDrive {
    fn name(&self) -> &'static str {
        "Google Drive"
    }

    fn upload(&self, path: &std::path::Path, progress: &dyn Fn(u64, u64)) -> Result<Shared, String> {
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "file".into());
        let mut file = std::fs::File::open(path).map_err(|e| format!("Could not read {name}: {e}"))?;
        let size = file.metadata().map_err(|e| e.to_string())?.len();
        // Start a resumable upload: Drive answers with where to send the bytes.
        let start = ureq::http::Request::builder()
            .method("POST")
            .uri(format!("{}/upload/drive/v3/files?uploadType=resumable&fields=id", self.base))
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Content-Type", "application/json; charset=UTF-8")
            .header("X-Upload-Content-Length", size.to_string())
            .body(json!({"name": name}).to_string())
            .map_err(|e| e.to_string())?;
        let mut resp = agent().run(start).map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        if status >= 400 {
            let v: Value = serde_json::from_str(&resp.body_mut().read_to_string().unwrap_or_default()).unwrap_or(Value::Null);
            return Err(drive_error(status, &v));
        }
        let session = resp.headers().get("location").and_then(|h| h.to_str().ok()).ok_or("Google Drive didn't start the upload")?.to_string();
        // The bytes, a piece at a time.
        let mut sent: u64 = 0;
        let mut buf = vec![0u8; CHUNK];
        let id;
        progress(0, size);
        loop {
            let mut n = 0;
            while n < CHUNK {
                let r = file.read(&mut buf[n..]).map_err(|e| e.to_string())?;
                if r == 0 {
                    break;
                }
                n += r;
            }
            let range = if size == 0 { "bytes */0".to_string() } else { format!("bytes {}-{}/{}", sent, sent + n as u64 - 1, size) };
            let put = ureq::http::Request::builder()
                .method("PUT")
                .uri(&session)
                .header("Content-Range", range)
                .body(buf[..n].to_vec())
                .map_err(|e| e.to_string())?;
            let mut resp = agent().run(put).map_err(|e| e.to_string())?;
            let status = resp.status().as_u16();
            sent += n as u64;
            progress(sent.min(size), size);
            match status {
                // More to send.
                308 => continue,
                200 | 201 => {
                    let v: Value = serde_json::from_str(&resp.body_mut().read_to_string().unwrap_or_default()).unwrap_or(Value::Null);
                    id = v.get("id").and_then(Value::as_str).unwrap_or("").to_string();
                    break;
                }
                _ => {
                    let v: Value = serde_json::from_str(&resp.body_mut().read_to_string().unwrap_or_default()).unwrap_or(Value::Null);
                    return Err(drive_error(status, &v));
                }
            }
        }
        if id.is_empty() {
            return Err("Google Drive didn't say where the file went".into());
        }
        // Anyone with the link can see it.
        self.json("POST", &format!("{}/drive/v3/files/{id}/permissions", self.base), Some(json!({"type": "anyone", "role": "reader"})))?;
        let v = self.json("GET", &format!("{}/drive/v3/files/{id}?fields=webViewLink", self.base), None)?;
        let url = v.get("webViewLink").and_then(Value::as_str).ok_or("Google Drive gave no link")?.to_string();
        Ok(Shared { name, size, url, provider: self.name().into(), id })
    }

    fn delete(&self, id: &str) -> Result<(), String> {
        self.json("DELETE", &format!("{}/drive/v3/files/{id}", self.base), None).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};

    /// A stand-in for Drive's API: the resumable upload, the sharing and the
    /// link, checking what's sent. Returns its address and what it received.
    fn fake_drive() -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let log = seen.clone();
        let session = format!("{base}/session/1");
        let base2 = base.clone();
        std::thread::spawn(move || {
            let mut received: u64 = 0;
            let mut received_od: u64 = 0;
            for stream in listener.incoming().flatten() {
                let mut r = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                if r.read_line(&mut line).is_err() || line.is_empty() {
                    continue;
                }
                let mut len = 0usize;
                let mut range = String::new();
                let mut signed = false;
                loop {
                    let mut h = String::new();
                    r.read_line(&mut h).unwrap();
                    if h.trim().is_empty() {
                        break;
                    }
                    let lower = h.to_ascii_lowercase();
                    if let Some(v) = lower.strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap_or(0);
                    }
                    if let Some(v) = lower.strip_prefix("content-range:") {
                        range = v.trim().to_string();
                    }
                    signed |= lower.starts_with("authorization:");
                }
                let mut body = vec![0u8; len];
                r.read_exact(&mut body).unwrap();
                let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
                let method = line.split_whitespace().next().unwrap_or("").to_string();
                log.lock().unwrap().push(format!("{method} {path} {range}{}", if signed { " [signed]" } else { "" }));
                let json = |code: &str, b: &str| format!("HTTP/1.1 {code}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{b}", b.len());
                let mut out = stream;
                let reply = if path.starts_with("/upload/drive/v3/files") {
                    format!("HTTP/1.1 200 OK\r\nLocation: {session}\r\nContent-Length: 0\r\n\r\n")
                } else if path == "/session/1" {
                    received += body.len() as u64;
                    let total: u64 = range.rsplit('/').next().and_then(|t| t.parse().ok()).unwrap_or(0);
                    if received < total {
                        "HTTP/1.1 308 Resume Incomplete\r\nContent-Length: 0\r\n\r\n".to_string()
                    } else {
                        let b = r#"{"id":"F1"}"#;
                        format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{b}", b.len())
                    }
                } else if path.starts_with("/drive/v3/files/F1/permissions") {
                    let b = r#"{"id":"anyoneWithLink"}"#;
                    format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{b}", b.len())
                } else if path.starts_with("/drive/v3/files/F1") {
                    let b = r#"{"webViewLink":"https://drive.google.com/file/d/F1/view"}"#;
                    format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{b}", b.len())
                // OneDrive (Microsoft Graph)
                } else if path.starts_with("/me/drive/root:/Almanac/") && path.ends_with(":/createUploadSession") {
                    json("200 OK", &format!(r#"{{"uploadUrl":"{base2}/up/1"}}"#))
                } else if path == "/up/1" {
                    received_od += body.len() as u64;
                    let total: u64 = range.rsplit('/').next().and_then(|t| t.parse().ok()).unwrap_or(0);
                    if received_od < total { json("202 Accepted", "{}") } else { json("201 Created", r#"{"id":"O1"}"#) }
                } else if path == "/me/drive/items/O1/createLink" {
                    json("200 OK", r#"{"link":{"webUrl":"https://1drv.ms/u/s!O1"}}"#)
                } else {
                    "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_string()
                };
                let _ = out.write_all(reply.as_bytes());
            }
        });
        (base, seen)
    }

    #[test]
    fn a_big_file_goes_up_in_pieces_and_comes_back_as_a_link() {
        let (base, seen) = fake_drive();
        let dir = std::env::temp_dir().join(format!("almanac-drive-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("big video.mp4");
        std::fs::write(&path, vec![7u8; CHUNK * 2 + 1000]).unwrap();
        let drive = GoogleDrive { token: "t".into(), base };
        let steps = std::sync::Mutex::new(Vec::new());
        let shared = drive.upload(&path, &|sent, total| steps.lock().unwrap().push((sent, total))).expect("uploaded");
        assert_eq!(shared, Shared { name: "big video.mp4".into(), size: (CHUNK * 2 + 1000) as u64, url: "https://drive.google.com/file/d/F1/view".into(), provider: "Google Drive".into(), id: "F1".into() });
        let log = seen.lock().unwrap().clone();
        let puts: Vec<&String> = log.iter().filter(|l| l.starts_with("PUT /session/1")).collect();
        assert_eq!(puts.len(), 3, "three pieces: {log:?}");
        assert!(puts[0].ends_with(&format!("bytes 0-{}/{}", CHUNK - 1, CHUNK * 2 + 1000)), "{log:?}");
        assert!(log.iter().any(|l| l.starts_with("POST /drive/v3/files/F1/permissions")), "shared: {log:?}");
        let s = steps.lock().unwrap();
        assert_eq!(s.last(), Some(&((CHUNK * 2 + 1000) as u64, (CHUNK * 2 + 1000) as u64)), "progress to the end");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Against the real Google Drive of the first Google account in
    /// ALMANAC_REAL_DATA: the token's permissions, a 30 MB upload, the link
    /// opening for anyone, and the file removed again.
    #[test]
    #[ignore]
    fn drive_for_real() {
        let Some(dir) = std::env::var_os("ALMANAC_REAL_DATA").map(std::path::PathBuf::from) else { return };
        let db: crate::data::Db = serde_json::from_str(&std::fs::read_to_string(dir.join("data.json")).unwrap()).unwrap();
        let a = db.live.iter().find(|a| a.provider == "google").expect("a Google account").clone();
        let token = crate::live::token(&a, crate::oauth::Resource::Drive).expect("token");
        let mut info = agent().get(format!("https://oauth2.googleapis.com/tokeninfo?access_token={token}")).call().expect("tokeninfo");
        let info: Value = serde_json::from_str(&info.body_mut().read_to_string().unwrap()).unwrap();
        let scopes = info["scope"].as_str().unwrap_or("").to_string();
        eprintln!("scopes: {scopes}");
        assert!(scopes.contains("https://www.googleapis.com/auth/drive.file"), "no drive.file in {scopes}");
        let tmp = std::env::temp_dir().join(format!("almanac-real-drive-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("almanac-drive-test.bin");
        let mut bytes = vec![0u8; 30 * 1024 * 1024];
        ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut bytes).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        let drive = GoogleDrive::new(token);
        let t = std::time::Instant::now();
        let shared = drive.upload(&path, &|sent, total| eprintln!("  {sent}/{total} at {:?}", t.elapsed()));
        let _ = std::fs::remove_dir_all(&tmp);
        let shared = shared.expect("uploaded");
        eprintln!("uploaded {} ({} bytes) in {:?}: {}", shared.name, shared.size, t.elapsed(), shared.url);
        // Anyone with the link: the page opens without signing in (no redirect to a sign-in page).
        let status = agent().get(&shared.url).call().map(|r| r.status().as_u16()).map_err(|e| e.to_string());
        eprintln!("anonymous GET: {status:?}");
        drive.delete(&shared.id).expect("deleted");
        eprintln!("deleted {}", shared.id);
        assert_eq!(status, Ok::<u16, String>(200), "the link should open for anyone");
    }

    #[test]
    fn onedrive_takes_a_big_file_too() {
        let (base, seen) = fake_drive();
        let dir = std::env::temp_dir().join(format!("almanac-onedrive-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("big video.mp4");
        std::fs::write(&path, vec![7u8; CHUNK + 5000]).unwrap();
        let od = OneDrive { token: "t".into(), base };
        let shared = od.upload(&path, &|_, _| {}).expect("uploaded");
        assert_eq!((shared.url.as_str(), shared.id.as_str(), shared.provider.as_str()), ("https://1drv.ms/u/s!O1", "O1", "OneDrive"));
        let log = seen.lock().unwrap().clone();
        assert!(log.iter().any(|l| l.starts_with("POST /me/drive/root:/Almanac/big%20video.mp4:/createUploadSession") && l.ends_with("[signed]")), "{log:?}");
        let puts: Vec<&String> = log.iter().filter(|l| l.starts_with("PUT /up/1")).collect();
        assert_eq!(puts.len(), 2, "two pieces: {log:?}");
        assert!(puts.iter().all(|l| !l.ends_with("[signed]")), "the upload address needs no sign-in header: {log:?}");
        assert!(puts[1].contains(&format!("bytes {}-{}/{}", CHUNK, CHUNK + 4999, CHUNK + 5000)), "{log:?}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn sizes_once_in_a_message() {
        assert!(as_sent(18 * 1024 * 1024) > mail_limit("google") * 9 / 10, "18 MB is close to Gmail's 25 MB once encoded");
        assert!(as_sent(20 * 1024 * 1024) > mail_limit("google"), "20 MB doesn't fit");
        assert!(as_sent(1024 * 1024) < mail_limit("google"));
    }
}
