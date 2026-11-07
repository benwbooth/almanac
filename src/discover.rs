//! Finding the mail servers for an address whose provider Almanac doesn't
//! know (custom domains, smaller providers), the way Thunderbird does:
//! 1. the provider's own published settings (autoconfig on its domain),
//! 2. Mozilla's shared database of providers (ISPDB),
//! 3. where the domain's mail goes (its MX record): Google Workspace,
//!    Microsoft 365, iCloud, Fastmail, Proton…, then that host's ISPDB entry.
//! Only the domain is ever sent, never the address.

use crate::live::LiveAccount;

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(std::time::Duration::from_secs(8)))
        .build()
        .into()
}

fn get(url: &str) -> Option<String> {
    let mut r = agent().get(url).call().ok()?;
    if !r.status().is_success() {
        return None;
    }
    r.body_mut().read_to_string().ok()
}

/// Servers from an autoconfig file (config-v1.1.xml): IMAP and SMTP.
pub fn parse_autoconfig(xml: &str, email: &str) -> Option<LiveAccount> {
    let local = email.split('@').next().unwrap_or("");
    let domain = email.rsplit('@').next().unwrap_or("");
    let fill = |s: &str| s.replace("%EMAILADDRESS%", email).replace("%EMAILLOCALPART%", local).replace("%EMAILDOMAIN%", domain);
    let block = |tag: &str, kind: &str| -> Option<(String, u16, String, String)> {
        let re = regex::Regex::new(&format!(r#"(?s)<{tag}\s+type="{kind}">(.*?)</{tag}>"#)).ok()?;
        let inner = re.captures(xml)?.get(1)?.as_str().to_string();
        let field = |f: &str| {
            regex::Regex::new(&format!(r"(?s)<{f}>\s*(.*?)\s*</{f}>")).ok().and_then(|r| r.captures(&inner)).map(|c| c[1].to_string()).unwrap_or_default()
        };
        let port: u16 = field("port").parse().ok()?;
        Some((fill(&field("hostname")), port, field("socketType"), fill(&field("username"))))
    };
    let (incoming, (imap_host, imap_port, _, user)) = match block("incomingServer", "imap") {
        Some(b) => (String::new(), b),
        None => ("pop".to_string(), block("incomingServer", "pop3")?),
    };
    let (smtp_host, smtp_port, _, _) = block("outgoingServer", "smtp")?;
    if imap_host.is_empty() || smtp_host.is_empty() {
        return None;
    }
    Some(LiveAccount {
        email: email.to_string(),
        name: email.to_string(),
        username: if user.is_empty() { email.to_string() } else { user },
        imap_host,
        imap_port,
        smtp_host,
        smtp_port,
        incoming,
        ..Default::default()
    })
}

/// The domain's mail hosts (MX records), most preferred first, asked over
/// DNS-over-HTTPS (only the domain is sent).
fn mx_hosts(domain: &str) -> Vec<String> {
    let Some(body) = get(&format!("https://dns.google/resolve?name={domain}&type=MX")) else { return vec![] };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) else { return vec![] };
    let mut hosts: Vec<(u32, String)> = v["Answer"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|r| r["data"].as_str())
                .filter_map(|d| {
                    let mut it = d.split_whitespace();
                    Some((it.next()?.parse().ok()?, it.next()?.trim_end_matches('.').to_lowercase()))
                })
                .collect()
        })
        .unwrap_or_default();
    hosts.sort();
    hosts.into_iter().map(|(_, h)| h).collect()
}

