//! The desktop character window: a small transparent, always-on-top surface
//! that shows Yohta where the user left him.
//!
//! The window is laid out as `[menu slot | character | menu slot]` so the menu
//! can open on whichever side has room without resizing the window. Blank
//! pixels must not swallow clicks, so the window stays click-through except
//! while the cursor is over visible artwork or the open menu; polling the
//! cursor (needed for the eyes anyway) switches that. Clicks never activate
//! the window, so the app underneath keeps keyboard focus.

use std::time::{Duration, Instant};

use iced::widget::canvas::{self, Frame, Geometry};
use iced::widget::{Column, button, canvas as canvas_widget, column, container, row, space, text};
use iced::window::{self, settings::PlatformSpecific};
use iced::{
    Alignment, Element, Length, Point, Rectangle, Renderer, Size, Task, Theme, Vector, mouse,
};

use super::Msg;
use super::lines::{BubbleSide, Lines, MenuAlign, MenuSide, Picker, SoundRef, Speech, Trigger};
use super::motion::{Animator, Expression, Inputs, PartMotion, Pose};
use super::reactions::Event;
use super::rig::Rig;
use crate::app::Message;
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

/// A released character falling straight down to its landing row.
#[derive(Debug, Clone, Copy)]
struct Fall {
    /// Physical pixels per second.
    speed: f32,
    /// The anchor's landing y, in physical pixels.
    land: f32,
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
}

/// Most a carried character leans, in radians, before the window cap.
const CARRY_LEAN: f32 = 0.15;
/// Most a held pose swings from its scruff.
const HANG_SWING: f32 = 0.6;
/// Gravity for a swinging held pose, logical pixels per second squared.
const HANG_GRAVITY: f32 = 1800.0;
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
        let (left, top, right, bottom) =
            (work.0 as f32, work.1 as f32, work.2 as f32, work.3 as f32);
        let min_x = left + (self.anchor.x - self.character.x) * k;
        let max_x = right - (self.character.x + self.character.width - self.anchor.x) * k;
        let x = if min_x <= max_x {
            anchor.0.clamp(min_x, max_x)
        } else {
            (left + right) * 0.5
        };
        let min_y = top + self.anchor.y * k;
        let y = if min_y <= bottom {
            anchor.1.clamp(min_y, bottom)
        } else {
            bottom
        };
        (x, y)
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

/// Character choices that change behaviour, read from settings each update.
#[derive(Debug, Clone, Copy)]
pub struct Options {
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
    /// The followed beat's phase, while Follow beat has one.
    pub beat: Option<f32>,
}

/// One menu row. `checked` draws a switch; `on_press: None` disables it.
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
    /// The current carry lean, eased toward the drag's.
    lean: f32,
    /// A swinging held pose: angular speed, and the last drag velocity
    /// for the acceleration that swings it.
    swing: f32,
    last_vx: f32,
}

/// Where the window goes so window point `grab` sits under `cursor`, both
/// physical, at `k` physical pixels per logical pixel.
pub fn carried_origin(cursor: (i32, i32), grab: Point, k: f32) -> (i32, i32) {
    (
        (cursor.0 as f32 - grab.x * k).round() as i32,
        (cursor.1 as f32 - grab.y * k).round() as i32,
    )
}

pub struct Desktop {
    pub id: window::Id,
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
    /// A Click or Drop picture showing until this time.
    showing: Option<(f32, Event)>,
    /// After release, fall to the screen edge instead of staying.
    fall_on_release: bool,
    falling: Option<Fall>,
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
}

impl Desktop {
    /// Decodes the rig and opens the window off-screen; it moves into place
    /// once its native handle arrives. (A window opened invisible and shown
    /// later never presents a frame with Iced 0.14 here, so it starts visible
    /// but out of sight.)
    pub fn open(
        character: &'static str,
        rig: Rig,
        size: f32,
        anchor: (f64, f64),
        rows: usize,
        hidden: bool,
    ) -> (Self, Task<window::Id>) {
        let layout = layout(
            size,
            rig.visible_height(),
            rig.size(),
            rig.anchor(),
            rows,
            0.0,
        );
        let parts = rig.motion_parts();
        let (id, task) = window::open(window::Settings {
            size: layout.window,
            position: window::Position::Specific(Point::new(-32000.0, -32000.0)),
            visible: true,
            decorations: false,
            transparent: true,
            resizable: false,
            closeable: false,
            minimizable: false,
            level: window::Level::AlwaysOnTop,
            exit_on_close_request: false,
            platform_specific: PlatformSpecific {
                skip_taskbar: true,
                drag_and_drop: false,
                ..PlatformSpecific::default()
            },
            ..window::Settings::default()
        });
        let desktop = Self {
            id,
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
            showing: None,
            fall_on_release: false,
            falling: None,
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
        };
        (desktop, task)
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
        if !o.lines || self.hidden || (t.unprompted() && o.quiet) {
            return;
        }
        if let Some(line) = self.picker.pick(&self.lines, t)
            && let Some(speech) = Speech::new(&line.text, self.now)
        {
            self.speech = Some(speech);
            self.speech_at = self.now;
            self.sound = line.sound.clone();
        }
    }

