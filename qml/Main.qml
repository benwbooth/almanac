import QtQuick
import QtQuick.Controls.Basic
// The system's own menus (desktop style on Linux, native on macOS and Windows)
import Qt.labs.platform as P
import QtQuick.Layouts
import QtQuick.Window
import QtCore
import org.craft.pim

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
                // Drag the bar to move the window; double-click to zoom it.
                DragHandler {
                    target: null
                    grabPermissions: PointerHandler.CanTakeOverFromItems | PointerHandler.CanTakeOverFromHandlersOfDifferentType
                    onActiveChanged: if (active) win.startSystemMove()
                }
                TapHandler {
                    gesturePolicy: TapHandler.DragThreshold
                    onDoubleTapped: win.visibility === Window.Maximized ? win.showNormal() : win.showMaximized()
                }
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: win.macOS ? 78 : 10
                    anchors.rightMargin: win.macOS ? 10 : 0
                    spacing: 6

                    ToolButtonX {
                        id: menuButton
                        text: "☰"
                        textSize: 19
                        tip: "Menu: every command, Accounts and Settings"
                        onClicked: appMenu.open(menuButton)
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
                            overflowMenu.open(overflowButton)
                        }
                        P.Menu { id: overflowMenu }
                        Component {
                            id: overflowItem
                            P.MenuItem {
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
                MailView { id: mailView; store: store; onAccountsRequested: sourcesDialog.open() }
                CalendarView { id: calendarView; store: store }
                ContactsView { id: contactsView; store: store; onComposeRequested: function(p) { win.section = 0; mailView.reopen(p) } }
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

    // ☰ menu: every command in one place, laid out like Apple Mail's menu
    // bar (Almanac, File, Edit, View, Mailbox, Message, Window, Help) with
    // Calendar and Contacts added. Shown with the system's own menus.
    function editAction(name) {
        var f = win.activeFocusItem
        if (f && typeof f[name] === "function") f[name]()
    }
    P.Menu {
        id: appMenu
        // The top level has short names: a narrow menu, each section with its icon.
        minimumWidth: 150
        P.Menu {
            title: "Almanac"
            icon.name: "almanac"
            icon.source: store.iconUrl("almanac")
            P.MenuItem { id: menuCmd1; icon.name: "help-about"; shortcut: "Shift+F1"; text: "About Almanac"; onTriggered: aboutPopup.open() }
            P.MenuSeparator {}
            P.MenuItem { icon.name: "configure"; text: "Settings…"; shortcut: "Ctrl+,"; onTriggered: settingsDialog.open() }
            P.MenuItem { id: menuCmd2; icon.name: "system-users"; shortcut: "Ctrl+Alt+A"; text: "Accounts…"; onTriggered: sourcesDialog.open() }
            P.MenuItem { id: menuCmd3; icon.name: "dialog-password"; shortcut: "Ctrl+Alt+G"; text: "Sign-in Setup (Google, Microsoft, Yahoo, AOL)…"; onTriggered: signInSetup.openFor("google", false) }
            P.MenuItem { id: menuCmd4; icon.name: "help-contents"; shortcut: "Ctrl+F1"; text: "Setup Guide"; onTriggered: store.onboarded = false }
            P.MenuSeparator {}
            P.MenuItem { icon.name: "application-exit"; text: "Quit Almanac"; shortcut: "Ctrl+Q"; onTriggered: Qt.quit() }
        }
        P.Menu {
            title: "File"
            icon.name: "folder-open"
            P.MenuItem { icon.name: "mail-message-new"; text: "New Message"; shortcut: "Ctrl+N"; onTriggered: { win.section = 0; mailView.newMessage() } }
            P.MenuItem { id: menuCmd5; icon.name: "appointment-new"; shortcut: "Ctrl+Shift+E"; text: "New Event"; onTriggered: { win.section = 1; calendarView.newEventNow() } }
            P.MenuItem { id: menuCmd6; icon.name: "contact-new"; shortcut: "Ctrl+Shift+C"; text: "New Contact"; onTriggered: { win.section = 2; contactsView.newContact() } }
            P.MenuSeparator {}
            P.MenuItem { id: menuCmd7; icon.name: "mail-attachment"; shortcut: "Ctrl+Shift+S"; text: "Save Attachments…"; enabled: !!mailView.current && mailView.current.attachments.length > 0; onTriggered: mailView.saveAttachments() }
            P.MenuItem { id: menuCmd8; icon.name: "document-import"; shortcut: "Ctrl+Shift+I"; text: "Import Mailboxes…"; onTriggered: sourcesDialog.open() }
            P.MenuItem { id: menuCmd9; icon.name: "document-import"; shortcut: "Ctrl+Alt+I"; text: "Import vCard…"; onTriggered: { win.section = 2; contactsView.importCards() } }
            P.MenuItem { id: menuCmd10; icon.name: "document-export"; shortcut: "Ctrl+Alt+E"; text: "Export vCard…"; onTriggered: { win.section = 2; contactsView.exportCards() } }
            P.MenuItem { id: menuCmd11; icon.name: "application-pdf"; shortcut: "Ctrl+Shift+P"; text: "Export as PDF…"; enabled: mailView.current !== null; onTriggered: mailView.exportCurrentPdf() }
            P.MenuSeparator {}
            P.MenuItem { icon.name: "document-print"; text: "Print…"; shortcut: "Ctrl+P"; onTriggered: win.section === 0 ? mailView.printCurrent() : win.section === 1 ? calendarView.printView() : contactsView.printCards(false) }
        }
        P.Menu {
            title: "Edit"
            icon.name: "document-edit"
            P.MenuItem { icon.name: "edit-undo"; text: "Undo"; shortcut: "Ctrl+Z"; onTriggered: win.editAction("undo") }
            P.MenuItem { icon.name: "edit-redo"; text: "Redo"; shortcut: "Ctrl+Shift+Z"; onTriggered: win.editAction("redo") }
            P.MenuSeparator {}
            P.MenuItem { icon.name: "edit-cut"; text: "Cut"; shortcut: "Ctrl+X"; onTriggered: win.editAction("cut") }
            P.MenuItem { icon.name: "edit-copy"; text: "Copy"; shortcut: "Ctrl+C"; onTriggered: win.editAction("copy") }
            P.MenuItem { icon.name: "edit-paste"; text: "Paste"; shortcut: "Ctrl+V"; onTriggered: win.editAction("paste") }
            P.MenuItem { id: menuCmd12; icon.name: "edit-delete"; shortcut: "Del"; text: "Delete"; enabled: win.section === 0 && mailView.current !== null; onTriggered: mailView.removeCurrent("delete") }
            P.MenuItem { icon.name: "edit-select-all"; text: "Select All"; shortcut: "Ctrl+A"; onTriggered: win.editAction("selectAll") }
            P.MenuSeparator {}
            P.MenuItem { icon.name: "edit-find"; text: "Find…"; shortcut: "Ctrl+F"; onTriggered: win.views[win.section].focusSearch() }
            P.Menu {
                title: "Spelling and Grammar"
                icon.name: "tools-check-spelling"
                P.MenuItem { icon.name: "tools-check-spelling"; text: "Check Spelling While Typing"; checkable: true; checked: mailView.composer.spellOn; onTriggered: mailView.composer.setSpelling(!mailView.composer.spellOn) }
                P.MenuItem { icon.name: "story-editor"; text: "Check Grammar With Spelling"; checkable: true; checked: mailView.composer.grammarOn; enabled: mailView.composer.spellOn; onTriggered: mailView.composer.setGrammar(!mailView.composer.grammarOn) }
            }
        }
        P.Menu {
            title: "View"
            icon.name: "view-visible"
            P.MenuItem { icon.name: "mail-message"; text: "Mail"; shortcut: "Ctrl+1"; checkable: true; checked: win.section === 0; onTriggered: win.section = 0 }
            P.MenuItem { icon.name: "view-calendar"; text: "Calendar"; shortcut: "Ctrl+2"; checkable: true; checked: win.section === 1; onTriggered: win.section = 1 }
            P.MenuItem { icon.name: "x-office-address-book"; text: "Contacts"; shortcut: "Ctrl+3"; checkable: true; checked: win.section === 2; onTriggered: win.section = 2 }
            P.MenuSeparator {}
            P.MenuItem { icon.name: "view-sidetree"; text: win.views[win.section].sidebarShown ? "Hide Mailbox List" : "Show Mailbox List"; shortcut: "Ctrl+Alt+S"; onTriggered: win.views[win.section].sidebarShown = !win.views[win.section].sidebarShown }
            P.MenuItem { id: menuCmd13; icon.name: "view-list-tree"; shortcut: "Ctrl+Shift+O"; text: "Organize by Conversation"; checkable: true; checked: mailView.threaded; onTriggered: mailView.threaded = !mailView.threaded }
            P.Menu {
                title: "Layout"
                icon.name: "view-split-left-right"
                P.MenuItem { id: menuCmd14; icon.name: "view-split-left-right"; shortcut: "Ctrl+Alt+H"; text: "Standard (List Beside Message)"; checkable: true; checked: !mailView.classic; onTriggered: { win.section = 0; mailView.setLayout("standard") } }
                P.MenuItem { id: menuCmd15; icon.name: "view-split-top-bottom"; shortcut: "Ctrl+Alt+K"; text: "Classic (List Above Message)"; checkable: true; checked: mailView.classic; onTriggered: { win.section = 0; mailView.setLayout("classic") } }
                P.MenuSeparator {}
                P.MenuItem { icon.name: "view-sidetree"; shortcut: "Ctrl+Alt+S"; text: "Show Mailbox List"; checkable: true; checked: mailView.sidebarShown; onTriggered: mailView.sidebarShown = !mailView.sidebarShown }
            }
            P.Menu {
                id: sortByMenu
                title: "Sort By"
                icon.name: "view-sort"
                Instantiator {
                    model: ["date", "from", "subject", "to", "attachments", "flags", "unread"]
                    onObjectAdded: (i, o) => sortByMenu.insertItem(i, o)
                    onObjectRemoved: (i, o) => sortByMenu.removeItem(o)
                    delegate: P.MenuItem {
                        required property string modelData
                        icon.name: "view-sort"
                        text: mailView.sortNames[modelData]
                        checkable: true
                        checked: mailView.sortBy === modelData
                        onTriggered: mailView.setSort(modelData, mailView.sortAscending)
                    }
                }
                P.MenuSeparator {}
                P.MenuItem { id: menuCmd16; icon.name: "view-sort-ascending"; shortcut: "Ctrl+Alt+Up"; text: "Ascending"; checkable: true; checked: mailView.sortAscending; onTriggered: mailView.setSort(mailView.sortBy, true) }
                P.MenuItem { id: menuCmd17; icon.name: "view-sort-descending"; shortcut: "Ctrl+Alt+Down"; text: "Descending"; checkable: true; checked: !mailView.sortAscending; onTriggered: mailView.setSort(mailView.sortBy, false) }
            }
            P.Menu {
                title: "Filter"
                icon.name: "view-filter"
                P.MenuItem { id: menuCmd18; icon.name: "view-list-details"; shortcut: "Ctrl+Alt+M"; text: "All Messages"; checkable: true; checked: !mailView.unreadOnly; onTriggered: mailView.unreadOnly = false }
                P.MenuItem { id: menuCmd19; icon.name: "mail-mark-unread"; shortcut: "Ctrl+Alt+U"; text: "Only Unread"; checkable: true; checked: mailView.unreadOnly; onTriggered: mailView.unreadOnly = true }
            }
            P.Menu {
                title: "Message"
                icon.name: "text-html"
                P.MenuItem { id: menuCmd20; icon.name: "text-html"; shortcut: "Ctrl+Alt+F"; text: "Formatted"; checkable: true; checked: mailView.formatted; onTriggered: mailView.formatted = true }
                P.MenuItem { id: menuCmd21; icon.name: "text-plain"; shortcut: "Ctrl+Alt+P"; text: "Plain Text Alternative"; checkable: true; checked: !mailView.formatted; onTriggered: mailView.formatted = false }
                P.MenuSeparator {}
                P.MenuItem { id: menuCmd22; icon.name: "image-x-generic"; shortcut: "Ctrl+Alt+Shift+R"; text: "Load Remote Content Automatically"; checkable: true; checked: !!mailView.prefs.load_remote; onTriggered: store.setSetting("load_remote", JSON.stringify(!mailView.prefs.load_remote)) }
            }
            P.MenuSeparator {}
            P.MenuItem { id: menuCmd23; icon.name: "flashlight-on"; shortcut: "Ctrl+Alt+Shift+L"; text: "Light Appearance"; checkable: true; checked: store.theme === "light"; onTriggered: store.chooseTheme("light") }
            P.MenuItem { id: menuCmd24; icon.name: "flashlight-off"; shortcut: "Ctrl+Alt+Shift+D"; text: "Dark Appearance"; checkable: true; checked: store.theme === "dark"; onTriggered: store.chooseTheme("dark") }
            P.MenuItem { id: menuCmd25; icon.name: "color-management"; shortcut: "Ctrl+Alt+Shift+M"; text: "Match System Appearance"; checkable: true; checked: store.theme === "system"; onTriggered: store.chooseTheme("system") }
            P.MenuSeparator {}
            P.MenuItem { icon.name: "view-fullscreen"; text: win.visibility === Window.FullScreen ? "Exit Full Screen" : "Enter Full Screen"; shortcut: "F11"; onTriggered: win.visibility === Window.FullScreen ? win.showNormal() : win.showFullScreen() }
        }
        P.Menu {
            title: "Mailbox"
            icon.name: "mail-folder-inbox"
            P.MenuItem { icon.name: "mail-receive"; text: "Get All New Mail"; shortcut: "Ctrl+Shift+N"; onTriggered: store.refreshAll() }
            P.MenuSeparator {}
            P.MenuItem { id: menuCmd26; icon.name: "edit-delete"; shortcut: "Ctrl+Shift+Del"; text: "Erase Deleted Items"; onTriggered: mailView.eraseMailbox("trash") }
            P.MenuItem { id: menuCmd27; icon.name: "mail-mark-junk"; shortcut: "Ctrl+Alt+J"; text: "Erase Junk Mail"; onTriggered: mailView.eraseMailbox("junk") }
            P.MenuSeparator {}
            P.MenuItem { id: menuCmd28; icon.name: "folder-new"; shortcut: "Ctrl+Alt+Shift+N"; text: "New Smart Mailbox…"; onTriggered: { win.section = 0; mailView.newSmartMailbox() } }
            P.MenuItem { id: menuCmd29; icon.name: "mail-mark-read"; shortcut: "Ctrl+Alt+Shift+U"; text: "Mark All Messages as Read"; onTriggered: store.markAllRead(mailView.selection) }
            P.MenuSeparator {}
            P.Menu {
                title: "Go To"
                icon.name: "go-jump"
                P.MenuItem { id: menuCmd30; icon.name: "mail-folder-inbox"; shortcut: "Alt+1"; text: "Inbox"; onTriggered: { win.section = 0; mailView.goTo("inbox", "Inbox") } }
                P.MenuItem { id: menuCmd31; icon.name: "document-edit"; shortcut: "Alt+2"; text: "Drafts"; onTriggered: { win.section = 0; mailView.goTo("drafts", "Drafts") } }
                P.MenuItem { id: menuCmd32; icon.name: "mail-folder-sent"; shortcut: "Alt+3"; text: "Sent"; onTriggered: { win.section = 0; mailView.goTo("sent", "Sent") } }
                P.MenuItem { id: menuCmd33; icon.name: "archive-insert"; shortcut: "Alt+4"; text: "Archive"; onTriggered: { win.section = 0; mailView.goTo("archive", "Archive") } }
                P.MenuItem { id: menuCmd34; icon.name: "mail-mark-junk"; shortcut: "Alt+5"; text: "Junk"; onTriggered: { win.section = 0; mailView.goTo("junk", "Junk") } }
                P.MenuItem { id: menuCmd35; icon.name: "user-trash"; shortcut: "Alt+6"; text: "Trash"; onTriggered: { win.section = 0; mailView.goTo("trash", "Trash") } }
            }
            P.MenuSeparator {}
            P.MenuItem { id: menuCmd36; icon.name: "view-filter"; shortcut: "Ctrl+Alt+R"; text: "Rules…"; onTriggered: mailView.rulesDlg.open() }
            P.MenuItem { id: menuCmd37; icon.name: "mail-signed"; shortcut: "Ctrl+Alt+Shift+G"; text: "Signatures…"; onTriggered: mailView.signaturesDlg.open() }
        }
        P.Menu {
            id: appMessageMenu
            title: "Message"
            icon.name: "mail-message"
            P.MenuItem { id: menuCmd38; icon.name: "mail-send"; shortcut: "Ctrl+Shift+D"; text: "Send Again"; enabled: mailView.current !== null; onTriggered: { win.section = 0; mailView.sendAgain() } }
            P.MenuSeparator {}
            P.MenuItem { icon.name: "mail-reply-sender"; text: "Reply"; shortcut: "Ctrl+R"; enabled: mailView.current !== null; onTriggered: { win.section = 0; mailView.reply(false) } }
            P.MenuItem { icon.name: "mail-reply-all"; text: "Reply All"; shortcut: "Ctrl+Shift+R"; enabled: mailView.current !== null; onTriggered: { win.section = 0; mailView.reply(true) } }
            P.MenuItem { icon.name: "mail-forward"; text: "Forward"; shortcut: "Ctrl+Shift+F"; enabled: mailView.current !== null; onTriggered: { win.section = 0; mailView.forward() } }
            P.MenuSeparator {}
            P.Menu {
                title: "Mark"
                icon.name: "mail-mark-important"
                enabled: mailView.currentRow !== null
                P.MenuItem { icon.name: "mail-mark-unread"; text: mailView.currentRow && mailView.currentRow.read ? "As Unread" : "As Read"; shortcut: "Ctrl+Shift+U"; onTriggered: mailView.setCurrentRead(!mailView.currentRow.read) }
                P.MenuItem { icon.name: "mail-mark-junk"; text: "As Junk Mail"; shortcut: "Ctrl+Shift+J"; onTriggered: mailView.removeCurrent("junk") }
            }
            P.MenuItem { icon.name: "flag-red"; text: mailView.current && mailView.current.flagged ? "Unflag" : "Flag"; shortcut: "Ctrl+Shift+L"; enabled: mailView.current !== null; onTriggered: mailView.toggleCurrentFlag() }
            P.Menu {
                id: appFlagMenu
                title: "Flag Colour"
                icon.name: "flag"
                enabled: mailView.current !== null
                Instantiator {
                    model: 7
                    onObjectAdded: (i, o) => appFlagMenu.insertItem(i, o)
                    onObjectRemoved: (i, o) => appFlagMenu.removeItem(o)
                    delegate: P.MenuItem {
                        required property int index
                        text: Theme.flagNames[index]
                        icon.source: mailView.flagIcon(index)
                        onTriggered: mailView.setCurrentFlag(index)
                    }
                }
                P.MenuSeparator {}
                P.MenuItem { text: "Clear Flag"; onTriggered: mailView.setCurrentFlag(-1) }
            }
            P.MenuItem { id: menuCmd39; icon.name: "mail-thread-ignored"; shortcut: "Ctrl+Shift+M"; text: mailView.current && mailView.current.muted ? "Unmute" : "Mute"; enabled: mailView.current !== null; onTriggered: store.toggleMute(mailView.current.id) }
            P.MenuSeparator {}
            P.MenuItem { icon.name: "archive-insert"; text: "Archive"; shortcut: "Ctrl+Shift+A"; enabled: mailView.current !== null; onTriggered: mailView.removeCurrent("archive") }
            P.MenuItem { icon.name: "mail-mark-junk"; shortcut: "Ctrl+Shift+J"; text: "Move to Junk"; enabled: mailView.current !== null; onTriggered: mailView.removeCurrent("junk") }
            P.Menu {
                id: appMoveMenu
                title: "Move to"
                icon.name: "folder-move"
                enabled: mailView.current !== null
                Instantiator {
                    model: mailView.folders
                    onObjectAdded: (i, o) => appMoveMenu.insertItem(i, o)
                    onObjectRemoved: (i, o) => appMoveMenu.removeItem(o)
                    delegate: P.MenuItem {
                        required property var modelData
                        icon.name: "folder"
                        text: (mailView.sidebar.accounts.length > 1 ? modelData.account + " › " : "") + modelData.name
                        onTriggered: mailView.moveCurrentToFolder(modelData.id)
                    }
                }
            }
            P.MenuSeparator {}
            P.MenuItem { id: menuCmd40; icon.name: "system-run"; shortcut: "Ctrl+Alt+Shift+A"; text: "Apply Rules"; onTriggered: store.applyRulesNow() }
            P.MenuItem { id: menuCmd41; icon.name: "emblem-favorite"; shortcut: "Ctrl+Alt+V"; text: mailView.current && mailView.current.vip ? "Remove Sender from VIPs" : "Add Sender to VIPs"; enabled: mailView.current !== null; onTriggered: store.toggleVip(mailView.current.fromEmail) }
            P.MenuItem { id: menuCmd42; icon.name: "im-ban-user"; shortcut: "Ctrl+Alt+B"; text: mailView.current && mailView.current.blocked ? "Unblock Sender" : "Block Sender"; enabled: mailView.current !== null; onTriggered: store.toggleBlocked(mailView.current.fromEmail) }
            P.MenuSeparator {}
            P.Menu {
                title: "Remind Me"
                icon.name: "chronometer"
                enabled: mailView.current !== null
                P.MenuItem { id: menuCmd43; icon.name: "chronometer"; shortcut: "Ctrl+Alt+Shift+H"; text: "In 1 Hour"; onTriggered: mailView.remindIn(3600000) }
                P.MenuItem { id: menuCmd44; icon.name: "chronometer"; shortcut: "Ctrl+Alt+Shift+T"; text: "Tonight"; onTriggered: mailView.remindAt(21, 0) }
                P.MenuItem { id: menuCmd45; icon.name: "chronometer"; shortcut: "Ctrl+Alt+Shift+O"; text: "Tomorrow"; onTriggered: mailView.remindAt(8, 1) }
                P.MenuItem { id: menuCmd46; icon.name: "chronometer"; shortcut: "Ctrl+Alt+Shift+W"; text: "Next Week"; onTriggered: mailView.remindAt(8, 7) }
                P.MenuSeparator {}
                P.MenuItem { id: menuCmd47; icon.name: "edit-clear-history"; shortcut: "Ctrl+Alt+Shift+X"; text: "Remove Reminder"; onTriggered: store.remindMe(mailView.current.id, 0) }
            }
            P.MenuItem { id: menuCmd48; icon.name: "mail-folder-outbox"; shortcut: "Ctrl+Alt+O"; text: "Outbox (Send Later)…"; onTriggered: mailView.outboxDlg.open() }
        }
        P.Menu {
            title: "Calendar"
            icon.name: "view-calendar"
            P.MenuItem { icon.name: "appointment-new"; shortcut: "Ctrl+Shift+E"; text: "New Event"; onTriggered: { win.section = 1; calendarView.newEventNow() } }
            P.MenuItem { id: menuCmd49; icon.name: "view-calendar-list"; shortcut: "Ctrl+Alt+N"; text: "New Calendar…"; onTriggered: { win.section = 1; calendarView.newCalendar() } }
            P.MenuItem { id: menuCmd50; icon.name: "view-calendar-upcoming-events"; shortcut: "Ctrl+Alt+Shift+S"; text: "New Calendar Subscription…"; onTriggered: { win.section = 1; calendarView.newSubscription() } }
            P.MenuSeparator {}
            P.MenuItem { icon.name: "go-jump-today"; text: "Go to Today"; shortcut: "Ctrl+T"; onTriggered: { win.section = 1; calendarView.cursor = new Date() } }
            P.MenuItem { id: menuCmd51; icon.name: "view-calendar-day"; shortcut: "Ctrl+Alt+1"; text: "Day"; checkable: true; checked: calendarView.view === "day"; onTriggered: { win.section = 1; calendarView.view = "day" } }
            P.MenuItem { id: menuCmd52; icon.name: "view-calendar-week"; shortcut: "Ctrl+Alt+2"; text: "Week"; checkable: true; checked: calendarView.view === "week"; onTriggered: { win.section = 1; calendarView.view = "week" } }
            P.MenuItem { id: menuCmd53; icon.name: "view-calendar-month"; shortcut: "Ctrl+Alt+3"; text: "Month"; checkable: true; checked: calendarView.view === "month"; onTriggered: { win.section = 1; calendarView.view = "month" } }
            P.MenuItem { id: menuCmd54; icon.name: "view-calendar-timeline"; shortcut: "Ctrl+Alt+4"; text: "Year"; checkable: true; checked: calendarView.view === "year"; onTriggered: { win.section = 1; calendarView.view = "year" } }
            P.MenuSeparator {}
            P.MenuItem { id: menuCmd55; icon.name: "view-refresh"; shortcut: "Ctrl+Alt+Shift+F"; text: "Refresh Subscriptions"; onTriggered: store.syncSubscriptions() }
        }
        P.Menu {
            title: "Contacts"
            icon.name: "x-office-address-book"
            P.MenuItem { icon.name: "contact-new"; shortcut: "Ctrl+Shift+C"; text: "New Contact"; onTriggered: { win.section = 2; contactsView.newContact() } }
            P.MenuItem { id: menuCmd56; icon.name: "user-group-new"; shortcut: "Ctrl+Alt+Shift+K"; text: "New Group…"; onTriggered: { win.section = 2; contactsView.newGroup() } }
            P.MenuItem { id: menuCmd57; icon.name: "user-group-properties"; shortcut: "Ctrl+Alt+Shift+J"; text: "New Smart Group…"; onTriggered: { win.section = 2; contactsView.newSmartGroup() } }
            P.MenuSeparator {}
            P.MenuItem { id: menuCmd58; icon.name: "edit-duplicate"; shortcut: "Ctrl+Alt+D"; text: "Look for Duplicates…"; onTriggered: { win.section = 2; contactsView.findDuplicates() } }
            P.MenuItem { id: menuCmd59; icon.name: "user-identity"; shortcut: "Ctrl+Alt+Shift+I"; text: "Go to My Card"; onTriggered: { win.section = 2; contactsView.goToMyCard() } }
        }
        P.Menu {
            title: "Window"
            icon.name: "window"
            P.MenuItem { icon.name: "window-minimize"; text: "Minimize"; shortcut: "Ctrl+M"; onTriggered: win.showMinimized() }
            P.MenuItem { id: menuCmd60; icon.name: "window-maximize"; shortcut: "Ctrl+Alt+Z"; text: "Zoom"; onTriggered: win.visibility === Window.Maximized ? win.showNormal() : win.showMaximized() }
            P.MenuSeparator {}
            P.MenuItem { id: menuCmd61; icon.name: "mail-message"; shortcut: "Ctrl+Alt+0"; text: "Message Viewer"; onTriggered: win.section = 0 }
            P.MenuItem { icon.name: "mail-folder-outbox"; shortcut: "Ctrl+Alt+O"; text: "Outbox"; onTriggered: mailView.outboxDlg.open() }
        }
        P.Menu {
            title: "Help"
            icon.name: "help-contents"
            P.MenuItem { id: menuCmd62; icon.name: "help-keybord-shortcuts"; shortcut: "F1"; text: "Keyboard Shortcuts"; onTriggered: shortcutsPopup.open() }
            P.MenuItem { icon.name: "help-contents"; shortcut: "Ctrl+F1"; text: "Setup Guide"; onTriggered: store.onboarded = false }
        }
    }
    // The menus' shortcuts work with the menus closed (each runs its menu command).
    Shortcut { sequence: menuCmd1.shortcut; enabled: menuCmd1.enabled; onActivated: menuCmd1.triggered() }
    Shortcut { sequence: menuCmd2.shortcut; enabled: menuCmd2.enabled && !win.typing; onActivated: menuCmd2.triggered() }
    Shortcut { sequence: menuCmd3.shortcut; enabled: menuCmd3.enabled && !win.typing; onActivated: menuCmd3.triggered() }
    Shortcut { sequence: menuCmd4.shortcut; enabled: menuCmd4.enabled; onActivated: menuCmd4.triggered() }
    Shortcut { sequence: menuCmd5.shortcut; enabled: menuCmd5.enabled; onActivated: menuCmd5.triggered() }
    Shortcut { sequence: menuCmd6.shortcut; enabled: menuCmd6.enabled; onActivated: menuCmd6.triggered() }
    Shortcut { sequence: menuCmd7.shortcut; enabled: menuCmd7.enabled; onActivated: menuCmd7.triggered() }
    Shortcut { sequence: menuCmd8.shortcut; enabled: menuCmd8.enabled; onActivated: menuCmd8.triggered() }
    Shortcut { sequence: menuCmd9.shortcut; enabled: menuCmd9.enabled && !win.typing; onActivated: menuCmd9.triggered() }
    Shortcut { sequence: menuCmd10.shortcut; enabled: menuCmd10.enabled && !win.typing; onActivated: menuCmd10.triggered() }
    Shortcut { sequence: menuCmd11.shortcut; enabled: menuCmd11.enabled; onActivated: menuCmd11.triggered() }
    Shortcut { sequence: menuCmd12.shortcut; enabled: menuCmd12.enabled && !win.typing; onActivated: menuCmd12.triggered() }
    Shortcut { sequence: menuCmd13.shortcut; enabled: menuCmd13.enabled; onActivated: menuCmd13.triggered() }
    Shortcut { sequence: menuCmd14.shortcut; enabled: menuCmd14.enabled && !win.typing; onActivated: menuCmd14.triggered() }
    Shortcut { sequence: menuCmd15.shortcut; enabled: menuCmd15.enabled && !win.typing; onActivated: menuCmd15.triggered() }
    Shortcut { sequence: menuCmd16.shortcut; enabled: menuCmd16.enabled && !win.typing; onActivated: menuCmd16.triggered() }
    Shortcut { sequence: menuCmd17.shortcut; enabled: menuCmd17.enabled && !win.typing; onActivated: menuCmd17.triggered() }
    Shortcut { sequence: menuCmd18.shortcut; enabled: menuCmd18.enabled && !win.typing; onActivated: menuCmd18.triggered() }
    Shortcut { sequence: menuCmd19.shortcut; enabled: menuCmd19.enabled && !win.typing; onActivated: menuCmd19.triggered() }
    Shortcut { sequence: menuCmd20.shortcut; enabled: menuCmd20.enabled && !win.typing; onActivated: menuCmd20.triggered() }
    Shortcut { sequence: menuCmd21.shortcut; enabled: menuCmd21.enabled && !win.typing; onActivated: menuCmd21.triggered() }
    Shortcut { sequence: menuCmd22.shortcut; enabled: menuCmd22.enabled && !win.typing; onActivated: menuCmd22.triggered() }
    Shortcut { sequence: menuCmd23.shortcut; enabled: menuCmd23.enabled && !win.typing; onActivated: menuCmd23.triggered() }
    Shortcut { sequence: menuCmd24.shortcut; enabled: menuCmd24.enabled && !win.typing; onActivated: menuCmd24.triggered() }
    Shortcut { sequence: menuCmd25.shortcut; enabled: menuCmd25.enabled && !win.typing; onActivated: menuCmd25.triggered() }
    Shortcut { sequence: menuCmd26.shortcut; enabled: menuCmd26.enabled; onActivated: menuCmd26.triggered() }
    Shortcut { sequence: menuCmd27.shortcut; enabled: menuCmd27.enabled && !win.typing; onActivated: menuCmd27.triggered() }
    Shortcut { sequence: menuCmd28.shortcut; enabled: menuCmd28.enabled && !win.typing; onActivated: menuCmd28.triggered() }
    Shortcut { sequence: menuCmd29.shortcut; enabled: menuCmd29.enabled && !win.typing; onActivated: menuCmd29.triggered() }
    Shortcut { sequence: menuCmd30.shortcut; enabled: menuCmd30.enabled && !win.typing; onActivated: menuCmd30.triggered() }
    Shortcut { sequence: menuCmd31.shortcut; enabled: menuCmd31.enabled && !win.typing; onActivated: menuCmd31.triggered() }
    Shortcut { sequence: menuCmd32.shortcut; enabled: menuCmd32.enabled && !win.typing; onActivated: menuCmd32.triggered() }
    Shortcut { sequence: menuCmd33.shortcut; enabled: menuCmd33.enabled && !win.typing; onActivated: menuCmd33.triggered() }
    Shortcut { sequence: menuCmd34.shortcut; enabled: menuCmd34.enabled && !win.typing; onActivated: menuCmd34.triggered() }
    Shortcut { sequence: menuCmd35.shortcut; enabled: menuCmd35.enabled && !win.typing; onActivated: menuCmd35.triggered() }
    Shortcut { sequence: menuCmd36.shortcut; enabled: menuCmd36.enabled && !win.typing; onActivated: menuCmd36.triggered() }
    Shortcut { sequence: menuCmd37.shortcut; enabled: menuCmd37.enabled && !win.typing; onActivated: menuCmd37.triggered() }
    Shortcut { sequence: menuCmd38.shortcut; enabled: menuCmd38.enabled; onActivated: menuCmd38.triggered() }
    Shortcut { sequence: menuCmd39.shortcut; enabled: menuCmd39.enabled; onActivated: menuCmd39.triggered() }
    Shortcut { sequence: menuCmd40.shortcut; enabled: menuCmd40.enabled && !win.typing; onActivated: menuCmd40.triggered() }
    Shortcut { sequence: menuCmd41.shortcut; enabled: menuCmd41.enabled && !win.typing; onActivated: menuCmd41.triggered() }
    Shortcut { sequence: menuCmd42.shortcut; enabled: menuCmd42.enabled && !win.typing; onActivated: menuCmd42.triggered() }
    Shortcut { sequence: menuCmd43.shortcut; enabled: menuCmd43.enabled && !win.typing; onActivated: menuCmd43.triggered() }
    Shortcut { sequence: menuCmd44.shortcut; enabled: menuCmd44.enabled && !win.typing; onActivated: menuCmd44.triggered() }
    Shortcut { sequence: menuCmd45.shortcut; enabled: menuCmd45.enabled && !win.typing; onActivated: menuCmd45.triggered() }
    Shortcut { sequence: menuCmd46.shortcut; enabled: menuCmd46.enabled && !win.typing; onActivated: menuCmd46.triggered() }
    Shortcut { sequence: menuCmd47.shortcut; enabled: menuCmd47.enabled && !win.typing; onActivated: menuCmd47.triggered() }
    Shortcut { sequence: menuCmd48.shortcut; enabled: menuCmd48.enabled && !win.typing; onActivated: menuCmd48.triggered() }
    Shortcut { sequence: menuCmd49.shortcut; enabled: menuCmd49.enabled && !win.typing; onActivated: menuCmd49.triggered() }
    Shortcut { sequence: menuCmd50.shortcut; enabled: menuCmd50.enabled && !win.typing; onActivated: menuCmd50.triggered() }
    Shortcut { sequence: menuCmd51.shortcut; enabled: menuCmd51.enabled && !win.typing; onActivated: menuCmd51.triggered() }
    Shortcut { sequence: menuCmd52.shortcut; enabled: menuCmd52.enabled && !win.typing; onActivated: menuCmd52.triggered() }
    Shortcut { sequence: menuCmd53.shortcut; enabled: menuCmd53.enabled && !win.typing; onActivated: menuCmd53.triggered() }
    Shortcut { sequence: menuCmd54.shortcut; enabled: menuCmd54.enabled && !win.typing; onActivated: menuCmd54.triggered() }
    Shortcut { sequence: menuCmd55.shortcut; enabled: menuCmd55.enabled && !win.typing; onActivated: menuCmd55.triggered() }
    Shortcut { sequence: menuCmd56.shortcut; enabled: menuCmd56.enabled && !win.typing; onActivated: menuCmd56.triggered() }
    Shortcut { sequence: menuCmd57.shortcut; enabled: menuCmd57.enabled && !win.typing; onActivated: menuCmd57.triggered() }
    Shortcut { sequence: menuCmd58.shortcut; enabled: menuCmd58.enabled && !win.typing; onActivated: menuCmd58.triggered() }
    Shortcut { sequence: menuCmd59.shortcut; enabled: menuCmd59.enabled && !win.typing; onActivated: menuCmd59.triggered() }
    Shortcut { sequence: menuCmd60.shortcut; enabled: menuCmd60.enabled && !win.typing; onActivated: menuCmd60.triggered() }
    Shortcut { sequence: menuCmd61.shortcut; enabled: menuCmd61.enabled && !win.typing; onActivated: menuCmd61.triggered() }
    Shortcut { sequence: menuCmd62.shortcut; enabled: menuCmd62.enabled; onActivated: menuCmd62.triggered() }
    Shortcut { sequence: "Ctrl+,"; onActivated: settingsDialog.open() }
    // Shortcuts the menus show (menus only list them; these make them work).
    Shortcut { sequence: "Ctrl+P"; onActivated: win.section === 0 ? mailView.printCurrent() : win.section === 1 ? calendarView.printView() : contactsView.printCards(false) }
    Shortcut { sequence: "F11"; onActivated: win.visibility === Window.FullScreen ? win.showNormal() : win.showFullScreen() }
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
                            ["Ctrl+R / Ctrl+Shift+R", "Reply / Reply All"], ["Ctrl+Shift+F", "Forward"], ["Ctrl+Return", "Send"],
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
            Text { text: "Mail, Calendar and Contacts for Linux, Windows and macOS.\nBuilt with Rust and Qt."; color: Theme.secondary }
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
            var maxW = room ? Math.min(room.width, room.desktopAvailableWidth) - 48 : 100000
            var maxH = room ? Math.min(room.height, room.desktopAvailableHeight) - 48 : 100000
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
    onClosing: { saveState(); Qt.callLater(Qt.quit) }
    Connections { target: Qt.application; function onAboutToQuit() { win.saveState(); store.flush() } }
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
        store.startupMark("window set up")
        // Window buttons drawn like the desktop's own (Plasma's round Breeze ones here).
        if (Qt.platform.os === "linux") Theme.windowButtons = /KDE/i.test(store.desktopName()) ? "breeze" : "adwaita"
        if (store.arg("--buttons") !== "") Theme.windowButtons = store.arg("--buttons")
        restoreState()
        // --width N (for pictures of narrow windows)
        var w = parseInt(store.arg("--width"))
        if (w > 0) width = w
        var p = store.startPanel
        if (p === "accounts") sourcesDialog.open()
        else if (p === "add-account") { sourcesDialog.open(); sourcesDialog.newAccount() }
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
        else if (p === "signin-steps") { signInSetup.openFor("google", false); signInSetup.byHand = true; for (var i = 0; i < 5; i++) signInSetup.markDone(i) }
        else if (p === "menu") appMenu.open(menuButton)
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

    Timer { id: pdfCheck; interval: 2600; onTriggered: mailView.checkPdfBar() }
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
            else if (r.calendar) win.section = 1
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
