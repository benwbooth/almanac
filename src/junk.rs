//! Junk mail: a naive Bayes filter of Almanac's own, trained on this
//! person's mail (what the server put in Spam / Junk against what's in the
//! Inbox), then on every Move to Junk and Not Junk. Words and a few header
//! signs count once per message; a message's score is how strongly its most
//! telling tokens point one way. Kept in the mail database (junk_tokens).

use std::collections::{HashMap, HashSet};

use crate::data::Message;

/// What a message is made of, for the filter: words of its subject and
/// text, who sent it, and signs from its headers.
pub fn tokens(m: &Message, body: &str, list_heads: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut words = |prefix: &str, text: &str, out: &mut HashSet<String>| {
        for w in text.split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '$' || c == '€' || c == '£')) {
            let w = w.trim_matches('\'');
            let n = w.chars().count();
            if !(3..=24).contains(&n) || w.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            out.insert(format!("{prefix}{}", w.to_lowercase()));
        }
    };
    words("s:", &m.subject, &mut out);
    // (A long text: its start says enough, and keeps scoring quick.)
    let text: String = body.chars().take(20_000).collect();
    words("", &text, &mut out);
    words("n:", &m.from_name, &mut out);
    let email = m.from_email.trim().to_lowercase();
    if let Some((user, domain)) = email.rsplit_once('@') {
        out.insert(format!("fd:{domain}"));
        // The domain's last two parts (mail.shop.example → shop.example).
        let parts: Vec<&str> = domain.split('.').collect();
        if parts.len() > 2 {
            out.insert(format!("fd:{}", parts[parts.len() - 2..].join(".")));
        }
        if user.chars().filter(|c| c.is_ascii_digit()).count() >= 4 {
            out.insert("h:digits-in-sender".into());
        }
        if !m.reply_to.is_empty() && !m.reply_to.to_lowercase().contains(domain) {
            out.insert("h:reply-elsewhere".into());
        }
    } else {
        out.insert("h:no-sender".into());
    }
    if m.to.trim().is_empty() {
        out.insert("h:no-to".into());
    }
    let heads = list_heads.to_ascii_lowercase();
    if heads.contains("list-unsubscribe:") {
        out.insert("h:list".into());
    }
    if heads.contains("list-unsubscribe=one-click") {
        out.insert("h:one-click".into());
    }
    let links = body.matches("http://").count() + body.matches("https://").count();
    out.insert(format!("h:links-{}", match links { 0 => "0", 1..=3 => "few", 4..=15 => "some", _ => "many" }));
    if m.subject.chars().filter(|c| c.is_alphabetic()).count() > 8
        && m.subject.chars().filter(|c| c.is_alphabetic()).all(|c| c.is_uppercase())
    {
        out.insert("h:shouting".into());
    }
    if m.subject.contains('!') {
        out.insert("h:bang".into());
    }
    out
}

/// The filter: how many junk and good messages each token was seen in.
#[derive(Clone, Debug, Default)]
pub struct Model {
    pub spam_docs: u32,
    pub ham_docs: u32,
    pub counts: HashMap<String, (u32, u32)>,
}

/// The most telling tokens a score is made of.
const TELLING: usize = 15;
/// How much a token's own counts must outweigh "no idea" (Robinson's s).
const STRENGTH: f64 = 0.3;

impl Model {
    /// Learns a message as junk (or not); `forget` takes a lesson back
    /// (a message once learned the other way).
    pub fn learn(&mut self, toks: &HashSet<String>, spam: bool, forget: bool) {
        let step = |n: &mut u32| if forget { *n = n.saturating_sub(1) } else { *n += 1 };
        step(if spam { &mut self.spam_docs } else { &mut self.ham_docs });
        for t in toks {
            let e = self.counts.entry(t.clone()).or_default();
            step(if spam { &mut e.0 } else { &mut e.1 });
        }
    }

    /// Whether there's enough to go on (some of each).
    pub fn ready(&self) -> bool {
        self.spam_docs >= 10 && self.ham_docs >= 50
    }

    /// How likely a message is junk, 0 to 1 (0.5: no idea).
    pub fn score(&self, toks: &HashSet<String>) -> f64 {
        self.score_with(toks, TELLING, STRENGTH)
    }

