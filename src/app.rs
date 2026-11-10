//! Iced event loop: normalizes input into session events, runs requested
//! effects and keeps the overlay window in step with the session.

use std::time::Instant;

use iced::keyboard::{self, key::Named};
use iced::window::{self, settings::PlatformSpecific};
use iced::{Element, Point, Size, Subscription, Task, Theme, event, theme};

use crate::features::system;
use crate::platform::{self, External};
use crate::registry::{Dispatched, Key, Registry, ShellAction, ShortcutContext, Unavailable};
use crate::session::{
    Effect, Event, Millis, Mode, OutputId, OverlayState, PointerEvent, PointerKind, Pt, Session,
    Ticket,
};
use crate::view::{self, InkCache};

/// Phase 1 drives a single overlay on the primary display.
const PRIMARY: OutputId = OutputId(0);

#[derive(Debug, Clone)]
pub enum Message {
    External(External),
    OverlayOpened(window::Id),
    CloseRequested(window::Id),
    Pointer(PointerKind, Point),
    Key(keyboard::Key, keyboard::Modifiers, window::Id),
    Command(&'static str),
    Acquired(Ticket),
    Frame,
}

struct Overlay {
    id: window::Id,
    ready: bool,
    /// What was last applied: `Some(true)` interactive, `Some(false)` click-through.
    applied: Option<bool>,
}

pub struct App {
    registry: &'static Registry,
    session: Session,
    started: Instant,
    overlay: Option<Overlay>,
    previous_focus: Option<isize>,
    hotkeys: Option<platform::hotkeys::Hotkeys>,
    tray: Option<platform::tray::Tray>,
    ink: InkCache,
}

pub fn run(registry: &'static Registry) -> iced::Result {
    iced::daemon(move || App::boot(registry), App::update, App::view)
        .title(|_: &App, _| "TinyPaw".to_owned())
        .subscription(App::subscription)
        .style(|_: &App, _: &Theme| theme::Style {
            background_color: iced::Color::TRANSPARENT,
            text_color: iced::Color::WHITE,
        })
        .theme(|_: &App, _| Theme::Dark)
        .antialiasing(true)
        .run()
}

fn overlay_settings() -> window::Settings {
    window::Settings {
        size: Size::new(800.0, 600.0),
        position: window::Position::Specific(Point::ORIGIN),
        fullscreen: true,
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
    }
}

impl App {
    fn boot(registry: &'static Registry) -> (Self, Task<Message>) {
        let tool = registry.default_tool();
        let session = Session::new(tool.id, tool.factory, registry.defaults());
        let hotkeys = match platform::hotkeys::register(registry) {
            Ok(h) => {
                for failure in &h.failures {
                    eprintln!("tinypaw: shortcut not registered: {failure}");
                }
                Some(h)
            }
            Err(e) => {
                eprintln!("tinypaw: global shortcuts unavailable: {e}");
                None
            }
        };
        let tray = platform::tray::build(registry)
            .inspect_err(|e| eprintln!("tinypaw: tray unavailable: {e}"))
            .ok();
        let mut app = Self {
            registry,
            session,
            started: Instant::now(),
            overlay: None,
            previous_focus: None,
            hotkeys,
            tray,
            ink: InkCache::default(),
        };
        app.sync_tray();
        (app, Task::none())
    }

