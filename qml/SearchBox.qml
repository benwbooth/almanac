import QtQuick
import QtQuick.Layouts
import QtQuick.Controls.Basic

// Rounded search box with a magnifier and a clear button.
Rectangle {
    id: field
    property alias text: input.text
    property string placeholderText: "Search"
    readonly property bool hasFocus: input.activeFocus || popField.activeFocus
    // On a very narrow window only the magnifier shows; it opens the field.
    property bool collapsed: false
    // Still searching: a small spinner in place of the magnifier.
    property bool busy: false
    // Escape pressed, or the clear button clicked (the text is cleared too).
    signal escapePressed()
    signal clearClicked()
    function focusField() {
        if (collapsed) { pop.open(); popField.forceActiveFocus(); popField.selectAll() }
        else { input.forceActiveFocus(); input.selectAll() }
    }

    implicitWidth: 200
    implicitHeight: 28
    Layout.preferredHeight: 28
    Layout.alignment: Qt.AlignVCenter
    radius: 7
    clip: true
    color: collapsed ? (openArea.containsMouse || input.text !== "" ? Theme.hover : "transparent") : (input.activeFocus ? Theme.field : Theme.fieldIdle)
    border.color: input.activeFocus ? Theme.accent : "transparent"
    border.width: input.activeFocus ? 2 : 0

    BusyIndicator {
        visible: field.busy && !field.collapsed
        running: visible
        width: 16; height: 16
        padding: 0
        anchors.left: parent.left
        anchors.leftMargin: 8
        anchors.verticalCenter: parent.verticalCenter
    }
    Icon {
        visible: !(field.busy && !field.collapsed)
        name: "search"
        width: 15; height: 15
        color: Theme.tertiary
        anchors.left: parent.left
        anchors.leftMargin: 9
        anchors.verticalCenter: parent.verticalCenter
    }
    TextInput {
        id: input
        visible: !field.collapsed
        anchors.left: parent.left
        anchors.leftMargin: 30
        anchors.right: clear.visible ? clear.left : parent.right
        anchors.rightMargin: 6
        anchors.verticalCenter: parent.verticalCenter
        font.pixelSize: Theme.base
        color: Theme.text
        selectionColor: Theme.accentFill
        selectedTextColor: Theme.accentInk
        selectByMouse: true
        clip: true
        Keys.onEscapePressed: { text = ""; field.escapePressed() }
        Text {
            visible: input.text === "" && !input.activeFocus
            text: field.placeholderText
            color: Theme.tertiary
            font: input.font
        }
    }
    Rectangle {
        id: clear
        visible: input.text !== "" && !field.collapsed
        width: 16; height: 16; radius: 8
        color: Theme.tertiary
        anchors.right: parent.right
        anchors.rightMargin: 6
        anchors.verticalCenter: parent.verticalCenter
        Icon { anchors.centerIn: parent; width: 10; height: 10; name: "close"; color: "white"; stroke: 3 }
        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: { input.text = ""; field.clearClicked() } }
    }
    MouseArea {
        id: openArea
        anchors.fill: parent
        z: field.collapsed ? 1 : -1
        hoverEnabled: field.collapsed
        cursorShape: field.collapsed ? Qt.PointingHandCursor : Qt.IBeamCursor
        onClicked: field.collapsed ? field.focusField() : input.forceActiveFocus()
    }
    Popup {
        id: pop
        y: field.height + 4
        x: Math.min(0, -(240 - field.width))
        width: 240
        padding: 6
        background: Rectangle { radius: 9; color: Theme.popup; border.color: Theme.border }
        FieldX {
            id: popField
            width: parent.width
            placeholderText: field.placeholderText
            text: input.text
            onTextEdited: input.text = text
            Keys.onEscapePressed: pop.close()
            onAccepted: pop.close()
        }
    }
}
