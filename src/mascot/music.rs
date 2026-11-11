//! Music response, level stage: turns a stream of playback peak readings
//! into a smooth level the animator can move to, and notices when music
//! starts and stops. No beat detection: the level follows loudness only.
//! Levels are auto-gained against the loudest recent peak, so a quiet app
//! or a microphone moves the character as much as loud playback does.

/// A reading above this starts "playing"; below `STOP` for `HOLD` seconds
/// stops it. The gap stops flicker on quiet passages.
const START: f32 = 0.05;
const STOP: f32 = 0.02;
const HOLD: f32 = 0.8;
/// How quickly the level rises and falls, per second.
const ATTACK: f32 = 30.0;
const RELEASE: f32 = 5.0;
/// Peaks relative to the recent loudest one; a little above 1 so music
/// reaches full movement on its louder moments.
const GAIN: f32 = 1.25;
/// Peaks below this are treated as silence (hum, room noise).
const GATE: f32 = 0.01;
/// The auto-gain never amplifies more than reaching full movement at this
/// peak, so near-silence isn't blown up into motion.
const REFERENCE_FLOOR: f32 = 0.15;
/// How long the loudest recent peak takes to fall to a third.
const REFERENCE_DECAY: f32 = 12.0;
/// Range the meters show, in decibels below full scale.
const METER_DB: f32 = 60.0;

/// How full a meter is for `peak` (0–1), on a decibel scale like a mixer's,
/// so quiet sources still visibly move.
pub fn meter(peak: f32) -> f32 {
    if peak <= 0.0 {
        return 0.0;
    }
    ((20.0 * peak.log10() + METER_DB) / METER_DB).clamp(0.0, 1.0)
}

/// A meter reading that jumps up and falls back gently, for display.
pub fn fall(shown: f32, peak: f32, dt: f32) -> f32 {
    peak.max(shown * (-dt.max(0.0) * 6.0).exp())
}

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
    /// The loudest recent peak, which the auto-gain compares against.
    reference: f32,
}

impl Envelope {
    /// Feeds one peak reading (0–1) taken `dt` seconds after the last.
    /// Returns true when music has just started.
    pub fn step(&mut self, peak: f32, dt: f32) -> bool {
        let peak = peak.clamp(0.0, 1.0);
        self.reference = (self.reference * (-dt.max(0.0) / REFERENCE_DECAY).exp()).max(peak);
        let peak = if peak < GATE {
            0.0
        } else {
            (peak / self.reference.max(REFERENCE_FLOOR) * GAIN).min(1.0)
        };
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

/// The pace music bobs at with no beat to follow.
pub const STEADY_BPM: f32 = 120.0;

/// A beat clock that always runs smoothly: it eases its tempo toward the
/// followed one (or the steady pace) and nudges its phase toward the
/// followed beat, so a tempo change, a re-detected beat or losing the beat
/// never makes the character jump.
#[derive(Debug, Clone, Copy)]
pub struct BeatClock {
    phase: f32,
    bpm: f32,
}

impl Default for BeatClock {
    fn default() -> Self {
        Self {
            phase: 0.0,
            bpm: STEADY_BPM,
        }
    }
}

/// How quickly the tempo eases to a new one, per second (about a second
/// to settle).
const TEMPO_EASE: f32 = 2.5;
/// How quickly phase errors are pulled in, per second.
const PHASE_PULL: f32 = 3.0;

impl BeatClock {
    /// Advances by `dt` seconds toward `target`: the followed beat's phase
    /// (0 on the beat) and tempo, or `None` for the steady pace. Returns the
    /// phase to animate with.
    pub fn step(&mut self, dt: f32, target: Option<(f32, f32)>) -> f32 {
        let dt = dt.clamp(0.0, 0.1);
        let goal = target.map_or(STEADY_BPM, |(_, bpm)| bpm.clamp(40.0, 220.0));
        self.bpm += (goal - self.bpm) * (1.0 - (-dt * TEMPO_EASE).exp());
        self.phase += dt * self.bpm / 60.0;
        if let Some((p, _)) = target {
            // The shortest way round to the followed phase.
            let error = (p - self.phase + 0.5).rem_euclid(1.0) - 0.5;
            self.phase += error * (1.0 - (-dt * PHASE_PULL).exp());
        }
        self.phase = self.phase.rem_euclid(1.0);
        self.phase
    }

    pub fn bpm(&self) -> f32 {
        self.bpm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_beat_clock_eases_tempo_and_never_jumps() {
        let mut c = BeatClock::default();
        let dt = 1.0 / 60.0;
        let mut last = c.step(dt, None);
        let mut followed = 0.0f32;
        let mut biggest = 0.0f32;
        // Steady pace, then a followed beat at 90 BPM out of phase, then
        // the beat is lost.
        for n in 0..1200 {
            let target = (300..900).contains(&n).then(|| {
                followed = (followed + dt * 1.5).rem_euclid(1.0);
                (followed, 90.0)
            });
            let p = c.step(dt, target);
            let moved = (p - last + 0.5).rem_euclid(1.0) - 0.5;
            biggest = biggest.max(moved.abs());
            assert!(moved > -0.01, "never runs backwards much: {moved}");
            last = p;
            if n == 899 {
                assert!((c.bpm() - 90.0).abs() < 1.0, "settled on the tempo");
                let error = (followed - p + 0.5).rem_euclid(1.0) - 0.5;
                assert!(error.abs() < 0.02, "locked to the beat: {error}");
            }
        }
        // At most a normal frame's advance plus a small nudge.
        assert!(biggest < 0.06, "{biggest}");
        assert!(
            (c.bpm() - STEADY_BPM).abs() < 1.0,
            "back to the steady pace"
        );
    }

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
    fn quiet_sources_are_gained_up_but_noise_is_not() {
        // A microphone hearing music around a tenth of full scale moves
        // much more than the raw level would.
        let mut e = Envelope::default();
        for n in 0..100 {
            e.step(if n % 2 == 0 { 0.12 } else { 0.06 }, 0.05);
        }
        assert!(e.playing() && e.level() > 0.6, "{}", e.level());
        // Room hum stays silent.
        let mut e = Envelope::default();
        for _ in 0..100 {
            e.step(0.008, 0.05);
        }
        assert!(!e.playing());
        // The meter is on a decibel scale.
        assert_eq!(meter(1.0), 1.0);
        assert!((meter(0.1) - 2.0 / 3.0).abs() < 0.01);
        assert_eq!(meter(0.0), 0.0);
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
