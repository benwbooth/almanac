pragma Singleton
import QtQuick

// Colours, sizes and small date/text helpers shared by every screen.
QtObject {
    // The browser engine's last download already handled (every page hears
    // about every download: the first one it's meant for takes it).
    property int handledDownload: -1
    // Flag colours, as in Apple Mail (the same order: 0 red … 6 gray).
    readonly property var flagColors: ["#ff3b30", "#ff9500", "#f5c000", "#34c759", "#007aff", "#af52de", "#8e8e93"]
    // A tab being dragged: {owner, index}, and where it's let go {target, at}.
    property var tabDrag: null
    readonly property var flagNames: ["Red", "Orange", "Yellow", "Green", "Blue", "Purple", "Gray"]
    // The page (message, tab or window) the pointer was last over: a PDF
    // viewer's download or print button there doesn't always say which page
    // it's in, so it's this one.
    property var pointerPage: null
    // When a print was last asked for (the engine can ask twice for one click).
    property double printAskedAt: 0
    // "system", "light" or "dark"; set from the saved choice in Main.qml.
    property string mode: "system"
    readonly property bool dark: mode === "dark" || (mode === "system" && Qt.styleHints.colorScheme === Qt.ColorScheme.Dark)

    // Readable in both appearances: every text colour on every background it's
    // used on meets the WCAG contrast ratios (7:1 for main text, 4.5:1 for
    // smaller and secondary text and links, 3:1 for icons); a test checks this
    // (src/contrast.rs), so a colour can't be changed into an unreadable one.
    readonly property color accent: dark ? "#4ea3ff" : "#0062cc"
    // Blue behind white text or ticks (selected rows, primary buttons, checked
    // boxes): deeper than `accent`, which in dark mode is bright to read as text.
    readonly property color accentFill: dark ? "#1f6fd1" : "#0062cc"
    readonly property color accentSoft: dark ? "#132c4d" : "#dde9fd"
    readonly property color accentInk: "#ffffff"
    readonly property color red: dark ? "#ff453a" : "#d70015"
    readonly property color orange: dark ? "#ff9f0a" : "#c25e00"
    readonly property color sidebar: dark ? "#202024" : "#eef0f3"
    readonly property color sidebarSelected: dark ? "#45454d" : "#dadde4"
    readonly property color window: dark ? "#141416" : "#ffffff"
    readonly property color dim: dark ? "#0f0f11" : "#fafafb"
    readonly property color toolbar: dark ? "#232327" : "#f7f7f9"
    // A little depth, as in Apple Mail: bars shaded lighter at the top, the
    // sidebar a touch darker toward the bottom, a soft shadow under the top
    // bar and under the chosen segment. (Text on each end is checked too.)
    readonly property color toolbarTop: dark ? "#303036" : "#fdfdfe"
    readonly property color toolbarBottom: dark ? "#222226" : "#eeeef2"
    readonly property color sidebarTop: dark ? "#25252a" : "#f0f1f4"
    readonly property color sidebarBottom: dark ? "#1c1c20" : "#ebedf1"
    readonly property color shade: dark ? Qt.rgba(0, 0, 0, 0.5) : Qt.rgba(0, 0, 0, 0.09)
    readonly property color segmentShade: dark ? Qt.rgba(0, 0, 0, 0.55) : Qt.rgba(0, 0, 0, 0.16)
    readonly property color sheen: dark ? Qt.rgba(1, 1, 1, 0.07) : Qt.rgba(1, 1, 1, 0.9)
    readonly property Gradient toolbarGradient: Gradient {
        GradientStop { position: 0; color: toolbarTop }
        GradientStop { position: 1; color: toolbarBottom }
    }
    readonly property Gradient sidebarGradient: Gradient {
        GradientStop { position: 0; color: sidebarTop }
        GradientStop { position: 1; color: sidebarBottom }
    }
    readonly property color border: dark ? "#55555c" : "#dcdde2"
    readonly property color borderSoft: dark ? "#3a3a40" : "#ececf0"
    readonly property color text: dark ? "#ffffff" : "#1d1d1f"
    readonly property color secondary: dark ? "#c8c8ce" : "#56565b"
    readonly property color tertiary: dark ? "#9a9aa2" : "#6b6b70"
    readonly property color hover: dark ? "#33333a" : "#e7e8ec"
    readonly property color pressed: dark ? "#45454c" : "#d6d7dc"
    readonly property color field: dark ? "#1c1c20" : "#ffffff"
    readonly property color fieldIdle: dark ? "#2c2c32" : "#e9eaee"
    readonly property color segment: dark ? "#33333a" : "#e6e7eb"
    readonly property color segmentOn: dark ? "#4e4e56" : "#ffffff"
    readonly property color card: dark ? "#222227" : "#f6f7f9"
    readonly property color popup: dark ? "#26262b" : "#ffffff"
    readonly property color toast: dark ? "#e8e8ea" : "#323236"
    readonly property color toastText: dark ? "#1d1d1f" : "#ffffff"
    // Event text drawn on a tinted background
    function eventText(c) { return dark ? Qt.lighter(c, 1.75) : Qt.darker(c, 1.6) }
    function eventFill(c, a) { return Qt.rgba(c.r, c.g, c.b, dark ? a * 1.5 : a) }
    readonly property int toolbarHeight: 52
    // How window buttons look: "windows", "breeze" (KDE Plasma) or "adwaita"
    // (GNOME and other Linux desktops); set at start from the desktop in use.
    property string windowButtons: Qt.platform.os === "windows" ? "windows" : (Qt.platform.os === "linux" ? "adwaita" : "windows")
    readonly property int base: 13

    function pad2(n) { return (n < 10 ? "0" : "") + n }
    function isoDate(d) { return d.getFullYear() + "-" + pad2(d.getMonth() + 1) + "-" + pad2(d.getDate()) }
    function isoDT(d) { return isoDate(d) + "T" + pad2(d.getHours()) + ":" + pad2(d.getMinutes()) }
    function parseDT(s) {
        if (!s) return new Date()
        var y = parseInt(s.substr(0, 4)), m = parseInt(s.substr(5, 2)) - 1, d = parseInt(s.substr(8, 2))
        var h = s.length >= 16 ? parseInt(s.substr(11, 2)) : 0
        var mi = s.length >= 16 ? parseInt(s.substr(14, 2)) : 0
        return new Date(y, m, d, h, mi)
    }
    function addDays(d, n) { var r = new Date(d.getFullYear(), d.getMonth(), d.getDate() + n, d.getHours(), d.getMinutes()); return r }
    function startOfDay(d) { return new Date(d.getFullYear(), d.getMonth(), d.getDate()) }
    function sameDay(a, b) { return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate() }
    // -1 = as the system does (Settings › Calendar)
    property int weekStart: -1
    function firstDay() { return weekStart >= 0 ? weekStart : Qt.locale().firstDayOfWeek % 7 }
    function startOfWeek(d) {
        var s = startOfDay(d)
        var diff = (s.getDay() - firstDay() + 7) % 7
        return addDays(s, -diff)
    }
    function monthName(m) { return Qt.locale().standaloneMonthName(m, Locale.LongFormat) }
    function dayName(wd, fmt) { return Qt.locale().dayName(wd, fmt === undefined ? Locale.ShortFormat : fmt) }
    // 12-hour clock (with AM/PM) or 24-hour, as the system is set; used for
    // every time shown so they all agree.
    readonly property bool twelveHour: /a/i.test(Qt.locale().timeFormat(Locale.ShortFormat).replace(/'[^']*'/g, ""))
    function fmtTime(d) {
        var h = d.getHours(), m = d.getMinutes()
        if (!twelveHour) return pad2(h) + ":" + pad2(m)
        var ap = h < 12 ? "AM" : "PM"
        var hh = h % 12; if (hh === 0) hh = 12
        return hh + (m ? ":" + pad2(m) : "") + " " + ap
    }
    function fmtHour(h) {
        if (!twelveHour) return pad2(h) + ":00"
        if (h === 0) return "12 AM"
        if (h === 12) return "Noon"
        return (h % 12) + (h < 12 ? " AM" : " PM")
    }
    // Tooltip for a calendar event: full title, time and place.
    function eventTip(e) {
        if (!e) return ""
        var s = parseDT(e.start), en = parseDT(e.end)
        var when = e.allDay ? "All day" : (s ? fmtTime(s) : "") + (en ? " – " + fmtTime(en) : "")
        return e.title + "\n" + when + (e.location ? "\n" + e.location : "")
    }
    function fmtListDate(ms) {
        if (!ms) return ""
        var d = new Date(ms), now = new Date()
        if (sameDay(d, now)) return fmtTime(d)
        if (sameDay(d, addDays(now, -1))) return "Yesterday"
        var diff = (startOfDay(now) - startOfDay(d)) / 86400000
        if (diff > 0 && diff < 7) return dayName(d.getDay(), Locale.LongFormat)
        return d.toLocaleDateString(Qt.locale(), Locale.ShortFormat)
    }
    function fmtLongDate(ms) {
        var d = new Date(ms)
        return d.toLocaleDateString(Qt.locale(), Locale.LongFormat) + " at " + fmtTime(d)
    }
    function initials(name) {
        if (!name) return "?"
        // A bare address: the first letter of it ("benwbooth@gmail.com" is B, not BC).
        if (name.indexOf("@") > 0 && name.indexOf(" ") < 0) return name.charAt(0).toUpperCase()
        var parts = name.replace(/["'<>()\[\]@.,_-]/g, " ").trim().split(/\s+/)
        if (parts.length === 0 || parts[0] === "") return name.charAt(0).toUpperCase()
        var s = parts[0].charAt(0)
        if (parts.length > 1) s += parts[parts.length - 1].charAt(0)
        return s.toUpperCase()
    }
    function avatarColor(name) {
        var colors = ["#8e8e93", "#5e9cf0", "#56b366", "#e9a23b", "#c56cd6", "#e9655a", "#47b5b0", "#7f7fdc"]
        var h = 0
        for (var i = 0; i < (name || "").length; i++) h = (h * 31 + name.charCodeAt(i)) % 9973
        return colors[h % colors.length]
    }
    function alpha(c, a) { return Qt.rgba(c.r, c.g, c.b, a) }
}
