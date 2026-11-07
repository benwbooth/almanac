//! Several IMAP commands sent at once ("pipelined") and their answers read
//! back together. Gmail, when it slows an account down, makes every round trip
//! wait about ten seconds however little is asked; sent together, a dozen
//! commands wait once. Used for the regular check for new mail (see
//! `live::quick_check`); everything else goes through the `imap` crate, which
//! sends one command at a time.

use imap_proto::types::{AttributeValue, MailboxDatum, Response, ResponseCode, Status, StatusAttribute};
use std::io::{Read, Write};

pub type Stream = crate::live::Limited;

/// One message's part of a FETCH answer.
#[derive(Debug, Default, Clone)]
pub struct Fetched {
    pub uid: u32,
    /// As the `imap` crate words them ("Seen", "Flagged"…; keywords as sent)
    pub flags: Vec<String>,
    pub internal_date: Option<i64>,
    /// The part asked for (headers, or the message or its start)
    pub header: Option<Vec<u8>>,
    /// The whole message's size (RFC822.SIZE)
    pub size: Option<u32>,
}

/// What came back for one command.
#[derive(Debug, Default, Clone)]
pub struct Reply {
    pub ok: bool,
    pub info: String,
    pub fetches: Vec<Fetched>,
    pub search: Vec<u32>,
    /// LIST: (name, delimiter, attributes as the `imap` crate words them)
    pub lists: Vec<(String, String, Vec<String>)>,
    /// LIST-STATUS: (name, messages, uidnext, uidvalidity, highestmodseq)
    pub statuses: Vec<(String, u32, u32, u32, u64)>,
}

pub struct Pipe {
    tls: Stream,
    buf: Vec<u8>,
    next: u32,
}

/// A flag as the `imap` crate's `{:?}` shows it, so the same checks work on both.
fn flag_name(f: &str) -> String {
    match f {
        "\\Seen" => "Seen".into(),
        "\\Answered" => "Answered".into(),
        "\\Flagged" => "Flagged".into(),
        "\\Deleted" => "Deleted".into(),
        "\\Draft" => "Draft".into(),
        "\\Recent" => "Recent".into(),
        other => format!("Custom({other:?})"),
    }
}

/// "06-Nov-2026 10:00:00 -0800" (the day may start with a space).
fn internal_date(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_str(s.trim(), "%d-%b-%Y %H:%M:%S %z").ok().map(|d| d.timestamp())
}

/// A string as an IMAP quoted string.
pub fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

impl Pipe {
    /// Over an encrypted connection (its greeting not yet read): signed in.
    pub fn sign_in(tls: Stream, auth: &Auth) -> Result<Pipe, String> {
        let mut p = Pipe { tls, buf: Vec::new(), next: 1 };
        p.greeting()?;
        let cmd = match auth {
            Auth::OAuth { user, token } => {
                use base64::Engine;
                let raw = format!("user={user}\x01auth=Bearer {token}\x01\x01");
                format!("AUTHENTICATE XOAUTH2 {}", base64::engine::general_purpose::STANDARD.encode(raw))
            }
            Auth::Password { user, pass } => format!("LOGIN {} {}", quote(user), quote(pass)),
        };
        let r = p.batch(&[cmd])?;
        if !r[0].ok {
            return Err(format!("the server didn't accept the sign-in: {}", r[0].info));
        }
        Ok(p)
    }

    fn greeting(&mut self) -> Result<(), String> {
        loop {
            if let Some(end) = self.buf.windows(2).position(|w| w == b"\r\n") {
                self.buf.drain(..end + 2);
                return Ok(());
            }
            self.fill()?;
        }
    }

