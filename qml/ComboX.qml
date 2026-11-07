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
    popup.padding: 4
    popup.y: c.height + 2
    popup.height: Math.min(c.count * 30, 360) + 8
    popup.background: Rectangle {
        radius: 8
        color: Theme.popup
        border.color: Theme.border
    }
    // The open list stays still under the pointer: no scrolling to follow it
    // and no sliding highlight (each row just lights up), as tall as its rows
    // up to what fits.
    popup.contentItem: ListView {
        id: comboList
        Momentum { flick: comboList }
        clip: true
        implicitHeight: Math.min(c.count * 30, 360)
        model: c.popup.visible ? c.delegateModel : null
        currentIndex: c.highlightedIndex
        highlightRangeMode: ListView.NoHighlightRange
        highlightFollowsCurrentItem: false
        boundsBehavior: Flickable.StopAtBounds
        ScrollIndicator.vertical: ScrollIndicator {}
    }
    delegate: ItemDelegate {
        required property int index
        width: ListView.view ? ListView.view.width : c.width
        height: 30
        hoverEnabled: true
        highlighted: c.highlightedIndex === index
        leftPadding: 28
        rightPadding: 10
        contentItem: Text {
            text: c.textAt(index)
            color: Theme.text
            font.pixelSize: c.font.pixelSize
            font.weight: c.currentIndex === index ? Font.DemiBold : Font.Normal
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
        }
        background: Rectangle {
            radius: 5
            color: parent.highlighted || parent.hovered ? Theme.hover : "transparent"
            Icon {
                visible: c.currentIndex === index
                x: 9; anchors.verticalCenter: parent.verticalCenter
                width: 12; height: 12
                name: "check"
                color: Theme.accent
                stroke: 2.2
            }
        }
    }
}
