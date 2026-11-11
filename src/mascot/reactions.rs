//! Character reactions: the typed gesture events a character emits, and the
//! single character voice that decides which of them may play a sound.
//!
//! The desktop and the Settings preview emit the same [`Event`]s. This
//! module owns eligibility, cooldowns, the idle deadline and the current
//! voice; the app routes events here and plays what it is told to. Pointer
//! taps are a different category and share none of this state.

use serde_json::{Map, Value as Json, json};

use super::lines::{SoundRef, valid_pack_sound, valid_user_sound};

/// What happened to the character. Emitted exactly once per gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Event {
    /// A short left press released without crossing the drag threshold.
    Click,
    /// A captured press first crossed the drag threshold.
    Pickup,
    /// A real drag ended on left-button release. Never after a click,
    /// cancellation, hiding or a position reset.
    Drop,
    /// An eligible character reached its idle deadline.
    Idle,
}

impl Event {
    pub const ALL: [Event; 4] = [Event::Click, Event::Pickup, Event::Drop, Event::Idle];

    /// The key in `reactions` (see the behaviors plan).
    pub fn key(self) -> &'static str {
        match self {
            Event::Click => "click",
            Event::Pickup => "pickup",
            Event::Drop => "drop",
            Event::Idle => "idle",
        }
    }

    pub fn parse(key: &str) -> Option<Self> {
        Event::ALL.into_iter().find(|e| e.key() == key)
    }

    fn index(self) -> usize {
        self as usize
    }

}

/// A sound a trigger plays: an exact bundled clip or a managed file.
#[derive(Debug, Clone, PartialEq)]
pub enum Sound {
    /// A bundled clip ID such as `duck-toy`; never the pointer's `pack` or
    /// `custom` aliases.
    Builtin(&'static str),
    /// A managed WAV, relative to the pack or the character's overrides.
    File(SoundRef),
}

/// One trigger's sound setup.
#[derive(Debug, Clone, PartialEq)]
pub struct TriggerSound {
    pub enabled: bool,
    pub sound: Option<Sound>,
    /// 0–1, multiplied by the character volume.
    pub gain: f32,
    pub cooldown_ms: u32,
}

impl Default for TriggerSound {
    fn default() -> Self {
        Self {
            enabled: false,
            sound: None,
            gain: 1.0,
            cooldown_ms: DEFAULT_COOLDOWN_MS,
        }
    }
}

pub const DEFAULT_COOLDOWN_MS: u32 = 250;
pub const MAX_COOLDOWN_MS: u32 = 10_000;
pub const DEFAULT_VOLUME: f32 = 0.7;
/// Idle sounds come at a random time in this range, in seconds.
pub const IDLE_INTERVAL: (f32, f32) = (60.0, 300.0);

/// A character's reaction sounds. With nothing assigned it stays silent.
#[derive(Debug, Clone, PartialEq)]
pub struct Sounds {
    pub enabled: bool,
    pub volume: f32,
    /// Automatic idle sounds: off until this user opts in.
    pub idle_enabled: bool,
    pub idle_interval: (f32, f32),
    triggers: [TriggerSound; 4],
}

impl Default for Sounds {
    fn default() -> Self {
        Self {
            enabled: true,
            volume: DEFAULT_VOLUME,
            idle_enabled: false,
            idle_interval: IDLE_INTERVAL,
            triggers: Default::default(),
        }
    }
}

impl Sounds {
    pub fn get(&self, e: Event) -> &TriggerSound {
        &self.triggers[e.index()]
    }

    pub fn get_mut(&mut self, e: Event) -> &mut TriggerSound {
        &mut self.triggers[e.index()]
    }

}

/// The bundled clips a character may use, by exact ID.
pub const BUILTIN_SOUNDS: [(&str, &str); 4] = [
    ("duck-toy", "Duck Toy"),
    ("mac-quack", "Mac Quack"),
    ("bonk-clip", "Bonk"),
    ("dry-fart", "Dry Fart"),
];

/// Most distinct WAV files one character's reactions may use.
pub const MAX_FILES: usize = 4;

/// What an event does to the picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visual {
    None,
    /// Briefly shortens and widens the picture, then restores it.
    Compress,
    /// Shows one alternate whole-character PNG briefly (the entry's `art`).
    Artwork,
}

impl Visual {
    pub fn name(self) -> &'static str {
        match self {
            Visual::None => "none",
            Visual::Compress => "compress",
            Visual::Artwork => "artwork",
        }
    }
}

