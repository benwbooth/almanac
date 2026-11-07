//! Editing help for the message editor's formatted text (Qt's rich text
//! HTML): Tab and Shift+Tab move a list item one level in or out; Return in
//! quoted text breaks the quote; quotes become real quotes in what's sent.

/// Where the cursor is: the editor puts this character there before asking.
pub const MARK: char = '\u{E000}';

/// One open list at some point of the HTML.
#[derive(Clone)]
struct OpenList {
    tag: String,
    open: String,
    indent: usize,
}

/// The `<ul>` / `<ol>` tags in `html[..end]`, as a stack of the ones still open.
fn open_lists(html: &str, end: usize) -> Vec<OpenList> {
    let mut stack: Vec<OpenList> = Vec::new();
    let mut i = 0;
    while let Some(off) = html[i..end].find('<') {
        let at = i + off;
        let Some(close) = html[at..end].find('>') else { break };
        let tag_text = &html[at..at + close + 1];
        let lower = tag_text.to_ascii_lowercase();
        for t in ["ul", "ol"] {
            if lower.starts_with(&format!("<{t}")) && lower[1 + t.len()..].starts_with(|c: char| c == ' ' || c == '>') {
                let indent = list_indent(&lower).unwrap_or(stack.len() + 1);
                stack.push(OpenList { tag: t.into(), open: tag_text.to_string(), indent });
            } else if lower.starts_with(&format!("</{t}")) {
                stack.pop();
            }
        }
        i = at + close + 1;
    }
    stack
}

fn list_indent(tag: &str) -> Option<usize> {
    let at = tag.find("-qt-list-indent:")?;
    tag[at + 16..].trim_start().chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok()
}

/// The opening tag of a list at `indent` (bullets and numbers change by level, as in Apple Mail).
fn list_open(tag: &str, indent: usize) -> String {
    let kinds: [&str; 3] = if tag == "ol" { ["decimal", "lower-alpha", "lower-roman"] } else { ["disc", "circle", "square"] };
    let kind = kinds[(indent.max(1) - 1) % 3];
    format!("<{tag} style=\"margin-top: 0px; margin-bottom: 0px; margin-left: 0px; margin-right: 0px; -qt-list-indent: {indent}; list-style-type: {kind};\">")
}

/// Removes lists left with nothing in them.
fn drop_empty_lists(mut html: String) -> String {
    loop {
        let before = html.len();
        for t in ["ul", "ol"] {
            let mut from = 0;
            while let Some(off) = html[from..].find(&format!("<{t} ")).or_else(|| html[from..].find(&format!("<{t}>"))) {
                let at = from + off;
                let Some(gt) = html[at..].find('>') else { break };
                let after = at + gt + 1;
                let rest = &html[after..];
                let trimmed = rest.trim_start();
                let close = format!("</{t}>");
                if trimmed.starts_with(&close) {
                    let end = after + (rest.len() - trimmed.len()) + close.len();
                    html.replace_range(at..end, "");
                } else {
                    from = after;
                }
            }
        }
        if html.len() == before {
            return html;
        }
    }
}

/// Joins `</ul>` + `<ul …>` pairs of the same kind and level that meet (so a
/// run of indented items is one list, numbered on from each other).
fn merge_neighbours(mut html: String) -> String {
    'again: loop {
        for t in ["ul", "ol"] {
            let close = format!("</{t}>");
            let mut from = 0;
            while let Some(off) = html[from..].find(&close) {
                let c = from + off;
                let after = c + close.len();
                let rest = &html[after..];
                let trimmed = rest.trim_start();
                let next = after + (rest.len() - trimmed.len());
                if trimmed.to_ascii_lowercase().starts_with(&format!("<{t}")) {
                    let gt = trimmed.find('>').unwrap_or(0);
                    let next_tag = trimmed[..gt + 1].to_ascii_lowercase();
                    // The level of the list that closes here.
                    let mut stack = open_lists(&html, c);
                    let closing = stack.pop();
                    let next_indent = list_indent(&next_tag).unwrap_or(stack.len() + 1);
                    if closing.map(|l| l.tag == t && l.indent == next_indent).unwrap_or(false) {
                        html.replace_range(c..next + gt + 1, "");
                        continue 'again;
                    }
                }
                from = after;
            }
        }
        return html;
    }
}

