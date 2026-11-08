//! Mail, Calendar and Contacts in one desktop app.
//! Rust holds the data; the window is drawn with Qt Quick through cxx-qt.
//! Runs on Linux first, and builds on Windows and macOS with a Qt 6 install.

// Release builds on Windows open no console window next to the app.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod data;
mod autoreg;
mod browser;
mod cloud;
mod codeblock;
mod dav;
mod discover;
mod ews;
mod extras;
mod cloudfiles;
mod gist;
mod pgp;
mod smime;
mod unsubscribe;
mod ics;
mod imap_pipe;
mod instance;
mod kwin;
mod live;
mod mailstore;
#[cfg(test)]
mod contrast;
mod mime;
mod oauth;
mod pop;
mod richtext;
mod darkcolours;
mod spelling;
mod sources;
mod tz;
pub mod store;

use cxx_qt_lib::{QQmlApplicationEngine, QString, QUrl};
use std::sync::OnceLock;

static SCREENSHOT: OnceLock<Option<String>> = OnceLock::new();

/// The app's icons (built into the program), written beside its data so Qt
/// and the desktop can load them: the window/taskbar icon and the tray icons
/// (plain, and with a blue dot for unread mail).
const ICONS: [(&str, &str); 3] = [
    ("almanac", include_str!("../icons/almanac.svg")),
    ("almanac-tray", include_str!("../icons/almanac-tray.svg")),
    ("almanac-tray-unread", include_str!("../icons/almanac-tray-unread.svg")),
];

/// Where an icon is on disk (written there by `install_icons`).
pub fn icon_path(name: &str) -> std::path::PathBuf {
    data::data_dir().join("icons").join(format!("{name}.svg"))
}

fn install_icons() {
    let _ = std::fs::create_dir_all(data::data_dir().join("icons"));
    let write = |name: &str, svg: &str| {
        let path = icon_path(name);
        if std::fs::read_to_string(&path).map(|old| old != svg).unwrap_or(true) {
            let _ = std::fs::write(&path, svg);
        }
    };
    for (name, svg) in ICONS {
        if name.starts_with("almanac-tray") {
            // One colour, like other tray icons: light on a dark panel, dark on a
            // light one (Breeze's own text colours).
            write(&format!("{name}-dark"), &svg.replace("INK", "#fcfcfc").replace("PANEL", "#2a2e32"));
            write(&format!("{name}-light"), &svg.replace("INK", "#232629").replace("PANEL", "#eff0f1"));
        } else {
            write(name, svg);
        }
    }
    // Also where the desktop's icon lookup finds it by name ("almanac"), for
    // menus and dialogs that ask for icons by name.
    let themed = data::data_dir().join("icons/hicolor/scalable/apps");
    if std::fs::create_dir_all(&themed).is_ok() {
        if let Some((_, svg)) = ICONS.iter().find(|(n, _)| *n == "almanac") {
            if std::fs::read_to_string(themed.join("almanac.svg")).map(|old| old != *svg).unwrap_or(true) {
                let _ = std::fs::write(themed.join("almanac.svg"), svg);
            }
        }
    }
}

/// File to save a picture of the window to, when started with `--screenshot <file.png>`.
pub fn screenshot_path() -> Option<String> {
    SCREENSHOT.get().cloned().flatten()
}

/// Which app to open first: `--open mail|calendar|contacts`.
pub fn start_section() -> i32 {
    let args: Vec<String> = std::env::args().collect();
    match args
        .iter()
        .position(|a| a == "--open")
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
    {
        Some("calendar") => 1,
        Some("contacts") => 2,
        _ => 0,
    }
}

/// The word after a command-line option, e.g. `--show accounts`.
pub fn arg_value(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

/// `--theme light|dark|system` overrides the saved appearance for this run.
pub fn theme_arg() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == "--theme")
        .and_then(|i| args.get(i + 1))
        .filter(|t| ["light", "dark", "system"].contains(&t.as_str()))
        .cloned()
}

/// ALMANAC_OFFLINE set: no mail checks and nothing sent (for timing or
/// checking a copy of real data without touching the accounts).
pub fn offline() -> bool {
    std::env::var_os("ALMANAC_OFFLINE").is_some()
}

