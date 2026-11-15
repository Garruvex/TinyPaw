//! The desktop character window: a small transparent, always-on-top surface
//! that shows Yohta where the user left him.
//!
//! The window is laid out as `[menu slot | character | menu slot]` so the menu
//! can open on whichever side has room without resizing the window. Blank
//! pixels must not swallow clicks, so the window stays click-through except
//! while the cursor is over visible artwork or the open menu; polling the
//! cursor (needed for the eyes anyway) switches that. Clicks never activate
//! the window, so the app underneath keeps keyboard focus.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use iced::{Point, Rectangle, Size, Vector};

use super::Msg;
use super::companionship::{self, Controller, State};
use super::lines::{BubbleSide, Lines, MenuAlign, MenuSide, Picker, SoundRef, Speech, Trigger};
use super::motion::{Animator, Expression, Inputs, PartMotion, Pose};
use super::reactions::Event;
use super::rig::Rig;
use super::surface::Surface;
use crate::features::mascot as feature;
use crate::platform::windows as native;
use crate::view::theme;

const MENU_W: f32 = 208.0;
const ROW_H: f32 = 40.0;
const MENU_PAD: f32 = 8.0;
/// Room between the character and its menu, where the menu's tail points.
const GAP: f32 = 16.0;
/// How far the menu's tail reaches out of the bubble toward the character.
const MENU_TAIL: f32 = 12.0;
/// How tall the speech bubble's tail is.
const SPEECH_TAIL: f32 = 10.0;
/// Seconds the menu takes to pop open and to fade away.
const MENU_OPEN: f32 = 0.24;
const MENU_CLOSE: f32 = 0.12;
/// Each row fades in this much later than the one above it.
const ROW_STAGGER: f32 = 0.035;
/// Seconds a new speech bubble takes to pop in.
const SPEECH_POP: f32 = 0.28;

/// Eases out past the end and settles back: a little bounce.
fn ease_out_back(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0) - 1.0;
    let c = 1.70158;
    1.0 + (c + 1.0) * t * t * t + c * t * t
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

fn faded(c: iced::Color, a: f32) -> iced::Color {
    iced::Color { a: c.a * a, ..c }
}

fn mix(a: iced::Color, b: iced::Color, t: f32) -> iced::Color {
    iced::Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}
/// Logical pixels the cursor must travel before a press becomes a drag.
const DRAG_START: f32 = 5.0;
/// Seconds to ease from the pose held during a press back to live motion.
const SETTLE: f32 = 0.25;
/// Fall acceleration, in logical pixels per second squared.
const GRAVITY: f32 = 2600.0;

/// Slowest release, in logical pixels per second, that throws the character
/// rather than putting it down.
const THROW_MIN: f32 = 350.0;
/// Fastest throw, in logical pixels per second.
const THROW_MAX: f32 = 4200.0;
/// The release velocity is the cursor's over this much of the drag's end, in
/// seconds: long enough to average out mouse jitter, short enough that
/// stopping before letting go puts the character down.
const THROW_WINDOW: f32 = 0.09;
/// Share of its speed a thrown character keeps bouncing off an edge.
const BOUNCE: f32 = 0.45;
/// A landing slower than this, in logical pixels per second, stays down.
const BOUNCE_MIN: f32 = 260.0;
/// How fast a glide (no gravity, or along the ground) sheds speed, per second.
const FRICTION: f32 = 5.0;
/// Slower than this, in logical pixels per second, a glide has stopped.
const REST: f32 = 30.0;

/// A released character in motion: thrown, falling, or both.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Flight {
    /// Physical pixels per second.
    v: (f32, f32),
    /// Falls to the bottom edge; otherwise it glides to a stop.
    gravity: bool,
    /// Where the anchor may go, in physical pixels: left, top, right, bottom.
    bounds: (f32, f32, f32, f32),
}

/// What one step of a flight did.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Flown {
    at: (f32, f32),
    /// It hit the ground hard enough to squash.
    thud: bool,
    /// It came to rest.
    done: bool,
}

impl Flight {
    /// Moves the anchor on from `at` by `dt` seconds, bouncing off the
    /// bounds. A character let go past an edge is turned back without
    /// being snapped inside. `k` is physical pixels per logical pixel.
    fn step(&mut self, at: (f32, f32), k: f32, dt: f32) -> Flown {
        let (left, top, right, bottom) = self.bounds;
        let grounded = self.gravity && at.1 >= bottom - 0.5 && self.v.1 >= 0.0;
        if grounded {
            self.v.1 = 0.0;
        } else if self.gravity {
            self.v.1 += GRAVITY * k * dt;
        }
        if grounded || !self.gravity {
            let keep = (-FRICTION * dt).exp();
            self.v.0 *= keep;
            self.v.1 *= keep;
        }
        let (mut x, mut y) = (at.0 + self.v.0 * dt, at.1 + self.v.1 * dt);
        if grounded {
            y = bottom;
        }
        if x < left && self.v.0 < 0.0 {
            x = at.0.min(left);
            self.v.0 *= -BOUNCE;
        } else if x > right && self.v.0 > 0.0 {
            x = at.0.max(right);
            self.v.0 *= -BOUNCE;
        }
        let mut thud = false;
        if y < top && self.v.1 < 0.0 {
            y = at.1.min(top);
            self.v.1 *= -BOUNCE;
        } else if y >= bottom && self.v.1 > 0.0 {
            y = at.1.max(bottom);
            thud = self.v.1 > BOUNCE_MIN * k;
            self.v.1 = if thud || !self.gravity {
                -self.v.1 * BOUNCE
            } else {
                0.0
            };
            // The ground takes some of the sideways speed too.
            self.v.0 *= 0.8;
        }
        let done = if self.gravity {
            y >= bottom - 0.5 && self.v.1 == 0.0 && self.v.0.abs() < REST * k
        } else {
            self.v.0.hypot(self.v.1) < REST * k
        };
        Flown {
            at: (x, y),
            thud,
            done,
        }
    }
}

/// How long a Click or Drop artwork reaction shows, in seconds.
const ART_TIME: f32 = 0.7;

/// The character's replacement pictures, decoded.
#[derive(Debug, Clone, Default)]
pub struct Pictures {
    /// Shown instead of the character while it is carried.
    pub held: Option<super::rig::Picture>,
    /// Shown briefly after a click or a drop.
    pub click: Option<super::rig::Picture>,
    pub drop: Option<super::rig::Picture>,
    pub headpat: Option<super::rig::Picture>,
    pub sleep: Option<super::rig::Picture>,
    pub sleep_poses: Vec<(super::rig::Picture, [f32; 2])>,
    pub wake: Option<super::rig::Picture>,
    pub settings: companionship::Settings,
}

/// Most a carried character leans, in radians, before the window cap.
const CARRY_LEAN: f32 = 0.25;
/// Most a held pose swings from its scruff.
const HANG_SWING: f32 = 0.8;
/// Gravity for a carried character's swing, logical pixels per second
/// squared.
const HANG_GRAVITY: f32 = 1800.0;
/// How fast the drag velocity that swings a carried character follows the
/// cursor's, per second. The cursor moves in whole pixels at uneven
/// intervals, so its raw speed (and far more its acceleration) is noisy.
const CARRY_SMOOTH: f32 = 30.0;
/// A carried character trails the drag by this many radians per logical
/// pixel per second, as if through air.
const CARRY_TRAIL: f32 = 0.0004;
/// How a carried character swings: the share of the drag's acceleration
/// that reaches it, how fast the swing dies (per second), and its limit.
/// A held pose hangs loose; an upright character is stiffer.
const HANG: (f32, f32, f32) = (0.2, 3.5, HANG_SWING);
const UPRIGHT: (f32, f32, f32) = (0.08, 5.0, CARRY_LEAN);
/// Gaze reaches its limit this far from the eyes, as a share of the
/// character's height (Yohta: about 420 rig pixels).
const GAZE_REACH: f32 = 0.49;
const VK_LBUTTON: i32 = 0x01;
const VK_RBUTTON: i32 = 0x02;
const VK_ESCAPE: i32 = 0x1B;

fn menu_height(rows: usize) -> f32 {
    MENU_PAD * 2.0 + ROW_H * rows as f32
}

/// Which side of the character the menu opens on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// Window geometry in logical pixels for one character size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    pub scale: f32,
    /// The rig canvas inside the window.
    pub character: Rectangle,
    /// Ground anchor (bottom centre of the feet) in window coordinates.
    pub anchor: Point,
    pub window: Size,
    menu: Size,
    /// The menu's top, from [`Layout::align_menu`]; level with the feet
    /// by default.
    menu_y: f32,
}

