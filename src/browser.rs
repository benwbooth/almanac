//! Browser-driven setup: Almanac starts Chrome, Brave, Chromium or Edge in a
//! window of its own (a separate profile, with the DevTools remote-debugging
//! port on) and clicks through the Google Cloud pages itself. The person only
//! signs in, ticks Google's terms boxes and approves access; whenever a click
//! can't be done automatically, a banner on the page says exactly what to do
//! and Almanac waits until it's done. Every step first checks whether it's
//! already done, so the whole thing can be run again safely.

use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

static CANCEL: AtomicBool = AtomicBool::new(false);

/// Stops a running browser setup at its next check.
pub fn cancel() {
    CANCEL.store(true, Ordering::SeqCst);
}

const STOPPED: &str = "Browser setup stopped.";

// ------------------------------------------------------------ finding it

/// A Chrome-family browser on this computer.
#[derive(Clone)]
pub struct Installed {
    pub name: String,
    pub exe: PathBuf,
    /// Its normal profile folder (where it notes the remote-control port).
    pub user_data: Option<PathBuf>,
}

impl Installed {
    /// Whether it's open right now.
    pub fn running(&self) -> bool {
        let Some(d) = &self.user_data else { return false };
        // Linux and macOS leave a lock link while it runs; Windows a lockfile.
        std::fs::symlink_metadata(d.join("SingletonLock")).is_ok() || d.join("lockfile").exists() || port_file(d).is_some()
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_default()
}

/// Every Chrome-family browser found, open ones first.
pub fn installed() -> Vec<Installed> {
    // (name, program names, profile folder per system)
    let known: &[(&str, &[&str], &str, &str, &str)] = &[
        ("Google Chrome", &["google-chrome-stable", "google-chrome", "chrome"], ".config/google-chrome", "Google/Chrome", "Google/Chrome/User Data"),
        ("Brave", &["brave-browser", "brave"], ".config/BraveSoftware/Brave-Browser", "BraveSoftware/Brave-Browser", "BraveSoftware/Brave-Browser/User Data"),
        ("Chromium", &["chromium", "chromium-browser"], ".config/chromium", "Chromium", "Chromium/User Data"),
        ("Microsoft Edge", &["microsoft-edge", "microsoft-edge-stable", "msedge"], ".config/microsoft-edge", "Microsoft Edge", "Microsoft/Edge/User Data"),
        ("Vivaldi", &["vivaldi"], ".config/vivaldi", "Vivaldi", "Vivaldi/User Data"),
    ];
    let mut out = Vec::new();
    for (name, exes, linux_dir, mac_dir, win_dir) in known {
        let user_data = if cfg!(windows) {
            std::env::var_os("LOCALAPPDATA").map(|b| PathBuf::from(b).join(win_dir))
        } else if cfg!(target_os = "macos") {
            Some(home().join("Library/Application Support").join(mac_dir))
        } else {
            Some(home().join(linux_dir))
        };
        let mut exe = exes.iter().find_map(|e| crate::autoreg::find_tool(e));
        if exe.is_none() && cfg!(target_os = "macos") {
            let app = match *name {
                "Brave" => "Brave Browser",
                n => n,
            };
            for base in [PathBuf::from("/Applications"), home().join("Applications")] {
                let p = base.join(format!("{app}.app/Contents/MacOS/{app}"));
                if p.is_file() {
                    exe = Some(p);
                    break;
                }
            }
        }
        if exe.is_none() && cfg!(windows) {
            let file = match *name {
                "Brave" => "BraveSoftware/Brave-Browser/Application/brave.exe",
                "Microsoft Edge" => "Microsoft/Edge/Application/msedge.exe",
                "Google Chrome" => "Google/Chrome/Application/chrome.exe",
                _ => "",
            };
            if !file.is_empty() {
                for var in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
                    if let Some(p) = std::env::var_os(var).map(|b| PathBuf::from(b).join(file)).filter(|p| p.is_file()) {
                        exe = Some(p);
                        break;
                    }
                }
            }
        }
        if let Some(exe) = exe {
            out.push(Installed { name: name.to_string(), exe, user_data: user_data.filter(|d| d.is_dir()) });
        }
    }
    out.sort_by_key(|b| !b.running());
    out
}

/// The browser Almanac would use: one that's open, else the first found.
pub fn find_browser() -> Option<(String, PathBuf)> {
    installed().into_iter().next().map(|b| (b.name, b.exe))
}

fn profile_dir() -> PathBuf {
    crate::data::data_dir().join("setup-browser")
}

fn downloads_dir() -> PathBuf {
    crate::data::data_dir().join("setup-browser-downloads")
}

/// The remote-control address a running browser has written down
/// (port, path), if it answers.
fn port_file(dir: &Path) -> Option<(u16, String)> {
    let text = std::fs::read_to_string(dir.join("DevToolsActivePort")).ok()?;
    let mut lines = text.lines();
    let port = lines.next()?.trim().parse::<u16>().ok()?;
    let path = lines.next().unwrap_or("").trim().to_string();
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&addr, Duration::from_millis(400)).ok()?;
    Some((port, path))
}

fn browser_ws_url(port: u16, path: &str) -> Result<String, String> {
    if path.starts_with("/devtools/browser/") {
        return Ok(format!("ws://127.0.0.1:{port}{path}"));
    }
    // Older layout: ask the browser for its address.
    let agent = ureq::Agent::new_with_config(ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(5))).build());
    let mut resp = agent.get(&format!("http://127.0.0.1:{port}/json/version")).call().map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&resp.body_mut().read_to_string().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    v["webSocketDebuggerUrl"].as_str().map(String::from).ok_or_else(|| "The browser didn't give its remote-control address.".into())
}

