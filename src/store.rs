//! The bridge between Rust and the Qt Quick interface. One `Store` object
//! holds all mail, calendar and contact data; QML calls its methods and
//! re-reads whatever it shows when `revision` changes.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    unsafe extern "C++" {
        include!("craft_shot.h");
        #[cxx_name = "craftSaveWindow"]
        fn craft_save_window(path: &QString) -> bool;
        #[cxx_name = "craftPrintPdf"]
        fn craft_print_pdf(html: &QString, path: &QString) -> bool;
        #[cxx_name = "craftPrintDialog"]
        fn craft_print_dialog(html: &QString, title: &QString) -> bool;
        #[cxx_name = "craftPrintPdfFile"]
        fn craft_print_pdf_file(path: &QString, title: &QString) -> bool;
        #[cxx_name = "craftPdfPage"]
        fn craft_pdf_page(path: &QString, index: i32, out: &QString, width: i32) -> i32;
        #[cxx_name = "craftCreateApp"]
        fn craft_create_app(plugins: &QString, session: &QString);
        #[cxx_name = "craftSetAppIcon"]
        fn craft_set_app_icon(path: &QString);
        #[cxx_name = "craftSessionId"]
        fn craft_session_id() -> QString;
        #[cxx_name = "craftShowWindows"]
        fn craft_show_windows();
    }

    unsafe extern "C++" {
        include!(<QtWebEngineQuick/qtwebenginequickglobal.h>);
        /// Prepares Qt WebEngine (shows mail like a browser); call before the app exists.
        #[namespace = "QtWebEngineQuick"]
        #[cxx_name = "initialize"]
        fn webengine_initialize();
        #[cxx_name = "craftExec"]
        fn craft_exec() -> i32;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(i32, revision)]
        #[qproperty(QString, screenshot_path, cxx_name = "screenshotPath")]
        #[qproperty(i32, start_section, cxx_name = "startSection")]
        #[qproperty(QString, theme)]
        #[qproperty(QString, start_panel, cxx_name = "startPanel")]
        /// A web page or file the window should open (sign-in pages, attachments, print-outs)
        #[qproperty(QString, open_url, cxx_name = "openUrl")]
        /// Id of the message that can still be un-sent (0 = none)
        #[qproperty(i64, undo_id, cxx_name = "undoId")]
        /// What Edit › Undo would put back ("" when nothing): "Undo Move to Trash".
        #[qproperty(QString, mail_undo_text, cxx_name = "mailUndoText")]
        /// False until the setup guide is finished or skipped
        #[qproperty(bool, onboarded)]
        /// Provider whose sign-in page is open ("" when none)
        #[qproperty(QString, signing_in, cxx_name = "signingIn")]
        /// Provider being registered automatically ("" when none)
        #[qproperty(QString, registering)]
        /// Google Cloud project made by "Prepare automatically"
        #[qproperty(QString, google_project, cxx_name = "googleProject")]
        /// Attachments copied for forwarding: {"id": message id, "files": [paths]}
        #[qproperty(QString, attachments_ready, cxx_name = "attachmentsReady")]
        #[qproperty(bool, busy)]
        /// The mail still being read at start.
        #[qproperty(bool, loading)]
        #[qproperty(QString, status_text, cxx_name = "statusText")]
        /// Download progress (a bar at the bottom of the Mail sidebar); total 0 = none.
        /// New messages that arrived since the window was last looked at (the
        /// tray icon's green dot); the window sets it back to 0.
        #[qproperty(i32, new_mail, cxx_name = "newMail")]
        /// Goes up each time Almanac is started again while running: the
        /// window comes forward.
        #[qproperty(i32, show_requests, cxx_name = "showRequests")]
        /// A notification was clicked: what to open (JSON {mail: id} / {inbox}
        /// / {event: id, start} / {calendar}), the window brought forward.
        #[qproperty(QString, open_request, cxx_name = "openRequest")]
        /// Servers found for an address ({email, imapHost, …, how}), from `discover_account`.
        #[qproperty(QString, discovered)]
        /// The newest spelling and grammar check of the message being written
        /// ({generation, lints: [{start, end, word, kind, message, suggestions}]}).
        #[qproperty(QString, spelling)]
        #[qproperty(i32, progress_done, cxx_name = "progressDone")]
        #[qproperty(i32, progress_total, cxx_name = "progressTotal")]
        /// Calendar alerts showing until dismissed or snoozed ([{title, when}]).
        #[qproperty(QString, alerts_json, cxx_name = "alertsJson")]
        /// The latest signature check: {"id", "state", "signer", "key", "trust", "detail"}.
        #[qproperty(QString, signature_json, cxx_name = "signatureJson")]
        /// The latest decryption: {"id", "state", "body", "html", "attachments", "signature", "detail"}
        /// (in memory only: never saved).
        #[qproperty(QString, decrypted_json, cxx_name = "decryptedJson")]
        /// The latest OpenPGP key job (Settings, compose's unlock):
        /// {"op", "email", "busy", "ok", "text"}.
        #[qproperty(QString, pgp_status_json, cxx_name = "pgpStatusJson")]
        /// Attachments being uploaded to cloud storage, by file path:
        /// {path: {"state": "uploading"|"done"|"error", "sent", "total", "name", "size", "url", "provider", "error"}}.
        #[qproperty(QString, cloud_uploads_json, cxx_name = "cloudUploadsJson")]
        type Store = super::StoreRust;

        // mail
        #[qinvokable]
        fn mail_sidebar(self: &Store) -> QString;
        #[qinvokable]
        fn messages(self: &Store, selection: &QString, query: &QString, threaded: bool) -> QString;
        #[qinvokable]
        fn thread(self: &Store, selection: &QString, id: i64) -> QString;
        #[qinvokable]
        fn mail_folders(self: &Store) -> QString;
        #[qinvokable]
        fn move_to_folder(self: Pin<&mut Store>, id: i64, folder: &QString);
        #[qinvokable]
        fn mark_all_read(self: Pin<&mut Store>, selection: &QString);
        #[qinvokable]
        fn refresh_all(self: Pin<&mut Store>);
        /// Reads the next older messages of the shown mailbox from the server.
        #[qinvokable]
        fn load_older(self: Pin<&mut Store>, selection: &QString);
        /// Searches the shown mailbox on the server (finds older mail too).
        #[qinvokable]
        fn search_on_server(self: Pin<&mut Store>, selection: &QString, query: &QString);
        /// Whether the shown mailbox is on a mail server (so the above apply).
        #[qinvokable]
        fn is_server_mailbox(self: &Store, selection: &QString) -> bool;
        /// Pictures and PDFs attached to a message, ready to show inside it
        /// ([{name, mime, data (base64)}]); missing ones are fetched in the background.
        #[qinvokable]
        fn attachment_previews(self: &Store, id: i64) -> QString;
        /// Opens or closes a PDF shown in a message (closed by default: its
        /// pages are made only once it's opened). Remembered until Almanac quits.
        #[qinvokable]
        fn set_pdf_open(self: &Store, id: i64, name: &QString, open: bool);
        /// A command-line option's value ("" if not given), e.g. --width for pictures.
        #[qinvokable]
        fn arg(self: &Store, name: &QString) -> QString;
        #[qinvokable]
        fn message(self: &Store, id: i64) -> QString;
        #[qinvokable]
        fn set_read(self: Pin<&mut Store>, id: i64, read: bool);
        #[qinvokable]
        fn toggle_flag(self: Pin<&mut Store>, id: i64);
        /// Flags in a colour (0 red … 6 gray), or -1: not flagged.
        #[qinvokable]
        fn set_flag_color(self: Pin<&mut Store>, id: i64, color: i32);
        #[qinvokable]
        fn move_to(self: Pin<&mut Store>, id: i64, kind: &QString);
        #[qinvokable]
        fn delete_message(self: Pin<&mut Store>, id: i64);
        /// Deletes many messages at once (one save, one server call per folder).
        #[qinvokable]
        fn delete_messages(self: Pin<&mut Store>, ids_json: &QString);
        #[qinvokable]
        fn send_message(self: Pin<&mut Store>, json: &QString) -> i64;
        #[qinvokable]
        fn save_draft(self: Pin<&mut Store>, json: &QString) -> i64;
        #[qinvokable]
        fn email_suggestions(self: &Store, prefix: &QString) -> QString;

        // calendar
        #[qinvokable]
        fn calendars(self: &Store) -> QString;
        #[qinvokable]
        fn set_calendar_visible(self: Pin<&mut Store>, id: &QString, visible: bool);
        #[qinvokable]
        fn events(self: &Store, from: &QString, to: &QString) -> QString;
        #[qinvokable]
        fn save_event(self: Pin<&mut Store>, json: &QString) -> i64;
        #[qinvokable]
        fn delete_event(self: Pin<&mut Store>, id: i64);
        #[qinvokable]
        fn search_events(self: &Store, query: &QString) -> QString;
        #[qinvokable]
        fn add_calendar(self: Pin<&mut Store>, name: &QString, color: &QString) -> QString;
        #[qinvokable]
        fn update_calendar(self: Pin<&mut Store>, id: &QString, name: &QString, color: &QString);
        #[qinvokable]
        fn delete_calendar(self: Pin<&mut Store>, id: &QString);

        // vCards
        #[qinvokable]
        fn export_vcards(self: Pin<&mut Store>, group: &QString, url: &QString);
        /// One card as a .vcf file in a temporary folder (for sharing by email)
        #[qinvokable]
        fn card_file(self: &Store, id: i64) -> QString;
        #[qinvokable]
        fn import_vcards(self: Pin<&mut Store>, url: &QString, group: &QString);

        // appearance: "system", "light" or "dark"
        #[qinvokable]
        fn choose_theme(self: Pin<&mut Store>, theme: &QString);

        // contacts
        #[qinvokable]
        fn contact_groups(self: &Store) -> QString;
        #[qinvokable]
        fn contacts(self: &Store, group: &QString, query: &QString) -> QString;
        #[qinvokable]
        fn contact(self: &Store, id: i64) -> QString;
        #[qinvokable]
        fn save_contact(self: Pin<&mut Store>, json: &QString) -> i64;
        #[qinvokable]
        fn delete_contact(self: Pin<&mut Store>, id: i64);
        #[qinvokable]
        fn add_group(self: Pin<&mut Store>, name: &QString) -> QString;

        // other apps' data
        #[qinvokable]
        fn sources(self: &Store) -> QString;
        #[qinvokable]
        fn import_source(self: Pin<&mut Store>, id: &QString);
        #[qinvokable]
        fn set_auto_import(self: Pin<&mut Store>, id: &QString, on: bool);
        #[qinvokable]
        fn reset_sample_data(self: Pin<&mut Store>);

        // live accounts (IMAP / SMTP / CalDAV / CardDAV)
        #[qinvokable]
        fn live_accounts(self: &Store) -> QString;
        #[qinvokable]
        fn account_preset(self: &Store, email: &QString) -> QString;
        /// The mail providers offered when adding an account (JSON list).
        #[qinvokable]
        fn mail_providers(self: &Store) -> QString;
        /// Checks the text being written (in the background; the answer
        /// arrives in `spelling`, tagged with `generation`).
        #[qinvokable]
        fn check_spelling(self: &Store, generation: i64, text: &QString, grammar: bool);
        /// "Learn Spelling": the word is never underlined again.
        #[qinvokable]
        fn learn_spelling(self: Pin<&mut Store>, word: &QString);
        /// Proton Mail Bridge running on this computer.
        #[qinvokable]
        fn bridge_running(self: &Store) -> bool;
        /// For an address whose provider Almanac doesn't know: looks its servers
        /// up on the network; the answer arrives in `discovered`.
        #[qinvokable]
        fn discover_account(self: &Store, email: &QString);
        #[qinvokable]
        fn save_live_account(self: Pin<&mut Store>, json: &QString, password: &QString);
        #[qinvokable]
        fn remove_live_account(self: Pin<&mut Store>, id: &QString);
        #[qinvokable]
        fn senders(self: &Store) -> QString;
        #[qinvokable]
        fn message_account(self: &Store, id: i64) -> QString;
        /// The editor's HTML with the list item at the cursor mark one level
        /// in (`deeper`) or out; empty when the cursor isn't in a list.
        #[qinvokable]
        fn nest_list_item(self: &Store, html: &QString, deeper: bool) -> QString;
        /// Return in quoted text (cursor at the mark): the HTML with the quote
        /// broken there (the new cursor marked); empty when not in a quote.
        #[qinvokable]
        fn split_quote(self: &Store, html: &QString) -> QString;
        /// Increase (1) / Decrease (-1) Quote Level for the paragraphs between
        /// the two marks; empty when the marks aren't in paragraphs.
        #[qinvokable]
        fn change_quote(self: &Store, html: &QString, step: i32) -> QString;
        /// Dark mode while writing: the HTML with colours made for a white page
        /// adapted to the dark one (`page`), as JSON {html, back}.
        #[qinvokable]
        fn adapt_colours(self: &Store, html: &QString, page: &QString) -> QString;
        /// The original colours back (`back` from adaptColours, as JSON).
        #[qinvokable]
        fn restore_colours(self: &Store, html: &QString, back: &QString) -> QString;
        /// A code block for the editor: `lang` ("py", "Rust"…) over the code, coloured.
        #[qinvokable]
        fn code_block_html(self: &Store, lang: &QString, code: &QString, dark: bool) -> QString;
        /// The editor's HTML with every code block coloured again (the cursor's
        /// mark kept); empty when there are none.
        #[qinvokable]
        fn refresh_code_blocks(self: &Store, html: &QString, dark: bool) -> QString;
        /// The languages a code block can be (JSON list of names).
        #[qinvokable]
        fn code_languages(self: &Store) -> QString;
        /// A message opened for writing: its <pre><code class="language-…">
        /// blocks as editor code blocks.
        #[qinvokable]
        fn code_blocks_for_editing(self: &Store, html: &QString, dark: bool) -> QString;
        /// Pasted text that looks like Markdown, as HTML for the editor; empty
        /// when it doesn't look like Markdown.
        #[qinvokable]
        fn markdown_html(self: &Store, text: &QString, dark: bool) -> QString;
        /// Saves what's changed now (normally done a moment after each change;
        /// called when Almanac quits).
        #[qinvokable]
        fn flush(self: Pin<&mut Store>);
        /// The desktop in use (XDG_CURRENT_DESKTOP, e.g. "KDE", "GNOME"), "" if unknown.
        #[qinvokable]
        fn desktop_name(self: &Store) -> QString;
        /// The message list a page at a time: {rows, total, all, unread} (see Db::message_page).
        #[qinvokable]
        fn message_page(self: &Store, selection: &QString, query: &QString, threaded: bool, sort_by: &QString, ascending: bool, unread_only: bool, keep: i64, limit: i32) -> QString;
        /// Saves a reading-pane page too big to hand over directly (over 2 MB)
        /// as a private file; returns its file:// address.
        #[qinvokable]
        fn page_file(self: &Store, slot: &QString, html: &QString) -> QString;
        /// Plasma on Wayland: has KWin remember the main window's place and
        /// whether it's maximised (Plasma's own window rule, added once). False elsewhere.
        #[qinvokable]
        fn place_window(self: &Store, x: f64, y: f64, w: f64, h: f64, maximized: bool) -> bool;
        /// Plasma on Wayland: has KWin bring the main window up on the current
        /// desktop and activate it. False elsewhere (the window does it itself).
        #[qinvokable]
        fn bring_forward(self: &Store) -> bool;
        /// For timing how long starting takes (ALMANAC_TIMING).
        #[qinvokable]
        fn startup_mark(self: &Store, what: &QString);
        /// The file:// address of one of the app's icons ("almanac", "almanac-tray", "almanac-tray-unread").
        #[qinvokable]
        fn icon_url(self: &Store, name: &QString) -> QString;

        // calendar extras
        #[qinvokable]
        fn time_zones(self: &Store) -> QString;
        #[qinvokable]
        fn move_event(self: Pin<&mut Store>, id: i64, occurrence: &QString, start: &QString, end: &QString, scope: &QString);

        // background work every few seconds: outbox, reminders, alerts
        #[qinvokable]
        fn tick(self: Pin<&mut Store>);
        #[qinvokable]
        fn undo_send(self: Pin<&mut Store>) -> QString;
        /// Puts the messages last deleted or moved back where they were (here
        /// and on the server).
        #[qinvokable]
        fn undo_mail(self: Pin<&mut Store>);
        #[qinvokable]
        fn outbox(self: &Store) -> QString;
        #[qinvokable]
        fn cancel_outbox(self: Pin<&mut Store>, id: i64) -> QString;
        #[qinvokable]
        fn toggle_vip(self: Pin<&mut Store>, email: &QString);
        #[qinvokable]
        fn toggle_blocked(self: Pin<&mut Store>, email: &QString);
        #[qinvokable]
        fn toggle_mute(self: Pin<&mut Store>, id: i64);
        #[qinvokable]
        fn remind_me(self: Pin<&mut Store>, id: i64, at_ms: i64);
        #[qinvokable]
        fn rules(self: &Store) -> QString;
        #[qinvokable]
        fn save_rule(self: Pin<&mut Store>, json: &QString);
        #[qinvokable]
        fn delete_rule(self: Pin<&mut Store>, id: &QString);
        #[qinvokable]
        fn apply_rules_now(self: Pin<&mut Store>);
        #[qinvokable]
        fn smart_mailbox(self: &Store, id: &QString) -> QString;
        #[qinvokable]
        fn save_smart_mailbox(self: Pin<&mut Store>, json: &QString) -> QString;
        #[qinvokable]
        fn delete_smart_mailbox(self: Pin<&mut Store>, id: &QString);
        #[qinvokable]
        fn signature(self: &Store, account: &QString) -> QString;
        #[qinvokable]
        fn set_signature(self: Pin<&mut Store>, account: &QString, text: &QString);
        #[qinvokable]
        fn open_attachment(self: Pin<&mut Store>, id: i64, name: &QString, save: bool);
        /// Saves an attachment to the file (or into the folder) the person chose.
        #[qinvokable]
        fn save_attachment_as(self: Pin<&mut Store>, id: i64, name: &QString, dest: &QString);
        /// Where Save dialogs start: the Downloads folder (file:// address).
        #[qinvokable]
        fn downloads_folder(self: &Store) -> QString;
        /// The file:// address of an attachment kept on this computer ("" if it
        /// isn't here yet), to show it in a tab or window.
        #[qinvokable]
        fn attachment_url(self: &Store, id: i64, name: &QString) -> QString;
        /// Prints a PDF attachment (the system's print dialog first).
        #[qinvokable]
        fn print_attachment(self: Pin<&mut Store>, id: i64, name: &QString);
        #[qinvokable]
        fn invite_info(self: &Store, id: i64) -> QString;
        #[qinvokable]
        fn respond_invite(self: Pin<&mut Store>, id: i64, status: &QString, calendar: &QString);
        #[qinvokable]
        fn save_smart_group(self: Pin<&mut Store>, json: &QString) -> QString;
        #[qinvokable]
        fn delete_smart_group(self: Pin<&mut Store>, id: &QString);
        #[qinvokable]
        fn delete_group(self: Pin<&mut Store>, id: &QString);
        #[qinvokable]
        fn rename_group(self: Pin<&mut Store>, id: &QString, name: &QString);
        #[qinvokable]
        fn duplicates(self: &Store) -> QString;
        #[qinvokable]
        fn merge_contacts(self: Pin<&mut Store>, ids_json: &QString);
        #[qinvokable]
        fn set_my_card(self: Pin<&mut Store>, id: i64);
        #[qinvokable]
        fn set_contact_photo(self: Pin<&mut Store>, id: i64, url: &QString);
        #[qinvokable]
        fn print_html(self: Pin<&mut Store>, title: &QString, html: &QString);
        #[qinvokable]
        fn export_pdf(self: Pin<&mut Store>, title: &QString, html: &QString);
        #[qinvokable]
        fn finish_setup(self: Pin<&mut Store>);
        /// Settings › Calendar: a made-up event's alert, shown the way real ones are.
        /// Checks a signed message's OpenPGP signature (in the background; the
        /// result comes in signatureJson). Only for messages that look signed.
        #[qinvokable]
        fn check_signature(self: &Store, id: i64);
        /// Decrypts an encrypted message (in the background; the result comes
        /// in decryptedJson, never saved). Only for messages that look encrypted.
        #[qinvokable]
        /// `passphrase`: "" to use one gpg-agent remembers (state "locked" when needed).
        fn decrypt_message(self: &Store, id: i64, passphrase: &QString);
        /// For compose's Sign / Encrypt: {"gpg", "canSign", "missing": [addresses without a key]}.
        #[qinvokable]
        fn pgp_ready(self: &Store, from: &QString, recipients: &QString) -> QString;
        /// Settings › Privacy › OpenPGP: each sending address and its key:
        /// [{"email", "name", "fingerprint"}], and whether GnuPG is installed.
        #[qinvokable]
        fn pgp_keys(self: &Store) -> QString;
        /// In the background (pgpStatusJson and the status line say how it went):
        #[qinvokable]
        fn pgp_create_key(self: &Store, name: &QString, email: &QString, passphrase: &QString);
        /// Unlocks the key for `email` (gpg-agent remembers the passphrase a while).
        #[qinvokable]
        fn pgp_unlock(self: &Store, email: &QString, passphrase: &QString);
        /// Whether the key for `email` can sign now without its passphrase.
        #[qinvokable]
        fn pgp_sign_ready(self: &Store, email: &QString) -> bool;
        /// A file's size in bytes (0 if it can't be read).
        #[qinvokable]
        fn file_size(self: &Store, path: &QString) -> i64;
        /// Whether attachments of `bytes` are too big to send from `account`:
        /// {"over", "limitMb", "store" ("Google Drive", or "" for none), "mode"}.
        #[qinvokable]
        fn cloud_offer(self: &Store, account: &QString, bytes: i64) -> QString;
        /// Uploads a file to the account's cloud storage and shares it by link
        /// (in the background; progress in cloudUploadsJson).
        #[qinvokable]
        fn cloud_upload(self: &Store, account: &QString, path: &QString);
        #[qinvokable]
        fn pgp_import_keys(self: &Store, file_url: &QString);
        #[qinvokable]
        fn pgp_publish_key(self: &Store, fingerprint: &QString);
        #[qinvokable]
        fn pgp_look_up(self: &Store, email: &QString);
        #[qinvokable]
        fn test_alert(self: Pin<&mut Store>);
        /// Settings › General: a made-up new-mail notification, as real ones are.
        #[qinvokable]
        fn test_mail_notification(self: Pin<&mut Store>);
        #[qinvokable]
        fn use_sample_data(self: Pin<&mut Store>);
        #[qinvokable]
        fn clear_sample_data(self: Pin<&mut Store>);
        #[qinvokable]
        fn settings(self: &Store) -> QString;
        #[qinvokable]
        fn set_setting(self: Pin<&mut Store>, key: &QString, value_json: &QString);
        /// Closes an alert (index in alertsJson); with minutes > 0 it comes back then.
        #[qinvokable]
        fn dismiss_alert(self: Pin<&mut Store>, index: i32, snooze_minutes: i32);
        #[qinvokable]
        fn add_subscription(self: Pin<&mut Store>, name: &QString, url: &QString, color: &QString);
        #[qinvokable]
        fn sync_subscriptions(self: Pin<&mut Store>);
        #[qinvokable]
        fn fetch_attachments(self: Pin<&mut Store>, id: i64);
        #[qinvokable]
        fn blocked_and_vips(self: &Store) -> QString;
        #[qinvokable]
        fn sign_in_providers(self: &Store) -> QString;
        #[qinvokable]
        fn sign_in_setup(self: &Store) -> QString;
        #[qinvokable]
        fn save_oauth_client(self: Pin<&mut Store>, provider: &QString, client_id: &QString, client_secret: &QString);
        #[qinvokable]
        fn submit_sign_in_code(self: Pin<&mut Store>, code: &QString);
        #[qinvokable]
        fn cancel_sign_in(self: Pin<&mut Store>);
        #[qinvokable]
        fn setup_tools(self: &Store) -> QString;
        #[qinvokable]
        fn auto_register(self: Pin<&mut Store>, provider: &QString);
        /// Name of the Chrome-family browser Almanac can drive ("" if none).
        #[qinvokable]
        fn browser_name(self: &Store) -> QString;
        #[qinvokable]
        fn set_up_in_browser(self: Pin<&mut Store>, provider: &QString, own_browser: bool);
        #[qinvokable]
        fn ai_prompt(self: &Store, provider: &QString) -> QString;
        #[qinvokable]
        fn use_pasted_registration(self: Pin<&mut Store>, provider: &QString, text: &QString) -> bool;
        #[qinvokable]
        fn read_text_file(self: &Store, url: &QString) -> QString;
        #[qinvokable]
        fn sign_in_with(self: Pin<&mut Store>, provider: &QString, client_id: &QString, client_secret: &QString, tenant: &QString);
        #[qinvokable]
        fn save_window(self: &Store, path: &QString) -> bool;
        #[qinvokable]
        fn delete_event_occurrence(self: Pin<&mut Store>, id: i64, occurrence: &QString, mode: &QString);
    }

    impl cxx_qt::Threading for Store {}
    impl cxx_qt::Initialize for Store {}
}

