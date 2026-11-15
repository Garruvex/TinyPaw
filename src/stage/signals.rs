//! Live music signals the stage samples on its own clock, so the character
//! moves to the beat without the app's dispatcher passing a message per frame.
//! The app owns the meter and the beat follower; the stage holds weak handles,
//! so dropping them in the app stops the music response.

use std::sync::{Arc, Mutex, Weak};
use std::time::Instant;

use crate::mascot::music::BeatClock;
use crate::platform::beat::Follower;
use crate::platform::music::Meter;

/// The beat clock, shared with the app so Settings can show its tempo.
pub type SharedClock = Arc<Mutex<(BeatClock, Option<Instant>)>>;

#[derive(Clone, Default)]
pub struct Signals {
    pub meter: Weak<Meter>,
    pub follower: Weak<Follower>,
    pub clock: SharedClock,
    /// Music response is on.
    pub music_on: bool,
    /// Music strength, as a multiplier.
    pub strength: f32,
    /// Beat timing shift, in seconds.
    pub beat_offset: f32,
}

impl PartialEq for Signals {
    fn eq(&self, other: &Self) -> bool {
        self.meter.ptr_eq(&other.meter)
            && self.follower.ptr_eq(&other.follower)
            && Arc::ptr_eq(&self.clock, &other.clock)
            && self.music_on == other.music_on
            && self.strength == other.strength
            && self.beat_offset == other.beat_offset
    }
}

impl Signals {
    /// Playback loudness scaled by strength (0 when off or silent), the raw
    /// energy, and the followed beat's phase while music plays.
    pub fn sample(&self) -> (f32, Option<f32>, Option<f32>) {
        let Some(meter) = self.meter.upgrade() else {
            return (0.0, Some(0.0), None);
        };
        let heard = meter.level();
        let level = if self.music_on {
            heard * self.strength
        } else {
            0.0
        };
        let energy = Some(if self.music_on { heard } else { 0.0 });
        if level <= 0.0 {
            return (level, energy, None);
        }
        // The followed beat when there is one; the clock eases to it, and
        // back to the steady pace when it is lost.
        let target = self
            .follower
            .upgrade()
            .and_then(|b| b.phase(self.beat_offset).zip(b.bpm()));
        let mut guard = self.clock.lock().unwrap_or_else(|e| e.into_inner());
        let (mut clock, last) = *guard;
        let now = Instant::now();
        let dt = last.map_or(0.0, |l| now.duration_since(l).as_secs_f32());
        let phase = clock.step(dt, target);
        *guard = (clock, Some(now));
        (level, energy, Some(phase))
    }
}
