//! Yohta, the optional desktop character. `desktop` is the resident app's
//! character window; `run` is the standalone `--mascot-preview` window, which
//! starts no hooks, capture, tray or audio device. Both share the rig.
#[cfg(windows)]
pub mod desktop;
pub mod motion;
pub mod pack;
pub mod rig;

/// Messages from the desktop character window.
#[derive(Debug, Clone)]
pub enum Msg {
    /// Left button pressed on visible artwork.
    Pressed,
    Released,
    /// Cursor, drag and hit-test polling while visible.
    Poll,
    Native(isize),
    /// Right button pressed on visible artwork: toggle the menu.
    Menu,
    /// A menu entry that runs a registered command.
    Command(&'static str),
    /// Open Settings on the Character page.
    CharacterSettings,
}

use iced::widget::canvas::{self as drawing, Frame, Geometry};
use iced::widget::{button, canvas, checkbox, column, container, row, slider, text};
use iced::{
    Color, Element, Length, Point, Rectangle, Renderer, Size, Subscription, Theme, Vector, mouse,
};
use rig::Rig;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub fn run() -> iced::Result {
    iced::application(Preview::new, Preview::update, Preview::view)
        .title("TinyPaw · Yohta sitting preview")
        .theme(Theme::Dark)
        .subscription(Preview::subscription)
        .window_size(Size::new(980.0, 790.0))
        .centered()
        .antialiasing(true)
        .run()
}

#[derive(Debug, Clone)]
enum Message {
    Snapshot(iced::window::Screenshot),
    Tick,
    Gaze(Vector),
    Menu,
    Idle(bool),
    Track(bool),
    Beat(bool),
    Background(bool),
    Scale(f32),
    Tempo(f32),
    Pick(&'static str),
}

struct Preview {
    snapshot: Option<PathBuf>,
    snapshot_pending: bool,
    snapshot_scene: String,
    rig: Rig,
    started: Instant,
    now: f32,
    gaze: Vector,
    target: Vector,
    idle: bool,
    track: bool,
    beat: bool,
    light: bool,
    size: f32,
    tempo: f32,
    menu: bool,
    note: &'static str,
}

impl Preview {
    fn new() -> Self {
        let args = std::env::args().collect::<Vec<_>>();
        let scene = args
            .windows(2)
            .find(|pair| pair[0] == "--snapshot-scene")
            .map_or("normal", |pair| pair[1].as_str());
        Self {
            snapshot_pending: false,
            snapshot_scene: scene.to_owned(),
            snapshot: std::env::args()
                .collect::<Vec<_>>()
                .windows(2)
                .find(|pair| pair[0] == "--snapshot")
                .map(|pair| PathBuf::from(&pair[1])),
            rig: Rig::load(),
            started: Instant::now(),
            now: 0.0,
            gaze: Vector::ZERO,
            target: Vector::ZERO,
            idle: true,
            track: true,
            beat: scene == "menu",
            light: scene == "light",
            size: if scene == "small" { 96.0 } else { 390.0 },
            tempo: 110.0,
            menu: scene == "menu",
            note: "Move the cursor around the preview. Click Yohta to open the menu.",
        }
    }

    fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::Snapshot(screenshot) => {
                if let Some(path) = self.snapshot.take()
                    && let Err(error) = image::save_buffer(
                        &path,
                        &screenshot.rgba,
                        screenshot.size.width,
                        screenshot.size.height,
                        image::ColorType::Rgba8,
                    )
                {
                    eprintln!("mascot snapshot failed: {error}");
                }
                return iced::exit();
            }
            Message::Tick => {
                self.now = self.started.elapsed().as_secs_f32();
                let target = if self.track {
                    self.target
                } else {
                    Vector::ZERO
                };
                self.gaze = self.gaze + (target - self.gaze) * 0.24;
                if self.snapshot.is_some() && self.now > 0.6 {
                    if self.snapshot_scene == "blink" {
                        self.now = 3.55;
                    }
                    // Let the new scene draw before capturing the compositor's
                    // previous frame on the next tick.
                    if !self.snapshot_pending {
                        self.snapshot_pending = true;
                        return iced::Task::none();
                    }
                    return iced::window::oldest().then(|id| match id {
                        Some(id) => iced::window::screenshot(id).map(Message::Snapshot),
                        None => iced::Task::none(),
                    });
                }
            }
            Message::Gaze(target) => self.target = target,
            Message::Menu => self.menu = !self.menu,
            Message::Idle(value) => self.idle = value,
            Message::Track(value) => {
                self.track = value;
                if !value {
                    self.gaze = Vector::ZERO;
                }
            }
            Message::Beat(value) => self.beat = value,
            Message::Background(value) => self.light = value,
            Message::Scale(value) => self.size = value,
            Message::Tempo(value) => self.tempo = value,
            Message::Pick(tool) => {
                self.note = tool;
                self.menu = false;
            }
        }
        iced::Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        // No frame-rate wakeups when all motion is disabled and the eyes settle.
        let delta = self.target - self.gaze;
        if self.idle || self.beat || (self.track && delta.x.hypot(delta.y) > 0.01) {
            iced::time::every(Duration::from_millis(33)).map(|_| Message::Tick)
        } else {
            Subscription::none()
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let stage = canvas(Stage { preview: self })
            .width(Length::Fill)
            .height(Length::Fill);
        let background = if self.light {
            Color::from_rgb8(218, 224, 235)
        } else {
            Color::from_rgb8(19, 24, 37)
        };
        let stage = container(stage)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(background.into()),
                ..Default::default()
            });
        let mut controls = column![
            text("YOHTA").size(26),
            text("Sitting · animation prototype").size(14),
            text(""),
            checkbox(self.track)
                .label("Eyes follow cursor")
                .on_toggle(Message::Track),
            checkbox(self.idle)
                .label("Idle motion + blinking")
                .on_toggle(Message::Idle),
            checkbox(self.beat)
                .label("Simulate music bob")
                .on_toggle(Message::Beat),
            text("Size").size(14),
            slider(96.0..=440.0, self.size, Message::Scale).step(1.0_f32),
            text(format!("{:.0} px tall", self.size)).size(13),
            checkbox(self.light)
                .label("Light background")
                .on_toggle(Message::Background),
        ]
        .spacing(14);
        if self.beat {
            controls = controls
                .push(text(format!("Demo tempo · {:.0} BPM", self.tempo)).size(14))
                .push(slider(60.0..=180.0, self.tempo, Message::Tempo).step(1.0_f32));
        }
        if self.menu {
            controls = controls.push(text("Yohta menu").size(20));
            for (label, note) in [
                ("Pointer", "Pointer selected — preview only."),
                ("Draw", "Draw selected — preview only."),
                ("Sound", "Sound selected — preview only."),
                ("Settings", "Settings selected — preview only."),
            ] {
                controls = controls.push(
                    button(label)
                        .width(Length::Fill)
                        .on_press(Message::Pick(note)),
                );
            }
        }
        controls = controls.push(text(self.note).size(13)).push(
            text("Music uses a demo beat here. Menu actions do not change the running app.")
                .size(12),
        );
        column![
            row![stage, container(controls).padding(24).width(290)].height(Length::Fill),
            container(
                text("TinyPaw / approved sitting design · cursor eyes · neck pivot · tail pivot")
                    .size(13)
            )
            .padding(12),
        ]
        .into()
    }
}

struct Stage<'a> {
    preview: &'a Preview,
}

#[derive(Default)]
struct StageState {
    last_target: Option<Vector>,
    pressed: bool,
}

fn placement(bounds: Size, height: f32) -> (Vector, f32) {
    let scale = (height / rig::HEIGHT)
        .min((bounds.width - 40.0).max(1.0) / rig::WIDTH)
        .min((bounds.height - 40.0).max(1.0) / rig::HEIGHT);
    let origin = Vector::new(
        (bounds.width - rig::WIDTH * scale) * 0.5,
        (bounds.height - rig::HEIGHT * scale) * 0.5,
    );
    (origin, scale)
}