/// Room above the character for its speech bubble, in logical pixels.
const BUBBLE_BAND: f32 = 104.0;

/// `height` is the character's visible logical height and `visible` that
/// art's height in rig pixels, so transparent padding doesn't shrink it;
/// `canvas` the rig's size and `feet` where it meets the ground, both in rig
/// pixels; `rows` the number of menu entries; `top` room kept above the
/// character for its bubble.
pub fn layout(
    height: f32,
    visible: f32,
    canvas: Size,
    feet: Point,
    rows: usize,
    top: f32,
) -> Layout {
    let scale = height / visible.max(1.0);
    let size = Size::new(canvas.width * scale, canvas.height * scale);
    let below = (canvas.height - feet.y).max(0.0) * scale;
    let menu = Size::new(MENU_W, menu_height(rows));
    let side = MENU_W + GAP;
    let h = (size.height + top).max(menu.height + below).ceil();
    let character = Rectangle::new(Point::new(side, h - size.height), size);
    let anchor = Point::new(character.x + feet.x * scale, character.y + feet.y * scale);
    Layout {
        scale,
        character,
        anchor,
        window: Size::new((size.width + 2.0 * side).ceil(), h),
        menu,
        menu_y: (anchor.y - menu.height).max(0.0),
    }
}

impl Layout {
    /// Reserve the exact rotation envelope of a held picture around its scruff.
    /// Placement offsets cancel when the scruff is pinned to the carry pivot.
    fn with_held_room(mut self, held: &super::rig::Picture, stage: Size, top: f32) -> Self {
        let pivot = Point::new(stage.width * 0.5, top);
        let grab = Point::new(
            self.character.x + pivot.x * self.scale,
            self.character.y + pivot.y * self.scale,
        );
        let hold = held.hold_in(stage);
        let art = held.art;
        let mut min = Point::ORIGIN;
        let mut max = Point::new(self.window.width, self.window.height);
        for corner in [
            art.position(),
            Point::new(art.x + art.width, art.y),
            Point::new(art.x, art.y + art.height),
            Point::new(art.x + art.width, art.y + art.height),
        ] {
            let v = (held.in_stage(stage, corner) - hold) * self.scale;
            let x_extreme = (-v.y).atan2(v.x);
            let y_extreme = v.x.atan2(v.y);
            for angle in [
                -HANG_SWING,
                0.0,
                HANG_SWING,
                x_extreme,
                x_extreme + std::f32::consts::PI,
                y_extreme,
                y_extreme + std::f32::consts::PI,
            ] {
                let angle = (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                    - std::f32::consts::PI;
                if angle.abs() > HANG_SWING + 0.0001 {
                    continue;
                }
                let (s, c) = angle.sin_cos();
                let q = Point::new(grab.x + v.x * c - v.y * s, grab.y + v.x * s + v.y * c);
                min.x = min.x.min(q.x);
                min.y = min.y.min(q.y);
                max.x = max.x.max(q.x);
                max.y = max.y.max(q.y);
            }
        }
        let left = if min.x < 0.0 {
            (-min.x + 1.0).ceil()
        } else {
            0.0
        };
        let top = if min.y < 0.0 {
            (-min.y + 1.0).ceil()
        } else {
            0.0
        };
        let right = if max.x > self.window.width {
            (max.x - self.window.width + 1.0).ceil()
        } else {
            0.0
        };
        let bottom = if max.y > self.window.height {
            (max.y - self.window.height + 1.0).ceil()
        } else {
            0.0
        };
        self.character.x += left;
        self.character.y += top;
        self.anchor.x += left;
        self.anchor.y += top;
        self.menu_y += top;
        self.window.width += left + right;
        self.window.height += top + bottom;
        self
    }

    /// Lines the menu up with the feet, the middle or the top of the
    /// visible art (`visible`: its top and bottom rows, in rig pixels),
    /// kept inside the window.
    pub fn align_menu(mut self, align: MenuAlign, visible: (f32, f32)) -> Self {
        let (top, bottom) = (
            self.character.y + visible.0 * self.scale,
            self.character.y + visible.1 * self.scale,
        );
        let y = match align {
            MenuAlign::Feet => self.anchor.y - self.menu.height,
            MenuAlign::Middle => (top + bottom - self.menu.height) / 2.0,
            MenuAlign::Head => top,
        };
        self.menu_y = y.clamp(0.0, (self.window.height - self.menu.height).max(0.0));
        self
    }

    /// The window point a speech bubble's tail aims at: the bubble's
    /// anchor, or the top centre of the visible art.
    pub fn speech_anchor(&self, anchor: Option<(f32, f32)>, visible_top: f32) -> Point {
        let c = self.character;
        match anchor {
            Some((x, y)) => Point::new(c.x + x * c.width, c.y + y * c.height),
            None => Point::new(self.anchor.x, c.y + visible_top * self.scale),
        }
    }

    /// The menu sits beside the character, lined up by [`Layout::align_menu`].
    pub fn menu(&self, side: Side) -> Rectangle {
        let x = match side {
            Side::Left => self.character.x - GAP - MENU_W,
            Side::Right => self.character.x + self.character.width + GAP,
        };
        Rectangle::new(Point::new(x, self.menu_y), self.menu)
    }

    /// Physical window origin that puts the ground anchor at `anchor`.
    /// `k` is physical pixels per logical pixel.
    pub fn origin(&self, anchor: (f32, f32), k: f32) -> (i32, i32) {
        (
            (anchor.0 - self.anchor.x * k).round() as i32,
            (anchor.1 - self.anchor.y * k).round() as i32,
        )
    }

    /// Recovers a released or restored character just enough to leave it
    /// reachable on a work area: at least half its width inside the sides,
    /// with the same top and bottom limits as `clamp`. A character partly
    /// past a side edge stays where it was put.
    pub fn recover(&self, anchor: (f32, f32), k: f32, work: (i32, i32, i32, i32)) -> (f32, f32) {
        let (left, right) = (work.0 as f32, work.2 as f32);
        let before = (self.anchor.x - self.character.x) * k;
        let after = (self.character.x + self.character.width - self.anchor.x) * k;
        let half = (before + after) * 0.5;
        let (min_x, max_x) = (left - half + before, right + half - after);
        let x = if min_x <= max_x {
            anchor.0.clamp(min_x, max_x)
        } else {
            (left + right) * 0.5
        };
        (x, self.clamp(anchor, k, work).1)
    }

    /// Keeps the character on a work area (`left, top, right, bottom`): feet
    /// no lower than the taskbar, the window (and so any menu) no higher than
    /// the top, and the character's sides within the edges.
    pub fn clamp(&self, anchor: (f32, f32), k: f32, work: (i32, i32, i32, i32)) -> (f32, f32) {
        let (min_x, min_y, max_x, max_y) = self.range(k, work);
        (anchor.0.clamp(min_x, max_x), anchor.1.clamp(min_y, max_y))
    }

    /// Where `clamp` lets the anchor be on a work area, as `left, top,
    /// right, bottom`. A work area too small for the character leaves one
    /// spot: centred, on the bottom edge.
    pub fn range(&self, k: f32, work: (i32, i32, i32, i32)) -> (f32, f32, f32, f32) {
        let (left, top, right, bottom) =
            (work.0 as f32, work.1 as f32, work.2 as f32, work.3 as f32);
        let min_x = left + (self.anchor.x - self.character.x) * k;
        let max_x = right - (self.character.x + self.character.width - self.anchor.x) * k;
        let (min_x, max_x) = if min_x <= max_x {
            (min_x, max_x)
        } else {
            ((left + right) * 0.5, (left + right) * 0.5)
        };
        (min_x, (top + self.anchor.y * k).min(bottom), max_x, bottom)
    }

    /// The first spot: bottom right of the work area, standing on the taskbar.
    pub fn default_anchor(&self, k: f32, work: (i32, i32, i32, i32)) -> (f32, f32) {
        let right = work.2 as f32 - 24.0 * k;
        let x = right - (self.character.x + self.character.width - self.anchor.x) * k;
        self.clamp((x, work.3 as f32), k, work)
    }

    /// Right unless the menu would cross the work area's right edge.
    pub fn menu_side(&self, anchor_x: f32, k: f32, work_right: i32) -> Side {
        let menu = self.menu(Side::Right);
        let edge = anchor_x + (menu.x + menu.width - self.anchor.x) * k;
        if edge <= work_right as f32 {
            Side::Right
        } else {
            Side::Left
        }
    }

    /// The side the user chose, unless the menu would leave the work area
    /// (`left`, `right`) there and fits on the other side.
    pub fn chosen_side(&self, choice: MenuSide, anchor_x: f32, k: f32, work: (i32, i32)) -> Side {
        let fits = |side: Side| {
            let m = self.menu(side);
            let (l, r) = (
                anchor_x + (m.x - self.anchor.x) * k,
                anchor_x + (m.x + m.width - self.anchor.x) * k,
            );
            l >= work.0 as f32 && r <= work.1 as f32
        };
        let flip = |s: Side| match s {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        };
        let want = match choice {
            MenuSide::Auto => return self.menu_side(anchor_x, k, work.1),
            MenuSide::Left => Side::Left,
            MenuSide::Right => Side::Right,
        };
        if fits(want) || !fits(flip(want)) {
            want
        } else {
            flip(want)
        }
    }
}

/// Where the eyes should look: toward `cursor`, full deflection at `reach`.
pub fn gaze_toward(cursor: Point, eyes: Point, reach: f32) -> Vector {
    let d = cursor - eyes;
    let distance = d.x.hypot(d.y);
    if distance < 1e-3 || reach <= 0.0 {
        return Vector::ZERO;
    }
    d * ((distance / reach).min(1.0) / distance)
}

/// Whether a press has travelled far enough to become a drag.
pub fn is_drag(delta: (i32, i32), k: f32) -> bool {
    let (dx, dy) = (delta.0 as f32, delta.1 as f32);
    dx.hypot(dy) / k.max(0.01) > DRAG_START
}

/// The flight a release starts, if any: a throw when the cursor was moving
/// at `v` (physical pixels per second) fast enough, capped; and with
/// `falls`, a drop from wherever it is above the ground. `k` is physical
/// pixels per logical pixel; `bounds` as in [`Layout::range`].
fn flight(
    v: (f32, f32),
    k: f32,
    falls: bool,
    anchor: (f32, f32),
    bounds: (f32, f32, f32, f32),
) -> Option<Flight> {
    let speed = v.0.hypot(v.1);
    let v = if speed < THROW_MIN * k {
        (0.0, 0.0)
    } else {
        let cap = (THROW_MAX * k / speed).min(1.0);
        (v.0 * cap, v.1 * cap)
    };
    let thrown = v != (0.0, 0.0);
    (thrown || (falls && anchor.1 < bounds.3 - 0.5)).then_some(Flight {
        v,
        gravity: falls,
        bounds,
    })
}

/// Character choices that change behaviour, read from settings each update.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    /// Unsaved style preview for the currently selected character.
    pub bop: Option<super::motion::Bop>,
    pub dance: super::motion::Dance,
    pub eyes: bool,
    pub idle: bool,
    pub reduced: bool,
    /// Speech lines are shown.
    pub lines: bool,
    /// Quiet: no idle or music lines.
    pub quiet: bool,
    /// Idle lines come at a random time in this range, in seconds.
    pub idle_every: (f32, f32),
    /// Playback loudness to move with, 0 when off or silent.
    pub music: f32,
    pub energy: Option<f32>,
    /// The followed beat's phase, while Follow beat has one.
    pub beat: Option<f32>,
}

