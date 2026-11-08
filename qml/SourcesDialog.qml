import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// Accounts: live mail/calendar/contact accounts (IMAP, SMTP, CalDAV,
// CardDAV), plus reading data from Thunderbird or Mailspring.
Popup {
    id: dlg
    property var store
    property var sources: { store.revision; store.busy; return visible ? JSON.parse(store.sources()) : [] }
    property var accounts: { store.revision; store.busy; return visible ? JSON.parse(store.liveAccounts()) : [] }
    property bool editing: false
    property var form: ({})
    // Choosing a provider for a new account (before its form).
    property bool choosing: false
    // The provider picked for the account being added (null: any other).
    property var chosen: null
    readonly property var mailProviders: JSON.parse(store.mailProviders())
    // Google / Microsoft sign-in form ("" when not shown)
    property string oauthProvider: ""
    modal: true
    focus: true
    anchors.centerIn: Overlay.overlay
    width: 600
    height: Math.min(parent ? parent.height - 60 : 700, col.implicitHeight + 44)
    padding: 0
    background: Rectangle { radius: 14; color: Theme.popup; border.color: Theme.border }

    property var providers: { store.revision; return visible ? JSON.parse(store.signInProviders()) : [] }
    function providerInfo(id) { var p = JSON.parse(store.signInProviders()).filter(function(x) { return x.id === id }); return p.length ? p[0] : null }
    // Sign-in Setup panel (in the main window)
    property var signInSetup
    function startOAuth(provider) {
        editing = false
        oauthProvider = ""
        // Set up: straight to the provider's page. Not yet: the setup steps.
        signInSetup.openFor(provider, true)
    }
    function newAccount() {
        form = { id: "", name: "", fullName: "", email: "", username: "", imapHost: "", imapPort: 993,
                 smtpHost: "", smtpPort: 465, caldavUrl: "", carddavUrl: "" }
        password.text = ""
        showServers.checked = false
        chosen = null
        choosing = false
        editing = true
    }
    // "Add Account…": which provider (as in Apple Mail), then its form or sign-in page.
    function chooseProvider() { editing = false; oauthProvider = ""; choosing = true }
    function pickProvider(id) {
        var p = mailProviders.filter(function(x) { return x.id === id })[0]
        if (!p) { newAccount(); return }
        if (p.how === "none") { noteFor = p; return }
        choosing = false
        if (p.how === "oauth") { startOAuth(p.oauth); return }
        newAccount()
        chosen = p
        var f = JSON.parse(JSON.stringify(form))
        if (p.how === "ews") f.incoming = "ews"
        // Fixed servers (Proton Mail Bridge): kept whatever the address (custom domains too).
        if (p.servers) { for (var k in p.servers) f[k] = p.servers[k]; f.locked = true }
        form = f
    }
    // A provider that can't be added: why, shown under the choices.
    property var noteFor: null
    onChoosingChanged: noteFor = null
    function editAccount(a) {
        form = JSON.parse(JSON.stringify(a))
        password.text = ""
        showServers.checked = true
        editing = true
    }
    function fillFromEmail() {
        if (!form.email || form.email.indexOf("@") < 0) return
        var p = JSON.parse(store.accountPreset(form.email))
        var f = JSON.parse(JSON.stringify(form))
        if (f.locked) {
            // The provider's servers stay; only the name fields come from the address.
            if (!f.name) f.name = f.email
            if (!f.username) f.username = f.email
            form = f
            return
        }
        if (f.incoming === "ews") { if (!f.name) f.name = f.email; if (!f.username) f.username = f.email; form = f; return }
        for (var k in p) if (!f.id || !f[k]) f[k] = p[k]
        if (!f.name) f.name = f.email
        form = f
        // A provider Almanac doesn't know: its servers are looked up (custom domains too).
        if (!f.id) store.discoverAccount(f.email)
    }
    Connections {
        target: store
        function onDiscoveredChanged() {
            var d = JSON.parse(store.discovered || "{}")
            if (!d.email || d.email !== dlg.form.email || dlg.form.id) return
            var f = JSON.parse(JSON.stringify(dlg.form))
            for (var k in d) if (k !== "how" && k !== "email") f[k] = d[k]
            dlg.form = f
        }
    }
    // Proton Mail goes through its Bridge app on this computer.
    readonly property bool proton: (chosen && chosen.id === "proton") || /@(proton\.me|protonmail\.com|protonmail\.ch|pm\.me)$/i.test(form.email || "")
    // Whether Proton Mail Bridge is running, checked every few seconds while it matters.
    property bool bridgeUp: false
    Timer {
        interval: 2500; repeat: true; triggeredOnStart: true
        running: dlg.visible && dlg.editing && dlg.proton
        onTriggered: dlg.bridgeUp = dlg.store.bridgeRunning()
    }
    function saveAccount() {
        store.saveLiveAccount(JSON.stringify(form), password.text)
        password.text = ""
        editing = false
    }

    component Field: ColumnLayout {
        property alias label: lbl.text
        property alias text: tf.text
        property alias placeholder: tf.placeholderText
        signal edited(string text)
        signal done()
        Layout.fillWidth: true
        spacing: 3
        Text { id: lbl; color: Theme.secondary; font.pixelSize: 11 }
        FieldX {
            id: tf
            Layout.fillWidth: true
            selectByMouse: true
            onTextEdited: parent.edited(text)
            onEditingFinished: parent.done()
        }
    }

    contentItem: Flickable {
        clip: true
        contentHeight: col.implicitHeight + 44
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar {}
        ColumnLayout {
            id: col
            x: 22; y: 22
            width: dlg.width - 44
            spacing: 14

            RowLayout {
                Layout.fillWidth: true
                Text {
                    Layout.fillWidth: true
                    text: dlg.choosing ? "Choose a Mail Provider" : dlg.editing ? (dlg.form.id ? "Account Settings" : (dlg.chosen ? "Add " + dlg.chosen.name + " Account" : "Add Account")) : "Accounts"
                    font.pixelSize: 18; font.weight: Font.Bold; color: Theme.text
                }
                ToolButtonX { visible: dlg.choosing; text: "Cancel"; onClicked: dlg.choosing = false }
                ToolButtonX { visible: !dlg.editing && !dlg.choosing; text: "Done"; onClicked: dlg.close() }
            }

            // ------------------------------------------------ account list
            Text {
                visible: !dlg.editing && !dlg.choosing
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: Theme.secondary
                text: "Mail, calendars and contacts sync with your accounts. Passwords are kept in this computer's keychain."
            }
            Repeater {
                model: dlg.editing || dlg.choosing ? [] : dlg.accounts
                delegate: Rectangle {
                    required property var modelData
                    Layout.fillWidth: true
                    Layout.preferredHeight: acol.implicitHeight + 24
                    radius: 10
                    color: Theme.card
                    border.color: Theme.borderSoft
                    ColumnLayout {
                        id: acol
                        anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
                        anchors.margins: 12
                        spacing: 4
                        RowLayout {
                            Layout.fillWidth: true
                            Avatar { name: modelData.name; size: 34 }
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 1
                                Text { text: modelData.name; font.pixelSize: 15; font.weight: Font.DemiBold; color: Theme.text; elide: Text.ElideRight; Layout.fillWidth: true }
                                Text { text: modelData.email; color: Theme.secondary; font.pixelSize: 12 }
                            }
                            ToolButtonX { icon: "refresh"; tip: "Sync now"; enabled: !store.busy; onClicked: store.refreshAll() }
                            // Google and Microsoft accounts: their sign-in done again (to
                            // allow new permissions, e.g. Drive, or after being signed out).
                            ToolButtonX {
                                visible: modelData.provider === "google" || modelData.provider === "microsoft"
                                text: "Sign In Again"
                                tip: "Sign in with " + (modelData.provider === "google" ? "Google" : "Microsoft") + " again (to allow new permissions, or after being signed out)"
                                enabled: !store.busy
                                onClicked: store.signInWith(modelData.provider, modelData.clientId, modelData.clientSecret, modelData.tenant)
                            }
                            ToolButtonX { text: "Settings"; onClicked: dlg.editAccount(modelData) }
                        }
                        Text {
                            visible: modelData.driveAccess === "no"
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            text: "Google Drive access wasn't allowed, so big attachments can't be sent as links. Press Sign In Again and tick the Google Drive box on Google's page."
                            color: Theme.orange; font.pixelSize: 12
                        }
                        Text {
                            text: modelData.messages + " messages · " + modelData.events + " events · " + modelData.contacts + " contacts"
                                  + (modelData.lastSync ? "  (synced " + modelData.lastSync + ")" : "  (not synced yet)")
                            color: Theme.secondary; font.pixelSize: 12
                        }
                        Text {
                            visible: modelData.lastError !== ""
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            text: modelData.lastError
                            color: Theme.red; font.pixelSize: 12
                        }
                    }
                }
            }
            RowLayout {
                visible: !dlg.editing && !dlg.choosing && dlg.oauthProvider === ""
                spacing: 8
                ToolButtonX { icon: "plus"; text: "Add Account…"; primary: true; onClicked: dlg.chooseProvider() }
            }

            // ------------------------------------------------ choose a provider
            GridLayout {
                visible: dlg.choosing
                Layout.fillWidth: true
                columns: 2
                columnSpacing: 8
                rowSpacing: 8
                Repeater {
                    model: dlg.mailProviders.filter(function(p) { return p.how !== "none" }).concat([{ id: "other", name: "Other Mail Account", detail: "Any other provider, or your own server", glyph: "", tint: "#8e8e93", how: "form" }])
                    delegate: Rectangle {
                        id: tile
                        required property var modelData
                        Layout.fillWidth: true
                        Layout.preferredWidth: 1
                        Layout.preferredHeight: 52
                        radius: 10
                        color: tileArea.containsMouse ? Theme.hover : Theme.card
                        border.color: Theme.borderSoft
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 10; anchors.rightMargin: 10
                            spacing: 10
                            Rectangle {
                                width: 32; height: 32; radius: 8
                                color: tile.modelData.tint
                                Icon { visible: !tile.modelData.glyph; anchors.centerIn: parent; name: "mail"; color: "white"; width: 18; height: 18 }
                                Text { visible: !!tile.modelData.glyph; anchors.centerIn: parent; text: tile.modelData.glyph || ""; color: "white"; font.pixelSize: (tile.modelData.glyph || "").length > 2 ? 10 : 15; font.weight: Font.Bold }
                            }
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 1
                                Text { Layout.fillWidth: true; text: tile.modelData.name; color: Theme.text; font.pixelSize: 14; font.weight: Font.DemiBold; elide: Text.ElideRight }
                                Text { Layout.fillWidth: true; text: tile.modelData.detail; color: Theme.secondary; font.pixelSize: 11; elide: Text.ElideRight }
                            }
                        }
                        MouseArea {
                            id: tileArea
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: tile.modelData.id === "other" ? dlg.newAccount() : dlg.pickProvider(tile.modelData.id)
                        }
                    }
                }
            }
            // Services other mail apps can't reach, and why.
            Text {
                visible: dlg.choosing
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: Theme.secondary; font.pixelSize: 12
                text: dlg.mailProviders.filter(function(p) { return p.how === "none" }).map(function(p) { return p.help }).join(" ")
            }

            // ------------------------------------- Google / Microsoft sign-in
            ColumnLayout {
                visible: !dlg.editing && dlg.oauthProvider !== ""
                Layout.fillWidth: true
                spacing: 8
                Text {
                    text: "Set up " + (dlg.providerInfo(dlg.oauthProvider) || { name: "" }).name + " sign-in"
                    font.pixelSize: 15; font.weight: Font.DemiBold; color: Theme.text
                }
                Text {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: Theme.secondary; font.pixelSize: 12
                    text: "Apps like Thunderbird come with a registration from Google, Microsoft and Yahoo built in; this copy of Almanac doesn't have one yet. "
                        + "Whoever publishes Almanac can include it once for everyone (see oauth-clients.json); until then you can register your own below.\n\n"
                        + ({
                            google: "In the Google Cloud console: create a project, turn on the Gmail, Google Calendar and People APIs, then under Credentials create an OAuth client of type “Desktop app”. Paste its Client ID and Client secret here.",
                            microsoft: "In the Microsoft Entra admin center: App registrations › New registration, choose “Accounts in any organizational directory and personal Microsoft accounts”, add the platform “Mobile and desktop applications” with redirect URI http://localhost, and allow public client flows. Paste the Application (client) ID here.",
                            yahoo: "On developer.yahoo.com create an app with Mail read/write access and the redirect address http://localhost, then paste its Client ID and Client secret here.",
                            aol: "On developer.aol.com create an app with Mail read/write access and the redirect address http://localhost, then paste its Client ID and Client secret here."
                        }[dlg.oauthProvider] || "")
                        + "\n\nYour browser then shows the provider's own sign-in page; your password never passes through this app."
                }
                FieldX { id: oClient; Layout.fillWidth: true; placeholderText: "Client ID"; selectByMouse: true }
                FieldX { id: oSecret; visible: dlg.oauthProvider !== "microsoft"; Layout.fillWidth: true; placeholderText: "Client secret"; echoMode: TextInput.Password; selectByMouse: true }
                FieldX { id: oTenant; visible: dlg.oauthProvider === "microsoft"; Layout.fillWidth: true; placeholderText: "Directory (tenant): common"; selectByMouse: true }
                RowLayout {
                    Item { Layout.fillWidth: true }
                    ToolButtonX { text: "Cancel"; onClicked: dlg.oauthProvider = "" }
                    ToolButtonX {
                        text: "Open Sign-in Page"; primary: true
                        enabled: oClient.text.trim() !== "" && !store.busy
                        onClicked: { store.signInWith(dlg.oauthProvider, oClient.text, oSecret.text, oTenant.text); dlg.oauthProvider = "" }
                    }
                }
            }

            // ------------------------------------------------ account form
            ColumnLayout {
                visible: dlg.editing
                Layout.fillWidth: true
                spacing: 10
                // What this provider needs (an app password, IMAP turned on…).
                Text {
                    visible: !!dlg.chosen && !!dlg.chosen.help && !dlg.form.id
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: Theme.secondary; font.pixelSize: 12
                    textFormat: Text.StyledText
                    linkColor: Theme.accent
                    onLinkActivated: function(url) { Qt.openUrlExternally(url) }
                    text: dlg.chosen ? (dlg.chosen.help || "") + (dlg.chosen.link ? " <a href=\"" + dlg.chosen.link + "\">Open that page</a>" : "") : ""
                }
                Field {
                    label: "Email address"; placeholder: dlg.chosen && dlg.chosen.domain ? "name@" + dlg.chosen.domain : "name@example.com"
                    text: dlg.form.email || ""
                    onEdited: function(t) { dlg.form.email = t }
                    onDone: dlg.fillFromEmail()
                }
                FieldX {
                    id: password
                    Layout.fillWidth: true
                    echoMode: TextInput.Password
                    placeholderText: dlg.form.id ? "Password (leave empty to keep the saved one)" : (dlg.proton ? "Bridge password (shown in Proton Mail Bridge)" : "Password or app password")
                    selectByMouse: true
                }
                Text {
                    visible: dlg.proton
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: Theme.secondary
                    font.pixelSize: 12
                    textFormat: Text.StyledText
                    linkColor: Theme.accent
                    onLinkActivated: function(url) { Qt.openUrlExternally(url) }
                    text: "Proton Mail is end-to-end encrypted, so Almanac reaches it through <b>Proton Mail Bridge</b>, Proton's free app that runs on this computer "
                          + "(<a href=\"https://proton.me/mail/bridge\">get it here</a>; it needs a paid Proton plan). Start Bridge, sign in there, "
                          + "then use the address and the password Bridge shows (custom-domain addresses work too). Proton calendars and contacts can't be synced by other apps."
                }
                RowLayout {
                    visible: dlg.proton
                    spacing: 6
                    Icon { name: dlg.bridgeUp ? "check" : "close"; width: 14; height: 14; color: dlg.bridgeUp ? "#34a853" : Theme.red }
                    Text {
                        text: dlg.bridgeUp ? "Proton Mail Bridge is running on this computer." : "Proton Mail Bridge isn't running yet: start it and sign in there first."
                        color: dlg.bridgeUp ? Theme.secondary : Theme.red; font.pixelSize: 12
                    }
                }
                Field { label: "Your name (shown on mail you send)"; text: dlg.form.fullName || ""; onEdited: function(t) { dlg.form.fullName = t } }
                Field { label: "Account description"; text: dlg.form.name || ""; placeholder: "e.g. Work"; onEdited: function(t) { dlg.form.name = t } }
                Text {
                    visible: !dlg.chosen && !dlg.form.id
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: Theme.secondary; font.pixelSize: 11
                    text: "Many providers want an app password from your account's security settings, and some need IMAP turned on first. "
                          + "Server settings are looked up from the address; open Server settings to change them."
                }
                CheckX { id: showServers; text: "Server settings"; checked: false }
                GridLayout {
                    visible: showServers.checked
                    Layout.fillWidth: true
                    columns: 2
                    columnSpacing: 10
                    rowSpacing: 8
                    Field { Layout.columnSpan: 2; label: "User name"; text: dlg.form.username || ""; onEdited: function(t) { dlg.form.username = t } }
                    // IMAP keeps mail on the server (folders, read and flagged everywhere);
                    // POP downloads it to this computer's Inbox.
                    RowLayout {
                        Layout.columnSpan: 2
                        Text { text: "Incoming mail:"; color: Theme.secondary }
                        ComboX {
                            model: ["IMAP (recommended)", "POP", "Microsoft Exchange (EWS)"]
                            currentIndex: dlg.form.incoming === "pop" ? 1 : (dlg.form.incoming === "ews" ? 2 : 0)
                            onActivated: function(i) {
                                var f = JSON.parse(JSON.stringify(dlg.form))
                                f.incoming = i === 1 ? "pop" : (i === 2 ? "ews" : "")
                                // Exchange: the server is found from the address (or typed in).
                                if (i === 2) { f.imapHost = ""; f.smtpHost = ""; dlg.form = f; return }
                                if (i === 1 && (!f.imapPort || f.imapPort === 993 || f.imapPort === 143)) { f.imapPort = 995; if (/^imap\./.test(f.imapHost || "")) f.imapHost = f.imapHost.replace(/^imap\./, "pop.") }
                                if (i === 0 && (f.imapPort === 995 || f.imapPort === 110)) { f.imapPort = 993; if (/^pop3?\./.test(f.imapHost || "")) f.imapHost = f.imapHost.replace(/^pop3?\./, "imap.") }
                                dlg.form = f
                            }
                        }
                        Item { Layout.fillWidth: true }
                    }
                    readonly property bool ews: dlg.form.incoming === "ews"
                    Field {
                        Layout.columnSpan: parent.ews ? 2 : 1
                        label: parent.ews ? "Exchange server (EWS address, empty to find it from the email address)" : (dlg.form.incoming === "pop" ? "Incoming mail server (POP)" : "Incoming mail server (IMAP)")
                        placeholder: parent.ews ? "https://mail.example.com/EWS/Exchange.asmx" : ""
                        text: dlg.form.imapHost || ""; onEdited: function(t) { dlg.form.imapHost = t }
                    }
                    Field { visible: !parent.ews; Layout.preferredWidth: 120; Layout.fillWidth: false; label: dlg.form.incoming === "pop" ? "Port (995 / 110)" : "Port (993 / 143)"; text: "" + (dlg.form.imapPort || 993); onEdited: function(t) { dlg.form.imapPort = parseInt(t) || 993 } }
                    Field { visible: !parent.ews; label: "Outgoing mail server (SMTP)"; text: dlg.form.smtpHost || ""; onEdited: function(t) { dlg.form.smtpHost = t } }
                    Field { visible: !parent.ews; Layout.preferredWidth: 120; Layout.fillWidth: false; label: "Port (465 / 587)"; text: "" + (dlg.form.smtpPort || 465); onEdited: function(t) { dlg.form.smtpPort = parseInt(t) || 465 } }
                    Field { Layout.columnSpan: 2; label: "Calendar server (CalDAV), empty to skip"; text: dlg.form.caldavUrl || ""; onEdited: function(t) { dlg.form.caldavUrl = t } }
                    Field { Layout.columnSpan: 2; label: "Contacts server (CardDAV), empty to skip"; text: dlg.form.carddavUrl || ""; onEdited: function(t) { dlg.form.carddavUrl = t } }
                }
                RowLayout {
                    Layout.fillWidth: true
                    ToolButtonX {
                        visible: !!dlg.form.id
                        icon: "trash"; text: "Remove Account"
                        onClicked: removeConfirm.open()
                    }
                    Item { Layout.fillWidth: true }
                    ToolButtonX { text: dlg.form.id ? "Cancel" : "Back"; onClicked: { dlg.editing = false; if (!dlg.form.id) dlg.choosing = true } }
                    ToolButtonX {
                        text: dlg.form.id ? "Save" : "Sign In"
                        primary: true
                        enabled: (dlg.form.email || "").indexOf("@") > 0 && (password.text !== "" || !!dlg.form.id) && !store.busy
                        onClicked: { dlg.fillFromEmail(); dlg.saveAccount() }
                    }
                }
            }

            // ------------------------------------ other apps on this computer
            Rectangle { visible: !dlg.editing; Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.borderSoft; Layout.topMargin: 6 }
            Text { visible: !dlg.editing; text: "From other apps on this computer"; font.pixelSize: 15; font.weight: Font.DemiBold; color: Theme.text }
            Text {
                visible: !dlg.editing
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: Theme.secondary
                text: "Read a copy of what Thunderbird or Mailspring keep on this computer. Nothing is changed in those apps."
            }
            Repeater {
                model: dlg.editing ? [] : dlg.sources
                delegate: Rectangle {
                    required property var modelData
                    Layout.fillWidth: true
                    Layout.preferredHeight: scol.implicitHeight + 24
                    radius: 10
                    color: Theme.card
                    border.color: Theme.borderSoft
                    ColumnLayout {
                        id: scol
                        anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
                        anchors.margins: 12
                        spacing: 6
                        RowLayout {
                            Layout.fillWidth: true
                            Rectangle {
                                width: 34; height: 34; radius: 8
                                color: modelData.id === "thunderbird" ? "#2c6fd1" : "#36b5a4"
                                Icon { anchors.centerIn: parent; name: "mail"; color: "white"; width: 20; height: 20 }
                            }
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 1
                                Text { text: modelData.name; font.pixelSize: 15; font.weight: Font.DemiBold; color: Theme.text }
                                Text {
                                    Layout.fillWidth: true
                                    text: modelData.found ? modelData.path : "Not found on this computer"
                                    color: Theme.secondary; font.pixelSize: 11; elide: Text.ElideMiddle
                                }
                            }
                            ToolButtonX {
                                text: modelData.counts.messages + modelData.counts.events + modelData.counts.contacts > 0 ? "Import Again" : "Import"
                                enabled: modelData.found && !store.busy
                                onClicked: store.importSource(modelData.id)
                            }
                        }
                        Text {
                            visible: modelData.lastImport !== ""
                            text: modelData.counts.messages + " messages · " + modelData.counts.events + " events · "
                                  + modelData.counts.contacts + " contacts  (last imported " + modelData.lastImport + ")"
                            color: Theme.secondary; font.pixelSize: 12
                        }
                        CheckX {
                            enabled: modelData.found
                            text: "Keep up to date (read again at start-up and with Get Mail)"
                            checked: modelData.autoImport
                            onToggled: store.setAutoImport(modelData.id, checked)
                        }
                    }
                }
            }
            RowLayout {
                Layout.fillWidth: true
                BusyIndicator { running: store.busy; visible: store.busy; Layout.preferredWidth: 26; Layout.preferredHeight: 26 }
                Text {
                    Layout.fillWidth: true
                    text: store.statusText
                    color: Theme.secondary; font.pixelSize: 12; elide: Text.ElideRight
                }
            }
            RowLayout {
                visible: !dlg.editing
                ToolButtonX { text: "Setup Guide"; onClicked: { dlg.close(); dlg.store.onboarded = false } }
                ToolButtonX { text: "Try Sample Data"; tip: "Fill On My Computer with made-up examples"; onClicked: dlg.store.useSampleData() }
                ToolButtonX { text: "Remove Sample Data"; onClicked: dlg.store.clearSampleData() }
            }
        }
    }

    Popup {
        id: removeConfirm
        anchors.centerIn: Overlay.overlay
        modal: true
        padding: 18
        background: Rectangle { radius: 12; color: Theme.popup; border.color: Theme.border }
        contentItem: ColumnLayout {
            spacing: 12
            Text { text: "Remove " + (dlg.form.name || "this account") + "?"; font.pixelSize: 15; font.weight: Font.DemiBold; color: Theme.text }
            Text { text: "Its mail, calendars and contacts are removed from this app (not from the server)."; color: Theme.secondary }
            RowLayout {
                Item { Layout.fillWidth: true }
                ToolButtonX { text: "Cancel"; onClicked: removeConfirm.close() }
                ToolButtonX { text: "Remove"; primary: true; onClicked: { store.removeLiveAccount(dlg.form.id); dlg.editing = false; removeConfirm.close() } }
            }
        }
    }

    Popup {
        id: resetConfirm
        anchors.centerIn: Overlay.overlay
        modal: true
        padding: 18
        background: Rectangle { radius: 12; color: Theme.popup; border.color: Theme.border }
        contentItem: ColumnLayout {
            spacing: 12
            Text { text: "Replace all data with the sample data?"; font.pixelSize: 15; font.weight: Font.DemiBold; color: Theme.text }
            Text { text: "Imported items and your own changes in this app will be removed."; color: Theme.secondary }
            RowLayout {
                Item { Layout.fillWidth: true }
                ToolButtonX { text: "Cancel"; onClicked: resetConfirm.close() }
                ToolButtonX { text: "Replace"; primary: true; onClicked: { store.resetSampleData(); resetConfirm.close() } }
            }
        }
    }
}
