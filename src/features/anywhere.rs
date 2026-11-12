//! Lens and Spotlight "anywhere": a global shortcut shows either one with
//! annotation on or off. A tap toggles it, a hold shows it until release, and
//! while the shortcut is down a few extra keys adjust it live.

use crate::features::{spotlight, upcoming};
use crate::registry::{SettingValues, Value};
use crate::session::Millis;

/// A press at least this long counts as a hold: release hides again.
pub const HOLD_MS: Millis = 350;

/// Lens magnification steps, as stored in `lens.zoom`.
pub const ZOOM_STEPS: &[&str] = &["1.5", "2", "3", "4", "6"];

/// Tap-to-toggle, hold-for-momentary state for one global shortcut.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct HeldToggle {
    on: bool,
    down_at: Option<Millis>,
    was_on: bool,
    adjusted: bool,
}

impl HeldToggle {
    pub fn on(&self) -> bool {
        self.on
    }

    /// The shortcut is physically down.
    pub fn held(&self) -> bool {
        self.down_at.is_some()
    }

    /// Shows immediately. A repeated press while still down is ignored.
    pub fn press(&mut self, now: Millis) {
        if self.down_at.is_some() {
            return;
        }
        self.was_on = self.on;
        self.on = true;
        self.down_at = Some(now);
        self.adjusted = false;
    }

    /// A tap flips the state from before the press. A hold hides again. A
    /// press used for adjusting leaves the state as it was before, so a lit
    /// spotlight can be resized without switching it off.
    pub fn release(&mut self, now: Millis) {
        let Some(at) = self.down_at.take() else {
            return;
        };
        self.on = if self.adjusted {
            self.was_on
        } else if now.saturating_sub(at) >= HOLD_MS {
            false
        } else {
            !self.was_on
        };
    }

    /// Notes a live adjustment during the current press.
    pub fn adjusted(&mut self) {
        if self.down_at.is_some() {
            self.adjusted = true;
        }
    }

    /// Explicit on/off from the CLI or a failure; forgets any press.
    pub fn set(&mut self, on: bool) {
        *self = Self {
            on,
            ..Self::default()
        };
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Lens,
    Spotlight,
}

/// Keys that adjust the held lens or spotlight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Adjust {
    /// Up: larger lens or spotlight.
    Bigger,
    /// Down: smaller.
    Smaller,
    /// `+`: more zoom, or a darker spotlight.
    More,
    /// `-`: less zoom, or a lighter spotlight.
    Less,
}

/// The setting change one adjustment makes, or `None` at a limit.
pub fn adjust(target: Target, key: Adjust, s: &SettingValues) -> Option<(&'static str, Value)> {
    let float = |id: &'static str, step: f64, min: f64, max: f64| {
        let now = s.float(id);
        let next = (now + step).clamp(min, max);
        ((next - now).abs() > f64::EPSILON).then_some((id, Value::Float(next)))
    };
    match (target, key) {
        (Target::Spotlight, Adjust::Bigger) => float(spotlight::RADIUS, 20.0, 60.0, 400.0),
        (Target::Spotlight, Adjust::Smaller) => float(spotlight::RADIUS, -20.0, 60.0, 400.0),
        (Target::Spotlight, Adjust::More) => float(spotlight::DIM, 0.1, 0.3, 0.85),
        (Target::Spotlight, Adjust::Less) => float(spotlight::DIM, -0.1, 0.3, 0.85),
        (Target::Lens, Adjust::Bigger) => float(upcoming::SIZE, 40.0, 120.0, 480.0),
        (Target::Lens, Adjust::Smaller) => float(upcoming::SIZE, -40.0, 120.0, 480.0),
        (Target::Lens, Adjust::More | Adjust::Less) => {
            let i = ZOOM_STEPS
                .iter()
                .position(|z| *z == s.choice(upcoming::ZOOM))
                .unwrap_or(1);
            let next = if key == Adjust::More {
                (i + 1).min(ZOOM_STEPS.len() - 1)
            } else {
                i.saturating_sub(1)
            };
            (next != i).then_some((upcoming::ZOOM, Value::Choice(ZOOM_STEPS[next])))
        }
    }
}

/// The lens magnification setting as a factor.
pub fn zoom(s: &SettingValues) -> f32 {
    s.choice(upcoming::ZOOM).parse().unwrap_or(2.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tap_toggles_and_hold_is_momentary() {
        let mut t = HeldToggle::default();
        t.press(0);
        assert!(t.on());
        t.release(100);
        assert!(t.on(), "a tap leaves it on");
        t.press(1000);
        t.release(1100);
        assert!(!t.on(), "a second tap turns it off");
        t.press(2000);
        assert!(t.on(), "shows while held");
        t.release(2000 + HOLD_MS);
        assert!(!t.on(), "a hold hides on release");
    }

    #[test]
    fn hold_while_on_turns_off_and_repeats_are_ignored() {
        let mut t = HeldToggle::default();
        t.press(0);
        t.release(10);
        t.press(100);
        t.press(200);
        t.release(100 + HOLD_MS);
        assert!(!t.on());
    }

    #[test]
    fn adjusting_keeps_the_previous_state() {
        let mut t = HeldToggle::default();
        t.press(0);
        t.release(10);
        t.press(100);
        t.adjusted();
        t.release(2000);
        assert!(t.on(), "resizing a lit spotlight keeps it lit");
        t.set(false);
        t.press(3000);
        t.adjusted();
        t.release(3010);
        assert!(!t.on(), "adjusting from off is a peek");
    }

    #[test]
    fn adjust_steps_and_stops_at_limits() {
        let reg = crate::registry::Registry::build(crate::features::BUILTINS).unwrap();
        let mut s = reg.defaults();
        assert_eq!(
            adjust(Target::Lens, Adjust::More, &s),
            Some((upcoming::ZOOM, Value::Choice("3")))
        );
        reg.set(&mut s, upcoming::ZOOM, Value::Choice("6")).unwrap();
        assert_eq!(adjust(Target::Lens, Adjust::More, &s), None);
        assert_eq!(zoom(&s), 6.0);
        reg.set(&mut s, spotlight::RADIUS, Value::Float(400.0))
            .unwrap();
        assert_eq!(adjust(Target::Spotlight, Adjust::Bigger, &s), None);
        assert_eq!(
            adjust(Target::Spotlight, Adjust::Smaller, &s),
            Some((spotlight::RADIUS, Value::Float(380.0)))
        );
    }
}
