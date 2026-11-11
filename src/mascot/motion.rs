//! Character motion. Each moving part has a type and a preset, BUG Clicker
//! style: no keyframes, just a small function of time and inputs scaled by
//! Strength and Speed. Floppy and springy parts follow with a damped spring,
//! the only "physics" here. Reactions (press, drag, drop) pick a face and add
//! short motion on top of idle motion.

use iced::Vector;

/// What a part is, which decides its presets and default layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartType {
    Head,
    Tail,
    Ear,
    Wing,
    Hair,
    Other,
}

impl std::fmt::Display for PartType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            PartType::Head => "Head",
            PartType::Tail => "Tail",
            PartType::Ear => "Ear",
            PartType::Wing => "Wing",
            PartType::Hair => "Hair",
            PartType::Other => "Other",
        })
    }
}

impl PartType {
    pub const ALL: [PartType; 6] = [
        PartType::Head,
        PartType::Tail,
        PartType::Ear,
        PartType::Wing,
        PartType::Hair,
        PartType::Other,
    ];

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.name() == text)
    }

    pub fn name(self) -> &'static str {
        match self {
            PartType::Head => "head",
            PartType::Tail => "tail",
            PartType::Ear => "ear",
            PartType::Wing => "wing",
            PartType::Hair => "hair",
            PartType::Other => "other",
        }
    }

    pub fn presets(self) -> &'static [Preset] {
        use Preset::*;
        match self {
            PartType::Head => &[Tilt, Nod, Still],
            PartType::Tail => &[Sway, Wag, Hang, Still],
            PartType::Ear => &[Twitch, Floppy, Still],
            PartType::Wing => &[Flap, Fold],
            PartType::Hair => &[Spring, Still],
            PartType::Other => &[Still, Sway, Spring],
        }
    }

    pub fn default_preset(self) -> Preset {
        self.presets()[0]
    }

    /// Tails and wings sit behind the body; ears behind the head.
    pub fn behind_by_default(self) -> bool {
        matches!(self, PartType::Tail | PartType::Wing | PartType::Ear)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Still,
    Tilt,
    Nod,
    Sway,
    Wag,
    Hang,
    Twitch,
    Floppy,
    Flap,
    Fold,
    Spring,
}

impl std::fmt::Display for Preset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = self.name();
        let mut c = name.chars();
        if let Some(first) = c.next() {
            write!(f, "{}{}", first.to_ascii_uppercase(), c.as_str())?;
        }
        Ok(())
    }
}

impl Preset {
    const ALL: [Preset; 11] = [
        Preset::Still,
        Preset::Tilt,
        Preset::Nod,
        Preset::Sway,
        Preset::Wag,
        Preset::Hang,
        Preset::Twitch,
        Preset::Floppy,
        Preset::Flap,
        Preset::Fold,
        Preset::Spring,
    ];

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.name() == text)
    }

    pub fn name(self) -> &'static str {
        match self {
            Preset::Still => "still",
            Preset::Tilt => "tilt",
            Preset::Nod => "nod",
            Preset::Sway => "sway",
            Preset::Wag => "wag",
            Preset::Hang => "hang",
            Preset::Twitch => "twitch",
            Preset::Floppy => "floppy",
            Preset::Flap => "flap",
            Preset::Fold => "fold",
            Preset::Spring => "spring",
        }
    }

    /// Follows its parent and the drag with a damped spring.
    fn springy(self) -> bool {
        matches!(self, Preset::Floppy | Preset::Spring | Preset::Hang)
    }

    /// Whether the preset has idle motion, the only thing Speed paces.
    /// Strength always applies: it also scales drag and music motion.
    pub fn uses_speed(self) -> bool {
        !matches!(
            self,
            Preset::Still | Preset::Hang | Preset::Fold | Preset::Floppy | Preset::Spring
        )
    }
}

