//! Iced event loop: normalizes input into session events, runs requested
//! effects and keeps the overlay, settings and first-run windows in step with
//! the session.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use iced::advanced::image;
use iced::keyboard::{self, key::Named};
use iced::window::{self, settings::PlatformSpecific};
use iced::{Element, Point, Size, Subscription, Task, Theme, event, theme};

use crate::audio::{self, Audio};
use crate::features::{point, sound, system, wheel as wheel_feature};
use crate::platform::{self, External};
use crate::packs::{self, Draft, Pack};
use crate::preferences;
use crate::registry::{
    Dispatched, Key, Registry, ShellAction, ShortcutContext, Unavailable, Value,
};
use crate::session::{
    Effect, Event, Millis, Mode, OutputId, OverlayState, PointerEvent, PointerKind, Pt, Session,
    TextInput, Ticket,
};
use crate::view::{self, InkCache, OverlayUi, UiMsg, settings::Page, wheel};

/// This alpha drives a single overlay on the primary display.
const PRIMARY: OutputId = OutputId(0);

#[derive(Debug, Clone)]
pub enum Message {
    External(External),
    WindowOpened(window::Id),
    CloseRequested(window::Id),
    Pointer(PointerKind, Point),
    Key(keyboard::Key, keyboard::Modifiers, window::Id),
    Command(&'static str),
    Set(&'static str, Value),
    Ui(UiMsg),
    WavPicked(Option<PathBuf>),
    PointerPicked(Option<PathBuf>),
    Acquired(Ticket),
    TextChanged(String),
    TextSubmit,
    Still(Ticket, Result<image::Handle, String>),
    Frame,
    Tick,
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
    settings_window: Option<window::Id>,
    first_run_window: Option<window::Id>,
    page: Page,
    notice: Option<String>,
    ui: OverlayUi,
    previous_focus: Option<isize>,
    hotkeys: Option<platform::hotkeys::Hotkeys>,
    tray: Option<platform::tray::Tray>,
    audio: Audio,
    ink: InkCache,
    /// The frozen frame's pixels, kept outside annotation history.
    still: Option<(u64, image::Handle)>,
    next_frame: u64,
    /// Short overlay message and when it appeared.
    overlay_notice: Option<(String, Millis)>,
    editor_focused: bool,
    packs: Vec<Pack>,
    pointer_draft: Option<Draft>,
}

pub fn run(registry: &'static Registry) -> iced::Result {
    iced::daemon(move || App::boot(registry), App::update, App::view)
        .title(App::title)
        .subscription(App::subscription)
        .style(|_: &App, _: &Theme| theme::Style {
            background_color: iced::Color::TRANSPARENT,
            text_color: view::theme::TEXT,
        })
        .theme(|_: &App, _| Theme::Dark)
        .default_font(view::theme::BODY)
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

fn panel_settings(size: Size, min: Option<Size>, resizable: bool) -> window::Settings {
    window::Settings {
        size,
        min_size: min,
        position: window::Position::Centered,
        resizable,
        exit_on_close_request: false,
        platform_specific: PlatformSpecific {
            drag_and_drop: false,
            ..PlatformSpecific::default()
        },
        ..window::Settings::default()
    }
}

impl App {
    fn boot(registry: &'static Registry) -> (Self, Task<Message>) {
        let loaded = preferences::load(registry);
        let tool = registry.default_tool();
        let session = Session::new(tool.id, tool.factory, loaded.values);
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
            settings_window: None,
            first_run_window: None,
            page: Page::default(),
            notice: None,
            ui: OverlayUi::default(),
            previous_focus: None,
            hotkeys,
            tray,
            audio: Audio::new(),
            ink: InkCache::default(),
            still: None,
            next_frame: 0,
            overlay_notice: None,
            editor_focused: false,
            packs: Vec::new(),
            pointer_draft: None,
        };
        let (loaded_packs, problems) = packs::load_all();
        for problem in &problems {
            eprintln!("tinypaw: pointer pack skipped: {problem}");
        }
        if !problems.is_empty() {
            app.notice = Some(format!("{} pointer pack(s) could not be loaded", problems.len()));
        }
        app.packs = loaded_packs;
        app.sync_tray();
        let task = if loaded.first_run {
            // Saving now means the card shows once.
            let _ = preferences::save(app.session.settings());
            let (id, open) = window::open(panel_settings(Size::new(420.0, 330.0), None, false));
            app.first_run_window = Some(id);
            open.map(Message::WindowOpened)
        } else {
            Task::none()
        };
        (app, task)
    }

    fn now(&self) -> Millis {
        self.started.elapsed().as_millis() as Millis
    }

    fn title(&self, window: window::Id) -> String {
        if Some(window) == self.settings_window {
            "TinyPaw Settings".into()
        } else {
            "TinyPaw".into()
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        let now = self.now();
        if std::env::var_os("TINYPAW_TRACE").is_some()
            && !matches!(message, Message::Frame | Message::Tick)
        {
            eprintln!("[{now:>7}] {message:?} mode={:?}", self.session.mode());
        }
        let was_enabled = self.session.is_enabled();
        let mut out = Outcomes::default();
        let mut tasks = Vec::new();

        match message {
            Message::External(External::Hotkey { id, pressed }) => {
                if let Some(cmd) = self.hotkeys.as_ref().and_then(|h| h.command(id)) {
                    let result = if pressed {
                        self.registry.dispatch(cmd, &mut self.session, now).ok()
                    } else {
                        self.registry.dispatch_release(cmd, &mut self.session, now)
                    };
                    out.collect(result);
                }
            }
            Message::External(External::Menu(id)) => {
                out.collect(self.dispatch(&id, now).ok());
            }
            Message::External(External::TrayClick) => {
                out.collect(self.dispatch(system::MASTER_TOGGLE, now).ok());
            }
            Message::External(External::Cli { line, reply }) => {
                let (answer, result) = self.cli(&line, now);
                out.collect(result);
                let _ = reply.send(answer);
            }
            Message::Command(id) => {
                self.ui.close_menus();
                out.collect(self.dispatch(id, now).ok());
                if id == point::USE_PAW || id == point::USE_LASER {
                    out.collect(self.dispatch(point::SELECT, now).ok());
                }
            }
            Message::Set(id, value) => out.sets.push((id, value)),
            Message::Ui(ui) => tasks.push(self.ui_message(ui, now, &mut out)),
            Message::PointerPicked(path) => {
                if let Some(path) = path {
                    match packs::draft_from_file(&path) {
                        Ok(draft) => {
                            self.pointer_draft = Some(draft);
                            self.page = Page::Appearance;
                            self.notice = None;
                        }
                        Err(e) => self.notice = Some(format!("Could not use that image: {e}")),
                    }
                }
            }
            Message::WavPicked(path) => {
                if let Some(path) = path {
                    match audio::import_wav(&path) {
                        Ok(clip) => {
                            self.audio.set_custom(clip);
                            self.notice = None;
                            out.sets.push((sound::TAP, Value::Choice("custom")));
                        }
                        Err(e) => self.notice = Some(format!("Could not use that WAV: {e}")),
                    }
                }
            }
            Message::Key(key, modifiers, window) => {
                if self.overlay.as_ref().is_some_and(|o| o.id == window) {
                    let escape = matches!(key.as_ref(), keyboard::Key::Named(Named::Escape));
                    if self.session.text_draft().is_some() {
                        // The editor owns the keyboard; Escape cancels the
                        // draft before it can exit the session.
                        if escape {
                            out.effects.extend(
                                self.session.handle(Event::Text(TextInput::Cancel), now),
                            );
                        }
                    } else if escape && (self.ui.wheel.take().is_some() || self.ui.close_menus()) {
                        // Escape closes the wheel or a menu before it exits.
                    } else if let Some(cmd) = self.overlay_shortcut(&key, modifiers) {
                        out.collect(self.dispatch(cmd, now).ok());
                    }
                }
            }
            Message::TextChanged(text) => {
                out.effects
                    .extend(self.session.handle(Event::Text(TextInput::Changed(text)), now));
            }
            Message::TextSubmit => {
                out.effects
                    .extend(self.session.handle(Event::Text(TextInput::Submit), now));
            }
            Message::Still(ticket, result) => match result {
                Ok(handle) => {
                    self.next_frame += 1;
                    self.still = Some((self.next_frame, handle));
                    let frame = self.next_frame;
                    out.effects
                        .extend(self.session.handle(Event::StillReady { ticket, frame }, now));
                }
                Err(e) => {
                    eprintln!("tinypaw: freeze capture failed: {e}");
                    out.effects
                        .extend(self.session.handle(Event::StillFailed(ticket), now));
                }
            },
            Message::Pointer(kind, p) => self.pointer(kind, p, now, &mut out),
            Message::Acquired(ticket) => {
                out.effects
                    .extend(self.session.handle(Event::InputAcquired(ticket), now));
            }
            Message::WindowOpened(id) => {
                if let Some(o) = self.overlay.as_mut().filter(|o| o.id == id) {
                    o.ready = true;
                }
            }
            Message::CloseRequested(id) => {
                if self.overlay.as_ref().is_some_and(|o| o.id == id) {
                    out.effects.extend(self.session.handle(Event::MasterOff, now));
                } else if Some(id) == self.settings_window {
                    self.settings_window = None;
                    self.notice = None;
                    tasks.push(window::close(id));
                } else if Some(id) == self.first_run_window {
                    self.first_run_window = None;
                    tasks.push(window::close(id));
                }
            }
            Message::Tick => {
                out.effects.extend(self.session.handle(Event::Tick, now));
                if self
                    .overlay_notice
                    .as_ref()
                    .is_some_and(|(_, at)| now.saturating_sub(*at) > NOTICE_MS)
                {
                    self.overlay_notice = None;
                }
            }
            Message::Frame => {}
        }

        // Shell actions first: a wheel pick dispatches commands of its own.
        for action in std::mem::take(&mut out.shell) {
            match action {
                ShellAction::Quit => self.quit(now),
                ShellAction::OpenSettings => tasks.push(self.open_settings()),
                ShellAction::WheelOpen => self.open_wheel(now),
                ShellAction::WheelClose => self.close_wheel(now, &mut out),
            }
        }
        for (id, value) in std::mem::take(&mut out.sets) {
            self.apply_setting(id, value, now);
        }
        for effect in std::mem::take(&mut out.effects) {
            match effect {
                Effect::PlayTap => self.play_tap(),
                Effect::ReturnFocus => platform::restore_foreground(self.previous_focus.take()),
                Effect::CaptureStill(ticket) => tasks.push(capture_still(ticket)),
                Effect::ReleaseStill => self.still = None,
                Effect::Notice(text) => self.overlay_notice = Some((text.to_owned(), now)),
            }
        }
        // Give the label editor keyboard focus when a draft appears.
        match (self.session.text_draft().is_some(), self.editor_focused) {
            (true, false) => {
                self.editor_focused = true;
                tasks.push(iced::widget::operation::focus(view::TEXT_EDITOR));
            }
            (false, true) => self.editor_focused = false,
            _ => {}
        }

        if was_enabled && !self.session.is_enabled() {
            // Off stops tap playback and closes transient overlay UI.
            self.ui = OverlayUi::default();
            if self.settings_window.is_none() {
                self.audio.close();
            }
        }
        self.sync_tray();
        tasks.push(self.reconcile());
        Task::batch(tasks)
    }

    fn ui_message(&mut self, ui: UiMsg, now: Millis, out: &mut Outcomes) -> Task<Message> {
        match ui {
            UiMsg::TogglePointerMenu => {
                self.ui.ink_menu = false;
                self.ui.pointer_menu = !self.ui.pointer_menu;
            }
            UiMsg::ToggleInkMenu => {
                self.ui.pointer_menu = false;
                self.ui.ink_menu = !self.ui.ink_menu;
            }
            UiMsg::Nav(page) => self.page = page,
            UiMsg::PickPack(id) => {
                out.sets.push((point::STYLE, Value::Choice("paw")));
                out.sets.push((point::PACK, Value::Choice(id)));
            }
            UiMsg::AddPointer => {
                return Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose a transparent PNG pointer")
                        .add_filter("PNG image", &["png"])
                        .pick_file(),
                    |file| Message::PointerPicked(file.map(|f| f.path().to_path_buf())),
                );
            }
            UiMsg::DraftHotspot(x, y) => {
                if let Some(d) = &mut self.pointer_draft {
                    d.hotspot = (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0));
                }
            }
            UiMsg::DraftName(name) => {
                if let Some(d) = &mut self.pointer_draft {
                    d.name = name.chars().take(40).collect();
                }
            }
            UiMsg::CancelDraft => self.pointer_draft = None,
            UiMsg::SaveDraft => {
                if let Some(draft) = &self.pointer_draft {
                    let taken: Vec<&str> = self.packs.iter().map(|p| p.id).collect();
                    match packs::install(draft, &taken) {
                        Ok(pack) => {
                            let id = pack.id;
                            self.packs.push(pack);
                            self.pointer_draft = None;
                            self.notice = None;
                            out.sets.push((point::STYLE, Value::Choice("paw")));
                            out.sets.push((point::PACK, Value::Choice(id)));
                        }
                        Err(e) => self.notice = Some(format!("Could not save the pointer: {e}")),
                    }
                }
            }
            UiMsg::PreviewSound => {
                let s = self.session.settings();
                let (tap, volume) = (s.choice(sound::TAP), s.float(sound::VOLUME) as f32);
                self.audio.play(tap, volume);
            }
            UiMsg::ImportWav => {
                return Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose a tap sound")
                        .add_filter("WAV audio", &["wav"])
                        .pick_file(),
                    |file| Message::WavPicked(file.map(|f| f.path().to_path_buf())),
                );
            }
            UiMsg::TurnOn => {
                if !self.session.is_enabled() {
                    out.collect(self.dispatch(system::MASTER_TOGGLE, now).ok());
                }
                if let Some(id) = self.first_run_window.take() {
                    return window::close(id);
                }
            }
            UiMsg::OpenSettings => {
                let open = self.open_settings();
                return match self.first_run_window.take() {
                    Some(id) => window::close(id).chain(open),
                    None => open,
                };
            }
        }
        Task::none()
    }

