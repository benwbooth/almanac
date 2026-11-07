//! POP3 accounts (as Apple Mail and Thunderbird offer them): new mail is
//! downloaded whole into the account's Inbox on this computer. POP has no
//! folders or flags on the server, so reading, flagging, moving and deleting
//! stay on this computer; messages are left on the server.

use crate::data::{Account, Folder, Imported, MailDelta, Message};
use crate::live::LiveAccount;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;

trait Stream: Read + Write + Send {}
impl<T: Read + Write + Send> Stream for T {}

pub struct Pop {
    r: BufReader<Box<dyn Stream>>,
}

fn tls(host: &str, tcp: TcpStream) -> Result<Box<dyn Stream>, String> {
    let mut roots = rustls::RootCertStore::empty();
    for c in rustls_native_certs::load_native_certs().certs {
        let _ = roots.add(c);
    }
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .with_root_certificates(roots)
        .with_no_client_auth();
    let name = rustls::pki_types::ServerName::try_from(host.to_string()).map_err(|e| e.to_string())?;
    let conn = rustls::ClientConnection::new(Arc::new(config), name).map_err(|e| e.to_string())?;
    Ok(Box::new(rustls::StreamOwned::new(conn, tcp)))
}

impl Pop {
    /// Connects (encrypted from the start on 995, upgraded with STLS otherwise) and signs in.
    pub fn connect(host: &str, port: u16, user: &str, pass: &str) -> Result<Pop, String> {
        let tcp = TcpStream::connect((host, port)).map_err(|e| format!("Could not reach {host}: {e}"))?;
        let _ = tcp.set_read_timeout(Some(std::time::Duration::from_secs(90)));
        let mut pop = if port == 995 {
            let mut p = Pop { r: BufReader::new(tls(host, tcp)?) };
            p.reply()?;
            p
        } else {
            let mut plain = Pop { r: BufReader::new(Box::new(tcp.try_clone().map_err(|e| e.to_string())?)) };
            plain.reply()?;
            plain.command("STLS").map_err(|_| format!("{host} doesn't offer an encrypted connection on port {port}; try port 995"))?;
            Pop { r: BufReader::new(tls(host, tcp)?) }
        };
        pop.command(&format!("USER {user}"))?;
        pop.command(&format!("PASS {pass}")).map_err(|e| format!("{host} did not accept the sign-in: {e}"))?;
        Ok(pop)
    }

    fn reply(&mut self) -> Result<String, String> {
        let mut line = String::new();
        self.r.read_line(&mut line).map_err(|e| e.to_string())?;
        if line.starts_with("+OK") {
            Ok(line)
        } else {
            Err(line.trim().trim_start_matches("-ERR").trim().to_string())
        }
    }

    fn command(&mut self, cmd: &str) -> Result<String, String> {
        self.r.get_mut().write_all(format!("{cmd}\r\n").as_bytes()).map_err(|e| e.to_string())?;
        self.reply()
    }

    /// The lines after a "+OK" up to the lone ".", with dot-stuffing undone.
    fn multiline(&mut self) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        loop {
            let mut line = Vec::new();
            if self.r.read_until(b'\n', &mut line).map_err(|e| e.to_string())? == 0 {
                return Err("connection closed".into());
            }
            if line == b".\r\n" || line == b".\n" {
                return Ok(out);
            }
            out.extend_from_slice(if line.starts_with(b"..") { &line[1..] } else { &line });
        }
    }

    /// Each message's number and its lasting id on the server.
    pub fn uidl(&mut self) -> Result<Vec<(u32, String)>, String> {
        self.command("UIDL")?;
        let text = String::from_utf8_lossy(&self.multiline()?).to_string();
        Ok(text
            .lines()
            .filter_map(|l| {
                let mut it = l.split_whitespace();
                Some((it.next()?.parse().ok()?, it.next()?.to_string()))
            })
            .collect())
    }

    pub fn retr(&mut self, n: u32) -> Result<Vec<u8>, String> {
        self.command(&format!("RETR {n}"))?;
        self.multiline()
    }

    pub fn quit(mut self) {
        let _ = self.command("QUIT");
    }
}

/// A POP message's lasting id as a number (Almanac keeps server ids as numbers).
pub fn uid_of(uidl: &str) -> u32 {
    let mut h: u32 = 0x811c9dc5;
    for b in uidl.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x01000193);
    }
    (h & 0x7fff_ffff).max(1)
}

