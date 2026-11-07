import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// Mail rules: when new mail matches, move / flag / mark read / delete it.
Popup {
    // A stray click outside doesn't throw away the edits (Escape still closes).
    closePolicy: Popup.CloseOnEscape
    id: dlg
    property var store
    property var rules: { store.revision; return visible ? JSON.parse(store.rules()) : [] }
    property var folders: { store.revision; return visible ? JSON.parse(store.mailFolders()) : [] }
    property var editing: null
    readonly property var mailFields: [["from", "From"], ["to", "To"], ["cc", "Cc"], ["subject", "Subject"], ["body", "Message"],
        ["any", "Any part"], ["account", "Account"], ["unread", "Is unread", false], ["flagged", "Is flagged", false],
        ["attachments", "Has attachments", false], ["vip", "Sender is a VIP", false]]
    readonly property var actionKinds: [["move", "Move to folder"], ["flag", "Flag it"], ["read", "Mark as read"], ["junk", "Move to Junk"], ["delete", "Move to Trash"]]
    modal: true
    anchors.centerIn: Overlay.overlay
    width: 640
    height: Math.min(parent ? parent.height - 60 : 600, 600)
    padding: 20
    background: Rectangle { radius: 14; color: Theme.popup; border.color: Theme.border }

    function edit(r) {
        editing = r ? JSON.parse(JSON.stringify(r)) : { id: "", name: "", enabled: true, match_all: true,
                    conditions: [{ field: "from", op: "contains", value: "" }], actions: [{ kind: "move", value: "" }] }
        cond.conditions = editing.conditions
        cond.matchAll = editing.match_all
        ruleName.text = editing.name
    }
    function refreshActions() { editing = JSON.parse(JSON.stringify(editing)) }

    contentItem: ColumnLayout {
        spacing: 12
        RowLayout {
            Text { Layout.fillWidth: true; text: dlg.editing ? "Rule" : "Rules"; font.pixelSize: 18; font.weight: Font.Bold; color: Theme.text }
            ToolButtonX { visible: !dlg.editing; text: "Apply to Inbox Now"; onClicked: store.applyRulesNow() }
            ToolButtonX { visible: !dlg.editing; text: "Done"; onClicked: dlg.close() }
        }
        Text {
            visible: !dlg.editing
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            color: Theme.secondary
            text: "Rules run on new mail as it arrives in an Inbox, from top to bottom."
        }
        ListView {
            visible: !dlg.editing
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: dlg.rules
            spacing: 4
            delegate: Rectangle {
                required property var modelData
                width: ListView.view.width
                height: 40
                radius: 8
                color: Theme.card
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 10
                    anchors.rightMargin: 6
                    CheckX {
                        checked: modelData.enabled
                        onToggled: { var r = JSON.parse(JSON.stringify(modelData)); r.enabled = checked; store.saveRule(JSON.stringify(r)) }
                    }
                    Text { Layout.fillWidth: true; text: modelData.name; color: Theme.text; elide: Text.ElideRight }
                    ToolButtonX { text: "Edit"; onClicked: dlg.edit(modelData) }
                    ToolButtonX { icon: "trash"; onClicked: store.deleteRule(modelData.id) }
                }
            }
            Text { anchors.centerIn: parent; visible: dlg.rules.length === 0; text: "No rules yet"; color: Theme.tertiary }
        }
        ToolButtonX { visible: !dlg.editing; icon: "plus"; text: "Add Rule"; primary: true; onClicked: dlg.edit(null) }

        // ----------------------------------------------------- one rule
        Flickable {
            visible: dlg.editing !== null
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            contentHeight: form.implicitHeight
            ColumnLayout {
                id: form
                width: parent.width
                spacing: 10
                FieldX { id: ruleName; Layout.fillWidth: true; placeholderText: "Description, e.g. Newsletters"; selectByMouse: true }
                ConditionEditor { id: cond; Layout.fillWidth: true; fields: dlg.mailFields }
                Text { text: "Then:"; color: Theme.text; font.weight: Font.DemiBold }
                Repeater {
                    model: dlg.editing ? dlg.editing.actions : []
                    delegate: RowLayout {
                        required property var modelData
                        required property int index
                        Layout.fillWidth: true
                        ComboX {
                            Layout.preferredWidth: 170
                            model: dlg.actionKinds.map(function(a) { return a[1] })
                            currentIndex: Math.max(0, dlg.actionKinds.findIndex(function(a) { return a[0] === modelData.kind }))
                            onActivated: { dlg.editing.actions[index].kind = dlg.actionKinds[currentIndex][0]; dlg.refreshActions() }
                        }
                        ComboX {
                            visible: modelData.kind === "move"
                            Layout.fillWidth: true
                            model: dlg.folders.map(function(f) { return f.account + " › " + f.name })
                            currentIndex: Math.max(0, dlg.folders.findIndex(function(f) { return f.id === modelData.value }))
                            onActivated: dlg.editing.actions[index].value = dlg.folders[currentIndex].id
                            Component.onCompleted: if (modelData.kind === "move" && !modelData.value && dlg.folders.length) dlg.editing.actions[index].value = dlg.folders[0].id
                        }
                        Item { Layout.fillWidth: modelData.kind !== "move" }
                        ToolButtonX { icon: "minus"; onClicked: { dlg.editing.actions.splice(index, 1); dlg.refreshActions() } }
                    }
                }
                ToolButtonX { icon: "plus"; text: "Add Action"; onClicked: { dlg.editing.actions.push({ kind: "flag", value: "" }); dlg.refreshActions() } }
            }
        }
        RowLayout {
            visible: dlg.editing !== null
            Item { Layout.fillWidth: true }
            ToolButtonX { text: "Cancel"; onClicked: dlg.editing = null }
            ToolButtonX {
                text: "Save"; primary: true
                onClicked: {
                    var r = dlg.editing
                    r.name = ruleName.text
                    r.conditions = cond.conditions
                    r.match_all = cond.matchAll
                    store.saveRule(JSON.stringify(r))
                    dlg.editing = null
                }
            }
        }
    }
}