    fn fill(&mut self) -> Result<(), String> {
        let mut chunk = [0u8; 65536];
        let n = self.tls.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("the mail server closed the connection".into());
        }
        self.buf.extend_from_slice(&chunk[..n]);
        Ok(())
    }

    /// Sends all the commands at once, then reads every answer.
    pub fn batch(&mut self, cmds: &[String]) -> Result<Vec<Reply>, String> {
        let first = self.next;
        let mut out = String::new();
        for c in cmds {
            out.push_str(&format!("p{} {c}\r\n", self.next));
            self.next += 1;
        }
        // (A continuation request would wait for more: an empty line ends a
        // refused sign-in.)
        self.tls.write_all(out.as_bytes()).map_err(|e| e.to_string())?;
        self.tls.flush().map_err(|e| e.to_string())?;
        let mut replies: Vec<Reply> = vec![Reply::default(); cmds.len()];
        // Untagged answers belong to the first command not yet done.
        let mut current = 0usize;
        let mut pos = 0usize;
        while current < cmds.len() {
            let (used, response) = match imap_proto::parser::parse_response(&self.buf[pos..]) {
                Ok((rest, r)) => (self.buf.len() - pos - rest.len(), Some(r)),
                Err(nom::Err::Incomplete(_)) => {
                    self.buf.drain(..pos);
                    pos = 0;
                    self.fill()?;
                    continue;
                }
                // Something the parser doesn't know: that line is skipped.
                Err(_) => match self.buf[pos..].windows(2).position(|w| w == b"\r\n") {
                    Some(end) => (end + 2, None),
                    None => {
                        self.fill()?;
                        continue;
                    }
                },
            };
            if let Some(r) = response {
                let reply = &mut replies[current];
                match r {
                    Response::Done { tag, status, information, .. } => {
                        let n: u32 = tag.0.trim_start_matches('p').parse().unwrap_or(0);
                        if n >= first {
                            let i = (n - first) as usize;
                            if i < replies.len() {
                                replies[i].ok = status == Status::Ok;
                                replies[i].info = information.map(|s| s.to_string()).unwrap_or_default();
                                current = current.max(i + 1);
                            }
                        }
                    }
                    Response::Continue { .. } => {
                        self.tls.write_all(b"\r\n").map_err(|e| e.to_string())?;
                        self.tls.flush().map_err(|e| e.to_string())?;
                    }
                    Response::Fetch(_, attrs) => {
                        let mut f = Fetched::default();
                        for a in attrs {
                            match a {
                                AttributeValue::Uid(u) => f.uid = u,
                                AttributeValue::Flags(l) => f.flags = l.iter().map(|x| flag_name(x)).collect(),
                                AttributeValue::InternalDate(d) => f.internal_date = internal_date(&d),
                                AttributeValue::BodySection { data, .. } => f.header = data.map(|d| d.to_vec()),
                                AttributeValue::Rfc822Header(data) => f.header = data.map(|d| d.to_vec()),
                                AttributeValue::Rfc822Size(n) => f.size = Some(n),
                                _ => {}
                            }
                        }
                        reply.fetches.push(f);
                    }
                    Response::MailboxData(MailboxDatum::Search(l)) => reply.search.extend(l),
                    Response::MailboxData(MailboxDatum::List { name_attributes, delimiter, name }) => {
                        reply.lists.push((name.to_string(), delimiter.map(|d| d.to_string()).unwrap_or_else(|| "/".into()), name_attributes.iter().map(|x| format!("{x:?}")).collect()));
                    }
                    Response::MailboxData(MailboxDatum::Status { mailbox, status }) => {
                        let (mut m, mut next, mut valid, mut modseq) = (0, 0, 0, 0);
                        for s in status {
                            match s {
                                StatusAttribute::Messages(x) => m = x,
                                StatusAttribute::UidNext(x) => next = x,
                                StatusAttribute::UidValidity(x) => valid = x,
                                StatusAttribute::HighestModSeq(x) => modseq = x,
                                _ => {}
                            }
                        }
                        reply.statuses.push((mailbox.to_string(), m, next, valid, modseq));
                    }
                    Response::Data { code: Some(ResponseCode::HighestModSeq(_)), .. } => {}
                    _ => {}
                }
            }
            pos += used;
        }
        self.buf.drain(..pos);
        Ok(replies)
    }

    pub fn logout(mut self) {
        let _ = self.tls.write_all(format!("p{} LOGOUT\r\n", self.next).as_bytes());
        let _ = self.tls.flush();
    }
}

pub enum Auth {
    OAuth { user: String, token: String },
    Password { user: String, pass: String },
}

#[cfg(test)]
mod tests {
    #[test]
    fn flags_read_as_the_imap_crate_words_them() {
        assert_eq!(super::flag_name("\\Seen"), "Seen");
        assert_eq!(super::flag_name("\\Flagged"), "Flagged");
        assert!(super::flag_name("$MailFlagBit1").contains("$MailFlagBit1"));
        assert_eq!(super::internal_date(" 6-Nov-2026 10:00:00 -0800"), Some(1_793_988_000));
        assert_eq!(super::quote("a\"b\\c"), "\"a\\\"b\\\\c\"");
    }
}
