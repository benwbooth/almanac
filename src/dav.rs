//! CalDAV (calendars) and CardDAV (contacts) over HTTPS: finding a user's
//! calendars and address books, reading their items, and writing changes
//! back with change tags (ETags) so edits made elsewhere are not lost.

use base64::Engine;
use quick_xml::events::Event as X;
use std::collections::HashMap;
use std::time::Duration;

pub struct Dav {
    agent: ureq::Agent,
    auth: String,
}

#[derive(Debug, Default, Clone)]
pub struct Resp {
    pub href: String,
    /// Property local name → text (nested hrefs are flattened into the text)
    pub props: HashMap<String, String>,
    pub resourcetypes: Vec<String>,
    pub components: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Collection {
    pub url: String,
    pub name: String,
    pub color: String,
}

pub struct Item {
    pub href: String,
    pub etag: String,
    pub data: String,
}

fn entity(name: &str) -> String {
    match name {
        "amp" => "&".into(),
        "lt" => "<".into(),
        "gt" => ">".into(),
        "quot" => "\"".into(),
        "apos" => "'".into(),
        n if n.starts_with("#x") || n.starts_with("#X") => u32::from_str_radix(&n[2..], 16)
            .ok()
            .and_then(char::from_u32)
            .map(String::from)
            .unwrap_or_default(),
        n if n.starts_with('#') => n[1..].parse::<u32>().ok().and_then(char::from_u32).map(String::from).unwrap_or_default(),
        _ => String::new(),
    }
}

/// Reads a WebDAV multistatus document.
pub fn parse_multistatus(xml: &str) -> Vec<Resp> {
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut out = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut cur: Option<Resp> = None;
    let mut ok_props = true;
    let mut pending: HashMap<String, String> = HashMap::new();
    let mut pending_types: Vec<String> = Vec::new();
    let mut pending_comps: Vec<String> = Vec::new();
    loop {
        let ev = match reader.read_event() {
            Ok(X::Eof) | Err(_) => break,
            Ok(e) => e,
        };
        match ev {
            X::Start(e) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if name == "response" {
                    cur = Some(Resp::default());
                }
                if name == "propstat" {
                    ok_props = true;
                    pending.clear();
                    pending_types.clear();
                    pending_comps.clear();
                }
                if stack.last().map(|s| s == "resourcetype").unwrap_or(false) {
                    pending_types.push(name.clone());
                }
                stack.push(name);
            }
            X::Empty(e) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if stack.last().map(|s| s == "resourcetype").unwrap_or(false) {
                    pending_types.push(name.clone());
                }
                if name == "comp" {
                    if let Ok(Some(a)) = e.try_get_attribute("name") {
                        pending_comps.push(String::from_utf8_lossy(&a.value).to_uppercase());
                    }
                }
            }
            X::End(e) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                stack.pop();
                if name == "propstat" {
                    if ok_props {
                        if let Some(r) = cur.as_mut() {
                            r.props.extend(pending.drain());
                            r.resourcetypes.append(&mut pending_types);
                            r.components.append(&mut pending_comps);
                        }
                    }
                }
                if name == "response" {
                    if let Some(r) = cur.take() {
                        out.push(r);
                    }
                }
            }
            X::Text(_) | X::CData(_) | X::GeneralRef(_) => {
                let text = match &ev {
                    X::Text(t) => t.decode().map(|c| c.to_string()).unwrap_or_default(),
                    X::CData(t) => String::from_utf8_lossy(t.as_ref()).to_string(),
                    X::GeneralRef(r) => entity(&r.decode().map(|c| c.to_string()).unwrap_or_default()),
                    _ => String::new(),
                };
                let Some(r) = cur.as_mut() else { continue };
                let parent = stack.last().cloned().unwrap_or_default();
                if parent == "href" && stack.len() >= 2 && stack[stack.len() - 2] == "response" {
                    r.href.push_str(&text);
                } else if parent == "status" && stack.iter().any(|s| s == "propstat") {
                    ok_props = text.contains(" 200") || text.contains(" 2");
                } else if let Some(pos) = stack.iter().position(|s| s == "prop") {
                    if let Some(prop) = stack.get(pos + 1) {
                        pending.entry(prop.clone()).or_default().push_str(&text);
                    }
                }
            }
            _ => {}
        }
    }
    for r in out.iter_mut() {
        r.href = r.href.trim().to_string();
        for v in r.props.values_mut() {
            *v = v.trim_matches(|c: char| c == ' ' || c == '\n' || c == '\t').to_string();
        }
    }
    out
}

