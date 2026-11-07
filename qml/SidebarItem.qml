import QtQuick
import QtQuick.Controls.Basic

// One row in a sidebar: icon, name and a count badge.
Rectangle {
    id: item
    property string icon: ""
    property color iconColor: Theme.accent
    property string label: ""
    property int badge: 0
    property bool selected: false
    property int indent: 0
    signal clicked()
    signal doubleClicked()
    signal rightClicked()
    width: parent ? parent.width : 200
    // Compact, as in Apple Mail's sidebar.
    height: 24
    radius: 6
    color: selected ? Theme.sidebarSelected : (area.containsMouse ? Theme.hover : "transparent")

    Icon {
        id: ic
        visible: item.icon !== ""
        name: item.icon
        color: item.iconColor
        width: 16; height: 16
        anchors.left: parent.left
        anchors.leftMargin: 8 + item.indent
        anchors.verticalCenter: parent.verticalCenter
    }
    Text {
        id: labelText
        text: item.label
        color: Theme.text
        elide: Text.ElideRight
        // Cut short: the whole name when pointed at.
        ToolTip.visible: area.containsMouse && truncated
        ToolTip.delay: 500
        ToolTip.text: item.label
        font.pixelSize: Theme.base
        anchors.left: item.icon !== "" ? ic.right : parent.left
        anchors.leftMargin: item.icon !== "" ? 8 : 10 + item.indent
        anchors.right: badgeText.left
        anchors.rightMargin: 6
        anchors.verticalCenter: parent.verticalCenter
    }
    Text {
        id: badgeText
        text: item.badge > 0 ? item.badge : ""
        color: Theme.secondary
        font.pixelSize: Theme.base - 1
        anchors.right: parent.right
        anchors.rightMargin: 10
        anchors.verticalCenter: parent.verticalCenter
    }
    MouseArea {
        id: area
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        onClicked: function(ev) { if (ev.button === Qt.RightButton) item.rightClicked(); else item.clicked() }
        onDoubleClicked: item.doubleClicked()
    }
}
