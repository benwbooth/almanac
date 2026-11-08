//! S/MIME: signed and encrypted mail with X.509 certificates (what Apple
//! Mail and Outlook use), through the `openssl` command (`cms`).
//!
//! - Signatures are checked against the system's trusted certificate
//!   authorities (the CA bundle the system keeps, from p11-kit/NSS's list);
//!   a good signature whose certificate doesn't lead to one shows as
//!   "not verified". Senders' certificates are kept, to encrypt to them later.
//! - Your own certificate comes from a .p12 / .pfx file you import (as issued
//!   by a certificate authority); its key is kept in Almanac's folder,
//!   encrypted with that file's password, which Almanac asks for to sign or
//!   to read encrypted mail (and remembers for 10 minutes).
//! - Passwords go to openssl through a private file deleted at once, never on
//!   its command line.

use crate::pgp::{Check, Decrypted};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// What kind of S/MIME a message is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    /// multipart/signed with an application/pkcs7-signature part.
    Signed,
    /// application/pkcs7-mime, signed-data: the text inside the signature.
    Opaque,
    /// application/pkcs7-mime, enveloped-data (or authEnveloped).
    Encrypted,
}

pub fn kind(raw: &[u8]) -> Option<Kind> {
    let head_end = find(raw, b"\r\n\r\n").or_else(|| find(raw, b"\n\n")).unwrap_or(raw.len().min(8192));
    let head = String::from_utf8_lossy(&raw[..head_end]).to_lowercase().replace("\r\n ", " ").replace("\r\n\t", " ").replace("\n ", " ");
    let ct = head.lines().find(|l| l.starts_with("content-type:")).unwrap_or("").to_string();
    if ct.contains("multipart/signed") && (ct.contains("pkcs7-signature")) {
        return Some(Kind::Signed);
    }
    if ct.contains("pkcs7-mime") {
        if ct.contains("signed-data") {
            return Some(Kind::Opaque);
        }
        return Some(Kind::Encrypted);
    }
    None
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// From what's stored about a message (its attachments' names): signed or
/// encrypted with S/MIME?
pub fn looks_signed(attachments: &[String]) -> bool {
    attachments.iter().any(|a| {
        let a = a.to_lowercase();
        a == "smime.p7s" || a.ends_with(".p7s")
    })
}

pub fn looks_encrypted(attachments: &[String]) -> bool {
    attachments.iter().any(|a| {
        let a = a.to_lowercase();
        a == "smime.p7m" || a.ends_with(".p7m")
    })
}

pub fn available() -> bool {
    Command::new("openssl").arg("version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

/// The system's trusted certificate authorities.
fn ca_bundle() -> Option<PathBuf> {
    for p in ["/etc/ssl/certs/ca-certificates.crt", "/etc/ssl/certs/ca-bundle.crt", "/etc/pki/tls/certs/ca-bundle.crt", "/etc/ssl/cert.pem"] {
        if Path::new(p).exists() {
            return Some(PathBuf::from(p));
        }
    }
    std::env::var_os("SSL_CERT_FILE").map(PathBuf::from).filter(|p| p.exists())
}

/// Almanac's S/MIME folder: `mine` (your certificates and keys) and
/// `others` (certificates of people who signed mail to you).
fn store_dir(which: &str) -> PathBuf {
    let d = crate::data::data_dir().join("smime").join(which);
    let _ = std::fs::create_dir_all(&d);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(crate::data::data_dir().join("smime"), std::fs::Permissions::from_mode(0o700));
    }
    d
}

fn safe(addr: &str) -> String {
    addr.trim().to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() || "@.-_+".contains(c) { c } else { '_' }).collect()
}

/// A private scratch folder (removed when dropped).
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Option<Scratch> {
        let d = std::env::temp_dir().join(format!(
            "almanac-smime-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos()
        ));
        std::fs::create_dir_all(&d).ok()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o700));
        }
        Some(Scratch(d))
    }
    fn file(&self, name: &str, bytes: &[u8]) -> Option<PathBuf> {
        let p = self.0.join(name);
        let mut f = std::fs::OpenOptions::new().create(true).write(true).truncate(true).open(&p).ok()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
        }
        f.write_all(bytes).ok()?;
        Some(p)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// openssl with `args`; (succeeded, stdout, stderr).