impl std::fmt::Display for Visual {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Visual::None => "None",
            Visual::Compress => "Compress",
            Visual::Artwork => "Use artwork",
        })
    }
}

/// A whole-character replacement picture on the character's own stage:
/// a canvas-sized PNG and its alignment, `[x, y, scale]` with the offset
/// in canvas fractions, so the creator can line poses up.
#[derive(Debug, Clone, PartialEq)]
pub struct Art {
    pub image: SoundRef,
    pub place: [f32; 3],
}

impl Art {
    pub fn new(image: SoundRef) -> Self {
        Self { image, place: [0.0, 0.0, 1.0] }
    }

    fn to_json(&self) -> Json {
        json!({ "image": file_json(&self.image), "place": self.place })
    }
}

fn file_json(r: &SoundRef) -> String {
    match r {
        SoundRef::Pack(p) => p.clone(),
        SoundRef::User(f) => format!("user:{f}"),
    }
}

/// A PNG path inside a pack: relative, `/`-separated, no `..`.
pub fn valid_pack_image(path: &str) -> bool {
    let wav = format!("{}.wav", path.strip_suffix(".png").unwrap_or("x"));
    path.to_ascii_lowercase().ends_with(".png") && valid_pack_sound(&wav)
}

/// A file name TinyPaw gave an imported reaction picture.
pub fn valid_user_image(file: &str) -> bool {
    file.strip_suffix(".png").is_some_and(|stem| valid_user_sound(&format!("{stem}.wav")))
}

fn parse_art(v: &Json, user: bool) -> Result<Art, String> {
    let image = v["image"].as_str().ok_or("a reaction picture needs an image")?;
    let image = match image.strip_prefix("user:") {
        Some(file) if user && valid_user_image(file) => SoundRef::User(file.into()),
        Some(_) => return Err(format!("`{image}` isn't a picture in this pack")),
        None if valid_pack_image(image) => SoundRef::Pack(image.into()),
        None => return Err(format!("reaction picture `{image}` must be a PNG inside the pack folder")),
    };
    let place = match v["place"].as_array().map(Vec::as_slice) {
        None if v["place"].is_null() => [0.0, 0.0, 1.0],
        Some([x, y, k]) => {
            let n = |j: &Json| j.as_f64().filter(|n| n.is_finite()).map(|n| n as f32);
            match (n(x), n(y), n(k)) {
                (Some(x), Some(y), Some(k)) if x.abs() <= 1.0 && y.abs() <= 1.0 && (0.1..=4.0).contains(&k) => [x, y, k],
                _ => return Err("a reaction picture's place must be [x, y, scale] within the stage".into()),
            }
        }
        _ => return Err("a reaction picture's place must be [x, y, scale]".into()),
    };
    Ok(Art { image, place })
}

/// One event's entry in a `reactions` object: its sound and visual, plus
/// keys this version doesn't edit, kept as they were.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Entry {
    pub sound: TriggerSound,
    /// `None` when this layer doesn't set it.
    pub visual: Option<Visual>,
    /// The picture a `Visual::Artwork` shows.
    pub art: Option<Art>,
    /// Pick up only: the whole-character picture shown while carried.
    pub held: Option<Art>,
    /// Drop only: `Some(true)` falls to the screen edge after release,
    /// `Some(false)` stays where it was put.
    pub fall: Option<bool>,
    pub other: Map<String, Json>,
}

impl Entry {
    /// A new character's explicit "nothing": no visual, no sound.
    pub fn none() -> Self {
        Self { visual: Some(Visual::None), ..Self::default() }
    }
}

/// Character-wide sound settings, `reactions.sound`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    pub enabled: bool,
    pub volume: f32,
    pub idle_enabled: bool,
    pub idle_interval: (f32, f32),
}

impl Default for Settings {
    fn default() -> Self {
        let d = Sounds::default();
        Self {
            enabled: d.enabled,
            volume: d.volume,
            idle_enabled: d.idle_enabled,
            idle_interval: d.idle_interval,
        }
    }
}

/// One layer of a `reactions` object: the pack's defaults in
/// `character.json`, or the user's changes in overrides. An event entry
/// replaces the layer below it as a whole.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Config {
    pub settings: Option<Settings>,
    events: [Option<Entry>; 4],
}

