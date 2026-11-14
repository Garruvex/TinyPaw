//! The Settings window, in Slint (plan step 4). It runs as its own process,
//! `tinypaw --settings`, started on demand by the resident app: winit allows
//! one event loop per process and Iced already owns it there. The window asks
//! the resident app for the current settings over its command endpoint and
//! sends every change back the same way the CLI does, so each control saves at
//! once. Closing the window ends the process and frees its memory.

mod link;
mod model;
pub mod pages;

use std::cell::RefCell;
use std::collections::HashMap;
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::mpsc::Sender;

use slint::{CloseRequestResponse, ComponentHandle, Model, ModelRc, SharedString, VecModel};

pub use link::DisplayInfo;
pub use model::{Extra, Item, Snapshot};

use crate::registry::Value;

slint::include_modules!();

/// What the window asks its link to the resident app to do.
pub enum Cmd {
    Set(&'static str, Value),
    Action(String),
    Navigate(usize),
}

struct UiState {
    window: slint::Weak<SettingsWindow>,
    sections: Rc<VecModel<SectionModel>>,
    items: Vec<Rc<VecModel<ItemModel>>>,
    last: Option<Snapshot>,
    extras: Rc<RefCell<HashMap<String, Extra>>>,
}

thread_local! {
    static UI: RefCell<Option<UiState>> = const { RefCell::new(None) };
}

/// `tinypaw --settings`: shows the window until it is closed.
pub fn run(page: &str) -> ExitCode {
    let window = match SettingsWindow::new() {
        Ok(window) => window,
        Err(e) => {
            eprintln!("tinypaw: could not create the Settings window: {e}");
            return ExitCode::FAILURE;
        }
    };
    let extras: Rc<RefCell<HashMap<String, Extra>>> = Rc::default();
    let sections = Rc::new(VecModel::<SectionModel>::default());
    window.set_sections(ModelRc::from(sections.clone()));
    let cmds = link::start(pages::index_of(page));

    let send = |cmds: &Sender<Cmd>, cmd: Cmd| {
        let _ = cmds.send(cmd);
    };
    {
        let cmds = cmds.clone();
        window.on_toggled(move |id, on| {
            send(
                &cmds,
                Cmd::Set(crate::platform::intern(&id), Value::Bool(on)),
            )
        });
    }
    {
        let (cmds, extras) = (cmds.clone(), extras.clone());
        window.on_ranged(move |id, value| {
            let mut v = f64::from(value);
            if let Some(step) = extras.borrow().get(id.as_str()).and_then(|e| e.step) {
                v = (v / step).round() * step;
            }
            send(
                &cmds,
                Cmd::Set(crate::platform::intern(&id), Value::Float(v)),
            );
        });
    }
    {
        let (cmds, extras) = (cmds.clone(), extras.clone());
        window.on_chosen(move |id, index| {
            let value = extras
                .borrow()
                .get(id.as_str())
                .and_then(|e| e.choices.get(index as usize).cloned());
            if let Some(value) = value {
                send(&cmds, Cmd::Set(crate::platform::intern(&id), value));
            }
        });
    }
    {
        let cmds = cmds.clone();
        window.on_pressed(move |id| send(&cmds, Cmd::Action(id.to_string())));
    }
    {
        let cmds = cmds.clone();
        window.on_navigate_to(move |index| send(&cmds, Cmd::Navigate(index as usize)));
    }
    window.window().on_close_requested(|| {
        let _ = slint::quit_event_loop();
        CloseRequestResponse::HideWindow
    });

    UI.with(|ui| {
        *ui.borrow_mut() = Some(UiState {
            window: window.as_weak(),
            sections,
            items: Vec::new(),
            last: None,
            extras,
        });
    });
    if let Err(e) = window.show() {
        eprintln!("tinypaw: could not show the Settings window: {e}");
        return ExitCode::FAILURE;
    }
    match slint::run_event_loop() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("tinypaw: Settings window event loop: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Shows `snapshot`. Called on the window's thread.
fn apply(snapshot: Snapshot, focus: bool) {
    UI.with(|ui| {
        let mut ui = ui.borrow_mut();
        let Some(state) = ui.as_mut() else { return };
        let Some(window) = state.window.upgrade() else {
            return;
        };
        if focus {
            window.window().set_minimized(false);
            let _ = window.show();
        }
        if state.last.as_ref() == Some(&snapshot) {
            return;
        }
        let same_shape = state
            .last
            .as_ref()
            .is_some_and(|last| last.title == snapshot.title && shape(last) == shape(&snapshot));
        if state.last.as_ref().is_none_or(|l| l.nav != snapshot.nav) {
            window.set_nav_entries(ModelRc::new(VecModel::from(
                snapshot
                    .nav
                    .iter()
                    .map(|(group, name)| NavEntry {
                        group: group.as_str().into(),
                        name: name.as_str().into(),
                    })
                    .collect::<Vec<_>>(),
            )));
        }
        window.set_current_page(snapshot.current as i32);
        window.set_page_title(snapshot.title.as_str().into());
        window.set_page_summary(snapshot.summary.as_str().into());
        *state.extras.borrow_mut() = snapshot.extras.clone();
        if same_shape {
            let last = state
                .last
                .as_ref()
                .expect("same shape needs a last snapshot");
            for (si, section) in snapshot.sections.iter().enumerate() {
                for (ii, item) in section.items.iter().enumerate() {
                    if last.sections[si].items[ii] != *item {
                        state.items[si].set_row_data(ii, to_slint(item));
                    }
                }
            }
        } else {
            state.items.clear();
            let rows: Vec<SectionModel> = snapshot
                .sections
                .iter()
                .map(|section| {
                    let items = Rc::new(VecModel::from(
                        section.items.iter().map(to_slint).collect::<Vec<_>>(),
                    ));
                    state.items.push(items.clone());
                    SectionModel {
                        title: section.title.as_str().into(),
                        items: ModelRc::from(items),
                    }
                })
                .collect();
            state.sections.set_vec(rows);
        }
        state.last = Some(snapshot);
    });
}

/// Hands a snapshot to the window's thread.
fn show(snapshot: Snapshot, focus: bool) {
    let _ = slint::invoke_from_event_loop(move || apply(snapshot, focus));
}

/// Closes the window, which ends the process.
fn quit() {
    let _ = slint::invoke_from_event_loop(|| {
        let _ = slint::quit_event_loop();
    });
}

fn to_slint(item: &Item) -> ItemModel {
    ItemModel {
        id: item.id.as_str().into(),
        kind: item.kind as i32,
        label: item.label.as_str().into(),
        hint: item.hint.as_str().into(),
        on: item.on,
        value: item.value,
        min: item.min,
        max: item.max,
        text: item.text.as_str().into(),
        options: ModelRc::new(VecModel::from(
            item.options
                .iter()
                .map(|o| SharedString::from(o.as_str()))
                .collect::<Vec<_>>(),
        )),
        index: item.index,
        enabled: item.enabled,
        warning: item.warning,
    }
}

/// The page's shape: section titles and item IDs. While it stays the same,
/// items are updated in place so a control being dragged is not rebuilt.
fn shape(snapshot: &Snapshot) -> Vec<(&str, Vec<(&str, i32)>)> {
    snapshot
        .sections
        .iter()
        .map(|s| {
            (
                s.title.as_str(),
                s.items
                    .iter()
                    .map(|i| (i.id.as_str(), i.kind as i32))
                    .collect(),
            )
        })
        .collect()
}
