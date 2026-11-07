import QtQuick
import QtQuick.Controls.Basic
import Qt.labs.platform as P

// To / Cc / Bcc: each address is a chip (as in Apple Mail) that can be
// clicked to select it, deleted, edited (double-click) or right-clicked;
// what's being typed sits after them. `text` is all of it as one
// comma-separated list, which is what the rest of the message uses.
FocusScope {
    id: box
    property string text: ""
    // Kept so code that places a text cursor can do so (it means "at the end").
    property int cursorPosition: 0
    signal textEdited()

    // The finished addresses, and what's being typed after them.
    property var chips: []
    property alias pending: input.text
    property int selected: -1
    property bool syncing: false

    implicitHeight: Math.max(34, flow.implicitHeight + 8)

    // "Ann <ann@x.com>, bob@y.org" → ["Ann <ann@x.com>", "bob@y.org"] (commas inside quotes kept).
    function split(t) {
        var out = [], cur = "", quoted = false, angle = false
        for (var i = 0; i < t.length; i++) {
            var ch = t[i]
            if (ch === '"') quoted = !quoted
            else if (ch === "<") angle = true
            else if (ch === ">") angle = false
            if ((ch === "," || ch === ";") && !quoted && !angle) { if (cur.trim()) out.push(cur.trim()); cur = ""; continue }
            cur += ch
        }
        return { done: out, rest: cur }
    }
    // The whole list from the chips and what's typed.
    function publish() {
        syncing = true
        var all = chips.slice()
        if (input.text.trim()) all.push(input.text.trim())
        text = all.join(", ") + (input.text === "" && chips.length ? ", " : "")
        syncing = false
    }
    // Set from outside (a reply, a draft, a picked suggestion): everything
    // finished becomes chips; an unfinished last part stays in the typing box.
    onTextChanged: {
        if (syncing) return
        var s = split(text)
        // Addresses put in by the app (not typed) are all finished.
        if (s.rest.trim()) s.done.push(s.rest.trim())
        chips = s.done
        syncing = true
        input.text = ""
        syncing = false
        selected = -1
    }
    // What's typed becomes a chip (at a comma, Enter, Tab or leaving the field).
    function commit() {
        var s = split(input.text + ",")
        if (!s.done.length) return false
        chips = chips.concat(s.done)
        syncing = true
        input.text = ""
        syncing = false
        publish()
        return true
    }
    function removeAt(i) {
        var c = chips.slice()
        c.splice(i, 1)
        chips = c
        selected = Math.min(i, chips.length - 1)
        if (chips.length === 0) selected = -1
        publish()
        input.forceActiveFocus()
    }
    // Editing a chip happens in place: Enter or clicking elsewhere makes it
    // a chip again (empty: it goes), Escape leaves it as it was.
    property int editing: -1
    function editAt(i) {
        selected = -1
        editing = i
    }
    function finishEdit(i, t, keepTyping) {
        if (editing !== i) return
        editing = -1
        var s = split(t + ",")
        var c = chips.slice()
        c.splice.apply(c, [i, 1].concat(s.done))
        chips = c
        publish()
        if (keepTyping) input.forceActiveFocus()
    }
    function cancelEdit() {
        editing = -1
        input.forceActiveFocus()
    }
    // Name and address of a chip, for showing it.
    function nameOf(a) {
        var m = /^\s*"?([^"<]*?)"?\s*<([^>]+)>\s*$/.exec(a)
        return m && m[1] ? m[1] : (m ? m[2] : a)
    }
    function emailOf(a) {
        var m = /<([^>]+)>/.exec(a)
        return m ? m[1].trim() : a.trim()
    }

    TextEdit { id: clip; visible: false }

    MouseArea {
        anchors.fill: parent
        cursorShape: Qt.IBeamCursor
        onClicked: { box.selected = -1; input.forceActiveFocus() }
    }
    Flow {
        id: flow
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        spacing: 4
        Repeater {
            model: box.chips
            delegate: Rectangle {
                id: chip
                required property string modelData
                required property int index
                readonly property bool on: box.selected === index
                readonly property bool editing: box.editing === index
                readonly property bool valid: /^[^\s@<>]+@[^\s@<>]+\.[^\s@<>]+$/.test(box.emailOf(modelData))
                height: 24
                width: editing ? Math.min(flow.width, Math.max(160, editor.contentWidth + 24)) : Math.min(chipText.implicitWidth + 18, flow.width)
                radius: 12
                color: editing ? Theme.field : (on ? Theme.accentFill : (chipHover.hovered ? Theme.hover : Theme.segment))
                border.color: editing ? Theme.accent : (valid ? "transparent" : Theme.red)
                // The address being edited, in the chip's place.
                TextInput {
                    id: editor
                    visible: chip.editing
                    anchors.fill: parent
                    leftPadding: 10; rightPadding: 10
                    verticalAlignment: TextInput.AlignVCenter
                    color: Theme.text
                    font.pixelSize: Theme.base
                    selectByMouse: true
                    selectionColor: Theme.accentFill
                    selectedTextColor: Theme.accentInk
                    clip: true
                    property bool cancelled: false
                    onVisibleChanged: if (visible) { cancelled = false; text = chip.modelData; forceActiveFocus(); selectAll() }
                    Keys.onReturnPressed: box.finishEdit(chip.index, text, true)
                    Keys.onEnterPressed: box.finishEdit(chip.index, text, true)
                    Keys.onEscapePressed: { cancelled = true; box.cancelEdit() }
                    onActiveFocusChanged: if (!activeFocus && chip.editing && !cancelled) box.finishEdit(chip.index, text, false)
                }
                Text {
                    id: chipText
                    visible: !chip.editing
                    anchors.centerIn: parent
                    width: Math.min(implicitWidth, flow.width - 18)
                    elide: Text.ElideRight
                    text: box.nameOf(chip.modelData)
                    color: chip.on ? Theme.accentInk : (chip.valid ? Theme.text : Theme.red)
                    font.pixelSize: Theme.base
                }
                HoverHandler { id: chipHover }
                ToolTip.visible: chipHover.hovered
                ToolTip.delay: 600
                ToolTip.text: chip.valid ? box.emailOf(chip.modelData) : box.emailOf(chip.modelData) + " doesn't look like an email address"
                MouseArea {
                    anchors.fill: parent
                    enabled: !chip.editing
                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                    onClicked: function(ev) {
                        box.selected = chip.index
                        box.forceActiveFocus()
                        keys.forceActiveFocus()
                        if (ev.button === Qt.RightButton) { menu.at = chip.index; menu.open() }
                    }
                    onDoubleClicked: box.editAt(chip.index)
                }
            }
        }
        TextInput {
            id: input
            focus: true
            Keys.onReturnPressed: function(ev) { ev.accepted = box.commit() }
            Keys.onEnterPressed: function(ev) { ev.accepted = box.commit() }
            width: Math.min(flow.width, Math.max(box.chips.length ? 140 : flow.width, contentWidth + 24))
            height: 24
            verticalAlignment: TextInput.AlignVCenter
            color: Theme.text
            font.pixelSize: Theme.base
            selectByMouse: true
            selectionColor: Theme.accentFill
            selectedTextColor: Theme.accentInk
            clip: true
            onTextEdited: {
                box.selected = -1
                // A comma or semicolon finishes an address.
                if (/[,;]/.test(text)) box.commit()
                box.publish()
                box.cursorPosition = box.text.length
                box.textEdited()
            }
            onActiveFocusChanged: if (!activeFocus && text.trim() && !menu.visible) box.commit()
            Keys.onPressed: function(ev) {
                // Backspace at the start: the last chip is selected (again: removed).
                if ((ev.key === Qt.Key_Backspace) && input.cursorPosition === 0 && input.selectedText === "" && box.chips.length) {
                    box.selected = box.chips.length - 1
                    keys.forceActiveFocus()
                    ev.accepted = true
                } else if ((ev.key === Qt.Key_Left) && input.cursorPosition === 0 && box.chips.length) {
                    box.selected = box.chips.length - 1
                    keys.forceActiveFocus()
                    ev.accepted = true
                }
            }
        }
    }
    // Keys while a chip is selected.
    Item {
        id: keys
        focus: false
        Keys.onPressed: function(ev) {
            if (box.selected < 0) return
            if (ev.key === Qt.Key_Backspace || ev.key === Qt.Key_Delete) { box.removeAt(box.selected); ev.accepted = true }
            else if (ev.key === Qt.Key_Left) { box.selected = Math.max(0, box.selected - 1); ev.accepted = true }
            else if (ev.key === Qt.Key_Right) {
                if (box.selected >= box.chips.length - 1) { box.selected = -1; input.forceActiveFocus() } else box.selected++
                ev.accepted = true
            }
            else if (ev.key === Qt.Key_Return || ev.key === Qt.Key_Enter) { box.editAt(box.selected); ev.accepted = true }
            else if (ev.key === Qt.Key_Escape) { box.selected = -1; input.forceActiveFocus(); ev.accepted = true }
            else if (ev.text && ev.text.length && (ev.modifiers === Qt.NoModifier || ev.modifiers === Qt.ShiftModifier)) {
                // Typing goes on after the chips.
                box.selected = -1
                input.forceActiveFocus()
                if (ev.text && ev.text.length) { input.insert(input.cursorPosition, ev.text); box.publish(); box.textEdited() }
                ev.accepted = true
            }
        }
    }
    onActiveFocusChanged: if (!activeFocus) selected = -1
    // Right-click on an address.
    P.Menu {
        id: menu
        property int at: -1
        P.MenuItem { icon.name: "edit-copy"; text: "Copy Address"; onTriggered: { clip.text = box.emailOf(box.chips[menu.at]); clip.selectAll(); clip.copy() } }
        P.MenuItem { icon.name: "document-edit"; text: "Edit Address"; onTriggered: box.editAt(menu.at) }
        P.MenuItem { icon.name: "list-remove"; text: "Remove Address"; onTriggered: box.removeAt(menu.at) }
    }

    // Focus on the box means typing in it.
    onFocusChanged: if (focus && selected < 0) input.forceActiveFocus()
}