    fn pointer(&mut self, kind: PointerKind, p: Point, now: Millis, out: &mut Outcomes) {
        if let Some(w) = self.ui.wheel.as_mut() {
            match kind {
                PointerKind::Moved => w.track(p),
                PointerKind::Down => {
                    w.track(p);
                    self.close_wheel(now, out);
                }
                _ => {}
            }
            return;
        }
        if kind == PointerKind::Down && self.ui.close_menus() {
            return; // A click outside a menu only dismisses it.
        }
        out.effects.extend(self.session.handle(
            Event::Pointer(PointerEvent {
                kind,
                output: PRIMARY,
                pos: Pt::new(p.x, p.y),
                at: now,
            }),
            now,
        ));
    }

    fn open_wheel(&mut self, now: Millis) {
        if !self.session.is_pointing() {
            let _ = self.dispatch(system::LATCH, now);
        }
        self.ui.close_menus();
        self.ui.wheel = Some(wheel::WheelState {
            center: self.session.pointer().map(|(_, p)| Point::new(p.x, p.y)),
            hover: None,
        });
    }

    fn close_wheel(&mut self, now: Millis, out: &mut Outcomes) {
        let Some(state) = self.ui.wheel.take() else {
            return;
        };
        if let Some(i) = state.hover {
            let cmd = wheel::ITEMS[i].command;
            out.collect(self.dispatch(cmd, now).ok());
            if cmd == point::USE_PAW || cmd == point::USE_LASER {
                out.collect(self.dispatch(point::SELECT, now).ok());
            }
        }
    }

