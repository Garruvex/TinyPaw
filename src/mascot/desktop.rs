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
use super::motion::{Animator, Inputs, PartMotion, Pose};
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

/// `height` is the character's logical height; `canvas` the rig's size and
/// `feet` where it meets the ground, both in rig pixels; `rows` the number of
/// menu entries.
pub fn layout(height: f32, canvas: Size, feet: Point, rows: usize) -> Layout {
    let scale = height / canvas.height;
    let size = Size::new(canvas.width * scale, canvas.height * scale);
    let below = (canvas.height - feet.y).max(0.0) * scale;
    let menu = Size::new(MENU_W, menu_height(rows));
    let side = MENU_W + GAP;
    let h = size.height.max(menu.height + below).ceil();
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
}

/// One menu row. `checked` draws a tick column; `on_press: None` disables it.
pub struct Item {
    pub label: &'static str,
    pub checked: Option<bool>,
    pub on_press: Option<Msg>,
}

struct Press {
    cursor: (i32, i32),
    origin: (i32, i32),
    dragging: bool,
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
        let layout = layout(size, rig.size(), rig.anchor(), rows);
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
            passthrough: None,
            hidden,
            shown: false,
            left_down: false,
            right_down: false,
        };
        (desktop, task)
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
        self.layout = layout(self.size, self.rig.size(), self.rig.anchor(), self.rows);
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
                if let (Some(cursor), Some(rect)) =
                    (native::cursor_pos(), native::window_rect(self.hwnd))
                {
                    self.press = Some(Press {
                        cursor,
                        origin: (rect.0, rect.1),
                        dragging: false,
                    });
                }
                (Task::none(), None)
            }
            Msg::Released => (Task::none(), self.release()),
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

        if let Some(press) = &mut self.press {
            let delta = (cursor.0 - press.cursor.0, cursor.1 - press.cursor.1);
            if !left {
                // The release happened where the canvas could not see it.
                save = self.release();
            } else {
                if !press.dragging && is_drag(delta, k) {
                    press.dragging = true;
                    self.menu = None;
                }
                if press.dragging {
                    let origin = (press.origin.0 + delta.0, press.origin.1 + delta.1);
                    self.drag_to(origin, k);
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
        (task, save)
    }

    /// Advances idle motion and reactions to now.
    fn step_motion(&mut self, o: &Options, k: f32) {
        let dt = (self.now - self.stepped).max(0.0);
        self.stepped = self.now;
        let dragging = self.press.as_ref().is_some_and(|p| p.dragging);
        let moved = (self.anchor.0 - self.last_anchor.0, self.anchor.1 - self.last_anchor.1);
        self.last_anchor = self.anchor;
        let velocity = if dt > 0.0 {
            Vector::new(moved.0 / k / dt, moved.1 / k / dt)
        } else {
            Vector::ZERO
        };
        let mut pose = self.animator.step(
            &self.parts,
            &Inputs {
                now: self.now,
                dt,
                idle: o.idle && !o.reduced,
                reduced: o.reduced,
                pressed: self.press.as_ref().is_some_and(|p| !p.dragging),
                dragging,
                velocity,
            },
        );
        // Counter-rotate the gaze into the head's space so it stays
        // directional while the head tilts.
        let (s, c) = (-self.rig.head_angle(&pose)).sin_cos();
        let g = self.gaze;
        pose.gaze = Vector::new(g.x * c - g.y * s, g.x * s + g.y * c);
        self.pose = pose;
    }

    /// Ends a press: a short one is a click the character reacts to, a drag
    /// saves the spot. The menu is on the right button.
    fn release(&mut self) -> Option<(f64, f64)> {
        let press = self.press.take()?;
        if press.dragging {
            return Some((self.anchor.0 as f64, self.anchor.1 as f64));
        }
        self.menu = None;
        self.animator.clicked(self.now);
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

    fn drag_to(&mut self, origin: (i32, i32), k: f32) {
        let anchor = (
            origin.0 as f32 + self.layout.anchor.x * k,
            origin.1 as f32 + self.layout.anchor.y * k,
        );
        let (work, _) = native::monitor_at(anchor.0 as i32, anchor.1 as i32 - 1, false);
        self.anchor = self.layout.clamp(anchor, k, work);
        let (x, y) = self.layout.origin(self.anchor, k);
        native::move_window(self.hwnd, x, y, None);
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
            self.layout.clamp(self.anchor, k, work)
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
        self.layout = layout(size, self.rig.size(), self.rig.anchor(), rows);
        self.place();
    }

    pub fn reset_position(&mut self) {
        self.anchor = (feature::UNPLACED as f32, feature::UNPLACED as f32);
        self.menu = None;
        self.place();
    }

    /// A display was added, removed or rescaled.
    pub fn displays_changed(&mut self) {
        self.place();
    }

    /// Steps aside while annotation owns input; comes back at the same spot.
    pub fn set_hidden(&mut self, hidden: bool) -> Task<Message> {
        if hidden == self.hidden {
            return Task::none();
        }
        self.hidden = hidden;
        if hidden {
            self.menu = None;
            self.press = None;
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
            d.x.hypot(d.y) > 0.01 || self.animator.settling()
        };
        Some(Duration::from_millis(if self.press.is_some() {
            16
        } else if self.menu.is_some() || (o.idle && !o.reduced) || settling {
            33
        } else {
            // Still: only hit testing and new gaze targets.
            100
        }))
    }

    /// Whether a window-space point lies on visible artwork.
    fn over_character(&self, p: Point) -> bool {
        let c = self.layout.character;
        let local = Point::new(
            (p.x - c.x) / self.layout.scale,
            (p.y - c.y) / self.layout.scale,
        );
        self.rig.opaque(local)
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
        .width(l.character.width)
        .height(Length::Fill);
        row![
            slot(Side::Left, &mut items),
            stage,
            slot(Side::Right, &mut items)
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
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
    /// Canvas-local point to window coordinates.
    fn window_point(&self, p: Point) -> Point {
        Point::new(p.x + self.desktop.layout.character.x, p.y)
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
        frame.translate(Vector::new(0.0, l.character.y));
        frame.scale(l.scale);
        if pose.squash > 0.001 || pose.lean.abs() > 0.0005 {
            // Lean while carried, squash when pressed: both around the
            // feet, so they stay on the ground.
            let feet = d.rig.anchor();
            frame.translate(Vector::new(feet.x, feet.y));
            frame.rotate(pose.lean);
            frame.scale_nonuniform(Vector::new(
                1.0 + 0.06 * pose.squash,
                1.0 - 0.08 * pose.squash,
            ));
            frame.translate(Vector::new(-feet.x, -feet.y));
        }
        d.rig.draw_pose(&mut frame, pose);
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
            Size::new(rig::WIDTH, rig::HEIGHT),
            Point::new(rig::WIDTH * 0.5, GROUND),
            5,
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
        let l = layout(100.0, Size::new(400.0, 200.0), Point::new(100.0, 190.0), 3);
        assert!((l.character.width - 200.0).abs() < 0.01);
        assert!((l.anchor.x - (l.character.x + 50.0)).abs() < 0.01);
        assert!((l.anchor.y - (l.character.y + 95.0)).abs() < 0.01);
    }

    #[test]
    fn small_movements_stay_clicks() {
        assert!(!is_drag((3, 3), 1.0));
        assert!(is_drag((6, 0), 1.0));
        // Five logical pixels are ten physical ones at 200 %.
        assert!(!is_drag((8, 0), 2.0));
        assert!(is_drag((11, 0), 2.0));
    }
}