use crate::data::{Db, Imported};
use crate::sources;
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use serde_json::{json, Value};

#[path = "store_more.rs"]
mod more;

/// One delete or move, to undo: each message's folder before, and where it
/// went on the server (to find it by Message-ID and move it back).
pub struct MailUndo {
    what: String,
    items: Vec<(i64, String, Option<(crate::live::LiveAccount, String, String, String)>)>,
}

pub struct StoreRust {
    mail_undo: Vec<MailUndo>,
    mail_undo_text: QString,
    revision: i32,
    screenshot_path: QString,
    start_section: i32,
    theme: QString,
    start_panel: QString,
    open_url: QString,
    undo_id: i64,
    onboarded: bool,
    signing_in: QString,
    registering: QString,
    google_project: QString,
    attachments_ready: QString,
    last_alert_check: i64,
    /// A check for new mail was asked for while one was running: run again then.
    check_again: bool,
    /// Alerts on screen, and snoozed ones (when to show again, title, when)
    open_alerts: Vec<(String, String)>,
    snoozed_alerts: Vec<(i64, String, String)>,
    busy: bool,
    loading: bool,
    status_text: QString,
    alerts_json: QString,
    signature_json: QString,
    decrypted_json: QString,
    pgp_status_json: QString,
    cloud_uploads_json: QString,
    new_mail: i32,
    show_requests: i32,
    open_request: QString,
    discovered: QString,
    spelling: QString,
    progress_done: i32,
    progress_total: i32,
    db: Db,
    persist: bool,
}

