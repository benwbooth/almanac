import QtQuick
import QtQuick.Controls.Basic
// The system's own menus (desktop style on Linux, native on macOS and Windows)
import Qt.labs.platform as P
import QtQuick.Layouts

// Pop-up form to add or change one calendar event.
Popup {
    // A stray click outside doesn't throw away the edits (Escape still closes).
    closePolicy: Popup.CloseOnEscape
    id: ed
    property var store
    readonly property alias notesArea: notes.area
    property var calendars: []
    property var ev: ({})
    property bool isNew: true
    property bool showZone: true
    // Time zones: event times are entered in the event's own zone.
    property var zoneInfo: ({ system: "UTC", zones: [] })
    readonly property var zoneChoices: ["Floating (no time zone)"].concat(zoneInfo.zones)
    modal: true
    focus: true
    anchors.centerIn: Overlay.overlay
    width: Math.min(640, (Overlay.overlay ? Overlay.overlay.width : 680) - 40)
    // (The notes give up room first when the window is short.)
    height: Math.min(implicitHeight, (Overlay.overlay ? Overlay.overlay.height : 800) - 40)
    padding: 20
    background: Rectangle { radius: 12; color: Theme.popup; border.color: Theme.border }

    readonly property var repeats: [
        { label: "Never", rule: "" },
        { label: "Every Day", rule: "FREQ=DAILY" },
        { label: "Every Weekday", rule: "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR" },
        { label: "Every Week", rule: "FREQ=WEEKLY" },
        { label: "Every Month", rule: "FREQ=MONTHLY" },
        { label: "Every Year", rule: "FREQ=YEARLY" }
    ]

    readonly property var alertChoices: [["None", null], ["At time of event", 0], ["5 minutes before", 5], ["10 minutes before", 10],
        ["15 minutes before", 15], ["30 minutes before", 30], ["1 hour before", 60], ["2 hours before", 120],
        ["1 day before", 1440], ["2 days before", 2880], ["1 week before", 10080], ["On the day (9:00 AM)", -540]]
    function alertIndex(m) { for (var i = 0; i < alertChoices.length; i++) if (alertChoices[i][1] === m) return i; return 0 }

    function openEvent(e, isNew) {
        ev = e
        var al = e.alarms || []
        alert1.currentIndex = alertIndex(al.length > 0 ? al[0] : null)
        alert2.currentIndex = alertIndex(al.length > 1 ? al[1] : null)
        invitees.text = (e.attendees || []).map(function(a) { return a.email }).join(", ")
        ed.isNew = isNew
        title.text = e.title || ""
        location.text = e.location || ""
        // The notes, formatted (as they came, or as written here before).
        var prefs = JSON.parse(store.settings())
        notes.markdown = prefs.markdown_compose !== false
        notes.spellOn = prefs.spell_check !== false
        notes.grammarOn = prefs.grammar_check !== false
        notes.lints = []; notes.ignored = []
        notes.converting = true
        // (Some calendars put HTML in the plain notes; characters written by
        // number twice over, &amp;#128205;, as the characters.)
        var n = (e.notesHtml || e.notes || "").replace(/&amp;#(x[0-9a-f]+|\d+);/gi, "&#$1;")
        if (/<\s*(a|b|i|u|p|br|div|span|ul|ol|li|strong|em|table|h[1-6])\b[^>]*>/i.test(n)) notes.setBodyHtml(n)
        else { notes.colourBack = {}; notes.area.text = notes.esc(n).replace(/\n/g, "<br>") }
        notes.converting = false
        notes.area.cursorPosition = 0
        allDay.checked = !!e.allDay
        if (zoneInfo.zones.length === 0) zoneInfo = JSON.parse(store.timeZones())
        var z = e.tz === undefined ? zoneInfo.system : e.tz
        var zi = z === "" ? 0 : zoneChoices.indexOf(z)
        zone.currentIndex = zi >= 0 ? zi : 0
        var s = Theme.parseDT(e.start), en = Theme.parseDT(e.end)
        if (e.allDay) en = Theme.addDays(en, -1)
        startDate.text = Theme.isoDate(s)
        startTime.value = Theme.pad2(s.getHours()) + ":" + Theme.pad2(s.getMinutes())
        endDate.text = Theme.isoDate(en)
        endTime.value = Theme.pad2(en.getHours()) + ":" + Theme.pad2(en.getMinutes())
        lastStart = startDate.text + "T" + startTime.value
        var ci = 0
        for (var i = 0; i < calendars.length; i++) if (calendars[i].id === e.calendar) ci = i
        cal.currentIndex = ci
        var ri = -1
        for (var j = 0; j < repeats.length; j++) if (repeats[j].rule === (e.rrule || "")) ri = j
        repeat.model = ri >= 0 ? repeats.map(function(r) { return r.label })
                                : repeats.map(function(r) { return r.label }).concat(["Custom (kept)"])
        repeat.currentIndex = ri >= 0 ? ri : repeats.length
        open()
        title.forceActiveFocus()
        title.selectAll()
    }
    // Moving the start keeps the event's length, as in Apple Calendar.
    property string lastStart: ""
    function startMoved() {
        var was = Theme.parseDT(lastStart), now = Theme.parseDT(startDate.text + "T" + startTime.value)
        var end = Theme.parseDT(endDate.text + "T" + endTime.value)
        if (was && now && end) {
            var ne = new Date(end.getTime() + (now.getTime() - was.getTime()))
            endDate.text = Theme.isoDate(ne)
            endTime.value = Theme.pad2(ne.getHours()) + ":" + Theme.pad2(ne.getMinutes())
        }
        lastStart = startDate.text + "T" + startTime.value
    }
    function payload() {
        var s = startDate.text + "T" + (allDay.checked ? "00:00" : startTime.value)
        var e
        if (allDay.checked) e = Theme.isoDate(Theme.addDays(Theme.parseDT(endDate.text), 1)) + "T00:00"
        else e = endDate.text + "T" + endTime.value
        var rule = repeat.currentIndex < repeats.length ? repeats[repeat.currentIndex].rule : (ev.rrule || "")
        var alarms = []
        var a1 = alertChoices[alert1.currentIndex][1], a2 = alertChoices[alert2.currentIndex][1]
        if (a1 !== null) alarms.push(a1)
        if (a2 !== null && a2 !== a1) alarms.push(a2)
        return {
            id: ev.id || 0, title: title.text, location: location.text, notes: notes.plainText(),
            notesHtml: store.restoreColours(notes.area.text.replace(/\u200b|&#8203;/g, ""), JSON.stringify(notes.colourBack)),
            allDay: allDay.checked, start: s, end: e, calendar: calendars.length ? calendars[cal.currentIndex].id : "",
            rrule: rule, alarms: alarms,
            attendees: invitees.text.split(/[,;]/).map(function(x) { return x.trim() }).filter(function(x) { return x.indexOf("@") > 0 }),
            tz: zone.currentIndex <= 0 ? "" : zoneChoices[zone.currentIndex]
        }
    }
    function save() {
        // Changing one occurrence of a repeating event: ask how far it goes.
        if (!isNew && ev.repeats && !ev.override && ev.occurrence) { scopeMenu.open(saveBtn); return }
        doSave("all")
    }
    function doSave(scope) {
        var p = payload()
        if (!isNew && ev.repeats && ev.occurrence) {
            if (scope === "all") {
                // Move the whole series by as much as this occurrence moved.
                var occ = Theme.parseDT(ev.occurrence), ns = Theme.parseDT(p.start), ne = Theme.parseDT(p.end)
                var ss = new Date(Theme.parseDT(ev.seriesStart).getTime() + (ns - occ))
                p.start = Theme.isoDT(ss)
                p.end = Theme.isoDT(new Date(ss.getTime() + (ne - ns)))
            } else {
                p.scope = scope
                p.occurrence = ev.occurrence
            }
        }
        store.saveEvent(JSON.stringify(p))
        close()
    }

    component Label2: Text { color: Theme.secondary; font.pixelSize: Theme.base; Layout.preferredWidth: 92; horizontalAlignment: Text.AlignRight; elide: Text.ElideRight }

    contentItem: ColumnLayout {
        spacing: 10
        FieldX {
            id: title
            Layout.fillWidth: true
            placeholderText: "New Event"
            font.pixelSize: 18
            font.weight: Font.DemiBold
            selectByMouse: true
            onAccepted: ed.save()
        }
        FieldX { id: location; Layout.fillWidth: true; placeholderText: "Add Location"; selectByMouse: true }
        GridLayout {
            // Fields share the panel's width (nothing runs past its edge).
            Layout.fillWidth: true
            columns: 3
            columnSpacing: 8
            rowSpacing: 8
            Label2 { text: "all-day:" }
            CheckX { id: allDay; Layout.columnSpan: 2 }
            Label2 { text: "starts:" }
            DateField { id: startDate; Layout.fillWidth: true; Layout.maximumWidth: 180; Layout.minimumWidth: 120; onEdited: ed.startMoved() }
            TimeField { id: startTime; visible: !allDay.checked; onEdited: ed.startMoved() }
            Label2 { text: "ends:" }
            DateField { id: endDate; Layout.fillWidth: true; Layout.maximumWidth: 180; Layout.minimumWidth: 120 }
            TimeField { id: endTime; visible: !allDay.checked }
            Label2 { text: "time zone:"; visible: !allDay.checked && ed.showZone }
            ComboX {
                id: zone
                visible: !allDay.checked && ed.showZone
                Layout.columnSpan: 2
                Layout.fillWidth: true
                Layout.minimumWidth: 120
                model: ed.zoneChoices
                editable: true
                onAccepted: { var i = find(editText); if (i >= 0) currentIndex = i }
            }
            Item { width: 1; height: 1; visible: !allDay.checked && zone.currentIndex > 0 && ed.zoneChoices[zone.currentIndex] !== ed.zoneInfo.system }
            Text {
                visible: !allDay.checked && zone.currentIndex > 0 && ed.zoneChoices[zone.currentIndex] !== ed.zoneInfo.system
                Layout.columnSpan: 2
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                text: "Shown in your calendar in " + ed.zoneInfo.system + " time"
                color: Theme.secondary; font.pixelSize: 11
            }
            Label2 { text: "alert:" }
            ComboX { id: alert1; Layout.columnSpan: 2; Layout.fillWidth: true; Layout.maximumWidth: 240; model: ed.alertChoices.map(function(a) { return a[0] }) }
            Label2 { text: "second alert:"; visible: alert1.currentIndex > 0 }
            ComboX { id: alert2; visible: alert1.currentIndex > 0; Layout.columnSpan: 2; Layout.fillWidth: true; Layout.maximumWidth: 240; model: ed.alertChoices.map(function(a) { return a[0] }) }
            Label2 { text: "invitees:" }
            FieldX {
                id: invitees
                Layout.columnSpan: 2
                Layout.fillWidth: true
                placeholderText: "Add Invitees (email, email…)"
                selectByMouse: true
            }
            Item { width: 1; height: 1; visible: (ed.ev.attendees || []).length > 0 }
            Flow {
                visible: (ed.ev.attendees || []).length > 0
                Layout.columnSpan: 2
                Layout.fillWidth: true
                spacing: 6
                Repeater {
                    model: ed.ev.attendees || []
                    delegate: Text {
                        required property var modelData
                        text: ({ "ACCEPTED": "✓ ", "DECLINED": "✗ ", "TENTATIVE": "? " })[modelData.status] || "· "
                        + (modelData.name || modelData.email)
                        color: modelData.status === "ACCEPTED" ? "#34a853" : (modelData.status === "DECLINED" ? Theme.red : Theme.secondary)
                        font.pixelSize: 11
                    }
                }
            }
            Label2 { text: "repeat:" }
            ComboX { id: repeat; Layout.columnSpan: 2; Layout.fillWidth: true; Layout.maximumWidth: 240 }
            Label2 { text: "calendar:" }
            ComboX {
                id: cal
                Layout.columnSpan: 2
                Layout.fillWidth: true
                Layout.maximumWidth: 240
                model: ed.calendars.map(function(c) { return c.name })
            }
        }
        // Notes: the same writing area (and formatting bar) as a mail message.
        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.preferredHeight: 380
            Layout.minimumHeight: 150
            radius: 6
            color: Theme.field
            border.color: notes.area.activeFocus ? Theme.accent : Theme.border
            clip: true
            RichEditor {
                id: notes
                anchors.fill: parent
                anchors.margins: 1
                store: ed.store
                active: ed.opened
                Component.onCompleted: { area.placeholderText = "Add Notes"; area.topPadding = 8; area.leftPadding = 10; area.rightPadding = 10 }
            }
        }
        RowLayout {
            Layout.fillWidth: true
            ToolButtonX {
                id: delBtn
                visible: !ed.isNew
                text: "Delete"; icon: "trash"
                onClicked: {
                    if (ed.ev.repeats) delMenu.open(delBtn)
                    else { store.deleteEvent(ed.ev.id); ed.close() }
                }
                P.Menu {
                    id: delMenu
                    P.MenuItem { icon.name: "edit-delete"; text: "Delete Only This Event"; onTriggered: { store.deleteEventOccurrence(ed.ev.id, ed.ev.occurrence || "", "this"); ed.close() } }
                    P.MenuItem { icon.name: "edit-delete"; text: "Delete All Future Events"; onTriggered: { store.deleteEventOccurrence(ed.ev.id, ed.ev.occurrence || "", "future"); ed.close() } }
                    P.MenuItem { icon.name: "edit-delete"; text: "Delete the Whole Series"; onTriggered: { store.deleteEvent(ed.ev.id); ed.close() } }
                }
            }
            Item { Layout.fillWidth: true }
            ToolButtonX { text: "Cancel"; onClicked: ed.close() }
            ToolButtonX {
                id: saveBtn
                text: ed.isNew ? "Add" : "Save"; primary: true
                onClicked: ed.save()
                P.Menu {
                    id: scopeMenu
                    P.MenuItem { icon.name: "document-save"; text: "Save for This Event Only"; onTriggered: ed.doSave("this") }
                    P.MenuItem { icon.name: "document-save"; text: "Save for Future Events"; onTriggered: ed.doSave("future") }
                    P.MenuItem { icon.name: "document-save"; text: "Save for All Events"; onTriggered: ed.doSave("all") }
                }
            }
        }
    }
}
