import QtQuick
import QtQuick.Window

// Saved pane sizes put back, and kept right while the window settles. On
// Wayland the desktop gives a window its real size a moment after it opens:
// the sizes are put back again then, and sizes seen before (panes squeezed
// into a window not yet its size) aren't saved over the saved ones.
// Settled at the window's first real resize, a drag of a pane's edge, or
// after 3 seconds, whichever comes first.
QtObject {
    id: k
    // Something in the window (for the window's size).
    required property Item item
    // Puts the sizes back: function(sizes, windowWidth, windowHeight).
    property var apply: null
    property var sizes: ({})
    property bool settled: true
    // A pane's edge being dragged (a SplitView's `resizing`, or a PaneGrip):
    // that's the person's choice, kept at once.
    property bool resizing: false
    onResizingChanged: if (resizing) settle()

    function restore(s) {
        sizes = s || {}
        settled = false
        timer.restart()
        put()
    }
    function put() {
        var win = item.Window.window
        if (apply) apply(sizes, (win ? win.width : 0) || 1280, (win ? win.height : 0) || 800)
    }
    function settle() { if (!settled) { settled = true; timer.stop(); after.stop() } }

    property Timer timer: Timer { interval: 3000; onTriggered: k.settle() }
    // (The panes laid out at the new size first.)
    property Timer after: Timer { interval: 250; onTriggered: k.settle() }
    property Connections watch: Connections {
        target: k.item.Window.window
        function onWidthChanged() { k.resized() }
        function onHeightChanged() { k.resized() }
    }
    function resized() {
        if (settled) return
        put()
        after.restart()
    }
}
