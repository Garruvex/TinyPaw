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
    LeftArm,
    RightArm,
    HoldingArm,
    Grip,
    Phone,
    GlowStick,
    PhoneGlow,
    StickGlow,
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
            PartType::LeftArm => "Left arm",
            PartType::RightArm => "Right arm",
            PartType::HoldingArm => "Holding arm",
            PartType::Grip => "Grip fingers",
            PartType::Phone => "Phone",
            PartType::GlowStick => "Glow stick",
            PartType::PhoneGlow => "Phone glow",
            PartType::StickGlow => "Stick glow",
        })
    }
}

impl PartType {
    pub const ALL: [PartType; 14] = [
        PartType::Head,
        PartType::Tail,
        PartType::Ear,
        PartType::Wing,
        PartType::Hair,
        PartType::Other,
        PartType::LeftArm,
        PartType::RightArm,
        PartType::HoldingArm,
        PartType::Grip,
        PartType::Phone,
        PartType::GlowStick,
        PartType::PhoneGlow,
        PartType::StickGlow,
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
            PartType::LeftArm => "left_arm",
            PartType::RightArm => "right_arm",
            PartType::HoldingArm => "holding_arm",
            PartType::Grip => "grip",
            PartType::Phone => "phone",
            PartType::GlowStick => "glow_stick",
            PartType::PhoneGlow => "phone_glow",
            PartType::StickGlow => "stick_glow",
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
            _ => &[Still],
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
    /// The whole picture briefly shortens and widens on each beat, then
    /// restores; works with one PNG. Nothing else moves.
    Compress,
    LightWave,
    Random,
}

impl std::fmt::Display for Bop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Bop::Nod => "Nod",
            Bop::Groove => "Groove",
            Bop::RubberHose => "Rubber hose",
            Bop::Compress => "Compress",
            Bop::LightWave => "Light wave",
            Bop::Random => "Random",
        })
    }
}

impl Bop {
    pub const ALL: [Bop; 6] = [
        Bop::Nod,
        Bop::Groove,
        Bop::RubberHose,
        Bop::Compress,
        Bop::LightWave,
        Bop::Random,
    ];

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|b| b.name() == text)
    }

    pub fn name(self) -> &'static str {
        match self {
            Bop::Nod => "nod",
            Bop::Groove => "groove",
            Bop::RubberHose => "rubber_hose",
            Bop::Compress => "compress",
            Bop::LightWave => "light_wave",
            Bop::Random => "random",
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
    Petted,
}

/// Shared dance choices from Music settings. Style stays saved per character.
#[derive(Debug, Clone, Copy)]
pub struct Dance {
    /// 0 follows sustained energy, 1 seated, 2 standing while music plays.
    pub pose: u8,
    pub stand_threshold: f32,
    /// Offset as fractions of the shared artwork canvas.
    pub hand_offset: Vector,
    pub hand_angle: f32,
    pub wave_range: f32,
    pub every: u32,
    pub included: [bool; 5],
    /// 0 glow stick, 1 phone, 2 random.
    pub item: u8,
}
impl Default for Dance {
    fn default() -> Self {
        Self {
            pose: 0,
            stand_threshold: 0.62,
            hand_offset: Vector::ZERO,
            hand_angle: 0.0,
            wave_range: 1.0,
            every: 16,
            included: [true; 5],
            item: 2,
        }
    }
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
    /// Normalized audio energy before the motion-strength slider.
    pub energy: Option<f32>,
    /// Bit 0: authored phone arm; bit 1: authored glow-stick arm.
    pub music_items: u8,
    /// Where we are between followed beats (0 on the beat), while Follow
    /// beat has one; otherwise music bobs at a relaxed fixed pace.
    pub beat: Option<f32>,
    /// How the character bops while music plays.
    pub bop: Bop,
    pub dance: Dance,
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
    /// Music head bob: how far the head (with everything on it) drops, as
    /// a fraction of the character's height (0 at rest, never up, so the
    /// neck never opens).
    pub head_drop: f32,
    /// Raised paw visibility and side-to-side swing for Light wave.
    pub light_lift: f32,
    pub light_swing: f32,
    pub light_phone: bool,
    pub hand_offset: Vector,
    pub hand_angle: f32,
    pub music_active: bool,
    /// Standing artwork opacity during a pose change. None renders fully.
    pub music_mix: Option<f32>,
    pub expression: Expression,
    /// Where the eyes look, each axis -1 to 1, in head space.
    pub gaze: Vector,
    /// While carried: the whole character turns by this angle around the
    /// grabbed point (in the frame's rig coordinates), so that point stays
    /// under the cursor however it leans.
    pub carry: Option<(iced::Point, f32)>,
}

