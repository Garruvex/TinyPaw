//! The Motion tab's live preview: the selected character animated inside
//! Settings by the same rig and animator as the desktop, with test reactions
//! that feed the animator exactly as a real click or drag would.

use iced::Vector;

use super::lines::{Lines, Picker, SoundRef, Speech, Trigger};
use super::motion::{Animator, Bop, Inputs, MotionSpec, PartType, Pose};
use super::rig::Rig;

/// A reaction the preview can play on demand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Test {
    Click,
    DragLeft,
    DragRight,
    /// Say a line: a click line, or any line if there are none.
    Speak,
    /// A few seconds of simulated music, to try music motion without audio.
    Music,
}

/// How long a test press is held before it releases into a click.
const PRESS: f32 = 0.12;
/// How long a test drag carries the character.
const DRAG: f32 = 0.9;
/// Peak test drag speed, in logical pixels per second.
const DRAG_SPEED: f32 = 700.0;
/// How long the simulated music plays.
const MUSIC: f32 = 4.0;

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
    /// The lines being edited, what's saved, and the pack's own.
    pub lines: Lines,
    lines_saved: Lines,
    lines_defaults: Lines,
    picker: Picker,
    speech: Option<Speech>,
    sound: Option<SoundRef>,
}

impl MotionPreview {
    /// `rig` carries the saved motion; `defaults` is the pack's own.
    pub fn new(
        id: &'static str,
        rig: Rig,
        defaults: Vec<MotionSpec>,
        lines: Lines,
        pack_lines: Lines,
    ) -> Self {
        let specs = rig.motion().to_vec();
        Self {
            lines_saved: lines.clone(),
            lines,
            lines_defaults: pack_lines,
            picker: Picker::default(),
            speech: None,
            sound: None,
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

    /// Shows a bop style at once (it's saved as soon as it's picked).
    pub fn set_bop(&mut self, bop: Bop) {
        self.rig.set_bop(bop);
    }

    pub fn test(&mut self, test: Test) {
        if test == Test::Speak {
            let t = if self.lines.get(Trigger::Click).is_empty() {
                Trigger::ALL
                    .into_iter()
                    .find(|t| !self.lines.get(*t).is_empty())
                    .unwrap_or(Trigger::Click)
            } else {
                Trigger::Click
            };
            if let Some(line) = self.picker.pick(&self.lines, t)
                && let Some(speech) = Speech::new(&line.text, self.now)
            {
                self.speech = Some(speech);
                self.sound = line.sound.clone();
            }
            return;
        }
        self.test = Some((test, self.now));
    }

    pub fn lines_dirty(&self) -> bool {
        self.lines != self.lines_saved
    }

    pub fn lines_customised(&self) -> bool {
        self.lines != self.lines_defaults
    }

    pub fn reset_lines(&mut self) {
        self.lines = self.lines_defaults.clone();
    }

    pub fn mark_lines_saved(&mut self) {
        self.lines_saved = self.lines.clone();
    }

    /// The line being said in the preview.
    pub fn speech(&self) -> Option<&str> {
        self.speech.as_ref().map(Speech::text)
    }

    pub fn hold_speech(&mut self, seconds: f32) {
        if let Some(s) = &mut self.speech {
            s.hold(self.now, seconds);
        }
    }

    pub fn take_sound(&mut self) -> Option<SoundRef> {
        self.sound.take()
    }

    /// Advances the preview by `dt` seconds.
    /// `music` is live playback loudness (0 when off or silent).
    pub fn tick(&mut self, dt: f32, idle: bool, reduced: bool, music: f32, beat: Option<f32>) {
        self.now += dt.clamp(0.0, 0.1);
        if let Some(s) = &mut self.speech
            && !s.advance(self.now)
        {
            self.speech = None;
        }
        let mut inputs = Inputs {
            now: self.now,
            dt,
            idle: idle && !reduced,
            reduced,
            music,
            beat,
            bop: self.rig.bop(),
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
                Test::Music if t < MUSIC => {
                    // Rises and falls like a short phrase.
                    inputs.music = inputs.music.max((t / MUSIC * std::f32::consts::PI).sin());
                    inputs.beat = Some((t * 2.0).fract());
                }
                Test::DragLeft | Test::DragRight | Test::Speak | Test::Music => self.test = None,
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
        MotionPreview::new("yohta", rig, defaults, Lines::default(), Lines::default())
    }

    #[test]
    fn editing_marks_changes_and_reset_returns_to_the_pack() {
        let mut p = yohta();
        assert_eq!(p.labels, ["1 Head", "2 Tail"]);
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
    fn speak_says_a_line_and_lines_edits_are_tracked() {
        let mut p = yohta();
        p.test(Test::Speak);
        assert!(p.speech().is_none(), "nothing to say yet");
        p.lines.get_mut(Trigger::Idle).push(crate::mascot::lines::Line {
            text: "Hmm // hm".into(),
            sound: None,
        });
        assert!(p.lines_dirty() && p.lines_customised());
        p.test(Test::Speak);
        assert_eq!(p.speech(), Some("Hmm"), "falls back to other triggers' lines");
        for _ in 0..200 {
            p.tick(0.05, true, false, 0.0, None);
        }
        assert!(p.speech().is_none(), "the line ends");
        p.mark_lines_saved();
        p.reset_lines();
        assert!(p.lines_dirty() && !p.lines_customised());
    }

    #[test]
    fn test_reactions_drive_the_animator() {
        let mut p = yohta();
        p.test(Test::DragRight);
        let mut dragged = false;
        let mut leaned = 0.0f32;
        for _ in 0..20 {
            p.tick(0.033, true, false, 0.0, None);
            dragged |= p.pose().expression == Expression::Dragging;
            leaned = leaned.min(p.pose().lean);
        }
        assert!(dragged && leaned < -0.01, "a rightward drag leans the character left");
        // Let it settle, then click.
        for _ in 0..40 {
            p.tick(0.033, true, false, 0.0, None);
        }
        p.test(Test::Click);
        let mut squashed = false;
        let mut pressed_face = false;
        for _ in 0..12 {
            p.tick(0.033, true, false, 0.0, None);
            squashed |= p.pose().squash > 0.1;
            pressed_face |= p.pose().expression == Expression::Pressed;
        }
        assert!(squashed && pressed_face);
    }
}
