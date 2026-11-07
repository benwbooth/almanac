//! Signing in with Google or Microsoft (OAuth 2.0 with PKCE). The browser
//! shows the provider's own sign-in page and sends the result back to a
//! one-off address on this computer. The long-lived refresh token is kept in
//! the system keychain; short-lived access tokens are cached in memory.
//!
//! Each provider only accepts sign-ins from registered apps. The
//! registration (client ID) can be built into the app when it is packaged
//! (ALMANAC_GOOGLE_CLIENT_ID etc.), put in an `oauth-clients.json` file next
//! to the program or in its data folder, or typed into Accounts.

use base64::Engine;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const GOOGLE: &str = "google";
pub const MICROSOFT: &str = "microsoft";
pub const YAHOO: &str = "yahoo";
pub const AOL: &str = "aol";

/// Sign-in providers: (id, name shown, what it syncs).
pub const PROVIDERS: [(&str, &str, &str); 4] = [
    (GOOGLE, "Google", "Gmail, Google Calendar and Contacts"),
    (MICROSOFT, "Microsoft", "Outlook.com, Hotmail, Live and Microsoft 365: mail, calendars and contacts"),
    (YAHOO, "Yahoo", "Yahoo Mail"),
    (AOL, "AOL", "AOL Mail"),
];

/// The app registration for a provider, if one comes with the app:
/// built in when packaged, from the environment, or from `oauth-clients.json`
/// ({"google": {"client_id": "…", "client_secret": "…"}, …}).
pub fn builtin_client(provider: &str) -> Option<(String, String)> {
    let up = provider.to_uppercase();
    let built: Option<(&str, &str)> = match provider {
        GOOGLE => option_env!("ALMANAC_GOOGLE_CLIENT_ID").map(|i| (i, option_env!("ALMANAC_GOOGLE_CLIENT_SECRET").unwrap_or(""))),
        MICROSOFT => option_env!("ALMANAC_MICROSOFT_CLIENT_ID").map(|i| (i, "")),
        YAHOO => option_env!("ALMANAC_YAHOO_CLIENT_ID").map(|i| (i, option_env!("ALMANAC_YAHOO_CLIENT_SECRET").unwrap_or(""))),
        AOL => option_env!("ALMANAC_AOL_CLIENT_ID").map(|i| (i, option_env!("ALMANAC_AOL_CLIENT_SECRET").unwrap_or(""))),
        _ => None,
    };
    if let Some((i, s)) = built.filter(|(i, _)| !i.is_empty()) {
        return Some((i.to_string(), s.to_string()));
    }
    if let Ok(i) = std::env::var(format!("ALMANAC_{up}_CLIENT_ID")) {
        if !i.is_empty() {
            return Some((i, std::env::var(format!("ALMANAC_{up}_CLIENT_SECRET")).unwrap_or_default()));
        }
    }
    let mut places = vec![crate::data::data_dir().join("oauth-clients.json")];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            places.push(d.join("oauth-clients.json"));
        }
    }
    for p in places {
        let Ok(text) = std::fs::read_to_string(p) else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
        if let Some(id) = v.pointer(&format!("/{provider}/client_id")).and_then(Value::as_str).filter(|x| !x.is_empty()) {
            let secret = v.pointer(&format!("/{provider}/client_secret")).and_then(Value::as_str).unwrap_or("");
            return Some((id.to_string(), secret.to_string()));
        }
    }
    None
}

pub fn providers_json() -> Value {
    Value::Array(
        PROVIDERS
            .iter()
            .map(|(id, name, what)| serde_json::json!({"id": id, "name": name, "detail": what, "ready": builtin_client(id).is_some()}))
            .collect(),
    )
}