    /// The pack the paw draws with; an unknown or missing pack falls back to
    /// the first loaded one.
    fn current_pack(&self) -> Option<&Pack> {
        let id = self.session.settings().choice(point::PACK);
        self.packs
            .iter()
            .find(|p| p.id == id)
            .or_else(|| self.packs.first())
    }

    /// The single update path for settings: validate, apply, persist.
    fn apply_setting(&mut self, id: &'static str, value: Value, now: Millis) {
        // A pack must be installed and valid before it can be selected.
        if id == point::PACK
            && let Value::Choice(pack) = &value
            && !self.packs.iter().any(|p| p.id == *pack)
        {
            self.notice = Some(format!("{id}: no installed pointer pack `{pack}`"));
            return;
        }
        let mut values = self.session.settings().clone();
        match self.registry.set(&mut values, id, value) {
            Ok(()) => {
                self.session.handle(Event::SettingsChanged(values), now);
                if let Err(e) = preferences::save(self.session.settings()) {
                    self.notice = Some(format!("Settings could not be saved: {e}"));
                }
            }
            Err(why) => self.notice = Some(format!("{id}: {why}")),
        }
    }

    fn play_tap(&mut self) {
        let s = self.session.settings();
        let laser = s.choice(point::STYLE) == "laser";
        if s.bool(sound::MUTE) || (laser && !s.bool(sound::LASER)) {
            return;
        }
        let (tap, volume) = (s.choice(sound::TAP), s.float(sound::VOLUME) as f32);
        self.audio.play(tap, volume);
    }