#[derive(Debug, Clone, Copy, Default)]
struct Spring {
    angle: f32,
    velocity: f32,
}

/// Steps motion over time. One per visible character.
#[derive(Debug, Clone, Default)]
pub struct Animator {
    selected: Bop,
    random_slot: u32,
    random_ready: bool,
    random_seed: u64,
    previous_music_pose: Option<Pose>,
    transition: Option<(Pose, f32)>,
    item_phone: bool,
    item_choice: u8,
    item_ready: bool,

    light_lift: f32,
    music_seen: Option<f32>,
    excitement: f32,
    excited_for: f32,
    calm_for: f32,
    standing: bool,
    standing_mix: f32,
    springs: Vec<Spring>,
    /// Last frame's angles, so a part can feel its parent move.
    last: Vec<f32>,
    lean: f32,
    squash: f32,
    /// Smoothed drag velocity, so a jerky drag reads as one motion.
    velocity: Vector,
    /// The click reaction's face shows until this time.
    react_until: f32,
    /// When the last Compress reaction started. A new one restarts this
    /// one bounded envelope rather than stacking scale.
    compressed: Option<f32>,
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

    /// Starts a Compress reaction: the picture shortens and widens, then
    /// restores. Overlapping ones restart the envelope.
    pub fn compress(&mut self, now: f32) {
        self.compressed = Some(now);
    }

    /// Whether anything is still moving toward rest, so the host keeps
    /// stepping even with idle motion off.
    pub fn settling(&self) -> bool {
        self.music_seen.is_some()
            || self.light_lift > 0.001
            || self.transition.is_some()
            || self.lean.abs() > 0.002
            || self.squash > 0.01
            || self.compressed.is_some()
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
            // Pressing only changes the face: a squash before the drag
            // threshold would pull the grabbed point away from the cursor.
            self.squash += (0.0 - self.squash) * ease(25.0);
        }

        // Where the bop is: this beat's phase, and which of two beats.
        let phase = beat_phase(i);
        if phase < self.last_phase - 0.5 && i.music > 0.0 && !i.reduced {
            self.beats = self.beats.wrapping_add(1);
        }
        self.last_phase = phase;
        let beat = Beat {
            p: phase,
            two: (self.beats % 2) as f32 + phase,
        };
        let level = i.music.min(2.0);

        if i.music_items == 0 {
            self.music_seen = None;
        } else if i.music > 0.001 {
            self.music_seen = Some(i.now);
        } else if self.music_seen.is_some_and(|at| i.now - at >= 0.65) {
            self.music_seen = None;
        }
        let playing = self.music_seen.is_some_and(|at| i.now - at < 0.65);
        let energy = i.energy.unwrap_or(i.music).clamp(0.0, 1.0);
        self.excitement += (energy - self.excitement) * ease(1.0 / 1.2);
        let high = i.dance.stand_threshold.clamp(0.3, 0.9);
        self.excited_for = if self.excitement >= high {
            self.excited_for + dt
        } else {
            0.0
        };
        self.calm_for = if self.excitement < high * 0.65 {
            self.calm_for + dt
        } else {
            0.0
        };
        match i.dance.pose {
            1 => self.standing = false,
            2 => self.standing = playing,
            _ => {
                if !playing {
                    self.standing = false;
                } else if self.excited_for >= 1.2 {
                    self.standing = true;
                } else if self.calm_for >= 2.5 {
                    self.standing = false;
                }
            }
        }
        let music_active = self.standing && playing && !i.pressed && !i.dragging;
        self.standing_mix += ((music_active as u8 as f32) - self.standing_mix) * ease(8.0);
        if self.standing_mix < 0.001 {
            self.standing_mix = 0.0;
        }
        if i.pressed || i.dragging {
            self.standing_mix = 0.0;
        }

