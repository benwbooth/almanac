//! Searches written as in Gmail: `from:jane subject:"weekly report" -is:read
//! after:2025/01/01 (invoice OR receipt) has:attachment`. Read into a tree,
//! then turned into SQL over the mail here (words through the full-text
//! index), an IMAP search for servers other than Gmail (Gmail gets the words
//! as typed), a check of one message (for what a server finds), and the words
//! to mark in what's shown.

use crate::mailstore::Arg;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Any,
    From,
    To,
    Cc,
    Bcc,
    Subject,
    DeliveredTo,
    List,
    Filename,
    MsgId,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    And(Vec<Node>),
    Or(Vec<Node>),
    Not(Box<Node>),
    /// Words (each a word's start), or a phrase (these words, in this order).
    Text { field: Field, words: Vec<String>, phrase: bool, raw: String },
    Unread,
    Read,
    /// Flagged; with a colour (Apple Mail's order: 0 red … 6 gray).
    Flagged(Option<u8>),
    Attachment,
    Important,
    Reminder,
    /// in: a kind of mailbox ("inbox", "sent", …; "anywhere": all of them).
    In(String),
    /// label: a mailbox by its name.
    Label(String),
    /// Received on or after / before (seconds).
    After(i64),
    Before(i64),
    /// Something that can't be told here (size:, AROUND): left out.
    True,
}

// ---------------------------------------------------------------- reading

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Open,
    Close,
    OpenBrace,
    CloseBrace,
    Not,
    Or,
    Word(String),
    Phrase(String),
    /// An operator and what follows its colon (empty: a group or phrase next).
    Op(String, String),
}

const OPERATORS: &[&str] = &[
    "from", "to", "cc", "bcc", "subject", "deliveredto", "list", "filename", "rfc822msgid", "is", "has", "in", "label", "after", "before", "older",
    "newer", "older_than", "newer_than", "larger", "smaller", "size",
];

/// Whether a search uses anything beyond plain words (then it's read as a
/// query; plain words keep the quick search of words).
pub fn has_operators(q: &str) -> bool {
    lex(q).iter().any(|t| !matches!(t, Tok::Word(_)))
}

fn lex(q: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let chars: Vec<char> = q.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        match c {
            '(' => {
                out.push(Tok::Open);
                i += 1;
                continue;
            }
            ')' => {
                out.push(Tok::Close);
                i += 1;
                continue;
            }
            '{' => {
                out.push(Tok::OpenBrace);
                i += 1;
                continue;
            }
            '}' => {
                out.push(Tok::CloseBrace);
                i += 1;
                continue;
            }
            '"' => {
                // (Not closed yet, while typing: the rest is the phrase.)
                let end = chars[i + 1..].iter().position(|c| *c == '"').map(|p| i + 1 + p).unwrap_or(chars.len());
                out.push(Tok::Phrase(chars[i + 1..end].iter().collect()));
                i = (end + 1).min(chars.len());
                continue;
            }
            '-' if i + 1 < chars.len() && !chars[i + 1].is_whitespace() && (i == 0 || chars[i - 1].is_whitespace() || chars[i - 1] == '(') => {
                out.push(Tok::Not);
                i += 1;
                continue;
            }
            _ => {}
        }
        let start = i;
        while i < chars.len() && !chars[i].is_whitespace() && !matches!(chars[i], '(' | ')' | '{' | '}' | '"') {
            i += 1;
        }
        let word: String = chars[start..i].iter().collect();
        if word == "OR" || word == "|" {
            out.push(Tok::Or);
        } else if word == "AND" {
            // (Implied anyway.)
        } else if let Some((op, rest)) = word.split_once(':').filter(|(op, _)| OPERATORS.contains(&op.to_ascii_lowercase().as_str())) {
            out.push(Tok::Op(op.to_ascii_lowercase(), rest.to_string()));
        } else {
            out.push(Tok::Word(word));
        }
    }
    out
}