/// How the whole character bops to music.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Bop {
    /// A relaxed nod with a light hop.
    #[default]
    Nod,
    /// A sharp head dip on each beat while the body leans side to side
    /// across two beats; no hopping.
    Groove,
    /// Old-cartoon bounce: a hop every beat, squashing on landing and
    /// stretching at the top, rocking side to side with parts flopping late.
    RubberHose,
}

impl std::fmt::Display for Bop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Bop::Nod => "Nod",
            Bop::Groove => "Groove",
            Bop::RubberHose => "Rubber hose",
        })
    }
}

impl Bop {
    pub const ALL: [Bop; 3] = [Bop::Nod, Bop::Groove, Bop::RubberHose];

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|b| b.name() == text)
    }

    pub fn name(self) -> &'static str {
        match self {
            Bop::Nod => "nod",
            Bop::Groove => "groove",
            Bop::RubberHose => "rubber_hose",
        }
    }
}

/// One part's motion settings. Strength and Speed run 0–2, 1 being the
/// preset as designed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionSpec {
    pub preset: Preset,
    pub strength: f32,
    pub speed: f32,
}

impl MotionSpec {
    /// A preset at its designed strength and speed.
    pub fn of(preset: Preset) -> Self {
        Self {
            preset,
            strength: 1.0,
            speed: 1.0,
        }
    }

    pub fn default_for(kind: PartType) -> Self {
        Self {
            preset: kind.default_preset(),
            strength: 1.0,
            speed: 1.0,
        }
    }
}

/// What the animator needs to know about a part.
#[derive(Debug, Clone, Copy)]
pub struct PartMotion {
    pub kind: PartType,
    pub spec: MotionSpec,
    /// Index of the part this one rides on.
    pub parent: Option<usize>,
}

/// Which face the head shows. Priority: dragging, pressed, blink, idle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Expression {
    #[default]
    Idle,
    Blink,
    Pressed,
    Dragging,
}

/// What happened this frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct Inputs {
    /// Seconds since the character appeared.
    pub now: f32,
    /// Seconds since the last step.
    pub dt: f32,
    /// Idle motion and blinking are on (and motion isn't reduced).
    pub idle: bool,
    /// Reduced motion: no sway, bounce, lean or springs.
    pub reduced: bool,
    /// A press is held without dragging.
    pub pressed: bool,
    pub dragging: bool,
    /// How fast the character is being dragged, in logical pixels per second.
    pub velocity: Vector,
    /// Playback loudness, 0 (silent or off) to 1, already scaled by the
    /// user's music strength.
    pub music: f32,
    /// Where we are between followed beats (0 on the beat), while Follow
    /// beat has one; otherwise music bobs at a relaxed fixed pace.
    pub beat: Option<f32>,
    /// How the character bops while music plays.
    pub bop: Bop,
}

/// Everything the renderer needs for one frame.
#[derive(Debug, Clone, Default)]
pub struct Pose {
    /// Rotation per part, in radians, relative to its parent.
    pub angles: Vec<f32>,
    /// Whole-character rotation around the feet, from dragging.
    pub lean: f32,
    /// Squash around the feet: positive is shorter and wider (a press, a
    /// landing), negative taller and narrower (a bop's stretch).
    pub squash: f32,
    /// Music bounce: how far the whole character rises, as a fraction of
    /// its height (0 at rest).
    pub bounce: f32,
    pub expression: Expression,
    /// Where the eyes look, each axis -1 to 1, in head space.
    pub gaze: Vector,
}

#[derive(Debug, Clone, Copy, Default)]
struct Spring {
    angle: f32,
    velocity: f32,
}

/// Steps motion over time. One per visible character.
#[derive(Debug, Clone, Default)]
pub struct Animator {
    springs: Vec<Spring>,
    /// Last frame's angles, so a part can feel its parent move.
    last: Vec<f32>,
    lean: f32,
    squash: f32,
    /// Smoothed drag velocity, so a jerky drag reads as one motion.
    velocity: Vector,
    /// The click reaction's face shows until this time.
    react_until: f32,
    /// Beats so far and the last beat phase, so two-beat bops know which
    /// side they're on.
    beats: u32,
    last_phase: f32,
}