fn openssl(args: &[&str]) -> Result<(bool, Vec<u8>, String), String> {
    let out = Command::new("openssl").args(args).stdin(Stdio::null()).output().map_err(|e| format!("openssl isn't installed ({e})"))?;
    Ok((out.status.success(), out.stdout, String::from_utf8_lossy(&out.stderr).to_string()))
}

fn path(p: &Path) -> &str {
    p.to_str().unwrap_or("")
}

/// A certificate's addresses (subject alternative names, else the subject's
/// emailAddress), lower case.
fn cert_emails(cert: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok((true, o, _)) = openssl(&["x509", "-in", path(cert), "-noout", "-email"]) {
        out.extend(String::from_utf8_lossy(&o).lines().map(|l| l.trim().to_lowercase()).filter(|l| l.contains('@')));
    }
    if out.is_empty() {
        if let Ok((true, o, _)) = openssl(&["x509", "-in", path(cert), "-noout", "-subject", "-nameopt", "multiline"]) {
            for l in String::from_utf8_lossy(&o).lines() {
                if let Some(v) = l.trim().strip_prefix("emailAddress") {
                    out.push(v.trim_start_matches([' ', '=']).trim().to_lowercase());
                }
            }
        }
    }
    out
}

/// A certificate's subject (its common name and address), fingerprint and end date.
fn cert_info(cert: &Path) -> (String, String, String) {
    let field = |args: &[&str]| openssl(args).ok().filter(|r| r.0).map(|r| String::from_utf8_lossy(&r.1).trim().to_string()).unwrap_or_default();
    let subject = field(&["x509", "-in", path(cert), "-noout", "-subject", "-nameopt", "oneline"]);
    let fpr = field(&["x509", "-in", path(cert), "-noout", "-fingerprint", "-sha256"]);
    // (As a date: 2027-11-08.)
    let end = field(&["x509", "-in", path(cert), "-noout", "-enddate", "-dateopt", "iso_8601"]);
    let end = end.trim_start_matches("notAfter=").split(' ').next().unwrap_or("").to_string();
    let cn = subject.trim_start_matches("subject=").split(',').find_map(|p| p.trim().strip_prefix("CN = ")).unwrap_or("").to_string();
    (cn, fpr.rsplit('=').next().unwrap_or("").replace(':', ""), end)
}

/// Checks an S/MIME signature (multipart/signed or opaque). None: not S/MIME signed.
pub fn verify(raw: &[u8]) -> Option<Check> {
    kind(raw).filter(|k| *k != Kind::Encrypted)?;
    let err = |d: &str| Some(Check { state: "error".into(), trust: "unknown".into(), detail: d.into(), ..Default::default() });
    let Some(dir) = Scratch::new() else { return err("No room for a scratch file") };
    let Some(msg) = dir.file("msg.eml", raw) else { return err("No room for a scratch file") };
    let signer = dir.0.join("signer.pem");
    let base = ["cms", "-verify", "-inform", "SMIME", "-in", path(&msg), "-out", "/dev/null", "-signer", path(&signer)];
    // Against the system's certificate authorities first; then, to tell a
    // signature that's wrong from one only not traced to a trusted authority,
    // without that.
    let mut trusted = false;
    let mut ok = false;
    let mut expired = false;
    if let Some(ca) = ca_bundle() {
        let mut a = base.to_vec();
        a.extend(["-CAfile", path(&ca), "-purpose", "smimesign"]);
        match openssl(&a) {
            Ok((true, _, _)) => {
                trusted = true;
                ok = true;
            }
            Ok((false, _, e)) => expired = e.contains("expired"),
            Err(_) => {}
        }
    }
    let mut detail = String::new();
    if !ok {
        let mut a = base.to_vec();
        a.push("-noverify");
        match openssl(&a) {
            Ok((true, _, _)) => {
                ok = true;
                detail = "The signer's certificate isn't from an authority this system trusts".into();
            }
            Ok((false, _, e)) => detail = e.lines().find(|l| !l.trim().is_empty()).unwrap_or("").to_string(),
            Err(e) => return err(&e),
        }
    }
    let (cn, fpr, _) = if signer.exists() { cert_info(&signer) } else { (String::new(), String::new(), String::new()) };
    let issuer = if signer.exists() {
        openssl(&["x509", "-in", path(&signer), "-noout", "-issuer", "-nameopt", "oneline"])
            .ok()
            .filter(|r| r.0)
            .map(|r| String::from_utf8_lossy(&r.1).trim().trim_start_matches("issuer=").split(',').find_map(|p| p.trim().strip_prefix("CN = ").map(str::to_string)).unwrap_or_default())
            .unwrap_or_default()
    } else {
        String::new()
    };
    if !issuer.is_empty() {
        detail = if detail.is_empty() { format!("Certificate issued by {issuer}") } else { format!("{detail}\nCertificate issued by {issuer}") };
    }
    let emails = if signer.exists() { cert_emails(&signer) } else { Vec::new() };
    // Kept, to encrypt to this person later.
    if ok && signer.exists() {
        for e in &emails {
            let _ = std::fs::copy(&signer, store_dir("others").join(format!("{}.pem", safe(e))));
        }
    }
    let who = match (cn.is_empty(), emails.first()) {
        (false, Some(e)) => format!("{cn} <{e}>"),
        (true, Some(e)) => e.clone(),
        _ => cn,
    };
    Some(Check {
        state: if ok && expired { "expired".into() } else if ok { "good".into() } else { "bad".into() },
        signer: who,
        key: fpr.chars().rev().take(16).collect::<String>().chars().rev().collect(),
        trust: if trusted { "full".into() } else { "unknown".into() },
        detail,
    })
}

