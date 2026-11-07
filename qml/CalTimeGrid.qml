import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// Hours down the side, one column per day (day and week views).
Item {
    property var cal
    id: tg
    property int days: 7
    // Pinch (or Ctrl + scroll) to make the hours taller or shorter, as in
    // Apple Calendar.
    property real hourH: 48
    readonly property real gutter: 58
    readonly property date start: days === 1 ? Theme.startOfDay(cal.cursor) : Theme.startOfWeek(cal.cursor)
    readonly property real colW: (width - gutter - 12) / days
    clip: true

    // Day headings
    Row {
        id: heads
        x: tg.gutter
        height: 30
        Repeater {
            model: tg.days
            delegate: Item {
                required property int index
                property date day: Theme.addDays(tg.start, index)
                property bool isToday: Theme.sameDay(day, cal.today)
                width: tg.colW
                height: 30
                Row {
                    anchors.centerIn: parent
                    spacing: 5
                    Text { text: Theme.dayName(parent.parent.day.getDay()); color: Theme.secondary; font.pixelSize: Theme.base; anchors.verticalCenter: parent.verticalCenter }
                    Rectangle {
                        width: 26; height: 26; radius: 13
                        color: parent.parent.isToday ? Theme.red : "transparent"
                        Text {
                            anchors.centerIn: parent
                            text: parent.parent.parent.day.getDate()
                            font.pixelSize: 15
                            font.weight: parent.parent.parent.isToday ? Font.Bold : Font.Normal
                            color: parent.parent.parent.isToday ? "white" : Theme.text
                        }
                    }
                }
                MouseArea { anchors.fill: parent; onClicked: { cal.cursor = parent.day; cal.view = "day" } }
            }
        }
    }

    // All-day row
    Item {
        id: allDayRow
        y: heads.height
        width: parent.width
        property int maxCount: {
            var m = 0
            for (var i = 0; i < tg.days; i++) {
                var n = cal.dayEvents(Theme.addDays(tg.start, i)).filter(function(e) { return e.allDay }).length
                if (n > m) m = n
            }
            return Math.min(m, 4)
        }
        height: Math.max(1, maxCount) * 22 + 8
        Text { x: 6; y: 6; text: "all-day"; color: Theme.tertiary; font.pixelSize: 11 }
        Repeater {
            model: tg.days
            delegate: Column {
                required property int index
                x: tg.gutter + index * tg.colW + 2
                y: 4
                width: tg.colW - 4
                spacing: 2
                Repeater {
                    model: cal.dayEvents(Theme.addDays(tg.start, index)).filter(function(e) { return e.allDay }).slice(0, 4)
                    delegate: Rectangle {
                        required property var modelData
                        width: parent.width
                        height: 20
                        radius: 4
                        color: Theme.eventFill(Qt.color(modelData.color), 0.22)
                        Rectangle { width: 3; height: parent.height; radius: 1.5; color: modelData.color }
                        Text {
                            x: 7; anchors.verticalCenter: parent.verticalCenter
                            width: parent.width - 10
                            text: modelData.title; elide: Text.ElideRight
                            font.pixelSize: 12; font.weight: Font.DemiBold
                            color: Theme.eventText(Qt.color(modelData.color))
                        }
                        MouseArea { id: allDayArea; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: cal.edit(modelData) }
                        ToolTip.visible: allDayArea.containsMouse
                        ToolTip.delay: 500
                        ToolTip.text: Theme.eventTip(modelData)
                    }
                }
            }
        }
        MouseArea {
            anchors.fill: parent
            anchors.leftMargin: tg.gutter
            z: -1
            onDoubleClicked: function(ev) { cal.newEvent(Theme.addDays(tg.start, Math.floor(ev.x / tg.colW)), true) }
        }
        Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border }
    }

    Flickable {
        // Trackpad scrolling glides on (momentum).
        Momentum { flick: flick }
        id: flick
        anchors.top: allDayRow.bottom
        anchors.bottom: parent.bottom
        width: parent.width
        contentHeight: tg.hourH * 24 + 16
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar {}
        Component.onCompleted: contentY = tg.hourH * 7.5
        function zoomTo(h, focusY) {
            h = Math.max(24, Math.min(160, h))
            var at = (contentY + focusY) / tg.hourH
            tg.hourH = h
            contentY = Math.max(0, Math.min(contentHeight - height, at * h - focusY))
        }
        PinchHandler {
            target: null
            property real startH: 48
            onActiveChanged: if (active) startH = tg.hourH
            onActiveScaleChanged: if (active) flick.zoomTo(startH * activeScale, centroid.position.y)
        }
        WheelHandler {
            target: null
            acceptedModifiers: Qt.ControlModifier
            onWheel: function(ev) { flick.zoomTo(tg.hourH * (ev.angleDelta.y > 0 ? 1.1 : 1 / 1.1), ev.y) }
        }
        Item {
            id: grid
            y: 8
            width: flick.width
            height: tg.hourH * 24

            Repeater {
                model: 24
                delegate: Item {
                    required property int index
                    y: index * tg.hourH
                    width: grid.width
                    Text {
                        visible: index > 0 && Math.abs(cal.today.getHours() + cal.today.getMinutes() / 60 - index) > 0.3
                        x: tg.gutter - width - 8; y: -7
                        text: Theme.fmtHour(index)
                        color: Theme.tertiary; font.pixelSize: 11
                    }
                    Rectangle { x: tg.gutter; width: grid.width - tg.gutter; height: 1; color: Theme.borderSoft }
                }
            }
            Repeater {
                model: tg.days
                delegate: Rectangle {
                    required property int index
                    x: tg.gutter + index * tg.colW
                    width: 1; height: grid.height
                    color: Theme.borderSoft
                }
            }
            MouseArea {
                x: tg.gutter
                width: grid.width - tg.gutter
                height: grid.height
                onDoubleClicked: function(ev) {
                    var d = Theme.addDays(tg.start, Math.floor(ev.x / tg.colW))
                    var mins = Math.floor(ev.y / tg.hourH * 2) * 30
                    d.setHours(Math.floor(mins / 60), mins % 60, 0, 0)
                    cal.newEvent(d, false)
                }
            }

            // Events of each day, side by side where they overlap.
            Repeater {
                model: tg.days
                delegate: Item {
                    id: dayCol
                    required property int index
                    property date day: Theme.addDays(tg.start, index)
                    x: tg.gutter + index * tg.colW
                    width: tg.colW
                    height: grid.height
                    property var placed: {
                        var list = cal.dayEvents(day).filter(function(e) { return !e.allDay })
                        var dayStart = day.getTime(), dayEnd = Theme.addDays(day, 1).getTime()
                        var items = list.map(function(e) {
                            var s = Math.max(Theme.parseDT(e.start).getTime(), dayStart)
                            var en = Math.min(Math.max(Theme.parseDT(e.end).getTime(), s + 15 * 60000), dayEnd)
                            return { ev: e, s: s, e: en, col: 0, cols: 1 }
                        })
                        items.sort(function(a, b) { return a.s - b.s || b.e - a.e })
                        var cluster = [], colsEnd = [], clusterEnd = 0
                        function finish() { for (var k = 0; k < cluster.length; k++) cluster[k].cols = colsEnd.length; cluster = []; colsEnd = [] }
                        for (var i = 0; i < items.length; i++) {
                            var it = items[i]
                            if (cluster.length && it.s >= clusterEnd) finish()
                            var c = 0
                            while (c < colsEnd.length && colsEnd[c] > it.s) c++
                            colsEnd[c] = it.e
                            it.col = c
                            cluster.push(it)
                            clusterEnd = Math.max(clusterEnd, it.e)
                        }
                        finish()
                        return items.map(function(it) {
                            return { ev: it.ev, top: (it.s - dayStart) / 3600000 * tg.hourH, h: (it.e - it.s) / 3600000 * tg.hourH, col: it.col, cols: it.cols }
                        })
                    }
                    Repeater {
                        model: dayCol.placed
                        delegate: Rectangle {
                            id: block
                            required property var modelData
                            property color c: Qt.color(modelData.ev.color)
                            // Dragging: how far it has been moved / stretched (pixels).
                            property real dx: 0
                            property real dy: 0
                            property real dh: 0
                            x: 2 + modelData.col * (dayCol.width - 4) / modelData.cols + dx
                            y: modelData.top + 1 + dy
                            z: (dx !== 0 || dy !== 0 || dh !== 0) ? 10 : 1
                            opacity: z === 10 ? 0.85 : 1
                            width: (dayCol.width - 4) / modelData.cols - 2
                            height: Math.max(modelData.h - 2 + dh, 18)
                            // Short events: one line, title and time side by side (as in Apple Calendar).
                            readonly property bool compact: height < 34
                            function snapMinutes(px) { return Math.round(px / tg.hourH * 4) * 15 }
                            function finish() {
                                var days = Math.round(dx / tg.colW)
                                var mins = snapMinutes(dy)
                                var grow = snapMinutes(dh)
                                dx = 0; dy = 0; dh = 0
                                if (days === 0 && mins === 0 && grow === 0) return
                                var s = Theme.parseDT(modelData.ev.start), e = Theme.parseDT(modelData.ev.end)
                                var shift = days * 86400000 + mins * 60000
                                var ns = new Date(s.getTime() + shift)
                                var ne = new Date(e.getTime() + shift + grow * 60000)
                                if (ne <= ns) ne = new Date(ns.getTime() + 15 * 60000)
                                cal.moveEvent(modelData.ev, ns, ne)
                            }
                            radius: 5
                            color: Theme.eventFill(c, 0.18)
                            border.color: Theme.eventFill(c, 0.35)
                            clip: true
                            Rectangle { width: 3; height: parent.height; color: parent.c; radius: 1.5 }
                            Column {
                                x: 7
                                y: block.compact ? Math.max(0, (block.height - implicitHeight) / 2) : 3
                                width: parent.width - 10
                                Text {
                                    width: parent.width
                                    text: block.compact ? modelData.ev.title + "  <font color='" + Theme.eventText(parent.parent.c) + "' style='font-weight:normal'>" + Theme.fmtTime(Theme.parseDT(modelData.ev.start)) + "</font>" : modelData.ev.title
                                    textFormat: block.compact ? Text.StyledText : Text.PlainText
                                    font.pixelSize: block.compact ? 11 : 12; font.weight: Font.DemiBold
                                    color: Theme.eventText(parent.parent.c)
                                    elide: Text.ElideRight
                                    wrapMode: Text.WordWrap
                                    maximumLineCount: block.compact ? 1 : Math.max(1, Math.floor((parent.parent.height - 20) / 15))
                                }
                                Text {
                                    visible: !block.compact
                                    width: parent.width
                                    text: Theme.fmtTime(Theme.parseDT(modelData.ev.start)) + (modelData.ev.location ? " · " + modelData.ev.location : "")
                                    font.pixelSize: 11
                                    color: Theme.eventText(parent.parent.c)
                                    elide: Text.ElideRight
                                }
                            }
                            ToolTip.visible: blockArea.containsMouse && !blockArea.moved && !blockArea.pressed
                            ToolTip.delay: 500
                            ToolTip.text: Theme.eventTip(modelData.ev)
                            MouseArea {
                                id: blockArea
                                anchors.fill: parent
                                anchors.bottomMargin: 6
                                hoverEnabled: true
                                property point start
                                property bool moved: false
                                preventStealing: true
                                cursorShape: moved ? Qt.ClosedHandCursor : Qt.PointingHandCursor
                                onPressed: function(ev) { start = mapToItem(grid, ev.x, ev.y); moved = false; block.dx = 0; block.dy = 0 }
                                onPositionChanged: function(ev) {
                                    if (!pressed) return   // hovering only (tooltip), not dragging
                                    if (modelData.ev.virtual || modelData.ev.readOnly) return
                                    var p = mapToItem(grid, ev.x, ev.y)
                                    if (!moved && Math.abs(p.x - start.x) + Math.abs(p.y - start.y) < 5) return
                                    moved = true
                                    block.dx = Math.round((p.x - start.x) / tg.colW) * tg.colW
                                    block.dy = Math.round((p.y - start.y) / (tg.hourH / 4)) * (tg.hourH / 4)
                                }
                                onReleased: if (moved) { moved = false; block.finish() }
                                onClicked: if (!moved) cal.edit(modelData.ev)
                            }
                            // Bottom edge: drag to change how long it lasts.
                            MouseArea {
                                visible: !modelData.ev.virtual && !modelData.ev.readOnly
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.bottom: parent.bottom
                                height: 6
                                cursorShape: Qt.SizeVerCursor
                                property real startY
                                preventStealing: true
                                onPressed: function(ev) { startY = mapToItem(grid, ev.x, ev.y).y }
                                onPositionChanged: function(ev) {
                                    if (!pressed) return   // hovering only (tooltip), not dragging
                                    var y = mapToItem(grid, ev.x, ev.y).y
                                    block.dh = Math.round((y - startY) / (tg.hourH / 4)) * (tg.hourH / 4)
                                }
                                onReleased: block.finish()
                            }
                        }
                    }
                }
            }

            // Now line
            Item {
                id: nowLine
                property int todayIndex: Math.floor((Theme.startOfDay(cal.today) - tg.start) / 86400000 + 0.01)
                visible: todayIndex >= 0 && todayIndex < tg.days
                y: (cal.today.getHours() + cal.today.getMinutes() / 60) * tg.hourH
                x: tg.gutter
                width: grid.width - tg.gutter
                Rectangle { x: nowLine.todayIndex * tg.colW - 4; y: -4; width: 9; height: 9; radius: 4.5; color: Theme.red }
                Rectangle { x: nowLine.todayIndex * tg.colW; width: tg.colW; height: 2; y: -1; color: Theme.red }
                Text {
                    x: -width - 4 - 4; y: -7
                    text: Theme.fmtTime(cal.today)
                    color: Theme.red; font.pixelSize: 11; font.weight: Font.DemiBold
                }
            }
        }
    }
}
