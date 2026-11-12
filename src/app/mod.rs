//! Iced event loop: normalizes input into session events, runs requested
//! effects and keeps the overlay, settings and first-run windows in step with
//! the session.

mod input;
mod mascot;
mod music;
mod pointing;
mod studio;
use studio::Studio;

use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};
use std::time::{Duration, Instant};

use iced::advanced::image;
use iced::keyboard::{self, key::Named};
use iced::window::{self, settings::PlatformSpecific};
use iced::{Element, Point, Size, Subscription, Task, Theme, event, theme};

use crate::audio::{self, Audio};
use crate::features::anywhere::{self, Adjust, HeldToggle, Target};
use crate::features::{mascot as character, point, sound, system, wheel as wheel_feature};
use crate::mascot::pack::{self as characters, Character};
use crate::packs::{self, Draft, Pack};
use crate::platform::{
    self, External,
    displays::{self, Display},
};
use crate::preferences;
use crate::registry::{
    Dispatched, Edge, Key, Registry, ShellAction, ShortcutContext, Unavailable, Value,
};
use crate::session::{
    Effect, Event, Millis, Mode, OutputId, OverlayState, PointerEvent, PointerKind, Pt, Session,
    TextInput, Ticket,
};
use crate::view::{self, InkCache, OverlayUi, UiMsg, settings::Page, wheel};

