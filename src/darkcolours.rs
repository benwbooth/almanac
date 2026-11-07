//! Dark mode for formatted mail being written (a reply's quoted original, a
//! draft): colours the email set for a white page are adapted for the dark
//! one, as the message view does: text too dark to read there is made light
//! (same hue), light backgrounds are made dark. Only for showing: the colours
//! go back to the original ones in what's sent or saved.

use std::collections::{HashMap, HashSet};

type Rgb = (u8, u8, u8);

fn hex(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2)
}

/// `#abc`, `#aabbcc`, `#aabbccdd`, `rgb(…)`, `rgba(…)` or a common colour name.
fn parse(s: &str) -> Option<Rgb> {
    let s = s.trim().to_ascii_lowercase();
    if let Some(h) = s.strip_prefix('#') {
        let n = |i: usize, len: usize| u8::from_str_radix(&h[i..i + len], 16).ok();
        return match h.len() {
            3 => Some((n(0, 1)? * 17, n(1, 1)? * 17, n(2, 1)? * 17)),
            6 | 8 => Some((n(0, 2)?, n(2, 2)?, n(4, 2)?)),
            _ => None,
        };
    }
    if s.starts_with("rgb") {
        let inner = s.split_once('(')?.1.trim_end_matches(')');
        let parts: Vec<&str> = inner.split(|c| c == ',' || c == ' ' || c == '/').filter(|p| !p.is_empty()).collect();
        if parts.len() < 3 {
            return None;
        }
        // A see-through colour (alpha 0) isn't a colour to adapt.
        if parts.len() >= 4 && parts[3].trim_end_matches('%').parse::<f64>().map(|a| a == 0.0).unwrap_or(false) {
            return None;
        }
        let ch = |p: &str| -> Option<u8> {
            if let Some(pc) = p.strip_suffix('%') {
                pc.parse::<f64>().ok().map(|v| (v.clamp(0.0, 100.0) * 2.55).round() as u8)
            } else {
                p.parse::<f64>().ok().map(|v| v.clamp(0.0, 255.0).round() as u8)
            }
        };
        return Some((ch(parts[0])?, ch(parts[1])?, ch(parts[2])?));
    }
    let named = match s.as_str() {
        "black" => (0, 0, 0),
        "white" => (255, 255, 255),
        "gray" | "grey" => (128, 128, 128),
        "darkgray" | "darkgrey" => (169, 169, 169),
        "dimgray" | "dimgrey" => (105, 105, 105),
        "silver" => (192, 192, 192),
        "lightgray" | "lightgrey" => (211, 211, 211),
        "whitesmoke" => (245, 245, 245),
        "gainsboro" => (220, 220, 220),
        "red" => (255, 0, 0),
        "darkred" | "maroon" => (128, 0, 0),
        "green" => (0, 128, 0),
        "darkgreen" => (0, 100, 0),
        "blue" => (0, 0, 255),
        "navy" | "darkblue" => (0, 0, 128),
        "purple" => (128, 0, 128),
        "teal" => (0, 128, 128),
        "olive" => (128, 128, 0),
        "yellow" => (255, 255, 0),
        "lightyellow" => (255, 255, 224),
        "orange" => (255, 165, 0),
        "brown" => (165, 42, 42),
        "ivory" => (255, 255, 240),
        "beige" => (245, 245, 220),
        "aliceblue" => (240, 248, 255),
        "lightblue" => (173, 216, 230),
        _ => return None,
    };
    Some(named)
}

