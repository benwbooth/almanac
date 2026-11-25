import QtQuick
import QtQuick.Controls.Basic
// The system's own menus (desktop style on Linux, native on macOS and Windows)
import Qt.labs.platform as P
import QtQuick.Layouts
import QtQuick.Window
import QtCore
import org.craft.pim
import GlobalMenu

ApplicationWindow {
    // Back in view after the desktop hid it (on another desktop, covered…): see
    // CraftExposeWatch in include/craft_shot.h.
    signal reexposed()
    id: win
    width: 1280
    height: 800
    minimumWidth: 600
    minimumHeight: 480
    // Shown by the app once it's named for the desktop's session restore
    // (Wayland puts it back where it was).
    visible: false
    objectName: "almanac-main"
    property bool showAtStart: true
    title: ["Mail", "Calendar", "Contacts"][section] + " — Almanac"
    // A unified title bar, as in Apple's apps: the top bar *is* the title bar.
    // macOS keeps its own window buttons inside it; elsewhere the window has
    // no system frame and the top bar draws the buttons and moves the window.
    readonly property bool macOS: Qt.platform.os === "osx"
    flags: macOS ? (Qt.Window | Qt.ExpandedClientAreaHint | Qt.NoTitleBarBackgroundHint)
                 : (Qt.Window | Qt.FramelessWindowHint)
    color: Theme.window
    font.pixelSize: Theme.base

    property int section: store.startSection
    // Narrow window: Mail / Calendar / Contacts show icons only.
    readonly property bool compactBar: width < 1240
    // Very narrow: shortest labels (e.g. D W M Y in Calendar).
    readonly property bool tinyBar: width < 900
    // Sidebars step aside on narrow windows (and come back when it widens).
    property bool narrowed: false
    onWidthChanged: {
        if (keepState && restored) geometrySave.restart()
        if (width < 900 && !narrowed) { narrowed = true; for (var i = 0; i < views.length; i++) views[i].sidebarShown = false }
        else if (width >= 980 && narrowed) { narrowed = false; for (var j = 0; j < views.length; j++) views[j].sidebarShown = true }
        Qt.callLater(fitTools)
    }

    // Toolbar buttons that don't fit fold into the » menu at the end of the
    // bar (rightmost first), so every command stays reachable.
    property var overflowButtons: []
    function fitTools() {
        var row = toolsHost.item
        if (!row) return
        var kids = row.children, spacing = row.spacing || 0
        var fixed = 0, buttons = [], searches = []
        for (var i = 0; i < kids.length; i++) {
            var k = kids[i]
            if (k.naturalWidth !== undefined) { if (k.visible) { buttons.push(k); fixed += spacing } continue }
            if (!k.visible) continue
            if (k.collapsed !== undefined) searches.push(k)
            // Small fixed gaps (width set, no implicit width) count too; stretchy spacers don't.
            fixed += (k.minW !== undefined ? k.minW : (k.implicitWidth > 0 ? k.implicitWidth : (k.width <= 12 ? k.width : 0))) + spacing
        }
        var total = fixed
        for (var b = 0; b < buttons.length; b++) total += buttons[b].naturalWidth
        var hidden = []
        if (total > toolsHost.width) {
            var room = toolsHost.width - 40
            // Least important first; among equals, the rightmost first.
            var order = buttons.map(function(b, i) { return { b: b, i: i } })
            order.sort(function(x, y) { return (x.b.priority - y.b.priority) || (y.i - x.i) })
            for (var j = 0; j < order.length && total > room; j++) { total -= order[j].b.naturalWidth; hidden.push(order[j].b) }
            hidden.sort(function(x, y) { return buttons.indexOf(x) - buttons.indexOf(y) })
        }
        for (var m = 0; m < buttons.length; m++) buttons[m].overflowed = hidden.indexOf(buttons[m]) >= 0
        overflowButtons = hidden
        // Still too wide with every button folded away: the search box shrinks to its magnifier.
        var tight = total > toolsHost.width - (hidden.length ? 40 : 0)
        for (var q = 0; q < searches.length; q++) searches[q].collapsed = tight
    }
    function buttonLabel(b) {
        if (b.text) return b.text
        var t = b.tip || b.icon
        var cut = t.search(/ \(|: /)
        return cut > 0 ? t.slice(0, cut) : t
    }
    readonly property var views: [mailView, calendarView, contactsView]
    Timer { id: printCheck; interval: 2500; onTriggered: { var m = mailView.current; store.attachmentPreviews(m.id); store.printAttachment(m.id, "Agenda.pdf") } }
    Timer {
        id: glideTest
        interval: 6000
        onTriggered: mailView.runInPage(
            "(function(){var tall=document.createElement('div');tall.style.height='6000px';document.body.appendChild(tall);var log=[];function swipe(at,count,c){for(var i=0;i<count;i++)setTimeout(function(){window.dispatchEvent(new WheelEvent('wheel',{deltaY:12,wheelDeltaY:-14,cancelable:c!==false}))},at+i*12)}"
            + "swipe(0,10);swipe(400,10," + (store.arg("--uncancelable") === "") + ");"
            + "for(var t=0;t<=1400;t+=100)(function(t){setTimeout(function(){log.push(t+':'+Math.round(scrollY))},t)})(t);"
            + "setTimeout(function(){console.warn('GLIDETEST '+log.join(' ')+' max '+(document.documentElement.scrollHeight-innerHeight)+' inner '+innerHeight+' frames '+document.querySelectorAll('iframe').length+' title '+document.title+' fh '+document.querySelector('iframe').style.height+' doc '+(function(){try{return document.querySelector('iframe').contentDocument.documentElement.scrollHeight}catch(e){return 'x'}})()+' vis '+document.visibilityState)},1450)})()")
    }

    // Controls (fields, menus, check boxes) follow the light/dark choice.
    palette.window: Theme.popup
    palette.windowText: Theme.text
    palette.base: Theme.field
    palette.alternateBase: Theme.card
    palette.text: Theme.text
    palette.button: Theme.segment
    palette.buttonText: Theme.text
    palette.highlight: Theme.accent
    palette.highlightedText: Theme.accentInk
    palette.placeholderText: Theme.tertiary
    palette.mid: Theme.border
    palette.midlight: Theme.borderSoft
    palette.light: Theme.popup
    palette.dark: Theme.secondary
    palette.shadow: "#000000"
    palette.toolTipBase: Theme.popup
    palette.toolTipText: Theme.text

    Store { id: store }
    Binding { target: Theme; property: "mode"; value: store.theme }
    Binding { target: Theme; property: "weekStart"; value: { store.revision; return JSON.parse(store.settings()).week_start } }

    Rectangle {
        id: shotRoot
        anchors.fill: parent
        color: Theme.window

        ColumnLayout {
            anchors.fill: parent
            // A thousandth of a pixel down. At 130% scaling, text that lands
            // exactly on a half device pixel has its letters rounded up or down
            // one by one, so round ones (e, o, s) sit a pixel below the rest.
            // Off the exact half, they all agree. (As in Earshot. Here, not on
            // the window's contentItem: ApplicationWindow lays that out again.)
            anchors.topMargin: 0.001
            spacing: 0

            // ------------------------------------------- the one top bar
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: Theme.toolbarHeight
                gradient: Theme.toolbarGradient
                z: 2
                // A soft shadow under the bar, over what's below it.
                Rectangle {
                    anchors.top: parent.bottom
                    width: parent.width; height: 6
                    gradient: Gradient {
                        GradientStop { position: 0; color: Theme.shade }
                        GradientStop { position: 1; color: "transparent" }
                    }
                }
                // Drag the bar to move the window, double-click to zoom it, right-click
                // for the window menu (the shared ChromeTitleArea, as every app).
                ChromeTitleArea { anchors.fill: parent; window: win; menuDark: Theme.dark; menuAccent: Theme.accentFill }
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: win.macOS ? 78 : 10
                    anchors.rightMargin: win.macOS ? 10 : 0
                    spacing: 6

                    MacMenuButton {
                        id: menuButton
                        dark: Theme.dark
                        fg: Theme.text
                        menu: appMenu
                        tip: "Menu: every command, Accounts and Settings"
                        onAboutToOpen: win.focusBeforeMenu = win.activeFocusItem
                    }
                    ToolButtonX {
                        icon: "sidebar"
                        tip: "Show or hide the sidebar"
                        checked: !win.views[win.section].sidebarShown
                        onClicked: win.views[win.section].sidebarShown = !win.views[win.section].sidebarShown
                    }

                    // Switch between Mail, Calendar and Contacts.
                    Rectangle {
                        Layout.preferredHeight: 32
                        Layout.preferredWidth: appSeg.implicitWidth + 4
                        radius: 8
                        color: Theme.segment
                        Row {
                            id: appSeg
                            anchors.centerIn: parent
                            Repeater {
                                model: [{ icon: "mail", name: "Mail" }, { icon: "calendar", name: "Calendar" }, { icon: "contacts", name: "Contacts" }]
                                delegate: Rectangle {
                                    required property var modelData
                                    required property int index
                                    width: segRow.implicitWidth + (label.visible ? 22 : 16)
                                    height: 28
                                    radius: 6
                                    color: win.section === index ? Theme.segmentOn : (segArea.containsMouse ? Theme.hover : "transparent")
                                    // The chosen one sits a little raised.
                                    Rectangle {
                                        z: -1
                                        visible: win.section === index
                                        anchors.fill: parent; anchors.topMargin: 1; anchors.bottomMargin: -1
                                        radius: parent.radius
                                        color: Theme.segmentShade
                                    }
                                    Row {
                                        id: segRow
                                        anchors.centerIn: parent
                                        spacing: 6
                                        Icon {
                                            width: 17; height: 17
                                            anchors.verticalCenter: parent.verticalCenter
                                            name: modelData.icon
                                            color: win.section === index ? Theme.accent : Theme.secondary
                                        }
                                        Text {
                                            id: label
                                            visible: win.section === index && !win.compactBar
                                            anchors.verticalCenter: parent.verticalCenter
                                            text: modelData.name
                                            color: Theme.text
                                            font.pixelSize: Theme.base
                                            font.weight: win.section === index ? Font.DemiBold : Font.Normal
                                        }
                                    }
                                    MouseArea { id: segArea; anchors.fill: parent; hoverEnabled: true; onClicked: win.section = index }
                                    ToolTip.visible: segArea.containsMouse
                                    ToolTip.delay: 700
                                    ToolTip.text: modelData.name + "  (Ctrl+" + (index + 1) + ")"
                                }
                            }
                        }
                    }
                    Rectangle { Layout.preferredWidth: 1; Layout.preferredHeight: 26; color: Theme.border; Layout.leftMargin: 4; Layout.rightMargin: 4 }

                    // The open app's own buttons.
                    Loader {
                        id: toolsHost
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        clip: true
                        sourceComponent: win.views[win.section].tools
                        onLoaded: Qt.callLater(win.fitTools)
                        onWidthChanged: Qt.callLater(win.fitTools)
                    }
                    Connections {
                        target: toolsHost.item
                        function onImplicitWidthChanged() { Qt.callLater(win.fitTools) }
                    }
                    ToolButtonX {
                        id: overflowButton
                        visible: win.overflowButtons.length > 0
                        text: "»"
                        tip: "More toolbar buttons"
                        onClicked: {
                            overflowMenu.clear()
                            for (var i = 0; i < win.overflowButtons.length; i++)
                                overflowMenu.addItem(overflowItem.createObject(overflowMenu, { target: win.overflowButtons[i] }))
                            overflowMenu.popup(overflowButton, 0, overflowButton.height)
                        }
                        AMenu { id: overflowMenu }
                        Component {
                            id: overflowItem
                            MacMenuItem {
                                property var target
                                text: target ? win.buttonLabel(target) : ""
                                icon.name: target ? (win.themeIcons[target.icon] || "") : ""
                                shortcut: target ? win.buttonShortcut(target) : ""
                                enabled: target ? target.enabled : false
                                checkable: target ? target.checked : false
                                checked: target ? target.checked : false
                                onTriggered: target.clicked()
                            }
                        }
                    }

                    // Windows / Linux: Qt-style window buttons on the right (macOS has its own).
                    WindowControls { visible: !win.macOS; window: win; Layout.leftMargin: 6; Layout.fillHeight: true }
                }
                Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border }
            }

            StackLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                currentIndex: win.section
                // (A pane resized: saved a moment after the drag.)
                MailView { id: mailView; store: store; onAccountsRequested: sourcesDialog.open(); onPanesChanged: if (win.keepState && win.restored) geometrySave.restart() }
                CalendarView { id: calendarView; store: store; onPanesChanged: if (win.keepState && win.restored) geometrySave.restart() }
                ContactsView { id: contactsView; store: store; onComposeRequested: function(p) { win.section = 0; mailView.reopen(p) }; onPanesChanged: if (win.keepState && win.restored) geometrySave.restart() }
            }
        }

        // First start: guide to adding accounts (covers the window until done).
        WelcomeGuide {
            anchors.fill: parent
            visible: !store.onboarded
            z: 4
            store: store
            accountsDialog: sourcesDialog
        }

        // "Undo Send" for a few seconds after sending.
        Rectangle {
            visible: store.undoId !== 0
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 60
            anchors.horizontalCenter: parent.horizontalCenter
            width: undoRow.implicitWidth + 28
            height: 38
            radius: 19
            color: Theme.toast
            z: 5
            Row {
                id: undoRow
                anchors.centerIn: parent
                spacing: 14
                Text { text: "Sending…"; color: Theme.toastText; font.pixelSize: Theme.base; anchors.verticalCenter: parent.verticalCenter }
                Text {
                    text: "Undo Send"; color: Theme.accent; font.pixelSize: Theme.base; font.weight: Font.Bold
                    anchors.verticalCenter: parent.verticalCenter
                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: { var p = store.undoSend(); if (p) { win.section = 0; mailView.reopen(JSON.parse(p)) } }
                    }
                }
            }
        }
    }

    SignInSetup { id: signInSetup; store: store; accountsDialog: sourcesDialog }
    SourcesDialog { id: sourcesDialog; store: store; signInSetup: signInSetup }
    SettingsDialog {
        id: settingsDialog
        store: store
        accountsDialog: sourcesDialog
        rulesDialog: mailView.rulesDlg
        signaturesDialog: mailView.signaturesDlg
    }

    // The menus: in the desktop's global menu bar (hearth, Plasma's Global
    // Menu) and in the ☰ button, from one model, laid out like Apple Mail's
    // menu bar (Almanac, File, Edit, View, Mailbox, Message, Window, Help)
    // with Calendar and Contacts added. Items: { id, text, shortcut, icon,
    // enabled, checkable, checked, items (a submenu) }; ids are unique (a
    // repeated command gets "#2"), and choosing one runs menuAction(id).
    // The shortcuts are only shown: the Shortcut objects below (and the
    // views' own) do the keys.
    function editAction(name) {
        // From the ☰ menu the focus is on the menu: start from what had it.
        var from = win.fromAppMenu ? win.focusBeforeMenu : win.activeFocusItem
        // The focus in an email's page sits on an item inside the browser
        // engine's view: walk up to whatever offers the action.
        for (var f = from; f; f = f.parent) {
            // (The message editor pastes formatted text in its own colours.)
            if (name === "paste" && typeof f.pasteFormatted === "function") { f.pasteFormatted(); return }
            if (typeof f[name] === "function") { f[name](); return }
        }
    }
    property bool fromAppMenu: false
    property Item focusBeforeMenu: null
    readonly property bool hasMessage: mailView.current !== null
    // (Built once everything has started: read while the views were still
    // setting themselves up, their first values counted as a binding loop.)
    property bool menusReady: false
    readonly property var menuModel: {
        if (!menusReady) return []
        var cur = mailView.current, row = mailView.currentRow
        var sortIcons = { date: "clock", from: "im-user", subject: "sort-name", to: "mail-sent", attachments: "mail-attachment", flags: "flag", unread: "mail-unread" }
        var sortItems = ["date", "from", "subject", "to", "attachments", "flags", "unread"].map(function(k) {
            return { id: "sortBy:" + k, text: mailView.sortNames[k], icon: sortIcons[k], checkable: true, checked: mailView.sortBy === k }
        })
        // (The icon theme has no orange or purple flag: those get the plain one.)
        var flagIcons = ["flag-red", "flag", "flag-yellow", "flag-green", "flag-blue", "flag", "flag-black"]
        var flagItems = []
        for (var c = 0; c < 7; c++) flagItems.push({ id: "flagColour:" + c, text: Theme.flagNames[c], icon: flagIcons[c] })
        flagItems.push({ separator: true }, { id: "flagColour:-1", text: "Clear Flag", icon: "edit-clear" })
        var many = mailView.sidebar.accounts.length > 1
        var folderIcons = { inbox: "mail-folder-inbox", drafts: "document-edit", sent: "mail-folder-sent", archive: "archive-insert", junk: "mail-mark-junk", trash: "user-trash" }
        var moveItems = (mailView.folders || []).map(function(f) {
            return { id: "moveTo:" + f.id, text: (many ? f.account + " › " : "") + f.name, icon: folderIcons[f.kind] || "folder" }
        })
        var fullScreen = win.visibility === Window.FullScreen
        var view = win.views[win.section]
        return [
            { title: "Almanac", icon: "almanac", iconSource: store.iconUrl("almanac"), items: [
                { id: "about", text: "About Almanac", icon: "help-about", shortcut: "Shift+F1" },
                { separator: true },
                { id: "settings", text: "Settings…", icon: "configure", shortcut: "Ctrl+," },
                { id: "accounts", text: "Accounts…", icon: "system-users", shortcut: "Ctrl+Alt+A" },
                { id: "signIn", text: "Sign-in Setup (Google, Microsoft, Yahoo, AOL)…", icon: "dialog-password", shortcut: "Ctrl+Alt+G" },
                { id: "setupGuide", text: "Setup Guide", icon: "help-contents", shortcut: "Ctrl+F1" },
                { separator: true },
                { id: "quit", text: "Quit Almanac", icon: "application-exit", shortcut: "Ctrl+Q" }
            ] },
            { title: "File", icon: "folder-open", items: [
                { id: "newMessage", text: "New Message", icon: "mail-message-new", shortcut: "Ctrl+N" },
                { id: "newEvent", text: "New Event", icon: "appointment-new", shortcut: "Ctrl+Shift+E" },
                { id: "newContact", text: "New Contact", icon: "contact-new", shortcut: "Ctrl+Shift+C" },
                { separator: true },
                { id: "saveAttachments", text: "Save Attachments…", icon: "mail-attachment", shortcut: "Ctrl+Shift+S", enabled: !!cur && cur.attachments.length > 0 },
                { id: "importMailboxes", text: "Import Mailboxes…", icon: "document-import", shortcut: "Ctrl+Shift+I" },
                { id: "importVcard", text: "Import vCard…", icon: "document-import", shortcut: "Ctrl+Alt+I" },
                { id: "exportVcard", text: "Export vCard…", icon: "document-export", shortcut: "Ctrl+Alt+E" },
                { id: "exportPdf", text: "Export as PDF…", icon: "application-pdf", shortcut: "Ctrl+Shift+P", enabled: cur !== null },
                { separator: true },
                { id: "print", text: "Print…", icon: "document-print", shortcut: "Ctrl+P" }
            ] },
            { title: "Edit", icon: "document-edit", items: [
                { id: "undo", text: !win.typing && store.mailUndoText !== "" ? store.mailUndoText : "Undo", icon: "edit-undo", shortcut: "Ctrl+Z" },
                { id: "redo", text: "Redo", icon: "edit-redo", shortcut: "Ctrl+Shift+Z" },
                { separator: true },
                { id: "cut", text: "Cut", icon: "edit-cut", shortcut: "Ctrl+X" },
                { id: "copy", text: "Copy", icon: "edit-copy", shortcut: "Ctrl+C" },
                { id: "paste", text: "Paste", icon: "edit-paste", shortcut: "Ctrl+V" },
                { id: "delete", text: "Delete", icon: "edit-delete", shortcut: "Del", enabled: win.section === 0 && cur !== null },
                { id: "selectAll", text: "Select All", icon: "edit-select-all", shortcut: "Ctrl+A" },
                { separator: true },
                { id: "find", text: "Find…", icon: "edit-find", shortcut: "Ctrl+F" },
                { id: "findInMessage", text: "Find in Message…", icon: "edit-find-mail", shortcut: "Ctrl+G", enabled: win.section === 0 && cur !== null },
                { id: "spelling", text: "Spelling and Grammar", icon: "tools-check-spelling", items: [
                    { id: "spellCheck", text: "Check Spelling While Typing", icon: "checkmark", checkable: true, checked: mailView.composer.spellOn },
                    { id: "grammarCheck", text: "Check Grammar With Spelling", icon: "story-editor", checkable: true, checked: mailView.composer.grammarOn, enabled: mailView.composer.spellOn }
                ] }
            ] },
            { title: "View", icon: "view-visible", items: [
                { id: "showMail", text: "Mail", icon: "mail-message", shortcut: "Ctrl+1", checkable: true, checked: win.section === 0 },
                { id: "showCalendar", text: "Calendar", icon: "view-calendar", shortcut: "Ctrl+2", checkable: true, checked: win.section === 1 },
                { id: "showContacts", text: "Contacts", icon: "x-office-address-book", shortcut: "Ctrl+3", checkable: true, checked: win.section === 2 },
                { separator: true },
                { id: "toggleSidebar", text: view.sidebarShown ? "Hide Mailbox List" : "Show Mailbox List", icon: "view-sidetree", shortcut: "Ctrl+Alt+S" },
                { id: "threaded", text: "Organize by Conversation", icon: "view-list-tree", shortcut: "Ctrl+Shift+O", checkable: true, checked: mailView.threaded },
                { id: "layout", text: "Layout", icon: "view-choose", items: [
                    { id: "layoutStandard", text: "Standard (List Beside Message)", icon: "view-split-left-right", shortcut: "Ctrl+Alt+H", checkable: true, checked: !mailView.classic },
                    { id: "layoutClassic", text: "Classic (List Above Message)", icon: "view-split-top-bottom", shortcut: "Ctrl+Alt+K", checkable: true, checked: mailView.classic },
                    { separator: true },
                    { id: "mailSidebar", text: "Show Mailbox List", icon: "view-sidetree", checkable: true, checked: mailView.sidebarShown }
                ] },
                { id: "sortBy", text: "Sort By", icon: "view-sort", items: sortItems.concat([
                    { separator: true },
                    { id: "sortAscending", text: "Ascending", icon: "view-sort-ascending", shortcut: "Ctrl+Alt+Up", checkable: true, checked: mailView.sortAscending },
                    { id: "sortDescending", text: "Descending", icon: "view-sort-descending", shortcut: "Ctrl+Alt+Down", checkable: true, checked: !mailView.sortAscending }
                ]) },
                { id: "filter", text: "Filter", icon: "filter-symbolic", items: [
                    { id: "allMessages", text: "All Messages", icon: "view-list-details", shortcut: "Ctrl+Alt+M", checkable: true, checked: !mailView.unreadOnly },
                    { id: "onlyUnread", text: "Only Unread", icon: "mail-mark-unread", shortcut: "Ctrl+Alt+U", checkable: true, checked: mailView.unreadOnly }
                ] },
                { id: "messageView", text: "Message", icon: "document-preview", items: [
                    { id: "formatted", text: "Formatted", icon: "text-html", shortcut: "Ctrl+Alt+F", checkable: true, checked: mailView.formatted },
                    { id: "plainText", text: "Plain Text Alternative", icon: "text-plain", shortcut: "Ctrl+Alt+P", checkable: true, checked: !mailView.formatted },
                    { separator: true },
                    { id: "loadRemote", text: "Load Remote Content Automatically", icon: "image-x-generic", shortcut: "Ctrl+Alt+Shift+R", checkable: true, checked: !!mailView.prefs.load_remote }
                ] },
                { separator: true },
                { id: "themeLight", text: "Light Appearance", icon: "flashlight-on", shortcut: "Ctrl+Alt+Shift+L", checkable: true, checked: store.theme === "light" },
                { id: "themeDark", text: "Dark Appearance", icon: "flashlight-off", shortcut: "Ctrl+Alt+Shift+D", checkable: true, checked: store.theme === "dark" },
                { id: "themeSystem", text: "Match System Appearance", icon: "color-management", shortcut: "Ctrl+Alt+Shift+M", checkable: true, checked: store.theme === "system" },
                { separator: true },
                { id: "fullScreen", text: fullScreen ? "Exit Full Screen" : "Enter Full Screen", icon: "view-fullscreen", shortcut: "F11" }
            ] },
            { title: "Mailbox", icon: "mail-folder-inbox", items: [
                { id: "getMail", text: "Get All New Mail", icon: "mail-receive", shortcut: "Ctrl+Shift+N" },
                { separator: true },
                { id: "eraseTrash", text: "Erase Deleted Items", icon: "edit-delete", shortcut: "Ctrl+Shift+Del" },
                { id: "eraseJunk", text: "Erase Junk Mail", icon: "mail-mark-junk", shortcut: "Ctrl+Alt+J" },
                { separator: true },
                { id: "newSmartMailbox", text: "New Smart Mailbox…", icon: "folder-new", shortcut: "Ctrl+Alt+Shift+N" },
                { id: "markAllRead", text: "Mark All Messages as Read", icon: "mail-mark-read", shortcut: "Ctrl+Alt+Shift+U" },
                { separator: true },
                { id: "goTo", text: "Go To", icon: "go-jump", items: [
                    { id: "go:inbox", text: "Inbox", icon: "mail-folder-inbox", shortcut: "Alt+1" },
                    { id: "go:drafts", text: "Drafts", icon: "document-edit", shortcut: "Alt+2" },
                    { id: "go:sent", text: "Sent", icon: "mail-folder-sent", shortcut: "Alt+3" },
                    { id: "go:archive", text: "Archive", icon: "archive-insert", shortcut: "Alt+4" },
                    { id: "go:junk", text: "Junk", icon: "mail-mark-junk", shortcut: "Alt+5" },
                    { id: "go:trash", text: "Trash", icon: "user-trash", shortcut: "Alt+6" }
                ] },
                { separator: true },
                { id: "rules", text: "Rules…", icon: "view-filter", shortcut: "Ctrl+Alt+R" },
                { id: "signatures", text: "Signatures…", icon: "mail-signed", shortcut: "Ctrl+Alt+Shift+G" }
            ] },
            { title: "Message", icon: "mail-message", items: [
                { id: "sendAgain", text: "Send Again", icon: "mail-send", shortcut: "Ctrl+Shift+D", enabled: cur !== null },
                { separator: true },
                { id: "reply", text: "Reply", icon: "mail-reply-sender", shortcut: "Ctrl+R", enabled: cur !== null },
                { id: "replyAll", text: "Reply All", icon: "mail-reply-all", shortcut: "Ctrl+Shift+R", enabled: cur !== null },
                { id: "forward", text: "Forward", icon: "mail-forward", shortcut: "Ctrl+Shift+F", enabled: cur !== null },
                { separator: true },
                { id: "mark", text: "Mark", icon: "mail-mark-important", enabled: row !== null, items: [
                    { id: "markRead", text: row && row.read ? "As Unread" : "As Read", icon: "mail-mark-unread", shortcut: "Ctrl+Shift+U", enabled: row !== null },
                    { id: "junk#2", text: "As Junk Mail", icon: "mail-mark-junk", shortcut: "Ctrl+Shift+J", enabled: row !== null }
                ] },
                { id: "flag", text: cur && cur.flagged ? "Unflag" : "Flag", icon: "flag-red", shortcut: "Ctrl+Shift+L", enabled: cur !== null },
                { id: "flagColour", text: "Flag Colour", icon: "flag", enabled: cur !== null, items: flagItems },
                { id: "mute", text: cur && cur.muted ? "Unmute" : "Mute", icon: "mail-thread-ignored", shortcut: "Ctrl+Shift+M", enabled: cur !== null },
                { separator: true },
                { id: "archive", text: "Archive", icon: "archive-insert", shortcut: "Ctrl+Shift+A", enabled: cur !== null },
                { id: "junk", text: "Move to Junk", icon: "mail-mark-junk", shortcut: "Ctrl+Shift+J", enabled: cur !== null },
                { id: "moveTo", text: "Move to", icon: "edit-move", enabled: cur !== null && moveItems.length > 0, items: moveItems },
                { separator: true },
                { id: "applyRules", text: "Apply Rules", icon: "system-run", shortcut: "Ctrl+Alt+Shift+A" },
                { id: "vip", text: cur && cur.vip ? "Remove Sender from VIPs" : "Add Sender to VIPs", icon: "emblem-favorite", shortcut: "Ctrl+Alt+V", enabled: cur !== null },
                { id: "block", text: cur && cur.blocked ? "Unblock Sender" : "Block Sender", icon: "im-ban-user", shortcut: "Ctrl+Alt+B", enabled: cur !== null },
                { separator: true },
                { id: "remind", text: "Remind Me", icon: "appointment-reminder", enabled: cur !== null, items: [
                    { id: "remind:hour", text: "In 1 Hour", icon: "chronometer", shortcut: "Ctrl+Alt+Shift+H", enabled: cur !== null },
                    { id: "remind:tonight", text: "Tonight", icon: "weather-clear-night", shortcut: "Ctrl+Alt+Shift+T", enabled: cur !== null },
                    { id: "remind:tomorrow", text: "Tomorrow", icon: "view-calendar-upcoming-days", shortcut: "Ctrl+Alt+Shift+O", enabled: cur !== null },
                    { id: "remind:week", text: "Next Week", icon: "view-calendar-week", shortcut: "Ctrl+Alt+Shift+W", enabled: cur !== null },
                    { separator: true },
                    { id: "remind:none", text: "Remove Reminder", icon: "edit-clear-history", shortcut: "Ctrl+Alt+Shift+X", enabled: cur !== null }
                ] },
                { id: "outbox", text: "Outbox (Send Later)…", icon: "mail-folder-outbox", shortcut: "Ctrl+Alt+O" }
            ] },
            { title: "Calendar", icon: "view-calendar", items: [
                { id: "newEvent#2", text: "New Event", icon: "appointment-new", shortcut: "Ctrl+Shift+E" },
                { id: "newCalendar", text: "New Calendar…", icon: "view-calendar-list", shortcut: "Ctrl+Alt+N" },
                { id: "newSubscription", text: "New Calendar Subscription…", icon: "view-calendar-upcoming-events", shortcut: "Ctrl+Alt+Shift+S" },
                { separator: true },
                { id: "today", text: "Go to Today", icon: "go-jump-today", shortcut: "Ctrl+T" },
                { id: "cal:day", text: "Day", icon: "view-calendar-day", shortcut: "Ctrl+Alt+1", checkable: true, checked: calendarView.view === "day" },
                { id: "cal:week", text: "Week", icon: "view-calendar-week", shortcut: "Ctrl+Alt+2", checkable: true, checked: calendarView.view === "week" },
                { id: "cal:month", text: "Month", icon: "view-calendar-month", shortcut: "Ctrl+Alt+3", checkable: true, checked: calendarView.view === "month" },
                { id: "cal:year", text: "Year", icon: "view-calendar-timeline", shortcut: "Ctrl+Alt+4", checkable: true, checked: calendarView.view === "year" },
                { separator: true },
                { id: "refreshSubscriptions", text: "Refresh Subscriptions", icon: "view-refresh", shortcut: "Ctrl+Alt+Shift+F" }
            ] },
            { title: "Contacts", icon: "x-office-address-book", items: [
                { id: "newContact#2", text: "New Contact", icon: "contact-new", shortcut: "Ctrl+Shift+C" },
                { id: "newGroup", text: "New Group…", icon: "user-group-new", shortcut: "Ctrl+Alt+Shift+K" },
                { id: "newSmartGroup", text: "New Smart Group…", icon: "user-group-properties", shortcut: "Ctrl+Alt+Shift+J" },
                { separator: true },
                { id: "duplicates", text: "Look for Duplicates…", icon: "edit-duplicate", shortcut: "Ctrl+Alt+D" },
                { id: "myCard", text: "Go to My Card", icon: "user-identity", shortcut: "Ctrl+Alt+Shift+I" }
            ] },
            { title: "Window", icon: "window", items: [
                { id: "minimize", text: "Minimize", icon: "window-minimize", shortcut: "Ctrl+M" },
                { id: "zoom", text: "Zoom", icon: "window-maximize", shortcut: "Ctrl+Alt+Z" },
                { separator: true },
                { id: "messageViewer", text: "Message Viewer", icon: "mail-message", shortcut: "Ctrl+Alt+0" },
                { id: "outbox#2", text: "Outbox", icon: "mail-folder-outbox", shortcut: "Ctrl+Alt+O" }
            ] },
            { title: "Help", icon: "help-contents", items: [
                { id: "keys", text: "Keyboard Shortcuts", icon: "help-keybord-shortcuts", shortcut: "F1" },
                { id: "setupGuide#2", text: "Setup Guide", icon: "help-contents", shortcut: "Ctrl+F1" }
            ] }
        ]
    }
    function menuAction(id) {
        var cut = id.indexOf("#")
        if (cut > 0) id = id.slice(0, cut)
        var arg = id.indexOf(":") > 0 ? id.slice(id.indexOf(":") + 1) : ""
        var cur = mailView.current
        switch (id.indexOf(":") > 0 ? id.slice(0, id.indexOf(":")) : id) {
        case "about": aboutPopup.open(); break
        case "settings": settingsDialog.open(); break
        case "accounts": sourcesDialog.open(); break
        case "signIn": signInSetup.openFor("google", false); break
        case "setupGuide": store.onboarded = false; break
        case "quit": Qt.quit(); break
        case "newMessage": win.section = 0; mailView.newMessage(); break
        case "newEvent": win.section = 1; calendarView.newEventNow(); break
        case "newContact": win.section = 2; contactsView.newContact(); break
        case "saveAttachments": if (cur && cur.attachments.length > 0) mailView.saveAttachments(); break
        case "importMailboxes": sourcesDialog.open(); break
        case "importVcard": win.section = 2; contactsView.importCards(); break
        case "exportVcard": win.section = 2; contactsView.exportCards(); break
        case "exportPdf": if (cur) mailView.exportCurrentPdf(); break
        case "print": win.section === 0 ? mailView.printCurrent() : win.section === 1 ? calendarView.printView() : contactsView.printCards(false); break
        case "undo": if (!win.typing && !win.fromAppMenu && store.mailUndoText !== "" || win.fromAppMenu && store.mailUndoText !== "" && (!win.focusBeforeMenu || win.focusBeforeMenu.cursorPosition === undefined)) store.undoMail(); else win.editAction("undo"); break
        case "redo": case "cut": case "copy": case "paste": case "selectAll": win.editAction(id); break
        case "delete": if (win.section === 0 && cur) mailView.removeCurrent("delete"); break
        case "find": win.section === 0 ? mailView.find() : win.views[win.section].focusSearch(); break
        case "findInMessage": if (win.section === 0 && cur) mailView.findInMessage(); break
        case "spellCheck": mailView.composer.setSpelling(!mailView.composer.spellOn); break
        case "grammarCheck": if (mailView.composer.spellOn) mailView.composer.setGrammar(!mailView.composer.grammarOn); break
        case "showMail": win.section = 0; break
        case "showCalendar": win.section = 1; break
        case "showContacts": win.section = 2; break
        case "toggleSidebar": win.views[win.section].sidebarShown = !win.views[win.section].sidebarShown; break
        case "threaded": mailView.threaded = !mailView.threaded; break
        case "layoutStandard": win.section = 0; mailView.setLayout("standard"); break
        case "layoutClassic": win.section = 0; mailView.setLayout("classic"); break
        case "mailSidebar": mailView.sidebarShown = !mailView.sidebarShown; break
        case "sortBy": mailView.setSort(arg, mailView.sortAscending); break
        case "sortAscending": mailView.setSort(mailView.sortBy, true); break
        case "sortDescending": mailView.setSort(mailView.sortBy, false); break
        case "allMessages": mailView.unreadOnly = false; break
        case "onlyUnread": mailView.unreadOnly = true; break
        case "formatted": mailView.formatted = true; break
        case "plainText": mailView.formatted = false; break
        case "loadRemote": store.setSetting("load_remote", JSON.stringify(!mailView.prefs.load_remote)); break
        case "themeLight": store.chooseTheme("light"); break
        case "themeDark": store.chooseTheme("dark"); break
        case "themeSystem": store.chooseTheme("system"); break
        case "fullScreen": win.visibility === Window.FullScreen ? win.showNormal() : win.showFullScreen(); break
        case "getMail": store.refreshAll(); break
        case "eraseTrash": mailView.eraseMailbox("trash"); break
        case "eraseJunk": mailView.eraseMailbox("junk"); break
        case "newSmartMailbox": win.section = 0; mailView.newSmartMailbox(); break
        case "markAllRead": store.markAllRead(mailView.selection); break
        case "go":
            win.section = 0
            mailView.goTo(arg, { inbox: "Inbox", drafts: "Drafts", sent: "Sent", archive: "Archive", junk: "Junk", trash: "Trash" }[arg])
            break
        case "rules": mailView.rulesDlg.open(); break
        case "signatures": mailView.signaturesDlg.open(); break
        case "sendAgain": if (cur) { win.section = 0; mailView.sendAgain() } break
        case "reply": if (cur) { win.section = 0; mailView.reply(false) } break
        case "replyAll": if (cur) { win.section = 0; mailView.reply(true) } break
        case "forward": if (cur) { win.section = 0; mailView.forward() } break
        case "markRead": if (mailView.currentRow) mailView.setCurrentRead(!mailView.currentRow.read); break
        case "flag": if (cur) mailView.toggleCurrentFlag(); break
        case "flagColour": if (cur) mailView.setCurrentFlag(parseInt(arg)); break
        case "mute": if (cur) store.toggleMute(cur.id); break
        case "archive": if (cur) mailView.removeCurrent("archive"); break
        case "junk": if (cur || mailView.currentRow) mailView.removeCurrent("junk"); break
        case "moveTo": if (cur) mailView.moveCurrentToFolder(isNaN(Number(arg)) ? arg : Number(arg)); break
        case "applyRules": store.applyRulesNow(); break
        case "vip": if (cur) store.toggleVip(cur.fromEmail); break
        case "block": if (cur) store.toggleBlocked(cur.fromEmail); break
        case "remind":
            if (arg === "hour") mailView.remindIn(3600000)
            else if (arg === "tonight") mailView.remindAt(21, 0)
            else if (arg === "tomorrow") mailView.remindAt(8, 1)
            else if (arg === "week") mailView.remindAt(8, 7)
            else if (cur) store.remindMe(cur.id, 0)
            break
        case "outbox": mailView.outboxDlg.open(); break
        case "newCalendar": win.section = 1; calendarView.newCalendar(); break
        case "newSubscription": win.section = 1; calendarView.newSubscription(); break
        case "today": win.section = 1; calendarView.cursor = new Date(); break
        case "cal": win.section = 1; calendarView.view = arg; break
        case "refreshSubscriptions": store.syncSubscriptions(); break
        case "newGroup": win.section = 2; contactsView.newGroup(); break
        case "newSmartGroup": win.section = 2; contactsView.newSmartGroup(); break
        case "duplicates": win.section = 2; contactsView.findDuplicates(); break
        case "myCard": win.section = 2; contactsView.goToMyCard(); break
        case "minimize": win.showMinimized(); break
        case "zoom": win.visibility === Window.Maximized ? win.showNormal() : win.showMaximized(); break
        case "messageViewer": win.section = 0; break
        case "keys": shortcutsPopup.open(); break
        }
    }
    GlobalMenuBar { id: globalMenu; window: win; model: win.menuModel; onTriggered: id => win.menuAction(id) }
    onVisibleChanged: if (visible) Qt.callLater(globalMenu.attach)
    // The ☰ menu: the same model in the shared glass look, in Almanac's colours.
    AppMenu {
        id: appMenu
        accent: Theme.accentFill
        dark: Theme.dark
        popupType: Popup.Item
        model: win.menuModel
        onTriggered: id => { win.fromAppMenu = true; win.menuAction(id); win.fromAppMenu = false }
    }
    function openAppMenu() {
        win.focusBeforeMenu = win.activeFocusItem
        appMenu.openBelow(menuButton)
    }

    // The menus' keys (the menus only show them). Delete, Archive, Junk and
    // the like act once per press (holding Delete once sent row after row to
    // the Trash), and keys that type in text fields (Alt, Delete) wait while
    // typing.
    Shortcut { sequence: "Shift+F1"; onActivated: win.menuAction("about") }
    Shortcut { sequence: "Ctrl+Alt+A"; enabled: !win.typing; onActivated: win.menuAction("accounts") }
    Shortcut { sequence: "Ctrl+Alt+G"; enabled: !win.typing; onActivated: win.menuAction("signIn") }
    Shortcut { sequence: "Ctrl+F1"; onActivated: win.menuAction("setupGuide") }
    Shortcut { sequence: "Ctrl+Shift+E"; onActivated: win.menuAction("newEvent") }
    Shortcut { sequence: "Ctrl+Shift+C"; onActivated: win.menuAction("newContact") }
    Shortcut { sequence: "Ctrl+Shift+S"; enabled: !!mailView.current && mailView.current.attachments.length > 0; onActivated: win.menuAction("saveAttachments") }
    Shortcut { sequence: "Ctrl+Shift+I"; onActivated: win.menuAction("importMailboxes") }
    Shortcut { sequence: "Ctrl+Alt+I"; enabled: !win.typing; onActivated: win.menuAction("importVcard") }
    Shortcut { sequence: "Ctrl+Alt+E"; enabled: !win.typing; onActivated: win.menuAction("exportVcard") }
    Shortcut { sequence: "Ctrl+Shift+P"; enabled: win.hasMessage; onActivated: win.menuAction("exportPdf") }
    Shortcut { sequence: "Del"; autoRepeat: false; enabled: win.section === 0 && win.hasMessage && !win.typing; onActivated: win.menuAction("delete") }
    // Undo, outside text: the last delete or move put back.
    Shortcut { sequence: "Ctrl+Z"; autoRepeat: false; enabled: !win.typing && store.mailUndoText !== ""; onActivated: store.undoMail() }
    Shortcut { sequence: "Ctrl+Shift+O"; onActivated: win.menuAction("threaded") }
    Shortcut { sequence: "Ctrl+Alt+H"; enabled: !win.typing; onActivated: win.menuAction("layoutStandard") }
    Shortcut { sequence: "Ctrl+Alt+K"; enabled: !win.typing; onActivated: win.menuAction("layoutClassic") }
    Shortcut { sequence: "Ctrl+Alt+Up"; enabled: !win.typing; onActivated: win.menuAction("sortAscending") }
    Shortcut { sequence: "Ctrl+Alt+Down"; enabled: !win.typing; onActivated: win.menuAction("sortDescending") }
    Shortcut { sequence: "Ctrl+Alt+M"; enabled: !win.typing; onActivated: win.menuAction("allMessages") }
    Shortcut { sequence: "Ctrl+Alt+U"; enabled: !win.typing; onActivated: win.menuAction("onlyUnread") }
    Shortcut { sequence: "Ctrl+Alt+F"; enabled: !win.typing; onActivated: win.menuAction("formatted") }
    Shortcut { sequence: "Ctrl+Alt+P"; enabled: !win.typing; onActivated: win.menuAction("plainText") }
    Shortcut { sequence: "Ctrl+Alt+Shift+R"; enabled: !win.typing; onActivated: win.menuAction("loadRemote") }
    Shortcut { sequence: "Ctrl+Alt+Shift+L"; enabled: !win.typing; onActivated: win.menuAction("themeLight") }
    Shortcut { sequence: "Ctrl+Alt+Shift+D"; enabled: !win.typing; onActivated: win.menuAction("themeDark") }
    Shortcut { sequence: "Ctrl+Alt+Shift+M"; enabled: !win.typing; onActivated: win.menuAction("themeSystem") }
    Shortcut { sequence: "Ctrl+Shift+Del"; autoRepeat: false; onActivated: win.menuAction("eraseTrash") }
    Shortcut { sequence: "Ctrl+Alt+J"; autoRepeat: false; enabled: !win.typing; onActivated: win.menuAction("eraseJunk") }
    Shortcut { sequence: "Ctrl+Alt+Shift+N"; enabled: !win.typing; onActivated: win.menuAction("newSmartMailbox") }
    Shortcut { sequence: "Ctrl+Alt+Shift+U"; autoRepeat: false; enabled: !win.typing; onActivated: win.menuAction("markAllRead") }
    Shortcut { sequence: "Alt+1"; enabled: !win.typing; onActivated: win.menuAction("go:inbox") }
    Shortcut { sequence: "Alt+2"; enabled: !win.typing; onActivated: win.menuAction("go:drafts") }
    Shortcut { sequence: "Alt+3"; enabled: !win.typing; onActivated: win.menuAction("go:sent") }
    Shortcut { sequence: "Alt+4"; enabled: !win.typing; onActivated: win.menuAction("go:archive") }
    Shortcut { sequence: "Alt+5"; enabled: !win.typing; onActivated: win.menuAction("go:junk") }
    Shortcut { sequence: "Alt+6"; enabled: !win.typing; onActivated: win.menuAction("go:trash") }
    Shortcut { sequence: "Ctrl+Alt+R"; enabled: !win.typing; onActivated: win.menuAction("rules") }
    Shortcut { sequence: "Ctrl+Alt+Shift+G"; enabled: !win.typing; onActivated: win.menuAction("signatures") }
    Shortcut { sequence: "Ctrl+Shift+D"; autoRepeat: false; enabled: win.hasMessage; onActivated: win.menuAction("sendAgain") }
    Shortcut { sequence: "Ctrl+Shift+M"; autoRepeat: false; enabled: win.hasMessage; onActivated: win.menuAction("mute") }
    Shortcut { sequence: "Ctrl+Alt+Shift+A"; autoRepeat: false; enabled: !win.typing; onActivated: win.menuAction("applyRules") }
    Shortcut { sequence: "Ctrl+Alt+V"; enabled: win.hasMessage && !win.typing; onActivated: win.menuAction("vip") }
    Shortcut { sequence: "Ctrl+Alt+B"; autoRepeat: false; enabled: win.hasMessage && !win.typing; onActivated: win.menuAction("block") }
    Shortcut { sequence: "Ctrl+Alt+Shift+H"; enabled: win.hasMessage && !win.typing; onActivated: win.menuAction("remind:hour") }
    Shortcut { sequence: "Ctrl+Alt+Shift+T"; enabled: win.hasMessage && !win.typing; onActivated: win.menuAction("remind:tonight") }
    Shortcut { sequence: "Ctrl+Alt+Shift+O"; enabled: win.hasMessage && !win.typing; onActivated: win.menuAction("remind:tomorrow") }
    Shortcut { sequence: "Ctrl+Alt+Shift+W"; enabled: win.hasMessage && !win.typing; onActivated: win.menuAction("remind:week") }
    Shortcut { sequence: "Ctrl+Alt+Shift+X"; enabled: win.hasMessage && !win.typing; onActivated: win.menuAction("remind:none") }
    Shortcut { sequence: "Ctrl+Alt+O"; enabled: !win.typing; onActivated: win.menuAction("outbox") }
    Shortcut { sequence: "Ctrl+Alt+N"; enabled: !win.typing; onActivated: win.menuAction("newCalendar") }
    Shortcut { sequence: "Ctrl+Alt+Shift+S"; enabled: !win.typing; onActivated: win.menuAction("newSubscription") }
    Shortcut { sequence: "Ctrl+Alt+1"; enabled: !win.typing; onActivated: win.menuAction("cal:day") }
    Shortcut { sequence: "Ctrl+Alt+2"; enabled: !win.typing; onActivated: win.menuAction("cal:week") }
    Shortcut { sequence: "Ctrl+Alt+3"; enabled: !win.typing; onActivated: win.menuAction("cal:month") }
    Shortcut { sequence: "Ctrl+Alt+4"; enabled: !win.typing; onActivated: win.menuAction("cal:year") }
    Shortcut { sequence: "Ctrl+Alt+Shift+F"; enabled: !win.typing; onActivated: win.menuAction("refreshSubscriptions") }
    Shortcut { sequence: "Ctrl+Alt+Shift+K"; enabled: !win.typing; onActivated: win.menuAction("newGroup") }
    Shortcut { sequence: "Ctrl+Alt+Shift+J"; enabled: !win.typing; onActivated: win.menuAction("newSmartGroup") }
    Shortcut { sequence: "Ctrl+Alt+D"; enabled: !win.typing; onActivated: win.menuAction("duplicates") }
    Shortcut { sequence: "Ctrl+Alt+Shift+I"; enabled: !win.typing; onActivated: win.menuAction("myCard") }
    Shortcut { sequence: "Ctrl+Alt+Z"; enabled: !win.typing; onActivated: win.menuAction("zoom") }
    Shortcut { sequence: "Ctrl+Alt+0"; enabled: !win.typing; onActivated: win.menuAction("messageViewer") }
    Shortcut { sequence: "F1"; onActivated: win.menuAction("keys") }
    Shortcut { sequence: "Ctrl+,"; onActivated: settingsDialog.open() }
    Shortcut { sequence: "Ctrl+P"; onActivated: win.menuAction("print") }
    Shortcut { sequence: "F11"; onActivated: win.menuAction("fullScreen") }
    Shortcut { sequence: "Ctrl+M"; onActivated: win.showMinimized() }
    Shortcut { sequence: "Ctrl+Shift+N"; enabled: win.section !== 0; onActivated: store.refreshAll() }
    Shortcut { sequence: "Ctrl+Q"; onActivated: Qt.quit() }

    Popup {
        id: shortcutsPopup
        anchors.centerIn: Overlay.overlay
        modal: true
        padding: 22
        background: Rectangle { radius: 14; color: Theme.popup; border.color: Theme.border }
        contentItem: ColumnLayout {
            spacing: 6
            Text { text: "Keyboard Shortcuts"; font.pixelSize: 18; font.weight: Font.Bold; color: Theme.text; bottomPadding: 6 }
            GridLayout {
                columns: 2
                columnSpacing: 24
                rowSpacing: 4
                Repeater {
                    model: [["Ctrl+1 / 2 / 3", "Mail / Calendar / Contacts"], ["Ctrl+N", "New message, event or contact"],
                            ["Ctrl+R / Ctrl+Shift+R", "Reply / Reply All"], ["Ctrl+Shift+F", "Forward"], ["Ctrl+Shift+D", "Send Again"], ["Ctrl+Return", "Send"],
                            ["Ctrl+Shift+N", "Get Mail"], ["Ctrl+Shift+U", "Mark as read / unread"], ["Ctrl+Shift+L", "Flag"],
                            ["Ctrl+Shift+A / J", "Archive / Junk"], ["Delete", "Delete message"], ["Ctrl+F", "Search"],
                            ["Ctrl+T", "Calendar: today"], ["Ctrl+← / →", "Calendar: previous / next"], ["Ctrl+B / I / U", "Bold / italic / underline"],
                            ["Ctrl+S", "Save draft"], ["Ctrl+,", "Settings"], ["Ctrl+Alt+S", "Show / hide sidebar"], ["Ctrl+Q", "Quit"]]
                    delegate: Text {
                        required property var modelData
                        required property int index
                        text: modelData[0] + "\t" + modelData[1]
                        color: Theme.text
                        font.pixelSize: Theme.base
                    }
                }
            }
            ToolButtonX { Layout.alignment: Qt.AlignRight; text: "Close"; onClicked: shortcutsPopup.close() }
        }
    }
    Popup {
        id: aboutPopup
        anchors.centerIn: Overlay.overlay
        modal: true
        padding: 24
        background: Rectangle { radius: 14; color: Theme.popup; border.color: Theme.border }
        contentItem: ColumnLayout {
            spacing: 8
            Text { text: "Almanac"; font.pixelSize: 22; font.weight: Font.Bold; color: Theme.text }
            Text { text: "Version " + Qt.application.version; color: Theme.secondary; visible: Qt.application.version !== "" }
            Text { text: "Mail, Calendar and Contacts for Linux, Windows and macOS.\nBuilt with Rust and Qt."; color: Theme.secondary }
            Text {
                text: "<a href=\"https://github.com/benwbooth/almanac\">github.com/benwbooth/almanac</a>"
                textFormat: Text.StyledText
                linkColor: Theme.accent
                onLinkActivated: link => Qt.openUrlExternally(link)
                HoverHandler { cursorShape: Qt.PointingHandCursor }
            }
            ToolButtonX { Layout.alignment: Qt.AlignRight; text: "Close"; onClicked: aboutPopup.close() }
        }
    }

    // `--show accounts|add-account|compose|event` opens a panel at start (for checking pictures).
    Timer {
        id: listUpdateTimer
        interval: 400
        // Something changes in the background (another message is marked read).
        onTriggered: { var rows = JSON.parse(store.messages("kind:inbox", "", true)); store.setRead(rows[1].id, !rows[1].read) }
    }
    // ---------------------------------------- saved window and view state
    // Size, position, maximised, the open section and what each view shows,
    // kept from one run to the next (not when taking pictures).
    Settings { id: savedState; category: "WindowState" }
    readonly property bool keepState: store.screenshotPath === "" || store.arg("--keep-state") === "1"
    // Plasma on Wayland: KWin remembers the window's place (a window rule of
    // Plasma's own); the place kept here is only for elsewhere.
    property bool placedByKWin: false
    function saveState() {
        if (!keepState) return
        var st = {
            // The normal (not maximised) place and size, whatever the window is now.
            window: { x: savedX, y: savedY, w: savedW, h: savedH,
                      onWayland: placedByKWin,
                      mode: win.visibility === Window.Maximized ? "max" : (win.visibility === Window.FullScreen ? "full" : "normal") },
            section: win.section,
            mail: mailView.saveState(), calendar: calendarView.saveState(), contacts: contactsView.saveState()
        }
        savedState.setValue("ui", JSON.stringify(st))
        savedState.sync()
    }
    // The normal (not maximised) size, to come back to.
    property real savedW: width
    property real savedH: height
    property real savedX: x
    property real savedY: y
    // Noted as the window moves or resizes while it's a normal window.
    onVisibilityChanged: Qt.callLater(noteNormalSize)
    // Kept once the size has settled (while maximising or leaving full screen the
    // size passes through the screen's, which mustn't be taken as the normal size).
    // A window snapped to the screen's full height or width (tiled to a side
    // or edge) isn't its normal size either: the last free-floating size is kept.
    function noteNormalSize() {
        if (win.visibility !== Window.Windowed) return
        // (Where Plasma places the window, it says exactly where; elsewhere a
        // window as big as the screen is the desktop's, not a size to keep.)
        if (!placedByKWin && (win.height >= Screen.desktopAvailableHeight - 8 || win.width >= Screen.desktopAvailableWidth - 8)) return
        win.savedW = win.width; win.savedH = win.height
        // (On Wayland a window's own x and y are always 0: there the place comes
        // from KWin, and the one restored is kept until KWin says otherwise.)
        if (!placedByKWin && Qt.platform.pluginName !== "wayland") { win.savedX = win.x; win.savedY = win.y }
    }
    function restoreState() {
        if (!keepState) return
        var raw = savedState.value("ui", "")
        if (!raw) { restored = true; win.placedByKWin = store.placeWindow(0, 0, 0, 0, false); return }
        var st
        try { st = JSON.parse(raw) } catch (e) { return }
        // Plasma on Wayland: KWin remembers where the window was and whether it
        // was maximised (its own window rule); its size is set here.
        if (st.window) win.placedByKWin = store.placeWindow(0, 0, 0, 0, st.window.mode === "max")
        if (st.window) {
            // A little inside the screen's free area: a window as big as the
            // screen is maximised by the desktop (KDE does this), so it would come
            // back filling the screen even though it wasn't.
            var room = Qt.application.screens.length ? Qt.application.screens[0] : null
            for (var s = 0; s < Qt.application.screens.length; s++) {
                var on = Qt.application.screens[s]
                if (st.window.x >= on.virtualX && st.window.x < on.virtualX + on.width && st.window.y >= on.virtualY && st.window.y < on.virtualY + on.height) room = on
            }
            // (Placed by KWin, on Plasma with Wayland: the size as it was, up to the
            // free area; it keeps the window's place and maximised state itself.
            // Taking 48 px off there made the window come back a little short of
            // the bottom every time: the title bar is smaller than that.)
            var margin = win.placedByKWin ? 0 : 48
            var maxW = room ? Math.min(room.width, room.desktopAvailableWidth) - margin : 100000
            var maxH = room ? Math.min(room.height, room.desktopAvailableHeight) - margin : 100000
            var wantW = Math.min(st.window.w || 0, maxW), wantH = Math.min(st.window.h || 0, maxH)
            // Saved while snapped to the full height or width of the screen: it would
            // open filling the screen, so it opens at a comfortable size instead.
            // (Not where Plasma places it: it puts it exactly as it was.)
            if (room && !st.window.onWayland && (st.window.h >= maxH || st.window.w >= maxW)) {
                wantW = Math.min(1280, Math.round(maxW * 0.8)); wantH = Math.min(860, Math.round(maxH * 0.8))
                st.window.x = undefined
            }
            if (wantW >= minimumWidth && wantH >= minimumHeight) { width = wantW; height = wantH; savedW = width; savedH = height }
            if (st.window.x !== undefined) { savedX = st.window.x; savedY = st.window.y }
            // Back where it was, if that spot is still on a connected screen
            // (otherwise the system places it). Wayland desktops place windows themselves.
            if (!win.placedByKWin && st.window.x !== undefined && st.window.y !== undefined) {
                var onScreen = false
                for (var i = 0; i < Qt.application.screens.length; i++) {
                    var sc = Qt.application.screens[i]
                    if (st.window.x + 80 >= sc.virtualX && st.window.x + 80 <= sc.virtualX + sc.width
                        && st.window.y + 20 >= sc.virtualY && st.window.y + 20 <= sc.virtualY + sc.height) onScreen = true
                }
                if (onScreen && room) {
                    x = Math.max(room.virtualX, Math.min(st.window.x, room.virtualX + room.width - width))
                    y = Math.max(room.virtualY, Math.min(st.window.y, room.virtualY + room.height - height))
                }
            }
            var max = st.window.mode === "max"
            // Elsewhere (X11, Windows, macOS): placed above, then maximised; the
            // system remembers the normal place for un-maximising. Full screen comes
            // back as a normal window (starting in full screen is never wanted).
            if (max && !win.placedByKWin) Qt.callLater(win.showMaximized)
        }
        if (store.arg("--open") === "" && st.section !== undefined) section = st.section
        // After every view has finished starting (they set their own defaults then).
        Qt.callLater(function() {
            mailView.restoreState(st.mail)
            calendarView.restoreState(st.calendar)
            contactsView.restoreState(st.contacts)
            win.restored = true
        })
    }
    // Closing the window quits (the tray icon is how to hide it and keep it running).
    onClosing: { mailView.composer.saveIfUnsaved(); saveState(); Qt.callLater(Qt.quit) }
    Connections { target: Qt.application; function onAboutToQuit() { mailView.composer.saveIfUnsaved(); win.saveState(); store.flush() } }
    // Size and position are saved a moment after the window is moved or
    // resized, so even a crash or a forced quit keeps them.
    Timer { id: geometrySave; interval: 1000; onTriggered: { win.noteNormalSize(); win.saveState() } }
    onXChanged: if (keepState && restored) geometrySave.restart()
    onYChanged: if (keepState && restored) geometrySave.restart()
    onHeightChanged: if (keepState && restored) geometrySave.restart()
    property bool restored: false
    // Our toolbar icons → the desktop's icon theme, for menus.
    readonly property var themeIcons: ({
        compose: "mail-message-new", archive: "archive-insert", trash: "edit-delete", junk: "mail-mark-junk",
        reply: "mail-reply-sender", replyAll: "mail-reply-all", forward: "mail-forward", flag: "flag-red",
        flagOutline: "flag-red", envelopeOpen: "mail-mark-read", unread: "mail-mark-unread", search: "edit-find",
        plus: "list-add", minus: "list-remove", chevronLeft: "go-previous", chevronRight: "go-next",
        sidebar: "view-sidetree", refresh: "view-refresh", calendar: "view-calendar", contacts: "x-office-address-book",
        mail: "mail-message", printer: "document-print", export: "document-export", import: "document-import",
        filter: "view-filter", people: "system-users", gear: "configure", bell: "notifications", undo: "edit-undo",
        star: "emblem-favorite", starOutline: "emblem-favorite", clock: "chronometer", popOut: "window-new",
        paperclip: "mail-attachment", more: "overflow-menu", sun: "flashlight-on", moon: "flashlight-off" })
    // "Reply (Ctrl+R)" → "Ctrl+R"
    function buttonShortcut(b) { var m = /\(((?:Ctrl|Alt|Shift|F\d)[^)]*)\)/.exec(b.tip || ""); return m ? m[1] : "" }
    // Typing in a text field: Alt shortcuts (AltGr on some keyboards types
    // characters with them) and Delete belong to the text.
    readonly property bool typing: activeFocusItem !== null && activeFocusItem.cursorPosition !== undefined
    // Also every half minute, so even a crash loses little.
    Timer { interval: 30000; repeat: true; running: win.keepState; onTriggered: win.saveState() }

    // (For timing start-up: the first picture on screen.)
    property bool shown: false
    onFrameSwapped: if (!shown) { shown = true; store.startupMark("first picture on screen") }
    Component.onCompleted: {
        // (Once: the mail's conversations worked out in the background for the list.)
        Qt.callLater(store.buildConversations)
        Qt.callLater(function() { win.menusReady = true })
        store.startupMark("window set up")
        // Window buttons drawn like the desktop's own (Plasma's round Breeze ones here).
        if (Qt.platform.os === "linux") Theme.windowButtons = /GNOME|Unity|Pantheon|Budgie|Cinnamon|MATE|XFCE/i.test(store.desktopName()) ? "adwaita" : "breeze"
        if (store.arg("--buttons") !== "") Theme.windowButtons = store.arg("--buttons")
        restoreState()
        // --width N / --height N (for pictures of narrow or short windows)
        var w = parseInt(store.arg("--width"))
        if (w > 0) width = w
        var h = parseInt(store.arg("--height"))
        if (h > 0) height = h
        if (parseInt(store.arg("--list-width")) > 0) mailView.setListWidth(parseInt(store.arg("--list-width")))
        var p = store.startPanel
        if (p === "accounts") sourcesDialog.open()
        else if (p === "type-search") typeCheck.begin()
        else if (p === "swipe-open") swipeCheck.start()
        else if (p === "search-open") Qt.callLater(function() { mailView.focusSearch() })
        else if (p === "search-query") Qt.callLater(function() { mailView.focusSearch(); mailView.typeQuery(store.arg("--query") || "project") })
        else if (p === "add-account") { sourcesDialog.open(); sourcesDialog.newAccount() }
        else if (p === "find-message") findCheck.start()
        else if (p === "message-tab") Qt.callLater(function() { mailView.openInTab(mailView.selectedId) })
        // Many tabs (more than fit), the third one open.
        // A picture from a message in a tab of its own.
        else if (p === "attachment-tab") { mailView.selectSubject(store.arg("--subject") || "Photos from the trip"); Qt.callLater(function() { mailView.openAttachmentTab(mailView.current.id, mailView.current.attachments[0]) }) }
        else if (p === "tabs-many") Qt.callLater(function() { var l = mailView.list; for (var i = 0; i < Math.min(9, l.length); i++) mailView.openInTab(l[i].id); mailView.tabIndex = 3 })
        else if (p === "choose-provider") { sourcesDialog.open(); sourcesDialog.chooseProvider() }
        else if (p === "add-proton") { sourcesDialog.open(); sourcesDialog.pickProvider("proton") }
        // One message by its id (--id N), for pictures and checks against a copy of real mail.
        else if (p === "message") Qt.callLater(function() { mailView.openMessageById(parseInt(store.arg("--id"))) })
        else if (p === "compose") mailView.composeNew()
        // Paragraphs (Enter) and line breaks (Shift+Enter) in a new message: logs what's sent.
        else if (p === "compose-paragraphs") {
            mailView.composer.openWith({ to: "riley@example.org", subject: "Paragraphs" })
            Qt.callLater(function() {
                var c = mailView.composer
                c.typeForCheck("Hi Riley,")
                c.enterForCheck(false); c.typeForCheck("Here are my notes for the kickoff.", true)
                c.enterForCheck(false); c.typeForCheck("First line", true)
                c.enterForCheck(true); c.typeForCheck("second line of the same paragraph.", true)
                c.enterForCheck(false); c.typeForCheck("Thanks,", true)
                var m = c.payload()
                console.log("PARAGRAPHS-HTML", m.html)
                console.log("PARAGRAPHS-TEXT", JSON.stringify(m.body))
            })
        }
        // Send Again on a message with pictures attached.
        else if (p === "send-again") { mailView.selectSubject("Photos from the trip"); Qt.callLater(function() { mailView.sendAgain() }) }
        // A new message with a file attached (--attach <path>), e.g. a big one.
        else if (p === "compose-attach") mailView.composer.openWith({ to: "me@example.com", subject: "Big file", body: "Here it is.\n", attachments: [store.arg("--attach")] })
        else if (p === "event") calendarView.editFirst()
        else if (p === "invite") mailView.selectInvite()
        else if (p === "newsletter") mailView.selectSubject("Spring newsletter")
        else if (p === "photos") mailView.selectSubject("Photos from the trip")
        else if (p === "pdf-message") mailView.selectSubject("Project kickoff")
        // Printing a PDF attachment (the print dialog should open).
        else if (p === "print-pdf") { mailView.selectSubject("Project kickoff"); printCheck.start() }
        // Two trackpad swipes the same way, the second while the first still glides:
        // logs where the page is (it must keep going, not stop).
        else if (p === "glide-test") {
            mailView.selectSubject(store.arg("--subject") || "Spring newsletter")
            glideTest.start()
        }
        // A reply written right under the message (its own Reply button).
        else if (p === "reply-inline") { mailView.selectSubject(store.arg("--subject") || "Photos from the trip"); Qt.callLater(function() { mailView.messageAction(mailView.current, "reply", true)
            // --to-end: the end of the quoted original in view.
            if (store.arg("--to-end") !== "") Qt.callLater(function() { var b = mailView.composer.bodyArea; b.cursorPosition = b.length })
            // --log-colours: the colours what would be sent has (the originals, not the dark ones).
            if (store.arg("--log-colours") !== "") console.log("SENT-COLOURS", JSON.stringify(mailView.composer.payload().html.match(/(background-color|color|bgcolor)[:=]"?#[0-9a-fA-F]{6}/g))) }) }
        // Return in the middle of the quote, then a line of reply there.
        else if (p === "reply-split") {
            mailView.selectSubject("Photos from the trip")
            Qt.callLater(function() {
                mailView.messageAction(mailView.current, "reply", true)
                var c = mailView.composer, b = c.bodyArea, t = b.getText(0, b.length)
                b.cursorPosition = t.indexOf("album.") + 6
                c.splitQuote()
                b.insert(b.cursorPosition, "Love the sunsets!")
            })
        }
        else if (p === "reply-inline-trash") { mailView.selectSubject("Photos from the trip"); Qt.callLater(function() { mailView.messageAction(mailView.current, "reply", true); mailView.composer.typeForCheck("Thanks!"); mailView.composer.discard() }) }
        // Spelling and grammar underlines (and, with --spell-menu, the fixes for the first).
        else if (p === "compose-spelling") {
            mailView.newMessage()
            mailView.composer.typeForCheck("Hi Sam, I recieved the the letter yesterdy and it look great.")
        }
        else if (p === "compose-window") { mailView.newMessage(); mailView.composer.popOut() }
        else if (p === "compose-md") {
            mailView.newMessage()
            mailView.composer.typeForCheck("# Plan for Thursday\nPlease bring **your notes** and the *draft budget* -- it's \"nearly\" done. Code: `npm run build` and [the doc](https://example.com).\n- timeline")
        }
        else if (p === "compose-list") {
            // A list, then Tab on its 2nd and 3rd items and Shift+Tab on the 3rd's child.
            mailView.newMessage()
            var c = mailView.composer
            c.bodyArea.text = "<ul><li>fruit</li><li>apples</li><li>pears</li><li>beans</li></ul><p>end</p>"
            var t = c.plainText()
            c.bodyArea.cursorPosition = t.indexOf("apples") + 2; c.nestList(true)
            c.bodyArea.cursorPosition = c.plainText().indexOf("pears") + 2; c.nestList(true)
            c.bodyArea.cursorPosition = c.plainText().indexOf("pears") + 2; c.nestList(true)
            c.bodyArea.cursorPosition = c.plainText().indexOf("beans") + 2; c.nestList(false)
        }
        else if (p === "sort-from") mailView.setSort("from", true)
        else if (p === "reply") Qt.callLater(function() { mailView.reply(false) })
        else if (p === "message-window") Qt.callLater(function() { mailView.openInWindow(mailView.selectedId); win.visible = false })
        // Lots typed into an event's notes: the notes scroll to keep the cursor in view.
        else if (p === "event-notes-long") { win.section = 1; Qt.callLater(function() {
            var e = calendarView.events.filter(function(x) { return x.title === "Design review" }); if (!e.length) return
            calendarView.editEvent(e[0])
            var a = calendarView.eventEditor.notesArea
            a.forceActiveFocus(); a.cursorPosition = a.length
            for (var i = 1; i <= 25; i++) a.insert(a.cursorPosition, "<br>Line " + i + " of the agenda")
        }) }
        else if (p === "event-edit") { win.section = 1; Qt.callLater(function() { var e = calendarView.events.filter(function(x) { return x.title === "Design review" }); if (e.length) calendarView.editEvent(e[0]) }) }
        else if (p === "event-details") { win.section = 1; Qt.callLater(function() { var e = calendarView.events.filter(function(x) { return x.title === "Design review" }); if (e.length) calendarView.edit(e[0]) }) }
        else if (p === "code-block") { mailView.newMessage(); Qt.callLater(function() {
            var c = mailView.composer
            c.typeForCheck("Here's the fix:\n```py\ndef double(x):")
            c.typeForCheck("\n    # twice as much\n    return x * 2\n```\nThanks!", true)
            c.refreshCode()
            if (store.arg("--log-sent") === "1") {
                var sent = c.payload().html, k = sent.indexOf("<pre")
                console.warn("ALMANAC-SENT " + sent.slice(k, k + 900).replace(/\n/g, "¶"))
                var back = store.codeBlocksForEditing(sent, false)
                console.warn("ALMANAC-REOPENED blocks " + (back.match(/bgcolor="#eaeef2"/g) || []).length)
            }
        }) }
        else if (p === "notify-click") Qt.callLater(function() {
            var all = JSON.parse(store.events("2000-01-01T00:00", "2100-01-01T00:00")).filter(function(e) { return e.title === "Design review" })
            if (all.length) store.openRequest = JSON.stringify({ event: all[0].id, start: all[0].start, at: 1 })
        })
        else if (p === "pdf-tab") Qt.callLater(function() { mailView.selectSubject("Project kickoff"); mailView.openAttachmentTab(mailView.selectedId, "Agenda.pdf") })
        else if (p.indexOf("cal-") === 0) { win.section = 1; Qt.callLater(function() { Qt.callLater(function() { calendarView.view = p.slice(4) }) }) }
        else if (p === "pdf-scroll") { mailView.selectSubject("Project kickoff"); pdfCheck.start() }
        else if (p === "tabs") Qt.callLater(function() { for (var i = 0; i < 3; i++) mailView.openInTab(mailView.list[i].id); mailView.openAttachmentTab(mailView.list[1].id, "Agenda.pdf"); mailView.tabIndex = 2 })
        else if (p === "tab-moves") Qt.callLater(function() {
            var mv = mailView
            var ids = function(o) { return o.tabs.map(function(t) { return mv.list.findIndex(function(r) { return r.id === t.id }) }).join(",") }
            for (var i = 0; i < 4; i++) mv.openInTab(mv.list[i].id)
            console.warn("ALMANAC-TABS opened " + ids(mv) + " open " + mv.tabIndex)
            mv.moveTab(0, 4)
            console.warn("ALMANAC-TABS first to the end " + ids(mv) + " open " + mv.tabIndex)
            mv.moveTab(3, 1)
            console.warn("ALMANAC-TABS last to second " + ids(mv) + " open " + mv.tabIndex)
            mv.tearOut(0)
            var w = mv.tabWindows[0]
            console.warn("ALMANAC-TABS torn out: main " + ids(mv) + ", window " + ids(w) + ", windows " + mv.tabWindows.length)
            w.insertTab(mv.takeTab(1), 0)
            console.warn("ALMANAC-TABS one to the window: main " + ids(mv) + ", window " + ids(w) + " open " + w.tabIndex)
            w.tearOut(1)
            console.warn("ALMANAC-TABS window's second out: windows " + mv.tabWindows.length + ", " + mv.tabWindows.map(ids).join(" | "))
            mv.mergeAllWindows()
            console.warn("ALMANAC-TABS merged: main " + ids(mv) + ", windows " + mv.tabWindows.length + " open " + mv.tabIndex)
            mv.closeOtherTabs(1)
            console.warn("ALMANAC-TABS others closed: main " + ids(mv) + " open " + mv.tabIndex)
        })
        else if (p === "tab-window") Qt.callLater(function() {
            var w = mailView.openInWindow(mailView.list[0].id)
            w.insertTab(mailView.messageTab(mailView.list[2].id), 1)
            w.insertTab(mailView.messageTab(mailView.list[3].id), 2)
            w.tabIndex = 1
            win.visible = false
        })
        else if (p === "progress") { store.progressDone = 420; store.progressTotal = 1200; store.statusText = "Work: downloading messages (420 of 1200)…" }
        // Something going on whose length isn't known: the notice's moving bar.
        else if (p === "progress-unknown") { store.busy = true; store.statusText = "Work: signing in…" }
        else if (p === "classic") mailView.setLayout("classic")
        else if (p === "pdf") mailView.selectSubject("Project kickoff")
        // Check: background updates keep the list where it is.
        else if (p === "list-update") {
            mailView.selectSubject("Quarterly report")
            mailView.listView.contentY = 260
            listUpdateTimer.start()
        }
        else if (p === "empty") store.onboarded = true
        else if (p === "settings") settingsDialog.open()
        else if (p === "settings-mail") { settingsDialog.tab = "mail"; settingsDialog.open() }
        else if (p === "settings-privacy") { settingsDialog.tab = "privacy"; settingsDialog.open() }
        else if (p === "chip-edit") Qt.callLater(function() { mailView.reply(false); Qt.callLater(function() { mailView.composer.toBox.field.editAt(0) }) })
        else if (/^picker-/.test(p)) { mailView.newMessage(); Qt.callLater(function() { mailView.composer.pickers.openPicker(p.slice(7)) }) }
        else if (p === "table") { mailView.newMessage(); Qt.callLater(function() { mailView.composer.bodyArea.forceActiveFocus(); mailView.composer.pickers.insertTable(3, 3) }) }
        else if (p === "table-tab") {
            // A table, "one" typed in the first cell, Tab twice, "three" typed.
            mailView.newMessage()
            Qt.callLater(function() {
                var c = mailView.composer, b = c.bodyArea
                b.forceActiveFocus(); c.pickers.insertTable(2, 3)
                b.insert(b.cursorPosition, "one"); c.nestList(true); b.insert(b.cursorPosition, "two"); c.nestList(true); b.insert(b.cursorPosition, "three")
            })
        }
        else if (p === "from-menu") { mailView.newMessage(); Qt.callLater(function() { mailView.composer.fromMenu.popup.open() }) }
        else if (p === "signin") signInSetup.openFor("microsoft", false)
        else if (p === "signin-other") { signInSetup.openFor("microsoft", false); Qt.callLater(function() { signInSetup.showOthers() }) }
        else if (p === "signin-google") signInSetup.openFor("google", false)
        // signin-step3: Google's setup at that step (signin-steps: the first)
        else if (p.indexOf("signin-step") === 0) { signInSetup.openFor("google", false); signInSetup.gStep = parseInt(p.slice(11)) || 1 }
        else if (p === "menu") Qt.callLater(win.openAppMenu)
    }

    Shortcut { sequence: "Ctrl+1"; onActivated: win.section = 0 }
    Shortcut { sequence: "Ctrl+2"; onActivated: win.section = 1 }
    Shortcut { sequence: "Ctrl+3"; onActivated: win.section = 2 }
    Shortcut { sequence: "Ctrl+Alt+S"; onActivated: win.views[win.section].sidebarShown = !win.views[win.section].sidebarShown }

    // Outbox (Undo Send / Send Later), reminders and calendar alerts.
    Timer {
        interval: 2000
        repeat: true
        running: store.screenshotPath === ""
        onTriggered: store.tick()
    }

    // Sign-in pages, attachments and print-outs open in the system's apps.
    Connections {
        target: store
        function onOpenUrlChanged() { if (store.openUrl !== "") { Qt.openUrlExternally(store.openUrl); store.openUrl = "" } }
    }

    Timer { id: swipeCheck; interval: 1200; onTriggered: mailView.swipeOpenForCheck(1) }
    // --show find-message [--query word]: Find in Message open on the shown message.
    Timer { id: findCheck; interval: 1500; onTriggered: { var pg = mailView.shownPage(); if (pg) pg.findFor(store.arg("--query") || "the") } }
    Timer { id: pdfCheck; interval: 2600; onTriggered: mailView.checkPdfBar() }
    // For checks (--show type-search): types searches into the mail search box
    // a letter at a time in several mailboxes, list and conversations, and
    // reports the longest the window went without handling events (a ticker
    // every 4 ms notes the gaps). Only times are printed.
    QtObject {
        id: typeCheck
        property var steps: []
        property int at: 0
        property double last: 0
        property double worst: 0
        property string label: ""
        function begin() {
            var boxes = store.arg("--boxes") !== "" ? store.arg("--boxes").split(",") : ["kind:inbox", "kind:archive", "kind:flagged"]
            if (store.arg("--smart") !== "") boxes.push("smart:" + store.arg("--smart"))
            var words = store.arg("--words") !== "" ? store.arg("--words").split(",") : ["invoice", "github", "a"]
            var s = []
            boxes.forEach(function(b) {
                [false, true].forEach(function(th) {
                    s.push({ select: b, threaded: th })
                    words.forEach(function(w) {
                        for (var i = 1; i <= w.length; i++) s.push({ text: w.slice(0, i), label: b + (th ? " conv " : " list ") + w })
                        s.push({ wait: 2500 })
                        for (var j = w.length - 1; j >= 0; j--) s.push({ text: w.slice(0, j), label: b + (th ? " conv " : " list ") + w + " ⌫" })
                        s.push({ wait: 2500 })
                    })
                })
            })
            steps = s
            at = 0
            stepTimer.start()
            ticker.start()
        }
        function report() {
            if (label !== "") console.warn("ALMANAC-TYPING " + label + ": longest stall " + Math.round(worst) + " ms")
            worst = 0
        }
    }
    Timer {
        id: ticker; interval: 4; repeat: true
        onTriggered: { var n = Date.now(); if (typeCheck.last > 0) typeCheck.worst = Math.max(typeCheck.worst, n - typeCheck.last - 4); typeCheck.last = n }
    }
    Timer {
        id: stepTimer; interval: 110
        onTriggered: {
            if (typeCheck.at >= typeCheck.steps.length) { typeCheck.report(); console.warn("ALMANAC-TYPING done"); Qt.quit(); return }
            var st = typeCheck.steps[typeCheck.at++]
            interval = 110
            if (st.select !== undefined) { typeCheck.report(); typeCheck.label = ""; mailView.typeQuery(""); mailView.selection = st.select; mailView.threaded = st.threaded; interval = 4000 }
            else if (st.wait !== undefined) interval = st.wait
            else { if (st.label !== typeCheck.label) { typeCheck.report(); typeCheck.label = st.label } mailView.typeQuery(st.text) }
            start()
        }
    }
    // Check accounts for new mail and changes (Settings › General).
    Timer {
        readonly property int minutes: { store.revision; return JSON.parse(store.settings()).check_minutes }
        interval: Math.max(1, minutes) * 60 * 1000
        repeat: true
        running: store.screenshotPath === "" && minutes > 0
        onTriggered: if (!store.busy && JSON.parse(store.liveAccounts()).length > 0) store.refreshAll()
    }

    // --screenshot <file.png>: draw ~120 frames, save the window and quit.
    Timer {
        id: shotTimer
        property int frames: 0
        interval: 16
        repeat: true
        running: store.screenshotPath !== ""
        onTriggered: {
            frames++
            // (--frames N: wait longer, for checks of things that take a while.)
            if (frames === (parseInt(store.arg("--frames")) || 120)) {
                stop()
                // The whole window, so open panels and menus are in the picture too.
                if (store.saveWindow(store.screenshotPath)) Qt.quit()
                else shotRoot.grabToImage(function(result) {
                    result.saveToFile(store.screenshotPath)
                    Qt.quit()
                })
            }
        }
    }

    // In the system tray: Almanac's letter, with a blue dot while there's unread
    // mail in the inboxes (read anywhere, here or on another device: no dot).
    // Click it to bring the window up.
    P.SystemTrayIcon {
        visible: store.screenshotPath === ""
        // One colour matching the system's appearance (light glyph on a dark panel).
        icon.source: store.iconUrl((unread > 0 ? "almanac-tray-unread" : "almanac-tray")
                                   + (Qt.styleHints.colorScheme === Qt.ColorScheme.Light ? "-light" : "-dark"))
        // "Almanac — 3 unread messages" (all inboxes), and how many are new since you last looked.
        readonly property int unread: {
            var boxes = mailView.sidebar.smart || []
            for (var i = 0; i < boxes.length; i++) if (boxes[i].id === "kind:inbox") return boxes[i].badge
            return 0
        }
        tooltip: "Almanac — " + unread.toLocaleString(Qt.locale(), "f", 0) + (unread === 1 ? " unread message" : " unread messages")
                 + (store.newMail > 0 ? " (" + store.newMail + " new)" : "")
        // Click: show Almanac, or hide it if it's the window you're using.
        onActivated: function(reason) {
            if (reason === P.SystemTrayIcon.Context) return
            if (win.inUse()) { win.hideToTray(); return }
            win.comeForward()
            if (store.newMail > 0) { win.section = 0; mailView.goTo("inbox", "Inbox") }
        }
        menu: P.Menu {
            P.MenuItem { icon.name: "window"; text: win.visible && win.visibility !== Window.Minimized ? "Minimize Almanac" : "Show Almanac"
                         onTriggered: win.visible && win.visibility !== Window.Minimized ? win.hideToTray() : win.comeForward() }
            P.MenuItem { icon.name: "mail-message-new"; text: "New Message"; onTriggered: { win.comeForward(); win.section = 0; mailView.newMessage() } }
            P.MenuItem { icon.name: "mail-receive"; text: "Get New Mail"; onTriggered: store.refreshAll() }
            P.MenuSeparator {}
            P.MenuItem { icon.name: "application-exit"; text: "Quit Almanac"; onTriggered: Qt.quit() }
        }
    }
    // The window to the front, on the desktop you're on (from the tray, or when
    // Almanac is started again while it runs).
    function comeForward() {
        var wasHidden = !win.visible || win.visibility === Window.Minimized
        // Back as it was (maximised stays maximised); Plasma un-minimises it below.
        if (!win.visible) win.show()
        else if (win.visibility === Window.Minimized && !store.bringForward()) { if (wasMaximized) win.showMaximized(); else win.showNormal() }
        win.raise()
        win.requestActivate()
        store.bringForward()
        // A window shown again (or un-minimised) takes a moment to appear: asked again then.
        if (wasHidden) activateAgain.restart()
    }
    Timer { id: activateAgain; interval: 250; onTriggered: { win.raise(); win.requestActivate(); store.bringForward() } }
    Connections { target: store; function onShowRequestsChanged() { win.comeForward() } }
    // A notification clicked: forward, with its message or event open.
    Connections {
        target: store
        function onOpenRequestChanged() {
            var r = JSON.parse(store.openRequest || "{}")
            win.comeForward()
            if (r.mail !== undefined) { win.section = 0; mailView.openMessageById(r.mail) }
            else if (r.inbox) { win.section = 0; mailView.goTo("inbox", "Inbox") }
            else if (r.event !== undefined) { win.section = 1; calendarView.showEvent(r.event, r.start) }
            else if (r.newEvent) { win.section = 1; calendarView.newEventOn(r.newEvent) }
            else if (r.calendar) win.section = 1
            else if (r.compose) { win.section = 0; mailView.composeFrom(r.compose) }
        }
    }
    // The window you're using: shown, not minimised, and active (or it was a
    // moment ago: clicking the panel's tray icon can take focus away first).
    property double lastActive: 0
    function inUse() {
        return win.visible && win.visibility !== Window.Minimized && (win.active || Date.now() - lastActive < 600)
    }
    // Out of the way until the tray icon (or starting Almanac again) brings it
    // back: minimised, so the desktop keeps its place, size and maximised state
    // (a hidden window is forgotten by Wayland desktops and comes back centred).
    property bool wasMaximized: false
    function hideToTray() {
        win.saveState()
        wasMaximized = win.visibility === Window.Maximized
        win.showMinimized()
    }
    // Looking at the window counts as having seen the new mail.
    onActiveChanged: {
        if (active && store.newMail > 0) store.newMail = 0
        if (!active) lastActive = Date.now()
    }

    // Resizing from the edges (the window has no system frame off macOS).
    ResizeEdges { visible: !win.macOS && win.visibility !== Window.Maximized && win.visibility !== Window.FullScreen; window: win }

    // Calendar alerts stay until you dismiss or snooze them (as in Apple Calendar).
    Column {
        anchors.top: parent.top
        anchors.right: parent.right
        anchors.topMargin: Theme.toolbarHeight + 12
        anchors.rightMargin: 16
        spacing: 8
        z: 50
        Repeater {
            model: JSON.parse(store.alertsJson || "[]")
            delegate: Rectangle {
                required property var modelData
                required property int index
                width: 320
                height: alertCol.implicitHeight + 24
                radius: 12
                color: Theme.popup
                border.color: Theme.border
                Column {
                    id: alertCol
                    x: 14; y: 12
                    width: parent.width - 28
                    spacing: 8
                    Row {
                        spacing: 8
                        Icon { name: "bell"; width: 16; height: 16; color: Theme.red }
                        Text { text: modelData.title; font.pixelSize: 14; font.weight: Font.DemiBold; color: Theme.text; width: alertCol.width - 24; elide: Text.ElideRight }
                    }
                    Text { text: modelData.when; color: Theme.secondary; font.pixelSize: 12; width: alertCol.width; wrapMode: Text.WordWrap }
                    Row {
                        spacing: 6
                        ToolButtonX { text: "Snooze 5 min"; onClicked: store.dismissAlert(index, 5) }
                        ToolButtonX { text: "Snooze 15 min"; onClicked: store.dismissAlert(index, 15) }
                        ToolButtonX { text: "Dismiss"; primary: true; onClicked: store.dismissAlert(index, 0) }
                    }
                }
            }
        }
    }
}
