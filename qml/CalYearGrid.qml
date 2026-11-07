import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// Twelve small months (year view).
Flickable {
    property var cal
    contentHeight: yg.implicitHeight + 20
    clip: true
    GridLayout {
        id: yg
        x: 20
        width: parent.width - 40
        columns: Math.max(2, Math.floor((parent.width - 40) / 200))
        rowSpacing: 18
        columnSpacing: 20
        Repeater {
            model: 12
            delegate: MiniMonth {
                required property int index
                compact: true
                month: new Date(cal.cursor.getFullYear(), index, 1)
                selected: cal.cursor
                onPicked: function(d) { cal.cursor = d; cal.view = "day" }
                onTitleClicked: { cal.cursor = month; cal.view = "month" }
            }
        }
    }
}