/// The mail being read at start (on its own thread while the window is built).
static MAIL_READ: std::sync::Mutex<Option<std::sync::mpsc::Receiver<crate::data::MailRead>>> = std::sync::Mutex::new(None);

impl Default for StoreRust {
    fn default() -> Self {
        let shot = crate::screenshot_path();
        let wants_sample = std::env::args().any(|a| a == "--sample");
        let (mut db, persist) = if shot.is_some() && std::env::args().any(|a| a == "--real-data") {
            // For checks: a picture of the mail in this data folder (never saved to).
            let mut db = Db::load_without_mail().map(|(d, _)| d).unwrap_or_default();
            db.backed = crate::mailstore::db_path().exists();
            (db, false)
        } else if shot.is_some() {
            (if wants_sample { Db { onboarded: true, ..Db::sample() } } else { Db::empty() }, false)
        } else if wants_sample {
            (Db::load(), true)
        } else {
            // The window comes up straight away; the mail is read meanwhile (on a
            // thread of its own, see MAIL_READ) and shown when it's there.
            match Db::load_without_mail() {
                // The usual case: the mail is in the database and read from there
                // as it's needed (a page of the list, a count), so nothing waits
                // for it, however much there is.
                Some((mut db, legacy)) if legacy.is_empty() && db.html_version >= 3 && crate::mailstore::db_path().exists() => {
                    db.backed = true;
                    crate::startup_mark("mail in the database: nothing to read first");
                    (db, true)
                }
                // Mail still in data.json (an older version), or an update to make
                // to every message: read once as before.
                Some((db, legacy)) => {
                    let version = db.html_version;
                    let (tx, rx) = std::sync::mpsc::channel();
                    *MAIL_READ.lock().unwrap() = Some(rx);
                    std::thread::spawn(move || {
                        crate::startup_mark("reading mail…");
                        let read = Db::read_mail(legacy, version);
                        crate::startup_mark("mail read");
                        let _ = tx.send(read);
                    });
                    (db, true)
                }
                None => (Db::load(), true),
            }
        };
        // For checks: `--html-file <file>` shows that HTML as the sample newsletter
        // (pictures only, so real mail can be looked at without real accounts).
        if shot.is_some() {
            if let Some(html) = crate::arg_value("--html-file").and_then(|p| std::fs::read_to_string(p).ok()) {
                if let Some(m) = db.messages.iter_mut().find(|m| m.subject.starts_with("Spring newsletter")) {
                    m.html = crate::mime::block_remote(&html);
                }
                // `--load-remote`: as with "Always load remote content".
                if std::env::args().any(|x| x == "--load-remote") {
                    db.settings.load_remote = true;
                }
            }
        }
        if wants_sample && shot.is_none() && db.messages.is_empty() {
            let keep = std::mem::take(&mut db.live);
            db = Db { live: keep, onboarded: true, ..Db::sample() };
        }
        StoreRust {
            revision: 0,
            screenshot_path: QString::from(shot.unwrap_or_default().as_str()),
            start_section: if crate::arg_value("--open").is_some() {
                crate::start_section()
            } else {
                match db.settings.start_section.as_str() {
                    "calendar" => 1,
                    "contacts" => 2,
                    _ => 0,
                }
            },
            theme: QString::from(
                crate::theme_arg()
                    .unwrap_or_else(|| if db.theme.is_empty() { "system".into() } else { db.theme.clone() })
                    .as_str(),
            ),
            start_panel: QString::from(crate::arg_value("--show").unwrap_or_default().as_str()),
            open_url: QString::default(),
            undo_id: 0,
            mail_undo: Vec::new(),
            mail_undo_text: QString::default(),
            onboarded: db.onboarded,
            signing_in: QString::default(),
            registering: QString::default(),
            google_project: QString::from(crate::oauth::load_extra("google_project").as_str()),
            attachments_ready: QString::default(),
            last_alert_check: 0,
            check_again: false,
            open_alerts: Vec::new(),
            snoozed_alerts: Vec::new(),
            alerts_json: QString::from("[]"),
            signature_json: QString::from("{}"),
            decrypted_json: QString::from("{}"),
            pgp_status_json: QString::from("{}"),
            cloud_uploads_json: QString::from("{}"),
            new_mail: 0,
            show_requests: 0,
            open_request: QString::default(),
            discovered: QString::default(),
            spelling: QString::default(),
            progress_done: 0,
            progress_total: 0,
            busy: false,
            loading: false,
            status_text: QString::default(),
            db,
            persist,
        }
    }
}

/// A kind of mailbox as Edit › Undo names it.
fn kind_name(kind: &str) -> &str {
    match kind {
        "trash" => "Trash",
        "archive" => "Archive",
        "junk" => "Junk",
        "inbox" => "Inbox",
        k => k,
    }
}

fn q(v: &Value) -> QString {
    QString::from(v.to_string().as_str())
}

fn parse(s: &QString) -> Value {
    serde_json::from_str(&s.to_string()).unwrap_or(Value::Null)
}

impl cxx_qt::Initialize for qobject::Store {
    fn initialize(mut self: Pin<&mut Self>) {
        let _timed = crate::Timed::new("initialize");
        if !self.persist {
            return;
        }
        // The mail still being read: the rest waits for it (saving before it's
        // here would save an empty mailbox; a mail check would fetch it all again).
        if let Some(rx) = MAIL_READ.lock().unwrap().take() {
            self.as_mut().set_loading(true);
            let thread = self.qt_thread();
            std::thread::spawn(move || {
                if let Ok(read) = rx.recv() {
                    let _ = thread.queue(move |mut store| {
                        let save = store.as_mut().db_mut().take_mail(read);
                        store.as_mut().set_loading(false);
                        if save {
                            store.as_mut().flush();
                        }
                        store.as_mut().changed();
                        crate::startup_mark("mail shown");
                        store.as_mut().initialize();
                    });
                }
            });
            return;
        }
        // Started again while running: show this window.
        let thread = self.qt_thread();
        crate::instance::serve(move || {
            let _ = thread.queue(|mut store| {
                let n = *store.show_requests() + 1;
                store.as_mut().set_show_requests(n);
            });
        });
        let auto: Vec<String> = self
            .db
            .sources
            .iter()
            .filter(|s| s.auto_import)
            .map(|s| s.id.clone())
            .collect();
        let live = self.db.live.clone();
        if !self.db.subscriptions.is_empty() {
            self.as_mut().sync_subscriptions();
        }
        // Each account's Inbox watched (IMAP IDLE): the server says when mail
        // arrives, and it's fetched straight away, not at the next timed check.
        for a in live.iter().filter(|a| !a.imap_host.is_empty() && !a.is_pop() && !a.is_ews()) {
            let thread = self.qt_thread();
            crate::live::watch_inbox(a.clone(), move || {
                let _ = thread.queue(|mut store| store.as_mut().refresh_all());
            });
        }
        if !auto.is_empty() || !live.is_empty() {
            self.as_mut().start_jobs(auto, live);
        }
    }
}

