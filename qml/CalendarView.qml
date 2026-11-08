import QtQuick
import QtQuick.Controls.Basic
// The system's own menus (desktop style on Linux, native on macOS and Windows)
import Qt.labs.platform as P
import QtQuick.Layouts

// Calendar: calendars list and a small month on the left; day, week, month
// or year view on the right.
Item {
    id: page
    property var store
    property bool sidebarShown: true
    property string search: ""
    property bool searchActive: false
    property var searchResults: { store.revision; return search.trim() ? JSON.parse(store.searchEvents(search)) : [] }
    signal focusSearch()
    property string view: "week"
    property var prefs: { store.revision; return JSON.parse(store.settings()) }
    property var liveNames: { store.revision; var m = {}; JSON.parse(store.liveAccounts()).forEach(function(a) { m["live:" + a.id] = a.name }); return m }
    Component.onCompleted: view = prefs.calendar_view || "week"
    // What's on screen, saved when Almanac closes and put back when it opens.
    property real keptSideW: 0
    // (Noted as it changes, so a hidden view or window still saves the right one.)
    signal panesChanged()
    Connections {
        target: sidebarPane
        function onWidthChanged() { if (page.visible && page.width > 0 && page.sidebarShown && sidebarPane.width > 60) { page.keptSideW = sidebarPane.width; page.panesChanged() } }
    }
    function saveState() { return { sidebar: sidebarShown, view: view, cursor: cursor.getTime(), sideW: keptSideW } }
    function restoreState(s) {
        if (!s) return
        keptSideW = s.sideW || 0
        sidebarShown = s.sidebar !== false
        if (s.view) view = s.view
        if (s.cursor) cursor = new Date(s.cursor)
        if (s.sideW > 60) sidebarPane.SplitView.preferredWidth = Math.min(s.sideW, ((page.Window.window ? page.Window.window.width : 0) || 1280) * 0.4)
    }
    function newEventNow() { var n = new Date(cursor); var now = new Date(); n.setHours(now.getHours() + 1, 0, 0, 0); newEvent(n, false) }
    function newCalendar() { calEditor.openFor(null) }
    function newSubscription() { subDialog.open() }
    property date cursor: new Date()
    property date today: new Date()

    property var calendars: { store.revision; return JSON.parse(store.calendars()) }

    readonly property date rangeStart: {
        if (view === "day") return Theme.startOfDay(cursor)
        if (view === "week") return Theme.startOfWeek(cursor)
        if (view === "month") return Theme.startOfWeek(new Date(cursor.getFullYear(), cursor.getMonth(), 1))
        return new Date(cursor.getFullYear(), 0, 1)
    }
    readonly property date rangeEnd: {
        if (view === "day") return Theme.addDays(rangeStart, 1)
        if (view === "week") return Theme.addDays(rangeStart, 7)
        if (view === "month") return Theme.addDays(rangeStart, 42)
        return new Date(cursor.getFullYear() + 1, 0, 1)
    }
    property var events: { store.revision; return JSON.parse(store.events(Theme.isoDT(rangeStart), Theme.isoDT(rangeEnd))) }

    // date string -> events touching that day
    property var byDay: {
        var map = {}
        for (var i = 0; i < events.length; i++) {
            var e = events[i]
            var s = Theme.startOfDay(Theme.parseDT(e.start))
            var en = Theme.parseDT(e.end)
            var d = s
            var guard = 0
            do {
                var k = Theme.isoDate(d)
                if (!map[k]) map[k] = []
                map[k].push(e)
                d = Theme.addDays(d, 1)
                guard++
            } while (d < en && guard < 400)
        }
        return map
    }

    function dayEvents(d) { return byDay[Theme.isoDate(d)] || [] }
    function shift(n) {
        var c = cursor
        if (view === "day") cursor = Theme.addDays(c, n)
        else if (view === "week") cursor = Theme.addDays(c, 7 * n)
        else if (view === "month") cursor = new Date(c.getFullYear(), c.getMonth() + n, 1)
        else cursor = new Date(c.getFullYear() + n, c.getMonth(), 1)
    }
    // A click on an event: its details (to read, with working links), and
    // from there Edit.
    function edit(e) { details.show(e) }
    // An event by its id and the start of the occurrence (from a notification):
    // its day shown, and its details.
    function showEvent(id, start) {
        cursor = Theme.parseDT(start)
        if (view === "year" || view === "month") view = "day"
        Qt.callLater(function() {
            for (var i = 0; i < events.length; i++) if (events[i].id === id && events[i].start === start) { edit(events[i]); return }
            for (var j = 0; j < events.length; j++) if (events[j].id === id) { edit(events[j]); return }
        })
    }
    function editEvent(e) {
        if (e.virtual || e.readOnly) { details.show(e); return }
        var copy = JSON.parse(JSON.stringify(e))
        // The editor works in the event's own zone and, for repeats, on the whole series.
        if (e.repeats && e.occurrence) {
            var occ = Theme.parseDT(e.occurrence)
            var dur = Theme.parseDT(e.seriesEnd) - Theme.parseDT(e.seriesStart)
            copy.start = e.occurrence
            copy.end = Theme.isoDT(new Date(occ.getTime() + dur))
        } else {
            copy.start = e.seriesStart; copy.end = e.seriesEnd
        }
        editor.openEvent(copy, false)
    }
    readonly property alias eventEditor: editor
    function editFirst() {
        for (var i = 0; i < events.length; i++) if (!events[i].allDay && !events[i].virtual) { editEvent(events[i]); return }
    }
    // Drag to move: repeating events ask whether to move one or all.
    property var pendingMove: null
    function moveEvent(e, ns, ne) {
        pendingMove = { e: e, s: Theme.isoDT(ns), en: Theme.isoDT(ne) }
        if (e.repeats && !e.override) moveScope.open()
        else applyMove("all")
    }
    function applyMove(scope) {
        var m = pendingMove
        if (!m) return
        store.moveEvent(m.e.id, m.e.occurrence || "", m.s, m.en, scope)
        pendingMove = null
    }
    P.Menu {
        id: moveScope
        P.MenuItem { icon.name: "transform-move"; text: "Move This Event Only"; onTriggered: page.applyMove("this") }
        P.MenuItem { icon.name: "transform-move"; text: "Move All Events"; onTriggered: page.applyMove("all") }
        onAboutToHide: Qt.callLater(function() { page.pendingMove = null })
    }
    function printView() {
        var esc = function(t) { return (t || "").replace(/&/g, "&amp;").replace(/</g, "&lt;") }
        var title = view === "day" ? cursor.toLocaleDateString(Qt.locale(), Locale.LongFormat)
                  : (view === "year" ? "" + cursor.getFullYear() : Theme.monthName(cursor.getMonth()) + " " + cursor.getFullYear())
        var h = "<h2>" + esc(title) + "</h2><table width='100%' cellspacing='0' cellpadding='4' border='0'>"
        var lastDay = ""
        for (var i = 0; i < events.length; i++) {
            var e = events[i]
            var s = Theme.parseDT(e.start)
            var day = s.toLocaleDateString(Qt.locale(), Locale.LongFormat)
            if (day !== lastDay) { h += "<tr><td colspan='3' style='background:#eee'><b>" + esc(day) + "</b></td></tr>"; lastDay = day }
            h += "<tr><td width='18%' style='color:" + e.color + "'>■ " + (e.allDay ? "all-day" : Theme.fmtTime(s)) + "</td><td><b>" + esc(e.title)
               + "</b></td><td style='color:#666'>" + esc(e.location) + "</td></tr>"
        }
        h += "</table>"
        store.printHtml("Calendar " + title, h)
    }
    function newEvent(start, allDay) {
        var writable = calendars.filter(function(c) { return !c.readOnly })
        var vis = writable.filter(function(c) { return c.visible })
        var pref = writable.filter(function(c) { return c.id === prefs.default_calendar })
        var calId = pref.length ? pref[0].id : (vis.length ? vis[0].id : (writable.length ? writable[0].id : ""))
        var s = start, e
        if (allDay) { s = Theme.startOfDay(start); e = Theme.addDays(s, 1) }
        else e = new Date(s.getTime() + (prefs.event_minutes || 60) * 60000)
        var alarms = prefs.default_alert === null || prefs.default_alert === undefined ? [] : [prefs.default_alert]
        editor.openEvent({ id: 0, title: "", start: Theme.isoDT(s), end: Theme.isoDT(e), allDay: allDay, calendar: calId, rrule: "", tz: undefined, alarms: alarms }, true)
    }
    function sourceName(s) {
        if (s === "local") return "On My Computer"
        if (s === "thunderbird") return "Thunderbird"
        if (s === "mailspring") return "Mailspring"
        if (s.indexOf("sub:") === 0) return "Subscriptions"
        return liveNames[s] || s
    }

    EventDetails { id: details; calendars: page.calendars; onEditRequested: function(e) { page.editEvent(e) } }
    EventEditor { id: editor; store: page.store; showZone: !!page.prefs.time_zone_support; calendars: page.calendars.filter(function(c) { return !c.readOnly }) }

    Confirm { id: calConfirm }

    // Subscribe to a calendar published on the web (holidays, sports, school…).
    Popup {
        id: subDialog
        anchors.centerIn: Overlay.overlay
        modal: true
        padding: 20
        width: 520
        background: Rectangle { radius: 12; color: Theme.popup; border.color: Theme.border }
        onOpened: { subUrl.text = ""; subName.text = ""; subUrl.forceActiveFocus() }
        contentItem: ColumnLayout {
            spacing: 10
            Text { text: "New Calendar Subscription"; font.pixelSize: 16; font.weight: Font.Bold; color: Theme.text }
            Text { Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.secondary; font.pixelSize: 12
                   text: "Paste the web address of a calendar (it usually ends in .ics or starts with webcal://). It stays up to date and can't be changed here." }
            FieldX { id: subUrl; Layout.fillWidth: true; placeholderText: "https://example.com/holidays.ics"; selectByMouse: true }
            FieldX { id: subName; Layout.fillWidth: true; placeholderText: "Name, e.g. Public Holidays"; selectByMouse: true }
            RowLayout {
                Item { Layout.fillWidth: true }
                ToolButtonX { text: "Cancel"; onClicked: subDialog.close() }
                ToolButtonX {
                    text: "Subscribe"; primary: true
                    enabled: subUrl.text.trim().indexOf("://") > 0
                    onClicked: { store.addSubscription(subName.text, subUrl.text, page.palette8[page.calendars.length % 8]); subDialog.close() }
                }
            }
        }
    }

    // Birthdays come from Contacts, so they are changed there.
    // Add, rename, recolour or delete a calendar.
    readonly property var palette8: ["#2f7cf6", "#e8453c", "#34a853", "#f5a623", "#9b59d0", "#1abc9c", "#e67e22", "#8e8e93"]
    Popup {
        id: calEditor
        property string calId: ""
        property string color: "#2f7cf6"
        anchors.centerIn: Overlay.overlay
        modal: true
        padding: 20
        background: Rectangle { radius: 12; color: Theme.popup; border.color: Theme.border }
        function openFor(c) {
            calId = c ? c.id : ""
            calName.text = c ? c.name : ""
            color = c ? c.color : page.palette8[page.calendars.length % 8]
            open()
            calName.forceActiveFocus()
        }
        contentItem: ColumnLayout {
            spacing: 12
            Text { text: calEditor.calId ? "Calendar" : "New Calendar"; font.pixelSize: 16; font.weight: Font.Bold; color: Theme.text }
            FieldX { id: calName; Layout.preferredWidth: 260; placeholderText: "Name"; onAccepted: calSave.clicked() }
            Row {
                spacing: 8
                Repeater {
                    model: page.palette8
                    delegate: Rectangle {
                        required property string modelData
                        width: 24; height: 24; radius: 12
                        color: modelData
                        border.width: calEditor.color === modelData ? 3 : 0
                        border.color: Theme.text
                        MouseArea { anchors.fill: parent; onClicked: calEditor.color = modelData }
                    }
                }
            }
            RowLayout {
                ToolButtonX {
                    visible: calEditor.calId !== "" && calEditor.calId !== "birthdays"
                    icon: "trash"; text: "Delete"
                    onClicked: {
                        var id = calEditor.calId
                        calEditor.close()
                        calConfirm.ask("Delete this calendar?", "Its events are removed from Almanac. A calendar that lives on a server can come back with the next sync; remove it there to be rid of it for good.",
                                       "Delete Calendar", function() { store.deleteCalendar(id) })
                    }
                }
                Item { Layout.fillWidth: true }
                ToolButtonX { text: "Cancel"; onClicked: calEditor.close() }
                ToolButtonX {
                    id: calSave
                    text: calEditor.calId ? "Save" : "Add"; primary: true
                    onClicked: {
                        if (calEditor.calId) store.updateCalendar(calEditor.calId, calName.text, calEditor.color)
                        else store.addCalendar(calName.text, calEditor.color)
                        calEditor.close()
                    }
                }
            }
        }
    }

    // ------------------------------------------------- buttons for the top bar
    property Component tools: Component {
        RowLayout {
            spacing: 6
            ToolButtonX {
                priority: 2; icon: "plus"; tip: "New Event (Ctrl+N)"
                onClicked: { var n = new Date(page.cursor); var now = new Date(); n.setHours(now.getHours() + 1, 0, 0, 0); page.newEvent(n, false) }
            }
            Item { Layout.fillWidth: true }
            Rectangle {
                Layout.preferredHeight: 30
                Layout.preferredWidth: seg.implicitWidth + 4
                readonly property real minW: seg.implicitWidth + 4
                radius: 8
                color: Theme.segment
                Row {
                    id: seg
                    anchors.centerIn: parent
                    Repeater {
                        model: [["day", "Day"], ["week", "Week"], ["month", "Month"], ["year", "Year"]]
                        delegate: Rectangle {
                            required property var modelData
                            width: win.tinyBar ? 32 : 64; height: 26; radius: 6
                            color: page.view === modelData[0] ? Theme.segmentOn : "transparent"
                            Rectangle {
                                z: -1
                                visible: page.view === modelData[0]
                                anchors.fill: parent; anchors.topMargin: 1; anchors.bottomMargin: -1
                                radius: parent.radius
                                color: Theme.segmentShade
                            }
                            Text { anchors.centerIn: parent; text: win.tinyBar ? modelData[1].charAt(0) : modelData[1]; color: Theme.text; font.pixelSize: Theme.base }
                            ToolTip.visible: win.tinyBar && segTip.containsMouse
                            ToolTip.text: modelData[1]
                            MouseArea { id: segTip; anchors.fill: parent; hoverEnabled: true; acceptedButtons: Qt.NoButton }
                            MouseArea { anchors.fill: parent; onClicked: page.view = modelData[0] }
                        }
                    }
                }
            }
            Item { Layout.fillWidth: true }
            ToolButtonX { priority: 3; icon: "chevronLeft"; tip: "Previous (Ctrl+Left)"; onClicked: page.shift(-1) }
            ToolButtonX { priority: 3; text: "Today"; tip: "Ctrl+T"; onClicked: page.cursor = new Date() }
            ToolButtonX { priority: 3; icon: "chevronRight"; tip: "Next (Ctrl+Right)"; onClicked: page.shift(1) }
            Item { width: 6 }
            SearchBox {
                id: calSearch
                Layout.preferredWidth: collapsed ? 30 : 200
                Layout.maximumWidth: collapsed ? 30 : 200
                Layout.fillWidth: true
                Layout.minimumWidth: collapsed ? 30 : 84
                readonly property real minW: 84
                text: page.search
                onTextChanged: page.search = text
                onHasFocusChanged: page.searchActive = hasFocus
                Connections { target: page; function onFocusSearch() { calSearch.focusField() } }
                Popup {
                    y: calSearch.height + 6
                    x: calSearch.width - width
                    width: 340
                    height: Math.max(50, Math.min(page.searchResults.length * 48 + 10, 420))
                    visible: page.searchActive && page.search.trim() !== ""
                    closePolicy: Popup.NoAutoClose
                    padding: 5
                    background: Rectangle { radius: 10; color: Theme.popup; border.color: Theme.border }
                    ListView {
                        id: scroller5
                        // Trackpad scrolling glides on (momentum).
                        Momentum { flick: scroller5 }
                        anchors.fill: parent
                        clip: true
                        model: page.searchResults
                        delegate: Rectangle {
                            required property var modelData
                            width: ListView.view.width
                            height: 48
                            radius: 6
                            color: hit.containsMouse ? Theme.hover : "transparent"
                            Rectangle { x: 8; width: 4; height: 32; radius: 2; anchors.verticalCenter: parent.verticalCenter; color: modelData.color }
                            Column {
                                x: 20
                                anchors.verticalCenter: parent.verticalCenter
                                width: parent.width - 28
                                Text { width: parent.width; text: modelData.title; color: Theme.text; font.weight: Font.DemiBold; elide: Text.ElideRight }
                                Text {
                                    width: parent.width
                                    property date d: Theme.parseDT(modelData.start)
                                    text: d.toLocaleDateString(Qt.locale(), Locale.ShortFormat) + (modelData.allDay ? "" : "  " + Theme.fmtTime(d))
                                          + (modelData.location ? "  ·  " + modelData.location : "")
                                    color: Theme.secondary; font.pixelSize: 12; elide: Text.ElideRight
                                }
                            }
                            MouseArea {
                                id: hit
                                anchors.fill: parent
                                hoverEnabled: true
                                onClicked: { page.cursor = Theme.parseDT(modelData.start); page.view = "day"; page.search = ""; page.edit(modelData) }
                            }
                        }
                    }
                    Text {
                        anchors.centerIn: parent
                        visible: page.searchResults.length === 0
                        text: "No Events Found"
                        color: Theme.tertiary
                    }
                }
            }
        }
    }
    Shortcut { sequence: "Ctrl+F"; enabled: page.visible; onActivated: page.focusSearch() }

    Shortcut { sequence: "Ctrl+T"; enabled: page.visible; onActivated: page.cursor = new Date() }
    Shortcut { sequence: "Ctrl+N"; enabled: page.visible; onActivated: { var n = new Date(); n.setMinutes(0); page.newEvent(Theme.addDays(n, 0), false) } }
    Shortcut { sequences: ["Ctrl+Right"]; enabled: page.visible; onActivated: page.shift(1) }
    Shortcut { sequences: ["Ctrl+Left"]; enabled: page.visible; onActivated: page.shift(-1) }
    // Page Up / Down: the previous / next day, week, month or year; Home: today.
    readonly property bool typing: Window.activeFocusItem !== null && Window.activeFocusItem.cursorPosition !== undefined
    Shortcut { sequence: "PgDown"; enabled: page.visible && !page.typing; onActivated: page.shift(1) }
    Shortcut { sequence: "PgUp"; enabled: page.visible && !page.typing; onActivated: page.shift(-1) }
    Shortcut { sequence: "Home"; enabled: page.visible && !page.typing; onActivated: page.cursor = new Date() }

    Timer { interval: 60000; running: true; repeat: true; onTriggered: page.today = new Date() }

    // Grips over the pane dividers (the lines are too thin to catch).
    PaneGrip { pane: sidebarPane }

    // Panes with handles to drag their widths.
    SplitView {
        anchors.fill: parent
        orientation: Qt.Horizontal
        handle: Rectangle {
            implicitWidth: 1
            color: SplitHandle.pressed || SplitHandle.hovered ? Theme.accent : Theme.border
        }

        // ---------------------------------------------------------- sidebar
        Rectangle {
            id: sidebarPane
            visible: page.sidebarShown
            SplitView.preferredWidth: 220
            SplitView.minimumWidth: 150
            gradient: Theme.sidebarGradient
            Flickable {
                id: scroller6
                // Trackpad scrolling glides on (momentum).
                Momentum { flick: scroller6 }
                anchors.fill: parent
                anchors.margins: 10
                anchors.topMargin: 14
                anchors.bottomMargin: 220 + activity.height
                contentHeight: calCol.implicitHeight
                clip: true
                Column {
                    id: calCol
                    width: parent.width
                    spacing: 1
                    Repeater {
                        model: {
                            var seen = [], out = []
                            for (var i = 0; i < page.calendars.length; i++) {
                                var s = page.calendars[i].source
                                if (seen.indexOf(s) < 0) { seen.push(s); out.push(s) }
                            }
                            return out
                        }
                        delegate: Column {
                            required property var modelData
                            width: calCol.width
                            spacing: 1
                            Text {
                                text: page.sourceName(modelData)
                                color: Theme.tertiary; font.pixelSize: 11; font.weight: Font.DemiBold
                                leftPadding: 8; topPadding: 6; bottomPadding: 4
                            }
                            Repeater {
                                model: page.calendars.filter(function(c) { return c.source === modelData })
                                delegate: Rectangle {
                                    required property var modelData
                                    width: calCol.width
                                    height: 28
                                    radius: 6
                                    color: calHover.containsMouse ? Theme.hover : "transparent"
                                    Rectangle {
                                        id: box
                                        x: 8
                                        anchors.verticalCenter: parent.verticalCenter
                                        width: 15; height: 15; radius: 4
                                        color: modelData.visible ? modelData.color : "transparent"
                                        border.color: modelData.color
                                        border.width: 1.5
                                        Icon { anchors.centerIn: parent; width: 12; height: 12; name: "check"; color: "white"; stroke: 3; visible: modelData.visible }
                                    }
                                    Text {
                                        anchors.left: box.right; anchors.leftMargin: 9
                                        anchors.right: parent.right; anchors.rightMargin: 6
                                        anchors.verticalCenter: parent.verticalCenter
                                        text: modelData.name; elide: Text.ElideRight
                                        color: Theme.text; font.pixelSize: Theme.base
                                        // Cut short: the whole name when pointed at.
                                        HoverHandler { id: calNameHover }
                                        ToolTip.visible: calNameHover.hovered && truncated
                                        ToolTip.delay: 500
                                        ToolTip.text: modelData.name
                                    }
                                    MouseArea {
                                        id: calHover; anchors.fill: parent; hoverEnabled: true
                                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                                        onClicked: function(ev) {
                                            if (ev.button === Qt.RightButton) calEditor.openFor(modelData)
                                            else page.store.setCalendarVisible(modelData.id, !modelData.visible)
                                        }
                                        onDoubleClicked: calEditor.openFor(modelData)
                                    }
                                }
                            }
                        }
                    }
                    Item { width: 1; height: 8 }
                    // One per line, so neither is cut off in a narrow sidebar.
                    Column {
                        ToolButtonX { icon: "plus"; text: "Add Calendar"; onClicked: calEditor.openFor(null) }
                        ToolButtonX { icon: "calendar"; text: "Subscribe to Calendar…"; tip: "Subscribe to a calendar on the web (holidays, sports, school…)"; onClicked: subDialog.open() }
                    }
                }
            }
            Rectangle { anchors.bottom: mini.top; anchors.bottomMargin: 10; x: 10; width: parent.width - 20; height: 1; color: Theme.border }
            // What's going on (syncing calendars…), at the bottom.
            ActivityPanel {
                id: activity
                store: page.store
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
            }
            Column {
                id: mini
                anchors.bottom: activity.top
                anchors.bottomMargin: 14
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 2
                property date month: new Date(page.cursor.getFullYear(), page.cursor.getMonth(), 1)
                RowLayout {
                    width: miniMonth.width
                    Text {
                        Layout.fillWidth: true
                        text: Theme.monthName(mini.month.getMonth()) + " " + mini.month.getFullYear()
                        font.pixelSize: Theme.base; font.weight: Font.DemiBold; color: Theme.text; leftPadding: 4
                    }
                    ToolButtonX { icon: "chevronLeft"; implicitWidth: 22; implicitHeight: 22; onClicked: page.cursor = new Date(mini.month.getFullYear(), mini.month.getMonth() - 1, 1) }
                    ToolButtonX { icon: "chevronRight"; implicitWidth: 22; implicitHeight: 22; onClicked: page.cursor = new Date(mini.month.getFullYear(), mini.month.getMonth() + 1, 1) }
                }
                MiniMonth {
                    id: miniMonth
                    showTitle: false
                    month: mini.month
                    selected: page.cursor
                    onPicked: function(d) { page.cursor = d; if (page.view === "year" || page.view === "month") page.view = "day" }
                }
            }
        }

        ColumnLayout {
            SplitView.fillWidth: true
            spacing: 0

            // ------------------------------------------------------ title
            Item {
                Layout.fillWidth: true
                Layout.preferredHeight: 56
                Row {
                    anchors.left: parent.left
                    anchors.leftMargin: 18
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 8
                    Text {
                        text: page.view === "year" ? "" + page.cursor.getFullYear()
                              : (page.view === "day" ? page.cursor.getDate() + " " + Theme.monthName(page.cursor.getMonth())
                                 : Theme.monthName(page.cursor.getMonth()))
                        font.pixelSize: 26; font.weight: Font.Bold; color: Theme.text
                    }
                    Text {
                        visible: page.view !== "year"
                        text: page.cursor.getFullYear()
                        font.pixelSize: 26; color: Theme.text
                    }
                    Text {
                        visible: page.view === "day"
                        anchors.baseline: parent.children[0].baseline
                        text: "  " + Theme.dayName(page.cursor.getDay(), Locale.LongFormat)
                        font.pixelSize: 16; color: Theme.secondary
                    }
                }
            }

            StackLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                currentIndex: ["day", "week", "month", "year"].indexOf(page.view)

                // Two-finger swipe sideways goes to the next or previous day,
                // week, month or year (as in Apple Calendar).
                property real swipeSum: 0
                property bool swipeDone: false
                Timer { id: swipeRest; interval: 250; onTriggered: { parent.swipeSum = 0; parent.swipeDone = false } }
                WheelHandler {
                    target: null
                    orientation: Qt.Horizontal
                    onWheel: function(ev) {
                        var st = parent
                        var dx = ev.pixelDelta.x !== 0 ? ev.pixelDelta.x : ev.angleDelta.x / 2
                        swipeRest.restart()
                        if (st.swipeDone || ev.phase === Qt.ScrollMomentum) return
                        st.swipeSum += dx
                        if (Math.abs(st.swipeSum) > 90) { page.shift(st.swipeSum < 0 ? 1 : -1); st.swipeDone = true }
                    }
                }

                CalTimeGrid { days: 1; cal: page }
                CalTimeGrid { days: 7; cal: page }
                CalMonthGrid { cal: page }
                CalYearGrid { cal: page }
            }
        }
    }
}
