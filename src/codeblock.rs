//! Code blocks in a message being written. ``` and a language then Enter (or
//! Format › Code Block) makes one; its text is coloured by language (syntect,
//! with Sublime Text's syntaxes). In the editor a block is a two-row table,
//! the language over the code, told apart by its colours; what's sent (and
//! saved) is a plain <pre><code class="language-…">, which other mail apps
//! show as a code block too, and which turns back into an editor block when
//! the message is opened again.
//!
//! Also: Markdown pasted into a message becomes formatting (`markdown_html`).

use std::sync::OnceLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Style, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

fn syntaxes() -> &'static SyntaxSet {
    static S: OnceLock<SyntaxSet> = OnceLock::new();
    S.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn themes() -> &'static ThemeSet {
    static T: OnceLock<ThemeSet> = OnceLock::new();
    T.get_or_init(ThemeSet::load_defaults)
}

/// The colours of a block in the editor (light or dark) and what's sent.
struct Look {
    label_bg: &'static str,
    label_fg: &'static str,
    code_bg: &'static str,
    theme: &'static str,
}
const LIGHT: Look = Look { label_bg: "#eaeef2", label_fg: "#57606a", code_bg: "#f6f8fa", theme: "InspiredGitHub" };
const DARK: Look = Look { label_bg: "#2c2e37", label_fg: "#a0a4ad", code_bg: "#1f2028", theme: "base16-ocean.dark" };
fn look(dark: bool) -> &'static Look {
    if dark {
        &DARK
    } else {
        &LIGHT
    }
}

/// The languages offered in the picker (those the highlighter knows), by name.
pub fn languages() -> Vec<String> {
    let wanted = [
        "Plain Text", "Bash", "C", "C#", "C++", "CSS", "Clojure", "D", "Diff", "Erlang", "Go", "Groovy", "HTML", "Haskell", "Java", "JavaScript",
        "JSON", "LaTeX", "Lisp", "Lua", "Makefile", "Markdown", "MATLAB", "Objective-C", "OCaml", "Pascal", "Perl", "PHP", "Python", "R", "Ruby", "Rust",
        "Scala", "SQL", "Tcl", "TypeScript", "XML", "YAML", "Batch File",
    ];
    wanted.iter().filter(|n| **n == "TypeScript" || syntax_named(n).is_some()).map(|n| n.to_string()).collect()
}

/// The highlighter's own name for a language shown by its usual one.
fn real_name(shown: &str) -> &str {
    if shown.eq_ignore_ascii_case("bash") {
        "Bourne Again Shell (bash)"
    } else {
        shown
    }
}

/// …and the other way.
fn shown_name(real: &str) -> String {
    if real == "Bourne Again Shell (bash)" {
        "Bash".into()
    } else {
        real.to_string()
    }
}

fn syntax_named(name: &str) -> Option<&'static SyntaxReference> {
    let name = real_name(name);
    syntaxes().syntaxes().iter().find(|s| s.name.eq_ignore_ascii_case(name))
}

/// The syntax for a language as people write it after ``` ("py", "rust",
/// "C++", "sh"…); plain text when it's not known.
fn syntax_for(lang: &str) -> &'static SyntaxReference {
    let l = lang.trim().to_lowercase();
    let alias = match l.as_str() {
        "sh" | "shell" | "zsh" | "console" | "shellscript" => "Bash",
        "js" | "jsx" | "node" | "ts" | "tsx" | "typescript" => "JavaScript",
        "py" | "python3" => "Python",
        "rs" => "Rust",
        "c++" | "cpp" | "cc" | "hpp" => "C++",
        "cs" | "csharp" | "c#" => "C#",
        "golang" => "Go",
        "rb" => "Ruby",
        "yml" => "YAML",
        "md" => "Markdown",
        "objc" | "objective-c" => "Objective-C",
        "tex" => "LaTeX",
        "html" | "htm" => "HTML",
        "xml" | "svg" => "XML",
        "json" => "JSON",
        "text" | "txt" | "plain" | "plain text" | "" => "Plain Text",
        _ => "",
    };
    let set = syntaxes();
    (!alias.is_empty()).then(|| syntax_named(alias)).flatten()
        .or_else(|| syntax_named(&l))
        .or_else(|| set.find_syntax_by_token(&l))
        .unwrap_or_else(|| set.find_syntax_plain_text())
}

