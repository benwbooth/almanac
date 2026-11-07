import QtQuick

// Without the system's window frame, these thin strips along the edges and
// corners let the window be resized as usual.
Item {
    id: edges
    required property var window
    property int grip: 5
    anchors.fill: parent
    z: 10000
    visible: window.visibility !== Window.Maximized && window.visibility !== Window.FullScreen
    component Edge: MouseArea {
        property int edgesToMove
        hoverEnabled: true
        onPressed: edges.window.startSystemResize(edgesToMove)
    }
    Edge { x: edges.grip; width: parent.width - 2 * edges.grip; height: edges.grip; edgesToMove: Qt.TopEdge; cursorShape: Qt.SizeVerCursor }
    Edge { x: edges.grip; y: parent.height - edges.grip; width: parent.width - 2 * edges.grip; height: edges.grip; edgesToMove: Qt.BottomEdge; cursorShape: Qt.SizeVerCursor }
    Edge { y: edges.grip; width: edges.grip; height: parent.height - 2 * edges.grip; edgesToMove: Qt.LeftEdge; cursorShape: Qt.SizeHorCursor }
    Edge { x: parent.width - edges.grip; y: edges.grip; width: edges.grip; height: parent.height - 2 * edges.grip; edgesToMove: Qt.RightEdge; cursorShape: Qt.SizeHorCursor }
    Edge { width: edges.grip * 2; height: edges.grip * 2; edgesToMove: Qt.TopEdge | Qt.LeftEdge; cursorShape: Qt.SizeFDiagCursor }
    Edge { x: parent.width - edges.grip * 2; width: edges.grip * 2; height: edges.grip * 2; edgesToMove: Qt.TopEdge | Qt.RightEdge; cursorShape: Qt.SizeBDiagCursor }
    Edge { y: parent.height - edges.grip * 2; width: edges.grip * 2; height: edges.grip * 2; edgesToMove: Qt.BottomEdge | Qt.LeftEdge; cursorShape: Qt.SizeBDiagCursor }
    Edge { x: parent.width - edges.grip * 2; y: parent.height - edges.grip * 2; width: edges.grip * 2; height: edges.grip * 2; edgesToMove: Qt.BottomEdge | Qt.RightEdge; cursorShape: Qt.SizeFDiagCursor }
}