fn text(field: Field, raw: &str, phrase: bool) -> Node {
    let words: Vec<String> = raw.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(|w| w.to_lowercase()).collect();
    if words.is_empty() && !matches!(field, Field::Filename | Field::MsgId) {
        return Node::True;
    }
    Node::Text { field, words, phrase: phrase && raw.split_whitespace().count() > 1, raw: raw.to_string() }
}

struct Parser {
    toks: Vec<Tok>,
    at: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.at)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.at).cloned();
        self.at += 1;
        t
    }
    fn or(&mut self, field: Field) -> Node {
        let mut parts = vec![self.and(field)];
        while self.peek() == Some(&Tok::Or) {
            self.next();
            parts.push(self.and(field));
        }
        if parts.len() == 1 { parts.pop().unwrap() } else { Node::Or(parts) }
    }
    fn and(&mut self, field: Field) -> Node {
        let mut parts = Vec::new();
        while let Some(t) = self.peek() {
            if matches!(t, Tok::Or | Tok::Close | Tok::CloseBrace) {
                break;
            }
            parts.push(self.unary(field));
        }
        match parts.len() {
            0 => Node::True,
            1 => parts.pop().unwrap(),
            _ => Node::And(parts),
        }
    }
    fn unary(&mut self, field: Field) -> Node {
        if self.peek() == Some(&Tok::Not) {
            self.next();
            return Node::Not(Box::new(self.unary(field)));
        }
        self.primary(field)
    }
    fn group(&mut self, field: Field, brace: bool) -> Node {
        let inner = if brace {
            // {a b c}: any of them.
            let mut parts = Vec::new();
            while let Some(t) = self.peek() {
                if matches!(t, Tok::CloseBrace) {
                    break;
                }
                if matches!(t, Tok::Or) {
                    self.next();
                    continue;
                }
                parts.push(self.unary(field));
            }
            Node::Or(parts)
        } else {
            self.or(field)
        };
        self.next();
        inner
    }
    fn primary(&mut self, field: Field) -> Node {
        match self.next() {
            Some(Tok::Open) => self.group(field, false),
            Some(Tok::OpenBrace) => self.group(field, true),
            Some(Tok::Phrase(p)) => text(field, &p, true),
            Some(Tok::Word(w)) => text(field, &w, false),
            Some(Tok::Op(op, value)) => self.operator(&op, value),
            _ => Node::True,
        }
    }
    fn operator(&mut self, op: &str, value: String) -> Node {
        let field = match op {
            "from" => Some(Field::From),
            "to" => Some(Field::To),
            "cc" => Some(Field::Cc),
            "bcc" => Some(Field::Bcc),
            "subject" => Some(Field::Subject),
            "deliveredto" => Some(Field::DeliveredTo),
            "list" => Some(Field::List),
            "filename" => Some(Field::Filename),
            "rfc822msgid" => Some(Field::MsgId),
            _ => None,
        };
        if let Some(f) = field {
            if value.is_empty() {
                // from:(a OR b), subject:"a phrase"
                return match self.peek() {
                    Some(Tok::Open) => {
                        self.next();
                        self.group(f, false)
                    }
                    Some(Tok::OpenBrace) => {
                        self.next();
                        self.group(f, true)
                    }
                    Some(Tok::Phrase(_)) => match self.next() {
                        Some(Tok::Phrase(p)) => text(f, &p, true),
                        _ => Node::True,
                    },
                    _ => Node::True,
                };
            }
            return text(f, &value, false);
        }
        let v = value.to_lowercase();
        match op {
            "is" => match v.as_str() {
                "unread" => Node::Unread,
                "read" => Node::Read,
                "starred" | "flagged" => Node::Flagged(None),
                "important" | "vip" => Node::Important,
                "snoozed" => Node::Reminder,
                _ => Node::True,
            },
            "has" => match v.as_str() {
                "attachment" | "attachments" => Node::Attachment,
                c if c.ends_with("-star") || c.ends_with("-flag") => {
                    let colour = c.trim_end_matches("-star").trim_end_matches("-flag");
                    match ["red", "orange", "yellow", "green", "blue", "purple", "gray"].iter().position(|x| *x == colour) {
                        Some(i) => Node::Flagged(Some(i as u8)),
                        None => Node::Flagged(None),
                    }
                }
                _ => Node::True,
            },
            "in" => match v.as_str() {
                "starred" | "flagged" => Node::Flagged(None),
                "spam" => Node::In("junk".into()),
                "" => Node::True,
                k => Node::In(k.trim_end_matches('s').replace("draft", "drafts").replace("anywhere", "anywhere")),
            },
            "label" => {
                if v.is_empty() {
                    if let Some(Tok::Phrase(p)) = self.peek().cloned() {
                        self.next();
                        return Node::Label(p.to_lowercase());
                    }
                    return Node::True;
                }
                Node::Label(v.replace('-', " "))
            }
            "after" | "newer" => date(&v).map(Node::After).unwrap_or(Node::True),
            "before" | "older" => date(&v).map(Node::Before).unwrap_or(Node::True),
            "older_than" => ago(&v).map(Node::Before).unwrap_or(Node::True),
            "newer_than" => ago(&v).map(Node::After).unwrap_or(Node::True),
            // (Message sizes aren't kept here: left to the server.)
            _ => Node::True,
        }
    }
}