/// Longest step, so a stall doesn't fling springs.
const MAX_DT: f32 = 0.1;
/// How long the pressed face stays after a click.
const CLICK_FACE: f32 = 0.35;

impl Animator {
    /// A click (press and release without dragging) just happened.
    pub fn clicked(&mut self, now: f32) {
        self.react_until = now + CLICK_FACE;
    }

    /// Whether anything is still moving toward rest, so the host keeps
    /// stepping even with idle motion off.
    pub fn settling(&self) -> bool {
        self.lean.abs() > 0.002
            || self.squash > 0.01
            || self.velocity.x.hypot(self.velocity.y) > 1.0
            || self
                .springs
                .iter()
                .any(|s| s.angle.abs() > 0.002 || s.velocity.abs() > 0.01)
    }

    pub fn step(&mut self, parts: &[PartMotion], i: &Inputs) -> Pose {
        let dt = i.dt.clamp(0.0, MAX_DT);
        let ease = |rate: f32| 1.0 - (-dt * rate).exp();
        self.springs.resize(parts.len(), Spring::default());
        self.last.resize(parts.len(), 0.0);

        let previous = self.velocity;
        let target = if i.dragging { i.velocity } else { Vector::ZERO };
        self.velocity = self.velocity + (target - self.velocity) * ease(12.0);
        // Horizontal acceleration kicks springy parts.
        let accel = if dt > 0.0 {
            (self.velocity.x - previous.x) / dt
        } else {
            0.0
        };

        if i.reduced {
            self.lean = 0.0;
            self.squash = 0.0;
            self.springs.fill(Spring::default());
        } else {
            let lean = (-self.velocity.x * 0.0004).clamp(-0.15, 0.15);
            self.lean += (lean - self.lean) * ease(10.0);
            let squash = if i.pressed { 1.0 } else { 0.0 };
            self.squash += (squash - self.squash) * ease(25.0);
        }

        // Where the bop is: this beat's phase, and which of two beats.
        let phase = beat_phase(i);
        if phase < self.last_phase - 0.5 {
            self.beats = self.beats.wrapping_add(1);
        }
        self.last_phase = phase;
        let beat = Beat {
            p: phase,
            two: (self.beats % 2) as f32 + phase,
        };
        let level = i.music.min(2.0);

        let mut angles = vec![0.0; parts.len()];
        if !i.reduced {
            // Parents first, so a child knows how far its parent just moved.
            let order = parts
                .iter()
                .enumerate()
                .filter(|(_, p)| p.parent.is_none())
                .chain(parts.iter().enumerate().filter(|(_, p)| p.parent.is_some()));
            for (n, part) in order {
                let spec = part.spec;
                let t = i.now * spec.speed.clamp(0.0, 2.0);
                let k = spec.strength.clamp(0.0, 2.0);
                let base = if i.idle { idle_angle(spec.preset, t, i.dragging) } else { 0.0 };
                let drag = drag_angle(part.kind, spec.preset, self.velocity.x, i.dragging);
                let music = if i.music > 0.0 {
                    bop_angle(i.bop, part.kind, beat) * level
                } else {
                    0.0
                };
                let goal = (base + drag + music) * k;
                angles[n] = if spec.preset.springy() {
                    // Inertia: when the parent turns, the part lags behind.
                    let parent_turn = part
                        .parent
                        .map_or(0.0, |p| angles[p] - self.last.get(p).copied().unwrap_or(0.0));
                    let s = &mut self.springs[n];
                    s.angle -= parent_turn * 0.8;
                    let (stiff, damp) = if spec.preset == Preset::Floppy {
                        (35.0, 5.0)
                    } else {
                        (60.0, 8.0)
                    };
                    let force = stiff * (goal - s.angle) - damp * s.velocity - accel * 0.00004 * k;
                    s.velocity += force * dt;
                    s.angle = (s.angle + s.velocity * dt).clamp(-0.8, 0.8);
                    s.angle
                } else {
                    goal
                };
            }
        }

        self.last.clone_from(&angles);

        let expression = if i.dragging {
            Expression::Dragging
        } else if i.pressed || i.now < self.react_until {
            Expression::Pressed
        } else if i.idle && !i.reduced && blinking(i.now) {
            Expression::Blink
        } else {
            Expression::Idle
        };

        let body = if i.reduced || i.music <= 0.0 {
            Body::default()
        } else {
            let b = bop_body(i.bop, beat);
            Body {
                bounce: b.bounce * level,
                lean: b.lean * level,
                squash: b.squash * level,
            }
        };
        Pose {
            angles,
            lean: self.lean + body.lean,
            squash: self.squash + body.squash,
            bounce: body.bounce,
            expression,
            gaze: Vector::ZERO,
        }
    }
}

