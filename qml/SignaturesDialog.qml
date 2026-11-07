import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// One signature per sending account, added to new messages and replies.
Popup {
    // A stray click outside doesn't throw away the edits (Escape still closes).
    closePolicy: Popup.CloseOnEscape
    id: dlg
    property var store
    property var senders: []
    modal: true
    anchors.centerIn: Overlay.overlay
    width: 560
    padding: 20
    background: Rectangle { radius: 14; color: Theme.popup; border.color: Theme.border }
    onOpened: { senders = JSON.parse(store.senders()); pick.currentIndex = 0; sig.text = store.signature(senders.length ? senders[0].id : "") }

    contentItem: ColumnLayout {
        spacing: 12
        Text { text: "Signatures"; font.pixelSize: 18; font.weight: Font.Bold; color: Theme.text }
        ComboX {
            id: pick
            Layout.fillWidth: true
            model: dlg.senders.map(function(s) { return s.label })
            onActivated: sig.text = store.signature(dlg.senders[currentIndex].id)
        }
        TextArea {
            id: sig
            Layout.fillWidth: true
            Layout.preferredHeight: 140
            wrapMode: TextEdit.Wrap
            placeholderText: "e.g.\nBest wishes,\nAlex"
            selectByMouse: true
            background: Rectangle { radius: 6; border.color: Theme.border; color: Theme.field }
        }
        RowLayout {
            Item { Layout.fillWidth: true }
            ToolButtonX { text: "Close"; onClicked: dlg.close() }
            ToolButtonX {
                text: "Save"; primary: true
                onClicked: { if (dlg.senders.length) store.setSignature(dlg.senders[pick.currentIndex].id, sig.text); dlg.close() }
            }
        }
    }
}
