import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// Flat toolbar button: an icon, optional text, hover and pressed states.
Rectangle {
    id: btn
    property string icon: ""
    property string text: ""
    // Size of the text (a symbol used as the icon can be set larger).
    property int textSize: Theme.base
    property string tip: ""
    property bool checked: false
    property bool primary: false
    property color iconColor: primary ? "white" : (checked ? Theme.accent : Theme.secondary)
    signal clicked()
    // Set by the top bar when there isn't room: the button folds into its » menu.
    property bool overflowed: false
    // Higher stays on the bar longer when space runs out.
    property int priority: 0
    readonly property real naturalWidth: row.implicitWidth + (text ? 20 : 12)
    implicitHeight: 30
    implicitWidth: overflowed ? 0 : naturalWidth
    // Layouts don't always notice the width dropping to nothing; say it outright.
    Layout.maximumWidth: overflowed ? 0 : Number.POSITIVE_INFINITY
    radius: 7
    opacity: enabled ? 1 : 0.4
    color: primary ? (area.pressed ? Qt.darker(Theme.accentFill, 1.15) : Theme.accentFill)
                   : (area.pressed ? Theme.pressed : (checked ? Theme.accentSoft : (area.containsMouse ? Theme.hover : "transparent")))

    Row {
        id: row
        visible: !btn.overflowed
        anchors.centerIn: parent
        spacing: 6
        Icon {
            visible: btn.icon !== ""
            name: btn.icon
            color: btn.iconColor
            width: 18; height: 18
            anchors.verticalCenter: parent.verticalCenter
        }
        Text {
            visible: btn.text !== ""
            text: btn.text
            color: btn.primary ? "white" : (btn.checked ? Theme.accent : Theme.text)
            font.pixelSize: btn.textSize
            font.weight: btn.primary ? Font.DemiBold : Font.Normal
            anchors.verticalCenter: parent.verticalCenter
        }
    }
    MouseArea {
        id: area
        enabled: !btn.overflowed
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: btn.clicked()
    }
    ToolTip.visible: tip !== "" && area.containsMouse
    ToolTip.delay: 600
    ToolTip.text: tip
}
