//! OpenPGP: checking signed mail (PGP/MIME, RFC 3156, and inline), with the
//! person's own GnuPG (their keyring, agent and pinentry, through the `gpg`
//! program, as Enigmail did: nothing to build in, and on Windows / macOS it
//! works wherever GnuPG is installed, Gpg4win or GPG Suite; without it the
//! message just shows no badge).

use std::io::Write;
use std::process::{Command, Stdio};

/// What a message is, as far as OpenPGP goes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// multipart/signed with an application/pgp-signature part.
    Signed,
    /// multipart/encrypted (application/pgp-encrypted).
    Encrypted,
    /// "-----BEGIN PGP SIGNED MESSAGE-----" in the text.
    InlineSigned,
    /// "-----BEGIN PGP MESSAGE-----" in the text.
    InlineEncrypted,
}

/// The outcome of checking a signature.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct Check {
    /// "good", "bad", "expired", "revoked", "unknown-key", "error"
    pub state: String,
    /// Who signed it, as the key says ("Name <address>"); "" if not known.
    pub signer: String,
    /// The key's fingerprint (or its id, when the key isn't here).
    pub key: String,
    /// How far the key is trusted: "ultimate", "full", "marginal", "never", "unknown".
    pub trust: String,
    /// A line of explanation for errors.
    pub detail: String,
}

/// The top-level headers of a raw message (unfolded), and where its body starts.
fn headers(raw: &[u8]) -> (Vec<(String, String)>, usize) {
    let text = String::from_utf8_lossy(raw);
    let end = text.find("\r\n\r\n").map(|i| i + 4).or_else(|| text.find("\n\n").map(|i| i + 2)).unwrap_or(text.len());
    let mut out: Vec<(String, String)> = Vec::new();
    for line in text[..end].lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(last) = out.last_mut() {
                last.1.push(' ');
                last.1.push_str(line.trim());
            }
        } else if let Some((k, v)) = line.split_once(':') {
            out.push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
        }
    }
    (out, end)
}

/// A parameter of a header value ("boundary", "protocol"), unquoted.
fn param(value: &str, name: &str) -> Option<String> {
    for part in value.split(';').skip(1) {
        let (k, v) = part.split_once('=')?;
        if k.trim().eq_ignore_ascii_case(name) {
            return Some(v.trim().trim_matches('"').to_string());
        }
    }
    None
}

/// Whether (and how) a raw message is signed or encrypted.
pub fn kind(raw: &[u8]) -> Option<Kind> {
    let (h, body_at) = headers(raw);
    let ct = h.iter().find(|(k, _)| k == "content-type").map(|(_, v)| v.clone()).unwrap_or_default();
    let low = ct.to_ascii_lowercase();
    if low.starts_with("multipart/signed") && param(&ct, "protocol").is_some_and(|p| p.eq_ignore_ascii_case("application/pgp-signature")) {
        return Some(Kind::Signed);
    }
    if low.starts_with("multipart/encrypted") {
        return Some(Kind::Encrypted);
    }
    let body = String::from_utf8_lossy(&raw[body_at.min(raw.len())..]);
    if body.contains("-----BEGIN PGP SIGNED MESSAGE-----") {
        return Some(Kind::InlineSigned);
    }
    if body.contains("-----BEGIN PGP MESSAGE-----") {
        return Some(Kind::InlineEncrypted);
    }
    None
}

/// Whether a message (as the app keeps it) may well be signed or encrypted:
/// worth fetching its original to check.
pub fn looks_signed(attachments: &[String], body: &str, html: &str) -> bool {
    attachments.iter().any(|a| {
        let a = a.to_ascii_lowercase();
        // ("OpenPGP_signature" is how Thunderbird names it.)
        a == "signature.asc" || a.ends_with(".sig") || a == "encrypted.asc" || a == "openpgp.asc" || a.starts_with("openpgp_signature") || a == "msg.asc"
    }) || body.contains("-----BEGIN PGP") || html.contains("-----BEGIN PGP")
}