/// Other people's certificates (from their signed mail): [(address, name, fingerprint, ends)].
pub fn others_certificates() -> Vec<(String, String, String, String)> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(store_dir("others")) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "pem") {
                let addr = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                let (cn, fpr, end) = cert_info(&p);
                out.push((addr, cn, fpr, end));
            }
        }
    }
    out.sort();
    out
}

pub fn remove_other(addr: &str) {
    let _ = std::fs::remove_file(store_dir("others").join(format!("{}.pem", safe(addr))));
}

/// Your certificates: [(address, name, fingerprint, ends)].
pub fn own_certificates() -> Vec<(String, String, String, String)> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(store_dir("mine")) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "crt") {
                let addr = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                let (cn, fpr, end) = cert_info(&p);
                out.push((addr, cn, fpr, end));
            }
        }
    }
    out.sort();
    out
}

fn own(addr: &str) -> Option<(PathBuf, PathBuf, PathBuf)> {
    let d = store_dir("mine");
    let a = safe(addr);
    let (crt, key, chain) = (d.join(format!("{a}.crt")), d.join(format!("{a}.key")), d.join(format!("{a}.chain.pem")));
    (crt.exists() && key.exists()).then_some((crt, key, chain))
}

pub fn can_sign(addr: &str) -> bool {
    own(addr).is_some()
}

/// The certificate to encrypt to someone (theirs from a signed message, or yours).
fn cert_for(addr: &str) -> Option<PathBuf> {
    let a = safe(addr);
    let theirs = store_dir("others").join(format!("{a}.pem"));
    if theirs.exists() {
        return Some(theirs);
    }
    own(addr).map(|(crt, _, _)| crt)
}

/// Of these addresses, those without a certificate to encrypt to.
pub fn missing_certificates(addrs: &[String]) -> Vec<String> {
    addrs.iter().filter(|a| cert_for(a).is_none()).cloned().collect()
}

