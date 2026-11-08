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

// ------------------------------------------------------------------ Dropbox

/// Dropbox (an app registered with Dropbox, signed in once): files go in an
/// "Almanac" folder, shared with "anyone with the link can view".
pub struct Dropbox {
    pub token: String,
    /// api.dropboxapi.com and content.dropboxapi.com (test servers in the tests).
    pub api: String,
    pub content: String,
}

impl Dropbox {
    pub fn new(token: String) -> Dropbox {
        let test = std::env::var("ALMANAC_DROPBOX_TEST_BASE").ok();
        Dropbox {
            token,
            api: test.clone().unwrap_or_else(|| "https://api.dropboxapi.com".into()),
            content: test.unwrap_or_else(|| "https://content.dropboxapi.com".into()),
        }
    }

    fn rpc(&self, path: &str, body: Value) -> Result<Value, String> {
        let req = ureq::http::Request::builder()
            .method("POST")
            .uri(format!("{}{path}", self.api))
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Content-Type", "application/json")
            .body(body.to_string())
            .map_err(|e| e.to_string())?;
        let mut resp = agent().run(req).map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        let v = json_of(&mut resp);
        if status >= 400 {
            return Err(dropbox_error(status, &v));
        }
        Ok(v)
    }

    /// One piece of a file to the content server (`arg`: Dropbox-API-Arg).
    fn content_call(&self, path: &str, arg: Value, bytes: &[u8]) -> Result<Value, String> {
        let req = ureq::http::Request::builder()
            .method("POST")
            .uri(format!("{}{path}", self.content))
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Content-Type", "application/octet-stream")
            .header("Dropbox-API-Arg", arg.to_string())
            .body(bytes.to_vec())
            .map_err(|e| e.to_string())?;
        let mut resp = agent().run(req).map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        let v = json_of(&mut resp);
        if status >= 400 {
            return Err(dropbox_error(status, &v));
        }
        Ok(v)
    }
}

fn dropbox_error(status: u16, v: &Value) -> String {
    let summary = v.get("error_summary").and_then(Value::as_str).unwrap_or("");
    if status == 401 {
        return "Almanac isn't signed in to Dropbox any more: Connect it again in Settings › Mail".into();
    }
    if summary.contains("insufficient_space") {
        return "Your Dropbox is full".into();
    }
    if summary.is_empty() { format!("Dropbox: HTTP {status}") } else { format!("Dropbox: {summary}") }
}

/// Dropbox takes pieces of a multiple of 4 MB.
const DROPBOX_CHUNK: usize = 8 * 1024 * 1024;

impl CloudStore for Dropbox {
    fn name(&self) -> &'static str {
        "Dropbox"
    }

    fn upload(&self, path: &std::path::Path, progress: &dyn Fn(u64, u64)) -> Result<Shared, String> {
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "file".into());
        let mut file = std::fs::File::open(path).map_err(|e| format!("Could not read {name}: {e}"))?;
        let size = file.metadata().map_err(|e| e.to_string())?.len();
        let mut buf = vec![0u8; DROPBOX_CHUNK];
        progress(0, size);
        let n = read_piece(&mut file, &mut buf)?;
        let started = self.content_call("/2/files/upload_session/start", json!({"close": false}), &buf[..n])?;
        let session = started.get("session_id").and_then(Value::as_str).ok_or("Dropbox didn't start the upload")?.to_string();
        let mut sent = n as u64;
        progress(sent.min(size), size);
        loop {
            let n = read_piece(&mut file, &mut buf)?;
            if n == 0 {
                break;
            }
            self.content_call("/2/files/upload_session/append_v2", json!({"cursor": {"session_id": session, "offset": sent}, "close": false}), &buf[..n])?;
            sent += n as u64;
            progress(sent.min(size), size);
        }
        let done = self.content_call(
            "/2/files/upload_session/finish",
            json!({"cursor": {"session_id": session, "offset": sent},
                   "commit": {"path": format!("/Almanac/{name}"), "mode": "add", "autorename": true, "mute": true}}),
            &[],
        )?;
        let at = done.get("path_display").and_then(Value::as_str).ok_or("Dropbox didn't say where the file went")?.to_string();
        let link = match self.rpc(
            "/2/sharing/create_shared_link_with_settings",
            json!({"path": at, "settings": {"requested_visibility": "public", "audience": "public", "access": "viewer"}}),
        ) {
            Ok(v) => v,
            // (Shared already: that link.)
            Err(e) if e.contains("shared_link_already_exists") => self.rpc("/2/sharing/list_shared_links", json!({"path": at, "direct_only": true}))?
                .pointer("/links/0")
                .cloned()
                .unwrap_or(Value::Null),
            Err(e) => return Err(e),
        };
        let url = link.get("url").and_then(Value::as_str).ok_or("Dropbox gave no link")?.to_string();
        let name = at.rsplit('/').next().unwrap_or(&name).to_string();
        Ok(Shared { name, size, url, provider: self.name().into(), id: at })
    }

    fn delete(&self, id: &str) -> Result<(), String> {
        self.rpc("/2/files/delete_v2", json!({"path": id})).map(|_| ())
    }
}

