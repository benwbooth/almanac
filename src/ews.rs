//! Microsoft Exchange servers through Exchange Web Services (EWS), as Apple
//! Mail and Thunderbird reach company Exchange servers: the server is found
//! from the address (Autodiscover), then folders and mail are read, read /
//! flagged / moved / deleted mail is changed on the server, and mail is sent
//! through it (a copy kept in Sent Items). Sign-in is the user name and
//! password (Exchange's "basic" sign-in). Microsoft 365 and Outlook.com use the
//! Microsoft sign-in instead (see oauth.rs).

use crate::data::{Account, Folder, Imported, MailDelta, Message};
use crate::live::{LiveAccount, MailOp};
use base64::Engine;
use std::collections::HashMap;
use std::sync::Mutex;

// ------------------------------------------------------------- a little XML tree

#[derive(Debug, Default, Clone)]
pub struct Node {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
    pub text: String,
}

impl Node {
    /// The first element named `name` anywhere below (or this one).
    pub fn find(&self, name: &str) -> Option<&Node> {
        if self.name == name {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.find(name))
    }
    /// Every element named `name` below.
    pub fn all<'a>(&'a self, name: &str, out: &mut Vec<&'a Node>) {
        for c in &self.children {
            if c.name == name {
                out.push(c);
            }
            c.all(name, out);
        }
    }
    pub fn list(&self, name: &str) -> Vec<&Node> {
        let mut v = Vec::new();
        self.all(name, &mut v);
        v
    }
    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }
    /// The text of the first element named `name` below ("" if none).
    pub fn text_of(&self, name: &str) -> String {
        self.find(name).map(|n| n.text.clone()).unwrap_or_default()
    }
}

/// Reads XML into a tree, names without their namespace prefix.
pub fn parse(xml: &str) -> Option<Node> {
    use quick_xml::events::Event as X;
    let local = |q: &[u8]| {
        let s = String::from_utf8_lossy(q).to_string();
        s.rsplit(':').next().unwrap_or("").to_string()
    };
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut stack: Vec<Node> = vec![Node::default()];
    loop {
        match reader.read_event().ok()? {
            X::Start(e) => {
                let attrs = e.attributes().filter_map(|a| a.ok()).map(|a| (local(a.key.as_ref()), a.unescape_value().map(|v| v.to_string()).unwrap_or_default())).collect();
                stack.push(Node { name: local(e.name().as_ref()), attrs, ..Default::default() });
            }
            X::Empty(e) => {
                let attrs = e.attributes().filter_map(|a| a.ok()).map(|a| (local(a.key.as_ref()), a.unescape_value().map(|v| v.to_string()).unwrap_or_default())).collect();
                let n = Node { name: local(e.name().as_ref()), attrs, ..Default::default() };
                stack.last_mut()?.children.push(n);
            }
            X::Text(t) => {
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&t.decode().map(|s| s.to_string()).unwrap_or_default());
                }
            }
            // Character references (&amp; &#233; …) come separately.
            X::GeneralRef(r) => {
                let name = r.decode().map(|s| s.to_string()).unwrap_or_default();
                let ch = match name.as_str() {
                    "amp" => "&".to_string(),
                    "lt" => "<".to_string(),
                    "gt" => ">".to_string(),
                    "quot" => "\"".to_string(),
                    "apos" => "'".to_string(),
                    n if n.starts_with("#x") => u32::from_str_radix(&n[2..], 16).ok().and_then(char::from_u32).map(String::from).unwrap_or_default(),
                    n if n.starts_with('#') => n[1..].parse().ok().and_then(char::from_u32).map(String::from).unwrap_or_default(),
                    _ => String::new(),
                };
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&ch);
                }
            }
            X::CData(t) => {
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&String::from_utf8_lossy(&t));
                }
            }
            X::End(_) => {
                let n = stack.pop()?;
                stack.last_mut()?.children.push(n);
            }
            X::Eof => break,
            _ => {}
        }
    }
    stack.pop()
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