#[derive(Debug, Clone)]
pub enum Message {
    External(External),
    WindowOpened(window::Id),
    NativeReady(window::Id, isize),
    CloseRequested(window::Id),
    Pointer(OutputId, PointerKind, Point),
    Key(keyboard::Key, keyboard::Modifiers, window::Id),
    Command(&'static str),
    Set(&'static str, Value),
    Ui(UiMsg),
    WavPicked(Option<PathBuf>),
    PointerPicked(Option<PathBuf>),
    PackPicked(Option<PathBuf>),
    ForearmPicked(Option<PathBuf>),
    PressedPicked(Option<PathBuf>),
    PackTapPicked(Option<PathBuf>),
    CharacterPicked(Option<PathBuf>),
    CharacterFolderPicked(Option<PathBuf>),
    CharacterWorked(u64, Result<CharacterWork, String>),
    ReactionWavPicked(crate::mascot::reactions::Event, Option<PathBuf>),
    ReactionPicturePicked(crate::mascot::reactions::Event, Option<PathBuf>),
    ReferencePicked(Option<PathBuf>),
    ReferenceResults(Vec<PathBuf>),
    /// The artwork draft built as a rig for Test character, by request.
    DraftRig(u64, Box<crate::mascot::rig::Rig>),
    CharacterExportPicked(Option<PathBuf>),
    /// A dialogue file to import.
    DialoguePicked(Option<PathBuf>),
    /// Where to export the dialogue, or (`true`) write a template.
    DialogueExportPicked(bool, Option<PathBuf>),
    ArtFilesPicked(u64, crate::mascot::editor::Target, Vec<PathBuf>),
    /// Frames for the Motion tab's preview.
    MotionTick,
    /// A WAV picked for a line: its trigger and index.
    LineSoundPicked(crate::mascot::lines::Trigger, usize, Option<PathBuf>),
    ArtBodyPicked(u64, Option<PathBuf>),
    Acquired(Ticket),
    TextChanged(String),
    TextSubmit,
    Still(Ticket, Result<Vec<(OutputId, image::Handle)>, String>),
    Mascot(crate::mascot::Msg),
    /// An overlay just started taking input: keep it from occluding apps.
    Unoccluding(window::Id),
    Frame,
    Tick,
    /// Follows the cursor for Spotlight anywhere while not annotating.
    SpotTick,
    /// Mouse wheel over the overlay: up, and whether Shift was held.
    ToolWheel(bool, bool),
}

#[derive(Debug, Clone)]
pub enum CharacterWork {
    Draft(characters::Draft),
    Editor(Box<crate::mascot::editor::Editor>, Vec<String>),
    Prepared(Arc<characters::PreparedFolder>),
    /// A folder whose images need fitting to its canvas, and what it holds.
    Misfits(PathBuf, characters::Inspected),
    Imported(characters::Imported, bool),
}

struct Overlay {
    id: window::Id,
    ready: bool,
    /// What was last applied: `Some(true)` interactive, `Some(false)` click-through.
    applied: Option<bool>,
    display: Display,
    ink: InkCache,
    hwnd: isize,
}

pub struct App {
    preference_writer: preferences::Writer,
    registry: Registry,
    session: Session,
    started: Instant,
    overlays: Vec<Overlay>,
    displays: Vec<Display>,
    settings_window: Option<window::Id>,
    /// Whether Settings uses the merged Windows header.
    settings_framed: bool,
    helper_hwnds: std::collections::BTreeMap<window::Id, isize>,
    first_run_window: Option<window::Id>,
    page: Page,
    character_tab: view::settings::CharacterTab,
    notice: Option<String>,
    ui: OverlayUi,
    previous_focus: Option<isize>,
    hotkeys: Option<platform::hotkeys::Hotkeys>,
    tray: Option<platform::tray::Tray>,
    audio: Audio,
    /// The frozen frame's pixels, kept outside annotation history.
    still: Option<(u64, Vec<(OutputId, image::Handle)>)>,
    next_frame: u64,
    /// Short overlay message and when it appeared.
    overlay_notice: Option<(String, Millis)>,
    editor_focused: bool,
    packs: Vec<Pack>,
    pointer_draft: Option<Draft>,
    /// Bundled Yohta first, then installed characters.
    characters: Vec<Character>,
    /// The character studio: Add and import jobs, Art, Motion and
    /// Create from reference.
    studio: Studio,
    quit_pending: bool,
    /// The character voice: cooldowns, idle deadline, what's playing.
    voice: crate::mascot::reactions::Voice,
    /// The desktop character's reaction sounds.
    reaction_sounds: crate::mascot::reactions::Sounds,
    /// The voice's monotonic clock.
    voice_clock: Instant,
    /// The playback meter, while music response is on and needed.
    #[cfg(windows)]
    music: Option<platform::music::Meter>,
    /// The Follow beat trial's loopback analysis, while it's on.
    #[cfg(windows)]
    beat: Option<platform::beat::Follower>,
    /// Eases between followed tempos and the steady pace, so the music
    /// motion never jumps. Stepped by wall time wherever the phase is read.
    beat_clock: std::cell::Cell<(crate::mascot::music::BeatClock, Option<Instant>)>,
    editing_pack: Option<&'static str>,
    rebinding: Option<&'static str>,
    alt_down: bool,
    #[cfg(windows)]
    lens: platform::lens::Controller,
    /// Lens anywhere (Ctrl+Alt+Z), with annotation on or off.
    lens_anywhere: HeldToggle,
    /// Spotlight anywhere (Ctrl+Alt+X), with annotation on or off.
    spotlight_anywhere: HeldToggle,
    /// Where the cursor is for Spotlight anywhere while not annotating.
    spot_cursor: Option<(OutputId, Pt)>,
    /// The desktop character window, while enabled.
    #[cfg(windows)]
    mascot: Option<crate::mascot::desktop::Desktop>,
    #[cfg(windows)]
    sources: Vec<platform::windows::WindowSource>,
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
        fullscreen: false,
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
        icon: Some(crate::branding::window_icon()),
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
        let mut registry = registry.clone();
        preferences::load_shortcuts(&mut registry);
        let mut loaded = preferences::load(&registry);
        #[cfg(windows)]
        let _ = registry.set(
            &mut loaded.values,
            system::SYSTEM_REDUCED,
            Value::Bool(platform::windows::reduced_motion()),
        );
        let tool = registry.default_tool();
        let session = Session::new(tool.id, tool.factory, loaded.values);
        let hotkeys = match platform::hotkeys::register(&registry) {
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
        let tray = platform::tray::build(&registry)
            .inspect_err(|e| eprintln!("tinypaw: tray unavailable: {e}"))
            .ok();
        let mut app = Self {
            preference_writer: preferences::Writer::new(),
            registry,
            session,
            started: Instant::now(),
            overlays: Vec::new(),
            displays: displays::list(),
            settings_window: None,
            settings_framed: false,
            helper_hwnds: std::collections::BTreeMap::new(),
            first_run_window: None,
            page: Page::default(),
            character_tab: Default::default(),
            notice: None,
            ui: OverlayUi::default(),
            previous_focus: None,
            hotkeys,
            tray,
            audio: Audio::new(),
            still: None,
            next_frame: 0,
            overlay_notice: None,
            editor_focused: false,
            packs: Vec::new(),
            characters: Vec::new(),
            studio: Studio::default(),
            quit_pending: false,
            voice: Default::default(),
            reaction_sounds: Default::default(),
            voice_clock: Instant::now(),
            #[cfg(windows)]
            music: None,
            #[cfg(windows)]
            beat: None,
            beat_clock: Default::default(),
            pointer_draft: None,
            editing_pack: None,
            rebinding: None,
            alt_down: false,
            #[cfg(windows)]
            lens: platform::lens::Controller::default(),
            lens_anywhere: HeldToggle::default(),
            spotlight_anywhere: HeldToggle::default(),
            spot_cursor: None,
            #[cfg(windows)]
            mascot: None,
            #[cfg(windows)]
            sources: platform::windows::window_sources(),
        };
        let (loaded_packs, problems) = packs::load_all();
        for problem in &problems {
            eprintln!("tinypaw: pointer pack skipped: {problem}");
        }
        if !problems.is_empty() {
            app.notice = Some(format!(
                "{} pointer pack(s) could not be loaded",
                problems.len()
            ));
        }
        app.packs = loaded_packs;
        let (loaded_characters, problems) = characters::load_all();
        for problem in &problems {
            eprintln!("tinypaw: character skipped: {problem}");
        }
        if !problems.is_empty() {
            app.notice = Some(format!(
                "Character recovery or loading failed: {}",
                problems.join("; ")
            ));
        }
        app.characters = loaded_characters;
        app.migrate_character_height(loaded.first_run);
        app.sync_tray();
        let task = if loaded.first_run {
            // Saving now means the card shows once.
            app.preference_writer.schedule(app.session.settings());
            let (id, open) = window::open(panel_settings(Size::new(420.0, 330.0), None, false));
            app.first_run_window = Some(id);
            open.map(Message::WindowOpened)
        } else {
            Task::none()
        };
        // Restore the desktop character if it was showing last time.
        let task = Task::batch([task, app.reconcile_mascot()]);
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

        if let Message::NativeReady(id, hwnd) = &message
            && (Some(*id) == self.settings_window || Some(*id) == self.first_run_window)
        {
            self.helper_hwnds.insert(*id, *hwnd);
        }
        match message {
            Message::External(External::PreferencesSaved(result)) => match result {
                Err(e) => {
                    self.notice = Some(format!("Settings could not be saved (retrying): {e}"))
                }
                Ok(())
                    if self
                        .notice
                        .as_deref()
                        .is_some_and(|n| n.starts_with("Settings could not be saved")) =>
                {
                    self.notice = None
                }
                Ok(()) => {}
            },
            Message::NativeReady(id, hwnd) if Some(id) == self.settings_window => {
                #[cfg(windows)]
                {
                    self.settings_framed = hwnd != 0 && platform::windows::frame::install(hwnd);
                }
            }
            // Redraws the Settings header for caption-button hover.
            Message::External(External::FrameChanged) => {}
            Message::NativeReady(id, hwnd) => {
                if let Some(o) = self.overlays.iter_mut().find(|o| o.id == id) {
                    o.hwnd = hwnd;
                    #[cfg(windows)]
                    {
                        platform::windows::observe(hwnd);
                        platform::windows::place(hwnd, &o.display);
                        if o.applied == Some(true) {
                            platform::windows::unoccluding(hwnd);
                        }
                    }
                }
            }
            Message::Unoccluding(id) => {
                #[cfg(windows)]
                if let Some(o) = self
                    .overlays
                    .iter()
                    .find(|o| o.id == id && o.hwnd != 0 && o.applied == Some(true))
                {
                    platform::windows::unoccluding(o.hwnd);
                }
            }
            Message::External(External::DisplaysChanged) => {
                #[cfg(windows)]
                if let Some(m) = &mut self.mascot {
                    m.displays_changed();
                }
                let displays = displays::list();
                if self.displays != displays {
                    out.effects
                        .extend(self.session.handle(Event::Interrupted, now));
                    self.displays = displays;
                    self.notice = Some(
                        "Displays changed. Choose a display before restarting TinyPaw.".into(),
                    );
                }
            }
            Message::External(External::Interrupted) => {
                out.effects
                    .extend(self.session.handle(Event::Interrupted, now));
            }
            Message::External(External::InteractKey(pressed)) => {
                let commands: Vec<_> = self
                    .registry
                    .shortcuts(ShortcutContext::Global)
                    .filter(|(s, _)| s.key == Key::Alt)
                    .map(|(_, d)| d.id)
                    .collect();
                for cmd in commands {
                    out.collect(if pressed {
                        self.registry.dispatch(cmd, &mut self.session, now).ok()
                    } else {
                        self.registry.dispatch_release(cmd, &mut self.session, now)
                    });
                }
            }
            Message::External(External::MotionChanged) => {
                #[cfg(windows)]
                out.sets.push((
                    system::SYSTEM_REDUCED,
                    Value::Bool(platform::windows::reduced_motion()),
                ));
            }
            Message::External(External::Hotkey { id, pressed }) => {
                if self.hotkeys.as_ref().is_some_and(|h| h.is_dismiss(id)) {
                    if pressed {
                        self.dismiss_anywhere(now);
                    }
                } else if let Some(key) = self.hotkeys.as_ref().and_then(|h| h.adjust(id)) {
                    if pressed {
                        self.adjust_anywhere(key, &mut out);
                    }
                } else if let Some(cmd) = self.hotkeys.as_ref().and_then(|h| h.command(id)) {
                    if pressed {
                        self.run(cmd, now, &mut out);
                    } else {
                        let result = self.registry.dispatch_release(cmd, &mut self.session, now);
                        out.collect(result);
                    }
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
                self.run(id, now, &mut out);
            }
            Message::Set(id, value) => out.sets.push((id, value)),
            Message::Mascot(msg) => self.mascot_message(msg, now, &mut out, &mut tasks),
            Message::Ui(ui) => tasks.push(self.ui_message(ui, now, &mut out)),
            Message::Key(key, modifiers, window) => {
                if Some(window) == self.settings_window && self.rebinding.is_some() {
                    if matches!(
                        key.as_ref(),
                        keyboard::Key::Named(
                            Named::Control | Named::Shift | Named::Alt | Named::Super
                        )
                    ) {
                        return Task::none();
                    }
                    let command = self.rebinding.take().unwrap();
                    if !matches!(key.as_ref(), keyboard::Key::Named(Named::Escape)) {
                        let base = match key.as_ref() {
                            keyboard::Key::Character(c) => Some(c.to_uppercase()),
                            keyboard::Key::Named(Named::Alt) => Some("Alt".into()),
                            keyboard::Key::Named(n) => Some(format!("{n:?}")),
                            _ => None,
                        };
                        if let Some(base) = base {
                            let binding = if base == "Alt" {
                                base
                            } else {
                                format!(
                                    "{}{}{}{}",
                                    if modifiers.control() { "Ctrl+" } else { "" },
                                    if modifiers.alt() { "Alt+" } else { "" },
                                    if modifiers.shift() { "Shift+" } else { "" },
                                    base
                                )
                            };
                            let old = self.registry.clone();
                            match self.registry.rebind(command, &binding) {
                                Err(e) => self.notice = Some(e),
                                Ok(()) => {
                                    self.hotkeys = None;
                                    match platform::hotkeys::register(&self.registry) {
                                        Ok(h) if !h.taken.contains(&command) => {
                                            self.hotkeys = Some(h);
                                            self.notice =
                                                preferences::save_shortcuts(&self.registry)
                                                    .err()
                                                    .map(|e| e.to_string());
                                        }
                                        result => {
                                            drop(result);
                                            self.registry = old;
                                            self.hotkeys =
                                                platform::hotkeys::register(&self.registry).ok();
                                            self.notice = Some(
                                                "Shortcut unavailable; kept the previous binding"
                                                    .into(),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else if Some(window) == self.settings_window
                    && self.studio.character_job.is_none()
                    && self.studio.art.is_some()
                {
                    let delta = match key.as_ref() {
                        keyboard::Key::Named(Named::ArrowLeft) => Some((-1.0, 0.0)),
                        keyboard::Key::Named(Named::ArrowRight) => Some((1.0, 0.0)),
                        keyboard::Key::Named(Named::ArrowUp) => Some((0.0, -1.0)),
                        keyboard::Key::Named(Named::ArrowDown) => Some((0.0, 1.0)),
                        _ => None,
                    };
                    if let Some((x, y)) = delta
                        && let Some(e) = &mut self.studio.art
                    {
                        // Holding an arrow is one undo step.
                        let step = if modifiers.shift() { 10.0 } else { 1.0 };
                        e.checkpoint_gesture(crate::mascot::editor::Gesture::Nudge);
                        e.nudge(x * step, y * step);
                    } else if matches!(key.as_ref(), keyboard::Key::Named(Named::Escape))
                        && let Some(e) = &mut self.studio.art
                    {
                        // Escape leaves a temporary tool.
                        e.finish_joint();
                        e.finish_rest();
                    }
                } else if self.overlays.iter().any(|o| o.id == window) {
                    let escape = matches!(key.as_ref(), keyboard::Key::Named(Named::Escape));
                    if self.session.text_draft().is_some() {
                        // The editor owns the keyboard; Escape cancels the
                        // draft before it can exit the session.
                        if modifiers.control()
                            && matches!(key.as_ref(), keyboard::Key::Character("z" | "Z"))
                        {
                            out.effects
                                .extend(self.session.handle(Event::Text(TextInput::Undo), now));
                        } else if escape {
                            out.effects
                                .extend(self.session.handle(Event::Text(TextInput::Cancel), now));
                        }
                    } else if escape && (self.ui.wheel.take().is_some() || self.ui.close_menus()) {
                        // Escape closes the wheel or a menu before it exits.
                    } else if escape && (self.lens_anywhere.on() || self.spotlight_anywhere.on()) {
                        // Then hides Lens and Spotlight anywhere.
                        self.dismiss_anywhere(now);
                    } else if let Some(cmd) = self.overlay_shortcut(&key, modifiers) {
                        self.ui.close_menus();
                        self.run(cmd, now, &mut out);
                    }
                }
            }
            Message::TextChanged(text) => {
                out.effects.extend(
                    self.session
                        .handle(Event::Text(TextInput::Changed(text)), now),
                );
            }
            Message::TextSubmit => {
                out.effects
                    .extend(self.session.handle(Event::Text(TextInput::Submit), now));
            }
            Message::Still(ticket, result) if self.session.capture_ticket() != Some(ticket) => {
                let _ = result;
            }
            Message::Still(ticket, result) => match result {
                Ok(handle) => {
                    self.next_frame += 1;
                    self.still = Some((self.next_frame, handle));
                    let frame = self.next_frame;
                    out.effects.extend(
                        self.session
                            .handle(Event::StillReady { ticket, frame }, now),
                    );
                }
                Err(e) => {
                    eprintln!("tinypaw: freeze capture failed: {e}");
                    out.effects
                        .extend(self.session.handle(Event::StillFailed(ticket), now));
                }
            },
            Message::Pointer(output, kind, p) => self.pointer(output, kind, p, now, &mut out),
            Message::Acquired(ticket) => {
                out.effects
                    .extend(self.session.handle(Event::InputAcquired(ticket), now));
            }
            Message::WindowOpened(id) => {
                #[cfg(windows)]
                if Some(id) == self.settings_window || Some(id) == self.first_run_window {
                    tasks.push(window::run(id, move |w| {
                        use iced::window::raw_window_handle::RawWindowHandle;
                        let hwnd = match w.window_handle().map(|h| h.as_raw()) {
                            Ok(RawWindowHandle::Win32(h)) => h.hwnd.get(),
                            _ => 0,
                        };
                        Message::NativeReady(id, hwnd)
                    }));
                }
                #[cfg(windows)]
                if self.mascot.as_ref().is_some_and(|m| m.id == id) {
                    tasks.push(window::run(id, |w| {
                        use iced::window::raw_window_handle::RawWindowHandle;
                        let hwnd = match w.window_handle().map(|h| h.as_raw()) {
                            Ok(RawWindowHandle::Win32(h)) => h.hwnd.get(),
                            _ => 0,
                        };
                        Message::Mascot(crate::mascot::Msg::Native(hwnd))
                    }));
                }
                if let Some(o) = self.overlays.iter_mut().find(|o| o.id == id) {
                    o.ready = true;
                    #[cfg(windows)]
                    tasks.push(window::run(id, move |w| {
                        use iced::window::raw_window_handle::RawWindowHandle;
                        let hwnd = match w.window_handle().map(|h| h.as_raw()) {
                            Ok(RawWindowHandle::Win32(h)) => h.hwnd.get(),
                            _ => 0,
                        };
                        Message::NativeReady(id, hwnd)
                    }));
                }
            }
            Message::CloseRequested(id) => {
                self.helper_hwnds.remove(&id);
                if self.overlays.iter().any(|o| o.id == id) {
                    out.effects
                        .extend(self.session.handle(Event::MasterOff, now));
                } else if Some(id) == self.settings_window {
                    if self.character_unsaved() {
                        self.page = Page::Character;
                        self.studio.leaving = Some(view::settings::Leave::Close);
                    } else {
                        tasks.push(self.close_settings());
                    }
                } else if Some(id) == self.first_run_window {
                    self.first_run_window = None;
                    tasks.push(window::close(id));
                } else if self.mascot_window() == Some(id) {
                    out.sets.push((character::ENABLED, Value::Bool(false)));
                }
            }
            Message::Tick => {
                #[cfg(windows)]
                if self.session.is_enabled() {
                    let down =
                        platform::windows::key_down(0x12) && !platform::windows::key_down(0x11);
                    if !platform::windows::has_keyboard_hook() && down != self.alt_down {
                        self.alt_down = down;
                        let commands: Vec<_> = self
                            .registry
                            .shortcuts(ShortcutContext::Global)
                            .filter(|(s, _)| s.key == Key::Alt)
                            .map(|(_, d)| d.id)
                            .collect();
                        for cmd in commands {
                            out.collect(if down {
                                self.registry.dispatch(cmd, &mut self.session, now).ok()
                            } else {
                                self.registry.dispatch_release(cmd, &mut self.session, now)
                            });
                        }
                    }
                    if platform::windows::buttons_up() {
                        out.effects
                            .extend(self.session.handle(Event::ButtonsReleased, now));
                    }
                }
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
            Message::SpotTick => self.track_spot_cursor(),
            Message::ToolWheel(up, shift) => {
                let target = match self.session.active_tool() {
                    t if t == crate::features::spotlight::TOOL => Some(Target::Spotlight),
                    t if t == crate::features::upcoming::TOOL => Some(Target::Lens),
                    _ if self.spotlight_anywhere.on() => Some(Target::Spotlight),
                    _ => None,
                };
                if let Some(target) = target.filter(|_| self.session.is_pointing()) {
                    let key = match (up, shift) {
                        (true, false) => Adjust::Bigger,
                        (false, false) => Adjust::Smaller,
                        (true, true) => Adjust::More,
                        (false, true) => Adjust::Less,
                    };
                    out.sets
                        .extend(anywhere::adjust(target, key, self.session.settings()));
                }
            }
            other => self.picked(other, &mut out, &mut tasks),
        }

        #[cfg(windows)]
        {
            platform::windows::arm_interact(self.session.is_enabled());
            if self.session.waiting_for_interact_resume() && platform::windows::buttons_up() {
                out.effects
                    .extend(self.session.handle(Event::ButtonsReleased, now));
            }
        }
        // Shell actions first: a wheel pick dispatches commands of its own.
        for action in std::mem::take(&mut out.shell) {
            match action {
                ShellAction::Quit => self.quit(now),
                ShellAction::OpenSettings => tasks.push(self.open_settings()),
                ShellAction::WheelOpen => self.open_wheel(now),
                ShellAction::WheelClose => self.close_wheel(now, &mut out),
                ShellAction::Anywhere(target, edge) => self.anywhere(target, edge, now),
            }
        }
        if let Some(h) = self.hotkeys.as_mut() {
            h.arm_adjust(self.lens_anywhere.held() || self.spotlight_anywhere.held());
            // Not while typing a text mark, whose Escape cancels the draft.
            h.arm_dismiss(
                (self.lens_anywhere.on() || self.spotlight_anywhere.on())
                    && self.session.text_draft().is_none(),
            );
        }
        for (id, value) in std::mem::take(&mut out.sets) {
            self.apply_setting(id, value, now);
        }
        for effect in std::mem::take(&mut out.effects) {
            match effect {
                Effect::PlayTap => self.play_tap(),
                Effect::ReturnFocus => platform::restore_foreground(self.previous_focus.take()),
                Effect::CaptureStill(ticket) => tasks.push(capture_still(
                    ticket,
                    displays::selected(
                        &self.displays,
                        self.session
                            .settings()
                            .choice(crate::features::display::TARGET),
                    ),
                )),
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
            self.audio.close();
        }
        self.sync_tray();
        self.update_lens(now, &mut out);
        for effect in std::mem::take(&mut out.effects) {
            if let Effect::Notice(text) = effect {
                self.overlay_notice = Some((text.to_owned(), now));
            }
        }
        tasks.push(self.reconcile());
        tasks.push(self.reconcile_mascot());
        self.sync_motion_preview();
        #[cfg(windows)]
        platform::windows::fine_timers(self.live_on_screen());
        self.sync_music();
        if self.quit_pending && self.studio.character_jobs.is_empty() {
            self.quit(now);
        }
        Task::batch(tasks)
    }

    fn go_page(&mut self, page: Page) {
        if page != Page::Character {
            self.cancel_character_job();
            self.studio.prepared_character = None;
            self.studio.character_conflict = None;
        }
        self.page = page;
    }

    fn close_settings(&mut self) -> Task<Message> {
        let Some(id) = self.settings_window.take() else {
            return Task::none();
        };
        self.cancel_character_job();
        self.studio.prepared_character = None;
        self.studio.character_conflict = None;
        self.settings_framed = false;
        self.notice = None;
        self.studio.leaving = None;
        window::close(id)
    }

    fn ui_message(&mut self, ui: UiMsg, now: Millis, out: &mut Outcomes) -> Task<Message> {
        if matches!(
            &ui,
            UiMsg::ArtOnHead(..)
                | UiMsg::ArtBehind(..)
                | UiMsg::ArtRemoveFace(..)
                | UiMsg::ArtRemoveEye(..)
                | UiMsg::ArtRemoveBlink(..)
        ) && let Some(e) = &mut self.studio.art
        {
            e.checkpoint();
        }
        match ui {
            UiMsg::RefreshSources => {
                #[cfg(windows)]
                {
                    self.sources = platform::windows::window_sources();
                }
            }
            UiMsg::Rebind(id) => {
                self.rebinding = Some(id);
                self.notice = Some("Press the new shortcut (Escape cancels)".into());
            }
            UiMsg::ResetShortcuts => {
                self.registry = Registry::build(crate::features::BUILTINS).expect("builtins");
                self.hotkeys = None;
                self.hotkeys = platform::hotkeys::register(&self.registry).ok();
                self.notice = preferences::save_shortcuts(&self.registry)
                    .err()
                    .map(|e| e.to_string());
            }
            UiMsg::ImportPack => {
                return Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Import pointer pack folder")
                        .pick_folder(),
                    |f| Message::PackPicked(f.map(|f| f.path().to_path_buf())),
                );
            }
            UiMsg::EditPack => {
                if let Some(pack) = self.current_pack() {
                    let id = pack.id;
                    match packs::edit_draft(pack) {
                        Ok(d) => {
                            self.pointer_draft = Some(d);
                            self.editing_pack = Some(id);
                        }
                        Err(e) => self.notice = Some(e),
                    }
                }
            }
            UiMsg::RemovePack => {
                if let Some(pack) = self.current_pack() {
                    let id = pack.id;
                    match packs::remove(pack) {
                        Ok(()) => {
                            self.packs.retain(|p| p.id != id);
                            out.sets.push((point::PACK, Value::Choice(packs::DEFAULT)));
                            self.notice = None;
                        }
                        Err(e) => self.notice = Some(e),
                    }
                }
            }
            UiMsg::ToggleMoreMenu => {
                let open = !self.ui.more_menu;
                self.ui.close_menus();
                self.ui.more_menu = open;
            }
            UiMsg::TogglePointerMenu => {
                self.ui.ink_menu = false;
                self.ui.more_menu = false;
                self.ui.pointer_menu = !self.ui.pointer_menu;
            }
            UiMsg::ToggleInkMenu => {
                self.ui.pointer_menu = false;
                self.ui.more_menu = false;
                self.ui.ink_menu = !self.ui.ink_menu;
            }
            UiMsg::ResetMascotPosition => {
                #[cfg(windows)]
                if let Some(m) = &mut self.mascot {
                    m.reset_position();
                }
                let unplaced = Value::Float(character::UNPLACED);
                out.sets.push((character::X, unplaced.clone()));
                out.sets.push((character::Y, unplaced));
            }
            UiMsg::AddPressed => {
                return Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose pressed pose on the same canvas")
                        .add_filter("PNG image", &["png"])
                        .pick_file(),
                    |f| Message::PressedPicked(f.map(|f| f.path().to_path_buf())),
                );
            }
            UiMsg::AddPackTap => {
                return Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose this pack's default tap")
                        .add_filter("WAV audio", &["wav"])
                        .pick_file(),
                    |f| Message::PackTapPicked(f.map(|f| f.path().to_path_buf())),
                );
            }
            UiMsg::AddForearm => {
                return Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose the matching forearm PNG")
                        .add_filter("PNG image", &["png"])
                        .pick_file(),
                    |file| Message::ForearmPicked(file.map(|f| f.path().to_path_buf())),
                );
            }
            UiMsg::RemoveForearm => {
                if let Some(d) = &mut self.pointer_draft {
                    d.forearm = None;
                }
            }
            UiMsg::DraftWristX(x) => {
                if let Some(d) = &mut self.pointer_draft {
                    d.wrist.0 = x;
                }
            }
            UiMsg::DraftWristY(y) => {
                if let Some(d) = &mut self.pointer_draft {
                    d.wrist.1 = y;
                }
            }
            UiMsg::DraftJointWidth(width) => {
                if let Some(d) = &mut self.pointer_draft {
                    d.joint_width = width;
                }
            }
            UiMsg::DraftForearmAnchor(x, y) => {
                if let Some(arm) = self.pointer_draft.as_mut().and_then(|d| d.forearm.as_mut()) {
                    arm.set_anchor((x, y));
                }
            }
            UiMsg::CopyPointerPrompt => {
                self.notice = Some(
                    "Pointer prompt copied. Attach your character reference when using it.".into(),
                );
                return iced::clipboard::write(packs::POINTER_PROMPT.to_owned());
            }
            UiMsg::CopyForearmPrompt => {
                self.notice = Some(
                    "Forearm prompt copied. Attach your character reference and pointer artwork."
                        .into(),
                );
                return iced::clipboard::write(packs::FOREARM_PROMPT.to_owned());
            }
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
            UiMsg::ForearmColor(color) => {
                out.sets.push((point::FOREARM_COLOR, Value::Color(color)));
                out.sets
                    .push((point::FOREARM_AUTO_COLOR, Value::Bool(false)));
            }
            UiMsg::CancelDraft => {
                self.pointer_draft = None;
                self.editing_pack = None;
            }
            UiMsg::SaveDraft => {
                if let Some(draft) = &self.pointer_draft {
                    let taken: Vec<&str> = self.packs.iter().map(|p| p.id).collect();
                    let result = if let Some(id) = self.editing_pack {
                        self.packs
                            .iter()
                            .find(|p| p.id == id)
                            .ok_or("pointer missing".into())
                            .and_then(|p| packs::reaim(p, draft))
                    } else {
                        packs::install(draft, &taken)
                    };
                    match result {
                        Ok(pack) => {
                            let id = pack.id;
                            self.packs.retain(|p| p.id != id);
                            self.packs.push(pack);
                            self.editing_pack = None;
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
                if s.bool(sound::MUTE_ALL) {
                    return Task::none();
                }
                let (tap, volume) = (s.choice(sound::TAP), s.float(sound::VOLUME) as f32);
                self.audio
                    .set_pack(self.current_pack().and_then(|p| p.tap.clone()));
                self.audio.play(tap, volume);
                self.own_sound(0.25);
                self.own_sound(0.25);
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
            other => return self.character_ui(other, now, out),
        }
        Task::none()
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
        if matches!(id, character::CHARACTER | character::ENABLED)
            && self.studio.character_job.is_some()
        {
            self.cancel_character_job();
        }
        if id == crate::features::display::TARGET {
            if let Value::Choice(target) = &value
                && displays::selected(&self.displays, target).is_empty()
            {
                self.notice = Some("Choose an available display".into());
                return;
            }
            for effect in self.session.handle(Event::Interrupted, now) {
                match effect {
                    Effect::ReturnFocus => platform::restore_foreground(self.previous_focus.take()),
                    Effect::ReleaseStill => self.still = None,
                    _ => {}
                }
            }
        }
        #[cfg(windows)]
        if id == crate::features::upcoming::SOURCE {
            self.lens.reset();
        }
        #[cfg(not(windows))]
        if id == character::ENABLED && value == Value::Bool(true) {
            self.notice = Some("The desktop character is available on Windows".into());
            return;
        }
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
                self.preference_writer.schedule(self.session.settings());
            }
            Err(why) => self.notice = Some(format!("{id}: {why}")),
        }
    }

    fn play_tap(&mut self) {
        let s = self.session.settings();
        let laser = s.choice(point::STYLE) == "laser";
        if s.bool(sound::MUTE) || s.bool(sound::MUTE_ALL) || (laser && !s.bool(sound::LASER)) {
            return;
        }
        let (tap, volume) = (s.choice(sound::TAP), s.float(sound::VOLUME) as f32);
        self.audio
            .set_pack(self.current_pack().and_then(|p| p.tap.clone()));
        self.audio.play(tap, volume);
    }

    /// The merged header's live state, when it is installed.
    fn settings_frame(&self) -> Option<view::settings::caption::Frame> {
        #[cfg(windows)]
        if self.settings_framed {
            use platform::windows::frame;
            return Some(view::settings::caption::Frame {
                hover: frame::hover(),
                maximized: frame::maximized(),
            });
        }
        None
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
        open.map(Message::WindowOpened)
            .chain(window::gain_focus(id))
    }

    fn quit(&mut self, now: Millis) {
        if !self.studio.character_jobs.is_empty() {
            self.quit_pending = true;
            for job in self.studio.character_jobs.values() {
                let _ = job.compare_exchange(0, 2, Ordering::SeqCst, Ordering::SeqCst);
            }
            self.cancel_character_job();
            self.notice = Some("Finishing character work before quitting…".into());
            return;
        }
        self.quit_pending = false;
        if let Err(e) = self.preference_writer.flush() {
            self.notice = Some(format!("Settings could not be saved; try Quit again: {e}"));
            return;
        }
        self.session.handle(Event::MasterOff, now);
        platform::restore_foreground(self.previous_focus.take());
        // Remove the tray icon now rather than leaving a ghost entry.
        self.tray = None;
        // Renderer teardown can hang on Windows after the overlay closed
        // (seen with iced 0.14); pending writes and imports are already finished.
        std::process::exit(0);
    }

    /// Runs a command the way every surface does: picking the paw or laser
    /// also switches to Point, so a key behaves like a click.
    fn run(&mut self, id: &str, now: Millis, out: &mut Outcomes) {
        out.collect(self.dispatch(id, now).ok());
        if id == point::USE_PAW || id == point::USE_LASER {
            out.collect(self.dispatch(point::SELECT, now).ok());
        }
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
        let scene = if self.session.frozen_frame().is_some() {
            "frozen"
        } else {
            "live"
        };
        let tool = self.session.active_tool();
        match self.session.mode() {
            Mode::Off => "TinyPaw is Off".into(),
            Mode::Passthrough => format!("TinyPaw is On (passthrough, {scene}, {marks} marks)"),
            Mode::Annotating(_) => {
                format!("TinyPaw is On (annotating, {tool}, {scene}, {marks} marks)")
            }
        }
    }

    /// Whether anything on screen animates or follows the cursor, so timers
    /// need to fire on time. Off with the character resting, they don't.
    #[cfg(windows)]
    fn live_on_screen(&self) -> bool {
        self.session.is_enabled()
            || self.spotlight_anywhere.on()
            || self.lens_anywhere.on()
            || self.studio.motion_preview.is_some()
            || self
                .mascot
                .as_ref()
                .and_then(|m| m.poll_interval(&self.mascot_options()))
                .is_some_and(|every| every < Duration::from_millis(100))
    }

    fn sync_tray(&mut self) {
        let label = self.registry.shortcut_label(system::MASTER_TOGGLE);
        if let Some(tray) = &mut self.tray {
            tray.sync(&self.registry, &self.session, label.as_deref());
        }
    }

    /// Brings the overlay window in line with the session's overlay state.
    fn reconcile(&mut self) -> Task<Message> {
        let want = match self.session.overlay_state() {
            // Spotlight anywhere needs the overlay to draw on, clicks still
            // going to the app underneath.
            OverlayState::Hidden if self.spotlight_anywhere.on() => OverlayState::ClickThrough,
            state => state,
        };
        if want == OverlayState::Hidden {
            return Task::batch(self.overlays.drain(..).map(|o| window::close(o.id)));
        }
        if self.overlays.is_empty() {
            let selected = displays::selected(
                &self.displays,
                self.session
                    .settings()
                    .choice(crate::features::display::TARGET),
            );
            if selected.is_empty() && self.session.overlay_state() == OverlayState::Hidden {
                self.spotlight_anywhere.set(false);
                self.notice = Some("Spotlight needs a display. Choose one under Display.".into());
                return Task::none();
            }
            if selected.is_empty() {
                self.session.handle(Event::Interrupted, self.now());
                self.notice = Some("The selected display is unavailable. Choose a display.".into());
                self.page = Page::Display;
                return self.open_settings();
            }
            if want == OverlayState::Interactive {
                self.previous_focus = platform::foreground_window();
            }
            let mut tasks = Vec::new();
            for display in selected {
                let (id, open) = window::open(overlay_settings());
                self.overlays.push(Overlay {
                    id,
                    ready: false,
                    applied: None,
                    display,
                    ink: InkCache::default(),
                    hwnd: 0,
                });
                tasks.push(open.map(Message::WindowOpened));
            }
            return Task::batch(tasks);
        }
        let mut tasks = Vec::new();
        let all_ready = self
            .overlays
            .iter()
            .all(|o| o.ready && (!cfg!(windows) || o.hwnd != 0));
        for o in &mut self.overlays {
            if !o.ready {
                continue;
            }
            let interactive = want == OverlayState::Interactive;
            if o.applied != Some(interactive) {
                o.applied = Some(interactive);
                tasks.push(if interactive {
                    // winit drops the layered style with passthrough; put
                    // it back so apps underneath keep rendering.
                    window::disable_mouse_passthrough(o.id)
                        .chain(Task::done(Message::Unoccluding(o.id)))
                } else {
                    window::enable_mouse_passthrough(o.id)
                });
            }
        }
        let buttons_up = {
            #[cfg(windows)]
            {
                platform::windows::buttons_up()
            }
            #[cfg(not(windows))]
            {
                true
            }
        };
        if all_ready
            && want == OverlayState::Interactive
            && buttons_up
            && let Some(ticket) = self.session.pending_ticket()
        {
            self.previous_focus = platform::foreground_window().or(self.previous_focus);
            let target = self
                .session
                .pointer()
                .and_then(|(id, _)| self.overlays.iter().find(|o| o.display.id == id))
                .unwrap_or(&self.overlays[0]);
            tasks.push(window::gain_focus(target.id).chain(Task::done(Message::Acquired(ticket))));
        }
        Task::batch(tasks)
    }

    fn view(&self, window: window::Id) -> Element<'_, Message> {
        if let Some(o) = self.overlays.iter().find(|o| o.id == window) {
            return view::overlay(view::Overlay {
                registry: &self.registry,
                session: &self.session,
                ink: &o.ink,
                ui: &self.ui,
                output: o.display.id,
                now: self.now(),
                still: self
                    .still
                    .as_ref()
                    .filter(|(id, _)| Some(*id) == self.session.frozen_frame())
                    .and_then(|(_, handles)| {
                        handles
                            .iter()
                            .find(|(id, _)| *id == o.display.id)
                            .map(|(_, h)| h)
                    }),
                notice: self.overlay_notice.as_ref().map(|(text, _)| text.as_str()),
                pack: self.current_pack(),
                spotlight: self.spotlight(),
            });
        }
        if Some(window) == self.settings_window {
            let built = std::time::Instant::now();
            let element = view::settings::view(view::settings::SettingsView {
                registry: &self.registry,
                session: &self.session,
                displays: &self.displays,
                #[cfg(windows)]
                sources: &self.sources,
                page: self.page,
                character_tab: self.character_tab,
                has_custom_sound: self.audio.has_custom(),
                taken: self.hotkeys.as_ref().map_or(&[], |h| h.taken.as_slice()),
                packs: &self.packs,
                pack: self.current_pack(),
                draft: self.pointer_draft.as_ref(),
                editing: self.editing_pack.is_some(),
                notice: self.notice.as_deref(),
                frame: self.settings_frame(),
                characters: &self.characters,
                character: self.current_character().id,
                character_draft: self.studio.character_draft.as_ref(),
                character_busy: self.studio.character_job.is_some(),
                character_conflict: self.studio.character_conflict.as_deref(),
                transfer: self.studio.transfer.as_ref(),
                art: self.studio.art.as_ref(),
                motion: self.studio.motion_preview.as_ref(),
                reference: self
                    .studio
                    .reference
                    .as_ref()
                    .map(|r| (r, self.studio.reference_thumb.as_ref())),
                unsaved: self.character_unsaved(),
                leaving: self.studio.leaving.is_some(),
                music: self.music_status(),
                music_source: self.music_source(),
                music_rows: self.music_rows(),
                music_inputs: self.music_inputs(),
                beat: self.beat_status(),
            });
            if std::env::var_os("TINYPAW_TRACE_DRAW").is_some() {
                eprintln!("view settings {:.1} ms", built.elapsed().as_secs_f32() * 1000.0);
            }
            return element;
        }
        #[cfg(windows)]
        if let Some(m) = self.mascot.as_ref().filter(|m| m.id == window) {
            return m.view(self.mascot_menu(), &self.mascot_options());
        }
        if Some(window) == self.first_run_window {
            return view::first_run::view(&self.registry);
        }
        iced::widget::space().into()
    }

    /// Character animation steps at the frame rate chosen in Settings.
    fn animation_ticks(&self) -> Subscription<()> {
        let rate = self
            .session
            .settings()
            .choice(crate::features::mascot::FRAME_RATE);
        match crate::features::mascot::frame_period(rate) {
            Some(every) => iced::time::every(every).map(|_| ()),
            None => Subscription::run(platform::refresh_ticks),
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        let now = self.now();
        let mut subs = vec![
            Subscription::run(platform::external_stream).map(Message::External),
            event::listen_with(|event, status, window| match event {
                // The draft owns Ctrl+Z, including events captured by text_input.
                iced::Event::Keyboard(keyboard::Event::KeyPressed {
                    ref key,
                    modifiers,
                    repeat: false,
                    ..
                }) if modifiers.control()
                    && matches!(key.as_ref(), keyboard::Key::Character("z" | "Z")) =>
                {
                    Some(Message::Key(key.clone(), modifiers, window))
                }
                iced::Event::Keyboard(keyboard::Event::KeyPressed {
                    ref key,
                    modifiers,
                    repeat: false,
                    ..
                }) if modifiers.control()
                    && matches!(key.as_ref(), keyboard::Key::Character("z" | "Z")) =>
                {
                    Some(Message::Key(key.clone(), modifiers, window))
                }
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
        #[cfg(windows)]
        if self.session.is_enabled()
            && (!platform::windows::has_keyboard_hook()
                || self.session.waiting_for_interact_resume()
                || self.session.pending_ticket().is_some())
        {
            subs.push(iced::time::every(Duration::from_millis(16)).map(|_| Message::Tick));
        } else if self.session.is_pointing()
            && self.session.active_tool() == crate::features::upcoming::TOOL
        {
            subs.push(iced::time::every(Duration::from_millis(250)).map(|_| Message::Tick));
        }
        if self.spotlight_anywhere.on() && !self.session.is_pointing() {
            subs.push(Subscription::run(platform::refresh_ticks).map(|_| Message::SpotTick));
        }
        #[cfg(windows)]
        if let Some(m) = &self.mascot
            && let Some(every) = m.poll_interval(&self.mascot_options())
        {
            // Every message rebuilds every open window, and idle polling
            // runs at the chosen frame rate (60 per second by default). While
            // Settings is open, idle motion polls at 15 per second instead so
            // its pages stay responsive; carried, thrown and popping states
            // keep full rate.
            let idle = every == Duration::from_millis(33) && !m.moving();
            // Carried or thrown: every display frame, whatever the chosen
            // rate, since the window itself is moving. Otherwise moving: at
            // the chosen frame rate. Still: a slow timer.
            subs.push(if m.moving() {
                Subscription::run(platform::refresh_ticks)
                    .map(|_| Message::Mascot(crate::mascot::Msg::Poll))
            } else if idle && self.settings_window.is_some() {
                iced::time::every(Duration::from_millis(66))
                    .map(|_| Message::Mascot(crate::mascot::Msg::Poll))
            } else if every < Duration::from_millis(100) {
                self.animation_ticks()
                    .map(|_| Message::Mascot(crate::mascot::Msg::Poll))
            } else {
                iced::time::every(every).map(|_| Message::Mascot(crate::mascot::Msg::Poll))
            });
        }
        // The Motion tab's preview animates while it's open.
        if self.studio.motion_preview.is_some() {
            // Slowly, to refresh what the canvas can't see; faster while
            // music plays, which it follows.
            let every = if self.music_level() > 0.0 { 100 } else { 500 };
            subs.push(iced::time::every(Duration::from_millis(every)).map(|_| Message::MotionTick));
        }
        // Frames only while something animates; otherwise the process sleeps.
        // Paced by the display refresh: `window::frames()` fires for every
        // window's redraw and each redraw requests the next, so animation
        // ran uncapped and once per open window.
        if self.session.animating(now) {
            subs.push(Subscription::run(platform::refresh_ticks).map(|_| Message::Frame));
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
fn capture_still(ticket: Ticket, displays: Vec<Display>) -> Task<Message> {
    let (tx, rx) = iced::futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        let result = displays
            .iter()
            .map(|d| {
                platform::capture::display(d)
                    .map(|s| (d.id, image::Handle::from_rgba(s.width, s.height, s.rgba)))
            })
            .collect();
        let _ = tx.send(result);
    });
    Task::perform(rx, move |r| {
        Message::Still(
            ticket,
            r.unwrap_or_else(|_| Err("capture thread ended".into())),
        )
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

/// Filesystem and PNG work runs independently of Iced's event executor.
/// A character's replacement pictures for the desktop: its held pose and
/// Click and Drop artwork, from the pack and the user's overrides.
#[cfg(windows)]
fn character_pictures(c: &Character) -> crate::mascot::desktop::Pictures {
    use crate::mascot::reactions::{self, Event};
    let pack = c.pack_reactions();
    let user = characters::load_overrides(c.id)
        .reactions
        .unwrap_or_default();
    let picture = |art: Option<reactions::Art>| {
        let art = art?;
        let img = c.reaction_image(&art)?;
        Some(crate::mascot::rig::Picture::new(&img, art.place))
    };
    crate::mascot::desktop::Pictures {
        held: picture(reactions::held(&pack, &user)),
        click: picture(reactions::art(&pack, &user, Event::Click)),
        drop: picture(reactions::art(&pack, &user, Event::Drop)),
    }
}

fn character_work(
    id: u64,
    work: impl FnOnce() -> Result<CharacterWork, String> + Send + 'static,
) -> Task<Message> {
    let (tx, rx) = iced::futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(work());
    });
    Task::perform(rx, move |r| {
        Message::CharacterWorked(
            id,
            r.unwrap_or_else(|_| Err("Character worker ended unexpectedly".into())),
        )
    })
}
