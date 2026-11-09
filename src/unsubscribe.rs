//! Unsubscribing from mailing lists and newsletters, as Gmail does: from the
//! List-Unsubscribe header (RFC 2369) and, when the list offers it, the
//! one-click POST of RFC 8058 (List-Unsubscribe-Post). Nothing is ever done
//! without the person's click.

use serde::Serialize;

/// What a message's headers say about leaving its list.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ListInfo {
    /// The list's name (List-Id's description, else its id, else the sender).
    pub list: String,
    /// The list's id (List-Id), or the sender's address: what's remembered as
    /// unsubscribed.
    pub key: String,
    /// One click: an https address to POST "List-Unsubscribe=One-Click" to.
    pub one_click: Option<String>,
    /// An address to email to leave (mailto:…).
    pub mailto: Option<String>,
    /// A web page to leave on.
    pub https: Option<String>,
}

impl ListInfo {
    pub fn can_unsubscribe(&self) -> bool {
        self.one_click.is_some() || self.mailto.is_some() || self.https.is_some()
    }
}

/// A header's value (unfolded), by name.
fn header(headers: &str, name: &str) -> Option<String> {
    let mut out: Option<String> = None;
    for line in headers.lines() {
        if line.is_empty() {
            break;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(v) = out.as_mut() {
                v.push(' ');
                v.push_str(line.trim());
            }
            continue;
        }
        if out.is_some() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case(name) {
                out = Some(v.trim().to_string());
            }
        }
    }
    out
}

/// The list's header lines from a message's headers (unfolded, CRLF ended),
/// to keep: "" when it isn't from a list.
pub fn list_headers(raw: &[u8]) -> String {
    let end = raw.windows(4).position(|w| w == b"\r\n\r\n").or_else(|| raw.windows(2).position(|w| w == b"\n\n")).unwrap_or(raw.len());
    let text = String::from_utf8_lossy(&raw[..end]).replace("\r\n", "\n");
    if header(&text, "List-Unsubscribe").is_none() {
        return String::new();
    }
    let mut out = String::new();
    for name in ["List-Id", "List-Unsubscribe", "List-Unsubscribe-Post"] {
        if let Some(v) = header(&text, name) {
            out.push_str(&format!("{name}: {v}\r\n"));
        }
    }
    out
}

/// Reads a message's headers (the start of its source: up to the first
/// blank line). None when it isn't from a list one can leave.
pub fn parse(raw: &[u8], from_email: &str) -> Option<ListInfo> {
    let end = raw.windows(4).position(|w| w == b"\r\n\r\n").or_else(|| raw.windows(2).position(|w| w == b"\n\n")).unwrap_or(raw.len());
    let text = String::from_utf8_lossy(&raw[..end]).replace("\r\n", "\n");
    let unsub = header(&text, "List-Unsubscribe")?;
    let post = header(&text, "List-Unsubscribe-Post").unwrap_or_default();
    let mut info = ListInfo::default();
    for part in unsub.split(',') {
        let p = part.trim().trim_start_matches('<').trim_end_matches('>').trim();
        let lower = p.to_ascii_lowercase();
        if lower.starts_with("mailto:") && info.mailto.is_none() {
            info.mailto = Some(p.to_string());
        } else if lower.starts_with("https://") && info.https.is_none() {
            info.https = Some(p.to_string());
        }
    }
    // One click: only an https address, and only when the list says so.
    if post.to_ascii_lowercase().replace(' ', "").contains("list-unsubscribe=one-click") {
        info.one_click = info.https.clone();
    }
    if !info.can_unsubscribe() {
        return None;
    }
    let list_id = header(&text, "List-Id").unwrap_or_default();
    let (name, id) = match (list_id.find('<'), list_id.rfind('>')) {
        (Some(a), Some(b)) if b > a => (list_id[..a].trim().trim_matches('"').to_string(), list_id[a + 1..b].trim().to_lowercase()),
        _ => (String::new(), list_id.trim().to_lowercase()),
    };
    info.key = if id.is_empty() { from_email.trim().to_lowercase() } else { id.clone() };
    info.list = if !name.is_empty() { name } else if !id.is_empty() { id } else { from_email.to_string() };
    Some(info)
}

