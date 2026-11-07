import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// A group that fills itself: every card matching the conditions.
Popup {
    // A stray click outside doesn't throw away the edits (Escape still closes).
    closePolicy: Popup.CloseOnEscape
    id: dlg
    property var store
    property string groupId: ""
    signal saved(string id)
    modal: true
    anchors.centerIn: Overlay.overlay
    width: 620
    padding: 20
    background: Rectangle { radius: 14; color: Theme.popup; border.color: Theme.border }

    function openFor(g) {
        groupId = g ? g.id : ""
        nameField.text = g ? g.name : ""
        cond.conditions = g ? JSON.parse(JSON.stringify(g.conditions)) : [{ field: "company", op: "contains", value: "" }]
        cond.matchAll = g ? g.matchAll : true
        open()
        nameField.forceActiveFocus()
    }

    contentItem: ColumnLayout {
        spacing: 12
        Text { text: dlg.groupId ? "Smart Group" : "New Smart Group"; font.pixelSize: 18; font.weight: Font.Bold; color: Theme.text }
        FieldX { id: nameField; Layout.fillWidth: true; placeholderText: "Name, e.g. Colleagues at Northwind"; selectByMouse: true }
        ConditionEditor {
            id: cond
            Layout.fillWidth: true
            fields: [["name", "Name"], ["company", "Company or job"], ["email", "Email"], ["phone", "Phone"], ["address", "Address"],
                     ["notes", "Note"], ["group", "Group"], ["birthday_month", "Birthday month (1–12)", "number"]]
        }
        RowLayout {
            ToolButtonX { visible: dlg.groupId !== ""; icon: "trash"; text: "Delete"; onClicked: { store.deleteSmartGroup(dlg.groupId); dlg.close() } }
            Item { Layout.fillWidth: true }
            ToolButtonX { text: "Cancel"; onClicked: dlg.close() }
            ToolButtonX {
                text: "Save"; primary: true
                onClicked: {
                    var id = store.saveSmartGroup(JSON.stringify({ id: dlg.groupId, name: nameField.text, match_all: cond.matchAll, conditions: cond.conditions }))
                    dlg.saved(id)
                    dlg.close()
                }
            }
        }
    }
}