fn date(v: &str) -> Option<i64> {
    let v = v.replace('/', "-");
    let d = chrono::NaiveDate::parse_from_str(&v, "%Y-%m-%d").ok()?;
    use chrono::TimeZone;
    chrono::Local.from_local_datetime(&d.and_hms_opt(0, 0, 0)?).earliest().map(|t| t.timestamp())
}

fn ago(v: &str) -> Option<i64> {
    let (n, unit) = v.split_at(v.len().checked_sub(1)?);
    let n: i64 = n.parse().ok()?;
    let days = match unit {
        "d" => n,
        "m" => n * 30,
        "y" => n * 365,
        _ => return None,
    };
    Some(chrono::Local::now().timestamp() - days * 86_400)
}

/// A search read as a query.
pub fn parse(q: &str) -> Node {
    let mut p = Parser { toks: lex(q), at: 0 };
    let mut parts = vec![p.or(Field::Any)];
    // (Stray closing brackets: what follows is read too.)
    while p.at < p.toks.len() {
        p.next();
        parts.push(p.or(Field::Any));
    }
    simplify(if parts.len() == 1 { parts.pop().unwrap() } else { Node::And(parts) })
}

fn simplify(n: Node) -> Node {
    match n {
        Node::And(v) => {
            let v: Vec<Node> = v.into_iter().map(simplify).filter(|x| *x != Node::True).collect();
            match v.len() {
                0 => Node::True,
                1 => v.into_iter().next().unwrap(),
                _ => Node::And(v),
            }
        }
        Node::Or(v) => {
            let v: Vec<Node> = v.into_iter().map(simplify).collect();
            if v.iter().any(|x| *x == Node::True) {
                return Node::True;
            }
            if v.len() == 1 { v.into_iter().next().unwrap() } else { Node::Or(v) }
        }
        Node::Not(b) => match simplify(*b) {
            Node::True => Node::True,
            x => Node::Not(Box::new(x)),
        },
        x => x,
    }
}

// ---------------------------------------------------------------- uses

/// The mailboxes (for in: and label:): (id, kind, name, path); and the VIPs.
#[derive(Default)]
pub struct Places {
    pub folders: Vec<(String, String, String, String)>,
    pub vips: Vec<String>,
}

impl Places {
    fn of_kind(&self, kind: &str) -> Vec<String> {
        self.folders.iter().filter(|f| f.1 == kind).map(|f| f.0.clone()).collect()
    }
    fn named(&self, name: &str) -> Vec<String> {
        let n = name.to_lowercase();
        self.folders
            .iter()
            .filter(|f| f.2.to_lowercase() == n || f.3.to_lowercase() == n || f.3.to_lowercase().rsplit('/').next() == Some(n.as_str()))
            .map(|f| f.0.clone())
            .collect()
    }
}