/// How to register the app with each provider (shown in Sign-in Setup).
/// Every step has a plain title, a fuller explanation and, where there is
/// one, the exact page to open ("url") or what Almanac does ("action").
/// All steps can be repeated safely.
pub fn setup_json() -> Value {
    use serde_json::json;
    let project = {
        let p = load_extra("google_project");
        if p.is_empty() {
            builtin_client(GOOGLE).map(|(id, _)| id.chars().take_while(|c| c.is_ascii_digit()).collect::<String>()).unwrap_or_default()
        } else {
            p
        }
    };
    let gq = |path: &str| -> String {
        if project.is_empty() {
            format!("https://console.cloud.google.com/{path}")
        } else {
            format!("https://console.cloud.google.com/{path}{}project={project}", if path.contains('?') { "&" } else { "?" })
        }
    };
    let steps = |p: &str| -> Value {
        match p {
            GOOGLE => json!([
                {"title": "Make a project for Almanac",
                 "detail": "A project is just a folder in Google Cloud that holds Almanac's settings. It's free.",
                 "substeps": [
                     "Press Open page. Google Cloud opens in your web browser.",
                     "If Google asks you to sign in, use your normal Google account.",
                     "If it shows terms of service, tick the box to agree and press Agree and continue.",
                     "In the Project name box, type Almanac.",
                     "Leave everything else as it is and press the blue Create button.",
                     "Wait about 10 seconds. When the bell icon says the project is ready, press Select project."],
                 "done": "The box at the top-left of the page says Almanac.",
                 "trouble": "Already made an Almanac project before? Don't make another one: click the project box at the top-left and choose Almanac.",
                 "url": "https://console.cloud.google.com/projectcreate"},
                {"title": "Turn on Gmail",
                 "detail": "This lets Almanac read and send your mail.",
                 "substeps": [
                     "Press Open page.",
                     "Check the top-left of the page says Almanac. If not, click it and choose Almanac.",
                     "Press the blue Enable button."],
                 "done": "The page says API Enabled, or shows a Manage button instead of Enable.",
                 "url": gq("apis/library/gmail.googleapis.com")},
                {"title": "Turn on Google Calendar",
                 "detail": "This lets Almanac show and change your calendars.",
                 "substeps": [
                     "Press Open page.",
                     "Check the top-left says Almanac.",
                     "Press the blue Enable button."],
                 "done": "The page says API Enabled, or shows a Manage button.",
                 "url": gq("apis/library/calendar-json.googleapis.com")},
                {"title": "Turn on Google Contacts",
                 "detail": "Google calls this the People API.",
                 "substeps": [
                     "Press Open page.",
                     "Check the top-left says Almanac.",
                     "Press the blue Enable button."],
                 "done": "The page says API Enabled, or shows a Manage button.",
                 "url": gq("apis/library/people.googleapis.com")},
                {"title": "Give the app a name",
                 "detail": "Google asks a few questions about the app. It takes four short pages.",
                 "substeps": [
                     "Press Open page, then press Get started.",
                     "Page 1, App information: in App name type Almanac. In User support email pick your address from the list. Press Next.",
                     "Page 2, Audience: choose External. Press Next.",
                     "Page 3, Contact information: type your email address. Press Next.",
                     "Page 4, Finish: tick the box that says you agree to the Google API Services User Data Policy. Press Continue.",
                     "Press Create."],
                 "done": "The page shows an Overview instead of a Get started button. If you already see the Overview, this step was done earlier.",
                 "url": gq("auth/overview")},
                {"title": "Add yourself as a test user",
                 "detail": "Your copy of Almanac stays in Google's Testing mode, so only people you list here can sign in. This prevents Google's “Access blocked” message.",
                 "substeps": [
                     "Press Open page. It opens the Audience page.",
                     "Leave Publishing status as Testing. Don't press Publish app.",
                     "Scroll down to Test users and press + Add users.",
                     "Type the Gmail address you'll use in Almanac. Have more than one Google account? Add each one, separated by commas.",
                     "Press Save."],
                 "done": "Your address is listed under Test users.",
                 "trouble": "Seeing “Valid app name, support email, homepage url, and privacy policy url are required for switching the app to external production mode”? That appears when you press Publish app. You don't need to publish. Close that message, press Cancel or Back to testing, and add yourself under Test users instead. While in Testing, Google signs Almanac out after 7 days. When that happens, sign in again from Accounts. It takes one click and keeps all your mail.",
                 "url": gq("auth/audience")},
                {"title": "Create the sign-in key and download it",
                 "detail": "The key is a small file that identifies your copy of Almanac to Google. It isn't your password.",
                 "substeps": [
                     "Press Open page.",
                     "Application type: choose Desktop app.",
                     "Name: leave it or type Almanac.",
                     "Press Create.",
                     "A box shows “OAuth client created”. Press Download JSON in that box. The file goes to your Downloads folder.",
                     "Press OK."],
                 "done": "A file named client_secret_….json is in your Downloads folder.",
                 "trouble": "Closed the box without downloading? Just do this step again. An extra key does no harm.",
                 "url": gq("auth/clients/create")},
                {"title": "Give the file to Almanac",
                 "detail": "Almanac reads the key from the file you just downloaded.",
                 "substeps": [
                     "Press Choose File….",
                     "Open your Downloads folder.",
                     "Pick the file whose name starts with client_secret and press Open."],
                 "done": "This page changes to say Google is ready, with a Sign In button.",
                 "action": "load"},
                {"title": "Sign in",
                 "detail": "Google's own sign-in page opens in your browser. Almanac never sees your password.",
                 "substeps": [
                     "Choose the Google account you added as a test user.",
                     "Google says “Google hasn't verified this app”. This is expected, because it's your own copy. Press the small Advanced link, then Go to Almanac (unsafe).",
                     "On the access page, tick Select all (or every box) so mail, calendars and contacts all work.",
                     "Press Continue.",
                     "When the page says Signed in, close it and go back to Almanac."],
                 "done": "Your Google account appears in Almanac's sidebar and starts syncing.",
                 "trouble": "If Google says “Access blocked”, your address isn't in Test users yet. Do the Add yourself as a test user step again, then sign in again.",
                 "action": "signin"},
            ]),
            MICROSOFT => json!([
                {"title": "Open App registrations",
                 "detail": "Microsoft's page for registering apps. It's free.",
                 "substeps": [
                     "Press Open page.",
                     "Sign in with your Microsoft account (the same one you use for Outlook is fine).",
                     "If Microsoft asks you to set up a free Azure account or directory first, follow its steps. It takes a minute and costs nothing."],
                 "done": "You see a page titled App registrations.",
                 "url": "https://entra.microsoft.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade"},
                {"title": "Register Almanac",
                 "detail": "This tells Microsoft that your copy of Almanac exists.",
                 "substeps": [
                     "Press Open page, or press + New registration.",
                     "Name: type Almanac.",
                     "Supported account types: choose “Accounts in any organizational directory and personal Microsoft accounts”.",
                     "Redirect URI: in the first list choose “Public client/native (mobile & desktop)”. In the box next to it type http://localhost",
                     "Press Register."],
                 "done": "You see Almanac's Overview page.",
                 "trouble": "Already registered Almanac before? Don't register again. Open App registrations, press All applications and click Almanac.",
                 "url": "https://entra.microsoft.com/#view/Microsoft_AAD_RegisteredApps/CreateApplicationBlade"},
                {"title": "Allow desktop sign-in",
                 "detail": "This setting lets a desktop app like Almanac sign in.",
                 "substeps": [
                     "On Almanac's page, click Authentication in the list on the left.",
                     "Scroll to the bottom, to Advanced settings.",
                     "Set “Allow public client flows” to Yes.",
                     "Press Save at the top or bottom."],
                 "done": "“Allow public client flows” shows Yes.",
                 "url": ""},
                {"title": "Copy the Application ID",
                 "detail": "This is the only thing Almanac needs from Microsoft.",
                 "substeps": [
                     "Click Overview in the list on the left.",
                     "Find “Application (client) ID”. It looks like 3f2504e0-4f89-11d3-9a0c-0305e82c3301.",
                     "Press the copy icon next to it.",
                     "Click the box below in Almanac and paste (Ctrl+V), then press Use This and Sign In."],
                 "action": "paste"},
                {"title": "Sign in",
                 "detail": "Microsoft's own page opens in your browser. Almanac never sees your password.",
                 "substeps": [
                     "Sign in with your Outlook, Hotmail, Live or work account.",
                     "Press Accept to allow access to mail, calendars and contacts.",
                     "When the page says Signed in, close it and go back to Almanac."],
                 "done": "Your account appears in Almanac's sidebar and starts syncing.",
                 "action": "signin"},
            ]),
            _ => {
                let site = if p == YAHOO { "https://developer.yahoo.com/apps/" } else { "https://developer.aol.com/apps/" };
                json!([
                    {"title": "Open the developer apps page",
                     "detail": "The provider's free page for registering apps.",
                     "substeps": [
                         "Press Open page.",
                         "Sign in with your account.",
                         "Press Create an App."],
                     "trouble": "Already made an Almanac app? Don't make another. Open the existing one and go to the Copy step.",
                     "url": site},
                    {"title": "Describe the app",
                     "detail": "Fill in the form.",
                     "substeps": [
                         "Application name: type Almanac.",
                         "Application type: choose Installed Application.",
                         "Redirect URI: type oob",
                         "API permissions: tick Mail and choose Read/Write. Tick OpenID Connect Permissions and choose Email.",
                         "Press Create App."],
                     "url": ""},
                    {"title": "Copy the Client ID and Client Secret",
                     "detail": "They're shown on the app's page.",
                     "substeps": [
                         "Copy Client ID and paste it into the box below.",
                         "Copy Client Secret and paste it into the same box on a new line.",
                         "Press Use This and Sign In."],
                     "action": "paste"},
                    {"title": "Sign in",
                     "detail": "The provider's own page opens in your browser.",
                     "substeps": [
                         "Sign in and press Agree.",
                         "The page shows a short code. Copy it.",
                         "Paste it into the box Almanac shows and press Continue."],
                     "done": "Your account appears in Almanac's sidebar.",
                     "action": "signin"},
                ])
            }
        }
    };
    let console = |p: &str| match p {
        GOOGLE => "https://console.cloud.google.com/apis/credentials",
        MICROSOFT => "https://entra.microsoft.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade",
        YAHOO => "https://developer.yahoo.com/apps/",
        _ => "https://developer.aol.com/apps/",
    };
    Value::Array(
        PROVIDERS
            .iter()
            .map(|(id, name, what)| {
                let (cid, secret) = builtin_client(id).unwrap_or_default();
                // Google client IDs start with the project number: link straight
                // to that project's Audience page (test users).
                // The project the sign-in key in use belongs to: test users
                // must be added in that very project.
                let key_project: String = if *id == GOOGLE { cid.chars().take_while(|c| c.is_ascii_digit()).collect() } else { String::new() };
                let audience = if *id != GOOGLE {
                    String::new()
                } else if key_project.is_empty() {
                    gq("auth/audience")
                } else {
                    format!("https://console.cloud.google.com/auth/audience?project={key_project}")
                };
                serde_json::json!({
                    "audience": audience, "keyProject": key_project,
                    "id": id, "name": name, "detail": what, "ready": !cid.is_empty(),
                    "source": client_source(id), "clientId": cid, "hasSecret": !secret.is_empty(),
                    "needsSecret": *id != MICROSOFT, "console": console(id), "steps": steps(id),
                    "pasteCode": *id == YAHOO || *id == AOL,
                })
            })
            .collect(),
    )
}

