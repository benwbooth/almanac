//! Registering Almanac with Google / Microsoft with as little clicking as
//! the providers allow:
//! - Microsoft: fully automatic through the Azure command-line tool (`az`),
//!   after one browser sign-in.
//! - Google: the project and services are made with `gcloud`; Google only
//!   lets the final "create client" step happen on its web page.
//! - Any provider: instructions an AI assistant can follow, and picking the
//!   IDs out of whatever text or file comes back.

use regex::Regex;
use serde_json::Value;
use std::process::Command;

/// Finds a program on PATH (with .exe/.cmd on Windows).
pub fn find_tool(name: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    let exts: &[&str] = if cfg!(windows) { &[".exe", ".cmd", ".bat", ""] } else { &[""] };
    for dir in std::env::split_paths(&path) {
        for ext in exts {
            let p = dir.join(format!("{name}{ext}"));
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// How a tool can be run here: directly, or fetched on demand with Nix.
fn runner(tool: &str, nix_pkg: &str) -> Option<Vec<String>> {
    if let Some(p) = find_tool(tool) {
        return Some(vec![p.display().to_string()]);
    }
    if find_tool("nix-shell").is_some() {
        return Some(vec!["nix-shell".into(), "-p".into(), nix_pkg.into(), "--run".into()]);
    }
    None
}

/// Runs `tool args…` and returns its output (error text on failure).
fn run(base: &[String], args: &[&str]) -> Result<String, String> {
    let mut cmd;
    if base.first().map(|s| s == "nix-shell").unwrap_or(false) {
        // nix-shell -p pkg --run "tool 'arg' 'arg'…"
        let tool = if base[2] == "azure-cli" { "az" } else { "gcloud" };
        let line = std::iter::once(tool.to_string())
            .chain(args.iter().map(|a| format!("'{}'", a.replace('\'', "'\\''"))))
            .collect::<Vec<_>>()
            .join(" ");
        cmd = Command::new(&base[0]);
        cmd.args(&base[1..]).arg(line);
    } else {
        cmd = Command::new(&base[0]);
        cmd.args(args);
    }
    let out = cmd.output().map_err(|e| format!("Could not start {}: {e}", base[0]))?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if out.status.success() {
        Ok(stdout)
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if err.is_empty() { stdout } else { err })
    }
}

pub fn tools_json() -> Value {
    serde_json::json!({
        "az": find_tool("az").is_some(),
        "gcloud": find_tool("gcloud").is_some(),
        "nix": find_tool("nix-shell").is_some(),
    })
}

/// Microsoft: signs in to Azure in the browser (only if not signed in yet),
/// finds or creates the "Almanac" app registration, makes sure its settings
/// are right, and returns its client ID. Safe to run again: it reuses the
/// registration it made before instead of making another one.
pub fn register_microsoft(progress: &dyn Fn(String)) -> Result<String, String> {
    let az = runner("az", "azure-cli").ok_or("Needs the Azure CLI (az). Install it from https://aka.ms/azcli and try again.")?;
    if run(&az, &["account", "show", "--output", "none"]).is_err() {
        progress("Sign in to Microsoft in the browser window that opens…".into());
        run(&az, &["login", "--allow-no-subscriptions", "--only-show-errors", "--output", "none"])
            .map_err(|e| format!("Microsoft sign-in did not finish: {e}"))?;
    }
    progress("Looking for an existing Almanac registration…".into());
    let found = run(&az, &["ad", "app", "list", "--display-name", "Almanac", "--query", "[0].{appId: appId, id: id}", "--output", "json"])
        .unwrap_or_default();
    let existing: Value = serde_json::from_str(found.trim()).unwrap_or(Value::Null);
    let (app_id, object_id) = match (existing.get("appId").and_then(Value::as_str), existing.get("id").and_then(Value::as_str)) {
        (Some(a), Some(o)) => {
            progress("Found it; checking its settings…".into());
            let _ = run(
                &az,
                &["ad", "app", "update", "--id", a, "--public-client-redirect-uris", "http://localhost", "--is-fallback-public-client", "true"],
            );
            (a.to_string(), o.to_string())
        }
        _ => {
            progress("Creating the Almanac app registration…".into());
            let created = run(
                &az,
                &[
                    "ad", "app", "create",
                    "--display-name", "Almanac",
                    "--public-client-redirect-uris", "http://localhost",
                    "--is-fallback-public-client", "true",
                    "--query", "{appId: appId, id: id}",
                    "--output", "json",
                ],
            )
            .map_err(|e| format!("Could not create the registration: {e}"))?;
            let v: Value = serde_json::from_str(&created).map_err(|_| format!("Unexpected answer from az: {created}"))?;
            let a = v.get("appId").and_then(Value::as_str).ok_or("No app ID in the answer")?.to_string();
            let o = v.get("id").and_then(Value::as_str).unwrap_or("").to_string();
            (a, o)
        }
    };
    // Let personal Outlook.com / Hotmail accounts sign in too (needs v2 tokens first).
    progress("Allowing personal Microsoft accounts…".into());
    if !object_id.is_empty() {
        let body = r#"{"api":{"requestedAccessTokenVersion":2},"signInAudience":"AzureADandPersonalMicrosoftAccount"}"#;
        let uri = format!("https://graph.microsoft.com/v1.0/applications/{object_id}");
        if let Err(e) = run(&az, &["rest", "--method", "PATCH", "--uri", &uri, "--headers", "Content-Type=application/json", "--body", body]) {
            progress(format!("Work and school accounts are ready; personal accounts could not be turned on ({e})."));
        }
    }
    Ok(app_id)
}

/// Google: signs in to Google Cloud (only if needed), finds or makes the
/// "Almanac" project and turns on Gmail, Calendar and People (turning on
/// again is harmless). Returns the project id. Safe to run again.
pub fn prepare_google(known: &str, progress: &dyn Fn(String)) -> Result<String, String> {
    let gc = runner("gcloud", "google-cloud-sdk").ok_or("Needs the Google Cloud CLI (gcloud). Install it from https://cloud.google.com/sdk and try again.")?;
    let signed_in = run(&gc, &["auth", "list", "--filter=status:ACTIVE", "--format=value(account)"]).unwrap_or_default();
    if signed_in.trim().is_empty() {
        progress("Sign in to Google in the browser window that opens…".into());
        run(&gc, &["auth", "login", "--brief"]).map_err(|e| format!("Google sign-in did not finish: {e}"))?;
    }
    progress("Looking for an existing Almanac project…".into());
    let mut project = String::new();
    if !known.is_empty() && run(&gc, &["projects", "describe", known, "--format=value(projectId)"]).is_ok() {
        project = known.to_string();
    }
    if project.is_empty() {
        project = run(&gc, &["projects", "list", "--filter=name:Almanac AND lifecycleState:ACTIVE", "--format=value(projectId)", "--limit=1"])
            .unwrap_or_default()
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
    }
    if project.is_empty() {
        project = format!("almanac-{}", chrono::Utc::now().timestamp() % 1_000_000_000);
        progress(format!("Creating the Google Cloud project {project}…"));
        run(&gc, &["projects", "create", &project, "--name=Almanac", "--quiet"]).map_err(|e| format!("Could not create the project: {e}"))?;
    } else {
        progress(format!("Using your existing project {project}…"));
    }
    progress("Turning on Gmail, Calendar, Contacts and Drive for it…".into());
    run(&gc, &["services", "enable", "gmail.googleapis.com", "calendar-json.googleapis.com", "people.googleapis.com", "drive.googleapis.com", "--project", &project])
        .map_err(|e| format!("Could not turn on the services: {e}"))?;
    Ok(project)
}

/// The two Google pages that finish the job for a prepared project.
pub fn google_pages(project: &str) -> (String, String) {
    (
        format!("https://console.cloud.google.com/auth/overview/create?project={project}"),
        format!("https://console.cloud.google.com/auth/clients/create?project={project}"),
    )
}

/// Instructions for an AI assistant that can use a web browser.
pub fn ai_prompt(provider: &str) -> String {
    let task = match provider {
        crate::oauth::GOOGLE => "In the Google Cloud console (console.cloud.google.com), using my signed-in Google account:\n\
1. Create a project named \"Almanac\".\n\
2. Enable these APIs for it: Gmail API, Google Calendar API, People API.\n\
3. Set up the OAuth consent screen (Google Auth Platform): app name \"Almanac\", user type External, my email as support and developer contact. Then on the Audience page leave it in Testing (do NOT press Publish app; that needs a homepage and privacy policy) and under Test users add my Gmail address (ask me for it if you don't know it), then Save.\n\
4. Create an OAuth client ID of type \"Desktop app\" named \"Almanac\".",
        crate::oauth::MICROSOFT => "In the Microsoft Entra admin center (entra.microsoft.com), using my signed-in Microsoft account:\n\
1. App registrations › New registration. Name \"Almanac\".\n\
2. Supported account types: \"Accounts in any organizational directory and personal Microsoft accounts\".\n\
3. Redirect URI: platform \"Public client/native (mobile & desktop)\", value http://localhost. Register.\n\
4. Under Authentication, turn on \"Allow public client flows\" and save.",
        crate::oauth::YAHOO => "On the Yahoo developer site (developer.yahoo.com/apps), using my signed-in Yahoo account:\n\
1. Create an app named \"Almanac\", type \"Installed Application\", redirect URI \"oob\".\n\
2. Give it the Mail API permission with Read/Write, and OpenID Connect Email.",
        _ => "On the AOL developer site (developer.aol.com/apps), using my signed-in AOL account:\n\
1. Create an app named \"Almanac\", type \"Installed Application\", redirect URI \"oob\".\n\
2. Give it the Mail API permission with Read/Write, and OpenID Connect Email.",
    };
    format!(
        "Please register my desktop mail app \"Almanac\" so it can sign in with OAuth. Do this in my web browser.\n\n{task}\n\n\
When you're done, reply with exactly these lines and nothing secret besides them:\n\
client_id=<the client ID>\n\
client_secret=<the client secret, or leave empty if there is none>\n"
    )
}

/// Picks the client ID and secret out of pasted text: an AI's answer,
/// copied page text, or Google's downloaded client_secret JSON file.
pub fn extract_ids(provider: &str, text: &str) -> Option<(String, String)> {
    let t = text.trim();
    // A JSON file (Google's download, or our own format).
    if let Ok(v) = serde_json::from_str::<Value>(t) {
        for path in ["/installed", "/web", "", &format!("/{provider}")] {
            if let Some(id) = v.pointer(&format!("{path}/client_id")).and_then(Value::as_str) {
                let secret = v.pointer(&format!("{path}/client_secret")).and_then(Value::as_str).unwrap_or("");
                return Some((id.to_string(), secret.to_string()));
            }
        }
    }
    let kv = |key: &str| -> Option<String> {
        let re = Regex::new(&format!(r#"(?i)\b{key}\b["']?\s*[:=]\s*["']?([A-Za-z0-9._~+/=\-]+)"#)).ok()?;
        re.captures(t).map(|c| c[1].to_string()).filter(|s| !s.starts_with('<'))
    };
    let mut id = kv("client[_ ]?id").or_else(|| kv("application \\(client\\) id"));
    let mut secret = kv("client[_ ]?secret").unwrap_or_default();
    match provider {
        crate::oauth::GOOGLE => {
            if id.as_deref().map(|i| !i.ends_with(".apps.googleusercontent.com")).unwrap_or(true) {
                id = Regex::new(r"\b\d+-[a-z0-9]+\.apps\.googleusercontent\.com\b").ok()?.find(t).map(|m| m.as_str().to_string()).or(id);
            }
            if secret.is_empty() {
                secret = Regex::new(r"GOCSPX-[A-Za-z0-9_\-]+").ok()?.find(t).map(|m| m.as_str().to_string()).unwrap_or_default();
            }
        }
        crate::oauth::MICROSOFT => {
            let guid = Regex::new(r"(?i)\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b").ok()?;
            if id.as_deref().map(|i| !guid.is_match(i)).unwrap_or(true) {
                id = guid.find(t).map(|m| m.as_str().to_string());
            }
        }
        _ => {}
    }
    id.filter(|i| i.len() >= 8).map(|i| (i, secret))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_ids_out_of_text() {
        let g = r#"{"installed":{"client_id":"123-abc.apps.googleusercontent.com","client_secret":"GOCSPX-xyz_1","redirect_uris":["http://localhost"]}}"#;
        assert_eq!(extract_ids("google", g), Some(("123-abc.apps.googleusercontent.com".into(), "GOCSPX-xyz_1".into())));
        let ai = "Done! Here you go:\nclient_id=987-qwe.apps.googleusercontent.com\nclient_secret=GOCSPX-AbC-9\n";
        assert_eq!(extract_ids("google", ai), Some(("987-qwe.apps.googleusercontent.com".into(), "GOCSPX-AbC-9".into())));
        let ms = "Overview\nApplication (client) ID\n3f2504e0-4f89-11d3-9a0c-0305e82c3301\nDirectory (tenant) ID\n";
        assert_eq!(extract_ids("microsoft", ms).map(|x| x.0), Some("3f2504e0-4f89-11d3-9a0c-0305e82c3301".into()));
        let y = "client_id=dj0yJmk9aGVsbG8mZD1YV2s5 \nclient_secret=0123456789abcdef0123456789abcdef01234567";
        assert_eq!(extract_ids("yahoo", y).map(|x| x.1.len()), Some(40));
        assert_eq!(extract_ids("microsoft", "nothing useful here"), None);
    }

    /// Runs the Microsoft automation against a stand-in `az` program.
    #[cfg(unix)]
    #[test]
    fn microsoft_automation_with_stand_in_az() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("almanac-fake-az-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("calls.log");
        let script = format!(
            "#!/bin/sh\necho \"$@\" >> '{}'\ncase \"$1 $2 $3\" in\n  'ad app create') echo '{{\"appId\":\"11111111-2222-3333-4444-555555555555\",\"id\":\"obj-1\"}}' ;;\n  'ad app list') if [ -f '{}.made' ]; then echo '{{\"appId\":\"11111111-2222-3333-4444-555555555555\",\"id\":\"obj-1\"}}'; else echo null; touch '{}.made'; fi ;;\n  'account show '*|'account show') exit 1 ;;\nesac\nexit 0\n",
            log.display(),
            log.display(),
            log.display()
        );
        let az = dir.join("az");
        std::fs::write(&az, script).unwrap();
        std::fs::set_permissions(&az, std::fs::Permissions::from_mode(0o755)).unwrap();
        let old = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = vec![dir.clone()];
        paths.extend(std::env::split_paths(&old));
        std::env::set_var("PATH", std::env::join_paths(paths).unwrap());
        let id = register_microsoft(&|_| {}).expect("register");
        // Running it again reuses the same registration.
        let again = register_microsoft(&|_| {}).expect("register again");
        std::env::set_var("PATH", old);
        assert_eq!(id, "11111111-2222-3333-4444-555555555555");
        assert_eq!(again, id);
        let calls = std::fs::read_to_string(&log).unwrap();
        assert!(calls.contains("login"));
        assert!(calls.contains("--public-client-redirect-uris http://localhost"));
        assert_eq!(calls.matches("ad app create").count(), 1, "made a second registration");
        assert!(calls.contains("ad app update --id 11111111-2222-3333-4444-555555555555"));
        assert!(calls.contains("rest --method PATCH --uri https://graph.microsoft.com/v1.0/applications/obj-1"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
