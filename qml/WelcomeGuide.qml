import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// First start: a short guide to adding mail, calendar and contact
// accounts. Shown until finished; open again from the menu (Setup Guide).
Rectangle {
    id: guide
    property var store
    property var accountsDialog
    property var accounts: { store.revision; store.busy; return JSON.parse(store.liveAccounts()) }
    property var sources: { store.revision; return visible ? JSON.parse(store.sources()) : [] }
    color: Theme.window

    function openAccounts(mode) {
        accountsDialog.open()
        if (mode === "add") accountsDialog.chooseProvider()
        else if (mode === "add-other") accountsDialog.newAccount()
        else if (mode) accountsDialog.startOAuth(mode)
    }

    component Choice: Rectangle {
        id: choice
        property string icon: ""
        // A short mark instead of an icon (the provider's initial)
        property string glyph: ""
        property color tint: Theme.accent
        property string title: ""
        property string detail: ""
        signal picked()
        Layout.fillWidth: true
        Layout.preferredHeight: 66
        radius: 12
        color: area.containsMouse ? Theme.hover : Theme.card
        border.color: Theme.borderSoft
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 14
            anchors.rightMargin: 14
            spacing: 14
            Rectangle {
                width: 38; height: 38; radius: 10
                color: choice.tint
                Icon { visible: choice.glyph === ""; anchors.centerIn: parent; name: choice.icon; color: "white"; width: 22; height: 22 }
                Text { visible: choice.glyph !== ""; anchors.centerIn: parent; text: choice.glyph; color: "white"; font.pixelSize: choice.glyph.length > 2 ? 12 : 18; font.weight: Font.Bold }
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2
                Text { text: choice.title; font.pixelSize: 15; font.weight: Font.DemiBold; color: Theme.text }
                Text { Layout.fillWidth: true; text: choice.detail; font.pixelSize: 12; color: Theme.secondary; elide: Text.ElideRight }
            }
            Icon { name: "chevronRight"; width: 16; height: 16; color: Theme.tertiary }
        }
        MouseArea { id: area; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: choice.picked() }
    }

    Flickable {
        anchors.fill: parent
        contentHeight: col.implicitHeight + 80
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        ColumnLayout {
            id: col
            width: Math.min(620, guide.width - 60)
            x: (guide.width - width) / 2
            y: 44
            spacing: 14

            RowLayout {
                spacing: 10
                Repeater {
                    model: [["mail", "#2f7cf6"], ["calendar", "#e8453c"], ["contacts", "#8e8e93"]]
                    delegate: Rectangle {
                        required property var modelData
                        width: 54; height: 54; radius: 14
                        color: modelData[1]
                        Icon { anchors.centerIn: parent; name: modelData[0]; color: "white"; width: 30; height: 30 }
                    }
                }
            }
            Text { text: "Welcome to Almanac"; font.pixelSize: 30; font.weight: Font.Bold; color: Theme.text }
            Text {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                font.pixelSize: 15
                color: Theme.secondary
                text: "Mail, Calendar and Contacts in one window. Add the accounts you use and everything stays in sync: "
                      + "read, sort and send mail, keep your calendars and address books up to date, and get alerts."
            }

            Text { text: "1.  Add your accounts"; font.pixelSize: 17; font.weight: Font.DemiBold; color: Theme.text; Layout.topMargin: 10 }
            // Every provider Accounts offers (as Apple Mail's "Choose a Mail account
            // provider"), then any other.
            Repeater {
                model: JSON.parse(guide.store.mailProviders()).filter(function(p) { return p.how !== "none" })
                delegate: Choice {
                    required property var modelData
                    icon: "mail"
                    glyph: modelData.glyph || ""
                    tint: modelData.tint
                    title: modelData.name
                    detail: modelData.detail + (modelData.how === "oauth" ? " — sign in on " + modelData.name + "'s own page" : "")
                    onPicked: { guide.accountsDialog.open(); guide.accountsDialog.pickProvider(modelData.id) }
                }
            }
            Choice {
                icon: "mail"; tint: "#8e8e93"
                title: "Other Mail Account"
                detail: "Any other provider, or your own server: its settings are looked up for you"
                onPicked: guide.openAccounts("add-other")
            }
            Repeater {
                model: guide.sources.filter(function(s) { return s.found })
                delegate: Choice {
                    required property var modelData
                    icon: "import"; tint: modelData.id === "thunderbird" ? "#2c6fd1" : "#36b5a4"
                    title: "Bring in mail from " + modelData.name
                    detail: modelData.counts.messages > 0 ? "Imported " + modelData.counts.messages + " messages" : "A copy of what " + modelData.name + " keeps on this computer"
                    onPicked: guide.store.importSource(modelData.id)
                }
            }

            // What's been added so far
            Text {
                visible: guide.accounts.length > 0
                text: "Added"
                font.pixelSize: 13; font.weight: Font.DemiBold; color: Theme.secondary; Layout.topMargin: 4
            }
            Repeater {
                model: guide.accounts
                delegate: RowLayout {
                    required property var modelData
                    Layout.fillWidth: true
                    Icon { name: modelData.lastError ? "close" : "check"; width: 16; height: 16; color: modelData.lastError ? Theme.red : "#34a853" }
                    Text { text: modelData.email; color: Theme.text; font.weight: Font.DemiBold }
                    Text {
                        Layout.fillWidth: true
                        text: modelData.lastError ? modelData.lastError : (modelData.lastSync ? modelData.messages + " messages · " + modelData.events + " events · " + modelData.contacts + " contacts" : "syncing…")
                        color: modelData.lastError ? Theme.red : Theme.secondary
                        font.pixelSize: 12
                        elide: Text.ElideRight
                    }
                }
            }
            RowLayout {
                visible: guide.store.busy
                BusyIndicator { running: guide.store.busy; Layout.preferredWidth: 22; Layout.preferredHeight: 22 }
                Text { Layout.fillWidth: true; text: guide.store.statusText; color: Theme.secondary; font.pixelSize: 12; elide: Text.ElideRight }
            }

            Text { text: "2.  Good to know"; font.pixelSize: 17; font.weight: Font.DemiBold; color: Theme.text; Layout.topMargin: 10 }
            Text {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: Theme.secondary
                font.pixelSize: 13
                text: "• Passwords are kept in this computer's keychain.\n"
                      + "• Many providers want an app password instead of your normal one; the account form says where to get it.\n"
                      + "• Switch between Mail, Calendar and Contacts at the top, or with Ctrl+1, 2, 3. The ☰ menu has every command and Settings.\n"
                      + "• No account? Everything also works on this computer only (\"On My Computer\")."
            }

            RowLayout {
                Layout.fillWidth: true
                Layout.topMargin: 14
                ToolButtonX { text: "Try with Sample Data"; tip: "Fill the app with made-up examples to look around"; onClicked: guide.store.useSampleData() }
                Item { Layout.fillWidth: true }
                ToolButtonX { text: guide.accounts.length ? "Done" : "Skip for Now"; primary: guide.accounts.length > 0; onClicked: guide.store.finishSetup() }
            }
        }
    }
}
