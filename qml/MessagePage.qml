import QtQuick
import QtQuick.Layouts
import QtWebEngine
// The system's own menus (quick to open, unlike the browser engine's)
import Qt.labs.platform as P

// A conversation shown the way a browser shows a page (Qt WebEngine, i.e.
// Chromium), as Apple Mail uses WebKit: real email layouts, background
// pictures, wide newsletters that scroll sideways, pinch to zoom.
//
// Safety: each email sits in its own sealed frame (no scripts, no forms, no
// plug-ins, its styles can't touch the rest of the page), and the page's
// Content-Security-Policy lets nothing load from the internet unless remote
// content was allowed. Links open in the system browser (mail links in a new
// message here).
WebEngineView {
    id: web
    // Back from the desktop's overview, or after the window lost and got back the
    // focus, the page could draw black until clicked: the browser engine stopped
    // drawing meanwhile. Hiding and showing it right away wasn't always enough (it
    // could happen before a frame was drawn, or before the window was really back).
    // Now the page is made 1 px shorter for a moment, which makes the engine draw
    // a fresh frame at the new size, and its content is nudged; this is done
    // straight away and again shortly after, in case the window came back late.
    Connections {
        target: web.Window.window
        function onActiveChanged() { if (web.Window.window && web.Window.window.active) web.redraw() }
        function onVisibilityChanged() { web.redraw() }
        // Back in view without getting the focus (the black-pane case).
        function onReexposed() { web.paneLog("window shown again"); web.redraw() }
    }
    Connections {
        target: Qt.application
        function onStateChanged() { if (Qt.application.state === Qt.ApplicationActive) web.redraw() }
    }
    // What happens around a black page, written to the log (ALMANAC-PANE …) so
    // the next time it happens shows what came before: the window's focus and
    // visibility, the page's, the engine's state, lost graphics.
    function paneLog(what) {
        console.warn("ALMANAC-PANE " + new Date().toISOString() + " " + what
                     + " | win active=" + (web.Window.window ? web.Window.window.active : "?")
                     + " vis=" + (web.Window.window ? web.Window.window.visibility : "?")
                     + " app=" + Qt.application.state + " page visible=" + web.visible
                     + " life=" + web.lifecycleState + " rec=" + web.recommendedState
                     + " api=" + GraphicsInfo.api)
    }
    Connections {
        target: web.Window.window
        function onActiveChanged() { web.paneLog("window active changed") }
        function onVisibilityChanged() { web.paneLog("window visibility changed") }
        function onSceneGraphInvalidated() { web.paneLog("scene graph invalidated") }
        function onSceneGraphError(error, message) { web.paneLog("scene graph error " + error + ": " + message) }
    }
    Connections {
        target: Qt.application
        function onStateChanged() { web.paneLog("app state changed") }
    }
    onVisibleChanged: paneLog("page visible changed")
    onLifecycleStateChanged: paneLog("lifecycle state changed")
    onRecommendedStateChanged: paneLog("recommended state changed")
    // The page tells which messages were opened or closed, and whose details are
    // shown, through its title ("state/<message>/<open|details|all>/<1|0>/<n>"):
    // kept, so the next redraw keeps them.
    onTitleChanged: {
        var p = String(web.title).split("/")
        if (p[0] !== "state") return
        var m = web.messages[parseInt(p[1])]
        if (!m) return
        var on = p[3] === "1"
        if (p[2] === "open") web.expandedState[m.id] = on
        else if (p[2] === "details") web.detailsState[m.id] = on
        else if (p[2] === "all") for (var k = 0; k < web.messages.length; k++) web.expandedState[web.messages[k].id] = on
    }
    onLoadingChanged: function(info) { paneLog("loading " + ["started", "stopped", "succeeded", "failed"][info.status] + (info.errorString ? " " + info.errorString : "")) }
    onRenderProcessTerminated: function(status, code) { paneLog("render process ended, status " + status + " code " + code) }
    property int redrawPasses: 0
    function redraw() {
        if (!web.visible) return
        // (Already redrawing: start the passes over, from now.)
        web.redrawPasses = 3
        if (!shrinkBack.running && !nextPass.running) redrawPass()
    }
    function redrawPass() {
        if (!web.visible || web.redrawPasses <= 0) return
        web.redrawPasses--
        web.paneLog("redraw pass")
        web.Layout.bottomMargin = 1
        shrinkBack.restart()
    }
    Timer {
        id: shrinkBack
        interval: 50
        onTriggered: {
            web.Layout.bottomMargin = 0
            web.runJavaScript("(function(d) { if (!d) return; d.style.outline = '0 solid transparent';"
                              + " requestAnimationFrame(function() { d.style.outline = ''; }); })(document.documentElement)")
            web.update()
            if (web.redrawPasses > 0) nextPass.restart()
        }
    }
    Timer { id: nextPass; interval: 350; onTriggered: web.redrawPass() }
    property var messages: []
    property string subject: ""
    property bool formatted: true
    property bool darkMessages: true
    property bool loadRemoteAlways: false
    property var remoteLoaded: ({})
    // In a conversation: which messages were opened or closed by hand, and whose
    // details (To, Cc, address) are shown, by message id. Kept across reloads.
    property var expandedState: ({})
    property var detailsState: ({})
    // Where to scroll once drawn: a conversation newly opened goes to its first
    // unread message (or its newest); drawn again (mail arriving, a message
    // marked read) it stays where it was (keepScroll, read from the page first).
    property string shownConv: ""
    property real keepScroll: -1
    // Senders whose remote content always loads (Settings › Privacy).
    property var remoteSenders: []
    // Pictures and PDFs to show inside each message: a function (message id
    // → [{name, mime, data}]) supplied by the mail view.
    property var previewsFor: null
    // For pages over 2 MB (the most a page can be handed over directly):
    // they're saved to a private file and loaded from there.
    property var store: null
    readonly property string slot: "p" + Math.floor(Math.random() * 1e12).toString(36)
    property int fileLoads: 0
    property var previews: ({})
    // Pictures and PDFs read once per message (big PDFs are slow to read and
    // encode, and the page is asked to redraw whenever anything changes).
    property var previewCache: ({})
    function previewsOf(m) {
        var key = m.attachments.join("|")
        var c = previewCache[m.id]
        if (c && c.key === key) return c.list
        var list = web.previewsFor(m.id)
        var ready = list.filter(function(p) { return !!p.data || !!p.pages })
        // All here: kept. Some still downloading: asked again on the next redraw.
        if (ready.length === list.length) previewCache[m.id] = { key: key, list: ready }
        return ready
    }
    // The PDFs on the page (to tell which one the PDF viewer means).
    function pdfs() {
        var out = []
        for (var i = 0; i < web.messages.length; i++) {
            var shown = web.previews[web.messages[i].id] || []
            for (var p = 0; p < shown.length; p++) if (shown[p].mime === "application/pdf") out.push({ m: web.messages[i], name: shown[p].name })
        }
        return out
    }
    function pdfNamed(file) {
        var all = pdfs()
        for (var i = 0; i < all.length; i++) if (all[i].name === file) return all[i]
        return all.length === 1 ? all[0] : null
    }
    // A PDF viewer's download button: the Save dialog for that attachment.
    // (Downloads are the browser profile's, shared by every page: only this page's.)
    Connections {
        target: web.profile
        function onDownloadRequested(download) {
            // (From a PDF viewer inside the page, the view given is the viewer's
            // own, or none: then it's the page the pointer is over.)
            console.warn("ALMANAC-PDF download asked: " + download.suggestedFileName + " view " + (download.view === web ? "this page" : download.view ? "another" : "none"))
            if (Theme.handledDownload === download.id) return
            if (download.view !== web && Theme.pointerPage !== web) return
            var hit = web.pdfNamed(download.suggestedFileName) || web.pdfs()[0]
            if (!hit) return
            Theme.handledDownload = download.id
            download.cancel()
            web.attachmentClicked(hit.m, hit.name, true)
        }
    }
    HoverHandler { onHoveredChanged: if (hovered) Theme.pointerPage = web }
    // The PDF viewer's print button (asked for in one of two ways).
    function printPdf(how) {
        console.warn("ALMANAC-PDF print asked (" + how + ")")
        var now = Date.now()
        if (now - Theme.printAskedAt < 1500) return
        var hit = web.pdfNamed("") || web.pdfs()[0]
        if (!hit || !web.store) return
        Theme.printAskedAt = now
        web.store.printAttachment(hit.m.id, hit.name)
    }
    onPrintRequested: printPdf("page")

    // The PDF viewer's print button: the PDF itself, through the print dialog.
    onPrintRequestedByFrame: function(frame) { printPdf("frame") }

    // Right-click: the system's menu (the browser engine's is slow to open).
    onContextMenuRequested: function(request) {
        request.accepted = true
        var link = request.linkUrl.toString()
        if (link.indexOf("almanac:addr/") === 0) {
            var ap = link.slice(8).split("/")
            web.openAddressMenu(decodeURIComponent(ap[2] || ""), decodeURIComponent(ap[3] || ""))
            return
        }
        var att = null
        if (/^almanac:(att|save|viewtab|viewwin|openmenu)\//.test(link)) {
            var parts = link.slice(8).split("/")
            att = { m: web.messages[parseInt(parts[1])], name: decodeURIComponent(parts.slice(2).join("/")) }
        } else if (request.mediaType === ContextMenuRequest.MediaTypePlugin || request.mediaType === ContextMenuRequest.MediaTypeImage) {
            var all = pdfs()
            if (request.mediaType === ContextMenuRequest.MediaTypePlugin && all.length === 1) att = all[0]
        }
        pageMenu.att = att && att.m ? att : null
        pageMenu.link = /^(https?|mailto):/i.test(link) ? link : ""
        pageMenu.hasSelection = request.selectedText !== ""
        pageMenu.attOnly = false
        pageMenu.open()
    }
    // "Open ▾" on a picture, PDF or attachment: the ways to open it, as a drop-down.
    function openMenuFor(m, name) {
        pageMenu.att = { m: m, name: name }
        pageMenu.link = ""
        pageMenu.hasSelection = false
        pageMenu.attOnly = true
        pageMenu.open()
    }
    // "Name <a@b>, \"Last, First\" <c@d>, e@f" → [{name, email}] (commas inside
    // quotes or angle brackets don't split).
    function parseAddresses(text) {
        var out = [], cur = "", quoted = false, angle = false
        var flush = function() {
            var t = cur.trim(); cur = ""
            if (!t) return
            var lt = t.lastIndexOf("<"), gt = t.lastIndexOf(">")
            var email = lt >= 0 && gt > lt ? t.slice(lt + 1, gt).trim() : t
            var name = lt >= 0 ? t.slice(0, lt).trim().replace(/^"(.*)"$/, "$1").trim() : ""
            out.push({ name: name, email: email })
        }
        for (var k = 0; k < text.length; k++) {
            var c = text.charAt(k)
            if (c === '"') quoted = !quoted
            else if (c === "<" && !quoted) angle = true
            else if (c === ">" && !quoted) angle = false
            if ((c === "," || c === ";") && !quoted && !angle) flush()
            else cur += c
        }
        flush()
        return out
    }
    // The start of a message's text, on one line (for a closed message).
    function snippet(m) {
        var t = m.body || (m.html ? String(m.html).replace(/<style[\s\S]*?<\/style>|<script[\s\S]*?<\/script>/gi, " ").replace(/<[^>]+>/g, " ")
                                           .replace(/&nbsp;/g, " ").replace(/&amp;/g, "&").replace(/&lt;/g, "<").replace(/&gt;/g, ">") : "")
        t = t.split("\n").filter(function(l) { return !/^\s*>/.test(l) }).join(" ")
        return t.replace(/\s+/g, " ").trim().slice(0, 220)
    }
    function chips(i, text) {
        return parseAddresses(text).map(function(a) {
            var full = a.name ? a.name + " <" + a.email + ">" : a.email
            return '<a class="chip" title="' + esc(full) + '" href="almanac:addr/' + i + '/'
                   + encodeURIComponent(a.email) + '/' + encodeURIComponent(a.name) + '">' + esc(a.name || a.email) + '</a>'
        }).join("")
    }
    function openAddressMenu(email, name) {
        addressMenu.email = email
        addressMenu.name = name
        addressMenu.open()
    }
    function copyText(t) { copyHelper.text = t; copyHelper.selectAll(); copyHelper.copy() }
    TextEdit { id: copyHelper; visible: false }
    P.Menu {
        id: addressMenu
        property string email: ""
        property string name: ""
        P.MenuItem { icon.name: "mail-message-new"; text: "New Message to " + (addressMenu.name || addressMenu.email); onTriggered: web.mailto(addressMenu.email) }
        P.MenuSeparator {}
        P.MenuItem { icon.name: "edit-copy"; text: "Copy Email Address"; onTriggered: web.copyText(addressMenu.email) }
        P.MenuItem { visible: addressMenu.name !== ""; icon.name: "edit-copy"; text: "Copy Name"; onTriggered: web.copyText(addressMenu.name) }
        P.MenuItem { visible: addressMenu.name !== ""; icon.name: "edit-copy"; text: "Copy Name and Address"; onTriggered: web.copyText(addressMenu.name + " <" + addressMenu.email + ">") }
    }
    P.Menu {
        id: pageMenu
        property var att: null
        property string link: ""
        property bool hasSelection: false
        // Just the attachment's own choices (the Open ▾ drop-down).
        property bool attOnly: false
        P.MenuItem { visible: !!pageMenu.att; icon.name: "document-open"; text: "Open"; onTriggered: web.attachmentClicked(pageMenu.att.m, pageMenu.att.name, false) }
        P.MenuItem { visible: !!pageMenu.att; icon.name: "tab-new"; text: "Open in New Tab"; onTriggered: web.attachmentView(pageMenu.att.m, pageMenu.att.name, "tab") }
        P.MenuItem { visible: !!pageMenu.att; icon.name: "window-new"; text: "Open in New Window"; onTriggered: web.attachmentView(pageMenu.att.m, pageMenu.att.name, "window") }
        P.MenuSeparator { visible: !!pageMenu.att }
        P.MenuItem { visible: !!pageMenu.att && /\.pdf$/i.test(pageMenu.att.name); icon.name: "document-print"; text: "Print…"; onTriggered: { if (web.store) web.store.printAttachment(pageMenu.att.m.id, pageMenu.att.name) } }
        P.MenuItem { visible: !!pageMenu.att; icon.name: "document-save-as"; text: "Save As…"; onTriggered: web.attachmentClicked(pageMenu.att.m, pageMenu.att.name, true) }
        P.MenuSeparator { visible: !!pageMenu.att && !pageMenu.attOnly }
        P.MenuItem { visible: pageMenu.link !== ""; icon.name: "internet-web-browser"; text: "Open Link"; onTriggered: web.follow(pageMenu.link) }
        P.MenuItem { visible: pageMenu.link !== ""; icon.name: "edit-copy"; text: "Copy Link"; onTriggered: { linkCopy.text = pageMenu.link.replace(/^mailto:/i, ""); linkCopy.selectAll(); linkCopy.copy() } }
        P.MenuSeparator { visible: pageMenu.link !== "" }
        P.MenuItem { visible: !pageMenu.attOnly; enabled: pageMenu.hasSelection; icon.name: "edit-copy"; text: "Copy"; onTriggered: web.triggerWebAction(WebEngineView.Copy) }
        P.MenuItem { visible: !pageMenu.attOnly; icon.name: "edit-select-all"; text: "Select All"; onTriggered: web.triggerWebAction(WebEngineView.SelectAll) }
    }
    TextEdit { id: linkCopy; visible: false }
    function senderTrusted(m) { return web.remoteSenders.indexOf((m.fromEmail || "").toLowerCase()) >= 0 }
    property real zoom: 1
    signal senderClicked(var message)
    signal attachmentClicked(var message, string name, bool save)
    /// A picture or PDF shown in a message, to open in a tab ("tab") or a window ("window").
    signal attachmentView(var message, string name, string where)
    signal remoteRequested(var message, string scope)
    signal formatToggled()
    signal mailto(string address)
    /// A button on one message's header: "reply", "replyAll", "forward" or "delete".
    signal messageAction(var message, string action)

    zoomFactor: zoom
    // Dark mode with "Use dark backgrounds for messages": every email,
    // designed ones too, is adapted to a dark background by the browser
    // engine's own dark mode (like Apple Mail and Thunderbird), instead of
    // being shown on a white page.
    readonly property bool darkify: Theme.dark && darkMessages
    // macOS trackpads glide by themselves; elsewhere the page adds it.
    readonly property bool glide: Qt.platform.os !== "osx"
    settings.forceDarkMode: darkify
    backgroundColor: Theme.window
    // Only this page's own script runs (it sizes the frames); emails never run any.
    settings.javascriptEnabled: true
    settings.javascriptCanOpenWindows: false
    settings.javascriptCanAccessClipboard: false
    // Only Chromium's own PDF viewer (for attached PDFs shown in the message).
    settings.pluginsEnabled: true
    settings.pdfViewerEnabled: true
    settings.localContentCanAccessFileUrls: false
    settings.localContentCanAccessRemoteUrls: true
    settings.autoLoadImages: true
    settings.focusOnNavigationEnabled: false
    settings.navigateOnDropEnabled: false
    settings.showScrollBars: true

    onMessagesChanged: Qt.callLater(render)
    onFormattedChanged: Qt.callLater(render)
    onRemoteLoadedChanged: Qt.callLater(render)
    onLoadRemoteAlwaysChanged: Qt.callLater(render)
    onRemoteSendersChanged: Qt.callLater(render)
    onDarkMessagesChanged: Qt.callLater(render)
    Connections { target: Theme; function onDarkChanged() { Qt.callLater(web.render) } }

    // Clicks: our own links ("almanac:…"), links inside an email, new windows.
    onNavigationRequested: function(request) {
        var url = request.url.toString()
        // Loading the page and its frames is fine; anything clicked is handled here.
        if (request.navigationType !== WebEngineNavigationRequest.LinkClickedNavigation && url.indexOf("almanac:") !== 0) {
            request.accept()
            return
        }
        request.reject()
        web.follow(url)
    }
    onNewWindowRequested: function(request) { web.follow(request.requestedUrl.toString()) }

    function follow(url) {
        if (url.indexOf("almanac:") === 0) {
            var parts = url.slice(8).split("/")
            var m = web.messages[parseInt(parts[1])]
            if (!m) return
            if (parts[0] === "sender") web.senderClicked(m)
            else if (parts[0] === "addr") web.openAddressMenu(decodeURIComponent(parts[2] || ""), decodeURIComponent(parts[3] || ""))
            else if (parts[0] === "remote") web.remoteRequested(m, "message")
            else if (parts[0] === "remote-sender") web.remoteRequested(m, "sender")
            else if (parts[0] === "remote-all") web.remoteRequested(m, "all")
            else if (parts[0] === "format") web.formatToggled()
            else if (parts[0] === "act") web.messageAction(m, parts[2])
            else if (parts[0] === "print") { if (web.store) web.store.printAttachment(m.id, decodeURIComponent(parts.slice(2).join("/"))) }
            else if (parts[0] === "openmenu") web.openMenuFor(m, decodeURIComponent(parts.slice(2).join("/")))
            else if (parts[0] === "viewtab" || parts[0] === "viewwin") web.attachmentView(m, decodeURIComponent(parts.slice(2).join("/")), parts[0] === "viewtab" ? "tab" : "window")
            else if (parts[0] === "att" || parts[0] === "save") web.attachmentClicked(m, decodeURIComponent(parts.slice(2).join("/")), parts[0] === "save")
            return
        }
        if (url.indexOf("mailto:") === 0) { web.mailto(decodeURIComponent(url.slice(7).split("?")[0])); return }
        if (/^(https?|ftp):/i.test(url)) Qt.openUrlExternally(url)
    }

    function esc(t) { return String(t || "").replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;") }
    function hasRemote(html) { return /data-remote-(src|background)=|remote-url\(/.test(html || "") }
    function withRemote(html) { return html.replace(/data-remote-src=/g, "src=").replace(/data-remote-background=/g, "background=").replace(/remote-url\(/g, "url(") }
    // A designed email: sets its own colours or backgrounds.
    function ownLook(html) { return /bgcolor|background|color\s*[:=]/i.test(html || "") }
    function linkify(t) { return t.replace(/(https?:\/\/[^\s<>"]+)/g, '<a href="$1">$1</a>') }
    function css(c) { return c.toString() }

    function emailDoc(m, paper, remote) {
        // Adapted by the engine's dark mode: written as a light page, which it darkens.
        var lightDoc = paper || web.darkify || !Theme.dark
        var dark = !lightDoc
        var text = lightDoc ? "#1d1d1f" : css(Theme.text)
        var link = lightDoc ? "#0a66d8" : css(Theme.accent)
        var content
        if (m.html && web.formatted) content = remote ? withRemote(m.html) : m.html
        else content = '<div style="white-space:pre-wrap">' + linkify(esc(m.body)) + '</div>'
        return '<!doctype html><html><head><meta charset="utf-8"><base target="_blank">'
             + '<style>html{overflow:hidden;overscroll-behavior:none;color-scheme:' + (dark ? 'dark' : 'light') + ';background:' + (paper ? '#ffffff' : 'transparent') + '}'
             + 'body{margin:0;padding:' + (paper ? '12px' : '0') + ';color:' + text + ';font:14px/1.45 system-ui,-apple-system,"Segoe UI",sans-serif;overflow-wrap:break-word}'
             + 'img{max-width:100%;height:auto}a{color:' + link + '}pre{white-space:pre-wrap}blockquote{margin-left:6px;padding-left:10px;border-left:2px solid ' + (dark ? css(Theme.accent) : '#2f7cf6') + '}</style>'
             + '</head><body>' + content + '</body></html>'
    }

    function actions(i) {
        var icons = [
            ["reply", "Reply", "M7 5 2.5 9.5 7 14M3 9.5h7.5a5 5 0 0 1 5 5V16"],
            ["replyAll", "Reply All", "M8 5 3.5 9.5 8 14M5 5 .5 9.5 5 14M4.5 9.5h6.5a5 5 0 0 1 5 5V16"],
            ["forward", "Forward", "M13 5l4.5 4.5L13 14M17 9.5H9.5a5 5 0 0 0-5 5V16"],
            ["delete", "Delete", "M3.5 5.5h13M8 5.5V3.5h4v2M5 5.5l.8 11h8.4l.8-11"]]
        var h = ""
        for (var k = 0; k < icons.length; k++)
            h += '<a href="almanac:act/' + i + '/' + icons[k][0] + '" title="' + icons[k][1] + '"><svg viewBox="0 0 20 20"><path d="' + icons[k][2] + '"/></svg></a>'
        return h
    }

    function page() {
        var anyRemote = web.loadRemoteAlways
        for (var r in web.remoteLoaded) if (web.remoteLoaded[r]) anyRemote = true
        for (var q = 0; q < web.messages.length; q++) if (senderTrusted(web.messages[q])) anyRemote = true
        var remoteSrc = anyRemote ? " http: https:" : ""
        var nonce = Math.random().toString(36).slice(2)
        var boxed = web.messages.length > 1
        var h = '<!doctype html><html><head><meta charset="utf-8">'
              + '<meta http-equiv="Content-Security-Policy" content="default-src \'none\'; script-src \'nonce-' + nonce + '\'; style-src \'unsafe-inline\'' + remoteSrc
              + '; img-src data: cid: blob:' + remoteSrc + '; font-src data:' + remoteSrc + '; frame-src about: data:; object-src data:; media-src \'none\'; form-action \'none\'">'
              + '<style>:root{color-scheme:' + (Theme.dark ? 'dark' : 'light') + '}'
              + 'body{margin:0;padding:16px 18px 40px;background:' + css(Theme.window) + ';color:' + css(Theme.text) + ';font:13px/1.4 system-ui,-apple-system,"Segoe UI",sans-serif}'
              + 'h1{font-size:19px;margin:0 0 14px;overflow-wrap:break-word}'
              + '.msg{' + (boxed ? 'border:1px solid ' + css(Theme.borderSoft) + ';background:' + css(Theme.card) + ';border-radius:10px;padding:14px;' : '') + 'margin-bottom:14px;min-width:0}'
              + '.hdr{display:flex;gap:12px;align-items:flex-start}'
              + '.av{width:40px;height:40px;border-radius:20px;background:linear-gradient(#9cc0fa,#5b8def);color:#fff;font-weight:600;font-size:15px;display:flex;align-items:center;justify-content:center;flex:none}'
              + '.who{flex:1;min-width:0}.top{display:flex;flex-wrap:nowrap;gap:2px 8px;align-items:baseline;min-width:0}.name{flex:0 1 auto;font-weight:700;font-size:14px;color:' + css(Theme.text) + ';text-decoration:none;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;min-width:40px}'
              + '.grow{flex:1}.when{color:' + css(Theme.secondary) + ';font-size:12px;margin-top:1px}.fmtline{margin-top:3px}'
              // Compact, as in Apple Mail: in a conversation older messages are one line
              // each (opened with a click), and an open one's header is short, with
              // its people and address under Details.
              + '.convbar{display:flex;gap:14px;align-items:center;margin:-8px 0 12px;font-size:12px;color:' + css(Theme.secondary) + '}'
              + '.convbar a,.det,.chev{color:' + css(Theme.accent) + ';text-decoration:none}'
              + '.msg.col{padding:8px 12px;margin-bottom:6px;cursor:pointer}.msg.col:hover{background:' + css(Theme.hover) + '}'
              + '.msg.col .full,.msg:not(.col) .row{display:none}'
              // Closed: who and when, then the heart of what they said (up to three lines).
              + '.row{display:flex;gap:10px;align-items:flex-start;min-width:0}'
              + '.row .av{width:28px;height:28px;font-size:11px;margin-top:1px}'
              + '.row .rb{flex:1;min-width:0}.row .rt{display:flex;gap:8px;align-items:baseline}'
              + '.row .rn{font-weight:600;font-size:12.5px;flex:0 1 auto;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}'
              + '.row .rd{flex:none;color:' + css(Theme.secondary) + ';font-size:11.5px}'
              + '.row .snip{margin-top:2px;font-size:12px;color:' + css(Theme.text) + ';line-height:1.4;display:-webkit-box;-webkit-box-orient:vertical;-webkit-line-clamp:3;overflow:hidden;overflow-wrap:anywhere}'
              + '.sum{color:' + css(Theme.secondary) + ';font-size:12px;margin-top:1px;display:flex;gap:6px;min-width:0}'
              + '.sum .who2{min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.det{flex:none;font-size:11px}'
              + '.msg:not(.det-on) .details{display:none}'
              + '.chev{font-size:11px;padding:0 2px}'
              // Closed rows: a blue ▸ before the name (the open message's ▾ closes it).
              + '.top .acts{margin-left:6px;align-self:center}'
              // People on To and Cc as bubbles, as in compose (click or right-click: copy, write to).
              + '.rcpt{display:flex;flex-wrap:wrap;align-items:center;gap:4px;margin-top:3px;font-size:12px}.rcpt .lbl{color:' + css(Theme.tertiary) + ';margin-right:2px}'
              + '.chip{display:inline-block;max-width:100%;padding:1px 9px;border-radius:999px;background:' + css(Theme.segment) + ';color:' + css(Theme.text) + ';text-decoration:none;line-height:18px;overflow-wrap:anywhere;cursor:default}'
              + '.chip:hover{background:' + css(Theme.hover) + '}'
              + '.addr{color:' + css(Theme.tertiary) + ';font-size:12px}.fmt{color:' + css(Theme.accent) + ';font-size:11px;text-decoration:none;white-space:nowrap}'
              + '.rule{height:1px;background:' + css(Theme.borderSoft) + ';margin:10px 0}'
              + '.remote{display:flex;align-items:center;gap:10px;border:1px solid ' + css(Theme.borderSoft) + ';background:' + css(Theme.card) + ';border-radius:7px;padding:7px 10px;margin-bottom:10px;color:' + css(Theme.secondary) + ';font-size:12px}'
              + '.remote{flex-wrap:wrap}.remote a{color:' + css(Theme.accent) + ';text-decoration:none;font-weight:600;white-space:nowrap}.remote a.more{font-weight:normal}'
              + '.paper{background:#fff;border-radius:8px}iframe{border:0;width:100%;height:24px;display:block;background:transparent}'
              + '.atts{display:flex;flex-wrap:wrap;gap:8px;margin-top:12px}.att{display:inline-flex;gap:6px;align-items:center;padding:7px 10px;border-radius:8px;background:' + css(Theme.segment) + ';color:' + css(Theme.text) + ';text-decoration:none;font-size:12px}'
              + '.att b{font-weight:normal}.save{color:' + css(Theme.accent) + ';text-decoration:none;font-size:11px;margin-left:4px}'
              + '.wait{color:' + css(Theme.secondary) + '}'
              + 'html,body{overscroll-behavior:none}::-webkit-scrollbar{width:12px;height:12px;background:transparent}::-webkit-scrollbar-corner{background:transparent}'
              + '::-webkit-scrollbar-thumb{background-color:transparent;border-radius:6px;border:3px solid transparent;background-clip:padding-box}'
              // The message's buttons, as in Apple Mail: one pill under the header,
              // Reply | Reply All | Forward | Delete.
              // A little raised: a soft top-to-bottom shade and a slight shadow.
              + '.acts{display:inline-flex;border:1px solid ' + css(Theme.border) + ';border-radius:999px;overflow:hidden;'
              + 'background:linear-gradient(' + (Theme.dark ? 'rgba(255,255,255,.10),rgba(255,255,255,.03)' : '#ffffff,#eceef2') + ');'
              + 'box-shadow:0 1px 2px rgba(0,0,0,' + (Theme.dark ? '.45' : '.12') + '),inset 0 1px 0 rgba(255,255,255,' + (Theme.dark ? '.06' : '.8') + ')}'
              + '.acts a{display:flex;width:38px;height:24px;align-items:center;justify-content:center;color:' + css(Theme.secondary) + '}'
              + '.acts a+a{border-left:1px solid ' + css(Theme.border) + '}'
              + '.acts a:hover{background:' + css(Theme.segment) + ';color:' + css(Theme.text) + '}'
              + '.acts svg{width:15px;height:15px;fill:none;stroke:currentColor;stroke-width:1.6;stroke-linecap:round;stroke-linejoin:round}'
              + '.inl{display:block;margin-top:12px}.inl img{display:block;max-width:100%;height:auto;border-radius:6px}'
              + '.pdf{display:block;width:100%;height:560px;margin-top:12px;border:1px solid ' + css(Theme.borderSoft) + ';border-radius:6px}'
              + '.pdfwrap{position:relative;margin-top:6px}'
              + '.pdfbox{height:560px;overflow-y:auto;padding:10px;border:1px solid ' + css(Theme.borderSoft) + ';border-radius:6px;background:' + (Theme.dark ? '#2a2b30' : '#e9eaee') + ';scrollbar-width:none}'
              + '.pdfbox::-webkit-scrollbar{width:0;height:0}'
              // Its own scroll bar: shown while it scrolls or the pointer's over it, then fading.
              + '.pdfbar{position:absolute;z-index:2;top:4px;bottom:4px;right:3px;width:11px;opacity:0;transition:opacity .25s;cursor:default}'
              + '.pdfbar.on{opacity:1}'
              + '.pdfthumb{position:absolute;left:3px;right:2px;min-height:24px;border-radius:5px;background:rgba(' + (Theme.dark ? '255,255,255,.42' : '0,0,0,.38') + ');transition:left .15s}'
              + '.pdfbar:hover .pdfthumb,.pdfbar.drag .pdfthumb{left:0;background:rgba(' + (Theme.dark ? '255,255,255,.6' : '0,0,0,.55') + ')}'
              + '.pdfpage{display:block;width:100%;height:auto;margin:0 auto 10px;background:#fff;box-shadow:0 1px 3px rgba(0,0,0,.18)}'
              + '.pdfmore{display:block;margin:4px 0;font-size:12px;color:' + css(Theme.accent) + ';text-decoration:none}'

              + '.pvbar{display:flex;align-items:center;gap:6px;margin-top:14px;font-size:12px}.pvbar span{color:' + css(Theme.secondary) + ';flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}'
              + '.pvbar a{color:' + css(Theme.accent) + ';text-decoration:none;white-space:nowrap;padding:3px 8px;border-radius:6px;background:' + css(Theme.segment) + '}.pvbar a:hover{background:' + css(Theme.hover) + '}'
              + '.pvbar+.inl,.pvbar+.pdf{margin-top:6px}'
              // A split button: the left part opens (in a tab), the arrow shows the other ways.
              + '.pvbar .split{display:inline-flex}.pvbar .split a{border-radius:6px 0 0 6px}.pvbar .split a.more{border-radius:0 6px 6px 0;margin-left:1px;padding:3px 6px}'
              // The thumb's colour lives in a style sheet of its own (filled in by the fade below).
              + '</style><style id="sbshow"></style></head><body>'
              + '<h1>' + esc(web.subject || "(No Subject)") + '</h1>'
              + (boxed ? '<div class="convbar"><span>' + web.messages.length + ' messages</span>'
                         + '<a href="#" data-all="1">Expand All</a><a href="#" data-all="0">Collapse All</a></div>' : '')
        for (var i = 0; i < web.messages.length; i++) {
            var m = web.messages[i]
            var rich = !!m.html && web.formatted
            // A white page only when dark backgrounds for messages are turned off.
            var paper = rich && Theme.dark && !web.darkMessages
            var remote = web.loadRemoteAlways || !!web.remoteLoaded[m.id] || senderTrusted(m)
            var name = m.fromName || m.fromEmail
            // Closed: older, read messages of a conversation (unless opened by hand).
            var st = web.expandedState[m.id]
            var col = boxed && (st === false || (st !== true && i < web.messages.length - 1 && m.read))
            var det = !!web.detailsState[m.id]
            var people = parseAddresses(m.to || "").map(function(a) { return a.name || a.email })
            var ccCount = parseAddresses(m.cc || "").length
            var toLine = (people.length ? "To: " + people.slice(0, 3).join(", ") + (people.length > 3 ? " & " + (people.length - 3) + " more" : "") : "")
                       + (ccCount ? (people.length ? " · " : "") + "Cc: " + ccCount : "")
            h += '<div class="msg' + (col ? ' col' : '') + (det ? ' det-on' : '') + '" data-i="' + i + '"' + (m.read ? '' : ' data-unread="1"') + '>'
               // The one-line form: who, the start of the text, when.
               + '<div class="row" data-toggle="' + i + '"><div class="av">' + esc(Theme.initials(name)) + '</div>'
               + '<div class="rb"><div class="rt"><span class="rn">' + esc(name) + '</span><span class="chev" title="Open this message">▸</span><span class="grow"></span><span class="rd">' + esc(Theme.fmtListDate(m.date)) + '</span></div>'
               + '<div class="snip">' + esc(m.gist || snippet(m)) + '</div></div></div>'
               + '<div class="full"><div class="hdr"><div class="av">' + esc(Theme.initials(name)) + '</div><div class="who">'
               // Name, the message's buttons and its flag; the date; who it went to, with
               // Details for the people as bubbles, the address and Plain Text.
               + '<div class="top">' + (m.vip ? '★ ' : '') + '<a class="name" title="' + esc((m.fromName ? m.fromName + ' <' + m.fromEmail + '>' : m.fromEmail)) + '" href="almanac:sender/' + i + '">' + esc(name) + '</a>'
               + (boxed ? '<a class="chev" href="#" data-toggle="' + i + '" title="Close this message">▾</a>' : '')
               + '<span class="grow"></span>' + (m.flagged ? '<span style="color:' + Theme.flagColors[m.flagColor || 0] + '" title="' + Theme.flagNames[m.flagColor || 0] + ' flag">⚑</span>' : '')
               + '<span class="acts">' + actions(i) + '</span>'
               + '</div>'
               + '<div class="when">' + esc(Theme.fmtLongDate(m.date)) + '</div>'
               + '<div class="sum"><span class="who2">' + esc(toLine) + '</span><a class="det" href="#" data-det="' + i + '">Details</a></div>'
               + '<div class="details">'
               // To and Cc each on a line of their own, wrapped so every address can be read.
               + (m.to ? '<div class="rcpt"><span class="lbl">To:</span>' + chips(i, m.to) + '</div>' : '')
               + (m.cc ? '<div class="rcpt"><span class="lbl">Cc:</span>' + chips(i, m.cc) + '</div>' : '')
               + (m.fromName && m.fromEmail ? '<div class="addr">' + esc(m.fromEmail) + (m.folder ? ' · ' + esc(m.folder) : '') + '</div>' : '')
               + (m.html ? '<div class="fmtline"><a class="fmt" href="almanac:format/' + i + '">' + (web.formatted ? 'Show Plain Text' : 'Show Formatted') + '</a></div>' : '')
               + '</div>'
               + '</div></div>'
               + '<div class="rule"></div>'
            if (m.partial && !m.html && !m.body) {
                h += '<div class="wait">Downloading this message…</div>'
            } else {
                if (rich && !remote && hasRemote(m.html))
                    h += '<div class="remote"><span class="grow">Remote pictures are hidden to protect your privacy.</span>'
                       + '<a href="almanac:remote/' + i + '">Load Remote Content</a>'
                       + '<a class="more" href="almanac:remote-sender/' + i + '" title="Always load remote content in mail from ' + esc(m.fromEmail) + '">Always from This Sender</a>'
                       + '<a class="more" href="almanac:remote-all/' + i + '" title="Load remote content in every message (Settings › Privacy)">Always</a></div>'
                // (A closed message's text is loaded when it's opened.)
                h += '<div class="' + (paper && Theme.dark ? 'paper' : '') + '"><iframe sandbox="allow-same-origin allow-popups allow-popups-to-escape-sandbox" ' + (col ? 'data-doc' : 'srcdoc') + '="'
                   + esc(emailDoc(m, paper && Theme.dark, remote)) + '"></iframe></div>'
            }
            // Pictures and PDFs right in the message, as in Apple Mail.
            var shown = web.previews[m.id] || []
            for (var p = 0; p < shown.length; p++) {
                var it = shown[p], src = 'data:' + it.mime + ';base64,' + it.data
                var q = i + '/' + encodeURIComponent(it.name)
                h += '<div class="pvbar"><span>' + (it.mime === "application/pdf" ? '📄 ' : '🖼 ') + esc(it.name) + '</span>'
                   + '<span class="split"><a href="almanac:viewtab/' + q + '" title="Open in a new tab">Open</a><a class="more" href="almanac:openmenu/' + q + '" title="Open in a window, print or save it">▾</a></span></div>'
                // (The PDF viewer's own toolbar, with its download and print buttons,
                // left out: Open ▾ has those.)
                // (The PDF viewer's own toolbar, with its download and print buttons,
                // left out: Open ▾ has those.)
                // A PDF: its pages in a box of the page's own that scrolls (the wheel
                // and trackpad scroll it directly, so its scroll bars show then).
                if (it.mime === "application/pdf") {
                    h += '<div class="pdfwrap"><div class="pdfbar"><div class="pdfthumb"></div></div><div class="pdfbox" title="' + esc(it.name) + '">'
                    for (var pg = 0; pg < (it.pages || []).length; pg++) h += '<img class="pdfpage" src="data:image/png;base64,' + it.pages[pg] + '" alt="Page ' + (pg + 1) + '">'
                    if (it.total > (it.pages || []).length)
                        h += '<a class="pdfmore" href="almanac:viewtab/' + q + '">' + (it.total - it.pages.length) + ' more pages: open it to see them all</a>'
                    h += '</div></div>'
                }
                else
                    h += '<a class="inl" href="almanac:att/' + i + '/' + encodeURIComponent(it.name) + '" title="Open ' + esc(it.name) + '"><img src="' + src + '" alt="' + esc(it.name) + '"></a>'

            }
            if (m.attachments && m.attachments.length) {
                h += '<div class="atts">'
                for (var a = 0; a < m.attachments.length; a++) {
                    var n = m.attachments[a]
                    h += '<span class="att"><a class="att" style="padding:0;background:none" href="almanac:att/' + i + '/' + encodeURIComponent(n) + '">📎 <b>' + esc(n) + '</b></a>'
                       + '<a class="save" title="Open, in a new tab or in a new window" href="almanac:openmenu/' + i + '/' + encodeURIComponent(n) + '">▾</a>'
                       + '<a class="save" title="Save As…" href="almanac:save/' + i + '/' + encodeURIComponent(n) + '">⤓</a></span>'
                }
                h += '</div>'
            }
            h += '</div></div>'
        }
        // Each email's frame grows to its full height (and width, for wide ones).
        h += '<script nonce="' + nonce + '">(function(){'
           // Sized without feedback loops (an email sized to its frame made the frame
           // grow, then the email, every frame: the pane flickered): only real changes
           // are written, small ones ignored; measured afresh (the frame shrunk to
           // nothing first, so "100% high" content can't hold it up) only on loading
           // and when the window's width changes; growing on and on stops.
           + 'function fit(f,fresh){try{var d=f.contentDocument;if(!d||!d.documentElement)return;var de=d.documentElement,b=d.body||de;'
           + 'if(fresh){f.style.width="100%";f.__w=0;f.style.height="1px";f.__h=0;f.__gc=0}'
           + 'var cw=f.parentNode?f.parentNode.clientWidth:f.clientWidth,w=Math.max(de.scrollWidth,b.scrollWidth),ww=w>cw+1?w:0;'
           + 'if(ww>(f.__w||0)){f.style.width=ww+"px";f.__w=ww}'
           + 'var hh=Math.max(de.scrollHeight,b.scrollHeight,b.offsetHeight),cur=f.__h||0;'
           + 'if(cur&&Math.abs(hh-cur)<=2)return;'
           + 'var now=Date.now();if(!f.__gt||now-f.__gt>2000){f.__gt=now;f.__gc=0}if(cur&&hh>cur&&++f.__gc>30)return;'
           + 'f.style.height=hh+"px";f.__h=hh}catch(e){}}'
           // Dark mode, as dark-mode browser extensions do it: the engine darkens the
           // email's colours but not its background pictures, so each picture is
           // blended ("multiply") with a dark colour: white turns dark, patterns and
           // colours stay, only darker, and the lightened text on top stays readable.
           + 'var DARK=' + (web.darkify ? 'true' : 'false') + ';'
           + 'function tint(d){if(!DARK||!d||!d.body)return;var win=d.defaultView,els=[d.body].concat([].slice.call(d.body.getElementsByTagName("*")));'
           + 'for(var i=0;i<els.length;i++){var el=els[i],cs=win.getComputedStyle(el),bi=cs.backgroundImage;if(!bi||bi.indexOf("url(")<0)continue;'
           + 'var n=bi.split(/,(?![^(]*\\))/).length,modes=[];for(var k=0;k<n;k++)modes.push("multiply");'
           + 'el.style.setProperty("background-blend-mode",modes.join(","),"important");'
           + 'if(/rgba\\(0, 0, 0, 0\\)|transparent/.test(cs.backgroundColor))el.style.setProperty("background-color","#26262b","important")}}'
           + 'var fr=[].slice.call(document.querySelectorAll("iframe"));'
           // Opening and closing messages, and their details, right here (no reload);
           // Almanac is told so the choice stays when the page is next drawn.
           + 'var told=0;function tell(k,i,v){document.title="state/"+i+"/"+k+"/"+(v?1:0)+"/"+(++told)}'
           + 'function setOpen(el,open){var was=!el.classList.contains("col");if(was===open)return false;el.classList.toggle("col",!open);'
           + 'if(open)[].forEach.call(el.querySelectorAll("iframe[data-doc]"),function(f){f.srcdoc=f.getAttribute("data-doc");f.removeAttribute("data-doc")});return true}'
           + 'document.addEventListener("click",function(e){var t=e.target.closest("[data-toggle]");'
           + 'if(!t&&!e.target.closest("a")){var c=e.target.closest(".msg.col");if(c)t=c.querySelector("[data-toggle]")}'
           + 'if(t){e.preventDefault();var el=document.querySelector(\'.msg[data-i="\'+t.getAttribute("data-toggle")+\'"]\');var open=el.classList.contains("col");'
           + 'setOpen(el,open);tell("open",el.getAttribute("data-i"),open);return}'
           + 'var d=e.target.closest("[data-det]");if(d){e.preventDefault();var m=d.closest(".msg");var on=m.classList.toggle("det-on");d.textContent=on?"Hide Details":"Details";tell("details",m.getAttribute("data-i"),on);return}'
           + 'var a=e.target.closest("[data-all]");if(a){e.preventDefault();var open=a.getAttribute("data-all")==="1",changed=[];'
           + '[].forEach.call(document.querySelectorAll(".msg[data-i]"),function(el){if(setOpen(el,open))changed.push(el.getAttribute("data-i"))});'
           + 'if(changed.length)tell("all",changed[0],open)}});'
           + '[].forEach.call(document.querySelectorAll(".msg.det-on [data-det]"),function(d){d.textContent="Hide Details"});'
           // Scrolled into place once laid out (again as frames fill in), unless the
           // person scrolls first: the first unread message of a conversation (its
           // newest when all are read) at the top, or the same place as before.
           + 'var KEEP=' + web.keepScroll + ',moved=false;'
           + '["wheel","keydown","mousedown","touchstart"].forEach(function(t){addEventListener(t,function(){moved=true},{passive:true,capture:true})});'
           + 'function place(){if(moved)return;if(KEEP>=0){scrollTo(0,KEEP);return}var ms=document.querySelectorAll(".msg[data-i]");if(ms.length<2)return;'
           + 'var t=document.querySelector(".msg[data-unread]")||ms[ms.length-1];scrollTo(0,Math.max(0,t.offsetTop-8))}'
           + 'place();setTimeout(place,250);setTimeout(place,700);setTimeout(place,1500);'
           // Shown as soon as the email's text is there, not when every picture has
           // arrived (a slow or unreachable picture server would keep the whole
           // message hidden until it gave up): sized and tinted as soon as its
           // body exists, and again as pictures come in.
           + 'fr.forEach(function(f){function go(){if(f.__raf)return;f.__raf=requestAnimationFrame(function(){f.__raf=0;fit(f)})}'
           + 'function ready(){var d;try{d=f.contentDocument}catch(e){return false}if(!d||!d.body||f.__ready===d)return !!(d&&d.body);'
           + 'f.__ready=d;tint(d);fit(f,true);'
           // (Not a ResizeObserver on the email's body: watching bodies in other
           // frames made Chromium report changes every frame, a page busy redrawing
           // at 60 per second. A light look twice a second instead, for frames on screen.)
           + '[].slice.call(d.images||[]).forEach(function(im){im.addEventListener("load",go);im.addEventListener("error",go)});return true}'
           + 'var tries=0,iv=setInterval(function(){tries++;if(ready()&&tries>3)clearInterval(iv);else if(tries>100)clearInterval(iv)},50);'
           + 'f.addEventListener("load",function(){ready();fit(f,true);setTimeout(go,150);setTimeout(go,800)});go()});'
           // (A change of height only, e.g. the pane nudged to redraw, needs no fresh measuring.)
           + 'setInterval(function(){fr.forEach(function(f){if(f.offsetParent&&f.__ready)fit(f)})},400);'
           + 'var lastW=innerWidth;window.addEventListener("resize",function(){var fresh=innerWidth!==lastW;lastW=innerWidth;fr.forEach(function(f){if(fresh)fit(f,true);else if(f.__raf===undefined||!f.__raf)fit(f)})});'
           // Scrollbar: shown only while scrolling or while the pointer is over the
           // scrollbar's strip at the right edge, as on macOS.
           // It fades in and out: the engine can't animate scrollbar colours, so the
           // thumb's opacity is stepped a frame at a time (only while fading).
           + 'var de=document.documentElement,hide=0,near=false,scrolling=0,shown=false,alpha=0,goal=0,fade=0;'
           + 'var sheet=document.getElementById("sbshow");'
           + 'function paint(){sheet.textContent=alpha<=0?"":"::-webkit-scrollbar-thumb{background-color:rgba(' + (Theme.dark ? '255,255,255' : '0,0,0') + ','
           + '"+(' + (Theme.dark ? '.38' : '.32') + '*alpha).toFixed(3)+")}::-webkit-scrollbar-thumb:hover{background-color:rgba(' + (Theme.dark ? '255,255,255' : '0,0,0') + ','
           + '"+(' + (Theme.dark ? '.55' : '.5') + '*alpha).toFixed(3)+")}"}'
           + 'function fadeTo(g){goal=g;if(fade)return;var last=performance.now();'
           + 'fade=requestAnimationFrame(function step(now){var d=(now-last)/(goal>alpha?150:300);last=now;'
           + 'alpha=goal>alpha?Math.min(goal,alpha+d):Math.max(goal,alpha-d);paint();fade=alpha!==goal?requestAnimationFrame(step):0})}'
           + 'function update(){clearTimeout(hide);if(near||scrolling){if(!shown){shown=true;fadeTo(1)}}'
           + 'else hide=setTimeout(function(){if(shown&&!near&&!scrolling){shown=false;fadeTo(0)}},500)}'
           // The strip: the scrollbar itself (beyond clientWidth) and a few pixels before it.
           + 'document.addEventListener("mousemove",function(e){var n=de.scrollHeight>innerHeight&&e.clientX>=de.clientWidth-6;if(n!==near){near=n;update()}},{passive:true});'
           + 'de.addEventListener("mouseleave",function(){if(near){near=false;update()}});'
           + 'window.addEventListener("scroll",function(){clearTimeout(scrolling);scrolling=setTimeout(function(){scrolling=0;update()},700);update()},{passive:true});'
           // A PDF's box: its bar placed by how far it's scrolled, shown while it scrolls
           // or the pointer's over the box, gone a second after; dragged or clicked to move.
           + '[].forEach.call(document.querySelectorAll(".pdfwrap"),function(w){var box=w.querySelector(".pdfbox"),bar=w.querySelector(".pdfbar"),th=w.querySelector(".pdfthumb"),t=0,drag=null,inside=false;'
           + 'function place(){var h=box.clientHeight,s=box.scrollHeight;if(s<=h+1){bar.style.display="none";return}bar.style.display="";'
           + 'var bh=bar.clientHeight,tl=Math.max(24,bh*h/s);th.style.height=tl+"px";th.style.top=((bh-tl)*box.scrollTop/(s-h))+"px"}'
           + 'function show(){place();bar.classList.add("on");clearTimeout(t);if(!inside&&!drag)t=setTimeout(function(){bar.classList.remove("on")},1000)}'
           + 'function soon(){clearTimeout(t);t=setTimeout(function(){if(!drag)bar.classList.remove("on")},1000)}'
           + 'box.addEventListener("scroll",function(){place();bar.classList.add("on");soon()},{passive:true});'
           + 'w.addEventListener("mouseenter",function(){inside=true;show()});w.addEventListener("mousemove",function(){inside=true;if(!bar.classList.contains("on"))show()});'
           + 'w.addEventListener("mouseleave",function(){inside=false;soon()});'
           + 'th.addEventListener("mousedown",function(e){drag={y:e.clientY,top:box.scrollTop};bar.classList.add("drag");e.preventDefault();e.stopPropagation()});'
           + 'window.addEventListener("mousemove",function(e){if(!drag)return;var bh=bar.clientHeight,h=box.clientHeight,s=box.scrollHeight;box.scrollTop=drag.top+(e.clientY-drag.y)*(s-h)/Math.max(1,bh-th.offsetHeight)});'
           + 'window.addEventListener("mouseup",function(){if(drag){drag=null;bar.classList.remove("drag");if(!inside)soon()}});'
           + 'bar.addEventListener("mousedown",function(e){if(e.target!==bar)return;var r=bar.getBoundingClientRect();box.scrollTop=(e.clientY-r.top)/r.height*(box.scrollHeight-box.clientHeight);e.preventDefault()});'
           + '[].forEach.call(box.querySelectorAll("img"),function(i){i.addEventListener("load",place)});place()});'
           // Trackpad momentum (Linux and Windows trackpads don't glide on their own):
           // after the fingers lift, the page goes on at their speed and slows down.
           // Another swipe while it's still gliding (same way) adds to the glide
           // instead of stopping it: gv is the glide's speed, boost what's carried over.
           // Trackpad scrolling is done here, not by the engine: the page follows the
           // fingers, glides on when they lift, and a swipe or flick the same way while
           // it glides keeps it going, faster (the engine's own scrolling and flinging,
           // cancelled by the next flick, made every other flick stop dead). Mouse
           // wheels (whole notches) and sideways scrolling stay the engine's.
           // Started with --trace-scroll: every swipe is written to the log ("ALMANAC-SCROLL page …").
           + (web.glide ? 'var v=0,last=0,n=0,end=0,raf=0,gv=0,pos=-1,hist=[],flush=0,TRACE=' + (Qt.application.arguments.indexOf("--trace-scroll") >= 0) + ';'
           + 'function note(x){if(!TRACE)return;hist.push(Math.round(performance.now())+" "+x);if(!flush)flush=setTimeout(function(){flush=0;if(hist.length)console.warn("ALMANAC-SCROLL page "+hist.join(" ; "));hist=[]},1500)}'
           + 'function lim(){return Math.max(0,de.scrollHeight-innerHeight)}'
           + 'function halt(r){if(gv)note("stop "+r);gv=0;cancelAnimationFrame(raf)}'
           + 'function moveTo(y){pos=Math.max(0,Math.min(lim(),y));scrollTo(0,pos)}'
           + 'function wheel(e){if(e.ctrlKey)return;if(e.target&&e.target.closest&&e.target.closest(".pdfbox"))return;var precise=e.wheelDeltaY%120!==0||e.wheelDeltaX%120!==0;'
           + 'if(!e.deltaY||!precise||Math.abs(e.deltaX)>Math.abs(e.deltaY)){if(e.deltaY&&!precise)halt("wheel");return}'
           + 'var now=performance.now(),dt=Math.max(4,Math.min(80,now-last));'
           + 'if(e.cancelable)e.preventDefault();note("w "+e.deltaY.toFixed(1)+(e.cancelable?"":" nc")+(gv?" g"+gv.toFixed(2):"")+" y"+Math.round(scrollY));'
           + 'if(pos<0||Math.abs(scrollY-pos)>3)pos=scrollY;'
           + 'if(gv&&gv*e.deltaY<0)halt("other way");'
           + 'if(now-last>150){v=0;n=0}last=now;n++;v=0.6*v+0.4*(e.deltaY/dt);'
           // Gliding the same way: it goes on, faster; the fingers' movement adds to it.
           + 'if(gv){gv=(gv>0?1:-1)*Math.min(10,Math.abs(gv)+0.5*Math.abs(e.deltaY)/dt)}'
           + 'if(e.cancelable)moveTo(pos+e.deltaY);clearTimeout(end);end=setTimeout(release,70)}'
           + 'function release(){var go=n?v:0;if(gv&&gv*go>=0){note("on "+gv.toFixed(2));return}'
           + 'if(Math.abs(go)<0.06){note("no glide "+go.toFixed(2));return}glide(go)}'
           + 'function glide(go){var y=scrollY;if((go<0&&y<=0)||(go>0&&y>=lim()))return;gv=go;pos=y;note("glide "+go.toFixed(2));var p=performance.now();'
           + 'raf=requestAnimationFrame(function step(t){var dt=Math.min(32,t-p);p=t;'
           + 'if(Math.abs(scrollY-pos)>3)pos=scrollY;moveTo(pos+gv*dt);gv*=Math.pow(0.998,dt);'
           + 'if(Math.abs(gv)<=0.02){gv=0;note("slowed")}else if((gv<0&&pos<=0)||(gv>0&&pos>=lim())){gv=0;note("edge")}else raf=requestAnimationFrame(step)})}'
           + 'window.addEventListener("wheel",wheel,{passive:false,capture:true});'
           + 'function click(){halt("click")}window.addEventListener("mousedown",click);'
           // Each email's frame too (its document can arrive after this runs, or be replaced).
           + 'function hook(f){try{var w=f.contentWindow;if(w&&w.document&&!w.document.__glide){w.document.__glide=1;w.addEventListener("wheel",wheel,{passive:false,capture:true});w.addEventListener("mousedown",click)}}catch(e){}}'
           + 'fr.forEach(function(f){hook(f);f.addEventListener("load",function(){hook(f)})});var hooks=0,hi=setInterval(function(){fr.forEach(hook);if(++hooks>40)clearInterval(hi)},100);' : '')
           + ''
           + '})()</script></body></html>'
        return h
    }

    // Reload only when what's shown actually changed: the app refreshes often
    // (mail arriving in the background) and a reload would make the pane flicker.
    property string shownKey: ""
    function render() {
        // (Called later, after the page's tab was already closed: nothing to show.)
        if (typeof web.page !== "function") return
        var pv = {}, pvKey = []
        for (var i = 0; i < web.messages.length; i++) {
            var m = web.messages[i]
            if (!m.attachments || !m.attachments.length || !web.previewsFor) continue
            pv[m.id] = web.previewsOf(m)
            pvKey.push(m.id + ":" + pv[m.id].map(function(p) { return p.name + p.data.length + "/" + (p.pages || []).length }).join(","))
        }
        var key = JSON.stringify([web.messages, web.subject, web.formatted, web.darkMessages, web.darkify, web.loadRemoteAlways,
                                  web.remoteLoaded, web.remoteSenders, Theme.dark, pvKey])
        if (key === shownKey) return
        paneLog("page reloaded (" + web.messages.length + " message(s))")
        shownKey = key
        var conv = web.messages.length ? web.messages[0].id + "|" + web.subject : ""
        if (conv !== "" && conv === web.shownConv) {
            // The same conversation again: keep the place it's scrolled to.
            web.runJavaScript("window.scrollY", function(y) { web.keepScroll = y || 0; web.loadPage(pv) })
            return
        }
        web.shownConv = conv
        web.keepScroll = -1
        loadPage(pv)
    }
    function loadPage(pv) {
        if (typeof web.page !== "function") return
        web.previews = pv
        var doc = page()
        var file = doc.length > 1800000 && web.store ? web.store.pageFile(web.slot, doc) : ""
        if (file) url = file + "?v=" + (++fileLoads)
        else loadHtml(doc, "https://almanac.invalid/")
    }
}
