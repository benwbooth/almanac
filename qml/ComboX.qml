import QtQuick
import QtQuick.Controls.Basic

// A drop-down list in the app's look (rounded, with a chevron), used
// everywhere instead of the plain default one.
ComboBox {
    id: c
    implicitHeight: 30
    font.pixelSize: Theme.base
    // The text box inside has its own left padding already.
    leftPadding: 0
    rightPadding: 28
    hoverEnabled: true
    palette.button: Theme.field
    palette.buttonText: Theme.text
    palette.text: Theme.text
    palette.base: Theme.field
    palette.window: Theme.popup
    palette.highlight: Theme.accent
    palette.highlightedText: Theme.accentInk
    palette.light: Theme.hover
    palette.midlight: Theme.hover
    palette.mid: Theme.border
    palette.dark: Theme.border
    palette.placeholderText: Theme.tertiary
    background: Rectangle {
        implicitWidth: 140
        radius: 7
        color: c.down ? Theme.pressed : (c.hovered && !c.editable ? Theme.hover : Theme.field)
        border.color: c.activeFocus ? Theme.accent : Theme.border
        border.width: c.activeFocus ? 2 : 1
    }
    indicator: Icon {
        x: c.width - width - 10
        y: (c.height - height) / 2
        width: 12; height: 12
        name: "chevronDown"
        color: Theme.secondary
        stroke: 2.2
    }
    // Editable lists: no second box drawn inside this one.
    Component.onCompleted: if (contentItem && contentItem.background) contentItem.background.visible = false
    // The list: the shared macOS pop-up list (hearth/shared/app-menus), the
    // current choice over the field with its text lined up (an editable one
    // opens below).
    delegate: MacComboItem { combo: c }
    popup: MacComboPopup { combo: c; dark: Theme.dark; accent: Theme.accentFill; textX: 10 }
}