        let supports_wave = (i.music_items != 0 && self.standing)
            || parts.iter().any(|p| p.kind == PartType::HoldingArm)
                && parts
                    .iter()
                    .any(|p| matches!(p.kind, PartType::Phone | PartType::GlowStick));
        let slot = self.beats / i.dance.every.max(1);
        let old_style = self.selected;
        if i.bop != Bop::Random {
            self.selected = i.bop;
            self.random_ready = false;
        } else if i.music > 0.0 && !i.reduced && (slot != self.random_slot || !self.random_ready) {
            self.random_ready = true;
            if self.random_seed == 0 {
                self.random_seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(1, |d| d.as_nanos() as u64)
                    | 1;
            }
            self.random_seed ^= self.random_seed << 13;
            self.random_seed ^= self.random_seed >> 7;
            self.random_seed ^= self.random_seed << 17;
            let choices: Vec<Bop> = Bop::ALL[..5]
                .iter()
                .copied()
                .enumerate()
                .filter(|(n, b)| i.dance.included[*n] && (*b != Bop::LightWave || supports_wave))
                .map(|(_, b)| b)
                .collect();
            let alternatives: Vec<Bop> = choices
                .iter()
                .copied()
                .filter(|b| *b != self.selected)
                .collect();
            let pool = if alternatives.is_empty() {
                &choices
            } else {
                &alternatives
            };
            self.selected = pool
                .get(self.random_seed as usize % pool.len().max(1))
                .copied()
                .unwrap_or(Bop::Nod);
        }
        self.random_slot = slot;
        if old_style != self.selected {
            self.transition = self.previous_music_pose.clone().map(|p| (p, 0.0));
        }
        if self.random_seed == 0 {
            self.random_seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |d| d.as_nanos() as u64)
                | 1;
        }
        let phone_available =
            i.music_items & 1 != 0 || parts.iter().any(|p| p.kind == PartType::Phone);
        let stick_available =
            i.music_items & 2 != 0 || parts.iter().any(|p| p.kind == PartType::GlowStick);
        let changing_item = self.item_choice != i.dance.item;
        if self.light_lift < 0.01 && (!self.item_ready || changing_item) {
            self.random_seed ^= self.random_seed << 13;
            self.random_seed ^= self.random_seed >> 7;
            self.random_seed ^= self.random_seed << 17;
            self.item_phone = phone_available
                && (!stick_available
                    || i.dance.item == 1
                    || (i.dance.item == 2 && self.random_seed % 2 == 1));
            self.item_choice = i.dance.item;
            self.item_ready = true;
        }
        if self.selected != Bop::LightWave {
            self.item_ready = false;
        }
        let bop = if self.selected == Bop::LightWave && i.music_items != 0 && !self.standing {
            Bop::Nod
        } else {
            self.selected
        };

        if i.reduced {
            self.light_lift = 0.0;
        } else {
            let lift = if bop == Bop::LightWave && !changing_item {
                level.clamp(0.0, 1.0)
            } else {
                0.0
            };
            self.light_lift += (lift - self.light_lift) * ease(7.0);
            if self.light_lift < 0.001 {
                self.light_lift = 0.0;
            }
        }

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
                let base = if i.idle {
                    idle_angle(spec.preset, t, i.dragging)
                } else {
                    0.0
                };
                let drag = drag_angle(part.kind, spec.preset, self.velocity.x, i.dragging);
                let music = if i.music > 0.0 {
                    bop_angle(bop, part.kind, beat) * level
                } else {
                    0.0
                };
                let goal = if matches!(
                    part.kind,
                    PartType::HoldingArm
                        | PartType::Grip
                        | PartType::Phone
                        | PartType::GlowStick
                        | PartType::PhoneGlow
                        | PartType::StickGlow
                ) {
                    self.light_lift
                        * (-1.8
                            + i.dance.hand_angle
                            + (beat.two * std::f32::consts::PI).cos() * 0.45 * i.dance.wave_range)
                } else {
                    (base + drag + music) * k
                };
                angles[n] = if spec.preset.springy() {
                    // Inertia: when the parent turns, the part lags behind.
                    let parent_turn = part.parent.map_or(0.0, |p| {
                        angles[p] - self.last.get(p).copied().unwrap_or(0.0)
                    });
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
            let b = bop_body(bop, beat);
            Body {
                bounce: b.bounce * level,
                lean: b.lean * level,
                squash: b.squash * level,
                head_drop: b.head_drop * level,
            }
        };
        // A Compress reaction: rises fast, eases back, bounded.
        let click = match self.compressed {
            Some(at) if !i.reduced && !i.dragging && i.now >= at && i.now - at < COMPRESS_TIME => {
                let t = (i.now - at) / COMPRESS_TIME;
                hit(t, 2) * COMPRESS
            }
            Some(_) => {
                self.compressed = None;
                0.0
            }
            None => 0.0,
        };
        let pose = Pose {
            angles,
            lean: self.lean + body.lean,
            // A click's Compress and a beat's share one envelope height.
            squash: self.squash
                + if click > 0.0 {
                    body.squash.max(click)
                } else {
                    body.squash
                },
            bounce: body.bounce,
            head_drop: body.head_drop,
            light_lift: self.light_lift,
            light_phone: self.item_phone,
            hand_offset: i.dance.hand_offset * self.light_lift,
            hand_angle: i.dance.hand_angle * self.light_lift,
            music_active,
            music_mix: Some(self.standing_mix),
            light_swing: (beat.two * std::f32::consts::PI).cos() * i.dance.wave_range,
            expression,
            gaze: Vector::ZERO,
            carry: None,
        };
        let pose = if let Some((from, elapsed)) = &mut self.transition {
            *elapsed += dt;
            let t = (*elapsed / 0.35).min(1.0);
            let blended = blend(from, pose, t);
            if t >= 1.0 {
                self.transition = None;
            }
            blended
        } else {
            pose
        };
        if i.reduced {
            self.transition = None;
        }
        let pose = if i.reduced {
            Pose {
                angles: vec![0.0; parts.len()],
                expression,
                music_active: pose.music_active,
                music_mix: pose.music_mix,
                ..Pose::default()
            }
        } else {
            pose
        };
        self.previous_music_pose = Some(pose.clone());
        pose
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
    head_drop: f32,
}

