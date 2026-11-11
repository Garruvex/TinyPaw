//! Music response, level stage: turns a stream of playback peak readings
//! into a smooth level the animator can move to, and notices when music
//! starts and stops. No beat detection: the level follows loudness only.

/// A reading above this starts "playing"; below `STOP` for `HOLD` seconds
/// stops it. The gap stops flicker on quiet passages.
const START: f32 = 0.05;
const STOP: f32 = 0.02;
const HOLD: f32 = 0.8;
/// How quickly the level rises and falls, per second.
const ATTACK: f32 = 30.0;
const RELEASE: f32 = 5.0;
/// Typical music peaks sit well below full scale; this maps them to 0–1.
const GAIN: f32 = 1.6;

/// Longest saved device name, so `in:<name>` fits a choice setting.
const MAX_NAME: usize = 60;

/// What music response listens to.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Source {
    /// Every other app on the default output, added together.
    #[default]
    All,
    /// One app, by its program file name (`Spotify.exe`), matched without
    /// regard to case so it survives restarts.
    App(String),
    /// An input device (a microphone or line-in), by its name.
    Input(String),
}

impl Source {
    /// Reads the setting's value; anything unknown listens to all apps.
    pub fn parse(text: &str) -> Self {
        if let Some(exe) = text.strip_prefix("app:").filter(|s| !s.is_empty()) {
            Source::App(exe.to_owned())
        } else if let Some(name) = text.strip_prefix("in:").filter(|s| !s.is_empty()) {
            Source::Input(name.to_owned())
        } else {
            Source::All
        }
    }

    /// An input device source. Long names are cut so the setting stays
    /// short; the device is then found by the start of its name.
    pub fn input(name: &str) -> Self {
        Source::Input(name.chars().take(MAX_NAME).collect())
    }

    /// The setting's value for this source.
    pub fn id(&self) -> String {
        match self {
            Source::All => "all".into(),
            Source::App(exe) => format!("app:{exe}"),
            Source::Input(name) => format!("in:{name}"),
        }
    }

    /// Whether `exe` is this app.
    pub fn is_app(&self, exe: &str) -> bool {
        matches!(self, Source::App(a) if a.eq_ignore_ascii_case(exe))
    }
}

/// An app's name for people: its file name without `.exe`.
pub fn app_name(exe: &str) -> &str {
    let n = exe.len();
    if n > 4 && exe[n - 4..].eq_ignore_ascii_case(".exe") {
        &exe[..n - 4]
    } else {
        exe
    }
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Source::All => f.write_str("All apps"),
            Source::App(exe) => f.write_str(app_name(exe)),
            Source::Input(name) => write!(f, "Input: {name}"),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Envelope {
    level: f32,
    playing: bool,
    quiet_for: f32,
}

impl Envelope {
    /// Feeds one peak reading (0–1) taken `dt` seconds after the last.
    /// Returns true when music has just started.
    pub fn step(&mut self, peak: f32, dt: f32) -> bool {
        let peak = (peak.clamp(0.0, 1.0) * GAIN).min(1.0);
        let rate = if peak > self.level { ATTACK } else { RELEASE };
        self.level += (peak - self.level) * (1.0 - (-dt.max(0.0) * rate).exp());
        let was = self.playing;
        if self.level > START {
            self.playing = true;
            self.quiet_for = 0.0;
        } else if self.level < STOP {
            self.quiet_for += dt;
            if self.quiet_for >= HOLD {
                self.playing = false;
            }
        }
        self.playing && !was
    }

    /// The level to move to: zero once the music has stopped.
    pub fn level(&self) -> f32 {
        if self.playing { self.level } else { 0.0 }
    }

    pub fn playing(&self) -> bool {
        self.playing
    }

    /// A device change: start again from silence.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_round_trip_and_match_apps_without_case() {
        for s in [
            Source::All,
            Source::App("Spotify.exe".into()),
            Source::Input("Microphone (USB Audio)".into()),
        ] {
            assert_eq!(Source::parse(&s.id()), s);
        }
        assert_eq!(Source::parse("app:"), Source::All);
        assert_eq!(Source::parse("bogus"), Source::All);
        let spotify = Source::App("Spotify.exe".into());
        assert!(spotify.is_app("spotify.EXE") && !spotify.is_app("chrome.exe"));
        assert_eq!(spotify.to_string(), "Spotify");
        assert_eq!(app_name("foobar2000.EXE"), "foobar2000");
    }

    #[test]
    fn music_starts_follows_and_settles() {
        let mut e = Envelope::default();
        let mut started = 0;
        // Silence stays silent.
        for _ in 0..20 {
            started += e.step(0.0, 0.05) as u32;
        }
        assert_eq!((started, e.level()), (0, 0.0));
        // Music: starts once, and the level rises quickly.
        for _ in 0..10 {
            started += e.step(0.5, 0.05) as u32;
        }
        assert_eq!(started, 1);
        assert!(e.playing() && e.level() > 0.6);
        // A short quiet gap doesn't stop it.
        for _ in 0..4 {
            e.step(0.0, 0.05);
        }
        assert!(e.playing());
        for _ in 0..4 {
            started += e.step(0.5, 0.05) as u32;
        }
        assert_eq!(started, 1, "no second start after a pause");
        // Real silence stops it and the level reads zero.
        for _ in 0..60 {
            e.step(0.0, 0.05);
        }
        assert!(!e.playing() && e.level() == 0.0);
    }

    #[test]
    fn a_device_change_resets() {
        let mut e = Envelope::default();
        for _ in 0..10 {
            e.step(0.8, 0.05);
        }
        e.reset();
        assert!(!e.playing() && e.level() == 0.0);
    }
}