/// Mail servers used with each provider's sign-in: (IMAP, SMTP, SMTP port).
pub fn mail_servers(provider: &str) -> (&'static str, &'static str, u16) {
    match provider {
        GOOGLE => ("imap.gmail.com", "smtp.gmail.com", 465),
        MICROSOFT => ("outlook.office365.com", "smtp.office365.com", 587),
        YAHOO => ("imap.mail.yahoo.com", "smtp.mail.yahoo.com", 465),
        _ => ("imap.aol.com", "smtp.aol.com", 465),
    }
}

/// What each kind of access is for.
pub enum Resource {
    /// IMAP + SMTP
    Mail,
    /// Calendars and contacts (Google APIs / Microsoft Graph)
    Data,
}

/// Stand-in provider used by the automated tests (address from ALMANAC_OAUTH_TEST_BASE).
const TEST: &str = "test";

fn test_base() -> String {
    std::env::var("ALMANAC_OAUTH_TEST_BASE").unwrap_or_default()
}

fn auth_url(provider: &str, tenant: &str) -> String {
    match provider {
        TEST => format!("{}/auth", test_base()),
        GOOGLE => "https://accounts.google.com/o/oauth2/v2/auth".into(),
        YAHOO => "https://api.login.yahoo.com/oauth2/request_auth".into(),
        AOL => "https://api.login.aol.com/oauth2/request_auth".into(),
        _ => format!("https://login.microsoftonline.com/{}/oauth2/v2.0/authorize", if tenant.is_empty() { "common" } else { tenant }),
    }
}