/// Your certificate and key from a .p12 / .pfx file (its password kept as
/// the key's). Returns the address(es) it's for.
pub fn import_p12(file: &Path, password: &str) -> Result<Vec<String>, String> {
    let dir = Scratch::new().ok_or("No room for a scratch file")?;
    let pass = dir.file("pass", password.as_bytes()).ok_or("No room for a scratch file")?;
    let passin = format!("file:{}", path(&pass));
    let (crt, key, chain) = (dir.0.join("c.pem"), dir.0.join("k.pem"), dir.0.join("chain.pem"));
    // (Files from older tools use ciphers OpenSSL 3 keeps in its "legacy" set.)
    let run = |extra: &[&str], legacy: bool| -> Result<(bool, Vec<u8>, String), String> {
        let mut a: Vec<&str> = vec!["pkcs12", "-in", path(file), "-passin", &passin];
        if legacy {
            a.push("-legacy");
        }
        a.extend_from_slice(extra);
        openssl(&a)
    };
    let mut legacy = false;
    let (ok, _, e) = run(&["-nokeys", "-clcerts", "-out", path(&crt)], false)?;
    if !ok {
        let (ok2, _, e2) = run(&["-nokeys", "-clcerts", "-out", path(&crt)], true)?;
        if !ok2 {
            let why = format!("{e}{e2}");
            return Err(if why.contains("mac verify") || why.contains("password") { "That password isn't right for this file".into() } else { "It couldn't be read as a certificate file (.p12 / .pfx)".into() });
        }
        legacy = true;
    }
    // (A file of its own: given the same one, openssl reads its second line for this.)
    let pass_out = dir.file("pass_out", password.as_bytes()).ok_or("No room for a scratch file")?;
    let passout = format!("file:{}", path(&pass_out));
    let (ok, _, e) = run(&["-nocerts", "-out", path(&key), "-passout", &passout], legacy)?;
    if !ok {
        return Err(format!("The key couldn't be read: {}", e.lines().next().unwrap_or("")));
    }
    let _ = run(&["-nokeys", "-cacerts", "-out", path(&chain)], legacy);
    let emails = cert_emails(&crt);
    if emails.is_empty() {
        return Err("This certificate isn't for an email address".into());
    }
    let mine = store_dir("mine");
    for e in &emails {
        // (Its password in the system keychain: no asking to sign or read.)
        let _ = crate::live::save_password(&format!("smime:{}", safe(e)), password);
        let a = safe(e);
        std::fs::copy(&crt, mine.join(format!("{a}.crt"))).map_err(|e| e.to_string())?;
        std::fs::copy(&key, mine.join(format!("{a}.key"))).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(mine.join(format!("{a}.key")), std::fs::Permissions::from_mode(0o600));
        }
        if chain.exists() && std::fs::metadata(&chain).map(|m| m.len() > 0).unwrap_or(false) {
            let _ = std::fs::copy(&chain, mine.join(format!("{a}.chain.pem")));
        }
    }
    Ok(emails)
}

/// Removes your certificate for an address.
pub fn remove_own(addr: &str) {
    let d = store_dir("mine");
    let a = safe(addr);
    crate::live::forget_password(&format!("smime:{a}"));
    for ext in ["crt", "key", "chain.pem"] {
        let _ = std::fs::remove_file(d.join(format!("{a}.{ext}")));
    }
}

// ---- Passwords, remembered a while once given.

static REMEMBERED: std::sync::Mutex<Option<std::collections::HashMap<String, (String, std::time::Instant)>>> = std::sync::Mutex::new(None);

fn remembered(addr: &str) -> Option<String> {
    let mut m = REMEMBERED.lock().ok()?;
    let map = m.get_or_insert_with(Default::default);
    match map.get(&safe(addr)) {
        Some((p, at)) if at.elapsed() < std::time::Duration::from_secs(600) => Some(p.clone()),
        _ => None,
    }
}

fn remember(addr: &str, pass: &str) {
    if let Ok(mut m) = REMEMBERED.lock() {
        m.get_or_insert_with(Default::default).insert(safe(addr), (pass.to_string(), std::time::Instant::now()));
    }
}

/// Whether the key for `addr` can be used now without asking (its password remembered).
pub fn unlocked(addr: &str) -> bool {
    remembered(addr).is_some()
}

/// Checks the password for `addr`'s key and remembers it (10 minutes).
pub fn unlock(addr: &str, password: &str) -> Result<(), String> {
    let (_, key, _) = own(addr).ok_or("No certificate of yours for that address")?;
    let dir = Scratch::new().ok_or("No room for a scratch file")?;
    let pass = dir.file("pass", password.as_bytes()).ok_or("No room for a scratch file")?;
    let (ok, _, _) = openssl(&["pkey", "-in", path(&key), "-passin", &format!("file:{}", path(&pass)), "-noout"])?;
    if !ok {
        return Err(crate::pgp::BAD_PASSPHRASE.into());
    }
    remember(addr, password);
    Ok(())
}

/// The password for `addr`'s key: given now (checked and remembered), or remembered.
fn password_for(addr: &str, given: Option<&str>) -> Result<String, String> {
    match given {
        Some(p) => {
            unlock(addr, p)?;
            Ok(p.to_string())
        }
        None => remembered(addr)
            .or_else(|| crate::live::password(&format!("smime:{}", safe(addr))).filter(|p| !p.is_empty()))
            .ok_or_else(|| crate::pgp::NEED_PASSPHRASE.to_string()),
    }
}

