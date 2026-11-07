import QtQuick
import QtQuick.Controls.Basic
// The system's own menus (desktop style on Linux, native on macOS and Windows)
import Qt.labs.platform as P
import QtQuick.Layouts
import QtQuick.Window
import QtQuick.Dialogs

// Writing a new message, reply or forward — a sheet inside the main window,
// a window of its own, or (replies and forwards from the buttons on a
// message) right under the message, in the reading pane.
// Formatting (bold, italic, lists, colours, sizes), attachments, Bcc,
// signatures and Send Later.
Popup {
    id: sheet
    property var store
    property var draftId: null
    property var senders: []
    property var attachments: []
    property string inReplyTo: ""
    property string lastSignature: ""
    // Id of the message being forwarded (its attachments arrive a moment later)
    property var forwardOf: -1
    parent: Overlay.overlay
    x: Math.round((parent.width - width) / 2)
    // Drops down from just under the top bar, like a sheet in Apple Mail.
    y: Theme.toolbarHeight
    // Size: the corner grip resizes it; the expand button makes it nearly as
    // big as the window. The chosen size is kept for the next message.
    // Starts at most of the window (as big as Apple Mail's), resizable from the corner.
    property real wantW: parent ? Math.max(980, parent.width * 0.78) : 980
    property real wantH: parent ? Math.max(760, parent.height * 0.9) : 760
    property bool large: false

    // Its own window (like Apple Mail's separate message windows): the
    // sheet's contents move into a window and back.
    property bool detached: false
    // Written right under the message (in `inlineHost`, an item of the reading pane).
    property bool inline: false
    property Item inlineHost: null
    readonly property bool active: opened || detached || inline
    // The writing area moves between the sheet, its own window and the reading
    // pane (always filling where it is; the sheet sizes only its holder).
    function moveContent(to) { pane.parent = to || holder }
    function toInline(host) { inlineHost = host; moveContent(host); inline = true }
    function fromInline() { moveContent(null); inline = false; inlineHost = null }
    // Moving on to another message: the reply written under this one is kept in
    // Drafts (if anything was written), and the space goes.
    function leaveInline() {
        if (!inline) return
        if (snapshot() !== savedSnapshot) { saveDraftNow(); store.statusText = "Your unsent message was saved in Drafts." }
        fromInline()
    }
    // Under the message → a sheet over the window.
    function inlineToSheet() { fromInline(); open() }
    Window {
        id: ownWindow
        title: (subjectRow.field.text || "New Message") + " — Almanac"
        width: 1000; height: 820
        minimumWidth: 480; minimumHeight: 360
        // A window of its own (not tied to the main one), as in Apple Mail,
        // whose top bar is its title bar (like the main window's).
        transientParent: null
        flags: Qt.platform.os === "osx" ? (Qt.Window | Qt.ExpandedClientAreaHint | Qt.NoTitleBarBackgroundHint)
                                        : (Qt.Window | Qt.FramelessWindowHint)
        color: Theme.window
        ResizeEdges { visible: Qt.platform.os !== "osx" && ownWindow.visibility !== Window.Maximized; window: ownWindow }
        onClosing: function(ev) { ev.accepted = false; sheet.requestClose() }
    }
    function popOut() {
        if (inline) { inline = false; inlineHost = null }
        detached = true
        close()
        moveContent(ownWindow.contentItem)
        ownWindow.show(); ownWindow.raise(); ownWindow.requestActivate()
    }
    function putBack() {
        ownWindow.hide()
        moveContent(null)
        detached = false
    }
    // For checks: types text as if from the keyboard (Enter as "\n").
    function typeForCheck(text, carryOn) {
        body.forceActiveFocus()
        if (!carryOn) body.cursorPosition = 0
        for (var i = 0; i < text.length; i++) {
            if (text[i] === "\n") { if (!blockMarkdown()) body.insert(body.cursorPosition, "<br>") }
            else { body.insert(body.cursorPosition, text[i] === " " ? "\u00a0" : esc(text[i])); inlineMarkdown() }
        }
    }
    // Done with the message (sent, saved or thrown away).
    function finish() { if (detached) putBack(); else if (inline) fromInline(); else close() }

    // The writing area (see RichEditor).
    property alias markdown: editor.markdown
    readonly property alias bodyArea: editor.area
    readonly property alias body: editor.area
    readonly property alias pickers: editor.pickers
    readonly property alias fromMenu: fromBox
    readonly property alias toBox: toRow
    property alias spellOn: editor.spellOn
    property alias grammarOn: editor.grammarOn
    property alias lints: editor.lints
    property alias ignored: editor.ignored
    property alias colourBack: editor.colourBack
    function esc(t) { return editor.esc(t) }
    function inlineMarkdown() { editor.inlineMarkdown() }
    function blockMarkdown() { return editor.blockMarkdown() }
    function nestList(deeper) { return editor.nestList(deeper) }
    function splitQuote() { return editor.splitQuote() }
    function refreshCode() { editor.refreshCode() }
    function setSpelling(on) { editor.setSpelling(on) }
    function setGrammar(on) { editor.setGrammar(on) }
    function setBodyHtml(html) { editor.setBodyHtml(html) }
    width: large ? parent.width - 24 : Math.max(420, Math.min(wantW, parent.width - 24))
    height: large ? parent.height - Theme.toolbarHeight - 12 : Math.max(320, Math.min(wantH, parent.height - Theme.toolbarHeight - 12))
    modal: true
    focus: true
    padding: 0
    // Closing asks first when something has been written (see requestClose).
    closePolicy: Popup.NoAutoClose
    background: Rectangle {
        color: Theme.window
        radius: 12
        border.color: Theme.border
        Rectangle { width: parent.width; height: 12; color: Theme.window }
    }
    enter: Transition { NumberAnimation { property: "y"; from: Theme.toolbarHeight - sheet.height; to: Theme.toolbarHeight; duration: 180; easing.type: Easing.OutCubic } }
    exit: Transition { NumberAnimation { property: "y"; from: Theme.toolbarHeight; to: Theme.toolbarHeight - sheet.height; duration: 150; easing.type: Easing.InCubic } }

    function escapeHtml(t) { return (t || "").replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/\n/g, "<br>") }

    /// m: { to, cc, bcc, subject, body, html, draftId, account, attachments, inReplyTo }
    /// host: an item to write in (under a message) instead of the sheet.
    function openWith(m, host) {
        if (detached) { ownWindow.raise(); ownWindow.requestActivate(); store.statusText = "Finish the message in its own window first."; return }
        // One message at a time: the one being written under a message goes to Drafts.
        if (inline) leaveInline()
        var prefs = JSON.parse(store.settings())
        markdown = prefs.markdown_compose !== false
        spellOn = prefs.spell_check !== false
        grammarOn = prefs.grammar_check !== false
        lints = []; ignored = []
        senders = JSON.parse(store.senders())
        var acct = m.account === undefined ? (senders.length ? senders[0].id : "") : m.account
        var idx = 0
        for (var i = 0; i < senders.length; i++) if (senders[i].id === acct) idx = i
        fromBox.currentIndex = idx
        toRow.field.text = m.to || ""
        ccRow.field.text = m.cc || ""
        bccRow.field.text = m.bcc || ""
        showBcc = !!m.bcc
        subjectRow.field.text = m.subject || ""
        attachments = m.attachments || []
        forwardOf = -1
        inReplyTo = m.inReplyTo || ""
        draftId = m.draftId === undefined ? null : m.draftId
        // New messages and replies get the account's signature (not re-opened drafts).
        var sig = (m.draftId === undefined && m.html === undefined) ? store.signature(senders.length ? senders[idx].id : "") : ""
        lastSignature = sig
        if (m.html) setBodyHtml(m.html)
        // Replies and forwards: room to write, the signature, then the original as it looked.
        else if (m.quoteHtml) setBodyHtml("<p><br></p>" + (sig ? "<p>-- <br>" + escapeHtml(sig) + "</p>" : "") + "<p><br></p>" + m.quoteHtml)
        else { colourBack = {}; body.text = (sig ? "<br><br>-- <br>" + escapeHtml(sig) : "") + escapeHtml(m.body || "") }
        if (host) toInline(host)
        else open()
        savedSnapshot = snapshot()
        if (toRow.field.text === "") toRow.field.forceActiveFocus()
        else { body.forceActiveFocus(); body.cursorPosition = 0 }
    }
    // (Tables, code blocks among them, have marks of their own in the editor's text: line breaks in the plain version.)
    function plainText() { return body.getText(0, body.length).replace(/\u200b/g, "").replace(/[\ufdd0\ufdd1]/g, "\n") }
    function payload() {
        var p = { to: toRow.field.text, cc: ccRow.field.text, bcc: bccRow.field.text, subject: subjectRow.field.text,
                  body: plainText(), html: store.restoreColours(body.text.replace(/\u200b|&#8203;/g, ""), JSON.stringify(colourBack)), attachments: attachments, inReplyTo: inReplyTo,
                  account: senders.length ? senders[fromBox.currentIndex].id : "" }
        if (draftId !== null) p.draftId = draftId
        return p
    }
    function send(atMs) {
        var p = payload()
        if (atMs) p.sendAt = atMs
        store.sendMessage(JSON.stringify(p))
        finish()
    }
    property bool showBcc: false


    // Something written that would be lost (the signature alone doesn't count).
    function hasContent() {
        var text = plainText().replace(lastSignature || "", "").replace(/--\s*$/m, "").trim()
        return toRow.field.text.trim() !== "" || subjectRow.field.text.trim() !== "" || text !== "" || attachments.length > 0
    }
    // The header's buttons (also in its » menu when the bar is narrow).
    readonly property bool headTight: pane.width < 470
    function attachFiles() { attachDialog.createObject(sheet).open() }
    function biggerOrSmaller() { if (inline) inlineToSheet(); else large = !large }
    function windowOrBack() { if (detached) { putBack(); open() } else popOut() }
    function saveAndClose() { draftId = store.saveDraft(JSON.stringify(payload())); finish() }
    // The trash button: asks first when something has been written.
    function discard() {
        // (A reply nobody has typed in yet goes without asking.)
        if (hasContent() && snapshot() !== savedSnapshot) confirm.ask("Throw away this message?", "What you've written will be lost.", "Discard", function() { sheet.finish() })
        else finish()
    }
    function requestClose() {
        if (!hasContent()) { finish(); return }
        // Already saved as it is now: nothing to ask.
        if (draftId !== null && snapshot() === savedSnapshot) { finish(); return }
        confirm.ask("Save this message as a draft?", "You can finish it later from Drafts.", "Save Draft",
                    function() { sheet.draftId = sheet.store.saveDraft(JSON.stringify(sheet.payload())); sheet.finish() },
                    "Don't Save", function() { sheet.finish() })
    }
    // Its questions and pickers appear over the writing area, wherever it is
    // (the sheet, its own window, or under a message).
    Confirm { id: confirm; parent: pane }
    // Drafts save themselves while you write (as in Apple Mail): every half
    // minute, when something changed. Each save replaces the previous copy.
    property string savedSnapshot: ""
    function snapshot() { return JSON.stringify([toRow.field.text, ccRow.field.text, bccRow.field.text, subjectRow.field.text, body.text, attachments]) }
    function saveDraftNow() {
        draftId = store.saveDraft(JSON.stringify(payload()))
        savedSnapshot = snapshot()
    }
    Timer {
        interval: 30000
        repeat: true
        running: sheet.active
        onTriggered: if (sheet.hasContent() && sheet.snapshot() !== sheet.savedSnapshot) sheet.saveDraftNow()
    }
    // Drag the bottom-right corner to resize.
    MouseArea {
        // On the sheet itself (not inside its column layout); the separate
        // window has the system's own resize edges.
        parent: holder.parent ? holder.parent : sheet.background
        visible: !sheet.detached
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        width: 18; height: 18
        z: 100
        cursorShape: Qt.SizeFDiagCursor
        property point start
        property real w0
        property real h0
        onPressed: function(ev) { start = mapToItem(null, ev.x, ev.y); w0 = sheet.width; h0 = sheet.height; sheet.large = false }
        onPositionChanged: function(ev) {
            var p = mapToItem(null, ev.x, ev.y)
            // Centred horizontally, so it grows to both sides.
            sheet.wantW = w0 + 2 * (p.x - start.x)
            sheet.wantH = h0 + (p.y - start.y)
        }
        Icon { anchors.right: parent.right; anchors.bottom: parent.bottom; anchors.margins: 3; width: 11; height: 11; name: "resizeGrip"; color: Theme.tertiary }
    }
    Shortcut { context: Qt.ApplicationShortcut; enabled: sheet.active && !confirm.opened && !laterPicker.opened && !editor.asking; sequence: "Escape"; onActivated: sheet.requestClose() }
    Shortcut { context: Qt.ApplicationShortcut; enabled: sheet.active; sequence: "Ctrl+Return"; onActivated: sheet.send(0) }
    Shortcut { context: Qt.ApplicationShortcut; enabled: sheet.active; sequence: "Ctrl+S"; onActivated: sheet.saveDraftNow() }

    Component {
        id: attachDialog
        FileDialog {
            title: "Attach Files"
            fileMode: FileDialog.OpenFiles
            onAccepted: {
                var list = sheet.attachments.slice()
                for (var i = 0; i < selectedFiles.length; i++) {
                    var u = selectedFiles[i].toString()
                    list.push(decodeURIComponent(u.replace(/^file:\/\//, "").replace(/^\/([A-Za-z]:)/, "$1")))
                }
                sheet.attachments = list
                destroy()
            }
            onRejected: destroy()
        }
    }

    component HeaderField: RowLayout {
        property alias label: lbl.text
        property alias field: tf
        Layout.fillWidth: true
        Layout.leftMargin: 16
        Layout.rightMargin: 16
        Layout.preferredHeight: 34
        Text { id: lbl; color: Theme.secondary; Layout.preferredWidth: 64; font.pixelSize: Theme.base }
        FieldX {
            id: tf
            Layout.fillWidth: true
            background: null
            font.pixelSize: Theme.base
            color: Theme.text
            selectByMouse: true
        }
    }

    // To / Cc / Bcc: addresses as chips.
    component AddressRow: RowLayout {
        property alias label: albl.text
        property alias field: abox
        Layout.fillWidth: true
        Layout.leftMargin: 16
        Layout.rightMargin: 16
        Layout.preferredHeight: abox.implicitHeight
        Text { id: albl; color: Theme.secondary; Layout.preferredWidth: 64; Layout.alignment: Qt.AlignTop; topPadding: 9; font.pixelSize: Theme.base }
        AddressBox { id: abox; Layout.fillWidth: true }
    }


    contentItem: Item {
      id: holder
      ColumnLayout {
        id: pane
        width: parent ? parent.width : 0
        height: parent ? parent.height : 0
        spacing: 0
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: sheet.detached ? Theme.toolbarHeight : sheet.inline ? 42 : 48
            color: Theme.toolbar
            // In its own window: drag the bar to move it, double-click to zoom.
            DragHandler {
                enabled: sheet.detached
                target: null
                grabPermissions: PointerHandler.CanTakeOverFromItems | PointerHandler.CanTakeOverFromHandlersOfDifferentType
                onActiveChanged: if (active) ownWindow.startSystemMove()
            }
            TapHandler {
                enabled: sheet.detached
                gesturePolicy: TapHandler.DragThreshold
                onDoubleTapped: ownWindow.visibility === Window.Maximized ? ownWindow.showNormal() : ownWindow.showMaximized()
            }
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: sheet.detached && Qt.platform.os === "osx" ? 78 : 12
                anchors.rightMargin: sheet.detached && Qt.platform.os !== "osx" ? 0 : 12
                ToolButtonX {
                    id: sendButton
                    icon: "send"; text: "Send"; primary: true
                    tip: "Send (Ctrl+Return)"
                    enabled: toRow.field.text.trim() !== ""
                    onClicked: sheet.send(0)
                }
                ToolButtonX {
                    id: laterBtn
                    icon: "chevronDown"; tip: "Send Later"
                    enabled: toRow.field.text.trim() !== ""
                    onClicked: laterMenu.open(laterBtn)
                    P.Menu {
                        id: laterMenu
                        P.MenuItem { icon.name: "chronometer"; text: "Send in 1 Hour"; onTriggered: sheet.send(Date.now() + 3600000) }
                        P.MenuItem { icon.name: "chronometer"; text: "Send Tonight 9:00 PM"; onTriggered: { var d = new Date(); d.setHours(21, 0, 0, 0); if (d < new Date()) d = Theme.addDays(d, 1); sheet.send(d.getTime()) } }
                        P.MenuItem { icon.name: "chronometer"; text: "Send Tomorrow 8:00 AM"; onTriggered: { var d = Theme.addDays(new Date(), 1); d.setHours(8, 0, 0, 0); sheet.send(d.getTime()) } }
                        P.MenuItem { icon.name: "view-calendar-day"; text: "Send Later…"; onTriggered: laterPicker.open() }
                    }
                }
                Text {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    horizontalAlignment: Text.AlignHCenter
                    text: subjectRow.field.text ? subjectRow.field.text : "New Message"
                    font.weight: Font.DemiBold
                    color: Theme.text
                    elide: Text.ElideRight
                }
                ToolButtonX { visible: !sheet.headTight; icon: "paperclip"; tip: "Attach Files"; onClicked: sheet.attachFiles() }
                ToolButtonX {
                    visible: !sheet.detached && !sheet.headTight
                    icon: sheet.large && !sheet.inline ? "winRestore" : "winMaximize"
                    tip: sheet.inline ? "Larger (over the window)" : sheet.large ? "Smaller" : "Larger"
                    onClicked: sheet.biggerOrSmaller()
                }
                ToolButtonX {
                    visible: !sheet.headTight
                    icon: "popOut"
                    tip: sheet.detached ? "Back into the main window" : "Open in a separate window"
                    onClicked: sheet.windowOrBack()
                }
                ToolButtonX { visible: !sheet.headTight; text: "Save Draft"; tip: "Ctrl+S"; onClicked: sheet.saveAndClose() }
                ToolButtonX { visible: !sheet.headTight; icon: "trash"; tip: "Throw away this message"; onClicked: sheet.discard() }
                ToolButtonX {
                    id: headMore
                    visible: sheet.headTight
                    text: "»"; tip: "More"
                    onClicked: headMenu.open(headMore)
                    P.Menu {
                        id: headMenu
                        P.MenuItem { icon.name: "mail-attachment"; text: "Attach Files…"; onTriggered: sheet.attachFiles() }
                        P.MenuItem { icon.name: "view-fullscreen"; visible: !sheet.detached; text: sheet.inline ? "Larger (over the window)" : sheet.large ? "Smaller" : "Larger"; onTriggered: sheet.biggerOrSmaller() }
                        P.MenuItem { icon.name: "window-new"; text: sheet.detached ? "Back into the Main Window" : "Open in a Separate Window"; onTriggered: sheet.windowOrBack() }
                        P.MenuItem { icon.name: "document-save"; text: "Save Draft"; onTriggered: sheet.saveAndClose() }
                        P.MenuItem { icon.name: "edit-delete"; text: "Throw Away…"; onTriggered: sheet.discard() }
                    }
                }
                WindowControls { visible: sheet.detached && Qt.platform.os !== "osx"; window: ownWindow; Layout.leftMargin: 6; Layout.fillHeight: true }
            }
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border }
        }

        AddressRow { id: toRow; label: "To:" }
        Rectangle { Layout.fillWidth: true; Layout.leftMargin: 16; Layout.preferredHeight: 1; color: Theme.borderSoft }
        RowLayout {
            Layout.fillWidth: true
            spacing: 0
            AddressRow { id: ccRow; label: "Cc:" }
            Text {
                visible: !sheet.showBcc
                text: "Bcc"; color: Theme.accent; font.pixelSize: 12
                Layout.rightMargin: 16
                MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: sheet.showBcc = true }
            }
        }
        Rectangle { Layout.fillWidth: true; Layout.leftMargin: 16; Layout.preferredHeight: 1; color: Theme.borderSoft }
        AddressRow { id: bccRow; label: "Bcc:"; visible: sheet.showBcc }
        Rectangle { visible: sheet.showBcc; Layout.fillWidth: true; Layout.leftMargin: 16; Layout.preferredHeight: 1; color: Theme.borderSoft }
        HeaderField { id: subjectRow; label: "Subject:" }
        Rectangle { Layout.fillWidth: true; Layout.leftMargin: 16; Layout.preferredHeight: 1; color: Theme.borderSoft }
        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 16
            Layout.rightMargin: 16
            Layout.preferredHeight: 38
            Text { text: "From:"; color: Theme.secondary; Layout.preferredWidth: 64; font.pixelSize: Theme.base }
            ComboX {
                id: fromBox
                Layout.fillWidth: true
                Layout.minimumWidth: 120
                model: sheet.senders.map(function(s) { return s.label })
                font.pixelSize: Theme.base
                onActivated: {
                    // Swap the signature for the new account's.
                    var sig = sheet.store.signature(sheet.senders[currentIndex].id)
                    var plain = sheet.plainText()
                    if (sheet.lastSignature && plain.indexOf(sheet.lastSignature) >= 0) {
                        body.text = body.text.replace(sheet.escapeHtml(sheet.lastSignature), sheet.escapeHtml(sig))
                    } else if (sig) {
                        body.text = body.text + "<br><br>-- <br>" + sheet.escapeHtml(sig)
                    }
                    sheet.lastSignature = sig
                }
            }
        }
        Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border }

        RichEditor {
            id: editor
            Layout.fillWidth: true
            Layout.fillHeight: true
            store: sheet.store
            active: sheet.active
            popupParent: pane
        }
        // Attachments
        Flow {
            visible: sheet.attachments.length > 0
            Layout.fillWidth: true
            Layout.margins: 12
            spacing: 8
            Repeater {
                model: sheet.attachments
                delegate: Rectangle {
                    required property string modelData
                    required property int index
                    width: chip.implicitWidth + 16
                    height: 30
                    radius: 8
                    color: Theme.segment
                    Row {
                        id: chip
                        anchors.centerIn: parent
                        spacing: 6
                        Icon { name: "paperclip"; width: 14; height: 14; color: Theme.secondary; anchors.verticalCenter: parent.verticalCenter }
                        Text { text: modelData.split(/[\\/]/).pop(); color: Theme.text; font.pixelSize: 12; anchors.verticalCenter: parent.verticalCenter }
                        Icon {
                            name: "close"; width: 12; height: 12; color: Theme.secondary; anchors.verticalCenter: parent.verticalCenter
                            MouseArea { anchors.fill: parent; onClicked: { var l = sheet.attachments.slice(); l.splice(index, 1); sheet.attachments = l } }
                        }
                    }
                }
            }
        }
      }
    }

    // Custom Send Later time
    Popup {
        id: laterPicker
        parent: pane
        anchors.centerIn: parent
        modal: true
        padding: 18
        background: Rectangle { radius: 12; color: Theme.popup; border.color: Theme.border }
        onOpened: { var d = new Date(Date.now() + 3600000); laterDate.text = Theme.isoDate(d); laterTime.value = Theme.pad2(d.getHours()) + ":00" }
        contentItem: ColumnLayout {
            spacing: 10
            Text { text: "Send Later"; font.pixelSize: 15; font.weight: Font.Bold; color: Theme.text }
            RowLayout {
                DateField { id: laterDate; Layout.preferredWidth: 160 }
                TimeField { id: laterTime }
            }
            RowLayout {
                Item { Layout.fillWidth: true }
                ToolButtonX { text: "Cancel"; onClicked: laterPicker.close() }
                ToolButtonX {
                    text: "Schedule"; primary: true
                    // Only a time still to come (a past one would send it at once).
                    enabled: { var t = Theme.parseDT(laterDate.text + "T" + laterTime.value); return !!t && t.getTime() > Date.now() }
                    onClicked: { laterPicker.close(); sheet.send(Theme.parseDT(laterDate.text + "T" + laterTime.value).getTime()) }
                }
            }
        }
    }


    // Address suggestions from Contacts while typing in To/Cc/Bcc.
    Popup {
        id: suggest
        parent: pane
        property var target: null
        property var items: []
        property int current: 0
        onItemsChanged: current = 0
        function pick(i) {
            var it = items[i]
            if (!it || !target) return
            var t = target
            var parts = t.text.split(",")
            parts[parts.length - 1] = (parts.length > 1 ? " " : "") + it.name + " <" + it.email + ">"
            t.text = parts.join(",") + ", "
            items = []
            t.forceActiveFocus()
            t.cursorPosition = t.text.length
        }
        x: 80
        y: target ? target.mapToItem(suggest.parent, 0, target.height).y + 2 : 0
        width: 380
        height: Math.min(items.length, 6) * 30 + 8
        padding: 4
        visible: items.length > 0 && target !== null && target.activeFocus
        closePolicy: Popup.NoAutoClose
        background: Rectangle { radius: 8; color: Theme.popup; border.color: Theme.border }
        ListView {
            anchors.fill: parent
            clip: true
            model: suggest.items
            currentIndex: suggest.current
            delegate: Rectangle {
                required property var modelData
                required property int index
                width: ListView.view.width
                height: 30
                radius: 5
                color: index === suggest.current ? Theme.accent : (hov.containsMouse ? Theme.accentSoft : "transparent")
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    x: 8
                    width: parent.width - 16
                    elide: Text.ElideRight
                    text: modelData.name + "  <" + modelData.email + ">"
                    color: index === suggest.current ? Theme.accentInk : Theme.text
                    font.pixelSize: Theme.base
                }
                MouseArea {
                    id: hov
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: suggest.pick(index)
                }
            }
        }
    }
    function refreshSuggest(t) {
        var parts = t.text.split(",")
        var last = parts[parts.length - 1].trim()
        suggest.target = t
        suggest.items = last.length >= 1 && last.indexOf("<") < 0 ? JSON.parse(store.emailSuggestions(last)) : []
    }
    // Up / Down to choose a suggestion, Enter or Tab to take it, Escape to close the list.
    Shortcut { enabled: suggest.visible; context: Qt.ApplicationShortcut; sequence: "Down"; onActivated: suggest.current = Math.min(suggest.items.length - 1, suggest.current + 1) }
    Shortcut { enabled: suggest.visible; context: Qt.ApplicationShortcut; sequence: "Up"; onActivated: suggest.current = Math.max(0, suggest.current - 1) }
    Shortcut { enabled: suggest.visible; context: Qt.ApplicationShortcut; sequences: ["Return", "Enter", "Tab"]; onActivated: suggest.pick(suggest.current) }
    Connections { target: toRow.field; function onTextEdited() { sheet.refreshSuggest(toRow.field) } }
    Connections { target: ccRow.field; function onTextEdited() { sheet.refreshSuggest(ccRow.field) } }
    Connections { target: bccRow.field; function onTextEdited() { sheet.refreshSuggest(bccRow.field) } }
}
