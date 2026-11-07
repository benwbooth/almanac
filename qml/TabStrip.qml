import QtQuick
import QtQuick.Controls.Basic
// The system's own menus (desktop style on Linux, native on macOS and Windows)
import Qt.labs.platform as P
import QtQuick.Layouts

// A row of tabs, as in a browser: click to open one, middle-click or × to
// close it, drag it along the row to move it, out of the window for a window
// of its own, or onto another Almanac window to move it there. The open tab
// joins the page below it; tabs that don't fit are in the drop-down at the end.
Item {
    id: strip
    // The tabs' owner (the Mail view, or a message window), with
    // tabs [{id, selection, threaded, title, att, attachment}], tabIndex (of
    // this row, so counting a first tab that stays), closeTab(i),
    // closeOtherTabs(i), closeAllTabs(), moveTab(from, to), takeTab(i),
    // insertTab(tab, at), canTearOut(i), tearOut(i) and raiseWindow(); and,
    // in a message window, toMain(i). (i: in tabs.)
    property var owner
    // A first tab that stays put (the main window's mailbox).
    property string homeTitle: ""
    // The Mail view (for "Merge All Windows").
    property var mail
    readonly property int home: homeTitle !== "" ? 1 : 0
    readonly property var all: (home ? [{ title: homeTitle, mailbox: true }] : []).concat(owner ? owner.tabs : [])
    readonly property real minTab: 120
    readonly property real maxTab: 230
    readonly property real room: width - 16
    // How many fit; when not all do, the tab that stays plus a run of tabs
    // that includes the open one, and the drop-down.
    readonly property int fitAll: Math.max(1, Math.floor(room / minTab))
    readonly property bool overflow: all.length > fitAll
    readonly property int fit: overflow ? Math.max(home + 1, Math.floor((room - 44) / minTab)) : all.length
    readonly property int run: fit - home
    readonly property int first: {
        if (!overflow) return home
        return Math.max(home, Math.min(owner.tabIndex - run + 1, all.length - run))
    }
    function shows(i) { return i < home || (i >= first && i < first + run) }
    readonly property real tabWidth: Math.max(minTab, Math.min(maxTab, (room - (overflow ? 44 : 0)) / Math.max(1, fit)))

    // Where a tab dragged over the row would go (a place in tabs), and its line.
    property int dropSlot: -1
    property real dropX: 0
    function slotAt(x) {
        var k = Math.max(home, Math.min(fit, Math.round((x - 8) / tabWidth)))
        dropX = 8 + k * tabWidth
        return Math.min(first + (k - home), all.length) - home
    }
    // After a tab's drag (once it's over): moved along the row, to another
    // window, or (let go elsewhere) out into a window of its own.
    function finishDrag(d) {
        if (!d.target) { if (d.owner.canTearOut(d.index)) d.owner.tearOut(d.index); return }
        if (d.target === d.owner) { d.owner.moveTab(d.index, d.at); return }
        var t = d.owner.takeTab(d.index)
        d.target.insertTab(t, d.at)
        d.target.raiseWindow()
    }

    // The line under the row (the open tab covers it, joining the page).
    Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border }
    Row {
        x: 8
        anchors.bottom: parent.bottom
        spacing: 0
        Repeater {
            model: strip.all
            delegate: Item {
                id: tabItem
                required property var modelData
                required property int index
                readonly property bool on: strip.owner.tabIndex === index
                readonly property int tab: index - strip.home
                visible: strip.shows(index)
                width: visible ? strip.tabWidth : 0
                height: strip.height - 5
                clip: true
                z: on ? 2 : 1
                // A picture of the tab, carried by the pointer while it's dragged.
                property url shot
                // Rounded at the top, open at the bottom (the rest is clipped off).
                Rectangle {
                    width: parent.width
                    height: parent.height + 12
                    radius: 8
                    color: tabItem.on ? Theme.window : (tabHov.hovered ? Theme.hover : "transparent")
                    border.color: tabItem.on ? Theme.border : "transparent"
                }
                // Between two tabs that aren't open, a thin divider.
                Rectangle {
                    visible: !tabItem.on && strip.owner.tabIndex !== tabItem.index + 1 && tabItem.index + 1 < strip.first + strip.run && tabItem.index + 1 < strip.all.length
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    width: 1; height: 16
                    color: Theme.border
                }
                HoverHandler {
                    id: tabHov
                    onHoveredChanged: if (hovered && !tabItem.modelData.mailbox) tabItem.grabToImage(function(r) { tabItem.shot = r.url })
                }
                // What's dragged (the system's drag, so it can cross windows).
                Item {
                    id: carried
                    width: tabItem.width; height: tabItem.height
                    Drag.dragType: Drag.Automatic
                    Drag.supportedActions: Qt.MoveAction
                    Drag.mimeData: ({ "application/x-almanac-tab": "tab" })
                    Drag.imageSource: tabItem.shot
                    Drag.active: tabMouse.drag.active
                    Drag.onDragStarted: Theme.tabDrag = { owner: strip.owner, index: tabItem.tab }
                    Drag.onDragFinished: function(action) {
                        var d = Theme.tabDrag
                        Theme.tabDrag = null
                        carried.x = 0; carried.y = 0
                        var s = strip
                        if (d) Qt.callLater(function() { s.finishDrag(d) })
                    }
                }
                MouseArea {
                    id: tabMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    acceptedButtons: Qt.LeftButton | Qt.MiddleButton | Qt.RightButton
                    drag.target: tabItem.modelData.mailbox ? null : carried
                    drag.threshold: 10
                    onPressed: function(ev) { carried.Drag.hotSpot = Qt.point(ev.x, ev.y) }
                    // Middle-click closes a tab, as in browsers.
                    onClicked: function(ev) {
                        if (ev.button === Qt.MiddleButton) { if (!tabItem.modelData.mailbox) strip.owner.closeTab(tabItem.tab) }
                        else if (ev.button === Qt.RightButton) { if (!tabItem.modelData.mailbox) { tabMenu.tab = tabItem.tab; tabMenu.open() } }
                        else strip.owner.tabIndex = tabItem.index
                    }
                }
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 10; anchors.rightMargin: 4
                    spacing: 6
                    Icon { name: tabItem.modelData.mailbox ? "inbox" : tabItem.modelData.attachment ? "paperclip" : "mail"; width: 14; height: 14; color: tabItem.on ? Theme.accent : Theme.secondary }
                    Text {
                        id: tabTitle
                        Layout.fillWidth: true
                        text: tabItem.modelData.title
                        elide: Text.ElideRight
                        ToolTip.visible: tabHov.hovered && tabTitle.truncated
                        ToolTip.delay: 400
                        ToolTip.text: tabItem.modelData.title
                        color: tabItem.on ? Theme.text : Theme.secondary
                        font.pixelSize: 12
                        font.weight: tabItem.on ? Font.DemiBold : Font.Normal
                    }
                    ToolButtonX {
                        // On the open tab, or the one under the pointer.
                        visible: !tabItem.modelData.mailbox
                        opacity: tabItem.on || tabHov.hovered ? 1 : 0
                        implicitWidth: 22; implicitHeight: 22
                        icon: "close"; tip: "Close Tab (Ctrl+W)"
                        onClicked: strip.owner.closeTab(tabItem.tab)
                    }
                }
            }
        }
    }
    // A tab dragged over the row: where it would go.
    Rectangle {
        visible: strip.dropSlot >= 0
        x: strip.dropX - 1
        y: 6; width: 3; height: strip.height - 10
        radius: 1.5
        color: Theme.accent
        z: 5
    }
    // Tabs dropped here: from this row (moved along it) or another window.
    DropArea {
        anchors.fill: parent
        keys: ["application/x-almanac-tab"]
        onEntered: function(drag) { if (!Theme.tabDrag) drag.accepted = false; else strip.dropSlot = strip.slotAt(drag.x) }
        onPositionChanged: function(drag) { strip.dropSlot = strip.slotAt(drag.x) }
        onExited: strip.dropSlot = -1
        onDropped: function(drop) {
            var at = strip.slotAt(drop.x)
            strip.dropSlot = -1
            if (!Theme.tabDrag) return
            Theme.tabDrag.target = strip.owner
            Theme.tabDrag.at = at
            drop.accept(Qt.MoveAction)
        }
    }

    // Right-click on a tab.
    P.Menu {
        id: tabMenu
        property int tab: -1
        P.MenuItem { icon.name: "tab-close"; text: "Close Tab"; onTriggered: strip.owner.closeTab(tabMenu.tab) }
        P.MenuItem { icon.name: "tab-close-other"; text: "Close Other Tabs"; enabled: strip.owner.tabs.length > 1; onTriggered: strip.owner.closeOtherTabs(tabMenu.tab) }
        P.MenuSeparator {}
        P.MenuItem { icon.name: "window-new"; text: "Move Tab to New Window"; enabled: tabMenu.tab >= 0 && strip.owner.canTearOut(tabMenu.tab); onTriggered: strip.owner.tearOut(tabMenu.tab) }
        P.MenuItem { icon.name: "go-home"; text: "Move Tab to Main Window"; visible: !!strip.owner.toMain; onTriggered: strip.owner.toMain(tabMenu.tab) }
        P.MenuItem { icon.name: "window-duplicate"; text: "Merge All Windows"; visible: !!strip.mail; enabled: !!strip.mail && strip.mail.tabWindows.length > 0; onTriggered: strip.mail.mergeAllWindows() }
    }
    // The tabs that don't fit (and every tab), in a drop-down.
    ToolButtonX {
        id: tabMore
        visible: strip.overflow
        anchors.right: parent.right; anchors.rightMargin: 6
        anchors.verticalCenter: parent.verticalCenter
        icon: "chevronDown"
        text: "" + (strip.all.length - strip.fit)
        tip: "All Tabs"
        onClicked: allMenu.open(tabMore)
        P.Menu {
            id: allMenu
            Instantiator {
                model: strip.all
                onObjectAdded: (i, o) => allMenu.insertItem(i, o)
                onObjectRemoved: (i, o) => allMenu.removeItem(o)
                delegate: P.MenuItem {
                    required property var modelData
                    required property int index
                    icon.name: modelData.mailbox ? "mail-folder-inbox" : modelData.attachment ? "mail-attachment" : "mail-message"
                    text: modelData.title
                    checkable: true
                    checked: strip.owner.tabIndex === index
                    onTriggered: strip.owner.tabIndex = index
                }
            }
            P.MenuSeparator {}
            P.MenuItem { icon.name: "tab-close"; text: strip.home ? "Close All Message Tabs" : "Close All Tabs"; onTriggered: strip.owner.closeAllTabs() }
        }
    }
}
