import QtQuick
import QtQuick.Controls.Basic
// The system's own menus (desktop style on Linux, native on macOS and Windows)
import Qt.labs.platform as P
import QtQuick.Layouts
import QtQuick.Dialogs

// Contacts: groups, an A–Z list and the selected person's card.
Item {
    id: page
    property var store
    property bool sidebarShown: true
    property string query: ""
    signal focusSearch()
    signal composeRequested(var payload)
    property string group: ""
    property var selectedId: -1
    // What's on screen, saved when Almanac closes and put back when it opens.
    property var paneSizes: ({})
    signal panesChanged()
    function notePanes() {
        if (!visible || width <= 0) return
        var p = Object.assign({}, paneSizes)
        if (sidebarShown && sidebarPane.width > 60) p.sideW = sidebarPane.width
        if (listPane.width > 120) p.listW = listPane.width
        paneSizes = p
        panesChanged()
    }
    Connections { target: sidebarPane; function onWidthChanged() { page.notePanes() } }
    Connections { target: listPane; function onWidthChanged() { page.notePanes() } }
    function saveState() {
        notePanes()
        return { sidebar: sidebarShown, group: group, selectedId: selectedId, sideW: paneSizes.sideW, listW: paneSizes.listW }
    }
    function restoreState(s) {
        if (!s) return
        paneSizes = { sideW: s.sideW, listW: s.listW }
        sidebarShown = s.sidebar !== false
        if (s.group !== undefined) group = s.group
        if (s.selectedId !== undefined) selectedId = s.selectedId
        var W = (page.Window.window ? page.Window.window.width : 0) || 1280
        if (s.sideW > 60) sidebarPane.SplitView.preferredWidth = Math.min(s.sideW, W * 0.4)
        if (s.listW > 120) listPane.SplitView.preferredWidth = Math.min(s.listW, Math.max(220, W - Math.min(s.sideW || 0, W * 0.4) - 320))
    }
    property bool editing: false
    property var draft: null
    readonly property string groupName: {
        for (var i = 0; i < groups.length; i++) if (groups[i].id === group) return groups[i].name
        return "All Contacts"
    }

    // Import / export cards as vCard (.vcf) files.
    // No cards at all yet.
    Column {
        z: 3
        visible: page.list.length === 0 && page.group === "" && page.query === "" && !page.editing
        anchors.centerIn: parent
        spacing: 12
        Icon { anchors.horizontalCenter: parent.horizontalCenter; name: "contacts"; width: 54; height: 54; color: Theme.tertiary }
        Text { anchors.horizontalCenter: parent.horizontalCenter; text: "No contacts yet"; font.pixelSize: 22; font.weight: Font.Bold; color: Theme.text }
        Text { anchors.horizontalCenter: parent.horizontalCenter; text: "Add a card, import a vCard file, or add an account with contacts."; color: Theme.secondary }
        Row {
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: 8
            ToolButtonX { text: "New Contact"; primary: true; onClicked: page.newContact() }
            ToolButtonX { text: "Import vCard…"; onClicked: page.importCards() }
        }
    }

    SmartGroupDialog { id: smartGroupDialog; store: page.store; onSaved: function(id) { page.group = id } }
    DuplicatesDialog { id: duplicatesDialog; store: page.store }
    Component {
        id: photoDialogComp
        FileDialog {
            title: "Choose a Photo"
            nameFilters: ["Pictures (*.jpg *.jpeg *.png)"]
            onAccepted: {
                store.setContactPhoto(page.draft.id, selectedFile.toString())
                var c = JSON.parse(store.contact(page.draft.id))
                if (c) page.draft.photo = c.photo
                page.refreshDraft()
                destroy()
            }
            onRejected: destroy()
        }
    }

    // Made only when used, so starting the app never waits on the system's file picker.
    Component {
        id: importDialogComp
        FileDialog {
            title: "Import Cards"
            nameFilters: ["vCard files (*.vcf *.vcard)", "All files (*)"]
            onAccepted: { store.importVcards(selectedFile.toString(), page.group); destroy() }
            onRejected: destroy()
        }
    }
    Component {
        id: exportDialogComp
        FileDialog {
            title: "Export Cards"
            fileMode: FileDialog.SaveFile
            defaultSuffix: "vcf"
            nameFilters: ["vCard files (*.vcf)"]
            onAccepted: { store.exportVcards(page.group, selectedFile.toString()); destroy() }
            onRejected: destroy()
        }
    }
    function newContact() { selectedId = -1; startEdit(null) }
    function newGroup() { groupPopup.renameId = ""; groupPopup.open() }
    function newSmartGroup() { smartGroupDialog.openFor(null) }
    function findDuplicates() { duplicatesDialog.open() }
    function goToMyCard() { group = ""; var me = list.filter(function(c) { return c.isMe }); if (me.length) selectedId = me[0].id; else store.statusText = "No card is set as My Card yet." }
    function printCards(onlyCurrent) {
        var esc = function(t) { return (t || "").replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/\n/g, "<br>") }
        var ids = onlyCurrent ? [selectedId] : list.map(function(c) { return c.id })
        var h = ""
        for (var i = 0; i < ids.length && i < 500; i++) {
            var c = JSON.parse(store.contact(ids[i]))
            if (!c) continue
            h += "<h3>" + esc(c.name) + "</h3><p>"
            if (c.company || c.job_title) h += esc([c.job_title, c.company].filter(function(x) { return x }).join(", ")) + "<br>"
            c.phones.forEach(function(x) { h += esc(x.label) + ": " + esc(x.value) + "<br>" })
            c.emails.forEach(function(x) { h += esc(x.label) + ": " + esc(x.value) + "<br>" })
            if (c.address) h += esc(c.address) + "<br>"
            if (c.birthday) h += "birthday: " + esc(c.birthday)
            h += "</p><hr>"
        }
        store.printHtml(onlyCurrent && current ? current.name : page.groupName, h)
    }
    function importCards() { importDialogComp.createObject(page).open() }
    function exportCards() { exportDialogComp.createObject(page).open() }

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
                Text { text: page.groupName; font.pixelSize: 14; font.weight: Font.Bold; color: Theme.text; elide: Text.ElideRight; width: parent.width }
                Text { text: page.list.length + (page.list.length === 1 ? " card" : " cards"); font.pixelSize: 11; color: Theme.secondary }
            }
            ToolButtonX { priority: 2; icon: "plus"; tip: "New Contact (Ctrl+N)"; onClicked: { page.selectedId = -1; page.startEdit(null) } }
            Item { Layout.fillWidth: true }
            Item { width: 6 }
            SearchBox {
                id: search
                Layout.preferredWidth: collapsed ? 30 : 220
                Layout.maximumWidth: collapsed ? 30 : 220
                Layout.fillWidth: true
                Layout.minimumWidth: collapsed ? 30 : 84
                readonly property real minW: 84
                text: page.query
                onTextChanged: page.query = text
                Connections { target: page; function onFocusSearch() { search.focusField() } }
            }
        }
    }

    property var groups: { store.revision; return JSON.parse(store.contactGroups()) }
    property var list: { store.revision; return JSON.parse(store.contacts(group, query)) }
    property var current: { store.revision; return selectedId >= 0 ? JSON.parse(store.contact(selectedId)) : null }

    onListChanged: {
        var idx = -1
        for (var i = 0; i < list.length; i++) if (list[i].id === selectedId) idx = i
        if (idx < 0 && !editing) selectedId = list.length ? list[0].id : -1
    }

    function startEdit(c) {
        var d = JSON.parse(JSON.stringify(c || { id: 0, first: "", last: "", company: "", job_title: "", emails: [], phones: [], address: "", birthday: "", notes: "", groups: [], nickname: "", urls: [], photo: "" }))
        if (!d.urls) d.urls = []
        if (!c && page.group !== "") d.groups = [page.group]
        if (!d.emails.length) d.emails.push({ label: "home", value: "" })
        if (!d.phones.length) d.phones.push({ label: "mobile", value: "" })
        draft = d
        editing = true
    }
    function finishEdit() {
        if (!draft) { editing = false; return }
        var filled = function(list) { return (list || []).some(function(x) { return (x.value || "").trim() !== "" }) }
        var blank = !(draft.first || "").trim() && !(draft.last || "").trim() && !(draft.company || "").trim()
                    && !filled(draft.emails) && !filled(draft.phones) && !draft.id
        if (blank) { cancelEdit(); return }
        var id = store.saveContact(JSON.stringify(draft))
        editing = false
        draft = null
        selectedId = id
    }
    function cancelEdit() { editing = false; draft = null }
    function refreshDraft() { draft = JSON.parse(JSON.stringify(draft)) }

    Shortcut { sequence: "Ctrl+N"; enabled: page.visible; onActivated: { page.selectedId = -1; page.startEdit(null) } }
    Shortcut { sequence: "Ctrl+F"; enabled: page.visible; onActivated: page.focusSearch() }
    Confirm { id: confirm }
    // "Last, First" from Settings
    property bool lastFirst: { store.revision; return !!JSON.parse(store.settings()).contacts_last_first }

    Popup {
        id: groupPopup
        property string renameId: ""
        anchors.centerIn: Overlay.overlay
        modal: true
        padding: 18
        background: Rectangle { radius: 12; color: Theme.popup; border.color: Theme.border }
        contentItem: ColumnLayout {
            spacing: 10
            Text { text: groupPopup.renameId ? "Rename Group" : "New Group"; font.pixelSize: 15; font.weight: Font.Bold; color: Theme.text }
            FieldX { id: groupName; Layout.preferredWidth: 260; placeholderText: "Group name"; onAccepted: addBtn.clicked() }
            RowLayout {
                Item { Layout.fillWidth: true }
                ToolButtonX { text: "Cancel"; onClicked: groupPopup.close() }
                ToolButtonX {
                    id: addBtn
                    text: groupPopup.renameId ? "Rename" : "Add"; primary: true
                    onClicked: {
                        if (groupName.text.trim()) {
                            if (groupPopup.renameId) store.renameGroup(groupPopup.renameId, groupName.text.trim())
                            else page.group = store.addGroup(groupName.text.trim())
                        }
                        groupPopup.close()
                    }
                }
            }
        }
        onOpened: {
            var g = page.groups.filter(function(x) { return x.id === groupPopup.renameId })
            groupName.text = g.length ? g[0].name : ""
            groupName.forceActiveFocus()
        }
    }

    // Grips over the pane dividers (the lines are too thin to catch).
    PaneGrip { pane: sidebarPane }
    PaneGrip { pane: listPane }

    // Panes with handles to drag their widths.
    SplitView {
        anchors.fill: parent
        orientation: Qt.Horizontal
        handle: Rectangle {
            implicitWidth: 1
            color: SplitHandle.pressed || SplitHandle.hovered ? Theme.accent : Theme.border
        }

        // ------------------------------------------------------------ groups
        Rectangle {
            id: sidebarPane
            visible: page.sidebarShown
            SplitView.preferredWidth: 220
            SplitView.minimumWidth: 150
            gradient: Theme.sidebarGradient
            Flickable {
                id: scroller2
                // Trackpad scrolling glides on (momentum).
                Momentum { flick: scroller2 }
                anchors.fill: parent
                anchors.margins: 10
                anchors.topMargin: 14
                anchors.bottomMargin: 44 + activity.height
                contentHeight: gcol.implicitHeight
                clip: true
                Column {
                    id: gcol
                    width: parent.width
                    spacing: 1
                    Text { text: "Contacts"; color: Theme.tertiary; font.pixelSize: 11; font.weight: Font.DemiBold; leftPadding: 8; bottomPadding: 2 }
                    Repeater {
                        model: page.groups
                        delegate: SidebarItem {
                            required property var modelData
                            required property int index
                            icon: index === 0 ? "contacts" : (modelData.smart ? "gear" : "folder")
                            label: modelData.name
                            badge: modelData.count
                            selected: page.group === modelData.id
                            onClicked: { page.group = modelData.id; page.selectedId = -1 }
                            onRightClicked: if (index > 0) { groupMenu.target = modelData; groupMenu.open() }
                            onDoubleClicked: if (modelData.smart) smartGroupDialog.openFor(modelData)
                        }
                    }
                }
            }
            // What's going on (syncing contacts…), at the bottom.
            ActivityPanel {
                id: activity
                store: page.store
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
            }
            ToolButtonX {
                anchors.left: parent.left
                anchors.bottom: activity.top
                anchors.margins: 10
                icon: "plus"; text: "New Group"
                onClicked: { groupPopup.renameId = ""; groupPopup.open() }
            }
            P.Menu {
                id: groupMenu
                property var target: ({})
                P.MenuItem { icon.name: "document-edit"; visible: !!groupMenu.target.smart; text: "Edit Smart Group…"; onTriggered: smartGroupDialog.openFor(groupMenu.target) }
                P.MenuItem { icon.name: "edit-rename"; visible: !groupMenu.target.smart; text: "Rename Group…"; onTriggered: { groupPopup.renameId = groupMenu.target.id; groupPopup.open() } }
                P.MenuItem {
                    icon.name: "user-group-delete"
                    text: "Delete Group"
                    onTriggered: {
                        var g = groupMenu.target
                        confirm.ask("Delete the group “" + g.name + "”?", g.smart ? "The cards in it stay." : "The cards in it stay in All Contacts.", "Delete Group", function() {
                            if (g.smart) store.deleteSmartGroup(g.id)
                            else store.deleteGroup(g.id)
                            if (page.group === g.id) page.group = ""
                        })
                    }
                }
            }
        }

        // --------------------------------------------------------- the list
        ColumnLayout {
            id: listPane
            SplitView.preferredWidth: 280
            SplitView.minimumWidth: 220
            spacing: 0
            ListView {
                // Trackpad scrolling glides on (momentum).
                Momentum { flick: contactList }
                id: contactList
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: page.list
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar {}
                // Up and Down move through the cards.
                focus: true
                keyNavigationEnabled: true
                currentIndex: { for (var i = 0; i < page.list.length; i++) if (page.list[i].id === page.selectedId) return i; return -1 }
                Keys.onUpPressed: if (currentIndex > 0 && !page.editing) page.selectedId = page.list[currentIndex - 1].id
                Keys.onDownPressed: if (currentIndex < page.list.length - 1 && !page.editing) page.selectedId = page.list[currentIndex + 1].id
                // Page Up / Down a screenful of cards; Home / End the first and last.
                Keys.onPressed: function(ev) {
                    if (page.editing || !page.list.length) return
                    var step = Math.max(1, Math.floor(height / 44)), i = -1
                    if (ev.key === Qt.Key_Home) i = 0
                    else if (ev.key === Qt.Key_End) i = page.list.length - 1
                    else if (ev.key === Qt.Key_PageDown) i = Math.min(page.list.length - 1, Math.max(0, currentIndex) + step)
                    else if (ev.key === Qt.Key_PageUp) i = Math.max(0, currentIndex - step)
                    if (i < 0) return
                    page.selectedId = page.list[i].id
                    positionViewAtIndex(i, ListView.Contain)
                    ev.accepted = true
                }
                section.property: "section"
                section.criteria: ViewSection.FullString
                section.delegate: Rectangle {
                    required property string section
                    width: ListView.view.width
                    height: 24
                    color: Theme.window
                    Text { x: 16; anchors.verticalCenter: parent.verticalCenter; text: section; color: Theme.secondary; font.pixelSize: 12; font.weight: Font.Bold }
                    Rectangle { anchors.bottom: parent.bottom; x: 16; width: parent.width - 16; height: 1; color: Theme.borderSoft }
                }
                delegate: Rectangle {
                    required property var modelData
                    property bool sel: modelData.id === page.selectedId
                    width: ListView.view.width
                    height: 30
                    color: sel ? Theme.accentFill : Theme.window
                    Text {
                        x: 16
                        width: parent.width - 24
                        anchors.verticalCenter: parent.verticalCenter
                        textFormat: Text.StyledText
                        elide: Text.ElideRight
                        color: parent.sel ? "white" : Theme.text
                        font.pixelSize: Theme.base
                        text: {
                            var esc = function(s) { return (s || "").replace(/&/g, "&amp;").replace(/</g, "&lt;") }
                            if (modelData.last && page.lastFirst) return "<b>" + esc(modelData.last) + "</b>" + (modelData.first ? ", " + esc(modelData.first) : "")
                            if (modelData.last) return esc(modelData.first) + " <b>" + esc(modelData.last) + "</b>"
                            return "<b>" + esc(modelData.name) + "</b>"
                        }
                        // Cut short: the whole name when pointed at.
                        HoverHandler { id: nameHover }
                        ToolTip.visible: nameHover.hovered && truncated
                        ToolTip.delay: 500
                        ToolTip.text: modelData.name || ((modelData.first || "") + " " + (modelData.last || "")).trim()
                    }
                    MouseArea {
                        anchors.fill: parent
                        onClicked: { if (page.editing) page.finishEdit(); page.selectedId = modelData.id; contactList.forceActiveFocus() }
                    }
                }
                Text {
                    anchors.centerIn: parent
                    visible: page.list.length === 0
                    text: page.query ? "No Results" : "No Contacts"
                    color: Theme.tertiary; font.pixelSize: 16
                }
            }
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 26
                color: Theme.toolbar
                Text { anchors.centerIn: parent; text: page.list.length + (page.list.length === 1 ? " card" : " cards"); color: Theme.secondary; font.pixelSize: 11 }
                Rectangle { width: parent.width; height: 1; color: Theme.border }
            }
        }

        // ---------------------------------------------------------- the card
        Rectangle {
            SplitView.fillWidth: true
            color: Theme.window

            Text {
                anchors.centerIn: parent
                visible: !page.current && !page.editing
                text: "No Card Selected"
                color: Theme.tertiary; font.pixelSize: 20
            }

            // Read-only card
            Flickable {
                id: scroller3
                // Trackpad scrolling glides on (momentum).
                Momentum { flick: scroller3 }
                anchors.fill: parent
                anchors.bottomMargin: 50
                visible: page.current !== null && !page.editing
                contentHeight: card.implicitHeight + 40
                clip: true
                ColumnLayout {
                    id: card
                    x: 40; y: 32
                    width: parent.width - 80
                    spacing: 14
                    RowLayout {
                        spacing: 18
                        Avatar { name: page.current ? page.current.name : ""; photo: page.current ? (page.current.photo || "") : ""; size: 88 }
                        ColumnLayout {
                            spacing: 2
                            Text { text: page.current ? page.current.name : ""; font.pixelSize: 26; font.weight: Font.Bold; color: Theme.text }
                            Text { visible: !!(page.current && page.current.nickname); text: page.current ? "“" + (page.current.nickname || "") + "”" : ""; font.pixelSize: 14; color: Theme.secondary }
                            Text { visible: !!(page.current && page.current.isMe); text: "My Card"; font.pixelSize: 12; color: Theme.accent }
                            Text {
                                visible: text !== ""
                                text: page.current ? [page.current.job_title, page.current.company].filter(function(s) { return s }).join(" — ") : ""
                                font.pixelSize: 15; color: Theme.secondary
                            }
                            // Quick actions under the name: message, call, mail, share.
                            Row {
                                spacing: 10
                                topPadding: 8
                                component Action: Column {
                                    property string icon: ""
                                    property string label: ""
                                    property bool on: true
                                    signal hit()
                                    spacing: 3
                                    opacity: on ? 1 : 0.35
                                    Rectangle {
                                        width: 46; height: 34; radius: 8
                                        color: actArea.containsMouse && parent.on ? Theme.accentSoft : Theme.segment
                                        anchors.horizontalCenter: parent.horizontalCenter
                                        Icon { anchors.centerIn: parent; name: parent.parent.icon; width: 18; height: 18; color: Theme.accent }
                                        MouseArea { id: actArea; anchors.fill: parent; hoverEnabled: true; enabled: parent.parent.on; cursorShape: Qt.PointingHandCursor; onClicked: parent.parent.hit() }
                                    }
                                    Text { anchors.horizontalCenter: parent.horizontalCenter; text: parent.label; font.pixelSize: 11; color: Theme.accent }
                                }
                                // Text message to their phone (the system's messaging app).
                                Action {
                                    icon: "chat"; label: "message"
                                    on: !!(page.current && page.current.phones.length)
                                    onHit: Qt.openUrlExternally("sms:" + page.current.phones[0].value.replace(/[^0-9+]/g, ""))
                                }
                                Action {
                                    icon: "phone"; label: "call"
                                    on: !!(page.current && page.current.phones.length)
                                    onHit: Qt.openUrlExternally("tel:" + page.current.phones[0].value.replace(/[^0-9+]/g, ""))
                                }
                                Action {
                                    icon: "mail"; label: "mail"
                                    on: !!(page.current && page.current.emails.length)
                                    onHit: page.composeRequested({ to: page.current.name + " <" + page.current.emails[0].value + ">" })
                                }
                                Action {
                                    icon: "export"; label: "share"
                                    on: !!page.current
                                    onHit: {
                                        var f = store.cardFile(page.current.id)
                                        if (f) page.composeRequested({ subject: page.current.name, body: "", attachments: [f] })
                                    }
                                }
                            }
                        }
                    }
                    Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.borderSoft }
                    GridLayout {
                        columns: 2
                        columnSpacing: 14
                        rowSpacing: 10
                        Repeater {
                            model: {
                                var c = page.current
                                if (!c) return []
                                var rows = []
                                c.phones.forEach(function(p) { rows.push({ k: p.label, v: p.value }) })
                                c.emails.forEach(function(e) { rows.push({ k: e.label, v: e.value, mail: true }) })
                                ;(c.urls || []).forEach(function(u) { rows.push({ k: u.label, v: u.value, link: true }) })
                                if (c.address) rows.push({ k: "address", v: c.address })
                                if (c.birthday) rows.push({ k: "birthday", v: new Date(Theme.parseDT(c.birthday)).toLocaleDateString(Qt.locale(), Locale.LongFormat) })
                                if (c.groupNames && c.groupNames.length) rows.push({ k: "groups", v: c.groupNames.join(", ") })
                                if (c.notes) rows.push({ k: "note", v: c.notes })
                                var out = []
                                rows.forEach(function(r) { out.push({ t: "k", s: r.k }); out.push({ t: "v", s: r.v, mail: !!r.mail, link: !!r.link }) })
                                return out
                            }
                            delegate: Text {
                                required property var modelData
                                Layout.alignment: Qt.AlignTop
                                Layout.preferredWidth: modelData.t === "k" ? 90 : -1
                                Layout.fillWidth: modelData.t === "v"
                                horizontalAlignment: modelData.t === "k" ? Text.AlignRight : Text.AlignLeft
                                text: modelData.s
                                color: modelData.t === "k" ? Theme.secondary : (modelData.mail || modelData.link ? Theme.accent : Theme.text)
                                font.pixelSize: Theme.base + (modelData.t === "k" ? 0 : 1)
                                wrapMode: Text.WordWrap
                                MouseArea {
                                    enabled: !!(modelData.mail || modelData.link)
                                    anchors.fill: parent
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: modelData.mail ? page.composeRequested({ to: page.current.name + " <" + modelData.s + ">" })
                                                              : Qt.openUrlExternally(modelData.s.indexOf("://") > 0 ? modelData.s : "https://" + modelData.s)
                                }
                            }
                        }
                    }
                }
            }

            // Editing form
            Flickable {
                id: scroller4
                // Trackpad scrolling glides on (momentum).
                Momentum { flick: scroller4 }
                anchors.fill: parent
                anchors.bottomMargin: 50
                visible: page.editing && page.draft !== null
                contentHeight: form.implicitHeight + 40
                clip: true
                ColumnLayout {
                    id: form
                    x: 40; y: 28
                    width: Math.min(parent.width - 80, 560)
                    spacing: 10
                    RowLayout {
                        spacing: 18
                        ColumnLayout {
                            Layout.alignment: Qt.AlignTop
                            Avatar { name: page.draft ? (page.draft.first + " " + page.draft.last) : ""; photo: page.draft ? (page.draft.photo || "") : ""; size: 72 }
                            Text {
                                visible: page.draft && page.draft.id > 0
                                text: "Photo…"; color: Theme.accent; font.pixelSize: 12
                                MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: photoDialogComp.createObject(page).open() }
                            }
                        }
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 6
                            FieldX { Layout.fillWidth: true; placeholderText: "First name"; text: page.draft ? page.draft.first : ""; onTextEdited: page.draft.first = text; font.pixelSize: 16 }
                            FieldX { Layout.fillWidth: true; placeholderText: "Last name"; text: page.draft ? page.draft.last : ""; onTextEdited: page.draft.last = text; font.pixelSize: 16 }
                            FieldX { Layout.fillWidth: true; placeholderText: "Job title"; text: page.draft ? page.draft.job_title : ""; onTextEdited: page.draft.job_title = text }
                            FieldX { Layout.fillWidth: true; placeholderText: "Company"; text: page.draft ? page.draft.company : ""; onTextEdited: page.draft.company = text }
                            FieldX { Layout.fillWidth: true; placeholderText: "Nickname"; text: page.draft ? (page.draft.nickname || "") : ""; onTextEdited: page.draft.nickname = text }
                        }
                    }
                    Text { text: "Phone"; color: Theme.secondary; font.weight: Font.DemiBold; topPadding: 8 }
                    Repeater {
                        model: page.draft ? page.draft.phones : []
                        delegate: RowLayout {
                            required property var modelData
                            required property int index
                            Layout.fillWidth: true
                            ComboX {
                                Layout.preferredWidth: 110
                                editable: true
                                model: ["mobile", "home", "work", "main", "other"]
                                Component.onCompleted: { var i = find(modelData.label); if (i >= 0) currentIndex = i; else editText = modelData.label }
                                onEditTextChanged: if (page.draft && page.draft.phones[index]) page.draft.phones[index].label = editText
                            }
                            FieldX { Layout.fillWidth: true; placeholderText: "Phone"; text: modelData.value; onTextEdited: page.draft.phones[index].value = text }
                            ToolButtonX { icon: "minus"; onClicked: { page.draft.phones.splice(index, 1); page.refreshDraft() } }
                        }
                    }
                    ToolButtonX { icon: "plus"; text: "add phone"; onClicked: { page.draft.phones.push({ label: "mobile", value: "" }); page.refreshDraft() } }
                    Text { text: "Email"; color: Theme.secondary; font.weight: Font.DemiBold; topPadding: 8 }
                    Repeater {
                        model: page.draft ? page.draft.emails : []
                        delegate: RowLayout {
                            required property var modelData
                            required property int index
                            Layout.fillWidth: true
                            ComboX {
                                Layout.preferredWidth: 110
                                editable: true
                                model: ["home", "work", "other"]
                                Component.onCompleted: { var i = find(modelData.label); if (i >= 0) currentIndex = i; else editText = modelData.label }
                                onEditTextChanged: if (page.draft && page.draft.emails[index]) page.draft.emails[index].label = editText
                            }
                            FieldX { Layout.fillWidth: true; placeholderText: "Email"; text: modelData.value; onTextEdited: page.draft.emails[index].value = text }
                            ToolButtonX { icon: "minus"; onClicked: { page.draft.emails.splice(index, 1); page.refreshDraft() } }
                        }
                    }
                    ToolButtonX { icon: "plus"; text: "add email"; onClicked: { page.draft.emails.push({ label: "home", value: "" }); page.refreshDraft() } }
                    Text { text: "Web"; color: Theme.secondary; font.weight: Font.DemiBold; topPadding: 8 }
                    Repeater {
                        model: page.draft ? (page.draft.urls || []) : []
                        delegate: RowLayout {
                            required property var modelData
                            required property int index
                            Layout.fillWidth: true
                            ComboX {
                                Layout.preferredWidth: 110
                                editable: true
                                model: ["homepage", "work", "home", "other"]
                                Component.onCompleted: { var i = find(modelData.label); if (i >= 0) currentIndex = i; else editText = modelData.label }
                                onEditTextChanged: if (page.draft && page.draft.urls[index]) page.draft.urls[index].label = editText
                            }
                            FieldX { Layout.fillWidth: true; placeholderText: "https://"; text: modelData.value; onTextEdited: page.draft.urls[index].value = text }
                            ToolButtonX { icon: "minus"; onClicked: { page.draft.urls.splice(index, 1); page.refreshDraft() } }
                        }
                    }
                    ToolButtonX { icon: "plus"; text: "add URL"; onClicked: { page.draft.urls.push({ label: "homepage", value: "" }); page.refreshDraft() } }
                    Text { text: "Address"; color: Theme.secondary; font.weight: Font.DemiBold; topPadding: 8 }
                    TextArea {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 60
                        text: page.draft ? page.draft.address : ""
                        onTextChanged: if (page.draft) page.draft.address = text
                        background: Rectangle { radius: 6; border.color: Theme.border }
                    }
                    Text { text: "Birthday (YYYY-MM-DD)"; color: Theme.secondary; font.weight: Font.DemiBold; topPadding: 8 }
                    FieldX { Layout.preferredWidth: 160; text: page.draft ? page.draft.birthday : ""; placeholderText: "1990-01-31"; onTextEdited: page.draft.birthday = text }
                    Text { text: "Groups"; color: Theme.secondary; font.weight: Font.DemiBold; topPadding: 8; visible: page.groups.length > 1 }
                    Flow {
                        Layout.fillWidth: true
                        spacing: 6
                        Repeater {
                            model: page.groups.slice(1)
                            delegate: CheckX {
                                required property var modelData
                                text: modelData.name
                                checked: page.draft ? page.draft.groups.indexOf(modelData.id) >= 0 : false
                                onToggled: {
                                    var g = page.draft.groups, i = g.indexOf(modelData.id)
                                    if (checked && i < 0) g.push(modelData.id)
                                    if (!checked && i >= 0) g.splice(i, 1)
                                }
                            }
                        }
                    }
                    Text { text: "Note"; color: Theme.secondary; font.weight: Font.DemiBold; topPadding: 8 }
                    TextArea {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 70
                        wrapMode: TextEdit.Wrap
                        text: page.draft ? page.draft.notes : ""
                        onTextChanged: if (page.draft) page.draft.notes = text
                        background: Rectangle { radius: 6; border.color: Theme.border }
                    }
                }
            }

            // Bottom bar
            Rectangle {
                anchors.bottom: parent.bottom
                width: parent.width
                height: 50
                color: Theme.window
                Rectangle { width: parent.width; height: 1; color: Theme.borderSoft }
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 16
                    anchors.rightMargin: 16
                    ToolButtonX { icon: "plus"; tip: "New Contact (Ctrl+N)"; visible: !page.editing; onClicked: { page.selectedId = -1; page.startEdit(null) } }
                    ToolButtonX {
                        visible: !page.editing && page.current !== null && page.current.emails.length > 0
                        icon: "mail"; tip: "Send Email"
                        onClicked: page.composeRequested({ to: page.current.name + " <" + page.current.emails[0].value + ">" })
                    }
                    ToolButtonX {
                        visible: !page.editing && page.current !== null && !page.current.isMe
                        text: "Make This My Card"
                        onClicked: store.setMyCard(page.current.id)
                    }
                    Item { Layout.fillWidth: true }
                    ToolButtonX {
                        visible: !page.editing && page.current !== null
                        icon: "trash"; tip: "Delete Card"
                        onClicked: {
                            var id = page.selectedId, name = page.current.name
                            confirm.ask("Delete the card for " + name + "?", "This can't be undone.", "Delete Card", function() { page.selectedId = -1; store.deleteContact(id) })
                        }
                    }
                    ToolButtonX { visible: page.editing; text: "Cancel"; onClicked: page.cancelEdit() }
                    ToolButtonX {
                        visible: page.editing || page.current !== null
                        text: page.editing ? "Done" : "Edit"
                        primary: page.editing
                        onClicked: page.editing ? page.finishEdit() : page.startEdit(page.current)
                    }
                }
            }
        }
    }
}