/// One menu row. `checked` draws a switch; `on_press: None` disables it.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub icon: crate::view::icons::Glyph,
    pub label: &'static str,
    pub checked: Option<bool>,
    pub on_press: Option<Msg>,
}

/// A left press on the character, held until release.
struct Press {
    cursor: (i32, i32),
    /// The grabbed point, in window logical pixels. While dragging, the
    /// window goes wherever keeps this point under the cursor.
    grab: Point,
    /// The pose when pressed. Its geometry holds until release, so nothing
    /// pulls the grabbed point away from the cursor; faces and gaze still
    /// change.
    pose: Pose,
    dragging: bool,
    /// The grabbed point in the stage frame's rig coordinates: the pivot
    /// a carried character leans around.
    pivot: Point,
    /// Parts that keep their pressed angle while carried: the grabbed part
    /// and every part it rides on, so moving them can't move the contact.
    /// Other parts trail freely.
    protected: Vec<bool>,
    /// The current carry lean and how fast it is swinging.
    lean: f32,
    swing: f32,
    /// The drag's smoothed velocity, in logical pixels per second; its
    /// changes are the acceleration that swings the character.
    v: Vector,
    /// Where the cursor was over the last `THROW_WINDOW` of the drag, by
    /// time: what a release throws with.
    trail: VecDeque<(f32, (i32, i32))>,
}

impl Press {
    /// Notes where the cursor is now, forgetting what is too old to count.
    fn track(&mut self, now: f32, cursor: (i32, i32)) {
        self.trail.push_back((now, cursor));
        while self
            .trail
            .front()
            .is_some_and(|(t, _)| now - t > THROW_WINDOW)
        {
            self.trail.pop_front();
        }
    }

    /// The cursor's velocity as the drag ended, in physical pixels per
    /// second: zero when it had stopped, or was let go too soon to tell.
    fn velocity(&self, now: f32) -> (f32, f32) {
        let recent = || self.trail.iter().filter(|(t, _)| now - t <= THROW_WINDOW);
        let (Some((t0, a)), Some((t1, b))) = (recent().next(), recent().next_back()) else {
            return (0.0, 0.0);
        };
        let span = t1 - t0;
        if span < 0.01 {
            return (0.0, 0.0);
        }
        ((b.0 - a.0) as f32 / span, (b.1 - a.1) as f32 / span)
    }
}

/// Where the window goes so window point `grab` sits under `cursor`, both
/// physical, at `k` physical pixels per logical pixel.
pub fn carried_origin(cursor: (i32, i32), grab: Point, k: f32) -> (i32, i32) {
    (
        (cursor.0 as f32 - grab.x * k).round() as i32,
        (cursor.1 as f32 - grab.y * k).round() as i32,
    )
}

/// What a host does after a step, besides drawing.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Effects {
    /// Mouse passthrough should become this (true: clicks pass through).
    pub passthrough: Option<bool>,
    /// The window should be hidden (true) or shown (false).
    pub hide: Option<bool>,
    /// A resting spot to save: the feet's physical position.
    pub save: Option<(f64, f64)>,
}

pub struct Desktop {
    hwnd: isize,
    rig: Rig,
    /// The selected character this rig was loaded for.
    character: &'static str,
    /// The rig's moving parts, for the animator.
    parts: Vec<PartMotion>,
    animator: Animator,
    /// The latest frame of motion.
    pose: Pose,
    lines: Lines,
    picker: Picker,
    /// The line being said, if any.
    speech: Option<Speech>,
    /// When the next idle line is due, in `now` seconds.
    next_idle: f32,
    /// A line sound for the app to play.
    sound: Option<SoundRef>,
    /// Gesture events since the app last took them, each exactly once.
    events: Vec<Event>,
    /// A click plays Compress.
    click_compress: bool,
    pictures: Pictures,
    companion: Controller,
    audio: companionship::Audio,
    voice_busy: bool,
    nearby: bool,
    /// A Click or Drop picture showing until this time.
    showing: Option<(f32, Event)>,
    /// After release, fall to the screen edge instead of staying.
    fall_on_release: bool,
    /// Thrown or falling since its release.
    flight: Option<Flight>,
    /// When motion last stepped, and where the feet were then.
    stepped: f32,
    last_anchor: (f32, f32),
    size: f32,
    rows: usize,
    layout: Layout,
    /// Physical ground anchor on the virtual desktop.
    anchor: (f32, f32),
    started: Instant,
    now: f32,
    gaze: Vector,
    target: Vector,
    menu: Option<Side>,
    /// When the menu last opened, in `now` seconds.
    opened: f32,
    /// A menu fading away after closing, and when it closed.
    closing: Option<(Side, f32)>,
    /// How highlighted each menu row is, 0–1, eased toward the hovered row.
    hover: Vec<f32>,
    /// When the line shown now first appeared, for its pop-in.
    speech_at: f32,
    /// Reduced motion: menus and bubbles appear without animating.
    reduced: bool,
    press: Option<Press>,
    /// When the last press ended and the pose it held, to ease out of.
    released: Option<(f32, Pose)>,
    /// Mouse passthrough last applied to the window, once known.
    passthrough: Option<bool>,
    /// Annotation owns input, so the character steps aside.
    hidden: bool,
    /// The window is placed and showing.
    shown: bool,
    left_down: bool,
    right_down: bool,
    /// The other characters on the stage, and this one's ID among them.
    crowd: Option<(std::sync::Arc<super::crowd::Crowd>, u32)>,
}

