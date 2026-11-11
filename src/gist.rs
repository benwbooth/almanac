//! The heart of a message, for closed messages in a conversation: the first
//! few sentences the person actually wrote, without what's quoted from earlier
//! messages, greetings, signatures, or the boilerplate notifications wrap
//! around a comment ("X left a comment (repo#12)", "Reply to this email
//! directly…", "Unsubscribe"), so the flow of a conversation can be followed
//! with its messages closed.

/// At most this many sentences, and about this many characters.
const SENTENCES: usize = 3;
const CHARS: usize = 280;

/// The gist of a message from its plain text, or its HTML when there's none.
pub fn gist(body: &str, html: &str) -> String {
    let text = if !body.trim().is_empty() {
        body.to_string()
    } else if !html.trim().is_empty() {
        crate::mime::html_to_text(&without_html_quotes(html))
    } else {
        return String::new();
    };
    let kept = heart_lines(&text);
    let joined = tidy(&kept.join(" "));
    if joined.is_empty() {
        // Nothing left once trimmed: the start of the text as it is.
        return first_sentences(&tidy(&text.lines().filter(|l| !l.trim_start().starts_with('>')).collect::<Vec<_>>().join(" ")));
    }
    first_sentences(&joined)
}

/// HTML without what it quotes: blockquotes (all of each, however nested) and
/// Gmail's / Outlook's quoted part (from where it starts to the end).
fn without_html_quotes(html: &str) -> String {
    // (ASCII only: the same length as the HTML, so its places are the HTML's.)
    let lower = html.to_ascii_lowercase();
    // Quoted parts that run to the end.
    let mut end = html.len();
    for marker in ["class=\"gmail_quote", "id=\"appendonsend", "id=\"divrplyfwdmsg", "class=\"yahoo_quoted", "id=\"stopspelling"] {
        if let Some(at) = lower.find(marker) {
            let start = lower[..at].rfind('<').unwrap_or(at);
            end = end.min(start);
        }
    }
    let html = &html[..end];
    let lower = &lower[..end];
    // Blockquotes, nested ones included.
    let mut out = String::with_capacity(html.len());
    let mut i = 0;
    while let Some(rel) = lower[i..].find("<blockquote") {
        let start = i + rel;
        out.push_str(&html[i..start]);
        let mut depth = 0usize;
        let mut j = start;
        loop {
            let open = lower[j..].find("<blockquote").map(|x| j + x);
            let close = lower[j..].find("</blockquote").map(|x| j + x);
            match (open, close) {
                (Some(o), Some(c)) if o < c => {
                    depth += 1;
                    j = o + 1;
                }
                (_, Some(c)) => {
                    depth = depth.saturating_sub(1);
                    j = lower[c..].find('>').map(|x| c + x + 1).unwrap_or(lower.len());
                    if depth == 0 {
                        break;
                    }
                }
                _ => {
                    j = lower.len();
                    break;
                }
            }
        }
        i = j;
    }
    out.push_str(&html[i..]);
    out
}

/// Where what follows is no longer the person's own words.
fn ends_here(line: &str, next: &str) -> bool {
    let l = line.trim();
    let low = l.to_lowercase();
    if l == "--" || l == "—" || l == "––" || low.starts_with("-----original message") || low.starts_with("---------- forwarded message")
        || (l.len() >= 8 && l.chars().all(|c| c == '_' || c == '-'))
    {
        return true;
    }
    // "On Fri, 7 Nov 2026 at 10:02, Sam <sam@example.com> wrote:" (sometimes on two lines), and others' languages.
    let two = format!("{low} {}", next.trim().to_lowercase());
    for (start, end) in [("on ", "wrote:"), ("am ", "schrieb"), ("le ", "a écrit"), ("el ", "escribió"), ("il ", "ha scritto"), ("op ", "geschreven:")] {
        if low.starts_with(start) && (low.trim_end().ends_with(end) || (two.contains(end) && !low.contains('.') && l.len() < 140)) {
            return true;
        }
    }
    // Outlook's header block for the message replied to.
    if (low.starts_with("from:") || low.starts_with("von:") || low.starts_with("de:")) && {
        let n = next.trim().to_lowercase();
        n.starts_with("sent:") || n.starts_with("date:") || n.starts_with("to:") || n.starts_with("gesendet:") || n.starts_with("envoyé")
    } {
        return true;
    }
    // Signatures and the footers of notifications and newsletters.
    for footer in [
        "sent from my ", "get outlook for", "reply to this email directly", "you are receiving this because", "view it on github",
        "unsubscribe", "manage notifications", "manage your notification", "you received this message because", "to unsubscribe",
        "this email was sent to", "if you no longer wish", "view this email in your browser", "update your preferences",
    ] {
        if low.starts_with(footer) {
            return true;
        }
    }
    false
}