/// The name shown over a block ("Python" for "py"); TypeScript keeps its own
/// name (coloured as JavaScript).
pub fn language_name(lang: &str) -> String {
    let l = lang.trim().to_lowercase();
    if matches!(l.as_str(), "ts" | "tsx" | "typescript") {
        return "TypeScript".into();
    }
    shown_name(&syntax_for(lang).name)
}

fn esc(t: &str) -> String {
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn hex(c: syntect::highlighting::Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

/// The code's lines as coloured runs (style, text), each line without its end.
fn coloured(lang: &str, code: &str, dark: bool, sending: bool) -> Vec<Vec<(Style, String)>> {
    let theme = &themes().themes[if sending { LIGHT.theme } else { look(dark).theme }];
    let mut h = HighlightLines::new(syntax_for(lang), theme);
    let code = code.replace('\t', "    ");
    let mut out = Vec::new();
    for line in LinesWithEndings::from(&code) {
        let runs = h.highlight_line(line, syntaxes()).unwrap_or_default();
        out.push(runs.into_iter().map(|(s, t)| (s, t.trim_end_matches(['\n', '\r']).to_string())).filter(|(_, t)| !t.is_empty()).collect());
    }
    if code.ends_with('\n') || code.is_empty() {
        out.push(Vec::new());
    }
    out
}

fn run_html(s: &Style, text: &str) -> String {
    let mut css = format!("color:{};", hex(s.foreground));
    if s.font_style.contains(FontStyle::BOLD) {
        css.push_str("font-weight:bold;");
    }
    if s.font_style.contains(FontStyle::ITALIC) {
        css.push_str("font-style:italic;");
    }
    format!("<span style=\"{css}\">{}</span>", esc(text))
}

/// A block as the editor shows it: the language over the coloured code, each
/// line a paragraph (so Enter in it makes a new line of code).
pub fn editor_block(lang: &str, code: &str, dark: bool) -> String {
    let lk = look(dark);
    let lines: Vec<String> = coloured(lang, code, dark, false)
        .iter()
        .map(|runs| {
            let inner: String = runs.iter().map(|(s, t)| run_html(s, t)).collect();
            format!("<p style=\"margin:0;white-space:pre-wrap;font-family:'monospace';\">{inner}</p>")
        })
        .collect();
    format!(
        "<table width=\"100%\" cellspacing=\"0\" cellpadding=\"6\" border=\"0\" style=\"margin-top:6px;margin-bottom:6px;\">\
         <tr><td bgcolor=\"{}\"><p style=\"margin:0;\"><span style=\"font-size:8pt;color:{};\">{}</span></p></td></tr>\
         <tr><td bgcolor=\"{}\">{}</td></tr></table>",
        lk.label_bg,
        lk.label_fg,
        esc(&language_name(lang)),
        lk.code_bg,
        lines.join("")
    )
}

/// A block as sent: <pre><code> with the colours inline (light, whatever the
/// editor shows).
pub fn sent_block(lang: &str, code: &str) -> String {
    let lines: Vec<String> = coloured(lang, code, false, true).iter().map(|runs| runs.iter().map(|(s, t)| run_html(s, t)).collect()).collect();
    let class = language_name(lang).to_lowercase().replace('#', "sharp").replace("++", "pp").replace(' ', "-");
    format!(
        "<pre style=\"background:#f6f8fa;border:1px solid #d0d7de;border-radius:6px;padding:12px;margin:8px 0;\
         font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,'Liberation Mono',monospace;font-size:13px;line-height:1.45;\
         white-space:pre;overflow-x:auto;color:#24292f\"><code class=\"language-{class}\">{}</code></pre>",
        lines.join("\n")
    )
}

/// HTML text → plain text (tags dropped, entities read).
fn text_of(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    decode(&out)
}

fn decode(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let after = &rest[i..];
        let Some(end) = after.find(';').filter(|e| *e <= 10) else {
            out.push('&');
            rest = &after[1..];
            continue;
        };
        let name = &after[1..end];
        let ch = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            n if n.starts_with("#x") || n.starts_with("#X") => u32::from_str_radix(&n[2..], 16).ok().and_then(char::from_u32),
            n if n.starts_with('#') => n[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => {
                out.push(if c == '\u{a0}' { ' ' } else { c });
                rest = &after[end + 1..];
            }
            None => {
                out.push('&');
                rest = &after[1..];
            }
        }
    }
    out.push_str(rest);
    out.replace('\u{a0}', " ")
}

/// A cell's inner HTML (as the editor writes it) → its lines.
fn cell_text(inner: &str) -> String {
    let lower = inner.to_ascii_lowercase();
    let mut lines = Vec::new();
    if lower.contains("<p") {
        let mut at = 0;
        while let Some(s) = lower[at..].find("<p").map(|i| at + i) {
            let open_end = lower[s..].find('>').map(|i| s + i + 1).unwrap_or(lower.len());
            let close = lower[open_end..].find("</p>").map(|i| open_end + i).unwrap_or(lower.len());
            lines.push(br_lines(&inner[open_end..close]));
            at = (close + 4).min(lower.len());
        }
    } else {
        lines.push(br_lines(inner));
    }
    lines.join("\n")
}

fn br_lines(html: &str) -> String {
    let mut s = html.to_string();
    for b in ["<br />", "<br/>", "<br>", "<BR>"] {
        s = s.replace(b, "\n");
    }
    text_of(&s).replace('\u{2028}', "\n")
}

/// The editor's code blocks: (start, end) of each table in `html`, its
/// language and its code.
fn editor_blocks(html: &str) -> Vec<(usize, usize, String, String)> {
    // (ASCII only: same length as the HTML, so a place in one is a place in the other.)
    let lower = html.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(s) = lower[at..].find("<table").map(|i| at + i) {
        let Some(e) = lower[s..].find("</table>").map(|i| s + i + "</table>".len()) else { break };
        let t = &lower[s..e];
        let is_code = [LIGHT.label_bg, DARK.label_bg].iter().any(|bg| t.contains(&format!("bgcolor=\"{bg}\"")))
            && [LIGHT.code_bg, DARK.code_bg].iter().any(|bg| t.contains(&format!("bgcolor=\"{bg}\"")));
        // (Nested tables: not code blocks.)
        if is_code && !t[6..].contains("<table") {
            let cells: Vec<(usize, usize)> = {
                let mut v = Vec::new();
                let mut c = 0;
                while let Some(td) = t[c..].find("<td").map(|i| c + i) {
                    let open_end = t[td..].find('>').map(|i| td + i + 1).unwrap_or(t.len());
                    let close = t[open_end..].find("</td>").map(|i| open_end + i).unwrap_or(t.len());
                    v.push((s + open_end, s + close));
                    c = close;
                }
                v
            };
            if cells.len() == 2 {
                let lang = text_of(&html[cells[0].0..cells[0].1]).trim().to_string();
                let code = cell_text(&html[cells[1].0..cells[1].1]);
                out.push((s, e, lang, code));
            }
        }
        at = e;
    }
    out
}

/// Every code block in the editor coloured again (after typing in one, or
/// a new language). The cursor's mark (U+E000) stays where it was in the
/// code. None when there are no code blocks.
pub fn refresh(html: &str, dark: bool) -> Option<String> {
    let blocks = editor_blocks(html);
    if blocks.is_empty() {
        return None;
    }
    let mut out = String::new();
    let mut last = 0;
    for (s, e, lang, code) in blocks {
        out.push_str(&html[last..s]);
        out.push_str(&with_mark(&lang, &code, |l, c| editor_block(l, c, dark), |l, c| editor_block(l, c, dark)));
        last = e;
    }
    out.push_str(&html[last..]);
    Some(out)
}

/// A block made again, the cursor's mark (U+E000) kept at its place in the
/// code (or in the language) and left out of the colouring.
fn with_mark(lang: &str, code: &str, make: impl Fn(&str, &str) -> String, _plain: impl Fn(&str, &str) -> String) -> String {
    const MARK: char = '\u{e000}';
    if let Some(i) = code.find(MARK) {
        let clean = code.replacen(MARK, "", 1);
        let built = make(lang, &clean);
        // Where the mark goes: after the same number of code characters in the
        // made block's code cell.
        let chars_before = code[..i].chars().filter(|c| *c != '\n').count();
        let lines_before = code[..i].matches('\n').count();
        return place_mark(&built, chars_before, lines_before);
    }
    if lang.contains(MARK) {
        let built = make(&lang.replace(MARK, ""), code);
        // In the language: at the end of its name.
        return built.replacen("</span></p></td></tr>", "\u{e000}</span></p></td></tr>", 1);
    }
    make(lang, code)
}

/// Puts the mark into a made block's code after `chars` characters of text
/// (on line `line`).
fn place_mark(block: &str, chars: usize, line: usize) -> String {
    let cell = block.rfind("<td").and_then(|t| block[t..].find('>').map(|i| t + i + 1)).unwrap_or(0);
    // Count text characters (outside tags, entities as one) from the cell's start.
    let mut count = 0usize;
    let mut para = 0usize;
    let mut i = cell;
    let b = block.as_bytes();
    let mut in_tag = false;
    let mut seen_lines = 0usize;
    while i < block.len() {
        let c = b[i];
        if in_tag {
            if c == b'>' {
                in_tag = false;
            }
            i += 1;
            continue;
        }
        if c == b'<' {
            if block[i..].starts_with("</p>") {
                para += 1;
                seen_lines = para;
            }
            if block[i..].starts_with("</td>") {
                break;
            }
            // At the right place already (start of a line, before its first run).
            if count == chars && seen_lines == line && !block[i..].starts_with("</") && !block[i..].starts_with("<p") {
                return format!("{}\u{e000}{}", &block[..i], &block[i..]);
            }
            in_tag = true;
            i += 1;
            continue;
        }
        if count == chars && seen_lines == line {
            return format!("{}\u{e000}{}", &block[..i], &block[i..]);
        }
        // One character of text (an entity counts once).
        if c == b'&' {
            if let Some(e) = block[i..].find(';') {
                i += e + 1;
            } else {
                i += 1;
            }
        } else {
            let ch = block[i..].chars().next().unwrap();
            i += ch.len_utf8();
        }
        count += 1;
    }
    // The end of a line (or an empty one): before that paragraph's end.
    let mut p = 0usize;
    let mut at = cell;
    while let Some(close) = block[at..].find("</p>").map(|x| at + x) {
        if p == line {
            return format!("{}\u{e000}{}", &block[..close], &block[close..]);
        }
        p += 1;
        at = close + 4;
    }
    block.to_string()
}

/// For sending and saving: the editor's code blocks as <pre><code>.
pub fn for_sending(html: &str) -> String {
    let blocks = editor_blocks(html);
    if blocks.is_empty() {
        return html.to_string();
    }
    let mut out = String::new();
    let mut last = 0;
    for (s, e, lang, code) in blocks {
        out.push_str(&html[last..s]);
        out.push_str(&sent_block(&lang, &code.replace('\u{e000}', "")));
        last = e;
    }
    out.push_str(&html[last..]);
    out
}

/// A message opened to be written again (a draft, a reply's quote): its
/// <pre><code class="language-…"> blocks as editor blocks.
pub fn for_editing(html: &str, dark: bool) -> String {
    // (ASCII only: same length as the HTML, so a place in one is a place in the other.)
    let lower = html.to_ascii_lowercase();
    if !lower.contains("<pre") || !lower.contains("language-") {
        return html.to_string();
    }
    let mut out = String::new();
    let mut last = 0;
    let mut at = 0;
    while let Some(s) = lower[at..].find("<pre").map(|i| at + i) {
        let Some(e) = lower[s..].find("</pre>").map(|i| s + i + 6) else { break };
        let inner = &lower[s..e];
        if let Some(c) = inner.find("<code") {
            let tag_end = inner[c..].find('>').map(|i| c + i).unwrap_or(c);
            let tag = &inner[c..tag_end];
            if let Some(l) = tag.find("language-") {
                let lang: String = tag[l + 9..].chars().take_while(|ch| ch.is_alphanumeric() || "+#-_.".contains(*ch)).collect();
                let lang = match lang.as_str() {
                    "csharp" => "C#".to_string(),
                    "cpp" => "C++".to_string(),
                    "plain-text" => "Plain Text".to_string(),
                    other => other.to_string(),
                };
                let code_html = &html[s + tag_end + 1..e - 6];
                let code_html = code_html.strip_suffix("</code>").or_else(|| code_html.strip_suffix("</CODE>")).unwrap_or(code_html);
                out.push_str(&html[last..s]);
                out.push_str(&editor_block(&lang, &br_lines(code_html), dark));
                last = e;
            }
        }
        at = e;
    }
    out.push_str(&html[last..]);
    out
}

/// Markdown (pasted into a message) → HTML for the editor, its fenced code
/// as code blocks.
pub fn markdown_html(md: &str, dark: bool) -> String {
    use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    let mut events: Vec<Event> = Vec::new();
    let mut code: Option<(String, String)> = None;
    for ev in Parser::new_ext(md, opts) {
        match (&mut code, ev) {
            (None, Event::Start(Tag::CodeBlock(kind))) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(l) => l.split_whitespace().next().unwrap_or("").to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                code = Some((lang, String::new()));
            }
            (Some((_, text)), Event::Text(t)) => text.push_str(&t),
            (Some(_), Event::End(TagEnd::CodeBlock)) => {
                let (lang, text) = code.take().unwrap();
                events.push(Event::Html(editor_block(&lang, text.trim_end_matches('\n'), dark).into()));
            }
            (Some(_), _) => {}
            (None, Event::Code(c)) => events.push(Event::Html(format!("<code style=\"font-family:monospace\">{}</code>", esc(&c)).into())),
            (None, ev) => events.push(ev),
        }
    }
    let mut out = String::new();
    pulldown_cmark::html::push_html(&mut out, events.into_iter());
    out
}