/// How long starting took so far, written out when ALMANAC_TIMING is set.
pub fn startup_mark(what: &str) {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let start = START.get_or_init(std::time::Instant::now);
    if std::env::var_os("ALMANAC_TIMING").is_some() {
        eprintln!("almanac start: {:>6} ms  {what}", start.elapsed().as_millis());
    }
}

/// Notes calls that take long (over 50 ms) when ALMANAC_TIMING is set.
pub struct Timed(&'static str, std::time::Instant);
impl Timed {
    pub fn new(what: &'static str) -> Timed {
        Timed(what, std::time::Instant::now())
    }
}
impl Drop for Timed {
    fn drop(&mut self) {
        let took = self.1.elapsed();
        // ALMANAC_TIMING=<ms> sets how long counts as long (50 by default).
        let limit = std::env::var("ALMANAC_TIMING").ok().and_then(|v| v.parse::<u128>().ok()).unwrap_or(50);
        // (Over a second is always noted in the log: the window was frozen that long.)
        if (took.as_millis() > limit && std::env::var_os("ALMANAC_TIMING").is_some()) || took.as_millis() > 1000 {
            eprintln!("almanac slow: {:>6} ms  {}", took.as_millis(), self.0);
        }
    }
}

fn main() {
    startup_mark("started");
    let args: Vec<String> = std::env::args().collect();
    let shot = args
        .iter()
        .position(|a| a == "--screenshot")
        .and_then(|i| args.get(i + 1))
        .map(|p| {
            let path = std::path::Path::new(p);
            let abs = if path.is_absolute() {
                path.to_path_buf()
            } else {
                std::env::current_dir().unwrap_or_default().join(path)
            };
            if let Some(dir) = abs.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            abs.display().to_string()
        });
    SCREENSHOT.set(shot.clone()).ok();

    // Same look on every platform (desktop themes such as Breeze would
    // otherwise restyle the text fields). CRAFT_PIM_STYLE picks another one.
    let style = std::env::var("CRAFT_PIM_STYLE").unwrap_or_else(|_| "Basic".into());
    std::env::set_var("QT_QUICK_CONTROLS_STYLE", style);
    // Scroll bars in pages and PDFs that show only while scrolling or pointed at
    // (the browser engine's overlay ones), as on a Mac. ALMANAC_CLASSIC_SCROLLBARS
    // keeps the usual ones.
    if std::env::var_os("ALMANAC_CLASSIC_SCROLLBARS").is_none() {
        let flags = std::env::var("QTWEBENGINE_CHROMIUM_FLAGS").unwrap_or_default();
        if !flags.contains("FluentOverlayScrollbar") {
            std::env::set_var("QTWEBENGINE_CHROMIUM_FLAGS", format!("{flags} --enable-features=FluentOverlayScrollbar").trim());
        }
    }
    // The message pane could stay black after the window came back to the front,
    // until clicked: Chromium treats a page in a window that's covered, in the
    // desktop's overview or not focused as in the background and stops drawing
    // it, and on Wayland it doesn't always start again until there's input.
    // Keep it drawing (the page is small; this costs next to nothing).
    {
        let flags = std::env::var("QTWEBENGINE_CHROMIUM_FLAGS").unwrap_or_default();
        let mut add = Vec::new();
        // (And mail pages composited on the processor, handed over as pictures
        // rather than a graphics surface shared with Qt that could be lost while
        // the window was hidden, leaving the pane black; cheap for mail. Qt's own
        // drawing keeps the graphics card.)
        for f in ["--disable-renderer-backgrounding", "--disable-backgrounding-occluded-windows", "--disable-background-timer-throttling", "--disable-gpu-compositing"] {
            if !flags.contains(f) {
                add.push(f.to_string());
            }
        }
        if !flags.contains("CalculateNativeWinOcclusion") {
            add.push("--disable-features=CalculateNativeWinOcclusion".into());
        }
        if !add.is_empty() {
            std::env::set_var("QTWEBENGINE_CHROMIUM_FLAGS", format!("{flags} {}", add.join(" ")).trim());
        }
    }
    if shot.is_some() {
        // Draw off-screen so the picture never depends on the desktop (a
        // locked screen or hidden window would stop the window painting).
        // Set QT_QPA_PLATFORM to take the picture from a real window instead.
        if std::env::var_os("QT_QPA_PLATFORM").is_none() {
            std::env::set_var("QT_QPA_PLATFORM", "offscreen");
            if std::env::var_os("QT_QUICK_BACKEND").is_none() {
                std::env::set_var("QT_QUICK_BACKEND", "software");
            }
        }
    }

    if shot.is_some() {
        // Never hang when taking a picture, even if the window fails to load.
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_secs(45));
            eprintln!("screenshot: timed out");
            std::process::exit(2);
        });
    }

    // A widgets application, so the system print dialog is available.
    let plugins = option_env!("CRAFT_QT_PLUGINS").filter(|p| std::path::Path::new(p).exists()).unwrap_or("");
    store::qobject::webengine_initialize();
    startup_mark("browser engine ready");
    // Already running (for this data folder)? Then that one comes forward instead.
    if shot.is_none() && !instance::claim() {
        return;
    }

    // Wayland: the desktop remembers where the windows were (KDE Plasma 6.4+,
    // GNOME 48+); the session it gave last time lets it put them back.
    let session_file = data::data_dir().join("window-session");
    let wayland = shot.is_none() && std::env::var_os("WAYLAND_DISPLAY").is_some();
    if wayland && std::env::var_os("QT_WAYLAND_ENABLE_XX_SESSION_MANAGER").is_none() {
        std::env::set_var("QT_WAYLAND_ENABLE_XX_SESSION_MANAGER", "1");
    }
    let session = if wayland { std::fs::read_to_string(&session_file).unwrap_or_default().trim().to_string() } else { String::new() };
    // Almanac's own icons where icon lookups by name search (XDG_DATA_DIRS/icons).
    install_icons();
    let shared = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    let mine = data::data_dir().display().to_string();
    if !shared.split(':').any(|d| d == mine) {
        std::env::set_var("XDG_DATA_DIRS", format!("{mine}:{shared}"));
    }
    store::qobject::craft_create_app(&QString::from(plugins), &QString::from(session.as_str()));
    // The icon in the taskbar and window switcher (on Plasma's Wayland too).
    store::qobject::craft_set_app_icon(&QString::from(icon_path("almanac").display().to_string().as_str()));

    let mut engine = QQmlApplicationEngine::new();
    if let Some(mut engine) = engine.as_mut() {
        if let Some(qml) = option_env!("CRAFT_QT_QML") {
            if std::path::Path::new(qml).exists() {
                engine.as_mut().add_import_path(&QString::from(qml));
            }
        }
        engine.load(&QUrl::from("qrc:/qt/qml/org/craft/pim/qml/Main.qml"));
        startup_mark("window built");
    }
    store::qobject::craft_show_windows();

    let code = store::qobject::craft_exec();
    if wayland {
        // Qt reads "-session id_key", so an id with "_" in it can't be passed back.
        let id = store::qobject::craft_session_id().to_string();
        if !id.is_empty() && !id.contains('_') {
            let _ = std::fs::write(&session_file, id);
        }
    }
    std::process::exit(code);
}

#[cfg(test)]
mod qml_checks {
    /// Pull to refresh, with made-up trackpad and wheel gestures (tests/qml/
    /// pull_check.qml): scrolled up to the top and pulling on in the same
    /// gesture refreshes; a fling into the top or the system's momentum never
    /// does. Needs Qt's `qml` tool (in the development shell).
    #[test]
    fn pull_to_refresh_gestures() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/qml/pull_check.qml");
        let out = match std::process::Command::new("qml")
            .arg(&file)
            .env("QT_QPA_PLATFORM", "offscreen")
            .env("QT_FORCE_STDERR_LOGGING", "1")
            .output()
        {
            Ok(o) => o,
            Err(e) => {
                eprintln!("skipped: no qml tool ({e})");
                return;
            }
        };
        let log = String::from_utf8_lossy(&out.stderr);
        let checks: Vec<&str> = log.lines().filter(|l| l.contains("PULL-CHECK")).collect();
        assert!(out.status.success() && log.contains("PULL-CHECK done passed"), "{}", checks.join("\n"));
        assert!(checks.len() >= 9, "every scenario ran:\n{}", checks.join("\n"));
    }
}