/// Downloads new mail into the account's Inbox (the newest `limit`, or all).
pub fn sync(a: &LiveAccount, imp: &mut Imported, known: &HashMap<String, Vec<u32>>, all: bool, progress: &dyn Fn(String)) -> Result<(), String> {
    let account = format!("{}:mail", a.source());
    let folder_id = format!("{account}/INBOX");
    imp.accounts.push(Account { id: account.clone(), name: a.name.clone(), email: a.email.clone(), source: a.source() });
    imp.folders.push(Folder { id: folder_id.clone(), account, name: "Inbox".into(), kind: "inbox".into(), path: "INBOX".into() });
    let pass = crate::live::need_password(a)?;
    progress(format!("{}: checking for new mail…", a.name));
    let mut pop = Pop::connect(&a.imap_host, a.imap_port, &a.username, &pass)?;
    let list = pop.uidl()?;
    let here: std::collections::HashSet<u32> = known.get(&folder_id).map(|v| v.iter().copied().collect()).unwrap_or_default();
    let mut new: Vec<(u32, String)> = list.into_iter().filter(|(_, u)| !here.contains(&uid_of(u))).collect();
    // Newest first (the highest numbers), so recent mail shows up soonest.
    new.sort_by(|x, y| y.0.cmp(&x.0));
    if !all {
        new.truncate(400);
    }
    let dir = crate::data::data_dir().join("pop").join(a.id.replace(['/', '\\', ':'], "_"));
    let _ = std::fs::create_dir_all(&dir);
    let mut delta = MailDelta::default();
    let total = new.len();
    for (i, (n, uidl)) in new.into_iter().enumerate() {
        if total > 20 && i % 20 == 0 {
            progress(crate::live::counted(&format!("{}: downloading messages", a.name), i, total));
        }
        let raw = pop.retr(n)?;
        let uid = uid_of(&uidl);
        // The original, kept as a file (for attachments and "show original").
        let file = dir.join(format!("{uid}.eml"));
        let _ = std::fs::write(&file, &raw);
        let p = crate::mime::parse(&raw);
        delta.new.push(Message {
            folder: folder_id.clone(),
            source: a.source(),
            uid,
            from_name: p.from_name,
            from_email: p.from_email,
            to: p.to,
            cc: p.cc,
            subject: p.subject,
            date: p.date,
            body: p.body,
            html: p.html,
            invite: p.invite,
            attachments: p.attachments,
            message_id: p.message_id,
            in_reply_to: p.in_reply_to,
            references: p.references,
            reply_to: p.reply_to,
            raw_ref: format!("mbox:{}:0:{}", file.display(), raw.len()),
            ..Default::default()
        });
    }
    pop.quit();
    imp.mail_delta = Some(delta);
    Ok(())
}

/// Checks that the server takes the sign-in.
pub fn test(a: &LiveAccount) -> Result<(), String> {
    let pass = crate::live::need_password(a)?;
    Pop::connect(&a.imap_host, a.imap_port, &a.username, &pass)?.quit();
    Ok(())
}

#[cfg(test)]
mod tests {
    /// A pretend server: answers from a script, keeps what's sent to it.
    struct Script {
        input: std::io::Cursor<Vec<u8>>,
        sent: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    }
    impl std::io::Read for Script {
        fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
            self.input.read(b)
        }
    }
    impl std::io::Write for Script {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.sent.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn lists_and_downloads_messages() {
        let answers = "+OK 2 messages\r\n1 AAA111\r\n2 BBB222\r\n.\r\n+OK 40 octets\r\nSubject: Hi\r\n\r\n..a line starting with a dot\r\nend\r\n.\r\n";
        let sent = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut pop = super::Pop { r: std::io::BufReader::new(Box::new(Script { input: std::io::Cursor::new(answers.as_bytes().to_vec()), sent: sent.clone() })) };
        assert_eq!(pop.uidl().unwrap(), vec![(1, "AAA111".to_string()), (2, "BBB222".to_string())]);
        let raw = String::from_utf8(pop.retr(2).unwrap()).unwrap();
        assert_eq!(raw, "Subject: Hi\r\n\r\n.a line starting with a dot\r\nend\r\n");
        assert_eq!(String::from_utf8(sent.lock().unwrap().clone()).unwrap(), "UIDL\r\nRETR 2\r\n");
    }

    #[test]
    fn server_ids_become_steady_numbers() {
        assert_eq!(super::uid_of("000001a2b3c4"), super::uid_of("000001a2b3c4"));
        assert_ne!(super::uid_of("a"), super::uid_of("b"));
        assert!(super::uid_of("") >= 1);
    }
}