/// A multipart/signed message's signed part, exactly as signed (its own
/// headers and body, lines ending CRLF), and its signature.
fn signed_parts(raw: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    let (h, body_at) = headers(raw);
    let ct = h.iter().find(|(k, _)| k == "content-type")?.1.clone();
    let boundary = param(&ct, "boundary")?;
    let body = &raw[body_at..];
    let delim = format!("--{boundary}");
    // Lines, keeping where each starts (the signed part is the bytes between
    // the first and second delimiter lines, without the line break before the second).
    let mut starts = Vec::new();
    let mut i = 0;
    while i <= body.len() {
        starts.push(i);
        match body[i..].iter().position(|&b| b == b'\n') {
            Some(p) => i += p + 1,
            None => break,
        }
    }
    let line_at = |s: usize| -> &[u8] {
        let e = body[s..].iter().position(|&b| b == b'\n').map(|p| s + p).unwrap_or(body.len());
        let l = &body[s..e];
        l.strip_suffix(b"\r").unwrap_or(l)
    };
    let delims: Vec<usize> = starts.iter().copied().filter(|&s| s < body.len() && line_at(s).starts_with(delim.as_bytes())).collect();
    if delims.len() < 2 {
        return None;
    }
    let first_after = |s: usize| body[s..].iter().position(|&b| b == b'\n').map(|p| s + p + 1).unwrap_or(body.len());
    let part1_start = first_after(delims[0]);
    let mut part1_end = delims[1];
    // (The line break before the delimiter belongs to the delimiter.)
    if part1_end > part1_start && body[part1_end - 1] == b'\n' {
        part1_end -= 1;
        if part1_end > part1_start && body[part1_end - 1] == b'\r' {
            part1_end -= 1;
        }
    }
    let signed = crlf(&body[part1_start..part1_end]);
    let part2_start = first_after(delims[1]);
    let part2_end = delims.get(2).copied().unwrap_or(body.len());
    let part2 = &body[part2_start..part2_end];
    // The signature part's body: after its headers.
    let text = String::from_utf8_lossy(part2);
    let sig_at = text.find("-----BEGIN PGP SIGNATURE-----")?;
    let sig_end = text[sig_at..].find("-----END PGP SIGNATURE-----").map(|e| sig_at + e + "-----END PGP SIGNATURE-----".len())?;
    Some((signed, text[sig_at..sig_end].as_bytes().to_vec()))
}

/// Line endings made CRLF (how signed MIME parts are signed).
fn crlf(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(b.len() + b.len() / 40);
    for (i, &c) in b.iter().enumerate() {
        if c == b'\n' && (i == 0 || b[i - 1] != b'\r') {
            out.push(b'\r');
        }
        out.push(c);
    }
    out
}

/// The `gpg` program, if there is one.
fn gpg() -> Option<String> {
    for name in ["gpg", "gpg2"] {
        if Command::new(name).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success()) {
            return Some(name.to_string());
        }
    }
    None
}

/// Reads gpg's status lines (--status-fd) into a Check.
fn from_status(status: &str) -> Check {
    let mut c = Check { trust: "unknown".into(), ..Default::default() };
    for line in status.lines() {
        let Some(rest) = line.strip_prefix("[GNUPG:] ") else { continue };
        let mut w = rest.splitn(3, ' ');
        let tag = w.next().unwrap_or("");
        let a = w.next().unwrap_or("");
        let b = w.next().unwrap_or("");
        match tag {
            "GOODSIG" => {
                c.state = "good".into();
                c.key = a.into();
                c.signer = b.into();
            }
            "BADSIG" => {
                c.state = "bad".into();
                c.key = a.into();
                c.signer = b.into();
            }
            "EXPSIG" | "EXPKEYSIG" => {
                c.state = "expired".into();
                c.key = a.into();
                c.signer = b.into();
            }
            "REVKEYSIG" => {
                c.state = "revoked".into();
                c.key = a.into();
                c.signer = b.into();
            }
            "ERRSIG" => {
                if c.state.is_empty() {
                    c.state = "unknown-key".into();
                    c.key = a.into();
                }
            }
            "NO_PUBKEY" => {
                c.state = "unknown-key".into();
                c.key = a.into();
            }
            "VALIDSIG" => c.key = a.into(),
            "TRUST_ULTIMATE" => c.trust = "ultimate".into(),
            "TRUST_FULLY" => c.trust = "full".into(),
            "TRUST_MARGINAL" => c.trust = "marginal".into(),
            "TRUST_NEVER" => c.trust = "never".into(),
            "TRUST_UNDEFINED" => c.trust = "unknown".into(),
            _ => {}
        }
    }
    if c.state.is_empty() {
        c.state = "error".into();
        c.detail = "No signature could be checked".into();
    }
    c
}

