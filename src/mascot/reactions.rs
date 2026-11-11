//! Character reactions: the typed gesture events a character emits, and the
//! single character voice that decides which of them may play a sound.
//!
//! The desktop and the Settings preview emit the same [`Event`]s. This
//! module owns eligibility, cooldowns, the idle deadline and the current
//! voice; the app routes events here and plays what it is told to. Pointer
//! taps are a different category and share none of this state.
// The sound editor and pack persistence (phase 6) assign and read these.
#![allow(dead_code)]

use super::lines::SoundRef;

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

    pub fn label(self) -> &'static str {
        match self {
            Event::Click => "Click",
            Event::Pickup => "Pick up",
            Event::Drop => "Drop",
            Event::Idle => "Idle",
        }
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

    /// Whether any trigger could play.
    pub fn any(&self) -> bool {
        self.enabled && self.triggers.iter().any(|t| t.enabled && t.sound.is_some())
    }
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
    fn event_keys_round_trip() {
        for e in Event::ALL {
            assert_eq!(Event::parse(e.key()), Some(e));
        }
    }
}
