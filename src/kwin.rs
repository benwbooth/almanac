//! KDE Plasma on Wayland: apps can't place their own windows there (the
//! desktop does). Plasma's own way to have a window come back where it was is
//! a window rule set to "Remember" its position (System Settings › Window
//! Management › Window Rules); Almanac adds that rule for its main window
//! once, and KWin does the rest. (Its size Almanac sets itself, as Wayland
//! apps do.) Elsewhere (X11, Windows, macOS) Qt places the window itself.

#[cfg(target_os = "linux")]
use std::sync::Mutex;

/// The rule's name in KWin's rules (kwinrulesrc).
const RULE: &str = "almanac-main-window";

/// Plasma on Wayland (where this is needed and KWin can do it).
pub fn wanted() -> bool {
    cfg!(target_os = "linux")
        && std::env::var_os("WAYLAND_DISPLAY").is_some()
        && std::env::var("XDG_CURRENT_DESKTOP").map(|d| d.to_uppercase().contains("KDE")).unwrap_or(false)
        && std::env::var("QT_QPA_PLATFORM").map(|p| p.is_empty() || p.starts_with("wayland")).unwrap_or(true)
}

/// KWin's rules file (ALMANAC_KWIN_CONFIG: the folder it's in, for checks run
/// with a config folder of their own).
fn rules_file() -> std::path::PathBuf {
    let dir = std::env::var_os("ALMANAC_KWIN_CONFIG")
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"));
    dir.join("kwinrulesrc")
}

/// KWin's rules with Almanac's added (None: it's there already, and stays as
/// it is: KWin keeps the remembered place in it). `seed`: where the window
/// was last time, if known, so the first start with the rule comes back there.
fn with_rule(text: &str, seed: Option<(f64, f64)>, maximized: bool) -> Option<String> {
    let mut sections: Vec<(String, Vec<String>)> = vec![(String::new(), Vec::new())];
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            sections.push((t[1..t.len() - 1].to_string(), Vec::new()));
        } else {
            sections.last_mut().unwrap().1.push(line.to_string());
        }
    }
    if let Some((_, lines)) = sections.iter_mut().find(|(name, _)| name == RULE) {
        // (The first version also had KWin remember the size, which it doesn't
        // give back to Almanac's window: Almanac sizes it itself.)
        if !lines.iter().any(|l| l.starts_with("sizerule=")) {
            return None;
        }
        lines.retain(|l| !l.starts_with("size=") && !l.starts_with("sizerule="));
        return Some(join(&sections));
    }
    if !sections.iter().any(|(name, _)| name == "General") {
        sections.insert(1, ("General".into(), Vec::new()));
    }
    let general = sections.iter_mut().find(|(name, _)| name == "General").unwrap();
    let rules_now = general.1.iter().find_map(|l| l.strip_prefix("rules=").map(str::to_string)).unwrap_or_default();
    let mut rules: Vec<String> = rules_now.split(',').map(str::trim).filter(|r| !r.is_empty()).map(String::from).collect();
    rules.push(RULE.into());
    general.1.retain(|l| !l.starts_with("rules=") && !l.starts_with("count="));
    while general.1.last().is_some_and(|l| l.trim().is_empty()) {
        general.1.pop();
    }
    general.1.insert(0, format!("rules={}", rules.join(",")));
    general.1.insert(0, format!("count={}", rules.len()));
    general.1.push(String::new());
    let mut rule = vec!["Description=Almanac: its main window back where it was".to_string(), "Enabled=true".into()];
    if let Some((x, y)) = seed {
        rule.push(format!("position={},{}", x.round() as i64, y.round() as i64));
    }
    rule.push("positionrule=4".into());
    if maximized {
        rule.push("maximizehoriz=true".into());
        rule.push("maximizevert=true".into());
    }
    rule.push("maximizehorizrule=4".into());
    rule.push("maximizevertrule=4".into());
    // (Only the main window: written and opened messages have windows of their own.)
    rule.push("title=^(Mail|Calendar|Contacts) — Almanac$".into());
    rule.push("titlematch=3".into());
    rule.push("types=1".into());
    rule.push("wmclass=almanac".into());
    rule.push("wmclasscomplete=false".into());
    rule.push("wmclassmatch=1".into());
    if let Some(last) = sections.last_mut() {
        if last.1.last().is_some_and(|l| !l.trim().is_empty()) {
            last.1.push(String::new());
        }
    }
    sections.push((RULE.into(), rule));
    Some(join(&sections))
}

fn join(sections: &[(String, Vec<String>)]) -> String {
    let mut out = String::new();
    for (i, (name, lines)) in sections.iter().enumerate() {
        if i > 0 {
            out.push_str(&format!("[{name}]\n"));
        }
        for l in lines {
            out.push_str(l);
            out.push('\n');
        }
    }
    out
}