fn token_url(provider: &str, tenant: &str) -> String {
    match provider {
        TEST => format!("{}/token", test_base()),
        GOOGLE => "https://oauth2.googleapis.com/token".into(),
        YAHOO => "https://api.login.yahoo.com/oauth2/get_token".into(),
        AOL => "https://api.login.aol.com/oauth2/get_token".into(),
        _ => format!("https://login.microsoftonline.com/{}/oauth2/v2.0/token", if tenant.is_empty() { "common" } else { tenant }),
    }
}

pub fn scopes(provider: &str, r: &Resource) -> &'static str {
    match (provider, r) {
        (GOOGLE, _) => "openid email https://mail.google.com/ https://www.googleapis.com/auth/calendar https://www.googleapis.com/auth/contacts",
        (YAHOO, _) | (AOL, _) => "openid email mail-w",
        (TEST, _) => "openid email",
        (_, Resource::Mail) => "offline_access https://outlook.office.com/IMAP.AccessAsUser.All https://outlook.office.com/SMTP.Send",
        (_, Resource::Data) => "offline_access openid email https://graph.microsoft.com/User.Read https://graph.microsoft.com/Calendars.ReadWrite https://graph.microsoft.com/Contacts.ReadWrite",
    }
}

fn random_string(n: usize) -> String {
    use ring::rand::SecureRandom;
    let mut bytes = vec![0u8; n];
    let _ = ring::rand::SystemRandom::new().fill(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn pkce_challenge(verifier: &str) -> String {
    let d = ring::digest::digest(&ring::digest::SHA256, verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(d.as_ref())
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

fn dec(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(x) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(x);
                i += 3;
                continue;
            }
        }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn form(pairs: &[(&str, &str)]) -> String {
    pairs.iter().map(|(k, v)| format!("{}={}", enc(k), enc(v))).collect::<Vec<_>>().join("&")
}

/// Posts a form to a token endpoint. `basic` = (client id, secret) for
/// providers that want the app's registration in an Authorization header.
fn post_form(url: &str, body: &str, basic: Option<(&str, &str)>) -> Result<Value, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(60)))
        .build()
        .into();
    let mut req = agent.post(url).header("Content-Type", "application/x-www-form-urlencoded").header("Accept", "application/json");
    if let Some((id, secret)) = basic {
        let token = base64::engine::general_purpose::STANDARD.encode(format!("{id}:{secret}"));
        req = req.header("Authorization", &format!("Basic {token}"));
    }
    let mut resp = req.send(body).map_err(|e| format!("Could not reach the sign-in service: {e}"))?;
    let text = resp.body_mut().read_to_string().unwrap_or_default();
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if let Some(err) = v.get("error") {
        let desc = v.get("error_description").and_then(Value::as_str).unwrap_or("");
        let code = err.as_str().unwrap_or("error");
        let hint = match code {
            "invalid_client" | "unauthorized_client" => " (check the app's client ID and secret in Sign-in Setup)",
            "invalid_grant" => " (the sign-in expired: open Accounts and sign in with this account again; it keeps all your mail. Google signs apps in Testing mode out every 7 days)",
            "access_denied" => " (Google: add your address under Test users on the project's Audience page, then sign in again)",
            _ => "",
        };
        return Err(format!("Sign-in failed: {code} {desc}{hint}").trim().to_string());
    }
    if v.is_null() {
        return Err(format!("Sign-in failed: unexpected answer from {url}"));
    }
    Ok(v)
}