/// Starts the browser in a window of its own with a separate profile (or
/// reuses that window if it's still open). Returns (port, path).
pub fn launch(exe: &Path, profile: &Path, url: &str, headless: bool) -> Result<(u16, String), String> {
    std::fs::create_dir_all(profile).map_err(|e| e.to_string())?;
    if let Some(p) = port_file(profile) {
        return Ok(p);
    }
    let _ = std::fs::remove_file(profile.join("DevToolsActivePort"));
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("--remote-debugging-port=0")
        .arg(format!("--user-data-dir={}", profile.display()))
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--new-window");
    if headless {
        cmd.arg("--headless=new").arg("--disable-gpu");
    }
    cmd.arg(url).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    cmd.spawn().map_err(|e| format!("Couldn't start {}: {e}", exe.display()))?;
    let until = Instant::now() + Duration::from_secs(30);
    while Instant::now() < until {
        if let Some(p) = port_file(profile) {
            return Ok(p);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Err("The browser started but didn't open its remote-control port. Close all of its windows and try again.".into())
}

/// Connects to the person's own, already-open browser. If remote control
/// isn't on yet, opens its switch page and waits for the person to turn it
/// on (and to press Allow).
fn attach_own(b: &Installed, progress: &dyn Fn(String)) -> Result<Arc<Page>, String> {
    let dir = b.user_data.clone().ok_or_else(|| format!("Couldn't find {}'s profile folder.", b.name))?;
    if port_file(&dir).is_none() {
        // Opens a tab in the running browser (or starts it normally).
        let _ = std::process::Command::new(&b.exe)
            .arg("chrome://inspect/#remote-debugging")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
        progress(format!(
            "Your turn, in {}: on the page that just opened, tick “Allow remote debugging for this browser instance”. Almanac continues by itself.",
            b.name
        ));
        let until = Instant::now() + Duration::from_secs(10 * 60);
        while port_file(&dir).is_none() {
            check_cancel()?;
            if Instant::now() > until {
                return Err(format!("Remote control wasn't turned on in {}. Try again, or use a separate window instead.", b.name));
            }
            std::thread::sleep(Duration::from_millis(700));
        }
    }
    let (port, path) = port_file(&dir).ok_or("The browser closed.")?;
    progress(format!("Connecting to {}. If it asks “Allow remote debugging?”, press Allow.", b.name));
    Page::open_tab(&browser_ws_url(port, &path)?)
}

// ------------------------------------------------- a minimal WebSocket

struct Ws {
    s: TcpStream,
}

fn random_bytes<const N: usize>() -> [u8; N] {
    use ring::rand::SecureRandom;
    let mut b = [0u8; N];
    let _ = ring::rand::SystemRandom::new().fill(&mut b);
    b
}

impl Ws {
    fn connect(url: &str) -> Result<Ws, String> {
        use base64::Engine;
        let rest = url.strip_prefix("ws://").ok_or("not a ws:// address")?;
        let (host, path) = rest.split_once('/').map(|(h, p)| (h.to_string(), format!("/{p}"))).unwrap_or((rest.to_string(), "/".into()));
        let mut s = TcpStream::connect(&host).map_err(|e| e.to_string())?;
        // The browser may first ask the person to allow remote control.
        s.set_read_timeout(Some(Duration::from_secs(300))).ok();
        let key = base64::engine::general_purpose::STANDARD.encode(random_bytes::<16>());
        write!(s, "GET {path} HTTP/1.1\r\nHost: {host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n")
            .map_err(|e| e.to_string())?;
        // Read the reply headers byte by byte so no frame data is swallowed.
        let mut head = Vec::new();
        let mut b = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            s.read_exact(&mut b).map_err(|e| e.to_string())?;
            head.push(b[0]);
            if head.len() > 16384 {
                return Err("bad WebSocket reply".into());
            }
        }
        let head = String::from_utf8_lossy(&head);
        if !head.starts_with("HTTP/1.1 101") {
            return Err(format!("The browser refused remote control: {}", head.lines().next().unwrap_or("")));
        }
        s.set_read_timeout(Some(Duration::from_secs(90))).ok();
        Ok(Ws { s })
    }

    fn send_frame(&mut self, opcode: u8, data: &[u8]) -> Result<(), String> {
        let mut f = vec![0x80 | opcode];
        let n = data.len();
        if n < 126 {
            f.push(0x80 | n as u8);
        } else if n < 65536 {
            f.push(0x80 | 126);
            f.extend_from_slice(&(n as u16).to_be_bytes());
        } else {
            f.push(0x80 | 127);
            f.extend_from_slice(&(n as u64).to_be_bytes());
        }
        let mask = random_bytes::<4>();
        f.extend_from_slice(&mask);
        f.extend(data.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
        self.s.write_all(&f).map_err(|e| e.to_string())
    }

    fn send(&mut self, text: &str) -> Result<(), String> {
        self.send_frame(1, text.as_bytes())
    }

    fn recv(&mut self) -> Result<String, String> {
        let mut msg = Vec::new();
        loop {
            let mut h = [0u8; 2];
            self.s.read_exact(&mut h).map_err(|e| e.to_string())?;
            let fin = h[0] & 0x80 != 0;
            let opcode = h[0] & 0x0f;
            let masked = h[1] & 0x80 != 0;
            let mut len = (h[1] & 0x7f) as u64;
            if len == 126 {
                let mut l = [0u8; 2];
                self.s.read_exact(&mut l).map_err(|e| e.to_string())?;
                len = u16::from_be_bytes(l) as u64;
            } else if len == 127 {
                let mut l = [0u8; 8];
                self.s.read_exact(&mut l).map_err(|e| e.to_string())?;
                len = u64::from_be_bytes(l);
            }
            let mut mask = [0u8; 4];
            if masked {
                self.s.read_exact(&mut mask).map_err(|e| e.to_string())?;
            }
            let mut data = vec![0u8; len as usize];
            self.s.read_exact(&mut data).map_err(|e| e.to_string())?;
            if masked {
                for (i, b) in data.iter_mut().enumerate() {
                    *b ^= mask[i % 4];
                }
            }
            match opcode {
                8 => return Err("The browser window was closed.".into()),
                9 => self.send_frame(10, &data)?,
                10 => {}
                _ => {
                    msg.extend_from_slice(&data);
                    if fin {
                        return Ok(String::from_utf8_lossy(&msg).into_owned());
                    }
                }
            }
        }
    }
}

// ------------------------------------------------------ DevTools page

/// Page helpers injected before every script: find things by their visible
/// text or label, click them, show Almanac's banner.
const HELPERS: &str = r#"window.__alm = window.__alm || (function () {
  const A = {};
  A.vis = e => { if (!e || !e.getBoundingClientRect) return false; const r = e.getBoundingClientRect(); const s = getComputedStyle(e); return r.width > 0 && r.height > 0 && s.visibility !== 'hidden' && s.display !== 'none'; };
  A.all = sel => { const out = []; const walk = root => { root.querySelectorAll(sel).forEach(e => out.push(e)); root.querySelectorAll('*').forEach(e => { if (e.shadowRoot) walk(e.shadowRoot); }); }; walk(document); return out; };
  A.label = e => ((e.innerText || e.value || '') + ' ' + (e.getAttribute('aria-label') || '')).replace(/\s+/g, ' ').trim();
  A.clickables = 'button,a,[role=button],[role=option],[role=radio],[role=tab],[role=menuitem],[role=menuitemradio],[role=checkbox],[role=combobox],[role=listbox],mat-option,mat-radio-button,mat-select,cfc-select,label,li,input[type=submit]';
  A.find = (re, sel) => { const rx = new RegExp(re, 'i'); let best = null; for (const e of A.all(sel || A.clickables)) { if (!A.vis(e)) continue; const t = A.label(e); if (!t || t.length > 140 || !rx.test(t)) continue; if (!best || t.length < best.t.length) best = { e: e, t: t }; } return best ? best.e : null; };
  A.field = re => { const rx = new RegExp(re, 'i'); for (const e of A.all('input,textarea')) { if (!A.vis(e) || ['hidden', 'checkbox', 'radio', 'submit', 'button'].indexOf(e.type) >= 0) continue; const ff = e.closest('mat-form-field,.mat-mdc-form-field,cfc-form-field,label,[role=dialog]'); const forl = e.id ? document.querySelector('label[for="' + e.id + '"]') : null; const lab = [e.getAttribute('aria-label'), e.placeholder, e.name, forl ? forl.innerText : '', ff ? ff.innerText : ''].join(' '); if (rx.test(lab)) return e; } return null; };
  A.center = e => { e.scrollIntoView({ block: 'center', inline: 'center' }); const r = e.getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2, disabled: !!(e.disabled || e.getAttribute('aria-disabled') === 'true') }; };
  A.mark = e => { A.all('[data-almanac-mark]').forEach(x => { x.style.outline = ''; x.removeAttribute('data-almanac-mark'); }); if (e) { e.setAttribute('data-almanac-mark', '1'); e.style.outline = '4px solid #ff9500'; e.style.outlineOffset = '3px'; } };
  A.text = () => document.body ? document.body.innerText : '';
  A.banner = (msg, warn) => { let b = document.getElementById('__almanac_banner'); if (!b) { b = document.createElement('div'); b.id = '__almanac_banner'; (document.body || document.documentElement).appendChild(b); } b.style.cssText = 'position:fixed;left:50%;transform:translateX(-50%);bottom:24px;z-index:2147483647;width:min(760px,90vw);box-sizing:border-box;padding:14px 20px;border-radius:12px;font:600 16px/1.45 system-ui,sans-serif;color:#fff;box-shadow:0 8px 30px rgba(0,0,0,.35);pointer-events:none;white-space:pre-line;background:' + (warn ? '#b35400' : '#0a66d8'); b.textContent = 'Almanac: ' + msg; };
  A.email = () => { for (const e of A.all('[aria-label]')) { const m = (e.getAttribute('aria-label') || '').match(/Google Account:[^(]*\(([^()\s]+@[^()\s]+)\)/); if (m) return m[1]; } return ''; };
  return A;
})()"#;

