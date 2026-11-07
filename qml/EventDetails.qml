import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// One calendar event, to read (as Apple Calendar's event pop-over): when and
// where, a Join button for video calls (Zoom, Meet, Teams…), who's invited,
// and the notes, formatted as sent, with working links. Edit opens the form.
Popup {
    id: d
    property var ev: ({})
    // The calendars ({id, name, color}), for the event's.
    property var calendars: []
    // Edit: changing it (EventEditor); none for birthdays and subscriptions.
    signal editRequested(var ev)
    readonly property bool editable: !!ev && !ev.virtual && !ev.readOnly

    function show(e) { ev = e; open(); notesView.contentY = 0 }

    modal: true
    focus: true
    anchors.centerIn: Overlay.overlay
    width: Math.min(520, (Overlay.overlay ? Overlay.overlay.width : 560) - 40)
    height: Math.min(implicitHeight, (Overlay.overlay ? Overlay.overlay.height : 700) - 60)
    padding: 20
    background: Rectangle { radius: 12; color: Theme.popup; border.color: Theme.border }

    // ------------------------------------------------------------ wording
    readonly property var start: ev && ev.start ? Theme.parseDT(ev.start) : null
    readonly property var end: ev && ev.end ? Theme.parseDT(ev.end) : null
    readonly property string when: {
        if (!start || !end) return ""
        var longDate = function(x) { return x.toLocaleDateString(Qt.locale(), Locale.LongFormat) }
        if (ev.allDay) {
            var last = Theme.addDays(end, -1)
            return last > start ? longDate(start) + " – " + longDate(last) : longDate(start) + ", all day"
        }
        if (Theme.isoDate(start) === Theme.isoDate(end)) return longDate(start) + "\n" + Theme.fmtTime(start) + " – " + Theme.fmtTime(end)
        return longDate(start) + ", " + Theme.fmtTime(start) + " –\n" + longDate(end) + ", " + Theme.fmtTime(end)
    }
    readonly property string repeatText: {
        var r = ev && ev.rrule ? ev.rrule : ""
        if (!r) return ""
        var part = function(k) { var m = r.match(new RegExp("(?:^|;)" + k + "=([^;]*)")); return m ? m[1] : "" }
        var n = parseInt(part("INTERVAL") || "1")
        var unit = ({ DAILY: "day", WEEKLY: "week", MONTHLY: "month", YEARLY: "year" })[part("FREQ")] || ""
        if (!unit) return "Repeats"
        var t = n > 1 ? "Every " + n + " " + unit + "s" : "Every " + unit
        var days = part("BYDAY")
        if (days === "MO,TU,WE,TH,FR") t = "Every weekday"
        else if (days) t += " on " + days.split(",").map(function(x) { return ({ MO: "Mon", TU: "Tue", WE: "Wed", TH: "Thu", FR: "Fri", SA: "Sat", SU: "Sun" })[x.slice(-2)] || x }).join(", ")
        var until = part("UNTIL")
        if (until) t += " until " + Theme.parseDT(until.slice(0, 4) + "-" + until.slice(4, 6) + "-" + until.slice(6, 8) + "T00:00").toLocaleDateString(Qt.locale(), Locale.ShortFormat)
        return t
    }
    function alertText(m) {
        if (m === 0) return "At time of event"
        if (m < 0) return "On the day (" + Theme.fmtTime(new Date(2000, 0, 1, Math.floor(-m / 60), -m % 60)) + ")"
        if (m % 10080 === 0) return (m / 10080) + (m === 10080 ? " week" : " weeks") + " before"
        if (m % 1440 === 0) return (m / 1440) + (m === 1440 ? " day" : " days") + " before"
        if (m % 60 === 0) return (m / 60) + (m === 60 ? " hour" : " hours") + " before"
        return m + " minutes before"
    }
    readonly property var calendar: {
        for (var i = 0; i < calendars.length; i++) if (ev && calendars[i].id === ev.calendar) return calendars[i]
        return null
    }

    // A video call's link (in the location or the notes), for the Join button.
    readonly property var meeting: {
        var text = ((ev && ev.location) || "") + "\n" + ((ev && ev.notes) || "") + "\n" + ((ev && ev.notesHtml) || "")
        text = text.replace(/&amp;/g, "&")
        var kinds = [[/zoom\.us\//i, "Zoom"], [/meet\.google\.com\//i, "Google Meet"], [/teams\.(microsoft|live)\.com\//i, "Microsoft Teams"],
                     [/webex\.com\//i, "Webex"], [/whereby\.com\//i, "Whereby"], [/(gotomeeting|goto)\.com\//i, "GoTo Meeting"], [/chime\.aws\//i, "Amazon Chime"]]
        var urls = (ev && ev.url ? [ev.url] : []).concat(text.match(/https?:\/\/[^\s"'<>)\]]+/gi) || [])
        for (var k = 0; k < kinds.length; k++)
            for (var i = 0; i < urls.length; i++) if (kinds[k][0].test(urls[i])) return { url: urls[i].replace(/[.,;]+$/, ""), name: kinds[k][1] }
        // A link given as the event's own (whatever service): still a call.
        return ev && ev.url && /^https?:/i.test(ev.url) ? { url: ev.url, name: "" } : null
    }

    // The notes as the page shows them: HTML kept (without its own colours,
    // which would vanish in dark mode), plain text with its line breaks, and
    // web addresses in either made into links.
    function esc(t) { return t.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;") }
    function linkify(t) {
        return t.replace(/(https?:\/\/[^\s"'<>]+[^\s"'<>.,;:!?)\]])/gi, '<a href="$1">$1</a>')
                .replace(/(^|[\s(])([A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,})/g, '$1<a href="mailto:$2">$2</a>')
    }
    // Links in the accent colour (readable in dark mode too).
    function tint(h) { return h.replace(/<a\b/gi, '<a style="color:' + Theme.accent + '"') }
    readonly property string notesHtml: {
        var n = (ev && (ev.notesHtml || ev.notes)) || ""
        if (!n.trim()) return ""
        // Characters written by number (&#128205; is 📍), also encoded twice over
        // by the sender's app (&amp;#128205;): shown as the characters.
        n = n.replace(/&amp;#(x[0-9a-f]+|\d+);/gi, "&#$1;")
        n = n.replace(/&#(x[0-9a-f]+|\d+);/gi, function(all, code) {
            var cp = code[0] === "x" || code[0] === "X" ? parseInt(code.slice(1), 16) : parseInt(code, 10)
            return cp > 0 && cp <= 0x10ffff ? String.fromCodePoint(cp) : all
        })
        var html = /<\s*(a|b|i|u|p|br|div|span|ul|ol|li|strong|em|table|h[1-6])\b[^>]*>/i.test(n)
        if (!html) return tint(linkify(esc(n)).replace(/\r?\n/g, "<br>"))
        n = n.replace(/<(script|style)\b[\s\S]*?<\/\1>/gi, "")
             .replace(/\s(color|bgcolor)\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)/gi, "")
             .replace(/(style\s*=\s*")([^"]*)"/gi, function(all, head, css) { return head + css.replace(/(^|;)\s*(background(-color)?|color)\s*:[^;]*/gi, "") + '"' })
        // Web addresses in the text (not already links, nor inside tags).
        var parts = n.split(/(<a\b[\s\S]*?<\/a>|<[^>]*>)/i)
        for (var i = 0; i < parts.length; i++) if (parts[i] && parts[i][0] !== "<") parts[i] = linkify(parts[i])
        return tint(parts.join(""))
    }

    component Row2: RowLayout {
        property string icon
        property string text
        property alias textColor: rowText.color
        visible: text !== ""
        spacing: 10
        Layout.fillWidth: true
        Icon { name: parent.icon; width: 16; height: 16; color: Theme.secondary; Layout.alignment: Qt.AlignTop; Layout.topMargin: 2 }
        TextEdit {
            id: rowText
            Layout.fillWidth: true
            text: parent.text
            readOnly: true
            selectByMouse: true
            wrapMode: TextEdit.Wrap
            textFormat: TextEdit.RichText
            color: Theme.text
            font.pixelSize: Theme.base
            onLinkActivated: function(link) { Qt.openUrlExternally(link) }
            HoverHandler { cursorShape: parent.hoveredLink ? Qt.PointingHandCursor : Qt.IBeamCursor }
        }
    }

    contentItem: ColumnLayout {
        spacing: 12
        RowLayout {
            Layout.fillWidth: true
            spacing: 10
            Rectangle { width: 5; Layout.fillHeight: true; radius: 2.5; color: (d.ev && d.ev.color) || Theme.accent }
            TextEdit {
                Layout.fillWidth: true
                text: (d.ev && d.ev.title) || "(No Title)"
                readOnly: true
                selectByMouse: true
                wrapMode: TextEdit.Wrap
                font.pixelSize: 19
                font.weight: Font.Bold
                color: Theme.text
            }
        }
        Row2 { icon: "calendar"; text: d.esc(d.when).replace(/\n/g, "<br>") + (d.ev && d.ev.tz && !d.ev.allDay ? '<br><span style="color:' + Theme.secondary + '">' + d.esc(d.ev.tz) + "</span>" : "") }
        Row2 { icon: "repeat"; text: d.esc(d.repeatText) }
        Row2 { icon: "location"; text: d.ev && d.ev.location ? d.tint(d.linkify(d.esc(d.ev.location))) : "" }
        // A video call: one click to join it.
        ToolButtonX {
            visible: d.meeting !== null
            Layout.leftMargin: 26
            primary: true
            icon: d.meeting && !d.meeting.name ? "link" : "video"
            text: !d.meeting ? "" : d.meeting.name ? "Join " + d.meeting.name : "Open Link"
            tip: d.meeting ? d.meeting.url : ""
            onClicked: Qt.openUrlExternally(d.meeting.url)
        }
        Row2 {
            icon: "bell"
            text: d.ev && d.ev.alarms && d.ev.alarms.length ? d.ev.alarms.map(d.alertText).join("<br>") : ""
        }
        Row2 {
            icon: "people"
            text: {
                var a = (d.ev && d.ev.attendees) || []
                if (!a.length) return ""
                var mark = { ACCEPTED: '<span style="color:#34a853">✓</span> ', DECLINED: '<span style="color:' + Theme.red + '">✗</span> ', TENTATIVE: "? " }
                var org = d.ev.organizer ? "Organizer: " + d.esc(d.ev.organizer.replace(/^mailto:/i, "")) + "<br>" : ""
                return org + a.map(function(x) { return (mark[x.status] || "· ") + d.esc(x.name || x.email) + (x.name ? ' <span style="color:' + Theme.secondary + '">' + d.esc(x.email) + "</span>" : "") }).join("<br>")
            }
        }
        Row2 {
            icon: "info"
            textColor: Theme.secondary
            text: d.ev && d.ev.virtual ? "Birthdays come from the cards in Contacts. Change the date there."
                : d.ev && d.ev.readOnly ? "From a subscribed calendar, so it can't be changed here." : ""
        }
        RowLayout {
            visible: d.calendar !== null
            spacing: 10
            Rectangle { width: 10; height: 10; radius: 5; color: d.calendar ? d.calendar.color : "transparent"; Layout.leftMargin: 3; Layout.rightMargin: 3 }
            Text { text: d.calendar ? d.calendar.name : ""; color: Theme.secondary; font.pixelSize: Theme.base }
        }
        // The notes, scrolling when long.
        Rectangle {
            visible: d.notesHtml !== ""
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.preferredHeight: Math.min(notesText.implicitHeight + 20, 380)
            Layout.minimumHeight: Math.min(notesText.implicitHeight + 20, 80)
            radius: 8
            color: Theme.field
            border.color: Theme.borderSoft
            Flickable {
                id: notesView
                anchors.fill: parent
                anchors.margins: 10
                clip: true
                contentHeight: notesText.implicitHeight
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar {}
                TextEdit {
                    id: notesText
                    width: notesView.width
                    readOnly: true
                    selectByMouse: true
                    wrapMode: TextEdit.Wrap
                    textFormat: TextEdit.RichText
                    text: d.notesHtml
                    color: Theme.text
                    font.pixelSize: Theme.base
                    onLinkActivated: function(link) { Qt.openUrlExternally(link) }
                    HoverHandler { cursorShape: notesText.hoveredLink ? Qt.PointingHandCursor : Qt.IBeamCursor }
                }
            }
        }
        RowLayout {
            Layout.fillWidth: true
            Item { Layout.fillWidth: true }
            ToolButtonX { text: "Close"; onClicked: d.close() }
            ToolButtonX {
                visible: d.editable
                text: "Edit"; icon: "edit"; primary: true
                onClicked: { var e = d.ev; d.close(); d.editRequested(e) }
            }
        }
    }
}