// ------------------------------------------------------------------ talking to it

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(std::time::Duration::from_secs(120)))
        .build()
        .into()
}

fn basic(user: &str, pass: &str) -> String {
    format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!("{user}:{pass}")))
}

fn envelope(body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"
  xmlns:t="http://schemas.microsoft.com/exchange/services/2006/types"
  xmlns:m="http://schemas.microsoft.com/exchange/services/2006/messages">
<soap:Header><t:RequestServerVersion Version="Exchange2013"/></soap:Header>
<soap:Body>{body}</soap:Body></soap:Envelope>"#
    )
}

/// The account's EWS address: the one given, else found by Autodiscover (kept for the run).
fn endpoint(a: &LiveAccount, pass: &str) -> Result<String, String> {
    static FOUND: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);
    if a.imap_host.starts_with("https://") || a.imap_host.starts_with("http://") {
        return Ok(a.imap_host.clone());
    }
    if let Some(u) = FOUND.lock().unwrap().as_ref().and_then(|m| m.get(&a.email).cloned()) {
        return Ok(u);
    }
    let url = autodiscover(&a.email, &a.username, pass)
        .or_else(|| (!a.imap_host.is_empty()).then(|| format!("https://{}/EWS/Exchange.asmx", a.imap_host.trim_end_matches('/'))))
        .ok_or_else(|| "Couldn't find the Exchange server for this address; enter its EWS address (https://…/EWS/Exchange.asmx) in Server settings.".to_string())?;
    FOUND.lock().unwrap().get_or_insert_with(HashMap::new).insert(a.email.clone(), url.clone());
    Ok(url)
}

/// Exchange's own way of telling an address's server (the "POX" Autodiscover).
pub fn autodiscover(email: &str, user: &str, pass: &str) -> Option<String> {
    let domain = email.rsplit('@').next()?.to_lowercase();
    let req = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<Autodiscover xmlns="http://schemas.microsoft.com/exchange/autodiscover/outlook/requestschema/2006">
<Request><EMailAddress>{}</EMailAddress>
<AcceptableResponseSchema>http://schemas.microsoft.com/exchange/autodiscover/outlook/responseschema/2006a</AcceptableResponseSchema>
</Request></Autodiscover>"#,
        esc(email)
    );
    for url in [format!("https://autodiscover.{domain}/autodiscover/autodiscover.xml"), format!("https://{domain}/autodiscover/autodiscover.xml")] {
        let Ok(mut r) = agent().post(&url).header("Authorization", basic(user, pass)).header("Content-Type", "text/xml; charset=utf-8").send(req.as_bytes()) else { continue };
        if !r.status().is_success() {
            continue;
        }
        let Ok(body) = r.body_mut().read_to_string() else { continue };
        if let Some(found) = ews_url_in(&body) {
            return Some(found);
        }
    }
    None
}

/// The EWS address in an Autodiscover answer (inside or outside the company network).
pub fn ews_url_in(xml: &str) -> Option<String> {
    let doc = parse(xml)?;
    let protocols = doc.list("Protocol");
    for kind in ["EXCH", "EXPR", "WEB"] {
        if let Some(p) = protocols.iter().find(|p| p.text_of("Type") == kind) {
            let u = p.text_of("EwsUrl");
            if !u.is_empty() {
                return Some(u);
            }
        }
    }
    None
}