impl Desktop {
    /// Decodes the rig and opens the window off-screen; it moves into place
    /// once its native handle arrives. (A window opened invisible and shown
    /// later never presents a frame with Iced 0.14 here, so it starts visible
    /// but out of sight.)
    /// The character's state, with no window yet. The stage attaches its
    /// native window with `attach`.
    pub fn new(
        character: &'static str,
        rig: Rig,
        size: f32,
        anchor: (f64, f64),
        rows: usize,
        hidden: bool,
    ) -> Self {
        let layout = layout(
            size,
            rig.visible_height(),
            rig.size(),
            rig.anchor(),
            rows,
            0.0,
        );
        let parts = rig.motion_parts();
        Self {
            hwnd: 0,
            rig,
            character,
            parts,
            animator: Animator::default(),
            pose: Pose::default(),
            lines: Lines::default(),
            picker: Picker::default(),
            speech: None,
            next_idle: f32::INFINITY,
            sound: None,
            events: Vec::new(),
            click_compress: true,
            pictures: Pictures::default(),
            companion: Controller::new(Instant::now().elapsed().as_nanos() as u64 + 1),
            audio: companionship::Audio::Unknown,
            voice_busy: false,
            nearby: false,
            showing: None,
            fall_on_release: false,
            flight: None,
            stepped: 0.0,
            last_anchor: (anchor.0 as f32, anchor.1 as f32),
            size,
            rows,
            layout,
            anchor: (anchor.0 as f32, anchor.1 as f32),
            started: Instant::now(),
            now: 0.0,
            gaze: Vector::ZERO,
            target: Vector::ZERO,
            menu: None,
            opened: 0.0,
            closing: None,
            hover: vec![0.0; rows],
            speech_at: 0.0,
            reduced: false,
            press: None,
            released: None,
            passthrough: None,
            hidden,
            shown: false,
            left_down: false,
            right_down: false,
            crowd: None,
        }
    }

    /// Joins the other characters: stand clear of them and take turns to speak.
    pub fn join(&mut self, crowd: std::sync::Arc<super::crowd::Crowd>, id: u32) {
        self.crowd = Some((crowd, id));
    }

    /// Leaves the others behind when this character goes.
    pub fn leave(&mut self) {
        if let Some((crowd, id)) = self.crowd.take() {
            crowd.remove(id);
        }
    }

    /// Room above the character for a bubble, when it has lines, and
    /// always a little for a music hop and stretch.
    fn band(&self) -> f32 {
        let swing = (self.size * 0.3).ceil();
        if self.lines.is_empty() {
            swing
        } else {
            BUBBLE_BAND.max(swing)
        }
    }

    /// The character's lines; re-lays out for the bubble band.
    pub fn set_lines(&mut self, lines: Lines, o: &Options) {
        self.lines = lines;
        self.speech = None;
        self.menu = None;
        self.relayout();
        self.schedule_idle(o);
        self.place();
    }

    fn schedule_idle(&mut self, o: &Options) {
        let (min, max) = (
            o.idle_every.0.min(o.idle_every.1),
            o.idle_every.0.max(o.idle_every.1),
        );
        let span = ((max - min) * 1000.0) as usize + 1;
        self.next_idle = self.now + min + self.picker.below(span) as f32 / 1000.0;
    }

    /// Says a line for `t`, unless lines are off, Quiet holds back an
    /// unprompted one, or the character is hidden.
    pub fn say(&mut self, t: Trigger, o: &Options) {
        if !o.lines
            || self.hidden
            || (t.unprompted() && o.quiet)
            || (t != Trigger::Sleep
                && matches!(self.companion.state, State::Asleep | State::Waking))
        {
            return;
        }
        if let Some(line) = self.picker.pick(&self.lines, t)
            && let Some(speech) = Speech::new(&line.text, self.now)
        {
            // One bubble at a time: a line nobody asked for waits its turn,
            // and one the user provoked takes it.
            if let Some((crowd, id)) = &self.crowd {
                let seconds = 2.0 + line.text.chars().count() as f32 * 0.06;
                let prompted = !t.unprompted() && t != Trigger::AnnotationEnd;
                if !crowd.claim_voice(*id, Instant::now(), seconds, prompted) {
                    return;
                }
            }
            self.speech = Some(speech);
            self.speech_at = self.now;
            self.sound = line.sound.clone();
        }
    }

    /// Keeps the bubble up while its sound plays.
    pub fn hold_speech(&mut self, seconds: f32) {
        if let Some(s) = &mut self.speech {
            s.hold(self.now, seconds);
            if let Some((crowd, id)) = &self.crowd {
                crowd.claim_voice(*id, Instant::now(), seconds, true);
            }
        }
    }

    /// The line sound to play now, once.
    pub fn take_sound(&mut self) -> Option<SoundRef> {
        self.sound.take()
    }

    /// Whether a click plays Compress (after release, so it never moves a
    /// held point).
    pub fn set_click_compress(&mut self, on: bool) {
        self.click_compress = on;
    }

    pub fn set_fall(&mut self, on: bool) {
        self.fall_on_release = on;
        if !on {
            self.flight = None;
        }
    }

    pub fn set_pictures(&mut self, pictures: Pictures) {
        self.pictures = pictures;
        self.companion.reset();
        self.companion
            .set_sleep_pose_count(self.pictures.sleep_poses.len());
        self.showing = None;
        self.relayout();
        self.place();
    }

    /// The picture drawn instead of the character now, if any. Carrying
    /// shows the held pose; a recent click or drop shows its artwork.
    fn sleep_art(&self) -> Option<&super::rig::Picture> {
        self.pictures
            .sleep_poses
            .get(self.companion.sleep_pose)
            .map(|p| &p.0)
            .or(self.pictures.sleep.as_ref())
    }
    fn sleep_settings(&self) -> companionship::Settings {
        let mut settings = self.pictures.settings.clone();
        if let Some((_, mouth)) = self.pictures.sleep_poses.get(self.companion.sleep_pose) {
            settings.mouth = *mouth;
        }
        settings
    }
    fn picture(&self) -> Option<&super::rig::Picture> {
        if self.press.as_ref().is_some_and(|p| p.dragging) {
            return self.pictures.held.as_ref();
        }
        match self.companion.state {
            State::Asleep | State::Waking => {
                return self
                    .sleep_art()
                    .filter(|_| self.companion.sleep_picture(self.now));
            }
            State::Petting => return self.pictures.headpat.as_ref(),
            _ => {}
        }
        match self.showing {
            Some((until, Event::Click)) if self.now < until => self.pictures.click.as_ref(),
            Some((until, Event::Drop)) if self.now < until => self.pictures.drop.as_ref(),
            Some((until, Event::Wake)) if self.now < until && self.companion.sleep_pose == 0 => {
                self.pictures.wake.as_ref()
            }
            _ => None,
        }
    }

    /// Gesture events (Click, Pickup, Drop) since the last call.
    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Visible and not busy with a press or its menu: idle may react.
    pub fn idle_free(&self) -> bool {
        !self.hidden
            && self.shown
            && self.press.is_none()
            && self.menu.is_none()
            && matches!(self.companion.state, State::Awake)
            && !self.companion.sleep_due
    }

    pub fn set_voice_busy(&mut self, busy: bool) {
        self.voice_busy = busy;
    }
    pub fn set_audio(&mut self, audio: companionship::Audio) {
        self.audio = audio;
    }

    /// Shows another character, keeping its feet where these were.
    pub fn set_rig(&mut self, character: &'static str, rig: Rig) {
        self.character = character;
        self.parts = rig.motion_parts();
        self.animator = Animator::default();
        self.rig = rig;
        self.companion.reset();
        self.menu = None;
        self.press = None;
        self.relayout();
        self.place();
    }

    /// Lays the window out for the current size, rig, menu and lines.
    fn relayout(&mut self) {
        self.layout = layout(
            self.size,
            self.rig.visible_height(),
            self.rig.size(),
            self.rig.anchor(),
            self.rows,
            self.band(),
        )
        .align_menu(self.lines.menu.align, self.rig.visible_rows());
        if let Some(held) = &self.pictures.held {
            self.layout =
                self.layout
                    .with_held_room(held, self.rig.size(), self.rig.visible_rows().0);
        }
    }