/// How a part moves with the music at full level.
fn bop_angle(bop: Bop, kind: PartType, b: Beat) -> f32 {
    use std::f32::consts::{PI, TAU};
    let beat = b.p * TAU;
    let pair = b.two * PI;
    let hit = hit(b.p, 3);
    match bop {
        Bop::Nod => match kind {
            PartType::Head => -pair.cos() * (0.025 + nod_dip(b.p) * 0.035),
            PartType::Tail => (pair - 0.6).sin() * 0.1,
            PartType::Ear => (beat - 0.5).cos() * 0.055,
            PartType::Wing => (beat * 2.0).sin() * 0.15,
            PartType::Hair | PartType::Other => (beat + 0.5).sin() * 0.04,
            _ => 0.0,
        },
        // Parts follow the side-to-side lean a little late; the head and
        // ears also snap on each beat.
        Bop::Groove => match kind {
            PartType::Head => (pair - 0.5).cos() * 0.06 + hit * 0.06,
            PartType::Tail => -(pair - 0.9).cos() * 0.14,
            PartType::Ear => (pair - 1.2).cos() * 0.08 + hit * 0.05,
            PartType::Wing => (pair - 0.6).cos() * 0.1,
            PartType::Hair | PartType::Other => (pair - 1.0).cos() * 0.06,
            _ => 0.0,
        },
        // Ears, wings and hair flop with each hop; head and tail rock with
        // the body, late.
        Bop::RubberHose => match kind {
            PartType::Head => (pair - 0.3).sin() * 0.05,
            PartType::Tail => (pair - 1.0).sin() * 0.16,
            PartType::Ear => -(beat - 1.0).cos() * 0.12,
            PartType::Wing => beat.sin() * 0.2,
            PartType::Hair | PartType::Other => -(beat - 1.2).cos() * 0.08,
            _ => 0.0,
        },
        Bop::LightWave if kind == PartType::Head => pair.cos() * 0.04,
        Bop::LightWave | Bop::Compress | Bop::Random => 0.0,
    }
}