// ---------------------------------------------------------------------- Box

/// Box (an app registered with Box, signed in once): files go in an "Almanac"
/// folder, shared by an open link.
pub struct BoxCom {
    pub token: String,
    /// api.box.com and upload.box.com (a test server in the tests).
    pub api: String,
    pub upload: String,
}

impl BoxCom {
    pub fn new(token: String) -> BoxCom {
        let test = std::env::var("ALMANAC_BOX_TEST_BASE").ok();
        BoxCom {
            token,
            api: test.clone().unwrap_or_else(|| "https://api.box.com".into()),
            upload: test.unwrap_or_else(|| "https://upload.box.com".into()),
        }
    }

    fn call(&self, method: &str, url: &str, headers: &[(&str, String)], body: Vec<u8>) -> Result<(u16, Value), String> {
        let mut b = ureq::http::Request::builder().method(method).uri(url).header("Authorization", format!("Bearer {}", self.token));
        for (k, v) in headers {
            b = b.header(*k, v);
        }
        let req = b.body(body).map_err(|e| e.to_string())?;
        let mut resp = agent().run(req).map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        let v = json_of(&mut resp);
        if status == 401 {
            return Err("Almanac isn't signed in to Box any more: Connect it again in Settings › Mail".into());
        }
        Ok((status, v))
    }

    fn json_call(&self, method: &str, url: &str, body: Value) -> Result<(u16, Value), String> {
        self.call(method, url, &[("Content-Type", "application/json".into())], body.to_string().into_bytes())
    }

    /// The "Almanac" folder's id (made the first time).
    fn folder(&self) -> Result<String, String> {
        let (st, v) = self.json_call("POST", &format!("{}/2.0/folders", self.api), json!({"name": "Almanac", "parent": {"id": "0"}}))?;
        let id = match st {
            200 | 201 => v.get("id").and_then(Value::as_str).map(str::to_string),
            409 => v.pointer("/context_info/conflicts/0/id").and_then(Value::as_str).map(str::to_string),
            _ => None,
        };
        id.ok_or_else(|| box_error(st, &v))
    }
}

fn box_error(status: u16, v: &Value) -> String {
    let msg = v.get("message").and_then(Value::as_str).unwrap_or("");
    if v.get("code").and_then(Value::as_str) == Some("storage_limit_exceeded") {
        return "Your Box is full".into();
    }
    if msg.is_empty() { format!("Box: HTTP {status}") } else { format!("Box: {msg}") }
}

fn sha1_b64(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, bytes).as_ref())
}