    /// Closes the menu, letting it fade away.
    pub fn close_menu(&mut self) {
        if let Some(side) = self.menu.take() {
            self.closing = Some((side, self.now));
        }
    }

    fn open_menu(&mut self) {
        self.now = self.started.elapsed().as_secs_f32();
        self.menu = Some(self.side());
        self.opened = self.now;
        self.closing = None;
        self.hover = vec![0.0; self.rows];
    }

    /// The menu bubble as drawn now: its side, scale and opacity, while it
    /// is open or still fading away.
    fn menu_look(&self) -> Option<(Side, f32, f32)> {
        if let Some(side) = self.menu {
            if self.reduced {
                return Some((side, 1.0, 1.0));
            }
            let t = (self.now - self.opened) / MENU_OPEN;
            return Some((side, 0.6 + 0.4 * ease_out_back(t), (t * 2.5).min(1.0)));
        }
        let (side, at) = self.closing?;
        let t = (self.now - at) / MENU_CLOSE;
        (!self.reduced && t < 1.0).then(|| (side, 1.0 - 0.08 * ease_out(t), 1.0 - ease_out(t)))
    }

    /// How far row `i` has faded in since the menu opened, 0–1.
    fn row_reveal(&self, i: usize) -> f32 {
        if self.reduced {
            return 1.0;
        }
        let t = self.now - self.opened - 0.06 - ROW_STAGGER * i as f32;
        ease_out(t / 0.16)
    }

    /// Something in the menu or bubble is still moving.
    fn animating(&self) -> bool {
        if self.reduced {
            return false;
        }
        let opening = self.menu.is_some()
            && self.now - self.opened < MENU_OPEN + 0.25 + ROW_STAGGER * self.rows as f32;
        let hovering = self.menu.is_some() && self.hover.iter().any(|h| *h > 0.005 && *h < 0.995);
        let popping = self.speech.is_some() && self.now - self.speech_at < SPEECH_POP;
        opening || hovering || popping || self.closing.is_some()
    }

    /// Eases each row's highlight toward the row under `local`.
    fn step_hover(&mut self, local: Point, dt: f32) {
        let target = self.menu.and_then(|side| {
            let r = self.layout.menu(side);
            let row = ((local.y - r.y - MENU_PAD) / ROW_H).floor();
            (r.contains(local) && row >= 0.0).then_some(row as usize)
        });
        self.hover.resize(self.rows, 0.0);
        let k = if self.reduced {
            1.0
        } else {
            1.0 - (-dt * 18.0).exp()
        };
        for (i, h) in self.hover.iter_mut().enumerate() {
            let goal = if target == Some(i) { 1.0 } else { 0.0 };
            *h += (goal - *h) * k;
        }
    }

    /// Handles window input and polling. Returns window operations and, when
    /// a drag ends, the ground anchor to save.
    /// Attaches the native window this character is shown in.
    pub fn attach(&mut self, hwnd: isize, o: &Options) -> Effects {
        self.step(Msg::Native(hwnd), o)
    }

    /// One message or poll. Window and mouse changes come back as effects,
    /// so any host can apply them.
    pub fn step(&mut self, msg: Msg, o: &Options) -> Effects {
        match msg {
            Msg::Native(hwnd) => {
                self.hwnd = hwnd;
                self.place();
                self.passthrough = Some(true);
                self.shown = !self.hidden;
                Effects {
                    passthrough: Some(true),
                    hide: self.hidden.then_some(true),
                    save: None,
                }
            }
            Msg::Pressed => {
                // Picking it up mid-flight catches it at once.
                self.flight = None;
                if let (Some(cursor), Some(rect)) =
                    (native::cursor_pos(), native::window_rect(self.hwnd))
                {
                    let k = self.k(rect);
                    let grab = Point::new(
                        (cursor.0 - rect.0) as f32 / k,
                        (cursor.1 - rect.1) as f32 / k,
                    );
                    let (pivot, protected) = self.grab_chain(grab);
                    if self.companion.wake(self.now).is_some() {
                        self.events.push(Event::Wake);
                        self.say(Trigger::Wake, o);
                        self.showing = Some((self.now + 0.7, Event::Wake));
                    }
                    self.press = Some(Press {
                        cursor,
                        grab,
                        pose: self.pose.clone(),
                        dragging: false,
                        pivot,
                        protected,
                        lean: 0.0,
                        swing: 0.0,
                        v: Vector::ZERO,
                        trail: VecDeque::new(),
                    });
                }
                Effects::default()
            }
            Msg::Released => Effects {
                save: self.release(o),
                ..Effects::default()
            },
            Msg::Menu => {
                self.reduced = o.reduced;
                if self.menu.is_some() {
                    self.close_menu();
                } else {
                    self.open_menu();
                }
                Effects::default()
            }
            Msg::Poll => self.poll(o),
            Msg::Command(_) | Msg::CharacterSettings => Effects::default(),
        }
    }

    fn poll(&mut self, o: &Options) -> Effects {
        self.now = self.started.elapsed().as_secs_f32();
        self.reduced = o.reduced;
        if self
            .closing
            .is_some_and(|(_, at)| self.now - at >= MENU_CLOSE)
        {
            self.closing = None;
        }
        if !self.shown {
            return Effects::default();
        }
        let mut picked_up = false;
        let (Some(cursor), Some(rect)) = (native::cursor_pos(), native::window_rect(self.hwnd))
        else {
            return Effects::default();
        };
        let k = self.k(rect);
        let local = Point::new(
            (cursor.0 - rect.0) as f32 / k,
            (cursor.1 - rect.1) as f32 / k,
        );
        let left = native::key_down(VK_LBUTTON);
        let right = native::key_down(VK_RBUTTON);
        let clicked = (left && !self.left_down) || (right && !self.right_down);
        self.left_down = left;
        self.right_down = right;
        let mut save = None;
        if self.press.is_none()
            && let Some(landed) = self.step_flight(k)
        {
            save = Some(landed);
        }

        if let Some(press) = &mut self.press {
            let delta = (cursor.0 - press.cursor.0, cursor.1 - press.cursor.1);
            if !left {
                // The release happened where the canvas could not see it.
                save = self.release(o);
            } else {
                if !press.dragging && is_drag(delta, k) {
                    press.dragging = true;
                    if let Some(side) = self.menu.take() {
                        self.closing = Some((side, self.now));
                    }
                    picked_up = true;
                    if self.pictures.held.is_some() {
                        let l = &self.layout;
                        let top = self.rig.visible_rows().0;
                        let grab = Point::new(
                            l.character.x + l.character.width / 2.0,
                            l.character.y + top * l.scale,
                        );
                        press.grab = grab;
                        press.pivot = Point::new((grab.x - l.character.x) / l.scale, top);
                    }
                }
                if press.dragging {
                    press.track(self.now, cursor);
                    let origin = carried_origin(cursor, press.grab, k);
                    self.carry_to(origin, k);
                }
            }
        }

        if let Some(side) = self.menu
            && self.press.is_none()
        {
            let outside = !self.layout.menu(side).contains(local) && !self.over_character(local);
            if native::key_down(VK_ESCAPE) || (clicked && outside) {
                self.close_menu();
            }
        }
        let dt = (self.now - self.stepped).clamp(0.0, 0.1);
        self.step_hover(local, dt);

        let hit = self.press.is_some()
            || self.over_character(local)
            || self
                .menu
                .is_some_and(|s| self.layout.menu(s).contains(local));
        let passthrough = if self.passthrough != Some(!hit) {
            self.passthrough = Some(!hit);
            Some(!hit)
        } else {
            None
        };

        self.target = match self.rig.eye_point() {
            Some(eye) if o.eyes => {
                let eyes = self.layout.character.position()
                    + Vector::new(eye.x, eye.y) * self.layout.scale;
                gaze_toward(local, eyes, GAZE_REACH * self.layout.character.height)
            }
            _ => Vector::ZERO,
        };
        self.gaze = if o.reduced {
            self.target
        } else {
            // The same easing at any frame rate: 30% of the way per 33 ms.
            let dt = (self.now - self.stepped).clamp(0.0, 0.1);
            self.gaze + (self.target - self.gaze) * (1.0 - 0.7f32.powf(dt / 0.033))
        };
        let margin = (self.size * 0.25).max(24.0) + if self.nearby { 8.0 } else { 0.0 };
        let bounds = if matches!(self.companion.state, State::Asleep | State::Waking) {
            self.sleep_art()
                .map(|p| {
                    let a = p.in_stage(self.rig.size(), p.art.position());
                    let b = p.in_stage(
                        self.rig.size(),
                        Point::new(p.art.x + p.art.width, p.art.y + p.art.height),
                    );
                    Rectangle::new(a, Size::new(b.x - a.x, b.y - a.y))
                })
                .unwrap_or_else(|| self.rig.visible_bounds())
        } else {
            self.rig.visible_bounds()
        };
        let bounds = Rectangle::new(
            Point::new(
                self.layout.character.x + bounds.x * self.layout.scale,
                self.layout.character.y + bounds.y * self.layout.scale,
            ),
            Size::new(
                bounds.width * self.layout.scale,
                bounds.height * self.layout.scale,
            ),
        );
        self.nearby = bounds.expand(margin).contains(local);
        let touching = self.over_character(local);
        let stage_cursor = Point::new(
            (local.x - self.layout.character.x) / self.layout.scale,
            (local.y - self.layout.character.y) / self.layout.scale,
        );
        let event = self.companion.step(
            &self.pictures.settings,
            companionship::Input {
                now: self.now,
                cursor: Some(stage_cursor),
                nearby: self.nearby,
                touching,
                pet_region: touching && self.rig.pet_region(stage_cursor, &self.pose),
                button: (left || right) && touching,
                busy: self.menu.is_some() || self.press.is_some() || self.flight.is_some(),
                response_busy: self.voice_busy
                    || self.speech.is_some()
                    || self.showing.is_some_and(|(until, _)| self.now < until),
                audio: self.audio,
                height: self.rig.visible_height(),
            },
        );
        if let Some(event) = event {
            let e = match event {
                companionship::Event::Headpat => Event::Headpat,
                companionship::Event::Sleep => Event::Sleep,
                companionship::Event::Wake => Event::Wake,
            };
            self.events.push(e);
            let trigger = match e {
                Event::Headpat => Trigger::Headpat,
                Event::Sleep => Trigger::Sleep,
                _ => Trigger::Wake,
            };
            self.say(trigger, o);
            if e == Event::Wake {
                self.showing = Some((self.now + 0.7, e));
            }
        }
        self.step_motion(o, k);
        if picked_up {
            self.events.push(Event::Pickup);
            self.say(Trigger::Pickup, o);
        }
        self.step_speech(o);
        Effects {
            passthrough,
            hide: None,
            save,
        }
    }