/// The editor's HTML with the list item holding [`MARK`] one level deeper
/// (`deeper`) or shallower; `None` when the mark isn't in a list item.
/// The mark is taken out of the result.
pub fn nest_list_item(html: &str, deeper: bool) -> Option<String> {
    let mark = html.find(MARK)?;
    let lower = html.to_ascii_lowercase();
    // The <li> around the mark.
    let mut start = None;
    let mut i = 0;
    while let Some(off) = lower[i..mark].find("<li") {
        let at = i + off;
        if lower[at + 3..].starts_with(|c: char| c == ' ' || c == '>') {
            start = Some(at);
        }
        i = at + 3;
    }
    let start = start?;
    if lower[start..mark].contains("</li>") {
        return None;
    }
    let end = mark + lower[mark..].find("</li>")? + "</li>".len();
    let stack = open_lists(html, start);
    let list = stack.last()?.clone();
    let item = html[start..end].to_string();
    let (before, after) = (&html[..start], &html[end..]);
    let out = if deeper {
        let level = list.indent + 1;
        format!("{before}{}{item}</{}>{after}", list_open(&list.tag, level), list.tag)
    } else if stack.len() <= 1 {
        // Out of the list altogether: a paragraph between the two halves.
        let gt = item.find('>').unwrap_or(0);
        let para = format!("<p{}>{}</p>", &item[3..gt], &item[gt + 1..item.len() - 5]);
        format!("{before}</{}>{para}{}{after}", list.tag, list.open)
    } else {
        format!("{before}</{}>{item}{}{after}", list.tag, list.open)
    };
    let out = merge_neighbours(drop_empty_lists(out));
    Some(out.replace(MARK, ""))
}

/// How far each quote level is indented in the editor. (An amount lists
/// never use, so a quoted paragraph can be told from a list item.)
pub const QUOTE_STEP: u32 = 18;

/// Where the next cursor goes after `split_quote` (taken out by the editor).
pub const CURSOR: char = '\u{E001}';

/// A paragraph's quote level, from its `margin-left` in Qt's HTML (0: not quoted).
fn quote_level(open_tag: &str) -> u32 {
    let lower = open_tag.to_ascii_lowercase();
    let Some(at) = lower.find("margin-left:") else { return 0 };
    let rest = lower[at + 12..].trim_start();
    let px: u32 = rest.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(0);
    if px > 0 && px % QUOTE_STEP == 0 { px / QUOTE_STEP } else { 0 }
}

/// The open tag (`<p …>`) of the paragraph holding byte `at`, and where it starts.
fn paragraph_at(lower: &str, at: usize) -> Option<(usize, usize)> {
    let start = lower[..at].rfind("<p")?;
    if !lower[start + 2..].starts_with(|c: char| c == ' ' || c == '>') || lower[start..at].contains("</p>") {
        return None;
    }
    let open_end = start + lower[start..].find('>')? + 1;
    Some((start, open_end))
}

/// Return pressed in quoted text (the cursor is at [`MARK`]): the quote is
/// broken there, as in Apple Mail: the text before stays quoted, a new empty
/// line that isn't (with [`CURSOR`] in it) comes next, then the rest of the
/// quote. `None` when the cursor isn't in a quoted paragraph.
pub fn split_quote(html: &str) -> Option<String> {
    let mark = html.find(MARK)?;
    let lower = html.to_ascii_lowercase();
    let (start, open_end) = paragraph_at(&lower, mark)?;
    let open = &html[start..open_end];
    if quote_level(open) == 0 {
        return None;
    }
    let end = mark + lower[mark..].find("</p>")?;
    // Inline formatting open at the cursor (spans, links, bold…): closed before
    // the break and opened again after it.
    let mut stack: Vec<String> = Vec::new();
    let mut i = open_end;
    while let Some(off) = lower[i..mark].find('<') {
        let at = i + off;
        let close = at + lower[at..].find('>').unwrap_or(0) + 1;
        let tag = &html[at..close];
        if tag.starts_with("</") {
            stack.pop();
        } else if !tag.ends_with("/>") && !lower[at..].starts_with("<br") && !lower[at..].starts_with("<img") {
            stack.push(tag.to_string());
        }
        i = close;
    }
    let name = |t: &str| t.trim_start_matches('<').split(|c: char| c == ' ' || c == '>').next().unwrap_or("").to_string();
    let closing: String = stack.iter().rev().map(|t| format!("</{}>", name(t))).collect();
    let reopening: String = stack.concat();
    let before = &html[open_end..mark];
    // (The space after the break point isn't kept at the start of the line.)
    let after = html[mark + MARK.len_utf8()..end].trim_start_matches(' ');
    let plain = open.replacen("margin-left:", "margin-left:0px; -almanac-was:", 1);
    let mut out = String::with_capacity(html.len() + 200);
    out.push_str(&html[..start]);
    let has_text = |s: &str| s.split('<').enumerate().any(|(n, part)| {
        let text = if n == 0 { part } else { part.split_once('>').map(|x| x.1).unwrap_or("") };
        !text.trim().is_empty()
    });
    if has_text(before) {
        out.push_str(&format!("{open}{before}{closing}</p>"));
    }
    out.push_str(&format!("{plain}{CURSOR}</p>"));
    if has_text(after) {
        out.push_str(&format!("{open}{reopening}{after}</p>"));
    }
    out.push_str(&html[end + 4..]);
    Some(out)
}