/// Has KWin remember the main window's place (adds the rule if it isn't
/// there yet, and has KWin read its rules again). False where it can't.
pub fn remember_place(seed: Option<(f64, f64)>, maximized: bool) -> bool {
    let path = rules_file();
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    if let Some(new) = with_rule(&text, seed, maximized) {
        if std::fs::write(&path, new).is_err() {
            return false;
        }
        #[cfg(target_os = "linux")]
        if let Some(conn) = connection() {
            let _ = conn.call_method(Some("org.kde.KWin"), "/KWin", Some("org.kde.KWin"), "reconfigure", &());
        }
    }
    forget_old_scripts();
    true
}

/// Window scripts earlier versions of Almanac gave KWin: taken out again.
fn forget_old_scripts() {
    let Ok(dir) = std::fs::read_dir(crate::data::data_dir()) else { return };
    for f in dir.flatten() {
        let name = f.file_name().to_string_lossy().to_string();
        if name.starts_with("almanac-window-") && name.ends_with(".js") {
            #[cfg(target_os = "linux")]
            if let Some(conn) = connection() {
                let _ = conn.call_method(Some("org.kde.KWin"), "/Scripting", Some("org.kde.kwin.Scripting"), "unloadScript", &(name.trim_end_matches(".js"),));
            }
            let _ = std::fs::remove_file(f.path());
        }
    }
}

#[cfg(target_os = "linux")]
static CONNECTION: Mutex<Option<zbus::blocking::Connection>> = Mutex::new(None);

#[cfg(target_os = "linux")]
fn connection() -> Option<zbus::blocking::Connection> {
    let mut held = CONNECTION.lock().unwrap();
    if held.is_none() {
        *held = zbus::blocking::Connection::session().ok();
    }
    held.clone()
}

/// Brings the main window up on the desktop you're on: un-minimised, moved
/// to the current virtual desktop and made active. (Plasma ignores an app's
/// own "activate me" unless it comes with a click's permission, which the
/// tray and a second start don't carry.) False when KWin can't be asked.
#[cfg(target_os = "linux")]
pub fn bring_forward() -> bool {
    let Some(conn) = connection() else { return false };
    let pid = std::process::id();
    let name = format!("almanac-raise-{pid}");
    let path = crate::data::data_dir().join(format!("{name}.js"));
    let js = format!(
        r#"// Bring Almanac's main window to the current desktop and make it active.
for (const w of workspace.windowList()) {{
    if (w.pid !== {pid} || !/^(Mail|Calendar|Contacts) — Almanac$/.test(w.caption)) continue;
    w.minimized = false;
    if (!w.onAllDesktops) w.desktops = [workspace.currentDesktop];
    workspace.activeWindow = w;
}}
"#
    );
    if std::fs::write(&path, js).is_err() {
        return false;
    }
    let (kwin, scripting) = (Some("org.kde.KWin"), Some("org.kde.kwin.Scripting"));
    let _ = conn.call_method(kwin, "/Scripting", scripting, "unloadScript", &(name.as_str(),));
    let ok = conn
        .call_method(kwin, "/Scripting", scripting, "loadScript", &(path.display().to_string(), name.as_str()))
        .ok()
        .and_then(|r| r.body().deserialize::<i32>().ok())
        .filter(|id| *id >= 0)
        // (Started through KWin's "start the loaded scripts", not the script's
        // numbered object: KWin reuses the number of one just taken out.)
        .map(|_| conn.call_method(kwin, "/Scripting", scripting, "start", &()).is_ok())
        .unwrap_or(false);
    let _ = conn.call_method(kwin, "/Scripting", scripting, "unloadScript", &(name.as_str(),));
    let _ = std::fs::remove_file(&path);
    ok
}

#[cfg(not(target_os = "linux"))]
pub fn bring_forward() -> bool {
    false
}

#[cfg(test)]
mod tests {
    #[test]
    fn rule_is_added_once_beside_others() {
        let theirs = "[General]\ncount=1\nrules=kog-main-window\n\n[kog-main-window]\nDescription=Kog\npositionrule=4\nwmclass=org.kog.player\n";
        let out = super::with_rule(theirs, Some((300.0, 200.0)), false).unwrap();
        assert!(out.starts_with("[General]\ncount=2\nrules=kog-main-window,almanac-main-window\n\n[kog-main-window]\n"), "{out}");
        assert!(out.contains("wmclass=org.kog.player\n\n[almanac-main-window]\n"), "{out}");
        assert!(out.contains("position=300,200\npositionrule=4\n"));
        assert!(!out.contains("sizerule"));
        assert!(out.contains("wmclass=almanac\n"));
        // Already there: left alone (KWin keeps the remembered place in it).
        assert!(super::with_rule(&out, None, false).is_none());
        // No rules yet.
        let fresh = super::with_rule("", None, true).unwrap();
        assert!(fresh.starts_with("[General]\ncount=1\nrules=almanac-main-window\n\n[almanac-main-window]\n"), "{fresh}");
        assert!(fresh.contains("maximizehoriz=true\nmaximizevert=true\n"));
        // The first version's size part: taken out.
        let first = out.replace("position=300,200\npositionrule=4\n", "position=300,200\npositionrule=4\nsize=1000,700\nsizerule=4\n");
        let fixed = super::with_rule(&first, None, false).unwrap();
        assert_eq!(fixed, out);
    }
}
