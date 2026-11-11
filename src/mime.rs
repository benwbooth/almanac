//! Minimal e-mail (RFC 5322 / MIME) reading: headers, encoded words,
//! multipart bodies, base64 and quoted-printable, character sets.

use base64::Engine;

pub struct Parsed {
    pub from_name: String,
    pub from_email: String,
    pub to: String,
    pub cc: String,
    pub subject: String,
    pub date: i64,
    pub body: String,
    pub attachments: Vec<String>,
    /// Formatted body, cleaned of scripts (empty when only plain text was sent)
    pub html: String,
    /// A calendar invitation (text/calendar part)
    pub invite: String,
    pub message_id: String,
    /// Message-IDs this one answers (In-Reply-To, then References), for conversations
    pub in_reply_to: String,
    pub references: String,
    /// Where replies should go, when not to the sender
    pub reply_to: String,
    pub mozilla_status: Option<u32>,
}

fn split_head_body(raw: &[u8]) -> (&[u8], &[u8]) {
    for i in 0..raw.len() {
        if raw[i] == b'\n' {
            if raw.get(i + 1) == Some(&b'\n') {
                return (&raw[..i], &raw[i + 2..]);
            }
            if raw.get(i + 1) == Some(&b'\r') && raw.get(i + 2) == Some(&b'\n') {
                return (&raw[..i], &raw[i + 3..]);
            }
        }
    }
    (raw, &[])
}

fn headers(head: &[u8]) -> Vec<(String, String)> {
    let text = String::from_utf8_lossy(head);
    let mut out: Vec<(String, String)> = Vec::new();
    for line in text.split('\n') {
        let line = line.trim_end_matches('\r');
        if (line.starts_with(' ') || line.starts_with('\t')) && !out.is_empty() {
            let last = out.last_mut().unwrap();
            last.1.push(' ');
            last.1.push_str(line.trim());
        } else if let Some(c) = line.find(':') {
            out.push((line[..c].trim().to_lowercase(), line[c + 1..].trim().to_string()));
        }
    }
    out
}

fn get<'a>(h: &'a [(String, String)], name: &str) -> &'a str {
    h.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str()).unwrap_or("")
}

pub fn decode_charset(bytes: &[u8], charset: &str) -> String {
    let cs = if charset.is_empty() { "utf-8" } else { charset };
    match encoding_rs::Encoding::for_label(cs.trim().as_bytes()) {
        Some(enc) => enc.decode(bytes).0.into_owned(),
        None => String::from_utf8_lossy(bytes).into_owned(),
    }
}

fn qp_decode(input: &[u8], header: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        let c = input[i];
        if c == b'=' {
            if input.get(i + 1) == Some(&b'\n') {
                i += 2;
                continue;
            }
            if input.get(i + 1) == Some(&b'\r') && input.get(i + 2) == Some(&b'\n') {
                i += 3;
                continue;
            }
            if i + 2 < input.len() {
                let hex = std::str::from_utf8(&input[i + 1..i + 3]).unwrap_or("");
                if let Ok(b) = u8::from_str_radix(hex, 16) {
                    out.push(b);
                    i += 3;
                    continue;
                }
            }
            out.push(c);
        } else if header && c == b'_' {
            out.push(b' ');
        } else {
            out.push(c);
        }
        i += 1;
    }
    out
}

fn b64_decode(input: &[u8]) -> Vec<u8> {
    let clean: Vec<u8> = input
        .iter()
        .copied()
        .filter(|c| c.is_ascii_alphanumeric() || *c == b'+' || *c == b'/')
        .collect();
    base64::engine::general_purpose::STANDARD_NO_PAD
        .decode(&clean)
        .unwrap_or_default()
}

/// Decodes =?charset?B?...?= and =?charset?Q?...?= words.
pub fn decode_words(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    let mut last_was_word = false;
    while let Some(start) = rest.find("=?") {
        let before = &rest[..start];
        let after = &rest[start + 2..];
        let parts: Vec<&str> = after.splitn(3, '?').collect();
        if parts.len() == 3 {
            if let Some(end) = parts[2].find("?=") {
                let (cs, enc, text) = (parts[0], parts[1], &parts[2][..end]);
                if !(last_was_word && before.trim().is_empty()) {
                    out.push_str(before);
                }
                let bytes = if enc.eq_ignore_ascii_case("B") {
                    b64_decode(text.as_bytes())
                } else {
                    qp_decode(text.as_bytes(), true)
                };
                let cs = cs.split('*').next().unwrap_or(cs);
                out.push_str(&decode_charset(&bytes, cs));
                rest = &parts[2][end + 2..];
                last_was_word = true;
                continue;
            }
        }
        out.push_str(&rest[..start + 2]);
        rest = &rest[start + 2..];
        last_was_word = false;
    }
    out.push_str(rest);
    out
}