impl CloudStore for BoxCom {
    fn name(&self) -> &'static str {
        "Box"
    }

    fn upload(&self, path: &std::path::Path, progress: &dyn Fn(u64, u64)) -> Result<Shared, String> {
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "file".into());
        let mut file = std::fs::File::open(path).map_err(|e| format!("Could not read {name}: {e}"))?;
        let size = file.metadata().map_err(|e| e.to_string())?.len();
        let folder = self.folder()?;
        progress(0, size);
        let entry: Value = if size < 20 * 1024 * 1024 {
            // (Box takes files in pieces from 20 MB: smaller ones in one go.)
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            let boundary = "almanac-box-boundary";
            let mut body = format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"attributes\"\r\n\r\n{}\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"upload\"\r\nContent-Type: application/octet-stream\r\n\r\n",
                json!({"name": name, "parent": {"id": folder}})
            )
            .into_bytes();
            body.extend_from_slice(&bytes);
            body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
            let (st, v) = self.call("POST", &format!("{}/api/2.0/files/content", self.upload), &[("Content-Type", format!("multipart/form-data; boundary={boundary}"))], body)?;
            if st >= 300 {
                return Err(box_error(st, &v));
            }
            progress(size, size);
            v.pointer("/entries/0").cloned().unwrap_or(Value::Null)
        } else {
            let (st, session) = self.json_call("POST", &format!("{}/api/2.0/files/upload_sessions", self.upload), json!({"folder_id": folder, "file_size": size, "file_name": name}))?;
            if st >= 300 {
                return Err(box_error(st, &session));
            }
            let id = session.get("id").and_then(Value::as_str).ok_or("Box didn't start the upload")?.to_string();
            let part_size = session.get("part_size").and_then(Value::as_u64).unwrap_or(8 * 1024 * 1024) as usize;
            let mut whole = ring::digest::Context::new(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY);
            let mut parts = Vec::new();
            let mut buf = vec![0u8; part_size];
            let mut sent = 0u64;
            loop {
                let n = read_piece(&mut file, &mut buf)?;
                if n == 0 {
                    break;
                }
                whole.update(&buf[..n]);
                let (st, v) = self.call(
                    "PUT",
                    &format!("{}/api/2.0/files/upload_sessions/{id}", self.upload),
                    &[
                        ("Content-Type", "application/octet-stream".into()),
                        ("Content-Range", format!("bytes {}-{}/{}", sent, sent + n as u64 - 1, size)),
                        ("Digest", format!("sha={}", sha1_b64(&buf[..n]))),
                    ],
                    buf[..n].to_vec(),
                )?;
                if st >= 300 {
                    return Err(box_error(st, &v));
                }
                parts.push(v.get("part").cloned().unwrap_or(Value::Null));
                sent += n as u64;
                progress(sent.min(size), size);
            }
            use base64::Engine;
            let digest = base64::engine::general_purpose::STANDARD.encode(whole.finish().as_ref());
            let mut tries = 0;
            loop {
                let (st, v) = self.call(
                    "POST",
                    &format!("{}/api/2.0/files/upload_sessions/{id}/commit", self.upload),
                    &[("Content-Type", "application/json".into()), ("Digest", format!("sha={digest}"))],
                    json!({"parts": parts}).to_string().into_bytes(),
                )?;
                match st {
                    200 | 201 => break v.pointer("/entries/0").cloned().unwrap_or(Value::Null),
                    // (Still putting it together: asked again in a moment.)
                    202 if tries < 30 => {
                        tries += 1;
                        std::thread::sleep(std::time::Duration::from_secs(1));
                    }
                    _ => return Err(box_error(st, &v)),
                }
            }
        };
        let file_id = entry.get("id").and_then(Value::as_str).ok_or("Box didn't say where the file went")?.to_string();
        let (st, v) = self.json_call("PUT", &format!("{}/2.0/files/{file_id}?fields=shared_link", self.api), json!({"shared_link": {"access": "open"}}))?;
        if st >= 300 {
            return Err(box_error(st, &v));
        }
        let url = v.pointer("/shared_link/url").and_then(Value::as_str).ok_or("Box gave no link")?.to_string();
        let name = entry.get("name").and_then(Value::as_str).unwrap_or(&name).to_string();
        Ok(Shared { name, size, url, provider: self.name().into(), id: file_id })
    }

    fn delete(&self, id: &str) -> Result<(), String> {
        let (st, v) = self.call("DELETE", &format!("{}/2.0/files/{id}", self.api), &[], Vec::new())?;
        if st >= 300 && st != 404 { Err(box_error(st, &v)) } else { Ok(()) }
    }
}

// ---------------------------------------------------------------- Nextcloud