pub struct Page {
    ws: Mutex<Ws>,
    next: AtomicU64,
    /// The tab Almanac opened (commands go to it; the rest of the browser is
    /// left alone).
    session: String,
}

impl Page {
    /// Opens a new tab in the browser at `browser_ws` and attaches to it.
    pub fn open_tab(browser_ws: &str) -> Result<Arc<Page>, String> {
        let page = Page { ws: Mutex::new(Ws::connect(browser_ws)?), next: AtomicU64::new(1), session: String::new() };
        let t = page.call("Target.createTarget", json!({"url": "about:blank"}))?;
        let target = t["targetId"].as_str().ok_or("The browser didn't open a tab.")?.to_string();
        let a = page.call("Target.attachToTarget", json!({"targetId": target, "flatten": true}))?;
        let session = a["sessionId"].as_str().ok_or("Couldn't attach to the new tab.")?.to_string();
        let page = Arc::new(Page { session, ..page });
        let _ = page.call("Page.enable", json!({}));
        let _ = page.call("Page.bringToFront", json!({}));
        Ok(page)
    }

    pub fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        let mut msg = json!({"id": id, "method": method, "params": params});
        if !self.session.is_empty() {
            msg["sessionId"] = Value::from(self.session.as_str());
        }
        let mut ws = self.ws.lock().unwrap();
        ws.send(&msg.to_string())?;
        loop {
            let v: Value = serde_json::from_str(&ws.recv()?).unwrap_or(Value::Null);
            if v["id"].as_u64() == Some(id) {
                if let Some(e) = v.get("error") {
                    return Err(e["message"].as_str().unwrap_or("DevTools error").to_string());
                }
                return Ok(v["result"].clone());
            }
            if v["method"] == "Target.detachedFromTarget" && v["params"]["sessionId"].as_str() == Some(self.session.as_str()) {
                return Err("Almanac's browser tab was closed.".into());
            }
        }
    }

    /// Runs a script in the page (with the helpers) and returns its value.
    pub fn eval(&self, expr: &str) -> Result<Value, String> {
        let r = self.call(
            "Runtime.evaluate",
            json!({"expression": format!("{HELPERS};\n({expr})"), "returnByValue": true, "awaitPromise": true}),
        )?;
        if let Some(d) = r.get("exceptionDetails") {
            return Err(d["exception"]["description"].as_str().or(d["text"].as_str()).unwrap_or("script error").to_string());
        }
        Ok(r["result"]["value"].clone())
    }

    fn eval_str(&self, expr: &str) -> String {
        self.eval(expr).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default()
    }

    pub fn url(&self) -> String {
        self.eval_str("location.href")
    }

    pub fn text(&self) -> String {
        self.eval_str("__alm.text()")
    }

    pub fn navigate(&self, url: &str) -> Result<(), String> {
        self.call("Page.navigate", json!({"url": url}))?;
        // Wait for the page and for the app inside it to draw something.
        let until = Instant::now() + Duration::from_secs(40);
        std::thread::sleep(Duration::from_millis(600));
        while Instant::now() < until {
            check_cancel()?;
            let ready = self.eval("document.readyState === 'complete' && document.body && document.body.innerText.length > 80").ok() == Some(Value::Bool(true));
            if ready {
                break;
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        std::thread::sleep(Duration::from_millis(1500));
        Ok(())
    }

    pub fn banner(&self, msg: &str, warn: bool) {
        let _ = self.eval(&format!("__alm.banner({}, {warn})", Value::from(msg)));
    }

    fn mouse_click(&self, x: f64, y: f64) -> Result<(), String> {
        self.call("Input.dispatchMouseEvent", json!({"type": "mouseMoved", "x": x, "y": y}))?;
        self.call("Input.dispatchMouseEvent", json!({"type": "mousePressed", "x": x, "y": y, "button": "left", "clickCount": 1}))?;
        self.call("Input.dispatchMouseEvent", json!({"type": "mouseReleased", "x": x, "y": y, "button": "left", "clickCount": 1}))?;
        Ok(())
    }

    /// Clicks the visible control whose text or label matches `re`
    /// (a JavaScript regular expression, case-insensitive).
    pub fn click(&self, re: &str) -> bool {
        self.click_in(re, "")
    }

    pub fn click_in(&self, re: &str, selector: &str) -> bool {
        let sel = if selector.is_empty() { Value::Null } else { Value::from(selector) };
        let js = format!(
            "(() => {{ const e = __alm.find({}, {}); if (!e) return null; __alm.mark(e); return __alm.center(e); }})()",
            Value::from(re),
            if sel.is_null() { "undefined".to_string() } else { sel.to_string() }
        );
        match self.eval(&js) {
            Ok(v) if v.is_object() && v["disabled"] != Value::Bool(true) => {
                std::thread::sleep(Duration::from_millis(250));
                // Coordinates again: scrolling may have moved it.
                let v2 = self.eval(&js).unwrap_or(v);
                self.mouse_click(v2["x"].as_f64().unwrap_or(0.0), v2["y"].as_f64().unwrap_or(0.0)).is_ok()
            }
            _ => false,
        }
    }

    pub fn exists(&self, re: &str) -> bool {
        self.eval(&format!("!!__alm.find({})", Value::from(re))).ok() == Some(Value::Bool(true))
    }

    /// Types into the text box whose label matches `re` (replacing what's there).
    pub fn fill(&self, re: &str, text: &str) -> bool {
        let js = format!(
            "(() => {{ const e = __alm.field({}); if (!e) return false; __alm.mark(e); e.scrollIntoView({{block: 'center'}}); e.focus(); if (e.select) e.select(); return true; }})()",
            Value::from(re)
        );
        if self.eval(&js).ok() != Some(Value::Bool(true)) {
            return false;
        }
        let _ = self.call("Input.dispatchKeyEvent", json!({"type": "keyDown", "key": "a", "code": "KeyA", "modifiers": if cfg!(target_os = "macos") { 4 } else { 2 }}));
        let _ = self.call("Input.dispatchKeyEvent", json!({"type": "keyUp", "key": "a", "code": "KeyA", "modifiers": if cfg!(target_os = "macos") { 4 } else { 2 }}));
        self.call("Input.insertText", json!({"text": text})).is_ok()
    }

    /// Ticks every visible checkbox on the page that isn't ticked yet.
    pub fn tick_all(&self) {
        let boxes = self
            .eval("(() => __alm.all('input[type=checkbox],[role=checkbox]').filter(__alm.vis).filter(x => !(x.checked || x.getAttribute('aria-checked') === 'true')).map(x => __alm.center(x)))()")
            .unwrap_or(Value::Null);
        for b in boxes.as_array().cloned().unwrap_or_default() {
            let _ = self.mouse_click(b["x"].as_f64().unwrap_or(0.0), b["y"].as_f64().unwrap_or(0.0));
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    pub fn press_enter(&self) {
        for t in ["keyDown", "keyUp"] {
            let _ = self.call(
                "Input.dispatchKeyEvent",
                json!({"type": t, "key": "Enter", "code": "Enter", "windowsVirtualKeyCode": 13, "nativeVirtualKeyCode": 13, "text": if t == "keyDown" { "\r" } else { "" }}),
            );
        }
    }

    /// Waits up to `secs` for `f` to become true.
    pub fn wait(&self, secs: u64, f: &dyn Fn(&Page) -> bool) -> Result<bool, String> {
        let until = Instant::now() + Duration::from_secs(secs);
        loop {
            check_cancel()?;
            if f(self) {
                return Ok(true);
            }
            if Instant::now() > until {
                return Ok(false);
            }
            std::thread::sleep(Duration::from_millis(400));
        }
    }

    /// Clicks once the control shows up (waiting up to `secs`).
    pub fn click_when(&self, re: &str, secs: u64) -> Result<bool, String> {
        self.wait(secs, &|p| p.click(re))
    }
}

fn check_cancel() -> Result<(), String> {
    if CANCEL.load(Ordering::SeqCst) {
        Err(STOPPED.into())
    } else {
        Ok(())
    }
}

/// One setup step. If `done` is already true nothing happens. Otherwise
/// `act` tries to do it (once: some steps must not be repeated, such as
/// creating something); if that doesn't finish it, the page banner tells the
/// person what to do and Almanac waits for `done`.
fn step(page: &Page, progress: &dyn Fn(String), title: &str, manual: &str, done: &dyn Fn(&Page) -> bool, act: &dyn Fn(&Page) -> Result<(), String>) -> Result<(), String> {
    check_cancel()?;
    if done(page) {
        progress(format!("✓ {title} (already done)"));
        return Ok(());
    }
    progress(format!("{title}…"));
    page.banner(&format!("{title}… (Almanac is doing this; please wait)"), false);
    if let Err(e) = act(page) {
        if e == STOPPED || e.contains("closed") {
            return Err(e);
        }
    }
    if page.wait(6, done)? {
        progress(format!("✓ {title}"));
        return Ok(());
    }
    progress(format!("{title}: waiting for you in the browser window"));
    let until = Instant::now() + Duration::from_secs(20 * 60);
    while Instant::now() < until {
        page.banner(&format!("Your turn: {manual}\n(Almanac carries on by itself once it's done.)"), true);
        if page.wait(2, done)? {
            progress(format!("✓ {title}"));
            return Ok(());
        }
    }
    Err(format!("“{title}” wasn't finished in time. Run Set Up in Browser again; finished steps are skipped."))
}

// --------------------------------------------------------- Google setup

pub struct GoogleSetup {
    pub page: Arc<Page>,
    pub project: String,
    pub email: String,
    pub client_id: String,
    pub client_secret: String,
}

fn console(path: &str, project: &str) -> String {
    let sep = if path.contains('?') { '&' } else { '?' };
    format!("https://console.cloud.google.com/{path}{sep}project={project}")
}

fn find_client_in(text: &str) -> Option<(String, String)> {
    let id = regex::Regex::new(r"\b\d{6,}-[a-z0-9]{10,}\.apps\.googleusercontent\.com\b").ok()?.find(text)?.as_str().to_string();
    let secret = regex::Regex::new(r"\bGOCSPX-[A-Za-z0-9_\-]{10,}").ok()?.find(text).map(|m| m.as_str().to_string()).unwrap_or_default();
    Some((id, secret))
}

fn find_client_download(since: std::time::SystemTime) -> Option<(String, String)> {
    for dir in [downloads_dir(), home().join("Downloads")] {
        if let Some(found) = find_client_download_in(&dir, since) {
            return Some(found);
        }
    }
    None
}

fn find_client_download_in(dir: &Path, since: std::time::SystemTime) -> Option<(String, String)> {
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !name.starts_with("client_secret") || !name.ends_with(".json") {
            continue;
        }
        if e.metadata().and_then(|m| m.modified()).map(|m| m < since).unwrap_or(true) {
            continue;
        }
        let text = std::fs::read_to_string(e.path()).ok()?;
        if let Some((id, secret)) = crate::autoreg::extract_ids(crate::oauth::GOOGLE, &text) {
            return Some((id, secret));
        }
    }
    None
}

/// Runs the whole Google Cloud setup in the browser. `known_project` is a
/// project made before (reused if it still exists).
pub fn setup_google(known_project: &str, own_browser: bool, progress: &dyn Fn(String)) -> Result<GoogleSetup, String> {
    CANCEL.store(false, Ordering::SeqCst);
    let b = installed().into_iter().next().ok_or("No Chrome, Brave, Chromium or Edge found. Install one of them, or follow the step-by-step guide instead.")?;
    let page = if own_browser {
        // The person's own browser: a new tab, usually already signed in.
        attach_own(&b, progress)?
    } else {
        progress(format!("Opening a separate {} window…", b.name));
        let (port, path) = launch(&b.exe, &profile_dir(), "about:blank", false)?;
        let page = Page::open_tab(&browser_ws_url(port, &path)?)?;
        let _ = std::fs::create_dir_all(downloads_dir());
        let _ = page.call("Page.setDownloadBehavior", json!({"behavior": "allow", "downloadPath": downloads_dir().display().to_string()}));
        page
    };

    // 1. Signed in to Google Cloud (the person types their own password).
    page.navigate("https://console.cloud.google.com/welcome")?;
    let signed_in = |p: &Page| p.url().starts_with("https://console.cloud.google.com/") && !p.eval_str("__alm.email()").is_empty();
    step(
        &page,
        progress,
        "Sign in to Google",
        "sign in with the Google account you want to use in Almanac (type your password, and answer any “I'm not a robot” or 2-step check). Almanac does the rest.",
        &|p| {
            // Google Cloud's terms of service on first use: ticked and accepted (as asked:
            // Almanac does everything it can by itself).
            if p.exists("^Agree and continue$") {
                p.tick_all();
                p.click("^Agree and continue$");
            }
            signed_in(p) && !p.exists("^Agree and continue$")
        },
        &|_| Ok(()),
    )?;
    let mut email = page.eval_str("__alm.email()");
    if email.is_empty() {
        email = page.eval_str("prompt('Almanac: which Gmail address will you use in Almanac?') || ''").trim().to_string();
    }
    if email.is_empty() {
        return Err("Almanac couldn't tell which Google account is signed in.".into());
    }
    progress(format!("Signed in as {email}"));

    // 2. The project: reuse the one made before, or an existing "Almanac", or make it.
    let mut project = String::new();
    if !known_project.is_empty() {
        page.navigate(&console("home/dashboard", known_project))?;
        let t = page.text();
        if !t.contains("don't have permission") && !t.contains("not found") && !t.contains("does not exist") {
            project = known_project.to_string();
        }
    }
    if project.is_empty() {
        page.navigate("https://console.cloud.google.com/cloud-resource-manager")?;
        let _ = page.wait(15, &|p| p.text().contains("Almanac") || p.exists("^Create project$"));
        project = page.eval_str(
            r#"(() => { for (const tr of __alm.all('tr')) { const cells = [...tr.querySelectorAll('td')].map(c => c.innerText.trim()); if (cells.some(c => /^Almanac$/i.test(c))) { const id = cells.find(c => /^[a-z][a-z0-9-]{4,28}[a-z0-9]$/.test(c) && c.toLowerCase() !== 'almanac'); if (id) return id; } } return ''; })()"#,
        );
    }
    if project.is_empty() {
        page.navigate("https://console.cloud.google.com/projectcreate")?;
        let created = |p: &Page| {
            let u = p.url();
            !u.contains("projectcreate") && u.contains("project=")
        };
        step(
            &page,
            progress,
            "Make the Almanac project",
            "type Almanac in the Project name box and press Create.",
            &created,
            &|p| {
                p.wait(20, &|p| p.fill("Project name", "Almanac"))?;
                std::thread::sleep(Duration::from_millis(1500));
                p.click_when("^Create$", 10)?;
                Ok(())
            },
        )?;
        project = regex::Regex::new(r"[?&]project=([a-z][a-z0-9-]{4,28}[a-z0-9])")
            .ok()
            .and_then(|r| r.captures(&page.url()).map(|c| c[1].to_string()))
            .unwrap_or_default();
        if project.is_empty() {
            return Err("The project was made but Almanac couldn't read its ID. Run Set Up in Browser again.".into());
        }
        // A new project needs a little while before services can be turned on.
        std::thread::sleep(Duration::from_secs(8));
    }
    let _ = crate::oauth::save_extra("google_project", &project);
    progress(format!("Using project {project}"));

    // 3. Turn on Gmail, Calendar and Contacts.
    for (api, label) in [("gmail.googleapis.com", "Gmail"), ("calendar-json.googleapis.com", "Google Calendar"), ("people.googleapis.com", "Google Contacts")] {
        page.navigate(&console(&format!("apis/library/{api}"), &project))?;
        let _ = page.wait(15, &|p| p.exists("^(Enable|Manage)$") || p.text().contains("API Enabled"));
        step(
            &page,
            progress,
            &format!("Turn on {label}"),
            "press the blue Enable button.",
            &|p| p.url().contains("/apis/api/") || p.text().contains("API Enabled") || p.exists("^Manage$"),
            &|p| {
                p.click_when("^Enable$", 15)?;
                p.wait(60, &|p| p.url().contains("/apis/api/") || p.text().contains("API Enabled") || p.exists("^Manage$"))?;
                Ok(())
            },
        )?;
    }

    // 4. Name the app (Google's “branding” wizard).
    page.navigate(&console("auth/overview", &project))?;
    let _ = page.wait(20, &|p| p.exists("^Get started$") || p.text().contains("Create OAuth client") || p.text().contains("Metrics"));
    let configured = |p: &Page| !p.exists("^Get started$") && !p.text().contains("not configured yet") && p.url().contains("/auth/");
    let em = email.clone();
    step(
        &page,
        progress,
        "Give the app a name",
        "press Get started. App name: Almanac. Support email: your address. Next. Audience: External. Next. Contact email: your address. Next. Tick the box to agree to Google's policy, press Continue, then Create.",
        &configured,
        &|p| {
            p.click_when("^Get started$", 10)?;
            p.wait(15, &|p| p.fill("App name", "Almanac"))?;
            // Support email: a drop-down list with the signed-in address.
            if p.click_when("User support email", 8)? {
                std::thread::sleep(Duration::from_millis(600));
                let _ = p.click_in(&regex::escape(&em), "[role=option],mat-option,li");
            }
            p.click_when("^Next$", 8)?;
            if p.click_when("^External", 10)? {
                std::thread::sleep(Duration::from_millis(400));
            }
            p.click_when("^Next$", 8)?;
            if p.wait(10, &|p| p.fill("Email addresses|Contact", &em))? {
                p.press_enter();
            }
            p.click_when("^Next$", 8)?;
            // Google's User Data Policy box: ticked by Almanac (as asked), and if
            // the page doesn't take that, the person is asked to tick it.
            let agreed = |p: &Page| p.eval("(() => { const b = __alm.all('input[type=checkbox]').filter(__alm.vis); return b.length > 0 && b.every(x => x.checked); })()").ok() == Some(Value::Bool(true));
            for _ in 0..3 {
                if agreed(p) {
                    break;
                }
                p.tick_all();
                std::thread::sleep(Duration::from_millis(500));
            }
            let until = Instant::now() + Duration::from_secs(15 * 60);
            while !agreed(p) && Instant::now() < until {
                check_cancel()?;
                let _ = p.eval("__alm.mark(__alm.all('input[type=checkbox]').filter(__alm.vis)[0] || null)");
                p.banner("Your turn: tick the box to agree to Google's User Data Policy. Almanac then carries on.", true);
                std::thread::sleep(Duration::from_millis(800));
            }
            p.click_when("^Continue$", 8)?;
            p.click_when("^Create$", 10)?;
            p.wait(30, &configured)?;
            Ok(())
        },
    )?;

    // 5. Let this account sign in (Test users) — avoids “Access blocked”.
    page.navigate(&console("auth/audience", &project))?;
    let _ = page.wait(20, &|p| p.text().contains("Test users") || p.text().contains("In production"));
    let low = email.to_lowercase();
    let listed = move |p: &Page| {
        let t = p.text().to_lowercase();
        t.contains("in production") || t.split("test users").nth(1).map(|rest| rest.contains(&low)).unwrap_or(false)
    };
    let em = email.clone();
    step(
        &page,
        progress,
        "Add you as a test user",
        &format!("under Test users press + Add users, type {em}, and press Save. Don't press Publish app."),
        &listed,
        &|p| {
            p.click_when("Add users", 10)?;
            std::thread::sleep(Duration::from_millis(800));
            let typed = p.wait(10, &|p| {
                p.eval("(() => { const d = __alm.all('[role=dialog] input, mat-dialog-container input, .cdk-overlay-pane input').filter(__alm.vis)[0]; if (!d) return false; d.focus(); return true; })()").ok() == Some(Value::Bool(true))
            })?;
            if typed {
                let _ = p.call("Input.insertText", json!({"text": em}));
                p.press_enter();
                std::thread::sleep(Duration::from_millis(500));
                p.click_when("^Save$", 8)?;
                std::thread::sleep(Duration::from_millis(2500));
                p.navigate(&console("auth/audience", &project))?;
            }
            Ok(())
        },
    )?;

    // 6. The sign-in key (a “Desktop app” client). Reuse the one made by an
    // earlier run for this same project.
    if crate::oauth::load_extra("google_client_project") == project {
        if let Some((id, secret)) = crate::oauth::builtin_client(crate::oauth::GOOGLE).filter(|(_, s)| !s.is_empty()) {
            progress("✓ Create the sign-in key (already done)".into());
            return Ok(GoogleSetup { page, project, email, client_id: id, client_secret: secret });
        }
    }
    let since = std::time::SystemTime::now();
    page.navigate(&console("auth/clients/create", &project))?;
    let found = Mutex::new(None::<(String, String)>);
    let have = |p: &Page| {
        if found.lock().unwrap().is_some() {
            return true;
        }
        let got = find_client_in(&p.text()).filter(|(_, s)| !s.is_empty()).or_else(|| find_client_download(since));
        if got.is_some() {
            *found.lock().unwrap() = got;
            return true;
        }
        false
    };
    step(
        &page,
        progress,
        "Create the sign-in key",
        "Application type: Desktop app. Press Create. In the box that appears press Download JSON.",
        &have,
        &|p| {
            if p.click_when("Application type", 15)? {
                std::thread::sleep(Duration::from_millis(600));
                p.click_when("^Desktop app$", 8)?;
            }
            std::thread::sleep(Duration::from_millis(500));
            let _ = p.fill("^\\s*Name", "Almanac");
            p.click_when("^Create$", 10)?;
            if !p.wait(20, &|p| find_client_in(&p.text()).map(|(_, s)| !s.is_empty()).unwrap_or(false))? {
                // The secret isn't shown on the page: download the file instead.
                p.click_when("Download JSON", 10)?;
            }
            Ok(())
        },
    )?;
    let (client_id, client_secret) = found.into_inner().unwrap().ok_or("No sign-in key was found.")?;
    let _ = page.click("^OK$");
    let _ = crate::oauth::save_extra("google_client_project", &project);
    Ok(GoogleSetup { page, project, email, client_id, client_secret })
}

/// While Google's sign-in pages are open in Almanac's window, keep a banner
/// on them with what to press. Stops when `done` is set.
pub fn coach_sign_in(page: Arc<Page>, email: String, done: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        while !done.load(Ordering::SeqCst) && !CANCEL.load(Ordering::SeqCst) {
            let t = page.text();
            // Almanac clicks through what it can (as asked); passwords and checks stay with the person.
            let msg = if t.contains("hasn't verified this app") || t.contains("has not verified this app") {
                if !page.click("^Go to Almanac") {
                    page.click("^Advanced$");
                }
                "This is your own copy of Almanac, so Google warns it isn't verified; Almanac is going past that."
            } else if t.contains("Select all") || t.contains("wants access") || t.contains("wants additional access") {
                page.click("^Select all$");
                page.tick_all();
                std::thread::sleep(Duration::from_millis(400));
                page.click("^(Continue|Allow)$");
                "Allowing access to mail, calendars and contacts…"
            } else if t.contains("Choose an account") {
                page.click(&regex::escape(&email));
                "Choosing your account…"
            } else if t.contains("Signed in") {
                "Done! Almanac is signed in. You can close this tab."
            } else if t.contains("Enter your password") || t.contains("2-Step") || t.contains("Verify it") || t.contains("robot") {
                "Your turn: sign in (password, 2-step check). Almanac carries on after."
            } else {
                ""
            };
            if !msg.is_empty() {
                page.banner(msg, msg.starts_with("Your turn"));
            }
            std::thread::sleep(Duration::from_millis(1200));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drives a real headless Chrome/Brave on a local page: finding by text,
    /// typing, clicking, the banner, reading results back.
    #[test]
    fn drives_a_local_page() {
        let Some((_, exe)) = find_browser() else {
            eprintln!("no Chrome-family browser here; skipped");
            return;
        };
        let profile = std::env::temp_dir().join(format!("almanac-cdp-test-{}", std::process::id()));
        let (port, path) = match launch(&exe, &profile, "about:blank", true) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("browser didn't start ({e}); skipped");
                return;
            }
        };
        // Same route as the person's own browser: its address, then a new tab.
        let page = Page::open_tab(&browser_ws_url(port, &path).unwrap()).unwrap();
        let html = r#"<html><body><h1>Test form</h1>
            <label for=n>Project name</label><input id=n value="My Project 123">
            <div role=combobox aria-label="Application type" onclick="document.getElementById('opts').style.display='block'">Choose…</div>
            <div id=opts style="display:none"><div role=option onclick="document.getElementById('t').textContent='desktop'">Desktop app</div><div role=option>Web application</div></div>
            <button onclick="document.getElementById('out').textContent = document.getElementById('n').value + '/' + document.getElementById('t').textContent">Create</button>
            <label><input type=checkbox id=agree> I agree</label>
            <p id=t></p><p id=out></p>
            <p>Lots of text so the page counts as loaded. Lots of text so the page counts as loaded. Lots of text so the page counts as loaded.</p>
            </body></html>"#;
        use base64::Engine;
        let url = format!("data:text/html;base64,{}", base64::engine::general_purpose::STANDARD.encode(html));
        page.navigate(&url).unwrap();
        assert!(page.exists("^Create$"));
        assert!(page.fill("Project name", "Almanac"));
        assert!(page.click("Application type"));
        assert!(page.click_when("^Desktop app$", 5).unwrap());
        assert!(page.click("^Create$"));
        assert!(page.wait(5, &|p| p.text().contains("Almanac/desktop")).unwrap(), "page said: {}", page.text());
        page.tick_all();
        assert_eq!(page.eval("document.getElementById('agree').checked").unwrap(), Value::Bool(true), "tick_all ticks boxes");
        page.banner("hello", false);
        assert!(page.text().contains("Almanac: hello"));
        let browser = Page { ws: Mutex::new(Ws::connect(&browser_ws_url(port, &path).unwrap()).unwrap()), next: AtomicU64::new(1), session: String::new() };
        let _ = browser.call("Browser.close", json!({}));
        // Chrome may still be writing its profile for a moment after closing.
        for _ in 0..20 {
            std::thread::sleep(Duration::from_millis(300));
            if std::fs::remove_dir_all(&profile).is_ok() && !profile.exists() {
                break;
            }
        }
    }

    #[test]
    fn finds_client_ids_in_text() {
        let t = "OAuth client created\nClient ID\n123456789012-abcdefghijklmnop0123.apps.googleusercontent.com\nClient secret\nGOCSPX-AbCdEf_123456-xyz\nOK";
        let (id, s) = find_client_in(t).unwrap();
        assert_eq!(id, "123456789012-abcdefghijklmnop0123.apps.googleusercontent.com");
        assert_eq!(s, "GOCSPX-AbCdEf_123456-xyz");
    }
}