/// The preset's motion with nothing else happening.
fn idle_angle(preset: Preset, t: f32, dragging: bool) -> f32 {
    match preset {
        Preset::Still | Preset::Hang | Preset::Fold | Preset::Floppy | Preset::Spring => 0.0,
        Preset::Tilt => (t * 1.4).sin() * 0.018,
        Preset::Nod => (t * 2.2).sin() * 0.03,
        Preset::Sway => (t * 1.1).sin() * 0.055,
        Preset::Wag => (t * 6.0).sin() * 0.12,
        // A quick flick now and then.
        Preset::Twitch => {
            let cycle = (t * 0.37).fract();
            if cycle < 0.06 {
                (cycle / 0.06 * std::f32::consts::PI).sin() * 0.15
            } else {
                0.0
            }
        }
        Preset::Flap if dragging => (t * 9.0).sin() * 0.3,
        Preset::Flap => (t * 1.2).sin() * 0.03,
    }
}

/// How a part reacts to being dragged at `vx` logical pixels per second.
fn drag_angle(kind: PartType, preset: Preset, vx: f32, dragging: bool) -> f32 {
    let trail = (vx * 0.0008).clamp(-0.5, 0.5);
    match kind {
        // Tails trail opposite the drag; springy ones swing on their own.
        PartType::Tail => trail,
        PartType::Hair | PartType::Other if preset.springy() => trail * 0.6,
        // Ears droop a little while carried.
        PartType::Ear if dragging => 0.12 + trail * 0.4,
        PartType::Ear => trail * 0.4,
        PartType::Head => (vx * -0.0002).clamp(-0.1, 0.1),
        _ => 0.0,
    }
}

/// Bobs per second while music plays: a relaxed nod, not a beat match.
const MUSIC_TEMPO: f32 = 2.0;

/// Where we are between beats (0 on the beat, up to 1): the followed beat
/// when there is one, else the fixed pace.
fn beat_phase(i: &Inputs) -> f32 {
    match i.beat {
        Some(p) => p.rem_euclid(1.0),
        None => (i.now * MUSIC_TEMPO).rem_euclid(1.0),
    }
}

/// A point in the bop: `p` within this beat, `two` across a pair of beats
/// (0 to 2), for moves that go left on one beat and right on the next.
#[derive(Debug, Clone, Copy)]
struct Beat {
    p: f32,
    two: f32,
}

/// Whole-character bop at full level.
#[derive(Debug, Clone, Copy, Default)]
struct Body {
    bounce: f32,
    lean: f32,
    squash: f32,
}

