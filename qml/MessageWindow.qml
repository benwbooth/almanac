import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtCore
import GlobalMenu

// Messages (each with its whole conversation) and pictures / PDFs from them in
// a window of their own, opened by Shift + double-clicking a message in the
// list, as in Apple Mail. With more than one, they're in tabs, which can be
// dragged along, out into a window of their own, or into another window.
Window {
    // Back in view after the desktop hid it (on another desktop, covered…): see
    // CraftExposeWatch in include/craft_shot.h.
    signal reexposed()
    id: w
    // A thousandth of a pixel down. At 130% scaling, text that lands exactly
    // on a half device pixel has its letters rounded up or down one by one,
    // so round ones (e, o, s) sit a pixel below the rest. Off the exact half,
    // they all agree. (As in Earshot.)
    Binding { target: w.contentItem; property: "y"; value: 0.001 }
    property var store
    // The Mail view: replies, forwards and the reading settings come from it.
    property var mail

    // The tabs ({id, selection, threaded, title} or {id, att, title,
    // attachment}) and the open one.
    property var tabs: []
    property int tabIndex: 0
    ListModel { id: pages }
    readonly property var tab: tabs.length ? tabs[Math.min(tabIndex, tabs.length - 1)] : null
    // The open tab's page (a message's has current, messages, ids, act, moveAll).
    readonly property var page: {
        tabIndex; tabs
        var p = pageRepeater.itemAt(tabIndex)
        return p ? p.content : null
    }
    readonly property bool onMessage: !!tab && !tab.attachment
    readonly property var current: onMessage && page && page.current !== undefined ? page.current : null
    readonly property var messages: current ? page.messages : []
    readonly property var ids: current ? page.ids : []

    function insertTab(t, at) {
        var l = tabs.slice(); l.splice(at, 0, t)
        pages.insert(at, { messageId: t.id, sel: t.selection || "", thr: !!t.threaded, att: t.att || "" })
        tabs = l
        tabIndex = at
    }
    function moveTab(from, to) {
        if (to > from) to--
        if (to === from) { tabIndex = from; return }
        var l = tabs.slice(); var t = l.splice(from, 1)[0]; l.splice(to, 0, t)
        pages.move(from, to, 1)
        tabs = l
        tabIndex = to
    }
    function closeTab(i) {
        if (i < 0 || i >= tabs.length) return
        var l = tabs.slice(); l.splice(i, 1)
        if (tabIndex > i || tabIndex >= l.length) tabIndex = Math.max(0, tabIndex - 1)
        tabs = l
        // (After the above: removing the page destroys it, and it may be what asked.)
        if (i < pages.count) pages.remove(i)
        // The last one gone: so is the window.
        if (!l.length) Qt.callLater(w.close)
    }
    function takeTab(i) { var t = tabs[i]; closeTab(i); return t }
    // A page asking to go: done afterwards, found by what it shows.
    function closeTabOf(id, att) {
        Qt.callLater(function() {
            for (var i = 0; i < tabs.length; i++) if (mail.sameTab(tabs[i], { id: id, att: att })) { closeTab(i); return }
        })
    }
    function closeOtherTabs(i) { var keep = tabs[i]; for (var j = tabs.length - 1; j >= 0; j--) if (tabs[j] !== keep) closeTab(j); tabIndex = 0 }
    function closeAllTabs() { w.close() }
    // (A window's only tab is already on its own.)
    function canTearOut(i) { return tabs.length > 1 }
    function tearOut(i) { if (canTearOut(i)) mail.openTabWindow(takeTab(i)) }
    function toMain(i) { mail.insertTab(takeTab(i), mail.tabs.length); mail.raiseWindow() }
    function raiseWindow() { w.raise(); w.requestActivate() }

    title: (tab ? tab.title : "Message") + " — Almanac"
    // The size the last one was (kept between runs).
    Settings { id: popSize; category: "MessageWindow"; property real w: 780; property real h: 760 }
    width: Math.max(420, Math.min(popSize.w, Screen.desktopAvailableWidth || 100000))
    height: Math.max(320, Math.min(popSize.h, Screen.desktopAvailableHeight || 100000))
    onWidthChanged: popSave.restart()
    onHeightChanged: popSave.restart()
    Timer { id: popSave; interval: 800; onTriggered: if (w.visible && w.visibility === Window.Windowed && sizeKeeper.settled) { popSize.w = w.width; popSize.h = w.height } }
    // (Opening: the desktop may give the window another size a moment later,
    // which isn't one chosen; once settled, resizes are kept.)
    PaneKeeper { id: sizeKeeper; item: w.contentItem; Component.onCompleted: restore({}) }
    minimumWidth: 420; minimumHeight: 320
    transientParent: null
    // The top bar is the title bar, as in the main window.
    readonly property bool macOS: Qt.platform.os === "osx"
    flags: macOS ? (Qt.Window | Qt.ExpandedClientAreaHint | Qt.NoTitleBarBackgroundHint) : (Qt.Window | Qt.FramelessWindowHint)
    color: Theme.window
    ResizeEdges { visible: !w.macOS && w.visibility !== Window.Maximized; window: w }
    onClosing: Qt.callLater(w.destroy)

    // Replies and forwards are written in the main window.
    function toMainWindow() { mail.raiseWindow() }
    function act(m, action) {
        if (action === "delete") { store.deleteMessages(JSON.stringify([m.id])); return }
        mail.messageAction(m, action)
        toMainWindow()
    }
    function moveAll(kind) {
        if (kind === "delete") store.deleteMessages(JSON.stringify(ids))
        else for (var i = 0; i < ids.length; i++) store.moveTo(ids[i], kind)
    }

    // The window's menus: in the desktop's global menu bar and in the ☰
    // button (see Main.qml). The keys are this window's Shortcuts below.
    readonly property var findPage: onMessage && page ? page.page : null
    property bool menusReady: false
    Component.onCompleted: Qt.callLater(function() { w.menusReady = true })
    readonly property var menuModel: {
        if (!menusReady) return []
        var cur = w.current, many = w.tabs.length > 1
        return [
            { title: "Almanac", icon: "almanac", items: [
                { id: "quit", text: "Quit Almanac", icon: "application-exit" }
            ] },
            { title: "File", icon: "folder-open", items: [
                { id: "close", text: many ? "Close Tab" : "Close Window", icon: "window-close", shortcut: "Ctrl+W" },
                { separator: true },
                { id: "print", text: "Print…", icon: "document-print", enabled: cur !== null && !!w.mail && !!w.mail.printMessages }
            ] },
            { title: "Edit", icon: "document-edit", items: [
                { id: "copy", text: "Copy", icon: "edit-copy", shortcut: "Ctrl+C" },
                { id: "selectAll", text: "Select All", icon: "edit-select-all", shortcut: "Ctrl+A" },
                { separator: true },
                { id: "find", text: "Find in Message…", icon: "edit-find", shortcut: "Ctrl+F", enabled: cur !== null && !!w.findPage },
                { id: "findNext", text: "Find Next", icon: "go-down-search", shortcut: "Ctrl+G", enabled: cur !== null && !!w.findPage },
                { id: "findPrevious", text: "Find Previous", icon: "go-up-search", shortcut: "Ctrl+Shift+G", enabled: cur !== null && !!w.findPage }
            ] },
            { title: "Message", icon: "mail-message", items: [
                { id: "reply", text: "Reply", icon: "mail-reply-sender", shortcut: "Ctrl+R", enabled: cur !== null },
                { id: "replyAll", text: "Reply All", icon: "mail-reply-all", shortcut: "Ctrl+Shift+R", enabled: cur !== null },
                { id: "forward", text: "Forward", icon: "mail-forward", shortcut: "Ctrl+Shift+F", enabled: cur !== null },
                { id: "sendAgain", text: "Send Again", icon: "mail-send", shortcut: "Ctrl+Shift+D", enabled: cur !== null },
                { separator: true },
                { id: "flag", text: cur && cur.flagged ? "Unflag" : "Flag", icon: "flag-red", shortcut: "Ctrl+Shift+L", enabled: cur !== null },
                { id: "markRead", text: cur && !cur.read ? "Mark as Read" : "Mark as Unread", icon: "mail-mark-unread", enabled: cur !== null },
                { separator: true },
                { id: "archive", text: "Archive", icon: "archive-insert", shortcut: "Ctrl+Shift+A", enabled: cur !== null },
                { id: "junk", text: "Move to Junk", icon: "mail-mark-junk", shortcut: "Ctrl+Shift+J", enabled: cur !== null },
                { id: "delete", text: "Delete", icon: "edit-delete", shortcut: "Del", enabled: cur !== null }
            ] },
            { title: "Window", icon: "window", items: [
                { id: "minimize", text: "Minimize", icon: "window-minimize" },
                { id: "zoom", text: "Zoom", icon: "window-maximize" },
                { separator: true },
                { id: "nextTab", text: "Show Next Tab", icon: "go-next", shortcut: "Ctrl+Tab", enabled: many },
                { id: "previousTab", text: "Show Previous Tab", icon: "go-previous", shortcut: "Ctrl+Shift+Tab", enabled: many },
                { id: "tearOut", text: "Move Tab to New Window", icon: "window-new", enabled: many },
                { id: "toMain", text: "Move Tab to Main Window", icon: "go-home", enabled: w.tabs.length > 0 },
                { separator: true },
                { id: "mainWindow", text: "Almanac", icon: "almanac" }
            ] }
        ]
    }
    property bool fromAppMenu: false
    property Item focusBeforeMenu: null
    // Copy / Select All: whatever has the focus (the message's page offers them).
    function editAction(name) {
        for (var f = w.fromAppMenu ? w.focusBeforeMenu : w.activeFocusItem; f; f = f.parent)
            if (typeof f[name] === "function") { f[name](); return }
        if (w.findPage && typeof w.findPage[name] === "function") w.findPage[name]()
    }
    function menuAction(id) {
        var cur = w.current
        switch (id) {
        case "quit": Qt.quit(); break
        case "close": w.tabs.length > 1 ? w.closeTab(w.tabIndex) : w.close(); break
        case "print": if (cur && w.mail.printMessages) w.mail.printMessages(w.messages); break
        case "copy": case "selectAll": w.editAction(id); break
        case "find": if (w.findPage) w.findPage.openFind(); break
        case "findNext": if (w.findPage) w.findPage.findStep(false); break
        case "findPrevious": if (w.findPage) w.findPage.findStep(true); break
        case "reply": if (cur) w.act(cur, "reply"); break
        case "replyAll": if (cur) w.act(cur, "replyAll"); break
        case "forward": if (cur) w.act(cur, "forward"); break
        case "sendAgain": if (cur) w.act(cur, "sendAgain"); break
        case "flag": if (cur) w.store.toggleFlag(cur.id); break
        case "markRead": if (cur) { var r = !cur.read; for (var i = 0; i < w.ids.length; i++) w.store.setRead(w.ids[i], r) } break
        case "archive": if (cur) w.moveAll("archive"); break
        case "junk": if (cur) w.moveAll("junk"); break
        case "delete": if (cur) w.moveAll("delete"); break
        case "minimize": w.showMinimized(); break
        case "zoom": w.visibility === Window.Maximized ? w.showNormal() : w.showMaximized(); break
        case "nextTab": if (w.tabs.length > 1) w.tabIndex = (w.tabIndex + 1) % w.tabs.length; break
        case "previousTab": if (w.tabs.length > 1) w.tabIndex = (w.tabIndex + w.tabs.length - 1) % w.tabs.length; break
        case "tearOut": w.tearOut(w.tabIndex); break
        case "toMain": if (w.tabs.length) w.toMain(w.tabIndex); break
        case "mainWindow": w.mail.raiseWindow(); break
        }
    }
    GlobalMenuBar { id: globalMenu; window: w; model: w.menuModel; onTriggered: id => w.menuAction(id) }
    onVisibleChanged: if (visible) Qt.callLater(globalMenu.attach)
    AppMenu {
        id: appMenu
        accent: Theme.accentFill
        dark: Theme.dark
        popupType: Popup.Item
        model: w.menuModel
        onTriggered: id => { w.fromAppMenu = true; w.menuAction(id); w.fromAppMenu = false }
    }

    Shortcut { sequences: [StandardKey.Close, "Escape"]; onActivated: w.tabs.length > 1 ? w.closeTab(w.tabIndex) : w.close() }
    Shortcut { sequence: "Ctrl+Tab"; enabled: w.tabs.length > 1; onActivated: w.tabIndex = (w.tabIndex + 1) % w.tabs.length }
    Shortcut { sequence: "Ctrl+Shift+Tab"; enabled: w.tabs.length > 1; onActivated: w.tabIndex = (w.tabIndex + w.tabs.length - 1) % w.tabs.length }
    Shortcut { sequence: "Ctrl+R"; enabled: w.current !== null; onActivated: w.act(w.current, "reply") }
    Shortcut { sequence: "Ctrl+Shift+R"; enabled: w.current !== null; onActivated: w.act(w.current, "replyAll") }
    Shortcut { sequence: "Ctrl+Shift+F"; enabled: w.current !== null; onActivated: w.act(w.current, "forward") }
    Shortcut { sequence: "Ctrl+Shift+D"; autoRepeat: false; enabled: w.current !== null; onActivated: w.act(w.current, "sendAgain") }
    Shortcut { sequence: "Ctrl+Shift+A"; enabled: w.current !== null; onActivated: w.moveAll("archive") }
    Shortcut { sequence: "Ctrl+Shift+J"; enabled: w.current !== null; onActivated: w.moveAll("junk") }
    Shortcut { sequence: "Ctrl+Shift+L"; enabled: w.current !== null; onActivated: w.store.toggleFlag(w.current.id) }
    Shortcut { sequences: [StandardKey.Delete, "Backspace"]; enabled: w.current !== null; onActivated: w.moveAll("delete") }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: Theme.toolbarHeight
            gradient: Theme.toolbarGradient
            // Drag the bar to move the window, double-click to zoom it, right-click
            // for the window menu (the shared ChromeTitleArea, as every app).
            ChromeTitleArea { anchors.fill: parent; window: w; menuDark: Theme.dark; menuAccent: Theme.accentFill }
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: w.macOS ? 78 : 10
                anchors.rightMargin: w.macOS ? 10 : 0
                spacing: 2
                MacMenuButton {
                    id: menuButton
                    dark: Theme.dark
                    fg: Theme.text
                    menu: appMenu
                    tip: "Menu: every command for this window"
                    onAboutToOpen: w.focusBeforeMenu = w.activeFocusItem
                }
                RowLayout {
                    visible: w.onMessage
                    spacing: 2
                    ToolButtonX { icon: "archive"; tip: "Archive (Ctrl+Shift+A)"; enabled: w.current !== null; onClicked: w.moveAll("archive") }
                    ToolButtonX { icon: "trash"; tip: "Delete"; enabled: w.current !== null; onClicked: w.moveAll("delete") }
                    ToolButtonX { icon: "junk"; tip: "Move to Junk (Ctrl+Shift+J)"; enabled: w.current !== null; onClicked: w.moveAll("junk") }
                    Item { width: 6 }
                    ToolButtonX { icon: "reply"; tip: "Reply (Ctrl+R)"; enabled: w.current !== null; onClicked: w.act(w.current, "reply") }
                    ToolButtonX { icon: "replyAll"; tip: "Reply All (Ctrl+Shift+R)"; enabled: w.current !== null; onClicked: w.act(w.current, "replyAll") }
                    ToolButtonX { icon: "forward"; tip: "Forward (Ctrl+Shift+F)"; enabled: w.current !== null; onClicked: w.act(w.current, "forward") }
                    Item { width: 6 }
                    ToolButtonX {
                        icon: "flag"
                        iconColor: w.current && w.current.flagged ? Theme.flagColors[w.current.flagColor || 0] : Theme.secondary
                        tip: "Flag (Ctrl+Shift+L)"; enabled: w.current !== null
                        onClicked: w.store.toggleFlag(w.current.id)
                    }
                    ToolButtonX {
                        icon: "envelopeOpen"
                        tip: w.current && !w.current.read ? "Mark as Read" : "Mark as Unread"
                        enabled: w.current !== null
                        onClicked: { var r = !w.current.read; for (var i = 0; i < w.ids.length; i++) w.store.setRead(w.ids[i], r) }
                    }
                }
                Text {
                    Layout.fillWidth: true
                    Layout.leftMargin: 8
                    text: w.current ? (w.current.subject || "(No Subject)") : w.tab ? w.tab.title : ""
                    font.weight: Font.DemiBold
                    color: Theme.text
                    elide: Text.ElideRight
                    horizontalAlignment: Text.AlignHCenter
                }
                ToolButtonX {
                    readonly property var pg: w.onMessage && w.page ? w.page.page : null
                    icon: "findInPage"; tip: "Find in Message (Ctrl+F)"; visible: w.onMessage; enabled: w.current !== null
                    checked: !!pg && pg.finding
                    onClicked: if (pg) { if (pg.finding) pg.closeFind(); else pg.openFind() }
                }
                ToolButtonX { icon: "printer"; tip: "Print (Ctrl+P)"; visible: w.onMessage && !!w.mail.printMessages; enabled: w.current !== null; onClicked: w.mail.printMessages(w.messages) }
                WindowControls { visible: !w.macOS; window: w; Layout.leftMargin: 6; Layout.fillHeight: true }
            }
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border; visible: w.tabs.length < 2 }
        }
        // With more than one, the tabs.
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: w.tabs.length > 1 ? 36 : 0
            visible: w.tabs.length > 1
            color: Theme.toolbar
            TabStrip { anchors.fill: parent; owner: w; mail: w.mail }
        }
        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Repeater {
                id: pageRepeater
                model: pages
                delegate: Item {
                    id: pg
                    required property var messageId
                    required property string sel
                    required property bool thr
                    required property string att
                    required property int index
                    readonly property var content: loader.item
                    anchors.fill: parent
                    visible: w.tabIndex === index
                    Loader {
                        id: loader
                        anchors.fill: parent
                        sourceComponent: pg.att !== "" ? attachmentPage : messagePage
                    }
                    Component {
                        id: messagePage
                        MessageTab {
                            bare: true
                            store: w.store
                            view: w.mail
                            selection: pg.sel
                            messageId: pg.messageId
                            threaded: pg.thr
                            onCloseRequested: w.closeTabOf(pg.messageId, pg.att)
                            onComposing: w.toMainWindow()
                        }
                    }
                    Component {
                        id: attachmentPage
                        AttachmentView {
                            store: w.store
                            view: w.mail
                            messageId: pg.messageId
                            name: pg.att
                            inWindow: w.tabs.length < 2
                            onCloseRequested: w.closeTabOf(pg.messageId, pg.att)
                            onPopOutRequested: w.tearOut(pg.index)
                        }
                    }
                }
            }
        }
    }
    // A tab let go anywhere over this window (from another) joins its tabs.
    DropArea {
        anchors.fill: parent
        anchors.topMargin: w.tabs.length > 1 ? Theme.toolbarHeight + 36 : 0
        keys: ["application/x-almanac-tab"]
        onEntered: function(drag) { if (!Theme.tabDrag || Theme.tabDrag.owner === w) drag.accepted = false }
        onDropped: function(drop) {
            if (!Theme.tabDrag || Theme.tabDrag.owner === w) return
            Theme.tabDrag.target = w
            Theme.tabDrag.at = w.tabs.length
            drop.accept(Qt.MoveAction)
        }
    }
}