    /// Moves a line along, and says an idle line when one is due and the
    /// character isn't busy or behind a full-screen app.
    fn step_speech(&mut self, o: &Options) {
        if let Some(s) = &mut self.speech {
            let part = s.index;
            let lost_turn = self
                .crowd
                .as_ref()
                .is_some_and(|(crowd, id)| !crowd.has_voice(*id, Instant::now()));
            if lost_turn || !s.advance(self.now) {
                self.speech = None;
                self.sound = None;
                if let Some((crowd, id)) = &self.crowd {
                    crowd.release_voice(*id);
                }
            } else if s.index != part {
                // Each part of a line pops in afresh.
                self.speech_at = self.now;
            }
        }
        if self.now < self.next_idle {
            return;
        }
        let free = self.idle_free() && self.speech.is_none();
        if free && !native::foreground_fullscreen() {
            self.say(Trigger::Idle, o);
        }
        self.schedule_idle(o);
    }

    /// Advances idle motion and reactions to now.
    fn step_motion(&mut self, o: &Options, k: f32) {
        let dt = (self.now - self.stepped).max(0.0);
        self.stepped = self.now;
        let moved = (
            self.anchor.0 - self.last_anchor.0,
            self.anchor.1 - self.last_anchor.1,
        );
        self.last_anchor = self.anchor;
        let velocity = if dt > 0.0 {
            Vector::new(moved.0 / k / dt, moved.1 / k / dt)
        } else {
            Vector::ZERO
        };
        // Pressed: keep the pressed pose's geometry; only the face changes.
        // Carried: lean around the grabbed point and let unprotected parts
        // trail, both of which leave that point under the cursor.
        if let Some(press) = &self.press {
            let mut pose = press.pose.clone();
            pose.expression = if press.dragging {
                Expression::Dragging
            } else {
                Expression::Pressed
            };
            if press.dragging && !o.reduced {
                let live = self.animator.step(
                    &self.parts,
                    &Inputs {
                        now: self.now,
                        dt,
                        dragging: true,
                        velocity,
                        bop: o.bop.unwrap_or_else(|| self.rig.bop()),
                        dance: o.dance,
                        ..Inputs::default()
                    },
                );
                for (i, angle) in pose.angles.iter_mut().enumerate() {
                    if !press.protected.get(i).copied().unwrap_or(true) {
                        *angle = live.angles.get(i).copied().unwrap_or(*angle);
                    }
                }
                // Carried, it swings from the grabbed point like a damped
                // pendulum: the drag's acceleration kicks it, so a shake
                // shows, and it settles trailing the way it came. Held above
                // its middle the feet swing behind; held below, the head.
                let dt = dt.clamp(0.0, 0.05);
                let v = press.v + (velocity - press.v) * (1.0 - (-dt * CARRY_SMOOTH).exp());
                let accel = if dt > 0.0 {
                    (v - press.v) * (1.0 / dt)
                } else {
                    Vector::ZERO
                };
                let (length, sign, (kick, damp, limit)) = match &self.pictures.held {
                    // A held pose hangs from its scruff.
                    Some(held) => (
                        (held.hold_in(self.rig.size()).distance(Point::new(
                            self.rig.size().width / 2.0,
                            self.rig.visible_middle(),
                        )) * self.layout.scale)
                            .max(20.0),
                        1.0,
                        HANG,
                    ),
                    None => {
                        let below = (self.rig.visible_middle() - press.pivot.y) * self.layout.scale;
                        (
                            below.abs().max(self.size * 0.3),
                            if below > 0.0 { 1.0 } else { -1.0 },
                            UPRIGHT,
                        )
                    }
                };
                let a = press.lean;
                let trail = sign * (v.x * CARRY_TRAIL).clamp(-limit * 0.6, limit * 0.6);
                // Yanked up it weighs more, dropped it weighs less.
                let gravity =
                    (HANG_GRAVITY - accel.y).clamp(HANG_GRAVITY * 0.4, HANG_GRAVITY * 2.5);
                let alpha = (sign * kick * accel.x.clamp(-20_000.0, 20_000.0) * a.cos()
                    - gravity * (a - trail).sin())
                    / length
                    - damp * press.swing;
                let swing = press.swing + alpha * dt;
                let free = a + swing * dt;
                let lean = self.fit_carry(press.grab, press.pivot, free.clamp(-limit, limit));
                // Stopped by its limit or the window, it stops swinging
                // that way rather than pressing against it.
                let swing = if (lean - free).abs() > 1e-4 && swing * free > 0.0 {
                    0.0
                } else {
                    swing
                };
                pose.carry = Some((press.pivot, lean));
                if let Some(p) = &mut self.press {
                    p.lean = lean;
                    p.swing = swing;
                    p.v = v;
                }
            }
            pose.gaze = self.head_gaze(&pose);
            self.pose = pose;
            return;
        }
        let sleeping = self.companion.sleep_motion(self.now);
        let mut pose = self.animator.step(
            &self.parts,
            &Inputs {
                now: self.now,
                dt,
                idle: o.idle && !o.reduced && !sleeping,
                reduced: o.reduced,
                pressed: false,
                // Thrown, its parts trail the flight as they did the drag.
                dragging: self.flight.is_some(),
                velocity,
                music: if sleeping { 0.0 } else { o.music },
                music_items: self.rig.music_items(),
                energy: o.energy,
                beat: if sleeping { None } else { o.beat },
                bop: o.bop.unwrap_or_else(|| self.rig.bop()),
                dance: o.dance,
            },
        );
        if let Some((at, held)) = &self.released {
            let t = (self.now - at) / SETTLE;
            if t >= 1.0 {
                self.released = None;
            } else {
                pose = super::motion::blend(held, pose, t);
            }
        }
        pose.gaze = self.head_gaze(&pose);
        if sleeping {
            pose.expression = Expression::Blink;
            pose.gaze = Vector::ZERO;
            if self.pictures.settings.breathing && !o.reduced {
                pose.squash = (self.now * 1.6).sin() * 0.05;
            }
        }
        if self.companion.state == State::Petting {
            pose.expression = Expression::Petted;
        }
        self.companion.apply_affection(
            &self.rig,
            &mut pose,
            self.now,
            o.reduced,
            self.picture().is_some(),
        );
        self.companion.apply_sleep(
            &self.rig,
            &mut pose,
            self.now,
            o.reduced,
            self.picture().is_some(),
        );
        self.pose = pose;
    }

