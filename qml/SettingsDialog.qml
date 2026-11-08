import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtQuick.Dialogs

// Settings: General, Mail, Calendar and Contacts preferences.
Popup {
    id: dlg
    property var store
    property var accountsDialog
    property var rulesDialog
    property var signaturesDialog
    property string tab: "general"
    property var s: { store.revision; return visible ? JSON.parse(store.settings()) : ({}) }
    property var lists: { store.revision; return visible ? JSON.parse(store.blockedAndVips()) : ({ vips: [], blocked: [] }) }
    property var cals: { store.revision; return visible ? JSON.parse(store.calendars()).filter(function(c) { return !c.readOnly }) : [] }
    modal: true
    anchors.centerIn: Overlay.overlay
    // Wide enough for the longest options, but never wider than the window.
    width: Math.min(780, parent ? parent.width - 40 : 780)
    height: Math.min(parent ? parent.height - 60 : 620, 620)
    padding: 0
    background: Rectangle { radius: 14; color: Theme.popup; border.color: Theme.border }

    function set(key, value) { store.setSetting(key, JSON.stringify(value)) }

    component Row2: RowLayout {
        property alias label: lbl.text
        default property alias content: holder.data
        Layout.fillWidth: true
        spacing: 12
        Text { id: lbl; Layout.preferredWidth: Math.min(200, dlg.width * 0.3); Layout.alignment: Qt.AlignTop; topPadding: 4; horizontalAlignment: Text.AlignRight; color: Theme.text; font.pixelSize: Theme.base; wrapMode: Text.WordWrap }
        RowLayout { id: holder; Layout.fillWidth: true; Layout.minimumWidth: 0 }
    }

    contentItem: ColumnLayout {
        spacing: 0
        // Tabs
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 74
            color: Theme.toolbar
            radius: 14
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 14; color: Theme.toolbar }
            Row {
                anchors.centerIn: parent
                spacing: 6
                Repeater {
                    model: [["general", "gear", "General"], ["mail", "mail", "Mail"], ["privacy", "bellOff", "Privacy"], ["calendar", "calendar", "Calendar"],
                            ["contacts", "contacts", "Contacts"], ["accounts", "people", "Accounts"]]
                    delegate: Rectangle {
                        required property var modelData
                        width: 84; height: 56; radius: 8
                        color: dlg.tab === modelData[0] ? Theme.segmentOn : (tabArea.containsMouse ? Theme.hover : "transparent")
                        Column {
                            anchors.centerIn: parent
                            spacing: 3
                            Icon { anchors.horizontalCenter: parent.horizontalCenter; name: modelData[1]; width: 22; height: 22; color: dlg.tab === modelData[0] ? Theme.accent : Theme.secondary }
                            Text { anchors.horizontalCenter: parent.horizontalCenter; text: modelData[2]; font.pixelSize: 12; color: Theme.text }
                        }
                        MouseArea {
                            id: tabArea; anchors.fill: parent; hoverEnabled: true
                            onClicked: { if (modelData[0] === "accounts") { dlg.close(); dlg.accountsDialog.open() } else dlg.tab = modelData[0] }
                        }
                    }
                }
            }
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border }
        }

        Flickable {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            contentHeight: body.implicitHeight + 40
            ColumnLayout {
                id: body
                x: 24; y: 20
                width: dlg.width - 48
                spacing: 12

                // ------------------------------------------------- General
                ColumnLayout {
                    visible: dlg.tab === "general"
                    Layout.fillWidth: true
                    spacing: 12
                    Row2 {
                        label: "Appearance:"
                        ComboX {
                            Layout.preferredWidth: 200
                            model: ["Light", "Dark", "Match System"]
                            currentIndex: ["light", "dark", "system"].indexOf(dlg.store.theme)
                            onActivated: dlg.store.chooseTheme(["light", "dark", "system"][currentIndex])
                        }
                    }
                    Row2 {
                        label: "Open at start:"
                        ComboX {
                            Layout.preferredWidth: 200
                            model: ["Mail", "Calendar", "Contacts"]
                            currentIndex: Math.max(0, ["mail", "calendar", "contacts"].indexOf(dlg.s.start_section))
                            onActivated: dlg.set("start_section", ["mail", "calendar", "contacts"][currentIndex])
                        }
                    }
                    Row2 {
                        label: "Check for new mail and changes:"
                        ComboX {
                            Layout.preferredWidth: 200
                            readonly property var mins: [1, 5, 15, 30, 60, 0]
                            model: ["Every minute", "Every 5 minutes", "Every 15 minutes", "Every 30 minutes", "Every hour", "Manually"]
                            currentIndex: Math.max(0, mins.indexOf(dlg.s.check_minutes))
                            onActivated: dlg.set("check_minutes", mins[currentIndex])
                        }
                    }
                    Row2 {
                        label: "Notifications:"
                        ColumnLayout {
                            Layout.fillWidth: true
                            CheckX { Layout.fillWidth: true; text: "New mail"; checked: !!dlg.s.notify_mail; onToggled: dlg.set("notify_mail", checked) }
                            CheckX { Layout.fillWidth: true; text: "Calendar alerts and reminders"; checked: !!dlg.s.notify_alerts; onToggled: dlg.set("notify_alerts", checked) }
                        }
                    }
                    // Shown the way real ones are (a made-up event; a made-up message).
                    Row2 {
                        label: "Try them:"
                        ToolButtonX { text: "Test Alert"; tip: "Show an alert for a made-up event (\"Test Event\", in 5 minutes), as real ones are shown"; onClicked: dlg.store.testAlert() }
                        ToolButtonX { text: "Test New Mail"; tip: "Show a made-up new-mail notification, as real ones are shown"; onClicked: dlg.store.testMailNotification() }
                    }
                    Row2 {
                        label: "Sample data:"
                        ToolButtonX { text: "Try Sample Data"; onClicked: dlg.store.useSampleData() }
                        ToolButtonX { text: "Remove Sample Data"; onClicked: dlg.store.clearSampleData() }
                    }
                    Row2 {
                        label: "Setup guide:"
                        ToolButtonX { text: "Show Setup Guide"; onClicked: { dlg.close(); dlg.store.onboarded = false } }
                    }
                }

                // ---------------------------------------------------- Mail
                ColumnLayout {
                    visible: dlg.tab === "mail"
                    Layout.fillWidth: true
                    spacing: 12
                    Row2 {
                        label: "Viewing:"
                        ColumnLayout {
                            Layout.fillWidth: true
                            CheckX { Layout.fillWidth: true; text: "Organise by conversation"; checked: !!dlg.s.conversations; onToggled: dlg.set("conversations", checked) }
                            CheckX { Layout.fillWidth: true; text: "Classic layout (message list above the message)"; checked: dlg.s.mail_layout === "classic"; onToggled: dlg.set("mail_layout", checked ? "classic" : "standard") }
                            CheckX { Layout.fillWidth: true; text: "Show messages as formatted by the sender"; checked: !!dlg.s.show_formatted; onToggled: dlg.set("show_formatted", checked) }
                            CheckX { Layout.fillWidth: true; text: "Use dark backgrounds for messages (in dark mode)"; checked: !!dlg.s.dark_messages; onToggled: dlg.set("dark_messages", checked) }
                            CheckX { Layout.fillWidth: true; text: "Turn Markdown into formatting as I type (**bold**, # heading, - list…)"; checked: dlg.s.markdown_compose !== false; onToggled: dlg.set("markdown_compose", checked) }
                            CheckX { Layout.fillWidth: true; text: "Check spelling while typing"; checked: dlg.s.spell_check !== false; onToggled: dlg.set("spell_check", checked) }
                            CheckX { Layout.fillWidth: true; enabled: dlg.s.spell_check !== false; text: "Check grammar with spelling"; checked: dlg.s.grammar_check !== false; onToggled: dlg.set("grammar_check", checked) }
                            CheckX { Layout.fillWidth: true; text: "Mark messages as read when opened"; checked: !!dlg.s.mark_read_on_open; onToggled: dlg.set("mark_read_on_open", checked) }
                            CheckX { Layout.fillWidth: true; text: "Keep all mail on this computer (for offline reading and full search)"; checked: dlg.s.download_all_mail !== false; onToggled: { dlg.set("download_all_mail", checked); if (checked) dlg.store.refreshAll() } }
                        }
                    }
                    Row2 {
                        label: "Undo Send delay:"
                        ComboX {
                            Layout.preferredWidth: 200
                            readonly property var secs: [0, 5, 8, 10, 20, 30]
                            model: ["Off", "5 seconds", "8 seconds", "10 seconds", "20 seconds", "30 seconds"]
                            currentIndex: Math.max(0, secs.indexOf(dlg.s.undo_seconds))
                            onActivated: dlg.set("undo_seconds", secs[currentIndex])
                        }
                    }
                    Row2 {
                        label: "Composing:"
                        ToolButtonX { text: "Signatures…"; onClicked: { dlg.close(); dlg.signaturesDialog.open() } }
                        ToolButtonX { text: "Rules…"; onClicked: { dlg.close(); dlg.rulesDialog.open() } }
                    }
                    Row2 {
                        label: "When sending:"
                        ColumnLayout {
                            Layout.fillWidth: true
                            CheckX { Layout.fillWidth: true; text: "Ask when there's no subject"; checked: dlg.s.warn_no_subject !== false; onToggled: dlg.set("warn_no_subject", checked) }
                            CheckX { Layout.fillWidth: true; text: "Ask when an attachment is mentioned but none is attached"; checked: dlg.s.warn_missing_attachment !== false; onToggled: dlg.set("warn_missing_attachment", checked) }
                        }
                    }
                    // Attachments over the mail account's limit (Gmail 25 MB, Outlook 20 MB).
                    Row2 {
                        label: "Big attachments:"
                        ComboX {
                            Layout.preferredWidth: 320
                            model: ["Ask to send them through cloud storage", "Always send them through cloud storage", "Send them as they are"]
                            currentIndex: Math.max(0, ["ask", "always", "never"].indexOf(dlg.s.cloud_attachments || "ask"))
                            onActivated: dlg.set("cloud_attachments", ["ask", "always", "never"][currentIndex])
                        }
                    }
                    // Where they go: the mail account's own (Google Drive, OneDrive), or
                    // Dropbox, Box or a Nextcloud server, each connected here once.
                    Row2 {
                        id: storeRow
                        label: "Send them through:"
                        readonly property var info: { store.revision; return dlg.visible ? JSON.parse(store.cloudStores()) : ({}) }
                        readonly property var keys: ["account", "dropbox", "box", "nextcloud"]
                        property string shown: info.store || "account"
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 6
                            ComboX {
                                Layout.preferredWidth: 320
                                model: ["The mail account's own (Drive, OneDrive)", "Dropbox", "Box", "Nextcloud"]
                                currentIndex: Math.max(0, storeRow.keys.indexOf(storeRow.shown))
                                onActivated: {
                                    storeRow.shown = storeRow.keys[currentIndex]
                                    // (A service is used once it's connected; the account's own at once.)
                                    if (storeRow.shown === "account" || (storeRow.info[storeRow.shown] && storeRow.info[storeRow.shown].connected)) dlg.set("cloud_store", storeRow.shown)
                                }
                            }
                            // Dropbox or Box: an app registered with them (once), then Connect.
                            ColumnLayout {
                                visible: storeRow.shown === "dropbox" || storeRow.shown === "box"
                                Layout.fillWidth: true
                                spacing: 6
                                readonly property var it: storeRow.info[storeRow.shown] || ({})
                                Text {
                                    Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.secondary; font.pixelSize: 12
                                    text: storeRow.shown === "dropbox"
                                          ? "Register an app at dropbox.com/developers (Scoped access; permissions files.content.write and sharing.write) with the redirect URI http://127.0.0.1:" + storeRow.info.port + ", then paste its App key here."
                                          : "Register an app at developer.box.com (Custom App, User Authentication (OAuth 2.0); Write all files and folders) with the redirect URI http://127.0.0.1:" + storeRow.info.port + ", then paste its Client ID and Client Secret here."
                                }
                                RowLayout {
                                    spacing: 8
                                    FieldX { id: appKey; Layout.preferredWidth: 220; placeholderText: storeRow.shown === "dropbox" ? "App key" : "Client ID"; text: parent.parent.it.clientId || "" }
                                    FieldX { id: appSecret; visible: storeRow.shown === "box"; Layout.preferredWidth: 180; placeholderText: "Client Secret"; echoMode: TextInput.Password }
                                    ToolButtonX {
                                        text: parent.parent.it.connected ? "Connect Again" : "Connect"
                                        primary: !parent.parent.it.connected
                                        enabled: appKey.text.trim() !== "" && (storeRow.shown !== "box" || appSecret.text !== "")
                                        onClicked: store.cloudConnect(storeRow.shown, appKey.text, appSecret.text)
                                    }
                                    ToolButtonX { visible: !!parent.parent.it.connected; text: "Disconnect"; onClicked: store.cloudDisconnect(storeRow.shown) }
                                }
                                Text { visible: !!parent.it.connected; text: "Connected ✓"; color: Theme.secondary; font.pixelSize: 12 }
                            }
                            // Nextcloud: its address, then its own sign-in in the browser.
                            ColumnLayout {
                                visible: storeRow.shown === "nextcloud"
                                Layout.fillWidth: true
                                spacing: 6
                                readonly property var it: storeRow.info.nextcloud || ({})
                                RowLayout {
                                    spacing: 8
                                    FieldX { id: ncServer; Layout.preferredWidth: 260; placeholderText: "cloud.example.com"; text: parent.parent.it.server || "" }
                                    ToolButtonX {
                                        text: parent.parent.it.connected ? "Connect Again" : "Connect"
                                        primary: !parent.parent.it.connected
                                        enabled: ncServer.text.trim() !== ""
                                        onClicked: store.cloudConnect("nextcloud", ncServer.text, "")
                                    }
                                    ToolButtonX { visible: !!parent.parent.it.connected; text: "Disconnect"; onClicked: store.cloudDisconnect("nextcloud") }
                                }
                                Text {
                                    Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.secondary; font.pixelSize: 12
                                    text: parent.it.connected ? "Connected ✓ as " + parent.it.user : "Nextcloud's own sign-in opens in your browser; nothing to register."
                                }
                            }
                        }
                    }
                    Row2 {
                        label: "VIPs:"
                        Text { Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.secondary; text: dlg.lists.vips.length ? dlg.lists.vips.join(", ") : "None. Right-click a message: Add Sender to VIPs." }
                    }
                    Row2 {
                        label: "Blocked senders:"
                        ColumnLayout {
                            Layout.fillWidth: true
                            Text { visible: dlg.lists.blocked.length === 0; color: Theme.secondary; text: "None" }
                            Repeater {
                                model: dlg.lists.blocked
                                delegate: RowLayout {
                                    required property string modelData
                                    Text { text: modelData; color: Theme.text }
                                    ToolButtonX { text: "Unblock"; onClicked: dlg.store.toggleBlocked(modelData) }
                                }
                            }
                        }
                    }
                }

                // ------------------------------------------------- Privacy
                ColumnLayout {
                    visible: dlg.tab === "privacy"
                    Layout.fillWidth: true
                    spacing: 12
                    // OpenPGP: your keys (one per sending address), others' keys.
                    Row2 {
                        id: pgpRow
                        label: "OpenPGP:"
                        readonly property var info: { dlg.store.revision; return JSON.parse(dlg.store.pgpKeys()) }
                        // How the latest key job (create, import, publish, look up) is going.
                        readonly property var job: { try { return JSON.parse(dlg.store.pgpStatusJson || "{}") } catch (e) { return {} } }
                        readonly property bool busy: !!job.busy && job.op !== "unlock"
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 6
                            Text { visible: !pgpRow.info.gpg; Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.secondary; text: "Install GnuPG to sign and encrypt mail (Gpg4win on Windows, GPG Suite on macOS)." }
                            Repeater {
                                model: pgpRow.info.gpg ? pgpRow.info.keys : []
                                delegate: RowLayout {
                                    required property var modelData
                                    Layout.fillWidth: true
                                    spacing: 8
                                    Text { text: modelData.email; color: Theme.text; font.pixelSize: Theme.base }
                                    Text {
                                        Layout.fillWidth: true
                                        text: modelData.fingerprint ? "Key " + modelData.fingerprint.slice(-16).replace(/(.{4})/g, "$1 ").trim() : "No key"
                                        color: Theme.secondary; font.pixelSize: 12; elide: Text.ElideRight
                                    }
                                    ToolButtonX { visible: !modelData.fingerprint; text: "Create Key…"; tip: "Make a key for this address (you'll be asked to choose a passphrase)"; enabled: !pgpRow.busy
                                        onClicked: { var who = modelData; passDlg.ask("Choose a passphrase for your key", "It protects the key for " + who.email + ". Almanac asks for it when you sign or read encrypted mail, then remembers it for a few minutes.", "Create Key", true, function(p) { dlg.store.pgpCreateKey(who.name, who.email, p) }) } }
                                    ToolButtonX {
                                        visible: !!modelData.fingerprint
                                        text: "Publish…"
                                        tip: "Upload your public key to keys.openpgp.org so people can find it to encrypt to you"
                                        onClicked: publishConfirm.ask("Publish your key?", "Your public key and " + modelData.email + " are sent to keys.openpgp.org, which emails you to confirm before anyone can find it.", "Publish", function() { dlg.store.pgpPublishKey(modelData.fingerprint) })
                                    }
                                }
                            }
                            RowLayout {
                                visible: !!pgpRow.job.op && pgpRow.job.op !== "unlock"
                                Layout.fillWidth: true
                                spacing: 8
                                BusyIndicator { visible: pgpRow.busy; running: pgpRow.busy; Layout.preferredWidth: 20; Layout.preferredHeight: 20 }
                                Text {
                                    Layout.fillWidth: true
                                    wrapMode: Text.WordWrap
                                    font.pixelSize: 12
                                    color: pgpRow.busy ? Theme.secondary : (pgpRow.job.ok ? Theme.text : Theme.red)
                                    text: (pgpRow.job.ok || pgpRow.busy ? "" : "Couldn't do that: ") + (pgpRow.job.text || "")
                                }
                            }
                            RowLayout {
                                visible: pgpRow.info.gpg
                                spacing: 8
                                ToolButtonX { text: "Import Key…"; tip: "Add keys from a file: yours, or the people you write to"; onClicked: keyFile.open() }
                                FieldX { id: lookUpField; Layout.preferredWidth: 200; placeholderText: "someone@example.com" }
                                ToolButtonX { text: "Look Up Key"; tip: "Find this person's key (their mail provider, then keys.openpgp.org)"; onClicked: if (lookUpField.text.indexOf("@") > 0) dlg.store.pgpLookUp(lookUpField.text) }
                            }
                        }
                    }
                    Row2 {
                        label: "Remote content:"
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 6
                            CheckX { Layout.fillWidth: true; text: "Always load remote content in messages"; checked: !!dlg.s.load_remote; onToggled: dlg.set("load_remote", checked) }
                            Text {
                                Layout.fillWidth: true
                                Layout.maximumWidth: 460
                                wrapMode: Text.WordWrap
                                color: Theme.secondary; font.pixelSize: 12
                                text: "Pictures and styles that an email fetches from the internet. Loading them lets the sender see that and when you "
                                      + "opened the message, and your internet address (roughly where you are). Apple Mail's “Protect Mail Activity” "
                                      + "hides this by loading them through Apple's servers; Almanac has no such relay, so it asks first unless you "
                                      + "turn this on. Pictures sent inside the email always show."
                            }
                        }
                    }
                    Row2 {
                        label: "Always load from:"
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 4
                            Text {
                                visible: !(dlg.s.remote_senders && dlg.s.remote_senders.length)
                                text: "No senders yet. Choose “Always from This Sender” on a message."
                                color: Theme.tertiary; font.pixelSize: 12
                            }
                            Repeater {
                                model: dlg.s.remote_senders || []
                                delegate: RowLayout {
                                    required property string modelData
                                    required property int index
                                    spacing: 8
                                    Text { text: modelData; color: Theme.text; font.pixelSize: Theme.base; Layout.preferredWidth: 260; elide: Text.ElideRight }
                                    ToolButtonX {
                                        icon: "close"; tip: "Stop loading remote content from this sender"
                                        onClicked: { var l = dlg.s.remote_senders.slice(); l.splice(index, 1); dlg.set("remote_senders", l) }
                                    }
                                }
                            }
                        }
                    }
                }

                // ------------------------------------------------ Calendar
                ColumnLayout {
                    visible: dlg.tab === "calendar"
                    Layout.fillWidth: true
                    spacing: 12
                    Row2 {
                        label: "Default calendar:"
                        ComboX {
                            Layout.preferredWidth: 220
                            model: dlg.cals.map(function(c) { return c.name })
                            currentIndex: Math.max(0, dlg.cals.findIndex(function(c) { return c.id === dlg.s.default_calendar }))
                            onActivated: dlg.set("default_calendar", dlg.cals[currentIndex].id)
                        }
                    }
                    Row2 {
                        label: "Default alert:"
                        ComboX {
                            Layout.preferredWidth: 220
                            readonly property var mins: [null, 0, 5, 10, 15, 30, 60, 1440]
                            model: ["None", "At time of event", "5 minutes before", "10 minutes before", "15 minutes before", "30 minutes before", "1 hour before", "1 day before"]
                            currentIndex: Math.max(0, mins.indexOf(dlg.s.default_alert === undefined ? null : dlg.s.default_alert))
                            onActivated: dlg.set("default_alert", mins[currentIndex])
                        }
                    }
                    Row2 {
                        label: "Alerts:"
                        ToolButtonX { text: "Send Test Alert"; tip: "Show an alert for a made-up event (\"Test Event\", in 5 minutes): the desktop notification and the alert in the window, as real ones are shown"; onClicked: dlg.store.testAlert() }
                    }
                    Row2 {
                        label: "New events last:"
                        ComboX {
                            Layout.preferredWidth: 220
                            readonly property var mins: [15, 30, 45, 60, 90, 120]
                            model: ["15 minutes", "30 minutes", "45 minutes", "1 hour", "1½ hours", "2 hours"]
                            currentIndex: Math.max(0, mins.indexOf(dlg.s.event_minutes))
                            onActivated: dlg.set("event_minutes", mins[currentIndex])
                        }
                    }
                    Row2 {
                        label: "Start week on:"
                        ComboX {
                            Layout.preferredWidth: 220
                            readonly property var days: [-1, 0, 1, 6]
                            model: ["As the system does", "Sunday", "Monday", "Saturday"]
                            currentIndex: Math.max(0, days.indexOf(dlg.s.week_start))
                            onActivated: dlg.set("week_start", days[currentIndex])
                        }
                    }
                    Row2 {
                        label: "Default view:"
                        ComboX {
                            Layout.preferredWidth: 220
                            model: ["Day", "Week", "Month", "Year"]
                            currentIndex: Math.max(0, ["day", "week", "month", "year"].indexOf(dlg.s.calendar_view))
                            onActivated: dlg.set("calendar_view", ["day", "week", "month", "year"][currentIndex])
                        }
                    }
                    Row2 {
                        label: "Time zones:"
                        CheckX { Layout.fillWidth: true; text: "Show time zone in the event editor"; checked: !!dlg.s.time_zone_support; onToggled: dlg.set("time_zone_support", checked) }
                    }
                }

                // ------------------------------------------------ Contacts
                ColumnLayout {
                    visible: dlg.tab === "contacts"
                    Layout.fillWidth: true
                    spacing: 12
                    Row2 {
                        label: "Sort by:"
                        ComboX {
                            Layout.preferredWidth: 200
                            model: ["Last name", "First name"]
                            currentIndex: dlg.s.contacts_sort_first ? 1 : 0
                            onActivated: dlg.set("contacts_sort_first", currentIndex === 1)
                        }
                    }
                    Row2 {
                        label: "Show names as:"
                        ComboX {
                            Layout.preferredWidth: 200
                            model: ["First Last", "Last, First"]
                            currentIndex: dlg.s.contacts_last_first ? 1 : 0
                            onActivated: dlg.set("contacts_last_first", currentIndex === 1)
                        }
                    }
                }
            }
        }
        RowLayout {
            Layout.fillWidth: true
            Layout.margins: 14
            Item { Layout.fillWidth: true }
            ToolButtonX { text: "Done"; primary: true; onClicked: dlg.close() }
        }
    }
    Confirm { id: publishConfirm }
    PassphraseDialog { id: passDlg }
    FileDialog {
        id: keyFile
        title: "Import OpenPGP Keys"
        nameFilters: ["OpenPGP keys (*.asc *.gpg *.pgp *.key)", "All files (*)"]
        onAccepted: dlg.store.pgpImportKeys(selectedFile.toString())
    }
}
