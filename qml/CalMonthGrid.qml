import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// Six weeks of days with their events (month view).
Item {
    property var cal
    id: mg
    readonly property date start: Theme.startOfWeek(new Date(cal.cursor.getFullYear(), cal.cursor.getMonth(), 1))
    readonly property real cw: width / 7
    readonly property real ch: (height - 24) / 6

    // What follows the pointer while an event is dragged to another day.
    Rectangle {
        id: ghost
        property string text: ""
        visible: false
        z: 100
        width: Math.min(mg.cw - 8, 160)
        height: 20
        radius: 4
        opacity: 0.9
        Text { anchors.fill: parent; anchors.leftMargin: 6; verticalAlignment: Text.AlignVCenter; text: ghost.text; color: "white"; font.pixelSize: 11; font.weight: Font.DemiBold; elide: Text.ElideRight }
    }
    Row {
        id: wdays
        height: 24
        Repeater {
            model: 7
            delegate: Text {
                required property int index
                width: mg.cw
                horizontalAlignment: Text.AlignRight
                rightPadding: 10
                text: Theme.dayName((Theme.firstDay() + index) % 7)
                color: Theme.secondary; font.pixelSize: 12
            }
        }
    }
    Rectangle { y: 24; width: parent.width; height: 1; color: Theme.border }
    // For working out how many lines an event's name takes.
    FontMetrics { id: nameMetrics; font.pixelSize: 11 }
    FontMetrics { id: timeMetrics; font.pixelSize: 10 }
    Repeater {
        model: 42
        delegate: Rectangle {
            id: cell
            required property int index
            property date day: Theme.addDays(mg.start, index)
            property bool inMonth: day.getMonth() === cal.cursor.getMonth()
            property bool isToday: Theme.sameDay(day, cal.today)
            property var evs: cal.dayEvents(day)
            // Each event's name on as many lines as it needs: how many lines one
            // takes, and so how many events fit (room left for "N more…").
            function linesOf(e) {
                // (Only the first line stops short of the time; the rest are full width.)
                var w = width - 16 - (e.allDay ? 0 : 11)
                var time = e.allDay ? 0 : timeMetrics.advanceWidth(Theme.fmtTime(Theme.parseDT(e.start))) + 4
                return Math.max(1, Math.ceil((nameMetrics.advanceWidth(e.title) * 1.08 + time) / Math.max(20, w)))
            }
            property int fit: {
                var room = height - 30, n = 0
                for (var i = 0; i < evs.length; i++) {
                    var h = linesOf(evs[i]) * 14 + 4
                    // (One alone too long for the day still shows, cut to fit.)
                    if (room - h < (i < evs.length - 1 ? 16 : 0) && n > 0) break
                    room -= h
                    n++
                }
                return n
            }
            x: (index % 7) * mg.cw
            y: 25 + Math.floor(index / 7) * mg.ch
            width: mg.cw
            height: mg.ch
            color: inMonth ? Theme.window : Theme.dim
            clip: true
            border.color: Theme.borderSoft
            border.width: 0.5
            Rectangle {
                anchors.right: parent.right; anchors.rightMargin: 5
                y: 4
                width: Math.max(24, dayNum.implicitWidth + 10); height: 22; radius: 11
                color: cell.isToday ? Theme.red : "transparent"
                Text {
                    id: dayNum
                    anchors.centerIn: parent
                    text: (cell.day.getDate() === 1 ? Theme.monthName(cell.day.getMonth()).substr(0, 3) + " " : "") + cell.day.getDate()
                    color: cell.isToday ? "white" : (cell.inMonth ? Theme.text : Theme.tertiary)
                    font.pixelSize: 13
                    font.weight: cell.isToday ? Font.Bold : Font.Normal
                }
            }
            MouseArea {
                anchors.fill: parent
                onClicked: cal.cursor = cell.day
                onDoubleClicked: { var d = new Date(cell.day); d.setHours(9); cal.newEvent(d, false) }
            }
            Column {
                x: 4; y: 28
                width: parent.width - 8
                spacing: 2
                Repeater {
                    model: cell.evs.length > cell.fit ? cell.evs.slice(0, cell.fit) : cell.evs
                    delegate: Rectangle {
                        required property var modelData
                        width: parent.width
                        height: Math.max(16, nameText.implicitHeight + 2)
                        radius: 4
                        color: modelData.allDay ? Theme.eventFill(Qt.color(modelData.color), 0.25) : "transparent"
                        Row {
                            x: 4
                            y: 1
                            spacing: 4
                            width: parent.width - 8
                            Rectangle {
                                visible: !modelData.allDay
                                width: 7; height: 7; radius: 3.5
                                color: modelData.color
                                y: 4
                            }
                            Text {
                                id: nameText
                                // The whole name, on more lines when it's long: the first line
                                // up to the time at the right, the others the full width.
                                width: parent.width - 12
                                text: modelData.title
                                wrapMode: Text.Wrap
                                // (Shortened only when even the whole day can't hold it.)
                                maximumLineCount: Math.max(1, Math.floor((cell.height - 30) / 14))
                                elide: Text.ElideRight
                                onLineLaidOut: function(line) {
                                    if (line.number === 0 && timeText.visible) line.width = width - timeText.implicitWidth - 4
                                }
                                font.pixelSize: 11
                                font.weight: modelData.allDay ? Font.DemiBold : Font.Normal
                                color: modelData.allDay ? Theme.eventText(Qt.color(modelData.color)) : Theme.text
                            }
                        }
                        Text {
                            id: timeText
                            visible: !modelData.allDay
                            anchors.right: parent.right; anchors.rightMargin: 3
                            y: 2
                            text: Theme.fmtTime(Theme.parseDT(modelData.start))
                            font.pixelSize: 10; color: Theme.tertiary
                        }
                        ToolTip.visible: monthArea.containsMouse && !monthArea.pressed
                        ToolTip.delay: 500
                        ToolTip.text: Theme.eventTip(modelData)
                        // Drag onto another day to move it there.
                        MouseArea {
                            id: monthArea
                            anchors.fill: parent
                            hoverEnabled: true
                            preventStealing: true
                            cursorShape: moved ? Qt.ClosedHandCursor : Qt.PointingHandCursor
                            property point start
                            property bool moved: false
                            onPressed: function(ev) { start = mapToItem(mg, ev.x, ev.y); moved = false }
                            onPositionChanged: function(ev) {
                                if (!pressed) return   // hovering only (tooltip), not dragging
                                if (modelData.virtual || modelData.readOnly) return
                                var p = mapToItem(mg, ev.x, ev.y)
                                if (Math.abs(p.x - start.x) + Math.abs(p.y - start.y) > 6) moved = true
                                if (moved) {
                                    ghost.text = modelData.title
                                    ghost.color = modelData.color
                                    ghost.x = p.x - ghost.width / 2
                                    ghost.y = p.y - ghost.height / 2
                                    ghost.visible = true
                                }
                            }
                            onReleased: function(ev) {
                                if (!moved) return
                                var p = mapToItem(mg, ev.x, ev.y)
                                var a = start
                                ghost.visible = false
                                moved = false
                                var dayFrom = Math.floor(a.x / mg.cw) + 7 * Math.floor((a.y - 25) / mg.ch)
                                var dayTo = Math.floor(p.x / mg.cw) + 7 * Math.floor((p.y - 25) / mg.ch)
                                var d = dayTo - dayFrom
                                if (d === 0) return
                                var s = Theme.parseDT(modelData.start), e = Theme.parseDT(modelData.end)
                                cal.moveEvent(modelData, Theme.addDays(s, d), Theme.addDays(e, d))
                            }
                            onClicked: if (!moved) cal.edit(modelData)
                        }
                    }
                }
                Text {
                    visible: cell.evs.length > cell.fit
                    text: (cell.evs.length - cell.fit) + " more…"
                    font.pixelSize: 11; color: Theme.secondary; leftPadding: 4
                    MouseArea { anchors.fill: parent; onClicked: { cal.cursor = cell.day; cal.view = "day" } }
                }
            }
        }
    }
}