    /// Where window point `grab` is in the stage frame, and which parts
    /// it hangs from.
    fn grab_chain(&self, grab: Point) -> (Point, Vec<bool>) {
        let l = &self.layout;
        let pivot = Point::new(
            (grab.x - l.character.x) / l.scale,
            (grab.y - l.character.y) / l.scale,
        );
        let local =
            super::motion::unplaced(&self.pose, self.rig.anchor(), self.rig.size().height, pivot);
        let mut protected = vec![false; self.parts.len()];
        if let Some(super::rig::Layer::Part(mut i)) = self.rig.hit(local, &self.pose) {
            loop {
                if let Some(p) = protected.get_mut(i) {
                    *p = true;
                }
                match self.parts.get(i).and_then(|p| p.parent) {
                    Some(parent) if !protected.get(parent).copied().unwrap_or(true) => i = parent,
                    _ => break,
                }
            }
        }
        (pivot, protected)
    }

    /// The largest lean toward `lean` that keeps the turned character
    /// inside its window: around a grab near the head the feet swing out,
    /// and anything past the window edge would be cut off.
    fn fit_carry(&self, grab: Point, pivot: Point, lean: f32) -> f32 {
        let l = &self.layout;
        let c = l.character;
        // The visible art's box in window pixels: the hanging held pose,
        // or the character's columns between its top and bottom art rows.
        let corners = match &self.pictures.held {
            Some(held) => {
                let stage = self.rig.size();
                let shift = pivot - held.hold_in(stage);
                let a = held.art;
                [
                    Point::new(a.x, a.y),
                    Point::new(a.x + a.width, a.y),
                    Point::new(a.x, a.y + a.height),
                    Point::new(a.x + a.width, a.y + a.height),
                ]
                .map(|p| {
                    let f = held.in_stage(stage, p) + shift;
                    Point::new(c.x + f.x * l.scale, c.y + f.y * l.scale)
                })
            }
            None => {
                let (top, bottom) = self.rig.visible_rows();
                let (top, bottom) = (c.y + top * l.scale, c.y + bottom * l.scale);
                [
                    Point::new(c.x, top),
                    Point::new(c.x + c.width, top),
                    Point::new(c.x, bottom),
                    Point::new(c.x + c.width, bottom),
                ]
            }
        };
        let fits = |a: f32| {
            let (s, co) = a.sin_cos();
            corners.iter().all(|p| {
                let (x, y) = (p.x - grab.x, p.y - grab.y);
                let q = Point::new(grab.x + x * co - y * s, grab.y + x * s + y * co);
                q.x >= -0.5
                    && q.y >= -0.5
                    && q.x <= l.window.width + 0.5
                    && q.y <= l.window.height + 0.5
            })
        };
        if fits(lean) {
            return lean;
        }
        if !fits(0.0) {
            return 0.0;
        }
        // Bisect for the limit itself, so a lean held against it stays
        // there instead of jumping between steps.
        let (mut inside, mut outside) = (0.0, lean);
        for _ in 0..12 {
            let mid = (inside + outside) * 0.5;
            if fits(mid) {
                inside = mid;
            } else {
                outside = mid;
            }
        }
        inside
    }

    /// The gaze counter-rotated into the head's space, so it stays
    /// directional while the head tilts.
    fn head_gaze(&self, pose: &Pose) -> Vector {
        let (s, c) = (-self.rig.head_angle(pose)).sin_cos();
        let g = self.gaze;
        Vector::new(g.x * c - g.y * s, g.x * s + g.y * c)
    }

    /// Ends a press: a short one is a click the character reacts to, a drag
    /// saves the spot. The menu is on the right button.
    fn release(&mut self, o: &Options) -> Option<(f64, f64)> {
        let press = self.press.take()?;
        self.released = Some((self.now, self.pose.clone()));
        if press.dragging {
            // Left where it was put, unless that leaves it out of reach;
            // let go on the move, it carries on from there.
            self.place();
            self.launch(press.velocity(self.now), o.reduced);
            // The held pose ends; the drop's artwork, if any, shows briefly.
            self.showing = self
                .pictures
                .drop
                .is_some()
                .then_some((self.now + ART_TIME, Event::Drop));
            self.events.push(Event::Drop);
            self.say(Trigger::Drop, o);
            return Some((self.anchor.0 as f64, self.anchor.1 as f64));
        }
        self.close_menu();
        self.animator.clicked(self.now);
        if self.click_compress {
            self.animator.compress(self.now);
        }
        self.showing = self
            .pictures
            .click
            .is_some()
            .then_some((self.now + ART_TIME, Event::Click));
        self.events.push(Event::Click);
        self.say(Trigger::Click, o);
        None
    }

    fn side(&self) -> Side {
        let Some(rect) = native::window_rect(self.hwnd) else {
            return Side::Right;
        };
        let k = self.k(rect);
        let (work, _) = native::monitor_at(self.anchor.0 as i32, self.anchor.1 as i32 - 1, false);
        self.layout
            .chosen_side(self.lines.menu.side, self.anchor.0, k, (work.0, work.2))
    }

    /// Physical pixels per logical pixel, from the window's actual size.
    fn k(&self, rect: (i32, i32, i32, i32)) -> f32 {
        ((rect.2 - rect.0) as f32 / self.layout.window.width).max(0.25)
    }

    /// Moves the window to `origin` while carried. No clamping: the
    /// character follows the cursor past screen edges and is recovered on
    /// release.
    fn carry_to(&mut self, origin: (i32, i32), k: f32) {
        self.anchor = (
            origin.0 as f32 + self.layout.anchor.x * k,
            origin.1 as f32 + self.layout.anchor.y * k,
        );
        native::move_window(self.hwnd, origin.0, origin.1, None);
    }

    /// Puts the window at the saved anchor, recovering to the nearest display
    /// (or the default spot) when that anchor is off every monitor.
    fn place(&mut self) {
        if self.hwnd == 0 {
            return;
        }
        let unplaced = f64::from(self.anchor.0) <= feature::UNPLACED + 1.0;
        let (x, y) = (self.anchor.0 as i32, self.anchor.1 as i32 - 1);
        let (work, dpi) = native::monitor_at(x, y, unplaced);
        let k = dpi as f32 / 96.0;
        self.anchor = if unplaced {
            self.layout.default_anchor(k, work)
        } else {
            self.layout.recover(self.anchor, k, work)
        };
        self.settle(work, k);
        let (x, y) = self.layout.origin(self.anchor, k);
        let size = (
            (self.layout.window.width * k).round() as i32,
            (self.layout.window.height * k).round() as i32,
        );
        native::move_window(self.hwnd, x, y, Some(size));
    }

    /// Comes to rest clear of the other characters, within the work area,
    /// and tells them where it stands.
    fn settle(&mut self, work: (i32, i32, i32, i32), k: f32) {
        let Some((crowd, id)) = &self.crowd else {
            return;
        };
        let half = self.layout.character.width * k * 0.5;
        let bounds = self.layout.range(k, work);
        self.anchor.0 = crowd.clear_x(*id, self.anchor.0, self.anchor.1, half, bounds.0, bounds.2);
        crowd.publish(super::crowd::Slot {
            id: *id,
            x: self.anchor.0,
            y: self.anchor.1,
            half,
        });
    }

    /// Re-lays out for a new size or menu length, keeping the feet in place.
    pub fn resize(&mut self, size: f32, rows: usize) {
        if (size - self.size).abs() < 0.5 && rows == self.rows {
            return;
        }
        self.size = size;
        self.rows = rows;
        self.relayout();
        self.place();
    }

    /// Sets a released character moving: thrown with the cursor's
    /// velocity `v` (physical pixels per second) when that is fast enough,
    /// and falling to the bottom of the release monitor's usable area when
    /// the character falls. Reduced motion never throws and lands at once.
    fn launch(&mut self, v: (f32, f32), reduced: bool) {
        let v = if reduced { (0.0, 0.0) } else { v };
        if self.hwnd == 0 || (v == (0.0, 0.0) && !self.fall_on_release) {
            return;
        }
        let (work, dpi) = native::monitor_at(self.anchor.0 as i32, self.anchor.1 as i32 - 1, false);
        let k = dpi as f32 / 96.0;
        let bounds = self.layout.range(k, work);
        self.flight = flight(v, k, self.fall_on_release, self.anchor, bounds);
        if reduced && self.flight.take().is_some() {
            self.anchor.1 = bounds.3;
            self.place();
        }
    }