/// Whether pasted text looks like Markdown (worth turning into formatting).
pub fn looks_like_markdown(t: &str) -> bool {
    let mut signs = 0;
    for line in t.lines() {
        let l = line.trim_start();
        if l.starts_with("```") || l.starts_with("~~~") {
            return true;
        }
        if l.starts_with("# ") || l.starts_with("## ") || l.starts_with("### ") || l.starts_with("> ") {
            signs += 2;
        }
        if l.starts_with("- ") || l.starts_with("* ") || (l.len() > 2 && l.split_once(". ").is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))) {
            signs += 1;
        }
        if l.contains("**") || l.contains("](http") || (l.matches('`').count() >= 2) {
            signs += 1;
        }
    }
    signs >= 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn list_syntaxes() {
        let names: Vec<&str> = syntaxes().syntaxes().iter().map(|s| s.name.as_str()).collect();
        eprintln!("{}", names.join(" | "));
    }

    #[test]
    fn languages_by_their_usual_names() {
        assert_eq!(language_name("py"), "Python");
        assert_eq!(language_name("rust"), "Rust");
        assert_eq!(language_name("sh"), "Bash");
        assert_eq!(language_name("c++"), "C++");
        assert_eq!(language_name("ts"), "TypeScript");
        assert_eq!(language_name("nonsense"), "Plain Text");
        assert!(languages().contains(&"Python".to_string()));
    }

    #[test]
    fn code_is_coloured_and_comes_back_out() {
        let b = editor_block("python", "def f(x):\n    return x  # done", false);
        assert!(b.contains("color:#"), "coloured");
        let found = editor_blocks(&format!("<p>hi</p>{b}<p>after</p>"));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].2, "Python");
        assert_eq!(found[0].3, "def f(x):\n    return x  # done");
    }

    #[test]
    fn sent_as_pre_code_and_opened_again_as_a_block() {
        let editor = format!("<p>hi</p>{}", editor_block("rust", "fn main() {\n    let a = 1 < 2;\n}", false));
        let sent = for_sending(&editor);
        assert!(sent.contains("<pre") && sent.contains("class=\"language-rust\""));
        assert!(sent.contains("&lt;"), "escaped");
        let again = for_editing(&sent, false);
        let found = editor_blocks(&again);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].2, "Rust");
        assert_eq!(found[0].3, "fn main() {\n    let a = 1 < 2;\n}");
    }

    #[test]
    fn the_cursor_mark_stays_in_place() {
        let editor = editor_block("python", "ab\ncd", false);
        // Typing put the mark after "c" (the second line's first letter).
        let marked = editor.replacen(">cd<", ">c\u{e000}d<", 1);
        assert_ne!(marked, editor);
        let found = editor_blocks(&marked);
        assert_eq!(found[0].3, "ab\nc\u{e000}d");
        let again = refresh(&marked, false).unwrap();
        let back = editor_blocks(&again);
        assert_eq!(back[0].3, "ab\nc\u{e000}d", "the mark is where it was");
    }

    #[test]
    fn pasted_markdown_becomes_formatting() {
        let md = "# Title\n\nSome **bold** text.\n\n```js\nconst a = 1;\n```\n";
        assert!(looks_like_markdown(md));
        let html = markdown_html(md, false);
        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
        assert_eq!(editor_blocks(&html)[0].2, "JavaScript");
        assert!(!looks_like_markdown("Hi Sam,\nsee you at 5.\n"));
    }
}