/// One EWS call; the answer's tree, or the server's error in words.
fn call(a: &LiveAccount, body: &str) -> Result<Node, String> {
    let pass = crate::live::need_password(a)?;
    let url = endpoint(a, &pass)?;
    let mut r = agent()
        .post(&url)
        .header("Authorization", basic(&a.username, &pass))
        .header("Content-Type", "text/xml; charset=utf-8")
        .send(envelope(body).as_bytes())
        .map_err(|e| format!("Could not reach the Exchange server: {e}"))?;
    let status = r.status().as_u16();
    let text = r.body_mut().read_to_string().unwrap_or_default();
    if status == 401 || status == 403 {
        return Err("The Exchange server did not accept the user name or password (some servers want DOMAIN\\name as the user name).".into());
    }
    let doc = parse(&text).ok_or_else(|| format!("The Exchange server answered with something unexpected (status {status})"))?;
    if let Some(fault) = doc.find("Fault") {
        return Err(format!("Exchange: {}", fault.text_of("faultstring")));
    }
    // Every part of a request must have gone well.
    for rm in doc.list("ResponseMessages").iter().flat_map(|r| r.children.iter()) {
        if rm.attr("ResponseClass") == Some("Error") {
            return Err(format!("Exchange: {}", rm.text_of("MessageText")));
        }
    }
    Ok(doc)
}

// ------------------------------------------------------------------- server ids

/// Exchange's long ids for messages, by (folder, the number Almanac uses) —
/// kept in a file so changes can be sent back after a restart.
static IDS: Mutex<Option<HashMap<String, HashMap<String, String>>>> = Mutex::new(None);

fn ids_file(acct: &str) -> std::path::PathBuf {
    crate::data::data_dir().join("ews").join(format!("{}.json", acct.replace(['/', '\\', ':'], "_")))
}

