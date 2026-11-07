//! Keeps the app readable: every text colour in qml/Theme.qml, on every
//! background it's drawn on, must meet the WCAG contrast ratios in both light
//! and dark mode (7:1 main text, 4.5:1 smaller / secondary text and links,
//! 3:1 icons). Change a colour in Theme.qml and this says if it got too faint.

use std::collections::HashMap;

/// name → (dark, light) from `readonly property color name: dark ? "#…" : "#…"`.
fn theme() -> HashMap<String, (String, String)> {
    let qml = include_str!("../qml/Theme.qml");
    let re = regex::Regex::new(r#"readonly property color (\w+): (?:dark \? "(#[0-9a-fA-F]{6})" : "(#[0-9a-fA-F]{6})"|"(#[0-9a-fA-F]{6})")"#).unwrap();
    re.captures_iter(qml)
        .map(|c| {
            let both = c.get(4).map(|m| m.as_str().to_string());
            let dark = c.get(2).map(|m| m.as_str().to_string()).or_else(|| both.clone()).unwrap();
            let light = c.get(3).map(|m| m.as_str().to_string()).or(both).unwrap();
            (c[1].to_string(), (dark, light))
        })
        .collect()
}

fn luminance(hex: &str) -> f64 {
    let ch = |i: usize| {
        let v = u8::from_str_radix(&hex[i..i + 2], 16).unwrap() as f64 / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * ch(1) + 0.7152 * ch(3) + 0.0722 * ch(5)
}

/// WCAG contrast ratio, 1 (none) to 21 (black on white).
fn ratio(a: &str, b: &str) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

#[test]
fn every_text_colour_is_readable_on_its_backgrounds() {
    let t = theme();
    // Where text is drawn: the main surfaces, and the highlighted ones.
    let main = ["window", "sidebar", "card", "toolbar", "popup", "field"];
    let all = [&main[..], &["hover", "sidebarSelected", "fieldIdle", "segmentOn", "accentSoft"]].concat();
    let rules: Vec<(&str, Vec<&str>, f64)> = vec![
        ("text", all.clone(), 7.0),
        ("secondary", all.clone(), 4.5),
        ("tertiary", main.to_vec(), 4.5),
        ("accent", [&main[..], &["accentSoft"]].concat(), 4.5),
        ("accentInk", vec!["accentFill"], 4.5),
        ("toastText", vec!["toast"], 7.0),
        ("red", vec!["window", "card"], 3.0),
        ("orange", vec!["window", "card"], 3.0),
    ];
    let mut faint = Vec::new();
    for (fg, bgs, need) in rules {
        for bg in bgs {
            for (mode, pick) in [("dark", 0), ("light", 1)] {
                let colour = |n: &str| {
                    let (d, l) = t.get(n).unwrap_or_else(|| panic!("no colour {n} in Theme.qml"));
                    if pick == 0 { d.clone() } else { l.clone() }
                };
                let r = ratio(&colour(fg), &colour(bg));
                if r < need {
                    faint.push(format!("{mode}: {fg} on {bg} is {r:.2}:1, needs {need}:1"));
                }
            }
        }
    }
    assert!(faint.is_empty(), "hard to read:\n{}", faint.join("\n"));
}

#[test]
fn ratios_match_the_standard() {
    assert!((ratio("#000000", "#ffffff") - 21.0).abs() < 0.01);
    assert!((ratio("#777777", "#ffffff") - 4.48).abs() < 0.01);
}
