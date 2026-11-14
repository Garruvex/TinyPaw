//! The Settings process's link to the resident app. One thread owns the last
//! known settings, applies the user's changes at once so controls answer
//! without waiting, forwards them over the command endpoint, and asks for the
//! settings again a few times a second so changes made elsewhere (a shortcut,
//! the tray) show up.

use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::Duration;

use serde_json::Value as Json;

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

/// What the resident app reports.
pub struct State {
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
    Some(State {
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

fn snapshot(registry: &Registry, state: &State, page: usize) -> Snapshot {
    let current = page.min(pages::NAV.len() - 1);
    let id: PageId = pages::NAV[current].2;
    let cx = Context {
        registry,
        settings: &state.values,
        displays: &state.displays,
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
    let mut page = start_page;
    super::show(snapshot(&registry, &state, page), false);
    let mut failures = 0;
    loop {
        match rx.recv_timeout(REFRESH) {
            Ok(Cmd::Set(id, value)) => {
                // Show it now; the resident app confirms on the next refresh.
                let _ = registry.set(&mut state.values, id, value.clone());
                super::show(snapshot(&registry, &state, page), false);
                let _ = ipc::request(&format!("set {id} {}", value_text(&value)));
            }
            Ok(Cmd::Action(name)) => {
                let _ = ipc::request(&format!("settings-action {name}"));
            }
            Ok(Cmd::Navigate(index)) => {
                page = index;
                super::show(snapshot(&registry, &state, page), false);
                continue;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        match fetch() {
            Some(next) => {
                failures = 0;
                let focus = next.focus;
                state = next;
                super::show(snapshot(&registry, &state, page), focus);
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
