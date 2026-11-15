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
    pub packs: Vec<PackInfo>,
    pub windows: Vec<WindowInfo>,
    pub shortcuts: Vec<ShortcutInfo>,
    pub values: SettingValues,
    pub displays: Vec<DisplayInfo>,
    pub custom_sound: bool,
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
    Some(State {
        packs,
        windows,
        shortcuts,
        values,
        displays,
        custom_sound: json.get("custom_sound")?.as_bool()?,
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
struct Thumbs(std::collections::HashMap<String, Thumb>);

fn fill_thumbs(packs: &mut [PackInfo], thumbs: &mut Thumbs) {
    if packs.iter().any(|p| !thumbs.0.contains_key(&p.id)) {
        let (loaded, _) = crate::packs::load_all();
        for pack in loaded {
            thumbs.0.insert(pack.id.to_owned(), thumb_of(&pack));
        }
    }
    for pack in packs {
        if let Some(thumb) = thumbs.0.get(&pack.id) {
            pack.thumb = thumb.clone();
        }
    }
}

/// The pack's picture at card size.
fn thumb_of(pack: &crate::packs::Pack) -> Thumb {
    use iced::advanced::image::Handle;
    let Handle::Rgba {
        width,
        height,
        pixels,
        ..
    } = &pack.image
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

fn snapshot(registry: &Registry, state: &State, page: usize, notice: &str) -> Snapshot {
    let current = page.min(pages::NAV.len() - 1);
    let id: PageId = pages::NAV[current].2;
    let cx = Context {
        registry,
        settings: &state.values,
        displays: &state.displays,
        shortcuts: &state.shortcuts,
        windows: &state.windows,
        packs: &state.packs,
        has_custom_sound: state.custom_sound,
    };
    let built = pages::build(id, &cx);
    Snapshot {
        nav: pages::NAV
            .iter()
            .map(|(group, name, _)| ((*group).to_owned(), (*name).to_owned()))
            .collect(),
        current,
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
    fill_thumbs(&mut state.packs, &mut thumbs);
    let mut page = start_page;
    let mut notice = String::new();
    super::show(snapshot(&registry, &state, page, &notice), false);
    let mut failures = 0;
    loop {
        match rx.recv_timeout(REFRESH) {
            Ok(Cmd::Set(id, value)) => {
                // Show it now; the resident app confirms on the next refresh.
                let sets = expand(id, value);
                for (id, value) in &sets {
                    let _ = registry.set(&mut state.values, id, value.clone());
                }
                notice.clear();
                super::show(snapshot(&registry, &state, page, &notice), false);
                for (id, value) in &sets {
                    let _ = ipc::request(&format!("set {id} {}", value_text(value)));
                }
            }
            Ok(Cmd::Action(name)) if name.starts_with("page:") => {
                page = pages::index_of(&name["page:".len()..]);
                notice.clear();
                super::show(snapshot(&registry, &state, page, &notice), false);
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
                notice.clear();
                super::show(snapshot(&registry, &state, page, &notice), false);
                continue;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        match fetch() {
            Some(mut next) => {
                failures = 0;
                let focus = next.focus;
                fill_thumbs(&mut next.packs, &mut thumbs);
                state = next;
                super::show(snapshot(&registry, &state, page, &notice), focus);
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
