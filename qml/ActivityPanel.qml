import QtQuick
import QtQuick.Controls.Basic

// What's going on (checking for mail, downloading, importing, sending…) at
// the bottom of the sidebar, as Apple Mail shows it: the step, how far it has
// got, and a bar (a moving one when how far isn't known). A message saying
// what happened ("up to date", "Sent") stays a few seconds, then goes.
Rectangle {
    id: act
    property var store
    readonly property bool working: store.busy || store.progressTotal > 0
    property bool recent: false
    readonly property bool shown: store.statusText !== "" && (working || recent)
    visible: shown
    height: shown ? col.implicitHeight + 18 : 0
    color: Theme.sidebarBottom
    Timer { id: fade; interval: 6000; onTriggered: act.recent = false }
    Connections {
        target: act.store
        function onStatusTextChanged() { act.recent = true; fade.restart() }
        function onBusyChanged() { if (!act.store.busy) fade.restart() }
    }
    Rectangle { width: parent.width; height: 1; color: Theme.borderSoft }
    Column {
        id: col
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: 10
        anchors.topMargin: 9
        spacing: 6
        // "Work: downloading messages (40 of 1200)…" → "Work — Downloading messages"
        Text {
            width: parent.width
            property var parts: /^(.*) \((\d+) of (\d+)\)…$/.exec(act.store.statusText)
            text: (parts ? parts[1] : act.store.statusText).replace(/^([^:]+): (.)/, function(all, who, c) { return who + " — " + c.toUpperCase() })
            color: Theme.secondary
            font.pixelSize: 11
            wrapMode: Text.Wrap
            maximumLineCount: 3
            elide: Text.ElideRight
        }
        // "42% · 3,360 of 8,000 messages" (or mailboxes, while a check reads them)
        Text {
            visible: act.store.progressTotal > 0
            width: parent.width
            readonly property int done: act.store.progressDone
            readonly property int total: Math.max(1, act.store.progressTotal)
            text: Math.floor(done * 100 / total) + "% · " + done.toLocaleString(Qt.locale(), "f", 0) + " of "
                  + act.store.progressTotal.toLocaleString(Qt.locale(), "f", 0)
                  + (/: reading /.test(act.store.statusText) ? " mailboxes" : " messages")
            color: Theme.tertiary
            font.pixelSize: 11
            font.features: { "tnum": 1 }
            wrapMode: Text.Wrap
        }
        Rectangle {
            id: track
            visible: act.working
            width: parent.width
            height: 5
            radius: 2.5
            clip: true
            color: Theme.dark ? Qt.rgba(1, 1, 1, 0.12) : Qt.rgba(0, 0, 0, 0.1)
            Rectangle {
                id: fill
                readonly property bool known: act.store.progressTotal > 0
                height: parent.height
                radius: parent.radius
                color: Theme.accent
                width: known ? parent.width * Math.min(1, act.store.progressDone / Math.max(1, act.store.progressTotal)) : parent.width * 0.3
                x: known ? 0 : slide.x
                Behavior on width { enabled: fill.known; NumberAnimation { duration: 250 } }
            }
            // The moving bar's place (when how far isn't known).
            Item {
                id: slide
                NumberAnimation on x {
                    running: act.working && act.store.progressTotal === 0 && act.visible
                    from: -track.width * 0.3; to: track.width
                    duration: 1100; loops: Animation.Infinite
                }
            }
        }
    }
    ToolTip.visible: hover.hovered
    ToolTip.text: act.store.statusText
    ToolTip.delay: 500
    HoverHandler { id: hover }
}
