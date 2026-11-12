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
    Headpat,
    Sleep,
    Wake,
}

impl Trigger {
    pub const ALL: [Trigger; 9] = [
        Trigger::Click,
        Trigger::Pickup,
        Trigger::Drop,
        Trigger::Idle,
        Trigger::MusicStart,
        Trigger::AnnotationEnd,
        Trigger::Headpat,
        Trigger::Sleep,
        Trigger::Wake,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Trigger::Click => "click",
            Trigger::Pickup => "drag",
            Trigger::Drop => "drop",
            Trigger::Idle => "idle",
            Trigger::MusicStart => "music",
            Trigger::AnnotationEnd => "annotation_end",
            Trigger::Headpat => "headpat",
            Trigger::Sleep => "sleep",
            Trigger::Wake => "wake",
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
            Trigger::Headpat => "Headpat",
            Trigger::Sleep => "Sleep",
            Trigger::Wake => "Wake",
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

/// Which side of its anchor the speech bubble opens on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BubbleSide {
    #[default]
    Above,
    Left,
    Right,
}

impl BubbleSide {
    pub const ALL: [BubbleSide; 3] = [BubbleSide::Above, BubbleSide::Left, BubbleSide::Right];

    pub fn name(self) -> &'static str {
        match self {
            BubbleSide::Above => "above",
            BubbleSide::Left => "left",
            BubbleSide::Right => "right",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|x| x.name() == s)
    }
}

impl std::fmt::Display for BubbleSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            BubbleSide::Above => "Above",
            BubbleSide::Left => "Left",
            BubbleSide::Right => "Right",
        })
    }
}

/// The speech bubble's look and where it sits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bubble {
    pub background: [u8; 4],
    pub text: [u8; 4],
    /// Widest the bubble grows, in logical pixels.
    pub max_width: f32,
    /// The point the bubble's tail aims at, as fractions of the canvas.
    /// `None` follows the top of the visible art.
    pub anchor: Option<(f32, f32)>,
    pub side: BubbleSide,
}

impl Default for Bubble {
    fn default() -> Self {
        Self {
            background: [255, 255, 255, 255],
            text: [27, 27, 31, 255],
            max_width: 220.0,
            anchor: None,
            side: BubbleSide::Above,
        }
    }
}

/// Which side of the character the menu opens on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MenuSide {
    /// Right, unless that would leave the screen.
    #[default]
    Auto,
    Left,
    Right,
}

impl MenuSide {
    pub const ALL: [MenuSide; 3] = [MenuSide::Auto, MenuSide::Left, MenuSide::Right];

    pub fn name(self) -> &'static str {
        match self {
            MenuSide::Auto => "auto",
            MenuSide::Left => "left",
            MenuSide::Right => "right",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|x| x.name() == s)
    }
}

impl std::fmt::Display for MenuSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            MenuSide::Auto => "Auto",
            MenuSide::Left => "Left",
            MenuSide::Right => "Right",
        })
    }
}

/// What the menu lines up with, vertically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MenuAlign {
    /// Its bottom level with the feet.
    #[default]
    Feet,
    /// Centred on the visible art.
    Middle,
    /// Its top level with the top of the visible art.
    Head,
}

impl MenuAlign {
    pub const ALL: [MenuAlign; 3] = [MenuAlign::Feet, MenuAlign::Middle, MenuAlign::Head];

    pub fn name(self) -> &'static str {
        match self {
            MenuAlign::Feet => "feet",
            MenuAlign::Middle => "middle",
            MenuAlign::Head => "head",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|x| x.name() == s)
    }
}

impl std::fmt::Display for MenuAlign {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            MenuAlign::Feet => "Feet",
            MenuAlign::Middle => "Middle",
            MenuAlign::Head => "Head",
        })
    }
}

/// Where the character's menu sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MenuPlace {
    pub side: MenuSide,
    pub align: MenuAlign,
}

/// How [`Lines::merge`] combines imported lines with the current ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Merge {
    /// The imported lines replace every current line.
    #[default]
    Replace,
    /// The imported lines are added after the current ones.
    Add,
}