    /// Keeps the bubble up while its sound plays.
    pub fn hold_speech(&mut self, seconds: f32) {
        if let Some(s) = &mut self.speech {
            s.hold(self.now, seconds);
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
            self.falling = None;
        }
    }

    pub fn set_pictures(&mut self, pictures: Pictures) {
        self.pictures = pictures;
        self.showing = None;
    }

    /// The picture drawn instead of the character now, if any. Carrying
    /// shows the held pose; a recent click or drop shows its artwork.
    fn picture(&self) -> Option<&super::rig::Picture> {
        if self.press.as_ref().is_some_and(|p| p.dragging) {
            return self.pictures.held.as_ref();
        }
        match self.showing {
            Some((until, Event::Click)) if self.now < until => self.pictures.click.as_ref(),
            Some((until, Event::Drop)) if self.now < until => self.pictures.drop.as_ref(),
            _ => None,
        }
    }

    /// Gesture events (Click, Pickup, Drop) since the last call.
    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Visible and not busy with a press or its menu: idle may react.
    pub fn idle_free(&self) -> bool {
        !self.hidden && self.shown && self.press.is_none() && self.menu.is_none()
    }

    pub fn is_hidden(&self) -> bool {
        self.hidden
    }

    /// The selected character this window shows.
    pub fn character(&self) -> &'static str {
        self.character
    }

    /// Shows another character, keeping its feet where these were.
    pub fn set_rig(&mut self, character: &'static str, rig: Rig) {
        self.character = character;
        self.parts = rig.motion_parts();
        self.animator = Animator::default();
        self.rig = rig;
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
    }

    /// The native window, once known; 0 before then.
    pub fn hwnd(&self) -> isize {
        self.hwnd
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
    pub fn update(&mut self, msg: Msg, o: &Options) -> (Task<Message>, Option<(f64, f64)>) {
        match msg {
            Msg::Native(hwnd) => {
                self.hwnd = hwnd;
                native::never_activate(hwnd);
                // Display, DPI, lock and suspend notifications while Off.
                native::observe(hwnd);
                self.place();
                self.passthrough = Some(true);
                let mut tasks = vec![window::enable_mouse_passthrough(self.id)];
                self.shown = !self.hidden;
                if self.hidden {
                    tasks.push(window::set_mode(self.id, window::Mode::Hidden));
                }
                (Task::batch(tasks), None)
            }
            Msg::Pressed => {
                // Picking it up mid-fall stops the fall at once.
                self.falling = None;
                if let (Some(cursor), Some(rect)) =
                    (native::cursor_pos(), native::window_rect(self.hwnd))
                {
                    let k = self.k(rect);
                    let grab = Point::new(
                        (cursor.0 - rect.0) as f32 / k,
                        (cursor.1 - rect.1) as f32 / k,
                    );
                    let (pivot, protected) = self.grab_chain(grab);
                    self.press = Some(Press {
                        cursor,
                        grab,
                        pose: self.pose.clone(),
                        dragging: false,
                        pivot,
                        protected,
                        lean: 0.0,
                        swing: 0.0,
                        last_vx: 0.0,
                    });
                }
                (Task::none(), None)
            }
            Msg::Released => (Task::none(), self.release(o)),
            Msg::Menu => {
                self.reduced = o.reduced;
                if self.menu.is_some() {
                    self.close_menu();
                } else {
                    self.open_menu();
                }
                (Task::none(), None)
            }
            Msg::Poll => self.poll(o),
            Msg::Command(_) | Msg::CharacterSettings => (Task::none(), None),
        }
    }

    fn poll(&mut self, o: &Options) -> (Task<Message>, Option<(f64, f64)>) {
        self.now = self.started.elapsed().as_secs_f32();
        self.reduced = o.reduced;
        if self
            .closing
            .is_some_and(|(_, at)| self.now - at >= MENU_CLOSE)
        {
            self.closing = None;
        }
        if !self.shown {
            return (Task::none(), None);
        }
        let mut picked_up = false;
        let (Some(cursor), Some(rect)) = (native::cursor_pos(), native::window_rect(self.hwnd))
        else {
            return (Task::none(), None);
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
            && let Some(landed) = self.step_fall(k)
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
        let task = if self.passthrough != Some(!hit) {
            self.passthrough = Some(!hit);
            if hit {
                window::disable_mouse_passthrough(self.id)
            } else {
                window::enable_mouse_passthrough(self.id)
            }
        } else {
            Task::none()
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
        self.step_motion(o, k);
        if picked_up {
            self.events.push(Event::Pickup);
            self.say(Trigger::Pickup, o);
        }
        self.step_speech(o);
        (task, save)
    }

    /// Moves a line along, and says an idle line when one is due and the
    /// character isn't busy or behind a full-screen app.
    fn step_speech(&mut self, o: &Options) {
        if let Some(s) = &mut self.speech {
            let part = s.index;
            if !s.advance(self.now) {
                self.speech = None;
            } else if s.index != part {
                // Each part of a line pops in afresh.
                self.speech_at = self.now;
            }
        }
        if self.now < self.next_idle {
            return;
        }
        let free = self.press.is_none() && self.menu.is_none() && self.speech.is_none();
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
                        bop: self.rig.bop(),
                        ..Inputs::default()
                    },
                );
                for (i, angle) in pose.angles.iter_mut().enumerate() {
                    if !press.protected.get(i).copied().unwrap_or(true) {
                        *angle = live.angles.get(i).copied().unwrap_or(*angle);
                    }
                }
                let dt = dt.clamp(0.0, 0.05);
                let (lean, swing) = if let Some(held) = &self.pictures.held {
                    // A held pose hangs from its scruff: a damped pendulum
                    // the drag's acceleration swings, so shaking it shows.
                    let length = (held.hold_in(self.rig.size()).distance(Point::new(
                        self.rig.size().width / 2.0,
                        self.rig.visible_middle(),
                    )) * self.layout.scale)
                        .max(20.0);
                    let accel = if dt > 0.0 {
                        (velocity.x - press.last_vx) / dt
                    } else {
                        0.0
                    };
                    let a = press.lean;
                    let alpha = (accel.clamp(-20_000.0, 20_000.0) * a.cos()
                        - HANG_GRAVITY * a.sin())
                        / length
                        - 4.0 * press.swing;
                    let swing = press.swing + alpha * dt;
                    ((a + swing * dt).clamp(-HANG_SWING, HANG_SWING), swing)
                } else {
                    // Upright: it trails the drag. Held above its middle,
                    // the feet swing behind; held below, the head does.
                    let hanging = press.pivot.y < self.rig.visible_middle();
                    let sign = if hanging { 1.0 } else { -1.0 };
                    let want = (sign * velocity.x * 0.0005).clamp(-CARRY_LEAN, CARRY_LEAN);
                    (
                        press.lean + (want - press.lean) * (1.0 - (-dt * 10.0).exp()),
                        0.0,
                    )
                };
                let lean = self.fit_carry(press.grab, press.pivot, lean);
                pose.carry = Some((press.pivot, lean));
                if let Some(p) = &mut self.press {
                    p.lean = lean;
                    p.swing = if lean == 0.0 && swing != 0.0 {
                        0.0
                    } else {
                        swing
                    };
                    p.last_vx = velocity.x;
                }
            }
            pose.gaze = self.head_gaze(&pose);
            self.pose = pose;
            return;
        }
        let mut pose = self.animator.step(
            &self.parts,
            &Inputs {
                now: self.now,
                dt,
                idle: o.idle && !o.reduced,
                reduced: o.reduced,
                pressed: false,
                dragging: false,
                velocity,
                music: o.music,
                beat: o.beat,
                bop: self.rig.bop(),
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
        let mut a = lean;
        for _ in 0..12 {
            if fits(a) {
                return a;
            }
            a *= 0.7;
        }
        0.0
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
            // Left where it was put, unless that leaves it out of reach.
            self.place();
            if self.fall_on_release {
                self.start_fall(o.reduced);
            }
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
        let (x, y) = self.layout.origin(self.anchor, k);
        let size = (
            (self.layout.window.width * k).round() as i32,
            (self.layout.window.height * k).round() as i32,
        );
        native::move_window(self.hwnd, x, y, Some(size));
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

    /// Starts falling straight down from the release spot to the bottom of
    /// the release monitor's usable area. Reduced motion lands at once.
    fn start_fall(&mut self, reduced: bool) {
        let (work, _) = native::monitor_at(self.anchor.0 as i32, self.anchor.1 as i32 - 1, false);
        let land = work.3 as f32;
        if self.anchor.1 >= land - 0.5 {
            return;
        }
        if reduced {
            self.anchor.1 = land;
            self.place();
        } else {
            self.falling = Some(Fall { speed: 0.0, land });
        }
    }

    /// One fall step; on landing, the spot to save. Landing is not a Drop.
    fn step_fall(&mut self, k: f32) -> Option<(f64, f64)> {
        let fall = self.falling.as_mut()?;
        let dt = (self.now - self.stepped).clamp(0.0, 0.05);
        fall.speed += GRAVITY * k * dt;
        let y = (self.anchor.1 + fall.speed * dt).min(fall.land);
        let landed = y >= fall.land;
        self.anchor.1 = y;
        let (x, oy) = self.layout.origin(self.anchor, k);
        native::move_window(self.hwnd, x, oy, None);
        if landed {
            self.falling = None;
            // A squash on landing, where motion allows.
            self.animator.compress(self.now);
            return Some((self.anchor.0 as f64, self.anchor.1 as f64));
        }
        None
    }

    pub fn reset_position(&mut self) {
        self.falling = None;
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
        self.place();
    }

    /// Steps aside while annotation owns input; comes back at the same spot.
    pub fn set_hidden(&mut self, hidden: bool) -> Task<Message> {
        if hidden == self.hidden {
            return Task::none();
        }
        self.hidden = hidden;
        if hidden {
            self.falling = None;
            // A cancelled gesture: no Drop is invented.
            self.speech = None;
            self.menu = None;
            self.press = None;
            self.events.clear();
        }
        if self.hwnd == 0 {
            return Task::none();
        }
        self.shown = !hidden;
        window::set_mode(
            self.id,
            if hidden {
                window::Mode::Hidden
            } else {
                window::Mode::Windowed
            },
        )
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
            if self.press.is_some() || self.falling.is_some() || self.animating() {
                16
            } else if self.menu.is_some() || (o.idle && !o.reduced) || settling || o.music > 0.001 {
                33
            } else {
                // Still: only hit testing and new gaze targets.
                100
            },
        ))
    }

    /// Whether a window-space point lies on visible artwork as drawn now.
    fn over_character(&self, p: Point) -> bool {
        over(&self.layout, &self.rig, &self.pose, p)
    }

    pub fn view<'a>(&'a self, items: Vec<Item>, o: &Options) -> Element<'a, Message> {
        let l = &self.layout;
        let slot = |side: Side, items: &mut Option<Vec<Item>>| -> Element<'a, Message> {
            let width = MENU_W + GAP;
            match (self.menu == Some(side), items.take()) {
                (true, Some(items)) => {
                    let r = l.menu(side);
                    // The bubble itself is drawn on the stage canvas, which
                    // repaints whole every frame, so it can pop and fade.
                    let rows = items.into_iter().enumerate().map(|(i, item)| {
                        let hover = self.hover.get(i).copied().unwrap_or(0.0);
                        menu_row(item, hover, self.row_reveal(i))
                    });
                    let menu = container(Column::with_children(rows))
                        .padding(MENU_PAD)
                        .width(MENU_W)
                        .height(r.height);
                    let menu = container(menu).width(width).align_x(match side {
                        Side::Left => Alignment::Start,
                        Side::Right => Alignment::End,
                    });
                    column![space().height(r.y), menu].width(width).into()
                }
                (_, rest) => {
                    *items = rest;
                    space().width(width).into()
                }
            }
        };
        let mut items = Some(items);
        let _ = o;
        let stage = canvas_widget(Stage { desktop: self })
            .width(Length::Fill)
            .height(Length::Fill);
        let menus = row![
            slot(Side::Left, &mut items),
            space().width(l.character.width),
            slot(Side::Right, &mut items)
        ]
        .width(Length::Fill)
        .height(Length::Fill);
        let body = iced::widget::stack![stage, menus];
        // Always a stack with a bubble layer, so a bubble appearing doesn't
        // change the widget structure under the stage's state.
        let Some(speech) = self.speech.as_ref().filter(|_| o.lines) else {
            return iced::widget::stack![body, space()].into();
        };
        // The bubble sits by its anchor, above the head unless the
        // character says otherwise, kept inside the window. It never takes
        // clicks.
        let b = self.lines.bubble;
        let place = speech_place(l, b, self.rig.visible_rows().0, self.menu);
        let width = match place {
            SpeechPlace::Above { width, .. } | SpeechPlace::Beside { width, .. } => width,
        };
        // Pops up from its anchor, with a little bounce.
        let (pop, alpha) = if self.reduced {
            (1.0, 1.0)
        } else {
            let t = (self.now - self.speech_at) / SPEECH_POP;
            (ease_out_back(t), (t * 3.0).min(1.0))
        };
        let lift = ((1.0 - pop) * 10.0).max(0.0);
        let fill = faded(
            iced::Color::from_rgb8(b.background[0], b.background[1], b.background[2]),
            alpha,
        );
        let ink = faded(
            iced::Color::from_rgb8(b.text[0], b.text[1], b.text[2]),
            alpha,
        );
        let line = faded(ink, 0.28);
        let bubble = container(
            text(speech.text().to_owned())
                .size(15)
                .font(theme::MEDIUM)
                .color(ink),
        )
        .padding([10, 15])
        .max_width(width)
        .style(move |_: &Theme| container::Style {
            background: Some(fill.into()),
            border: iced::Border {
                radius: 16.0.into(),
                width: 1.5,
                color: line,
            },
            ..container::Style::default()
        });
        let grow = SPEECH_TAIL * pop.clamp(0.0, 1.0);
        let (band, base, tip, normal) = match place {
            // Beside the anchor: the bubble's bottom a little below it, the
            // tail pointing straight at it from the bubble's edge.
            SpeechPlace::Beside {
                left,
                width,
                edge,
                at,
                side,
            } => {
                let band = container(row![
                    space().width(left),
                    container(bubble).width(width).align_x(match side {
                        Side::Left => Alignment::End,
                        Side::Right => Alignment::Start,
                    })
                ])
                .width(Length::Fill)
                .height((at.y + SIDE_DROP + lift).min(l.window.height).max(1.0))
                .align_y(Alignment::End);
                let base = Point::new(edge, at.y + lift);
                let (tip, normal) = match side {
                    Side::Left => (Point::new(edge + grow, base.y), Vector::new(1.0, 0.0)),
                    Side::Right => (Point::new(edge - grow, base.y), Vector::new(-1.0, 0.0)),
                };
                (band, base, tip, normal)
            }
            // Above the anchor, centred on it and kept inside the window.
            SpeechPlace::Above { left, width, at } => {
                let bottom = at.y - SPEECH_TAIL - lift;
                let band = container(row![
                    space().width(left),
                    container(bubble).width(width).center_x(width)
                ])
                .width(Length::Fill)
                .height(bottom.max(1.0))
                .align_y(Alignment::End);
                // The tail hangs from the middle of the bubble, leaning
                // toward the anchor, and grows in with the pop.
                let centre = left + width / 2.0;
                let base = Point::new(centre, bottom);
                let tip = Point::new(at.x.clamp(centre - 12.0, centre + 12.0), base.y + grow);
                (band, base, tip, Vector::new(0.0, 1.0))
            }
        };
        let tail = canvas_widget(SpeechTail {
            base,
            tip,
            normal,
            fill,
            line,
        })
        .width(Length::Fill)
        .height(Length::Fill);
        iced::widget::stack![body, band, tail].into()
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