fn basic_for(provider: &str, id: &str, secret: &str) -> Option<(String, String)> {
    ((provider == YAHOO || provider == AOL) && !secret.is_empty()).then(|| (id.to_string(), secret.to_string()))
}

/// A code pasted by the person from the provider's page (when the browser
/// could not hand it back on its own).
static PASTED_CODE: Mutex<Option<String>> = Mutex::new(None);
/// Bumped by every new sign-in and by Cancel, so an older sign-in that is
/// still waiting for the browser stops quietly.
static SIGN_IN_ROUND: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// Error returned by a sign-in that was cancelled or replaced by a newer one.
pub const SIGN_IN_STOPPED: &str = "Sign-in stopped.";

pub fn cancel_sign_in() {
    SIGN_IN_ROUND.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

pub fn paste_code(code: &str) {
    let c = code.trim();
    // Accept the whole address bar as well as just the code.
    let c = c.split("code=").nth(1).map(|x| x.split('&').next().unwrap_or(x)).unwrap_or(c);
    *PASTED_CODE.lock().unwrap() = Some(dec(c));
}

/// Saves an app registration for a provider (used for every account of it).
/// An empty id forgets the saved one.
pub fn save_client(provider: &str, id: &str, secret: &str) -> Result<(), String> {
    let path = crate::data::data_dir().join("oauth-clients.json");
    let mut v: Value = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| serde_json::json!({}));
    if id.trim().is_empty() {
        if let Some(m) = v.as_object_mut() {
            m.retain(|k, _| k != provider);
        }
    } else {
        // An empty secret keeps the one saved before.
        let keep = v.pointer(&format!("/{provider}/client_secret")).and_then(Value::as_str).unwrap_or("").to_string();
        let secret = if secret.trim().is_empty() { keep } else { secret.trim().to_string() };
        v[provider] = serde_json::json!({"client_id": id.trim(), "client_secret": secret});
    }
    let _ = std::fs::create_dir_all(crate::data::data_dir());
    std::fs::write(&path, serde_json::to_string_pretty(&v).unwrap_or_default()).map_err(|e| e.to_string())
}

/// Small extra values kept with the registrations (e.g. the Google project).
pub fn save_extra(key: &str, value: &str) -> Result<(), String> {
    let path = crate::data::data_dir().join("oauth-clients.json");
    let mut v: Value = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| serde_json::json!({}));
    v["_extra"][key] = serde_json::json!(value);
    let _ = std::fs::create_dir_all(crate::data::data_dir());
    std::fs::write(&path, serde_json::to_string_pretty(&v).unwrap_or_default()).map_err(|e| e.to_string())
}