/// Nextcloud (any server; signed in through the browser with Nextcloud's own
/// login, which gives Almanac an app password): files go in an "Almanac"
/// folder, shared by a public link.
pub struct Nextcloud {
    /// The server's address, e.g. https://cloud.example.com
    pub server: String,
    pub user: String,
    pub app_password: String,
}

impl Nextcloud {
    fn call(&self, method: &str, url: &str, headers: &[(&str, String)], body: Vec<u8>) -> Result<(u16, String), String> {
        use base64::Engine;
        let auth = base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", self.user, self.app_password));
        let mut b = ureq::http::Request::builder().method(method).uri(url).header("Authorization", format!("Basic {auth}")).header("OCS-APIRequest", "true");
        for (k, v) in headers {
            b = b.header(*k, v);
        }
        let req = b.body(body).map_err(|e| e.to_string())?;
        // (WebDAV's own methods, MKCOL, PROPFIND and MOVE, allowed.)
        let dav: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .allow_non_standard_methods(true)
            .max_redirects(0)
            .timeout_global(Some(std::time::Duration::from_secs(600)))
            .build()
            .into();
        let mut resp = dav.run(req).map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        let text = resp.body_mut().read_to_string().unwrap_or_default();
        if status == 401 {
            return Err("Almanac isn't signed in to Nextcloud any more: Connect it again in Settings › Mail".into());
        }
        Ok((status, text))
    }

    fn dav(&self, rest: &str) -> String {
        format!("{}/remote.php/dav/files/{}/{rest}", self.server.trim_end_matches('/'), path_part(&self.user))
    }
}

impl CloudStore for Nextcloud {
    fn name(&self) -> &'static str {
        "Nextcloud"
    }

    fn upload(&self, path: &std::path::Path, progress: &dyn Fn(u64, u64)) -> Result<Shared, String> {
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "file".into());
        let mut file = std::fs::File::open(path).map_err(|e| format!("Could not read {name}: {e}"))?;
        let size = file.metadata().map_err(|e| e.to_string())?.len();
        // The folder (there already: fine).
        let (st, _) = self.call("MKCOL", &self.dav("Almanac"), &[], Vec::new())?;
        if st >= 400 && st != 405 {
            return Err(format!("Nextcloud: couldn't make the Almanac folder (HTTP {st})"));
        }
        // A name not taken yet.
        let mut target = name.clone();
        for n in 2..50 {
            let (st, _) = self.call("PROPFIND", &self.dav(&format!("Almanac/{}", path_part(&target))), &[("Depth", "0".into())], Vec::new())?;
            if st == 404 {
                break;
            }
            let (stem, ext) = match name.rsplit_once('.') {
                Some((a, b)) => (a.to_string(), format!(".{b}")),
                None => (name.clone(), String::new()),
            };
            target = format!("{stem} ({n}){ext}");
        }
        // Up in pieces (Nextcloud's chunked upload), then put together.
        let base = format!("{}/remote.php/dav/uploads/{}", self.server.trim_end_matches('/'), path_part(&self.user));
        let upload = format!("{base}/almanac-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0));
        let dest = self.dav(&format!("Almanac/{}", path_part(&target)));
        let (st, _) = self.call("MKCOL", &upload, &[("Destination", dest.clone())], Vec::new())?;
        if st >= 400 {
            return Err(format!("Nextcloud: couldn't start the upload (HTTP {st})"));
        }
        let mut buf = vec![0u8; CHUNK];
        let mut sent = 0u64;
        let mut part = 1;
        progress(0, size);
        loop {
            let n = read_piece(&mut file, &mut buf)?;
            if n == 0 && part > 1 {
                break;
            }
            let (st, _) = self.call("PUT", &format!("{upload}/{part:05}"), &[("Destination", dest.clone())], buf[..n].to_vec())?;
            if st >= 400 {
                return Err(format!("Nextcloud: a piece of the file didn't go (HTTP {st})"));
            }
            sent += n as u64;
            part += 1;
            progress(sent.min(size), size);
            if n == 0 {
                break;
            }
        }
        let (st, _) = self.call("MOVE", &format!("{upload}/.file"), &[("Destination", dest.clone()), ("OC-Total-Length", size.to_string())], Vec::new())?;
        if st >= 400 {
            return Err(format!("Nextcloud: the file couldn't be put together (HTTP {st})"));
        }
        let form = format!("path={}&shareType=3&permissions=1", enc_form(&format!("/Almanac/{target}")));
        let (st, text) = self.call(
            "POST",
            &format!("{}/ocs/v2.php/apps/files_sharing/api/v1/shares?format=json", self.server.trim_end_matches('/')),
            &[("Content-Type", "application/x-www-form-urlencoded".into())],
            form.into_bytes(),
        )?;
        let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        if st >= 400 {
            let msg = v.pointer("/ocs/meta/message").and_then(Value::as_str).unwrap_or("");
            return Err(format!("Nextcloud: couldn't share it by link{}", if msg.is_empty() { String::new() } else { format!(" ({msg})") }));
        }
        let url = v.pointer("/ocs/data/url").and_then(Value::as_str).ok_or("Nextcloud gave no link")?.to_string();
        Ok(Shared { name: target.clone(), size, url, provider: self.name().into(), id: format!("Almanac/{target}") })
    }

    fn delete(&self, id: &str) -> Result<(), String> {
        let (st, _) = self.call("DELETE", &self.dav(&id.split('/').map(path_part).collect::<Vec<_>>().join("/")), &[], Vec::new())?;
        if st >= 400 && st != 404 { Err(format!("Nextcloud: HTTP {st}")) } else { Ok(()) }
    }
}

