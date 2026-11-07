import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// "Are you sure?" before something that can't be taken back.
// ask(title, text, button, onYes [, other, onOther])
Popup {
    id: dlg
    property string title: ""
    property string text: ""
    property string yesText: "Delete"
    property string otherText: ""
    property bool destructive: true
    property var onYes: null
    property var onOther: null
    function ask(t, body, yes, yesFn, other, otherFn) {
        title = t; text = body; yesText = yes; onYes = yesFn
        otherText = other || ""; onOther = otherFn || null
        destructive = /delete|erase|discard|remove/i.test(yes)
        open()
    }
    parent: Overlay.overlay
    anchors.centerIn: parent
    width: 380
    modal: true
    padding: 20
    closePolicy: Popup.CloseOnEscape
    background: Rectangle { radius: 12; color: Theme.popup; border.color: Theme.border }
    contentItem: ColumnLayout {
        spacing: 10
        Text { Layout.fillWidth: true; text: dlg.title; wrapMode: Text.WordWrap; font.pixelSize: 15; font.weight: Font.DemiBold; color: Theme.text }
        Text { Layout.fillWidth: true; visible: dlg.text !== ""; text: dlg.text; wrapMode: Text.WordWrap; font.pixelSize: Theme.base; color: Theme.secondary }
        RowLayout {
            Layout.topMargin: 6
            Layout.fillWidth: true
            ToolButtonX { visible: dlg.otherText !== ""; text: dlg.otherText; onClicked: { dlg.close(); if (dlg.onOther) dlg.onOther() } }
            Item { Layout.fillWidth: true }
            ToolButtonX { text: "Cancel"; onClicked: dlg.close() }
            ToolButtonX {
                id: yes
                text: dlg.yesText
                primary: true
                onClicked: { dlg.close(); if (dlg.onYes) dlg.onYes() }
            }
        }
    }
    onOpened: yes.forceActiveFocus()
}
