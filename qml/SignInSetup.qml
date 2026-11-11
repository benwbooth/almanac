import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtQuick.Dialogs

// Sign-in with Google, Microsoft, Yahoo or AOL: registers Almanac with the
// provider once (step by step), then signs in on the provider's own page.
Popup {
    id: dlg
    property var store
    // Accounts (where the services that don't need this setup are added).
    property var accountsDialog
    property string provider: "google"
    property var all: { store.revision; return visible ? JSON.parse(store.signInSetup()) : [] }
    property var info: { for (var i = 0; i < all.length; i++) if (all[i].id === provider) return all[i]; return null }
    property bool editingIds: false
    property bool byHand: false
    // Steps ticked off this session ("provider:index"); Google's first four
    // are done once "Prepare Automatically" has run.
    property var done: ({})
    function stepDone(i) {
        if (done[provider + ":" + i]) return true
        if (provider === "google" && store.googleProject !== "" && i <= 3) return true
        if (info && info.ready && info.steps && i < info.steps.length - 1) return true
        return false
    }
    function toggleDone(i) { var d = JSON.parse(JSON.stringify(done)); d[provider + ":" + i] = !d[provider + ":" + i]; done = d }
    function markDone(i) { var d = JSON.parse(JSON.stringify(done)); d[provider + ":" + i] = true; done = d }
    function doStep(i, s) {
        if (s.url) { Qt.openUrlExternally(s.url); markDone(i) }
        else if (s.action === "load") jsonPicker.createObject(dlg).open()
        else if (s.action === "paste") { pasteBox.forceActiveFocus(); markDone(i) }
        else if (s.action === "signin" && info && info.ready) signIn()
    }
    // After the automatic part, show the remaining steps.
    Connections { target: dlg.store; function onGoogleProjectChanged() { if (dlg.store.googleProject !== "") dlg.byHand = true } }
    property var tools: { store.revision; return visible ? JSON.parse(store.setupTools()) : ({}) }

    // Google's downloaded client file (client_secret_….json)
    Component {
        id: jsonPicker
        FileDialog {
            title: "Load the downloaded file"
            nameFilters: ["Google client file (*.json)", "All files (*)"]
            onAccepted: {
                var text = dlg.store.readTextFile(selectedFile.toString())
                if (dlg.store.usePastedRegistration(dlg.provider, text)) Qt.callLater(dlg.signIn)
                destroy()
            }
            onRejected: destroy()
        }
    }
    modal: true
    anchors.centerIn: Overlay.overlay
    width: 640
    height: Math.min(parent ? parent.height - 60 : 680, 680)
    padding: 0
    background: Rectangle { radius: 14; color: Theme.popup; border.color: Theme.border }

    /// Opens on a provider; signs in straight away when it's already set up.
    function openFor(p, signInNow) {
        provider = p
        editingIds = false
        open()
        Qt.callLater(function() {
            clientId.text = dlg.info && dlg.info.source === "saved" ? dlg.info.clientId : ""
            clientSecret.text = ""
            if (signInNow && dlg.info && dlg.info.ready) dlg.signIn()
        })
    }
    function showOthers() { scroller.contentY = Math.max(0, Math.min(otherCard.y, scroller.contentHeight - scroller.height)) }
    function signIn() { pasted.text = ""; store.signInWith(provider, "", "", "common") }
    readonly property bool waiting: store.signingIn === provider

    Connections {
        target: dlg.store
        // Close once the account has been added.
        function onSigningInChanged() { if (dlg.store.signingIn === "" && dlg.visible && dlg.store.statusText.indexOf("Signed in as") === 0) dlg.close() }
    }

    contentItem: ColumnLayout {
        spacing: 0
        // Provider tabs
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 58
            color: Theme.toolbar
            radius: 14
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 14; color: Theme.toolbar }
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 18
                anchors.rightMargin: 12
                Text { text: "Sign in with"; color: Theme.secondary; font.pixelSize: 13 }
                Repeater {
                    model: dlg.all
                    delegate: Rectangle {
                        required property var modelData
                        Layout.preferredHeight: 32
                        Layout.preferredWidth: tabRow.implicitWidth + 22
                        radius: 8
                        color: dlg.provider === modelData.id ? Theme.segmentOn : (tabArea.containsMouse ? Theme.hover : "transparent")
                        Row {
                            id: tabRow
                            anchors.centerIn: parent
                            spacing: 6
                            Text { text: modelData.name; color: Theme.text; font.pixelSize: 13; font.weight: dlg.provider === modelData.id ? Font.DemiBold : Font.Normal }
                            Text { visible: modelData.ready; text: "✓"; color: "#34a853"; font.pixelSize: 13 }
                        }
                        MouseArea { id: tabArea; anchors.fill: parent; hoverEnabled: true; onClicked: dlg.openFor(modelData.id, false) }
                    }
                }
                // Every other service: no setup needed (shown at the end of the page).
                Rectangle {
                    Layout.preferredHeight: 32
                    Layout.preferredWidth: otherTab.implicitWidth + 22
                    radius: 8
                    color: otherArea.containsMouse ? Theme.hover : "transparent"
                    Text { id: otherTab; anchors.centerIn: parent; text: "Other services…"; color: Theme.accent; font.pixelSize: 13 }
                    MouseArea { id: otherArea; anchors.fill: parent; hoverEnabled: true; onClicked: dlg.showOthers() }
                }
                Item { Layout.fillWidth: true }
                ToolButtonX { icon: "close"; onClicked: dlg.close() }
            }
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border }
        }

        Flickable {
            id: scroller
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            contentHeight: body.implicitHeight + 40
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}
            ColumnLayout {
                id: body
                x: 24; y: 20
                width: dlg.width - 48
                spacing: 12

                Text { text: dlg.info ? dlg.info.name : ""; font.pixelSize: 22; font.weight: Font.Bold; color: Theme.text }
                Text { Layout.fillWidth: true; wrapMode: Text.WordWrap; text: dlg.info ? dlg.info.detail : ""; color: Theme.secondary }

                // ------------------------------------------ ready: sign in
                Rectangle {
                    visible: !!dlg.info && dlg.info.ready && !dlg.editingIds
                    Layout.fillWidth: true
                    Layout.preferredHeight: readyCol.implicitHeight + 28
                    radius: 10
                    color: Theme.card
                    border.color: Theme.borderSoft
                    ColumnLayout {
                        id: readyCol
                        x: 14; y: 14
                        width: parent.width - 28
                        spacing: 10
                        RowLayout {
                            Text { text: "✓"; color: "#34a853"; font.pixelSize: 18 }
                            Text {
                                Layout.fillWidth: true
                                wrapMode: Text.WordWrap
                                color: Theme.text
                                text: "Almanac is set up for " + (dlg.info ? dlg.info.name : "") + " sign-in"
                                      + (dlg.info && dlg.info.source === "built in" ? " (comes with this copy)." : ".")
                            }
                        }
                        RowLayout {
                            ToolButtonX {
                                text: dlg.waiting ? "Waiting for your browser…" : "Sign In with " + (dlg.info ? dlg.info.name : "")
                                primary: true
                                enabled: !dlg.waiting && !dlg.store.busy
                                onClicked: dlg.signIn()
                            }
                            Item { Layout.fillWidth: true }
                            ToolButtonX { visible: !!dlg.info && dlg.info.source === "saved"; text: "Change App ID…"; onClicked: dlg.editingIds = true }
                        }
                    }
                }

                // Google's "Access blocked … has not completed the Google verification process"
                Rectangle {
                    visible: dlg.provider === "google" && !!dlg.info && (dlg.info.ready || dlg.store.googleProject !== "")
                    Layout.fillWidth: true
                    Layout.preferredHeight: gHelp.implicitHeight + 28
                    radius: 10
                    color: Theme.card
                    border.color: Theme.orange
                    ColumnLayout {
                        id: gHelp
                        x: 14; y: 14
                        width: parent.width - 28
                        spacing: 8
                        Text {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            font.weight: Font.DemiBold
                            color: Theme.text
                            text: "Google says “Access blocked: Almanac has not completed the Google verification process”?"
                        }
                        Text {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            color: Theme.secondary
                            font.pixelSize: 13
                            lineHeight: 1.15
                            text: "Google only lets the people on your app's Test users list sign in, and only in the project the sign-in key came from. "
                                  + (dlg.info && dlg.info.keyProject
                                     ? "Almanac's key comes from the Google Cloud project numbered " + dlg.info.keyProject + ". "
                                     : "")
                                  + "Check each of these:"
                        }
                        Repeater {
                            model: [
                                "Press Open the Audience Page below. It opens the project Almanac's key came from. If you made more than one project, this may not be the one you added yourself to.",
                                "Publishing status should say Testing. Don't press Publish app, which asks for a homepage and privacy policy.",
                                "Under Test users, your exact Gmail address must be listed. If it isn't, press + Add users, type it and press Save. Watch for typos, and use the full address with @gmail.com.",
                                "Press Sign In Again. Google now always asks which account to use. Pick the address you just listed, not another account that's signed in.",
                                "When Google says it hasn't verified the app, press Advanced, then Go to Almanac (unsafe).",
                                "Still blocked? Let Almanac do it all with Set Up in My Browser (above), or press Load Another Key and do steps 5 to 8 of the guide in the project where you added yourself."
                            ]
                            delegate: RowLayout {
                                required property string modelData
                                required property int index
                                Layout.fillWidth: true
                                spacing: 8
                                Text { Layout.alignment: Qt.AlignTop; text: (index + 1) + "."; color: Theme.text; font.pixelSize: 13; font.weight: Font.DemiBold }
                                Text { Layout.fillWidth: true; wrapMode: Text.WordWrap; text: modelData; color: Theme.text; font.pixelSize: 13 }
                            }
                        }
                        RowLayout {
                            ToolButtonX {
                                text: "Open the Audience Page"
                                primary: true
                                onClicked: Qt.openUrlExternally(dlg.info.audience)
                            }
                            ToolButtonX { visible: !!dlg.info && dlg.info.ready; text: "Sign In Again"; enabled: !dlg.store.busy; onClicked: dlg.signIn() }
                            ToolButtonX { visible: !!dlg.info && dlg.info.ready; text: "Load Another Key…"; onClicked: { dlg.store.cancelSignIn(); jsonPicker.createObject(dlg).open() } }
                        }
                    }
                }

                // While the provider's page is open in the browser
                Rectangle {
                    visible: dlg.waiting
                    Layout.fillWidth: true
                    Layout.preferredHeight: waitCol.implicitHeight + 28
                    radius: 10
                    color: Theme.accentSoft
                    ColumnLayout {
                        id: waitCol
                        x: 14; y: 14
                        width: parent.width - 28
                        spacing: 8
                        RowLayout {
                            Spinner { running: dlg.waiting; Layout.preferredWidth: 24; Layout.preferredHeight: 24 }
                            Text {
                                Layout.fillWidth: true
                                wrapMode: Text.WordWrap
                                color: Theme.text
                                text: dlg.info && dlg.info.pasteCode
                                      ? "Sign in on the " + dlg.info.name + " page in your browser. It then shows a code: copy it into the box below."
                                      : "Sign in on the " + (dlg.info ? dlg.info.name : "") + " page that opened in your browser. Almanac continues by itself when you're done."
                            }
                        }
                        Text {
                            visible: !(dlg.info && dlg.info.pasteCode)
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            color: Theme.secondary; font.pixelSize: 12
                            text: "If the page ends with an error or a code, paste the code (or the whole address from the browser) here:"
                        }
                        RowLayout {
                            FieldX { id: pasted; Layout.fillWidth: true; placeholderText: "Code from the sign-in page"; selectByMouse: true; onAccepted: okCode.clicked() }
                            ToolButtonX { id: okCode; text: "Continue"; primary: true; enabled: pasted.text.trim() !== ""; onClicked: dlg.store.submitSignInCode(pasted.text) }
                        }
                        RowLayout {
                            ToolButtonX { text: "Cancel Sign-in"; onClicked: dlg.store.cancelSignIn() }
                            ToolButtonX { text: "Start Over"; onClicked: dlg.signIn() }
                        }
                    }
                }

                // ------------------- Google: Almanac clicks through it in a browser
                Rectangle {
                    id: browserCard
                    readonly property string browser: dlg.visible ? dlg.store.browserName() : ""
                    visible: dlg.provider === "google" && !!dlg.info && !dlg.editingIds
                    Layout.fillWidth: true
                    Layout.preferredHeight: browserCol.implicitHeight + 28
                    radius: 10
                    color: Theme.accentSoft
                    border.color: Theme.accent
                    border.width: 2
                    ColumnLayout {
                        id: browserCol
                        x: 14; y: 14
                        width: parent.width - 28
                        spacing: 8
                        Text { text: "Set it all up in the browser (easiest)"; font.pixelSize: 15; font.weight: Font.DemiBold; color: Theme.text }
                        Text {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            color: Theme.text; font.pixelSize: 13; lineHeight: 1.15
                            text: browserCard.browser !== ""
                                ? "Almanac opens a new tab in your own " + browserCard.browser + ", where you're probably already signed in to Google, and does every step of the guide below by itself: "
                                  + "it makes the project, turns on Gmail, Calendar and Contacts, names the app, adds you as a test user, creates the sign-in key and signs you in.\n\n"
                                  + "It also accepts Google's terms and allows access for you. You only step in where a person must: the first time, to let Almanac "
                                  + "control the browser (tick one switch and press Allow), and if Google asks for your password, a 2-step code or a robot check. "
                                  + "A blue bar at the bottom of the page says what Almanac is doing. When it turns orange, it's your turn. "
                                  + "Safe to run again: finished steps are skipped."
                                : "This needs Google Chrome, Brave, Chromium or Microsoft Edge. Install one, then reopen this page."
                        }
                        RowLayout {
                            ToolButtonX {
                                visible: dlg.store.registering === ""
                                text: "Set Up in My " + (browserCard.browser || "Browser")
                                icon: "export"
                                primary: true
                                enabled: browserCard.browser !== ""
                                onClicked: dlg.store.setUpInBrowser("google", true)
                            }
                            ToolButtonX {
                                visible: dlg.store.registering === "" && browserCard.browser !== ""
                                text: "Use a Separate Window"
                                onClicked: dlg.store.setUpInBrowser("google", false)
                            }
                            ToolButtonX {
                                visible: dlg.store.registering === "google"
                                text: "Stop"
                                onClicked: dlg.store.cancelSignIn()
                            }
                            Spinner { visible: dlg.store.registering === "google"; running: visible; Layout.preferredWidth: 24; Layout.preferredHeight: 24 }
                            Text {
                                visible: dlg.store.registering === "google"
                                Layout.fillWidth: true
                                wrapMode: Text.WordWrap
                                text: dlg.store.statusText; color: Theme.text; font.pixelSize: 13
                            }
                        }
                    }
                }

                // ------------------------------- not set up: do it for me
                Rectangle {
                    visible: !!dlg.info && !dlg.info.ready && !dlg.editingIds && (dlg.provider === "microsoft" || dlg.provider === "google")
                    Layout.fillWidth: true
                    Layout.preferredHeight: autoCol.implicitHeight + 28
                    radius: 10
                    color: Theme.card
                    border.color: Theme.borderSoft
                    ColumnLayout {
                        id: autoCol
                        x: 14; y: 14
                        width: parent.width - 28
                        spacing: 8
                        Text { text: dlg.provider === "google" ? "Or: half-automatic, with Google's command-line tool" : "Do it for me"; font.pixelSize: 15; font.weight: Font.DemiBold; color: Theme.text }
                        Text {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            color: Theme.secondary; font.pixelSize: 12
                            text: dlg.provider === "microsoft"
                                ? "Almanac registers itself with Microsoft's command-line tool. You only sign in once in your browser; then it saves the ID and opens the normal sign-in."
                                  + (dlg.tools.az ? "" : dlg.tools.nix ? " (The tool is fetched automatically the first time.)" : " First install the Azure CLI.")
                                : "Almanac makes the Google Cloud project and turns on Gmail, Calendar and Contacts for you. After that, the guide below walks you through the few pages Google makes you click yourself: naming the app, adding yourself as a test user, and downloading the sign-in key."
                                  + (dlg.tools.gcloud ? "" : dlg.tools.nix ? " (The tool is fetched automatically the first time.)" : " First install the Google Cloud CLI.")
                        }
                        RowLayout {
                            ToolButtonX {
                                visible: dlg.store.registering === ""
                                text: dlg.provider === "microsoft" ? "Register Automatically" : "Prepare Automatically"
                                primary: true
                                enabled: !!(dlg.provider === "microsoft" ? (dlg.tools.az || dlg.tools.nix) : (dlg.tools.gcloud || dlg.tools.nix))
                                onClicked: dlg.store.autoRegister(dlg.provider)
                            }
                            ToolButtonX {
                                visible: !(dlg.provider === "microsoft" ? (dlg.tools.az || dlg.tools.nix) : (dlg.tools.gcloud || dlg.tools.nix))
                                text: "Get the tool"
                                onClicked: Qt.openUrlExternally(dlg.provider === "microsoft" ? "https://aka.ms/azcli" : "https://cloud.google.com/sdk/docs/install")
                            }
                            Spinner { visible: dlg.store.registering === dlg.provider; running: visible; Layout.preferredWidth: 24; Layout.preferredHeight: 24 }
                            Text {
                                visible: dlg.store.registering === dlg.provider
                                Layout.fillWidth: true
                                wrapMode: Text.WordWrap
                                text: dlg.store.statusText; color: Theme.text; font.pixelSize: 12
                            }
                        }
                        // Google: what's left after the automatic part.
                        Text {
                            visible: dlg.provider === "google" && dlg.store.googleProject !== ""
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            color: "#34a853"
                            font.weight: Font.DemiBold
                            text: "✓ Project " + dlg.store.googleProject + " is ready and Gmail, Calendar and Contacts are on. Steps 1 to 4 below are done. Carry on from step 5."
                        }
                    }
                }

                // ------------------------------------------ ask an AI
                Rectangle {
                    visible: !!dlg.info && !dlg.info.ready && !dlg.editingIds
                    Layout.fillWidth: true
                    Layout.preferredHeight: aiCol.implicitHeight + 28
                    radius: 10
                    color: Theme.card
                    border.color: Theme.borderSoft
                    ColumnLayout {
                        id: aiCol
                        x: 14; y: 14
                        width: parent.width - 28
                        spacing: 8
                        Text { text: "Ask an AI assistant"; font.pixelSize: 15; font.weight: Font.DemiBold; color: Theme.text }
                        Text {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            color: Theme.secondary; font.pixelSize: 12
                            text: "Copy ready-made instructions and give them to an AI assistant that can use your web browser (for example Claude in Chrome). "
                                  + "It does the clicking in your signed-in " + (dlg.info ? dlg.info.name : "") + " account and answers with the IDs. Paste its answer below."
                        }
                        RowLayout {
                            ToolButtonX {
                                text: copied ? "Copied ✓" : "Copy Instructions for an AI"
                                property bool copied: false
                                onClicked: { clip.text = dlg.store.aiPrompt(dlg.provider); clip.selectAll(); clip.copy(); copied = true }
                            }
                            ToolButtonX { text: "Show Them"; onClicked: { pasteBox.text = dlg.store.aiPrompt(dlg.provider); showingPrompt = true } property bool showingPrompt: false }
                        }
                        TextEdit { id: clip; visible: false }
                    }
                }

                // ------------------------------------- paste anything with IDs
                ColumnLayout {
                    visible: !!dlg.info && (!dlg.info.ready || dlg.editingIds)
                    Layout.fillWidth: true
                    spacing: 6
                    Text { text: "Paste the IDs, the AI's answer or the downloaded file here:"; color: Theme.text; font.pixelSize: 12 }
                    ScrollView {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 90
                        TextArea {
                            id: pasteBox
                            wrapMode: TextEdit.Wrap
                            selectByMouse: true
                            placeholderText: "client_id=…\nclient_secret=…"
                            background: Rectangle { radius: 6; color: Theme.field; border.color: Theme.border }
                        }
                    }
                    RowLayout {
                        ToolButtonX { text: "Load File…"; onClicked: jsonPicker.createObject(dlg).open() }
                        Item { Layout.fillWidth: true }
                        ToolButtonX {
                            text: "Use This and Sign In"
                            primary: true
                            enabled: pasteBox.text.trim() !== ""
                            onClicked: if (dlg.store.usePastedRegistration(dlg.provider, pasteBox.text)) { pasteBox.text = ""; dlg.editingIds = false; Qt.callLater(dlg.signIn) }
                        }
                    }
                }

                ToolButtonX {
                    visible: !!dlg.info && !dlg.info.ready && !dlg.editingIds
                    text: dlg.byHand ? "Hide the step-by-step guide" : "Show the step-by-step guide (do it by hand)…"
                    onClicked: dlg.byHand = !dlg.byHand
                }

                // -------------------------------------- by hand: steps
                ColumnLayout {
                    id: guide
                    // Bring the guide into view when it opens.
                    onVisibleChanged: if (visible) scrollTimer.restart()
                    Timer { id: scrollTimer; interval: 50; onTriggered: scroller.contentY = Math.max(0, Math.min(guide.y + 20 - 8, scroller.contentHeight - scroller.height)) }
                    visible: !!dlg.info && ((!dlg.info.ready && (dlg.byHand || dlg.store.googleProject !== "" && dlg.provider === "google")) || dlg.editingIds)
                    Layout.fillWidth: true
                    spacing: 10
                    Text {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        color: Theme.text
                        text: "First, register Almanac with " + (dlg.info ? dlg.info.name : "") + " (once, about 10 minutes; free). "
                              + "Apps like Thunderbird come with this done by their makers. After this, signing in any "
                              + (dlg.info ? dlg.info.name : "") + " account is one click, and your password is only ever typed on "
                              + (dlg.info ? dlg.info.name : "") + "'s own page."
                    }
                    Text {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        color: Theme.secondary
                        font.pixelSize: 12
                        text: "Click a step to open the right page. Every step is safe to repeat, so if you're not sure whether you did one, just do it again."
                    }
                    Repeater {
                        model: dlg.info ? dlg.info.steps : []
                        delegate: Rectangle {
                            id: step
                            required property var modelData
                            required property int index
                            readonly property bool isDone: dlg.stepDone(index)
                            // Finished steps fold up to their title; click the title to reopen.
                            property bool reopened: false
                            readonly property bool folded: isDone && !reopened
                            readonly property string kind: modelData.url ? "open" : (modelData.action || "read")
                            readonly property bool clickable: kind === "open" || kind === "load" || kind === "paste" || (kind === "signin" && !!dlg.info && dlg.info.ready)
                            Layout.fillWidth: true
                            Layout.preferredHeight: stepRow.implicitHeight + 24
                            radius: 10
                            color: stepArea.containsMouse && clickable ? Theme.hover : Theme.card
                            border.color: stepArea.containsMouse && clickable ? Theme.accent : Theme.borderSoft
                            border.width: stepArea.containsMouse && clickable ? 2 : 1
                            RowLayout {
                                id: stepRow
                                x: 12; y: 12
                                width: parent.width - 24
                                spacing: 12
                                // Number, or a tick once done (click it to change)
                                Rectangle {
                                    Layout.alignment: Qt.AlignTop
                                    width: 26; height: 26; radius: 13
                                    color: step.isDone ? "#1e7e34" : Theme.accentFill
                                    Text { anchors.centerIn: parent; text: step.isDone ? "✓" : step.index + 1; color: "white"; font.pixelSize: 13; font.weight: Font.Bold }
                                    MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: dlg.toggleDone(step.index) }
                                    ToolTip.visible: false
                                }
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 6
                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 10
                                        Text {
                                            Layout.fillWidth: true
                                            Layout.alignment: Qt.AlignVCenter
                                            wrapMode: Text.WordWrap
                                            text: step.modelData.title + (step.isDone ? (step.folded ? "  <font color='" + Theme.secondary + "'>(done · show again ▸)</font>" : "  <font color='" + Theme.secondary + "'>(done · hide ▾)</font>") : "")
                                            textFormat: Text.StyledText
                                            font.pixelSize: 15
                                            font.weight: Font.DemiBold
                                            color: Theme.text
                                            MouseArea {
                                                anchors.fill: parent
                                                enabled: step.isDone
                                                cursorShape: Qt.PointingHandCursor
                                                onClicked: step.reopened = !step.reopened
                                            }
                                        }
                                        ToolButtonX {
                                            Layout.alignment: Qt.AlignVCenter
                                            visible: step.kind !== "read"
                                            enabled: step.clickable
                                            primary: !step.isDone
                                            icon: step.kind === "open" ? "export" : (step.kind === "load" ? "import" : "")
                                            text: ({ open: "Open page ›", load: "Choose File…", paste: "Paste Below ↓", signin: "Sign In" })[step.kind] || ""
                                            onClicked: dlg.doStep(step.index, step.modelData)
                                        }
                                    }
                                    Text {
                                        visible: !step.folded
                                        Layout.fillWidth: true
                                        wrapMode: Text.WordWrap
                                        text: step.modelData.detail
                                        font.pixelSize: 13
                                        lineHeight: 1.15
                                        color: Theme.secondary
                                    }
                                    // Baby steps: one action per line.
                                    Repeater {
                                        model: step.folded ? [] : (step.modelData.substeps || [])
                                        delegate: RowLayout {
                                            required property string modelData
                                            required property int index
                                            Layout.fillWidth: true
                                            Layout.topMargin: 2
                                            spacing: 8
                                            Rectangle {
                                                Layout.alignment: Qt.AlignTop
                                                Layout.topMargin: 1
                                                width: 18; height: 18; radius: 9
                                                color: "transparent"
                                                border.color: Theme.accent
                                                Text { anchors.centerIn: parent; text: String.fromCharCode(97 + index); color: Theme.accent; font.pixelSize: 11; font.weight: Font.Bold }
                                            }
                                            Text {
                                                Layout.fillWidth: true
                                                wrapMode: Text.WordWrap
                                                text: modelData
                                                font.pixelSize: 13
                                                lineHeight: 1.15
                                                color: Theme.text
                                            }
                                        }
                                    }
                                    Text {
                                        visible: !!step.modelData.done && !step.folded
                                        Layout.fillWidth: true
                                        Layout.topMargin: 4
                                        wrapMode: Text.WordWrap
                                        text: "✓ Done when: " + (step.modelData.done || "")
                                        font.pixelSize: 13
                                        color: Theme.dark ? "#6fd08a" : "#1e8e3e"
                                    }
                                    Rectangle {
                                        visible: !!step.modelData.trouble && !step.folded
                                        Layout.fillWidth: true
                                        Layout.topMargin: 4
                                        Layout.preferredHeight: troubleText.implicitHeight + 16
                                        radius: 6
                                        color: Theme.dark ? "#3a2f1c" : "#fff4e0"
                                        border.color: Theme.orange
                                        Text {
                                            id: troubleText
                                            x: 10; y: 8
                                            width: parent.width - 20
                                            wrapMode: Text.WordWrap
                                            text: "<b>Stuck?</b> " + (step.modelData.trouble || "")
                                            textFormat: Text.StyledText
                                            font.pixelSize: 13
                                            lineHeight: 1.15
                                            color: Theme.text
                                        }
                                    }
                                }
                            }
                            MouseArea {
                                id: stepArea
                                anchors.fill: parent
                                z: -1
                                hoverEnabled: true
                                cursorShape: step.clickable ? Qt.PointingHandCursor : Qt.ArrowCursor
                                onClicked: if (step.clickable) dlg.doStep(step.index, step.modelData)
                            }
                        }
                    }
                    FieldX { id: clientId; Layout.fillWidth: true; placeholderText: "Client ID"; selectByMouse: true }
                    FieldX {
                        id: clientSecret
                        visible: !!dlg.info && dlg.info.needsSecret
                        Layout.fillWidth: true
                        placeholderText: dlg.info && dlg.info.hasSecret ? "Client secret (leave empty to keep)" : "Client secret"
                        echoMode: TextInput.Password
                        selectByMouse: true
                    }
                    RowLayout {
                        ToolButtonX {
                            visible: !!dlg.info && dlg.info.source === "saved"
                            text: "Forget App ID"
                            onClicked: { dlg.store.saveOauthClient(dlg.provider, "", ""); dlg.editingIds = false }
                        }
                        Item { Layout.fillWidth: true }
                        ToolButtonX { visible: dlg.editingIds; text: "Cancel"; onClicked: dlg.editingIds = false }
                        ToolButtonX {
                            text: "Save and Sign In"
                            primary: true
                            enabled: clientId.text.trim() !== "" && (!dlg.info.needsSecret || clientSecret.text.trim() !== "" || dlg.info.hasSecret)
                            onClicked: {
                                var secret = clientSecret.text.trim() !== "" ? clientSecret.text : ""
                                dlg.store.saveOauthClient(dlg.provider, clientId.text, secret)
                                dlg.editingIds = false
                                Qt.callLater(dlg.signIn)
                            }
                        }
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    visible: dlg.store.statusText !== ""
                    Text { Layout.fillWidth: true; wrapMode: Text.WordWrap; text: dlg.store.statusText; color: Theme.secondary; font.pixelSize: 12 }
                }

                // ------------------------------------------ every other service
                Rectangle {
                    id: otherCard
                    Layout.fillWidth: true
                    Layout.topMargin: 8
                    Layout.preferredHeight: otherCol.implicitHeight + 28
                    radius: 10
                    color: Theme.card
                    border.color: Theme.borderSoft
                    ColumnLayout {
                        id: otherCol
                        x: 14; y: 14
                        width: parent.width - 28
                        spacing: 10
                        Text { text: "Other mail services"; font.pixelSize: 15; font.weight: Font.DemiBold; color: Theme.text }
                        Text {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            color: Theme.secondary; font.pixelSize: 13
                            text: "Only Google, Microsoft, Yahoo and AOL need this setup (they sign in on their own page). Every other service "
                                  + "signs in with your address and a password (often an app password from its security settings); Almanac fills in its servers."
                        }
                        Flow {
                            Layout.fillWidth: true
                            spacing: 8
                            Repeater {
                                model: JSON.parse(dlg.store.mailProviders()).filter(function(p) { return p.how !== "oauth" && p.how !== "none" })
                                delegate: ToolButtonX {
                                    required property var modelData
                                    text: modelData.name
                                    tip: modelData.detail
                                    onClicked: { dlg.close(); if (dlg.accountsDialog) { dlg.accountsDialog.open(); dlg.accountsDialog.pickProvider(modelData.id) } }
                                }
                            }
                            ToolButtonX {
                                text: "Any other…"
                                onClicked: { dlg.close(); if (dlg.accountsDialog) { dlg.accountsDialog.open(); dlg.accountsDialog.newAccount() } }
                            }
                        }
                    }
                }
            }
        }
    }
}