/// Checks a signed message's signature with the person's keyring
/// (GNUPGHOME / the default). None: not a signed message.
pub fn verify(raw: &[u8]) -> Option<Check> {
    let k = kind(raw)?;
    let Some(gpg) = gpg() else {
        return Some(Check { state: "error".into(), detail: "GnuPG isn't installed".into(), trust: "unknown".into(), ..Default::default() });
    };
    let dir = tempdir()?;
    let out = match k {
        Kind::Signed => {
            let (data, sig) = signed_parts(raw)?;
            let (dp, sp) = (dir.join("data"), dir.join("sig.asc"));
            std::fs::write(&dp, data).ok()?;
            std::fs::write(&sp, sig).ok()?;
            Command::new(&gpg).args(["--batch", "--no-tty", "--status-fd", "1", "--verify"]).arg(&sp).arg(&dp).output().ok()?
        }
        Kind::InlineSigned => {
            let (_, body_at) = headers(raw);
            let text = String::from_utf8_lossy(&raw[body_at..]).to_string();
            let start = text.find("-----BEGIN PGP SIGNED MESSAGE-----")?;
            let end = text[start..].find("-----END PGP SIGNATURE-----").map(|e| start + e + "-----END PGP SIGNATURE-----".len())?;
            let mut child = Command::new(&gpg)
                .args(["--batch", "--no-tty", "--status-fd", "1", "--verify"])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .ok()?;
            child.stdin.take()?.write_all(text[start..end].as_bytes()).ok()?;
            child.wait_with_output().ok()?
        }
        Kind::Encrypted | Kind::InlineEncrypted => return None,
    };
    let _ = std::fs::remove_dir_all(&dir);
    Some(from_status(&String::from_utf8_lossy(&out.stdout)))
}

/// An encrypted message, decrypted (kept in memory only, never saved).
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Decrypted {
    /// "ok" or "error"
    pub state: String,
    pub body: String,
    pub html: String,
    pub attachments: Vec<String>,
    /// A signature inside the encrypted part, if there was one.
    pub signature: Option<Check>,
    pub detail: String,
}

/// Whether a message (as the app keeps it) looks encrypted.
pub fn looks_encrypted(attachments: &[String], body: &str, html: &str) -> bool {
    attachments.iter().any(|a| {
        let a = a.to_ascii_lowercase();
        a == "encrypted.asc" || a == "msg.asc" || a == "openpgp-encrypted-message.asc"
    }) || body.contains("-----BEGIN PGP MESSAGE-----") || html.contains("-----BEGIN PGP MESSAGE-----")
}