/// Signs in to a Nextcloud server with its own login page (Login Flow v2):
/// the page opens in the browser, and once the person has allowed Almanac
/// there, the server hands over an app password (polled for up to 10
/// minutes). Returns (server, user, app password).
pub fn nextcloud_login(server: &str, open_url: &dyn Fn(String)) -> Result<(String, String, String), String> {
    let server = server.trim().trim_end_matches('/');
    let server = if server.starts_with("http://") || server.starts_with("https://") { server.to_string() } else { format!("https://{server}") };
    let req = ureq::http::Request::builder()
        .method("POST")
        .uri(format!("{server}/index.php/login/v2"))
        .header("User-Agent", "Almanac")
        .body(String::new())
        .map_err(|e| e.to_string())?;
    let mut resp = agent().run(req).map_err(|e| format!("Couldn't reach {server}: {e}"))?;
    let status = resp.status().as_u16();
    let v = json_of(&mut resp);
    if status >= 400 {
        return Err(format!("{server} doesn't look like a Nextcloud server (HTTP {status})"));
    }
    let login = v.get("login").and_then(Value::as_str).ok_or("The server didn't give a sign-in page")?.to_string();
    let token = v.pointer("/poll/token").and_then(Value::as_str).ok_or("The server didn't give a sign-in token")?.to_string();
    let endpoint = v.pointer("/poll/endpoint").and_then(Value::as_str).ok_or("The server didn't give a sign-in token")?.to_string();
    open_url(login);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(600);
    while std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_secs(2));
        let req = ureq::http::Request::builder()
            .method("POST")
            .uri(&endpoint)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(format!("token={}", enc_form(&token)))
            .map_err(|e| e.to_string())?;
        let Ok(mut resp) = agent().run(req) else { continue };
        if resp.status().as_u16() != 200 {
            continue;
        }
        let v = json_of(&mut resp);
        let got = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        if let (Some(srv), Some(user), Some(pass)) = (got("server"), got("loginName"), got("appPassword")) {
            return Ok((srv, user, pass));
        }
    }
    Err("Nextcloud sign-in timed out".into())
}