/// A detached signature (DER) over `data` with `signer`'s certificate.
pub fn sign_detached(data: &[u8], signer: &str) -> Result<Vec<u8>, String> {
    let (crt, key, chain) = own(signer).ok_or("No certificate of yours to sign with")?;
    let password = password_for(signer, None)?;
    let dir = Scratch::new().ok_or("No room for a scratch file")?;
    let input = dir.file("in", data).ok_or("No room for a scratch file")?;
    let pass = dir.file("pass", password.as_bytes()).ok_or("No room for a scratch file")?;
    let passin = format!("file:{}", path(&pass));
    let mut a = vec!["cms", "-sign", "-binary", "-md", "sha256", "-in", path(&input), "-signer", path(&crt), "-inkey", path(&key), "-passin", &passin, "-outform", "DER"];
    if chain.exists() {
        a.extend(["-certfile", path(&chain)]);
    }
    let (ok, out, e) = openssl(&a)?;
    if !ok {
        return Err(format!("Couldn't sign: {}", e.lines().next().unwrap_or("")));
    }
    Ok(out)
}

/// `data` encrypted (DER, enveloped-data) to each recipient and to `me`.
pub fn encrypt_to(data: &[u8], recipients: &[String], me: &str) -> Result<Vec<u8>, String> {
    let mut certs = Vec::new();
    for r in recipients.iter().map(String::as_str).chain(std::iter::once(me)) {
        certs.push(cert_for(r).ok_or_else(|| format!("No certificate to encrypt to {r}"))?);
    }
    let dir = Scratch::new().ok_or("No room for a scratch file")?;
    let input = dir.file("in", data).ok_or("No room for a scratch file")?;
    let mut a: Vec<String> = ["cms", "-encrypt", "-binary", "-aes-256-cbc", "-in", path(&input), "-outform", "DER"].iter().map(|s| s.to_string()).collect();
    a.extend(certs.iter().map(|c| path(c).to_string()));
    let refs: Vec<&str> = a.iter().map(String::as_str).collect();
    let (ok, out, e) = openssl(&refs)?;
    if !ok {
        return Err(format!("Couldn't encrypt: {}", e.lines().next().unwrap_or("")));
    }
    Ok(out)
}