/// "Name <a@b>" → (Name, a@b)
pub fn split_address(s: &str) -> (String, String) {
    let s = s.trim();
    if let (Some(lt), Some(gt)) = (s.rfind('<'), s.rfind('>')) {
        if lt < gt {
            let name = s[..lt].trim().trim_matches('"').trim().to_string();
            return (name, s[lt + 1..gt].trim().to_string());
        }
    }
    (String::new(), s.trim_matches(|c| c == '<' || c == '>').to_string())
}

fn content_type(h: &[(String, String)]) -> (String, Vec<(String, String)>) {
    let ct = get(h, "content-type");
    let mut parts = ct.split(';');
    let mime = parts.next().unwrap_or("text/plain").trim().to_lowercase();
    let params = parts
        .filter_map(|p| {
            let mut kv = p.splitn(2, '=');
            Some((
                kv.next()?.trim().to_lowercase(),
                kv.next()?.trim().trim_matches('"').to_string(),
            ))
        })
        .collect();
    (if mime.is_empty() { "text/plain".into() } else { mime }, params)
}

fn param<'a>(p: &'a [(String, String)], k: &str) -> &'a str {
    p.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str()).unwrap_or("")
}

/// Text that's really a web page (some emails put their HTML, or an empty
/// HTML page, where the plain text belongs).
pub fn looks_like_html(s: &str) -> bool {
    let t = s.trim_start();
    let start: String = t.chars().take(200).collect::<String>().to_ascii_lowercase();
    start.starts_with("<!doctype html") || start.starts_with("<html") || (start.starts_with('<') && start.contains("<body"))
}

/// `&#128205;` / `&#x1F4CD;` → the character (📍); anything else left as it is.
pub fn decode_numeric(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("&#") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 2..];
        let end = after.find(';').filter(|e| *e > 0 && *e <= 8);
        let ch = end.and_then(|e| {
            let n = &after[..e];
            match n.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok(),
                None => n.parse::<u32>().ok(),
            }
            .and_then(char::from_u32)
        });
        match (ch, end) {
            (Some(c), Some(e)) => {
                out.push(c);
                rest = &after[e + 1..];
            }
            _ => {
                out.push_str("&#");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Where an element that isn't shown (display: none, hidden, no size, see
/// through) ends, when the tag at `at` opens one (`lower`: the HTML in lower case).
fn hidden_element_end(lower: &str, at: usize) -> Option<usize> {
    let tag_end = lower[at..].find('>').map(|e| at + e + 1)?;
    let tag = &lower[at..tag_end];
    if tag.starts_with("</") || tag.starts_with("<!") {
        return None;
    }
    let squeezed: String = tag.chars().filter(|c| !c.is_whitespace()).collect();
    // (Only what's never shown: a layout's font-size:0 or max-height:0 wraps
    // text that sets its own size inside, and mso-hide only hides in Outlook.)
    let hidden = ["display:none", "visibility:hidden"]
        .iter()
        .any(|h| squeezed.contains(h))
        || tag.split(|c: char| c.is_whitespace() || c == '>' || c == '/').any(|w| w == "hidden" || w.starts_with("hidden="));
    if !hidden {
        return None;
    }
    let name: String = tag[1..].chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
    if name.is_empty() || tag.ends_with("/>") || ["img", "br", "hr", "input", "meta", "link"].contains(&name.as_str()) {
        return Some(tag_end);
    }
    // To its own closing tag (the same element inside it counted).
    let (open, close) = (format!("<{name}"), format!("</{name}"));
    let mut depth = 1;
    let mut j = tag_end;
    while depth > 0 {
        let next_open = lower[j..].find(&open).map(|e| j + e);
        let next_close = lower[j..].find(&close).map(|e| j + e)?;
        match next_open {
            Some(o) if o < next_close && lower[o + open.len()..].starts_with(|c: char| !c.is_ascii_alphanumeric()) => {
                depth += 1;
                j = o + open.len();
            }
            _ => {
                depth -= 1;
                j = next_close + close.len();
            }
        }
    }
    Some(lower[j..].find('>').map(|e| j + e + 1).unwrap_or(lower.len()))
}

pub fn html_to_text(html: &str) -> String {
    // (Never failing: HTML this can't take apart gives its text roughly.)
    std::panic::catch_unwind(|| html_text(html, true)).unwrap_or_else(|_| {
        eprintln!("almanac: couldn't take the text out of some HTML ({} bytes)", html.len());
        let mut out = String::new();
        let mut inside = false;
        for c in html.chars() {
            match c {
                '<' => inside = true,
                '>' => inside = false,
                _ if !inside => out.push(c),
                _ => {}
            }
        }
        out
    })
}

/// The text of HTML (`shown_only`: without comments and hidden parts; false:
/// as it was taken before, to tell which stored texts came from their HTML).
pub fn html_text(html: &str, shown_only: bool) -> String {
    let mut out = String::new();
    // (ASCII only: the same length as the HTML, so a place in one is the same
    // place in the other; tag names and styles are ASCII.)
    let lower = html.to_ascii_lowercase();
    let mut i = 0;
    let bytes = html.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let rest = &lower[i..];
            // Comments, and what isn't shown (hidden preview lines and the
            // like): not the email's text, so not searched either.
            if shown_only && rest.starts_with("<!--") {
                i += rest.find("-->").map(|e| e + 3).unwrap_or(rest.len());
                continue;
            }
            if let Some(end) = shown_only.then(|| hidden_element_end(&lower, i)).flatten() {
                i = end;
                continue;
            }
            for skip in ["<style", "<script", "<head"] {
                if rest.starts_with(skip) {
                    let close = format!("</{}", &skip[1..]);
                    if let Some(e) = rest.find(&close) {
                        i += e;
                    }
                }
            }
            let tag_end = lower[i..].find('>').map(|e| i + e + 1).unwrap_or(bytes.len());
            let tag = &lower[i..tag_end];
            if tag.starts_with("<br") || tag.starts_with("<p") || tag.starts_with("</p")
                || tag.starts_with("<div") || tag.starts_with("</div") || tag.starts_with("<tr")
                || tag.starts_with("<li") || tag.starts_with("</h")
            {
                out.push('\n');
            }
            i = tag_end;
        } else {
            let next = html[i..].find('<').map(|e| i + e).unwrap_or(bytes.len());
            out.push_str(&html[i..next]);
            i = next;
        }
    }
    let out = out
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&rsquo;", "'")
        .replace("&zwnj;", "");
    // Characters written by number (&#128205; is 📍), also those encoded twice
    // over by the sender's app (&amp;#128205;, which the line above made one).
    let out = decode_numeric(&out);
    let mut cleaned = String::new();
    let mut blank = 0;
    for line in out.lines() {
        let l = line.trim();
        if l.is_empty() {
            blank += 1;
            if blank > 1 {
                continue;
            }
        } else {
            blank = 0;
        }
        cleaned.push_str(l);
        cleaned.push('\n');
    }
    cleaned.trim().to_string()
}