/// The end of a selection (its start is [`MARK`]).
pub const END: char = '\u{E002}';

/// A paragraph's open tag at quote `level` (its other formatting kept).
fn with_level(tag: &str, level: u32) -> String {
    let px = level * QUOTE_STEP;
    let lower = tag.to_ascii_lowercase();
    if let Some(at) = lower.find("margin-left:") {
        let rest = &tag[at + 12..];
        let lead = rest.len() - rest.trim_start().len();
        let digits = rest.trim_start().chars().take_while(|c| c.is_ascii_digit()).count();
        format!("{}{px}{}", &tag[..at + 12 + lead], &rest[lead + digits..])
    } else if let Some(at) = lower.find("style=\"") {
        format!("{}margin-left:{px}px; {}", &tag[..at + 7], &tag[at + 7..])
    } else {
        format!("<p style=\"margin-left:{px}px;\"{}", &tag[2..])
    }
}

/// Increase / Decrease Quote Level: every paragraph from the one holding
/// [`MARK`] to the one holding [`END`] one level in (`step` 1) or out (-1),
/// formatting kept. The marks are taken out.
pub fn change_quote(html: &str, step: i32) -> Option<String> {
    let (a, b) = (html.find(MARK)?, html.find(END)?);
    let (a, b) = (a.min(b), a.max(b));
    let lower = html.to_ascii_lowercase();
    let (first, _) = paragraph_at(&lower, a)?;
    let last = b + lower[b..].find("</p>")?;
    let mut out = String::with_capacity(html.len() + 64);
    out.push_str(&html[..first]);
    let mut i = first;
    while let Some(off) = lower[i..last].find("<p") {
        let at = i + off;
        out.push_str(&html[i..at]);
        if !lower[at + 2..].starts_with(|c: char| c == ' ' || c == '>') {
            out.push_str(&html[at..at + 2]);
            i = at + 2;
            continue;
        }
        let close = at + lower[at..].find('>')? + 1;
        let tag = &html[at..close];
        out.push_str(&with_level(tag, (quote_level(tag) as i32 + step).max(0) as u32));
        i = close;
    }
    out.push_str(&html[i..]);
    Some(out.replace(MARK, "").replace(END, ""))
}

