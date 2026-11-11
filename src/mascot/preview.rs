//! The Motion tab's live preview: the selected character animated inside
//! Settings by the same rig and animator as the desktop, with test reactions
//! that feed the animator exactly as a real click or drag would.

use iced::Vector;

use super::lines::{Lines, Picker, SoundRef, Speech, Trigger};
use super::motion::{Animator, Bop, Inputs, MotionSpec, PartType, Pose};
use super::reactions::{Config, Entry, Event, Settings, Sound, Sounds, effective};
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
    /// An event row's Try: its gesture, if it has one, and one of its lines.
    Say(Trigger),
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
    /// The bop style saved, and the pack's own.
    bop_saved: Bop,
    bop_default: Bop,
    /// The user's reaction layer being edited, what's saved, and the
    /// pack's layer under it.
    pub reactions: Config,
    reactions_saved: Config,
    reactions_pack: Config,
    picker: Picker,
    speech: Option<Speech>,
    sound: Option<SoundRef>,
}

impl MotionPreview {
    /// `rig` carries the saved motion and bop; `defaults` and `pack_bop`
    /// are the pack's own.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: &'static str,
        rig: Rig,
        defaults: Vec<MotionSpec>,
        lines: Lines,
        pack_lines: Lines,
        pack_bop: Bop,
        reactions: Config,
        pack_reactions: Config,
    ) -> Self {
        let specs = rig.motion().to_vec();
        Self {
            reactions_saved: reactions.clone(),
            reactions,
            reactions_pack: pack_reactions,
            bop_saved: rig.bop(),
            bop_default: pack_bop,
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

    /// Shows another rig for the same character, such as the unsaved
    /// artwork draft, keeping the behavior draft. Motion carries over when
    /// the parts still line up; otherwise the new parts start from their
    /// own defaults.
    pub fn set_rig(&mut self, mut rig: Rig) {
        rig.set_bop(self.rig.bop());
        if !rig.set_motion(&self.specs) {
            self.specs = rig.motion().to_vec();
            self.saved = self.specs.clone();
            self.defaults = self.specs.clone();
        }
        self.labels = rig.motion_labels();
        self.kinds = rig.motion_parts().iter().map(|p| p.kind).collect();
        self.rig = rig;
        self.animator = Animator::default();
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

    /// Picks a bop style for the draft; it shows at once and is written
    /// with the character's Save.
    pub fn set_bop(&mut self, bop: Bop) {
        self.rig.set_bop(bop);
    }

    pub fn bop_dirty(&self) -> bool {
        self.rig.bop() != self.bop_saved
    }

    /// Any unsaved behavior change: motion, lines, bop or reactions.
    pub fn unsaved(&self) -> bool {
        self.dirty() || self.lines_dirty() || self.bop_dirty() || self.reactions != self.reactions_saved
    }

    /// The reaction sounds as the draft would play them.
    pub fn sounds(&self) -> Sounds {
        effective(&self.reactions_pack, &self.reactions)
    }

    /// The user's entry for `e`, starting from what plays now.
    fn entry_mut(&mut self, e: Event) -> &mut Entry {
        if self.reactions.entry(e).is_none() {
            let start = self.reactions_pack.entry(e).cloned().unwrap_or_default();
            self.reactions.set_entry(e, Some(start));
        }
        self.reactions.entry_mut(e).expect("just set")
    }

    /// Assigns a sound to `e`; `None` turns the row off but keeps its
    /// other settings.
    pub fn set_reaction_sound(&mut self, e: Event, sound: Option<Sound>) {
        let entry = self.entry_mut(e);
        entry.sound.enabled = sound.is_some();
        entry.sound.sound = sound;
    }

    pub fn set_cooldown(&mut self, e: Event, ms: u32) {
        self.entry_mut(e).sound.cooldown_ms = ms.min(super::reactions::MAX_COOLDOWN_MS);
    }

    /// Character-wide sound settings, starting from what plays now.
    pub fn sound_settings_mut(&mut self) -> &mut Settings {
        let now = self.sounds();
        self.reactions.settings.get_or_insert(Settings {
            enabled: now.enabled,
            volume: now.volume,
            idle_enabled: now.idle_enabled,
            idle_interval: now.idle_interval,
        })
    }

    /// How many distinct WAV files would play if `e` used `file`.
    pub fn files_if(&self, e: Event, file: &super::lines::SoundRef) -> usize {
        let s = self.sounds();
        let mut files: Vec<&super::lines::SoundRef> = vec![file];
        for other in Event::ALL.into_iter().filter(|o| *o != e) {
            if let Some(Sound::File(r)) = &s.get(other).sound
                && !files.contains(&r)
            {
                files.push(r);
            }
        }
        files.len()
    }

    /// What `e` does to the picture in the draft.
    pub fn visual(&self, e: Event) -> super::reactions::Visual {
        super::reactions::visual(&self.reactions_pack, &self.reactions, e)
    }

    pub fn set_visual(&mut self, e: Event, v: super::reactions::Visual) {
        self.entry_mut(e).visual = Some(v);
    }

    /// Back to the pack's reaction sounds (unsaved until Save).
    pub fn reset_reactions(&mut self) {
        self.reactions = Config::default();
    }

    pub fn reactions_customised(&self) -> bool {
        !self.reactions.is_empty()
    }

    /// The draft as overrides on top of `base`: a value matching the pack's
    /// own is left out, so the pack's later changes still apply.
    pub fn overrides(&self, mut base: super::pack::Overrides) -> super::pack::Overrides {
        let mut lines = self.lines.clone();
        // Empty lines are dropped rather than saved.
        for t in Trigger::ALL {
            lines.get_mut(t).retain(|l| !l.text.trim().is_empty());
        }
        base.motion = self.customised().then(|| self.specs.clone());
        base.lines = (lines != self.lines_defaults).then_some(lines);
        base.bop = (self.rig.bop() != self.bop_default).then(|| self.rig.bop());
        base.reactions = (!self.reactions.is_empty()).then(|| self.reactions.clone());
        base
    }

    /// After a successful save: the draft is what's saved now.
    pub fn mark_all_saved(&mut self) {
        for t in Trigger::ALL {
            self.lines.get_mut(t).retain(|l| !l.text.trim().is_empty());
        }
        self.saved = self.specs.clone();
        self.lines_saved = self.lines.clone();
        self.bop_saved = self.rig.bop();
        self.reactions_saved = self.reactions.clone();
    }

    /// The saved reaction layer, for pruning sounds a discarded draft
    /// imported.
    pub fn saved_reactions(&self) -> &Config {
        &self.reactions_saved
    }

    pub fn saved_lines(&self) -> &Lines {
        &self.lines_saved
    }

    /// Back to what's saved, dropping every unsaved change.
    pub fn discard(&mut self) {
        self.specs = self.saved.clone();
        self.rig.set_motion(&self.specs);
        self.lines = self.lines_saved.clone();
        self.rig.set_bop(self.bop_saved);
        self.reactions = self.reactions_saved.clone();
    }

    pub fn test(&mut self, test: Test) {
        if let Test::Say(t) = test {
            self.say(t);
            let gesture = match t {
                Trigger::Click => Some(Test::Click),
                Trigger::Pickup | Trigger::Drop => Some(Test::DragRight),
                Trigger::MusicStart => Some(Test::Music),
                Trigger::Idle | Trigger::AnnotationEnd => None,
            };
            if let Some(g) = gesture {
                self.test = Some((g, self.now));
            }
            return;
        }
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

    /// Says a line for `t` now, if it has one.
    fn say(&mut self, t: Trigger) {
        if let Some(line) = self.picker.pick(&self.lines, t)
            && let Some(speech) = Speech::new(&line.text, self.now)
        {
            self.speech = Some(speech);
            self.sound = line.sound.clone();
        }
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
                    if self.visual(Event::Click) == super::reactions::Visual::Compress {
                        self.animator.compress(self.now);
                    }
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
                Test::DragLeft | Test::DragRight | Test::Speak | Test::Music | Test::Say(_) => {
                    self.test = None
                }
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
        MotionPreview::new("yohta", rig, defaults, Lines::default(), Lines::default(), Bop::default(), Config::default(), Config::default())
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
        p.mark_all_saved();
        assert!(!p.dirty());
        p.reset();
        assert!(p.dirty() && !p.customised());
    }

    #[test]
    fn one_draft_holds_motion_lines_and_bop_until_saved_or_discarded() {
        let mut p = yohta();
        assert!(!p.unsaved());
        p.set_bop(Bop::Groove);
        assert!(p.unsaved() && p.bop_dirty());
        p.lines.get_mut(Trigger::Click).push(crate::mascot::lines::Line { text: "Hi".into(), sound: None });
        p.lines.get_mut(Trigger::Drop).push(crate::mascot::lines::Line { text: "  ".into(), sound: None });
        // Overrides keep only what differs from the pack, without blanks.
        let o = p.overrides(Default::default());
        assert_eq!(o.bop, Some(Bop::Groove));
        assert_eq!(o.motion, None, "motion still matches the pack");
        let lines = o.lines.expect("lines differ from the pack");
        assert_eq!(lines.get(Trigger::Click).len(), 1);
        assert!(lines.get(Trigger::Drop).is_empty());
        // Discard returns to what's saved.
        p.discard();
        assert!(!p.unsaved());
        assert_eq!(p.rig().bop(), Bop::default());
        assert!(p.lines.get(Trigger::Click).is_empty());
        // Saving makes the draft the saved state; picking the pack's own
        // again drops the override.
        p.set_bop(Bop::RubberHose);
        p.mark_all_saved();
        assert!(!p.unsaved());
        p.set_bop(Bop::default());
        assert_eq!(p.overrides(Default::default()).bop, None);
    }

    #[test]
    fn reaction_sounds_are_part_of_the_one_draft() {
        let mut p = yohta();
        assert!(!p.sounds().get(Event::Click).enabled);
        p.set_reaction_sound(Event::Click, Some(Sound::Builtin("duck-toy")));
        p.set_cooldown(Event::Click, 600);
        p.sound_settings_mut().volume = 0.4;
        assert!(p.unsaved());
        let s = p.sounds();
        assert_eq!((s.get(Event::Click).enabled, s.get(Event::Click).cooldown_ms, s.volume), (true, 600, 0.4));
        let o = p.overrides(Default::default());
        assert!(o.reactions.is_some());
        // Clearing keeps the row's settings but turns it off.
        p.set_reaction_sound(Event::Click, None);
        assert!(!p.sounds().get(Event::Click).enabled);
        assert_eq!(p.sounds().get(Event::Click).cooldown_ms, 600);
        p.discard();
        assert!(!p.unsaved() && p.overrides(Default::default()).reactions.is_none());
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
        p.mark_all_saved();
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
