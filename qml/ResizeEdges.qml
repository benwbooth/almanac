import QtQuick

// Without the system's window frame, strips along the edges and corners let
// the window be resized as usual: the shared ChromeResizeEdges
// (hearth/shared/app-menus).
ChromeResizeEdges {
    anchors.fill: parent
    z: 10000
}
