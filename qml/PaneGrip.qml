import QtQuick
import QtQuick.Controls.Basic

// An easy-to-catch grip over the thin line between two panes of a
// SplitView (the line itself is one pixel, too thin to grab): drag it to
// change the width of the pane on its left.
MouseArea {
    id: grip
    required property Item pane
    // (A drag is the person's choice: the keeper saves it at once.)
    property PaneKeeper keeper: null
    // A divider below the pane (it changes the pane's height) instead of beside it.
    property bool vertical: false
    property real minimum: pane.SplitView.minimumWidth > 0 ? pane.SplitView.minimumWidth : 80
    property real maximum: pane.SplitView.maximumWidth > 0 && isFinite(pane.SplitView.maximumWidth) ? pane.SplitView.maximumWidth : 100000
    visible: pane.visible
    // Over the line, 4 pixels each side of it.
    x: { pane.x; pane.width; return vertical ? pane.mapToItem(parent, 0, 0).x : pane.mapToItem(parent, pane.width, 0).x - 4 }
    y: { pane.y; pane.height; return vertical ? pane.mapToItem(parent, 0, pane.height).y - 4 : pane.mapToItem(parent, 0, 0).y }
    width: vertical ? pane.width : 9
    height: vertical ? 9 : pane.height
    z: 1000
    hoverEnabled: true
    preventStealing: true
    cursorShape: vertical ? Qt.SplitVCursor : Qt.SplitHCursor
    property real startX: 0
    property real startWidth: 0
    onPressed: function(ev) { if (keeper) keeper.settle(); var p = mapToItem(null, ev.x, ev.y); startX = vertical ? p.y : p.x; startWidth = vertical ? pane.height : pane.width }
    onPositionChanged: function(ev) {
        if (!pressed) return
        var p = mapToItem(null, ev.x, ev.y)
        var d = (vertical ? p.y : p.x) - startX
        if (vertical) pane.SplitView.preferredHeight = Math.max(100, startWidth + d)
        else pane.SplitView.preferredWidth = Math.max(minimum, Math.min(maximum, startWidth + d))
    }
}
