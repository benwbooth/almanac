import QtQuick
import QtQuick.Controls.Basic

// A checkbox in the app's look: a small rounded box that fills with the
// accent colour and a tick when on.
CheckBox {
    id: c
    hoverEnabled: true
    font.pixelSize: Theme.base
    spacing: 8
    padding: 4
    indicator: Rectangle {
        implicitWidth: 16
        implicitHeight: 16
        x: c.leftPadding
        y: c.topPadding + (c.availableHeight - height) / 2
        radius: 4
        color: c.checked ? (c.down ? Qt.darker(Theme.accentFill, 1.15) : Theme.accentFill) : (c.down ? Theme.pressed : Theme.field)
        border.color: c.checked ? Theme.accentFill : (c.activeFocus || c.hovered ? Theme.accent : Theme.border)
        border.width: 1
        opacity: c.enabled ? 1 : 0.45
        Icon { anchors.centerIn: parent; visible: c.checked; name: "check"; width: 12; height: 12; color: Theme.accentInk; stroke: 2.6 }
    }
    contentItem: Text {
        leftPadding: c.indicator.width + c.spacing
        text: c.text
        font: c.font
        color: Theme.text
        opacity: c.enabled ? 1 : 0.45
        verticalAlignment: Text.AlignVCenter
        wrapMode: Text.WordWrap
    }
}
