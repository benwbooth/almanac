import QtQuick
import QtQuick.Layouts
import QtQuick.Controls.Basic

// Rounded search box with a magnifier and a clear button.
Rectangle {
    id: field
    property alias text: input.text
    property alias cursorPosition: input.cursorPosition
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
    color: collapsed ? (openArea.containsMouse || input.text !== "" ? Theme.hover : "transparent") : (input.activeFocus ? Theme.field : Theme.fieldIdle)
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
    TextInput {
        id: input
        visible: !field.collapsed
        anchors.left: parent.left
        anchors.leftMargin: 30
        anchors.right: clear.visible ? clear.left : parent.right
        anchors.rightMargin: 6
        anchors.verticalCenter: parent.verticalCenter
        font.pixelSize: Theme.base
        color: Theme.text
        selectionColor: Theme.accentFill
        selectedTextColor: Theme.accentInk
        selectByMouse: true
        clip: true
        Keys.onPressed: function(ev) { if (field.keyFilter && field.keyFilter(ev.key)) ev.accepted = true }
        Keys.onEscapePressed: { text = ""; field.escapePressed() }
        Text {
            visible: input.text === "" && !input.activeFocus
            text: field.placeholderText
            color: Theme.tertiary
            font: input.font
        }
    }
    Rectangle {
        id: clear
        visible: input.text !== "" && !field.collapsed
        width: 16; height: 16; radius: 8
        color: Theme.tertiary
        anchors.right: parent.right
        anchors.rightMargin: 6
        anchors.verticalCenter: parent.verticalCenter
        Icon { anchors.centerIn: parent; width: 10; height: 10; name: "close"; color: "white"; stroke: 3 }
        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: { input.text = ""; field.clearClicked() } }
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
            text: input.text
            onTextEdited: input.text = text
            Keys.onEscapePressed: pop.close()
            onAccepted: pop.close()
        }
    }
}
