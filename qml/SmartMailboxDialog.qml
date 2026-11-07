import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// A saved search shown in the sidebar (a smart mailbox).
Popup {
    // A stray click outside doesn't throw away the edits (Escape still closes).
    closePolicy: Popup.CloseOnEscape
    id: dlg
    property var store
    property string boxId: ""
    signal saved(string selection, string name)
    modal: true
    anchors.centerIn: Overlay.overlay
    width: 640
    padding: 20
    background: Rectangle { radius: 14; color: Theme.popup; border.color: Theme.border }

    function openFor(selection) {
        boxId = ""
        nameField.text = ""
        cond.conditions = [{ field: "from", op: "contains", value: "" }]
        cond.matchAll = true
        includeTrash.checked = false
        if (selection) {
            var s = JSON.parse(store.smartMailbox(selection))
            if (s) {
                boxId = s.id
                nameField.text = s.name
                cond.conditions = s.conditions
                cond.matchAll = s.match_all
                includeTrash.checked = s.include_trash
            }
        }
        open()
        nameField.forceActiveFocus()
    }

    contentItem: ColumnLayout {
        spacing: 12
        Text { text: dlg.boxId ? "Smart Mailbox" : "New Smart Mailbox"; font.pixelSize: 18; font.weight: Font.Bold; color: Theme.text }
        FieldX { id: nameField; Layout.fillWidth: true; placeholderText: "Name, e.g. From my team"; selectByMouse: true }
        ConditionEditor {
            id: cond
            Layout.fillWidth: true
            fields: [["from", "From"], ["to", "To"], ["cc", "Cc"], ["subject", "Subject"], ["body", "Message"], ["any", "Any part"],
                     ["account", "Account"], ["unread", "Is unread", false], ["flagged", "Is flagged", false],
                     ["attachments", "Has attachments", false], ["vip", "Sender is a VIP", false], ["days", "Received in the last … days", "number"]]
        }
        CheckX { id: includeTrash; text: "Include messages in Trash" }
        RowLayout {
            ToolButtonX { visible: dlg.boxId !== ""; icon: "trash"; text: "Delete"; onClicked: { store.deleteSmartMailbox(dlg.boxId); dlg.close() } }
            Item { Layout.fillWidth: true }
            ToolButtonX { text: "Cancel"; onClicked: dlg.close() }
            ToolButtonX {
                text: "Save"; primary: true
                onClicked: {
                    var sel = store.saveSmartMailbox(JSON.stringify({ id: dlg.boxId, name: nameField.text, match_all: cond.matchAll,
                                                                       conditions: cond.conditions, include_trash: includeTrash.checked }))
                    dlg.saved(sel, nameField.text || "Smart Mailbox")
                    dlg.close()
                }
            }
        }
    }
}
