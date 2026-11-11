import QtQuick
import QtQuick.Layouts
import QtQuick.Controls.Basic

// Rounded search box with a magnifier and a clear button.
Rectangle {
    id: field
    // The whole search, as typed (the one thing others read and set). With
    // `tokens`, its finished operators (to:jane, is:unread …) show as tokens
    // and only what's being typed is in the text field; the rest of the time
    // it's all in the text field.
    property string text: ""
    property alias cursorPosition: input.cursorPosition
    property bool tokens: false
    // (op, value) → a friendlier value for a token (a person's name for an address), or "".
    property var labelFor: null
    // The finished parts, in order: {raw, op ("" for words), neg, label}.
    property var items: []
    // A token chosen with the keys (-1: none; -2: all, after Select All).
    property int picked: -1
    property bool syncing: false
    readonly property var ops: ["from", "to", "cc", "bcc", "subject", "is", "has", "in", "label", "after", "before", "older_than", "newer_than", "older", "newer",
                                "filename", "list", "deliveredto", "rfc822msgid", "larger", "smaller", "size"]
    function compose() {
        var parts = items.map(function(i) { return i.raw })
        if (input.text.trim() !== "" || parts.length === 0) parts.push(input.text)
        return parts.join(" ")
    }
    function hasOps(q) { return new RegExp("(^|[\\s(-])(" + ops.join("|") + "):", "i").test(q) }
    // A search split where it can be (spaces outside quotes and brackets);
    // `rest`: the last part when it's still being typed (no space after it).
    function split(q) {
        var segs = [], cur = "", depth = 0, quote = false
        for (var i = 0; i < q.length; i++) {
            var c = q[i]
            if (c === '"') quote = !quote
            else if (!quote && c === "(") depth++
            else if (!quote && c === ")") depth = Math.max(0, depth - 1)
            if (/\s/.test(c) && !quote && depth === 0) { if (cur !== "") segs.push(cur); cur = "" }
            else cur += c
        }
        return { segs: segs, rest: cur }
    }
    function make(raw) {
        var m = new RegExp("^(-?)(" + ops.join("|") + "):(.*)$", "i").exec(raw)
        if (!m) return { raw: raw, op: "", neg: false, label: raw }
        return { raw: raw, op: m[2].toLowerCase(), neg: m[1] === "-", label: friendly(m[2].toLowerCase(), m[3], m[1] === "-") }
    }
    function friendly(op, v, neg) {
        var bare = v.replace(/^"(.*)"?$/, "$1").replace(/"$/, "")
        var lv = bare.toLowerCase()
        var names = { from: "From", to: "To", cc: "Cc", bcc: "Bcc", subject: "Subject", label: "Mailbox", "in": "In", filename: "Attachment", list: "List",
                      deliveredto: "Delivered to", rfc822msgid: "Message ID", after: "After", before: "Before", newer: "After", older: "Before",
                      older_than: "Older than", newer_than: "Newer than", larger: "Larger than", smaller: "Smaller than", size: "Size" }
        var out
        if (op === "is") out = ({ unread: "Unread", read: "Read", starred: "Flagged", flagged: "Flagged", important: "VIP", vip: "VIP", snoozed: "Snoozed" })[lv] || ("Is: " + bare)
        else if (op === "has") out = lv === "attachment" || lv === "attachments" ? "Has attachment" : /-(star|flag)$/.test(lv) ? "Flag: " + lv.replace(/-(star|flag)$/, "").replace(/^./, function(c) { return c.toUpperCase() }) : "Has: " + bare
        else if (op === "in") out = "In: " + (lv === "anywhere" ? "All mailboxes" : lv === "spam" ? "Junk" : bare.replace(/^./, function(c) { return c.toUpperCase() }))
        else if (["after", "before", "newer", "older"].indexOf(op) >= 0) {
            var d = new Date(bare.replace(/\//g, "-") + "T00:00:00")
            out = names[op] + ": " + (isNaN(d.getTime()) ? bare : d.toLocaleDateString(Qt.locale(), "MMM d, yyyy"))
        } else if (op === "older_than" || op === "newer_than") {
            var t = /^(\d+)([dmy])$/.exec(lv)
            out = names[op] + ": " + (t ? t[1] + " " + ({ d: "day", m: "month", y: "year" })[t[2]] + (t[1] === "1" ? "" : "s") : bare)
        } else {
            var nice = field.labelFor ? field.labelFor(op, bare) : ""
            out = names[op] + ": " + (nice || bare)
        }
        if (!neg) return out
        return /^Has /.test(out) ? out.replace(/^Has /, "No ") : "Not " + out.replace(/^./, function(c) { return c.toLowerCase() })
    }
    // A whole search set from outside: its finished parts made tokens.
    function load(q) {
        syncing = true
        picked = -1
        if (!tokens || !hasOps(q)) { items = []; input.text = q }
        else {
            var s = split(q)
            items = s.segs.map(make)
            input.text = s.rest
        }
        syncing = false
    }
    // What's typed: once it has operators, its finished parts become tokens.
    function absorb() {
        if (!tokens || input.cursorPosition !== input.text.length) return
        if (!hasOps(compose())) return
        var s = split(input.text)
        if (!s.segs.length) return
        syncing = true
        items = items.concat(s.segs.map(make))
        input.text = s.rest
        syncing = false
    }
    function push() {
        // (No operators left: back to plain text.)
        if (items.length && !items.some(function(i) { return i.op !== "" })) {
            var all = compose()
            syncing = true
            items = []
            input.text = all
            input.cursorPosition = all.length
            syncing = false
        }
        var q = compose()
        if (q !== text) { syncing = true; text = q; syncing = false }
    }
    onTextChanged: if (!syncing && text !== compose()) load(text)
    // A token clicked: it and what follows back into the text, to edit.
    function unwrap(i) {
        var tail = items.slice(i).map(function(x) { return x.raw }).join(" ")
        var cursor = items[i].raw.length
        syncing = true
        items = items.slice(0, i)
        input.text = tail + (input.text !== "" ? " " + input.text : "")
        syncing = false
        picked = -1
        input.forceActiveFocus()
        input.cursorPosition = cursor
        push()
    }
    function removeItem(i) {
        var v = items.slice()
        v.splice(i, 1)
        items = v
        picked = -1
        push()
    }
    // Keys at the text's start: across and out of the tokens.
    function tokenKey(ev) {
        if (!tokens || !items.length) return false
        var atStart = input.cursorPosition === 0 && input.selectedText === ""
        if (picked === -2 && (ev.key === Qt.Key_Backspace || ev.key === Qt.Key_Delete)) { items = []; input.text = ""; picked = -1; push(); return true }
        if ((ev.key === Qt.Key_Backspace || ev.key === Qt.Key_Delete) && picked >= 0) { removeItem(picked); return true }
        if (ev.key === Qt.Key_Backspace && atStart) {
            var last = items[items.length - 1]
            if (last.op === "") { unwrapLastWord(); return true }
            picked = items.length - 1
            return true
        }
        if (ev.key === Qt.Key_Left && (atStart || picked >= 0)) { picked = picked < 0 ? items.length - 1 : Math.max(0, picked - 1); return true }
        if (ev.key === Qt.Key_Right && picked >= 0) { picked = picked + 1 >= items.length ? -1 : picked + 1; return true }
        if ((ev.key === Qt.Key_Return || ev.key === Qt.Key_Enter) && picked >= 0) { unwrap(picked); return true }
        if (ev.matches(StandardKey.SelectAll)) { input.selectAll(); picked = -2; return true }
        if (ev.matches(StandardKey.Copy) && picked === -2) { copier.text = compose(); copier.selectAll(); copier.copy(); return true }
        if (picked !== -1 && ev.text !== "") picked = -1
        return false
    }
    function unwrapLastWord() {
        var w = items[items.length - 1].raw
        syncing = true
        items = items.slice(0, items.length - 1)
        input.text = w + (input.text !== "" ? " " + input.text : "")
        input.cursorPosition = w.length
        syncing = false
        push()
    }
    // (For copying the whole search as text.)
    TextInput { id: copier; visible: false }
    // Keys looked at first (a list of suggestions): true when it took the key.
    property var keyFilter: null
    property string placeholderText: "Search"
    readonly property bool hasFocus: input.activeFocus || popField.activeFocus
    // On a very narrow window only the magnifier shows; it opens the field.
    property bool collapsed: false
    // Still searching: a small spinner in place of the magnifier.
    property bool busy: false
    // Escape pressed, or the clear button clicked (the text is cleared too).
    signal escapePressed()
    signal clearClicked()
    function focusField() {
        if (collapsed) { pop.open(); popField.forceActiveFocus(); popField.selectAll() }
        else { input.forceActiveFocus(); input.selectAll() }
    }

    implicitWidth: 200
    implicitHeight: 28
    Layout.preferredHeight: 28
    Layout.alignment: Qt.AlignVCenter
    radius: 7
    clip: true
    color: collapsed ? (openArea.containsMouse || field.text !== "" ? Theme.hover : "transparent") : (input.activeFocus ? Theme.field : Theme.fieldIdle)
    border.color: input.activeFocus ? Theme.accent : "transparent"
    border.width: input.activeFocus ? 2 : 0

    Spinner {
        id: busySpinner
        visible: !field.collapsed
        running: field.busy
        width: 15; height: 15
        anchors.left: parent.left
        anchors.leftMargin: 8
        anchors.verticalCenter: parent.verticalCenter
    }
    Icon {
        // (The spinner in its place while searching, once it shows.)
        opacity: busySpinner.shown ? 0 : 1
        Behavior on opacity { NumberAnimation { duration: 150 } }
        name: "search"
        width: 15; height: 15
        color: Theme.tertiary
        anchors.left: parent.left
        anchors.leftMargin: 9
        anchors.verticalCenter: parent.verticalCenter
    }
    // Search operators (to:, is:unread, subject:"…") shown as such: a tinted
    // bubble behind each (red when it's a "not"); while one's value is still
    // being typed, just its operator. The text itself stays what's typed.
    readonly property var bubbles: { input.text; input.width; input.cursorPosition; return field.findBubbles() }
    function findBubbles() {
        var t = input.text, out = [], m
        var re = /(^|[\s(])(-?)(from|to|cc|bcc|subject|is|has|in|label|after|before|older_than|newer_than|older|newer|filename|list|deliveredto|rfc822msgid|larger|smaller|size):("[^"]*"?|\([^)]*\)?|[^\s()]*)/gi
        while ((m = re.exec(t)) !== null) {
            var start = m.index + m[1].length
            var opEnd = start + m[2].length + m[3].length + 1
            var end = opEnd + m[4].length
            var quoted = /^"/.test(m[4]), grouped = /^\(/.test(m[4])
            // Done: a closed quote or bracket, or something after it (or the cursor elsewhere).
            var done = m[4].length > 0 && (quoted ? (m[4].length > 1 && /"$/.test(m[4])) : grouped ? /\)$/.test(m[4]) : (end < t.length || input.cursorPosition < end || !input.activeFocus))
            var a = input.positionToRectangle(start), b = input.positionToRectangle(done ? end : opEnd)
            out.push({ x: a.x, w: Math.max(0, b.x - a.x), neg: m[2] === "-" })
            if (re.lastIndex === m.index) re.lastIndex++
        }
        return out
    }
    Item {
        x: input.x; y: input.y; width: input.width; height: input.height
        visible: input.visible
        clip: true
        Repeater {
            model: field.bubbles
            delegate: Rectangle {
                required property var modelData
                x: modelData.x - 3
                width: modelData.w + 6
                height: Math.min(parent.height + 2, input.contentHeight + 5)
                anchors.verticalCenter: parent.verticalCenter
                radius: 5
                readonly property color tint: modelData.neg ? Theme.red : Theme.accent
                color: Qt.rgba(tint.r, tint.g, tint.b, 0.16)
                border.color: Qt.rgba(tint.r, tint.g, tint.b, 0.35)
                border.width: 1
            }
        }
    }
    // The tokens, then the text being typed (moved along to keep it in view).
    Item {
        id: lane
        visible: !field.collapsed
        anchors.left: parent.left
        anchors.leftMargin: 30
        anchors.right: clear.visible ? clear.left : parent.right
        anchors.rightMargin: 6
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        clip: true
        readonly property real tokensWidth: tokenRow.width + (field.items.length ? 4 : 0)
        readonly property real shift: Math.max(0, tokensWidth + 60 - width)
        Row {
            id: tokenRow
            x: -lane.shift
            anchors.verticalCenter: parent.verticalCenter
            spacing: 4
            Repeater {
                model: field.items
                delegate: Rectangle {
                    id: tok
                    required property var modelData
                    required property int index
                    readonly property bool isOp: modelData.op !== ""
                    readonly property bool chosen: field.picked === index || field.picked === -2
                    readonly property color tint: modelData.neg ? Theme.red : Theme.accent
                    height: 20
                    width: label.implicitWidth + (isOp ? 14 : 0)
                    radius: 5
                    color: !isOp ? "transparent" : chosen ? tint : Qt.rgba(tint.r, tint.g, tint.b, tokMouse.containsMouse ? 0.26 : 0.16)
                    border.color: isOp ? Qt.rgba(tint.r, tint.g, tint.b, 0.4) : "transparent"
                    border.width: isOp ? 1 : 0
                    Text {
                        id: label
                        anchors.centerIn: parent
                        text: tok.modelData.label
                        font.pixelSize: tok.isOp ? Theme.base - 1 : Theme.base
                        color: tok.isOp && tok.chosen ? "white" : Theme.text
                        font.strikeout: false
                    }
                    MouseArea { id: tokMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: field.unwrap(tok.index) }
                    ToolTip.visible: tok.isOp && tokMouse.containsMouse
                    ToolTip.delay: 500
                    ToolTip.text: tok.modelData.raw + "  (click to change)"
                    Accessible.role: Accessible.Button
                    Accessible.name: tok.modelData.label
                }
            }
        }
    }
    TextInput {
        id: input
        visible: !field.collapsed
        anchors.left: lane.left
        anchors.leftMargin: lane.tokensWidth - lane.shift
        anchors.right: lane.right
        anchors.verticalCenter: parent.verticalCenter
        onTextChanged: { if (field.syncing) return; field.picked = -1; field.absorb(); field.push() }
        font.pixelSize: Theme.base
        color: Theme.text
        selectionColor: Theme.accentFill
        selectedTextColor: Theme.accentInk
        selectByMouse: true
        clip: true
        Keys.onPressed: function(ev) {
            if (field.keyFilter && field.keyFilter(ev.key)) { ev.accepted = true; return }
            if (field.tokenKey(ev)) ev.accepted = true
        }
        Keys.onEscapePressed: { field.load(""); field.push(); field.escapePressed() }
        Text {
            visible: field.text === "" && !input.activeFocus
            text: field.placeholderText
            color: Theme.tertiary
            font: input.font
        }
    }
    Rectangle {
        id: clear
        visible: field.text !== "" && !field.collapsed
        width: 16; height: 16; radius: 8
        color: Theme.tertiary
        anchors.right: parent.right
        anchors.rightMargin: 6
        anchors.verticalCenter: parent.verticalCenter
        Icon { anchors.centerIn: parent; width: 10; height: 10; name: "close"; color: "white"; stroke: 3 }
        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: { field.load(""); field.push(); field.clearClicked() } }
    }
    MouseArea {
        id: openArea
        anchors.fill: parent
        z: field.collapsed ? 1 : -1
        hoverEnabled: field.collapsed
        cursorShape: field.collapsed ? Qt.PointingHandCursor : Qt.IBeamCursor
        onClicked: field.collapsed ? field.focusField() : input.forceActiveFocus()
    }
    Popup {
        id: pop
        y: field.height + 4
        x: Math.min(0, -(240 - field.width))
        width: 240
        padding: 6
        background: Rectangle { radius: 9; color: Theme.popup; border.color: Theme.border }
        FieldX {
            id: popField
            width: parent.width
            placeholderText: field.placeholderText
            text: field.text
            onTextEdited: field.text = text
            Keys.onEscapePressed: pop.close()
            onAccepted: pop.close()
        }
    }
}