fn split_parts<'a>(text: &'a [u8], boundary: &str) -> Vec<&'a [u8]> {
    let delim = format!("--{boundary}");
    let mut parts = Vec::new();
    let mut pos = 0;
    let find = |hay: &[u8], from: usize| -> Option<usize> {
        hay[from..]
            .windows(delim.len())
            .position(|w| w == delim.as_bytes())
            .map(|p| p + from)
    };
    while let Some(s) = find(text, pos) {
        let after = s + delim.len();
        if text[after..].starts_with(b"--") {
            break;
        }
        let content_start = text[after..].iter().position(|&c| c == b'\n').map(|p| after + p + 1).unwrap_or(text.len());
        let end = find(text, content_start).unwrap_or(text.len());
        parts.push(&text[content_start..end]);
        pos = end;
        if end == text.len() {
            break;
        }
    }
    parts
}

/// A file name from a header parameter (filename=, name=, or the encoded filename*=).
fn file_param(s: &str, key: &str) -> Option<String> {
    s.split(';').find_map(|p| {
        let (k, v) = p.split_once('=')?;
        let k = k.trim().to_lowercase();
        if k == key || k == format!("{key}*") {
            let v = v.trim().trim_matches('"');
            let v = v.rsplit("''").next().unwrap_or(v);
            Some(decode_words(&v.replace("%20", " ")))
        } else {
            None
        }
    })
}

/// Pictures inside the message itself (logos, signatures) by Content-ID:
/// (id, type, bytes, file name). Only reasonably small ones.
fn inline_images(h: &[(String, String)], body: &[u8], depth: usize, out: &mut Vec<(String, String, Vec<u8>, String)>) {
    let (mime, params) = content_type(h);
    if mime.starts_with("multipart/") && depth < 8 {
        let boundary = param(&params, "boundary");
        if boundary.is_empty() {
            return;
        }
        for part in split_parts(body, boundary) {
            let (ph, pb) = split_head_body(part);
            inline_images(&headers(ph), pb, depth + 1, out);
        }
        return;
    }
    let cid = get(h, "content-id").trim().trim_start_matches('<').trim_end_matches('>').to_string();
    if mime.starts_with("image/") && !cid.is_empty() {
        let bytes = decode_part(h, body);
        if bytes.len() <= 3_000_000 {
            let name = file_param(get(h, "content-disposition"), "filename")
                .or_else(|| file_param(get(h, "content-type"), "name"))
                .unwrap_or_default();
            out.push((cid, mime.to_string(), bytes, name));
        }
    }
}