/// Decrypts an encrypted message with the person's keys (gpg-agent asks for
/// the passphrase with its pinentry). None: not an encrypted message.
pub fn decrypt(raw: &[u8]) -> Option<Decrypted> {
    let k = kind(raw)?;
    if !matches!(k, Kind::Encrypted | Kind::InlineEncrypted) {
        return None;
    }
    let fail = |d: &str| Some(Decrypted { state: "error".into(), detail: d.into(), ..Default::default() });
    let Some(gpg) = gpg() else { return fail("GnuPG isn't installed") };
    let text = String::from_utf8_lossy(raw).to_string();
    let start = text.find("-----BEGIN PGP MESSAGE-----")?;
    let end = text[start..].find("-----END PGP MESSAGE-----").map(|e| start + e + "-----END PGP MESSAGE-----".len())?;
    let mut child = match Command::new(&gpg)
        .args(["--batch", "--no-tty", "--status-fd", "2", "--decrypt"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return fail(&e.to_string()),
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text[start..end].as_bytes());
    }
    let out = match child.wait_with_output() {
        Ok(o) => o,
        Err(e) => return fail(&e.to_string()),
    };
    let status = String::from_utf8_lossy(&out.stderr).to_string();
    if !status.contains("[GNUPG:] DECRYPTION_OKAY") {
        let why = if status.contains("NO_SECKEY") {
            "This message was encrypted for a key you don't have"
        } else if status.contains("BAD_PASSPHRASE") || status.contains("MISSING_PASSPHRASE") {
            "The passphrase wasn't given"
        } else {
            "It couldn't be decrypted"
        };
        return fail(why);
    }
    let signature = (status.contains("GOODSIG") || status.contains("BADSIG") || status.contains("ERRSIG") || status.contains("EXPKEYSIG"))
        .then(|| from_status(&status));
    let plain = out.stdout;
    let mut d = Decrypted { state: "ok".into(), signature, ..Default::default() };
    if k == Kind::Encrypted {
        // The decrypted part is a MIME entity of its own (maybe signed inside).
        let p = crate::mime::parse(&plain);
        d.body = p.body;
        d.html = p.html;
        d.attachments = p.attachments.into_iter().filter(|a| !a.eq_ignore_ascii_case("signature.asc")).collect();
        if d.signature.is_none() && kind(&plain) == Some(Kind::Signed) {
            d.signature = verify(&plain);
        }
    } else {
        // Inline: the text instead of the armored block.
        d.body = format!("{}{}{}", &text[..start].trim_end(), String::from_utf8_lossy(&plain), &text[end..]);
        if let Some(at) = d.body.find("\r\n\r\n").or_else(|| d.body.find("\n\n")) {
            d.body = d.body[at..].trim().to_string();
        }
    }
    Some(d)
}

/// The plain addresses in a To / Cc / Bcc line ("Name <a@b>, c@d").
pub fn addresses(list: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in list.split(',') {
        let p = part.trim();
        let addr = match (p.rfind('<'), p.rfind('>')) {
            (Some(a), Some(b)) if b > a => &p[a + 1..b],
            _ => p,
        };
        let addr = addr.trim().to_ascii_lowercase();
        if addr.contains('@') && !out.contains(&addr) {
            out.push(addr);
        }
    }
    out
}

/// Whether the keyring has a key that can encrypt to this address (not
/// expired, revoked or disabled).
fn has_encryption_key(gpg: &str, addr: &str) -> bool {
    let Ok(out) = Command::new(gpg).args(["--batch", "--with-colons", "--list-keys", &format!("<{addr}>")]).output() else { return false };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut key_ok = false;
    for line in text.lines() {
        let f: Vec<&str> = line.split(':').collect();
        match f.first() {
            // The key as a whole: usable, and able to encrypt (a capital E).
            Some(&"pub") => key_ok = !matches!(f.get(1), Some(&"r") | Some(&"e") | Some(&"d")) && f.get(11).is_some_and(|c| c.contains('E')),
            Some(&"sub") if key_ok && !matches!(f.get(1), Some(&"r") | Some(&"e")) && f.get(11).is_some_and(|c| c.contains('e')) => return true,
            _ => {}
        }
    }
    false
}