/// How a part moves with the music at full level.
fn bop_angle(bop: Bop, kind: PartType, b: Beat) -> f32 {
    use std::f32::consts::{PI, TAU};
    let beat = b.p * TAU;
    let pair = b.two * PI;
    let hit = hit(b.p, 3);
    match bop {
        Bop::Nod => match kind {
            PartType::Head => beat.sin() * 0.05,
            PartType::Tail => pair.sin() * 0.1,
            PartType::Ear => (beat + 1.0).sin() * 0.06,
            PartType::Wing => (beat * 2.0).sin() * 0.15,
            PartType::Hair | PartType::Other => (beat + 0.5).sin() * 0.04,
        },
        // Parts follow the side-to-side lean a little late; the head and
        // ears also snap on each beat.
        Bop::Groove => match kind {
            PartType::Head => (pair - 0.5).cos() * 0.06 + hit * 0.06,
            PartType::Tail => -(pair - 0.9).cos() * 0.14,
            PartType::Ear => (pair - 1.2).cos() * 0.08 + hit * 0.05,
            PartType::Wing => (pair - 0.6).cos() * 0.1,
            PartType::Hair | PartType::Other => (pair - 1.0).cos() * 0.06,
        },
        // Ears, wings and hair flop with each hop; head and tail rock with
        // the body, late.
        Bop::RubberHose => match kind {
            PartType::Head => (pair - 0.3).sin() * 0.05,
            PartType::Tail => (pair - 1.0).sin() * 0.16,
            PartType::Ear => -(beat - 1.0).cos() * 0.12,
            PartType::Wing => beat.sin() * 0.2,
            PartType::Hair | PartType::Other => -(beat - 1.2).cos() * 0.08,
        },
    }
}

/// A beat accent: rises in the first moment after the beat (fast, but
/// not a jump) and eases out with `power` before the next.
fn hit(p: f32, power: i32) -> f32 {
    (p / 0.06).min(1.0) * (1.0 - p).powi(power)
}

/// How the whole character moves with the music at full level.
fn bop_body(bop: Bop, b: Beat) -> Body {
    use std::f32::consts::PI;
    let pair = b.two * PI;
    match bop {
        // A quick hop just after each beat, settling before the next; it
        // starts and ends on the ground, so it never jumps.
        Bop::Nod => Body {
            bounce: (b.p.powf(0.6) * PI).sin() * 0.013,
            ..Body::default()
        },
        // A dip on the beat, leaning to one side, then the other.
        Bop::Groove => Body {
            bounce: 0.0,
            lean: pair.cos() * 0.05,
            squash: hit(b.p, 3) * 0.5,
        },
        // Lands on the beat squashed, stretches at the top of the hop.
        Bop::RubberHose => {
            let air = (b.p * PI).sin();
            Body {
                bounce: air * 0.035,
                lean: pair.sin() * 0.06,
                squash: hit(b.p, 6) * 1.1 - air * 0.6,
            }
        }
    }
}

/// Places the character for `pose`: the music bounce lifts it, and lean and
/// squash turn and scale it around its feet so they stay on the ground.
/// Shared by the desktop and the Settings preview.
pub fn place(frame: &mut iced::widget::canvas::Frame, pose: &Pose, feet: iced::Point, height: f32) {
    frame.translate(Vector::new(0.0, -pose.bounce * height));
    if deforms(pose) {
        let (sx, sy) = squash_scale(pose);
        frame.translate(Vector::new(feet.x, feet.y));
        frame.rotate(pose.lean);
        frame.scale_nonuniform(Vector::new(sx, sy));
        frame.translate(Vector::new(-feet.x, -feet.y));
    }
}

/// Whether `place` turns and scales for this pose, beyond the bounce.
fn deforms(pose: &Pose) -> bool {
    pose.squash.abs() > 0.001 || pose.lean.abs() > 0.0005
}

fn squash_scale(pose: &Pose) -> (f32, f32) {
    (1.0 + 0.06 * pose.squash, 1.0 - 0.08 * pose.squash)
}

/// Where `place` draws a rig point: the forward whole-character transform.
#[cfg(test)]
pub fn placed(pose: &Pose, feet: iced::Point, height: f32, p: iced::Point) -> iced::Point {
    let lift = Vector::new(0.0, -pose.bounce * height);
    if !deforms(pose) {
        return p + lift;
    }
    let (sx, sy) = squash_scale(pose);
    let (x, y) = ((p.x - feet.x) * sx, (p.y - feet.y) * sy);
    let (s, c) = pose.lean.sin_cos();
    iced::Point::new(feet.x + x * c - y * s, feet.y + x * s + y * c) + lift
}