impl Config {
    pub fn is_empty(&self) -> bool {
        self.settings.is_none() && self.events.iter().all(Option::is_none)
    }

    pub fn entry(&self, e: Event) -> Option<&Entry> {
        self.events[e.index()].as_ref()
    }

    pub fn entry_mut(&mut self, e: Event) -> Option<&mut Entry> {
        self.events[e.index()].as_mut()
    }

    pub fn set_entry(&mut self, e: Event, entry: Option<Entry>) {
        self.events[e.index()] = entry;
    }

    /// Every WAV this layer names.
    pub fn files(&self) -> Vec<&SoundRef> {
        let mut out: Vec<&SoundRef> = Vec::new();
        for entry in self.events.iter().flatten() {
            if let Some(Sound::File(r)) = &entry.sound.sound
                && !out.contains(&r)
            {
                out.push(r);
            }
        }
        out
    }

    /// Every reaction picture this layer names.
    pub fn images(&self) -> Vec<&SoundRef> {
        let mut out: Vec<&SoundRef> = Vec::new();
        for entry in self.events.iter().flatten() {
            for art in [&entry.art, &entry.held].into_iter().flatten() {
                if !out.contains(&&art.image) {
                    out.push(&art.image);
                }
            }
        }
        out
    }

    /// Rewrites every picture reference, for export.
    pub fn map_images(&mut self, mut f: impl FnMut(&SoundRef) -> SoundRef) {
        for entry in self.events.iter_mut().flatten() {
            for art in [&mut entry.art, &mut entry.held].into_iter().flatten() {
                art.image = f(&art.image);
            }
        }
    }

    /// Rewrites every file reference, for export.
    pub fn map_files(&mut self, mut f: impl FnMut(&SoundRef) -> SoundRef) {
        for entry in self.events.iter_mut().flatten() {
            if let Some(Sound::File(r)) = &entry.sound.sound {
                entry.sound.sound = Some(Sound::File(f(r)));
            }
        }
    }

    /// Reads a `reactions` object. `user` allows `user:` files (overrides
    /// only). A missing object is an empty layer; anything malformed is an
    /// error, so a pack never half-loads its sounds.
    pub fn parse(v: &Json, user: bool) -> Result<Self, String> {
        let mut out = Self::default();
        let map = match v {
            Json::Null => return Ok(out),
            Json::Object(map) => map,
            _ => return Err("reactions must be an object".into()),
        };
        for (key, value) in map {
            if key == "sound" {
                out.settings = Some(parse_settings(value)?);
                continue;
            }
            let e = Event::parse(key).ok_or_else(|| format!("unknown reaction `{key}`"))?;
            out.events[e.index()] = Some(parse_entry(value, user)?);
        }
        if out.files().len() > MAX_FILES {
            return Err(format!("reactions may use at most {MAX_FILES} WAV files"));
        }
        Ok(out)
    }

    pub fn to_json(&self) -> Json {
        let mut m = Map::new();
        if let Some(s) = &self.settings {
            m.insert(
                "sound".into(),
                json!({
                    "enabled": s.enabled,
                    "volume": s.volume,
                    "idle_enabled": s.idle_enabled,
                    "idle_interval_seconds": [s.idle_interval.0, s.idle_interval.1],
                }),
            );
        }
        for e in Event::ALL {
            if let Some(entry) = &self.events[e.index()] {
                let mut v = entry.other.clone();
                match (entry.visual, &entry.art) {
                    (Some(Visual::Artwork), Some(art)) => {
                        v.insert("visual".into(), art.to_json());
                    }
                    (Some(Visual::Artwork), None) => {
                        v.insert("visual".into(), json!("none"));
                    }
                    (Some(visual), _) => {
                        v.insert("visual".into(), json!(visual.name()));
                    }
                    (None, _) => {}
                }
                if let Some(held) = &entry.held {
                    v.insert("held_pose".into(), held.to_json());
                }
                if let Some(fall) = entry.fall {
                    v.insert("after_release".into(), json!(if fall { "fall" } else { "stay" }));
                }
                let t = &entry.sound;
                v.insert("enabled".into(), json!(t.enabled));
                match &t.sound {
                    Some(Sound::Builtin(id)) => {
                        v.insert("sound".into(), json!({ "builtin": id }));
                    }
                    Some(Sound::File(SoundRef::Pack(p))) => {
                        v.insert("sound".into(), json!({ "file": p }));
                    }
                    Some(Sound::File(SoundRef::User(f))) => {
                        v.insert("sound".into(), json!({ "file": format!("user:{f}") }));
                    }
                    None => {
                        v.remove("sound");
                    }
                }
                v.insert("gain".into(), json!(t.gain));
                v.insert("cooldown_ms".into(), json!(t.cooldown_ms));
                m.insert(e.key().into(), Json::Object(v));
            }
        }
        Json::Object(m)
    }
}