/// What's sent: quoted paragraphs (indented in the editor) become real quotes,
/// with the blue bar at the left that mail apps show (one per level).
pub fn quotes_for_sending(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let Some(body) = lower.find("<body") else { return html.to_string() };
    let Some(body_open) = lower[body..].find('>').map(|x| body + x + 1) else { return html.to_string() };
    let body_close = lower.rfind("</body>").unwrap_or(html.len());
    let mut out = String::from(&html[..body_open]);
    let mut level = 0u32;
    let mut i = body_open;
    let quote = "<blockquote style=\"margin:0 0 0 0; padding-left:10px; border-left:2px solid #2f7cf6;\">";
    while i < body_close {
        // The next top-level element (or text between elements).
        let Some(off) = lower[i..body_close].find('<') else {
            out.push_str(&html[i..body_close]);
            break;
        };
        if off > 0 {
            out.push_str(&html[i..i + off]);
            i += off;
            continue;
        }
        let tag_end = i + lower[i..].find('>').map(|x| x + 1).unwrap_or(1);
        let name: String = lower[i + 1..tag_end].chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
        // The whole element (nested elements of the same name counted).
        let end = if name.is_empty() || lower[i..tag_end].ends_with("/>") || name == "br" || name == "hr" || name == "img" {
            tag_end
        } else {
            let (open_t, close_t) = (format!("<{name}"), format!("</{name}>"));
            let mut depth = 0i32;
            let mut j = i;
            loop {
                let next_open = lower[j..body_close].find(&open_t).map(|x| j + x);
                let next_close = lower[j..body_close].find(&close_t).map(|x| j + x);
                match (next_open, next_close) {
                    (Some(o), Some(c)) if o < c => { depth += 1; j = o + open_t.len(); }
                    (_, Some(c)) => { depth -= 1; j = c + close_t.len(); if depth <= 0 { break j; } }
                    _ => break body_close,
                }
            }
        };
        let element = &html[i..end];
        let want = if name == "p" { quote_level(&html[i..tag_end]) } else { level.min(if name == "table" || name == "ul" || name == "ol" { level } else { 0 }) };
        while level < want { out.push_str(quote); level += 1; }
        while level > want { out.push_str("</blockquote>"); level -= 1; }
        if want > 0 && name == "p" {
            let tag = &html[i..tag_end];
            let at = tag.to_ascii_lowercase().find("margin-left:").unwrap_or(0);
            let digits = tag[at + 12..].trim_start().chars().take_while(|c| c.is_ascii_digit()).count();
            let lead = tag[at + 12..].len() - tag[at + 12..].trim_start().len();
            out.push_str(&format!("{}0{}{}", &tag[..at + 12 + lead], &tag[at + 12 + lead + digits..], &element[tag.len()..]));
        } else {
            out.push_str(element);
        }
        i = end;
    }
    while level > 0 { out.push_str("</blockquote>"); level -= 1; }
    out.push_str(&html[body_close..]);
    out
}

#[cfg(test)]
mod quote_tests {
    use super::*;

    const P0: &str = "<p style=\" margin-top:0px; margin-left:0px;\">";
    const P1: &str = "<p style=\" margin-top:0px; margin-left:18px;\">";
    const P2: &str = "<p style=\" margin-top:0px; margin-left:36px;\">";

    fn doc(body: &str) -> String { format!("<html><body style=\"font-size:9pt\">\n{body}</body></html>") }

    #[test]
    fn return_breaks_a_quote_and_keeps_formatting() {
        let html = doc(&format!("{P0}Hi</p>\n{P1}On Monday Sam wrote:</p>\n{P1}one <span style=\"font-weight:700;\">bo{MARK}ld</span> two</p>\n{P1}three</p>"));
        let out = split_quote(&html).unwrap();
        assert!(out.contains(&format!("{P1}one <span style=\"font-weight:700;\">bo</span></p>")), "{out}");
        assert!(out.contains(&format!("{CURSOR}</p>{P1}<span style=\"font-weight:700;\">ld</span> two</p>")), "{out}");
        // The new line isn't quoted.
        let cur = out.find(CURSOR).unwrap();
        let open = &out[out[..cur].rfind("<p").unwrap()..cur];
        assert_eq!(quote_level(open), 0, "{open}");
        // At the end of a quoted line: nothing empty is left behind.
        let end = doc(&format!("{P1}line one{MARK}</p>\n{P1}line two</p>"));
        let out = split_quote(&end).unwrap();
        assert_eq!(out.matches("line one").count(), 1);
        assert!(!out.contains(&format!("{P1}</p>")), "{out}");
        // Not in a quote: nothing to do (an ordinary Return).
        assert!(split_quote(&doc(&format!("{P0}plain{MARK} text</p>"))).is_none());
    }

    #[test]
    fn quotes_are_sent_as_quotes() {
        let html = doc(&format!("{P0}Reply</p>\n{P1}On Monday Sam wrote:</p>\n{P1}one</p>\n{P2}older</p>\n{P0}after</p>"));
        let sent = quotes_for_sending(&html);
        assert_eq!(sent.matches("<blockquote").count(), 2, "{sent}");
        assert_eq!(sent.matches("</blockquote>").count(), 2, "{sent}");
        let q = sent.find("<blockquote").unwrap();
        assert!(sent[..q].contains("Reply") && !sent[..q].contains("one"));
        assert!(sent.find("after").unwrap() > sent.rfind("</blockquote>").unwrap());
        assert!(!sent.contains("margin-left:18px") && !sent.contains("margin-left:36px"), "{sent}");
        assert!(sent.contains("border-left:2px solid #2f7cf6"));
        // Nothing quoted: as it was.
        let plain = doc(&format!("{P0}Just text</p>"));
        assert_eq!(quotes_for_sending(&plain), plain);
    }

