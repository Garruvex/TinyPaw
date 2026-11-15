//! The Settings process's link to the resident app. One thread owns the last
//! known settings, applies the user's changes at once so controls answer
//! without waiting, forwards them over the command endpoint, and asks for the
//! settings again a few times a second so changes made elsewhere (a shortcut,
//! the tray) show up.

use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::Duration;

use serde_json::Value as Json;

use super::model::Thumb;
use super::pages::{self, Context, PageId};
use super::{Cmd, Snapshot};
use crate::features;
use crate::platform::ipc;
use crate::registry::{Registry, SettingValues, Value};

/// The header's switch: turns annotation on or off.
pub const ANNOTATION: &str = "annotation.toggle";

/// How often the window asks the resident app for changes.
const REFRESH: Duration = Duration::from_millis(400);
/// Failed requests in a row before the window gives up and closes.
const GIVE_UP: u32 = 5;

/// A display the resident app knows about.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayInfo {
    pub name: String,
    pub text: String,
}

/// One shortcut the resident app has registered.
#[derive(Debug, Clone, PartialEq)]
pub struct ShortcutInfo {
    pub id: String,
    pub label: String,
    pub binding: String,
    /// `global` works everywhere; `overlay` only while drawing.
    pub global: bool,
    /// Another app already holds this key combination.
    pub taken: bool,
}

/// A window the lens can be locked to.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowInfo {
    pub id: String,
    pub title: String,
}

/// A character the resident app has loaded.
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterInfo {
    pub id: String,
    pub name: String,
    pub builtin: bool,
    pub thumb: Thumb,
}

/// A character on the desktop.
#[derive(Debug, Clone, PartialEq)]
pub struct InstanceInfo {
    pub id: u32,
    pub character: String,
    pub shown: bool,
    pub height: f64,
    pub eyes: bool,
    pub idle: bool,
}

/// What music response is listening to and hearing.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MusicInfo {
    pub status: String,
    pub level: f32,
    pub beat: String,
    /// Sources to listen to: ID, label, playing now.
    pub sources: Vec<(String, String, bool)>,
}

/// A pointer pack the resident app has loaded.
#[derive(Debug, Clone, PartialEq)]
pub struct PackInfo {
    pub id: String,
    pub name: String,
    pub accent: Option<[u8; 4]>,
    /// Imported by the user, so it can be re-aimed and removed.
    pub custom: bool,
    pub forearm: bool,
    pub thumb: Thumb,
}

/// What the resident app reports.
pub struct State {
    pub instances: Vec<InstanceInfo>,
    pub characters: Vec<CharacterInfo>,
    pub music: MusicInfo,
    pub packs: Vec<PackInfo>,
    pub windows: Vec<WindowInfo>,
    pub shortcuts: Vec<ShortcutInfo>,
    pub values: SettingValues,
    pub displays: Vec<DisplayInfo>,
    pub custom_sound: bool,
    /// Annotation is on.
    pub annotation: bool,
    /// The user asked for Settings again: bring the window forward.
    pub focus: bool,
}