/// The rig point `place` draws at `q`: the inverse of `placed`.
pub fn unplaced(pose: &Pose, feet: iced::Point, height: f32, q: iced::Point) -> iced::Point {
    let q = q - Vector::new(0.0, -pose.bounce * height);
    if !deforms(pose) {
        return q;
    }
    let (sx, sy) = squash_scale(pose);
    let (x, y) = (q.x - feet.x, q.y - feet.y);
    let (s, c) = pose.lean.sin_cos();
    let (x, y) = (x * c + y * s, -x * s + y * c);
    iced::Point::new(feet.x + x / sx, feet.y + y / sy)
}

/// Eases geometry from `from` (the pose held during a press) to `to` (live
/// motion), `t` from 0 to 1. Face and gaze come from `to`.
pub fn blend(from: &Pose, to: Pose, t: f32) -> Pose {
    let t = t.clamp(0.0, 1.0);
    let t = t * t * (3.0 - 2.0 * t);
    let mix = |a: f32, b: f32| a + (b - a) * t;
    Pose {
        angles: to
            .angles
            .iter()
            .enumerate()
            .map(|(i, b)| mix(from.angles.get(i).copied().unwrap_or(*b), *b))
            .collect(),
        lean: mix(from.lean, to.lean),
        squash: mix(from.squash, to.squash),
        bounce: mix(from.bounce, to.bounce),
        ..to
    }
}