/// One menu row: icon, label and, for a setting, a switch. `hover` (0–1)
/// lights the row and nudges it right; `reveal` (0–1) fades and slides it in.
fn menu_row(item: Item, hover: f32, reveal: f32) -> Element<'static, Message> {
    use crate::view::icons;
    let enabled = item.on_press.is_some();
    let hover = if enabled { hover } else { 0.0 };
    let (fg, glyph) = if enabled {
        (
            mix(theme::TEXT_SOFT, iced::Color::WHITE, hover),
            mix(theme::TEXT_SOFT, theme::CYAN_TEXT, hover),
        )
    } else {
        (theme::DISABLED, theme::DISABLED)
    };
    let mut content = row![
        icons::icon(item.icon, 20.0, faded(glyph, reveal)),
        text(item.label)
            .size(15)
            .font(theme::MEDIUM)
            .color(faded(fg, reveal)),
        space().width(Length::Fill),
    ]
    .spacing(12)
    .height(Length::Fill)
    .align_y(Alignment::Center);
    if let Some(on) = item.checked {
        content = content.push(switch(on, enabled, reveal));
    }
    let shift = (1.0 - reveal) * 10.0 + hover * 3.0;
    button(content)
        .width(Length::Fill)
        .height(ROW_H)
        .padding(iced::Padding {
            top: 0.0,
            right: 10.0,
            bottom: 0.0,
            left: 10.0 + shift,
        })
        .style(move |_: &Theme, status| {
            let pressed = if status == button::Status::Pressed {
                0.06
            } else {
                0.0
            };
            button::Style {
                background: Some(
                    iced::Color::from_rgba(1.0, 1.0, 1.0, (0.10 * hover + pressed) * reveal).into(),
                ),
                text_color: fg,
                border: iced::Border {
                    radius: 10.0.into(),
                    ..iced::Border::default()
                },
                ..button::Style::default()
            }
        })
        .on_press_maybe(item.on_press.map(Message::Mascot))
        .into()
}