impl Node {
    /// Whether it says where to look (in:, label:): then all mailboxes are
    /// searched, not only the one shown.
    pub fn places(&self) -> bool {
        match self {
            Node::And(v) | Node::Or(v) => v.iter().any(|n| n.places()),
            Node::In(_) | Node::Label(_) => true,
            _ => false,
        }
    }

    /// As SQL over the messages table's columns (`p`: their prefix, as "m.";
    /// arguments as ?#1, ?#2 …, see `number_args`).
    pub fn sql(&self, p: &str, places: &Places, args: &mut Vec<Arg>) -> String {
        let arg = |a: Arg, args: &mut Vec<Arg>| -> String {
            args.push(a);
            format!("?#{}", args.len())
        };
        match self {
            Node::And(v) => format!("({})", v.iter().map(|n| n.sql(p, places, args)).collect::<Vec<_>>().join(" AND ")),
            Node::Or(v) => format!("({})", v.iter().map(|n| n.sql(p, places, args)).collect::<Vec<_>>().join(" OR ")),
            Node::Not(b) => format!("NOT ({})", b.sql(p, places, args)),
            Node::True => "1".into(),
            Node::Unread => format!("{p}read = 0"),
            Node::Read => format!("{p}read = 1"),
            Node::Flagged(None) => format!("{p}flagged = 1"),
            Node::Flagged(Some(0)) => format!("({p}flagged = 1 AND {p}id NOT IN (SELECT id FROM flag_colors))"),
            Node::Flagged(Some(c)) => format!("({p}flagged = 1 AND {p}id IN (SELECT id FROM flag_colors WHERE color = {c}))"),
            Node::Attachment => format!("{p}attachments <> '[]'"),
            Node::Reminder => format!("{p}remind_at > 0"),
            Node::Important => {
                if places.vips.is_empty() {
                    return "0".into();
                }
                let marks: Vec<String> = places.vips.iter().map(|v| arg(Arg::Text(v.to_lowercase()), args)).collect();
                format!("lower({p}from_email) IN ({})", marks.join(","))
            }
            Node::In(kind) if kind == "anywhere" || kind == "all" => "1".into(),
            Node::In(kind) => folders_sql(p, &places.of_kind(kind), args),
            Node::Label(name) => folders_sql(p, &places.named(name), args),
            Node::After(t) => format!("{p}date >= {t}"),
            Node::Before(t) => format!("{p}date < {t}"),
            Node::Text { field, words, phrase, raw } => {
                let fts = |col: &str, args: &mut Vec<Arg>| -> String {
                    let e = fts_expr(col, words, *phrase);
                    format!("{p}id IN (SELECT rowid FROM search WHERE search MATCH {})", arg(Arg::Text(e), args))
                };
                // A person's field: the index finds those with the words among
                // their people; the column says whether it's this one.
                let person = |cols: &[&str], args: &mut Vec<Arg>| -> String {
                    let mut parts = vec![fts("people", args)];
                    let pieces: Vec<String> = if *phrase { vec![words.join(" ")] } else { words.clone() };
                    for w in pieces {
                        let a = arg(Arg::Text(format!("%{w}%")), args);
                        parts.push(format!("({})", cols.iter().map(|c| format!("lower({p}{c}) LIKE {a}")).collect::<Vec<_>>().join(" OR ")));
                    }
                    format!("({})", parts.join(" AND "))
                };
                match field {
                    Field::Any => fts("", args),
                    Field::Subject => fts("subject", args),
                    Field::From => person(&["from_name", "from_email"], args),
                    Field::To | Field::DeliveredTo => person(&["to_addr"], args),
                    Field::Cc => person(&["cc"], args),
                    Field::Bcc | Field::List => person(&["to_addr", "cc"], args),
                    Field::Filename => format!("lower({p}attachments) LIKE {}", arg(Arg::Text(format!("%{}%", raw.to_lowercase())), args)),
                    Field::MsgId => format!("{p}message_id LIKE {}", arg(Arg::Text(format!("%{}%", raw.trim_matches(['<', '>']))), args)),
                }
            }
        }
    }