/// Joins a server path or absolute URL onto a base URL.
pub fn join(base: &str, href: &str) -> String {
    let href = href.trim();
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }
    let scheme_end = base.find("://").map(|i| i + 3).unwrap_or(0);
    let host_end = base[scheme_end..].find('/').map(|i| i + scheme_end).unwrap_or(base.len());
    if href.starts_with('/') {
        format!("{}{}", &base[..host_end], href)
    } else {
        let dir = base.rfind('/').filter(|i| *i >= host_end).map(|i| &base[..=i]).unwrap_or(base);
        format!("{}{}{}", dir, if dir.ends_with('/') { "" } else { "/" }, href)
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

impl Dav {
    pub fn new(user: &str, password: &str) -> Dav {
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .allow_non_standard_methods(true)
            .max_redirects(5)
            .redirect_auth_headers(ureq::config::RedirectAuthHeaders::SameHost)
            .timeout_global(Some(Duration::from_secs(90)))
            .build();
        let token = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}"));
        Dav { agent: ureq::Agent::new_with_config(config), auth: format!("Basic {token}") }
    }

    fn request(&self, method: &str, url: &str, depth: Option<&str>, body: &str, headers: &[(&str, &str)]) -> Result<(u16, String, HashMap<String, String>), String> {
        let mut b = ureq::http::Request::builder()
            .method(method)
            .uri(url)
            .header("Authorization", &self.auth)
            .header("User-Agent", "Almanac/0.1");
        if let Some(d) = depth {
            b = b.header("Depth", d);
        }
        if !body.is_empty() && !headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("content-type")) {
            b = b.header("Content-Type", "application/xml; charset=utf-8");
        }
        for (k, v) in headers {
            b = b.header(*k, *v);
        }
        let req = b.body(body.to_string()).map_err(|e| e.to_string())?;
        let mut resp = self.agent.run(req).map_err(|e| format!("{url}: {e}"))?;
        let status = resp.status().as_u16();
        let mut hdrs = HashMap::new();
        for (k, v) in resp.headers() {
            hdrs.insert(k.as_str().to_lowercase(), v.to_str().unwrap_or("").to_string());
        }
        let text = resp.body_mut().read_to_string().unwrap_or_default();
        if status == 401 || status == 403 {
            return Err("The server did not accept the user name or password".into());
        }
        Ok((status, text, hdrs))
    }

    fn propfind(&self, url: &str, depth: &str, props: &str) -> Result<Vec<Resp>, String> {
        let body = format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
             <d:propfind xmlns:d=\"DAV:\" xmlns:c=\"urn:ietf:params:xml:ns:caldav\" \
             xmlns:card=\"urn:ietf:params:xml:ns:carddav\" xmlns:cs=\"http://calendarserver.org/ns/\" \
             xmlns:ic=\"http://apple.com/ns/ical/\"><d:prop>{props}</d:prop></d:propfind>"
        );
        let (status, text, _) = self.request("PROPFIND", url, Some(depth), &body, &[])?;
        if status >= 400 {
            return Err(format!("{url}: HTTP {status}"));
        }
        Ok(parse_multistatus(&text))
    }

    /// Finds the home collection for calendars ("caldav") or contacts ("carddav").
    fn home(&self, base: &str, kind: &str) -> Result<String, String> {
        let home_prop = if kind == "caldav" { "calendar-home-set" } else { "addressbook-home-set" };
        let home_xml = if kind == "caldav" { "<c:calendar-home-set/>" } else { "<card:addressbook-home-set/>" };
        let mut candidates = vec![base.to_string()];
        candidates.push(join(base, &format!("/.well-known/{kind}")));
        for url in candidates {
            let Ok(resps) = self.propfind(&url, "0", &format!("<d:current-user-principal/>{home_xml}")) else { continue };
            if let Some(h) = resps.iter().find_map(|r| r.props.get(home_prop).filter(|h| !h.is_empty())) {
                return Ok(join(&url, h));
            }
            if let Some(p) = resps.iter().find_map(|r| r.props.get("current-user-principal").filter(|h| !h.is_empty())) {
                let principal = join(&url, p);
                let resps = self.propfind(&principal, "0", home_xml)?;
                if let Some(h) = resps.iter().find_map(|r| r.props.get(home_prop).filter(|h| !h.is_empty())) {
                    return Ok(join(&principal, h));
                }
            }
        }
        Err(format!("Could not find the {} on {base}", if kind == "caldav" { "calendars" } else { "address books" }))
    }

    pub fn calendars(&self, base: &str) -> Result<Vec<Collection>, String> {
        let home = self.home(base, "caldav")?;
        let resps = self.propfind(
            &home,
            "1",
            "<d:resourcetype/><d:displayname/><ic:calendar-color/><c:supported-calendar-component-set/><cs:getctag/>",
        )?;
        Ok(resps
            .into_iter()
            .filter(|r| r.resourcetypes.iter().any(|t| t == "calendar"))
            .filter(|r| r.components.is_empty() || r.components.iter().any(|c| c == "VEVENT"))
            .map(|r| {
                let color = r.props.get("calendar-color").cloned().unwrap_or_default();
                Collection {
                    url: join(&home, &r.href),
                    name: r.props.get("displayname").cloned().filter(|n| !n.is_empty()).unwrap_or_else(|| "Calendar".into()),
                    color: if color.len() >= 7 { color[..7].to_string() } else { String::new() },
                }
            })
            .collect())
    }

    pub fn address_books(&self, base: &str) -> Result<Vec<Collection>, String> {
        let home = self.home(base, "carddav")?;
        let resps = self.propfind(&home, "1", "<d:resourcetype/><d:displayname/><cs:getctag/>")?;
        Ok(resps
            .into_iter()
            .filter(|r| r.resourcetypes.iter().any(|t| t == "addressbook"))
            .map(|r| Collection {
                url: join(&home, &r.href),
                name: r.props.get("displayname").cloned().filter(|n| !n.is_empty()).unwrap_or_else(|| "Contacts".into()),
                color: String::new(),
            })
            .collect())
    }

    /// Events in a calendar between two UTC times ("20250101T000000Z").
    pub fn events(&self, url: &str, from_utc: &str, to_utc: &str) -> Result<Vec<Item>, String> {
        let body = format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
             <c:calendar-query xmlns:d=\"DAV:\" xmlns:c=\"urn:ietf:params:xml:ns:caldav\">\
             <d:prop><d:getetag/><c:calendar-data/></d:prop>\
             <c:filter><c:comp-filter name=\"VCALENDAR\"><c:comp-filter name=\"VEVENT\">\
             <c:time-range start=\"{from_utc}\" end=\"{to_utc}\"/></c:comp-filter></c:comp-filter></c:filter>\
             </c:calendar-query>"
        );
        let (status, text, _) = self.request("REPORT", url, Some("1"), &body, &[])?;
        if status >= 400 {
            return Err(format!("{url}: HTTP {status}"));
        }
        Ok(parse_multistatus(&text)
            .into_iter()
            .filter_map(|r| {
                let data = r.props.get("calendar-data").cloned().filter(|d| d.contains("BEGIN:VCALENDAR"))?;
                Some(Item { href: join(url, &r.href), etag: r.props.get("getetag").cloned().unwrap_or_default(), data })
            })
            .collect())
    }

    /// Every card in an address book (listed first, then fetched in batches).
    pub fn cards(&self, url: &str) -> Result<Vec<Item>, String> {
        let list = self.propfind(url, "1", "<d:getetag/><d:getcontenttype/><d:resourcetype/>")?;
        let hrefs: Vec<String> = list
            .iter()
            .filter(|r| !r.resourcetypes.iter().any(|t| t == "collection" || t == "addressbook"))
            .map(|r| r.href.clone())
            .filter(|h| !h.is_empty())
            .collect();
        let mut out = Vec::new();
        for chunk in hrefs.chunks(100) {
            let refs: String = chunk.iter().map(|h| format!("<d:href>{}</d:href>", xml_escape(h))).collect();
            let body = format!(
                "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
                 <card:addressbook-multiget xmlns:d=\"DAV:\" xmlns:card=\"urn:ietf:params:xml:ns:carddav\">\
                 <d:prop><d:getetag/><card:address-data/></d:prop>{refs}</card:addressbook-multiget>"
            );
            let (status, text, _) = self.request("REPORT", url, Some("1"), &body, &[])?;
            if status >= 400 {
                return Err(format!("{url}: HTTP {status}"));
            }
            for r in parse_multistatus(&text) {
                if let Some(data) = r.props.get("address-data").filter(|d| d.contains("BEGIN:VCARD")) {
                    out.push(Item { href: join(url, &r.href), etag: r.props.get("getetag").cloned().unwrap_or_default(), data: data.clone() });
                }
            }
        }
        Ok(out)
    }

    /// Creates or replaces an item. Returns the new ETag (may be empty if
    /// the server doesn't say; it is read again on the next sync).
    pub fn put(&self, href: &str, etag: &str, data: &str, content_type: &str) -> Result<String, String> {
        let mut headers = vec![("Content-Type", content_type)];
        if etag.is_empty() {
            headers.push(("If-None-Match", "*"));
        } else {
            headers.push(("If-Match", etag));
        }
        let (status, _, hdrs) = self.request("PUT", href, None, data, &headers)?;
        match status {
            200..=299 => Ok(hdrs.get("etag").cloned().unwrap_or_default()),
            412 => Err("It was changed somewhere else first. Sync again to see the newest version.".into()),
            s => Err(format!("The server refused the change (HTTP {s})")),
        }
    }

    pub fn delete(&self, href: &str, etag: &str) -> Result<(), String> {
        let headers: Vec<(&str, &str)> = if etag.is_empty() { vec![] } else { vec![("If-Match", etag)] };
        let (status, _, _) = self.request("DELETE", href, None, "", &headers)?;
        match status {
            200..=299 | 404 => Ok(()),
            412 => Err("It was changed somewhere else first. Sync again to see the newest version.".into()),
            s => Err(format!("The server refused to delete it (HTTP {s})")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_multistatus() {
        let xml = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
 <d:response><d:href>/cal/home/</d:href>
  <d:propstat><d:prop><d:resourcetype><d:collection/><c:calendar/></d:resourcetype>
   <d:displayname>Home &amp; Family</d:displayname>
   <c:supported-calendar-component-set><c:comp name="VEVENT"/></c:supported-calendar-component-set>
  </d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
  <d:propstat><d:prop><x:missing xmlns:x="X"/></d:prop><d:status>HTTP/1.1 404 Not Found</d:status></d:propstat>
 </d:response>
 <d:response><d:href>/cal/home/a.ics</d:href>
  <d:propstat><d:prop><d:getetag>"abc"</d:getetag><c:calendar-data>BEGIN:VCALENDAR&#13;
END:VCALENDAR</c:calendar-data></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
 </d:response>
</d:multistatus>"#;
        let r = parse_multistatus(xml);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].props.get("displayname").unwrap(), "Home & Family");
        assert!(r[0].resourcetypes.contains(&"calendar".to_string()));
        assert_eq!(r[0].components, vec!["VEVENT".to_string()]);
        assert_eq!(r[1].props.get("getetag").unwrap(), "\"abc\"");
        assert!(r[1].props.get("calendar-data").unwrap().contains("END:VCALENDAR"));
        assert_eq!(join("https://x.org/cal/home/", "/cal/home/a.ics"), "https://x.org/cal/home/a.ics");
    }
}