/// A small on/off pill for menu rows that toggle a setting.
fn switch(on: bool, enabled: bool, alpha: f32) -> Element<'static, Message> {
    let track = match (on, enabled) {
        (_, false) => theme::DISABLED,
        (true, true) => theme::CYAN,
        (false, true) => iced::Color::from_rgb8(0x4A, 0x4E, 0x5A),
    };
    let knob = container(space())
        .width(12)
        .height(12)
        .style(theme::fill(faded(iced::Color::WHITE, alpha), 6.0));
    container(knob)
        .width(30)
        .height(18)
        .padding(3)
        .align_x(if on { Alignment::End } else { Alignment::Start })
        .style(theme::fill(faded(track, alpha), 9.0))
        .into()
}

/// The menu's speech-bubble outline: a rounded box with a tail on the side
/// facing the character. Returns the path and the tail's tip.
fn menu_bubble(r: Rectangle, side: Side) -> (canvas::Path, Point) {
    // Inset so the outline's stroke stays inside the menu's bounds.
    let (x, y, w, h) = (r.x + 1.5, r.y + 1.5, r.width - 3.0, r.height - 3.0);
    let rad = 16.0f32.min(h / 3.0);
    let low = y + h - rad - 2.0;
    let high = low - 16.0;
    let tip = match side {
        Side::Right => Point::new(x - MENU_TAIL, low + 8.0),
        Side::Left => Point::new(x + w + MENU_TAIL, low + 8.0),
    };
    let path = canvas::Path::new(|p| {
        p.move_to(Point::new(x + rad, y));
        p.line_to(Point::new(x + w - rad, y));
        p.arc_to(Point::new(x + w, y), Point::new(x + w, y + rad), rad);
        if side == Side::Left {
            p.line_to(Point::new(x + w, high));
            p.line_to(tip);
            p.line_to(Point::new(x + w, low));
        }
        p.line_to(Point::new(x + w, y + h - rad));
        p.arc_to(
            Point::new(x + w, y + h),
            Point::new(x + w - rad, y + h),
            rad,
        );
        p.line_to(Point::new(x + rad, y + h));
        p.arc_to(Point::new(x, y + h), Point::new(x, y + h - rad), rad);
        if side == Side::Right {
            p.line_to(Point::new(x, low));
            p.line_to(tip);
            p.line_to(Point::new(x, high));
        }
        p.line_to(Point::new(x, y + rad));
        p.arc_to(Point::new(x, y), Point::new(x + rad, y), rad);
        p.close();
    });
    (path, tip)
}