/// A provider recognised by where its mail goes: the address of one of its
/// own users, whose settings Almanac knows.
pub fn provider_for_mx(mx: &str) -> Option<&'static str> {
    let ends = |s: &str| mx == s || mx.ends_with(&format!(".{s}"));
    if ends("google.com") || ends("googlemail.com") {
        Some("gmail.com")
    } else if ends("outlook.com") || ends("office365.us") {
        Some("outlook.com")
    } else if ends("icloud.com") || ends("me.com") {
        Some("icloud.com")
    } else if ends("messagingengine.com") || ends("fastmail.com") {
        Some("fastmail.com")
    } else if ends("protonmail.ch") || ends("proton.me") {
        Some("proton.me")
    } else if ends("zoho.com") || ends("zoho.eu") {
        Some("zoho.com")
    } else if ends("yahoodns.net") {
        Some("yahoo.com")
    } else {
        None
    }
}

/// The servers for `email`, looked up on the network (call off the window's thread).
pub fn discover(email: &str) -> Option<(LiveAccount, String)> {
    let domain = email.rsplit('@').next()?.trim().to_lowercase();
    if domain.is_empty() || !domain.contains('.') {
        return None;
    }
    for (url, how) in [
        (format!("https://autoconfig.{domain}/mail/config-v1.1.xml"), "the provider's settings"),
        (format!("https://{domain}/.well-known/autoconfig/mail/config-v1.1.xml"), "the provider's settings"),
        (format!("https://autoconfig.thunderbird.net/v1.1/{domain}"), "Mozilla's provider list"),
    ] {
        if let Some(a) = get(&url).and_then(|x| parse_autoconfig(&x, email)) {
            return Some((a, how.to_string()));
        }
    }
    for mx in mx_hosts(&domain) {
        if let Some(provider) = provider_for_mx(&mx) {
            let mut a = crate::live::preset(&format!("x@{provider}"));
            a.email = email.to_string();
            a.name = email.to_string();
            a.username = email.to_string();
            return Some((a, format!("the domain's mail host ({mx})")));
        }
        // The mail host's own provider in Mozilla's list (e.g. a hosting company).
        let base: Vec<&str> = mx.rsplitn(3, '.').collect();
        if base.len() >= 2 {
            let host_domain = format!("{}.{}", base[1], base[0]);
            if let Some(a) = get(&format!("https://autoconfig.thunderbird.net/v1.1/{host_domain}")).and_then(|x| parse_autoconfig(&x, email)) {
                return Some((a, format!("the domain's mail host ({mx})")));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_an_autoconfig_file() {
        let xml = r#"<clientConfig version="1.1"><emailProvider id="example.org">
            <incomingServer type="pop3"><hostname>pop.example.org</hostname><port>995</port></incomingServer>
            <incomingServer type="imap"><hostname>imap.example.org</hostname><port>993</port>
              <socketType>SSL</socketType><username>%EMAILLOCALPART%</username></incomingServer>
            <outgoingServer type="smtp"><hostname>smtp.example.org</hostname><port>587</port>
              <socketType>STARTTLS</socketType><username>%EMAILADDRESS%</username></outgoingServer>
        </emailProvider></clientConfig>"#;
        let a = super::parse_autoconfig(xml, "ann@example.org").unwrap();
        assert_eq!((a.imap_host.as_str(), a.imap_port), ("imap.example.org", 993));
        assert_eq!((a.smtp_host.as_str(), a.smtp_port), ("smtp.example.org", 587));
        assert_eq!(a.username, "ann");
        assert!(!a.is_pop(), "IMAP when there is one");
        let pop_only = xml.replace(r#"type="imap""#, r#"type="other""#);
        let p = super::parse_autoconfig(&pop_only, "ann@example.org").unwrap();
        assert!(p.is_pop() && p.imap_host == "pop.example.org" && p.imap_port == 995);
    }

    #[test]
    fn knows_big_providers_by_their_mail_hosts() {
        assert_eq!(super::provider_for_mx("aspmx.l.google.com"), Some("gmail.com"));
        assert_eq!(super::provider_for_mx("example-org.mail.protection.outlook.com"), Some("outlook.com"));
        assert_eq!(super::provider_for_mx("mail.protonmail.ch"), Some("proton.me"));
        assert_eq!(super::provider_for_mx("mx.example.org"), None);
    }
}
