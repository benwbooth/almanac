import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// Rows of "field — how — value" conditions, used by mail rules, smart
// mailboxes and smart groups. Edit `conditions` (an array of
// { field, op, value }) and read it back when saving.
ColumnLayout {
    id: ce
    property var conditions: []
    property bool matchAll: true
    // [[field, label, needsValue], …]
    property var fields: []
    readonly property var ops: [["contains", "contains"], ["not_contains", "does not contain"], ["is", "is"], ["begins", "begins with"], ["ends", "ends with"]]
    spacing: 6

    function refresh() { conditions = JSON.parse(JSON.stringify(conditions)) }
    function needsValue(f) { for (var i = 0; i < fields.length; i++) if (fields[i][0] === f) return fields[i][2] !== false; return true }
    function textual(f) { for (var i = 0; i < fields.length; i++) if (fields[i][0] === f) return fields[i][2] === true || fields[i][2] === undefined; return true }

    RowLayout {
        Text { text: "If"; color: Theme.text }
        ComboX {
            Layout.preferredWidth: 90
            model: ["all", "any"]
            currentIndex: ce.matchAll ? 0 : 1
            onActivated: ce.matchAll = currentIndex === 0
        }
        Text { text: "of these are true:"; color: Theme.text }
    }
    Repeater {
        model: ce.conditions
        delegate: RowLayout {
            required property var modelData
            required property int index
            Layout.fillWidth: true
            ComboX {
                Layout.preferredWidth: 170
                model: ce.fields.map(function(f) { return f[1] })
                currentIndex: Math.max(0, ce.fields.findIndex(function(f) { return f[0] === modelData.field }))
                onActivated: { ce.conditions[index].field = ce.fields[currentIndex][0]; ce.refresh() }
            }
            ComboX {
                visible: ce.textual(modelData.field)
                Layout.preferredWidth: 150
                model: ce.ops.map(function(o) { return o[1] })
                currentIndex: Math.max(0, ce.ops.findIndex(function(o) { return o[0] === modelData.op }))
                onActivated: ce.conditions[index].op = ce.ops[currentIndex][0]
            }
            FieldX {
                visible: ce.needsValue(modelData.field)
                Layout.fillWidth: true
                text: modelData.value || ""
                selectByMouse: true
                onTextEdited: ce.conditions[index].value = text
            }
            Item { Layout.fillWidth: !ce.needsValue(modelData.field) }
            ToolButtonX { icon: "minus"; onClicked: { ce.conditions.splice(index, 1); ce.refresh() } }
        }
    }
    ToolButtonX {
        icon: "plus"; text: "Add Condition"
        onClicked: { ce.conditions.push({ field: ce.fields[0][0], op: "contains", value: "" }); ce.refresh() }
    }
}