/// Whether there's a secret key that can sign as this address.
pub fn can_sign(addr: &str) -> bool {
    let Some(gpg) = gpg() else { return false };
    let Ok(out) = Command::new(&gpg).args(["--batch", "--with-colons", "--list-secret-keys", &format!("<{addr}>")]).output() else { return false };
    String::from_utf8_lossy(&out.stdout).lines().any(|l| {
        let f: Vec<&str> = l.split(':').collect();
        f.first() == Some(&"sec") && !matches!(f.get(1), Some(&"r") | Some(&"e") | Some(&"d")) && f.get(11).is_some_and(|c| c.contains('S'))
    })
}

/// Whether GnuPG is installed.
pub fn available() -> bool {
    gpg().is_some()
}

/// Of these addresses, the ones without a key to encrypt to (all of them
/// when GnuPG isn't installed).
pub fn missing_keys(addrs: &[String]) -> Vec<String> {
    let Some(gpg) = gpg() else { return addrs.to_vec() };
    addrs.iter().filter(|a| !has_encryption_key(&gpg, a)).cloned().collect()
}

fn run_gpg(args: &[&str], input: &[u8]) -> Result<Vec<u8>, String> {
    let gpg = gpg().ok_or("GnuPG isn't installed")?;
    let mut child = Command::new(&gpg)
        .args(["--batch", "--no-tty"])
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    child.stdin.take().ok_or("gpg")?.write_all(input).map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let line = err.lines().rfind(|l| l.starts_with("gpg:")).unwrap_or("gpg failed").trim_start_matches("gpg: ").to_string();
        return Err(line);
    }
    Ok(out.stdout)
}

/// A detached, armored signature over `data` with the key for `signer`
/// (gpg-agent asks for its passphrase).
pub fn sign_detached(data: &[u8], signer: &str) -> Result<String, String> {
    let out = run_gpg(&["--armor", "--detach-sign", "--digest-algo", "SHA256", "--local-user", &format!("<{signer}>")], data)?;
    Ok(String::from_utf8_lossy(&out).to_string())
}