    #[test]
    fn quote_level_goes_up_and_down_keeping_formatting() {
        let html = doc(&format!("{P0}<span style=\"font-weight:700;\">{MARK}bold</span></p>\n{P0}two{END}</p>\n{P0}three</p>"));
        let up = change_quote(&html, 1).unwrap();
        assert_eq!(up.matches("margin-left:18px").count(), 2, "{up}");
        assert!(up.contains("<span style=\"font-weight:700;\">bold</span>") && up.contains("three"));
        let again = up.replacen(&format!("{P1}<span"), &format!("{P1}{MARK}<span"), 1).replacen("two", &format!("two{END}"), 1);
        let deeper = change_quote(&again, 1).unwrap();
        assert_eq!(deeper.matches("margin-left:36px").count(), 2, "{deeper}");
        let back = change_quote(&deeper.replacen(&format!("{P2}<span"), &format!("{P2}{MARK}<span"), 1).replacen("two", &format!("two{END}"), 1), -1).unwrap();
        assert_eq!(back.matches("margin-left:18px").count(), 2, "{back}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(html: &str) -> Vec<(usize, String)> {
        // (level, text) of each item, from the lists open around it.
        let mut v = Vec::new();
        let mut i = 0;
        while let Some(off) = html[i..].find("<li") {
            let at = i + off;
            let gt = html[at..].find('>').unwrap() + at + 1;
            let end = html[gt..].find("</li>").unwrap() + gt;
            let level = open_lists(html, at).last().map(|l| l.indent).unwrap_or(0);
            v.push((level, html[gt..end].to_string()));
            i = end;
        }
        v
    }

    const QT: &str = "<ul style=\"margin-top: 0px; -qt-list-indent: 1;\"><li style=\" margin-top:0px;\">one</li>\n<li style=\" margin-top:0px;\">two\u{E000}</li>\n<li style=\" margin-top:0px;\">three</li></ul>\n<p>after</p>";

    #[test]
    fn tab_moves_an_item_in() {
        let out = nest_list_item(QT, true).unwrap();
        assert_eq!(items(&out), vec![(1, "one".into()), (2, "two".into()), (1, "three".into())]);
        assert!(!out.contains(MARK));
        assert!(out.contains("list-style-type: circle"));
    }

    #[test]
    fn neighbours_join_one_list() {
        let once = nest_list_item(QT, true).unwrap();
        // Now indent "three" too: it joins "two"'s list.
        let marked = once.replace("three", "three\u{E000}");
        let twice = nest_list_item(&marked, true).unwrap();
        assert_eq!(items(&twice), vec![(1, "one".into()), (2, "two".into()), (2, "three".into())]);
        assert_eq!(twice.matches("-qt-list-indent: 2").count(), 1, "{twice}");
    }

    #[test]
    fn shift_tab_moves_it_back_out() {
        let once = nest_list_item(QT, true).unwrap();
        let marked = once.replace("two", "two\u{E000}");
        let back = nest_list_item(&marked, false).unwrap();
        assert_eq!(items(&back), vec![(1, "one".into()), (1, "two".into()), (1, "three".into())]);
        assert!(!back.contains("-qt-list-indent: 2"), "{back}");
    }

    #[test]
    fn shift_tab_at_the_top_level_leaves_the_list() {
        let out = nest_list_item(QT, false).unwrap();
        assert_eq!(items(&out), vec![(1, "one".into()), (1, "three".into())]);
        assert!(out.contains("<p style=\" margin-top:0px;\">two</p>"), "{out}");
    }

    #[test]
    fn outside_a_list_nothing_happens() {
        assert!(nest_list_item("<p>plain\u{E000} text</p>", true).is_none());
        assert!(nest_list_item("<ul><li>a</li></ul><p>b\u{E000}</p>", true).is_none());
    }
}
