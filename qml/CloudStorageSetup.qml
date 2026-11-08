import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// Accounts › Cloud Storage: where attachments too big to send by mail go.
// The mail accounts' own (Google Drive, OneDrive: allowed when signing in),
// and Dropbox, Box or a Nextcloud server, each connected here once.
ColumnLayout {
    id: cs
    required property var store
    // The live mail accounts (Accounts' list: provider, email, driveAccess…).
    property var accounts: []
    readonly property var info: { store.revision; return JSON.parse(store.cloudStores()) }
    readonly property string inUse: info.store || "account"
    // Which service's setup is open.
    property string open: ""
    spacing: 8

    Text { text: "Cloud Storage"; font.pixelSize: 15; font.weight: Font.DemiBold; color: Theme.text }
    Text {
        Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.secondary; font.pixelSize: 12
        text: "Attachments too big to send by mail go to one of these instead, and the message gets a link to them."
    }

    // One service's row: its name, how it stands, and its buttons.
    component ServiceRow: RowLayout {
        id: srow
        property string title
        property string status
        property bool good: false
        // (Orange only for something to fix, not for "not connected".)
        property bool warn: false
        property string key: ""
        default property alias buttons: btns.data
        Layout.fillWidth: true
        spacing: 8
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 1
            RowLayout {
                spacing: 6
                Text { text: srow.title; color: Theme.text; font.pixelSize: Theme.base; font.weight: Font.DemiBold }
                Text {
                    visible: srow.key !== "" && cs.inUse === srow.key && srow.good
                    text: "In use for big attachments"; color: Theme.accent; font.pixelSize: 11
                }
            }
            Text { Layout.fillWidth: true; wrapMode: Text.WordWrap; text: srow.status; color: srow.warn ? Theme.orange : Theme.secondary; font.pixelSize: 12 }
        }
        RowLayout { id: btns; spacing: 6 }
        ToolButtonX {
            visible: srow.key !== "" && srow.good && cs.inUse !== srow.key
            text: "Use for Big Attachments"
            onClicked: cs.store.setSetting("cloud_store", JSON.stringify(srow.key))
        }
    }

    // The mail accounts' own.
    Repeater {
        model: cs.accounts.filter(function(a) { return a.provider === "google" || a.provider === "microsoft" })
        delegate: ServiceRow {
            required property var modelData
            title: (modelData.provider === "google" ? "Google Drive" : "OneDrive") + "  ·  " + modelData.email
            key: "account"
            good: modelData.provider !== "google" || modelData.driveAccess === "yes"
            warn: !good
            status: modelData.provider === "google"
                    ? (modelData.driveAccess === "yes" ? "Allowed ✓"
                       : modelData.driveAccess === "no" ? "Google Drive access wasn't allowed: Sign In Again and tick the Google Drive box on Google's page."
                       : "Not allowed yet: Sign In Again to allow Google Drive (only files Almanac puts there).")
                    : "With your Microsoft sign-in (Sign In Again if it isn't allowed yet)."
            ToolButtonX { text: "Sign In Again"; enabled: !cs.store.busy; onClicked: cs.store.signInWith(modelData.provider, modelData.clientId, modelData.clientSecret, modelData.tenant) }
        }
    }

    // Dropbox and Box: an app registered with them (once), then Connect.
    Repeater {
        model: [["dropbox", "Dropbox"], ["box", "Box"]]
        delegate: ColumnLayout {
            id: appRow
            required property var modelData
            readonly property string key: modelData[0]
            readonly property var it: cs.info[key] || ({})
            Layout.fillWidth: true
            spacing: 6
            ServiceRow {
                title: appRow.modelData[1]
                key: appRow.key
                good: !!appRow.it.connected
                status: appRow.it.connected ? "Connected ✓" : "Not connected"
                ToolButtonX { visible: !appRow.it.connected && cs.open !== appRow.key; text: "Set Up…"; onClicked: cs.open = appRow.key }
                ToolButtonX { visible: !!appRow.it.connected; text: "Disconnect"; onClicked: cs.store.cloudDisconnect(appRow.key) }
            }
            ColumnLayout {
                visible: cs.open === appRow.key || (!!appRow.it.connected && cs.open === appRow.key + "-again")
                Layout.fillWidth: true
                Layout.leftMargin: 12
                spacing: 6
                Text {
                    Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.secondary; font.pixelSize: 12
                    text: appRow.key === "dropbox"
                          ? "Register an app at dropbox.com/developers (Scoped access; permissions files.content.write and sharing.write) with the redirect URI http://127.0.0.1:" + cs.info.port + ", then paste its App key here."
                          : "Register an app at developer.box.com (Custom App, User Authentication (OAuth 2.0); Write all files and folders) with the redirect URI http://127.0.0.1:" + cs.info.port + ", then paste its Client ID and Client Secret here."
                }
                RowLayout {
                    spacing: 8
                    FieldX { id: appKey; Layout.preferredWidth: 220; placeholderText: appRow.key === "dropbox" ? "App key" : "Client ID"; text: appRow.it.clientId || "" }
                    FieldX { id: appSecret; visible: appRow.key === "box"; Layout.preferredWidth: 180; placeholderText: "Client Secret"; echoMode: TextInput.Password }
                    ToolButtonX {
                        text: "Connect"
                        primary: true
                        enabled: appKey.text.trim() !== "" && (appRow.key !== "box" || appSecret.text !== "")
                        onClicked: { cs.store.cloudConnect(appRow.key, appKey.text, appSecret.text); cs.open = "" }
                    }
                    ToolButtonX { text: "Cancel"; onClicked: cs.open = "" }
                }
            }
        }
    }

    // Nextcloud: its address, then its own sign-in in the browser.
    ColumnLayout {
        id: ncRow
        readonly property var it: cs.info.nextcloud || ({})
        Layout.fillWidth: true
        spacing: 6
        ServiceRow {
            title: "Nextcloud"
            key: "nextcloud"
            good: !!ncRow.it.connected
            status: ncRow.it.connected ? "Connected ✓ as " + ncRow.it.user + " on " + ncRow.it.server : "Not connected"
            ToolButtonX { visible: !ncRow.it.connected && cs.open !== "nextcloud"; text: "Set Up…"; onClicked: cs.open = "nextcloud" }
            ToolButtonX { visible: !!ncRow.it.connected; text: "Disconnect"; onClicked: cs.store.cloudDisconnect("nextcloud") }
        }
        RowLayout {
            visible: cs.open === "nextcloud"
            Layout.leftMargin: 12
            spacing: 8
            FieldX { id: ncServer; Layout.preferredWidth: 260; placeholderText: "cloud.example.com"; text: ncRow.it.server || "" }
            ToolButtonX {
                text: "Connect"
                primary: true
                enabled: ncServer.text.trim() !== ""
                onClicked: { cs.store.cloudConnect("nextcloud", ncServer.text, ""); cs.open = "" }
            }
            ToolButtonX { text: "Cancel"; onClicked: cs.open = "" }
        }
        Text {
            visible: cs.open === "nextcloud"
            Layout.leftMargin: 12
            Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.secondary; font.pixelSize: 12
            text: "Nextcloud's own sign-in opens in your browser; nothing to register."
        }
    }
}