fn parse_settings(v: &Json) -> Result<Settings, String> {
    let d = Settings::default();
    let unit = |key: &str, default: f32| -> Result<f32, String> {
        match v[key].as_f64() {
            None if v[key].is_null() => Ok(default),
            Some(n) if n.is_finite() && (0.0..=1.0).contains(&n) => Ok(n as f32),
            _ => Err(format!("reactions.sound.{key} must be 0–1")),
        }
    };
    let flag = |key: &str, default: bool| -> Result<bool, String> {
        match &v[key] {
            Json::Null => Ok(default),
            Json::Bool(b) => Ok(*b),
            _ => Err(format!("reactions.sound.{key} must be true or false")),
        }
    };
    let idle_interval = match v["idle_interval_seconds"].as_array().map(Vec::as_slice) {
        None if v["idle_interval_seconds"].is_null() => d.idle_interval,
        Some([lo, hi]) => {
            let (lo, hi) = (lo.as_f64().unwrap_or(-1.0), hi.as_f64().unwrap_or(-1.0));
            if !(1.0..=3600.0).contains(&lo) || !(1.0..=3600.0).contains(&hi) || lo > hi {
                return Err("idle_interval_seconds must be [low, high] within 1–3600".into());
            }
            (lo as f32, hi as f32)
        }
        _ => return Err("idle_interval_seconds must be [low, high]".into()),
    };
    Ok(Settings {
        enabled: flag("enabled", d.enabled)?,
        volume: unit("volume", d.volume)?,
        idle_enabled: flag("idle_enabled", d.idle_enabled)?,
        idle_interval,
    })
}

fn parse_entry(v: &Json, user: bool) -> Result<Entry, String> {
    let Json::Object(map) = v else {
        return Err("each reaction must be an object".into());
    };
    let sound = match &v["sound"] {
        Json::Null => None,
        s => Some(parse_sound(s, user)?),
    };
    let enabled = match &v["enabled"] {
        Json::Null => sound.is_some(),
        Json::Bool(b) => *b,
        _ => return Err("a reaction's enabled must be true or false".into()),
    };
    let gain = match v["gain"].as_f64() {
        None if v["gain"].is_null() => 1.0,
        Some(n) if n.is_finite() && (0.0..=1.0).contains(&n) => n as f32,
        _ => return Err("a reaction's gain must be 0–1".into()),
    };
    let cooldown_ms = match v["cooldown_ms"].as_u64() {
        None if v["cooldown_ms"].is_null() => DEFAULT_COOLDOWN_MS,
        Some(n) if n <= u64::from(MAX_COOLDOWN_MS) => n as u32,
        _ => return Err(format!("a reaction's cooldown_ms must be 0–{MAX_COOLDOWN_MS}")),
    };
    let (visual, art) = match &v["visual"] {
        Json::Null => (None, None),
        Json::String(name) => (
            Some(match name.as_str() {
                "none" => Visual::None,
                "compress" => Visual::Compress,
                other => return Err(format!("unknown reaction visual `{other}`")),
            }),
            None,
        ),
        o @ Json::Object(_) => (Some(Visual::Artwork), Some(parse_art(o, user)?)),
        _ => return Err("a reaction's visual must be a name or a picture".into()),
    };
    let held = match &v["held_pose"] {
        Json::Null => None,
        o => Some(parse_art(o, user)?),
    };
    let fall = match v["after_release"].as_str() {
        None if v["after_release"].is_null() => None,
        Some("stay") => Some(false),
        Some("fall") => Some(true),
        _ => return Err("after_release must be stay or fall".into()),
    };
    let mut other = map.clone();
    for key in ["enabled", "sound", "gain", "cooldown_ms", "visual", "held_pose", "after_release"] {
        other.remove(key);
    }
    Ok(Entry {
        visual,
        art,
        held,
        fall,
        sound: TriggerSound {
            // Enabled without a sound plays nothing; it reads as off.
            enabled: enabled && sound.is_some(),
            sound,
            gain,
            cooldown_ms,
        },
        other,
    })
}