pub fn load_extra(key: &str) -> String {
    std::fs::read_to_string(crate::data::data_dir().join("oauth-clients.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.pointer(&format!("/_extra/{key}")).and_then(Value::as_str).map(String::from))
        .unwrap_or_default()
}

/// Where a provider's registration comes from: "saved", "built in" or "".
pub fn client_source(provider: &str) -> &'static str {
    let saved = std::fs::read_to_string(crate::data::data_dir().join("oauth-clients.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.pointer(&format!("/{provider}/client_id")).and_then(Value::as_str).map(|s| !s.is_empty()))
        .unwrap_or(false);
    if saved {
        "saved"
    } else if builtin_client(provider).is_some() {
        "built in"
    } else {
        ""
    }
}

pub struct SignIn {
    pub refresh_token: String,
    pub email: String,
}

/// Runs the browser sign-in. `open_url` is called with the page to show.
pub fn sign_in(provider: &str, client_id: &str, client_secret: &str, tenant: &str, open_url: &dyn Fn(String)) -> Result<SignIn, String> {
    if client_id.trim().is_empty() {
        return Err("Enter the app (client) ID first.".into());
    }
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    // Yahoo and AOL only allow fixed return addresses, so their page shows a
    // code to paste back ("oob"); the others come straight back here.
    let manual = provider == YAHOO || provider == AOL;
    let redirect = if manual {
        "oob".to_string()
    } else if provider == GOOGLE || provider == TEST {
        format!("http://127.0.0.1:{port}")
    } else {
        format!("http://localhost:{port}")
    };
    *PASTED_CODE.lock().unwrap() = None;
    let round = SIGN_IN_ROUND.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    let verifier = random_string(48);
    let state = random_string(16);
    let scope = if provider != MICROSOFT {
        scopes(provider, &Resource::Data).to_string()
    } else {
        // Ask for mail and calendar/contacts consent together.
        format!("{} {}", scopes(MICROSOFT, &Resource::Data), "https://outlook.office.com/IMAP.AccessAsUser.All https://outlook.office.com/SMTP.Send")
    };
    let mut params = vec![
        ("client_id", client_id.to_string()),
        ("response_type", "code".to_string()),
        ("redirect_uri", redirect.clone()),
        ("scope", scope),
        ("state", state.clone()),
        ("code_challenge", pkce_challenge(&verifier)),
        ("code_challenge_method", "S256".to_string()),
    ];
    if provider == GOOGLE {
        params.push(("access_type", "offline".into()));
        // Always show the account chooser, so the account picked is the one
        // listed under Test users rather than whichever is signed in already.
        params.push(("prompt", "select_account consent".into()));
    } else if provider == MICROSOFT {
        params.push(("prompt", "select_account".into()));
    }
    let query = params.iter().map(|(k, v)| format!("{}={}", enc(k), enc(v))).collect::<Vec<_>>().join("&");
    open_url(format!("{}?{query}", auth_url(provider, tenant)));

    // Wait (up to five minutes) for the browser to come back.
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(300);
    let code = loop {
        if Instant::now() > deadline {
            return Err("Sign-in timed out.".into());
        }
        if SIGN_IN_ROUND.load(std::sync::atomic::Ordering::SeqCst) != round {
            return Err(SIGN_IN_STOPPED.into());
        }
        if let Some(c) = PASTED_CODE.lock().unwrap().take().filter(|c| !c.is_empty()) {
            break c;
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let mut line = String::new();
                let _ = BufReader::new(&stream).read_line(&mut line);
                let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
                let q: HashMap<String, String> = path
                    .split_once('?')
                    .map(|(_, q)| q)
                    .unwrap_or("")
                    .split('&')
                    .filter_map(|kv| kv.split_once('=').map(|(k, v)| (k.to_string(), dec(v))))
                    .collect();
                let ok = q.get("code").is_some() && q.get("state") == Some(&state);
                let page = if ok {
                    "<html><body style='font-family:sans-serif;padding:40px'><h2>Signed in</h2>You can close this page and go back to Almanac.</body></html>"
                } else {
                    "<html><body style='font-family:sans-serif;padding:40px'><h2>Sign-in did not finish</h2>Go back to Almanac and try again.</body></html>"
                };
                let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}", page.len());
                if let Some(err) = q.get("error") {
                    let hint = if provider == GOOGLE && err == "access_denied" {
                        " — if Google said “Access blocked”, add your Gmail address under Test users on the project's Audience page (leave it in Testing), then sign in again."
                    } else {
                        ""
                    };
                    return Err(format!("Sign-in was not allowed: {err} {}{hint}", q.get("error_description").cloned().unwrap_or_default()));
                }
                if ok {
                    break q["code"].clone();
                }
            }
            Err(_) => std::thread::sleep(Duration::from_millis(200)),
        }
    };

    let mut body = vec![
        ("client_id", client_id),
        ("grant_type", "authorization_code"),
        ("code", code.as_str()),
        ("redirect_uri", redirect.as_str()),
        ("code_verifier", verifier.as_str()),
    ];
    if !client_secret.is_empty() {
        body.push(("client_secret", client_secret));
    }
    // Microsoft hands out a token for one service at a time: ask for the
    // calendars/contacts one here; mail tokens come from the refresh token.
    let ms_scope = scopes(MICROSOFT, &Resource::Data);
    if provider == MICROSOFT {
        body.push(("scope", ms_scope));
    }
    let basic = basic_for(provider, client_id, client_secret);
    let v = post_form(&token_url(provider, tenant), &form(&body), basic.as_ref().map(|(a, b)| (a.as_str(), b.as_str())))?;
    let refresh = v.get("refresh_token").and_then(Value::as_str).ok_or("The provider gave no refresh token")?.to_string();
    let access = v.get("access_token").and_then(Value::as_str).unwrap_or("").to_string();
    let email = who_am_i(provider, &access, v.get("id_token").and_then(Value::as_str).unwrap_or(""));
    Ok(SignIn { refresh_token: refresh, email })
}

/// The signed-in address, from the ID token or the provider's profile.
fn who_am_i(provider: &str, access: &str, id_token: &str) -> String {
    if let Some(payload) = id_token.split('.').nth(1) {
        if let Ok(bytes) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload) {
            if let Ok(v) = serde_json::from_slice::<Value>(&bytes) {
                for k in ["email", "preferred_username", "upn"] {
                    if let Some(e) = v.get(k).and_then(Value::as_str) {
                        if e.contains('@') {
                            return e.to_string();
                        }
                    }
                }
            }
        }
    }
    let url = match provider {
        GOOGLE => "https://openidconnect.googleapis.com/v1/userinfo",
        YAHOO => "https://api.login.yahoo.com/openid/v1/userinfo",
        AOL => "https://api.login.aol.com/openid/v1/userinfo",
        _ => "https://graph.microsoft.com/v1.0/me",
    };
    let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).build().into();
    let v: Value = agent
        .get(url)
        .header("Authorization", &format!("Bearer {access}"))
        .call()
        .ok()
        .and_then(|mut r| r.body_mut().read_to_string().ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null);
    ["email", "mail", "userPrincipalName"]
        .iter()
        .find_map(|k| v.get(*k).and_then(Value::as_str).map(String::from))
        .unwrap_or_default()
}

