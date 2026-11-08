import QtQuick
import QtQuick.Controls.Basic
// The system's own menus (desktop style on Linux, native on macOS and Windows)
import Qt.labs.platform as P
import QtQuick.Layouts
import QtQuick.Dialogs

// Mail: mailboxes on the left, the message list (grouped into
// conversations), and the open conversation. Its buttons live in the
// window's shared top bar (`tools`).
Item {
    id: mail
    property var store
    property bool sidebarShown: true
    property string selection: "kind:inbox"
    onSelectionChanged: { selectedIds = []; resetPage() }
    property string selectionName: "Inbox"
    property var selectedId: -1
    // More than one row picked (Ctrl-click, Shift-click, Ctrl+A); empty otherwise.
    property var selectedIds: []
    property int anchorIndex: -1
    readonly property bool multi: selectedIds.length > 1
    function isPicked(id) { return id === selectedId || selectedIds.indexOf(id) >= 0 }
    // Clicks with Ctrl (add or remove one) or Shift (a range), as in Apple Mail.
    function pick(id, index, modifiers) {
        if (modifiers & Qt.ShiftModifier && anchorIndex >= 0) {
            var a = Math.min(anchorIndex, index), b = Math.max(anchorIndex, index), ids = []
            for (var i = a; i <= b && i < list.length; i++) ids.push((list[i] || {}).id)
            selectedIds = ids
            selectedId = id
            return
        }
        if (modifiers & Qt.ControlModifier) {
            var cur = selectedIds.length ? selectedIds.slice() : (selectedId >= 0 ? [selectedId] : [])
            var at = cur.indexOf(id)
            if (at >= 0) cur.splice(at, 1); else cur.push(id)
            selectedIds = cur
            selectedId = cur.length ? cur[cur.length - 1] : -1
            anchorIndex = index
            return
        }
        selectedIds = []
        anchorIndex = index
        select(id)
    }
    function selectAll() { selectedIds = list.filter(function(r) { return !!r }).map(function(r) { return r.id }); if (list.length && list[0]) selectedId = (list[0] || {}).id }
    property string query: ""
    property bool unreadOnly: false
    property bool threaded: true
    // Show messages as the sender formatted them (or as plain text).
    property bool formatted: true
    // Message text size: pinch on the trackpad, or Ctrl + scroll / Ctrl +/-.
    property real textZoom: 1
    // Messages whose remote pictures the person chose to load (this session).
    property var remoteLoaded: ({})
    function loadRemote(id) { var r = Object.assign({}, remoteLoaded); r[id] = true; remoteLoaded = r }
    function hasRemote(html) { return /data-remote-(src|background)=|remote-url\(/.test(html || "") }
    function withRemote(html) { return html.replace(/data-remote-src=/g, "src=").replace(/data-remote-background=/g, "background=").replace(/remote-url\(/g, "url(") }
    // A message that sets its own colours or backgrounds (a designed email).
    function ownLook(html) { return /bgcolor|background|color\s*[:=]/i.test(html || "") }
    signal focusSearch()
    signal accountsRequested()
    property var prefs: { store.revision; return JSON.parse(store.settings()) }
    property bool hasAccounts: { store.revision; return JSON.parse(store.liveAccounts()).length > 0 }
    // The shown mailbox is on a mail server: older mail and server search apply.
    property bool serverBox: { store.revision; return store.isServerMailbox(selection) }
    Component.onCompleted: { threaded = prefs.conversations; formatted = prefs.show_formatted; Qt.callLater(winReset) }
    // Settings changes apply straight away.
    readonly property bool prefConversations: prefs.conversations
    readonly property bool prefFormatted: prefs.show_formatted
    onPrefConversationsChanged: threaded = prefConversations
    onPrefFormattedChanged: formatted = prefFormatted
    readonly property var rulesDlg: rulesDialog
    readonly property var signaturesDlg: signaturesDialog
    readonly property var outboxDlg: outboxDialog
    function newMessage() { compose.openWith({}) }
    function newSmartMailbox() { smartDialog.openFor("") }

    property var sidebar: { store.revision; return JSON.parse(store.mailSidebar()) }
    // The list a page at a time (sorted and filtered by Rust), so a mailbox
    // with tens of thousands of messages stays quick; more rows load as you
    // scroll near the end (or press End / Page Down).
    readonly property int pageSize: 300
    property int limit: pageSize
    property var page: windowed ? ({ rows: winRows, total: winTotal, all: winTotal, unread: winUnread, countsMessages: !threaded })
                                : (store.revision, JSON.parse(store.messagePage(selection, query, threaded, sortBy, sortAscending, unreadOnly, keepId, limit)))

    // ---- A mailbox's list read a page at a time (by date, no search): the
    // list is as long as the mailbox from the start, rows not read yet show as
    // placeholders, and only the pages around what's on screen are kept. Pages
    // are read in the background.
    readonly property int winSize: 100
    readonly property int winKeep: 12
    property bool windowed: false
    property int winTotal: 0
    property int winUnread: 0
    property int winGen: 0
    property var winRows: []
    property var winLoaded: ({})
    property var winPending: ({})
    property int winWant: -1
    readonly property string winKey: [selection, query, threaded, sortBy, sortAscending, unreadOnly].join("\u0001")
    onWinKeyChanged: Qt.callLater(winReset)
    function placeholderRow(i) { return { id: -1 - i, placeholder: true, fromName: "", fromEmail: "", subject: "", preview: "", date: 0, read: true, flagged: false, attachments: false } }
    function winReset() {
        var on = store.canWindow(selection, query, sortBy, sortAscending)
        winGen++
        winLoaded = ({}); winPending = ({}); winRows = []; winTotal = 0; winUnread = 0
        rowsModel.clear()
        winModel.clear()
        windowed = on
        if (!on) { syncRows(); return }
        // The first page, and where the open message is (found in the background;
        // its page is read when that's known).
        winWant = -1
        winFetch(0, true)
        winLocate()
    }
    property var winLocating: -1
    function winLocate() {
        if (selectedId < 0 || winLocating === selectedId) return
        winLocating = selectedId
        store.requestPosition(winGen, selection, query, threaded, sortBy, sortAscending, unreadOnly, selectedId)
    }
    function winFetch(p, counted) {
        // (A page already asked for is asked again only for the list's length.)
        if (p < 0 || (winPending[p] === winGen && !counted)) return
        winPending[p] = winGen
        store.requestWindow(winGen, selection, query, threaded, sortBy, sortAscending, unreadOnly, selectedId, p * winSize, winSize, !!counted)
    }
    // Mail changed: what's on screen is read again (and how long the list is).
    Timer { id: winRefresh; interval: 300; onTriggered: { mail.winLoaded = ({}); mail.winPending = ({}); mail.winEnsure(true) } }
    Connections {
        target: store
        function onRevisionChanged() {
            if (mail.windowed) winRefresh.restart()
            // (Conversations just worked out, the first time: paging from now on.)
            else if (store.canWindow(mail.selection, mail.query, mail.sortBy, mail.sortAscending)) Qt.callLater(mail.winReset)
        }
    }
    Connections {
        target: store
        function onWindowJsonChanged() {
            var w = JSON.parse(store.windowJson || "{}")
            if (!mail.windowed || w.token !== mail.winGen) return
            // Where the open message is: its page read, and it's gone to there.
            if (w.locate !== undefined) {
                mail.winLocating = -1
                if (w.locate < 0 || w.id !== mail.selectedId) return
                if (w.locate < mail.winTotal && mail.winRows[w.locate]) { messageList.currentIndex = w.locate; Qt.callLater(mail.revealSelected) }
                else { mail.winWant = w.locate; mail.winFetch(Math.floor(w.locate / mail.winSize)) }
                return
            }
            var p = Math.floor(w.offset / mail.winSize)
            delete mail.winPending[p]
            var rows = mail.winRows
            if (w.total >= 0) {
                mail.winUnread = w.unread
                var n = winModel.count
                if (n === 0 && w.total > 0) {
                    var ph = []
                    for (var i = 0; i < w.total; i++) ph.push({ r: 0 })
                    winModel.append(ph)
                    // (null, not empty: QML drops an array's empty slots at the end.)
                    rows = new Array(w.total).fill(null)
                } else if (w.total > n) {
                    // New mail (at the top): room for it there, and what's below kept.
                    var more = []
                    for (var j = 0; j < w.total - n; j++) more.push({ r: 0 })
                    winModel.insert(0, more)
                    rows = new Array(w.total - n).fill(null).concat(rows)
                    mail.winLoaded = ({})
                } else if (w.total < n) {
                    winModel.remove(w.total, n - w.total)
                    rows = rows.slice(0, w.total)
                    mail.winLoaded = ({})
                }
                mail.winTotal = w.total
            }
            for (var k = 0; k < w.rows.length; k++) {
                var at = w.offset + k
                if (at >= winModel.count) break
                rows[at] = w.rows[k]
            }
            // (Its length not known yet: asked for.)
            if (w.total < 0 && winModel.count === 0) { mail.winFetch(p, true); return }
            mail.winLoaded[p] = Date.now()
            mail.winTrim(p)
            mail.winRows = rows.slice()
            if (mail.winWant >= 0) console.warn("DBGEND reply off=" + w.offset + " want=" + mail.winWant + " has=" + !!rows[mail.winWant])
            if (mail.winWant >= 0 && mail.winWant < mail.winTotal && rows[mail.winWant]) {
                var want = mail.winWant
                mail.winWant = -1
                if (rows[want].id !== mail.selectedId && !(rows[want].ids && rows[want].ids.indexOf(mail.selectedId) >= 0)) mail.select(rows[want].id)
                messageList.currentIndex = want
                messageList.positionViewAtIndex(want, ListView.Contain)
            }
            Qt.callLater(mail.winEnsure)
        }
    }
    // The pages around what's on screen are read (a page above and below too).
    function winEnsure(counted) {
        if (!windowed) return
        var lv = messageList
        var top = lv.indexAt(10, lv.contentY + 1), bottom = lv.indexAt(10, lv.contentY + lv.height - 1)
        if (top < 0) top = 0
        if (bottom < 0) bottom = Math.min(winTotal - 1, top + Math.ceil(lv.height / lv.rowEstimate))
        var p0 = Math.max(0, Math.floor((top - 30) / winSize)), p1 = Math.floor((bottom + 30) / winSize)
        var asked = false
        for (var p = p0; p <= p1 && p * winSize < Math.max(winTotal, 1); p++) {
            if (winLoaded[p] === undefined) { winFetch(p, counted && !asked); asked = true }
        }
        if (counted && !asked) winFetch(p0, true)
    }
    // Only the pages near `keep` are kept: the rest go back to placeholders.
    function winTrim(keep) {
        var pages = Object.keys(winLoaded).map(Number)
        if (pages.length <= winKeep) return
        pages.sort(function(a, b) { return Math.abs(b - keep) - Math.abs(a - keep) })
        for (var i = 0; i < pages.length - winKeep; i++) {
            var p = pages[i]
            delete winLoaded[p]
            for (var at = p * winSize; at < Math.min(winModel.count, (p + 1) * winSize); at++) {
                if (winRows[at] && winRows[at].id === selectedId) continue
                winRows[at] = null
            }
        }
    }
    // The open message is always in the page (a moment after it's chosen, so
    // choosing doesn't re-read the list while it's being read).
    property var keepId: -1
    onSelectedIdChanged: { compose.leaveInline(); Qt.callLater(function() {
        if (mail.keepId === mail.selectedId) return
        // (With "only unread", the open one is kept in view once it's read.)
        if (!mail.unreadOnly) for (var i = 0; i < mail.list.length; i++) if ((mail.list[i] || {}).id === mail.selectedId || ((mail.list[i] || {}).ids && (mail.list[i] || {}).ids.indexOf(mail.selectedId) >= 0)) return
        mail.keepId = mail.selectedId
    }) }
    function resetPage() { limit = pageSize }
    onQueryChanged: resetPage()
    onThreadedChanged: resetPage()
    onUnreadOnlyChanged: resetPage()
    onSortByChanged: resetPage()
    // Rows so far → at least `n` rows.
    function loadRows(n) { if (n > limit && list.length < page.total) limit = Math.min(page.total, Math.max(n, limit + pageSize)) }
    // Sort order (☰ › View › Sort By, or the "Sorted by" line above the list), as in Apple Mail.
    readonly property string sortBy: prefs.sort_by || "date"
    readonly property bool sortAscending: !!prefs.sort_ascending
    readonly property var sortNames: ({ date: "Date", from: "From", subject: "Subject", to: "To", attachments: "Attachments", flags: "Flags", unread: "Unread" })
    function setSort(key, ascending) { store.setSetting("sort_by", JSON.stringify(key)); store.setSetting("sort_ascending", JSON.stringify(ascending)) }
    property var list: page.rows
    property var folders: { store.revision; return JSON.parse(store.mailFolders()) }
    property var currentRow: { for (var i = 0; i < list.length; i++) if ((list[i] || {}).id === selectedId) return list[i]; return null }
    property var conversation: { store.revision; return selectedId >= 0 ? (threaded ? JSON.parse(store.thread(selection, selectedId)) : [JSON.parse(store.message(selectedId))]) : [] }
    property var current: conversation.length ? conversation[conversation.length - 1] : null
    property int unreadCount: page.unread

    // The rows on screen, updated in place (rows are added, moved, changed or
    // removed one by one), so the list keeps its scroll position and
    // selection while mail arrives in the background.
    ListModel { id: rowsModel; dynamicRoles: true }
    // (Windowed: one small entry per row; each row's contents come from winRows.)
    ListModel { id: winModel }
    readonly property alias listView: messageList
    // What's on screen, saved when Almanac closes and put back when it opens.
    // Pane sizes from when this view was last on screen (a hidden view's panes
    // have no real size).
    property var paneSizes: ({})
    // Accounts whose mailboxes are folded away in the sidebar (kept between runs).
    property var foldedAccounts: ({})
    function foldAccount(id, fold) { var f = Object.assign({}, foldedAccounts); if (fold) f[id] = true; else delete f[id]; foldedAccounts = f }
    // Each account's inbox, for the list under the unified Inbox.
    property bool inboxesOpen: true
    readonly property var accountInboxes: {
        var out = []
        var accts = sidebar.accounts || []
        for (var i = 0; i < accts.length; i++)
            for (var j = 0; j < accts[i].folders.length; j++)
                if (accts[i].folders[j].kind === "inbox") { out.push({ id: accts[i].folders[j].id, account: accts[i].name, badge: accts[i].folders[j].badge }); break }
        return out
    }
    function saveState() {
        var on = visible && width > 0
        return { sidebar: sidebarShown,
                 sideW: on && sidebarShown ? sidebarPane.width : paneSizes.sideW,
                 listW: on && !classic ? listPane.width : paneSizes.listW,
                 listH: on && classic ? listPane.height : paneSizes.listH,
                 selection: selection, selectionName: selectionName, selectedId: selectedId,
                 threaded: threaded, unreadOnly: unreadOnly, formatted: formatted, zoom: textZoom,
                 listY: messageList.contentY, folded: foldedAccounts }
    }
    function restoreState(s) {
        if (!s) return
        paneSizes = { sideW: s.sideW, listW: s.listW, listH: s.listH }
        foldedAccounts = s.folded || {}
        sidebarShown = s.sidebar !== false
        if (s.sideW > 60) sidebarPane.SplitView.preferredWidth = s.sideW
        if (s.listW > 120) listPane.SplitView.preferredWidth = s.listW
        if (s.listH > 80) listPane.SplitView.preferredHeight = s.listH
        if (s.selection) { selection = s.selection; selectionName = s.selectionName || selectionName }
        if (s.threaded !== undefined) threaded = s.threaded
        if (s.unreadOnly !== undefined) unreadOnly = s.unreadOnly
        if (s.formatted !== undefined) formatted = s.formatted
        if (s.zoom > 0) textZoom = s.zoom
        // The open message (without marking anything read) and where the list was scrolled.
        if (s.selectedId !== undefined && s.selectedId >= 0) {
            var id = s.selectedId, y = s.listY || 0
            Qt.callLater(function() {
                var found = JSON.parse(mail.store.message(id))
                if (found && found.id !== undefined) mail.select(id, true)
                Qt.callLater(function() { messageList.contentY = y })
            })
        }
    }
    // "standard" (list beside the message) or "classic" (list above the message, compact rows)
    readonly property bool classic: prefs.mail_layout === "classic"
    function setLayout(name) { store.setSetting("mail_layout", JSON.stringify(name)) }
    property int lastIndex: 0
    function syncRows() {
        if (windowed) return
        var rows = list, n = rowsModel.count
        var top = 0
        while (top < rows.length && top < n && rowsModel.get(top).rid === rows[top].id) top++
        var bottom = 0
        while (bottom < rows.length - top && bottom < n - top && rowsModel.get(n - 1 - bottom).rid === rows[rows.length - 1 - bottom].id) bottom++
        // Rows kept: update the ones whose contents changed (read, flagged…).
        var check = function(modelAt, r) {
            var json = JSON.stringify(r)
            if (rowsModel.get(modelAt).json !== json) rowsModel.set(modelAt, { rid: r.id, json: json, msg: r })
        }
        for (var i = 0; i < top; i++) check(i, rows[i])
        for (var b = 0; b < bottom; b++) check(n - 1 - b, rows[rows.length - 1 - b])
        // The part in between: out with the old, in with the new.
        var gone = n - top - bottom
        if (gone > 0) rowsModel.remove(top, gone)
        var fresh = []
        for (var k = top; k < rows.length - bottom; k++) fresh.push({ rid: rows[k].id, json: JSON.stringify(rows[k]), msg: rows[k] })
        if (fresh.length === 0) return
        // Scrolled (or dragged) into the room kept for rows not loaded yet:
        // the list would make every row between the loaded ones and where it's
        // scrolled to. Instead it moves next to the loaded rows for the change,
        // then jumps straight to the row it was scrolled to.
        var lv = messageList, deepIndex = -1
        var loadedEnd = lv.contentHeight - (lv.footerItem ? lv.footerItem.height : 0)
        if (rowsModel.count > 0 && lv.contentY - lv.originY > loadedEnd) {
            deepIndex = rowsModel.count + Math.floor((lv.contentY - lv.originY - loadedEnd) / lv.rowEstimate)
            lv.contentY = lv.originY + Math.max(0, loadedEnd - lv.height)
        }
        if (top === rowsModel.count) rowsModel.append(fresh)
        else for (var f = 0; f < fresh.length; f++) rowsModel.insert(top + f, fresh[f])
        if (deepIndex >= 0) lv.positionViewAtIndex(Math.min(deepIndex, rowsModel.count - 1), ListView.Beginning)
    }
    onListChanged: {
        syncRows()
        var idx = -1
        for (var i = 0; i < list.length; i++) if ((list[i] || {}).id === selectedId) { idx = i; break }
        if (idx < 0 && selectedId >= 0) {
            // The open conversation got a newer message: its row has the newest
            // message's id now. Stay on it.
            for (var k = 0; k < list.length && idx < 0; k++) if ((list[k] || {}).ids && (list[k] || {}).ids.indexOf(selectedId) >= 0) { idx = k; selectedId = (list[k] || {}).id }
        }
        if (windowed && idx < 0 && selectedId >= 0) {
            // Not among the rows read: where it is, if it's still in the list.
            winLocate()
            return
        }
        if (windowed && list.length > 0 && !list.some(function(r) { return r !== undefined })) return
        if (!windowed && idx < 0 && selectedId >= 0 && list.length > 0) {
            // Still in this list, only further down than the rows read so far
            // (Only Unread turned off, a search cleared): read that far at once
            // and stay on it (the list comes back here with it in).
            var before = store.rowsBefore(selection, query, sortBy, sortAscending, unreadOnly, selectedId)
            // (Later: changing the page from inside its own change isn't seen.)
            if (before >= list.length && list.length < page.total) { Qt.callLater(function() { mail.loadRows(before + mail.pageSize) }); return }
        }
        if (idx < 0 && list.length > 0) {
            // Nothing selected yet, or the selected message went away: the one
            // now in its place (not the top of the list).
            idx = Math.min(selectedId < 0 && lastIndex === 0 ? 0 : lastIndex, list.length - 1)
            if (list[idx]) select((list[idx] || {}).id)
        }
        if (idx >= 0) lastIndex = idx
        if (messageList.currentIndex !== idx) messageList.currentIndex = idx
        // Out of sight after the list changed (a filter turned off, a search
        // cleared): straight to it, in the middle.
        if (idx >= 0) Qt.callLater(mail.revealSelected)
    }
    function revealSelected() {
        var lv = messageList, i = lv.currentIndex
        if (i < 0 || i >= lv.count) return
        var it = lv.itemAtIndex(i)
        var shown = it && it.y >= lv.contentY && it.y + it.height <= lv.contentY + lv.height
        if (!shown) lv.positionViewAtIndex(i, ListView.Center)
    }

    // Every message the next action applies to: all picked rows (whole
    // conversations), or the selected one.
    function rowIds() {
        if (multi) {
            var out = []
            for (var i = 0; i < list.length; i++) if (selectedIds.indexOf((list[i] || {}).id) >= 0) out = out.concat((list[i] || {}).ids || [(list[i] || {}).id])
            return out
        }
        return currentRow && currentRow.ids ? currentRow.ids : (selectedId >= 0 ? [selectedId] : [])
    }
    function indexOfSelected() { for (var i = 0; i < list.length; i++) if ((list[i] || {}).id === selectedId) return i; return messageList.currentIndex }
    function select(id, keepUnread) {
        selectedIds = []
        selectedId = id
        if (id < 0) return
        markReadTimer.stop()
        if (keepUnread || !mail.prefs.mark_read_on_open) return
        // Read once it has stayed open half a second (just passing over it with
        // the arrow keys leaves it unread), as in Apple Mail.
        markReadTimer.forId = id
        markReadTimer.restart()
    }
    Timer {
        id: markReadTimer
        property var forId: -1
        interval: 500
        onTriggered: {
            if (mail.selectedId !== forId || mail.multi) return
            var ids = mail.rowIds()
            for (var i = 0; i < ids.length; i++) store.setRead(ids[i], true)
        }
    }
    function neighbour() {
        var i = messageList.currentIndex
        if (list.length <= 1) return -1
        return i + 1 < list.length ? (list[i + 1] || {}).id : (list[i - 1] || {}).id
    }
    function removeCurrent(action, sure) {
        if (selectedId < 0) return
        var ids = rowIds()
        // More than the one message in view (a whole conversation, or several
        // picked): said how many first, so a whole thread never goes unseen.
        if (!sure && ids.length > 1) {
            var rows = multi ? selectedIds.length : 1
            var where = action === "delete" ? "Trash" : action === "archive" ? "Archive" : action === "junk" ? "Junk" : action
            confirm.ask("Move " + ids.length + " messages to " + where + "?",
                        rows > 1 ? "The " + rows + " conversations picked have " + ids.length + " messages in all. Edit › Undo (Ctrl+Z) puts them back."
                                 : "This conversation has " + ids.length + " messages, and all of them go. Edit › Undo (Ctrl+Z) puts them back.",
                        "Move to " + where, function() { mail.removeCurrent(action, true) })
            return
        }
        var next = multi ? -1 : neighbour()
        selectedId = -1
        selectedIds = []
        if (action === "delete") store.deleteMessages(JSON.stringify(ids))
        else for (var i = 0; i < ids.length; i++) store.moveTo(ids[i], action)
        if (next >= 0) select(next)
    }
    function moveCurrentToFolder(folderId) {
        var ids = rowIds()
        var next = multi ? -1 : neighbour()
        selectedId = -1
        selectedIds = []
        for (var i = 0; i < ids.length; i++) store.moveToFolder(ids[i], folderId)
        if (next >= 0) select(next)
    }
    function setCurrentRead(read) { var ids = rowIds(); for (var i = 0; i < ids.length; i++) store.setRead(ids[i], read) }
    // Unflag clears every flag in the conversation; Flag flags the newest message.
    // Flags in a colour (0 red … 6 gray; -1 clears the flag): the picked rows, or
    // the open conversation's flagged messages (else its newest).
    function flagIcon(color) {
        return "data:image/svg+xml;utf8," + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M4 2v12" stroke="' + Theme.flagColors[color] + '" stroke-width="1.6" stroke-linecap="round"/><path d="M4.8 2.6h7.4l-1.8 2.9 1.8 2.9H4.8z" fill="' + Theme.flagColors[color] + '"/></svg>')
    }
    function setCurrentFlag(color) {
        var ids = []
        if (multi) ids = list.filter(function(r) { return selectedIds.indexOf(r.id) >= 0 }).map(function(r) { return r.id })
        else if (current) {
            var flagged = conversation.filter(function(m) { return m.flagged })
            ids = (flagged.length ? flagged : [current]).map(function(m) { return m.id })
        }
        for (var i = 0; i < ids.length; i++) store.setFlagColor(ids[i], color)
    }
    function toggleCurrentFlag() {
        if (multi) {
            // Several rows: flag them all, or unflag them all if every one is flagged.
            var picked = list.filter(function(r) { return selectedIds.indexOf(r.id) >= 0 })
            var allFlagged = picked.every(function(r) { return r.flagged })
            for (var j = 0; j < picked.length; j++) if (picked[j].flagged === allFlagged) store.toggleFlag(picked[j].id)
            return
        }
        if (!current) return
        var flagged = conversation.filter(function(m) { return m.flagged })
        if (flagged.length) for (var i = 0; i < flagged.length; i++) store.toggleFlag(flagged[i].id)
        else store.toggleFlag(current.id)
    }
    function quote(m) {
        var lines = (m.body || "").split("\n")
        return "\n\nOn " + Theme.fmtLongDate(m.date) + ", " + (m.fromName || m.fromEmail) + " wrote:\n> " + lines.join("\n> ")
    }
    // The original as it looked (formatted), inside a quote, as Apple Mail does.
    function quoteHtml(m, intro) {
        var inner = m.html ? m.html.replace(/<\/?(html|head|body)[^>]*>/gi, "").replace(/<blockquote[^>]*>/gi, '<blockquote style="margin-left:18px;margin-right:0px">')
                           : "<p>" + esc(m.body || "") + "</p>"
        return "<p>" + esc(intro) + "</p><blockquote style=\"margin-left:18px;margin-right:0px\">" + inner + "</blockquote>"
    }
    // here: written right under the message (its own Reply buttons) rather than in the sheet.
    function reply(all, which, here) {
        var m = which || current
        if (!m) return
        var subj = /^re:/i.test(m.subject) ? m.subject : "Re: " + m.subject
        var mine = JSON.parse(store.senders()).map(function(x) { var r = /<([^>]+)>/.exec(x.label); return r ? r[1].toLowerCase() : "" })
        var notMine = function(list) {
            return list.split(",").map(function(a) { return a.trim() }).filter(function(a) {
                if (!a) return false
                var r = /<([^>]+)>/.exec(a); var e = (r ? r[1] : a).toLowerCase()
                return mine.indexOf(e) < 0 && e !== (m.fromEmail || "").toLowerCase()
            })
        }
        var fromMe = mine.indexOf((m.fromEmail || "").toLowerCase()) >= 0
        // Replying to your own message goes to the people you wrote to.
        var to = fromMe ? notMine(m.to).join(", ") || m.to : (m.replyTo || m.fromEmail)
        var cc = all ? notMine([m.to, m.cc].filter(function(s) { return s && s.length }).join(", ")).filter(function(a) { return to.indexOf(a) < 0 }).join(", ") : ""
        // (Replying to an encrypted message: encrypted too, when every recipient has a key.)
        var wasEncrypted = (m.attachments || []).some(function(n) { return /^(encrypted\.asc|msg\.asc|openpgp-encrypted-message\.asc)$/i.test(n) })
        compose.openWith({ to: to, cc: cc, subject: subj, body: quote(m), account: store.messageAccount(m.id), inReplyTo: m.messageId, encrypt: wasEncrypted,
                           quoteHtml: quoteHtml(m, "On " + Theme.fmtLongDate(m.date) + ", " + (m.fromName || m.fromEmail) + " wrote:") },
                         here ? replyHost : null)
    }
    function forward(which, here) {
        var m = which || current
        if (!m) return
        compose.openWith({ account: store.messageAccount(m.id), to: "", cc: "", subject: (/^fwd?:/i.test(m.subject) ? "" : "Fwd: ") + m.subject,
                           body: "\n\n---------- Forwarded message ----------\nFrom: " + m.fromName + " <" + m.fromEmail + ">\nSubject: "
                                 + m.subject + "\nDate: " + Theme.fmtLongDate(m.date) + "\nTo: " + m.to + "\n\n" + m.body,
                           quoteHtml: "<p>Begin forwarded message:</p><p style=\"color:#5b5b60\"><b>From:</b> " + esc(m.fromName + " <" + m.fromEmail + ">")
                                      + "<br><b>Subject:</b> " + esc(m.subject) + "<br><b>Date:</b> " + esc(Theme.fmtLongDate(m.date)) + "<br><b>To:</b> " + esc(m.to) + "</p>"
                                      + (m.html ? m.html.replace(/<\/?(html|head|body)[^>]*>/gi, "") : "<p>" + esc(m.body || "") + "</p>") },
                         here ? replyHost : null)
        // Forwarded messages carry their attachments along.
        compose.forwardOf = m.id
        if (m.attachments && m.attachments.length) store.fetchAttachments(m.id)
    }
    Connections {
        target: store
        function onAttachmentsReadyChanged() {
            if (!store.attachmentsReady) return
            var r = JSON.parse(store.attachmentsReady)
            if (r.id === compose.forwardOf) compose.attachments = compose.attachments.concat(r.files)
        }
    }
    function composeNew() { compose.openWith({ to: "riley@example.org", subject: "Agenda for Thursday", body: "Hi Riley,\n\nHere are my notes for the kickoff.\n" }) }
    function openDraft() {
        if (!current || current.kind !== "drafts") return false
        var d = { to: current.to, cc: current.cc, subject: current.subject, body: current.body, draftId: current.id, account: store.messageAccount(current.id) }
        if (current.html) d.html = current.html
        compose.openWith(d)
        // Its attachments come back too.
        compose.forwardOf = current.id
        if (current.attachments && current.attachments.length) store.fetchAttachments(current.id)
        return true
    }

    function reopen(p) { compose.openWith(p) }
    // A message by its id (from a notification): in the Inbox list if it's
    // there, else in a tab of its own.
    function openMessageById(id) {
        goTo("inbox", "Inbox")
        for (var i = 0; i < list.length; i++) if ((list[i] || {}).id === id || ((list[i] || {}).ids && (list[i] || {}).ids.indexOf(id) >= 0)) { tabIndex = 0; select((list[i] || {}).id); return }
        openInTab(id)
    }
    // For checks: scrolls the open message's PDF box and reports its scroll bar.
    function checkPdfBar() {
        messagePage.runJavaScript('(function(){var b=document.querySelector(".pdfbox");if(!b)return "no box";var c=b.querySelector("img");for(var k=0;k<3&&c;k++)b.appendChild(c.cloneNode());b.scrollTop=200;var i=b.querySelector("img");return "scrolled "+b.scrollTop+" of "+(b.scrollHeight-b.clientHeight)+" box "+b.clientHeight+" imgs "+b.querySelectorAll("img").length+(i?" img "+i.naturalWidth+"x"+i.naturalHeight+" shown "+i.clientHeight:"")})()',
            function(r) { console.warn("ALMANAC-PDFBAR " + r) })
        pdfBarLater.start()
    }
    Timer {
        id: pdfBarLater
        interval: 150
        onTriggered: messagePage.runJavaScript('(function(){var bar=document.querySelector(".pdfbar"),th=document.querySelector(".pdfthumb");return "bar "+bar.className+" shown "+getComputedStyle(bar).opacity+" thumb top "+th.style.top+" height "+th.style.height})()',
            function(r) { console.warn("ALMANAC-PDFBAR " + r) })
    }
    function selectSubject(prefix) { for (var i = 0; i < list.length; i++) if ((list[i] || {}).subject.indexOf(prefix) === 0) { select((list[i] || {}).id); return } }
    function selectInvite() { for (var i = 0; i < list.length; i++) if ((list[i] || {}).subject.indexOf("Invitation") === 0) { select((list[i] || {}).id); return } }
    function remindIn(ms) { if (current) store.remindMe(current.id, Date.now() + ms) }
    function remindAt(hour, daysAhead) { if (!current) return; var d = Theme.addDays(new Date(), daysAhead); d.setHours(hour, 0, 0, 0); store.remindMe(current.id, d.getTime()) }
    function esc(t) { return (t || "").replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/\n/g, "<br>") }
    // For the menus (named as in Apple Mail)
    function goTo(kind, name) { selection = "kind:" + kind; selectionName = name; selectedId = -1 }
    function sendAgain() { if (current) compose.openWith({ to: current.to, cc: current.cc, subject: current.subject, body: current.body, html: current.html }) }
    // Saving asks where (the system's Save dialog, starting in Downloads).
    function saveAttachment(m, name) {
        saveOne.msgId = m.id
        saveOne.fileName = name
        saveOne.currentFolder = store.downloadsFolder()
        saveOne.selectedFile = store.downloadsFolder() + "/" + encodeURIComponent(name)
        saveOne.open()
    }
    function saveAttachments() {
        if (!current || !current.attachments.length) return
        if (current.attachments.length === 1) { saveAttachment(current, current.attachments[0]); return }
        saveAll.msg = current
        saveAll.currentFolder = store.downloadsFolder()
        saveAll.open()
    }
    FileDialog {
        id: saveOne
        property var msgId: -1
        property string fileName: ""
        title: "Save Attachment"
        fileMode: FileDialog.SaveFile
        onAccepted: store.saveAttachmentAs(msgId, fileName, selectedFile)
    }
    FolderDialog {
        id: saveAll
        property var msg: null
        title: "Save Attachments To"
        onAccepted: { for (var i = 0; i < msg.attachments.length; i++) store.saveAttachmentAs(msg.id, msg.attachments[i], selectedFolder) }
    }
    function exportCurrentPdf() { if (current) store.exportPdf(current.subject, currentHtml()) }
    // "Erase Deleted Items" / "Erase Junk Mail": empties Trash or Junk.
    function eraseMailbox(kind) {
        var rows = JSON.parse(store.messages("kind:" + kind, "", false))
        if (!rows.length) return
        confirm.ask(kind === "trash" ? "Erase deleted messages?" : "Erase junk mail?",
                    rows.length + (rows.length === 1 ? " message" : " messages") + " will be removed for good. This can't be undone.",
                    "Erase", function() { mail.eraseNow(kind) })
    }
    function eraseNow(kind) {
        var rows = JSON.parse(store.messages("kind:" + kind, "", false)), all = []
        for (var i = 0; i < rows.length; i++) all = all.concat(rows[i].ids || [rows[i].id])
        store.deleteMessages(JSON.stringify(all))
        if (selection === "kind:" + kind) selectedId = -1
    }
    function currentHtml() {
        var h = "<h2>" + esc(current.subject) + "</h2>"
        for (var i = 0; i < conversation.length; i++) {
            var m = conversation[i]
            h += "<p style='color:#555'><b>From:</b> " + esc(m.fromName + " <" + m.fromEmail + ">") + "<br><b>To:</b> " + esc(m.to)
               + (m.cc ? "<br><b>Cc:</b> " + esc(m.cc) : "") + "<br><b>Date:</b> " + esc(Theme.fmtLongDate(m.date)) + "</p>"
            h += (m.html ? m.html : "<p>" + esc(m.body) + "</p>") + "<hr>"
        }
        return h
    }
    function printCurrent() { printMessages(conversation) }
    // A page down (1) or up (-1) in the message being read.
    function scrollMessage(dir) { messagePage.runJavaScript("window.scrollBy({top: " + dir + " * window.innerHeight * 0.9, behavior: 'smooth'})") }
    function printMessages(list) {
        if (!list || !list.length) return
        var current = list[list.length - 1]
        var h = "<h2>" + esc(current.subject) + "</h2>"
        for (var i = 0; i < list.length; i++) {
            var m = list[i]
            h += "<p style='color:#555'><b>From:</b> " + esc(m.fromName + " <" + m.fromEmail + ">") + "<br><b>To:</b> " + esc(m.to)
               + (m.cc ? "<br><b>Cc:</b> " + esc(m.cc) : "") + "<br><b>Date:</b> " + esc(Theme.fmtLongDate(m.date)) + "</p>"
            h += (m.html ? m.html : "<p>" + esc(m.body) + "</p>") + "<hr>"
        }
        store.printHtml(current.subject, h)
    }

    ComposeSheet { id: compose; store: mail.store }

    // Double-click: the message (its conversation) in a window of its own.
    // Opening one that's already open brings its window forward.
    // Message windows (each with tabs of its own).
    property var tabWindows: []
    Component { id: messageWindowKind; MessageWindow {} }
    // Messages opened in tabs (double-click): {id, selection, threaded, title}.
    // Tab 0 is the mailbox itself.
    property var tabs: []
    // The tabs' pages (a list model, so closing one tab leaves the others' pages alone).
    ListModel { id: tabPages }
    function closeAllTabs() { tabIndex = 0; tabPages.clear(); tabs = [] }
    property int tabIndex: 0
    // A tab: a message (and its conversation) or a picture / PDF from one.
    function messageTab(id) {
        var m = JSON.parse(store.message(id))
        return { id: id, selection: selection, threaded: threaded, title: (m && m.subject) || "(No Subject)" }
    }
    function attachmentTab(id, name) { return { id: id, att: name, title: name, attachment: true } }
    function sameTab(a, b) { return a.id === b.id && (a.att || "") === (b.att || "") }
    function openInTab(id) {
        var t = messageTab(id)
        for (var i = 0; i < tabs.length; i++) if (sameTab(tabs[i], t)) { tabIndex = i + 1; return }
        insertTab(t, tabs.length)
    }
    // A picture or PDF from a message, on its own in a tab.
    function openAttachmentTab(id, name) {
        var t = attachmentTab(id, name)
        for (var i = 0; i < tabs.length; i++) if (sameTab(tabs[i], t)) { tabIndex = i + 1; return }
        insertTab(t, tabs.length)
    }
    // …or in a window of its own.
    function openAttachmentWindow(id, name) { openTabWindow(attachmentTab(id, name)) }
    function attachmentView(m, name, where) { if (where === "tab") openAttachmentTab(m.id, name); else openAttachmentWindow(m.id, name) }
    // Tabs moved about (by dragging, or from their menu). i, at: places in tabs.
    function insertTab(t, at) {
        var l = tabs.slice(); l.splice(at, 0, t)
        tabPages.insert(at, { messageId: t.id, sel: t.selection || "", thr: !!t.threaded, att: t.att || "" })
        tabs = l
        tabIndex = at + 1
    }
    function moveTab(from, to) {
        if (to > from) to--
        if (to === from) { tabIndex = from + 1; return }
        var l = tabs.slice(); var t = l.splice(from, 1)[0]; l.splice(to, 0, t)
        tabPages.move(from, to, 1)
        tabs = l
        tabIndex = to + 1
    }
    function takeTab(i) { var t = tabs[i]; closeTab(i); return t }
    function closeOtherTabs(i) { var keep = tabs[i]; for (var j = tabs.length - 1; j >= 0; j--) if (tabs[j] !== keep) closeTab(j); tabIndex = 1 }
    function canTearOut(i) { return true }
    function tearOut(i) { openTabWindow(takeTab(i)) }
    function raiseWindow() { var w = mail.Window.window; if (w) { w.raise(); w.requestActivate() } }
    // A message window, with these tabs.
    function openTabWindow(t) {
        var w = messageWindowKind.createObject(mail, { store: store, mail: mail })
        w.insertTab(t, 0)
        tabWindows = tabWindows.concat([w])
        w.closing.connect(function() { mail.tabWindows = mail.tabWindows.filter(function(x) { return x !== w }) })
        w.show(); w.raise(); w.requestActivate()
        return w
    }
    // Every message window's tabs into this one (Window › Merge All Windows).
    function mergeAllWindows() {
        var all = tabWindows.slice()
        for (var i = 0; i < all.length; i++) while (all[i].tabs.length) insertTab(all[i].takeTab(0), tabs.length)
        raiseWindow()
    }
    function closeTab(i) {
        var l = tabs.slice(); l.splice(i, 1)
        // Only that tab's page goes; the others stay as they are (no reloading).
        tabPages.remove(i)
        if (tabIndex > i + 1) tabIndex--
        else if (tabIndex === i + 1) tabIndex = Math.min(i, l.length)
        tabs = l
    }
    Shortcut { sequence: "Ctrl+W"; enabled: mail.visible && mail.tabIndex > 0; onActivated: mail.closeTab(mail.tabIndex - 1) }
    Shortcut { sequence: "Ctrl+Tab"; enabled: mail.visible && mail.tabs.length > 0; onActivated: mail.tabIndex = (mail.tabIndex + 1) % (mail.tabs.length + 1) }
    Shortcut { sequence: "Ctrl+Shift+Tab"; enabled: mail.visible && mail.tabs.length > 0; onActivated: mail.tabIndex = (mail.tabIndex + mail.tabs.length) % (mail.tabs.length + 1) }

    function openInWindow(id) {
        var t = messageTab(id)
        // Already open in a window: that tab, brought forward.
        for (var i = 0; i < tabWindows.length; i++) for (var j = 0; j < tabWindows[i].tabs.length; j++) {
            if (sameTab(tabWindows[i].tabs[j], t)) { tabWindows[i].tabIndex = j; tabWindows[i].raiseWindow(); return tabWindows[i] }
        }
        return openTabWindow(t)
    }
    // Shared by the reading pane and message windows.
    function senderClicked(m) { senderMenu.m = m; senderMenu.open() }
    function remoteRequested(m, scope) {
        if (scope === "all") store.setSetting("load_remote", "true")
        else if (scope === "sender") {
            var list = (prefs.remote_senders || []).slice()
            var who = (m.fromEmail || "").toLowerCase()
            if (who && list.indexOf(who) < 0) list.push(who)
            store.setSetting("remote_senders", JSON.stringify(list))
        } else loadRemote(m.id)
    }
    // here: from the buttons on the message in this window's reading pane (the
    // reply is written right under it).
    function messageAction(m, action, here) {
        if (action === "reply") reply(false, m, here)
        else if (action === "replyAll") reply(true, m, here)
        else if (action === "forward") forward(m, here)
        else if (action === "delete") {
            if (conversation.length <= 1 && current && current.id === m.id) removeCurrent("delete")
            else store.deleteMessages(JSON.stringify([m.id]))
        }
    }
    readonly property alias composer: compose
    // For checks: runs a script in the reading pane's page.
    function runInPage(js) { messagePage.runJavaScript(js) }
    // Another message chosen (see onSelectedIdChanged), several, or another part
    // of the app: a reply being written under the message is put away.
    onMultiChanged: if (multi) compose.leaveInline()
    onVisibleChanged: if (!visible) compose.leaveInline()
    Confirm { id: confirm }


    TextEdit { id: copyHelper; visible: false }
    RulesDialog { id: rulesDialog; store: mail.store }
    SignaturesDialog { id: signaturesDialog; store: mail.store }
    OutboxDialog { id: outboxDialog; store: mail.store; onEditRequested: function(p) { compose.openWith(p) } }
    SmartMailboxDialog {
        id: smartDialog
        store: mail.store
        onSaved: function(sel, name) { mail.selection = sel; mail.selectionName = name; mail.selectedId = -1 }
    }

    Shortcut { sequence: "Ctrl+N"; enabled: mail.visible; onActivated: compose.openWith({}) }
    Shortcut { sequence: "Ctrl+R"; enabled: mail.visible; onActivated: mail.reply(false) }
    Shortcut { sequence: "Ctrl+Shift+R"; enabled: mail.visible; onActivated: mail.reply(true) }
    Shortcut { sequence: "Ctrl+Shift+F"; enabled: mail.visible; onActivated: mail.forward() }
    Shortcut { sequence: "Ctrl+Shift+N"; enabled: mail.visible; onActivated: store.refreshAll() }
    Shortcut { sequence: "Ctrl+F"; enabled: mail.visible; onActivated: mail.focusSearch() }
    Shortcut { sequence: "Ctrl+A"; enabled: mail.visible && messageList.activeFocus; onActivated: mail.selectAll() }
    Shortcut { sequences: ["Ctrl++", "Ctrl+="]; enabled: mail.visible; onActivated: mail.textZoom = Math.min(3, mail.textZoom * 1.1) }
    Shortcut { sequence: "Ctrl+-"; enabled: mail.visible; onActivated: mail.textZoom = Math.max(0.6, mail.textZoom / 1.1) }
    Shortcut { sequence: "Ctrl+0"; enabled: mail.visible; onActivated: mail.textZoom = 1 }
    Shortcut { sequence: "Ctrl+Shift+U"; enabled: mail.visible && mail.currentRow !== null; onActivated: mail.setCurrentRead(!mail.currentRow.read) }
    Shortcut { sequence: "Ctrl+Shift+L"; enabled: mail.visible && mail.current !== null; onActivated: mail.toggleCurrentFlag() }
    // (Held down, these act once: never a run down the list.)
    Shortcut { sequence: "Ctrl+Shift+A"; autoRepeat: false; enabled: mail.visible && !confirm.opened; onActivated: mail.removeCurrent("archive") }
    Shortcut { sequence: "Ctrl+Shift+J"; autoRepeat: false; enabled: mail.visible && !confirm.opened; onActivated: mail.removeCurrent("junk") }

    // ------------------------------------------------- buttons for the top bar
    property Component tools: Component {
        RowLayout {
            spacing: 4
            Column {
                readonly property real minW: 90
                Layout.preferredWidth: 160
                Layout.maximumWidth: 160
                Layout.fillWidth: true
                Layout.minimumWidth: 90
                clip: true
                Text { text: mail.selectionName; font.pixelSize: 14; font.weight: Font.Bold; color: Theme.text; elide: Text.ElideRight; width: parent.width }
                Text {
                    text: mail.page.all.toLocaleString(Qt.locale(), "f", 0) + (mail.threaded && !mail.page.countsMessages ? (mail.page.all === 1 ? " conversation" : " conversations") : (mail.page.all === 1 ? " message" : " messages"))
                          + (mail.unreadCount > 0 ? ", " + mail.unreadCount + " unread" : "")
                    font.pixelSize: 11; color: Theme.secondary
                }
            }
            ToolButtonX { priority: 3; icon: "compose"; tip: "New Message (Ctrl+N)"; onClicked: compose.openWith({}) }
            Item { Layout.fillWidth: true }
            ToolButtonX { priority: 1; icon: "archive"; tip: "Archive (Ctrl+Shift+A)"; enabled: mail.current !== null; onClicked: mail.removeCurrent("archive") }
            ToolButtonX { priority: 2; icon: "trash"; tip: "Delete"; enabled: mail.current !== null; onClicked: mail.removeCurrent("delete") }
            ToolButtonX { icon: "junk"; tip: "Move to Junk (Ctrl+Shift+J)"; enabled: mail.current !== null; onClicked: mail.removeCurrent("junk") }
            Item { width: 6 }
            ToolButtonX { priority: 2; icon: "reply"; tip: "Reply (Ctrl+R)"; enabled: mail.current !== null; onClicked: mail.reply(false) }
            ToolButtonX { priority: 1; icon: "replyAll"; tip: "Reply All (Ctrl+Shift+R)"; enabled: mail.current !== null; onClicked: mail.reply(true) }
            ToolButtonX { priority: 1; icon: "forward"; tip: "Forward (Ctrl+Shift+F)"; enabled: mail.current !== null; onClicked: mail.forward() }
            Item { width: 6 }
            ToolButtonX {
                icon: "flag"
                iconColor: mail.current && mail.current.flagged ? Theme.flagColors[mail.current.flagColor || 0] : Theme.secondary
                tip: "Flag (Ctrl+Shift+L)"; enabled: mail.current !== null
                onClicked: mail.toggleCurrentFlag()
            }
            // The flag's colour (as Apple Mail's flag button menu).
            ToolButtonX {
                id: flagColorButton
                implicitWidth: 18
                icon: "chevronDown"; tip: "Flag Colour"; enabled: mail.current !== null || mail.multi
                onClicked: flagColorMenu.open(flagColorButton)
                P.Menu {
                    id: flagColorMenu
                    Instantiator {
                        model: 7
                        onObjectAdded: (i, o) => flagColorMenu.insertItem(i, o)
                        onObjectRemoved: (i, o) => flagColorMenu.removeItem(o)
                        delegate: P.MenuItem {
                            required property int index
                            text: Theme.flagNames[index]
                            icon.source: mail.flagIcon(index)
                            onTriggered: mail.setCurrentFlag(index)
                        }
                    }
                    P.MenuSeparator {}
                    P.MenuItem { text: "Clear Flag"; onTriggered: mail.setCurrentFlag(-1) }
                }
            }
            ToolButtonX {
                icon: "envelopeOpen"
                tip: mail.currentRow && !mail.currentRow.read ? "Mark as Read" : "Mark as Unread (Ctrl+Shift+U)"
                enabled: mail.currentRow !== null
                onClicked: mail.setCurrentRead(!mail.currentRow.read)
            }
            Item { width: 6 }
            SearchBox {
                id: search
                Layout.preferredWidth: collapsed ? 30 : 160
                Layout.maximumWidth: collapsed ? 30 : 160
                Layout.fillWidth: true
                Layout.minimumWidth: collapsed ? 30 : 84
                readonly property real minW: 84
                text: mail.query
                onTextChanged: mail.query = text
                Connections { target: mail; function onFocusSearch() { search.focusField() } }
            }
        }
    }

    // Grips over the pane dividers (the lines are too thin to catch).
    PaneGrip { pane: sidebarPane }
    PaneGrip { pane: listPane; vertical: mail.classic }

    // Tabs: the mailbox, then each message opened in a tab (shown once there's
    // one); they can be dragged along, out into windows, and between windows.
    Rectangle {
        id: tabStrip
        anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
        // (Above the drop area below, so a tab can be dropped at a place in the row.)
        z: 60
        height: mail.tabs.length ? 36 : 0
        visible: mail.tabs.length > 0
        color: Theme.toolbar
        TabStrip { anchors.fill: parent; owner: mail; homeTitle: mail.selectionName; mail: mail }
    }
    // A tab from a message window let go anywhere over this one joins its tabs.
    DropArea {
        anchors.fill: parent
        z: 50
        keys: ["application/x-almanac-tab"]
        onEntered: function(drag) { if (!Theme.tabDrag || Theme.tabDrag.owner === mail) drag.accepted = false }
        onDropped: function(drop) {
            if (!Theme.tabDrag || Theme.tabDrag.owner === mail) return
            Theme.tabDrag.target = mail
            Theme.tabDrag.at = mail.tabs.length
            drop.accept(Qt.MoveAction)
        }
    }
    Repeater {
        model: tabPages
        // A message (and its conversation), or a picture / PDF from one.
        delegate: Item {
            id: page
            required property var messageId
            required property string sel
            required property bool thr
            required property string att
            required property int index
            anchors.left: parent.left; anchors.right: parent.right
            anchors.top: tabStrip.bottom; anchors.bottom: parent.bottom
            visible: mail.tabIndex === index + 1
            Loader {
                anchors.fill: parent
                sourceComponent: page.att !== "" ? attachmentPage : messagePage
            }
            Component {
                id: messagePage
                MessageTab {
                    store: mail.store
                    view: mail
                    selection: page.sel
                    messageId: page.messageId
                    threaded: page.thr
                    onCloseRequested: mail.closeTab(page.index)
                    onPopOutRequested: mail.tearOut(page.index)
                }
            }
            Component {
                id: attachmentPage
                AttachmentView {
                    store: mail.store
                    view: mail
                    messageId: page.messageId
                    name: page.att
                    onCloseRequested: mail.closeTab(page.index)
                    onPopOutRequested: mail.tearOut(page.index)
                }
            }
        }
    }

    // Panes with handles to drag their widths.
    SplitView {
        anchors.left: parent.left; anchors.right: parent.right
        anchors.top: tabStrip.bottom; anchors.bottom: parent.bottom
        visible: mail.tabIndex === 0
        orientation: Qt.Horizontal
        handle: Rectangle {
            implicitWidth: 1
            color: SplitHandle.pressed || SplitHandle.hovered ? Theme.accent : Theme.border
        }

        // ---------------------------------------------------------- sidebar
        Rectangle {
            id: sidebarPane
            visible: mail.sidebarShown
            SplitView.preferredWidth: 220
            SplitView.minimumWidth: 150
            gradient: Theme.sidebarGradient
            // What's going on (checking, downloading, sending…), as in Apple Mail.
            ActivityPanel {
                id: activity
                store: mail.store
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
            }
            Flickable {
                id: scroller1
                // Trackpad scrolling glides on (momentum).
                Momentum { flick: scroller1 }
                anchors.fill: parent
                anchors.margins: 10
                anchors.topMargin: 14
                anchors.bottomMargin: activity.height + 10
                contentHeight: sideCol.implicitHeight
                clip: true
                Column {
                    id: sideCol
                    width: parent.width
                    spacing: 1
                    Text { text: "Favorites"; color: Theme.tertiary; font.pixelSize: 11; font.weight: Font.DemiBold; leftPadding: 8; bottomPadding: 2 }
                    Repeater {
                        model: mail.sidebar.smart
                        delegate: Column {
                            id: fav
                            required property var modelData
                            width: sideCol.width
                            // Inbox is every account's inbox together (a unified inbox);
                            // with more than one account, each one's own is listed under it.
                            readonly property var parts: modelData.id === "kind:inbox" ? mail.accountInboxes : []
                            SidebarItem {
                                icon: fav.modelData.icon
                                iconColor: fav.modelData.icon === "flag" ? Theme.orange : Theme.accent
                                label: fav.parts.length > 1 ? "All Inboxes" : fav.modelData.name
                                badge: fav.modelData.badge
                                selected: mail.selection === fav.modelData.id
                                onClicked: { mail.selection = fav.modelData.id; mail.selectionName = fav.modelData.name; mail.selectedId = -1 }
                                onRightClicked: { boxMenu.target = fav.modelData.id; boxMenu.open() }
                                Icon {
                                    visible: fav.parts.length > 1
                                    anchors.right: parent.left
                                    anchors.rightMargin: -8
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: 10; height: 10
                                    name: mail.inboxesOpen ? "chevronDown" : "chevronRight"
                                    color: Theme.tertiary
                                    MouseArea { anchors.fill: parent; anchors.margins: -4; onClicked: mail.inboxesOpen = !mail.inboxesOpen }
                                }
                            }
                            Repeater {
                                model: fav.modelData.colors || []
                                delegate: SidebarItem {
                                    required property var modelData
                                    indent: 18
                                    icon: "flag"
                                    iconColor: Theme.flagColors[modelData.color]
                                    label: modelData.name
                                    badge: modelData.badge
                                    selected: mail.selection === modelData.id
                                    onClicked: { mail.selection = modelData.id; mail.selectionName = modelData.name + " Flag"; mail.selectedId = -1 }
                                }
                            }
                            Repeater {
                                model: fav.parts.length > 1 && mail.inboxesOpen ? fav.parts : []
                                delegate: SidebarItem {
                                    required property var modelData
                                    indent: 18
                                    icon: "inbox"
                                    label: modelData.account
                                    badge: modelData.badge
                                    selected: mail.selection === modelData.id
                                    onClicked: { mail.selection = modelData.id; mail.selectionName = "Inbox — " + modelData.account; mail.selectedId = -1 }
                                    onRightClicked: { boxMenu.target = modelData.id; boxMenu.open() }
                                }
                            }
                        }
                    }
                    // Smart mailboxes (saved searches)
                    RowLayout {
                        width: sideCol.width
                        Text { Layout.fillWidth: true; text: "Smart Mailboxes"; color: Theme.tertiary; font.pixelSize: 11; font.weight: Font.DemiBold; leftPadding: 8; topPadding: 10; bottomPadding: 2 }
                        Text {
                            text: "+"; color: Theme.secondary; font.pixelSize: 16; rightPadding: 8; topPadding: 4
                            MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: smartDialog.openFor("") }
                        }
                    }
                    Repeater {
                        model: mail.sidebar.smartBoxes || []
                        delegate: SidebarItem {
                            required property var modelData
                            icon: "gear"
                            label: modelData.name
                            badge: modelData.badge
                            selected: mail.selection === modelData.id
                            onClicked: { mail.selection = modelData.id; mail.selectionName = modelData.name; mail.selectedId = -1 }
                            onRightClicked: { smartMenu.target = modelData.id; smartMenu.open() }
                            onDoubleClicked: smartDialog.openFor(modelData.id)
                        }
                    }
                    SidebarItem {
                        visible: (mail.sidebar.outbox || 0) > 0
                        icon: "clock"
                        label: "Outbox"
                        badge: mail.sidebar.outbox || 0
                        onClicked: outboxDialog.open()
                    }
                    Repeater {
                        model: mail.sidebar.accounts
                        delegate: Column {
                            id: acctSection
                            required property var modelData
                            width: sideCol.width
                            spacing: 1
                            readonly property bool folded: !!mail.foldedAccounts[modelData.id]
                            // The account's name: click to fold its mailboxes away or show them.
                            Item {
                                width: parent.width
                                height: acctName.implicitHeight
                                Text {
                                    id: acctName
                                    text: acctSection.modelData.name
                                    color: Theme.tertiary
                                    font.pixelSize: 11
                                    font.weight: Font.DemiBold
                                    leftPadding: 8; topPadding: 10; bottomPadding: 2
                                    rightPadding: 24
                                    elide: Text.ElideRight
                                    width: parent.width
                                    // Cut short: the whole name when pointed at.
                                    HoverHandler { id: acctNameHover }
                                    ToolTip.visible: acctNameHover.hovered && truncated
                                    ToolTip.delay: 500
                                    ToolTip.text: text
                                }
                                Icon {
                                    anchors.right: parent.right
                                    anchors.rightMargin: 8
                                    y: 11
                                    width: 11; height: 11
                                    name: acctSection.folded ? "chevronRight" : "chevronDown"
                                    color: Theme.tertiary
                                    opacity: acctSection.folded || acctHover.hovered ? 1 : 0
                                }
                                HoverHandler { id: acctHover }
                                MouseArea {
                                    anchors.fill: parent
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: mail.foldAccount(acctSection.modelData.id, !acctSection.folded)
                                }
                                ToolTip.visible: acctHover.hovered
                                ToolTip.delay: 600
                                ToolTip.text: acctSection.folded ? "Show " + acctSection.modelData.name : "Hide " + acctSection.modelData.name
                            }
                            Repeater {
                                model: acctSection.folded ? [] : acctSection.modelData.folders
                                delegate: SidebarItem {
                                    required property var modelData
                                    icon: modelData.icon
                                    label: modelData.name
                                    badge: modelData.badge
                                    selected: mail.selection === modelData.id
                                    onClicked: { mail.selection = modelData.id; mail.selectionName = modelData.name; mail.selectedId = -1 }
                                    onRightClicked: { boxMenu.target = modelData.id; boxMenu.open() }
                                }
                            }
                        }
                    }
                }
            }
            P.Menu {
                id: smartMenu
                property string target: ""
                P.MenuItem { icon.name: "document-edit"; text: "Edit Smart Mailbox…"; onTriggered: smartDialog.openFor(smartMenu.target) }
                P.MenuItem { icon.name: "mail-mark-read"; text: "Mark All as Read"; onTriggered: store.markAllRead(smartMenu.target) }
                P.MenuItem { icon.name: "edit-delete"; text: "Delete Smart Mailbox"; onTriggered: { store.deleteSmartMailbox(smartMenu.target); if (mail.selection === smartMenu.target) { mail.selection = "kind:inbox"; mail.selectionName = "Inbox" } } }
            }
            P.Menu {
                id: boxMenu
                property string target: ""
                P.MenuItem { icon.name: "mail-mark-read"; text: "Mark All as Read"; onTriggered: store.markAllRead(boxMenu.target) }
                P.MenuItem { icon.name: "mail-receive"; shortcut: "Ctrl+Shift+N"; text: "Get Mail"; onTriggered: store.refreshAll() }
            }
        }

        // ------------------------------------------------------ message list
        SplitView {
            id: contentSplit
            SplitView.fillWidth: true
            // Classic layout (Apple Mail's "Use Column Layout" off): the list
            // above the message. Standard: side by side.
            orientation: mail.classic ? Qt.Vertical : Qt.Horizontal
            handle: Rectangle {
                implicitWidth: 1
                implicitHeight: 1
                color: SplitHandle.pressed || SplitHandle.hovered ? Theme.accent : Theme.border
            }
        ColumnLayout {
            id: listPane
            SplitView.preferredWidth: 340
            SplitView.minimumWidth: 220
            SplitView.preferredHeight: 280
            SplitView.minimumHeight: 120
            spacing: 0
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 30
                color: Theme.window
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 14
                    anchors.rightMargin: 8
                    Text {
                        Layout.fillWidth: true
                        text: mail.unreadOnly ? "Filtered by: Unread" : (mail.query ? "Search results" : "Sorted by " + mail.sortNames[mail.sortBy] + " ▾")
                        color: mail.unreadOnly ? Theme.accent : Theme.secondary
                        font.pixelSize: 11
                        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: sortMenu.open() }
                        P.Menu {
                            id: sortMenu
                            Instantiator {
                                model: ["date", "from", "subject", "to", "attachments", "flags", "unread"]
                                onObjectAdded: (i, o) => sortMenu.insertItem(i, o)
                                onObjectRemoved: (i, o) => sortMenu.removeItem(o)
                                delegate: P.MenuItem {
                                    required property string modelData
                                    icon.name: "view-sort"
                                    text: mail.sortNames[modelData]
                                    checkable: true
                                    checked: mail.sortBy === modelData
                                    onTriggered: mail.setSort(modelData, mail.sortAscending)
                                }
                            }
                            P.MenuSeparator {}
                            P.MenuItem {
                                icon.name: "view-sort-ascending"; shortcut: "Ctrl+Alt+Up"
                                text: mail.sortBy === "date" ? "Oldest on Top" : (["attachments", "flags", "unread"].indexOf(mail.sortBy) >= 0 ? "Without on Top" : "Ascending (A to Z)")
                                checkable: true; checked: mail.sortAscending
                                onTriggered: mail.setSort(mail.sortBy, true)
                            }
                            P.MenuItem {
                                icon.name: "view-sort-descending"; shortcut: "Ctrl+Alt+Down"
                                text: mail.sortBy === "date" ? "Newest on Top" : (["attachments", "flags", "unread"].indexOf(mail.sortBy) >= 0 ? "With on Top" : "Descending (Z to A)")
                                checkable: true; checked: !mail.sortAscending
                                onTriggered: mail.setSort(mail.sortBy, false)
                            }
                        }
                    }
                    ToolButtonX {
                        implicitHeight: 22; implicitWidth: 24
                        icon: "filter"; checked: mail.unreadOnly
                        tip: mail.unreadOnly ? "Show All Messages" : "Show Only Unread"
                        onClicked: mail.unreadOnly = !mail.unreadOnly
                    }
                    ToolButtonX {
                        implicitHeight: 22; implicitWidth: 24
                        icon: "refresh"
                        tip: "Get Mail (Ctrl+Shift+N)"
                        onClicked: store.refreshAll()
                    }
                    Text {
                        text: mail.threaded ? "Conversations: On" : "Conversations: Off"
                        color: Theme.accent
                        font.pixelSize: 11
                        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: { mail.threaded = !mail.threaded; mail.selectedId = -1 } }
                    }
                }
                Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.borderSoft }
            }
            ListView {
                // Trackpad scrolling glides on (momentum).
                Momentum { flick: messageList }
                id: messageList
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                focus: true
                model: mail.windowed ? winModel : rowsModel
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar {}
                keyNavigationEnabled: true
                // The list goes to the selected message at once, never gliding
                // to it (ListView's own following moves at 400 px/s).
                highlightMoveDuration: 0
                highlightMoveVelocity: -1
                highlightResizeDuration: 0
                // At the end of the list: older mail, or this search on the server.
                footer: Item {
                    width: ListView.view ? ListView.view.width : 0
                    // With all mail kept on this computer there's nothing older to fetch;
                    // searching the server still helps before the first full download.
                    readonly property bool wanted: mail.serverBox && (!mail.prefs.download_all_mail || mail.query !== "")
                    // Room for the rows not loaded yet, so the list (and its scroll
                    // bar) is as long as the whole mailbox from the start.
                    readonly property real unloaded: messageList.unloadedHeight
                    height: unloaded + (wanted ? 58 : 0)
                    ToolButtonX {
                        visible: parent.wanted
                        anchors.bottom: parent.bottom; anchors.bottomMargin: 14
                        anchors.horizontalCenter: parent.horizontalCenter
                        icon: mail.query === "" ? "clock" : "search"
                        text: mail.query === "" ? "Load Older Messages" : "Search for “" + mail.query + "” on the Server"
                        tip: mail.query === "" ? "Get the next 200 older messages of this mailbox from the server"
                                               : "Look through all of this mailbox on the server, not only the mail downloaded so far"
                        onClicked: mail.query === "" ? store.loadOlder(mail.selection) : store.searchOnServer(mail.selection, mail.query)
                    }
                }
                // Rows load as the list nears its end.
                readonly property real rowHeight: count > 0 ? Math.max(30, (contentHeight - (footerItem ? footerItem.height : 0)) / count) : 86
                // Rows not loaded yet are estimated at a typical row's height.
                readonly property real rowEstimate: mail.classic ? 30 : 86
                readonly property real unloadedHeight: mail.windowed ? 0 : Math.max(0, mail.page.total - mail.list.length) * rowEstimate
                // Near the end of the loaded rows (or scrolled or dragged into the
                // room left for the rest): load up to a page past what's on screen.
                onContentYChanged: { if (mail.windowed) winScroll.restart(); else { loadMoreIfNear(); trimLater.restart() } }
                // (Windowed: the pages for what's on screen, once the list settles a moment.)
                Timer { id: winScroll; interval: 60; onTriggered: mail.winEnsure() }
                // Back near the top for a moment after going far down: the rows
                // loaded on the way go again, so a change to the mail (several a
                // minute) doesn't re-read thousands of them, freezing the window.
                // (Their room stays, so nothing on screen moves.)
                Timer {
                    id: trimLater
                    interval: 2000
                    onTriggered: if (messageList.contentY - messageList.originY < messageList.height * 2 && mail.limit > mail.pageSize * 2) mail.resetPage()
                }
                // (Also as rows come in: the ones loaded may still end above what's on screen.)
                onContentHeightChanged: if (!mail.windowed) Qt.callLater(loadMoreIfNear)
                function loadMoreIfNear() {
                    var loadedEnd = contentHeight - (footerItem ? footerItem.height : 0)
                    var bottom = contentY - originY + height
                    if (bottom <= loadedEnd - height * 2) return
                    var past = Math.max(0, Math.ceil((bottom - loadedEnd) / rowEstimate))
                    Qt.callLater(mail.loadRows, mail.list.length + past + mail.pageSize)
                }
                onCurrentIndexChanged: {
                    if (currentIndex >= 0) mail.lastIndex = currentIndex
                    if (currentIndex >= 0 && currentIndex < mail.list.length && mail.list[currentIndex] && (mail.list[currentIndex] || {}).id !== mail.selectedId) mail.select((mail.list[currentIndex] || {}).id)
                }
                // Delete / Backspace: once per press. Held down, the key repeats,
                // and each repeat used to take the next row too: a whole run of
                // mail went to the Trash.
                // (Delete itself: the window's Delete shortcut, see Main.qml.)
                Keys.onPressed: function(ev) {
                    if (ev.key === Qt.Key_Backspace) { ev.accepted = true; if (!ev.isAutoRepeat) mail.removeCurrent("delete") }
                    // Page Up / Down: a screenful up or down the list; Home / End: the
                    // first / last message. (In the message itself they scroll it.)
                    else if ((ev.key === Qt.Key_PageDown || ev.key === Qt.Key_PageUp) && mail.list.length) {
                        var rows = Math.max(1, Math.floor(messageList.height / messageList.rowHeight) - 1)
                        var at = Math.max(0, mail.indexOfSelected())
                        var to = ev.key === Qt.Key_PageDown ? Math.min(mail.page.total - 1, at + rows) : Math.max(0, at - rows)
                        mail.loadRows(to + 1)
                        to = Math.min(to, mail.list.length - 1)
                        mail.selectedIds = []
                        if (mail.windowed && !mail.list[to]) {
                            // (Not read yet: chosen when its page comes.)
                            mail.winWant = to
                            messageList.positionViewAtIndex(to, ListView.Contain)
                            mail.winFetch(Math.floor(to / mail.winSize))
                            ev.accepted = true
                            return
                        }
                        mail.select((mail.list[to] || {}).id)
                        messageList.currentIndex = to
                        messageList.positionViewAtIndex(to, ListView.Contain)
                        ev.accepted = true
                    } else if ((ev.key === Qt.Key_Home || ev.key === Qt.Key_End) && mail.list.length) {
                        // (The total is an estimate with conversations on, until all are read.)
                        if (ev.key === Qt.Key_End && !mail.windowed)
                            for (var more = 0; more < 10 && mail.list.length < mail.page.total; more++) mail.loadRows(mail.page.total)
                        var i = ev.key === Qt.Key_Home ? 0 : mail.list.length - 1
                        mail.selectedIds = []
                        if (mail.windowed && !mail.list[i]) {
                            mail.winWant = i
                            messageList.positionViewAtIndex(i, ev.key === Qt.Key_Home ? ListView.Beginning : ListView.End)
                            mail.winFetch(Math.floor(i / mail.winSize))
                            ev.accepted = true
                            return
                        }
                        mail.select((mail.list[i] || {}).id)
                        messageList.currentIndex = i
                        messageList.positionViewAtIndex(i, ev.key === Qt.Key_Home ? ListView.Beginning : ListView.End)
                        ev.accepted = true
                    }
                }
                Keys.onReturnPressed: mail.openDraft()

                delegate: Rectangle {
                    id: row
                    required property var model
                    required property int index
                    readonly property var modelData: mail.windowed ? (mail.winRows[index] || mail.placeholderRow(index)) : model.msg
                    property bool sel: mail.isPicked(modelData.id)
                    property bool strong: sel && messageList.activeFocus
                    width: ListView.view.width
                    // As tall as what's in it (a subject can take two lines).
                    height: mail.classic ? 30 : (modelData.placeholder ? 86 : Math.max(86, rowText.implicitHeight + 18))
                    color: "transparent"
                    // Not read yet (the list reads a page at a time): a light outline of a row.
                    Rectangle {
                        z: 50
                        anchors.fill: parent
                        visible: !!row.modelData.placeholder
                        color: Theme.window
                        Column {
                            anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
                            anchors.leftMargin: 20; anchors.rightMargin: 20; anchors.topMargin: mail.classic ? 10 : 16
                            spacing: 9
                            Rectangle { width: parent.width * 0.4; height: 10; radius: 5; color: Theme.segment }
                            Rectangle { visible: !mail.classic; width: parent.width * 0.75; height: 9; radius: 4.5; color: Theme.segment }
                            Rectangle { visible: !mail.classic; width: parent.width * 0.6; height: 9; radius: 4.5; color: Theme.segment }
                        }
                        MouseArea { anchors.fill: parent; acceptedButtons: Qt.AllButtons }
                    }
                    property color face: sel ? (messageList.activeFocus ? Theme.accentFill : Theme.accentSoft) : Theme.window
                    property color main: strong ? Theme.accentInk : Theme.text
                    property color sub: strong ? "#e8f1ff" : Theme.secondary

                    // Two-finger swipe sideways, as in Apple Mail: left to
                    // move to Trash, right to mark as read or unread.
                    property real swipe: 0
                    property bool swiping: false
                    readonly property real swipeLimit: Math.min(130, width * 0.35)
                    Behavior on swipe { enabled: !row.swiping; NumberAnimation { duration: 180; easing.type: Easing.OutCubic } }
                    function swipeBy(dx) {
                        swiping = true
                        swipe = Math.max(-width * 0.6, Math.min(width * 0.6, swipe + dx))
                        swipeEnd.restart()
                    }
                    function finishSwipe() {
                        swipeEnd.stop()
                        swiping = false
                        var id = modelData.id
                        if (swipe <= -swipeLimit) { swipe = 0; mail.select(id); mail.removeCurrent("delete") }
                        else if (swipe >= swipeLimit) { swipe = 0; store.setRead(id, !modelData.read) }
                        else swipe = 0
                    }
                    Timer { id: swipeEnd; interval: 180; onTriggered: row.finishSwipe() }
                    // What the swipe reveals, behind the row
                    Rectangle {
                        z: -2
                        anchors.fill: parent
                        visible: row.swipe !== 0
                        color: row.swipe > 0 ? Theme.accentFill : Theme.red
                        opacity: Math.min(1, Math.abs(row.swipe) / row.swipeLimit * 0.6 + 0.4)
                        Text {
                            visible: row.swipe > 0
                            x: 18; anchors.verticalCenter: parent.verticalCenter
                            text: row.modelData.read ? "Unread" : "Read"
                            color: "white"; font.pixelSize: Theme.base; font.weight: Font.DemiBold
                        }
                        Text {
                            visible: row.swipe < 0
                            anchors.right: parent.right; anchors.rightMargin: 18; anchors.verticalCenter: parent.verticalCenter
                            text: "Trash"
                            color: "white"; font.pixelSize: Theme.base; font.weight: Font.DemiBold
                        }
                    }
                    Rectangle {
                        z: -1
                        width: parent.width; height: parent.height
                        color: Theme.window
                        transform: Translate { x: row.swipe }
                        // Chosen: a rounded highlight set in a little, as in Apple Mail.
                        Rectangle {
                            visible: row.sel
                            anchors.fill: parent
                            anchors.leftMargin: 6; anchors.rightMargin: 6
                            anchors.topMargin: mail.classic ? 1 : 3; anchors.bottomMargin: mail.classic ? 1 : 3
                            radius: mail.classic ? 5 : 8
                            color: row.face
                        }
                    }

                    Rectangle {
                        visible: !modelData.read
                        width: 9; height: 9; radius: 4.5
                        color: row.strong ? Theme.accentInk : Theme.accent
                        x: 9; y: mail.classic ? 10.5 : 15
                        transform: Translate { x: row.swipe }
                    }
                    RowLayout {
                        visible: mail.classic
                        transform: Translate { x: row.swipe }
                        anchors.left: parent.left; anchors.leftMargin: 26
                        anchors.right: parent.right; anchors.rightMargin: 12
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 12
                        Text {
                            id: colFrom
                            Layout.preferredWidth: Math.min(200, parent.width * 0.28); text: modelData.fromName; elide: Text.ElideRight; color: row.main; font.pixelSize: Theme.base; font.weight: modelData.read ? Font.Normal : Font.Bold
                            HoverHandler { id: colFromHover }
                            ToolTip.visible: colFromHover.hovered && colFrom.truncated
                            ToolTip.delay: 400
                            ToolTip.text: modelData.fromName + (modelData.fromEmail ? " <" + modelData.fromEmail + ">" : "")
                        }
                        Text {
                            id: colSubject
                            Layout.fillWidth: true; text: modelData.subject + (modelData.count > 1 ? "  (" + modelData.count + ")" : ""); elide: Text.ElideRight; color: row.main; font.pixelSize: Theme.base
                            HoverHandler { id: colSubjectHover }
                            ToolTip.visible: colSubjectHover.hovered && colSubject.truncated
                            ToolTip.delay: 400
                            ToolTip.text: modelData.subject
                        }
                        Icon { visible: !!modelData.attachments; name: "paperclip"; width: 13; height: 13; color: row.sub }
                        Icon { visible: modelData.flagged; name: "flag"; width: 13; height: 13; color: row.strong ? Theme.accentInk : Theme.flagColors[modelData.flagColor || 0] }
                        Text { text: Theme.fmtListDate(modelData.date); color: row.sub; font.pixelSize: 12 }
                    }
                    Column {
                        id: rowText
                        visible: !mail.classic
                        transform: Translate { x: row.swipe }
                        anchors.left: parent.left
                        anchors.leftMargin: 26
                        anchors.right: parent.right
                        anchors.rightMargin: 12
                        anchors.top: parent.top
                        anchors.topMargin: 9
                        spacing: 2
                        RowLayout {
                            width: parent.width
                            spacing: 5
                            Text {
                                id: rowFrom
                                Layout.fillWidth: true
                                text: modelData.fromName
                                font.pixelSize: Theme.base + 1
                                font.weight: Font.Bold
                                color: row.main
                                elide: Text.ElideRight
                                // Cut short: the whole name (and address) on hover.
                                HoverHandler { id: rowFromHover }
                                ToolTip.visible: rowFromHover.hovered && rowFrom.truncated
                                ToolTip.delay: 400
                                ToolTip.text: modelData.fromName + (modelData.fromEmail ? " <" + modelData.fromEmail + ">" : "")
                            }
                            Rectangle {
                                visible: (modelData.count || 1) > 1
                                Layout.preferredWidth: countText.implicitWidth + 10
                                Layout.preferredHeight: 16
                                radius: 8
                                color: row.strong ? Qt.rgba(1, 1, 1, 0.25) : Theme.segment
                                Text { id: countText; anchors.centerIn: parent; text: modelData.count || ""; font.pixelSize: 10; color: row.main }
                            }
                            Icon { visible: !!modelData.attachments; name: "paperclip"; width: 13; height: 13; color: row.sub }
                            Icon { visible: !!modelData.vip; name: "star"; width: 13; height: 13; color: row.strong ? Theme.accentInk : Theme.accent }
                            Icon { visible: !!modelData.muted; name: "bellOff"; width: 13; height: 13; color: row.sub }
                            Icon { visible: !!modelData.remind; name: "clock"; width: 13; height: 13; color: row.strong ? Theme.accentInk : Theme.red }
                            Icon { visible: modelData.flagged; name: "flag"; width: 13; height: 13; color: row.strong ? Theme.accentInk : Theme.flagColors[modelData.flagColor || 0] }
                            Text { text: Theme.fmtListDate(modelData.date); color: row.sub; font.pixelSize: 12 }
                        }
                        // The subject on up to two lines; longer: all of it on hover.
                        Text {
                            id: rowSubject
                            width: parent.width
                            text: modelData.subject
                            color: row.main
                            font.pixelSize: Theme.base
                            wrapMode: Text.Wrap
                            maximumLineCount: 2
                            elide: Text.ElideRight
                            HoverHandler { id: rowSubjectHover }
                            ToolTip.visible: rowSubjectHover.hovered && rowSubject.truncated
                            ToolTip.delay: 400
                            ToolTip.text: modelData.subject
                        }
                        Text {
                            width: parent.width
                            text: (modelData.account ? modelData.account + " — " : "") + modelData.preview
                            color: row.sub
                            font.pixelSize: 12
                            wrapMode: Text.WordWrap
                            maximumLineCount: 2
                            elide: Text.ElideRight
                        }
                    }
                    Rectangle {
                        visible: !row.sel
                        anchors.bottom: parent.bottom
                        anchors.left: parent.left
                        anchors.leftMargin: 26
                        anchors.right: parent.right
                        height: 1
                        color: Theme.borderSoft
                    }
                    MouseArea {
                        anchors.fill: parent
                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                        // Sideways scrolling swipes the row; up and down still scrolls the list.
                        onWheel: function(ev) {
                            var dx = ev.pixelDelta.x !== 0 ? ev.pixelDelta.x : ev.angleDelta.x / 2
                            var dy = ev.pixelDelta.y !== 0 ? ev.pixelDelta.y : ev.angleDelta.y / 2
                            if (ev.phase === Qt.ScrollEnd && row.swiping) { row.finishSwipe(); return }
                            if (dx === 0 || Math.abs(dy) > Math.abs(dx)) { ev.accepted = false; return }
                            if (ev.phase === Qt.ScrollMomentum) return
                            row.swipeBy(dx)
                        }
                        onClicked: function(ev) {
                            messageList.forceActiveFocus()
                            // Right-click: the menu acts on this message (or on all the
                            // picked ones), and opening it doesn't count as reading.
                            if (ev.button === Qt.RightButton) {
                                if (!mail.isPicked(row.modelData.id)) { mail.selectedIds = []; mail.select(row.modelData.id, true) }
                                rowMenuLoader.active = true
                                rowMenuLoader.item.open()
                            } else {
                                mail.pick(row.modelData.id, row.index, ev.modifiers)
                            }
                        }
                        // Drafts open for editing; anything else in a tab of its own
                        // (with Shift held: in a window of its own).
                        onDoubleClicked: function(ev) {
                            if (ev.button !== Qt.LeftButton || mail.openDraft()) return
                            if (ev.modifiers & Qt.ShiftModifier) mail.openInWindow(row.modelData.id)
                            else mail.openInTab(row.modelData.id)
                        }
                    }
                    // Made when it's opened: a menu (and its shortcuts) in every row made
                    // long lists slow to build.
                    Loader {
                        id: rowMenuLoader
                        active: false
                        sourceComponent: Component {
                        P.Menu {
                            id: rowMenu
                            P.MenuItem { icon.name: "mail-mark-unread"; shortcut: "Ctrl+Shift+U"; text: row.modelData.read ? "Mark as Unread" : "Mark as Read"; onTriggered: mail.setCurrentRead(!row.modelData.read) }
                            P.MenuItem { icon.name: "flag-red"; shortcut: "Ctrl+Shift+L"; text: row.modelData.flagged ? "Unflag" : "Flag"; onTriggered: mail.toggleCurrentFlag() }
                            P.Menu {
                                id: rowFlagMenu
                                title: "Flag Colour"
                                icon.name: "flag"
                                Instantiator {
                                    model: 7
                                    onObjectAdded: (i, o) => rowFlagMenu.insertItem(i, o)
                                    onObjectRemoved: (i, o) => rowFlagMenu.removeItem(o)
                                    delegate: P.MenuItem {
                                        required property int index
                                        text: Theme.flagNames[index]
                                        icon.source: mail.flagIcon(index)
                                        checkable: true
                                        checked: row.modelData.flagged && (row.modelData.flagColor || 0) === index
                                        onTriggered: mail.setCurrentFlag(index)
                                    }
                                }
                            }
                            P.MenuSeparator {}
                            P.MenuItem { icon.name: "mail-reply-sender"; shortcut: "Ctrl+R"; text: "Reply"; onTriggered: mail.reply(false) }
                            P.MenuItem { icon.name: "mail-reply-all"; shortcut: "Ctrl+Shift+R"; text: "Reply All"; onTriggered: mail.reply(true) }
                            P.MenuItem { icon.name: "mail-forward"; shortcut: "Ctrl+Shift+F"; text: "Forward"; onTriggered: mail.forward() }
                            P.MenuSeparator {}
                            P.Menu {
                                id: rowMoveMenu
                                title: "Move to"
                                Instantiator {
                                    model: mail.folders
                                    onObjectAdded: (i, o) => rowMoveMenu.insertItem(i, o)
                                    onObjectRemoved: (i, o) => rowMoveMenu.removeItem(o)
                                    delegate: P.MenuItem {
                                        required property var modelData
                                        text: (mail.sidebar.accounts.length > 1 ? modelData.account + " › " : "") + modelData.name
                                        onTriggered: mail.moveCurrentToFolder(modelData.id)
                                    }
                                }
                            }
                            P.MenuItem { icon.name: "archive-insert"; text: "Archive\tCtrl+Shift+A"; onTriggered: mail.removeCurrent("archive") }
                            P.MenuItem { icon.name: "mail-mark-junk"; text: "Move to Junk\tCtrl+Shift+J"; onTriggered: mail.removeCurrent("junk") }
                            P.MenuItem { icon.name: "edit-delete"; text: "Delete\tDel"; onTriggered: mail.removeCurrent("delete") }
                            P.MenuSeparator {}
                            P.Menu {
                                title: "Remind Me"
                                P.MenuItem { icon.name: "chronometer"; shortcut: "Ctrl+Alt+Shift+H"; text: "In 1 Hour"; onTriggered: mail.remindIn(3600000) }
                                P.MenuItem { icon.name: "chronometer"; shortcut: "Ctrl+Alt+Shift+T"; text: "Tonight"; onTriggered: mail.remindAt(21, 0) }
                                P.MenuItem { icon.name: "chronometer"; shortcut: "Ctrl+Alt+Shift+O"; text: "Tomorrow Morning"; onTriggered: mail.remindAt(8, 1) }
                                P.MenuItem { icon.name: "chronometer"; shortcut: "Ctrl+Alt+Shift+W"; text: "Next Week"; onTriggered: mail.remindAt(8, 7) }
                                P.MenuItem { icon.name: "edit-clear-history"; shortcut: "Ctrl+Alt+Shift+X"; text: "Remove Reminder"; onTriggered: store.remindMe(row.modelData.id, 0) }
                            }
                            P.MenuItem { icon.name: "mail-thread-ignored"; text: (row.modelData.muted ? "Unmute Conversation" : "Mute Conversation") + "\tCtrl+Shift+M"; onTriggered: store.toggleMute(row.modelData.id) }
                            P.MenuItem { icon.name: "emblem-favorite"; shortcut: "Ctrl+Alt+V"; text: row.modelData.vip ? "Remove Sender from VIPs" : "Add Sender to VIPs"; onTriggered: store.toggleVip(row.modelData.fromEmail) }
                            P.MenuItem { icon.name: "im-ban-user"; text: ("Block Sender") + "\tCtrl+Alt+B"; onTriggered: store.toggleBlocked(row.modelData.fromEmail) }
                            P.MenuItem { icon.name: "document-print"; shortcut: "Ctrl+P"; text: "Print…"; onTriggered: mail.printCurrent() }
                        }
                        }
                    }
                }

                Text {
                    anchors.centerIn: parent
                    visible: mail.list.length === 0
                    text: store.loading ? "Loading mail…" : (!mail.hasAccounts && mail.page.all === 0 ? "" : (mail.query ? "No Results" : (mail.unreadOnly ? "No Unread Messages" : "No Messages")))
                    color: Theme.tertiary
                    font.pixelSize: 16
                }
            }
        }

        // ------------------------------------------- the open conversation
        Rectangle {
            SplitView.fillWidth: true
            SplitView.fillHeight: true
            color: Theme.window

            Text {
                anchors.centerIn: parent
                visible: mail.current === null
                text: mail.hasAccounts || mail.page.all > 0 ? "No Message Selected" : ""
                color: Theme.tertiary
                font.pixelSize: 20
            }
                // No accounts and nothing here yet: point to adding one.
                Column {
                    visible: !mail.hasAccounts && mail.page.all === 0 && mail.selection === "kind:inbox"
                    anchors.centerIn: parent
                    spacing: 12
                    Icon { anchors.horizontalCenter: parent.horizontalCenter; name: "mail"; width: 54; height: 54; color: Theme.tertiary }
                    Text { anchors.horizontalCenter: parent.horizontalCenter; text: "No mail yet"; font.pixelSize: 22; font.weight: Font.Bold; color: Theme.text }
                    Text { anchors.horizontalCenter: parent.horizontalCenter; text: "Add an email account to see your messages here."; color: Theme.secondary }
                    ToolButtonX { anchors.horizontalCenter: parent.horizontalCenter; text: "Add Account…"; primary: true; onClicked: mail.accountsRequested() }
                }

            Column {
                visible: mail.multi
                anchors.centerIn: parent
                spacing: 10
                Icon { anchors.horizontalCenter: parent.horizontalCenter; name: "mail"; width: 54; height: 54; color: Theme.tertiary }
                Text { anchors.horizontalCenter: parent.horizontalCenter; text: mail.selectedIds.length + " messages selected"; font.pixelSize: 20; font.weight: Font.Bold; color: Theme.text }
                Text { anchors.horizontalCenter: parent.horizontalCenter; text: "Delete, move, archive, flag or mark them all at once."; color: Theme.secondary }
            }

            // The open conversation: notices along the top (draft, invitation,
            // reminder), then the messages in a web view.
            ColumnLayout {
                id: reader
                anchors.fill: parent
                visible: mail.current !== null && !mail.multi
                spacing: 8
                Rectangle {
                    visible: mail.current !== null && mail.current.kind === "drafts"
                    Layout.leftMargin: 18; Layout.topMargin: 12
                    Layout.preferredWidth: draftRow.implicitWidth + 20; Layout.preferredHeight: 32; radius: 8
                    color: Theme.accentSoft
                    Row {
                        id: draftRow
                        anchors.centerIn: parent
                        spacing: 8
                        Text { text: "This is a draft."; color: Theme.text; anchors.verticalCenter: parent.verticalCenter }
                        Text {
                            text: "Continue writing"; color: Theme.accent; font.weight: Font.DemiBold
                            anchors.verticalCenter: parent.verticalCenter
                            MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: mail.openDraft() }
                        }
                    }
                }
                InviteCard {
                    visible: messageId >= 0
                    Layout.fillWidth: true
                    Layout.leftMargin: 18; Layout.rightMargin: 18; Layout.topMargin: 12
                    store: mail.store
                    messageId: mail.current && mail.current.invite ? mail.current.id : -1
                }
                Rectangle {
                    visible: mail.current !== null && !!mail.current.remindAt
                    Layout.leftMargin: 18; Layout.topMargin: 12
                    Layout.preferredWidth: remRow.implicitWidth + 20; Layout.preferredHeight: 30; radius: 8
                    color: Theme.accentSoft
                    Row {
                        id: remRow
                        anchors.centerIn: parent
                        spacing: 6
                        Icon { name: "clock"; width: 14; height: 14; color: Theme.red; anchors.verticalCenter: parent.verticalCenter }
                        Text { text: mail.current && mail.current.remindAt ? "Reminder " + Theme.fmtLongDate(mail.current.remindAt) : ""; color: Theme.text; font.pixelSize: 12; anchors.verticalCenter: parent.verticalCenter }
                    }
                }
                MessagePage {
                    id: messagePage
                    store: mail.store
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    messages: mail.conversation
                    subject: mail.current ? mail.current.subject : ""
                    formatted: mail.formatted
                    darkMessages: !!mail.prefs.dark_messages
                    loadRemoteAlways: !!mail.prefs.load_remote
                    remoteLoaded: mail.remoteLoaded
                    remoteSenders: mail.prefs.remote_senders || []
                    previewsFor: function(id) { return JSON.parse(store.attachmentPreviews(id)) }
                    zoom: mail.textZoom
                    onSenderClicked: function(m) { mail.senderClicked(m) }
                    onAttachmentClicked: function(m, name, save) { if (save) mail.saveAttachment(m, name); else store.openAttachment(m.id, name, false) }
                    onAttachmentView: function(m, name, where) { mail.attachmentView(m, name, where) }
                    onRemoteRequested: function(m, scope) { mail.remoteRequested(m, scope) }
                    onFormatToggled: mail.formatted = !mail.formatted
                    onMailto: function(address) { compose.openWith({ to: address }) }
                    // The buttons on one message of a conversation.
                    onMessageAction: function(m, action) { mail.messageAction(m, action, true) }
                }
                // A reply or forward written right under the message (the writing
                // area of the compose sheet moves in here). Drag its top edge to resize.
                Rectangle {
                    id: replyFrame
                    property real wantH: 0
                    visible: compose.inline && compose.inlineHost === replyHost
                    Layout.fillWidth: true
                    Layout.leftMargin: 12; Layout.rightMargin: 12; Layout.bottomMargin: 12
                    Layout.preferredHeight: Math.max(260, Math.min(reader.height - 140, wantH > 0 ? wantH : reader.height * 0.55))
                    radius: 8
                    color: Theme.window
                    border.color: Theme.border
                    Item { id: replyHost; anchors.fill: parent; anchors.margins: 1; clip: true }
                    MouseArea {
                        anchors.left: parent.left; anchors.right: parent.right
                        y: -6; height: 10; z: 10
                        cursorShape: Qt.SplitVCursor
                        preventStealing: true
                        property real y0: 0
                        property real h0: 0
                        onPressed: function(ev) { y0 = mapToItem(null, ev.x, ev.y).y; h0 = replyFrame.height }
                        onPositionChanged: function(ev) { if (pressed) replyFrame.wantH = h0 - (mapToItem(null, ev.x, ev.y).y - y0) }
                    }
                }
                P.Menu {
                    id: senderMenu
                    property var m: null
                    P.MenuItem { icon.name: "mail-message-new"; text: senderMenu.m ? "New Message to " + (senderMenu.m.fromName || senderMenu.m.fromEmail) : ""; onTriggered: compose.openWith({ to: senderMenu.m.fromEmail }) }
                    P.MenuItem { icon.name: "emblem-favorite"; shortcut: "Ctrl+Alt+V"; text: senderMenu.m && senderMenu.m.vip ? "Remove from VIPs" : "Add to VIPs"; onTriggered: store.toggleVip(senderMenu.m.fromEmail) }
                    P.MenuItem { icon.name: "im-ban-user"; text: (senderMenu.m && senderMenu.m.blocked ? "Unblock Contact" : "Block Contact") + "\tCtrl+Alt+B"; onTriggered: store.toggleBlocked(senderMenu.m.fromEmail) }
                    P.MenuItem { icon.name: "edit-copy"; text: "Copy Address"; onTriggered: { copyHelper.text = senderMenu.m.fromEmail; copyHelper.selectAll(); copyHelper.copy() } }
                }
                // Ctrl + scroll / pinch on the trackpad zoom the messages.
                WheelHandler {
                    target: null
                    acceptedModifiers: Qt.ControlModifier
                    onWheel: function(ev) { mail.textZoom = Math.max(0.5, Math.min(3, mail.textZoom * (ev.angleDelta.y > 0 ? 1.1 : 1 / 1.1))) }
                }
            }
        }
        }
    }
}
