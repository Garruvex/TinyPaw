//! Guards for the Slint UI in `ui/` (see docs/ui-guide.md).
//!
//! 1. Pages and templates hold no raw sizes, colours or stock widgets: they use
//!    the kit and `Tokens`. A line that must break the rule, such as a window's
//!    first size, carries `guard:allow`.
//! 2. The colours in `ui/tokens.slint` match `theme.rs`, so Slint windows and
//!    Iced overlays cannot drift apart.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn ui_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("ui")
}

fn slint_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n != "target") {
                slint_files(&path, out);
            }
        } else if path.extension().is_some_and(|e| e == "slint") {
            out.push(path);
        }
    }
}

/// Problems in one page or template source, as `line: message`.
fn violations(source: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (index, raw) in source.lines().enumerate() {
        if raw.contains("guard:allow") {
            continue;
        }
        let line = raw.split("//").next().unwrap_or("");
        let at = index + 1;
        if line.contains("std-widgets.slint") {
            found.push(format!(
                "{at}: stock widgets ignore the tokens; use the kit"
            ));
        }
        let bytes = line.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'#' && bytes.get(i + 1).is_some_and(u8::is_ascii_hexdigit) {
                found.push(format!("{at}: raw colour; add a token"));
                break;
            }
            if bytes[i].is_ascii_digit() && (i == 0 || !is_word(bytes[i - 1])) {
                let mut j = i;
                while j < bytes.len() && (bytes[j].is_ascii_digit() || bytes[j] == b'.') {
                    j += 1;
                }
                if line[j..].starts_with("px") && line[i..j].parse::<f64>().is_ok_and(|v| v != 0.0)
                {
                    found.push(format!("{at}: raw size {}px; use a token", &line[i..j]));
                }
                i = j;
                continue;
            }
            i += 1;
        }
    }
    found
}

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_'
}

#[test]
fn scanner_flags_raw_values() {
    assert_eq!(violations("padding: 12px;").len(), 1);
    assert_eq!(violations("color: #FF00FF;").len(), 1);
    assert_eq!(
        violations("import { Switch } from \"std-widgets.slint\";").len(),
        1
    );
    assert_eq!(violations("width: 1.5px;").len(), 1);
}

#[test]
fn scanner_allows_tokens_zero_and_marked_lines() {
    assert!(violations("padding: Tokens.space-3;").is_empty());
    assert!(violations("x: 0px;").is_empty());
    assert!(violations("spacing: Tokens.space-1; // 12px in a comment").is_empty());
    assert!(violations("preferred-width: 1000px; // guard:allow window size").is_empty());
    assert!(violations("property <int> col-2: 3;").is_empty());
}

#[test]
fn pages_and_templates_use_only_the_kit_and_tokens() {
    let mut files = Vec::new();
    slint_files(&ui_dir(), &mut files);
    let mut checked = 0;
    let mut problems = Vec::new();
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if name == "kit.slint" || name == "tokens.slint" {
            continue;
        }
        checked += 1;
        let source = fs::read_to_string(&path).unwrap();
        for v in violations(&source) {
            problems.push(format!(
                "{}:{v}",
                path.strip_prefix(ui_dir()).unwrap().display()
            ));
        }
    }
    assert!(
        checked >= 4,
        "expected pages and templates under ui/, found {checked}"
    );
    assert!(
        problems.is_empty(),
        "UI guard failed:\n{}",
        problems.join("\n")
    );
}

/// `#RRGGBB` or `#RRGGBBAA` to RGBA bytes.
fn hex(value: &str) -> [u8; 4] {
    let digits = value.trim_start_matches('#');
    let byte = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).unwrap();
    [
        byte(0),
        byte(2),
        byte(4),
        if digits.len() == 8 { byte(6) } else { 255 },
    ]
}

fn token_colors(source: &str) -> BTreeMap<String, [u8; 4]> {
    let mut map = BTreeMap::new();
    for line in source.lines() {
        if let Some(rest) = line.trim().strip_prefix("out property <color> ") {
            let (name, value) = rest.split_once(':').unwrap();
            let value = value.trim().trim_end_matches(';');
            if value.starts_with('#') {
                map.insert(name.trim().to_string(), hex(value));
            }
        }
    }
    map
}

/// `pub const NAME: Color = Color::from_rgb8(..)` or `from_rgba8(.., alpha)`.
fn theme_colors(source: &str) -> BTreeMap<String, [u8; 4]> {
    let mut map = BTreeMap::new();
    for line in source.lines() {
        let Some(rest) = line.trim().strip_prefix("pub const ") else {
            continue;
        };
        let Some((name, value)) = rest.split_once(": Color = Color::from_rgb") else {
            continue;
        };
        let (alpha, args) = match value.strip_prefix("a8(") {
            Some(args) => (true, args),
            None => (false, value.strip_prefix("8(").unwrap()),
        };
        let parts: Vec<&str> = args
            .trim_end_matches(");")
            .split(',')
            .map(str::trim)
            .collect();
        let channel = |s: &str| u8::from_str_radix(s.trim_start_matches("0x"), 16).unwrap();
        let a = if alpha {
            (parts[3].parse::<f32>().unwrap() * 255.0).round() as u8
        } else {
            255
        };
        map.insert(
            name.to_lowercase().replace('_', "-"),
            [channel(parts[0]), channel(parts[1]), channel(parts[2]), a],
        );
    }
    map
}

#[test]
fn slint_tokens_match_theme() {
    let tokens = token_colors(&fs::read_to_string(ui_dir().join("tokens.slint")).unwrap());
    let theme = theme_colors(include_str!("theme.rs"));
    let mut shared = 0;
    for (name, color) in &tokens {
        if let Some(expected) = theme.get(name) {
            shared += 1;
            let close = color.iter().zip(expected).all(|(a, b)| a.abs_diff(*b) <= 1);
            assert!(
                close,
                "token `{name}` is {color:?} but theme.rs has {expected:?}"
            );
        }
    }
    assert!(
        shared >= 20,
        "expected most tokens to mirror theme.rs, found {shared}"
    );
}
