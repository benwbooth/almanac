import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// Asks for an OpenPGP key's passphrase (Almanac asks itself rather than
// relying on GnuPG's pinentry, which many systems don't have).
// ask(title, text, button, choosing, onPass [, error]): `choosing` asks twice
// (a new key's passphrase).
Popup {
    id: dlg
    property string title: ""
    property string text: ""
    property string yesText: "OK"
    property string error: ""
    property bool choosing: false
    property var onPass: null
    readonly property bool valid: pass.text.length > 0 && (!choosing || pass.text === again.text)
    function ask(t, body, yes, choose, fn, err) {
        title = t; text = body; yesText = yes; choosing = !!choose; onPass = fn; error = err || ""
        pass.text = ""; again.text = ""
        open()
    }
    function accept() {
        if (!valid) return
        var p = pass.text, fn = onPass
        pass.text = ""; again.text = ""
        close()
        if (fn) fn(p)
    }
    parent: Overlay.overlay
    anchors.centerIn: parent
    width: 380
    modal: true
    padding: 20
    closePolicy: Popup.CloseOnEscape
    background: Rectangle { radius: 12; color: Theme.popup; border.color: Theme.border }
    onClosed: { pass.text = ""; again.text = "" }
    contentItem: ColumnLayout {
        spacing: 10
        Text { Layout.fillWidth: true; text: dlg.title; wrapMode: Text.WordWrap; font.pixelSize: 15; font.weight: Font.DemiBold; color: Theme.text }
        Text { Layout.fillWidth: true; visible: dlg.text !== ""; text: dlg.text; wrapMode: Text.WordWrap; font.pixelSize: Theme.base; color: Theme.secondary }
        FieldX {
            id: pass
            Layout.fillWidth: true
            echoMode: TextInput.Password
            placeholderText: "Passphrase"
            onAccepted: dlg.choosing ? again.forceActiveFocus() : dlg.accept()
        }
        FieldX {
            id: again
            visible: dlg.choosing
            Layout.fillWidth: true
            echoMode: TextInput.Password
            placeholderText: "Passphrase again"
            onAccepted: dlg.accept()
        }
        Text {
            Layout.fillWidth: true
            visible: text !== ""
            text: dlg.choosing && again.text !== "" && pass.text !== again.text ? "The two don't match" : dlg.error
            wrapMode: Text.WordWrap; font.pixelSize: 12; color: Theme.red
        }
        RowLayout {
            Layout.topMargin: 6
            Layout.fillWidth: true
            Item { Layout.fillWidth: true }
            ToolButtonX { text: "Cancel"; onClicked: dlg.close() }
            ToolButtonX { text: dlg.yesText; primary: true; enabled: dlg.valid; onClicked: dlg.accept() }
        }
    }
    onOpened: pass.forceActiveFocus()
}