/// "file:///home/me/a%20b.vcf" → /home/me/a b.vcf
pub(crate) fn url_to_path(url: &str) -> std::path::PathBuf {
    let rest = url.strip_prefix("file://").unwrap_or(url);
    // Windows: file:///C:/... → C:/...
    let rest = if rest.len() > 3 && rest.as_bytes()[0] == b'/' && rest.as_bytes()[2] == b':' {
        &rest[1..]
    } else {
        rest
    };
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&rest[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    std::path::PathBuf::from(String::from_utf8_lossy(&out).to_string())
}

impl qobject::Store {
    fn db_mut(self: Pin<&mut Self>) -> &mut Db {
        let _timed = crate::Timed::new("db_mut");
        &mut self.rust_mut().get_mut().db
    }

    fn changed(mut self: Pin<&mut Self>) {
        let _timed = crate::Timed::new("changed");
        let g = self.db.generation + 1;
        self.as_mut().db_mut().generation = g;
        // Saved a moment later, so the screen updates first (several changes
        // close together are saved once).
        static SAVE_SOON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if self.persist && !SAVE_SOON.swap(true, std::sync::atomic::Ordering::SeqCst) {
            let thread = self.qt_thread();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(600));
                let _ = thread.queue(|mut store| {
                    SAVE_SOON.store(false, std::sync::atomic::Ordering::SeqCst);
                    store.as_mut().flush();
                });
            });
        }
        let r = self.revision + 1;
        self.as_mut().set_revision(r);
    }

    /// For background work that goes on for a long time (older mail and its
    /// text arriving batch after batch): the screen and the saved copy catch up
    /// every couple of seconds instead of after every batch, so the window
    /// stays quick while it runs.
    fn changed_soon(self: Pin<&mut Self>) {
        let _timed = crate::Timed::new("changed_soon");
        static PENDING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if PENDING.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(2));
            let _ = thread.queue(|store| {
                PENDING.store(false, std::sync::atomic::Ordering::SeqCst);
                store.changed();
            });
        });
    }

    /// Runs network work off the window's thread; `done` runs back on it.
    /// Errors are shown as a status message.
    fn background<T: Send + 'static>(
        self: Pin<&mut Self>,
        work: impl FnOnce() -> Result<T, String> + Send + 'static,
        done: impl FnOnce(Pin<&mut qobject::Store>, T) + Send + 'static,
    ) {
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let r = work();
            let _ = thread.queue(move |mut store| match r {
                Ok(v) => done(store, v),
                Err(e) => store.as_mut().set_status_text(QString::from(e.as_str())),
            });
        });
    }

    /// What's needed to undo moving message `id` to the server folder `to_path`.
    fn undo_item(&self, id: i64, to_path: Option<String>) -> (i64, String, Option<(crate::live::LiveAccount, String, String, String)>) {
        let m = self.db.get(id);
        let folder = m.as_ref().map(|m| m.folder.clone()).unwrap_or_default();
        let server = match (self.db.live_message(id), to_path, m) {
            (Some((acct, from, _)), Some(to), Some(m)) if from != to && !m.message_id.is_empty() => Some((acct, from, to, m.message_id.clone())),
            _ => None,
        };
        (id, folder, server)
    }

    /// Kept for Edit › Undo (the last 20 deletes and moves).
    fn remember_undo(mut self: Pin<&mut Self>, what: &str, items: Vec<(i64, String, Option<(crate::live::LiveAccount, String, String, String)>)>) {
        if items.is_empty() {
            return;
        }
        let n = items.len();
        let mut rust = self.as_mut().rust_mut();
        rust.mail_undo.push(MailUndo { what: what.to_string(), items });
        if rust.mail_undo.len() > 20 {
            rust.mail_undo.remove(0);
        }
        let text = format!("Undo {what}");
        self.as_mut().set_mail_undo_text(QString::from(text.as_str()));
        let note = if n == 1 { format!("{what}: 1 message. Ctrl+Z puts it back.") } else { format!("{what}: {n} messages. Ctrl+Z puts them back.") };
        self.as_mut().set_status_text(QString::from(note.as_str()));
    }

    fn undo_mail(mut self: Pin<&mut Self>) {
        let _timed = crate::Timed::new("undo_mail");
        let Some(entry) = self.as_mut().rust_mut().mail_undo.pop() else { return };
        // Back here at once.
        for (id, folder, _) in &entry.items {
            self.as_mut().db_mut().move_to_folder(*id, folder);
        }
        // On the server: found by Message-ID where they went, moved back (the
        // inbox first: on Gmail that's the label that matters).
        let mut groups: std::collections::BTreeMap<(bool, String, String, String), (crate::live::LiveAccount, Vec<String>)> = Default::default();
        for (_, _, server) in entry.items {
            if let Some((acct, from, to, mid)) = server {
                let key = (from.to_uppercase() != "INBOX", acct.id.clone(), to.clone(), from.clone());
                groups.entry(key).or_insert_with(|| (acct, Vec::new())).1.push(mid);
            }
        }
        let jobs: Vec<(crate::live::LiveAccount, String, String, Vec<String>)> =
            groups.into_iter().map(|((_, _, to, from), (acct, mids))| (acct, to, from, mids)).collect();
        if !jobs.is_empty() {
            self.as_mut().background(
                move || {
                    let mut n = 0;
                    for (acct, now_in, back_to, mids) in jobs {
                        n += crate::live::move_back(&acct, &now_in, &mids, &back_to)?;
                    }
                    Ok(n)
                },
                |_, _| {},
            );
        }
        let next = self.mail_undo.last().map(|u| format!("Undo {}", u.what)).unwrap_or_default();
        self.as_mut().set_mail_undo_text(QString::from(next.as_str()));
        self.as_mut().set_status_text(QString::from(format!("{} undone.", entry.what).as_str()));
        self.changed();
    }

    /// Sends a mail change for a live-account message to its server.
    fn push_mail(self: Pin<&mut Self>, target: Option<(crate::live::LiveAccount, String, u32)>, op: crate::live::MailOp) {
        let _timed = crate::Timed::new("push_mail");
        let Some((acct, path, uid)) = target else { return };
        if uid == 0 {
            return;
        }
        self.background(move || crate::live::mail_op(&acct, &path, uid, op), |_, _| {});
    }

    // ---------------------------------------------------------------- mail

    fn mail_sidebar(&self) -> QString {
        let _timed = crate::Timed::new("mail_sidebar");
        q(&self.db.mail_sidebar())
    }

    fn messages(&self, selection: &QString, query: &QString, threaded: bool) -> QString {
        let _timed = crate::Timed::new("messages");
        q(&self.db.messages(&selection.to_string(), &query.to_string(), threaded))
    }

    fn thread(&self, selection: &QString, id: i64) -> QString {
        let _timed = crate::Timed::new("thread");
        let t = self.db.thread(&selection.to_string(), id);
        // An open conversation's messages whose text hasn't arrived yet: get it now
        // (the background download may be years of mail away from them).
        if let Some(list) = t.as_array() {
            let ids: Vec<i64> = list.iter().filter(|m| m["partial"].as_bool().unwrap_or(false)).filter_map(|m| m["id"].as_i64()).collect();
            self.fetch_texts_now(ids);
        }
        q(&t)
    }

    fn mail_folders(&self) -> QString {
        let _timed = crate::Timed::new("mail_folders");
        q(&self.db.mail_folders())
    }

    fn move_to_folder(mut self: Pin<&mut Self>, id: i64, folder: &QString) {
        let _timed = crate::Timed::new("move_to_folder");
        let target = self.db.live_message(id);
        let to_path = self.db.folder_path(&folder.to_string());
        let undo = self.undo_item(id, to_path.clone());
        self.as_mut().remember_undo("Move", vec![undo]);
        self.as_mut().db_mut().move_to_folder(id, &folder.to_string());
        if let Some(path) = to_path {
            self.as_mut().push_mail(target, crate::live::MailOp::Move(path));
        }
        self.changed();
    }

    fn mark_all_read(mut self: Pin<&mut Self>, selection: &QString) {
        let _timed = crate::Timed::new("mark_all_read");
        let sel = selection.to_string();
        let ids: Vec<i64> = serde_json::from_str::<Value>(&self.db.messages(&sel, "", false).to_string())
            .ok()
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter(|m| !m["read"].as_bool().unwrap_or(true))
            .filter_map(|m| m["id"].as_i64())
            .collect();
        for id in ids {
            let target = self.db.live_message(id);
            self.as_mut().push_mail(target, crate::live::MailOp::Seen(true));
        }
        self.as_mut().db_mut().mark_all_read(&sel);
        self.changed();
    }

    /// "Get Mail": sync live accounts and read again from apps imported before.
    fn refresh_all(mut self: Pin<&mut Self>) {
        let _timed = crate::Timed::new("refresh_all");
        let ids: Vec<String> = self
            .db
            .sources
            .iter()
            // Thunderbird / Mailspring are read again only when the person
            // asked for that ("Keep up to date" in Accounts), not on every Get Mail.
            .filter(|s| s.auto_import)
            .map(|s| s.id.clone())
            .collect();
        let live = self.db.live.clone();
        if !self.db.subscriptions.is_empty() {
            self.as_mut().sync_subscriptions();
        }
        if ids.is_empty() && live.is_empty() && !self.db.subscriptions.is_empty() {
            return;
        }
        if ids.is_empty() && live.is_empty() {
            self.as_mut().set_status_text(QString::from(
                "No mail accounts yet. Add one in Accounts (☰ menu › Almanac › Accounts…).",
            ));
            return;
        }
        self.start_jobs(ids, live);
    }

    fn message(&self, id: i64) -> QString {
        let _timed = crate::Timed::new("message");
        if self.db.get(id).is_some_and(|m| m.partial) {
            self.fetch_text_now(id);
        }
        let mut m = self.db.message(id);
        // Mail saved before the remote-content filter knew every way of
        // writing an address gets it now (it changes nothing twice).
        if let Some(html) = m.get("html").and_then(Value::as_str).filter(|h| !h.is_empty()) {
            m["html"] = json!(crate::mime::block_remote(html));
        }
        q(&m)
    }

    fn set_read(mut self: Pin<&mut Self>, id: i64, read: bool) {
        let _timed = crate::Timed::new("set_read");
        if self.as_mut().db_mut().set_read(id, read) {
            let target = self.db.live_message(id);
            self.as_mut().push_mail(target, crate::live::MailOp::Seen(read));
            self.changed();
        }
    }

    fn toggle_flag(mut self: Pin<&mut Self>, id: i64) {
        let _timed = crate::Timed::new("toggle_flag");
        self.as_mut().db_mut().toggle_flag(id);
        let on = self.db.get(id).map(|m| m.flagged).unwrap_or(false);
        let target = self.db.live_message(id);
        self.as_mut().push_mail(target, crate::live::MailOp::Flagged(on));
        self.changed();
    }

    fn set_flag_color(mut self: Pin<&mut Self>, id: i64, color: i32) {
        let color = if color < 0 { None } else { Some(color.min(6) as u8) };
        self.as_mut().db_mut().set_flag(id, color);
        let target = self.db.live_message(id);
        self.as_mut().push_mail(target, crate::live::MailOp::FlagColor(color));
        self.changed();
    }

    fn move_to(mut self: Pin<&mut Self>, id: i64, kind: &QString) {
        let _timed = crate::Timed::new("move_to");
        let kind = kind.to_string();
        let target = self.db.live_message(id);
        let to_path = self.db.live_folder_of_kind(id, &kind);
        let undo = self.undo_item(id, to_path.clone());
        self.as_mut().remember_undo(&format!("Move to {}", kind_name(&kind)), vec![undo]);
        self.as_mut().db_mut().move_to(id, &kind);
        if let Some(path) = to_path {
            self.as_mut().push_mail(target, crate::live::MailOp::Move(path));
        }
        self.changed();
    }

    fn delete_message(mut self: Pin<&mut Self>, id: i64) {
        let _timed = crate::Timed::new("delete_message");
        let target = self.db.live_message(id);
        let in_trash = target.as_ref().map(|(_, p, _)| Some(p.clone()) == self.db.live_folder_of_kind(id, "trash")).unwrap_or(false);
        let trash = self.db.live_folder_of_kind(id, "trash");
        if !in_trash {
            let undo = self.undo_item(id, trash.clone());
            self.as_mut().remember_undo("Move to Trash", vec![undo]);
        }
        self.as_mut().db_mut().delete_message(id);
        match (in_trash, trash) {
            (false, Some(path)) => self.as_mut().push_mail(target, crate::live::MailOp::Move(path)),
            _ => self.as_mut().push_mail(target, crate::live::MailOp::Expunge),
        }
        self.changed();
    }

    fn delete_messages(mut self: Pin<&mut Self>, ids_json: &QString) {
        let _timed = crate::Timed::new("delete_messages");
        let ids: Vec<i64> = serde_json::from_str(&ids_json.to_string()).unwrap_or_default();
        // Server work grouped per account, folder and what to do with it.
        let mut groups: std::collections::HashMap<(String, String, Option<String>), (crate::live::LiveAccount, Vec<u32>)> = Default::default();
        let mut undo = Vec::new();
        for id in &ids {
            let in_trash = self.db.get(*id).map(|m| self.db.folder_kind(&m.folder) == "trash").unwrap_or(false);
            if !in_trash {
                undo.push(self.undo_item(*id, self.db.live_folder_of_kind(*id, "trash")));
            }
            if let Some((acct, path, uid)) = self.db.live_message(*id) {
                // (A number not known yet: nothing done on the server, rather
                // than something done to another message.)
                if uid == 0 {
                    continue;
                }
                let trash = self.db.live_folder_of_kind(*id, "trash");
                let to_trash = trash.filter(|t| *t != path);
                groups.entry((acct.id.clone(), path, to_trash)).or_insert_with(|| (acct, Vec::new())).1.push(uid);
            }
        }
        if !undo.is_empty() {
            self.as_mut().remember_undo("Move to Trash", undo);
        }
        for id in &ids {
            self.as_mut().db_mut().delete_message(*id);
        }
        for ((_, path, to_trash), (acct, uids)) in groups {
            let op = match to_trash {
                Some(t) => crate::live::MailOp::Move(t),
                None => crate::live::MailOp::Expunge,
            };
            self.as_mut().background(move || crate::live::mail_op_many(&acct, &path, &uids, op), |_, _| {});
        }
        self.changed();
    }

    /// Puts a message in the outbox: it goes out after a short "Undo
    /// Send" delay, or at the chosen "Send Later" time.
    fn send_message(mut self: Pin<&mut Self>, json: &QString) -> i64 {
        let _timed = crate::Timed::new("send_message");
        let v = parse(json);
        let later = v.get("sendAt").and_then(Value::as_i64).unwrap_or(0) / 1000;
        let now = chrono::Local::now().timestamp();
        let (at, scheduled) = if later > now { (later, true) } else { (now + self.db.settings.undo_seconds.max(0), false) };
        if let Some(d) = v.get("draftId").and_then(Value::as_i64) {
            // The draft goes away here and on the server.
            if let Some(m) = self.db.get(d) {
                let account = m.source.strip_prefix("live:").unwrap_or("").to_string();
                if let (Some(acct), Some((_, path))) = (self.db.live_account(&account).cloned(), self.db.live_kind_path(&account, "drafts")) {
                    let key = m.message_id.clone();
                    self.as_mut().background(move || crate::live::delete_draft(&acct, &path, &key), |_, _| {});
                }
            }
            self.as_mut().db_mut().remove_message(d);
        }
        let id = self.as_mut().db_mut().queue_outgoing(v, at, scheduled);
        if scheduled {
            let when = chrono::DateTime::from_timestamp(at, 0).map(|t| t.with_timezone(&chrono::Local).format("%a %e %b %H:%M").to_string()).unwrap_or_default();
            self.as_mut().set_status_text(QString::from(format!("Will be sent {when} (see Outbox).").as_str()));
        } else if self.db.settings.undo_seconds > 0 {
            self.as_mut().set_undo_id(id);
            self.as_mut().set_status_text(QString::from("Sending…"));
        } else {
            self.as_mut().tick();
        }
        self.changed();
        id
    }

    /// Saves the message being written as a draft: one copy, kept in the
    /// sending account's Drafts (on the server too), replaced on every save.
    fn save_draft(mut self: Pin<&mut Self>, json: &QString) -> i64 {
        let _timed = crate::Timed::new("save_draft");
        let v = parse(json);
        let account = v.get("account").and_then(Value::as_str).unwrap_or("").to_string();
        // The draft keeps one Message-ID for its whole life.
        let key = v
            .get("draftId")
            .and_then(Value::as_i64)
            .and_then(|d| self.db.get(d))
            .map(|m| m.message_id.clone())
            .filter(|k| !k.is_empty())
            .unwrap_or_else(new_message_id);
        let id = self.as_mut().db_mut().store_outgoing(&v, "drafts");
        let drafts = self.db.live_kind_path(&account, "drafts");
        let attachments: Vec<String> = v
            .get("attachments")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
            .unwrap_or_default();
        // Attachments are kept with the draft (as files), so they're there when it's reopened.
        let mut names = Vec::new();
        for p in &attachments {
            let name = std::path::Path::new(p).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if let Ok(bytes) = std::fs::read(p) {
                if crate::mailstore::store_attachment(id, &name, &bytes).is_ok() {
                    names.push(name);
                }
            }
        }
        if let Some(m) = self.as_mut().db_mut().msg_mut(id) {
            m.html = v.get("html").and_then(Value::as_str).unwrap_or("").to_string();
            m.message_id = key.clone();
            m.attachments = names;
            if let Some((folder, _)) = &drafts {
                // In the account's own Drafts (the next sync swaps in the server's copy).
                m.folder = folder.clone();
                m.source = format!("live:{account}");
            }
        }
        if let (Some(acct), Some((_, path))) = (self.db.live_account(&account).cloned(), drafts) {
            let mut m = crate::live::OutMsg::from_json(&v);
            m.message_id = key;
            self.as_mut().background(
                move || crate::live::save_draft_msg(&acct, &path, &m),
                |mut store, _| store.as_mut().set_status_text(QString::from("Draft saved.")),
            );
        } else {
            self.as_mut().set_status_text(QString::from("Draft saved."));
        }
        self.changed();
        id
    }

    fn email_suggestions(&self, prefix: &QString) -> QString {
        let _timed = crate::Timed::new("email_suggestions");
        q(&self.db.email_suggestions(&prefix.to_string()))
    }

    // ------------------------------------------------------------ calendar

    fn calendars(&self) -> QString {
        let _timed = crate::Timed::new("calendars");
        q(&self.db.calendars_json())
    }

    fn set_calendar_visible(mut self: Pin<&mut Self>, id: &QString, visible: bool) {
        let _timed = crate::Timed::new("set_calendar_visible");
        self.as_mut().db_mut().set_calendar_visible(&id.to_string(), visible);
        self.changed();
    }

    fn events(&self, from: &QString, to: &QString) -> QString {
        let _timed = crate::Timed::new("events");
        q(&self.db.events(&from.to_string(), &to.to_string()))
    }

    fn save_event(mut self: Pin<&mut Self>, json: &QString) -> i64 {
        let _timed = crate::Timed::new("save_event");
        let v = parse(json);
        // A server event moved to another calendar: remove the old copy there.
        let old = v.get("id").and_then(Value::as_i64).and_then(|id| self.db.event_by_id(id).cloned());
        let scope = v.get("scope").and_then(Value::as_str).unwrap_or("all").to_string();
        let occurrence = v.get("occurrence").and_then(Value::as_str).unwrap_or("").to_string();
        let ids = self.as_mut().db_mut().save_event_scoped(&v, &occurrence, &scope);
        let id = ids.last().copied().unwrap_or(0);
        if ids.len() > 1 {
            for i in &ids[..ids.len() - 1] {
                self.as_mut().push_event(*i);
            }
        }
        // Invitations need an organiser: the calendar's account (or the first account).
        let needs_org = self.db.event_by_id(id).map(|e| !e.attendees.is_empty() && e.organizer.is_empty()).unwrap_or(false);
        if needs_org {
            let email = self
                .db
                .event_by_id(id)
                .and_then(|e| self.db.calendar(&e.calendar))
                .and_then(|c| self.db.live_account(&c.account))
                .or_else(|| self.db.live.first())
                .map(|a| a.email.clone())
                .unwrap_or_default();
            if let Some(e) = self.as_mut().db_mut().event_mut(id) {
                e.organizer = email;
            }
        }
        // Invite people only when something they'd care about changed (not
        // for a new alert or note), as Apple Calendar does.
        let worth_telling = match (&old, self.db.event_by_id(id)) {
            (None, Some(_)) => true,
            (Some(o), Some(e)) => {
                o.start != e.start || o.end != e.end || o.all_day != e.all_day || o.location != e.location || o.title != e.title
                    || o.rrule != e.rrule || o.tz != e.tz
                    || o.attendees.iter().map(|a| a.email.to_lowercase()).collect::<Vec<_>>() != e.attendees.iter().map(|a| a.email.to_lowercase()).collect::<Vec<_>>()
            }
            _ => false,
        };
        if worth_telling {
            self.as_mut().send_invitations(id, "REQUEST");
        }
        if let Some(o) = old.filter(|o| !o.href.is_empty() && self.db.event_by_id(id).map(|e| e.calendar != o.calendar).unwrap_or(false)) {
            if let Some(acct) = self.db.calendar(&o.calendar).and_then(|c| self.db.live_account(&c.account)).cloned() {
                self.as_mut().background(move || crate::live::delete_remote(&acct, &o.href, &o.etag), |_, _| {});
            }
        }
        self.as_mut().push_event(id);
        self.changed();
        id
    }

    /// Writes an event to its CalDAV calendar (if it lives on a server).
    fn push_event(self: Pin<&mut Self>, id: i64) {
        let _timed = crate::Timed::new("push_event");
        let Some((master, overrides)) = self.db.series(id) else { return };
        let Some(cal) = self.db.calendar(&master.calendar).cloned() else { return };
        let Some(acct) = self.db.live_account(&cal.account).cloned() else { return };
        let ids: Vec<i64> = std::iter::once(master.id).chain(overrides.iter().map(|o| o.id)).collect();
        self.background(
            move || crate::live::push_event(&acct, &cal, &master, &overrides),
            move |mut store, (href, etag, raw)| {
                for i in &ids {
                    if let Some(ev) = store.as_mut().db_mut().event_mut(*i) {
                        ev.href = href.clone();
                        ev.etag = etag.clone();
                        ev.raw = raw.clone();
                    }
                }
                store.changed();
            },
        );
    }

    /// "Delete Only This Event", "All Future Events" or the whole series.
    fn delete_event_occurrence(mut self: Pin<&mut Self>, id: i64, occurrence: &QString, mode: &QString) {
        let _timed = crate::Timed::new("delete_event_occurrence");
        match mode.to_string().as_str() {
            "this" => self.as_mut().db_mut().skip_occurrence(id, &occurrence.to_string()),
            "future" => self.as_mut().db_mut().end_series_before(id, &occurrence.to_string()),
            _ => {
                self.delete_event(id);
                return;
            }
        }
        self.as_mut().push_event(id);
        self.changed();
    }

    /// Picture of the whole window (for `--screenshot`).
    fn save_window(&self, path: &QString) -> bool {
        let _timed = crate::Timed::new("save_window");
        qobject::craft_save_window(path)
    }

    fn time_zones(&self) -> QString {
        let _timed = crate::Timed::new("time_zones");
        q(&json!({"system": crate::tz::system_zone(), "zones": crate::tz::all_zones()}))
    }

    fn delete_event(mut self: Pin<&mut Self>, id: i64) {
        let _timed = crate::Timed::new("delete_event");
        self.as_mut().send_invitations(id, "CANCEL");
        if let Some(e) = self.as_mut().db_mut().delete_event(id) {
            if !e.recurrence_id.is_empty() {
                // One changed occurrence: the series keeps skipping that date.
                let master = self.db.events.iter().find(|m| m.uid == e.uid && m.calendar == e.calendar && m.recurrence_id.is_empty()).map(|m| m.id);
                if let Some(m) = master {
                    self.as_mut().push_event(m);
                }
            } else {
                // A whole series goes with its changed occurrences.
                self.as_mut().db_mut().events.retain(|o| !(o.uid == e.uid && o.calendar == e.calendar && !e.uid.is_empty()));
                let acct = self.db.calendar(&e.calendar).and_then(|c| self.db.live_account(&c.account)).cloned();
                if let (Some(acct), Some(cal), false) = (acct, self.db.calendar(&e.calendar).cloned(), e.href.is_empty()) {
                    self.as_mut().background(move || crate::live::delete_event_remote(&acct, &cal, &e), |_, _| {});
                }
            }
        }
        self.changed();
    }

    /// Drag to move or resize an event (local times).
    fn move_event(mut self: Pin<&mut Self>, id: i64, occurrence: &QString, start: &QString, end: &QString, scope: &QString) {
        let _timed = crate::Timed::new("move_event");
        let ids = self.as_mut().db_mut().move_event(id, &occurrence.to_string(), &start.to_string(), &end.to_string(), &scope.to_string());
        for i in ids {
            self.as_mut().push_event(i);
        }
        self.changed();
    }

    /// Emails invitations (or cancellations) for events on calendars whose
    /// server doesn't do it itself (this computer's calendars).
    fn send_invitations(mut self: Pin<&mut Self>, id: i64, method: &str) {
        let _timed = crate::Timed::new("send_invitations");
        let Some(e) = self.db.event_by_id(id).cloned() else { return };
        if e.attendees.is_empty() {
            return;
        }
        // Someone else's meeting (an invitation you accepted): only its
        // organiser sends updates and cancellations.
        let organizer = e.organizer.trim_start_matches("mailto:").to_lowercase();
        if !organizer.is_empty() && !self.my_addresses().contains(&organizer) {
            return;
        }
        let Some(cal) = self.db.calendar(&e.calendar).cloned() else { return };
        if !cal.account.is_empty() {
            return; // CalDAV, Google and Microsoft send invitations themselves
        }
        let Some(acct) = self.db.live.iter().find(|a| !a.smtp_host.is_empty() || a.provider != "").cloned() else {
            self.as_mut().set_status_text(QString::from("Add a mail account to send invitations."));
            return;
        };
        let mut ev = e.clone();
        if ev.organizer.is_empty() {
            ev.organizer = acct.email.clone();
            if let Some(x) = self.as_mut().db_mut().event_mut(id) {
                x.organizer = acct.email.clone();
            }
        }
        let method = method.to_string();
        self.background(
            move || crate::live::send_invitation(&acct, &ev, &method),
            |mut store, _| store.as_mut().set_status_text(QString::from("Invitations sent.")),
        );
    }

    fn search_events(&self, query: &QString) -> QString {
        let _timed = crate::Timed::new("search_events");
        q(&self.db.search_events(&query.to_string()))
    }

    fn add_calendar(mut self: Pin<&mut Self>, name: &QString, color: &QString) -> QString {
        let _timed = crate::Timed::new("add_calendar");
        let id = self.as_mut().db_mut().add_calendar(&name.to_string(), &color.to_string());
        self.changed();
        QString::from(id.as_str())
    }

    fn update_calendar(mut self: Pin<&mut Self>, id: &QString, name: &QString, color: &QString) {
        let _timed = crate::Timed::new("update_calendar");
        self.as_mut()
            .db_mut()
            .update_calendar(&id.to_string(), &name.to_string(), &color.to_string());
        self.changed();
    }

    fn delete_calendar(mut self: Pin<&mut Self>, id: &QString) {
        let _timed = crate::Timed::new("delete_calendar");
        self.as_mut().db_mut().delete_calendar(&id.to_string());
        self.changed();
    }

    // -------------------------------------------------------------- vCards

    fn export_vcards(mut self: Pin<&mut Self>, group: &QString, url: &QString) {
        let _timed = crate::Timed::new("export_vcards");
        let path = url_to_path(&url.to_string());
        let text = self.db.export_vcards(&group.to_string());
        let n = text.matches("BEGIN:VCARD").count();
        let msg = match std::fs::write(&path, text) {
            Ok(_) => format!("Exported {n} cards to {}", path.display()),
            Err(e) => format!("Could not save the file: {e}"),
        };
        self.as_mut().set_status_text(QString::from(msg.as_str()));
    }

    fn import_vcards(mut self: Pin<&mut Self>, url: &QString, group: &QString) {
        let _timed = crate::Timed::new("import_vcards");
        let path = url_to_path(&url.to_string());
        let msg = match std::fs::read(&path) {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes).to_string();
                let n = self.as_mut().db_mut().import_vcards(&text, &group.to_string());
                format!("Imported {n} cards")
            }
            Err(e) => format!("Could not read the file: {e}"),
        };
        self.as_mut().set_status_text(QString::from(msg.as_str()));
        self.changed();
    }

    fn choose_theme(mut self: Pin<&mut Self>, theme: &QString) {
        let _timed = crate::Timed::new("choose_theme");
        self.as_mut().db_mut().theme = theme.to_string();
        self.as_mut().set_theme(theme.clone());
        self.changed();
    }

    // ------------------------------------------------------------ contacts

    fn contact_groups(&self) -> QString {
        let _timed = crate::Timed::new("contact_groups");
        q(&self.db.groups_json())
    }

    fn contacts(&self, group: &QString, query: &QString) -> QString {
        let _timed = crate::Timed::new("contacts");
        q(&self.db.contacts(&group.to_string(), &query.to_string()))
    }

    fn contact(&self, id: i64) -> QString {
        let _timed = crate::Timed::new("contact");
        q(&self.db.contact(id))
    }

    fn save_contact(mut self: Pin<&mut Self>, json: &QString) -> i64 {
        let _timed = crate::Timed::new("save_contact");
        let id = self.as_mut().db_mut().save_contact(&parse(json));
        // Cards in a server address book are written back there.
        let card = self.db.contacts.iter().find(|c| c.id == id).cloned();
        if let Some(c) = card {
            let book = c
                .groups
                .iter()
                .filter_map(|g| self.db.groups.iter().find(|x| &x.id == g && !x.url.is_empty()))
                .next()
                .cloned();
            if let Some(book) = book {
                if let Some(acct) = self.db.live_for_source(&book.source) {
                    self.as_mut().background(
                        move || {
                            let uid = if c.uid.is_empty() { format!("{}-{}@craft-pim", chrono::Utc::now().timestamp_millis(), c.id) } else { c.uid.clone() };
                            let mut c2 = c.clone();
                            c2.uid = uid.clone();
                            let (href, etag) = crate::live::push_contact(&acct, &book.url, &c2)?;
                            Ok((href, etag, uid, crate::ics::write_vcard(&c2, &c2.uid, &c2.raw), book.source.clone()))
                        },
                        move |mut store, (href, etag, uid, raw, source)| {
                            if let Some(x) = store.as_mut().db_mut().contacts.iter_mut().find(|x| x.id == id) {
                                x.href = href;
                                x.etag = etag;
                                x.uid = uid;
                                x.raw = raw;
                                x.source = source;
                            }
                            store.changed();
                        },
                    );
                }
            }
        }
        self.changed();
        id
    }

    fn delete_contact(mut self: Pin<&mut Self>, id: i64) {
        let _timed = crate::Timed::new("delete_contact");
        let card = self.db.contacts.iter().find(|c| c.id == id).cloned();
        self.as_mut().db_mut().delete_contact(id);
        if let Some(c) = card.filter(|c| !c.href.is_empty()) {
            if let Some(acct) = self.db.live_for_source(&c.source) {
                self.as_mut().background(move || crate::live::delete_contact_remote(&acct, &c), |_, _| {});
            }
        }
        self.changed();
    }

    // ---------------------------------------------------------- live accounts

    fn live_accounts(&self) -> QString {
        let _timed = crate::Timed::new("live_accounts");
        q(&self.db.live_json())
    }

    fn discover_account(&self, email: &QString) {
        let _timed = crate::Timed::new("discover_account");
        let email = email.to_string();
        let domain = email.rsplit('@').next().unwrap_or("").to_lowercase();
        // Known providers have their settings built in.
        if crate::live::preset(&email).imap_host != format!("imap.{domain}") {
            return;
        }
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let Some((a, how)) = crate::discover::discover(&email) else { return };
            let found = json!({"email": email, "incoming": a.incoming, "imapHost": a.imap_host, "imapPort": a.imap_port, "smtpHost": a.smtp_host,
                               "smtpPort": a.smtp_port, "caldavUrl": a.caldav_url, "carddavUrl": a.carddav_url,
                               "username": a.username, "how": how});
            let _ = thread.queue(move |mut store| {
                store.as_mut().set_discovered(q(&found));
                let domain = email.rsplit('@').next().unwrap_or("").to_string();
                store.as_mut().set_status_text(QString::from(format!("Found the mail servers for {domain} ({how}).").as_str()));
            });
        });
    }

    fn check_spelling(&self, generation: i64, text: &QString, grammar: bool) {
        let _timed = crate::Timed::new("check_spelling");
        let thread = self.qt_thread();
        crate::spelling::start(self.db.settings.spelling_words.clone(), move |found| {
            let _ = thread.queue(move |mut store| store.as_mut().set_spelling(QString::from(found.as_str())));
        });
        crate::spelling::request(generation, text.to_string(), grammar);
    }

    fn learn_spelling(mut self: Pin<&mut Self>, word: &QString) {
        let _timed = crate::Timed::new("learn_spelling");
        let word = word.to_string();
        if word.trim().is_empty() || self.db.settings.spelling_words.contains(&word) {
            return;
        }
        self.as_mut().db_mut().settings.spelling_words.push(word.clone());
        crate::spelling::learn(word);
        self.changed();
    }

    fn mail_providers(&self) -> QString {
        let _timed = crate::Timed::new("mail_providers");
        q(&crate::live::providers_json())
    }

    fn bridge_running(&self) -> bool {
        let _timed = crate::Timed::new("bridge_running");
        crate::live::bridge_running()
    }

    fn account_preset(&self, email: &QString) -> QString {
        let _timed = crate::Timed::new("account_preset");
        let p = crate::live::preset(&email.to_string());
        q(&json!({"incoming": p.incoming, "imapHost": p.imap_host, "imapPort": p.imap_port, "smtpHost": p.smtp_host,
                  "smtpPort": p.smtp_port, "caldavUrl": p.caldav_url, "carddavUrl": p.carddav_url,
                  "username": p.username}))
    }

    /// Adds or changes an account: stores the password in the keychain,
    /// checks the sign-in, then reads everything.
    fn save_live_account(mut self: Pin<&mut Self>, json: &QString, password: &QString) {
        let _timed = crate::Timed::new("save_live_account");
        let v = parse(json);
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").trim().to_string();
        let n = |k: &str, d: u16| v.get(k).and_then(|x| x.as_u64().or_else(|| x.as_str().and_then(|t| t.parse().ok()))).map(|x| x as u16).unwrap_or(d);
        let id = if s("id").is_empty() { format!("acct{}", chrono::Utc::now().timestamp_millis()) } else { s("id") };
        let email = s("email");
        let acct = crate::live::LiveAccount {
            id: id.clone(),
            name: if s("name").is_empty() { email.clone() } else { s("name") },
            email: email.clone(),
            full_name: s("fullName"),
            username: if s("username").is_empty() { email.clone() } else { s("username") },
            imap_host: s("imapHost"),
            imap_port: n("imapPort", 993),
            smtp_host: s("smtpHost"),
            smtp_port: n("smtpPort", 465),
            caldav_url: s("caldavUrl"),
            carddav_url: s("carddavUrl"),
            incoming: match s("incoming").as_str() {
                "pop" => "pop".into(),
                "ews" => "ews".into(),
                _ => String::new(),
            },
            ..Default::default()
        };
        let pw = password.to_string();
        if !pw.is_empty() {
            if let Some(note) = crate::live::save_password(&id, &pw) {
                self.as_mut().set_status_text(QString::from(note.as_str()));
            }
        }
        self.as_mut().db_mut().upsert_live(acct.clone());
        self.as_mut().changed();
        self.as_mut().set_status_text(QString::from(format!("Signing in to {}…", acct.email).as_str()));
        let check = acct.clone();
        self.background(
            move || crate::live::test(&check),
            move |mut store, _| {
                store.as_mut().set_status_text(QString::from("Signed in. Reading mail, calendars and contacts…"));
                store.start_jobs(vec![], vec![acct]);
            },
        );
    }

    fn remove_live_account(mut self: Pin<&mut Self>, id: &QString) {
        let _timed = crate::Timed::new("remove_live_account");
        let id = id.to_string();
        crate::live::forget_password(&id);
        self.as_mut().db_mut().remove_live(&id);
        self.changed();
    }

    fn senders(&self) -> QString {
        let _timed = crate::Timed::new("senders");
        q(&self.db.senders_json())
    }

    fn place_window(&self, x: f64, y: f64, w: f64, h: f64, maximized: bool) -> bool {
        let _timed = crate::Timed::new("place_window");
        if !crate::kwin::wanted() || crate::screenshot_path().is_some() {
            return false;
        }
        crate::kwin::remember_place(if w > 0.0 && h > 0.0 { Some((x, y)) } else { None }, maximized)
    }

    fn icon_url(&self, name: &QString) -> QString {
        let _timed = crate::Timed::new("icon_url");
        QString::from(format!("file://{}", crate::icon_path(&name.to_string()).display()).as_str())
    }

    fn startup_mark(&self, what: &QString) {
        let _timed = crate::Timed::new("startup_mark");
        crate::startup_mark(&what.to_string());
    }

    fn bring_forward(&self) -> bool {
        let _timed = crate::Timed::new("bring_forward");
        crate::kwin::wanted() && crate::kwin::bring_forward()
    }

    fn page_file(&self, slot: &QString, html: &QString) -> QString {
        let _timed = crate::Timed::new("page_file");
        let dir = crate::data::data_dir().join("pages");
        let _ = std::fs::create_dir_all(&dir);
        // Pages of windows closed long ago go.
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for e in entries.flatten() {
                let old = e.metadata().and_then(|m| m.modified()).map(|t| t.elapsed().map(|d| d.as_secs() > 86_400).unwrap_or(false)).unwrap_or(false);
                if old {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
        let name: String = slot.to_string().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        let path = dir.join(format!("{name}.html"));
        if std::fs::write(&path, html.to_string()).is_err() {
            return QString::default();
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        QString::from(format!("file://{}", path.display()).as_str())
    }

    #[allow(clippy::too_many_arguments)]
    fn message_page(&self, selection: &QString, query: &QString, threaded: bool, sort_by: &QString, ascending: bool, unread_only: bool, keep: i64, limit: i32) -> QString {
        let _timed = crate::Timed::new("message_page");
        q(&self.db.message_page(
            &selection.to_string(),
            &query.to_string(),
            threaded,
            &sort_by.to_string(),
            ascending,
            unread_only,
            keep,
            limit.max(1) as usize,
        ))
    }

    fn desktop_name(&self) -> QString {
        let _timed = crate::Timed::new("desktop_name");
        QString::from(std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().as_str())
    }

    fn flush(mut self: Pin<&mut Self>) {
        if self.loading {
            return;
        }
        let _timed = crate::Timed::new("flush");
        if self.persist {
            let saved = self.db.save();
            // Saved: the text is in the database; memory keeps a preview (or,
            // with mail read from the database as needed, nothing at all).
            self.as_mut().db_mut().forget_saved_text();
            if saved {
                self.as_mut().db_mut().release_saved();
            }
        }
    }

    fn adapt_colours(&self, html: &QString, page: &QString) -> QString {
        let _timed = crate::Timed::new("adapt_colours");
        let (html, back) = crate::darkcolours::adapt(&html.to_string(), &page.to_string());
        q(&json!({ "html": html, "back": back }))
    }

    fn restore_colours(&self, html: &QString, back: &QString) -> QString {
        let _timed = crate::Timed::new("restore_colours");
        let back: std::collections::HashMap<String, String> = serde_json::from_str(&back.to_string()).unwrap_or_default();
        // (and the quotes made real quotes, for what's sent and saved)
        let html = crate::richtext::quotes_for_sending(&html.to_string());
        // (and code blocks sent as <pre><code>, in their light colours)
        QString::from(crate::codeblock::for_sending(&crate::darkcolours::restore(&html, &back)).as_str())
    }

    fn code_block_html(&self, lang: &QString, code: &QString, dark: bool) -> QString {
        let _timed = crate::Timed::new("code_block_html");
        QString::from(crate::codeblock::editor_block(&lang.to_string(), &code.to_string(), dark).as_str())
    }

    fn refresh_code_blocks(&self, html: &QString, dark: bool) -> QString {
        let _timed = crate::Timed::new("refresh_code_blocks");
        QString::from(crate::codeblock::refresh(&html.to_string(), dark).unwrap_or_default().as_str())
    }

    fn code_languages(&self) -> QString {
        q(&json!(crate::codeblock::languages()))
    }

    fn code_blocks_for_editing(&self, html: &QString, dark: bool) -> QString {
        let _timed = crate::Timed::new("code_blocks_for_editing");
        QString::from(crate::codeblock::for_editing(&html.to_string(), dark).as_str())
    }

    fn markdown_html(&self, text: &QString, dark: bool) -> QString {
        let _timed = crate::Timed::new("markdown_html");
        let t = text.to_string();
        if !crate::codeblock::looks_like_markdown(&t) {
            return QString::default();
        }
        QString::from(crate::codeblock::markdown_html(&t, dark).as_str())
    }

    fn change_quote(&self, html: &QString, step: i32) -> QString {
        let _timed = crate::Timed::new("change_quote");
        QString::from(crate::richtext::change_quote(&html.to_string(), step).unwrap_or_default().as_str())
    }

    fn split_quote(&self, html: &QString) -> QString {
        let _timed = crate::Timed::new("split_quote");
        QString::from(crate::richtext::split_quote(&html.to_string()).unwrap_or_default().as_str())
    }

    fn nest_list_item(&self, html: &QString, deeper: bool) -> QString {
        let _timed = crate::Timed::new("nest_list_item");
        QString::from(crate::richtext::nest_list_item(&html.to_string(), deeper).unwrap_or_default().as_str())
    }

    fn message_account(&self, id: i64) -> QString {
        let _timed = crate::Timed::new("message_account");
        QString::from(self.db.message_account(id).as_str())
    }

    fn add_group(mut self: Pin<&mut Self>, name: &QString) -> QString {
        let _timed = crate::Timed::new("add_group");
        let id = self.as_mut().db_mut().add_group(&name.to_string());
        self.changed();
        QString::from(id.as_str())
    }

    // --------------------------------------------------------------- sources

    fn sources(&self) -> QString {
        let _timed = crate::Timed::new("sources");
        let tb = sources::thunderbird_profile();
        let ms = sources::mailspring_dir();
        let entry = |id: &str, name: &str, path: Option<std::path::PathBuf>| {
            let settings = self.db.sources.iter().find(|s| s.id == id);
            json!({
                "id": id,
                "name": name,
                "found": path.is_some(),
                "path": path.map(|p| p.display().to_string()).unwrap_or_default(),
                "autoImport": settings.map(|s| s.auto_import).unwrap_or(false),
                "lastImport": settings.map(|s| s.last_import.clone()).unwrap_or_default(),
                "counts": self.db.counts_for(id),
            })
        };
        q(&json!([
            entry("thunderbird", "Thunderbird", tb),
            entry("mailspring", "Mailspring", ms),
        ]))
    }

    fn import_source(self: Pin<&mut Self>, id: &QString) {
        let _timed = crate::Timed::new("import_source");
        self.start_jobs(vec![id.to_string()], vec![]);
    }

    /// Reads other apps' data and syncs live accounts on a background
    /// thread, one after another, applying each result as it arrives.
    fn start_jobs(mut self: Pin<&mut Self>, imports: Vec<String>, live: Vec<crate::live::LiveAccount>) {
        let _timed = crate::Timed::new("start_jobs");
        // (For timing a copy of real data: nothing fetched, sent or changed on servers.)
        if crate::offline() {
            return;
        }
        if self.busy {
            // (New mail being announced meanwhile: looked for as soon as this one's done.)
            if !live.is_empty() {
                self.as_mut().rust_mut().get_mut().check_again = true;
            }
            return;
        }
        let jobs: Vec<(String, std::path::PathBuf)> = imports
            .into_iter()
            .filter_map(|id| {
                let path = match id.as_str() {
                    "thunderbird" => sources::thunderbird_profile(),
                    "mailspring" => sources::mailspring_dir(),
                    _ => None,
                }?;
                Some((id, path))
            })
            .collect();
        if jobs.is_empty() && live.is_empty() {
            self.as_mut()
                .set_status_text(QString::from("That app was not found on this computer"));
            return;
        }
        // What's already here per account, so the sync only fetches changes.
        let all = self.db.settings.download_all_mail;
        // (Mail in the database: saved first, then read on the sync's own thread,
        // not this one: for a big mailbox that's a second or so.)
        let backed = self.db.backed;
        if backed {
            self.as_mut().flush();
        }
        let known_for: std::collections::HashMap<String, (Option<std::collections::HashMap<String, Vec<u32>>>, bool)> =
            live.iter().map(|a| (a.id.clone(), (if backed { None } else { Some(self.db.known_uids(&a.source())) }, all))).collect();
        let marks = self.db.folder_marks.clone();
        self.as_mut().set_busy(true);
        // What's being done, by name (a bare "Syncing…" left people guessing).
        let names: Vec<String> = live.iter().map(|a| a.name.clone()).collect();
        let what = if !names.is_empty() { format!("{}: checking for new mail…", names.join(", ")) } else { "Importing…".to_string() };
        self.as_mut().set_status_text(QString::from(what.as_str()));
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let progress_for = |t: cxx_qt::CxxQtThread<qobject::Store>| {
                move |msg: String| {
                    let _ = t.queue(move |mut store| {
                        let (done, total) = crate::live::parse_counted(&msg).unwrap_or((0, 0));
                        store.as_mut().set_progress_done(done as i32);
                        store.as_mut().set_progress_total(total as i32);
                        store.as_mut().set_status_text(QString::from(msg.as_str()));
                    });
                }
            };
            for (id, path) in jobs {
                let progress = progress_for(thread.clone());
                let result: Result<Imported, String> = match id.as_str() {
                    "thunderbird" => sources::import_thunderbird(&path, &progress),
                    _ => sources::import_mailspring(&path, &progress),
                };
                let _ = thread.queue(move |mut store| {
                    let text = match result {
                        Ok(imp) => {
                            let msg = format!(
                                "Imported {} messages, {} events and {} contacts",
                                imp.messages.len(),
                                imp.events.len(),
                                imp.contacts.len()
                            );
                            store.as_mut().db_mut().apply_import(imp);
                            msg
                        }
                        Err(e) => e,
                    };
                    store.as_mut().set_status_text(QString::from(text.as_str()));
                    store.changed();
                });
            }
            // In two stages, so new mail never waits behind old: first the
            // newest messages of every mailbox (with calendars and contacts),
            // shown at once with their text; then, with "keep all mail", the
            // rest of each account, in the background, while later checks for
            // new mail go on as usual.
            static DEEP_RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            use std::sync::atomic::Ordering;
            let deep = all && !DEEP_RUNNING.swap(true, Ordering::SeqCst);
            let started = chrono::Utc::now().timestamp();
            // One account's message list → shown; returns what still needs its text
            // and what's here now (for the next stage).
            type Known = std::collections::HashMap<String, Vec<u32>>;
            let stage = |acct: &crate::live::LiveAccount, known: &Known, everything: bool, first: bool| -> Option<(Vec<crate::live::Wanted>, Known)> {
                let progress = progress_for(thread.clone());
                let (mut imp, errors) = if first {
                    // New mail shown (and announced) a mailbox at a time, as it's found.
                    let t = thread.clone();
                    let fresh = move |msgs: Vec<crate::data::Message>| {
                        let _ = t.queue(move |mut store| {
                            let ids = store.as_mut().db_mut().add_messages(msgs);
                            if !ids.is_empty() {
                                // Their text straight away (ahead of the background
                                // download), so it's there when one is opened.
                                if ids.len() <= 50 {
                                    store.fetch_texts_now(ids.clone());
                                }
                                store.as_mut().after_new_mail(ids);
                                store.changed();
                            }
                        });
                    };
                    crate::live::sync(acct, known, &marks, false, &progress, Some(&fresh))
                } else {
                    // Mail only: calendars and contacts stay as the first stage left them.
                    let mut imp = Imported { source: acct.source(), server_truth: true, failed: vec!["calendars".into(), "contacts".into()], ..Default::default() };
                    // Older mail is shown as it arrives, a batch at a time.
                    let t = thread.clone();
                    let batch = move |msgs: Vec<crate::data::Message>| {
                        let _ = t.queue(move |mut store| {
                            if !store.as_mut().db_mut().add_messages(msgs).is_empty() {
                                store.changed_soon();
                            }
                        });
                    };
                    let errors = match crate::live::sync_mail(acct, &mut imp, known, &marks, everything, &progress, Some(&batch), true) {
                        Ok(()) => Vec::new(),
                        Err(e) => vec![format!("Mail: {e}")],
                    };
                    (imp, errors)
                };
                for e in &errors {
                    let part = if e.starts_with("Mail") { "mail" } else if e.starts_with("Calendars") { "calendars" } else if e.starts_with("Contacts") { "contacts" } else { "all" };
                    if part == "all" {
                        imp.failed = vec!["mail".into(), "calendars".into(), "contacts".into()];
                    } else {
                        imp.failed.push(part.into());
                    }
                }
                let (tx, rx) = std::sync::mpsc::channel();
                let id = acct.id.clone();
                let name = acct.name.clone();
                let _ = thread.queue(move |mut store| {
                    let now = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
                    // The first stage sees the server as it is now (folders too); the
                    // second, long one only adds what it found to folders still here.
                    let new_ids = if first {
                        store.as_mut().db_mut().apply_import(imp)
                    } else {
                        let src = imp.source.clone();
                        store.as_mut().db_mut().apply_late_mail(&src, imp)
                    };
                    // Older mail filled in later isn't "new mail" (no alerts or rules for
                    // it), except what really arrived meanwhile.
                    let new_ids: Vec<i64> = if first {
                        new_ids
                    } else {
                        new_ids.into_iter().filter(|i| store.db.get(*i).is_some_and(|m| m.date >= started - 600)).collect()
                    };
                    let summary = match new_ids.len() {
                        0 => format!("{name}: up to date"),
                        1 => format!("{name}: 1 new message"),
                        n => format!("{name}: {n} new messages"),
                    };
                    store.as_mut().after_new_mail(new_ids);
                    if let Some(a) = store.as_mut().db_mut().live.iter_mut().find(|a| a.id == id) {
                        a.last_sync = now;
                        a.last_error = errors.join("; ");
                    }
                    if first || !errors.is_empty() {
                        let text = if errors.is_empty() { summary } else { format!("{name}: {}", errors.join("; ")) };
                        store.as_mut().set_status_text(QString::from(text.as_str()));
                    }
                    let src = format!("live:{id}");
                    if store.db.backed {
                        // Saved, then worked out on the sync's thread (see below).
                        store.as_mut().flush();
                        let _ = tx.send(Err((src, store.db.folders.clone())));
                    } else {
                        let _ = tx.send(Ok((store.db.partial_messages(&src), store.db.known_uids(&src))));
                    }
                    store.changed();
                });
                match rx.recv_timeout(std::time::Duration::from_secs(300)).ok()? {
                    Ok(x) => Some(x),
                    Err((src, folders)) => Some((crate::data::partial_wanted(&src, &folders), crate::mailstore::known_uids(&src))),
                }
            };
            let texts = |acct: &crate::live::LiveAccount, wanted: &[crate::live::Wanted], lane: crate::live::Lane| {
                if wanted.is_empty() {
                    return;
                }
                let progress = progress_for(thread.clone());
                let t = thread.clone();
                let deliver = move |texts: Vec<crate::live::Text>| {
                    // Saved here, on the download's thread; the window's thread only
                    // updates copies it holds in memory (an open message) and redraws.
                    let texts = if backed {
                        let stored = crate::mailstore::store_texts(&texts);
                        if stored.is_empty() {
                            return;
                        }
                        texts
                    } else {
                        texts
                    };
                    let _ = t.queue(move |mut store| {
                        let texts: Vec<crate::live::Text> = if store.db.backed {
                            let here: std::collections::HashSet<i64> = store.db.messages.iter().map(|m| m.id).collect();
                            texts.into_iter().filter(|x| here.contains(&x.id)).collect()
                        } else {
                            texts
                        };
                        store.as_mut().db_mut().fill_texts(texts);
                        store.changed_soon();
                    });
                };
                if let Err(e) = crate::live::fetch_texts(acct, wanted, lane, &deliver, &progress) {
                    progress(format!("{}: {e}", acct.name));
                }
            };
            // Stage one: what's newest, everywhere.
            let mut after: Vec<(crate::live::LiveAccount, Vec<crate::live::Wanted>, Known)> = Vec::new();
            for acct in live {
                let (known, _) = known_for.get(&acct.id).cloned().unwrap_or_default();
                let known = known.unwrap_or_else(|| crate::mailstore::known_uids(&acct.source()));
                if let Some((wanted, now_known)) = stage(&acct, &known, false, true) {
                    after.push((acct, wanted, now_known));
                }
            }
            // Free for the next check for new mail (which skips the older-mail stage
            // while this one is at it).
            let _ = thread.queue(|mut store| {
                store.as_mut().set_progress_total(0);
                store.as_mut().set_busy(false);
                if std::mem::take(&mut store.as_mut().rust_mut().get_mut().check_again) {
                    store.as_mut().refresh_all();
                }
            });
            // The newest messages' text first (all of it, or with "keep all mail"
            // the first few hundred, so the list of older mail isn't kept waiting).
            for (acct, wanted, _) in &after {
                let first = if deep { &wanted[..wanted.len().min(500)] } else { &wanted[..] };
                texts(acct, first, crate::live::Lane::Recent);
            }
            // Stage two: everything older, then its text.
            if deep {
                for (acct, _, known) in &after {
                    if let Some((wanted, _)) = stage(acct, known, true, false) {
                        texts(acct, &wanted, crate::live::Lane::Whole);
                    }
                }
                DEEP_RUNNING.store(false, Ordering::SeqCst);
            }
            let _ = thread.queue(|mut store| {
                if *store.progress_total() > 0 {
                    store.as_mut().set_progress_total(0);
                    store.as_mut().set_status_text(QString::from("All messages downloaded."));
                }
            });
        });
    }

    /// A message opened before its text has arrived: get that one now.
    fn fetch_text_now(&self, id: i64) {
        let _timed = crate::Timed::new("fetch_text_now");
        self.fetch_texts_now(vec![id]);
    }

    /// Messages opened before their text has arrived (a whole conversation,
    /// say): get them now, over one connection per account (mail servers
    /// refuse more than a few at once, and the background download has some).
    fn fetch_texts_now(&self, ids: Vec<i64>) {
        let _timed = crate::Timed::new("fetch_texts_now");
        static ASKED: std::sync::Mutex<Vec<i64>> = std::sync::Mutex::new(Vec::new());
        let ids: Vec<i64> = {
            let mut asked = ASKED.lock().unwrap();
            let fresh: Vec<i64> = ids.into_iter().filter(|id| !asked.contains(id)).collect();
            asked.extend(&fresh);
            fresh
        };
        if ids.is_empty() {
            return;
        }
        let mut by_account: Vec<(crate::live::LiveAccount, Vec<crate::live::Wanted>)> = Vec::new();
        for id in &ids {
            let Some((acct, path, uid)) = self.db.live_message(*id) else { continue };
            match by_account.iter_mut().find(|(a, _)| a.id == acct.id) {
                Some((_, list)) => list.push((*id, path, uid)),
                None => by_account.push((acct, vec![(*id, path, uid)])),
            }
        }
        for (acct, wanted) in by_account {
            let thread = self.qt_thread();
            let done: Vec<i64> = wanted.iter().map(|w| w.0).collect();
            std::thread::spawn(move || {
                let t = thread.clone();
                let deliver = move |texts: Vec<crate::live::Text>| {
                    let _ = t.queue(move |mut store| {
                        if store.as_mut().db_mut().fill_texts(texts) {
                            store.changed();
                        }
                    });
                };
                let started = std::time::Instant::now();
                let result = crate::live::fetch_texts(&acct, &wanted, crate::live::Lane::Opened, &deliver, &|_| {});
                // Slow ones are noted (to find out why, should it happen).
                if started.elapsed().as_secs() >= 3 {
                    eprintln!("almanac: getting the text of {} message(s) from {} took {:.1} s", wanted.len(), acct.name, started.elapsed().as_secs_f32());
                }
                if let Err(e) = result {
                    eprintln!("getting the text of {} opened message(s) from {}: {e}", wanted.len(), acct.name);
                    let text = format!("Couldn't download this message from {}: {e}", acct.name);
                    let _ = thread.queue(move |store| store.set_status_text(QString::from(text.as_str())));
                }
                // A failed one can be asked for again when it's next opened.
                ASKED.lock().unwrap().retain(|x| !done.contains(x));
            });
        }
    }

    fn set_auto_import(mut self: Pin<&mut Self>, id: &QString, on: bool) {
        let _timed = crate::Timed::new("set_auto_import");
        self.as_mut().db_mut().source_settings(&id.to_string()).auto_import = on;
        self.changed();
    }

    /// Back to the starting examples; live accounts stay and sync again.
    fn reset_sample_data(mut self: Pin<&mut Self>) {
        let _timed = crate::Timed::new("reset_sample_data");
        let sources = std::mem::take(&mut self.as_mut().db_mut().sources);
        let live = std::mem::take(&mut self.as_mut().db_mut().live);
        let theme = std::mem::take(&mut self.as_mut().db_mut().theme);
        let db = self.as_mut().db_mut();
        *db = Db::sample();
        db.sources = sources;
        db.live = live.clone();
        db.theme = theme;
        self.as_mut().changed();
        if !live.is_empty() {
            self.start_jobs(vec![], live);
        }
    }
}

/// A new, unique Message-ID for a draft.
fn new_message_id() -> String {
    use ring::rand::SecureRandom;
    let mut b = [0u8; 12];
    let _ = ring::rand::SystemRandom::new().fill(&mut b);
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("<{}.{hex}@almanac.local>", chrono::Utc::now().timestamp_millis())
}