impl std::fmt::Display for Merge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Merge::Replace => "Replace the current lines",
            Merge::Add => "Add to the current lines",
        })
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
    by_trigger: [Vec<Line>; 9],
    pub bubble: Bubble,
    /// Where the menu sits; kept with the bubble as the character's layout.
    pub menu: MenuPlace,
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

    /// Whether `v` (a manifest, override or dialogue file) says anything
    /// about lines, the bubble or the menu.
    pub fn present(v: &Json) -> bool {
        ["lines", "bubble", "menu"].iter().any(|k| !v[*k].is_null())
    }

    /// Reads `lines`, `bubble` and `menu` from a manifest, override or
    /// dialogue file. Pack and dialogue files may only name sounds beside
    /// them; overrides may also name user sounds.
    pub fn parse(v: &Json, allow_user: bool) -> Result<Self, String> {
        let mut out = Self::parse_parts(&v["lines"], &v["bubble"], allow_user)?;
        let menu = &v["menu"];
        match menu {
            Json::Null => {}
            Json::Object(_) => {
                if let Some(s) = menu["side"].as_str() {
                    out.menu.side = MenuSide::parse(s)
                        .ok_or(format!("menu side `{s}` must be auto, left or right"))?;
                }
                if let Some(s) = menu["align"].as_str() {
                    out.menu.align = MenuAlign::parse(s)
                        .ok_or(format!("menu align `{s}` must be feet, middle or head"))?;
                }
            }
            _ => return Err("menu must be an object".into()),
        }
        Ok(out)
    }

    fn parse_parts(lines: &Json, bubble: &Json, allow_user: bool) -> Result<Self, String> {
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
            match &bubble["anchor"] {
                Json::Null => {}
                v => {
                    out.bubble.anchor = Some(
                        super::pack::fraction_pair(v)
                            .ok_or("bubble anchor must be two fractions of the canvas")?,
                    );
                }
            }
            if let Some(s) = bubble["side"].as_str() {
                out.bubble.side = BubbleSide::parse(s)
                    .ok_or(format!("bubble side `{s}` must be above, left or right"))?;
            }
        }
        Ok(out)
    }

    /// Writes `lines`, `bubble` and (when not the default) `menu` into `v`.
    /// Without `always`, nothing is written when everything is default.
    pub fn write(&self, v: &mut Json, always: bool) {
        match self.to_json() {
            Some((lines, bubble)) => {
                v["lines"] = lines;
                v["bubble"] = bubble;
            }
            None if always => {
                v["lines"] = json!({});
                v["bubble"] = json!({});
            }
            None => {}
        }
        if self.menu != MenuPlace::default() {
            v["menu"] = json!({ "side": self.menu.side.name(), "align": self.menu.align.name() });
        }
    }

    /// Combines `other` into these lines. `layout` also takes its bubble
    /// look and menu placement. Returns how many lines didn't fit.
    pub fn merge(&mut self, other: &Lines, mode: Merge, layout: bool) -> usize {
        if mode == Merge::Replace {
            self.by_trigger = Default::default();
        }
        let mut dropped = 0;
        for t in Trigger::ALL {
            for line in other.get(t) {
                if self.get(t).iter().any(|l| l.text == line.text) {
                    continue;
                }
                if self.total() >= MAX_LINES {
                    dropped += 1;
                    continue;
                }
                self.get_mut(t).push(line.clone());
            }
        }
        if layout {
            self.bubble = other.bubble;
            self.menu = other.menu;
        }
        dropped
    }

    /// Replaces each line's sound with `f`'s answer; `None` drops it.
    pub fn map_sounds(&mut self, mut f: impl FnMut(&SoundRef) -> Option<SoundRef>) {
        for line in self.by_trigger.iter_mut().flatten() {
            if let Some(s) = &line.sound {
                line.sound = f(s);
            }
        }
    }

    /// `(lines, bubble)` as JSON, or `None` when there's nothing to write.
    fn to_json(&self) -> Option<(Json, Json)> {
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
        Some((Json::Object(map), bubble_json(&self.bubble)))
    }
}

fn bubble_json(b: &Bubble) -> Json {
    let hex = |c: [u8; 4]| format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]);
    let mut bubble = json!({
        "background": hex(b.background),
        "text": hex(b.text),
        "max_width": b.max_width,
    });
    if let Some((x, y)) = b.anchor {
        bubble["anchor"] = json!([x, y]);
    }
    if b.side != BubbleSide::Above {
        bubble["side"] = json!(b.side.name());
    }
    bubble
}