static TOKENS: Mutex<Option<HashMap<String, (String, Instant)>>> = Mutex::new(None);

/// A current access token for the account (refreshed when needed).
/// Uses a refresh token: (access token, seconds valid, a new refresh token if given).
pub fn refresh_with(provider: &str, client_id: &str, client_secret: &str, tenant: &str, refresh: &str, r: &Resource) -> Result<(String, u64, Option<String>), String> {
    let scope = scopes(provider, r);
    let mut body = vec![("client_id", client_id), ("grant_type", "refresh_token"), ("refresh_token", refresh)];
    if provider == MICROSOFT {
        body.push(("scope", scope));
    }
    if !client_secret.is_empty() {
        body.push(("client_secret", client_secret));
    }
    let basic = basic_for(provider, client_id, client_secret);
    let v = post_form(&token_url(provider, tenant), &form(&body), basic.as_ref().map(|(a, b)| (a.as_str(), b.as_str())))?;
    let tok = v.get("access_token").and_then(Value::as_str).ok_or("No access token")?.to_string();
    let secs = v.get("expires_in").and_then(|x| x.as_u64().or_else(|| x.as_str().and_then(|s| s.parse().ok()))).unwrap_or(3000);
    let new_refresh = v.get("refresh_token").and_then(Value::as_str).filter(|n| *n != refresh).map(String::from);
    Ok((tok, secs, new_refresh))
}

/// A current access token for the account (refreshed when needed).
pub fn access_token(account_id: &str, provider: &str, client_id: &str, client_secret: &str, tenant: &str, r: Resource) -> Result<String, String> {
    let key = format!("{account_id}:{}", match r { Resource::Mail => "mail", Resource::Data => "data" });
    if let Some((tok, until)) = TOKENS.lock().unwrap().as_ref().and_then(|m| m.get(&key)).cloned() {
        if Instant::now() < until {
            return Ok(tok);
        }
    }
    let refresh = crate::live::password(&format!("{account_id}:refresh")).ok_or("Sign in again in Accounts (the sign-in has expired).")?;
    let (tok, secs, new_refresh) = refresh_with(provider, client_id, client_secret, tenant, &refresh, &r)?;
    if let Some(n) = new_refresh {
        let _ = crate::live::save_password(&format!("{account_id}:refresh"), &n);
    }
    TOKENS
        .lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .insert(key, (tok.clone(), Instant::now() + Duration::from_secs(secs.saturating_sub(120).max(60))));
    Ok(tok)
}

/// Drop cached access tokens for an account (after signing in again).
pub fn forget_tokens(account_id: &str) {
    if let Some(m) = TOKENS.lock().unwrap().as_mut() {
        m.retain(|k, _| !k.starts_with(&format!("{account_id}:")));
    }
}