/// `data` encrypted (armored) to each recipient and to the sender too (so
/// it can be read in Sent), signed as well with `sign_as`.
pub fn encrypt_to(data: &[u8], recipients: &[String], me: &str, sign_as: Option<&str>) -> Result<String, String> {
    let mut args: Vec<String> = vec!["--armor".into(), "--trust-model".into(), "always".into(), "--encrypt".into()];
    if let Some(s) = sign_as {
        args.extend(["--sign".into(), "--digest-algo".into(), "SHA256".into(), "--local-user".into(), format!("<{s}>")]);
    }
    for r in recipients.iter().chain(std::iter::once(&me.to_ascii_lowercase())) {
        args.push("--recipient".into());
        args.push(format!("<{r}>"));
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = run_gpg(&refs, data)?;
    Ok(String::from_utf8_lossy(&out).to_string())
}

/// Your own key for an address, if there is one: its fingerprint.
pub fn own_key(addr: &str) -> Option<String> {
    let gpg = gpg()?;
    let out = Command::new(&gpg).args(["--batch", "--with-colons", "--list-secret-keys", &format!("<{addr}>")]).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let mut usable = false;
    for line in text.lines() {
        let f: Vec<&str> = line.split(':').collect();
        match f.first() {
            Some(&"sec") => usable = !matches!(f.get(1), Some(&"r") | Some(&"e") | Some(&"d")),
            Some(&"fpr") if usable => return f.get(9).map(|x| x.to_string()),
            _ => {}
        }
    }
    None
}

/// A new key for `name <addr>` (ed25519 for signing, cv25519 for encryption,
/// for two years); gpg-agent's pinentry asks for its passphrase.
pub fn create_key(name: &str, addr: &str) -> Result<String, String> {
    let uid = if name.trim().is_empty() { format!("<{addr}>") } else { format!("{} <{addr}>", name.trim()) };
    run_gpg(&["--quick-generate-key", &uid, "ed25519", "sign,cert", "2y"], b"")?;
    let fpr = own_key(addr).ok_or("The key was made but can't be found")?;
    run_gpg(&["--quick-add-key", &fpr, "cv25519", "encrypt", "2y"], b"")?;
    Ok(fpr)
}

/// Keys from a file (yours, or other people's): what gpg says it imported.
pub fn import_keys(path: &std::path::Path) -> Result<String, String> {
    let gpg = gpg().ok_or("GnuPG isn't installed")?;
    let out = Command::new(&gpg).args(["--batch", "--status-fd", "1", "--import"]).arg(path).output().map_err(|e| e.to_string())?;
    let status = String::from_utf8_lossy(&out.stdout);
    let n = status.lines().filter(|l| l.starts_with("[GNUPG:] IMPORT_OK")).count();
    if n == 0 {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(err.lines().rfind(|l| l.starts_with("gpg:")).unwrap_or("No keys found in that file").trim_start_matches("gpg: ").to_string());
    }
    Ok(format!("{n} key(s) imported"))
}

/// Your public key uploaded to keys.openpgp.org (they email the address to
/// confirm before showing it). Only ever done when asked.
pub fn publish(fingerprint: &str) -> Result<(), String> {
    run_gpg(&["--keyserver", "hkps://keys.openpgp.org", "--send-keys", fingerprint], b"").map(|_| ())
}

/// Someone's key looked up (their mail provider's Web Key Directory, then
/// keys.openpgp.org) and added to the keyring. Only when asked.
pub fn look_up(addr: &str) -> Result<String, String> {
    run_gpg(&["--auto-key-locate", "clear,wkd,keyserver", "--keyserver", "hkps://keys.openpgp.org", "--locate-external-keys", addr], b"")?;
    Ok(format!("Found a key for {addr}"))
}

/// A private scratch folder for one check (removed after).
fn tempdir() -> Option<std::path::PathBuf> {
    let d = std::env::temp_dir().join(format!("almanac-pgp-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos()));
    std::fs::create_dir_all(&d).ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o700));
    }
    Some(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (The tests set GNUPGHOME for the whole process: one at a time.)
    static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// A throwaway keyring with one new key (no passphrase), for the tests.
    fn test_home() -> Option<std::path::PathBuf> {
        gpg()?;
        let home = tempdir()?;
        let ok = Command::new("gpg")
            .env("GNUPGHOME", &home)
            .args(["--batch", "--passphrase", "", "--quick-gen-key", "Test Signer <signer@example.org>", "ed25519", "sign", "never"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .ok()?
            .success();
        ok.then_some(home)
    }

    fn sign(home: &std::path::Path, data: &[u8]) -> String {
        let mut child = Command::new("gpg")
            .env("GNUPGHOME", home)
            .args(["--batch", "--armor", "--detach-sign", "--digest-algo", "SHA256"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(data).unwrap();
        String::from_utf8(child.wait_with_output().unwrap().stdout).unwrap()
    }

    fn signed_message(home: &std::path::Path, text: &str) -> Vec<u8> {
        let part = format!("Content-Type: text/plain; charset=utf-8\r\n\r\n{text}\r\n");
        let sig = sign(home, part.as_bytes());
        format!(
            "From: Test Signer <signer@example.org>\r\nTo: me@example.com\r\nSubject: Signed\r\nMIME-Version: 1.0\r\n\
             Content-Type: multipart/signed; micalg=pgp-sha256; protocol=\"application/pgp-signature\"; boundary=\"XYZ\"\r\n\r\n\
             --XYZ\r\n{part}\r\n--XYZ\r\nContent-Type: application/pgp-signature; name=\"signature.asc\"\r\n\r\n{sig}\r\n--XYZ--\r\n"
        )
        .into_bytes()
    }

    #[test]
    fn a_pgp_mime_encrypted_message_is_decrypted() {
        let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let Some(home) = test_home() else { return };
        // An encryption subkey for the test key, then a message encrypted to it.
        let fpr = String::from_utf8(
            Command::new("gpg").env("GNUPGHOME", &home).args(["--batch", "--with-colons", "--list-keys", "signer@example.org"]).output().unwrap().stdout,
        )
        .unwrap()
        .lines()
        .find(|l| l.starts_with("fpr:"))
        .map(|l| l.split(':').nth(9).unwrap().to_string())
        .unwrap();
        assert!(Command::new("gpg").env("GNUPGHOME", &home).args(["--batch", "--passphrase", "", "--quick-add-key", &fpr, "cv25519", "encrypt", "never"]).status().unwrap().success());
        let inner = "Content-Type: text/plain; charset=utf-8\r\n\r\nThe secret plan.\r\n";
        let mut child = Command::new("gpg")
            .env("GNUPGHOME", &home)
            .args(["--batch", "--armor", "--trust-model", "always", "--encrypt", "-r", "signer@example.org"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(inner.as_bytes()).unwrap();
        let armored = String::from_utf8(child.wait_with_output().unwrap().stdout).unwrap();
        let raw = format!(
            "From: a@example.org\r\nSubject: Secret\r\nMIME-Version: 1.0\r\n\
             Content-Type: multipart/encrypted; protocol=\"application/pgp-encrypted\"; boundary=\"B\"\r\n\r\n\
             --B\r\nContent-Type: application/pgp-encrypted\r\n\r\nVersion: 1\r\n\r\n\
             --B\r\nContent-Type: application/octet-stream; name=\"encrypted.asc\"\r\n\r\n{armored}\r\n--B--\r\n"
        );
        std::env::set_var("GNUPGHOME", &home);
        let d = decrypt(raw.as_bytes()).expect("an encrypted message");
        assert_eq!(d.state, "ok", "{d:?}");
        assert!(d.body.contains("The secret plan."), "{d:?}");
        // Someone else's keyring: can't.
        let other = test_home().unwrap();
        std::env::set_var("GNUPGHOME", &other);
        let d = decrypt(raw.as_bytes()).unwrap();
        assert_eq!(d.state, "error");
        std::env::remove_var("GNUPGHOME");
        let _ = std::fs::remove_dir_all(home);
        let _ = std::fs::remove_dir_all(other);
    }

    /// An encryption subkey for the test key.
    fn add_encryption(home: &std::path::Path) {
        let fpr = String::from_utf8(
            Command::new("gpg").env("GNUPGHOME", home).args(["--batch", "--with-colons", "--list-keys", "signer@example.org"]).output().unwrap().stdout,
        )
        .unwrap()
        .lines()
        .find(|l| l.starts_with("fpr:"))
        .map(|l| l.split(':').nth(9).unwrap().to_string())
        .unwrap();
        assert!(Command::new("gpg").env("GNUPGHOME", home).args(["--batch", "--passphrase", "", "--quick-add-key", &fpr, "cv25519", "encrypt", "never"]).status().unwrap().success());
    }

    #[test]
    fn mail_sent_signed_or_encrypted_reads_back() {
        let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let Some(home) = test_home() else { return };
        add_encryption(&home);
        std::env::set_var("GNUPGHOME", &home);
        let a = crate::live::LiveAccount { email: "signer@example.org".into(), name: "Test Signer".into(), ..Default::default() };
        let base = crate::live::OutMsg { to: "signer@example.org".into(), subject: "Plan".into(), body: "Meet at noon.".into(), html: "<p>Meet at <b>noon</b>.</p>".into(), ..Default::default() };
        // Signed: checks out.
        let signed = crate::live::build_message(&a, &crate::live::OutMsg { sign: true, ..base.clone() }).expect("signed");
        let c = verify(&signed.formatted()).expect("a signed message");
        assert_eq!(c.state, "good", "{c:?}");
        // Encrypted: reads back.
        let enc = crate::live::build_message(&a, &crate::live::OutMsg { encrypt: true, ..base.clone() }).expect("encrypted");
        let raw = enc.formatted();
        assert!(!String::from_utf8_lossy(&raw).contains("Meet at noon"), "nothing readable");
        let d = decrypt(&raw).expect("an encrypted message");
        assert_eq!(d.state, "ok", "{d:?}");
        assert!(d.body.contains("Meet at noon."), "{d:?}");
        // Both: decrypted, and the signature inside checks out.
        let both = crate::live::build_message(&a, &crate::live::OutMsg { sign: true, encrypt: true, ..base.clone() }).expect("both");
        let d = decrypt(&both.formatted()).unwrap();
        assert_eq!(d.signature.map(|s| s.state).as_deref(), Some("good"));
        // Someone without a key: not sent.
        let err = crate::live::build_message(&a, &crate::live::OutMsg { encrypt: true, to: "nokey@example.net".into(), ..base }).err().unwrap();
        assert!(err.contains("nokey@example.net"), "{err}");
        std::env::remove_var("GNUPGHOME");
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn a_key_is_made_for_an_address() {
        let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        if gpg().is_none() {
            return;
        }
        let home = tempdir().unwrap();
        // (No passphrase in the test: the agent's pinentry asks in the app.)
        std::fs::write(home.join("gpg.conf"), "pinentry-mode loopback\npassphrase \n").unwrap();
        std::fs::write(home.join("gpg-agent.conf"), "allow-loopback-pinentry\n").unwrap();
        std::env::set_var("GNUPGHOME", &home);
        assert_eq!(own_key("new@example.org"), None);
        let fpr = create_key("New Person", "new@example.org").expect("made");
        assert_eq!(own_key("new@example.org").as_deref(), Some(fpr.as_str()));
        assert!(can_sign("new@example.org"));
        assert!(missing_keys(&["new@example.org".into()]).is_empty(), "can encrypt to it too");
        std::env::remove_var("GNUPGHOME");
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn kinds_are_told_apart() {
        let signed = b"Content-Type: multipart/signed; protocol=\"application/pgp-signature\"; boundary=x\r\n\r\n--x\r\n";
        assert_eq!(kind(signed), Some(Kind::Signed));
        assert_eq!(kind(b"Content-Type: multipart/encrypted; protocol=\"application/pgp-encrypted\"; boundary=y\r\n\r\n"), Some(Kind::Encrypted));
        assert_eq!(kind(b"Subject: x\r\n\r\n-----BEGIN PGP SIGNED MESSAGE-----\r\n"), Some(Kind::InlineSigned));
        assert_eq!(kind(b"Subject: x\r\n\r\nHello\r\n"), None);
        assert!(looks_signed(&["signature.asc".into()], "", ""));
        assert!(looks_signed(&["OpenPGP_signature".into()], "", ""), "Thunderbird's name");
        assert!(!looks_signed(&["photo.jpg".into()], "Hello", ""));
    }

    #[test]
    fn a_pgp_mime_signature_is_checked() {
        let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let Some(home) = test_home() else { return }; // (no GnuPG here: nothing to test)
        std::env::set_var("GNUPGHOME", &home);
        let raw = signed_message(&home, "Meet at noon.");
        let c = verify(&raw).expect("a signed message");
        assert_eq!(c.state, "good", "{c:?}");
        assert!(c.signer.contains("signer@example.org"), "{c:?}");
        assert_eq!(c.trust, "ultimate", "own key: {c:?}");
        // Changed after signing: bad.
        let tampered = String::from_utf8(raw.clone()).unwrap().replace("Meet at noon.", "Meet at one.").into_bytes();
        assert_eq!(verify(&tampered).unwrap().state, "bad");
        // Lines ending LF only (as some stores keep mail): still good.
        let lf = String::from_utf8(raw).unwrap().replace("\r\n", "\n").into_bytes();
        assert_eq!(verify(&lf).unwrap().state, "good");
        // Someone else's key: unknown.
        let other = test_home().unwrap();
        std::env::set_var("GNUPGHOME", &other);
        assert_eq!(verify(&signed_message(&home, "Hi")).unwrap().state, "unknown-key");
        std::env::remove_var("GNUPGHOME");
        let _ = std::fs::remove_dir_all(home);
        let _ = std::fs::remove_dir_all(other);
    }
}