/// Puts the message's own pictures in place of their "cid:" references.
fn embed_inline_images(html: &str, images: &[(String, String, Vec<u8>, String)]) -> String {
    use base64::Engine;
    let mut s = html.to_string();
    for (cid, mime, bytes, _) in images {
        let data = format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes));
        for quote in ["\"", "'"] {
            s = s.replace(&format!("{quote}cid:{cid}{quote}"), &format!("{quote}{data}{quote}"));
        }
    }
    s
}

/// File names of attached files anywhere in the message.
fn attachment_names(h: &[(String, String)], body: &[u8], depth: usize, out: &mut Vec<String>) {
    let (mime, params) = content_type(h);
    if mime.starts_with("multipart/") && depth < 8 {
        let boundary = param(&params, "boundary");
        if boundary.is_empty() {
            return;
        }
        for p in split_parts(body, boundary) {
            let (ph, pb) = split_head_body(p);
            attachment_names(&headers(ph), pb, depth + 1, out);
        }
        return;
    }
    let disp = get(h, "content-disposition");
    let name = file_param(disp, "filename").or_else(|| file_param(get(h, "content-type"), "name"));
    let is_attachment = disp.to_lowercase().starts_with("attachment")
        || (name.is_some() && !mime.starts_with("text/"));
    if is_attachment {
        out.push(name.unwrap_or_else(|| "Attachment".into()));
    }
}

/// Best readable text of a MIME part: (text, is_plain)
fn part_text(h: &[(String, String)], body: &[u8], depth: usize) -> Option<(String, bool)> {
    let (mime, params) = content_type(h);
    if mime.starts_with("multipart/") && depth < 8 {
        let boundary = param(&params, "boundary");
        if boundary.is_empty() {
            return None;
        }
        let parts = split_parts(body, boundary);
        let mut html: Option<String> = None;
        for p in parts {
            let (ph, pb) = split_head_body(p);
            let ph = headers(ph);
            if get(&ph, "content-disposition").to_lowercase().starts_with("attachment") {
                continue;
            }
            if let Some((t, plain)) = part_text(&ph, pb, depth + 1) {
                if plain && !t.trim().is_empty() {
                    return Some((t, true));
                }
                if html.is_none() {
                    html = Some(t);
                }
            }
        }
        return html.map(|t| (t, false));
    }
    if !mime.starts_with("text/") {
        return None;
    }
    let cte = get(h, "content-transfer-encoding").to_lowercase();
    let raw = if cte.contains("base64") {
        b64_decode(body)
    } else if cte.contains("quoted-printable") {
        qp_decode(body, false)
    } else {
        body.to_vec()
    };
    let text = decode_charset(&raw, param(&params, "charset"));
    if mime == "text/html" {
        Some((html_to_text(&text), false))
    } else if mime == "text/plain" {
        Some((text.replace("\r\n", "\n"), true))
    } else {
        None
    }
}

fn decode_part(h: &[(String, String)], body: &[u8]) -> Vec<u8> {
    let cte = get(h, "content-transfer-encoding").to_lowercase();
    if cte.contains("base64") {
        b64_decode(body)
    } else if cte.contains("quoted-printable") {
        qp_decode(body, false)
    } else {
        body.to_vec()
    }
}

/// First part of the given type anywhere in the message, decoded to text.
fn find_text(h: &[(String, String)], body: &[u8], depth: usize, want: &str) -> Option<String> {
    let (mime, params) = content_type(h);
    if mime.starts_with("multipart/") && depth < 8 {
        let boundary = param(&params, "boundary");
        if boundary.is_empty() {
            return None;
        }
        for p in split_parts(body, boundary) {
            let (ph, pb) = split_head_body(p);
            if let Some(t) = find_text(&headers(ph), pb, depth + 1, want) {
                return Some(t);
            }
        }
        return None;
    }
    let is_ics_file = want == "text/calendar" && get(h, "content-type").to_lowercase().contains(".ics");
    if mime == want || is_ics_file || (want == "text/calendar" && mime == "application/ics") {
        if want == "text/html" && get(h, "content-disposition").to_lowercase().starts_with("attachment") {
            return None;
        }
        return Some(decode_charset(&decode_part(h, body), param(&params, "charset")));
    }
    None
}