    fn now(&self) -> Millis {
        self.started.elapsed().as_millis() as Millis
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        let now = self.now();
        if std::env::var_os("TINYPAW_TRACE").is_some() && !matches!(message, Message::Frame) {
            eprintln!("[{now:>7}] {message:?} mode={:?}", self.session.mode());
        }
        let mut effects = Vec::new();
        let mut shell = None;

        match message {
            Message::External(External::Hotkey { id, pressed }) => {
                if let Some(cmd) = self.hotkeys.as_ref().and_then(|h| h.command(id)) {
                    let result = if pressed {
                        self.registry.dispatch(cmd, &mut self.session, now).ok()
                    } else {
                        self.registry.dispatch_release(cmd, &mut self.session, now)
                    };
                    collect(result, &mut effects, &mut shell);
                }
            }
            Message::External(External::Menu(id)) => {
                collect(self.dispatch(&id, now).ok(), &mut effects, &mut shell);
            }
            Message::External(External::TrayClick) => {
                collect(
                    self.dispatch(system::MASTER_TOGGLE, now).ok(),
                    &mut effects,
                    &mut shell,
                );
            }
            Message::External(External::Cli { line, reply }) => {
                let (answer, result) = self.cli(&line, now);
                collect(result, &mut effects, &mut shell);
                let _ = reply.send(answer);
            }
            Message::Command(id) => {
                collect(self.dispatch(id, now).ok(), &mut effects, &mut shell);
            }
            Message::Key(key, modifiers, window) => {
                if self.overlay.as_ref().is_some_and(|o| o.id == window)
                    && let Some(cmd) = self.overlay_shortcut(&key, modifiers)
                {
                    collect(self.dispatch(cmd, now).ok(), &mut effects, &mut shell);
                }
            }
            Message::Pointer(kind, p) => {
                effects.extend(self.session.handle(
                    Event::Pointer(PointerEvent {
                        kind,
                        output: PRIMARY,
                        pos: Pt::new(p.x, p.y),
                        at: now,
                    }),
                    now,
                ));
            }
            Message::Acquired(ticket) => {
                effects.extend(self.session.handle(Event::InputAcquired(ticket), now));
            }
            Message::OverlayOpened(id) => {
                if let Some(o) = self.overlay.as_mut().filter(|o| o.id == id) {
                    o.ready = true;
                }
            }
            Message::CloseRequested(id) => {
                if self.overlay.as_ref().is_some_and(|o| o.id == id) {
                    effects.extend(self.session.handle(Event::MasterOff, now));
                }
            }
            Message::Frame => {}
        }

        for effect in effects {
            match effect {
                Effect::PlayTap => {} // Sound arrives with the pack workflow.
                Effect::ReturnFocus => platform::restore_foreground(self.previous_focus.take()),
            }
        }
        self.sync_tray();

        if shell == Some(ShellAction::Quit) {
            self.session.handle(Event::MasterOff, now);
            platform::restore_foreground(self.previous_focus.take());
            // Remove the tray icon now rather than leaving a ghost entry.
            self.tray = None;
            // Renderer teardown can hang on Windows after the overlay closed
            // (seen with iced 0.14 + wgpu); nothing else needs flushing.
            std::process::exit(0);
        }
        self.reconcile()
    }

    fn dispatch(&mut self, id: &str, now: Millis) -> Result<Dispatched, Unavailable> {
        self.registry.dispatch(id, &mut self.session, now)
    }

    fn cli(&mut self, line: &str, now: Millis) -> (String, Option<Dispatched>) {
        let name = line.split_whitespace().next().unwrap_or("");
        if name == "status" {
            return (format!("ok: {}", self.status()), None);
        }
        if name == "set" {
            let mut parts = line.split_whitespace().skip(1);
            let (Some(id), Some(text)) = (parts.next(), parts.next()) else {
                return ("error: usage: tinypaw set <setting> <value>".into(), None);
            };
            let mut values = self.session.settings().clone();
            let result = self
                .registry
                .parse_value(id, text)
                .and_then(|v| self.registry.set(&mut values, id, v));
            return match result {
                Ok(()) => {
                    self.session.handle(Event::SettingsChanged(values), now);
                    (format!("ok: {id} = {text}"), None)
                }
                Err(why) => (format!("error: {id}: {why}"), None),
            };
        }
        let Some(def) = self.registry.command_by_cli(name) else {
            return (format!("error: unknown command `{name}`"), None);
        };
        match self.registry.dispatch(def.id, &mut self.session, now) {
            Ok(d) => (format!("ok: {}", self.status()), Some(d)),
            Err(why) => (format!("error: {}: {why}", def.label), None),
        }
    }