    fn score_with(&self, toks: &HashSet<String>, telling: usize, strength: f64) -> f64 {
        if !self.ready() {
            return 0.5;
        }
        let (s, h) = (self.spam_docs as f64, self.ham_docs as f64);
        // Each token's leaning (Robinson): the share of junk among the
        // messages it was seen in, each kind counted as a share of its own
        // (so a few junk messages against thousands of good ones are fair),
        // pulled toward "no idea" when it's been seen little; as log odds.
        let mut leanings: Vec<f64> = toks
            .iter()
            .filter_map(|t| {
                let &(a, b) = self.counts.get(t)?;
                let n = (a + b) as f64;
                if n < 1.0 {
                    return None;
                }
                let (fs, fh) = (a as f64 / s.max(1.0), b as f64 / h.max(1.0));
                let p = fs / (fs + fh);
                let f = ((strength * 0.5 + n * p) / (strength + n)).clamp(0.001, 0.999);
                Some((f / (1.0 - f)).ln())
            })
            .collect();
        leanings.sort_by(|x, y| y.abs().partial_cmp(&x.abs()).unwrap_or(std::cmp::Ordering::Equal));
        leanings.truncate(telling);
        let sum: f64 = leanings.iter().sum::<f64>().clamp(-50.0, 50.0);
        1.0 / (1.0 + (-sum).exp())
    }
}

/// (A switch for the whole filter.)
pub const ENABLED: bool = true;

/// Above this a message is junk.
pub const THRESHOLD: f64 = 0.99;

// ------------------------------------------------- the filter in use

static MODEL: std::sync::Mutex<Option<Model>> = std::sync::Mutex::new(None);

/// A stored message's tokens (its text from the database; its preview while
/// the text is still downloading).
fn tokens_of(id: i64) -> Option<HashSet<String>> {
    let m = crate::mailstore::by_id(id)?;
    let (body, html, _) = crate::mailstore::text(id).unwrap_or_default();
    let text = if !body.trim().is_empty() {
        body
    } else if !html.is_empty() {
        crate::mime::html_to_text(&html)
    } else {
        m.preview.clone()
    };
    let heads = crate::mailstore::list_heads(id, false).unwrap_or_default();
    Some(tokens(&m, &text, &heads))
}

fn with_model<T>(f: impl FnOnce(&mut Model) -> T) -> T {
    let mut g = MODEL.lock().unwrap_or_else(|e| e.into_inner());
    if g.is_none() {
        *g = Some(crate::mailstore::junk_model().unwrap_or_default());
    }
    f(g.as_mut().unwrap())
}

/// Whether the filter has learned anything yet.
pub fn trained() -> bool {
    with_model(|m| m.spam_docs + m.ham_docs > 0)
}

/// Learns these messages as junk (or as good mail), taking back a lesson
/// the other way first. Slow-ish (reads each message): off the GUI thread.
pub fn learn(ids: &[i64], spam: bool) -> usize {
    let before = crate::mailstore::junk_learned(ids);
    let mut touched = HashSet::new();
    let mut learned = Vec::new();
    let lessons: Vec<(i64, HashSet<String>)> = ids
        .iter()
        .filter(|id| before.get(id) != Some(&spam))
        .filter_map(|id| tokens_of(*id).map(|t| (*id, t)))
        .collect();
    with_model(|model| {
        for (id, toks) in &lessons {
            if let Some(&was) = before.get(id) {
                model.learn(toks, was, true);
            }
            model.learn(toks, spam, false);
            touched.extend(toks.iter().cloned());
            learned.push((*id, Some(spam)));
        }
        crate::mailstore::save_junk(model, &touched, &learned);
    });
    lessons.len()
}

/// The first lessons: what's in Junk / Spam against the inboxes.
pub fn first_lessons(junk_folders: &[String], inbox_folders: &[String]) -> (usize, usize) {
    let examples = crate::mailstore::junk_examples(junk_folders, inbox_folders, 5000);
    let spam: Vec<i64> = examples.iter().filter(|e| e.1).map(|e| e.0).collect();
    let ham: Vec<i64> = examples.iter().filter(|e| !e.1).map(|e| e.0).collect();
    (learn(&spam, true), learn(&ham, false))
}

/// How likely message `id` is junk (0.5 when the filter can't tell).
pub fn score(id: i64) -> f64 {
    let Some(toks) = tokens_of(id) else { return 0.5 };
    with_model(|m| m.score(&toks))
}

