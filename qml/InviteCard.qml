import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// A meeting invitation inside an email: when, where, and Accept / Maybe /
// Decline (which adds it to a calendar and answers the organiser).
Rectangle {
    id: card
    property var store
    property var messageId: -1
    property var info: { store.revision; return messageId >= 0 ? JSON.parse(store.inviteInfo(messageId)) : null }
    visible: info !== null
    implicitHeight: visible ? col.implicitHeight + 24 : 0
    radius: 10
    color: Theme.card
    border.color: Theme.border

    function when() {
        if (!info) return ""
        var s = Theme.parseDT(info.start), e = Theme.parseDT(info.end)
        var d = s.toLocaleDateString(Qt.locale(), Locale.LongFormat)
        return info.allDay ? d + " (all day)" : d + ", " + Theme.fmtTime(s) + " – " + Theme.fmtTime(e)
    }

    ColumnLayout {
        id: col
        x: 14; y: 12
        width: parent.width - 28
        spacing: 6
        RowLayout {
            Layout.fillWidth: true
            Rectangle {
                width: 40; height: 44; radius: 8
                color: Theme.window
                border.color: Theme.border
                Column {
                    anchors.centerIn: parent
                    Text { anchors.horizontalCenter: parent.horizontalCenter; text: card.info ? Theme.monthName(Theme.parseDT(card.info.start).getMonth()).substr(0, 3).toUpperCase() : ""; color: Theme.red; font.pixelSize: 9; font.weight: Font.Bold }
                    Text { anchors.horizontalCenter: parent.horizontalCenter; text: card.info ? Theme.parseDT(card.info.start).getDate() : ""; color: Theme.text; font.pixelSize: 18; font.weight: Font.Bold }
                }
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 1
                Text {
                    Layout.fillWidth: true
                    text: card.info ? (card.info.method === "CANCEL" ? "Cancelled: " : (card.info.method === "REPLY" ? "Reply: " : "")) + card.info.title : ""
                    font.pixelSize: 15; font.weight: Font.Bold; color: Theme.text; elide: Text.ElideRight
                }
                Text { text: card.when() + (card.info && card.info.repeats ? "  · repeats" : ""); color: Theme.secondary; font.pixelSize: 12 }
                Text {
                    visible: card.info && (card.info.location || card.info.organizer)
                    text: card.info ? [card.info.location, card.info.organizer ? "from " + card.info.organizer : ""].filter(function(x) { return x }).join(" · ") : ""
                    color: Theme.secondary; font.pixelSize: 12
                }
            }
        }
        RowLayout {
            visible: card.info && card.info.method !== "REPLY"
            Layout.fillWidth: true
            spacing: 6
            ComboX {
                id: calPick
                visible: card.info && card.info.method !== "CANCEL"
                Layout.preferredWidth: 200
                model: card.info ? card.info.calendars.map(function(c) { return c.name }) : []
                currentIndex: {
                    if (!card.info) return 0
                    var i = card.info.calendars.findIndex(function(c) { return c.id === card.info.calendar })
                    return Math.max(0, i)
                }
            }
            Item { Layout.fillWidth: true }
            Text {
                visible: card.info && card.info.myStatus !== ""
                text: card.info ? ({ "ACCEPTED": "You accepted", "TENTATIVE": "You said maybe", "DECLINED": "You declined" })[card.info.myStatus] || "" : ""
                color: Theme.secondary; font.pixelSize: 12
            }
            function answer(st) { store.respondInvite(card.messageId, st, card.info.calendars.length ? card.info.calendars[calPick.currentIndex].id : "") }
            ToolButtonX { visible: card.info && card.info.method !== "CANCEL"; text: "Accept"; primary: card.info && card.info.myStatus !== "ACCEPTED"; onClicked: parent.answer("ACCEPTED") }
            ToolButtonX { visible: card.info && card.info.method !== "CANCEL"; text: "Maybe"; onClicked: parent.answer("TENTATIVE") }
            ToolButtonX { visible: card.info && card.info.method !== "CANCEL"; text: "Decline"; onClicked: parent.answer("DECLINED") }
            ToolButtonX { visible: card.info && card.info.method === "CANCEL" && card.info.inCalendar; text: "Remove from Calendar"; primary: true; onClicked: parent.answer("DECLINED") }
        }
    }
}