fn enc_form(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~/".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
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

    /// A stand-in server answering with `reply(method, path, headers, body)`.
    fn serve(reply: impl Fn(&str, &str, &[(String, String)], &[u8]) -> (u16, String) + Send + 'static) -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let log = seen.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut r = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                if r.read_line(&mut line).is_err() || line.is_empty() {
                    continue;
                }
                let mut headers = Vec::new();
                let mut len = 0usize;
                loop {
                    let mut h = String::new();
                    r.read_line(&mut h).unwrap();
                    if h.trim().is_empty() {
                        break;
                    }
                    if let Some((k, v)) = h.split_once(':') {
                        let (k, v) = (k.trim().to_ascii_lowercase(), v.trim().to_string());
                        if k == "content-length" {
                            len = v.parse().unwrap_or(0);
                        }
                        headers.push((k, v));
                    }
                }
                let mut body = vec![0u8; len];
                r.read_exact(&mut body).unwrap();
                let mut parts = line.split_whitespace();
                let (method, path) = (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").to_string());
                log.lock().unwrap().push(format!("{method} {path} {}", body.len()));
                let (code, text) = reply(&method, &path, &headers, &body);
                let mut out = stream;
                let _ = out.write_all(format!("HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{text}", text.len()).as_bytes());
            }
        });
        (base, seen)
    }

    #[test]
    fn dropbox_uploads_in_pieces_and_shares() {
        let (base, seen) = serve(|_, path, headers, _| {
            let arg = headers.iter().find(|(k, _)| k == "dropbox-api-arg").map(|(_, v)| v.clone()).unwrap_or_default();
            match path {
                "/2/files/upload_session/start" => (200, r#"{"session_id":"S1"}"#.into()),
                "/2/files/upload_session/append_v2" => {
                    assert!(arg.contains("\"session_id\":\"S1\""), "{arg}");
                    (200, "null".into())
                }
                "/2/files/upload_session/finish" => {
                    assert!(arg.contains("/Almanac/big video.mp4"), "{arg}");
                    (200, r#"{"path_display":"/Almanac/big video.mp4"}"#.into())
                }
                "/2/sharing/create_shared_link_with_settings" => (200, r#"{"url":"https://www.dropbox.com/scl/fi/x/big%20video.mp4?dl=0"}"#.into()),
                _ => (404, "{}".into()),
            }
        });
        let dir = std::env::temp_dir().join(format!("almanac-dropbox-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("big video.mp4");
        std::fs::write(&path, vec![7u8; DROPBOX_CHUNK * 2 + 10]).unwrap();
        let d = Dropbox { token: "t".into(), api: base.clone(), content: base };
        let shared = d.upload(&path, &|_, _| {}).expect("uploaded");
        assert_eq!((shared.provider.as_str(), shared.id.as_str()), ("Dropbox", "/Almanac/big video.mp4"));
        assert!(shared.url.starts_with("https://www.dropbox.com/"));
        let log = seen.lock().unwrap().clone();
        assert_eq!(log.iter().filter(|l| l.starts_with("POST /2/files/upload_session/append_v2")).count(), 2, "{log:?}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn nextcloud_uploads_in_pieces_and_shares() {
        let (base, seen) = serve(|method, path, headers, _| {
            let authed = headers.iter().any(|(k, v)| k == "authorization" && v.starts_with("Basic "));
            assert!(authed, "{method} {path}");
            match (method, path) {
                ("MKCOL", p) if p.ends_with("/Almanac") => (201, String::new()),
                ("PROPFIND", _) => (404, String::new()),
                ("MKCOL", p) if p.contains("/uploads/") => (201, String::new()),
                ("PUT", p) if p.contains("/uploads/") => (201, String::new()),
                ("MOVE", p) if p.ends_with("/.file") => {
                    let dest = headers.iter().find(|(k, _)| k == "destination").map(|(_, v)| v.clone()).unwrap_or_default();
                    assert!(dest.ends_with("/remote.php/dav/files/ben/Almanac/big%20video.mp4"), "{dest}");
                    (201, String::new())
                }
                ("POST", p) if p.contains("/files_sharing/api/v1/shares") => (200, r#"{"ocs":{"meta":{"status":"ok"},"data":{"url":"https://cloud.example.com/s/AbC"}}}"#.into()),
                _ => (404, String::new()),
            }
        });
        let dir = std::env::temp_dir().join(format!("almanac-nextcloud-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("big video.mp4");
        std::fs::write(&path, vec![7u8; CHUNK + 10]).unwrap();
        let n = Nextcloud { server: base, user: "ben".into(), app_password: "app-pass".into() };
        let shared = n.upload(&path, &|_, _| {}).expect("uploaded");
        assert_eq!((shared.url.as_str(), shared.provider.as_str()), ("https://cloud.example.com/s/AbC", "Nextcloud"));
        let log = seen.lock().unwrap().clone();
        assert_eq!(log.iter().filter(|l| l.starts_with("PUT ") && l.contains("/uploads/")).count(), 2, "{log:?}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn box_uploads_in_parts_and_shares() {
        let (base, seen) = serve(|method, path, headers, body| match (method, path) {
            ("POST", "/2.0/folders") => (409, r#"{"code":"item_name_in_use","context_info":{"conflicts":[{"id":"F9"}]}}"#.into()),
            ("POST", "/api/2.0/files/upload_sessions") => (201, r#"{"id":"U1","part_size":8388608}"#.into()),
            ("PUT", "/api/2.0/files/upload_sessions/U1") => {
                let digest = headers.iter().find(|(k, _)| k == "digest").map(|(_, v)| v.clone()).unwrap_or_default();
                assert_eq!(digest, format!("sha={}", sha1_b64(body)));
                (200, format!(r#"{{"part":{{"part_id":"P{}","size":{}}}}}"#, body.len(), body.len()))
            }
            ("POST", "/api/2.0/files/upload_sessions/U1/commit") => (201, r#"{"entries":[{"id":"B1","name":"big video.mp4"}]}"#.into()),
            ("PUT", p) if p.starts_with("/2.0/files/B1") => (200, r#"{"shared_link":{"url":"https://app.box.com/s/xyz"}}"#.into()),
            _ => (404, "{}".into()),
        });
        let dir = std::env::temp_dir().join(format!("almanac-box-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("big video.mp4");
        std::fs::write(&path, vec![7u8; 8 * 1024 * 1024 * 3 + 5]).unwrap();
        let b = BoxCom { token: "t".into(), api: base.clone(), upload: base };
        let shared = b.upload(&path, &|_, _| {}).expect("uploaded");
        assert_eq!((shared.url.as_str(), shared.id.as_str(), shared.provider.as_str()), ("https://app.box.com/s/xyz", "B1", "Box"));
        let log = seen.lock().unwrap().clone();
        assert_eq!(log.iter().filter(|l| l.starts_with("PUT /api/2.0/files/upload_sessions/U1")).count(), 4, "{log:?}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn nextcloud_login_hands_over_an_app_password() {
        let polls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let me = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let (p, m) = (polls.clone(), me.clone());
        let (base, _) = serve(move |method, path, _, body| match (method, path) {
            ("POST", "/index.php/login/v2") => {
                let endpoint = format!("{}/login/v2/poll", m.lock().unwrap());
                (200, format!(r#"{{"poll":{{"token":"T1","endpoint":"{endpoint}"}},"login":"https://cloud.example.com/login/v2/flow/abc"}}"#))
            }
            ("POST", "/login/v2/poll") => {
                assert_eq!(std::str::from_utf8(body).unwrap(), "token=T1");
                // (Not allowed yet the first time.)
                if p.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                    (404, "[]".into())
                } else {
                    (200, r#"{"server":"https://cloud.example.com","loginName":"ben","appPassword":"xyz"}"#.into())
                }
            }
            _ => (404, "{}".into()),
        });
        *me.lock().unwrap() = base.clone();
        let opened = std::sync::Mutex::new(String::new());
        let got = nextcloud_login(&base, &|u| *opened.lock().unwrap() = u).expect("signed in");
        assert_eq!(got, ("https://cloud.example.com".to_string(), "ben".to_string(), "xyz".to_string()));
        assert_eq!(*opened.lock().unwrap(), "https://cloud.example.com/login/v2/flow/abc");
        assert_eq!(polls.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[test]
    fn sizes_once_in_a_message() {
        assert!(as_sent(18 * 1024 * 1024) > mail_limit("google") * 9 / 10, "18 MB is close to Gmail's 25 MB once encoded");
        assert!(as_sent(20 * 1024 * 1024) > mail_limit("google"), "20 MB doesn't fit");
        assert!(as_sent(1024 * 1024) < mail_limit("google"));
    }
}