/// Head lands on phase zero, recovers over most of the beat, then
/// anticipates the next landing. Both endpoints have zero velocity.
fn nod_dip(phase: f32) -> f32 {
    use std::f32::consts::PI;
    let phase = phase.rem_euclid(1.0);
    if phase < 0.58 {
        0.5 + 0.5 * (PI * phase / 0.58).cos()
    } else {
        0.5 - 0.5 * (PI * (phase - 0.58) / 0.42).cos()
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
        // A head-led groove: land with a small shoulder dip on every beat,
        // lift through the recovery, and rock to the opposite side next beat.
        // The feet stay planted through the squash and lean.
        Bop::Nod => Body {
            bounce: (1.0 - nod_dip(b.p)) * 0.008,
            head_drop: nod_dip(b.p) * HEAD_DROP * 1.2,
            lean: pair.cos() * 0.018,
            squash: nod_dip(b.p) * 0.18,
        },
        // A dip on the beat, leaning to one side, then the other; the head
        // bobs down hard on each beat.
        Bop::Groove => Body {
            bounce: 0.0,
            lean: pair.cos() * 0.05,
            squash: hit(b.p, 3) * 0.5,
            head_drop: hit(b.p, 3) * HEAD_DROP * 1.2,
        },
        // Lands on the beat squashed, stretches at the top of the hop.
        Bop::RubberHose => {
            let air = (b.p * PI).sin();
            Body {
                bounce: air * 0.035,
                lean: pair.sin() * 0.06,
                squash: hit(b.p, 6) * 1.1 - air * 0.6,
                head_drop: hit(b.p, 6) * HEAD_DROP * 0.8,
            }
        }
        // One short squash per beat, back to rest before the next; silence
        // leaves the picture resting.
        Bop::Random => Body::default(),
        Bop::LightWave => Body {
            lean: -pair.cos() * 0.035,
            ..Body::default()
        },
        Bop::Compress => Body {
            squash: hit(b.p, 4) * COMPRESS,
            ..Body::default()
        },
    }
}

/// How far the head drops on a beat at full level, as a fraction of the
/// character's height: snaps down just after the beat, eases back up.
const HEAD_DROP: f32 = 0.03;

/// How far a Compress reaction squashes at its peak (the same visible
/// effect for a click and a beat).
const COMPRESS: f32 = 0.9;
/// A click's Compress envelope length, in seconds.
const COMPRESS_TIME: f32 = 0.28;

/// Places the character for `pose`: the music bounce lifts it, and lean and
/// squash turn and scale it around its feet so they stay on the ground.
/// Shared by the desktop and the Settings preview.
pub fn place(frame: &mut iced::widget::canvas::Frame, pose: &Pose, feet: iced::Point, height: f32) {
    if let Some((at, angle)) = pose.carry
        && angle.abs() > 0.0005
    {
        frame.translate(Vector::new(at.x, at.y));
        frame.rotate(angle);
        frame.translate(Vector::new(-at.x, -at.y));
    }
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
    let turn = |q: iced::Point, at: iced::Point, angle: f32| {
        let (s, c) = angle.sin_cos();
        let (x, y) = (q.x - at.x, q.y - at.y);
        iced::Point::new(at.x + x * c - y * s, at.y + x * s + y * c)
    };
    let lift = Vector::new(0.0, -pose.bounce * height);
    let q = if !deforms(pose) {
        p + lift
    } else {
        let (sx, sy) = squash_scale(pose);
        let q = iced::Point::new(feet.x + (p.x - feet.x) * sx, feet.y + (p.y - feet.y) * sy);
        turn(q, feet, pose.lean) + lift
    };
    match pose.carry {
        Some((at, angle)) if angle.abs() > 0.0005 => turn(q, at, angle),
        _ => q,
    }
}