impl drawing::Program<Message> for Stage<'_> {
    type State = StageState;

    fn update(
        &self,
        state: &mut StageState,
        event: &drawing::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<drawing::Action<Message>> {
        let (origin, scale) = placement(bounds.size(), self.preview.size);
        let hit = cursor.position_in(bounds).is_some_and(|point| {
            let local = Point::new((point.x - origin.x) / scale, (point.y - origin.y) / scale);
            Rectangle::new(Point::ORIGIN, Size::new(544.0, rig::HEIGHT)).contains(local)
        });
        match event {
            drawing::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if hit => {
                state.pressed = true;
                Some(drawing::Action::capture())
            }
            drawing::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let pressed = std::mem::take(&mut state.pressed);
                (pressed && hit).then(|| drawing::Action::publish(Message::Menu).and_capture())
            }
            drawing::Event::Mouse(mouse::Event::CursorMoved { .. } | mouse::Event::CursorLeft) => {
                let target = cursor.position_in(bounds).map_or(Vector::ZERO, |point| {
                    let eye = Point::new(origin.x + 232.0 * scale, origin.y + 360.0 * scale);
                    Vector::new(
                        ((point.x - eye.x) / 180.0).clamp(-1.0, 1.0),
                        ((point.y - eye.y) / 150.0).clamp(-1.0, 1.0),
                    )
                });
                if state.last_target != Some(target) {
                    state.last_target = Some(target);
                    Some(drawing::Action::publish(Message::Gaze(target)))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _: &StageState,
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let p = self.preview;
        let (origin, scale) = placement(bounds.size(), p.size);
        let now = if p.snapshot_scene == "blink" {
            3.55
        } else {
            p.now
        };
        let motion = motion(now, p.idle, p.beat, p.tempo);
        frame.translate(origin + Vector::new(0.0, motion.bounce * scale));
        frame.scale(scale);
        // Counter-rotate the target into the head's local space so eye tracking
        // remains directional while the neck tilts.
        let (s, c) = (-motion.head).sin_cos();
        let gaze = Vector::new(p.gaze.x * c - p.gaze.y * s, p.gaze.x * s + p.gaze.y * c);
        p.rig
            .draw(&mut frame, gaze, motion.head, motion.tail, motion.blink);
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _: &StageState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        let (origin, scale) = placement(bounds.size(), self.preview.size);
        if cursor.position_in(bounds).is_some_and(|p| {
            Rectangle::new(
                Point::new(origin.x, origin.y),
                Size::new(544.0 * scale, rig::HEIGHT * scale),
            )
            .contains(p)
        }) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

pub(crate) struct Motion {
    pub head: f32,
    pub tail: f32,
    pub bounce: f32,
    pub blink: bool,
}

pub(crate) fn motion(now: f32, idle: bool, beat: bool, tempo: f32) -> Motion {
    let phase = now * tempo / 60.0 * std::f32::consts::TAU;
    Motion {
        head: if beat {
            phase.sin() * 0.045
        } else if idle {
            (now * 1.4).sin() * 0.018
        } else {
            0.0
        },
        tail: if idle { (now * 1.1).sin() * 0.055 } else { 0.0 },
        bounce: if beat {
            -phase.sin().max(0.0) * 10.0
        } else {
            0.0
        },
        blink: idle && ((now + 1.2) % 4.7 < 0.15),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reduced_motion_has_no_animation() {
        let p = motion(5.7, false, false, 110.0);
        assert_eq!((p.head, p.tail, p.bounce, p.blink), (0.0, 0.0, 0.0, false));
    }
    #[test]
    fn small_viewport_fits_the_entire_rig() {
        let bounds = Size::new(180.0, 220.0);
        let (origin, scale) = placement(bounds, 440.0);
        assert!(origin.x >= 0.0 && origin.y >= 0.0);
        assert!(origin.x + rig::WIDTH * scale <= bounds.width);
        assert!(origin.y + rig::HEIGHT * scale <= bounds.height);
    }
}
