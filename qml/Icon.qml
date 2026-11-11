import QtQuick

// Simple line icons drawn on a 24×24 grid, so nothing depends on icon themes.
Canvas {
    id: icon
    property string name: ""
    property color color: Theme.secondary
    property real stroke: 1.7
    implicitWidth: 18
    implicitHeight: 18
    onNameChanged: requestPaint()
    onColorChanged: requestPaint()
    onWidthChanged: requestPaint()
    renderStrategy: Canvas.Immediate

    onPaint: {
        var c = getContext("2d")
        c.reset()
        c.save()
        c.scale(width / 24, height / 24)
        c.strokeStyle = icon.color
        c.fillStyle = icon.color
        c.lineWidth = icon.stroke
        c.lineCap = "round"
        c.lineJoin = "round"
        function rr(x, y, w, h, r) {
            c.beginPath(); c.moveTo(x + r, y); c.lineTo(x + w - r, y); c.arcTo(x + w, y, x + w, y + r, r)
            c.lineTo(x + w, y + h - r); c.arcTo(x + w, y + h, x + w - r, y + h, r); c.lineTo(x + r, y + h)
            c.arcTo(x, y + h, x, y + h - r, r); c.lineTo(x, y + r); c.arcTo(x, y, x + r, y, r); c.closePath()
        }
        function line(pts) { c.beginPath(); c.moveTo(pts[0], pts[1]); for (var i = 2; i < pts.length; i += 2) c.lineTo(pts[i], pts[i + 1]); c.stroke() }
        function circle(x, y, r, fill) { c.beginPath(); c.arc(x, y, r, 0, Math.PI * 2); if (fill) c.fill(); else c.stroke() }
        switch (icon.name) {
        case "inbox":
            rr(3, 4.5, 18, 15, 3); c.stroke()
            line([3, 13, 8, 13, 9.5, 16, 14.5, 16, 16, 13, 21, 13]); break
        case "flag":
            line([5.5, 3, 5.5, 21])
            c.beginPath(); c.moveTo(5.5, 4); c.lineTo(19, 4); c.lineTo(16, 8.5); c.lineTo(19, 13); c.lineTo(5.5, 13); c.closePath(); c.fill(); break
        case "flagOutline":
            line([5.5, 3, 5.5, 21])
            c.beginPath(); c.moveTo(5.5, 4); c.lineTo(19, 4); c.lineTo(16, 8.5); c.lineTo(19, 13); c.lineTo(5.5, 13); c.closePath(); c.stroke(); break
        case "draft":
            rr(5, 3, 14, 18, 2); c.stroke(); line([8.5, 8, 15.5, 8]); line([8.5, 12, 15.5, 12]); line([8.5, 16, 12.5, 16]); break
        case "send":
            c.beginPath(); c.moveTo(3, 11); c.lineTo(21, 3); c.lineTo(14, 21); c.lineTo(11, 13); c.closePath(); c.stroke(); line([11, 13, 21, 3]); break
        case "junk":
            rr(4, 4, 16, 16, 3); c.stroke(); line([8.5, 8.5, 15.5, 15.5]); line([15.5, 8.5, 8.5, 15.5]); break
        case "trash":
            line([4, 6.5, 20, 6.5]); line([9, 6.5, 9, 3.8, 15, 3.8, 15, 6.5])
            line([6, 6.5, 7, 20.5, 17, 20.5, 18, 6.5]); line([10, 10, 10, 17]); line([14, 10, 14, 17]); break
        case "archive":
            rr(3, 4, 18, 5, 1.5); c.stroke(); line([5, 9, 5, 20, 19, 20, 19, 9]); line([10, 13, 14, 13]); break
        case "folder":
            c.beginPath(); c.moveTo(3, 7); c.lineTo(3, 18.5); c.lineTo(21, 18.5); c.lineTo(21, 8.5); c.lineTo(11.5, 8.5)
            c.lineTo(9.5, 5.5); c.lineTo(3, 5.5); c.closePath(); c.stroke(); break
        case "compose":
            line([12, 5, 5, 5, 5, 19, 19, 19, 19, 12]); line([10, 14, 11, 10.5, 18.5, 3, 21, 5.5, 13.5, 13, 10, 14]); break
        case "reply":
            line([9.5, 5.5, 3.5, 11, 9.5, 16.5])
            c.beginPath(); c.moveTo(3.5, 11); c.lineTo(14, 11); c.quadraticCurveTo(20.5, 11, 20.5, 19); c.stroke(); break
        case "replyAll":
            line([8, 5.5, 2.5, 11, 8, 16.5]); line([12, 5.5, 6.5, 11, 12, 16.5])
            c.beginPath(); c.moveTo(6.5, 11); c.lineTo(15, 11); c.quadraticCurveTo(21, 11, 21, 19); c.stroke(); break
        case "forward":
            line([14.5, 5.5, 20.5, 11, 14.5, 16.5])
            c.beginPath(); c.moveTo(20.5, 11); c.lineTo(10, 11); c.quadraticCurveTo(3.5, 11, 3.5, 19); c.stroke(); break
        case "search":
            circle(10.5, 10.5, 6); line([15, 15, 20, 20]); break
        case "plus":
            line([12, 5, 12, 19]); line([5, 12, 19, 12]); break
        case "minus":
            line([5, 12, 19, 12]); break
        case "close":
            line([6.5, 6.5, 17.5, 17.5]); line([17.5, 6.5, 6.5, 17.5]); break
        case "chevronLeft":
            line([14.5, 5, 8, 12, 14.5, 19]); break
        case "chevronRight":
            line([9.5, 5, 16, 12, 9.5, 19]); break
        case "chevronDown":
            line([6, 9.5, 12, 15.5, 18, 9.5]); break
        case "expandAll":
            line([7, 9.5, 12, 4.5, 17, 9.5]); line([7, 14.5, 12, 19.5, 17, 14.5]); break
        case "collapseAll":
            line([7, 4.5, 12, 9.5, 17, 4.5]); line([7, 19.5, 12, 14.5, 17, 19.5]); break
        case "mail":
            rr(3, 5, 18, 14, 2.5); c.stroke(); line([3.5, 6.5, 12, 13, 20.5, 6.5]); break
        case "calendar":
            rr(3, 5, 18, 16, 3); c.stroke(); line([3, 10, 21, 10]); line([8, 3, 8, 7]); line([16, 3, 16, 7])
            circle(8, 14, 1.1, true); circle(12, 14, 1.1, true); circle(16, 14, 1.1, true); circle(8, 17.5, 1.1, true); circle(12, 17.5, 1.1, true); break
        case "contacts":
            circle(12, 8.5, 4)
            c.beginPath(); c.moveTo(4.5, 20.5); c.quadraticCurveTo(5, 14, 12, 14); c.quadraticCurveTo(19, 14, 19.5, 20.5); c.stroke(); break
        case "import":
            line([12, 3, 12, 14]); line([7.5, 9.5, 12, 14, 16.5, 9.5]); line([4, 15, 4, 20, 20, 20, 20, 15]); break
        case "unread":
            circle(12, 12, 5, true); break
        case "envelopeOpen":
            line([3, 10, 12, 4, 21, 10, 21, 20, 3, 20, 3, 10]); line([3.5, 10.5, 12, 15.5, 20.5, 10.5]); break
        case "alignLeft":
            line([4, 6, 20, 6]); line([4, 10, 14, 10]); line([4, 14, 20, 14]); line([4, 18, 14, 18]); break
        case "alignCenter":
            line([4, 6, 20, 6]); line([7, 10, 17, 10]); line([4, 14, 20, 14]); line([7, 18, 17, 18]); break
        case "alignRight":
            line([4, 6, 20, 6]); line([10, 10, 20, 10]); line([4, 14, 20, 14]); line([10, 18, 20, 18]); break
        case "listBullet":
            circle(5.5, 7, 1.4, true); circle(5.5, 12, 1.4, true); circle(5.5, 17, 1.4, true)
            line([9.5, 7, 20, 7]); line([9.5, 12, 20, 12]); line([9.5, 17, 20, 17]); break
        case "listNumber":
            c.font = "bold 6px sans-serif"; c.fillText("1", 3.5, 9); c.fillText("2", 3.5, 14); c.fillText("3", 3.5, 19)
            line([9.5, 7, 20, 7]); line([9.5, 12, 20, 12]); line([9.5, 17, 20, 17]); break
        case "table":
            rr(3.5, 4.5, 17, 15, 2); c.stroke()
            line([3.5, 9.5, 20.5, 9.5]); line([3.5, 14.5, 20.5, 14.5]); line([9.5, 4.5, 9.5, 19.5]); line([14.5, 4.5, 14.5, 19.5]); break
        case "winMinimize":
            line([5, 12, 19, 12]); break
        case "winMaximize":
            c.strokeRect(5.5, 5.5, 13, 13); break
        case "winRestore":
            c.strokeRect(5.5, 8.5, 10, 10); line([8.5, 8.5, 8.5, 5.5, 18.5, 5.5, 18.5, 15.5, 15.5, 15.5]); break
        case "winClose":
            line([6, 6, 18, 18]); line([18, 6, 6, 18]); break
        case "resizeGrip":
            line([20, 8, 8, 20]); line([20, 14, 14, 20]); break
        case "popOut":
            rr(4, 7, 13, 13, 2.5); c.stroke(); line([11, 4, 20, 4, 20, 13]); line([20, 4, 11, 13]); break
        case "alignJustify":
            line([4, 6, 20, 6]); line([4, 10, 20, 10]); line([4, 14, 20, 14]); line([4, 18, 20, 18]); break
        case "link":
            c.beginPath(); c.moveTo(10, 14); c.lineTo(14, 10); c.stroke()
            rr(3, 11, 9, 6, 3); c.stroke(); rr(12, 7, 9, 6, 3); c.stroke(); break
        case "quote":
            line([5, 5, 5, 19]); line([9, 8, 20, 8]); line([9, 12, 20, 12]); line([9, 16, 16, 16]); break
        case "check":
            line([5, 12.5, 10, 17.5, 19, 7]); break
        case "location":
            c.beginPath(); c.moveTo(12, 21); c.bezierCurveTo(6, 14, 5, 12, 5, 9.5); c.arc(12, 9.5, 7, Math.PI, 0); c.bezierCurveTo(19, 12, 18, 14, 12, 21); c.stroke()
            circle(12, 9.5, 2.5); break
        case "video":
            rr(3, 7, 13, 10, 2.5); c.stroke(); line([16, 11, 21, 8, 21, 16, 16, 13]); break
        case "info":
            circle(12, 12, 9); line([12, 11, 12, 16.5]); c.beginPath(); c.arc(12, 7.8, 0.9, 0, 2 * Math.PI); c.fill(); break
        case "edit":
            line([4, 20, 5, 15.5, 15.5, 5, 19, 8.5, 8.5, 19, 4, 20]); line([13.5, 7, 17, 10.5]); break
        case "repeat":
            c.beginPath(); c.moveTo(4, 12); c.lineTo(4, 10); c.quadraticCurveTo(4, 7, 7, 7); c.lineTo(19, 7); c.stroke(); line([16, 4, 19, 7, 16, 10])
            c.beginPath(); c.moveTo(20, 12); c.lineTo(20, 14); c.quadraticCurveTo(20, 17, 17, 17); c.lineTo(5, 17); c.stroke(); line([8, 14, 5, 17, 8, 20]); break
        case "refresh":
            c.beginPath(); c.arc(12, 12, 7.5, -Math.PI * 0.35, Math.PI * 1.45); c.stroke(); line([17, 2.8, 17.5, 6.5, 13.8, 7]); break
        case "sidebar":
            rr(3, 4.5, 18, 15, 3); c.stroke(); line([9.5, 4.5, 9.5, 19.5]); line([5.5, 8.5, 7.5, 8.5]); line([5.5, 11.5, 7.5, 11.5]); break
        case "sun":
            circle(12, 12, 4)
            for (var k = 0; k < 8; k++) {
                var a = k * Math.PI / 4
                line([12 + Math.cos(a) * 6.8, 12 + Math.sin(a) * 6.8, 12 + Math.cos(a) * 9, 12 + Math.sin(a) * 9])
            }
            break
        case "moon":
            c.beginPath(); c.arc(12, 12, 8, Math.PI * 0.25, Math.PI * 1.75, false)
            c.quadraticCurveTo(10, 12, 17.66, 17.66); c.closePath(); c.stroke(); break
        // An attachment sent as a link to cloud storage.
        case "cloud":
            c.beginPath(); c.moveTo(7, 18); c.arc(7, 14, 4, Math.PI * 0.5, Math.PI * 1.4, false)
            c.arc(12, 9.5, 5, Math.PI * 1.1, Math.PI * 1.95, false); c.arc(17, 13.5, 4.5, Math.PI * 1.5, Math.PI * 0.5, false)
            c.closePath(); c.stroke(); break
        // OpenPGP in compose: a lock (encrypt) open or shut, a seal with a tick (sign).
        case "lock":
        case "lockOpen":
            c.beginPath(); c.rect(5.5, 11, 13, 9); c.stroke()
            c.beginPath(); c.moveTo(8.5, 11); c.lineTo(8.5, 7.5); c.arc(12, 7.5, 3.5, Math.PI, 0, false)
            c.lineTo(15.5, name === "lock" ? 11 : 8.5); c.stroke(); break
        case "seal":
            circle(12, 10, 6.5); line([9, 10.2, 11.2, 12.3, 15.2, 8]); line([8.5, 15.5, 7.5, 21.5, 12, 19.5, 16.5, 21.5, 15.5, 15.5]); break
        case "paperclip":
            c.beginPath(); c.moveTo(16, 7.5); c.lineTo(9, 14.5); c.arc(10.75, 16.25, 2.47, Math.PI * 1.25, Math.PI * 0.25, true)
            c.lineTo(18.5, 8.5); c.arc(15, 5, 4.95, Math.PI * 0.25, Math.PI * 1.25, true); c.lineTo(5, 15)
            c.arc(8.5, 18.5, 4.95, Math.PI * 1.25, Math.PI * 0.25, true); c.lineTo(19, 12); c.stroke(); break
        case "filter":
            circle(12, 12, 9); line([7.5, 9.5, 16.5, 9.5]); line([9, 12.5, 15, 12.5]); line([10.5, 15.5, 13.5, 15.5]); break
        case "export":
            line([12, 14, 12, 3]); line([7.5, 7.5, 12, 3, 16.5, 7.5]); line([4, 13, 4, 20, 20, 20, 20, 13]); break
        case "sort":
            // Lines getting shorter, and an arrow down beside them.
            line([4, 7, 13, 7]); line([4, 12, 11, 12]); line([4, 17, 9, 17]); line([17.5, 5, 17.5, 19]); line([14.5, 16, 17.5, 19, 20.5, 16]); break
        case "more":
            circle(6, 12, 1.6, true); circle(12, 12, 1.6, true); circle(18, 12, 1.6, true); break
        case "star":
        case "starOutline":
            c.beginPath()
            for (var p = 0; p < 10; p++) {
                var ang = -Math.PI / 2 + p * Math.PI / 5
                var rad = p % 2 === 0 ? 9 : 4
                if (p === 0) c.moveTo(12 + Math.cos(ang) * rad, 12.5 + Math.sin(ang) * rad)
                else c.lineTo(12 + Math.cos(ang) * rad, 12.5 + Math.sin(ang) * rad)
            }
            c.closePath()
            if (icon.name === "star") c.fill(); else c.stroke()
            break
        case "bell":
        case "bellOff":
            c.beginPath(); c.moveTo(5, 17); c.lineTo(19, 17); c.quadraticCurveTo(17, 15, 17, 11); c.quadraticCurveTo(17, 5, 12, 5)
            c.quadraticCurveTo(7, 5, 7, 11); c.quadraticCurveTo(7, 15, 5, 17); c.stroke()
            line([10, 20, 14, 20])
            if (icon.name === "bellOff") line([4, 4, 20, 20])
            break
        case "clock":
            circle(12, 12, 8.5); line([12, 7, 12, 12, 15.5, 14]); break
        case "printer":
            line([7, 9, 7, 4, 17, 4, 17, 9]); rr(3.5, 9, 17, 8, 2); c.stroke(); line([7, 14, 7, 20, 17, 20, 17, 14]); break
        case "people":
            circle(9, 8.5, 3.3); circle(16.5, 9.5, 2.6)
            c.beginPath(); c.moveTo(3, 19.5); c.quadraticCurveTo(3.5, 14, 9, 14); c.quadraticCurveTo(14.5, 14, 15, 19.5); c.stroke()
            c.beginPath(); c.moveTo(15.5, 14.2); c.quadraticCurveTo(20.5, 14.2, 21, 19); c.stroke(); break
        case "phone":
            c.beginPath(); c.moveTo(7, 3.5); c.lineTo(9.6, 3.5); c.lineTo(11.2, 7.6); c.lineTo(9.2, 9.2)
            c.bezierCurveTo(10.2, 11.6, 12.4, 13.8, 14.8, 14.8); c.lineTo(16.4, 12.8); c.lineTo(20.5, 14.4); c.lineTo(20.5, 17)
            c.bezierCurveTo(20.5, 18.9, 19.1, 20.6, 17.2, 20.4); c.bezierCurveTo(10, 19.6, 4.4, 14, 3.6, 6.8)
            c.bezierCurveTo(3.4, 4.9, 5.1, 3.5, 7, 3.5); c.closePath(); c.stroke(); break
        case "chat":
            rr(3.5, 4.5, 17, 12, 4); c.stroke(); line([8.5, 16.5, 7, 20.5, 12.5, 16.5]); break
        case "photo":
            rr(3, 5, 18, 14, 2.5); c.stroke(); circle(9, 10, 1.8); line([3.5, 17, 10, 12.5, 14, 15.5, 17, 13, 20.5, 16]); break
        case "gear":
            circle(12, 12, 3)
            for (var g = 0; g < 8; g++) {
                var ga = g * Math.PI / 4
                line([12 + Math.cos(ga) * 6, 12 + Math.sin(ga) * 6, 12 + Math.cos(ga) * 8.5, 12 + Math.sin(ga) * 8.5])
            }
            circle(12, 12, 6); break
        case "undo":
            line([8, 5, 4, 9, 8, 13]); c.beginPath(); c.moveTo(4, 9); c.lineTo(14, 9); c.quadraticCurveTo(20, 9, 20, 14.5); c.quadraticCurveTo(20, 20, 14, 20); c.lineTo(9, 20); c.stroke(); break
        default:
            circle(12, 12, 3, true)
        }
        c.restore()
    }
}