/// The same blink rhythm as the original Yohta rig.
fn blinking(now: f32) -> bool {
    (now + 1.2) % 4.7 < 0.15
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tail() -> PartMotion {
        PartMotion {
            kind: PartType::Tail,
            spec: MotionSpec::default_for(PartType::Tail),
            parent: None,
        }
    }

    fn ear() -> PartMotion {
        PartMotion {
            kind: PartType::Ear,
            spec: MotionSpec {
                preset: Preset::Floppy,
                strength: 1.0,
                speed: 1.0,
            },
            parent: Some(0),
        }
    }

    #[test]
    fn every_type_has_its_default_among_its_presets() {
        for t in PartType::ALL {
            assert!(t.presets().contains(&t.default_preset()));
            assert_eq!(PartType::parse(t.name()), Some(t));
        }
        for p in Preset::ALL {
            assert_eq!(Preset::parse(p.name()), Some(p));
        }
    }

    #[test]
    fn reduced_motion_is_still_but_keeps_faces() {
        let mut a = Animator::default();
        let pose = a.step(
            &[tail(), ear()],
            &Inputs {
                now: 3.0,
                dt: 0.03,
                idle: true,
                reduced: true,
                dragging: true,
                velocity: Vector::new(900.0, 0.0),
                ..Inputs::default()
            },
        );
        assert!(pose.angles.iter().all(|a| *a == 0.0));
        assert_eq!((pose.lean, pose.squash), (0.0, 0.0));
        assert_eq!(pose.expression, Expression::Dragging);
    }

    #[test]
    fn speed_is_offered_only_where_it_paces_idle_motion() {
        for preset in Preset::ALL {
            let still = (0..20).all(|i| idle_angle(preset, i as f32 * 0.37, false) == 0.0);
            assert_eq!(preset.uses_speed(), !still, "{preset}");
        }
    }

    #[test]
    fn unplaced_inverts_placed() {
        let feet = iced::Point::new(310.0, 807.0);
        for (lean, squash, bounce) in [(0.0, 0.0, 0.0), (0.15, 0.0, 0.0), (-0.12, 1.0, 0.03), (0.06, -0.6, 0.035)] {
            let pose = Pose { lean, squash, bounce, ..Pose::default() };
            for p in [(310.0, 807.0), (0.0, 0.0), (620.0, 120.0), (40.0, 860.0)] {
                let p = iced::Point::new(p.0, p.1);
                let back = unplaced(&pose, feet, 860.0, placed(&pose, feet, 860.0, p));
                assert!(p.distance(back) < 1e-3, "{pose:?} {p:?} -> {back:?}");
            }
        }
        // The feet stay put through lean and squash; bounce lifts them.
        let pose = Pose { lean: 0.15, squash: 1.0, ..Pose::default() };
        assert!(placed(&pose, feet, 860.0, feet).distance(feet) < 1e-3);
    }

    #[test]
    fn blend_eases_geometry_and_takes_the_live_face() {
        let held = Pose { angles: vec![0.4], lean: 0.1, expression: Expression::Dragging, ..Pose::default() };
        let live = Pose { angles: vec![0.0], expression: Expression::Idle, ..Pose::default() };
        let start = blend(&held, live.clone(), 0.0);
        assert_eq!((start.angles[0], start.lean, start.expression), (0.4, 0.1, Expression::Idle));
        let end = blend(&held, live, 1.0);
        assert_eq!((end.angles[0], end.lean), (0.0, 0.0));
    }

    #[test]
    fn dragging_leans_and_trails_then_settles() {
        let mut a = Animator::default();
        let parts = [tail(), ear()];
        let mut now = 0.0;
        let mut pose = Pose::default();
        for _ in 0..30 {
            now += 0.033;
            pose = a.step(
                &parts,
                &Inputs {
                    now,
                    dt: 0.033,
                    dragging: true,
                    velocity: Vector::new(600.0, 0.0),
                    ..Inputs::default()
                },
            );
        }
        assert!(pose.lean < -0.05, "leans against a rightward drag");
        assert!(pose.angles[0] > 0.1, "the tail trails");
        assert_eq!(pose.expression, Expression::Dragging);
        for _ in 0..200 {
            now += 0.033;
            pose = a.step(&parts, &Inputs { now, dt: 0.033, ..Inputs::default() });
        }
        assert!(!a.settling(), "everything comes to rest after the drop");
        assert!(pose.lean.abs() < 0.002 && pose.angles.iter().all(|a| a.abs() < 0.01));
    }

    #[test]
    fn a_click_shows_the_pressed_face_briefly() {
        let mut a = Animator::default();
        a.clicked(1.0);
        let at = |a: &mut Animator, now| {
            a.step(&[], &Inputs { now, dt: 0.03, ..Inputs::default() })
                .expression
        };
        assert_eq!(at(&mut a, 1.1), Expression::Pressed);
        assert_eq!(at(&mut a, 1.5), Expression::Idle);
    }

    #[test]
    fn a_floppy_ear_lags_its_turning_head() {
        let head = PartMotion {
            kind: PartType::Head,
            spec: MotionSpec {
                preset: Preset::Nod,
                strength: 2.0,
                speed: 2.0,
            },
            parent: None,
        };
        let mut ear_first = ear();
        ear_first.parent = Some(1);
        let mut b = Animator::default();
        let mut swing = 0.0f32;
        for n in 1..60 {
            let pose = b.step(
                &[ear_first, head],
                &Inputs {
                    now: n as f32 * 0.033,
                    dt: 0.033,
                    idle: true,
                    ..Inputs::default()
                },
            );
            swing = swing.max(pose.angles[0].abs());
        }
        assert!(swing > 0.005, "the ear swings as the head nods");
    }

    #[test]
    fn music_moves_parts_and_bounces_unless_reduced() {
        let mut a = Animator::default();
        let parts = [tail()];
        let mut moved = 0.0f32;
        let mut bounced = 0.0f32;
        for n in 1..40 {
            let pose = a.step(
                &parts,
                &Inputs {
                    now: n as f32 * 0.033,
                    dt: 0.033,
                    music: 1.0,
                    ..Inputs::default()
                },
            );
            moved = moved.max(pose.angles[0].abs());
            bounced = bounced.max(pose.bounce);
        }
        assert!(moved > 0.03 && bounced > 0.005);
        let pose = a.step(
            &parts,
            &Inputs {
                now: 2.0,
                dt: 0.033,
                music: 1.0,
                reduced: true,
                ..Inputs::default()
            },
        );
        assert_eq!((pose.bounce, pose.angles[0]), (0.0, 0.0));
    }

    #[test]
    fn a_followed_beat_hops_on_the_beat() {
        let mut a = Animator::default();
        let at = |a: &mut Animator, p: f32| {
            a.step(
                &[],
                &Inputs {
                    now: 1.0,
                    dt: 0.03,
                    music: 1.0,
                    beat: Some(p),
                    ..Inputs::default()
                },
            )
            .bounce
        };
        // Up just after the beat, down again before the next.
        let after = at(&mut a, 0.3);
        let before = at(&mut a, 0.97);
        let on = at(&mut a, 0.0);
        assert!(after > 0.01 && before < after * 0.2 && on == 0.0, "{after} {before} {on}");
    }

    /// Two seconds of a bop at full music: the extremes of bounce, lean and
    /// squash, and how far the tail swung.
    fn bop_range(bop: Bop, reduced: bool) -> (f32, (f32, f32), (f32, f32), f32) {
        let mut a = Animator::default();
        let (mut bounce, mut lean, mut squash, mut tail_swing) =
            (0.0f32, (0.0f32, 0.0f32), (0.0f32, 0.0f32), 0.0f32);
        for n in 0..120 {
            let pose = a.step(
                &[tail()],
                &Inputs {
                    now: n as f32 / 60.0,
                    dt: 1.0 / 60.0,
                    music: 1.0,
                    reduced,
                    bop,
                    ..Inputs::default()
                },
            );
            bounce = bounce.max(pose.bounce);
            lean = (lean.0.min(pose.lean), lean.1.max(pose.lean));
            squash = (squash.0.min(pose.squash), squash.1.max(pose.squash));
            tail_swing = tail_swing.max(pose.angles[0].abs());
        }
        (bounce, lean, squash, tail_swing)
    }

    #[test]
    fn each_bop_style_moves_its_own_way() {
        let (bounce, lean, squash, _) = bop_range(Bop::Nod, false);
        assert!(bounce > 0.005 && lean == (0.0, 0.0) && squash == (0.0, 0.0));

        // Groove: no hop, leans to both sides, dips on the beat.
        let (bounce, lean, squash, tail) = bop_range(Bop::Groove, false);
        assert_eq!(bounce, 0.0);
        assert!(lean.0 < -0.03 && lean.1 > 0.03, "{lean:?}");
        assert!(squash.1 > 0.3 && squash.0 >= 0.0, "{squash:?}");
        assert!(tail > 0.1);

        // Rubber hose: hops, rocks both ways, squashes and stretches.
        let (bounce, lean, squash, tail) = bop_range(Bop::RubberHose, false);
        assert!(bounce > 0.03, "{bounce}");
        assert!(lean.0 < -0.03 && lean.1 > 0.03, "{lean:?}");
        assert!(squash.0 < -0.3 && squash.1 > 0.5, "{squash:?}");
        assert!(tail > 0.1);

        for bop in Bop::ALL {
            let (bounce, lean, squash, tail) = bop_range(bop, true);
            assert_eq!((bounce, lean, squash, tail), (0.0, (0.0, 0.0), (0.0, 0.0), 0.0), "{bop}");
            assert_eq!(Bop::parse(bop.name()), Some(bop));
        }
    }

    #[test]
    fn strength_zero_stops_a_part() {
        let mut a = Animator::default();
        let mut part = tail();
        part.spec.strength = 0.0;
        let pose = a.step(
            &[part],
            &Inputs {
                now: 0.7,
                dt: 0.03,
                idle: true,
                ..Inputs::default()
            },
        );
        assert_eq!(pose.angles[0], 0.0);
    }
}