/// A starter dialogue file: one example line for every trigger.
pub fn template() -> Json {
    let examples = [
        (Trigger::Click, "Hi there!"),
        (Trigger::Pickup, "Whoa, where are we going?"),
        (Trigger::Drop, "Thanks for the lift."),
        (Trigger::Idle, "…"),
        (Trigger::MusicStart, "Ooh, I like this one."),
        (Trigger::AnnotationEnd, "Nice drawing!"),
        (Trigger::Headpat, "That feels nice."),
        (Trigger::Sleep, "Zzz…"),
        (Trigger::Wake, "Hmm?"),
    ];
    let mut lines = serde_json::Map::new();
    for (t, text) in examples {
        lines.insert(t.key().to_owned(), json!([text]));
    }
    json!({
        "schema_version": 1,
        "lines": lines,
        "bubble": bubble_json(&Bubble::default()),
    })
}

fn parse_line(v: &Json, allow_user: bool) -> Result<Line, String> {
    // A bare string is a line with no sound.
    let text = v
        .as_str()
        .or_else(|| v["text"].as_str())
        .unwrap_or_default()
        .trim();
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
    last: [Option<usize>; 9],
}

impl Picker {
    pub fn new(seed: u64) -> Self {
        Self {
            state: seed | 1,
            last: [None; 9],
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
        let file = json!({ "lines": lines, "bubble": bubble });
        assert!(
            Lines::parse(&file, false).is_err(),
            "packs can't name user sounds"
        );
        let l = Lines::parse(&file, true).unwrap();
        assert_eq!(l.get(Trigger::Click).len(), 2);
        assert_eq!(l.sounds().len(), 2);
        assert_eq!(l.bubble.background, [32, 32, 32, 255]);
        let mut out = json!({});
        l.write(&mut out, false);
        assert_eq!(Lines::parse(&out, true).unwrap(), l);

        for bad in [
            json!({ "lines": { "wink": [{ "text": "x" }] } }),
            json!({ "lines": { "click": [{ "text": "" }] } }),
            json!({ "lines": { "click": [{ "text": "x", "sound": "../x.wav" }] } }),
            json!({ "lines": { "click": [{ "text": "x", "sound": "x.mp3" }] } }),
            json!({ "bubble": { "max_width": 9000 } }),
            json!({ "bubble": { "anchor": [0.5, 2.0] } }),
            json!({ "bubble": { "side": "below" } }),
            json!({ "menu": { "align": "knees" } }),
        ] {
            assert!(Lines::parse(&bad, true).is_err(), "{bad}");
        }
        let mut empty = json!({});
        Lines::default().write(&mut empty, false);
        assert_eq!(empty, json!({}));
        Lines::default().write(&mut empty, true);
        assert!(Lines::present(&empty), "an explicit empty override is kept");
    }

    #[test]
    fn shorthand_layout_and_merge() {
        let file = json!({
            "lines": { "click": ["Hi!", { "text": "Hey" }], "idle": ["…"] },
            "bubble": { "anchor": [0.5, 0.2], "side": "right" },
            "menu": { "side": "left", "align": "head" },
        });
        let imported = Lines::parse(&file, false).unwrap();
        assert_eq!(imported.get(Trigger::Click)[0].text, "Hi!");
        assert_eq!(imported.bubble.anchor, Some((0.5, 0.2)));
        assert_eq!(imported.bubble.side, BubbleSide::Right);
        assert_eq!(imported.menu.align, MenuAlign::Head);
        let mut out = json!({});
        imported.write(&mut out, false);
        assert_eq!(
            Lines::parse(&out, false).unwrap(),
            imported,
            "layout round-trips"
        );

        let mut mine = Lines::default();
        mine.get_mut(Trigger::Click).push(Line {
            text: "Hi!".into(),
            sound: None,
        });
        mine.get_mut(Trigger::Drop).push(Line {
            text: "Oof".into(),
            sound: None,
        });
        let mut added = mine.clone();
        assert_eq!(added.merge(&imported, Merge::Add, false), 0);
        assert_eq!(added.get(Trigger::Click).len(), 2, "duplicates are skipped");
        assert_eq!(added.get(Trigger::Drop).len(), 1, "Add keeps current lines");
        assert_eq!(added.bubble, Bubble::default(), "layout only when asked");
        let mut replaced = mine.clone();
        replaced.merge(&imported, Merge::Replace, true);
        assert!(replaced.get(Trigger::Drop).is_empty());
        assert_eq!(replaced.menu.side, MenuSide::Left);

        let mut full = Lines::default();
        for i in 0..MAX_LINES {
            full.get_mut(Trigger::Idle).push(Line {
                text: format!("{i}"),
                sound: None,
            });
        }
        assert_eq!(full.merge(&imported, Merge::Add, false), 3);
        assert!(Lines::parse(&template(), false).unwrap().total() == Trigger::ALL.len());
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
