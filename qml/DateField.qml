import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// A date shown the way the system writes dates ("Mon, Nov 2, 2026");
// click it to pick from a small calendar. `text` is the date as
// YYYY-MM-DD.
Rectangle {
    id: df
    property string text: Theme.isoDate(new Date())
    readonly property date value: Theme.parseDT(text) || new Date()
    signal edited()
    implicitWidth: 150
    implicitHeight: 30
    radius: 7
    color: area.containsMouse ? Theme.hover : Theme.field
    border.color: picker.opened ? Theme.accent : Theme.border
    border.width: picker.opened ? 2 : 1

    Text {
        anchors.verticalCenter: parent.verticalCenter
        x: 10
        width: parent.width - 34
        text: df.value.toLocaleDateString(Qt.locale(), "ddd, ") + df.value.toLocaleDateString(Qt.locale(), Locale.ShortFormat)
        color: Theme.text
        font.pixelSize: Theme.base
        elide: Text.ElideRight
    }
    Icon { anchors.verticalCenter: parent.verticalCenter; anchors.right: parent.right; anchors.rightMargin: 9; name: "calendar"; width: 14; height: 14; color: Theme.secondary }
    MouseArea { id: area; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: { cal.month = df.value; picker.open() } }

    Popup {
        id: picker
        y: df.height + 4
        padding: 10
        background: Rectangle { radius: 10; color: Theme.popup; border.color: Theme.border }
        ColumnLayout {
            spacing: 6
            RowLayout {
                Layout.fillWidth: true
                Text {
                    Layout.fillWidth: true
                    text: Theme.monthName(cal.month.getMonth()) + " " + cal.month.getFullYear()
                    font.pixelSize: Theme.base; font.weight: Font.DemiBold; color: Theme.text
                }
                ToolButtonX { icon: "chevronLeft"; tip: "Previous month"; onClicked: cal.month = new Date(cal.month.getFullYear(), cal.month.getMonth() - 1, 1) }
                ToolButtonX { text: "Today"; onClicked: { df.text = Theme.isoDate(new Date()); df.edited(); picker.close() } }
                ToolButtonX { icon: "chevronRight"; tip: "Next month"; onClicked: cal.month = new Date(cal.month.getFullYear(), cal.month.getMonth() + 1, 1) }
            }
            MiniMonth {
                id: cal
                showTitle: false
                selected: df.value
                onPicked: function(day) { df.text = Theme.isoDate(day); df.edited(); picker.close() }
            }
        }
    }
}
