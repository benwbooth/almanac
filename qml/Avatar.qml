import QtQuick

// Round picture: the person's photo, or their initials on a soft gradient.
Rectangle {
    id: av
    property string name: ""
    property string photo: ""
    property int size: 36
    width: size
    height: size
    radius: size / 2
    clip: true
    gradient: Gradient {
        GradientStop { position: 0; color: Qt.lighter(Theme.avatarColor(av.name), 1.25) }
        GradientStop { position: 1; color: Theme.avatarColor(av.name) }
    }
    Text {
        anchors.centerIn: parent
        visible: pic.status !== Image.Ready
        text: Theme.initials(av.name)
        color: "white"
        font.pixelSize: av.size * 0.4
        font.weight: Font.DemiBold
    }
    // Rounded picture: drawn into a circle with a Canvas clip.
    Image {
        id: pic
        visible: false
        source: av.photo ? (av.photo.indexOf("file:") === 0 ? av.photo : "file://" + av.photo) : ""
        asynchronous: true
        sourceSize.width: av.size * 2
        sourceSize.height: av.size * 2
        onStatusChanged: round.requestPaint()
    }
    Canvas {
        id: round
        anchors.fill: parent
        visible: pic.status === Image.Ready
        onPaint: {
            var c = getContext("2d")
            c.reset()
            if (pic.status !== Image.Ready) return
            c.beginPath()
            c.arc(width / 2, height / 2, width / 2, 0, Math.PI * 2)
            c.closePath()
            c.clip()
            var iw = pic.implicitWidth, ih = pic.implicitHeight
            var scale = Math.max(width / iw, height / ih)
            c.drawImage(pic, (width - iw * scale) / 2, (height - ih * scale) / 2, iw * scale, ih * scale)
        }
    }
}