/// Draws the menu bubble scaled about its tail's tip, so it pops out of
/// the character.
fn draw_menu(frame: &mut Frame, r: Rectangle, side: Side, scale: f32, alpha: f32) {
    let (path, tip) = menu_bubble(r, side);
    frame.with_save(|f| {
        f.translate(Vector::new(tip.x, tip.y));
        f.scale(scale);
        f.translate(Vector::new(-tip.x, -tip.y));
        f.with_save(|f| {
            f.translate(Vector::new(0.0, 3.0));
            f.fill(&path, iced::Color::from_rgba(0.0, 0.0, 0.0, 0.28 * alpha));
        });
        f.fill(
            &path,
            iced::Color::from_rgba8(0x1D, 0x20, 0x2C, 0.97 * alpha),
        );
        f.stroke(
            &path,
            canvas::Stroke::default()
                .with_color(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.92 * alpha))
                .with_width(2.0)
                .with_line_join(canvas::LineJoin::Round),
        );
    });
}

/// The speech bubble's tail, drawn over the bubble's bottom edge so the
/// two read as one outline. It never takes clicks.
struct SpeechTail {
    base: Point,
    tip: Point,
    /// Out of the bubble, toward the anchor: the base runs across it.
    normal: Vector,
    fill: iced::Color,
    line: iced::Color,
}

