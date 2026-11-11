//! Iced event loop: normalizes input into session events, runs requested
//! effects and keeps the overlay, settings and first-run windows in step with
//! the session.

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
use crate::features::{mascot as character, point, sound, system, wheel as wheel_feature};
use crate::mascot::pack::{self as characters, Character};
use crate::packs::{self, Draft, Pack};
use crate::platform::{
    self, External,
    displays::{self, Display},
};
use crate::preferences;
use crate::registry::{
    Dispatched, Key, Registry, ShellAction, ShortcutContext, Unavailable, Value,
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
    CharacterExportPicked(Option<PathBuf>),
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
}

#[derive(Debug, Clone)]
pub enum CharacterWork {
    Draft(characters::Draft),
    Editor(Box<crate::mascot::editor::Editor>, Vec<String>),
    Prepared(Arc<characters::PreparedFolder>),
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
    /// "Add character" in progress.
    character_draft: Option<characters::Draft>,
    /// A folder import waiting on Keep both or Replace: its display name.
    character_conflict: Option<String>,
    prepared_character: Option<Arc<characters::PreparedFolder>>,
    character_request: u64,
    character_job: Option<Arc<AtomicU8>>,
    character_jobs: std::collections::BTreeMap<u64, Arc<AtomicU8>>,
    quit_pending: bool,
    /// The Art tab's open character, while editing.
    art: Option<crate::mascot::editor::Editor>,
    /// The Motion tab's live preview, while Settings shows Character.
    motion_preview: Option<crate::mascot::preview::MotionPreview>,
    /// Somewhere the user asked to go with unsaved character changes,
    /// waiting on Save, Discard or Stay.
    leaving: Option<view::settings::Leave>,
    /// The character whose artwork Save is writing, so its behavior
    /// overrides follow once the pack is committed.
    art_saving: Option<&'static str>,
    /// Where to go once that save finishes, from Save in the leave prompt.
    after_save: Option<view::settings::Leave>,
    /// A character whose preview failed to load, so it isn't retried
    /// on every update.
    motion_failed: Option<&'static str>,
    motion_ticked: Option<Instant>,
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
    /// The lens shortcut is held, with annotation on or off.
    lens_anywhere: bool,
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
            character_draft: None,
            character_conflict: None,
            prepared_character: None,
            character_request: 0,
            character_job: None,
            character_jobs: std::collections::BTreeMap::new(),
            quit_pending: false,
            art: None,
            motion_preview: None,
            leaving: None,
            art_saving: None,
            after_save: None,
            motion_failed: None,
            motion_ticked: None,
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
            lens_anywhere: false,
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
                if let Some(cmd) = self.hotkeys.as_ref().and_then(|h| h.command(id)) {
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
            Message::PressedPicked(path) => {
                if let (Some(path), Some(draft)) = (path, self.pointer_draft.as_mut()) {
                    self.notice = packs::set_pressed(draft, &path).err();
                }
            }
            Message::PackTapPicked(path) => {
                if let (Some(path), Some(draft)) = (path, self.pointer_draft.as_mut()) {
                    match audio::decode_wav(&path) {
                        Ok(clip) => {
                            draft.tap = Some((clip, path));
                            self.notice = None;
                        }
                        Err(e) => self.notice = Some(e),
                    }
                }
            }
            Message::PackPicked(path) => {
                if let Some(path) = path {
                    let taken: Vec<_> = self.packs.iter().map(|p| p.id).collect();
                    match packs::import_folder(&path, &taken) {
                        Ok(pack) => {
                            let id = pack.id;
                            self.packs.push(pack);
                            out.sets.push((point::PACK, Value::Choice(id)));
                            self.notice = None;
                        }
                        Err(e) => self.notice = Some(e),
                    }
                }
            }
            Message::ForearmPicked(path) => {
                if let Some(path) = path {
                    match packs::forearm_from_file(&path) {
                        Ok(arm) => {
                            if let Some(draft) = &mut self.pointer_draft {
                                draft.forearm = Some(arm);
                            }
                            self.notice = None;
                        }
                        Err(e) => self.notice = Some(format!("Could not load the forearm: {e}")),
                    }
                }
            }
            Message::CharacterPicked(Some(path)) => {
                self.character_draft = None;
                self.prepared_character = None;
                self.character_conflict = None;
                self.page = Page::Character;
                let id = self.begin_character_job();
                tasks.push(character_work(id, move || {
                    characters::draft_from_png(&path).map(CharacterWork::Draft)
                }));
            }
            Message::CharacterFolderPicked(Some(path)) => {
                self.character_draft = None;
                self.prepared_character = None;
                self.character_conflict = None;
                self.page = Page::Character;
                let id = self.begin_character_job();
                tasks.push(character_work(id, move || {
                    characters::prepare_folder(&path).map(CharacterWork::Prepared)
                }));
            }
            Message::CharacterWorked(id, result) => {
                self.character_jobs.remove(&id);
                if id != self.character_request
                    && let Err(e) = &result
                    && e != "Import cancelled"
                {
                    self.notice = Some(format!("Background character import failed: {e}"));
                }
                // A completed commit always updates the catalog, even after cancellation.
                if let Ok(CharacterWork::Imported(characters::Imported::Installed(c), show)) =
                    &result
                {
                    if !self.session.settings().bool(character::ENABLED)
                        && (!show || id != self.character_request)
                    {
                        c.release_prepared_rig();
                    }
                    self.add_character(c.clone());
                    #[cfg(windows)]
                    if self.mascot.as_ref().is_some_and(|m| m.character() == c.id) {
                        let rig = self.character_rig(c.id);
                        if let Some(m) = &mut self.mascot {
                            m.set_rig(c.id, rig);
                        }
                    }
                }
                if id == self.character_request {
                    self.character_job = None;
                    self.notice = None;
                    match result {
                        Ok(CharacterWork::Draft(draft)) => self.character_draft = Some(draft),
                        Ok(CharacterWork::Editor(editor, errors)) => {
                            self.art = Some(*editor);
                            if !errors.is_empty() {
                                self.notice = Some(errors.join("; "));
                            }
                        }
                        Ok(CharacterWork::Prepared(prepared)) => {
                            self.prepared_character = Some(prepared);
                            tasks.push(self.import_character(characters::OnDuplicate::Ask));
                        }
                        Ok(CharacterWork::Imported(
                            characters::Imported::Duplicate { name },
                            _,
                        )) => {
                            self.character_conflict = Some(name);
                        }
                        Ok(CharacterWork::Imported(characters::Imported::Installed(c), show)) => {
                            self.character_conflict = None;
                            self.prepared_character = None;
                            self.character_draft = None;
                            self.art = None;
                            // One Save, two writes: the pack is committed, so
                            // now the behavior draft. A failure keeps it open.
                            if self.art_saving.take() == Some(c.id) {
                                match self.save_behavior() {
                                    Ok(()) => {
                                        // Rebuilt from the saved pack and overrides.
                                        if self.motion_preview.as_ref().is_some_and(|p| p.id == c.id) {
                                            self.motion_preview = None;
                                        }
                                        if let Some(to) = self.after_save.take() {
                                            tasks.push(self.finish_leave(to, &mut out));
                                        }
                                    }
                                    Err(e) => {
                                        self.after_save = None;
                                        self.notice = Some(format!(
                                            "The artwork is saved, but the behavior changes could not be: {e}. They are still open; try Save again."
                                        ));
                                    }
                                }
                            }
                            out.sets.push((character::CHARACTER, Value::Choice(c.id)));
                            if show {
                                out.sets.push((character::ENABLED, Value::Bool(true)));
                            }
                        }
                        Err(e) => {
                            self.art_saving = None;
                            self.after_save = None;
                            self.notice = Some(format!("Could not import that character: {e}"))
                        }
                    }
                }
            }
            Message::CharacterExportPicked(Some(into)) => {
                if !self.character_jobs.is_empty() {
                    self.notice = Some(
                        "Character work is still finishing. Try Export when it completes.".into(),
                    );
                    return Task::none();
                }
                let c = self.current_character().clone();
                self.notice = Some(match characters::export(&c, &into) {
                    Ok(folder) => format!("Exported {} to {}", c.name, folder.display()),
                    Err(e) => format!("Could not export {}: {e}", c.name),
                });
            }
            Message::ArtBodyPicked(generation, Some(path))
                if generation == self.character_request =>
            {
                let id = self.begin_character_job();
                let taken: Vec<_> = self.characters.iter().map(|c| c.id).collect();
                tasks.push(character_work(id, move || {
                    crate::mascot::editor::Editor::from_body(&path, &taken)
                        .map(|e| CharacterWork::Editor(Box::new(e), Vec::new()))
                }));
            }
            Message::ArtFilesPicked(generation, target, paths)
                if generation == self.character_request =>
            {
                if !paths.is_empty()
                    && let Some(mut editor) = self.art.clone()
                {
                    let id = self.begin_character_job();
                    tasks.push(character_work(id, move || {
                        let errors = editor.set_images(target, paths);
                        Ok(CharacterWork::Editor(Box::new(editor), errors))
                    }));
                }
            }
            Message::ArtBodyPicked(_, _) | Message::ArtFilesPicked(_, _, _) => {}
            Message::LineSoundPicked(t, i, Some(path)) => {
                if let Some(p) = &mut self.motion_preview {
                    match characters::import_line_sound(p.id, &path) {
                        Ok(sound) => {
                            if let Some(line) = p.lines.get_mut(t).get_mut(i) {
                                line.sound = Some(sound);
                            }
                            self.notice = None;
                        }
                        Err(e) => self.notice = Some(format!("Could not use that sound: {e}")),
                    }
                }
            }
            Message::LineSoundPicked(_, _, None) => {}
            Message::MotionTick => {
                let now_i = Instant::now();
                let dt = self
                    .motion_ticked
                    .map_or(0.0, |t| now_i.duration_since(t).as_secs_f32());
                self.motion_ticked = Some(now_i);
                let o = self.motion_options();
                let music = self.music_level();
                let beat = self.beat_phase();
                if let Some(p) = &mut self.motion_preview {
                    p.tick(dt, o.0, o.1, music, beat);
                }
            }
            Message::CharacterPicked(None)
            | Message::CharacterFolderPicked(None)
            | Message::CharacterExportPicked(None) => {}
            Message::PointerPicked(path) => {
                if let Some(path) = path {
                    match packs::draft_from_file(&path) {
                        Ok(draft) => {
                            self.pointer_draft = Some(draft);
                            self.editing_pack = None;
                            self.page = Page::Pointer;
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
                    && self.character_job.is_none()
                    && self.art.is_some()
                {
                    let delta = match key.as_ref() {
                        keyboard::Key::Named(Named::ArrowLeft) => Some((-1.0, 0.0)),
                        keyboard::Key::Named(Named::ArrowRight) => Some((1.0, 0.0)),
                        keyboard::Key::Named(Named::ArrowUp) => Some((0.0, -1.0)),
                        keyboard::Key::Named(Named::ArrowDown) => Some((0.0, 1.0)),
                        _ => None,
                    };
                    if let Some((x, y)) = delta
                        && let Some(e) = &mut self.art
                    {
                        let step = if modifiers.shift() { 10.0 } else { 1.0 };
                        e.checkpoint();
                        e.nudge(x * step, y * step);
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
                        self.leaving = Some(view::settings::Leave::Close);
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
                ShellAction::LensShow => self.lens_anywhere = true,
                ShellAction::LensHide => self.lens_anywhere = false,
            }
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
        self.sync_music();
        if self.quit_pending && self.character_jobs.is_empty() {
            self.quit(now);
        }
        Task::batch(tasks)
    }

    fn mascot_window(&self) -> Option<window::Id> {
        #[cfg(windows)]
        {
            self.mascot.as_ref().map(|m| m.id)
        }
        #[cfg(not(windows))]
        {
            None
        }
    }

    #[cfg(windows)]
    /// Playback loudness scaled by Music strength, 0 when off or silent.
    fn music_level(&self) -> f32 {
        #[cfg(windows)]
        if let Some(m) = &self.music {
            let strength = self.session.settings().float(character::MUSIC_STRENGTH) as f32;
            return m.level() * strength;
        }
        0.0
    }

    fn beat_sensitivity(&self) -> crate::mascot::beat::Sensitivity {
        use crate::mascot::beat::Sensitivity;
        match self.session.settings().choice(character::BEAT_SENSITIVITY) {
            "low" => Sensitivity::Low,
            "high" => Sensitivity::High,
            _ => Sensitivity::Normal,
        }
    }

    /// The followed beat's phase, shifted by Beat timing, while music plays.
    fn beat_phase(&self) -> Option<f32> {
        #[cfg(windows)]
        if self.music_level() > 0.0 {
            let offset = match self.session.settings().choice(character::BEAT_TIMING) {
                "earlier" => 0.06,
                "later" => -0.06,
                _ => 0.0,
            };
            // The followed beat when there is one; the clock eases to it,
            // and back to the steady pace when it's lost.
            let target = self
                .beat
                .as_ref()
                .and_then(|b| b.phase(offset).zip(b.bpm()));
            let (mut clock, last) = self.beat_clock.get();
            let now = Instant::now();
            let dt = last.map_or(0.0, |l| now.duration_since(l).as_secs_f32());
            let phase = clock.step(dt, target);
            self.beat_clock.set((clock, Some(now)));
            return Some(phase);
        }
        None
    }

    /// The Follow beat trial's status line.
    fn beat_status(&self) -> String {
        #[cfg(windows)]
        if self.beat.is_none()
            && self.music.is_some()
            && self.session.settings().bool(character::BEAT)
            && let source @ crate::mascot::music::Source::App(_) = self.music_source()
        {
            return format!("Starts when {source} plays");
        }
        #[cfg(windows)]
        if let Some(b) = &self.beat {
            return match (b.available(), b.bpm()) {
                (false, _) => "Can't listen to the output".into(),
                // The eased tempo the character is moving at.
                (true, Some(_)) => format!("Following · {:.0} BPM", self.beat_clock.get().0.bpm()),
                (true, None) => "Looking for a beat".into(),
            };
        }
        if self.session.settings().bool(character::BEAT) {
            "Starts with music response".into()
        } else {
            "Off".into()
        }
    }

    /// TinyPaw is playing a sound: Follow beat's loopback hears it too, so
    /// onsets are held off meanwhile.
    fn own_sound(&self, seconds: f32) {
        // A single app's capture never hears TinyPaw.
        #[cfg(windows)]
        if let Some(b) = &self.beat
            && !matches!(b.target(), platform::beat::Target::Process(_))
        {
            b.hold_off(seconds);
        }
        let _ = seconds;
    }

    /// What the Music section shows: off, starting, silent, playing or
    /// unavailable, with the raw level for the meter.
    fn music_status(&self) -> (String, f32) {
        #[cfg(windows)]
        if let Some(m) = &self.music {
            use crate::mascot::music::Source;
            use platform::music::State;
            let source = self.music_source();
            return match (m.state(), &source) {
                (State::Starting, _) => ("Listening…".into(), 0.0),
                (State::Silent, Source::App(exe)) if m.app_pids(exe).is_empty() => {
                    (format!("{source} isn't playing"), 0.0)
                }
                (State::Silent, _) => ("Too quiet to move to".into(), m.heard()),
                (State::Playing, _) => ("Moving to it".into(), m.heard()),
                (State::Unavailable, Source::Input(name)) => (format!("Can't open {name}"), 0.0),
                (State::Unavailable, _) => {
                    ("Unavailable: no playback device to listen to".into(), 0.0)
                }
            };
        }
        if self.session.settings().bool(character::MUSIC) {
            ("Starts while the character or this page is showing".into(), 0.0)
        } else {
            ("Off".into(), 0.0)
        }
    }

    /// What music response listens to.
    fn music_source(&self) -> crate::mascot::music::Source {
        crate::mascot::music::Source::parse(self.session.settings().choice(character::MUSIC_SOURCE))
    }

    /// The app sources to pick from: all apps, then each app playing now,
    /// with live meters; the saved app stays listed while it's quiet.
    fn music_rows(&self) -> Vec<(crate::mascot::music::Source, f32, bool)> {
        use crate::mascot::music::Source;
        let mut rows = vec![(Source::All, 0.0, true)];
        #[cfg(windows)]
        if let Some(m) = &self.music {
            rows[0].1 = m.all_apps();
            rows.extend(m.apps().into_iter().map(|a| (Source::App(a.exe), a.shown, true)));
        }
        let current = self.music_source();
        if matches!(current, Source::App(_)) && !rows.iter().any(|(s, _, _)| *s == current) {
            rows.push((current, 0.0, false));
        }
        rows
    }

    /// The input devices, keeping the saved one while it's unplugged.
    fn music_inputs(&self) -> Vec<crate::mascot::music::Source> {
        use crate::mascot::music::Source;
        #[cfg_attr(not(windows), allow(unused_mut))]
        let mut list: Vec<Source> = Vec::new();
        #[cfg(windows)]
        if let Some(m) = &self.music {
            list.extend(m.inputs().iter().map(|n| Source::input(n)));
        }
        let current = self.music_source();
        if matches!(current, Source::Input(_)) && !list.contains(&current) {
            list.push(current);
        }
        list
    }

    /// Runs the playback meter only while music response is on and the
    /// desktop character is showing, or the Character page is open to show
    /// the meter; otherwise it's stopped and costs nothing. Also passes on
    /// "music started" to the character.
    fn sync_music(&mut self) {
        #[cfg(windows)]
        {
            let on = self.session.settings().bool(character::MUSIC);
            let showing = self.mascot.as_ref().is_some_and(|m| !m.is_hidden());
            let wanted = on && (showing || self.motion_preview.is_some());
            use crate::mascot::music::Source;
            use platform::beat::Target;
            let source = self.music_source();
            match (wanted, &self.music) {
                (true, None) => self.music = Some(platform::music::Meter::start(source.clone())),
                (true, Some(m)) => m.set_source(&source),
                (false, Some(_)) => self.music = None,
                (false, None) => {}
            }
            // Follow beat runs only alongside music response, on the same
            // source: one app's own audio while it's playing.
            let target = match &source {
                Source::All => Some(Target::Output),
                Source::Input(name) => Some(Target::Input(name.clone())),
                Source::App(exe) => {
                    let pids = self.music.as_ref().map(|m| m.app_pids(exe)).unwrap_or_default();
                    match self.beat.as_ref().map(|b| b.target()) {
                        // Keep a capture still on one of the app's processes.
                        Some(Target::Process(pid)) if pids.contains(pid) => Some(Target::Process(*pid)),
                        _ => pids.first().map(|pid| Target::Process(*pid)),
                    }
                }
            };
            let follow = wanted && self.session.settings().bool(character::BEAT);
            let sensitivity = self.beat_sensitivity();
            match (follow.then_some(target).flatten(), &self.beat) {
                (Some(t), Some(b)) if *b.target() == t => b.set_sensitivity(sensitivity),
                (Some(t), _) => self.beat = Some(platform::beat::Follower::start(sensitivity, t)),
                (None, Some(_)) => self.beat = None,
                (None, None) => {}
            }
            if self.music.as_ref().is_some_and(|m| m.take_started()) {
                let options = self.mascot_options();
                if let Some(m) = &mut self.mascot {
                    m.say(crate::mascot::lines::Trigger::MusicStart, &options);
                }
                self.play_mascot_sound();
            }
        }
    }

    /// Idle motion on, and reduced motion, for the Motion tab preview.
    fn motion_options(&self) -> (bool, bool) {
        let s = self.session.settings();
        (s.bool(character::IDLE), system::reduced(s))
    }

    /// Builds the Motion tab's preview while Settings shows the Character
    /// page (not while editing art or adding a character), and drops it
    /// otherwise so its art isn't held in memory.
    fn sync_motion_preview(&mut self) {
        // Kept while the artwork editor is open: one draft spans both.
        let wanted = self.settings_window.is_some()
            && self.page == Page::Character
            && self.character_draft.is_none();
        if !wanted {
            self.motion_preview = None;
            self.motion_ticked = None;
            return;
        }
        let c = self.current_character().clone();
        if self.motion_preview.as_ref().is_some_and(|p| p.id == c.id)
            || self.motion_failed == Some(c.id)
        {
            return;
        }
        match c.rig() {
            Ok(base) => {
                let defaults = base.motion().to_vec();
                let mut rig = base;
                if let Some(specs) = characters::load_motion(c.id) {
                    rig.set_motion(&specs);
                }
                rig.set_bop(c.bop());
                self.motion_preview = Some(crate::mascot::preview::MotionPreview::new(
                    c.id,
                    rig,
                    defaults,
                    c.lines(),
                    c.pack_lines(),
                    c.pack_bop(),
                ));
                self.motion_failed = None;
                self.motion_ticked = None;
            }
            Err(e) => {
                self.motion_preview = None;
                self.motion_failed = Some(c.id);
                self.notice = Some(format!("Could not preview {}: {e}", c.name));
            }
        }
    }

    fn mascot_options(&self) -> crate::mascot::desktop::Options {
        let s = self.session.settings();
        crate::mascot::desktop::Options {
            eyes: s.bool(character::EYES),
            idle: s.bool(character::IDLE),
            reduced: system::reduced(s),
            lines: s.bool(character::LINES),
            quiet: s.bool(character::QUIET_LINES),
            idle_every: (
                s.float(character::IDLE_MIN) as f32 * 60.0,
                s.float(character::IDLE_MAX) as f32 * 60.0,
            ),
            music: self.music_level(),
            beat: self.beat_phase(),
        }
    }

    #[cfg_attr(not(windows), allow(unused_variables))]
    fn mascot_message(
        &mut self,
        msg: crate::mascot::Msg,
        now: Millis,
        out: &mut Outcomes,
        tasks: &mut Vec<Task<Message>>,
    ) {
        use crate::mascot::Msg;
        match msg {
            // Menu entries go through the registry like every other surface.
            Msg::Command(id) => {
                #[cfg(windows)]
                if let Some(m) = &mut self.mascot {
                    m.close_menu();
                }
                out.collect(self.dispatch(id, now).ok());
            }
            Msg::CharacterSettings => {
                #[cfg(windows)]
                if let Some(m) = &mut self.mascot {
                    m.close_menu();
                }
                self.page = Page::Character;
                tasks.push(self.open_settings());
            }
            #[cfg(windows)]
            msg => {
                let options = self.mascot_options();
                if let Some(m) = &mut self.mascot {
                    let (task, save) = m.update(msg, &options);
                    tasks.push(task);
                    if let Some((x, y)) = save {
                        out.sets.push((character::X, Value::Float(x)));
                        out.sets.push((character::Y, Value::Float(y)));
                    }
                }
                self.play_mascot_sound();
            }
            #[cfg(not(windows))]
            _ => {}
        }
    }

    /// Opens, closes, resizes and hides the character window to match its
    /// settings. It stays while annotation is Off and steps aside while the
    /// annotation surface owns input.
    fn reconcile_mascot(&mut self) -> Task<Message> {
        #[cfg(windows)]
        {
            let s = self.session.settings();
            let want = s.bool(character::ENABLED);
            let size = s.float(character::SIZE) as f32;
            let hidden = self.session.overlay_state() == OverlayState::Interactive;
            // The menu is as tall as its visible rows.
            let rows = self.mascot_menu().len();
            let selected = s.choice(character::CHARACTER);
            let anchor = (s.float(character::X), s.float(character::Y));
            let stale = self
                .mascot
                .as_ref()
                .is_some_and(|m| m.character() != selected);
            let options = self.mascot_options();
            if want && (self.mascot.is_none() || stale) {
                let rig = self.character_rig(selected);
                let lines = self.character_lines(selected);
                self.stop_character_voice();
                match &mut self.mascot {
                    Some(m) => {
                        m.set_rig(selected, rig);
                        m.set_lines(lines, &options);
                    }
                    None => {
                        let (mut m, open) = crate::mascot::desktop::Desktop::open(
                            selected, rig, size, anchor, rows, hidden,
                        );
                        m.set_lines(lines, &options);
                        self.mascot = Some(m);
                        return open.map(Message::WindowOpened);
                    }
                }
            }
            match (&mut self.mascot, want) {
                (None, true) => {}
                (Some(_), false) => {
                    self.stop_character_voice();
                    let m = self.mascot.take().expect("mascot window");
                    return window::close(m.id);
                }
                (Some(m), true) => {
                    m.resize(size, rows);
                    let was_hidden = m.is_hidden();
                    let task = m.set_hidden(hidden);
                    if hidden && !was_hidden {
                        // Annotation owns input: character audio stops.
                        self.voice.stop();
                        self.audio.stop_voice();
                    }
                    let Some(m) = &mut self.mascot else {
                        return task;
                    };
                    // Back from annotation: the character can say so.
                    if was_hidden && !hidden {
                        m.say(crate::mascot::lines::Trigger::AnnotationEnd, &options);
                    }
                    self.play_mascot_sound();
                    return task;
                }
                (None, false) => {}
            }
        }
        Task::none()
    }

    /// The character's lines: the user's if they changed them, else the pack's.
    fn character_lines(&self, id: &str) -> crate::mascot::lines::Lines {
        self.characters
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.lines())
            .unwrap_or_default()
    }

    /// Routes the desktop character's gesture events through the one
    /// character voice, then its line sound. A trigger plays one sound:
    /// an accepted reaction sound wins over that trigger's line sound.
    fn play_mascot_sound(&mut self) {
        #[cfg(windows)]
        {
            use crate::mascot::reactions::{Event, Quiet};
            let now = self.voice_clock.elapsed().as_secs_f64();
            let Some(m) = &mut self.mascot else {
                return;
            };
            let mut events = m.take_events();
            let (free, hidden) = (m.idle_free(), m.is_hidden());
            let line = m.take_sound();
            let pick = || (self.voice_clock.elapsed().subsec_nanos() % 1000) as f32 / 1000.0;
            if self.voice.idle_due(&self.reaction_sounds, free, now, pick)
                && !platform::windows::foreground_fullscreen()
            {
                events.push(Event::Idle);
            }
            let quiet = Quiet {
                mute_all: self.session.settings().bool(sound::MUTE_ALL),
                annotating: hidden,
                hidden,
            };
            let mut reacted = false;
            for e in events {
                if let Ok(play) = self.voice.accept(e, &self.reaction_sounds, quiet, now)
                    && let Some(seconds) = self.play_reaction(&play)
                {
                    self.voice.started(now, seconds);
                    reacted = true;
                }
            }
            if !reacted
                && let Some(sound) = line
                && let Some(seconds) = self.play_line_sound(&sound)
                && let Some(m) = &mut self.mascot
            {
                m.hold_speech(seconds);
            }
            self.audio.release_idle(self.session.is_enabled());
        }
    }

    /// Plays an accepted reaction as the character voice; how long it runs.
    fn play_reaction(&mut self, play: &crate::mascot::reactions::Play) -> Option<f32> {
        use crate::mascot::reactions::Sound;
        let clip = match &play.sound {
            Sound::Builtin(id) => self.audio.bundled(id)?,
            Sound::File(r) => {
                let path = self.current_character().sound_path(r)?;
                crate::audio::decode_wav(&path)
                    .inspect_err(|e| eprintln!("tinypaw: reaction sound {}: {e}", path.display()))
                    .ok()?
            }
        };
        if !self.audio.play_voice(&clip, play.volume) {
            self.notice = Some("No audio output for character sounds right now.".into());
            return None;
        }
        self.own_sound(clip.seconds());
        Some(clip.seconds())
    }

    /// Hiding, switching or removing the character stops its voice and its
    /// pending idle work.
    fn stop_character_voice(&mut self) {
        self.voice.stop();
        self.audio.stop_voice();
    }

    /// Plays a line sound as the character voice at Character volume;
    /// returns how long it runs.
    fn play_line_sound(&mut self, sound: &crate::mascot::lines::SoundRef) -> Option<f32> {
        if self.session.settings().bool(sound::MUTE_ALL) {
            return None;
        }
        let volume = self.session.settings().float(character::VOICE_VOLUME) as f32;
        let path = self.current_character().sound_path(sound)?;
        match crate::audio::decode_wav(&path) {
            Ok(clip) => {
                if !self.audio.play_voice(&clip, volume) {
                    return None;
                }
                self.own_sound(clip.seconds());
                Some(clip.seconds())
            }
            Err(e) => {
                eprintln!("tinypaw: line sound {}: {e}", path.display());
                None
            }
        }
    }

    /// Decodes the selected character's art. A missing or broken pack shows
    /// Yohta instead and says why, without retrying until the choice changes.
    #[cfg(windows)]
    fn character_rig(&mut self, id: &str) -> crate::mascot::rig::Rig {
        let Some(c) = self.characters.iter().find(|c| c.id == id) else {
            return crate::mascot::rig::Rig::load();
        };
        c.rig_with_motion().unwrap_or_else(|e| {
            self.notice = Some(format!("Could not show {}: {e}. Showing Yohta.", c.name));
            crate::mascot::rig::Rig::load()
        })
    }

    /// Whether the character draft (artwork or behavior) has changes Save
    /// would write.
    fn character_unsaved(&self) -> bool {
        self.motion_preview.as_ref().is_some_and(|p| p.unsaved())
            || self.art.as_ref().is_some_and(|e| e.new_install || e.can_undo())
    }

    /// Writes the behavior draft to the character's overrides in one file,
    /// then shows it on the desktop. Nothing to write is a success.
    fn save_behavior(&mut self) -> Result<(), String> {
        let Some(p) = &mut self.motion_preview else {
            return Ok(());
        };
        if !p.unsaved() {
            return Ok(());
        }
        let id = p.id;
        let o = p.overrides(characters::load_overrides(id));
        characters::save_overrides(id, &o)?;
        p.mark_all_saved();
        characters::prune_line_sounds(id, &p.lines);
        self.notice = None;
        #[cfg(windows)]
        if self.mascot.as_ref().is_some_and(|m| m.character() == id) {
            let lines = p.lines.clone();
            let rig = self.character_rig(id);
            let options = self.mascot_options();
            if let Some(m) = &mut self.mascot {
                m.set_rig(id, rig);
                m.set_lines(lines, &options);
            }
        }
        Ok(())
    }

    /// Cancel / Discard: drops the artwork edit and every unsaved behavior
    /// change. The installed character is untouched.
    fn discard_character(&mut self) {
        self.cancel_character_job();
        self.art = None;
        self.art_saving = None;
        if let Some(p) = &mut self.motion_preview {
            p.discard();
        }
    }

    fn go_page(&mut self, page: Page) {
        if page != Page::Character {
            self.cancel_character_job();
            self.prepared_character = None;
            self.character_conflict = None;
        }
        self.page = page;
    }

    fn pick_character(&mut self, id: &'static str, out: &mut Outcomes) {
        self.cancel_character_job();
        self.prepared_character = None;
        self.character_conflict = None;
        self.character_draft = None;
        self.art = None;
        out.sets.push((character::CHARACTER, Value::Choice(id)));
    }

    fn close_settings(&mut self) -> Task<Message> {
        let Some(id) = self.settings_window.take() else {
            return Task::none();
        };
        self.cancel_character_job();
        self.prepared_character = None;
        self.character_conflict = None;
        self.settings_framed = false;
        self.notice = None;
        self.leaving = None;
        window::close(id)
    }

    /// Goes where the leave prompt was holding.
    fn finish_leave(&mut self, to: view::settings::Leave, out: &mut Outcomes) -> Task<Message> {
        use view::settings::Leave;
        match to {
            Leave::Page(page) => self.go_page(page),
            Leave::Character(id) => self.pick_character(id, out),
            Leave::Close => return self.close_settings(),
        }
        Task::none()
    }

    /// The selected character, or bundled Yohta if it's gone.
    fn current_character(&self) -> &Character {
        let id = self.session.settings().choice(character::CHARACTER);
        self.characters
            .iter()
            .find(|c| c.id == id)
            .unwrap_or(&self.characters[0])
    }

    /// Adds or replaces a character in the list, keeping Yohta first and the
    /// rest by name.
    fn add_character(&mut self, c: Character) {
        self.characters.retain(|x| x.id != c.id);
        self.characters.push(c);
        self.characters[1..].sort_by_key(|c| c.name.to_lowercase());
    }

    fn cancel_character_job(&mut self) {
        self.character_request += 1;
        if let Some(job) = self.character_job.take() {
            let _ = job.compare_exchange(0, 2, Ordering::SeqCst, Ordering::SeqCst);
        }
    }

    fn begin_character_job(&mut self) -> u64 {
        self.cancel_character_job();
        self.character_job = Some(Arc::new(AtomicU8::new(0)));
        self.character_jobs.insert(
            self.character_request,
            self.character_job.as_ref().unwrap().clone(),
        );
        self.notice = Some("Preparing character…".into());
        self.character_request
    }

    fn import_character(&mut self, choice: characters::OnDuplicate) -> Task<Message> {
        let Some(prepared) = self.prepared_character.clone() else {
            return Task::none();
        };
        let id = self.begin_character_job();
        let job = self.character_job.as_ref().unwrap().clone();
        let taken: Vec<_> = self.characters.iter().map(|c| c.id).collect();
        character_work(id, move || {
            prepared
                .install(&taken, choice, &job)
                .map(|r| CharacterWork::Imported(r, false))
        })
    }

    /// The character's menu: annotation, its own settings and Hide. Commands
    /// go through the registry and are disabled when unavailable; eye
    /// tracking and tap mute live in Settings. A pose choice joins these once
    /// a pack has more than one pose.
    #[cfg(windows)]
    fn mascot_menu(&self) -> Vec<crate::mascot::desktop::Item> {
        use crate::mascot::{Msg, desktop::Item};
        let command = |id: &'static str, label: &'static str, checked: Option<bool>| Item {
            label,
            checked,
            on_press: self
                .registry
                .command(id)
                .filter(|d| self.registry.availability(d.id, &self.session).is_ok())
                .map(|_| Msg::Command(id)),
        };
        vec![
            command(
                system::MASTER_TOGGLE,
                "Annotation enabled",
                Some(self.session.is_enabled()),
            ),
            Item {
                label: "Character settings…",
                checked: None,
                on_press: Some(Msg::CharacterSettings),
            },
            command(character::HIDE, "Hide character", None),
        ]
    }

    fn ui_message(&mut self, ui: UiMsg, now: Millis, out: &mut Outcomes) -> Task<Message> {
        if matches!(
            &ui,
            UiMsg::ArtOnHead(..)
                | UiMsg::ArtBehind(..)
                | UiMsg::ArtRemoveFace(..)
                | UiMsg::ArtRemoveEye(..)
                | UiMsg::ArtRemoveBlink(..)
                | UiMsg::ArtTravel(..)
        ) && let Some(e) = &mut self.art
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
            UiMsg::CharacterTab(tab) => self.character_tab = tab,
            UiMsg::Nav(page) => {
                if page != Page::Character && self.character_unsaved() {
                    self.leaving = Some(view::settings::Leave::Page(page));
                } else {
                    self.go_page(page);
                }
            }
            UiMsg::PickCharacter(id) => {
                if id != self.current_character().id && self.character_unsaved() {
                    self.leaving = Some(view::settings::Leave::Character(id));
                } else {
                    self.pick_character(id, out);
                }
            }
            UiMsg::Leave(choice) => {
                use view::settings::LeaveChoice;
                let Some(to) = self.leaving.take() else {
                    return Task::none();
                };
                match choice {
                    LeaveChoice::Stay => {}
                    LeaveChoice::Discard => {
                        self.discard_character();
                        return self.finish_leave(to, out);
                    }
                    LeaveChoice::Save if self.art.is_some() => {
                        self.after_save = Some(to);
                        return self.ui_message(UiMsg::ArtSave, now, out);
                    }
                    LeaveChoice::Save => match self.save_behavior() {
                        Ok(()) => return self.finish_leave(to, out),
                        Err(e) => self.notice = Some(format!("Could not save: {e}")),
                    },
                }
            }
            UiMsg::CharacterSave => {
                if self.art.is_some() {
                    return self.ui_message(UiMsg::ArtSave, now, out);
                }
                if let Err(e) = self.save_behavior() {
                    self.notice = Some(format!("Could not save: {e}"));
                }
            }
            UiMsg::CharacterDiscard => self.discard_character(),
            UiMsg::AddCharacter => {
                return Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose a transparent PNG of your character")
                        .add_filter("PNG image", &["png"])
                        .pick_file(),
                    |file| Message::CharacterPicked(file.map(|f| f.path().to_path_buf())),
                );
            }
            UiMsg::ImportCharacter => {
                return Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Import a character folder")
                        .pick_folder(),
                    |f| Message::CharacterFolderPicked(f.map(|f| f.path().to_path_buf())),
                );
            }
            UiMsg::ExportCharacter => {
                return Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose where to save the character folder")
                        .pick_folder(),
                    |f| Message::CharacterExportPicked(f.map(|f| f.path().to_path_buf())),
                );
            }
            UiMsg::RemoveCharacter => {
                if !self.character_jobs.is_empty() {
                    self.notice = Some(
                        "Character work is still finishing. Try Remove when it completes.".into(),
                    );
                    return Task::none();
                }
                let c = self.current_character().clone();
                match characters::remove(&c) {
                    Ok(()) => {
                        self.characters.retain(|x| x.id != c.id);
                        out.sets
                            .push((character::CHARACTER, Value::Choice(characters::BUILTIN)));
                        self.notice = None;
                    }
                    Err(e) => self.notice = Some(e),
                }
            }
            UiMsg::CharacterName(name) => {
                if let Some(d) = &mut self.character_draft {
                    d.name = name.chars().take(60).collect();
                }
            }
            UiMsg::CancelCharacter => {
                self.cancel_character_job();
                self.character_draft = None;
            }
            UiMsg::SaveCharacter => {
                if self.character_job.is_none()
                    && let Some(draft) = self.character_draft.clone()
                {
                    let id = self.begin_character_job();
                    let job = self.character_job.as_ref().unwrap().clone();
                    let taken: Vec<_> = self.characters.iter().map(|c| c.id).collect();
                    return character_work(id, move || {
                        characters::install(&draft, &taken, &job).map(|c| {
                            CharacterWork::Imported(characters::Imported::Installed(c), true)
                        })
                    });
                }
            }
            UiMsg::ResolveCharacter(keep_both) => {
                if self.character_conflict.take().is_some() {
                    return self.import_character(if keep_both {
                        characters::OnDuplicate::KeepBoth
                    } else {
                        characters::OnDuplicate::Replace
                    });
                }
            }
            UiMsg::MotionPreset(i, preset) => {
                if let Some(p) = &mut self.motion_preview
                    && let Some(spec) = p.specs.get(i).copied()
                {
                    p.set(i, crate::mascot::motion::MotionSpec { preset, ..spec });
                }
            }
            UiMsg::MotionStrength(i, strength) => {
                if let Some(p) = &mut self.motion_preview
                    && let Some(spec) = p.specs.get(i).copied()
                {
                    p.set(i, crate::mascot::motion::MotionSpec { strength, ..spec });
                }
            }
            UiMsg::MotionSpeed(i, speed) => {
                if let Some(p) = &mut self.motion_preview
                    && let Some(spec) = p.specs.get(i).copied()
                {
                    p.set(i, crate::mascot::motion::MotionSpec { speed, ..spec });
                }
            }
            UiMsg::MotionTest(test) => {
                if let Some(p) = &mut self.motion_preview {
                    p.test(test);
                }
                if let Some(sound) = self.motion_preview.as_mut().and_then(|p| p.take_sound())
                    && let Some(seconds) = self.play_line_sound(&sound)
                    && let Some(p) = &mut self.motion_preview
                {
                    p.hold_speech(seconds);
                }
            }
            UiMsg::MusicBop(bop) => {
                if let Some(p) = &mut self.motion_preview {
                    p.set_bop(bop);
                    // Show it straight away, even without music.
                    p.test(crate::mascot::preview::Test::Music);
                }
            }
            UiMsg::LineText(t, i, text) => {
                if let Some(line) = self
                    .motion_preview
                    .as_mut()
                    .and_then(|p| p.lines.get_mut(t).get_mut(i))
                {
                    line.text = text.chars().take(crate::mascot::lines::MAX_CHARS).collect();
                }
            }
            UiMsg::LineAdd(t) => {
                if let Some(p) = &mut self.motion_preview
                    && p.lines.total() < crate::mascot::lines::MAX_LINES
                {
                    p.lines.get_mut(t).push(crate::mascot::lines::Line {
                        text: String::new(),
                        sound: None,
                    });
                }
            }
            UiMsg::LineRemove(t, i) => {
                if let Some(p) = &mut self.motion_preview
                    && i < p.lines.get(t).len()
                {
                    p.lines.get_mut(t).remove(i);
                }
            }
            UiMsg::LineClearSound(t, i) => {
                if let Some(line) = self
                    .motion_preview
                    .as_mut()
                    .and_then(|p| p.lines.get_mut(t).get_mut(i))
                {
                    line.sound = None;
                }
            }
            UiMsg::LineSound(t, i) => {
                return Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose a WAV for this line")
                        .add_filter("WAV sound", &["wav"])
                        .pick_file(),
                    move |f| Message::LineSoundPicked(t, i, f.map(|f| f.path().to_path_buf())),
                );
            }
            UiMsg::BubbleBackground(c) => {
                if let Some(p) = &mut self.motion_preview {
                    p.lines.bubble.background = c;
                }
            }
            UiMsg::BubbleText(c) => {
                if let Some(p) = &mut self.motion_preview {
                    p.lines.bubble.text = c;
                }
            }
            UiMsg::BubbleWidth(w) => {
                if let Some(p) = &mut self.motion_preview {
                    p.lines.bubble.max_width = w.clamp(
                        crate::mascot::lines::MIN_WIDTH,
                        crate::mascot::lines::MAX_WIDTH,
                    );
                }
            }
            UiMsg::LinesReset => {
                if let Some(p) = &mut self.motion_preview {
                    p.reset_lines();
                }
            }
            UiMsg::MotionReset => {
                if let Some(p) = &mut self.motion_preview {
                    p.reset();
                }
            }
            UiMsg::CancelCharacterImport => {
                self.cancel_character_job();
                self.character_conflict = None;
                self.prepared_character = None;
            }
            UiMsg::EditArt => {
                let mut c = self.current_character().clone();
                // Bundled Yohta stays built in: edit an installed copy of him.
                if c.is_builtin() {
                    let taken: Vec<&str> = self.characters.iter().map(|c| c.id).collect();
                    match characters::copy_builtin(&taken) {
                        Ok(copy) => {
                            out.sets.push((character::CHARACTER, Value::Choice(copy.id)));
                            self.add_character(copy.clone());
                            c = copy;
                        }
                        Err(e) => {
                            self.notice = Some(format!("Could not copy Yohta to edit: {e}"));
                            return Task::none();
                        }
                    }
                }
                let id = self.begin_character_job();
                self.character_draft = None;
                return character_work(id, move || {
                    crate::mascot::editor::Editor::open(&c)
                        .map(|e| CharacterWork::Editor(Box::new(e), Vec::new()))
                });
            }
            UiMsg::ArtFromParts => {
                self.cancel_character_job();
                let generation = self.character_request;
                return Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose the body PNG, then add its separate parts")
                        .add_filter("PNG image", &["png"])
                        .pick_file(),
                    move |f| Message::ArtBodyPicked(generation, f.map(|f| f.path().to_path_buf())),
                );
            }
            UiMsg::ArtPick(target) => {
                use crate::mascot::editor::{EyeSlot, FACE_NAMES, Target};
                let title = match target {
                    Target::Body => "Choose the body PNG".to_owned(),
                    Target::NewPart => {
                        "Choose a part PNG: head, tail, ear, wing or hair".to_owned()
                    }
                    Target::Face(_, slot) => {
                        format!(
                            "Choose the {} face PNG",
                            FACE_NAMES[slot.min(2)].to_lowercase()
                        )
                    }
                    Target::Eye(_, slot) => format!(
                        "Choose the eye {} PNG",
                        match slot {
                            EyeSlot::White => "white",
                            EyeSlot::Pupil => "pupil",
                            EyeSlot::Mask => "mask",
                            EyeSlot::Blink => "blink",
                        }
                    ),
                };
                let generation = self.character_request;
                return Task::perform(
                    async move {
                        let picker = rfd::AsyncFileDialog::new()
                            .set_title(title)
                            .add_filter("PNG image", &["png"]);
                        if target == Target::NewPart {
                            picker
                                .pick_files()
                                .await
                                .unwrap_or_default()
                                .into_iter()
                                .map(|f| f.path().to_path_buf())
                                .collect()
                        } else {
                            picker
                                .pick_file()
                                .await
                                .into_iter()
                                .map(|f| f.path().to_path_buf())
                                .collect()
                        }
                    },
                    move |paths| Message::ArtFilesPicked(generation, target, paths),
                );
            }
            UiMsg::ArtSelect(s) => {
                if let Some(e) = &mut self.art {
                    e.select(s);
                }
            }
            UiMsg::ArtKind(i, kind) => {
                if let Some(e) = &mut self.art {
                    e.checkpoint();
                    e.set_kind(i, kind);
                }
            }
            UiMsg::ArtOnHead(i, on) => {
                if let Some(p) = self.art.as_mut().and_then(|e| e.parts.get_mut(i)) {
                    p.on_head = on;
                }
            }
            UiMsg::ArtBehind(i, behind) => {
                if let Some(p) = self.art.as_mut().and_then(|e| e.parts.get_mut(i)) {
                    p.behind = behind;
                }
            }
            UiMsg::ArtRemovePart(i) => {
                if let Some(e) = &mut self.art {
                    e.checkpoint();
                    e.remove_part(i);
                }
            }
            UiMsg::ArtRemoveFace(i, slot) => {
                if let Some(p) = self.art.as_mut().and_then(|e| e.parts.get_mut(i)) {
                    p.faces[slot.min(2)] = None;
                }
            }
            UiMsg::ArtStaticEye(i) => {
                if let Some(e) = &mut self.art {
                    self.notice = e.keep_eye_static(i).err();
                }
            }
            UiMsg::ArtAddEye => {
                if let Some(e) = &mut self.art {
                    e.checkpoint();
                    e.add_eye();
                }
            }
            UiMsg::ArtRemoveEye(i) => {
                if let Some(e) = &mut self.art
                    && i < e.eyes.len()
                {
                    e.eyes.remove(i);
                }
            }
            UiMsg::ArtRemoveBlink(i) => {
                if let Some(eye) = self.art.as_mut().and_then(|e| e.eyes.get_mut(i)) {
                    eye.blink = None;
                }
            }
            UiMsg::ArtTravel(i, travel) => {
                if let Some(eye) = self.art.as_mut().and_then(|e| e.eyes.get_mut(i)) {
                    eye.travel = travel.clamp(0.0, 0.05);
                }
            }
            UiMsg::ArtClick(x, y) => {
                if let Some(e) = &mut self.art {
                    e.checkpoint();
                    e.click(x, y);
                }
            }
            UiMsg::ArtName(name) => {
                if let Some(e) = &mut self.art {
                    e.name = name.chars().take(60).collect();
                }
            }
            UiMsg::ArtCancel => self.discard_character(),
            UiMsg::ArtSave => {
                if self.character_job.is_none()
                    && let Some(editor) = self.art.clone()
                {
                    self.art_saving = Some(editor.id);
                    let id = self.begin_character_job();
                    let job = self.character_job.as_ref().unwrap().clone();
                    return character_work(id, move || {
                        editor.save(&job).map(|c| {
                            CharacterWork::Imported(
                                characters::Imported::Installed(c),
                                editor.new_install,
                            )
                        })
                    });
                }
            }
            UiMsg::ArtMode(mode) => {
                if let Some(e) = &mut self.art {
                    e.mode = mode;
                    e.finish_rest();
                }
            }
            UiMsg::ArtRest(action) => {
                if let Some(e) = &mut self.art {
                    use crate::mascot::editor::RestAction;
                    match action {
                        RestAction::Adjust => e.adjust_rest(),
                        RestAction::Done => e.finish_rest(),
                        RestAction::Reset => e.reset_rest(),
                    }
                }
            }
            UiMsg::ArtField(i, text) => {
                if let Some(e) = &mut self.art {
                    e.field(i, text);
                }
            }
            UiMsg::ArtCanvas(i, text) => {
                if let Some(e) = &mut self.art {
                    e.resize_canvas(i, text);
                }
            }
            UiMsg::ArtDragStart(selected) => {
                if let Some(e) = &mut self.art {
                    e.select(selected);
                    e.checkpoint();
                }
            }
            UiMsg::ArtDrag(dx, dy) => {
                if let Some(e) = &mut self.art {
                    e.translate(dx, dy);
                }
            }
            UiMsg::ArtNudge(dx, dy) => {
                if let Some(e) = &mut self.art {
                    e.checkpoint();
                    e.nudge(dx, dy);
                }
            }
            UiMsg::ArtOrder(i, up) => {
                if let Some(e) = &mut self.art {
                    e.reorder(i, up);
                }
            }
            UiMsg::ArtUndo => {
                if let Some(e) = &mut self.art {
                    e.undo();
                }
            }
            UiMsg::ArtRedo => {
                if let Some(e) = &mut self.art {
                    e.redo();
                }
            }
            UiMsg::ArtZoom(zoom) => {
                if let Some(e) = &mut self.art {
                    e.zoom = zoom.clamp(0.5, 3.0);
                }
            }
            UiMsg::ArtBackground(bg) => {
                if let Some(e) = &mut self.art {
                    e.background = bg;
                }
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
        }
        Task::none()
    }

    fn pointer(
        &mut self,
        output: OutputId,
        kind: PointerKind,
        p: Point,
        now: Millis,
        out: &mut Outcomes,
    ) {
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
                output,
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
            self.run(wheel::ITEMS[i].command, now, out);
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
        if matches!(id, character::CHARACTER | character::ENABLED) && self.character_job.is_some() {
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
        if !self.character_jobs.is_empty() {
            self.quit_pending = true;
            for job in self.character_jobs.values() {
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

    fn overlay_shortcut(
        &self,
        key: &keyboard::Key,
        m: keyboard::Modifiers,
    ) -> Option<&'static str> {
        let pressed = match key.as_ref() {
            keyboard::Key::Named(Named::Escape) => Key::Escape,
            keyboard::Key::Named(Named::Space) => Key::Space,
            keyboard::Key::Character(c) => Key::Char(c.chars().next()?.to_ascii_uppercase()),
            _ => return None,
        };
        self.registry
            .shortcuts(ShortcutContext::Overlay)
            .find(|(s, _)| {
                s.key == pressed
                    && s.ctrl == m.control()
                    && s.alt == m.alt()
                    && s.shift == m.shift()
            })
            .map(|(_, def)| def.id)
    }

    fn sync_tray(&mut self) {
        let label = self.registry.shortcut_label(system::MASTER_TOGGLE);
        if let Some(tray) = &mut self.tray {
            tray.sync(&self.registry, &self.session, label.as_deref());
        }
    }

    /// Brings the overlay window in line with the session's overlay state.
    fn reconcile(&mut self) -> Task<Message> {
        let want = self.session.overlay_state();
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

    fn update_lens(&mut self, now: Millis, out: &mut Outcomes) {
        // The still covers the desktop while frozen, and capture must not
        // see the lens, so neither lens shows then.
        let clear = !self.session.capturing() && self.session.frozen_frame().is_none();
        let tool = self.session.is_pointing()
            && self.session.active_tool() == crate::features::upcoming::TOOL
            && clear;
        let anywhere = self.lens_anywhere && clear;
        #[cfg(windows)]
        {
            use crate::features::upcoming;
            let s = self.session.settings();
            let source = s.choice_text(upcoming::SOURCE);
            let source_window = if matches!(source, upcoming::FOREGROUND | upcoming::DESKTOP) {
                self.previous_focus
            } else {
                self.sources
                    .iter()
                    .find(|w| w.id == source)
                    .map(|w| w.handle)
            };
            let output = self.session.pointer().map(|(o, _)| o);
            let destination = self
                .overlays
                .iter()
                .find(|o| Some(o.display.id) == output && o.hwnd != 0)
                .map(|o| (o.hwnd, &o.display));
            let mut exclude: Vec<_> = self
                .overlays
                .iter()
                .map(|o| o.hwnd)
                .filter(|h| *h != 0)
                .collect();
            if let Some(m) = &self.mascot {
                exclude.push(m.hwnd());
            }
            exclude.extend(
                self.helper_hwnds
                    .iter()
                    .filter(|(id, hwnd)| {
                        **hwnd != 0
                            && (Some(**id) == self.settings_window
                                || Some(**id) == self.first_run_window)
                    })
                    .map(|(_, hwnd)| *hwnd),
            );
            let result = self.lens.update(platform::lens::Input {
                tool,
                anywhere,
                source,
                source_window,
                destination,
                exclude: &exclude,
                size: s.float(upcoming::SIZE) as f32,
                zoom: if s.choice(upcoming::ZOOM) == "4" {
                    4.0
                } else {
                    2.0
                },
            });
            if let Some(notice) = result.notice {
                self.overlay_notice = Some((notice.clone(), now));
                if result.stop_anywhere {
                    self.notice = Some(notice);
                }
            }
            if result.stop_anywhere {
                self.lens_anywhere = false;
            }
            if result.return_to_point {
                out.collect(self.dispatch(point::SELECT, now).ok());
            }
        }
        #[cfg(not(windows))]
        {
            if self.lens_anywhere {
                self.lens_anywhere = false;
                self.notice = Some("Lens is available on Windows".into());
            }
            if tool {
                self.overlay_notice = Some(("Lens is unavailable on this desktop".into(), now));
                out.collect(self.dispatch(point::SELECT, now).ok());
            }
        }
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
            });
        }
        if Some(window) == self.settings_window {
            return view::settings::view(view::settings::SettingsView {
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
                character_draft: self.character_draft.as_ref(),
                character_busy: self.character_job.is_some(),
                character_conflict: self.character_conflict.as_deref(),
                art: self.art.as_ref(),
                motion: self.motion_preview.as_ref(),
                unsaved: self.character_unsaved(),
                leaving: self.leaving.is_some(),
                music: self.music_status(),
                music_source: self.music_source(),
                music_rows: self.music_rows(),
                music_inputs: self.music_inputs(),
                beat: self.beat_status(),
            });
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
        #[cfg(windows)]
        if let Some(every) = self
            .mascot
            .as_ref()
            .and_then(|m| m.poll_interval(&self.mascot_options()))
        {
            subs.push(iced::time::every(every).map(|_| Message::Mascot(crate::mascot::Msg::Poll)));
        }
        // The Motion tab's preview animates while it's open.
        if self.motion_preview.is_some() {
            subs.push(iced::time::every(Duration::from_millis(33)).map(|_| Message::MotionTick));
        }
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