/// IMAP XOAUTH2 sign-in.
pub struct XOAuth2 {
    pub user: String,
    pub token: String,
}

impl imap::Authenticator for XOAuth2 {
    type Response = String;
    fn process(&self, _: &[u8]) -> Self::Response {
        format!("user={}\x01auth=Bearer {}\x01\x01", self.user, self.token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::sync::Arc;

    /// A tiny stand-in sign-in service: checks PKCE and hands out tokens.
    fn fake_provider(challenge: Arc<Mutex<String>>) -> String {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
        std::thread::spawn(move || {
            for stream in l.incoming().flatten() {
                let mut s = stream;
                // Read the whole request (headers, then Content-Length bytes of body).
                let mut data: Vec<u8> = Vec::new();
                let mut buf = [0u8; 4096];
                loop {
                    let n = s.read(&mut buf).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    data.extend_from_slice(&buf[..n]);
                    let text = String::from_utf8_lossy(&data).to_string();
                    if let Some((head, rest)) = text.split_once("\r\n\r\n") {
                        let len: usize = head
                            .lines()
                            .find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap_or(0)))
                            .unwrap_or(0);
                        if rest.len() >= len {
                            break;
                        }
                    }
                }
                let req = String::from_utf8_lossy(&data).to_string();
                let body = req.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
                let f: HashMap<String, String> = body
                    .split('&')
                    .filter_map(|kv| kv.split_once('=').map(|(k, v)| (k.to_string(), dec(v))))
                    .collect();
                let json = match f.get("grant_type").map(String::as_str) {
                    Some("authorization_code") => {
                        let ok = f.get("code").map(String::as_str) == Some("the-code")
                            && pkce_challenge(f.get("code_verifier").map(String::as_str).unwrap_or("")) == *challenge.lock().unwrap();
                        if ok {
                            let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"email":"tester@example.com"}"#);
                            format!(r#"{{"access_token":"at1","refresh_token":"rt1","expires_in":3600,"id_token":"h.{payload}.s"}}"#)
                        } else {
                            r#"{"error":"invalid_grant"}"#.to_string()
                        }
                    }
                    Some("refresh_token") if f.get("refresh_token").map(String::as_str) == Some("rt1") => {
                        r#"{"access_token":"at2","expires_in":"3599","refresh_token":"rt2"}"#.to_string()
                    }
                    _ => r#"{"error":"invalid_request"}"#.to_string(),
                };
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}", json.len());
            }
        });
        base
    }

    fn query_of(url: &str) -> HashMap<String, String> {
        url.split_once('?').map(|(_, q)| q).unwrap_or("").split('&').filter_map(|kv| kv.split_once('=').map(|(k, v)| (k.to_string(), dec(v)))).collect()
    }

    #[test]
    fn sign_in_round_trip_with_stand_in_provider() {
        let challenge = Arc::new(Mutex::new(String::new()));
        std::env::set_var("ALMANAC_OAUTH_TEST_BASE", fake_provider(challenge.clone()));
        let ch = challenge.clone();
        // The "browser": goes straight back to Almanac with a code.
        let open = move |url: String| {
            let q = query_of(&url);
            assert_eq!(q.get("code_challenge_method").map(String::as_str), Some("S256"));
            *ch.lock().unwrap() = q["code_challenge"].clone();
            let back = format!("{}/?code=the-code&state={}", q["redirect_uri"], enc(&q["state"]));
            std::thread::spawn(move || {
                let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).build().into();
                let _ = agent.get(&back).call();
            });
        };
        let si = sign_in(TEST, "client-1", "", "", &open).expect("sign in");
        assert_eq!(si.refresh_token, "rt1");
        assert_eq!(si.email, "tester@example.com");
        let (tok, secs, new_refresh) = refresh_with(TEST, "client-1", "", "", "rt1", &Resource::Mail).expect("refresh");
        assert_eq!(tok, "at2");
        assert_eq!(secs, 3599);
        assert_eq!(new_refresh.as_deref(), Some("rt2"));
        assert!(refresh_with(TEST, "client-1", "", "", "wrong", &Resource::Mail).is_err());

        // Pasting the code by hand works too.
        let ch = challenge.clone();
        let open2 = move |url: String| {
            *ch.lock().unwrap() = query_of(&url)["code_challenge"].clone();
            std::thread::spawn(|| {
                std::thread::sleep(Duration::from_millis(300));
                paste_code("http://localhost/?code=the-code&state=x");
            });
        };
        let si = sign_in(TEST, "client-1", "", "", &open2).expect("sign in by pasted code");
        assert_eq!(si.refresh_token, "rt1");
    }
}