fn parse_sound(v: &Json, user: bool) -> Result<Sound, String> {
    match (v["builtin"].as_str(), v["file"].as_str()) {
        (Some(id), None) => BUILTIN_SOUNDS
            .iter()
            .find(|(b, _)| *b == id)
            .map(|(b, _)| Sound::Builtin(b))
            .ok_or_else(|| format!("unknown bundled sound `{id}`")),
        (None, Some(f)) => match f.strip_prefix("user:") {
            Some(file) if user && valid_user_sound(file) => {
                Ok(Sound::File(SoundRef::User(file.into())))
            }
            Some(_) => Err(format!("`{f}` isn't a sound in this pack")),
            None if valid_pack_sound(f) => Ok(Sound::File(SoundRef::Pack(f.into()))),
            None => Err(format!("reaction sound `{f}` must be a WAV inside the pack folder")),
        },
        _ => Err("a reaction sound names exactly one of builtin or file".into()),
    }
}

/// What `e` does to the picture: the user's choice, else the pack's. With
/// neither, a click keeps the legacy built-in squash (now a Compress after
/// release, so it never moves a held point) and other events do nothing.
pub fn visual(pack: &Config, user: &Config, e: Event) -> Visual {
    let set = |c: &Config| c.entry(e).and_then(|x| x.visual);
    set(user).or(set(pack)).unwrap_or(if e == Event::Click {
        Visual::Compress
    } else {
        Visual::None
    })
}

/// The picture `e` shows, when its visual is Use artwork.
pub fn art(pack: &Config, user: &Config, e: Event) -> Option<Art> {
    if visual(pack, user, e) != Visual::Artwork {
        return None;
    }
    let set = |c: &Config| c.entry(e).filter(|x| x.visual.is_some()).and_then(|x| x.art.clone());
    set(user).or_else(|| set(pack))
}

/// Whether a released character falls to the screen edge. Off unless set.
pub fn falls(pack: &Config, user: &Config) -> bool {
    user.entry(Event::Drop)
        .or(pack.entry(Event::Drop))
        .and_then(|e| e.fall)
        .unwrap_or(false)
}

/// The held pose while carried, if any: the user's Pick up entry replaces
/// the pack's as a whole.
pub fn held(pack: &Config, user: &Config) -> Option<Art> {
    user.entry(Event::Pickup)
        .or(pack.entry(Event::Pickup))
        .and_then(|e| e.held.clone())
}

/// The explicit defaults a new character is written with: None for each
/// touch event, so nothing depends on a missing field.
pub fn new_character() -> Config {
    let mut c = Config::default();
    for e in [Event::Click, Event::Pickup, Event::Drop] {
        c.set_entry(e, Some(Entry::none()));
    }
    c
}

/// What a character actually does: the pack's layer under the user's.
/// The pack never turns idle sounds on; only this user can.
pub fn effective(pack: &Config, user: &Config) -> Sounds {
    let mut out = Sounds::default();
    if let Some(s) = pack.settings {
        out.enabled = s.enabled;
        out.volume = s.volume;
        out.idle_interval = s.idle_interval;
    }
    if let Some(s) = user.settings {
        out.enabled = s.enabled;
        out.volume = s.volume;
        out.idle_enabled = s.idle_enabled;
        out.idle_interval = s.idle_interval;
    }
    for e in Event::ALL {
        if let Some(entry) = user.entry(e).or(pack.entry(e)) {
            *out.get_mut(e) = entry.sound.clone();
        }
    }
    out
}

/// The user's global and situational quiet rules, read when an event fires.
#[derive(Debug, Clone, Copy, Default)]
pub struct Quiet {
    /// Mute all sounds.
    pub mute_all: bool,
    /// Annotation owns input.
    pub annotating: bool,
    pub hidden: bool,
}

/// Why an event did not play.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skip {
    Muted,
    CharacterOff,
    /// The trigger is disabled or has no sound.
    Unassigned,
    Cooldown,
    /// Idle never interrupts another voice.
    Busy,
}

/// What to play for an accepted event.
#[derive(Debug, Clone, PartialEq)]
pub struct Play {
    pub sound: Sound,
    /// Final gain: character volume × trigger gain.
    pub volume: f32,
    /// An interactive event replaces the current voice; stop it first.
    pub interrupt: bool,
}

