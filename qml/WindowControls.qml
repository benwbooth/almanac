import QtQuick
import QtQuick.Controls.Basic

// Minimise, maximise/restore and close at the right end of the top bar (the
// top bar is the window's title bar), drawn the way the system draws them:
// Windows' wide square buttons, KDE Plasma's (Breeze) round ones, GNOME's
// (Adwaita) round grey ones. macOS keeps its own buttons on the left.
Row {
    id: wc
    required property var window
    readonly property bool maximized: window.visibility === Window.Maximized
    readonly property string look: Theme.windowButtons
    spacing: look === "windows" ? 0 : (look === "breeze" ? 8 : 12)
    rightPadding: look === "windows" ? 0 : 12
    leftPadding: look === "windows" ? 0 : 4

    component WinButton: Item {
        id: b
        property string glyph
        property bool danger: false
        property real turn: 0
        signal hit()
        width: wc.look === "windows" ? 46 : (wc.look === "breeze" ? 28 : 30)
        height: Theme.toolbarHeight
        readonly property bool hot: area.containsMouse
        // Windows: a full-height block that lights up.
        Rectangle {
            visible: wc.look === "windows"
            anchors.fill: parent
            color: area.pressed ? (b.danger ? "#b71c1c" : Theme.pressed) : (b.hot ? (b.danger ? "#e81123" : Theme.hover) : "transparent")
        }
        // Breeze: a circle that fills on hover (red for close).
        // Adwaita: a faint grey circle that darkens on hover.
        Rectangle {
            visible: wc.look !== "windows"
            anchors.centerIn: parent
            width: b.width; height: b.width; radius: width / 2
            color: wc.look === "breeze"
                   ? (b.hot ? (b.danger ? "#da4453" : Theme.text) : "transparent")
                   : Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, area.pressed ? 0.28 : (b.hot ? 0.18 : 0.1))
        }
        Icon {
            anchors.centerIn: parent
            width: wc.look === "windows" ? 14 : 15
            height: width
            stroke: wc.look === "windows" ? 1.4 : 1.8
            rotation: b.turn
            name: b.glyph
            color: wc.look === "windows" ? (b.hot && b.danger ? "white" : Theme.text)
                   : wc.look === "breeze" ? (b.hot ? (b.danger ? "white" : Theme.window) : Theme.text)
                   : Theme.text
        }
        MouseArea { id: area; anchors.fill: parent; hoverEnabled: true; onClicked: b.hit() }
    }
    // Breeze: ⌄ minimise, ⌃ maximise, ◇ restore, ✕ close; elsewhere — □ ❐ ✕.
    WinButton {
        glyph: wc.look === "breeze" ? "chevronDown" : "winMinimize"
        onHit: wc.window.showMinimized()
    }
    WinButton {
        glyph: wc.look === "breeze" ? (wc.maximized ? "winMaximize" : "chevronDown") : (wc.maximized ? "winRestore" : "winMaximize")
        turn: wc.look === "breeze" ? (wc.maximized ? 45 : 180) : 0
        onHit: wc.maximized ? wc.window.showNormal() : wc.window.showMaximized()
    }
    WinButton { glyph: "winClose"; danger: true; onHit: wc.window.close() }
}
