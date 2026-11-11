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
