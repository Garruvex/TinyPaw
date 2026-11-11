//! Speech lines: short things a character says in a bubble when clicked,
//! picked up, dropped, idle, when music starts or when annotation ends. A
//! pack carries its lines; the user's edits are kept as overrides beside it.

use serde_json::{Value as Json, json};

/// When a line can be said.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Trigger {
    Click,
    Pickup,
    Drop,
    Idle,
    MusicStart,
    AnnotationEnd,
}

impl Trigger {
    pub const ALL: [Trigger; 6] = [
        Trigger::Click,
        Trigger::Pickup,
        Trigger::Drop,
        Trigger::Idle,
        Trigger::MusicStart,
        Trigger::AnnotationEnd,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Trigger::Click => "click",
            Trigger::Pickup => "drag",
            Trigger::Drop => "drop",
            Trigger::Idle => "idle",
            Trigger::MusicStart => "music",
            Trigger::AnnotationEnd => "annotation_end",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Trigger::Click => "Click",
            Trigger::Pickup => "Pick up",
            Trigger::Drop => "Drop",
            Trigger::Idle => "Idle",
            Trigger::MusicStart => "Music starts",
            Trigger::AnnotationEnd => "Annotation ends",
        }
    }

    fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|t| *t == self)
            .expect("listed trigger")
    }

    /// Lines nobody asked for, which Quiet lines turns off.
    pub fn unprompted(self) -> bool {
        matches!(self, Trigger::Idle | Trigger::MusicStart)
    }
}

/// Where a line's sound lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SoundRef {
    /// A WAV inside the character's pack folder.
    Pack(String),
    /// A WAV the user added, in TinyPaw's own storage for this character.
    User(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub text: String,
    pub sound: Option<SoundRef>,
}

/// The speech bubble's look.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bubble {
    pub background: [u8; 4],
    pub text: [u8; 4],
    /// Widest the bubble grows, in logical pixels.
    pub max_width: f32,
}

impl Default for Bubble {
    fn default() -> Self {
        Self {
            background: [255, 255, 255, 255],
            text: [27, 27, 31, 255],
            max_width: 220.0,
        }
    }
}

pub const MAX_LINES: usize = 200;
pub const MAX_CHARS: usize = 280;
pub const MIN_WIDTH: f32 = 120.0;
pub const MAX_WIDTH: f32 = 400.0;
/// Encoded size limit for one line sound.
pub const MAX_SOUND: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Lines {
    by_trigger: [Vec<Line>; 6],
    pub bubble: Bubble,
}

impl Lines {
    pub fn get(&self, t: Trigger) -> &[Line] {
        &self.by_trigger[t.index()]
    }

    pub fn get_mut(&mut self, t: Trigger) -> &mut Vec<Line> {
        &mut self.by_trigger[t.index()]
    }

