use cxx_qt_build::{CxxQtBuilder, QmlModule};
use qt_build_utils_query::qmake_query;

/// Small helper so the app finds Qt the same way cxx-qt-build does.
mod qt_build_utils_query {
    use std::process::Command;

    pub fn qmake_query(var: &str) -> Option<String> {
        let qmake = std::env::var("QMAKE").unwrap_or_else(|_| "qmake6".into());
        let try_run = |bin: &str| {
            Command::new(bin)
                .args(["-query", var])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        };
        try_run(&qmake).or_else(|| try_run("qmake"))
    }
}

fn main() {
    // On Linux (NixOS) a merged Qt environment with Qt Quick may live next to
    // the project. On Windows/macOS/other distros set QMAKE or put qmake on PATH.
    if std::env::var_os("QMAKE").is_none() {
        let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let local = manifest.join(".qt-env").join("bin").join("qmake");
        if local.exists() {
            std::env::set_var("QMAKE", &local);
        }
    }
    println!("cargo::rerun-if-env-changed=QMAKE");

    // Let the program know where Qt keeps its QML modules and plugins so it
    // works when started straight from the build folder.
    if let Some(qml) = qmake_query("QT_INSTALL_QML") {
        println!("cargo::rustc-env=CRAFT_QT_QML={qml}");
    }
    if let Some(plugins) = qmake_query("QT_INSTALL_PLUGINS") {
        println!("cargo::rustc-env=CRAFT_QT_PLUGINS={plugins}");
    }

    // Qt's private window interface (for Wayland session restore).
    let mut private_includes = Vec::new();
    if let (Some(headers), Some(version)) = (qmake_query("QT_INSTALL_HEADERS"), qmake_query("QT_VERSION")) {
        for module in ["QtCore", "QtGui"] {
            let dir = std::path::Path::new(&headers).join(module).join(&version);
            if dir.exists() {
                private_includes.push(dir.clone());
                private_includes.push(dir.join(module));
            }
        }
    }

    let mut builder = CxxQtBuilder::new_qml_module(
        QmlModule::new("org.craft.pim").qml_files([
            cxx_qt_build::QmlFile::from("qml/Main.qml"),
            cxx_qt_build::QmlFile::from("qml/Theme.qml").singleton(true),
            cxx_qt_build::QmlFile::from("qml/Icon.qml"),
            cxx_qt_build::QmlFile::from("qml/ToolButtonX.qml"),
            cxx_qt_build::QmlFile::from("qml/SearchBox.qml"),
            cxx_qt_build::QmlFile::from("qml/Avatar.qml"),
            cxx_qt_build::QmlFile::from("qml/SidebarItem.qml"),
            cxx_qt_build::QmlFile::from("qml/MailView.qml"),
            cxx_qt_build::QmlFile::from("qml/ComposeSheet.qml"),
            cxx_qt_build::QmlFile::from("qml/RichEditor.qml"),
            cxx_qt_build::QmlFile::from("qml/CalendarView.qml"),
            cxx_qt_build::QmlFile::from("qml/MiniMonth.qml"),
            cxx_qt_build::QmlFile::from("qml/CalTimeGrid.qml"),
            cxx_qt_build::QmlFile::from("qml/CalMonthGrid.qml"),
            cxx_qt_build::QmlFile::from("qml/CalYearGrid.qml"),
            cxx_qt_build::QmlFile::from("qml/EventEditor.qml"),
            cxx_qt_build::QmlFile::from("qml/ContactsView.qml"),
            cxx_qt_build::QmlFile::from("qml/SourcesDialog.qml"),
            cxx_qt_build::QmlFile::from("qml/ConditionEditor.qml"),
            cxx_qt_build::QmlFile::from("qml/RulesDialog.qml"),
            cxx_qt_build::QmlFile::from("qml/SmartMailboxDialog.qml"),
            cxx_qt_build::QmlFile::from("qml/SignaturesDialog.qml"),
            cxx_qt_build::QmlFile::from("qml/OutboxDialog.qml"),
            cxx_qt_build::QmlFile::from("qml/InviteCard.qml"),
            cxx_qt_build::QmlFile::from("qml/DuplicatesDialog.qml"),
            cxx_qt_build::QmlFile::from("qml/SmartGroupDialog.qml"),
            cxx_qt_build::QmlFile::from("qml/WelcomeGuide.qml"),
            cxx_qt_build::QmlFile::from("qml/SettingsDialog.qml"),
            cxx_qt_build::QmlFile::from("qml/SignInSetup.qml"),
            cxx_qt_build::QmlFile::from("qml/PaneGrip.qml"),
            cxx_qt_build::QmlFile::from("qml/ComboX.qml"),
            cxx_qt_build::QmlFile::from("qml/CheckX.qml"),
            cxx_qt_build::QmlFile::from("qml/DateField.qml"),
            cxx_qt_build::QmlFile::from("qml/TimeField.qml"),
            cxx_qt_build::QmlFile::from("qml/FieldX.qml"),
            cxx_qt_build::QmlFile::from("qml/Confirm.qml"),
            cxx_qt_build::QmlFile::from("qml/PassphraseDialog.qml"),
            cxx_qt_build::QmlFile::from("qml/WindowControls.qml"),
            cxx_qt_build::QmlFile::from("qml/ResizeEdges.qml"),
            cxx_qt_build::QmlFile::from("qml/MessagePage.qml"),
            cxx_qt_build::QmlFile::from("qml/MessageWindow.qml"),
            cxx_qt_build::QmlFile::from("qml/MessageTab.qml"),
            cxx_qt_build::QmlFile::from("qml/TabStrip.qml"),
            cxx_qt_build::QmlFile::from("qml/ActivityPanel.qml"),
            cxx_qt_build::QmlFile::from("qml/EventDetails.qml"),
            cxx_qt_build::QmlFile::from("qml/AttachmentView.qml"),
            cxx_qt_build::QmlFile::from("qml/AddressBox.qml"),
            cxx_qt_build::QmlFile::from("qml/FormatPickers.qml"),
            cxx_qt_build::QmlFile::from("qml/Momentum.qml"),
        ]),
    )
    .include_dir("include");
    // Windows: the app icon inside almanac.exe (Explorer, the taskbar, shortcuts).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let ico = manifest.join("packaging").join("icons").join("almanac.ico");
        let rc = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("almanac.rc");
        let ico = ico.display().to_string().replace('\\', "\\\\");
        std::fs::write(&rc, format!("1 ICON \"{ico}\"\n")).unwrap();
        println!("cargo::rerun-if-changed=packaging/icons/almanac.ico");
        embed_resource::compile(&rc, embed_resource::NONE).manifest_optional().unwrap();
    }

    for dir in private_includes {
        builder = builder.include_dir(dir);
    }
    builder
    .qt_module("Quick")
    .qt_module("QuickControls2")
    .qt_module("Widgets")
    .qt_module("PrintSupport")
    .qt_module("Pdf")
    .qt_module("WebEngineQuick")
    .files(["src/store.rs"])
    .build();
}
