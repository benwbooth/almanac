//! Where a search's words are in a text, found as the search index finds
//! them: words starting with what's typed, ignoring case and accents. Used
//! for the bit of a message shown in the list while searching (the part that
//! matches, not its first lines).

use unicode_normalization::UnicodeNormalization;

/// A character as the search compares it: lower case, without accents.
fn fold_char(c: char) -> String {
    c.to_lowercase().collect::<String>().nfd().filter(|c| !unicode_normalization::char::is_combining_mark(*c)).collect()
}

/// A query's words, folded.
pub fn words(query: &str) -> Vec<String> {
    query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.chars().map(fold_char).collect::<String>())
        .filter(|w| !w.is_empty())
        .collect()
}

/// The matches in `text` (byte ranges): the start of each word of the text
/// that begins with one of `words`.
pub fn matches(text: &str, words: &[String]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    if words.is_empty() {
        return out;
    }
    let mut chars = text.char_indices().peekable();
    while let Some(&(start, c)) = chars.peek() {
        if !c.is_alphanumeric() {
            chars.next();
            continue;
        }
        // One word of the text: its characters, folded as they go.
        let mut folded = String::new();
        let mut best: Option<usize> = None;
        let mut end = start;
        while let Some(&(i, c)) = chars.peek() {
            if !c.is_alphanumeric() {
                break;
            }
            folded.push_str(&fold_char(c));
            end = i + c.len_utf8();
            if best.is_none() && words.iter().any(|w| folded.len() >= w.len() && folded.starts_with(w.as_str())) {
                best = Some(end);
            }
            chars.next();
        }
        let _ = end;
        if let Some(e) = best {
            out.push((start, e));
        }
    }
    out
}

/// The part of a message's text around its first match (a line or so, cut at
/// words, "…" where it's cut), or None when the words aren't in it.
pub fn snippet(body: &str, words: &[String]) -> Option<String> {
    let text: String = body.split_whitespace().collect::<Vec<_>>().join(" ");
    let (start, _) = *matches(&text, words).first()?;
    // Some words before it, then on to the end of the room a list row has.
    let before: Vec<(usize, char)> = text[..start].char_indices().collect();
    let mut from = 0;
    if before.len() > 30 {
        let cut = before[before.len() - 30].0;
        from = text[cut..start].find(' ').map(|i| cut + i + 1).unwrap_or(cut);
    }
    let rest = &text[from..];
    let mut to = rest.len();
    if rest.chars().count() > 140 {
        let limit = rest.char_indices().nth(140).map(|(i, _)| i).unwrap_or(rest.len());
        to = rest[..limit].rfind(' ').filter(|i| *i > start - from).unwrap_or(limit);
    }
    let mut out = String::new();
    if from > 0 {
        out.push('…');
    }
    out.push_str(rest[..to].trim());
    if to < rest.len() {
        out.push('…');
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn found_as_the_index_finds_them() {
        let w = words("Invoic café");
        assert_eq!(w, vec!["invoic".to_string(), "cafe".to_string()]);
        let t = "Your INVOICES from the Café are attached; reinvoice not.";
        let m = matches(t, &w);
        let found: Vec<&str> = m.iter().map(|(a, b)| &t[*a..*b]).collect();
        assert_eq!(found, vec!["INVOIC", "Café"], "word starts only, any case, accents ignored");
        let long = format!("{} the invoice is due {}", "word ".repeat(60), "more ".repeat(80));
        let s = snippet(&long, &words("invoice")).unwrap();
        assert!(s.starts_with('…') && s.ends_with('…') && s.contains("the invoice is due"), "{s}");
        assert!(s.chars().count() < 160);
        assert_eq!(snippet("short text", &words("invoice")), None);
        assert_eq!(snippet("Hi, the invoice", &words("invoice")).as_deref(), Some("Hi, the invoice"));
    }
}