/// Makes HTML safe to show: no scripts, styles, forms, frames or remote
/// pictures (which can track that mail was opened).
pub fn clean_html(html: &str) -> String {
    let mut s = html.to_string();
    for tag in ["script", "iframe", "object", "embed", "form", "link", "base", "meta", "title", "frameset", "frame", "applet"] {
        loop {
            let lower = s.to_ascii_lowercase();
            let Some(a) = lower.find(&format!("<{tag}")) else { break };
            let close = format!("</{tag}>");
            let b = lower[a..].find(&close).map(|i| a + i + close.len()).or_else(|| lower[a..].find('>').map(|i| a + i + 1)).unwrap_or(s.len());
            s.replace_range(a..b, "");
        }
    }
    // Remote pictures (and background pictures) are kept but switched off:
    // loading them tells the sender the message was opened. The reading pane
    // switches them on when the person chooses "Load Remote Content".
    let s = block_remote(&s);
    // Event handlers.
    let mut out = String::with_capacity(s.len());
    let mut rest = s.as_str();
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let end = rest[i..].find('>').map(|e| i + e + 1).unwrap_or(rest.len());
        let tag = &rest[i..end];
        let low = tag.to_lowercase();
        if low.contains(" on") && low.contains('=') {
            // Event handlers (onload=, onclick=…) go; the rest of the tag stays.
            let re = regex::Regex::new(r#"(?i)\son[a-z]+\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)"#).unwrap();
            out.push_str(&re.replace_all(tag, ""));
        } else {
            out.push_str(tag);
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// Turns remote picture addresses into inert ones (`data-remote-src`,
/// `data-remote-background`, `remote-url(...)`) that nothing loads.
pub fn block_remote(html: &str) -> String {
    // (Attributes run together, `height="128"src="…"`, or with spaces or
    // line breaks around "=": written the usual way first.)
    let tidy = regex::Regex::new(r#"(?i)([\s"'/])(src|background)\s*=\s*"#).unwrap();
    let mut s = tidy.replace_all(html, |c: &regex::Captures| format!("{} {}=", if c[1].trim().is_empty() { "" } else { &c[1] }, &c[2])).into_owned();
    // Addresses written out in full, from the page's own folder ("//host/…"),
    // and inside quotes written as entities (&#39; &quot;), which mail often uses.
    let quotes = ["", "'", "\"", "&#39;", "&quot;", "&#34;"];
    let mut pairs: Vec<(String, String)> = Vec::new();
    for scheme in ["http", "//"] {
        pairs.push((format!(" src=\"{scheme}"), format!(" data-remote-src=\"{scheme}")));
        pairs.push((format!(" src='{scheme}"), format!(" data-remote-src='{scheme}")));
        pairs.push((format!(" src={scheme}"), format!(" data-remote-src={scheme}")));
        pairs.push((format!(" background=\"{scheme}"), format!(" data-remote-background=\"{scheme}")));
        pairs.push((format!(" background='{scheme}"), format!(" data-remote-background='{scheme}")));
        pairs.push((format!(" background={scheme}"), format!(" data-remote-background={scheme}")));
        for q in quotes {
            for gap in ["", " "] {
                pairs.push((format!("url({gap}{q}{scheme}"), format!("remote-url({gap}{q}{scheme}")));
            }
        }
    }
    for (from, to) in &pairs {
        // Case-insensitive (ASCII lowercase keeps every byte where it was).
        let low = s.to_ascii_lowercase();
        if !low.contains(from.as_str()) {
            continue;
        }
        let mut out = String::with_capacity(s.len() + 64);
        let mut at = 0;
        for (i, _) in low.match_indices(from.as_str()) {
            // Already blocked ("remote-url(") stays as it is.
            if from.starts_with("url(") && low[..i].ends_with("remote-") {
                continue;
            }
            out.push_str(&s[at..i]);
            out.push_str(to);
            at = i + from.len();
        }
        out.push_str(&s[at..]);
        s = out;
    }
    s
}

#[cfg(test)]
mod remote_tests {
    #[test]
    fn many_inline_pictures_keep_the_formatting() {
        // An email whose own pictures add up to well over the old size limit.
        let pic = "A".repeat(300_000);
        let mut raw = String::from("From: a@b.c\r\nContent-Type: multipart/related; boundary=\"XX\"\r\n\r\n--XX\r\nContent-Type: text/html\r\n\r\n<p>Today's mail</p>");
        for i in 0..4 {
            raw.push_str(&format!("<img src=\"cid:scan{i}@usps\">"));
        }
        raw.push_str("\r\n");
        for i in 0..4 {
            raw.push_str(&format!("--XX\r\nContent-Type: image/jpeg; name=\"scan{i}.jpg\"\r\nContent-Disposition: inline; filename=\"scan{i}.jpg\"\r\nContent-ID: <scan{i}@usps>\r\nContent-Transfer-Encoding: base64\r\n\r\n{pic}\r\n"));
        }
        raw.push_str("--XX--\r\n");
        let p = super::parse(raw.as_bytes());
        assert!(p.html.contains("Today's mail"), "formatting kept");
        assert_eq!(p.html.matches("data:image/jpeg;base64,").count(), 4);
        assert!(p.attachments.is_empty(), "pictures shown in the email aren't attachments: {:?}", p.attachments);
    }

    #[test]
    fn pictures_inside_the_message_are_shown() {
        let raw = b"From: a@b.c\r\nContent-Type: multipart/related; boundary=\"XX\"\r\n\r\n--XX\r\nContent-Type: text/html\r\n\r\n<p>Hi <img src=\"cid:logo@x\"></p>\r\n--XX\r\nContent-Type: image/png\r\nContent-ID: <logo@x>\r\nContent-Transfer-Encoding: base64\r\n\r\niVBORw0KGgo=\r\n--XX--\r\n";
        let p = super::parse(raw);
        assert!(p.html.contains("src=\"data:image/png;base64,iVBORw0KGgo=\""), "{}", p.html);
    }

    #[test]
    fn remote_pictures_are_kept_but_inert() {
        let h = super::clean_html(r#"<table background="http://x/bg.png"><tr><td style="background-image: url(https://x/a.png)"><IMG SRC="https://x/p.png" onload="evil()"><img src="cid:logo"></td></tr></table>"#);
        assert!(!h.contains(" src=\"https"), "{h}");
        assert!(!h.to_lowercase().contains(" src=\"https"), "{h}");
        assert!(h.contains("data-remote-src=\"https://x/p.png\""), "{h}");
        assert!(h.contains("data-remote-background=\"http://x/bg.png\""), "{h}");
        assert!(h.contains("remote-url(https://x/a.png)"), "{h}");
        assert!(h.contains(r#"src="cid:logo""#), "local pictures stay: {h}");
        // Attributes run together, or "=" with spaces or a line break round it.
        let h = super::clean_html("<img height=\"128\"width=\"\"src=\"https://x/m.png\"><img\nsrc = 'http://x/n.png'><td background\n=\"https://x/bg.png\">");
        assert!(!h.contains(" src=") && !h.contains("\"src=") && !h.contains("\nsrc"), "{h}");
        assert!(h.contains("data-remote-src=\"https://x/m.png\"") && h.contains("data-remote-src='http://x/n.png'"), "{h}");
        assert!(h.contains("data-remote-background=\"https://x/bg.png\""), "{h}");
        assert!(!h.contains("evil"), "{h}");
    }
}

/// The bytes and type of a named attachment in a full message.
pub fn extract_attachment(raw: &[u8], name: &str) -> Option<(Vec<u8>, String)> {
    fn walk(h: &[(String, String)], body: &[u8], depth: usize, name: &str) -> Option<(Vec<u8>, String)> {
        let (mime, params) = content_type(h);
        if mime.starts_with("multipart/") && depth < 8 {
            let boundary = param(&params, "boundary");
            for p in split_parts(body, boundary) {
                let (ph, pb) = split_head_body(p);
                if let Some(x) = walk(&headers(ph), pb, depth + 1, name) {
                    return Some(x);
                }
            }
            return None;
        }
        let mut names = Vec::new();
        attachment_names(h, body, 99, &mut names);
        if names.iter().any(|n| n == name) {
            return Some((decode_part(h, body), mime));
        }
        None
    }
    let (head, body) = split_head_body(raw);
    walk(&headers(head), body, 0, name)
}

/// A message read (see `parse_whole`), never failing: a message this code
/// can't read (a mistake here, not in the mail) is noted in the log and kept
/// with what could be read of it, rather than stopping a mail check or the app.
pub fn parse(raw: &[u8]) -> Parsed {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| parse_whole(raw))) {
        Ok(p) => p,
        Err(_) => {
            eprintln!("almanac: couldn't read a message ({} bytes): kept as plain text", raw.len());
            let text = String::from_utf8_lossy(raw);
            let (head, body) = text.split_once("\r\n\r\n").or_else(|| text.split_once("\n\n")).unwrap_or(("", &text));
            let field = |name: &str| {
                head.lines()
                    .find(|l| l.len() > name.len() && l.as_bytes()[..name.len()].eq_ignore_ascii_case(name.as_bytes()) && l.as_bytes()[name.len()] == b':')
                    .map(|l| l[name.len() + 1..].trim().to_string())
                    .unwrap_or_default()
            };
            Parsed {
                from_name: String::new(),
                from_email: field("From"),
                to: field("To"),
                cc: field("Cc"),
                subject: field("Subject"),
                date: chrono::DateTime::parse_from_rfc2822(&field("Date")).map(|d| d.timestamp()).unwrap_or(0),
                body: first_chars(body, 60_000).to_string(),
                attachments: Vec::new(),
                html: String::new(),
                invite: String::new(),
                message_id: field("Message-ID"),
                in_reply_to: field("In-Reply-To"),
                references: field("References"),
                reply_to: field("Reply-To"),
                mozilla_status: None,
            }
        }
    }
}

/// The start of `s`, at most `n` characters (never cut inside one).
pub fn first_chars(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

/// `s` cut to at most `n` bytes, at a character's edge.
pub fn first_bytes(s: &str, n: usize) -> &str {
    if s.len() <= n {
        return s;
    }
    let mut i = n;
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    &s[..i]
}

fn parse_whole(raw: &[u8]) -> Parsed {
    let (head, body) = split_head_body(raw);
    let h = headers(head);
    let (from_name, from_email) = split_address(&decode_words(get(&h, "from")));
    let date_s = get(&h, "date");
    let date_clean = date_s.split(" (").next().unwrap_or(date_s).trim();
    let date = chrono::DateTime::parse_from_rfc2822(date_clean)
        .map(|d| d.timestamp())
        .unwrap_or(0);
    let mut attachments = Vec::new();
    attachment_names(&h, body, 0, &mut attachments);
    let mut images = Vec::new();
    inline_images(&h, body, 0, &mut images);
    // The size limit applies to the formatting itself, before its own pictures
    // go in (emails such as USPS Informed Delivery carry many of them).
    let own_html = find_text(&h, body, 0, "text/html").map(|t| clean_html(&t)).unwrap_or_default();
    let html = if own_html.len() > 2_000_000 { String::new() } else { embed_inline_images(&own_html, &images) };
    // Pictures the email shows itself aren't listed as attachments (as in Apple Mail).
    let shown: Vec<&String> = images.iter().filter(|i| !i.3.is_empty() && own_html.contains(&format!("cid:{}", i.0))).map(|i| &i.3).collect();
    attachments.retain(|n| !shown.contains(&n));
    let invite = find_text(&h, body, 0, "text/calendar").filter(|t| t.contains("BEGIN:VEVENT")).unwrap_or_default();
    let mut body = part_text(&h, body, 0).map(|(t, _)| t).unwrap_or_default();
    // A "plain text" part that's really HTML: the text comes from the HTML.
    if looks_like_html(&body) {
        body = html_to_text(if own_html.is_empty() { &body } else { &own_html });
    }
    if body.len() > 60_000 {
        let mut cut = 60_000;
        while !body.is_char_boundary(cut) {
            cut -= 1;
        }
        body.truncate(cut);
        body.push_str("\n…");
    }
    Parsed {
        from_name,
        from_email,
        to: decode_words(get(&h, "to")),
        cc: decode_words(get(&h, "cc")),
        subject: decode_words(get(&h, "subject")),
        date,
        body: body.trim().to_string(),
        attachments,
        html,
        invite,
        message_id: get(&h, "message-id").trim().to_string(),
        in_reply_to: get(&h, "in-reply-to").split_whitespace().collect::<Vec<_>>().join(" "),
        references: get(&h, "references").split_whitespace().collect::<Vec<_>>().join(" "),
        reply_to: split_address(&decode_words(get(&h, "reply-to"))).1,
        mozilla_status: u32::from_str_radix(get(&h, "x-mozilla-status").trim(), 16).ok(),
    }
}

#[cfg(test)]
mod remote_quote_tests {
    #[test]
    fn quoted_and_relative_addresses_are_held_back_once() {
        let html = "<div style=\"background: url(&#39;http://x.example/a.png&#39;)\"></div><img src=//cdn.example/b.png><td style=\"background:url( &quot;https://y/c.png&quot;)\">";
        let once = super::block_remote(html);
        assert!(once.contains("remote-url(&#39;http://x.example"), "{once}");
        assert!(once.contains("data-remote-src=//cdn.example"), "{once}");
        assert!(once.contains("remote-url( &quot;https://y"), "{once}");
        assert_eq!(super::block_remote(&once), once, "running it again changes nothing");
    }
}

#[cfg(test)]
mod html_as_text_tests {
    #[test]
    fn a_plain_part_thats_really_html_gives_the_text_of_the_html() {
        let raw = "From: Envoy <hello@envoy.com>\r\nSubject: Today's visit\r\nContent-Type: multipart/alternative; boundary=\"b\"\r\n\r\n\
--b\r\nContent-Type: text/plain; charset=UTF-8\r\n\r\n<!DOCTYPE html>\r\n<html><head><meta charset=\"UTF-8\"></head></html>\r\n\
--b\r\nContent-Type: text/html; charset=UTF-8\r\n\r\n<html><body><h1>Welcome to ACES Brentwood</h1><p>Your host has been notified.</p></body></html>\r\n--b--\r\n";
        let p = super::parse(raw.as_bytes());
        assert!(!p.body.contains('<'), "{}", p.body);
        assert!(p.body.contains("Welcome to ACES Brentwood") && p.body.contains("Your host has been notified."), "{}", p.body);
        let preview = crate::data::strip_preview("<!DOCTYPE html><html><body><p>Hello there</p></body></html>");
        assert_eq!(preview, "Hello there");
        assert!(!super::looks_like_html("Plain words < and > in text"));
    }
}

#[cfg(test)]
mod numbered_character_tests {
    use super::*;

    #[test]
    fn characters_written_by_number() {
        assert_eq!(decode_numeric("&#128205;: here &#x1F4CD; &#nope; & more"), "📍: here 📍 &#nope; & more");
        assert_eq!(html_to_text("&amp;#128205;: <a href=\"https://x\">https://x</a>").trim(), "📍: https://x");
        // Only what's shown: no styles, comments, hidden preview lines or link addresses.
        let page = "<html><head><style>p { font-family: Arial }</style></head><body><!-- family note -->\
                    <span style=\"display: none; max-height:0\">family preview <b>inside</b></span>\
                    <div hidden>family too</div><p style=\"font-family: Helvetica\">Hello <a href=\"https://x.example/family\">there</a></p></body></html>";
        let text = html_to_text(page);
        assert!(!text.to_lowercase().contains("family"), "{text}");
        // A layout's zero-size wrapper keeps what's in it.
        let layout = "<table><tr><td style=\"font-size:0;max-height:0\"><div style=\"font-size:14px\">Kept family text</div></td></tr></table>";
        assert!(html_to_text(layout).contains("Kept family text"));
        // (Letters whose lower case is longer, before tags: no panic.)
        assert!(html_to_text("<p>İİİ <b>x</b> ẞ</p><div>ok</div>").contains("ok"));
        assert!(text.contains("Hello") && text.contains("there"), "{text}");
    }
}

#[cfg(test)]
mod robustness {
    //! Mail in any language, and mail that isn't what it says it is: nothing
    //! here may panic (cutting text inside a character did).

    /// Random bytes (a fixed sequence, so a failure can be found again).
    fn bytes(seed: &mut u64, n: usize) -> Vec<u8> {
        (0..n)
            .map(|_| {
                *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                (*seed >> 33) as u8
            })
            .collect()
    }

    const PIECES: &[&str] = &[
        "<p>", "</p>", "<div style=\"display:none\">", "</div>", "<span hidden>", "</span>", "<!-- ", " -->", "<style>", "</style>", "<br>", "<a href=\"x\">",
        "</a>", "<", ">", "&amp;", "&#128205;", "&#x1F600;", "&", ";", "=?UTF-8?B?w6nDqA==?=", "=?iso-8859-1?Q?caf=E9?=", "=?bad?X?zz?=", "=E9", "=\r\n",
        "İİ", "ẞ", "K", "Ⅻ", "ǅ", "日本語のメール", "😀👍🏽", "e\u{301}", "\u{202e}مرحبا", "Ελληνικά", "ﬃ", "\u{0}", "\r\n", "\n\n", " ", "Re: ", "family",
    ];

    fn mixed(seed: &mut u64, n: usize) -> String {
        bytes(seed, n).iter().map(|b| PIECES[*b as usize % PIECES.len()]).collect()
    }

    #[test]
    fn any_text_at_any_length() {
        let mut seed = 7u64;
        let words = crate::searchhl::words("family café 日本");
        for round in 0..300 {
            let text = mixed(&mut seed, 40 + round % 60);
            // Every cut at a character's edge, and raw bytes too.
            let cuts: Vec<usize> = text.char_indices().map(|(i, _)| i).chain([text.len()]).collect();
            for &c in cuts.iter().step_by(3) {
                let t = &text[..c];
                let _ = super::html_text(t, true);
                let _ = super::html_text(t, false);
                let _ = super::decode_words(t);
                let _ = super::decode_numeric(t);
                let _ = super::clean_html(t);
                let _ = super::block_remote(t);
                let _ = super::looks_like_html(t);
                let _ = crate::data::strip_preview(t);
                let _ = crate::data::thread_key(t);
                let _ = crate::searchhl::snippet(t, &words);
                let _ = crate::searchhl::matches(t, &words);
                let _ = crate::mailstore::match_expr(t);
                let _ = crate::mailstore::search_expr(t);
                let _ = super::first_chars(t, c / 2);
                let _ = super::first_bytes(t, c / 2);
            }
            // Whole messages: headers and parts in any charset, broken encodings.
            let raw = format!(
                "From: {}\r\nSubject: {}\r\nContent-Type: multipart/alternative; boundary=\"b\"\r\n\r\n--b\r\nContent-Type: text/plain; charset=\"{}\"\r\nContent-Transfer-Encoding: {}\r\n\r\n{}\r\n--b\r\nContent-Type: text/html\r\n\r\n{}\r\n--b--\r\n",
                mixed(&mut seed, 6),
                mixed(&mut seed, 8),
                ["utf-8", "iso-8859-1", "windows-1252", "shift_jis", "nonsense", ""][round % 6],
                ["quoted-printable", "base64", "8bit", "x-unknown"][round % 4],
                text,
                mixed(&mut seed, 30)
            );
            let _ = super::parse_whole(raw.as_bytes());
            let noise = bytes(&mut seed, 200 + round);
            let _ = super::parse_whole(&noise);
            let _ = super::decode_charset(&noise, ["utf-8", "iso-2022-jp", "gb18030", "bogus"][round % 4]);
            // (And anything cut short.)
            for cut in (0..raw.len()).step_by(37) {
                let _ = super::parse_whole(&raw.as_bytes()[..cut]);
            }
        }
    }
}
