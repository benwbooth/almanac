import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtWebEngine

// A picture or PDF from a message, on its own (in a tab of the Mail view or
// a window): shown by the browser engine (its PDF viewer, or the picture,
// zoomable), with buttons to open it in its usual app or save it.
Item {
    id: v
    property var store
    // The Mail view (saving, and moving between tab and window).
    property var view
    property var messageId: -1
    property string name: ""
    // Alone in a window of its own (no "Open in a Separate Window" then).
    property bool inWindow: false
    signal closeRequested()
    // Into a window of its own (the tab it's in moves there).
    signal popOutRequested()
    property string fileUrl: ""
    // Not on this computer yet: fetched from the mail server, shown when it arrives.
    function refresh() {
        fileUrl = store.attachmentUrl(messageId, name)
        if (fileUrl === "" && messageId >= 0 && name !== "") store.fetchAttachments(messageId)
    }
    onMessageIdChanged: Qt.callLater(refresh)
    onNameChanged: Qt.callLater(refresh)
    Connections {
        target: v.store
        function onAttachmentsReadyChanged() {
            var r = JSON.parse(v.store.attachmentsReady || "{}")
            if (r.id === v.messageId && v.fileUrl === "") { v.fileUrl = v.store.attachmentUrl(v.messageId, v.name); v.pictureCheck++ }
        }
    }
    property int pictureCheck: 0
    // Pictures come with their real type (a name's ending can be wrong), shown
    // fitted on a page of their own; PDFs open from the file in the PDF viewer.
    readonly property var picture: {
        pictureCheck
        var all = JSON.parse(store.attachmentPreviews(messageId))
        for (var i = 0; i < all.length; i++) if (all[i].name === name && all[i].data && all[i].mime !== "application/pdf") return all[i]
        return null
    }
    function show() {
        if (!picture) { page.url = fileUrl; return }
        page.loadHtml('<!doctype html><html><body style="margin:0;background:' + Theme.window + ';display:flex;align-items:center;justify-content:center;min-height:100vh">'
                      + '<img style="max-width:100%;max-height:100vh;object-fit:contain" src="data:' + picture.mime + ';base64,' + picture.data + '"></body></html>')
    }
    onPictureChanged: show()
    onFileUrlChanged: show()
    Component.onCompleted: { refresh(); show() }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 40
            color: Theme.window
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 12; anchors.rightMargin: 10
                spacing: 6
                Icon { name: "paperclip"; width: 16; height: 16; color: Theme.secondary }
                Text { Layout.fillWidth: true; text: v.name; color: Theme.text; font.weight: Font.DemiBold; elide: Text.ElideMiddle }
                // (A PDF's own viewer below has zoom, print and download: not here again.)
                ToolButtonX { visible: !!v.picture; text: "−"; tip: "Zoom Out"; onClicked: page.zoomFactor = Math.max(0.25, page.zoomFactor / 1.2) }
                ToolButtonX { visible: !!v.picture; text: Math.round(page.zoomFactor * 100) + "%"; tip: "Actual Size"; onClicked: page.zoomFactor = 1 }
                ToolButtonX { visible: !!v.picture; text: "+"; tip: "Zoom In"; onClicked: page.zoomFactor = Math.min(5, page.zoomFactor * 1.2) }
                ToolButtonX { text: "Open"; tip: "Open in its usual app"; onClicked: v.store.openAttachment(v.messageId, v.name, false) }
                ToolButtonX {
                    visible: !!v.picture
                    text: "Save…"; tip: "Save a copy"
                    onClicked: { var m = JSON.parse(v.store.message(v.messageId)); if (m && m.id !== undefined) v.view.saveAttachment(m, v.name) }
                }
                ToolButtonX { visible: !v.inWindow; icon: "popOut"; tip: "Open in a Separate Window"; onClicked: v.popOutRequested() }
            }
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.borderSoft }
        }
        WebEngineView {
            id: page
            Layout.fillWidth: true
            Layout.fillHeight: true
            backgroundColor: Theme.window
            settings.pluginsEnabled: true
            settings.pdfViewerEnabled: true
            settings.javascriptCanOpenWindows: false
            settings.focusOnNavigationEnabled: false
            // Links in a PDF open in the browser.
            onNewWindowRequested: function(request) { Qt.openUrlExternally(request.requestedUrl) }
            // The PDF viewer's own download button: the Save dialog.
            Connections {
                target: page.profile
                function onDownloadRequested(download) {
                    console.warn("ALMANAC-PDF download asked (tab): " + download.suggestedFileName + " view " + (download.view === page ? "this page" : download.view ? "another" : "none"))
                    if (Theme.handledDownload === download.id) return
                    if (download.view !== page && Theme.pointerPage !== page) return
                    Theme.handledDownload = download.id
                    download.cancel()
                    var m = JSON.parse(v.store.message(v.messageId))
                    if (m && m.id !== undefined) v.view.saveAttachment(m, v.name)
                }
            }
            // The PDF viewer's print button: the PDF through the print dialog.
            HoverHandler { onHoveredChanged: if (hovered) Theme.pointerPage = page }
            function printPdf(how) {
                console.warn("ALMANAC-PDF print asked (tab, " + how + ")")
                var now = Date.now()
                if (v.picture || now - Theme.printAskedAt < 1500) return
                Theme.printAskedAt = now
                v.store.printAttachment(v.messageId, v.name)
            }
            onPrintRequested: printPdf("page")
            onPrintRequestedByFrame: function(frame) { printPdf("frame") }
            onNavigationRequested: function(request) {
                if (request.navigationType === WebEngineNavigationRequest.LinkClickedNavigation) { request.reject(); Qt.openUrlExternally(request.url) }
                else request.accept()
            }
        }
    }
    Text {
        visible: v.fileUrl === "" && !v.picture
        anchors.centerIn: parent
        text: "Getting " + v.name + " from the mail server…"
        color: Theme.secondary
    }
}