    fn open_settings(&mut self) -> Task<Message> {
        if let Some(id) = self.settings_window {
            return window::gain_focus(id);
        }
        let (id, open) = window::open(panel_settings(
            Size::new(1040.0, 680.0),
            Some(Size::new(860.0, 560.0)),
            true,
        ));
        self.settings_window = Some(id);
        open.map(Message::WindowOpened).chain(window::gain_focus(id))
    }

    fn quit(&mut self, now: Millis) {
        self.session.handle(Event::MasterOff, now);
        platform::restore_foreground(self.previous_focus.take());
        // Remove the tray icon now rather than leaving a ghost entry.
        self.tray = None;
        // Renderer teardown can hang on Windows after the overlay closed
        // (seen with iced 0.14); nothing else needs flushing.
        std::process::exit(0);
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
            let Some(def) = self.registry.setting(id) else {
                return (format!("error: unknown setting `{id}`"), None);
            };
            return match self.registry.parse_value(id, text) {
                Ok(value) => {
                    self.notice = None;
                    self.apply_setting(def.id, value, now);
                    match self.notice.take() {
                        Some(why) => (format!("error: {why}"), None),
                        None => (format!("ok: {id} = {text}"), None),
                    }
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
        let scene = if self.session.frozen_frame().is_some() { "frozen" } else { "live" };
        let tool = self.session.active_tool();
        match self.session.mode() {
            Mode::Off => "TinyPaw is Off".into(),
            Mode::Passthrough => format!("TinyPaw is On (passthrough, {scene}, {marks} marks)"),
            Mode::Annotating(_) => {
                format!("TinyPaw is On (annotating, {tool}, {scene}, {marks} marks)")
            }
        }
    }

    fn overlay_shortcut(&self, key: &keyboard::Key, m: keyboard::Modifiers) -> Option<&'static str> {
        let pressed = match key.as_ref() {
            keyboard::Key::Named(Named::Escape) => Key::Escape,
            keyboard::Key::Named(Named::Space) => Key::Space,
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
        let label = self.registry.shortcut_label(system::MASTER_TOGGLE);
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
                opened.map(Message::WindowOpened).chain(Task::done(Message::Frame))
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
        if self.overlay.as_ref().is_some_and(|o| o.id == window) {
            return view::overlay(view::Overlay {
                registry: self.registry,
                session: &self.session,
                ink: &self.ink,
                ui: &self.ui,
                output: PRIMARY,
                now: self.now(),
                still: self
                    .still
                    .as_ref()
                    .filter(|(id, _)| Some(*id) == self.session.frozen_frame())
                    .map(|(_, handle)| handle),
                notice: self.overlay_notice.as_ref().map(|(text, _)| text.as_str()),
                pack: self.current_pack(),
            });
        }
        if Some(window) == self.settings_window {
            return view::settings::view(view::settings::SettingsView {
                registry: self.registry,
                session: &self.session,
                page: self.page,
                has_custom_sound: self.audio.has_custom(),
                taken: self.hotkeys.as_ref().map_or(&[], |h| h.taken.as_slice()),
                packs: &self.packs,
                pack: self.current_pack(),
                draft: self.pointer_draft.as_ref(),
                notice: self.notice.as_deref(),
            });
        }
        if Some(window) == self.first_run_window {
            return view::first_run::view(self.registry.shortcut_label(system::MASTER_TOGGLE));
        }
        iced::widget::space().into()
    }

    fn subscription(&self) -> Subscription<Message> {
        let now = self.now();
        let mut subs = vec![
            Subscription::run(platform::external_stream).map(Message::External),
            event::listen_with(|event, status, window| match event {
                // Keys a focused editor consumed (typing a label) are not shortcuts.
                _ if status == event::Status::Captured => None,
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
        // Frames only while something animates; otherwise the process sleeps.
        if self.session.animating(now) {
            subs.push(window::frames().map(|_| Message::Frame));
        } else if self.overlay_notice.is_some() {
            subs.push(iced::time::every(Duration::from_millis(500)).map(|_| Message::Tick));
        } else if let Some(at) = self.session.next_fade(now) {
            // Wake when the next stroke starts fading.
            let wait = Duration::from_millis(at.saturating_sub(now).clamp(16, 1000));
            subs.push(iced::time::every(wait).map(|_| Message::Tick));
        }
        Subscription::batch(subs)
    }
}

/// How long an overlay notice stays in the status chip.
const NOTICE_MS: Millis = 3000;

/// Takes one still of the primary display off the UI thread. A short wait
/// first lets the overlay present a frame without marks, so they are not
/// baked into the still.
fn capture_still(ticket: Ticket) -> Task<Message> {
    let (tx, rx) = iced::futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        let result = platform::capture::primary_display()
            .map(|s| image::Handle::from_rgba(s.width, s.height, s.rgba));
        let _ = tx.send(result);
    });
    Task::perform(rx, move |r| {
        Message::Still(ticket, r.unwrap_or_else(|_| Err("capture thread ended".into())))
    })
}

/// What handlers asked for during one update, applied in a fixed order.
#[derive(Default)]
struct Outcomes {
    effects: Vec<Effect>,
    sets: Vec<(&'static str, Value)>,
    shell: Vec<ShellAction>,
}

impl Outcomes {
    fn collect(&mut self, result: Option<Dispatched>) {
        match result {
            Some(Dispatched::Effects(fx)) => self.effects.extend(fx),
            Some(Dispatched::Set(id, value)) => self.sets.push((id, value)),
            Some(Dispatched::Shell(action)) => self.shell.push(action),
            None => {}
        }
    }
}

// Keep the wheel feature's command ID referenced for the registry lookup.
const _: &str = wheel_feature::OPEN;