/// Lines that aren't the person's words but don't end them either.
fn skipped(line: &str) -> bool {
    let l = line.trim();
    if l.is_empty() || l.starts_with('>') {
        return true;
    }
    let low = l.to_lowercase();
    // "adamcstephens left a comment (NixOS/nixpkgs#582838)" and the like.
    for doing in [" left a comment", " commented on", " requested changes", " approved this", " requested your review", " requested a review", " reviewed ", " pushed ", " merged ", " closed ", " reopened ", " mentioned you"] {
        if low.contains(doing) && (l.ends_with(')') || l.ends_with('.') || l.ends_with(':') || l.contains('#')) && l.len() < 160 {
            return true;
        }
    }
    // A review comment's place: "In pkgs/by-name/br/brave/package.nix:".
    if low.starts_with("in ") && l.ends_with(':') && l.contains('/') && l.split_whitespace().count() <= 3 {
        return true;
    }
    // Greetings on their own ("Hi Ben,", "Hello everyone!").
    let words = low.split_whitespace().count();
    if words <= 4 && l.len() <= 40 {
        for hello in ["hi", "hello", "hey", "dear", "good morning", "good afternoon", "good evening", "greetings", "hiya", "howdy"] {
            if low == hello || low.starts_with(&format!("{hello} ")) || low.starts_with(&format!("{hello},")) || low.starts_with(&format!("{hello}!")) {
                return true;
            }
        }
    }
    // Links on their own, and footnote links ("[1]: https://…").
    let bare = l.trim_start_matches(|c: char| c == '[' || c == ']' || c.is_ascii_digit() || c == ':' || c == ' ');
    if (bare.starts_with("http://") || bare.starts_with("https://") || bare.starts_with("<http")) && !bare.contains(' ') {
        return true;
    }
    false
}

/// Sign-offs: what follows is a name.
fn sign_off(line: &str) -> bool {
    let low = line.trim().to_lowercase();
    let low = low.trim_end_matches([',', '!', '.', ' ']);
    low.len() <= 25
        && ["thanks", "thank you", "thanks again", "many thanks", "best", "best regards", "regards", "kind regards", "cheers", "sincerely", "all the best", "warm regards", "talk soon", "take care"]
            .contains(&low)
}

fn heart_lines(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut kept = Vec::new();
    for (k, line) in lines.iter().enumerate() {
        let next = lines.get(k + 1).copied().unwrap_or("");
        if ends_here(line, next) || (sign_off(line) && !kept.is_empty()) {
            break;
        }
        if !skipped(line) {
            kept.push(line.trim().to_string());
        }
    }
    kept
}