/// The rig point `place` draws at `q`: the inverse of `placed`.
pub fn unplaced(pose: &Pose, feet: iced::Point, height: f32, q: iced::Point) -> iced::Point {
    let q = match pose.carry {
        Some((at, angle)) if angle.abs() > 0.0005 => {
            let (s, c) = angle.sin_cos();
            let (x, y) = (q.x - at.x, q.y - at.y);
            iced::Point::new(at.x + x * c + y * s, at.y - x * s + y * c)
        }
        _ => q,
    };
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
        hand_offset: Vector::new(
            mix(from.hand_offset.x, to.hand_offset.x),
            mix(from.hand_offset.y, to.hand_offset.y),
        ),
        hand_angle: mix(from.hand_angle, to.hand_angle),
        light_lift: mix(from.light_lift, to.light_lift),
        light_swing: mix(from.light_swing, to.light_swing),
        lean: mix(from.lean, to.lean),
        squash: mix(from.squash, to.squash),
        bounce: mix(from.bounce, to.bounce),
        head_drop: mix(from.head_drop, to.head_drop),
        carry: from.carry.map(|(at, angle)| (at, mix(angle, 0.0))),
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

    #[test]
    fn standing_music_survives_beat_gaps_then_rests_and_respects_input() {
        let mut animator = Animator::default();
        let mut input = Inputs {
            music: 1.0,
            music_items: 3,
            bop: Bop::LightWave,
            dance: Dance {
                pose: 2,
                ..Dance::default()
            },
            dt: 0.1,
            ..Inputs::default()
        };
        assert!(animator.step(&[], &input).music_active);
        input.music = 0.0;
        input.now = 0.4;
        assert!(
            animator.step(&[], &input).music_active,
            "brief gap keeps standing"
        );
        input.now = 0.8;
        assert!(
            !animator.step(&[], &input).music_active,
            "silence restores sitting"
        );
        input.music = 1.0;
        input.dragging = true;
        assert!(!animator.step(&[], &input).music_active);
        input.dragging = false;
        input.pressed = true;
        assert!(!animator.step(&[], &input).music_active);
        input.pressed = false;
        input.reduced = true;
        let pose = animator.step(&[], &input);
        assert!(pose.music_active);
        assert_eq!(pose.light_lift, 0.0, "reduced motion keeps relaxed arm");
    }

    #[test]
    fn excitement_keeps_seated_bops_then_stands_and_calms_while_music_continues() {
        let mut animator = Animator::default();
        let mut input = Inputs {
            music: 0.25,
            energy: Some(0.25),
            music_items: 3,
            dt: 0.1,
            bop: Bop::Nod,
            ..Inputs::default()
        };
        let mut pose = Pose::default();
        for _ in 0..50 {
            input.now += input.dt;
            pose = animator.step(&[], &input);
        }
        assert!(!pose.music_active, "gentle music stays seated");
        assert!(
            pose.head_drop > 0.0 || pose.bounce > 0.0,
            "sitting still bops"
        );
        input.music = 1.0;
        input.energy = Some(1.0);
        for _ in 0..40 {
            input.now += input.dt;
            pose = animator.step(&[], &input);
        }
        assert!(pose.music_active, "sustained energy stands up");
        input.music = 0.2;
        input.energy = Some(0.2);
        for _ in 0..10 {
            input.now += input.dt;
            pose = animator.step(&[], &input);
        }
        assert!(pose.music_active, "short quieter passage holds the pose");
        for _ in 0..50 {
            input.now += input.dt;
            pose = animator.step(&[], &input);
        }
        assert!(!pose.music_active, "calmer music settles into a seated bop");
        input.dance.pose = 2;
        assert!(animator.step(&[], &input).music_active, "standing override");
        input.dance.pose = 1;
        assert!(!animator.step(&[], &input).music_active, "sitting override");
    }

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
    fn compress_is_bounded_restarts_and_ignores_presses() {
        let mut a = Animator::default();
        let step = |a: &mut Animator, now: f32, pressed: bool| {
            a.step(
                &[tail()],
                &Inputs {
                    now,
                    dt: 0.016,
                    pressed,
                    ..Inputs::default()
                },
            )
        };
        // A press alone never squashes.
        let mut peak = 0.0f32;
        for n in 0..30 {
            peak = peak.max(step(&mut a, n as f32 * 0.016, true).squash);
        }
        assert!(peak < 1e-3, "{peak}");
        // A click's Compress rises and restores.
        a.compress(1.0);
        let mut peak = 0.0f32;
        let mut end = 0.0f32;
        for n in 0..40 {
            let s = step(&mut a, 1.0 + n as f32 * 0.016, false).squash;
            peak = peak.max(s);
            end = s;
        }
        assert!(peak > 0.4 && peak <= COMPRESS + 1e-3, "{peak}");
        assert!(end.abs() < 1e-3);
        // Rapid clicks restart one envelope instead of stacking.
        let mut a = Animator::default();
        let mut most = 0.0f32;
        for n in 0..20 {
            let now = 2.0 + n as f32 * 0.03;
            a.compress(now);
            most = most.max(step(&mut a, now + 0.02, false).squash);
        }
        assert!(most <= COMPRESS + 1e-3, "{most}");
    }

    #[test]
    fn the_compress_beat_style_squashes_once_per_beat_and_nothing_else() {
        let (bounce, lean, squash, tail) = bop_range(Bop::Compress, false);
        assert_eq!((bounce, lean, tail), (0.0, (0.0, 0.0), 0.0));
        assert!(squash.1 > 0.5 && squash.0 >= 0.0, "{squash:?}");
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
        for (lean, squash, bounce) in [
            (0.0, 0.0, 0.0),
            (0.15, 0.0, 0.0),
            (-0.12, 1.0, 0.03),
            (0.06, -0.6, 0.035),
        ] {
            let pose = Pose {
                lean,
                squash,
                bounce,
                ..Pose::default()
            };
            for p in [(310.0, 807.0), (0.0, 0.0), (620.0, 120.0), (40.0, 860.0)] {
                let p = iced::Point::new(p.0, p.1);
                let back = unplaced(&pose, feet, 860.0, placed(&pose, feet, 860.0, p));
                assert!(p.distance(back) < 1e-3, "{pose:?} {p:?} -> {back:?}");
            }
        }
        // The feet stay put through lean and squash; bounce lifts them.
        let pose = Pose {
            lean: 0.15,
            squash: 1.0,
            ..Pose::default()
        };
        assert!(placed(&pose, feet, 860.0, feet).distance(feet) < 1e-3);
        // A carry turn keeps its own point fixed and inverts cleanly.
        let grab = iced::Point::new(250.0, 200.0);
        let carried = Pose {
            carry: Some((grab, 0.2)),
            lean: 0.05,
            ..Pose::default()
        };
        let on_grab = unplaced(&carried, feet, 860.0, grab);
        assert!(placed(&carried, feet, 860.0, on_grab).distance(grab) < 1e-3);
        for p in [(0.0, 0.0), (620.0, 860.0)] {
            let p = iced::Point::new(p.0, p.1);
            let back = unplaced(&carried, feet, 860.0, placed(&carried, feet, 860.0, p));
            assert!(p.distance(back) < 1e-3);
        }
    }

    #[test]
    fn blend_eases_geometry_and_takes_the_live_face() {
        let held = Pose {
            angles: vec![0.4],
            lean: 0.1,
            expression: Expression::Dragging,
            ..Pose::default()
        };
        let live = Pose {
            angles: vec![0.0],
            expression: Expression::Idle,
            ..Pose::default()
        };
        let start = blend(&held, live.clone(), 0.0);
        assert_eq!(
            (start.angles[0], start.lean, start.expression),
            (0.4, 0.1, Expression::Idle)
        );
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
            pose = a.step(
                &parts,
                &Inputs {
                    now,
                    dt: 0.033,
                    ..Inputs::default()
                },
            );
        }
        assert!(!a.settling(), "everything comes to rest after the drop");
        assert!(pose.lean.abs() < 0.002 && pose.angles.iter().all(|a| a.abs() < 0.01));
    }

    #[test]
    fn a_click_shows_the_pressed_face_briefly() {
        let mut a = Animator::default();
        a.clicked(1.0);
        let at = |a: &mut Animator, now| {
            a.step(
                &[],
                &Inputs {
                    now,
                    dt: 0.03,
                    ..Inputs::default()
                },
            )
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
    fn the_nod_body_lifts_gently_between_landings() {
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
        assert!(
            after > 0.002 && before < after * 0.2 && on == 0.0,
            "{after} {before} {on}"
        );
    }

    #[test]
    fn the_head_bobs_down_on_the_beat_and_back_up() {
        let at = |a: &mut Animator, p: f32, bop: Bop| {
            a.step(
                &[],
                &Inputs {
                    now: 1.0,
                    dt: 0.03,
                    music: 1.0,
                    beat: Some(p),
                    bop,
                    ..Inputs::default()
                },
            )
            .head_drop
        };
        for bop in [Bop::Groove, Bop::RubberHose] {
            let mut a = Animator::default();
            let on = at(&mut a, 0.0, bop);
            let down = at(&mut a, 0.08, bop);
            let up = at(&mut a, 0.95, bop);
            assert!(
                on == 0.0 && down > 0.01 && up < down * 0.05 && up >= 0.0,
                "{bop}: {on} {down} {up}"
            );
        }
        assert_eq!(at(&mut Animator::default(), 0.08, Bop::Compress), 0.0);
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
        assert!(bounce > 0.005 && bounce < 0.012);
        assert!(lean.0 < -0.01 && lean.1 > 0.01);
        assert!(squash.0 >= 0.0 && squash.1 > 0.1 && squash.1 < 0.3);

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
            assert_eq!(
                (bounce, lean, squash, tail),
                (0.0, (0.0, 0.0), (0.0, 0.0), 0.0),
                "{bop}"
            );
            assert_eq!(Bop::parse(bop.name()), Some(bop));
        }
    }

    #[test]
    fn nod_lands_on_the_beat_and_recovers_smoothly() {
        let body = |p| bop_body(Bop::Nod, Beat { p, two: p });
        assert!((body(0.0).head_drop - HEAD_DROP * 1.2).abs() < 0.00001);
        assert!(body(0.58).head_drop.abs() < 0.00001);
        assert!(body(0.25).head_drop < body(0.0).head_drop);
        assert!(body(0.5).head_drop < body(0.25).head_drop);
        assert!((body(0.9999).head_drop - body(0.0).head_drop).abs() < 0.00001);
        assert!((nod_dip(0.0001) - nod_dip(0.0)).abs() < 0.00001);
        let left = Beat { p: 0.0, two: 0.0 };
        let right = Beat { p: 0.0, two: 1.0 };
        assert!(bop_angle(Bop::Nod, PartType::Head, left) < -0.05);
        assert!(bop_angle(Bop::Nod, PartType::Head, right) > 0.05);
        let end = bop_body(
            Bop::Nod,
            Beat {
                p: 0.9999,
                two: 1.9999,
            },
        );
        assert!((end.lean - body(0.0).lean).abs() < 0.00001);
        assert!((end.squash - body(0.0).squash).abs() < 0.00001);
    }

    #[test]
    fn random_styles_switch_at_each_configured_beat_boundary() {
        for every in [8, 16, 32] {
            let mut animator = Animator {
                random_seed: 42,
                ..Animator::default()
            };
            let mut input = Inputs {
                dt: 1.0 / 60.0,
                music: 1.0,
                bop: Bop::Random,
                dance: Dance {
                    every,
                    included: [true, true, false, false, false],
                    ..Dance::default()
                },
                ..Inputs::default()
            };
            let mut previous = None;
            for frame in 0..=(every * 3 * 30) {
                input.now = frame as f32 / 60.0;
                input.beat = Some((frame % 30) as f32 / 30.0);
                animator.step(&[], &input);
                if let Some(old) = previous {
                    assert_eq!(
                        animator.selected != old,
                        frame % (every * 30) == 0,
                        "interval {every}, frame {frame}"
                    );
                }
                previous = Some(animator.selected);
            }
            // Leaving Random applies a fixed style, and returning selects
            // from the enabled pool immediately.
            input.bop = Bop::Compress;
            animator.step(&[], &input);
            assert_eq!(animator.selected, Bop::Compress);
            input.bop = Bop::Random;
            animator.step(&[], &input);
            assert!(matches!(animator.selected, Bop::Nod | Bop::Groove));
        }
    }

    #[test]
    fn random_wave_requires_art_and_keeps_items_aligned() {
        let mut animator = Animator::default();
        let mut input = Inputs {
            dt: 1.0 / 60.0,
            music: 1.0,
            bop: Bop::Random,
            dance: Dance {
                included: [false, false, false, false, true],
                item: 1,
                ..Dance::default()
            },
            ..Inputs::default()
        };
        let missing = animator.step(&[], &input);
        assert_eq!(missing.light_lift, 0.0);
        let parts: Vec<_> = [
            PartType::HoldingArm,
            PartType::Phone,
            PartType::Grip,
            PartType::PhoneGlow,
        ]
        .into_iter()
        .map(|kind| PartMotion {
            kind,
            spec: MotionSpec::default_for(kind),
            parent: None,
        })
        .collect();
        animator = Animator::default();
        let mut pose = Pose::default();
        for n in 0..120 {
            input.now = n as f32 * input.dt;
            input.beat = Some((input.now * 2.0).fract());
            pose = animator.step(&parts, &input);
        }
        assert!(pose.light_lift > 0.99 && pose.light_phone);
        assert!(
            pose.angles
                .iter()
                .all(|a| (*a - pose.angles[0]).abs() < 0.0001)
        );
        input.music = 0.0;
        for _ in 0..180 {
            pose = animator.step(&parts, &input);
        }
        assert_eq!(pose.light_lift, 0.0);
        assert!(!animator.settling());
    }

    #[test]
    fn style_changes_blend_and_reduced_motion_cancels_them() {
        let mut animator = Animator::default();
        let mut input = Inputs {
            dt: 1.0 / 60.0,
            music: 1.0,
            beat: Some(0.25),
            ..Inputs::default()
        };
        let before = animator.step(&[], &input);
        input.bop = Bop::Groove;
        let after = animator.step(&[], &input);
        assert!((after.bounce - before.bounce).abs() < 0.001);
        input.reduced = true;
        let stopped = animator.step(&[], &input);
        assert_eq!(
            (stopped.bounce, stopped.light_lift, stopped.head_drop),
            (0.0, 0.0, 0.0)
        );
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