/// The one character voice: cooldowns, idle deadline and what is playing.
/// Times are seconds on a monotonic clock the caller supplies.
#[derive(Debug, Clone, Default)]
pub struct Voice {
    /// When each trigger last played, for its cooldown.
    last: [Option<f64>; 4],
    /// Until when the current voice plays.
    busy_until: f64,
    next_idle: Option<f64>,
}

impl Voice {
    /// Decides whether `e` plays now. Cooldown starts only once playback is
    /// accepted; muted, unassigned, suppressed or rejected events don't
    /// consume it.
    pub fn accept(&mut self, e: Event, sounds: &Sounds, quiet: Quiet, now: f64) -> Result<Play, Skip> {
        if quiet.mute_all || quiet.annotating || quiet.hidden {
            return Err(Skip::Muted);
        }
        if !sounds.enabled {
            return Err(Skip::CharacterOff);
        }
        let t = sounds.get(e);
        let Some(sound) = t.sound.clone().filter(|_| t.enabled) else {
            return Err(Skip::Unassigned);
        };
        if e == Event::Idle && !sounds.idle_enabled {
            return Err(Skip::Unassigned);
        }
        if let Some(at) = self.last[e.index()]
            // In whole milliseconds, so a boundary isn't lost to rounding.
            && ((now - at) * 1000.0 + 1e-6) < f64::from(t.cooldown_ms.min(MAX_COOLDOWN_MS))
        {
            return Err(Skip::Cooldown);
        }
        let busy = now < self.busy_until;
        if e == Event::Idle && busy {
            return Err(Skip::Busy);
        }
        self.last[e.index()] = Some(now);
        Ok(Play {
            sound,
            volume: (sounds.volume * t.gain).clamp(0.0, 1.0),
            interrupt: busy,
        })
    }

    /// The accepted sound runs `seconds`; idle waits for it.
    pub fn started(&mut self, now: f64, seconds: f32) {
        self.busy_until = now + f64::from(seconds.max(0.0));
    }

    /// The voice was stopped (hidden, switched, annotation started).
    /// Pending idle work is dropped; resuming schedules a fresh interval,
    /// so missed events never play in a burst.
    pub fn stop(&mut self) {
        self.busy_until = 0.0;
        self.next_idle = None;
    }

    /// Whether an idle event is due. Schedules the first deadline when
    /// idle sounds become eligible, and nothing at all while they aren't.
    /// `pick` returns 0–1 for the random interval.
    pub fn idle_due(&mut self, sounds: &Sounds, eligible: bool, now: f64, pick: impl FnOnce() -> f32) -> bool {
        let wanted = eligible && sounds.idle_enabled && sounds.enabled && sounds.get(Event::Idle).sound.is_some();
        if !wanted {
            self.next_idle = None;
            return false;
        }
        match self.next_idle {
            None => {
                let (lo, hi) = sounds.idle_interval;
                let (lo, hi) = (lo.min(hi), lo.max(hi));
                self.next_idle = Some(now + f64::from(lo + (hi - lo) * pick().clamp(0.0, 1.0)));
                false
            }
            Some(at) if now >= at => {
                self.next_idle = None;
                true
            }
            Some(_) => false,
        }
    }

