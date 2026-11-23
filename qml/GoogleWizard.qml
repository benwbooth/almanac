import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// Google's one-time setup, one page at a time: Google only lets an app sign
// people in with an OAuth client from its owner's Google Cloud project, so
// this walks through making one. Each step opens its Google Cloud page in
// the browser (once, as the step is shown; again on request), says what to
// do in plain words and shows a drawing of the page with what to use ringed.
// The last step takes the client's ID and secret and signs in.
ColumnLayout {
    id: wiz
    property var dlg
    property var store
    // The step shown: the sheet's gStep (0 there is the sign-in page).
    readonly property int step: dlg ? Math.max(1, dlg.gStep) : 1
    // The project the pages should show, when Almanac knows it.
    property string project: ""
    readonly property int count: steps.length - 1
    spacing: 10

    function u(path) { return "https://console.cloud.google.com/" + path + (project === "" ? "" : (path.indexOf("?") >= 0 ? "&" : "?") + "project=" + project) }
    function api(id, name, what, why) {
        return { url: u("apis/library/" + id),
                 say: why + " Check your Almanac project is picked at the top of the page, then click Enable. (Says Manage or API Enabled instead? It's already on: press Next.)",
                 address: "console.cloud.google.com/apis/library/" + id, heading: name,
                 rows: [{ kind: "text", label: "Google Enterprise API · " + what }, { kind: "button", label: "ENABLE", hot: true }] }
    }
    readonly property var steps: [
        null,
        { url: "https://console.cloud.google.com/projectcreate",
          say: "Make a Google Cloud project for Almanac: type a name (Almanac is fine), then click Create. It's free. If Google asks you to agree to its terms first, tick the box and continue. Made one before? Skip this and pick it at the top of the next pages.",
          address: "console.cloud.google.com/projectcreate", heading: "New Project",
          rows: [{ kind: "field", label: "Project name *", value: "Almanac", hot: true }, { kind: "field", label: "Location", value: "No organization" }, { kind: "button", label: "CREATE", hot: true }] },
        api("gmail.googleapis.com", "Gmail API", "View and manage Gmail mailbox data.", "Turn on Gmail, so Almanac can read and send your mail."),
        api("calendar-json.googleapis.com", "Google Calendar API", "Integrate with Google Calendar.", "Turn on Google Calendar, so Almanac can show and change your calendars."),
        api("people.googleapis.com", "People API", "Read and manage the user's contacts.", "Turn on Contacts (Google calls it the People API)."),
        api("drive.googleapis.com", "Google Drive API", "Create and manage resources in Google Drive.", "Turn on Google Drive. Almanac uses it only to send big attachments as links, and only sees files it put there."),
        { url: u("auth/overview/create"),
          say: "Set up the sign-in screen. It has four parts, each ending with Next: fill in what's ringed. Your email comes twice (support email, then contact email). At the end tick the agreement, then Continue and Create. (Shows an Overview instead? It's done already: press Next.)",
          address: "console.cloud.google.com/auth/overview/create", heading: "Project configuration",
          rows: [{ kind: "field", label: "1 · App Information: App name *", value: "Almanac", hot: true }, { kind: "field", label: "1 · User support email *", value: "you@gmail.com", hot: true },
                 { kind: "choice", label: "2 · Audience: External", value: true, hot: true },
                 { kind: "field", label: "3 · Contact Information: Email addresses *", value: "you@gmail.com", hot: true },
                 { kind: "choice", label: "4 · Finish: I agree to the Google API Services: User Data Policy", value: true, hot: true }] },
        { url: u("auth/audience"),
          say: "Let your Google account use it: under Test users click Add users, type your Gmail address (more than one account? add each), then Save. Leave it in Testing: don't press Publish app. In Testing Google signs Almanac out every 7 days; the sidebar then shows a Sign In Again button (one click, your mail is kept).",
          address: "console.cloud.google.com/auth/audience", heading: "Audience",
          rows: [{ kind: "text", label: "Publishing status: Testing" }, { kind: "text", label: "Test users" }, { kind: "button", label: "+ ADD USERS", hot: true },
                 { kind: "field", label: "Add users", value: "you@gmail.com", hot: true }, { kind: "button", label: "SAVE", hot: true }] },
        { url: u("auth/clients/create"),
          say: "Make Almanac's sign-in key: Application type Desktop app, name Almanac, then Create. A box shows its Client ID and Client secret: keep it open for the next step.",
          address: "console.cloud.google.com/auth/clients/create", heading: "Create OAuth client ID",
          rows: [{ kind: "field", label: "Application type *", value: "Desktop app", hot: true }, { kind: "field", label: "Name *", value: "Almanac" }, { kind: "button", label: "CREATE", hot: true }] },
        { url: u("auth/clients"),
          say: "Copy the Client ID and the Client secret from that box (or from the client's page under Clients) into the boxes below, then Save and Sign In. Or press Download JSON in the box and load that file here.",
          address: "console.cloud.google.com/auth/clients", heading: "OAuth client created",
          rows: [{ kind: "field", label: "Client ID", value: "123456789-abc….apps.googleusercontent.com", hot: true }, { kind: "field", label: "Client secret", value: "GOCSPX-…", hot: true },
                 { kind: "link", label: "Download JSON" }, { kind: "button", label: "OK" }] }
    ]
    readonly property var cur: steps[step] || null

    // Each step's page opens by itself the first time it's shown.
    property var opened: ({})
    function openPage() { if (cur && store.screenshotPath === "") Qt.openUrlExternally(cur.url) }
    function shown() {
        if (!visible || !cur || opened[step]) return
        var o = opened; o[step] = true; opened = o
        openPage()
    }
    onStepChanged: shown()
    onVisibleChanged: shown()

    Text {
        text: "Set up Google sign-in — step " + wiz.step + " of " + wiz.count
        font.pixelSize: 17; font.weight: Font.Bold; color: Theme.text
    }
    Text { Layout.fillWidth: true; wrapMode: Text.WordWrap; text: wiz.cur ? wiz.cur.say : ""; color: Theme.text; font.pixelSize: 13; lineHeight: 1.1 }
    StepPicture {
        Layout.fillWidth: true
        Layout.preferredHeight: implicitHeight
        address: wiz.cur ? wiz.cur.address : ""
        heading: wiz.cur ? wiz.cur.heading : ""
        rows: wiz.cur ? wiz.cur.rows : []
    }
    // The last step: the client's ID and secret.
    GridLayout {
        visible: wiz.step === wiz.count
        Layout.fillWidth: true
        columns: 2
        columnSpacing: 10
        rowSpacing: 6
        Text { text: "Client ID"; color: Theme.secondary; font.pixelSize: 12 }
        FieldX { id: idField; Layout.fillWidth: true; placeholderText: "…apps.googleusercontent.com" }
        Text { text: "Client secret"; color: Theme.secondary; font.pixelSize: 12 }
        FieldX { id: secretField; Layout.fillWidth: true; placeholderText: "GOCSPX-…"; echoMode: TextInput.Password }
    }
    RowLayout {
        Layout.fillWidth: true
        ToolButtonX { text: "Back"; onClicked: wiz.dlg.gStep = wiz.step - 1 }
        ToolButtonX { text: "Open This Page Again"; icon: "export"; tip: wiz.cur ? wiz.cur.url : ""; onClicked: wiz.openPage() }
        ToolButtonX { visible: wiz.step === wiz.count; text: "Load File…"; icon: "import"; tip: "The client_secret_….json file from Download JSON"; onClicked: wiz.dlg.loadFile() }
        Item { Layout.fillWidth: true }
        ToolButtonX { visible: wiz.step < wiz.count; text: "Next"; primary: true; onClicked: wiz.dlg.gStep = wiz.step + 1 }
        ToolButtonX {
            visible: wiz.step === wiz.count
            text: "Save and Sign In"
            primary: true
            enabled: idField.text.trim().indexOf(".apps.googleusercontent.com") > 0 && secretField.text.trim() !== ""
            onClicked: {
                wiz.store.saveOauthClient("google", idField.text.trim(), secretField.text.trim())
                idField.text = ""; secretField.text = ""
                Qt.callLater(wiz.dlg.signIn)
            }
        }
    }
}