impl canvas::Program<Message> for SpeechTail {
    type State = ();

    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let (b, tip, n) = (self.base, self.tip, self.normal);
        // Across the base, and a little back into the bubble so the fill
        // covers its edge.
        let across = Vector::new(-n.y, n.x) * 8.0;
        let back = n * -2.0;
        let (one, two) = (b - across, b + across);
        let shape = canvas::Path::new(|p| {
            p.move_to(one + back);
            p.line_to(tip);
            p.line_to(two + back);
            p.close();
        });
        frame.fill(&shape, self.fill);
        let edges = canvas::Path::new(|p| {
            p.move_to(one);
            p.line_to(tip);
            p.line_to(two);
        });
        frame.stroke(
            &edges,
            canvas::Stroke::default()
                .with_color(self.line)
                .with_width(1.5)
                .with_line_join(canvas::LineJoin::Round),
        );
        vec![frame.into_geometry()]
    }
}

struct Stage<'a> {
    desktop: &'a Desktop,
}

impl Stage<'_> {
    /// Canvas-local point to window coordinates: the stage fills the window.
    fn window_point(&self, p: Point) -> Point {
        p
    }
}

impl canvas::Program<Message> for Stage<'_> {
    type State = ();

    fn update(
        &self,
        _: &mut (),
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let over = cursor
            .position_in(bounds)
            .is_some_and(|p| self.desktop.over_character(self.window_point(p)));
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if over => {
                Some(canvas::Action::publish(Message::Mascot(Msg::Pressed)).and_capture())
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right))
                if over && self.desktop.press.is_none() =>
            {
                Some(canvas::Action::publish(Message::Mascot(Msg::Menu)).and_capture())
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if self.desktop.press.is_some() =>
            {
                Some(canvas::Action::publish(Message::Mascot(Msg::Released)).and_capture())
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let d = self.desktop;
        let l = &d.layout;
        let pose = &d.pose;
        let mut frame = Frame::new(renderer, bounds.size());
        frame.translate(Vector::new(l.character.x, l.character.y));
        frame.scale(l.scale);
        // Carried with a held pose: it hangs from its scruff at the cursor.
        if let Some(press) = d.press.as_ref().filter(|p| p.dragging)
            && let Some(held) = &d.pictures.held
        {
            let angle = pose.carry.map_or(0.0, |(_, a)| a);
            d.rig.draw_hanging(&mut frame, held, press.pivot, angle);
        } else {
            super::motion::place(&mut frame, pose, d.rig.anchor(), d.rig.size().height);
            match d.picture() {
                Some(picture) => d.rig.draw_picture(&mut frame, picture),
                None => d.rig.draw_pose(&mut frame, pose),
            }
        }
        let mut geometry = vec![frame.into_geometry()];
        if let Some((side, scale, alpha)) = d.menu_look() {
            let mut ui = Frame::new(renderer, bounds.size());
            draw_menu(&mut ui, l.menu(side), side, scale, alpha);
            geometry.push(ui.into_geometry());
        }
        geometry
    }

    fn mouse_interaction(
        &self,
        _: &(),
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if self.desktop.press.as_ref().is_some_and(|p| p.dragging) {
            mouse::Interaction::Grabbing
        } else if cursor
            .position_in(bounds)
            .is_some_and(|p| self.desktop.over_character(self.window_point(p)))
        {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

#[cfg(test)]
mod tests;
