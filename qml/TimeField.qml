import QtQuick
import QtQuick.Controls.Basic

// A time, written the way the system does (9:30 AM or 09:30). Type it in
// any common way ("930", "9:30pm", "21:30"); Up and Down move it by 15
// minutes. `text` is the time as HH:MM (24-hour).
FieldX {
    id: tf
    property string value: "09:00"
    signal edited()
    implicitWidth: 92
    selectByMouse: true
    horizontalAlignment: Text.AlignHCenter
    function show(v) {
        var p = v.split(":"), d = new Date(2000, 0, 1, parseInt(p[0]) || 0, parseInt(p[1]) || 0)
        return Theme.twelveHour ? Theme.fmtTime(d).replace(/^(\d+) /, "$1:00 ") : Theme.fmtTime(d)
    }
    // Reads what was typed; returns HH:MM or "" if it isn't a time.
    function parse(t) {
        var s = t.trim().toLowerCase().replace(/\s+/g, "")
        var pm = /p/.test(s), am = /a/.test(s)
        s = s.replace(/[apm.]/g, "")
        var h, m
        if (s.indexOf(":") >= 0) { var p = s.split(":"); h = parseInt(p[0]); m = parseInt(p[1] || "0") }
        else if (s.length <= 2) { h = parseInt(s); m = 0 }
        else { h = parseInt(s.slice(0, s.length - 2)); m = parseInt(s.slice(-2)) }
        if (isNaN(h) || isNaN(m) || m > 59 || h > 23) return ""
        if (pm && h < 12) h += 12
        if (am && h === 12) h = 0
        return Theme.pad2(h) + ":" + Theme.pad2(m)
    }
    function step(mins) {
        var p = value.split(":"), t = ((parseInt(p[0]) * 60 + parseInt(p[1]) + mins) % 1440 + 1440) % 1440
        t = Math.round(t / 15) * 15 % 1440
        value = Theme.pad2(Math.floor(t / 60)) + ":" + Theme.pad2(t % 60)
        edited()
    }
    property alias timeText: tf.value
    text: show(value)
    onValueChanged: if (!activeFocus) text = show(value)
    onEditingFinished: { var v = parse(text); if (v !== "") { value = v; edited() } text = show(value) }
    Keys.onUpPressed: step(15)
    Keys.onDownPressed: step(-15)
}
