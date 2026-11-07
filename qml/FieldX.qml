import QtQuick
import QtQuick.Controls.Basic

// A one-line text box in the app's look (the same height and corners as
// the drop-down lists), used everywhere instead of the plain default one.
TextField {
    id: f
    implicitHeight: Math.max(30, Math.ceil(font.pixelSize * 1.45) + topPadding + bottomPadding)
    leftPadding: 9
    rightPadding: 9
    topPadding: 5
    bottomPadding: 5
    font.pixelSize: Theme.base
    color: Theme.text
    placeholderTextColor: Theme.tertiary
    selectionColor: Theme.accentFill
    selectedTextColor: Theme.accentInk
    selectByMouse: true
    background: Rectangle {
        implicitWidth: 120
        radius: 7
        color: f.enabled ? Theme.field : Theme.fieldIdle
        border.color: f.activeFocus ? Theme.accent : Theme.border
        border.width: f.activeFocus ? 2 : 1
    }
}