    /// Whether one message is what's searched for (`text`: what it shows; the
    /// mailbox parts left out: the server was asked in those already).
    pub fn matches(&self, m: &crate::data::Message, text: &str) -> bool {
        let has = |hay: &str, words: &[String], phrase: bool| -> bool {
            if phrase {
                phrase_in(hay, words)
            } else {
                let w = crate::searchhl::words(&words.join(" "));
                w.iter().all(|x| !crate::searchhl::matches(hay, std::slice::from_ref(x)).is_empty())
            }
        };
        match self {
            Node::And(v) => v.iter().all(|n| n.matches(m, text)),
            Node::Or(v) => v.iter().any(|n| n.matches(m, text)),
            Node::Not(b) => !b.matches(m, text),
            Node::True | Node::In(_) | Node::Label(_) | Node::Important => true,
            Node::Unread => !m.read,
            Node::Read => m.read,
            Node::Flagged(c) => m.flagged && c.is_none_or(|c| m.flag_color == c),
            Node::Attachment => !m.attachments.is_empty(),
            Node::Reminder => m.remind_at > 0,
            Node::After(t) => m.date >= *t,
            Node::Before(t) => m.date < *t,
            Node::Text { field, words, phrase, raw } => match field {
                Field::Any => {
                    let all = format!("{}\n{} {} {} {}\n{}", m.subject, m.from_name, m.from_email, m.to, m.cc, text);
                    has(&all, words, *phrase)
                }
                Field::Subject => has(&m.subject, words, *phrase),
                Field::From => has(&format!("{} {}", m.from_name, m.from_email), words, *phrase),
                Field::To | Field::DeliveredTo => has(&m.to, words, *phrase),
                Field::Cc => has(&m.cc, words, *phrase),
                Field::Bcc | Field::List => has(&format!("{} {}", m.to, m.cc), words, *phrase),
                Field::Filename => m.attachments.iter().any(|a| a.to_lowercase().contains(&raw.to_lowercase())),
                Field::MsgId => m.message_id.contains(raw.trim_matches(['<', '>'])),
            },
        }
    }

    /// Full-text searches every result must match (its words, phrases and
    /// field words not under OR or NOT): a quick first cut through the index.
    pub fn must_match(&self) -> Vec<String> {
        match self {
            Node::And(v) => v.iter().flat_map(|n| n.must_match()).collect(),
            Node::Text { field, words, phrase, .. } => match field {
                Field::Any => vec![fts_expr("", words, *phrase)],
                Field::Subject => vec![fts_expr("subject", words, *phrase)],
                Field::From | Field::To | Field::Cc | Field::Bcc | Field::DeliveredTo | Field::List => vec![fts_expr("people", words, *phrase)],
                _ => Vec::new(),
            },
            _ => Vec::new(),
        }
    }

    /// What every result has that the small table of messages knows (read or
    /// not, dates), as SQL over it as `t`: a quick first cut.
    pub fn thread_filter(&self) -> Vec<String> {
        match self {
            Node::And(v) => v.iter().flat_map(|n| n.thread_filter()).collect(),
            Node::Unread => vec!["t.read = 0".into()],
            Node::Read => vec!["t.read = 1".into()],
            Node::After(x) => vec![format!("t.date >= {x}")],
            Node::Before(x) => vec![format!("t.date < {x}")],
            _ => Vec::new(),
        }
    }

    /// Whether telling needs the message's text (not only its headers).
    pub fn needs_text(&self) -> bool {
        match self {
            Node::And(v) | Node::Or(v) => v.iter().any(|n| n.needs_text()),
            Node::Not(b) => b.needs_text(),
            Node::Text { field: Field::Any, .. } => true,
            _ => false,
        }
    }

    /// What to mark where it's shown: each a word's start, or a phrase's words.
    pub fn marks(&self) -> Vec<Vec<String>> {
        let mut out = Vec::new();
        self.collect_marks(false, &mut out);
        out
    }
    fn collect_marks(&self, negated: bool, out: &mut Vec<Vec<String>>) {
        match self {
            Node::And(v) | Node::Or(v) => v.iter().for_each(|n| n.collect_marks(negated, out)),
            Node::Not(b) => b.collect_marks(!negated, out),
            Node::Text { field, words, phrase, .. } if !negated && !matches!(field, Field::Filename | Field::MsgId) => {
                let folded = crate::searchhl::words(&words.join(" "));
                if *phrase {
                    out.push(folded);
                } else {
                    out.extend(folded.into_iter().map(|w| vec![w]));
                }
            }
            _ => {}
        }
    }