fn luminance(c: Rgb) -> f64 {
    let ch = |v: u8| {
        let v = v as f64 / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * ch(c.0) + 0.7152 * ch(c.1) + 0.0722 * ch(c.2)
}

/// WCAG contrast, 1 to 21.
fn ratio(a: Rgb, b: Rgb) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

fn to_hsl(c: Rgb) -> (f64, f64, f64) {
    let (r, g, b) = (c.0 as f64 / 255.0, c.1 as f64 / 255.0, c.2 as f64 / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) / 2.0;
    if max == min {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == r { (g - b) / d + if g < b { 6.0 } else { 0.0 } } else if max == g { (b - r) / d + 2.0 } else { (r - g) / d + 4.0 };
    (h / 6.0, s, l)
}

fn from_hsl(h: f64, s: f64, l: f64) -> Rgb {
    let l = l.clamp(0.0, 1.0);
    if s == 0.0 {
        let v = (l * 255.0).round() as u8;
        return (v, v, v);
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let f = |mut t: f64| {
        if t < 0.0 { t += 1.0 }
        if t > 1.0 { t -= 1.0 }
        let v = if t < 1.0 / 6.0 { p + (q - p) * 6.0 * t } else if t < 0.5 { q } else if t < 2.0 / 3.0 { p + (q - p) * (2.0 / 3.0 - t) * 6.0 } else { p };
        (v * 255.0).round().clamp(0.0, 255.0) as u8
    };
    (f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0))
}

/// Text: kept if it's readable on the dark page, otherwise the same hue made
/// light (mirrored lightness, then lighter until it reads well).
fn text_colour(c: Rgb, page: Rgb) -> Option<Rgb> {
    if ratio(c, page) >= 4.5 {
        return None;
    }
    let (h, s, l) = to_hsl(c);
    let mut l2 = (1.0 - l).max(0.6);
    let mut out = from_hsl(h, s, l2);
    while ratio(out, page) < 7.0 && l2 < 1.0 {
        l2 = (l2 + 0.02).min(1.0);
        out = from_hsl(h, s, l2);
    }
    Some(out)
}

/// Backgrounds: light ones (made for a white page) become dark, same hue;
/// dark ones stay.
fn background_colour(c: Rgb, page: Rgb) -> Option<Rgb> {
    let (h, s, l) = to_hsl(c);
    if l <= 0.5 {
        return None;
    }
    // White → the page itself; other light colours a dark shade of their hue.
    let (_, _, page_l) = to_hsl(page);
    Some(from_hsl(h, s * 0.6, page_l + (1.0 - l) * 0.35))
}

/// The HTML with its colours adapted for a dark page (`page`, e.g. "#1e1e1e"),
/// and what each adapted colour was (adapted hex → original hex) for `restore`.
pub fn adapt(html: &str, page: &str) -> (String, HashMap<String, String>) {
    let Some(page) = parse(page) else { return (html.to_string(), HashMap::new()) };
    // CSS: color / background-color / background (a colour first); attributes: color= and bgcolor=.
    let css = regex::Regex::new(r#"(?i)(^|[;"'\s{])(background-color|background|color)(\s*:\s*)(#[0-9a-f]{3,8}\b|rgba?\([^)]*\)|[a-z]+\b)"#).unwrap();
    let attr = regex::Regex::new(r#"(?i)(\s)(bgcolor|color)(\s*=\s*["']?)(#?[0-9a-f]{6}\b|#[0-9a-f]{3}\b|[a-z]+\b)"#).unwrap();
    // Every colour already in the message: an adapted one must differ from all of
    // them (and from each other), so putting the originals back is exact.
    let mut taken: HashSet<String> = HashSet::new();
    for re in [&css, &attr] {
        for c in re.captures_iter(html) {
            let v = c[4].to_string();
            let v = if !v.starts_with('#') && v.len() == 6 && v.chars().all(|ch| ch.is_ascii_hexdigit()) { format!("#{v}") } else { v };
            if let Some(rgb) = parse(&v) {
                taken.insert(hex(rgb));
            }
        }
    }
    let mut back: HashMap<String, String> = HashMap::new();
    let mut chosen: HashMap<(String, bool), String> = HashMap::new();
    let mut pick = |value: &str, is_background: bool| -> Option<String> {
        let v = if !value.starts_with('#') && value.len() == 6 && value.chars().all(|ch| ch.is_ascii_hexdigit()) { format!("#{value}") } else { value.to_string() };
        let rgb = parse(&v)?;
        let key = (hex(rgb), is_background);
        if let Some(done) = chosen.get(&key) {
            return Some(done.clone());
        }
        let mut out = if is_background { background_colour(rgb, page)? } else { text_colour(rgb, page)? };
        // Nudged to a colour of its own (no one could tell the difference).
        let mut step = 0u8;
        while taken.contains(&hex(out)) || back.contains_key(&hex(out)) {
            step = step.wrapping_add(1);
            out = (out.0, out.1, if out.2 >= 128 { out.2 - 1 } else { out.2 + 1 });
            if step > 200 {
                return None;
            }
        }
        let h = hex(out);
        back.insert(h.clone(), hex(rgb));
        chosen.insert(key, h.clone());
        Some(h)
    };
    let html = css
        .replace_all(html, |c: &regex::Captures| {
            let bg = c[2].to_ascii_lowercase().starts_with("background");
            match pick(&c[4], bg) {
                Some(new) => format!("{}{}{}{}", &c[1], &c[2], &c[3], new),
                None => c[0].to_string(),
            }
        })
        .into_owned();
    let html = attr
        .replace_all(&html, |c: &regex::Captures| {
            let bg = c[2].eq_ignore_ascii_case("bgcolor");
            match pick(&c[4], bg) {
                Some(new) => format!("{}{}{}{}", &c[1], &c[2], &c[3], new),
                None => c[0].to_string(),
            }
        })
        .into_owned();
    (html, back)
}

/// The original colours back in the editor's HTML (`back` from `adapt`).
pub fn restore(html: &str, back: &HashMap<String, String>) -> String {
    if back.is_empty() {
        return html.to_string();
    }
    let re = regex::Regex::new(r"(?i)#[0-9a-f]{6}\b").unwrap();
    re.replace_all(html, |c: &regex::Captures| back.get(&c[0].to_ascii_lowercase()).cloned().unwrap_or_else(|| c[0].to_string()))
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = "#1e1e1e";

    #[test]
    fn black_text_turns_light_and_comes_back() {
        let html = r#"<div style="color: rgb(0, 0, 0); font-family: Calibri">Hello</div><p style="color:#0b5394">link-ish</p><p style="color:#ffffff">white</p>"#;
        let (dark, back) = adapt(html, PAGE);
        let page = parse(PAGE).unwrap();
        for c in regex::Regex::new(r"color:\s*(#[0-9a-f]{6})").unwrap().captures_iter(&dark) {
            assert!(ratio(parse(&c[1]).unwrap(), page) >= 4.5, "{} is readable on the dark page", &c[1]);
        }
        assert!(dark.contains("color:#ffffff"), "white text left as it is");
        // What Qt's editor gives back (colours as #rrggbb) has the originals again.
        let restored = restore(&dark, &back);
        assert!(restored.contains("color: #000000"));
        assert!(restored.contains("color:#0b5394"));
        assert!(restored.contains("color:#ffffff"), "real white stays white");
    }

    #[test]
    fn light_backgrounds_turn_dark_and_come_back() {
        let html = r##"<table bgcolor="#ffffff"><tr><td style="background-color: rgb(252, 252, 252)">x</td><td style="background:#222">y</td></tr></table>"##;
        let (dark, back) = adapt(html, PAGE);
        let bg = regex::Regex::new(r#"bgcolor="(#[0-9a-f]{6})""#).unwrap().captures(&dark).unwrap()[1].to_string();
        assert!(luminance(parse(&bg).unwrap()) < 0.05, "white table background made dark: {bg}");
        assert!(dark.contains("background:#222"), "dark backgrounds stay");
        let restored = restore(&dark, &back);
        assert!(restored.contains(r##"bgcolor="#ffffff""##));
        assert!(restored.contains("background-color: #fcfcfc"));
    }

    #[test]
    fn border_colours_and_plain_words_are_left_alone() {
        let html = r#"<p style="border-color:#000;border-left:2px solid #c8c8ce">the color: is fine</p>"#;
        let (dark, _) = adapt(html, PAGE);
        assert!(dark.contains("border-color:#000"));
    }
}
