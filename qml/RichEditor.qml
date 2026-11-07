import QtQuick
import QtQuick.Controls.Basic
// The system's own menus (desktop style on Linux, native on macOS and Windows)
import Qt.labs.platform as P
import QtQuick.Layouts

// The writing area of a message (and of an event's notes): the formatting bar
// over the text, which scrolls to keep the cursor in view. Markdown as you
// type, code blocks, quotes, lists, tables, links, and spelling and grammar
// checked as you type.
ColumnLayout {
    id: sheet
    property var store
    // In use (spelling is checked and the shortcuts work only then).
    property bool active: false
    // Where its questions (Add Link) appear.
    property Item popupParent: sheet
    readonly property alias area: body
    readonly property alias scroller: bodyScroll
    readonly property alias pickers: fmtPickers
    // Asking for a link (Escape closes that, not what it's in).
    readonly property bool asking: linkPrompt.opened
    spacing: 0

    // Markdown typed in the message turns into formatting as you go
    // (Settings › Mail, or the "Md" button).
    property bool markdown: true

    property bool converting: false
    // The text as plain text (tables, code blocks among them, have marks of
    // their own in the editor's text: line breaks in the plain version).
    function plainText() { return body.getText(0, body.length).replace(/\u200b/g, "").replace(/[\ufdd0\ufdd1]/g, "\n") }
    function esc(t) { return t.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;") }
    // Text of the current paragraph up to the cursor: [start, text].
    function lineBeforeCursor() {
        var pos = body.cursorPosition
        // (A table cell starts after U+FDD0; the text up to a cell's end also has the table's end mark.)
        var all = body.getText(0, pos).replace(/\ufdd1+$/, "")
        var start = Math.max(all.lastIndexOf("\u2029"), all.lastIndexOf("\n"), all.lastIndexOf("\u2028"), all.lastIndexOf("\ufdd0")) + 1
        return [start, all.slice(start)]
    }
    function replaceRange(a, b, html, caret) {
        body.remove(a, b)
        body.insert(a, html)
        body.cursorPosition = a + caret
    }
    // **bold**, *italic*, ~~strike~~, `code`, [text](link): when the closing mark is typed.
    function inlineMarkdown() {
        if (!markdown || converting || codeHere()) return
        var pos = body.cursorPosition
        var lb = lineBeforeCursor(), start = lb[0], line = lb[1].replace(/\u00a0/g, " "), m
        converting = true
        if (/^[-*+] $/.test(line) || /^\d+[.)] $/.test(line)) {
            var tag = /^\d/.test(line) ? "ol" : "ul"
            body.remove(start, pos)
            body.insert(start, "<" + tag + "><li>\u200b</li></" + tag + ">")
            body.cursorPosition = start + 1
            converting = false
            return
        }
        // Smart dashes and quotes, as in Apple Mail.
        if (/--$/.test(line) && !/---$/.test(line)) { replaceRange(pos - 2, pos, "\u2014", 1); converting = false; return }
        if (/(^|[\s(\[{])"$/.test(line)) { replaceRange(pos - 1, pos, "\u201c", 1); converting = false; return }
        if (/[^\s(\[{]"$/.test(line)) { replaceRange(pos - 1, pos, "\u201d", 1); converting = false; return }
        if (/(^|[\s(\[{])'$/.test(line)) { replaceRange(pos - 1, pos, "\u2018", 1); converting = false; return }
        if (/[^\s(\[{]'$/.test(line)) { replaceRange(pos - 1, pos, "\u2019", 1); converting = false; return }
        if ((m = /\*\*([^*\n]+)\*\*$/.exec(line)) || (m = /__([^_\n]+)__$/.exec(line)))
            replaceRange(start + m.index, pos, "<b>" + esc(m[1]) + "</b>\u200b", m[1].length + 1)
        else if ((m = /~~([^~\n]+)~~$/.exec(line)))
            replaceRange(start + m.index, pos, "<s>" + esc(m[1]) + "</s>\u200b", m[1].length + 1)
        else if ((m = /`([^`\n]+)`$/.exec(line)))
            replaceRange(start + m.index, pos, "<code style=\"font-family:monospace\">" + esc(m[1]) + "</code>\u200b", m[1].length + 1)
        else if ((m = /\[([^\]\n]+)\]\(((?:https?:\/\/|mailto:)[^)\s]+)\)$/.exec(line)))
            replaceRange(start + m.index, pos, "<a href=\"" + esc(m[2]) + "\">" + esc(m[1]) + "</a>\u200b", m[1].length + 1)
        else if ((m = /(^|[^*\w])\*([^*\s][^*\n]*)\*$/.exec(line)) || (m = /(^|[^_\w])_([^_\s][^_\n]*)_$/.exec(line)))
            replaceRange(start + m.index + m[1].length, pos, "<i>" + esc(m[2]) + "</i>\u200b", m[2].length + 1)
        converting = false
    }
    // # heading, - / * list, 1. list, > quote: when Enter ends the line.
    function blockMarkdown() {
        if (converting) return false
        // In a code block: ``` alone on a line leaves it (Enter is a new line of code otherwise).
        if (codeHere()) return leaveCodeBlock()
        if (!markdown) return false
        var pos = body.cursorPosition
        var lb = lineBeforeCursor(), start = lb[0], line = lb[1], m
        var html = ""
        line = line.replace(/\u00a0/g, " ")
        // ``` (and a language): a code block.
        if ((m = /^```\s*([\w+#.-]*)\s*$/.exec(line))) { insertCodeBlock(m[1] || "Plain Text", "", start, pos); return true }
        // --- : a line across.
        if (/^(---+|\*\*\*+|___+)$/.test(line.trim())) {
            converting = true
            body.remove(start, pos)
            body.insert(start, "<hr><p></p>")
            converting = false
            return true
        }
        if ((m = /^(#{1,3}) (.+)$/.exec(line))) html = "<h" + (m[1].length + 1) + ">" + esc(m[2]) + "</h" + (m[1].length + 1) + "><p></p>"
        else if ((m = /^[-*+] (.+)$/.exec(line))) html = "<ul><li>" + esc(m[1]) + "</li><li>\u200b</li></ul>"
        else if ((m = /^\d+[.)] (.+)$/.exec(line))) html = "<ol><li>" + esc(m[1]) + "</li><li>\u200b</li></ol>"
        else if ((m = /^> (.+)$/.exec(line))) html = "<p style=\"margin-left:18px;margin-right:0px\">" + esc(m[1]) + "</p><p style=\"margin-left:0px\"></p>"
        if (!html) return false
        converting = true
        body.remove(start, pos)
        body.insert(start, html)
        // Into the new empty list item (or the paragraph after a heading or quote).
        var item = /^[-*+] (.+)$/.exec(line) || /^\d+[.)] (.+)$/.exec(line)
        if (item) body.cursorPosition = start + item[1].length + 2
        converting = false
        return true
    }

    // ------------------------------------------------------ code blocks
    // A block is a table: the language over the code (see codeblock.rs). In the
    // editor's text each cell starts with U+FDD0 and the table ends with U+FDD1.
    property var codeLanguages: []
    property string lastLanguage: "Plain Text"
    // The code block the cursor is in: {label, labelStart} (null when not in one).
    function codeBlockAt(pos) {
        // (The text up to the end of a cell also has the table's end mark.)
        var t = body.getText(0, pos).replace(/\ufdd1+$/, "")
        var cell = t.lastIndexOf("\ufdd0")
        if (cell < 1 || t.indexOf("\ufdd1", cell) >= 0) return null
        var table = t.lastIndexOf("\ufdd0", cell - 1)
        if (table < 0 || t.indexOf("\ufdd1", table) >= 0 && t.indexOf("\ufdd1", table) < cell) return null
        var label = t.slice(table + 1, cell)
        if (/[\u2029\u2028]/.test(label)) return null
        if (!codeLanguages.length) codeLanguages = JSON.parse(store.codeLanguages())
        if (codeLanguages.indexOf(label) < 0) return null
        return { label: label, labelStart: table + 1 }
    }
    function codeHere() { return codeBlockAt(body.cursorPosition) !== null }
    // Whether a place in the text is in a code block's code (for the spelling marks).
    function inCodeText(pos) { return codeBlockAt(pos + 1) !== null }
    // Where the cursor is: noted a moment after it moves (for the language picker).
    property var codeCursor: null
    Timer { id: codeSpot; interval: 120; onTriggered: sheet.codeCursor = sheet.codeBlockAt(body.cursorPosition) }
    // The mark (U+E000) put in the text: the cursor goes there, and it goes.
    function takeMark() {
        var t = body.getText(0, body.length), i = t.indexOf("\ue000")
        if (i < 0) return false
        body.remove(i, i + 1)
        body.cursorPosition = i
        return true
    }
    // A code block in place of [start, end) (its code `code`), the cursor in it.
    function insertCodeBlock(lang, code, start, end) {
        converting = true
        if (end > start) body.remove(start, end)
        body.insert(start, store.codeBlockHtml(lang, code + "\ue000", Theme.dark) + "<p></p>")
        takeMark()
        converting = false
        lastLanguage = lang
        codeSpot.restart()
    }
    // Format › Code Block: the selected text (or nothing yet) as code.
    function makeCodeBlock() {
        var a = body.selectionStart, b = body.selectionEnd
        var code = a !== b ? body.getText(a, b).replace(/[\u2029\u2028]/g, "\n") : ""
        insertCodeBlock(lastLanguage, code, a, b)
        body.forceActiveFocus()
    }
    // Colours brought up to date after typing in a block (a moment after the
    // typing stops), the cursor kept where it is.
    Timer { id: codeTimer; interval: 700; onTriggered: sheet.refreshCode() }
    function refreshCode() {
        if (converting) return
        var pos = body.cursorPosition
        var flick = bodyScroll.contentItem, top = flick ? flick.contentY : 0
        converting = true
        body.insert(pos, "\ue000")
        var html = store.refreshCodeBlocks(body.text, Theme.dark)
        if (html) { body.text = html; if (!takeMark()) body.cursorPosition = pos }
        else { body.remove(pos, pos + 1); body.cursorPosition = pos }
        if (flick) flick.contentY = top
        converting = false
    }
    // The block's language changed (the picker).
    function setCodeLanguage(lang) {
        var at = codeBlockAt(body.cursorPosition)
        if (!at) return
        var pos = body.cursorPosition
        converting = true
        body.remove(at.labelStart, at.labelStart + at.label.length)
        body.insert(at.labelStart, esc(lang))
        body.cursorPosition = pos - at.label.length + lang.length
        converting = false
        lastLanguage = lang
        refreshCode()
        codeSpot.restart()
        body.forceActiveFocus()
    }
    // Enter on a line of just ``` in a block: that line goes and the cursor
    // moves on past the block.
    function leaveCodeBlock() {
        var lb = lineBeforeCursor(), start = lb[0], line = lb[1].replace(/\u00a0/g, " ").trim()
        if (line !== "```") return false
        var pos = body.cursorPosition
        converting = true
        // (Also the line break before it, if any.)
        var from = start > 0 && /[\u2029\u2028]/.test(body.getText(start - 1, start)) ? start - 1 : start
        body.remove(from, pos)
        var t = body.getText(0, body.length)
        var end = t.indexOf("\ufdd1", from)
        if (end >= 0) {
            if (end + 1 >= body.length) body.insert(body.length, "<p></p>")
            body.cursorPosition = Math.min(body.length, end + 2)
        }
        converting = false
        refreshCode()
        codeSpot.restart()
        return true
    }
    // Pasting text written in Markdown: as formatting (and code blocks).
    function pasteMarkdown() {
        if (!markdown) return false
        plainPaste.text = ""
        plainPaste.paste()
        var html = plainPaste.text ? store.markdownHtml(plainPaste.text, Theme.dark) : ""
        if (!html) return false
        var a = body.selectionStart, b = body.selectionEnd
        converting = true
        if (b > a) body.remove(a, b)
        body.insert(a, html)
        converting = false
        return true
    }

    // Tab / Shift+Tab in a list: the item one level in or out (as in Apple Mail).
    function nestList(deeper) {
        var pos = body.cursorPosition
        if (body.selectionStart !== body.selectionEnd) return false
        converting = true
        body.insert(pos, "\ue000")
        var whole = body.text
        var html = store.nestListItem(whole, deeper)
        if (!html) {
            // In a table: Tab / Shift+Tab go to the next / previous cell.
            var m = whole.indexOf("\ue000")
            var inCell = m >= 0 && whole.lastIndexOf("<td", m) > whole.lastIndexOf("</td>", m)
            body.remove(pos, pos + 1)
            body.cursorPosition = pos
            converting = false
            if (inCell) {
                tableStep(deeper)
                return true
            }
            return false
        }
        body.text = html
        body.cursorPosition = Math.min(pos, body.length)
        converting = false
        return true
    }

    // The next (or previous) table cell: found in the editor's HTML, marked
    // there, and the cursor put where the mark lands.
    function tableStep(forward) {
        var pos = body.cursorPosition
        converting = true
        body.insert(pos, "\ue000")
        var h = body.text
        var m = h.indexOf("\ue000")
        var here = h.lastIndexOf("<td", m)
        h = h.replace("\ue000", "")
        var to = forward ? h.indexOf("<td", m) : h.lastIndexOf("<td", here - 1)
        if (to < 0) {
            // Past the last (or first) cell: stay put.
            body.remove(pos, pos + 1)
            body.cursorPosition = pos
            converting = false
            return
        }
        var inside = h.indexOf(">", to) + 1
        // Into the cell's first paragraph, if it has one.
        var para = /^\s*<p[^>]*>/.exec(h.slice(inside))
        if (para) inside += para[0].length
        body.text = h.slice(0, inside) + "\ue001" + h.slice(inside)
        for (var i = 0; i < body.length; i++) {
            if (body.getText(i, i + 1) === "\ue001") {
                body.remove(i, i + 1)
                body.cursorPosition = i
                break
            }
        }
        converting = false
    }

    // ---------------------------------------------- Format menu commands
    // Add Link (Ctrl+K): the selected words become a link.
    function addLink(url) {
        if (!url) return
        if (!/^[a-z]+:/i.test(url)) url = (url.indexOf("@") > 0 ? "mailto:" : "https://") + url
        var a = body.selectionStart, b = body.selectionEnd
        var label = a !== b ? body.getText(a, b) : url
        body.remove(a, b)
        body.insert(a, "<a href=\"" + esc(url) + "\">" + esc(label) + "</a>\u200b")
    }
    // Quote level: the selected lines become (or stop being) a quote.
    // Quote levels: the selected paragraphs (or the one with the cursor) one
    // level in or out, their formatting kept. Quoted text has a blue bar at the
    // left (drawn over the editor; real quotes in what's sent).
    function quoteMore() { changeQuote(1) }
    function quoteLess() { changeQuote(-1) }
    function changeQuote(step) {
        var a = body.selectionStart, b = body.selectionEnd
        converting = true
        body.insert(b, "\ue002")
        body.insert(a, "\ue000")
        var html = store.changeQuote(body.text, step)
        if (html) body.text = html
        else { body.remove(b + 1, b + 2); body.remove(a, a + 1) }
        converting = false
        body.select(a, b)
    }
    // Return in quoted text breaks the quote there (as in Apple Mail): a new,
    // unquoted line to write in, the rest of the quote after it.
    function splitQuote() {
        if (body.selectionStart !== body.selectionEnd || quoteLevelAt(body.cursorPosition) === 0) return false
        var pos = body.cursorPosition
        converting = true
        body.insert(pos, "\ue000")
        var html = store.splitQuote(body.text)
        if (!html) { body.remove(pos, pos + 1); converting = false; return false }
        body.text = html
        for (var i = Math.max(0, pos - 2); i < body.length; i++) {
            if (body.getText(i, i + 1) === "\ue001") { body.remove(i, i + 1); body.cursorPosition = i; break }
        }
        converting = false
        return true
    }
    // A paragraph's quote level, from where its text starts (each level is
    // indented 18 px; lists by 40, so the two can be told apart).
    function quoteLevelFromX(x) {
        var d = Math.round(x - body.leftPadding)
        for (var q = 0; q < 8; q++) { var r = d - 18 * q; if (r >= 0 && r % 40 === 0) return q }
        return 0
    }
    function quoteLevelAt(pos) {
        var s = body.getText(0, pos)
        var start = Math.max(s.lastIndexOf("\u2029"), s.lastIndexOf("\ufdd0"), s.lastIndexOf("\ufdd1")) + 1
        return quoteLevelFromX(body.positionToRectangle(start).x)
    }
    // The bars: one per quote level, beside each run of quoted paragraphs.
    property var quoteBars: []
    Timer { id: barTimer; interval: 120; onTriggered: sheet.computeBars() }
    function computeBars() {
        var s = body.getText(0, body.length), bars = [], open = [], lastBottom = 0, start = 0, seen = 0, inTable = false
        for (var i = 0; i <= s.length && seen < 4000; i++) {
            var c = i < s.length ? s.charCodeAt(i) : 0x2029
            if (c !== 0x2029 && c !== 0xfdd0 && c !== 0xfdd1) continue
            seen++
            var r0 = body.positionToRectangle(start), r1 = body.positionToRectangle(i)
            // (In a table — a code block or one made with the table button — no quote bars.)
            // (Nor on the empty line just after a table, which reports no proper place.)
            var q = inTable || (start === i && start > 0 && s.charCodeAt(start - 1) === 0xfdd1) ? 0 : quoteLevelFromX(r0.x)
            if (c === 0xfdd0) inTable = true
            else if (c === 0xfdd1) inTable = false
            for (var L = 1; L < Math.max(q + 1, open.length); L++) {
                if (L <= q && !open[L]) open[L] = { top: r0.y }
                else if (L > q && open[L]) { bars.push({ level: L, top: open[L].top, bottom: lastBottom }); open[L] = null }
            }
            if (q > 0) console.warn("ALMANAC-BARS q " + q + " at " + start + "-" + i + " x " + r0.x + " char " + s.charCodeAt(i).toString(16) + " prev " + (start > 0 ? s.charCodeAt(start - 1).toString(16) : ""))
            lastBottom = r1.y + r1.height
            start = i + 1
        }
        for (var k = 1; k < open.length; k++) if (open[k]) bars.push({ level: k, top: open[k].top, bottom: lastBottom })
        quoteBars = bars
    }
    onActiveChanged: if (active) barTimer.restart()
    // Bigger / Smaller: the selected text a step up or down.
    function resize(step) {
        var f = body.cursorSelection.font
        var px = f.pixelSize > 0 ? f.pixelSize : Theme.base + 1
        f.pixelSize = Math.max(9, Math.min(48, px + step))
        body.cursorSelection.font = f
    }
    // Paste and Match Style: what's on the clipboard, as plain text in the current style.
    function pastePlain(asQuote) {
        plainPaste.text = ""
        plainPaste.paste()
        var t = plainPaste.text
        if (!t) return
        var pos = body.cursorPosition
        if (body.selectionStart !== body.selectionEnd) { pos = body.selectionStart; body.remove(body.selectionStart, body.selectionEnd) }
        var html = esc(t).replace(/\n/g, "<br>")
        if (asQuote) html = "<p style=\"margin-left:18px;margin-right:0px\">" + html + "</p>"
        body.insert(pos, html)
    }
    TextEdit { id: plainPaste; visible: false; textFormat: TextEdit.PlainText }
    P.Menu {
        id: spellMenu
        property var lint: null
        P.MenuItem { visible: !!spellMenu.lint; enabled: false; text: spellMenu.lint ? spellMenu.lint.message : "" }
        P.MenuItem { visible: !!spellMenu.lint && !spellMenu.lint.suggestions.length; enabled: false; text: "No Guesses Found" }
        Instantiator {
            model: spellMenu.lint ? spellMenu.lint.suggestions : []
            onObjectAdded: (i, o) => spellMenu.insertItem(2 + i, o)
            onObjectRemoved: (i, o) => spellMenu.removeItem(o)
            delegate: P.MenuItem {
                required property string modelData
                text: modelData === "" ? "Delete “" + spellMenu.lint.word + "”" : modelData
                font.bold: true
                onTriggered: sheet.applySuggestion(spellMenu.lint, modelData)
            }
        }
        P.MenuSeparator { visible: !!spellMenu.lint }
        P.MenuItem { visible: !!spellMenu.lint; icon.name: "dialog-cancel"; text: "Ignore"; onTriggered: sheet.ignoreWord(spellMenu.lint.word) }
        P.MenuItem {
            visible: !!spellMenu.lint && spellMenu.lint.kind === "spelling"
            icon.name: "tools-check-spelling"; text: "Learn Spelling"
            onTriggered: { sheet.store.learnSpelling(spellMenu.lint.word); sheet.ignoreWord(spellMenu.lint.word) }
        }
        P.MenuSeparator { visible: !!spellMenu.lint }
        P.MenuItem { icon.name: "tools-check-spelling"; text: "Check Spelling While Typing"; checkable: true; checked: sheet.spellOn; onTriggered: sheet.setSpelling(!sheet.spellOn) }
        P.MenuItem { icon.name: "story-editor"; text: "Check Grammar With Spelling"; checkable: true; checked: sheet.grammarOn; enabled: sheet.spellOn; onTriggered: sheet.setGrammar(!sheet.grammarOn) }
    }

    // ------------------------------------------- spelling and grammar
    // Checked a moment after typing stops, offline (Harper), only what's
    // written (not the quoted original); problems are underlined with dots
    // (red: spelling, green: grammar) and right-click offers the fixes.
    property bool spellOn: true
    property bool grammarOn: true
    property int spellGen: Math.floor(Math.random() * 1000000) * 1000
    property var lints: []
    property bool lintsFresh: false
    // Words ignored in this message.
    property var ignored: []
    function setSpelling(on) { spellOn = on; store.setSetting("spell_check", JSON.stringify(on)); requestSpelling() }
    function setGrammar(on) { grammarOn = on; store.setSetting("grammar_check", JSON.stringify(on)); requestSpelling() }
    Timer { id: spellTimer; interval: 450; onTriggered: sheet.requestSpelling() }
    function textEdited() { spellGen++; lintsFresh = false; spellTimer.restart() }
    function requestSpelling() {
        spellGen++
        if (!spellOn || !active) { lints = []; return }
        var t = body.getText(0, body.length)
        // Up to the quoted original (a reply's "On … wrote:", a forward's header).
        var q = /(^|[\n\u2029])(On [^\n\u2029]{5,300} wrote:|Begin forwarded message:|-+ ?Forwarded message ?-+)/.exec(t)
        if (q) t = t.slice(0, q.index)
        store.checkSpelling(spellGen, t.slice(0, 30000).replace(/[\u2028\u2029]/g, "\n").replace(/\u00a0/g, " "), grammarOn)
    }
    onSpellOnChanged: if (!spellOn) lints = []
    Connections {
        target: sheet.store
        function onSpellingChanged() {
            var r = JSON.parse(sheet.store.spelling || "{}")
            if (r.generation !== sheet.spellGen) return
            // (Nothing in code blocks: names and symbols aren't misspellings.)
            sheet.lints = r.lints.filter(function(l) { return sheet.ignored.indexOf(l.word) < 0 && !sheet.inCodeText(l.start) })
            sheet.lintsFresh = true
        }
    }
    // Where a problem is on screen: one stretch per line it covers.
    function stretches(a, b) {
        var out = [], cur = null
        for (var p = a; p < b; p++) {
            var r = body.positionToRectangle(p), n = body.positionToRectangle(p + 1)
            if (!cur || Math.abs(r.y - cur.y) > 1) { if (cur) out.push(cur); cur = { x: r.x, y: r.y, h: r.height, w: 0 } }
            cur.w = Math.abs(n.y - r.y) <= 1 ? n.x - cur.x : Math.max(cur.w, r.x + 6 - cur.x)
        }
        if (cur) out.push(cur)
        return out
    }
    function lintAt(pos) {
        if (!lintsFresh) return null
        for (var i = 0; i < lints.length; i++) if (pos >= lints[i].start && pos <= lints[i].end) return lints[i]
        return null
    }
    function applySuggestion(l, text) {
        if (!lintsFresh || !l) return
        body.remove(l.start, l.end)
        if (text !== "") body.insert(l.start, esc(text))
        body.cursorPosition = l.start + text.length
    }
    function ignoreWord(w) { ignored = ignored.concat([w]); lints = lints.filter(function(l) { return l.word !== w }) }
    // Dark mode: colours the original (quoted or re-opened) set for a white
    // page are shown adapted to the dark one (black text light, white
    // backgrounds dark); what's sent and saved has the original colours.
    // Adapted colour → original.
    property var colourBack: ({})
    function setBodyHtml(html) {
        colourBack = {}
        html = html.replace(/<blockquote style="margin:0 0 0 0; padding-left:10px; border-left:2px solid #2f7cf6;">/g, '<blockquote style="margin-left:18px;margin-right:0px">')
        if (Theme.dark && /color/i.test(html)) {
            var r = JSON.parse(store.adaptColours(html, Theme.window.toString()))
            html = r.html
            colourBack = r.back
        }
        body.text = store.codeBlocksForEditing(html, Theme.dark)
    }

    // Formatting helpers on the current selection.
    function toggleFont(prop) {
        var f = body.cursorSelection.font
        f[prop] = !f[prop]
        body.cursorSelection.font = f
    }
    function setSize(px) { var f = body.cursorSelection.font; f.pixelSize = px; body.cursorSelection.font = f }

    Shortcut { context: Qt.ApplicationShortcut; enabled: sheet.active; sequence: "Ctrl+B"; onActivated: sheet.toggleFont("bold") }
    Shortcut { context: Qt.ApplicationShortcut; enabled: sheet.active; sequence: "Ctrl+K"; onActivated: linkPrompt.open() }
    Shortcut { context: Qt.ApplicationShortcut; enabled: sheet.active; sequence: "Ctrl+'"; onActivated: sheet.quoteMore() }
    Shortcut { context: Qt.ApplicationShortcut; enabled: sheet.active; sequence: "Ctrl+Alt+'"; onActivated: sheet.quoteLess() }
    Shortcut { context: Qt.ApplicationShortcut; enabled: sheet.active && body.activeFocus; sequences: ["Ctrl++", "Ctrl+="]; onActivated: sheet.resize(2) }
    Shortcut { context: Qt.ApplicationShortcut; enabled: sheet.active && body.activeFocus; sequence: "Ctrl+-"; onActivated: sheet.resize(-2) }
    Shortcut { context: Qt.ApplicationShortcut; enabled: sheet.active && body.activeFocus; sequence: "Ctrl+Shift+V"; onActivated: sheet.pastePlain(false) }
    Shortcut { context: Qt.ApplicationShortcut; enabled: sheet.active && body.activeFocus; sequence: "Ctrl+Alt+Shift+V"; onActivated: sheet.pastePlain(true) }
    Shortcut { context: Qt.ApplicationShortcut; enabled: sheet.active; sequence: "Ctrl+I"; onActivated: sheet.toggleFont("italic") }
    Shortcut { context: Qt.ApplicationShortcut; enabled: sheet.active; sequence: "Ctrl+U"; onActivated: sheet.toggleFont("underline") }

    component FmtButton: ToolButtonX { implicitWidth: 30 }
    component FmtSep: Item { width: 9; height: 30; Rectangle { anchors.centerIn: parent; width: 1; height: 20; color: Theme.border } }
    // Turns the selected lines (or the current line) into a real list.
    function makeList(tag) {
        var a = body.selectionStart, b = body.selectionEnd
        var lines = body.getText(a, b).split(/\u2029|\n/).filter(function(l) { return l.trim() !== "" })
        var esc = function(t) { return t.replace(/&/g, "&amp;").replace(/</g, "&lt;") }
        if (a !== b) body.remove(a, b)
        body.insert(a, "<" + tag + ">" + (lines.length ? lines : [""]).map(function(l) { return "<li>" + esc(l) + "</li>" }).join("") + "</" + tag + ">")
    }

    // Formatting bar: its groups wrap onto another line where there isn't room.
    Flow {
        id: fmtRow
        Layout.fillWidth: true
        Layout.leftMargin: 10; Layout.rightMargin: 10
        Layout.topMargin: 3; Layout.bottomMargin: 3
        Layout.preferredHeight: implicitHeight
        spacing: 2
        Row {
            spacing: 2
            FmtButton { text: "B"; tip: "Bold (Ctrl+B)"; checked: body.cursorSelection.font.bold; onClicked: sheet.toggleFont("bold") }
            FmtButton { text: "I"; tip: "Italic (Ctrl+I)"; checked: body.cursorSelection.font.italic; onClicked: sheet.toggleFont("italic") }
            FmtButton { text: "U"; tip: "Underline (Ctrl+U)"; checked: body.cursorSelection.font.underline; onClicked: sheet.toggleFont("underline") }
            FmtButton { text: "S"; tip: "Strikethrough"; checked: body.cursorSelection.font.strikeout; onClicked: sheet.toggleFont("strikeout") }
            FmtSep {}
        }
        // Font, size, colour and tables.
        Row {
            spacing: 2
            FormatPickers { id: fmtPickers; target: body; anchors.verticalCenter: parent.verticalCenter }
            FmtSep {}
        }
        Row {
            spacing: 2
            FmtButton { icon: "alignLeft"; tip: "Align Left"; onClicked: body.cursorSelection.alignment = Qt.AlignLeft }
            FmtButton { icon: "alignCenter"; tip: "Center"; onClicked: body.cursorSelection.alignment = Qt.AlignHCenter }
            FmtButton { icon: "alignRight"; tip: "Align Right"; onClicked: body.cursorSelection.alignment = Qt.AlignRight }
            FmtButton { icon: "alignJustify"; tip: "Justify"; onClicked: body.cursorSelection.alignment = Qt.AlignJustify }
            FmtSep {}
        }
        Row {
            spacing: 2
            FmtButton { icon: "link"; tip: "Add Link (Ctrl+K)"; onClicked: linkPrompt.open() }
            FmtButton { icon: "quote"; tip: "Increase Quote Level (Ctrl+')"; onClicked: sheet.quoteMore() }
            FmtSep {}
        }
        Row {
            spacing: 2
            FmtButton { icon: "listBullet"; tip: "Bulleted List"; onClicked: sheet.makeList("ul") }
            FmtButton { icon: "listNumber"; tip: "Numbered List"; onClicked: sheet.makeList("ol") }
            FmtButton {
                implicitWidth: 34
                text: "{ }"
                checked: sheet.codeCursor !== null
                tip: "Code Block (or type ``` and a language, then Return)"
                onClicked: sheet.makeCodeBlock()
            }
            // The language of the code block the cursor is in.
            ComboX {
                id: codeLang
                visible: sheet.codeCursor !== null
                width: 130
                anchors.verticalCenter: parent.verticalCenter
                model: sheet.codeLanguages
                currentIndex: sheet.codeCursor ? sheet.codeLanguages.indexOf(sheet.codeCursor.label) : -1
                onActivated: function(i) { sheet.setCodeLanguage(sheet.codeLanguages[i]) }
                ToolTip.visible: hovered
                ToolTip.text: "Language of this code block"
            }
            FmtSep {}
        }
        FmtButton {
            implicitWidth: 42
            text: "ABC"
            checked: sheet.spellOn
            tip: sheet.spellOn ? "Checking spelling" + (sheet.grammarOn ? " and grammar" : "") + " as you type (right-click an underlined word for fixes). Click to turn off."
                               : "Check spelling and grammar as you type"
            onClicked: sheet.setSpelling(!sheet.spellOn)
        }
        FmtButton {
            implicitWidth: 38
            text: "Md"
            checked: sheet.markdown
            tip: sheet.markdown ? "Markdown turns into formatting as you type (**bold**, *italic*, # heading, - list…). Click to turn off."
                                : "Turn Markdown into formatting as you type"
            onClicked: sheet.markdown = !sheet.markdown
        }
    }
    Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.borderSoft }

    ScrollView {
        id: bodyScroll
        Layout.fillWidth: true
        Layout.fillHeight: true
        TextArea {
            id: body
            textFormat: TextEdit.RichText
            wrapMode: TextEdit.Wrap
            font.pixelSize: Theme.base + 1
            color: Theme.text
            selectByMouse: true
            persistentSelection: true
            leftPadding: 16; rightPadding: 16; topPadding: 12
            background: null
            onTextChanged: {
                if (sheet.markdown && !sheet.converting) Qt.callLater(sheet.inlineMarkdown)
                // Typing in a code block: coloured again once the typing stops.
                if (!sheet.converting && sheet.codeCursor !== null) codeTimer.restart()
                sheet.textEdited(); barTimer.restart()
            }
            onCursorPositionChanged: codeSpot.restart()
            // Markdown pasted: as formatting.
            Keys.onPressed: function(ev) { if (ev.matches(StandardKey.Paste) && sheet.pasteMarkdown()) ev.accepted = true }
            // The underlines (they move with the text; hidden while it's being changed).
            Repeater {
                model: sheet.lintsFresh ? sheet.lints : []
                delegate: Item {
                    id: lintMark
                    required property var modelData
                    Repeater {
                        model: { body.width; body.contentHeight; return sheet.stretches(lintMark.modelData.start, lintMark.modelData.end) }
                        delegate: Canvas {
                            required property var modelData
                            x: modelData.x; y: modelData.y + modelData.h - 2
                            width: Math.max(4, modelData.w); height: 3
                            readonly property color ink: lintMark.modelData.kind === "spelling" ? Theme.red : "#30a14e"
                            onPaint: {
                                var c = getContext("2d")
                                c.reset()
                                c.fillStyle = ink
                                for (var dx = 1; dx < width; dx += 3) { c.beginPath(); c.arc(dx, 1.5, 1.1, 0, 2 * Math.PI); c.fill() }
                            }
                        }
                    }
                }
            }
            // Right-click on an underlined word: its fixes.
            TapHandler {
                acceptedButtons: Qt.RightButton
                onTapped: function(ev) {
                    var l = sheet.lintAt(body.positionAt(ev.position.x, ev.position.y))
                    spellMenu.lint = l
                    spellMenu.open()
                }
            }
            Keys.onReturnPressed: function(ev) { ev.accepted = sheet.blockMarkdown() || sheet.splitQuote() }
            Keys.onEnterPressed: function(ev) { ev.accepted = sheet.blockMarkdown() || sheet.splitQuote() }
            onWidthChanged: barTimer.restart()
            Repeater {
                model: sheet.quoteBars
                delegate: Rectangle {
                    required property var modelData
                    x: body.leftPadding + 18 * (modelData.level - 1) + 4
                    y: modelData.top
                    width: 2; height: Math.max(0, modelData.bottom - modelData.top)
                    radius: 1
                    color: Theme.accent
                }
            }
            Keys.onTabPressed: function(ev) {
                if (sheet.codeHere()) { body.insert(body.cursorPosition, "\u00a0\u00a0\u00a0\u00a0"); ev.accepted = true; return }
                ev.accepted = sheet.nestList(true)
            }
            // Shift+Tab outside a list stays in the message (doesn't jump to another field).
            Keys.onBacktabPressed: function(ev) { sheet.nestList(false); ev.accepted = true }
        }
    }

    Popup {
        id: linkPrompt
        parent: sheet.popupParent
        anchors.centerIn: parent
        width: 380
        padding: 16
        modal: true
        background: Rectangle { radius: 10; color: Theme.popup; border.color: Theme.border }
        onOpened: { linkField.text = ""; linkField.forceActiveFocus() }
        contentItem: Column {
            spacing: 10
            Text { text: "Add Link"; font.pixelSize: 15; font.weight: Font.DemiBold; color: Theme.text }
            FieldX { id: linkField; width: 348; placeholderText: "https://example.com or name@example.com"; onAccepted: okLink.clicked() }
            Row {
                anchors.right: parent.right
                spacing: 8
                ToolButtonX { text: "Cancel"; onClicked: linkPrompt.close() }
                ToolButtonX { id: okLink; text: "Add Link"; primary: true; onClicked: { linkPrompt.close(); body.forceActiveFocus(); sheet.addLink(linkField.text.trim()) } }
            }
        }
    }
}
