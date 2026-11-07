import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// Messages waiting to go out ("Send Later" and ones that will be retried).
Popup {
    id: dlg
    property var store
    property var items: { store.revision; return visible ? JSON.parse(store.outbox()) : [] }
    signal editRequested(var payload)
    modal: true
    anchors.centerIn: Overlay.overlay
    width: 560
    height: 420
    padding: 20
    background: Rectangle { radius: 14; color: Theme.popup; border.color: Theme.border }

    contentItem: ColumnLayout {
        spacing: 12
        RowLayout {
            Text { Layout.fillWidth: true; text: "Outbox"; font.pixelSize: 18; font.weight: Font.Bold; color: Theme.text }
            ToolButtonX { text: "Done"; onClicked: dlg.close() }
        }
        ListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: dlg.items
            spacing: 4
            delegate: Rectangle {
                required property var modelData
                width: ListView.view.width
                height: 52
                radius: 8
                color: Theme.card
                RowLayout {
                    anchors.fill: parent
                    anchors.margins: 10
                    Icon { name: "clock"; width: 18; height: 18; color: Theme.secondary }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 1
                        Text { text: modelData.subject || "(No Subject)"; color: Theme.text; font.weight: Font.DemiBold; elide: Text.ElideRight; Layout.fillWidth: true }
                        Text { text: "To " + modelData.to + " · " + Theme.fmtLongDate(modelData.sendAt); color: Theme.secondary; font.pixelSize: 12; elide: Text.ElideRight; Layout.fillWidth: true }
                    }
                    ToolButtonX { text: "Edit"; onClicked: { var p = store.cancelOutbox(modelData.id); dlg.close(); dlg.editRequested(JSON.parse(p)) } }
                    ToolButtonX { icon: "trash"; tip: "Don't send"; onClicked: store.cancelOutbox(modelData.id) }
                }
            }
            Text { anchors.centerIn: parent; visible: dlg.items.length === 0; text: "Nothing waiting to be sent"; color: Theme.tertiary }
        }
    }
}
