import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// A message (or its whole conversation) in a tab of the Mail view, opened by
// double-clicking it in the list (Shift + double-click: its own window).
Item {
    id: t
    property var store
    // The Mail view: replies, forwards and the reading settings come from it.
    property var view
    // The mailbox it was opened from (for the conversation) and the message.
    property string selection: ""
    property var messageId: -1
    property bool threaded: true
    // Deleted or moved away: the tab goes.
    signal closeRequested()
    // Into a window of its own (the tab moves there).
    signal popOutRequested()
    // In a message window, whose title bar has the buttons: no bar of its own.
    property bool bare: false
    // After a reply, forward or new message is started (in the main window).
    signal composing()

    readonly property var messages: {
        store.revision
        var list = threaded ? JSON.parse(store.thread(selection, messageId)) : []
        if (!list.length) {
            var m = JSON.parse(store.message(messageId))
            if (m && m.id !== undefined) list = [m]
        }
        return list
    }
    readonly property var current: messages.length ? messages[messages.length - 1] : null
    readonly property var ids: messages.map(function(m) { return m.id })
    onMessagesChanged: if (!messages.length) Qt.callLater(t.closeRequested)

    function act(m, action) {
        if (action === "delete") { store.deleteMessages(JSON.stringify([m.id])); return }
        view.messageAction(m, action)
        composing()
    }
    function moveAll(kind) {
        if (kind === "delete") store.deleteMessages(JSON.stringify(ids))
        else for (var i = 0; i < ids.length; i++) store.moveTo(ids[i], kind)
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        Rectangle {
            visible: !t.bare
            Layout.fillWidth: true
            Layout.preferredHeight: 40
            color: Theme.window
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 10; anchors.rightMargin: 10
                spacing: 2
                ToolButtonX { icon: "archive"; tip: "Archive"; enabled: t.current !== null; onClicked: t.moveAll("archive") }
                ToolButtonX { icon: "trash"; tip: "Delete"; enabled: t.current !== null; onClicked: t.moveAll("delete") }
                ToolButtonX { icon: "junk"; tip: "Move to Junk"; enabled: t.current !== null; onClicked: t.moveAll("junk") }
                Item { width: 6 }
                ToolButtonX { icon: "reply"; tip: "Reply"; enabled: t.current !== null; onClicked: t.act(t.current, "reply") }
                ToolButtonX { icon: "replyAll"; tip: "Reply All"; enabled: t.current !== null; onClicked: t.act(t.current, "replyAll") }
                ToolButtonX { icon: "forward"; tip: "Forward"; enabled: t.current !== null; onClicked: t.act(t.current, "forward") }
                Item { width: 6 }
                ToolButtonX {
                    icon: "flag"
                    iconColor: t.current && t.current.flagged ? Theme.flagColors[t.current.flagColor || 0] : Theme.secondary
                    tip: "Flag"; enabled: t.current !== null
                    onClicked: t.store.toggleFlag(t.current.id)
                }
                ToolButtonX {
                    icon: "envelopeOpen"
                    tip: t.current && !t.current.read ? "Mark as Read" : "Mark as Unread"
                    enabled: t.current !== null
                    onClicked: { var r = !t.current.read; for (var i = 0; i < t.ids.length; i++) t.store.setRead(t.ids[i], r) }
                }
                Item { Layout.fillWidth: true }
                ToolButtonX { icon: "popOut"; tip: "Open in a Separate Window"; enabled: t.current !== null; onClicked: t.popOutRequested() }
                ToolButtonX { icon: "printer"; tip: "Print"; enabled: t.current !== null; onClicked: t.view.printMessages(t.messages) }
            }
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.borderSoft }
        }
        MessagePage {
            Layout.fillWidth: true
            Layout.fillHeight: true
            store: t.store
            messages: t.messages
            subject: t.current ? t.current.subject : ""
            formatted: t.view.formatted
            darkMessages: !!t.view.prefs.dark_messages
            readableLogos: t.view.prefs.readable_logos !== false
            loadRemoteAlways: !!t.view.prefs.load_remote
            remoteLoaded: t.view.remoteLoaded
            remoteSenders: t.view.prefs.remote_senders || []
            previewsFor: function(id) { return JSON.parse(t.store.attachmentPreviews(id)) }
            zoom: t.view.textZoom
            onSenderClicked: function(m) { t.view.senderClicked(m) }
            onAttachmentClicked: function(m, name, save) { if (save) t.view.saveAttachment(m, name); else t.store.openAttachment(m.id, name, false) }
            onAttachmentView: function(m, name, where) { t.view.attachmentView(m, name, where); if (where === "tab") t.composing() }
            onRemoteRequested: function(m, scope) { t.view.remoteRequested(m, scope) }
            onFormatToggled: t.view.formatted = !t.view.formatted
            onMailto: function(address) { t.view.composer.openWith({ to: address }); t.composing() }
            onMessageAction: function(m, action) { t.act(m, action) }
        }
    }
}