    pub fn total(&self) -> usize {
        self.by_trigger.iter().map(Vec::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }

    /// Every sound the lines use, each once.
    pub fn sounds(&self) -> Vec<&SoundRef> {
        let mut out: Vec<&SoundRef> = Vec::new();
        for line in self.by_trigger.iter().flatten() {
            if let Some(s) = &line.sound
                && !out.contains(&s)
            {
                out.push(s);
            }
        }
        out
    }

    /// Reads `lines` and `bubble` from a manifest or override file. Pack
    /// files may only name pack sounds; overrides may also name user sounds.
    pub fn parse(lines: &Json, bubble: &Json, allow_user: bool) -> Result<Self, String> {
        let mut out = Self::default();
        match lines {
            Json::Null => {}
            Json::Object(map) => {
                for (key, list) in map {
                    let t = Trigger::ALL
                        .into_iter()
                        .find(|t| t.key() == key)
                        .ok_or(format!("unknown line trigger `{key}`"))?;
                    let list = list
                        .as_array()
                        .ok_or(format!("{key} lines must be a list"))?;
                    for v in list {
                        out.get_mut(t).push(parse_line(v, allow_user)?);
                    }
                }
            }
            _ => return Err("lines must map triggers to lists".into()),
        }
        if out.total() > MAX_LINES {
            return Err(format!("a character can have at most {MAX_LINES} lines"));
        }
        if let Json::Object(_) = bubble {
            if let Some(c) = bubble["background"].as_str() {
                out.bubble.background =
                    parse_color(c).ok_or("bubble background must be #RRGGBB")?;
            }
            if let Some(c) = bubble["text"].as_str() {
                out.bubble.text = parse_color(c).ok_or("bubble text colour must be #RRGGBB")?;
            }
            if let Some(w) = bubble["max_width"].as_f64() {
                let w = w as f32;
                if !(MIN_WIDTH..=MAX_WIDTH).contains(&w) {
                    return Err(format!("bubble max_width must be {MIN_WIDTH}–{MAX_WIDTH}"));
                }
                out.bubble.max_width = w;
            }
        }
        Ok(out)
    }

    /// `(lines, bubble)` as JSON, or `None` when there's nothing to write.
    pub fn to_json(&self) -> Option<(Json, Json)> {
        if self.is_empty() && self.bubble == Bubble::default() {
            return None;
        }
        let mut map = serde_json::Map::new();
        for t in Trigger::ALL {
            let list = self.get(t);
            if list.is_empty() {
                continue;
            }
            let items = list
                .iter()
                .map(|l| {
                    let mut v = json!({ "text": l.text });
                    match &l.sound {
                        Some(SoundRef::Pack(p)) => v["sound"] = json!(p),
                        Some(SoundRef::User(f)) => v["sound"] = json!(format!("user:{f}")),
                        None => {}
                    }
                    v
                })
                .collect();
            map.insert(t.key().to_owned(), Json::Array(items));
        }
        let hex = |c: [u8; 4]| format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]);
        Some((
            Json::Object(map),
            json!({
                "background": hex(self.bubble.background),
                "text": hex(self.bubble.text),
                "max_width": self.bubble.max_width,
            }),
        ))
    }
}

fn parse_line(v: &Json, allow_user: bool) -> Result<Line, String> {
    let text = v["text"].as_str().unwrap_or_default().trim();
    if text.is_empty() || text.chars().count() > MAX_CHARS {
        return Err(format!("each line needs 1–{MAX_CHARS} characters of text"));
    }
    let sound = match v["sound"].as_str() {
        None => None,
        Some(s) => Some(match s.strip_prefix("user:") {
            Some(file) if allow_user && valid_user_sound(file) => SoundRef::User(file.to_owned()),
            Some(_) => return Err(format!("`{s}` isn't a sound in this pack")),
            None if valid_pack_sound(s) => SoundRef::Pack(s.to_owned()),
            None => {
                return Err(format!(
                    "line sound `{s}` must be a WAV inside the pack folder"
                ));
            }
        }),
    };
    Ok(Line {
        text: text.to_owned(),
        sound,
    })
}

/// A WAV path inside a pack: relative, `/`-separated, no `..`.
pub fn valid_pack_sound(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 200
        && !path.contains(['\\', ':'])
        && path.to_ascii_lowercase().ends_with(".wav")
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

/// A file name TinyPaw gave an imported line sound.
pub fn valid_user_sound(file: &str) -> bool {
    (5..=64).contains(&file.len())
        && file.ends_with(".wav")
        && file
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
        && !file.starts_with('.')
}

fn parse_color(text: &str) -> Option<[u8; 4]> {
    let hex = text.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(hex, 16).ok()?;
    Some([(n >> 16) as u8, (n >> 8) as u8, n as u8, 255])
}

/// `//` splits a line into parts shown one after another.
pub fn parts(text: &str) -> Vec<String> {
    text.split("//")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_owned)
        .collect()
}

/// How long a part stays up: long enough to read, never forever.
pub fn reading_time(part: &str) -> f32 {
    (1.2 + part.chars().count() as f32 * 0.06).clamp(1.5, 8.0)
}

/// Picks lines at random, never the same one twice in a row per trigger.
#[derive(Debug, Clone)]
pub struct Picker {
    state: u64,
    last: [Option<usize>; 6],
}

impl Picker {
    pub fn new(seed: u64) -> Self {
        Self {
            state: seed | 1,
            last: [None; 6],
        }
    }

    /// A random number in `0..n` (xorshift; plenty for picking lines).
    pub fn below(&mut self, n: usize) -> usize {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        (self.state % n.max(1) as u64) as usize
    }