    fn status(&self) -> String {
        let marks = self.session.scene().marks().len();
        match self.session.mode() {
            Mode::Off => "TinyPaw is Off".into(),
            Mode::Passthrough => format!("TinyPaw is On (passthrough, {marks} marks)"),
            Mode::Annotating(_) => format!("TinyPaw is On (annotating, {marks} marks)"),
        }
    }

    fn overlay_shortcut(&self, key: &keyboard::Key, m: keyboard::Modifiers) -> Option<&'static str> {
        let pressed = match key.as_ref() {
            keyboard::Key::Named(Named::Escape) => Key::Escape,
            keyboard::Key::Character(c) => Key::Char(c.chars().next()?.to_ascii_uppercase()),
            _ => return None,
        };
        self.registry
            .shortcuts(ShortcutContext::Overlay)
            .find(|(s, _)| {
                s.key == pressed && s.ctrl == m.control() && s.alt == m.alt() && s.shift == m.shift()
            })
            .map(|(_, def)| def.id)
    }

    fn sync_tray(&mut self) {
        let label = self
            .hotkeys
            .as_ref()
            .and_then(|h| h.label(system::MASTER_TOGGLE))
            .map(str::to_owned);
        if let Some(tray) = &mut self.tray {
            tray.sync(self.registry, &self.session, label.as_deref());
        }
    }

    /// Brings the overlay window in line with the session's overlay state.
    fn reconcile(&mut self) -> Task<Message> {
        let want = self.session.overlay_state();
        match (want, &mut self.overlay) {
            (OverlayState::Hidden, None) => Task::none(),
            (OverlayState::Hidden, Some(o)) => {
                let id = o.id;
                self.overlay = None;
                window::close(id)
            }
            (_, None) => {
                if want == OverlayState::Interactive {
                    self.previous_focus = platform::foreground_window();
                }
                let (id, opened) = window::open(overlay_settings());
                self.overlay = Some(Overlay {
                    id,
                    ready: false,
                    applied: None,
                });
                opened.map(Message::OverlayOpened).chain(Task::done(Message::Frame))
            }
            (_, Some(o)) if !o.ready => Task::none(),
            (OverlayState::ClickThrough, Some(o)) => {
                if o.applied == Some(false) {
                    return Task::none();
                }
                o.applied = Some(false);
                window::enable_mouse_passthrough(o.id)
            }
            (OverlayState::Interactive, Some(o)) => {
                let id = o.id;
                let mut task = Task::none();
                if o.applied != Some(true) {
                    if o.applied == Some(false) {
                        self.previous_focus = platform::foreground_window();
                    }
                    o.applied = Some(true);
                    task = window::disable_mouse_passthrough(id).chain(window::gain_focus(id));
                }
                match self.session.pending_ticket() {
                    Some(t) => task.chain(Task::done(Message::Acquired(t))),
                    None => task,
                }
            }
        }
    }

    fn view(&self, window: window::Id) -> Element<'_, Message> {
        match &self.overlay {
            Some(o) if o.id == window => {
                view::overlay(self.registry, &self.session, &self.ink, PRIMARY, self.now())
            }
            _ => iced::widget::space().into(),
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            Subscription::run(platform::external_stream).map(Message::External),
            event::listen_with(|event, _status, window| match event {
                iced::Event::Keyboard(keyboard::Event::KeyPressed {
                    key,
                    modifiers,
                    repeat: false,
                    ..
                }) => Some(Message::Key(key, modifiers, window)),
                _ => None,
            }),
            window::close_requests().map(Message::CloseRequested),
        ];
        // Frames only while an animation runs; otherwise the process sleeps.
        if self.session.animating(self.now()) {
            subs.push(window::frames().map(|_| Message::Frame));
        }
        Subscription::batch(subs)
    }
}

fn collect(result: Option<Dispatched>, effects: &mut Vec<Effect>, shell: &mut Option<ShellAction>) {
    match result {
        Some(Dispatched::Effects(fx)) => effects.extend(fx),
        Some(Dispatched::Shell(action)) => *shell = Some(action),
        None => {}
    }
}