/// Markdown links to their words, markup marks dropped, spaces collapsed.
fn tidy(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        // [words](https://…) → words
        if chars[i] == '[' {
            if let Some(close) = chars[i..].iter().position(|&c| c == ']').map(|p| i + p) {
                if chars.get(close + 1) == Some(&'(') {
                    if let Some(end) = chars[close..].iter().position(|&c| c == ')').map(|p| close + p) {
                        out.extend(&chars[i + 1..close]);
                        i = end + 1;
                        continue;
                    }
                }
            }
        }
        if chars[i] != '*' && chars[i] != '`' {
            out.push(chars[i]);
        }
        i += 1;
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The first few sentences, cut at a word if they run long.
fn first_sentences(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut count = 0;
    let mut end = chars.len();
    let mut word_start = 0;
    for (i, &c) in chars.iter().enumerate() {
        if c.is_whitespace() {
            word_start = i + 1;
            continue;
        }
        let next_space = chars.get(i + 1).is_none_or(|n| n.is_whitespace());
        if matches!(c, '.' | '!' | '?') && next_space {
            // "1." of a list, "e.g." and the like aren't the end of a sentence.
            let word: String = chars[word_start..i].iter().collect();
            let listish = !word.is_empty() && word.chars().all(|x| x.is_ascii_digit());
            let short = matches!(word.to_lowercase().as_str(), "e.g" | "i.e" | "etc" | "vs" | "mr" | "mrs" | "dr" | "approx");
            if !listish && !short && !word.is_empty() {
                count += 1;
                if count == SENTENCES {
                    end = i + 1;
                    break;
                }
            }
        }
    }
    let mut out: String = chars[..end].iter().collect();
    if out.chars().count() > CHARS {
        let cut: String = out.chars().take(CHARS).collect();
        let at = cut.rfind(' ').unwrap_or(cut.len());
        out = format!("{}…", cut[..at].trim_end_matches([',', ';', ':', ' ']));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::gist;

    #[test]
    fn a_github_comment_is_just_what_was_said() {
        let body = "adamcstephens left a comment (NixOS/nixpkgs#582838)\r\n\r\nSorry if this merge was too quick for others. I clearly wasn't paying enough attention to its length of being open, but stand by my merge. Even if this package was being kept up to date\r\n\r\nI'm always happy to reconsider or roll back.\r\n\r\n-- \r\nReply to this email directly or view it on GitHub:\r\nhttps://github.com/NixOS/nixpkgs/pull/582838#issuecomment-1\r\nYou are receiving this because you are subscribed to this thread.\r\n";
        let g = gist(body, "");
        assert!(g.starts_with("Sorry if this merge was too quick for others."), "{g}");
        assert!(!g.contains("left a comment") && !g.contains("Reply to this email"), "{g}");
        assert!(g.contains("I clearly wasn't"), "{g}");
    }

    #[test]
    fn a_reply_quoting_the_one_before_keeps_only_the_reply() {
        let body = "MagicRB left a comment (NixOS/nixpkgs#582838)\n\n> Why don't you drop Nyxt? It's a browser, and has no maintainers too.\n\nnyxt relies on webkitgtk which is technically the browser and that has maintainers. Same cannot be said for brave.\n\n-- \nReply to this email directly";
        let g = gist(body, "");
        assert!(g.starts_with("nyxt relies on webkitgtk"), "{g}");
        assert!(!g.contains("Why don't you drop"), "{g}");
    }

    #[test]
    fn an_ordinary_reply_loses_greeting_quote_and_sign_off() {
        let body = "Hi Ben,\n\nSounds good, Saturday works for me. I'll bring the map.\n\nThanks,\nSam\n\nOn Fri, 6 Nov 2026 at 17:57, Ben Booth <ben@example.com>\nwrote:\n> Want to do the lake loop trail?\n";
        assert_eq!(gist(body, ""), "Sounds good, Saturday works for me. I'll bring the map.");
    }

    #[test]
    fn an_outlook_reply_stops_at_the_quoted_header() {
        let body = "Approved, go ahead with the order.\n\nFrom: Jordan Diaz <jordan@example.com>\nSent: Thursday, November 5, 2026 9:12 AM\nTo: Me\nSubject: Draft budget\n\nHere is the draft budget.";
        assert_eq!(gist(body, ""), "Approved, go ahead with the order.");
    }

    #[test]
    fn html_without_plain_text_drops_its_quotes() {
        let html = "<div>Looks great, ship it!</div><div class=\"gmail_quote\"><div>On Thu, Taylor wrote:</div><blockquote>Here's the new layout.</blockquote></div>";
        assert_eq!(gist("", html), "Looks great, ship it!");
        let inline = "<p>Agreed.</p><blockquote><p>Should we <blockquote>nested</blockquote> merge?</p></blockquote><p>Merging now.</p>";
        assert_eq!(gist("", inline), "Agreed. Merging now.");
    }

    #[test]
    fn a_newsletter_keeps_its_opening_and_loses_its_footer() {
        let body = "Garden Club · Spring\n\nPlant swap Saturday 10–2 at the community hall. Workshops every Tuesday evening. Bring cuttings and seeds. Tomato contest in June.\n\nUnsubscribe | View this email in your browser";
        let g = gist(body, "");
        assert!(g.contains("Plant swap Saturday"), "{g}");
        assert!(!g.contains("Tomato contest") && !g.contains("Unsubscribe"), "only the first sentences: {g}");
    }

    #[test]
    fn lists_and_numbers_dont_end_sentences_early() {
        let body = "Some data here:\n\n1. Over the past year the lag was ~1.25 days. 2. Brave itself lags 1 day. 3. Exploited CVEs lag more. 4. More.";
        let g = gist(body, "");
        assert!(g.contains("1.25 days") && g.contains("lags 1 day") && g.contains("lag more."), "{g}");
        assert!(!g.contains("4. More"), "{g}");
    }

    #[test]
    fn long_sentences_are_cut_at_a_word() {
        let body = "word ".repeat(200);
        let g = gist(&body, "");
        assert!(g.chars().count() <= 281 && g.ends_with('…'), "{g}");
    }
}