/// What the filter was taught about message `id` (by a click), if anything.
pub fn taught(id: i64) -> Option<bool> {
    crate::mailstore::junk_learned(&[id]).get(&id).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(from: &str, subject: &str) -> Message {
        Message { from_email: from.into(), subject: subject.into(), to: "me@example.com".into(), ..Default::default() }
    }

    #[test]
    fn learns_what_junk_looks_like() {
        let mut model = Model::default();
        for i in 0..20 {
            let m = msg(&format!("winner{i}0000@prizes.example"), "CLAIM YOUR PRIZE NOW!");
            model.learn(&tokens(&m, "You won a free cruise. Click https://prizes.example/claim to claim your prize", ""), true, false);
        }
        for i in 0..60 {
            let m = msg("sam@friends.example", &format!("Lunch on day {i}"));
            model.learn(&tokens(&m, "Are you free for lunch on Tuesday? The usual place.", ""), false, false);
        }
        let junk = msg("lucky55551@prizes.example", "You are a winner, claim your prize");
        let good = msg("sam@friends.example", "Lunch Thursday?");
        let sj = model.score(&tokens(&junk, "Claim your free prize at https://prizes.example/x", ""));
        let sg = model.score(&tokens(&good, "Free for lunch on Thursday?", ""));
        assert!(sj > THRESHOLD, "junk {sj}");
        assert!(sg < 0.5, "good {sg}");
        // Taken back: as if never learned.
        let mut again = model.clone();
        let m = msg("x@y.example", "hello there");
        let t = tokens(&m, "", "");
        again.learn(&t, true, false);
        again.learn(&t, true, true);
        assert_eq!((again.spam_docs, again.counts.get("s:hello").copied()), (model.spam_docs, Some((0, 0))));
    }

    /// How well it does on real mail (counts only; nothing of the mail shown):
    /// the server's Spam against a slice of the Inbox, five ways round, each
    /// time learning from four fifths and scoring the fifth left out.
    /// ALMANAC_JUNK_EVAL=<copy of mail.sqlite> cargo test --release junk_on_real_mail -- --ignored --nocapture
    #[test]
    #[ignore]
    fn junk_on_real_mail() {
        let Ok(path) = std::env::var("ALMANAC_JUNK_EVAL") else { return };
        let c = rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let read = |sql: &str| -> Vec<HashSet<String>> {
            let mut st = c.prepare(sql).unwrap();
            st.query_map([], |r| {
                let m = Message {
                    from_name: r.get(0)?, from_email: r.get(1)?, to: r.get(2)?, reply_to: r.get(3)?, subject: r.get(4)?,
                    ..Default::default()
                };
                let body: String = r.get(5)?;
                let html: String = r.get(6)?;
                let text = if body.trim().is_empty() { crate::mime::html_to_text(&html) } else { body };
                Ok(tokens(&m, &text, ""))
            })
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
        };
        let cols = "from_name, from_email, to_addr, reply_to, subject, body, html";
        let spam = read(&format!("SELECT {cols} FROM messages WHERE folder LIKE '%/[Gmail]/Spam' OR folder LIKE '%/Junk' ORDER BY id"));
        let ham_n: usize = std::env::var("ALMANAC_JUNK_HAM").ok().and_then(|x| x.parse().ok()).unwrap_or(5000);
        // The Inbox's newest, and as many again from across its years.
        let ham = read(&format!(
            "SELECT {cols} FROM messages WHERE id IN (SELECT id FROM (SELECT id FROM messages WHERE folder LIKE '%/INBOX' ORDER BY date DESC LIMIT {h})
             UNION SELECT id FROM (SELECT id FROM messages WHERE folder LIKE '%/INBOX' AND id % 17 = 3 LIMIT {h}))",
            h = ham_n / 2
        ));
        eprintln!("junk {} · good {}", spam.len(), ham.len());
        let folds = 5;
        let combos: Vec<(usize, f64)> = std::env::var("ALMANAC_JUNK_SWEEP").ok().map(|_| vec![(10, 1.0), (15, 1.0), (30, 0.3), (15, 0.3), (60, 1.0), (15, 3.0)]).unwrap_or(vec![(TELLING, STRENGTH)]);
        for (telling, strength) in combos {
        eprintln!("-- {telling} tokens, strength {strength}");
        let mut scored: Vec<(f64, bool)> = Vec::new();
        for k in 0..folds {
            let mut model = Model::default();
            for (i, t) in spam.iter().enumerate().filter(|(i, _)| i % folds != k) {
                let _ = i;
                model.learn(t, true, false);
            }
            for (_, t) in ham.iter().enumerate().filter(|(i, _)| i % folds != k) {
                model.learn(t, false, false);
            }
            scored.extend(spam.iter().enumerate().filter(|(i, _)| i % folds == k).map(|(_, t)| (model.score_with(t, telling, strength), true)));
            scored.extend(ham.iter().enumerate().filter(|(i, _)| i % folds == k).map(|(_, t)| (model.score_with(t, telling, strength), false)));
        }
        for th in [0.5, 0.9, 0.99] {
            let tp = scored.iter().filter(|(s, j)| *j && *s > th).count() as f64;
            let fp = scored.iter().filter(|(s, j)| !*j && *s > th).count() as f64;
            let fneg = scored.iter().filter(|(s, j)| *j && *s <= th).count() as f64;
            eprintln!(
                "above {th:<6} precision {:.3}  recall {:.3}  (caught {tp} of {}, good mail called junk {fp} of {})",
                if tp + fp > 0.0 { tp / (tp + fp) } else { 1.0 },
                tp / (tp + fneg),
                tp + fneg,
                scored.iter().filter(|(_, j)| !*j).count()
            );
        }
        }
    }
}
