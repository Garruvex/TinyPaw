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
use super::lines::{Lines, Picker, SoundRef, Speech, Trigger};
use super::motion::{Animator, Expression, Inputs, PartMotion, Pose};
use super::reactions::Event;
use super::rig::Rig;
use crate::app::Message;
use crate::features::mascot as feature;
use crate::platform::windows as native;
use crate::view::theme;

const MENU_W: f32 = 204.0;
const ROW_H: f32 = 30.0;
const MENU_PAD: f32 = 6.0;
const GAP: f32 = 8.0;
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
}

/// Room above the character for its speech bubble, in logical pixels.
const BUBBLE_BAND: f32 = 104.0;

/// `height` is the character's visible logical height and `visible` that
/// art's height in rig pixels, so transparent padding doesn't shrink it;
/// `canvas` the rig's size and `feet` where it meets the ground, both in rig
/// pixels; `rows` the number of menu entries; `top` room kept above the
/// character for its bubble.
pub fn layout(height: f32, visible: f32, canvas: Size, feet: Point, rows: usize, top: f32) -> Layout {
    let scale = height / visible.max(1.0);
    let size = Size::new(canvas.width * scale, canvas.height * scale);
    let below = (canvas.height - feet.y).max(0.0) * scale;
    let menu = Size::new(MENU_W, menu_height(rows));
    let side = MENU_W + GAP;
    let h = (size.height + top).max(menu.height + below).ceil();
    let character = Rectangle::new(Point::new(side, h - size.height), size);
    Layout {
        scale,
        character,
        anchor: Point::new(character.x + feet.x * scale, character.y + feet.y * scale),
        window: Size::new((size.width + 2.0 * side).ceil(), h),
        menu,
    }
}