    /// One flight step; when it comes to rest, the spot to save. Landing
    /// is not a Drop.
    fn step_flight(&mut self, k: f32) -> Option<(f64, f64)> {
        let flight = self.flight.as_mut()?;
        let dt = (self.now - self.stepped).clamp(0.0, 0.05);
        let flown = flight.step(self.anchor, k, dt);
        self.anchor = flown.at;
        let (x, y) = self.layout.origin(self.anchor, k);
        native::move_window(self.hwnd, x, y, None);
        if flown.thud {
            // A squash on landing, where motion allows.
            self.animator.compress(self.now);
        }
        if flown.done {
            self.flight = None;
            // Landed on a neighbour: step aside.
            if self.crowd.is_some() {
                self.place();
            }
            return Some((self.anchor.0 as f64, self.anchor.1 as f64));
        }
        None
    }

    pub fn reset_position(&mut self) {
        self.flight = None;
        self.anchor = (feature::UNPLACED as f32, feature::UNPLACED as f32);
        self.menu = None;
        self.place();
    }

    /// A display was added, removed or rescaled.
    /// While carried, the next poll repositions from the window's own new
    /// size instead, and release recovers.
    pub fn displays_changed(&mut self) {
        if self.press.as_ref().is_some_and(|p| p.dragging) {
            return;
        }
        // The edges a flight was bouncing off may be gone.
        self.flight = None;
        self.place();
    }

    /// Steps aside while annotation owns input; comes back at the same spot.
    /// The window change a new hidden state needs, if any.
    pub fn hide(&mut self, hidden: bool) -> Option<bool> {
        if hidden == self.hidden {
            return None;
        }
        self.hidden = hidden;
        self.companion.reset();
        if hidden {
            self.flight = None;
            // A cancelled gesture: no Drop is invented.
            self.speech = None;
            self.menu = None;
            self.press = None;
            self.events.clear();
        }
        if self.hwnd == 0 {
            return None;
        }
        self.shown = !hidden;
        Some(hidden)
    }

    /// How often to poll, or `None` when nothing needs watching. Anything
    /// under 100 ms means motion: the app then polls once per display frame.
    pub fn poll_interval(&self, o: &Options) -> Option<Duration> {
        if !self.shown {
            return None;
        }
        let settling = {
            let d = self.target - self.gaze;
            d.x.hypot(d.y) > 0.01
                || self.animator.settling()
                || self.released.is_some()
                || self.showing.is_some_and(|(until, _)| self.now < until)
        };
        Some(Duration::from_millis(
            if self.press.is_some()
                || self.flight.is_some()
                || self.animating()
                || (self.pictures.settings.zzz
                    && matches!(self.companion.state, State::Asleep | State::Waking)
                    && !o.reduced)
            {
                16
            } else if self.menu.is_some() || (o.idle && !o.reduced) || settling || o.music > 0.001 {
                33
            } else {
                // Still: only hit testing and new gaze targets.
                100
            },
        ))
    }

    /// Draws the character into `s`, whose origin is the window's top-left, in
    /// the window's logical pixels.
    pub fn paint_character<S: Surface>(&self, s: &mut S) {
        let l = &self.layout;
        let pose = &self.pose;
        s.translate(Vector::new(l.character.x, l.character.y));
        s.scale(l.scale);
        // Carried with a held pose: it hangs from its scruff at the cursor.
        if let Some(press) = self.press.as_ref().filter(|p| p.dragging)
            && let Some(held) = &self.pictures.held
        {
            let angle = pose.carry.map_or(0.0, |(_, a)| a);
            self.rig.draw_hanging(s, held, press.pivot, angle);
        } else {
            super::motion::place(s, pose, self.rig.anchor(), self.rig.size().height);
            match self.picture() {
                Some(picture) => self.rig.draw_picture(s, picture),
                None => self.rig.draw_pose(s, pose),
            }
        }
    }

    /// Sleep marks and hearts, drawn over the character.
    pub fn paint_overlay<S: Surface>(&self, s: &mut S) {
        let l = &self.layout;
        let pose = &self.pose;
        s.translate(Vector::new(l.character.x, l.character.y));
        s.scale(l.scale);
        super::motion::place(s, pose, self.rig.anchor(), self.rig.size().height);
        companionship::draw_zzz(
            s,
            self.rig.size(),
            &self.sleep_settings(),
            self.companion.state,
            self.companion.stirring,
            self.now,
            self.reduced,
        );
        if self.press.is_none() && self.flight.is_none() && self.menu.is_none() {
            self.companion.draw_affection(
                s,
                self.rig.size(),
                self.rig.visible_bounds(),
                self.now,
                self.reduced,
            );
        }
    }

    /// The window's physical rectangle and its physical pixels per logical
    /// pixel, once attached.
    pub fn window_scale(&self) -> Option<((i32, i32, i32, i32), f32)> {
        let rect = native::window_rect(self.hwnd)?;
        Some((rect, self.k(rect)))
    }

    /// Whether a physical screen point is over the character as drawn now.
    pub fn over_screen(&self, p: (i32, i32)) -> bool {
        let Some((rect, k)) = self.window_scale() else {
            return false;
        };
        self.over_character(Point::new(
            (p.0 - rect.0) as f32 / k,
            (p.1 - rect.1) as f32 / k,
        ))
    }

    /// Whether a window-space point lies on visible artwork as drawn now.
    fn over_character(&self, p: Point) -> bool {
        if let Some(picture) = self.picture() {
            let l = &self.layout;
            let q = Point::new(
                (p.x - l.character.x) / l.scale,
                (p.y - l.character.y) / l.scale,
            );
            let q =
                super::motion::unplaced(&self.pose, self.rig.anchor(), self.rig.size().height, q);
            return picture.hit(self.rig.size(), q);
        }
        over(&self.layout, &self.rig, &self.pose, p)
    }
}

/// How far a side bubble's bottom sits below its anchor, so the tail
/// leaves it near the bottom like the menu's.
const SIDE_DROP: f32 = 18.0;
/// Narrowest a side bubble may be before it moves above instead.
const SIDE_MIN: f32 = 96.0;

/// Where the speech bubble goes, in window coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpeechPlace {
    Above {
        left: f32,
        width: f32,
        at: Point,
    },
    Beside {
        left: f32,
        width: f32,
        /// The x of the bubble's edge facing the anchor.
        edge: f32,
        at: Point,
        side: Side,
    },
}

/// Places the bubble for `b` around its anchor. A side bubble moves above
/// when the menu is open on that side or there's no room for it.
pub fn speech_place(
    l: &Layout,
    b: super::lines::Bubble,
    visible_top: f32,
    menu: Option<Side>,
) -> SpeechPlace {
    let at = l.speech_anchor(b.anchor, visible_top);
    let side = match b.side {
        BubbleSide::Left => Some(Side::Left),
        BubbleSide::Right => Some(Side::Right),
        BubbleSide::Above => None,
    }
    .filter(|s| menu != Some(*s));
    match side {
        Some(Side::Left) => {
            let edge = at.x - SPEECH_TAIL;
            let left = (edge - b.max_width).max(0.0);
            if edge - left >= SIDE_MIN {
                return SpeechPlace::Beside {
                    left,
                    width: edge - left,
                    edge,
                    at,
                    side: Side::Left,
                };
            }
        }
        Some(Side::Right) => {
            let edge = at.x + SPEECH_TAIL;
            let width = b.max_width.min(l.window.width - edge);
            if width >= SIDE_MIN {
                return SpeechPlace::Beside {
                    left: edge,
                    width,
                    edge,
                    at,
                    side: Side::Right,
                };
            }
        }
        None => {}
    }
    let width = b.max_width.min(l.window.width);
    SpeechPlace::Above {
        left: (at.x - width / 2.0).clamp(0.0, (l.window.width - width).max(0.0)),
        width,
        at,
    }
}

/// Whether window point `p` lies on the artwork `Stage::draw` draws for
/// `pose`: the inverse of its transforms, from the window to each layer.
fn over(l: &Layout, rig: &Rig, pose: &Pose, p: Point) -> bool {
    let c = l.character;
    let drawn = Point::new((p.x - c.x) / l.scale, (p.y - c.y) / l.scale);
    let local = super::motion::unplaced(pose, rig.anchor(), rig.size().height, drawn);
    rig.hit(local, pose).is_some()
}

mod paint;
pub use paint::SceneKey;

#[cfg(test)]
mod tests;