    /// As an IMAP SEARCH (servers other than Gmail): what IMAP can't say is
    /// left out (ALL), and what's found is checked here.
    pub fn imap(&self) -> String {
        let q = |s: &str| format!("\"{}\"", s.replace(['"', '\\'], " "));
        match self {
            Node::And(v) => v.iter().map(|n| n.imap()).collect::<Vec<_>>().join(" "),
            Node::Or(v) => v.iter().map(|n| n.imap()).reduce(|a, b| format!("OR ({a}) ({b})")).unwrap_or_else(|| "ALL".into()),
            Node::Not(b) => format!("NOT ({})", b.imap()),
            Node::Unread => "UNSEEN".into(),
            Node::Read => "SEEN".into(),
            Node::Flagged(_) => "FLAGGED".into(),
            Node::After(t) => format!("SINCE {}", imap_date(*t)),
            Node::Before(t) => format!("BEFORE {}", imap_date(*t)),
            Node::Text { field, words, phrase, raw } => {
                let what = if *phrase { words.join(" ") } else { words.join(" ") };
                let key = match field {
                    Field::Any => "TEXT",
                    Field::From => "FROM",
                    Field::To | Field::DeliveredTo => "TO",
                    Field::Cc => "CC",
                    Field::Bcc => "BCC",
                    Field::Subject => "SUBJECT",
                    Field::List => "TEXT",
                    Field::Filename => return "ALL".into(),
                    Field::MsgId => return format!("HEADER Message-ID {}", q(raw)),
                };
                if *phrase || words.len() <= 1 {
                    format!("{key} {}", q(&what))
                } else {
                    words.iter().map(|w| format!("{key} {}", q(w))).collect::<Vec<_>>().join(" ")
                }
            }
            _ => "ALL".into(),
        }
    }
}

fn imap_date(t: i64) -> String {
    use chrono::TimeZone;
    chrono::Local.timestamp_opt(t, 0).single().map(|d| d.format("%-d-%b-%Y").to_string()).unwrap_or_else(|| "1-Jan-1970".into())
}

fn folders_sql(p: &str, ids: &[String], args: &mut Vec<Arg>) -> String {
    if ids.is_empty() {
        return "0".into();
    }
    let marks: Vec<String> = ids
        .iter()
        .map(|f| {
            args.push(Arg::Text(f.clone()));
            format!("?#{}", args.len())
        })
        .collect();
    format!("{p}folder IN ({})", marks.join(","))
}

/// The full-text index's words: each a word's start, or a phrase (in order,
/// whole words); limited to one column of it (subject, people), or all.
fn fts_expr(col: &str, words: &[String], phrase: bool) -> String {
    let clean = |w: &str| w.replace('"', "");
    let inner = if phrase {
        vec![format!("\"{}\"", words.iter().map(|w| clean(w)).collect::<Vec<_>>().join(" "))]
    } else {
        words.iter().map(|w| format!("\"{}\"*", clean(w))).collect()
    };
    if col.is_empty() {
        inner.join(" ")
    } else {
        inner.iter().map(|e| format!("{col} : {e}")).collect::<Vec<_>>().join(" AND ")
    }
}

/// Whether `hay` has these words one after another (each whole, any case or accents).
pub fn phrase_in(hay: &str, words: &[String]) -> bool {
    let want = crate::searchhl::words(&words.join(" "));
    if want.is_empty() {
        return true;
    }
    let toks = crate::searchhl::words(hay);
    toks.windows(want.len()).any(|w| w == want.as_slice())
}