fn with_ids<T>(acct: &str, f: impl FnOnce(&mut HashMap<String, String>) -> T) -> T {
    let mut all = IDS.lock().unwrap();
    let all = all.get_or_insert_with(HashMap::new);
    let map = all.entry(acct.to_string()).or_insert_with(|| std::fs::read_to_string(ids_file(acct)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default());
    f(map)
}

fn save_ids(acct: &str) {
    with_ids(acct, |m| {
        let path = ids_file(acct);
        let _ = std::fs::create_dir_all(path.parent().unwrap());
        let _ = std::fs::write(&path, serde_json::to_string(m).unwrap_or_default());
    });
}

fn key(folder_path: &str, uid: u32) -> String {
    format!("{folder_path}#{uid}")
}

// ------------------------------------------------------------------------- mail

/// Folders and new mail, with read / flagged changes and mail gone from the server.
pub fn sync(a: &LiveAccount, imp: &mut Imported, known: &HashMap<String, Vec<u32>>, all: bool, progress: &dyn Fn(String)) -> Result<(), String> {
    let account = format!("{}:mail", a.source());
    imp.accounts.push(Account { id: account.clone(), name: a.name.clone(), email: a.email.clone(), source: a.source() });
    progress(format!("{}: reading folders…", a.name));
    // Which folders are the Inbox, Sent, Drafts…
    let mut kinds: HashMap<String, &str> = HashMap::new();
    let special = [("inbox", "inbox"), ("sentitems", "sent"), ("drafts", "drafts"), ("deleteditems", "trash"), ("junkemail", "junk")];
    let ids: String = special.iter().map(|(d, _)| format!(r#"<t:DistinguishedFolderId Id="{d}"/>"#)).collect();
    if let Ok(doc) = call(a, &format!(r#"<m:GetFolder><m:FolderShape><t:BaseShape>IdOnly</t:BaseShape></m:FolderShape><m:FolderIds>{ids}</m:FolderIds></m:GetFolder>"#)) {
        for (f, (_, kind)) in doc.list("FolderId").iter().zip(special.iter()) {
            if let Some(id) = f.attr("Id") {
                kinds.insert(id.to_string(), kind);
            }
        }
    }
    let doc = call(
        a,
        r#"<m:FindFolder Traversal="Deep"><m:FolderShape><t:BaseShape>Default</t:BaseShape>
           <t:AdditionalProperties><t:FieldURI FieldURI="folder:FolderClass"/></t:AdditionalProperties></m:FolderShape>
           <m:ParentFolderIds><t:DistinguishedFolderId Id="msgfolderroot"/></m:ParentFolderIds></m:FindFolder>"#,
    )?;
    let mut folders: Vec<(String, String, String)> = doc
        .list("Folder")
        .iter()
        .filter(|f| matches!(f.text_of("FolderClass").as_str(), "" | "IPF.Note") || f.text_of("FolderClass").starts_with("IPF.Note."))
        .filter_map(|f| {
            let id = f.child("FolderId")?.attr("Id")?.to_string();
            let kind = kinds.get(&id).copied().unwrap_or("other").to_string();
            Some((id, f.text_of("DisplayName"), kind))
        })
        .collect();
    let rank = |k: &str| ["inbox", "drafts", "sent", "archive", "junk", "trash"].iter().position(|x| *x == k).unwrap_or(9);
    folders.sort_by(|x, y| rank(&x.2).cmp(&rank(&y.2)).then(x.1.to_lowercase().cmp(&y.1.to_lowercase())));
    let dir = crate::data::data_dir().join("ews").join(a.id.replace(['/', '\\', ':'], "_"));
    let _ = std::fs::create_dir_all(&dir);
    let mut delta = MailDelta::default();
    for (path, name, kind) in folders {
        let folder_id = format!("{account}/{path}");
        let pretty = if kind == "inbox" { "Inbox".to_string() } else { name };
        imp.folders.push(Folder { id: folder_id.clone(), account: account.clone(), name: pretty.clone(), kind: kind.clone(), path: path.clone() });
        progress(format!("{}: reading {pretty}…", a.name));
        // What's in the folder, newest first, a page at a time.
        let mut items: Vec<(String, bool, bool)> = Vec::new();
        let mut offset = 0;
        loop {
            let doc = call(
                a,
                &format!(
                    r#"<m:FindItem Traversal="Shallow"><m:ItemShape><t:BaseShape>IdOnly</t:BaseShape><t:AdditionalProperties>
                       <t:FieldURI FieldURI="message:IsRead"/><t:FieldURI FieldURI="item:Flag"/></t:AdditionalProperties></m:ItemShape>
                       <m:IndexedPageItemView MaxEntriesReturned="500" Offset="{offset}" BasePoint="Beginning"/>
                       <m:SortOrder><t:FieldOrder Order="Descending"><t:FieldURI FieldURI="item:DateTimeReceived"/></t:FieldOrder></m:SortOrder>
                       <m:ParentFolderIds><t:FolderId Id="{}"/></m:ParentFolderIds></m:FindItem>"#,
                    esc(&path)
                ),
            )?;
            let page: Vec<(String, bool, bool)> = doc
                .list("Message")
                .iter()
                .filter_map(|m| Some((m.child("ItemId")?.attr("Id")?.to_string(), m.text_of("IsRead") == "true", m.text_of("FlagStatus") == "Flagged")))
                .collect();
            let n = page.len();
            items.extend(page);
            let last = doc.find("RootFolder").and_then(|r| r.attr("IncludesLastItemInRange")).map(|v| v == "true").unwrap_or(true);
            offset += n;
            if last || n == 0 || (!all && items.len() >= 400) {
                break;
            }
        }
        let here: std::collections::HashSet<u32> = known.get(&folder_id).map(|v| v.iter().copied().collect()).unwrap_or_default();
        let on_server: std::collections::HashSet<u32> = items.iter().map(|(id, _, _)| crate::pop::uid_of(id)).collect();
        with_ids(&a.id, |m| {
            for (id, _, _) in &items {
                m.insert(key(&path, crate::pop::uid_of(id)), id.clone());
            }
        });
        if all || items.len() < 400 {
            for uid in &here {
                if !on_server.contains(uid) {
                    delta.gone.push((folder_id.clone(), *uid));
                }
            }
        }
        for (id, read, flagged) in &items {
            let uid = crate::pop::uid_of(id);
            if here.contains(&uid) {
                delta.flags.push((folder_id.clone(), uid, *read, *flagged, crate::data::NO_COLOR));
            }
        }
        // New mail, whole (text and attachments), a few at a time.
        let new: Vec<&(String, bool, bool)> = items.iter().filter(|(id, _, _)| !here.contains(&crate::pop::uid_of(id))).collect();
        let total = new.len();
        for (i, chunk) in new.chunks(25).enumerate() {
            if total > 25 {
                progress(crate::live::counted(&format!("{}: {pretty}: downloading messages", a.name), i * 25, total));
            }
            let wanted: String = chunk.iter().map(|(id, _, _)| format!(r#"<t:ItemId Id="{}"/>"#, esc(id))).collect();
            let doc = call(
                a,
                &format!(r#"<m:GetItem><m:ItemShape><t:BaseShape>IdOnly</t:BaseShape><t:IncludeMimeContent>true</t:IncludeMimeContent></m:ItemShape><m:ItemIds>{wanted}</m:ItemIds></m:GetItem>"#),
            )?;
            for (msg, (id, read, flagged)) in doc.list("Message").iter().zip(chunk.iter()) {
                let Ok(raw) = base64::engine::general_purpose::STANDARD.decode(msg.text_of("MimeContent").replace(['\r', '\n'], "")) else { continue };
                let uid = crate::pop::uid_of(id);
                let file = dir.join(format!("{uid}.eml"));
                let _ = std::fs::write(&file, &raw);
                let p = crate::mime::parse(&raw);
                delta.new.push(Message {
                    folder: folder_id.clone(),
                    source: a.source(),
                    uid,
                    from_name: p.from_name,
                    from_email: p.from_email,
                    to: p.to,
                    cc: p.cc,
                    subject: p.subject,
                    date: p.date,
                    body: p.body,
                    html: p.html,
                    invite: p.invite,
                    attachments: p.attachments,
                    message_id: p.message_id,
                    in_reply_to: p.in_reply_to,
                    references: p.references,
                    reply_to: p.reply_to,
                    read: *read,
                    flagged: *flagged,
                    raw_ref: format!("mbox:{}:0:{}", file.display(), raw.len()),
                    ..Default::default()
                });
            }
        }
    }
    save_ids(&a.id);
    imp.mail_delta = Some(delta);
    Ok(())
}

/// Read / flagged / moved / deleted mail, changed on the server too.
pub fn op(a: &LiveAccount, folder_path: &str, uids: &[u32], op: MailOp) -> Result<(), String> {
    // (Exchange has no flag colours: flagged or not.)
    let op = match op {
        MailOp::FlagColor(color) => MailOp::Flagged(color.is_some()),
        other => other,
    };
    let items: Vec<String> = with_ids(&a.id, |m| uids.iter().filter_map(|u| m.get(&key(folder_path, *u)).cloned()).collect());
    if items.is_empty() {
        return Ok(());
    }
    let ids: String = items.iter().map(|id| format!(r#"<t:ItemId Id="{}"/>"#, esc(id))).collect();
    let body = match op {
        MailOp::FlagColor(_) => unreachable!("made Flagged above"),
        MailOp::Seen(on) | MailOp::Flagged(on) => {
            let (field, value) = if matches!(op, MailOp::Seen(_)) {
                ("message:IsRead", format!("<t:Message><t:IsRead>{on}</t:IsRead></t:Message>"))
            } else {
                ("item:Flag", format!("<t:Message><t:Flag><t:FlagStatus>{}</t:FlagStatus></t:Flag></t:Message>", if on { "Flagged" } else { "NotFlagged" }))
            };
            let changes: String = items
                .iter()
                .map(|id| format!(r#"<t:ItemChange><t:ItemId Id="{}"/><t:Updates><t:SetItemField><t:FieldURI FieldURI="{field}"/>{value}</t:SetItemField></t:Updates></t:ItemChange>"#, esc(id)))
                .collect();
            format!(r#"<m:UpdateItem ConflictResolution="AlwaysOverwrite" MessageDisposition="SaveOnly" SuppressReadReceipts="true"><m:ItemChanges>{changes}</m:ItemChanges></m:UpdateItem>"#)
        }
        MailOp::Move(to) => format!(r#"<m:MoveItem><m:ToFolderId><t:FolderId Id="{}"/></m:ToFolderId><m:ItemIds>{ids}</m:ItemIds></m:MoveItem>"#, esc(&to)),
        MailOp::Expunge => format!(r#"<m:DeleteItem DeleteType="HardDelete"><m:ItemIds>{ids}</m:ItemIds></m:DeleteItem>"#),
    };
    call(a, &body).map(|_| ())
}

/// Sends a whole message through the server, keeping a copy in Sent Items.
pub fn send(a: &LiveAccount, mime: &[u8]) -> Result<(), String> {
    let b64 = base64::engine::general_purpose::STANDARD.encode(mime);
    call(
        a,
        &format!(
            r#"<m:CreateItem MessageDisposition="SendAndSaveCopy"><m:SavedItemFolderId><t:DistinguishedFolderId Id="sentitems"/></m:SavedItemFolderId>
               <m:Items><t:Message><t:MimeContent CharacterSet="UTF-8">{b64}</t:MimeContent></t:Message></m:Items></m:CreateItem>"#
        ),
    )
    .map(|_| ())
    .map_err(|e| format!("Sending failed: {e}"))
}

/// Checks that the server takes the sign-in.
pub fn test(a: &LiveAccount) -> Result<(), String> {
    call(a, r#"<m:GetFolder><m:FolderShape><t:BaseShape>IdOnly</t:BaseShape></m:FolderShape><m:FolderIds><t:DistinguishedFolderId Id="inbox"/></m:FolderIds></m:GetFolder>"#).map(|_| ())
}

#[cfg(test)]
mod tests {
    #[test]
    fn finds_the_server_in_an_autodiscover_answer() {
        let xml = r#"<?xml version="1.0"?><Autodiscover xmlns="http://schemas.microsoft.com/exchange/autodiscover/responseschema/2006">
          <Response xmlns="http://schemas.microsoft.com/exchange/autodiscover/outlook/responseschema/2006a"><Account>
          <Protocol><Type>WEB</Type></Protocol>
          <Protocol><Type>EXPR</Type><EwsUrl>https://mail.example.org/EWS/Exchange.asmx</EwsUrl></Protocol>
          <Protocol><Type>EXCH</Type><EwsUrl>https://exch01.corp.example.org/EWS/Exchange.asmx</EwsUrl></Protocol>
          </Account></Response></Autodiscover>"#;
        assert_eq!(super::ews_url_in(xml).as_deref(), Some("https://exch01.corp.example.org/EWS/Exchange.asmx"));
    }

    #[test]
    fn reads_a_find_item_answer() {
        let xml = r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body>
          <m:FindItemResponse xmlns:m="m" xmlns:t="t"><m:ResponseMessages><m:FindItemResponseMessage ResponseClass="Success">
          <m:RootFolder TotalItemsInView="2" IncludesLastItemInRange="true"><t:Items>
            <t:Message><t:ItemId Id="AAMkAD1=" ChangeKey="CQ"/><t:IsRead>true</t:IsRead><t:Flag><t:FlagStatus>Flagged</t:FlagStatus></t:Flag></t:Message>
            <t:Message><t:ItemId Id="AAMkAD2=" ChangeKey="CQ"/><t:IsRead>false</t:IsRead></t:Message>
          </t:Items></m:RootFolder></m:FindItemResponseMessage></m:ResponseMessages></m:FindItemResponse></s:Body></s:Envelope>"#;
        let doc = super::parse(xml).unwrap();
        let msgs = doc.list("Message");
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].child("ItemId").unwrap().attr("Id"), Some("AAMkAD1="));
        assert_eq!(msgs[0].text_of("FlagStatus"), "Flagged");
        assert_eq!(msgs[1].text_of("IsRead"), "false");
        assert_eq!(doc.find("RootFolder").unwrap().attr("IncludesLastItemInRange"), Some("true"));
    }
}
