//! The Motion tab's live preview: the selected character animated inside
//! Settings by the same rig and animator as the desktop, with test reactions
//! that feed the animator exactly as a real click or drag would.

use iced::Vector;

use super::motion::{Animator, Inputs, MotionSpec, PartType, Pose};
use super::rig::Rig;

/// A reaction the preview can play on demand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Test {
    Click,
    DragLeft,
    DragRight,
}

/// How long a test press is held before it releases into a click.
const PRESS: f32 = 0.12;
/// How long a test drag carries the character.
const DRAG: f32 = 0.9;
/// Peak test drag speed, in logical pixels per second.
const DRAG_SPEED: f32 = 700.0;

pub struct MotionPreview {
    pub id: &'static str,
    rig: Rig,
    /// The motion being edited, per part.
    pub specs: Vec<MotionSpec>,
    /// What's saved, to know whether there are unsaved changes.
    saved: Vec<MotionSpec>,
    /// The pack's own motion, which Reset returns to.
    defaults: Vec<MotionSpec>,
    pub labels: Vec<String>,
    pub kinds: Vec<PartType>,
    animator: Animator,
    pose: Pose,
    now: f32,
    test: Option<(Test, f32)>,
}

impl MotionPreview {
    /// `rig` carries the saved motion; `defaults` is the pack's own.
    pub fn new(id: &'static str, rig: Rig, defaults: Vec<MotionSpec>) -> Self {
        let specs = rig.motion().to_vec();
        Self {
            id,
            labels: rig.motion_labels(),
            kinds: rig.motion_parts().iter().map(|p| p.kind).collect(),
            saved: specs.clone(),
            specs,
            defaults,
            rig,
            animator: Animator::default(),
            pose: Pose::default(),
            now: 0.0,
            test: None,
        }
    }

    pub fn rig(&self) -> &Rig {
        &self.rig
    }

    pub fn pose(&self) -> &Pose {
        &self.pose
    }

    pub fn dirty(&self) -> bool {
        self.specs != self.saved
    }

    /// Whether the settings differ from the pack's own.
    pub fn customised(&self) -> bool {
        self.specs != self.defaults
    }

    pub fn set(&mut self, part: usize, spec: MotionSpec) {
        if let Some(slot) = self.specs.get_mut(part) {
            *slot = spec;
            self.rig.set_motion(&self.specs);
        }
    }

    /// Back to the pack's own motion (unsaved until Save).
    pub fn reset(&mut self) {
        self.specs = self.defaults.clone();
        self.rig.set_motion(&self.specs);
    }

    pub fn mark_saved(&mut self) {
        self.saved = self.specs.clone();
    }

    pub fn test(&mut self, test: Test) {
        self.test = Some((test, self.now));
    }

    /// Advances the preview by `dt` seconds.
    pub fn tick(&mut self, dt: f32, idle: bool, reduced: bool) {
        self.now += dt.clamp(0.0, 0.1);
        let mut inputs = Inputs {
            now: self.now,
            dt,
            idle: idle && !reduced,
            reduced,
            ..Inputs::default()
        };
        if let Some((test, started)) = self.test {
            let t = self.now - started;
            match test {
                Test::Click if t < PRESS => inputs.pressed = true,
                Test::Click => {
                    self.animator.clicked(self.now);
                    self.test = None;
                }
                Test::DragLeft | Test::DragRight if t < DRAG => {
                    let sign = if test == Test::DragLeft { -1.0 } else { 1.0 };
                    // Speeds up and slows down, like a hand carrying it.
                    let speed = (t / DRAG * std::f32::consts::PI).sin() * DRAG_SPEED;
                    inputs.dragging = true;
                    inputs.velocity = Vector::new(sign * speed, 0.0);
                }
                Test::DragLeft | Test::DragRight => self.test = None,
            }
        }
        self.pose = self.animator.step(&self.rig.motion_parts(), &inputs);
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mascot::motion::{Expression, Preset};

    fn yohta() -> MotionPreview {
        let rig = Rig::load();
        let defaults = rig.motion().to_vec();
        MotionPreview::new("yohta", rig, defaults)
    }

    #[test]
    fn editing_marks_changes_and_reset_returns_to_the_pack() {
        let mut p = yohta();
        assert_eq!(p.labels, ["Head", "Tail"]);
        assert!(!p.dirty() && !p.customised());
        p.set(
            1,
            MotionSpec {
                preset: Preset::Wag,
                strength: 1.5,
                speed: 0.5,
            },
        );
        assert!(p.dirty() && p.customised());
        assert_eq!(p.rig().motion()[1].preset, Preset::Wag);
        p.mark_saved();
        assert!(!p.dirty());
        p.reset();
        assert!(p.dirty() && !p.customised());
    }

    #[test]
    fn test_reactions_drive_the_animator() {
        let mut p = yohta();
        p.test(Test::DragRight);
        let mut dragged = false;
        let mut leaned = 0.0f32;
        for _ in 0..20 {
            p.tick(0.033, true, false);
            dragged |= p.pose().expression == Expression::Dragging;
            leaned = leaned.min(p.pose().lean);
        }
        assert!(dragged && leaned < -0.01, "a rightward drag leans the character left");
        // Let it settle, then click.
        for _ in 0..40 {
            p.tick(0.033, true, false);
        }
        p.test(Test::Click);
        let mut squashed = false;
        let mut pressed_face = false;
        for _ in 0..12 {
            p.tick(0.033, true, false);
            squashed |= p.pose().squash > 0.1;
            pressed_face |= p.pose().expression == Expression::Pressed;
        }
        assert!(squashed && pressed_face);
    }
}