/// `?#n` placeholders numbered from `offset` (after the arguments before them).
pub fn number_args(sql: &str, offset: usize) -> String {
    let mut out = String::with_capacity(sql.len());
    let mut rest = sql;
    while let Some(i) = rest.find("?#") {
        out.push_str(&rest[..i]);
        let digits: String = rest[i + 2..].chars().take_while(|c| c.is_ascii_digit()).collect();
        let n: usize = digits.parse().unwrap_or(0);
        out.push_str(&format!("?{}", n + offset));
        rest = &rest[i + 2 + digits.len()..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn reads_gmail_searches() {
        assert!(!has_operators("weekly invoice"));
        assert!(has_operators("from:jane"));
        assert!(has_operators("\"weekly report\""));
        assert_eq!(parse("from:jane"), Node::Text { field: Field::From, words: w(&["jane"]), phrase: false, raw: "jane".into() });
        assert_eq!(
            parse("subject:\"Weekly Report\" -is:read"),
            Node::And(vec![Node::Text { field: Field::Subject, words: w(&["weekly", "report"]), phrase: true, raw: "Weekly Report".into() }, Node::Not(Box::new(Node::Read))])
        );
        match parse("from:(alice OR bob) has:attachment") {
            Node::And(v) => {
                assert!(matches!(&v[0], Node::Or(o) if o.len() == 2 && matches!(&o[1], Node::Text { field: Field::From, .. })));
                assert_eq!(v[1], Node::Attachment);
            }
            x => panic!("{x:?}"),
        }
        assert!(matches!(parse("{invoice receipt}"), Node::Or(v) if v.len() == 2));
        assert!(matches!(parse("is:unread after:2025/01/01"), Node::And(v) if v[0] == Node::Unread && matches!(v[1], Node::After(_))));
        assert!(matches!(parse("newer_than:3d"), Node::After(_)));
        assert_eq!(parse("has:blue-star"), Node::Flagged(Some(4)));
        assert_eq!(parse("in:spam"), Node::In("junk".into()));
        // Unknown operators are words; an open quote takes the rest while typing.
        assert!(matches!(parse("foo:bar"), Node::Text { field: Field::Any, .. }));
        assert!(matches!(parse("subject:\"weekly rep"), Node::Text { field: Field::Subject, phrase: true, .. }));
        assert_eq!(parse("size:5m"), Node::True);
    }

    #[test]
    fn checks_one_message() {
        let m = crate::data::Message {
            subject: "The Weekly Report is in".into(),
            from_name: "Jane Doe".into(),
            from_email: "jane@example.org".into(),
            to: "me@example.org".into(),
            read: false,
            ..Default::default()
        };
        assert!(parse("subject:\"weekly report\"").matches(&m, ""));
        assert!(!parse("subject:\"report weekly\"").matches(&m, ""), "a phrase is in order");
        assert!(parse("from:jane is:unread").matches(&m, ""));
        assert!(!parse("from:jane -is:unread").matches(&m, ""));
        assert!(parse("family").matches(&m, "Bring the family"));
        assert!(!parse("family").matches(&m, "font stuff"));
        assert!(parse("from:\"jane doe\" OR to:nobody").matches(&m, ""));
        assert_eq!(parse("subject:\"weekly report\" invoice -spam").marks(), vec![w(&["weekly", "report"]), w(&["invoice"])]);
    }

    #[test]
    fn as_imap_and_sql() {
        assert_eq!(parse("from:jane subject:\"weekly report\" is:unread").imap(), "FROM \"jane\" SUBJECT \"weekly report\" UNSEEN");
        assert!(parse("a OR b").imap().starts_with("OR (TEXT"));
        let places = Places::default();
        let mut args = Vec::new();
        let sql = parse("from:jane \"weekly report\"").sql("m.", &places, &mut args);
        assert!(sql.contains("people : \"jane\"*") || args.iter().any(|a| matches!(a, Arg::Text(t) if t.contains("people : \"jane\"*"))));
        assert!(args.iter().any(|a| matches!(a, Arg::Text(t) if t == "\"weekly report\"")));
        assert_eq!(number_args("a = ?#1 AND b = ?#12", 3), "a = ?4 AND b = ?15");
    }
}