    /// When the next idle event is due, so the caller can wake for it
    /// instead of polling every frame.
    #[cfg(test)]
    pub fn next_idle(&self) -> Option<f64> {
        self.next_idle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assigned() -> Sounds {
        let mut s = Sounds::default();
        for e in Event::ALL {
            *s.get_mut(e) = TriggerSound {
                enabled: true,
                sound: Some(Sound::Builtin("duck-toy")),
                gain: 0.5,
                cooldown_ms: 250,
            };
        }
        s.idle_enabled = true;
        s
    }

    #[test]
    fn nothing_assigned_is_silent() {
        let mut v = Voice::default();
        for e in Event::ALL {
            assert_eq!(v.accept(e, &Sounds::default(), Quiet::default(), 1.0), Err(Skip::Unassigned));
        }
    }

    #[test]
    fn cooldown_starts_only_once_playback_is_accepted() {
        let s = assigned();
        let mut v = Voice::default();
        // A muted click does not start the cooldown.
        let muted = Quiet { mute_all: true, ..Quiet::default() };
        assert_eq!(v.accept(Event::Click, &s, muted, 0.0), Err(Skip::Muted));
        let play = v.accept(Event::Click, &s, Quiet::default(), 0.1).unwrap();
        assert_eq!(play.volume, DEFAULT_VOLUME * 0.5);
        // Boundaries: just inside the cooldown is discarded, at it plays.
        assert_eq!(v.accept(Event::Click, &s, Quiet::default(), 0.349), Err(Skip::Cooldown));
        assert!(v.accept(Event::Click, &s, Quiet::default(), 0.35).is_ok());
        // Cooldowns are per trigger.
        assert!(v.accept(Event::Pickup, &s, Quiet::default(), 0.36).is_ok());
    }

    #[test]
    fn interactive_events_replace_the_voice_and_idle_never_interrupts() {
        let s = assigned();
        let mut v = Voice::default();
        let first = v.accept(Event::Click, &s, Quiet::default(), 0.0).unwrap();
        assert!(!first.interrupt);
        v.started(0.0, 2.0);
        assert_eq!(v.accept(Event::Idle, &s, Quiet::default(), 1.0), Err(Skip::Busy));
        let pickup = v.accept(Event::Pickup, &s, Quiet::default(), 1.0).unwrap();
        assert!(pickup.interrupt, "stops the old voice first");
        // Rapid clicks never pile up: each is accepted or discarded now.
        let mut accepted = 0;
        for i in 0..40 {
            if v.accept(Event::Click, &s, Quiet::default(), 3.0 + f64::from(i) * 0.05).is_ok() {
                accepted += 1;
            }
        }
        assert!(accepted <= 8, "{accepted}");
    }

    #[test]
    fn quiet_rules_and_the_character_switch() {
        let mut s = assigned();
        let mut v = Voice::default();
        for q in [
            Quiet { annotating: true, ..Quiet::default() },
            Quiet { hidden: true, ..Quiet::default() },
        ] {
            assert_eq!(v.accept(Event::Click, &s, q, 0.0), Err(Skip::Muted));
        }
        s.enabled = false;
        assert_eq!(v.accept(Event::Click, &s, Quiet::default(), 0.0), Err(Skip::CharacterOff));
        s.enabled = true;
        s.get_mut(Event::Drop).enabled = false;
        assert_eq!(v.accept(Event::Drop, &s, Quiet::default(), 0.0), Err(Skip::Unassigned));
    }

    #[test]
    fn idle_schedules_lazily_and_never_bursts() {
        let s = assigned();
        let mut v = Voice::default();
        // Not eligible: no deadline at all.
        assert!(!v.idle_due(&s, false, 0.0, || 0.0));
        assert_eq!(v.next_idle(), None);
        // Eligible: first schedules, then fires once at the deadline.
        assert!(!v.idle_due(&s, true, 0.0, || 0.0));
        assert_eq!(v.next_idle(), Some(60.0));
        assert!(!v.idle_due(&s, true, 59.0, || 0.0));
        assert!(v.idle_due(&s, true, 60.0, || 0.0));
        // Hidden for an hour, then back: a fresh interval, no catch-up.
        v.stop();
        assert!(!v.idle_due(&s, true, 3660.0, || 1.0));
        assert_eq!(v.next_idle(), Some(3960.0));
        // Off by default.
        let mut off = assigned();
        off.idle_enabled = false;
        assert!(!v.idle_due(&off, true, 5000.0, || 0.0));
        assert_eq!(v.next_idle(), None);
    }

    #[test]
    fn reactions_round_trip_and_keep_keys_they_dont_edit() {
        let v = json!({
            "sound": { "enabled": true, "volume": 0.5, "idle_enabled": true, "idle_interval_seconds": [30, 90] },
            "click": { "visual": "compress", "sound": { "builtin": "duck-toy" }, "gain": 0.8, "cooldown_ms": 300 },
            "drop": { "sound": { "file": "user:reaction-1.wav" } },
            "idle": { "enabled": false }
        });
        let c = Config::parse(&v, true).unwrap();
        assert_eq!(c.entry(Event::Click).unwrap().visual, Some(Visual::Compress));
        assert!(c.entry(Event::Drop).unwrap().sound.enabled, "a sound with no flag is on");
        assert_eq!(c.files().len(), 1);
        let again = Config::parse(&c.to_json(), true).unwrap();
        assert_eq!(again, c);
        // Pack files can't name user sounds.
        assert!(Config::parse(&v, false).is_err());
    }

    #[test]
    fn malformed_reactions_are_rejected() {
        for bad in [
            json!({ "wave": {} }),
            json!({ "click": { "sound": { "builtin": "pack" } } }),
            json!({ "click": { "sound": { "builtin": "duck-toy", "file": "a.wav" } } }),
            json!({ "click": { "sound": { "file": "../x.wav" } } }),
            json!({ "click": { "sound": { "builtin": "duck-toy" }, "gain": 2.0 } }),
            json!({ "click": { "sound": { "builtin": "duck-toy" }, "cooldown_ms": 20000 } }),
            json!({ "sound": { "idle_interval_seconds": [300, 60] } }),
        ] {
            assert!(Config::parse(&bad, true).is_err(), "{bad}");
        }
        let four = json!({
            "click": { "sound": { "file": "a.wav" } }, "pickup": { "sound": { "file": "b.wav" } },
            "drop": { "sound": { "file": "c.wav" } }, "idle": { "sound": { "file": "d.wav" } },
        });
        assert!(Config::parse(&four, false).is_ok(), "four files is the limit");
    }

    #[test]
    fn the_users_layer_wins_and_the_pack_never_enables_idle() {
        let pack = Config::parse(&json!({
            "sound": { "volume": 0.4, "idle_enabled": true },
            "click": { "sound": { "builtin": "duck-toy" } },
            "idle": { "sound": { "builtin": "mac-quack" } }
        }), false).unwrap();
        let s = effective(&pack, &Config::default());
        assert_eq!((s.volume, s.idle_enabled), (0.4, false));
        assert_eq!(s.get(Event::Click).sound, Some(Sound::Builtin("duck-toy")));
        let user = Config::parse(&json!({ "click": { "enabled": false } }), true).unwrap();
        let s = effective(&pack, &user);
        assert!(!s.get(Event::Click).enabled, "the user's entry replaces the pack's");
        assert_eq!(s.get(Event::Idle).sound, Some(Sound::Builtin("mac-quack")));
    }

    #[test]
    fn visuals_default_to_legacy_only_when_unset() {
        let empty = Config::default();
        assert_eq!(visual(&empty, &empty, Event::Click), Visual::Compress, "legacy squash");
        assert_eq!(visual(&empty, &empty, Event::Drop), Visual::None);
        let fresh = new_character();
        assert_eq!(visual(&fresh, &empty, Event::Click), Visual::None);
        let again = Config::parse(&fresh.to_json(), false).unwrap();
        assert_eq!(again, fresh, "explicit defaults round-trip");
        let user = Config::parse(&json!({ "click": { "visual": "compress" } }), true).unwrap();
        assert_eq!(visual(&fresh, &user, Event::Click), Visual::Compress);
        assert!(Config::parse(&json!({ "click": { "visual": "wobble" } }), true).is_err());
    }

    #[test]
    fn held_poses_and_artwork_round_trip() {
        let v = json!({
            "pickup": { "visual": "none", "held_pose": { "image": "reactions/held.png", "place": [0.02, -0.04, 1.0] } },
            "drop": { "visual": { "image": "user:art-1.png", "place": [0.0, 0.0, 1.0] }, "after_release": "stay" }
        });
        let c = Config::parse(&v, true).unwrap();
        assert_eq!(held(&Config::default(), &c).unwrap().place, [0.02, -0.04, 1.0]);
        assert_eq!(visual(&Config::default(), &c, Event::Drop), Visual::Artwork);
        assert!(art(&Config::default(), &c, Event::Drop).is_some());
        assert_eq!(c.images().len(), 2);
        assert_eq!(c.entry(Event::Drop).unwrap().fall, Some(false));
        assert!(!falls(&Config::default(), &c));
        assert!(Config::parse(&json!({ "drop": { "after_release": "bounce" } }), true).is_err());
        assert_eq!(Config::parse(&c.to_json(), true).unwrap(), c);
        for bad in [
            json!({ "pickup": { "held_pose": { "image": "../x.png" } } }),
            json!({ "pickup": { "held_pose": { "image": "held.png", "place": [0, 0, 9] } } }),
            json!({ "drop": { "visual": { "image": "x.wav" } } }),
        ] {
            assert!(Config::parse(&bad, true).is_err(), "{bad}");
        }
    }

    #[test]
    fn event_keys_round_trip() {
        for e in Event::ALL {
            assert_eq!(Event::parse(e.key()), Some(e));
        }
    }
}
