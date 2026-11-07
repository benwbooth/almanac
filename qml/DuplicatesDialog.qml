import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// "Look for Duplicates": cards that seem to be the same person, merged
// into one with all their numbers and addresses.
Popup {
    id: dlg
    property var store
    property var sets: { store.revision; return visible ? JSON.parse(store.duplicates()) : [] }
    modal: true
    anchors.centerIn: Overlay.overlay
    width: 520
    height: 440
    padding: 20
    background: Rectangle { radius: 14; color: Theme.popup; border.color: Theme.border }

    contentItem: ColumnLayout {
        spacing: 12
        RowLayout {
            Text { Layout.fillWidth: true; text: "Duplicate Cards"; font.pixelSize: 18; font.weight: Font.Bold; color: Theme.text }
            ToolButtonX {
                visible: dlg.sets.length > 1
                text: "Merge All"
                onClicked: { var s = dlg.sets; for (var i = 0; i < s.length; i++) store.mergeContacts(JSON.stringify(s[i].ids)) }
            }
            ToolButtonX { text: "Done"; onClicked: dlg.close() }
        }
        ListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 4
            model: dlg.sets
            delegate: Rectangle {
                required property var modelData
                width: ListView.view.width
                height: 48
                radius: 8
                color: Theme.card
                RowLayout {
                    anchors.fill: parent
                    anchors.margins: 10
                    Avatar { name: modelData.names[0]; size: 28 }
                    Text {
                        Layout.fillWidth: true
                        text: modelData.names.join("  ·  ")
                        color: Theme.text; elide: Text.ElideRight
                    }
                    ToolButtonX { text: "Merge " + modelData.ids.length; primary: true; onClicked: store.mergeContacts(JSON.stringify(modelData.ids)) }
                }
            }
            Text { anchors.centerIn: parent; visible: dlg.sets.length === 0; text: "No duplicates found"; color: Theme.tertiary }
        }
    }
}
