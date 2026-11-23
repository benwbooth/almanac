import QtQuick

// A drawn picture of a web page for a setup step: a browser window with
// the page's address, its heading, and the fields, choices and buttons on
// it; the ones to use are ringed (pulsing). Drawn, not a screenshot, so it
// never goes stale or shows anyone's account.
//   rows: [{ kind: "field"|"choice"|"text"|"button"|"link", label, value, hot }]
Rectangle {
    id: sp
    property string address: ""
    property string heading: ""
    property var rows: []
    readonly property color ink: Theme.dark ? "#e8eaed" : "#202124"
    readonly property color blue: "#1a73e8"
    implicitHeight: page.implicitHeight + bar.height + 22
    radius: 10
    color: Theme.dark ? "#2b2d31" : "#f2f2f4"
    border.color: Theme.border
    clip: true

    // The browser's bar: dots and the address.
    Rectangle {
        id: bar
        width: parent.width
        height: 28
        color: Theme.dark ? "#202124" : "#e4e4e8"
        radius: sp.radius
        Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: sp.radius; color: parent.color }
        Row {
            x: 10
            anchors.verticalCenter: parent.verticalCenter
            spacing: 5
            Repeater { model: ["#ff5f57", "#febc2e", "#28c840"]; Rectangle { required property string modelData; width: 9; height: 9; radius: 4.5; color: modelData } }
        }
        Rectangle {
            x: 56
            width: parent.width - 66
            height: 19
            radius: 9.5
            anchors.verticalCenter: parent.verticalCenter
            color: Theme.dark ? "#35363a" : "#ffffff"
            Text {
                anchors.verticalCenter: parent.verticalCenter
                x: 10
                width: parent.width - 20
                text: "🔒 " + sp.address
                elide: Text.ElideRight
                font.pixelSize: 11
                color: Theme.secondary
            }
        }
    }
    // The page.
    Column {
        id: page
        anchors.top: bar.bottom
        anchors.topMargin: 9
        x: 16
        width: parent.width - 32
        spacing: 7
        Text { text: sp.heading; font.pixelSize: 15; font.weight: Font.DemiBold; visible: sp.heading !== ""; color: sp.ink }
        Repeater {
            model: sp.rows
            Item {
                id: row
                required property var modelData
                width: page.width
                height: Math.max(22, inner.implicitHeight)
                readonly property bool hot: !!modelData.hot
                Loader {
                    id: inner
                    width: parent.width
                    property var d: row.modelData
                    sourceComponent: row.modelData.kind === "field" ? fieldC : row.modelData.kind === "choice" ? choiceC
                                   : row.modelData.kind === "button" ? buttonC : row.modelData.kind === "link" ? linkC : textC
                }
                // The thing to use: a pulsing ring (a button's hugs the button).
                Rectangle {
                    visible: row.hot
                    x: -4; y: -4
                    width: (row.modelData.kind === "button" && inner.item ? inner.item.implicitWidth : parent.width) + 8
                    height: parent.height + 8
                    radius: 8
                    color: "transparent"
                    border.width: 2.5
                    border.color: Theme.accent
                    SequentialAnimation on opacity {
                        running: row.hot && sp.visible
                        loops: Animation.Infinite
                        NumberAnimation { from: 1; to: 0.3; duration: 700 }
                        NumberAnimation { from: 0.3; to: 1; duration: 700 }
                    }
                }
            }
        }
    }
    Component {
        id: fieldC
        Column {
            property var d: parent ? parent.d : ({})
            spacing: 2
            Text { text: d.label || ""; font.pixelSize: 11; color: Theme.secondary }
            Rectangle {
                width: page.width
                height: 22
                radius: 4
                color: "transparent"
                border.color: Theme.dark ? "#5f6368" : "#bdc1c6"
                Text { x: 8; width: parent.width - 16; anchors.verticalCenter: parent.verticalCenter; text: d.value || ""; font.pixelSize: 12; color: sp.ink; elide: Text.ElideRight }
            }
        }
    }
    Component {
        id: choiceC
        Row {
            property var d: parent ? parent.d : ({})
            spacing: 8
            Rectangle {
                width: 14; height: 14; radius: 7
                anchors.verticalCenter: parent.verticalCenter
                color: "transparent"
                border.color: d.value ? sp.blue : Theme.tertiary
                border.width: 2
                Rectangle { x: 4; y: 4; width: 6; height: 6; radius: 3; color: sp.blue; visible: !!d.value }
            }
            Text { text: d.label || ""; font.pixelSize: 12; anchors.verticalCenter: parent.verticalCenter; color: sp.ink }
        }
    }
    Component {
        id: buttonC
        Item {
            property var d: parent ? parent.d : ({})
            height: 24
            implicitWidth: btnText.implicitWidth + 24
            Rectangle {
                width: parent.implicitWidth
                height: 24
                radius: 4
                color: sp.blue
                Text { id: btnText; x: 12; anchors.verticalCenter: parent.verticalCenter; text: d.label || ""; color: "white"; font.pixelSize: 12; font.weight: Font.DemiBold }
            }
        }
    }
    Component {
        id: linkC
        Text { property var d: parent ? parent.d : ({}); text: d.label || ""; color: sp.blue; font.pixelSize: 12; font.underline: true }
    }
    Component {
        id: textC
        Text { property var d: parent ? parent.d : ({}); text: d.label || ""; wrapMode: Text.Wrap; width: page.width; font.pixelSize: 12; color: Theme.dark ? "#bdc1c6" : "#5f6368" }
    }
}
