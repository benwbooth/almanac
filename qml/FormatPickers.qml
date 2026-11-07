import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtQuick.Dialogs

// The message editor's font, size, colour and table controls, for the
// formatting bar: each a button that opens its own picker.
//  • Font: every font on the computer, each shown in itself, with a search box.
//  • Size: the usual sizes, each shown at its size (or type any size).
//  • Colour: a palette (greys and the colours in light-to-dark shades), the
//    text's own colour, and any other colour from the system's colour picker.
//  • Table: choose rows × columns on a grid; the table goes in at the cursor.
RowLayout {
    id: pickers
    required property var target   // the TextArea being written in
    spacing: 2

    readonly property var font: target ? target.cursorSelection.font : Qt.font({})
    function setFont(change) {
        var f = target.cursorSelection.font
        change(f)
        target.cursorSelection.font = f
        target.forceActiveFocus()
    }

    component PickerButton: Rectangle {
        id: pb
        property alias label: lbl.text
        property alias labelFont: lbl.font
        property string tip: ""
        property bool showChevron: true
        signal clicked()
        implicitHeight: 28
        radius: 7
        color: pba.pressed ? Theme.pressed : (pba.containsMouse ? Theme.hover : "transparent")
        border.color: Theme.border
        Text {
            id: lbl
            anchors.left: parent.left; anchors.leftMargin: 8
            anchors.right: parent.right; anchors.rightMargin: pb.showChevron ? 22 : 8
            anchors.verticalCenter: parent.verticalCenter
            color: Theme.text
            elide: Text.ElideRight
            font.pixelSize: Theme.base
        }
        Icon { visible: pb.showChevron; anchors.right: parent.right; anchors.rightMargin: 7; anchors.verticalCenter: parent.verticalCenter; width: 10; height: 10; name: "chevronDown"; color: Theme.secondary; stroke: 2 }
        MouseArea { id: pba; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: pb.clicked() }
        ToolTip.visible: pba.containsMouse && pb.tip !== ""
        ToolTip.delay: 600
        ToolTip.text: pb.tip
    }

    component PickerPopup: Popup {
        padding: 6
        background: Rectangle { radius: 9; color: Theme.popup; border.color: Theme.border }
    }

    // ------------------------------------------------------------ font family
    PickerButton {
        id: fontButton
        Layout.preferredWidth: 150
        tip: "Font"
        label: pickers.font.family || "Default"
        labelFont.family: pickers.font.family
        onClicked: { fontSearch.text = ""; fontPop.open(); fontSearch.forceActiveFocus() }
        PickerPopup {
            id: fontPop
            y: parent.height + 3
            width: 280
            height: 380
            readonly property var families: Qt.fontFamilies()
            readonly property var shown: {
                var q = fontSearch.text.trim().toLowerCase()
                return q ? families.filter(function(f) { return f.toLowerCase().indexOf(q) >= 0 }) : families
            }
            ColumnLayout {
                anchors.fill: parent
                spacing: 6
                FieldX { id: fontSearch; Layout.fillWidth: true; placeholderText: "Search fonts"; selectByMouse: true
                    Keys.onReturnPressed: if (fontPop.shown.length) { var f = fontPop.shown[0]; pickers.setFont(function(x) { x.family = f }); fontPop.close() } }
                ListView {
                    id: fontList
                    Momentum { flick: fontList }
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    model: fontPop.opened ? fontPop.shown : []
                    boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: ScrollBar {}
                    delegate: ItemDelegate {
                        required property string modelData
                        width: ListView.view.width - 8
                        height: 30
                        hoverEnabled: true
                        background: Rectangle { radius: 5; color: parent.hovered ? Theme.hover : (modelData === pickers.font.family ? Theme.accentSoft : "transparent") }
                        contentItem: Text {
                            text: modelData
                            font.family: modelData
                            font.pixelSize: 15
                            color: Theme.text
                            elide: Text.ElideRight
                            verticalAlignment: Text.AlignVCenter
                        }
                        onClicked: { var f = modelData; pickers.setFont(function(x) { x.family = f }); fontPop.close() }
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------------ size
    PickerButton {
        id: sizeButton
        Layout.preferredWidth: 58
        tip: "Size"
        readonly property int px: pickers.font.pixelSize > 0 ? pickers.font.pixelSize : Math.round((pickers.font.pointSize > 0 ? pickers.font.pointSize : 10.5) * 4 / 3)
        label: px
        onClicked: { sizeEdit.text = px; sizePop.open() }
        PickerPopup {
            id: sizePop
            y: parent.height + 3
            width: 110
            height: 400
            readonly property var sizes: [9, 10, 11, 12, 13, 14, 16, 18, 20, 24, 28, 32, 36, 48, 64, 72]
            ColumnLayout {
                anchors.fill: parent
                spacing: 6
                // Any size, typed.
                FieldX {
                    id: sizeEdit
                    Layout.fillWidth: true
                    placeholderText: "Size"
                    validator: IntValidator { bottom: 6; top: 200 }
                    selectByMouse: true
                    onAccepted: { var n = parseInt(text); if (n > 0) { pickers.setFont(function(x) { x.pixelSize = n }); sizePop.close() } }
                }
                ListView {
                    id: sizeList
                    Momentum { flick: sizeList }
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    model: sizePop.sizes
                    boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: ScrollBar {}
                    delegate: ItemDelegate {
                        required property int modelData
                        width: ListView.view.width - 8
                        // Each size shown at that size (the biggest kept to fit the list).
                        height: Math.max(26, Math.min(modelData, 40) + 10)
                        hoverEnabled: true
                        background: Rectangle { radius: 5; color: parent.hovered ? Theme.hover : (modelData === sizeButton.px ? Theme.accentSoft : "transparent") }
                        contentItem: Text {
                            text: modelData
                            font.pixelSize: Math.min(modelData, 40)
                            font.family: pickers.font.family
                            color: Theme.text
                            verticalAlignment: Text.AlignVCenter
                        }
                        onClicked: { var n = modelData; pickers.setFont(function(x) { x.pixelSize = n }); sizePop.close() }
                    }
                }
            }
        }
    }

    // ---------------------------------------------------------------- colour
    PickerButton {
        id: colorButton
        Layout.preferredWidth: 40
        showChevron: true
        tip: "Text Colour"
        label: ""
        // The letter A over a bar in the selection's colour.
        Text { x: 8; y: 3; text: "A"; font.pixelSize: 14; font.bold: true; color: Theme.text }
        Rectangle { x: 6; y: 20; width: 14; height: 4; radius: 1; color: pickers.target ? pickers.target.cursorSelection.color : Theme.text; border.color: Theme.border }
        onClicked: colorPop.open()
        PickerPopup {
            id: colorPop
            y: parent.height + 3
            width: grid.implicitWidth + 12
            height: colorCol.implicitHeight + 12
            // Greys, then each colour from light to dark.
            readonly property var hues: [0, 25, 45, 60, 120, 165, 200, 220, 265, 300]
            readonly property var shades: [0.92, 0.8, 0.65, 0.5, 0.38, 0.26]
            function swatch(col, row) {
                if (col === 0) return Qt.hsla(0, 0, [1, 0.85, 0.65, 0.45, 0.25, 0][row], 1)
                return Qt.hsla(hues[col - 1] / 360, 0.75, shades[row], 1)
            }
            ColumnLayout {
                id: colorCol
                anchors.fill: parent
                spacing: 8
                Grid {
                    id: grid
                    columns: 11
                    spacing: 3
                    Repeater {
                        model: 66
                        delegate: Rectangle {
                            required property int index
                            readonly property color c: colorPop.swatch(index % 11, Math.floor(index / 11))
                            width: 20; height: 20; radius: 4
                            color: c
                            border.color: sw.containsMouse ? Theme.text : Qt.rgba(0, 0, 0, 0.12)
                            border.width: sw.containsMouse ? 2 : 1
                            MouseArea { id: sw; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                                onClicked: { pickers.target.cursorSelection.color = parent.c; colorPop.close(); pickers.target.forceActiveFocus() } }
                        }
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    ToolButtonX { text: "Automatic"; onClicked: { pickers.target.cursorSelection.color = Theme.text; colorPop.close(); pickers.target.forceActiveFocus() } }
                    Item { Layout.fillWidth: true }
                    ToolButtonX { text: "Other Colour…"; onClicked: { colorPop.close(); colorDialog.selectedColor = pickers.target.cursorSelection.color; colorDialog.open() } }
                }
            }
        }
        ColorDialog {
            id: colorDialog
            title: "Text Colour"
            onAccepted: { pickers.target.cursorSelection.color = selectedColor; pickers.target.forceActiveFocus() }
        }
    }

    // ----------------------------------------------------------------- table
    ToolButtonX {
        id: tableButton
        implicitWidth: 30
        icon: "table"
        tip: "Insert Table"
        onClicked: { tablePop.rows = 0; tablePop.cols = 0; tablePop.open() }
        PickerPopup {
            id: tablePop
            y: parent.height + 3
            focus: true
            property int rows: 0
            property int cols: 0
            // The grid grows as the pointer nears its edge (up to 20 × 20); any
            // other size is typed in below.
            readonly property int gridRows: Math.min(20, Math.max(8, rows + 1))
            readonly property int gridCols: Math.min(20, Math.max(8, cols + 1))
            onRowsChanged: rowsField.text = rows > 0 ? String(rows) : ""
            onColsChanged: colsField.text = cols > 0 ? String(cols) : ""
            function insertTyped() {
                var r = parseInt(rowsField.text), c = parseInt(colsField.text)
                if (!(r > 0 && c > 0)) return
                pickers.insertTable(r, c)
                close()
            }
            width: Math.max(tableGrid.implicitWidth, sizeRow.implicitWidth) + 12
            height: tableGrid.implicitHeight + tableLabel.implicitHeight + sizeRow.implicitHeight + 26
            Column {
                anchors.fill: parent
                spacing: 6
                Grid {
                    id: tableGrid
                    columns: tablePop.gridCols
                    spacing: 2
                    Repeater {
                        model: tablePop.gridRows * tablePop.gridCols
                        delegate: Rectangle {
                            required property int index
                            readonly property int r: Math.floor(index / tablePop.gridCols) + 1
                            readonly property int c: index % tablePop.gridCols + 1
                            width: 18; height: 18; radius: 2
                            color: r <= tablePop.rows && c <= tablePop.cols ? Theme.accentSoft : Theme.field
                            border.color: r <= tablePop.rows && c <= tablePop.cols ? Theme.accent : Theme.border
                            MouseArea {
                                anchors.fill: parent
                                hoverEnabled: true
                                onEntered: { tablePop.rows = parent.r; tablePop.cols = parent.c }
                                onClicked: { pickers.insertTable(parent.r, parent.c); tablePop.close() }
                            }
                        }
                    }
                }
                Text {
                    id: tableLabel
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    text: tablePop.rows ? tablePop.rows + " × " + tablePop.cols + " table" : "Choose a size, or type one"
                    color: Theme.secondary
                    font.pixelSize: 12
                }
                // Any size: rows × columns, then Insert (or Enter).
                Row {
                    id: sizeRow
                    anchors.horizontalCenter: parent.horizontalCenter
                    spacing: 6
                    FieldX {
                        id: rowsField
                        width: 56
                        placeholderText: "Rows"
                        horizontalAlignment: TextInput.AlignHCenter
                        validator: IntValidator { bottom: 1; top: 1000 }
                        onAccepted: tablePop.insertTyped()
                    }
                    Text { text: "×"; color: Theme.secondary; anchors.verticalCenter: parent.verticalCenter }
                    FieldX {
                        id: colsField
                        width: 56
                        placeholderText: "Cols"
                        horizontalAlignment: TextInput.AlignHCenter
                        validator: IntValidator { bottom: 1; top: 100 }
                        onAccepted: tablePop.insertTyped()
                    }
                    ToolButtonX {
                        text: "Insert"; primary: true
                        anchors.verticalCenter: parent.verticalCenter
                        enabled: parseInt(rowsField.text) > 0 && parseInt(colsField.text) > 0
                        onClicked: tablePop.insertTyped()
                    }
                }
            }
        }
    }

    // For checks: opens one of the pickers ("font", "size", "color", "table").
    function openPicker(which) {
        if (which === "font") fontButton.clicked()
        else if (which === "size") sizeButton.clicked()
        else if (which === "color") colorButton.clicked()
        else if (which === "table") { tableButton.clicked(); tablePop.rows = 3; tablePop.cols = 4 }
        else if (which === "table-big") { tableButton.clicked(); tablePop.rows = 12; tablePop.cols = 9 }
    }

    // A table with empty cells at the cursor (typing goes in the first cell).
    function insertTable(rows, cols) {
        var at = target.cursorPosition
        var html = '<table border="1" cellspacing="0" cellpadding="6" width="100%" style="border-collapse:collapse; border-color:#c8c8ce;">'
        for (var r = 0; r < rows; r++) {
            html += "<tr>"
            for (var c = 0; c < cols; c++) html += "<td>&nbsp;</td>"
            html += "</tr>"
        }
        html += "</table><p></p>"
        target.insert(at, html)
        target.forceActiveFocus()
        target.cursorPosition = at + 1
    }
}