/// Parses the resident app's `settings-state` reply.
pub fn parse_state(registry: &Registry, reply: &str) -> Option<State> {
    let body = reply.strip_prefix("ok: ")?;
    let json: Json = serde_json::from_str(body).ok()?;
    let mut values = registry.defaults();
    crate::preferences::apply_json(registry, &mut values, &json.get("values")?.to_string());
    let displays = json
        .get("displays")?
        .as_array()?
        .iter()
        .filter_map(|d| {
            Some(DisplayInfo {
                name: d.get("name")?.as_str()?.to_owned(),
                text: d.get("text")?.as_str()?.to_owned(),
            })
        })
        .collect();
    let shortcuts = json
        .get("shortcuts")
        .and_then(Json::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|s| {
                    Some(ShortcutInfo {
                        id: s.get("id")?.as_str()?.to_owned(),
                        label: s.get("label")?.as_str()?.to_owned(),
                        binding: s.get("binding")?.as_str()?.to_owned(),
                        global: s.get("context")?.as_str()? == "global",
                        taken: s.get("taken")?.as_bool()?,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let windows = json
        .get("windows")
        .and_then(Json::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|w| {
                    Some(WindowInfo {
                        id: w.get("id")?.as_str()?.to_owned(),
                        title: w.get("title")?.as_str()?.to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let packs = json
        .get("packs")
        .and_then(Json::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|p| {
                    let accent = p.get("accent").and_then(Json::as_str).and_then(|hex| {
                        let n = u32::from_str_radix(hex.trim_start_matches('#'), 16).ok()?;
                        Some([(n >> 16) as u8, (n >> 8) as u8, n as u8, 255])
                    });
                    Some(PackInfo {
                        id: p.get("id")?.as_str()?.to_owned(),
                        name: p.get("name")?.as_str()?.to_owned(),
                        accent,
                        custom: p.get("custom")?.as_bool()?,
                        forearm: p.get("forearm")?.as_bool()?,
                        thumb: Thumb::none(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let characters = json
        .get("characters")
        .and_then(Json::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|c| {
                    Some(CharacterInfo {
                        id: c.get("id")?.as_str()?.to_owned(),
                        name: c.get("name")?.as_str()?.to_owned(),
                        builtin: c.get("builtin")?.as_bool()?,
                        thumb: Thumb::none(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let music = json
        .get("music")
        .map(|m| {
            let list = |key: &str, playing: bool| -> Vec<(String, String, bool)> {
                m.get(key)
                    .and_then(Json::as_array)
                    .map(|l| {
                        l.iter()
                            .filter_map(|s| {
                                Some((
                                    s.get("id")?.as_str()?.to_owned(),
                                    s.get("label")?.as_str()?.to_owned(),
                                    s.get("playing").and_then(Json::as_bool).unwrap_or(playing),
                                ))
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            };
            let mut sources = list("rows", true);
            sources.extend(list("inputs", true));
            MusicInfo {
                status: m
                    .get("status")
                    .and_then(Json::as_str)
                    .unwrap_or("")
                    .to_owned(),
                level: m.get("level").and_then(Json::as_f64).unwrap_or(0.0) as f32,
                beat: m
                    .get("beat")
                    .and_then(Json::as_str)
                    .unwrap_or("")
                    .to_owned(),
                sources,
            }
        })
        .unwrap_or_default();
    let instances = json
        .get("instances")
        .and_then(Json::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|i| {
                    Some(InstanceInfo {
                        id: i.get("id")?.as_u64()? as u32,
                        character: i.get("character")?.as_str()?.to_owned(),
                        shown: i.get("shown")?.as_bool()?,
                        height: i.get("height")?.as_f64()?,
                        eyes: i.get("eyes")?.as_bool()?,
                        idle: i.get("idle")?.as_bool()?,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Some(State {
        instances,
        characters,
        music,
        packs,
        windows,
        shortcuts,
        values,
        displays,
        custom_sound: json.get("custom_sound")?.as_bool()?,
        annotation: json
            .get("annotation")
            .and_then(Json::as_bool)
            .unwrap_or(false),
        focus: json.get("focus").and_then(Json::as_bool).unwrap_or(false),
    })
}

/// The text a setting takes on the command line.
pub fn value_text(value: &Value) -> String {
    match value {
        Value::Bool(b) => b.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Color([r, g, b, a]) => format!("#{r:02x}{g:02x}{b:02x}{a:02x}"),
        Value::Choice(c) => (*c).to_owned(),
        Value::DynamicChoice(c) => c.clone(),
    }
}

/// One change on a page can be several settings: choosing a pack also means
/// drawing with the paw, and picking a forearm colour means not using the
/// automatic one.
fn expand(id: &'static str, value: Value) -> Vec<(&'static str, Value)> {
    use crate::features::point;
    match (id, value) {
        (pages::POINTER, Value::Choice("laser")) => {
            vec![(point::STYLE, Value::Choice("laser"))]
        }
        (pages::POINTER, pack) => vec![(point::STYLE, Value::Choice("paw")), (point::PACK, pack)],
        (point::FOREARM_COLOR, color) => vec![
            (point::FOREARM_COLOR, color),
            (point::FOREARM_AUTO_COLOR, Value::Bool(false)),
        ],
        (id, value) => vec![(id, value)],
    }
}

/// Card pictures of the packs, loaded once from disk the way the resident app
/// loads them, and again when a pack it reports is not among them.
#[derive(Default)]
struct Thumbs {
    packs: std::collections::HashMap<String, Thumb>,
    characters: std::collections::HashMap<String, Thumb>,
}

/// Fills in the pictures the page about to be shown needs.
fn prepare(state: &mut State, page: usize, thumbs: &mut Thumbs) {
    fill_thumbs(&mut state.packs, thumbs);
    if pages::NAV[page.min(pages::NAV.len() - 1)].2 == PageId::Characters {
        fill_character_thumbs(&mut state.characters, thumbs);
    }
}

fn fill_character_thumbs(characters: &mut [CharacterInfo], thumbs: &mut Thumbs) {
    if characters
        .iter()
        .any(|c| !thumbs.characters.contains_key(&c.id))
    {
        let (loaded, _) = crate::mascot::pack::load_all();
        for c in loaded {
            let thumb = c.thumbnail().as_ref().map_or_else(Thumb::none, thumb_from);
            thumbs.characters.insert(c.id.to_owned(), thumb);
        }
    }
    for c in characters {
        if let Some(thumb) = thumbs.characters.get(&c.id) {
            c.thumb = thumb.clone();
        }
    }
}

fn fill_thumbs(packs: &mut [PackInfo], thumbs: &mut Thumbs) {
    if packs.iter().any(|p| !thumbs.packs.contains_key(&p.id)) {
        let (loaded, _) = crate::packs::load_all();
        for pack in loaded {
            thumbs.packs.insert(pack.id.to_owned(), thumb_of(&pack));
        }
    }
    for pack in packs {
        if let Some(thumb) = thumbs.packs.get(&pack.id) {
            pack.thumb = thumb.clone();
        }
    }
}

/// The pack's picture at card size.
fn thumb_of(pack: &crate::packs::Pack) -> Thumb {
    thumb_from(&pack.image)
}

/// A picture at card size.
fn thumb_from(handle: &iced::advanced::image::Handle) -> Thumb {
    use iced::advanced::image::Handle;
    let Handle::Rgba {
        width,
        height,
        pixels,
        ..
    } = handle
    else {
        return Thumb::none();
    };
    let Some(source) = image::RgbaImage::from_raw(*width, *height, pixels.to_vec()) else {
        return Thumb::none();
    };
    let scale = (128.0 / *height as f32).min(1.0);
    let (w, h) = (
        ((*width as f32 * scale) as u32).max(1),
        ((*height as f32 * scale) as u32).max(1),
    );
    let small = image::imageops::resize(&source, w, h, image::imageops::FilterType::Triangle);
    Thumb {
        width: w,
        height: h,
        rgba: std::sync::Arc::new(small.into_raw()),
    }
}

/// What the window is showing that is not a setting: which character's row
/// is open, and whether the page is the picker for adding one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct View {
    pub expanded: Option<u32>,
    /// A page's fine tuning is unfolded.
    pub advanced: bool,
}

/// Opens or closes a character's row: `open:instance.<id>.shown`.
fn toggle_open(view: &mut View, name: &str) {
    let id = name
        .strip_prefix("open:instance.")
        .and_then(|rest| rest.split('.').next())
        .and_then(|n| n.parse::<u32>().ok());
    if let Some(id) = id {
        view.expanded = (view.expanded != Some(id)).then_some(id);
    }
}

fn snapshot(registry: &Registry, state: &State, page: usize, notice: &str, view: View) -> Snapshot {
    let current = page.min(pages::NAV.len() - 1);
    let id: PageId = pages::NAV[current].2;
    let cx = Context {
        registry,
        settings: &state.values,
        displays: &state.displays,
        shortcuts: &state.shortcuts,
        windows: &state.windows,
        packs: &state.packs,
        characters: &state.characters,
        instances: &state.instances,
        music: &state.music,
        has_custom_sound: state.custom_sound,
        expanded: view.expanded,
        advanced: view.advanced,
    };
    let built = pages::build(id, &cx);
    Snapshot {
        nav: pages::NAV
            .iter()
            .map(|(group, name, _)| ((*group).to_owned(), (*name).to_owned()))
            .collect(),
        current,
        annotation: state.annotation,
        title: built.title.to_owned(),
        summary: built.summary,
        notice: notice.to_owned(),
        sections: built.sections,
        extras: built.extras,
    }
}

/// Starts the link thread and returns where the window sends its requests.
pub fn start(page: usize) -> Sender<Cmd> {
    let (tx, rx) = channel();
    let _ = std::thread::Builder::new()
        .name("tinypaw-settings-link".into())
        .spawn(move || run(rx, page));
    tx
}

fn run(rx: Receiver<Cmd>, start_page: usize) {
    let registry = match Registry::build(features::BUILTINS) {
        Ok(r) => r,
        Err(_) => return super::quit(),
    };
    let fetch = || {
        ipc::request("settings-state")
            .ok()
            .and_then(|reply| parse_state(&registry, &reply))
    };
    let Some(mut state) = fetch() else {
        eprintln!("tinypaw: Settings needs the running TinyPaw. Start it with `tinypaw`.");
        return super::quit();
    };
    let mut thumbs = Thumbs::default();
    prepare(&mut state, start_page, &mut thumbs);
    let mut page = start_page;
    let mut notice = String::new();
    let mut view = View::default();
    // The characters on the desktop before one was added, to open the new row.
    let mut before_add: Option<Vec<u32>> = None;
    super::show(snapshot(&registry, &state, page, &notice, view), false);
    let mut failures = 0;
    loop {
        match rx.recv_timeout(REFRESH) {
            Ok(Cmd::Set(pages::ADD, Value::Choice(pages::NEW))) => {
                // The library's last card: make a new character in the studio.
                notice.clear();
                let _ = ipc::request("settings-action characters.edit");
            }
            Ok(Cmd::Action(name)) if name == ANNOTATION => {
                state.annotation = !state.annotation;
                super::show(snapshot(&registry, &state, page, &notice, view), false);
                let _ = ipc::request(&format!("settings-action {ANNOTATION}"));
            }
            Ok(Cmd::Set(pages::ADD, value)) => {
                // A library card: put that character on the desktop.
                before_add = Some(state.instances.iter().map(|i| i.id).collect());
                notice.clear();
                let _ = ipc::request(&format!(
                    "settings-action instance.add.{}",
                    value_text(&value)
                ));
            }
            Ok(Cmd::Action(name)) if name == pages::ADVANCED => {
                view.advanced = !view.advanced;
                super::show(snapshot(&registry, &state, page, &notice, view), false);
                continue;
            }
            Ok(Cmd::Action(name)) if name.starts_with("open:") => {
                toggle_open(&mut view, &name);
                super::show(snapshot(&registry, &state, page, &notice, view), false);
                continue;
            }
            Ok(Cmd::Set(id, value)) if id.starts_with("instance.") => {
                // `instance.<id>.<field>`: one setting of one character.
                let mut parts = id.splitn(3, '.');
                if let (Some(n), Some(field)) = (
                    parts.nth(1).and_then(|n| n.parse::<u32>().ok()),
                    parts.next(),
                ) {
                    let text = value_text(&value);
                    if let Some(i) = state.instances.iter_mut().find(|i| i.id == n) {
                        match (field, &value) {
                            ("shown", Value::Bool(b)) => i.shown = *b,
                            ("eyes", Value::Bool(b)) => i.eyes = *b,
                            ("idle", Value::Bool(b)) => i.idle = *b,
                            ("height", Value::Float(h)) => i.height = *h,
                            _ => {}
                        }
                    }
                    notice.clear();
                    super::show(snapshot(&registry, &state, page, &notice, view), false);
                    let _ = ipc::request(&format!("instance-set {n} {field} {text}"));
                }
            }
            Ok(Cmd::Set(id, value)) => {
                // Show it now; the resident app confirms on the next refresh.
                let sets = expand(id, value);
                for (id, value) in &sets {
                    let _ = registry.set(&mut state.values, id, value.clone());
                }
                notice.clear();
                super::show(snapshot(&registry, &state, page, &notice, view), false);
                for (id, value) in &sets {
                    let _ = ipc::request(&format!("set {id} {}", value_text(value)));
                }
            }
            Ok(Cmd::Action(name)) if name.starts_with("page:") => {
                page = pages::index_of(&name["page:".len()..]);
                prepare(&mut state, page, &mut thumbs);
                notice.clear();
                super::show(snapshot(&registry, &state, page, &notice, view), false);
                continue;
            }
            Ok(Cmd::Action(name)) => {
                notice.clear();
                let _ = ipc::request(&format!("settings-action {name}"));
            }
            Ok(Cmd::Bind(id, binding)) => {
                notice = match ipc::request(&format!("shortcut-bind {id} {binding}")) {
                    Ok(reply) => reply
                        .strip_prefix("error: ")
                        .map(str::to_owned)
                        .unwrap_or_default(),
                    Err(_) => "TinyPaw is not answering.".into(),
                };
            }
            Ok(Cmd::Navigate(index)) => {
                page = index;
                prepare(&mut state, page, &mut thumbs);
                notice.clear();
                super::show(snapshot(&registry, &state, page, &notice, view), false);
                continue;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        match fetch() {
            Some(mut next) => {
                failures = 0;
                let focus = next.focus;
                prepare(&mut next, page, &mut thumbs);
                state = next;
                if let Some(before) = before_add.take()
                    && let Some(new) = state.instances.iter().find(|i| !before.contains(&i.id))
                {
                    view.expanded = Some(new.id);
                }
                super::show(snapshot(&registry, &state, page, &notice, view), focus);
            }
            None => {
                failures += 1;
                if failures >= GIVE_UP {
                    return super::quit();
                }
            }
        }
    }
}