/// The message's own unsubscribe link, for mail that leaves out the
/// List-Unsubscribe header (Steam, for one, puts its "Unsubscribe" only in
/// the text): a link reading "unsubscribe" or "opt out", or one whose address
/// says so, else a "click here" just after the word. The last one wins (it's
/// in the footer). Only web pages, which are opened, never visited by us.
pub fn body_link(html: &str, text: &str) -> Option<String> {
    use std::sync::OnceLock;
    static A: OnceLock<regex::Regex> = OnceLock::new();
    static TAG: OnceLock<regex::Regex> = OnceLock::new();
    static URL: OnceLock<regex::Regex> = OnceLock::new();
    let a = A.get_or_init(|| regex::Regex::new(r#"(?is)<a\s[^>]*?href\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))[^>]*>(.*?)</a\s*>"#).unwrap());
    let tag = TAG.get_or_init(|| regex::Regex::new(r"(?s)<[^>]*>").unwrap());
    let url = URL.get_or_init(|| regex::Regex::new(r#"(?i)https?://[^\s<>"')\]]+"#).unwrap());
    let words = |s: &str| -> String {
        let s = tag.replace_all(s, " ");
        let s = s.replace("&nbsp;", " ").replace("&#160;", " ").to_lowercase();
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    };
    let says = |s: &str| {
        let s = s.replace(['-', '_'], " ");
        s.contains("unsubscribe") || s.contains("opt out") || s.contains("optout")
    };
    let web = |h: &str| {
        let h = h.trim().replace("&amp;", "&").replace("&#38;", "&");
        let l = h.to_ascii_lowercase();
        (l.starts_with("https://") || l.starts_with("http://")).then_some(h)
    };
    let mut found: Option<String> = None;
    let mut last_end = 0;
    for c in a.captures_iter(html) {
        let whole = c.get(0).unwrap();
        let href = c.get(1).or(c.get(2)).or(c.get(3)).map(|m| m.as_str()).unwrap_or("");
        let label = words(c.get(4).map(|m| m.as_str()).unwrap_or(""));
        let Some(h) = web(href) else {
            last_end = whole.end();
            continue;
        };
        let before = words(&html[last_end.max(html.floor_char_boundary(whole.start().saturating_sub(400)))..whole.start()]);
        let tail: String = before.chars().rev().take(80).collect::<Vec<_>>().into_iter().rev().collect();
        let here = matches!(label.as_str(), "here" | "click here" | "this link" | "here." | "click here.");
        if says(&label) || says(&h.to_ascii_lowercase().split('?').next().unwrap_or("").to_string()) || (here && says(&tail)) {
            found = Some(h);
        }
        last_end = whole.end();
    }
    if found.is_some() {
        return found;
    }
    // Plain text: an address after the word (or one that says so).
    for m in url.find_iter(text) {
        let u = m.as_str().trim_end_matches(['.', ',', ';']);
        let near = text[text.floor_char_boundary(m.start().saturating_sub(120))..m.start()].to_lowercase();
        if says(&u.to_ascii_lowercase().split('?').next().unwrap_or("").to_string()) || says(&near) {
            found = Some(u.to_string());
        }
    }
    found
}

/// A message's list from its own unsubscribe link (no List-Unsubscribe
/// header): the sender is what's remembered as unsubscribed.
pub fn from_body(html: &str, text: &str, from_name: &str, from_email: &str) -> Option<ListInfo> {
    let link = body_link(html, text)?;
    let email = from_email.trim().to_lowercase();
    Some(ListInfo {
        list: if from_name.trim().is_empty() { from_email.to_string() } else { from_name.trim().to_string() },
        key: email,
        one_click: None,
        mailto: None,
        https: Some(link),
    })
}

/// RFC 8058: POSTs "List-Unsubscribe=One-Click" to the list's https address
/// (no cookies, no following the person's browser session).
pub fn one_click(url: &str) -> Result<(), String> {
    if !url.to_ascii_lowercase().starts_with("https://") && !cfg!(test) {
        return Err("The list's unsubscribe address isn't a secure (https) one".into());
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(std::time::Duration::from_secs(30)))
        .build()
        .into();
    let resp = agent
        .post(url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("User-Agent", "Almanac")
        .send("List-Unsubscribe=One-Click")
        .map_err(|e| format!("Couldn't reach the list: {e}"))?;
    let status = resp.status().as_u16();
    if (200..400).contains(&status) {
        Ok(())
    } else {
        Err(format!("The list answered HTTP {status}"))
    }
}

/// A mailto: address taken apart: (to, subject, body).
pub fn parse_mailto(mailto: &str) -> (String, String, String) {
    let rest = mailto.trim_start_matches("mailto:").trim_start_matches("MAILTO:");
    let (to, query) = rest.split_once('?').unwrap_or((rest, ""));
    let mut subject = String::new();
    let mut body = String::new();
    for kv in query.split('&') {
        if let Some((k, v)) = kv.split_once('=') {
            let v = decode(v);
            match k.to_ascii_lowercase().as_str() {
                "subject" => subject = v,
                "body" => body = v,
                _ => {}
            }
        }
    }
    if subject.is_empty() {
        subject = "unsubscribe".into();
    }
    (decode(to), subject, body)
}

fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                    out.push(b);
                    i += 3;
                    continue;
                }
                out.push(b'%');
            }
            b'+' => out.push(b' '),
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};

    #[test]
    fn headers_say_how_to_leave() {
        let raw = b"From: Garden Club <news@garden.example>\r\nList-Id: \"Garden Club News\" <news.garden.example>\r\nList-Unsubscribe: <mailto:leave@garden.example?subject=Remove%20me>,\r\n <https://garden.example/u/abc123>\r\nList-Unsubscribe-Post: List-Unsubscribe=One-Click\r\nSubject: Spring\r\n\r\nList-Unsubscribe: <https://not.a.header/>\r\n";
        let info = parse(raw, "news@garden.example").expect("a list");
        assert_eq!(info.list, "Garden Club News");
        assert_eq!(info.key, "news.garden.example");
        assert_eq!(info.one_click.as_deref(), Some("https://garden.example/u/abc123"));
        assert_eq!(info.mailto.as_deref(), Some("mailto:leave@garden.example?subject=Remove%20me"));
        assert_eq!(parse_mailto(info.mailto.as_deref().unwrap()), ("leave@garden.example".into(), "Remove me".into(), String::new()));
        // No Post header: not one click (the page is opened instead).
        let raw = b"List-Unsubscribe: <https://shop.example/unsub?u=1>\r\n\r\n";
        let info = parse(raw, "deals@shop.example").unwrap();
        assert_eq!((info.one_click, info.https.as_deref(), info.key.as_str()), (None, Some("https://shop.example/unsub?u=1"), "deals@shop.example"));
        // Not from a list.
        assert!(parse(b"From: a@b.example\r\nSubject: hi\r\n\r\n", "a@b.example").is_none());
        assert_eq!(list_headers(b"From: a@b.example\r\nSubject: hi\r\n\r\n"), "");
        // What's kept reads the same as the whole headers.
        let raw = b"From: Garden Club <news@garden.example>\r\nList-Id: \"Garden Club News\" <news.garden.example>\r\nList-Unsubscribe: <mailto:leave@garden.example?subject=Remove%20me>,\r\n <https://garden.example/u/abc123>\r\nList-Unsubscribe-Post: List-Unsubscribe=One-Click\r\nSubject: Spring\r\n\r\n";
        let kept = list_headers(raw);
        assert_eq!(kept.lines().count(), 3);
        assert_eq!(parse(kept.as_bytes(), "news@garden.example"), parse(raw, "news@garden.example"));
    }

    #[test]
    fn one_click_posts_the_form() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/u/abc", listener.local_addr().unwrap());
        let got = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut r = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            r.read_line(&mut line).unwrap();
            let mut len = 0;
            let mut cookie = false;
            loop {
                let mut h = String::new();
                r.read_line(&mut h).unwrap();
                if h.trim().is_empty() {
                    break;
                }
                let l = h.to_ascii_lowercase();
                if let Some(v) = l.strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap();
                }
                cookie |= l.starts_with("cookie:");
            }
            let mut body = vec![0u8; len];
            r.read_exact(&mut body).unwrap();
            let mut out = stream;
            out.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n").unwrap();
            (line, String::from_utf8(body).unwrap(), cookie)
        });
        one_click(&url).expect("unsubscribed");
        let (line, body, cookie) = got.join().unwrap();
        assert!(line.starts_with("POST /u/abc "), "{line}");
        assert_eq!(body, "List-Unsubscribe=One-Click");
        assert!(!cookie, "no cookies");
    }

    #[test]
    fn links_in_the_text() {
        // Steam's footer: no header, an "Unsubscribe" link (a span inside).
        let html = r#"<p>Hi</p><a href="https://store.example/app/1">Game</a> <strong><a href="https://store.example/account/notificationsettings?token=ab&amp;snr=1_620" class="link-u"><span>Unsubscribe</span></a> | <a href="https://store.example/account/notificationsettings?token=ab">Update My Preferences</a></strong>"#;
        assert_eq!(body_link(html, "").as_deref(), Some("https://store.example/account/notificationsettings?token=ab&snr=1_620"));
        // The address says so.
        assert_eq!(body_link(r#"<a href='https://x.example/unsubscribe/42'>Manage</a>"#, "").as_deref(), Some("https://x.example/unsubscribe/42"));
        // "Click here" after the word.
        assert_eq!(body_link(r#"To unsubscribe from these emails, <a href="https://x.example/p?t=1">click here</a>."#, "").as_deref(), Some("https://x.example/p?t=1"));
        // A "click here" elsewhere isn't one; mailto links aren't either.
        assert_eq!(body_link(r#"See the sale, <a href="https://x.example/sale">click here</a>. <a href="mailto:a@x.example">Unsubscribe</a>"#, ""), None);
        // Plain text.
        assert_eq!(body_link("", "Bye.\nUnsubscribe: https://x.example/u?id=9.\n").as_deref(), Some("https://x.example/u?id=9"));
        assert_eq!(body_link("", "Read more at https://x.example/news"), None);
        let info = from_body(html, "", "Steam", "NoReply@steampowered.com").unwrap();
        assert_eq!((info.list.as_str(), info.key.as_str(), info.one_click.is_none(), info.mailto.is_none()), ("Steam", "noreply@steampowered.com", true, true));
        assert!(info.can_unsubscribe());
    }

}