/// Decrypts an S/MIME encrypted message with your key (`passphrase`, or one
/// remembered; state "locked" when needed). None: not S/MIME encrypted.
pub fn decrypt(raw: &[u8], passphrase: Option<&str>) -> Option<Decrypted> {
    if kind(raw)? != Kind::Encrypted {
        return None;
    }
    let fail = |d: &str| Some(Decrypted { state: "error".into(), detail: d.into(), ..Default::default() });
    // Which of your certificates it's for: the first that has one (tried in turn).
    let mine = own_certificates();
    if mine.is_empty() {
        return fail("No certificate of yours to read it with (Settings › Privacy › S/MIME)");
    }
    let Some(dir) = Scratch::new() else { return fail("No room for a scratch file") };
    let Some(msg) = dir.file("msg.eml", raw) else { return fail("No room for a scratch file") };
    let mut locked = false;
    for (addr, _, _, _) in &mine {
        let Some((crt, key, _)) = own(addr) else { continue };
        let password = match password_for(addr, passphrase) {
            Ok(p) => p,
            Err(e) => {
                if e == crate::pgp::BAD_PASSPHRASE && mine.len() == 1 {
                    return Some(Decrypted { state: "locked".into(), detail: e, ..Default::default() });
                }
                locked = true;
                continue;
            }
        };
        let Some(pass) = dir.file("pass", password.as_bytes()) else { return fail("No room for a scratch file") };
        let passin = format!("file:{}", path(&pass));
        let r = openssl(&["cms", "-decrypt", "-inform", "SMIME", "-in", path(&msg), "-recip", path(&crt), "-inkey", path(&key), "-passin", &passin]);
        let _ = std::fs::remove_file(&pass);
        if let Ok((true, plain, _)) = r {
            let p = crate::mime::parse(&plain);
            let mut d = Decrypted { state: "ok".into(), body: p.body, html: p.html, ..Default::default() };
            d.attachments = p.attachments.into_iter().filter(|a| !a.eq_ignore_ascii_case("smime.p7s")).collect();
            if kind(&plain).is_some_and(|k| k != Kind::Encrypted) {
                d.signature = verify(&plain);
            }
            return Some(d);
        }
    }
    if locked {
        return Some(Decrypted { state: "locked".into(), detail: crate::pgp::NEED_PASSPHRASE.into(), ..Default::default() });
    }
    fail("This message was encrypted for a certificate you don't have")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (The tests share the data folder: one at a time.)
    static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// A throwaway certificate authority and a certificate for `email` from it,
    /// as a .p12 with `password`. Returns (p12, ca.pem).
    fn make_p12(dir: &Path, email: &str, password: &str) -> (PathBuf, PathBuf) {
        let p = |n: &str| dir.join(n);
        let run = |a: &[&str]| assert!(Command::new("openssl").args(a).stderr(Stdio::null()).status().unwrap().success(), "{a:?}");
        run(&["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", path(&p("ca.key")), "-out", path(&p("ca.pem")), "-days", "2", "-subj", "/CN=Test CA"]);
        run(&["req", "-newkey", "rsa:2048", "-nodes", "-keyout", path(&p("me.key")), "-out", path(&p("me.csr")), "-subj", &format!("/CN=Test Person/emailAddress={email}")]);
        std::fs::write(p("ext"), format!("subjectAltName=email:{email}\nkeyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage=emailProtection\n")).unwrap();
        run(&["x509", "-req", "-in", path(&p("me.csr")), "-CA", path(&p("ca.pem")), "-CAkey", path(&p("ca.key")), "-CAcreateserial", "-out", path(&p("me.pem")), "-days", "2", "-extfile", path(&p("ext"))]);
        run(&["pkcs12", "-export", "-inkey", path(&p("me.key")), "-in", path(&p("me.pem")), "-certfile", path(&p("ca.pem")), "-out", path(&p("me.p12")), "-passout", &format!("pass:{password}")]);
        (p("me.p12"), p("ca.pem"))
    }

    #[test]
    fn signed_and_encrypted_mail_reads_back() {
        let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        if !available() {
            return;
        }
        let dir = Scratch::new().unwrap();
        let (p12, _) = make_p12(&dir.0, "me@example.org", "secret pw");
        assert_eq!(import_p12(&p12, "wrong").unwrap_err(), "That password isn't right for this file");
        assert_eq!(import_p12(&p12, "secret pw").unwrap(), vec!["me@example.org".to_string()]);
        assert!(can_sign("me@example.org"));
        // Signing: the password comes from the keychain; without it there, asked for once.
        crate::live::forget_password("smime:me@example.org");
        // (Signed without the line break before the next boundary: it's the boundary's.)
        let part = b"Content-Type: text/plain\r\n\r\nHello there";
        assert_eq!(sign_detached(part, "me@example.org").unwrap_err(), crate::pgp::NEED_PASSPHRASE);
        assert_eq!(unlock("me@example.org", "nope").unwrap_err(), crate::pgp::BAD_PASSPHRASE);
        unlock("me@example.org", "secret pw").unwrap();
        let sig = sign_detached(part, "me@example.org").expect("signed");
        // As a message: multipart/signed, the part and the signature.
        use base64::Engine;
        // (Wrapped at 76, as mail has it: openssl's reader wants lines.)
        let wrap = |b: &[u8]| base64::engine::general_purpose::STANDARD.encode(b).as_bytes().chunks(76).map(|c| String::from_utf8_lossy(c).to_string()).collect::<Vec<_>>().join("\r\n");
        let b64 = wrap(&sig);
        let raw = format!(
            "Content-Type: multipart/signed; protocol=\"application/pkcs7-signature\"; micalg=sha-256; boundary=\"almanac-b1\"\r\n\r\n--almanac-b1\r\n{}\r\n--almanac-b1\r\nContent-Type: application/pkcs7-signature; name=smime.p7s\r\nContent-Transfer-Encoding: base64\r\n\r\n{b64}\r\n--almanac-b1--\r\n",
            String::from_utf8_lossy(part)
        );
        let c = verify(raw.as_bytes()).expect("checked");
        assert_eq!(c.state, "good", "{c:?}");
        assert_eq!(c.trust, "unknown", "a throwaway authority isn't trusted by the system");
        assert!(c.signer.contains("me@example.org"), "{c:?}");
        // Changed after signing: bad.
        let c = verify(raw.replace("Hello there", "Hello thera").as_bytes()).unwrap();
        assert_eq!(c.state, "bad");
        // Encrypted to me: locked until the password's given (forgotten here), then read.
        let enc = encrypt_to(part, &[], "me@example.org").expect("encrypted");
        let raw = format!(
            "Content-Type: application/pkcs7-mime; smime-type=enveloped-data; name=smime.p7m\r\nContent-Transfer-Encoding: base64\r\n\r\n{}\r\n",
            wrap(&enc)
        );
        assert_eq!(kind(raw.as_bytes()), Some(Kind::Encrypted));
        let d = decrypt(raw.as_bytes(), None).unwrap();
        assert_eq!(d.state, "ok", "remembered from unlocking: {d:?}");
        assert!(d.body.contains("Hello there"));
        REMEMBERED.lock().unwrap().take();
        let d = decrypt(raw.as_bytes(), None).unwrap();
        assert_eq!(d.state, "locked");
        let d = decrypt(raw.as_bytes(), Some("secret pw")).unwrap();
        assert_eq!(d.state, "ok");
        remove_own("me@example.org");
        assert!(!can_sign("me@example.org"));
    }

    /// Mail sent signed, encrypted or both with S/MIME (the real message
    /// builder) checks out and reads back.
    #[test]
    fn mail_sent_with_smime_reads_back() {
        let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        if !available() {
            return;
        }
        let dir = Scratch::new().unwrap();
        let (p12, _) = make_p12(&dir.0, "signer@example.org", "pw2");
        import_p12(&p12, "pw2").expect("imported");
        let a = crate::live::LiveAccount { email: "signer@example.org".into(), name: "Test Signer".into(), ..Default::default() };
        let base = crate::live::OutMsg { to: "signer@example.org".into(), subject: "Plan".into(), body: "Meet at noon.".into(), html: "<p>Meet at <b>noon</b>.</p>".into(), smime: true, ..Default::default() };
        // Signed (the key's password from the keychain: nothing asked).
        let signed = crate::live::build_message(&a, &crate::live::OutMsg { sign: true, ..base.clone() }).expect("signed");
        let raw = signed.formatted();
        assert_eq!(kind(&raw), Some(Kind::Signed));
        let c = verify(&raw).expect("checked");
        assert_eq!(c.state, "good", "{c:?}");
        assert!(c.detail.contains("Test CA"), "the issuer is said: {c:?}");
        // Encrypted: nothing readable, and reads back.
        let enc = crate::live::build_message(&a, &crate::live::OutMsg { encrypt: true, ..base.clone() }).expect("encrypted");
        let raw = enc.formatted();
        assert!(!String::from_utf8_lossy(&raw).contains("Meet at noon"));
        let d = decrypt(&raw, None).expect("encrypted");
        assert_eq!(d.state, "ok", "{d:?}");
        assert!(d.body.contains("Meet at noon."), "{d:?}");
        // Both: the signature inside checks out.
        let both = crate::live::build_message(&a, &crate::live::OutMsg { sign: true, encrypt: true, ..base.clone() }).expect("both");
        let d = decrypt(&both.formatted(), None).unwrap();
        assert_eq!(d.signature.map(|s| s.state).as_deref(), Some("good"));
        // Someone without a certificate: not sent.
        let err = crate::live::build_message(&a, &crate::live::OutMsg { encrypt: true, to: "nocert@example.net".into(), ..base }).err().unwrap();
        assert!(err.contains("nocert@example.net"), "{err}");
        remove_own("signer@example.org");
    }

    #[test]
    fn kinds_are_told_apart() {
        assert_eq!(kind(b"Content-Type: multipart/signed; protocol=\"application/pkcs7-signature\"; boundary=x\r\n\r\n"), Some(Kind::Signed));
        assert_eq!(kind(b"Content-Type: application/pkcs7-mime; smime-type=signed-data\r\n\r\n"), Some(Kind::Opaque));
        assert_eq!(kind(b"Content-Type: application/x-pkcs7-mime; smime-type=enveloped-data\r\n\r\n"), Some(Kind::Encrypted));
        assert_eq!(kind(b"Content-Type: multipart/signed; protocol=\"application/pgp-signature\"\r\n\r\n"), None);
        assert!(looks_signed(&["smime.p7s".into()]) && looks_encrypted(&["smime.p7m".into()]));
    }
}
