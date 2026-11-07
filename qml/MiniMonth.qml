import QtQuick

// Small month grid used in the calendar sidebar and the year view.
Item {
    id: mm
    property date month: new Date()
    property date selected: new Date()
    property bool showTitle: true
    property bool compact: false
    property var busyDays: ({})
    signal picked(date day)
    signal titleClicked()

    readonly property int cell: compact ? 22 : 26
    implicitWidth: cell * 7
    implicitHeight: (showTitle ? 26 : 0) + 18 + cell * 6

    readonly property date gridStart: {
        var first = new Date(month.getFullYear(), month.getMonth(), 1)
        return Theme.startOfWeek(first)
    }

    Text {
        id: title
        visible: mm.showTitle
        height: visible ? 26 : 0
        text: Theme.monthName(mm.month.getMonth()) + (mm.compact ? "" : " " + mm.month.getFullYear())
        font.pixelSize: mm.compact ? Theme.base + 1 : Theme.base
        font.weight: Font.DemiBold
        color: mm.compact && mm.month.getMonth() === new Date().getMonth() && mm.month.getFullYear() === new Date().getFullYear() ? Theme.red : Theme.text
        leftPadding: 4
        MouseArea { anchors.fill: parent; onClicked: mm.titleClicked() }
    }
    Row {
        id: wd
        y: title.height
        Repeater {
            model: 7
            delegate: Text {
                required property int index
                width: mm.cell
                horizontalAlignment: Text.AlignHCenter
                text: Theme.dayName((Theme.firstDay() + index) % 7, Locale.NarrowFormat)
                font.pixelSize: 10
                color: Theme.tertiary
            }
        }
    }
    Grid {
        y: title.height + 18
        columns: 7
        Repeater {
            model: 42
            delegate: Item {
                required property int index
                property date day: Theme.addDays(mm.gridStart, index)
                property bool inMonth: day.getMonth() === mm.month.getMonth()
                property bool isToday: Theme.sameDay(day, new Date())
                property bool isSel: Theme.sameDay(day, mm.selected)
                width: mm.cell
                height: mm.cell
                Rectangle {
                    anchors.centerIn: parent
                    width: mm.cell - 4; height: width; radius: width / 2
                    color: parent.isToday ? Theme.red : (parent.isSel && !mm.compact ? Theme.sidebarSelected : "transparent")
                }
                Text {
                    anchors.centerIn: parent
                    text: parent.day.getDate()
                    font.pixelSize: mm.compact ? 10 : 11
                    font.weight: parent.isToday ? Font.Bold : Font.Normal
                    color: parent.isToday ? "white" : (parent.inMonth ? Theme.text : Theme.tertiary)
                    visible: !mm.compact || parent.inMonth
                }
                MouseArea { anchors.fill: parent; onClicked: mm.picked(parent.day) }
            }
        }
    }
}
