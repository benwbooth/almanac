//! Spelling and grammar while writing a message, as Apple Mail underlines
//! them: Harper (offline, English; nothing leaves the computer) checks the
//! text on a thread of its own, so typing never waits for it.

use harper_core::linting::{LintGroup, LintKind, Linter, Suggestion};
use harper_core::spell::{FstDictionary, MergedDictionary, MutableDictionary};
use harper_core::parsers::PlainEnglish;
use harper_core::{DictWordMetadata, Dialect, Document};
use serde_json::{json, Value};
use std::sync::{mpsc, Arc, Mutex};

enum Job {
    Check { generation: i64, text: String, grammar: bool },
    Learn(String),
}

static WORKER: Mutex<Option<mpsc::Sender<Job>>> = Mutex::new(None);

/// The English of the computer's language setting (US unless it says otherwise).
fn dialect() -> Dialect {
    let lang = std::env::var("LC_ALL").or_else(|_| std::env::var("LC_MESSAGES")).or_else(|_| std::env::var("LANG")).unwrap_or_default();
    match lang.get(..5).unwrap_or("") {
        "en_GB" | "en_IE" | "en_NZ" => Dialect::British,
        "en_AU" => Dialect::Australian,
        "en_CA" => Dialect::Canadian,
        "en_IN" => Dialect::Indian,
        _ => Dialect::American,
    }
}

/// The rules, and the words they know (the curated ones and the person's).
pub struct Checker {
    rules: LintGroup,
    words: Arc<MergedDictionary>,
}

fn linter(learned: &[String]) -> Checker {
    let mut words = MutableDictionary::new();
    for w in learned {
        words.append_word_str(w, DictWordMetadata::default());
    }
    let mut dict = MergedDictionary::new();
    dict.add_dictionary(FstDictionary::curated());
    dict.add_dictionary(Arc::new(words));
    let words = Arc::new(dict);
    Checker { rules: LintGroup::new_curated(words.clone(), dialect()), words }
}

/// Problems in `text` as the editor counts positions (UTF-16), each with what
/// could replace it: [{start, end, kind: "spelling" | "grammar", message, suggestions}].
pub fn check(checker: &mut Checker, text: &str, grammar: bool) -> Vec<Value> {
    let chars: Vec<char> = text.chars().collect();
    // Character → editor position.
    let mut at = Vec::with_capacity(chars.len() + 1);
    let mut n = 0usize;
    for c in &chars {
        at.push(n);
        n += c.len_utf16();
    }
    at.push(n);
    let doc = Document::new(text, &PlainEnglish, &*checker.words);
    let mut out = Vec::new();
    for lint in checker.rules.lint(&doc) {
        let spelling = matches!(lint.lint_kind, LintKind::Spelling | LintKind::Typo);
        if !spelling && !grammar {
            continue;
        }
        let (s, e) = (lint.span.start.min(chars.len()), lint.span.end.min(chars.len()));
        if s >= e {
            continue;
        }
        let here: String = chars[s..e].iter().collect();
        let suggestions: Vec<String> = lint
            .suggestions
            .iter()
            .map(|sug| match sug {
                Suggestion::ReplaceWith(with) => with.iter().collect(),
                Suggestion::InsertAfter(more) => format!("{here}{}", more.iter().collect::<String>()),
                Suggestion::Remove => String::new(),
            })
            .take(6)
            .collect();
        out.push(json!({
            "start": at[s], "end": at[e], "word": here,
            "kind": if spelling { "spelling" } else { "grammar" },
            "message": lint.message, "suggestions": suggestions,
        }));
    }
    out
}

/// Starts the checker (once), with the words the person taught it; each
/// result goes to `deliver` as JSON {generation, lints}.
pub fn start(learned: Vec<String>, deliver: impl Fn(String) + Send + 'static) {
    let mut held = WORKER.lock().unwrap();
    if held.is_some() {
        return;
    }
    let (tx, rx) = mpsc::channel::<Job>();
    *held = Some(tx);
    std::thread::spawn(move || {
        let mut learned = learned;
        let mut lints = linter(&learned);
        while let Ok(mut job) = rx.recv() {
            // Only the newest text matters (typing sends one after another).
            while let Ok(next) = rx.try_recv() {
                match next {
                    Job::Learn(w) => {
                        learned.push(w);
                        lints = linter(&learned);
                    }
                    check => job = check,
                }
            }
            match job {
                Job::Learn(w) => {
                    learned.push(w);
                    lints = linter(&learned);
                }
                Job::Check { generation, text, grammar } => {
                    let found = check(&mut lints, &text, grammar);
                    deliver(json!({ "generation": generation, "lints": found }).to_string());
                }
            }
        }
    });
}

pub fn request(generation: i64, text: String, grammar: bool) {
    if let Some(tx) = WORKER.lock().unwrap().as_ref() {
        let _ = tx.send(Job::Check { generation, text, grammar });
    }
}

pub fn learn(word: String) {
    if let Some(tx) = WORKER.lock().unwrap().as_ref() {
        let _ = tx.send(Job::Learn(word));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_misspellings_and_grammar_with_editor_positions() {
        let mut l = linter(&[]);
        // An emoji first: two UTF-16 units, so positions are counted as the editor does.
        let text = "😀 I recieved the the letter.";
        let found = check(&mut l, text, true);
        let spelled = found.iter().find(|f| f["kind"] == "spelling").expect("a misspelling");
        assert_eq!(spelled["word"], "recieved");
        let start = spelled["start"].as_u64().unwrap() as usize;
        let utf16: Vec<u16> = text.encode_utf16().collect();
        assert_eq!(String::from_utf16(&utf16[start..start + 8]).unwrap(), "recieved");
        assert!(spelled["suggestions"].as_array().unwrap().iter().any(|s| s == "received"));
        assert!(found.iter().any(|f| f["kind"] == "grammar"), "the repeated “the the”: {found:?}");
        // Spelling only.
        assert!(check(&mut l, text, false).iter().all(|f| f["kind"] == "spelling"));
    }

    #[test]
    fn learned_words_are_not_misspelled() {
        let mut l = linter(&["Almanacky".to_string()]);
        assert!(check(&mut l, "The Almanacky app.", false).is_empty());
    }
}