    pub fn pick<'a>(&mut self, lines: &'a Lines, t: Trigger) -> Option<&'a Line> {
        let list = lines.get(t);
        let n = list.len();
        if n == 0 {
            return None;
        }
        let mut i = self.below(n);
        if n > 1 && Some(i) == self.last[t.index()] {
            i = (i + 1 + self.below(n - 1)) % n;
        }
        self.last[t.index()] = Some(i);
        list.get(i)
    }
}

impl Default for Picker {
    fn default() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0x9E37_79B9, |d| d.as_nanos() as u64);
        Self::new(seed)
    }
}

/// A line being said: its parts, which one shows, and until when.
#[derive(Debug, Clone)]
pub struct Speech {
    pub parts: Vec<String>,
    pub index: usize,
    pub until: f32,
}

impl Speech {
    pub fn new(text: &str, now: f32) -> Option<Self> {
        let parts = parts(text);
        let first = parts.first()?;
        let until = now + reading_time(first);
        Some(Self {
            parts,
            index: 0,
            until,
        })
    }

    /// Keeps the current part up for at least `seconds` more, so a bubble
    /// stays until its sound ends.
    pub fn hold(&mut self, now: f32, seconds: f32) {
        self.until = self.until.max(now + seconds);
    }

    pub fn text(&self) -> &str {
        &self.parts[self.index]
    }

    /// Moves to the next part when this one has been up long enough.
    /// Returns false once the line is over.
    pub fn advance(&mut self, now: f32) -> bool {
        while now >= self.until {
            self.index += 1;
            let Some(next) = self.parts.get(self.index) else {
                return false;
            };
            self.until += reading_time(next);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_round_trip() {
        let lines = json!({
            "click": [{ "text": "Hi!" }, { "text": "That tickles // stop it", "sound": "lines/giggle.wav" }],
            "idle": [{ "text": "…", "sound": "user:a1b2.wav" }],
        });
        let bubble = json!({ "background": "#202020", "max_width": 260 });
        assert!(
            Lines::parse(&lines, &bubble, false).is_err(),
            "packs can't name user sounds"
        );
        let l = Lines::parse(&lines, &bubble, true).unwrap();
        assert_eq!(l.get(Trigger::Click).len(), 2);
        assert_eq!(l.sounds().len(), 2);
        assert_eq!(l.bubble.background, [32, 32, 32, 255]);
        let (lj, bj) = l.to_json().unwrap();
        assert_eq!(Lines::parse(&lj, &bj, true).unwrap(), l);

        for bad in [
            json!({ "wink": [{ "text": "x" }] }),
            json!({ "click": [{ "text": "" }] }),
            json!({ "click": [{ "text": "x", "sound": "../x.wav" }] }),
            json!({ "click": [{ "text": "x", "sound": "x.mp3" }] }),
        ] {
            assert!(Lines::parse(&bad, &Json::Null, true).is_err(), "{bad}");
        }
        assert!(Lines::parse(&Json::Null, &json!({ "max_width": 9000 }), true).is_err());
        assert!(Lines::default().to_json().is_none());
    }

    #[test]
    fn picker_never_repeats_back_to_back() {
        let mut lines = Lines::default();
        for t in ["a", "b", "c"] {
            lines.get_mut(Trigger::Click).push(Line {
                text: t.into(),
                sound: None,
            });
        }
        let mut p = Picker::new(42);
        let mut last = String::new();
        let mut seen = std::collections::HashSet::new();
        for _ in 0..200 {
            let line = p.pick(&lines, Trigger::Click).unwrap();
            assert_ne!(line.text, last);
            last = line.text.clone();
            seen.insert(last.clone());
        }
        assert_eq!(seen.len(), 3, "every line comes up");
        assert!(p.pick(&lines, Trigger::Drop).is_none());
    }

    #[test]
    fn parts_show_in_turn() {
        assert_eq!(parts(" Hi // there //  "), ["Hi", "there"]);
        let mut s = Speech::new("One // Two", 0.0).unwrap();
        assert_eq!(s.text(), "One");
        assert!(s.advance(s.until));
        assert_eq!(s.text(), "Two");
        assert!(!s.advance(100.0));
        assert!(Speech::new(" // ", 0.0).is_none());
        assert!(reading_time("x") >= 1.5 && reading_time(&"x".repeat(500)) <= 8.0);
    }
}