impl Layout {
    /// The menu sits beside the character with its bottom level with the feet.
    pub fn menu(&self, side: Side) -> Rectangle {
        let y = (self.anchor.y - self.menu.height).max(0.0);
        let x = match side {
            Side::Left => self.character.x - GAP - MENU_W,
            Side::Right => self.character.x + self.character.width + GAP,
        };
        Rectangle::new(Point::new(x, y), self.menu)
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

/// One menu row. `checked` draws a tick column; `on_press: None` disables it.
pub struct Item {
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
        let layout = layout(size, rig.visible_height(), rig.size(), rig.anchor(), rows, 0.0);
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
        if self.lines.is_empty() {
            (self.size * 0.08).ceil()
        } else {
            BUBBLE_BAND
        }
    }

    /// The character's lines; re-lays out for the bubble band.
    pub fn set_lines(&mut self, lines: Lines, o: &Options) {
        self.lines = lines;
        self.speech = None;
        self.layout = layout(self.size, self.rig.visible_height(), self.rig.size(), self.rig.anchor(), self.rows, self.band());
        self.schedule_idle(o);
        self.place();
    }

    fn schedule_idle(&mut self, o: &Options) {
        let (min, max) = (o.idle_every.0.min(o.idle_every.1), o.idle_every.0.max(o.idle_every.1));
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
        self.layout = layout(self.size, self.rig.visible_height(), self.rig.size(), self.rig.anchor(), self.rows, self.band());
        self.place();
    }

    /// The native window, once known; 0 before then.
    pub fn hwnd(&self) -> isize {
        self.hwnd
    }

    pub fn close_menu(&mut self) {
        self.menu = None;
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
                    });
                }
                (Task::none(), None)
            }
            Msg::Released => (Task::none(), self.release(o)),
            Msg::Menu => {
                self.menu = match self.menu {
                    Some(_) => None,
                    None => Some(self.side()),
                };
                (Task::none(), None)
            }
            Msg::Poll => self.poll(o),
            Msg::Command(_) | Msg::CharacterSettings => (Task::none(), None),
        }
    }

    fn poll(&mut self, o: &Options) -> (Task<Message>, Option<(f64, f64)>) {
        self.now = self.started.elapsed().as_secs_f32();
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
        if self.press.is_none() && let Some(landed) = self.step_fall(k) {
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
                    self.menu = None;
                    picked_up = true;
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
                self.menu = None;
            }
        }

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
                let eyes = self.layout.character.position() + Vector::new(eye.x, eye.y) * self.layout.scale;
                gaze_toward(local, eyes, GAZE_REACH * self.layout.character.height)
            }
            _ => Vector::ZERO,
        };
        self.gaze = if o.reduced {
            self.target
        } else {
            self.gaze + (self.target - self.gaze) * 0.3
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
        if let Some(s) = &mut self.speech
            && !s.advance(self.now)
        {
            self.speech = None;
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
        let moved = (self.anchor.0 - self.last_anchor.0, self.anchor.1 - self.last_anchor.1);
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
                let want = (-velocity.x * 0.0004).clamp(-CARRY_LEAN, CARRY_LEAN);
                let lean = press.lean + (want - press.lean) * (1.0 - (-dt.min(0.1) * 10.0).exp());
                let lean = self.fit_carry(press.grab, lean);
                pose.carry = Some((press.pivot, lean));
                if let Some(p) = &mut self.press {
                    p.lean = lean;
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
        let pivot = Point::new((grab.x - l.character.x) / l.scale, (grab.y - l.character.y) / l.scale);
        let local = super::motion::unplaced(&self.pose, self.rig.anchor(), self.rig.size().height, pivot);
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
    fn fit_carry(&self, grab: Point, lean: f32) -> f32 {
        let l = &self.layout;
        let c = l.character;
        let corners = [
            Point::new(c.x, c.y),
            Point::new(c.x + c.width, c.y),
            Point::new(c.x, c.y + c.height),
            Point::new(c.x + c.width, c.y + c.height),
        ];
        let fits = |a: f32| {
            let (s, co) = a.sin_cos();
            corners.iter().all(|p| {
                let (x, y) = (p.x - grab.x, p.y - grab.y);
                let q = Point::new(grab.x + x * co - y * s, grab.y + x * s + y * co);
                q.x >= -0.5 && q.y >= -0.5 && q.x <= l.window.width + 0.5 && q.y <= l.window.height + 0.5
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
            self.showing = self.pictures.drop.is_some().then_some((self.now + ART_TIME, Event::Drop));
            self.events.push(Event::Drop);
            self.say(Trigger::Drop, o);
            return Some((self.anchor.0 as f64, self.anchor.1 as f64));
        }
        self.menu = None;
        self.animator.clicked(self.now);
        if self.click_compress {
            self.animator.compress(self.now);
        }
        self.showing = self.pictures.click.is_some().then_some((self.now + ART_TIME, Event::Click));
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
        self.layout.menu_side(self.anchor.0, k, work.2)
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
        self.layout = layout(size, self.rig.visible_height(), self.rig.size(), self.rig.anchor(), rows, self.band());
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

    /// How often to poll, or `None` when nothing needs watching.
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
        Some(Duration::from_millis(if self.press.is_some() || self.falling.is_some() {
            16
        } else if self.menu.is_some() || (o.idle && !o.reduced) || settling || o.music > 0.001 {
            33
        } else {
            // Still: only hit testing and new gaze targets.
            100
        }))
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
                    let rows = items.into_iter().map(menu_row);
                    let menu = container(Column::with_children(rows))
                        .padding(MENU_PAD)
                        .width(MENU_W)
                        .height(r.height)
                        // No drop shadow: it paints outside the panel's
                        // bounds, which the renderer's damage tracking does
                        // not repaint on this transparent window, so a closed
                        // menu left a stale shadow behind.
                        .style(theme::outlined(theme::SURFACE_A, 12.0, theme::LINE_STRONG));
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
        // The bubble sits in the band above the character, centred on it
        // and kept inside the window. It never takes clicks.
        let b = self.lines.bubble;
        let width = b.max_width.min(l.window.width);
        let left = (l.anchor.x - width / 2.0).clamp(0.0, (l.window.width - width).max(0.0));
        let bubble = container(
            text(speech.text().to_owned())
                .size(14)
                .color(iced::Color::from_rgba8(b.text[0], b.text[1], b.text[2], 1.0)),
        )
        .padding([8, 12])
        .max_width(width)
        .style(move |_: &Theme| container::Style {
            background: Some(
                iced::Color::from_rgba8(b.background[0], b.background[1], b.background[2], 1.0)
                    .into(),
            ),
            border: iced::Border {
                radius: 14.0.into(),
                width: 1.0,
                color: iced::Color::from_rgba8(0, 0, 0, 0.18),
            },
            ..container::Style::default()
        });
        let band = container(row![space().width(left), container(bubble).width(width).center_x(width)])
            .width(Length::Fill)
            .height(l.character.y.max(1.0))
            .align_y(Alignment::End);
        iced::widget::stack![body, band].into()
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

fn menu_row(item: Item) -> Element<'static, Message> {
    let tick = match item.checked {
        Some(true) => "✓",
        _ => "",
    };
    button(
        row![
            text(tick).size(13).color(theme::CYAN_TEXT).width(18),
            text(item.label).size(13),
        ]
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .height(ROW_H)
    .padding([5, 10])
    .style(theme::tool_button(theme::Tone::Plain))
    .on_press_maybe(item.on_press.map(Message::Mascot))
    .into()
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
        super::motion::place(&mut frame, pose, d.rig.anchor(), d.rig.size().height);
        match d.picture() {
            Some(picture) => d.rig.draw_picture(&mut frame, picture),
            None => d.rig.draw_pose(&mut frame, pose),
        }
        vec![frame.into_geometry()]
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
mod tests {
    use super::*;
    use crate::mascot::rig;

    const GROUND: f32 = 807.0;

    fn yohta(height: f32) -> Layout {
        layout(
            height,
            rig::HEIGHT,
            Size::new(rig::WIDTH, rig::HEIGHT),
            Point::new(rig::WIDTH * 0.5, GROUND),
            5,
            0.0,
        )
    }
    const WORK: (i32, i32, i32, i32) = (0, 0, 1920, 1040);

    #[test]
    fn layout_fits_character_and_menu_with_feet_on_the_anchor() {
        for height in [96.0, 160.0, 320.0] {
            let l = yohta(height);
            assert!((l.character.height - height).abs() < 0.01);
            for side in [Side::Left, Side::Right] {
                let m = l.menu(side);
                assert!(m.x >= 0.0 && m.y >= 0.0);
                assert!(m.x + m.width <= l.window.width + 0.01);
                assert!(m.y + m.height <= l.window.height + 0.01);
                // The menu never covers the character.
                assert!(m.x + m.width <= l.character.x || m.x >= l.character.x + l.character.width);
            }
            assert!(l.character.y + l.character.height <= l.window.height + 0.01);
            assert!(
                (l.menu(Side::Right).y + l.menu(Side::Right).height - l.anchor.y).abs() < 0.01
                    || l.menu(Side::Right).y == 0.0
            );
        }
    }

    #[test]
    fn clamping_keeps_the_character_on_the_work_area() {
        let l = yohta(160.0);
        for k in [1.0, 1.5, 2.0] {
            for wanted in [(-5000.0, -5000.0), (99999.0, 99999.0), (960.0, 500.0)] {
                let (x, y) = l.clamp(wanted, k, WORK);
                let (ox, oy) = l.origin((x, y), k);
                let left = ox as f32 + l.character.x * k;
                let right = ox as f32 + (l.character.x + l.character.width) * k;
                assert!(left >= -1.0 && right <= 1921.0, "{k} {wanted:?}");
                assert!(oy >= -1 && y <= 1040.0);
            }
            assert_eq!(l.clamp((960.0, 500.0), k, WORK), (960.0, 500.0));
        }
    }

    #[test]
    fn origin_round_trips_the_anchor() {
        let l = yohta(200.0);
        let (ox, oy) = l.origin((1000.0, 900.0), 1.25);
        assert!((ox as f32 + l.anchor.x * 1.25 - 1000.0).abs() <= 1.0);
        assert!((oy as f32 + l.anchor.y * 1.25 - 900.0).abs() <= 1.0);
    }

    #[test]
    fn default_spot_is_bottom_right_on_the_taskbar() {
        let l = yohta(160.0);
        let (x, y) = l.default_anchor(1.0, WORK);
        assert_eq!(y, 1040.0);
        assert!(x > 1700.0 && x < 1920.0);
    }

    #[test]
    fn menu_flips_left_near_the_right_edge() {
        let l = yohta(160.0);
        assert_eq!(l.menu_side(400.0, 1.0, 1920), Side::Right);
        assert_eq!(l.menu_side(1850.0, 1.0, 1920), Side::Left);
    }

    #[test]
    fn gaze_points_at_the_cursor_and_saturates() {
        let eyes = Point::new(100.0, 100.0);
        assert_eq!(gaze_toward(eyes, eyes, 50.0), Vector::ZERO);
        let near = gaze_toward(Point::new(125.0, 100.0), eyes, 50.0);
        assert!((near.x - 0.5).abs() < 1e-4 && near.y.abs() < 1e-4);
        let far = gaze_toward(Point::new(100.0, -900.0), eyes, 50.0);
        assert!((far.y + 1.0).abs() < 1e-4 && far.x.abs() < 1e-4);
    }

    #[test]
    fn layout_follows_a_custom_canvas_and_off_centre_feet() {
        // A wide image whose feet sit left of centre.
        let l = layout(100.0, 200.0, Size::new(400.0, 200.0), Point::new(100.0, 190.0), 3, 0.0);
        assert!((l.character.width - 200.0).abs() < 0.01);
        assert!((l.anchor.x - (l.character.x + 50.0)).abs() < 0.01);
        assert!((l.anchor.y - (l.character.y + 95.0)).abs() < 0.01);
    }

    #[test]
    fn the_bubble_band_sits_above_the_character() {
        let l = layout(100.0, 100.0, Size::new(100.0, 100.0), Point::new(50.0, 95.0), 3, BUBBLE_BAND);
        assert!(l.character.y >= BUBBLE_BAND - 0.01);
        assert!((l.window.height - (l.character.y + 100.0)).abs() < 1.0);
    }

    #[test]
    fn size_is_visible_height_so_padding_does_not_shrink_the_character() {
        // The same 100-pixel figure on a tight and a heavily padded canvas.
        let tight = layout(200.0, 100.0, Size::new(60.0, 110.0), Point::new(30.0, 105.0), 3, 0.0);
        let padded = layout(200.0, 100.0, Size::new(600.0, 1100.0), Point::new(300.0, 900.0), 3, 0.0);
        assert_eq!(tight.scale, padded.scale);
        assert!((tight.scale * 100.0 - 200.0).abs() < 1e-3);
        // Yohta: the new range shows him exactly as the old one did.
        let r = crate::mascot::rig::Rig::load();
        let ratio = f64::from(r.visible_height() / r.size().height);
        use crate::features::mascot as m;
        for (old, new) in [(m::MIN_SIZE, m::MIN_HEIGHT), (160.0, m::DEFAULT_HEIGHT), (m::MAX_SIZE, m::MAX_HEIGHT)] {
            assert!((old * ratio - new).abs() < 1.0, "{old} × {ratio} vs {new}");
        }
    }

    #[test]
    fn small_movements_stay_clicks() {
        assert!(!is_drag((3, 3), 1.0));
        assert!(is_drag((6, 0), 1.0));
        // Five logical pixels are ten physical ones at 200 %.
        assert!(!is_drag((8, 0), 2.0));
        assert!(is_drag((11, 0), 2.0));
    }

    /// Contact error: physical distance between the cursor and where the
    /// grabbed window point ends up after a carry step.
    fn contact_error(cursor: (i32, i32), grab: Point, k: f32) -> f32 {
        let o = carried_origin(cursor, grab, k);
        let held = (o.0 as f32 + grab.x * k, o.1 as f32 + grab.y * k);
        (held.0 - cursor.0 as f32).hypot(held.1 - cursor.1 as f32)
    }

    #[test]
    fn the_grabbed_point_stays_under_the_cursor() {
        // Centre, head, tail and edge grabs, in window logical pixels.
        let grabs = [(310.0, 450.0), (250.0, 200.0), (420.0, 700.0), (213.7, 801.3)];
        // A path across a negative-origin monitor and off the right edge.
        let path = [(-1500, 300), (-3, -40), (0, 0), (960, 540), (1919, 1079), (2600, 1500)];
        for k in [1.0, 1.25, 1.5, 2.0] {
            for g in grabs {
                for c in path {
                    let e = contact_error(c, Point::new(g.0, g.1), k);
                    assert!(e <= 1.0, "{k} {g:?} {c:?}: {e}");
                }
            }
        }
        // Crossing from 100 % to 200 % mid-drag: the window's own size
        // gives the new scale, and the same logical grab point holds.
        let g = Point::new(250.0, 200.0);
        for k in [1.0, 2.0, 1.5] {
            assert!(contact_error((1900, 500), g, k) <= 1.0);
        }
    }

    #[test]
    fn carrying_is_not_clamped_but_release_recovers() {
        let l = yohta(160.0);
        let k = 1.0;
        let width = l.character.width * k;
        let before = (l.anchor.x - l.character.x) * k;
        // A third past the left edge, so most of it shows: kept.
        let partly = (before - width * 0.3, 600.0);
        assert_eq!(l.recover(partly, k, WORK), partly);
        // Far off the right: brought back until half its width shows.
        let (x, _) = l.recover((99999.0, 600.0), k, WORK);
        let right = x + (l.character.x + l.character.width - l.anchor.x) * k;
        assert!((right - (1920.0 + width * 0.5)).abs() < 0.01, "{right}");
        // Above the top or below the taskbar: the same limits as clamp.
        for wanted in [(960.0, -5000.0), (960.0, 99999.0)] {
            assert_eq!(l.recover(wanted, k, WORK).1, l.clamp(wanted, k, WORK).1);
        }
        // A spot on screen is left alone.
        assert_eq!(l.recover((960.0, 500.0), k, WORK), (960.0, 500.0));
    }

    #[test]
    fn hit_testing_follows_the_drawn_lean() {
        let rig = Rig::load();
        let l = yohta(320.0);
        let rest = Pose::default();
        let leaning = Pose { lean: 0.15, squash: 1.0, ..Pose::default() };
        // A face point, drawn where the lean puts it.
        let face = Point::new(232.0, 360.0);
        let to_window = |pose: &Pose| {
            let d = super::super::motion::placed(pose, rig.anchor(), rig.size().height, face);
            Point::new(l.character.x + d.x * l.scale, l.character.y + d.y * l.scale)
        };
        assert!(over(&l, &rig, &rest, to_window(&rest)));
        assert!(over(&l, &rig, &leaning, to_window(&leaning)));
        // A corner of the window is never the character.
        assert!(!over(&l, &rig, &leaning, Point::new(1.0, 1.0)));
    }

    #[test]
    fn a_carried_character_leans_around_the_grab_and_trails_unprotected_parts() {
        let (mut d, _) = Desktop::open("yohta", Rig::load(), 146.0, (500.0, 500.0), 5, false);
        let o = Options {
            eyes: false,
            idle: true,
            reduced: false,
            lines: false,
            quiet: true,
            idle_every: (60.0, 300.0),
            music: 0.0,
            beat: None,
        };
        d.step_motion(&o, 1.0);
        let l = d.layout;
        // Grab the head, about where the face is.
        let face = Point::new(232.0, 360.0);
        let grab = Point::new(l.character.x + face.x * l.scale, l.character.y + face.y * l.scale);
        let (pivot, protected) = d.grab_chain(grab);
        assert!(protected.iter().any(|p| *p), "the head is protected");
        assert!(!protected.iter().all(|p| *p), "the tail may trail");
        let held = d.pose.clone();
        d.press = Some(Press { cursor: (0, 0), grab, pose: held.clone(), dragging: true, pivot, protected: protected.clone(), lean: 0.0 });
        let local = super::super::motion::unplaced(&held, d.rig.anchor(), d.rig.size().height, pivot);
        let mut leaned = 0.0f32;
        for _ in 0..40 {
            d.now += 0.016;
            d.last_anchor.0 -= 12.0; // carried to the right
            d.step_motion(&o, 1.0);
            let pose = &d.pose;
            leaned = leaned.max(pose.carry.map_or(0.0, |(_, a)| a.abs()));
            // The grabbed point is where it was, within a hundredth of a
            // rig pixel, and the protected chain hasn't turned.
            let drawn = super::super::motion::placed(pose, d.rig.anchor(), d.rig.size().height, local);
            assert!(drawn.distance(pivot) < 0.01, "{drawn:?} vs {pivot:?}");
            for (i, p) in protected.iter().enumerate() {
                if *p {
                    assert_eq!(pose.angles[i], held.angles[i]);
                }
            }
        }
        assert!(leaned > 0.01, "it leans while carried");
        // The lean never pushes the character out of its window.
        assert!(leaned <= CARRY_LEAN + 1e-4);
        // Release eases the lean out around the same point.
        d.release(&o);
        d.now += 0.001;
        d.step_motion(&o, 1.0);
        assert!(d.pose.carry.is_some_and(|(at, a)| at == pivot && a.abs() <= leaned + 1e-4));
    }

    #[test]
    fn a_released_character_falls_once_and_a_pickup_catches_it() {
        let (mut d, _) = Desktop::open("yohta", Rig::load(), 146.0, (500.0, 300.0), 5, false);
        let o = Options {
            eyes: false,
            idle: false,
            reduced: false,
            lines: false,
            quiet: true,
            idle_every: (60.0, 300.0),
            music: 0.0,
            beat: None,
        };
        d.falling = Some(Fall { speed: 0.0, land: 700.0 });
        let mut saves = Vec::new();
        for _ in 0..200 {
            d.now += 0.016;
            if let Some(spot) = d.step_fall(1.0) {
                saves.push(spot);
            }
            d.step_motion(&o, 1.0);
        }
        assert_eq!(saves, [(500.0, 700.0)], "lands once, exactly on the edge");
        assert!(d.take_events().is_empty(), "landing is not a Drop");
        // Picking it up mid-fall stops the fall at once.
        d.falling = Some(Fall { speed: 0.0, land: 900.0 });
        let _ = d.update(Msg::Pressed, &o);
        assert!(d.falling.is_none());
        // Off unless the character asks for it.
        d.set_fall(false);
        assert!(!d.fall_on_release);
    }

    #[test]
    fn gestures_emit_each_event_once_and_cancellation_none() {
        let (mut d, _) = Desktop::open("yohta", Rig::load(), 160.0, (500.0, 500.0), 5, false);
        let o = Options {
            eyes: false,
            idle: false,
            reduced: false,
            lines: false,
            quiet: true,
            idle_every: (60.0, 300.0),
            music: 0.0,
            beat: None,
        };
        let press = |dragging| Press {
            cursor: (0, 0),
            grab: Point::ORIGIN,
            pose: Pose::default(),
            dragging,
            pivot: Point::ORIGIN,
            protected: Vec::new(),
            lean: 0.0,
        };
        // A short press is one Click.
        d.press = Some(press(false));
        d.release(&o);
        assert_eq!(d.take_events(), [Event::Click]);
        assert!(d.take_events().is_empty(), "taken once");
        // A drag's release is one Drop and no Click (Pickup comes from the
        // poll that crosses the threshold).
        d.press = Some(press(true));
        d.release(&o);
        assert_eq!(d.take_events(), [Event::Drop]);
        // Releasing with no press, or hiding mid-drag, invents nothing.
        assert_eq!(d.release(&o), None);
        d.press = Some(press(true));
        let _ = d.set_hidden(true);
        assert_eq!(d.release(&o), None);
        assert!(d.take_events().is_empty());
    }

    #[test]
    fn a_held_press_keeps_its_pose_and_eases_out_on_release() {
        let (mut d, _) = Desktop::open("yohta", Rig::load(), 160.0, (500.0, 500.0), 5, false);
        let o = Options {
            eyes: false,
            idle: true,
            reduced: false,
            lines: false,
            quiet: true,
            idle_every: (60.0, 300.0),
            music: 1.0,
            beat: None,
        };
        for i in 1..=10 {
            d.now = i as f32 * 0.033;
            d.step_motion(&o, 1.0);
        }
        let held = d.pose.clone();
        // Reduced motion holds the whole pose, pressed or carried.
        let reduced = Options { reduced: true, ..o };
        d.press = Some(Press {
            cursor: (0, 0),
            grab: Point::ORIGIN,
            pose: held.clone(),
            dragging: false,
            pivot: Point::ORIGIN,
            protected: vec![true; held.angles.len()],
            lean: 0.0,
        });
        for (dragging, o) in [(false, o), (true, reduced)] {
            d.press.as_mut().unwrap().dragging = dragging;
            for i in 0..30 {
                d.now += 0.033;
                d.last_anchor.0 -= 40.0 * i as f32;
                d.step_motion(&o, 1.0);
                // No squash, lean, bounce or part motion while held.
                assert_eq!((d.pose.angles.clone(), d.pose.lean, d.pose.squash, d.pose.bounce),
                    (held.angles.clone(), held.lean, held.squash, held.bounce));
                assert!(d.pose.carry.is_none_or(|(_, a)| a == 0.0));
            }
            let face = if dragging { Expression::Dragging } else { Expression::Pressed };
            assert_eq!(d.pose.expression, face);
        }
        // Release: the first frame starts from the held geometry.
        d.release(&o);
        d.now += 0.001;
        d.step_motion(&o, 1.0);
        assert!((d.pose.lean - held.lean).abs() < 0.01 && (d.pose.bounce - held.bounce).abs() < 0.01);
        for _ in 0..20 {
            d.now += 0.033;
            d.step_motion(&o, 1.0);
        }
        assert!(d.released.is_none(), "back on live motion");
    }
}
