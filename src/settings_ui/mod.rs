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
    /// Bind a command to a key combination, as `Ctrl+Alt+X` or `F5`.
    Bind(String, String),
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
    {
        let cmds = cmds.clone();
        window.on_annotation_toggled(move || send(&cmds, Cmd::Action(link::ANNOTATION.to_owned())));
    }
    {
        let (cmds, weak) = (cmds.clone(), window.as_weak());
        window.on_key_captured(move |id, text, ctrl, alt, shift| {
            let Some(window) = weak.upgrade() else { return };
            match capture(&text, ctrl, alt, shift) {
                // Still waiting: a modifier alone, or a key that means nothing.
                Capture::Keep => {}
                Capture::Cancel => window.set_capturing_id("".into()),
                Capture::Binding(binding) => {
                    window.set_capturing_id("".into());
                    send(&cmds, Cmd::Bind(id.to_string(), binding));
                }
            }
        });
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
    #[cfg(debug_assertions)]
    scripted_clicks(&window);
    match slint::run_event_loop() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("tinypaw: Settings window event loop: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Debug builds only: clicks listed in `TINYPAW_UI_CLICKS` ("x,y;x,y", in
/// the window's own logical pixels) are delivered to the window one every two
/// seconds, starting after eight. This lets a page be exercised and
/// photographed without moving the user's mouse.
#[cfg(debug_assertions)]
fn scripted_clicks(window: &SettingsWindow) {
    use slint::platform::{PointerEventButton, WindowEvent};
    let Ok(list) = std::env::var("TINYPAW_UI_CLICKS") else {
        return;
    };
    for (n, at) in list.split(';').enumerate() {
        let mut xy = at.split(',').filter_map(|v| v.trim().parse::<f32>().ok());
        let (Some(x), Some(y)) = (xy.next(), xy.next()) else {
            continue;
        };
        let weak = window.as_weak();
        slint::Timer::single_shot(
            std::time::Duration::from_millis(8000 + 2000 * n as u64),
            move || {
                let Some(window) = weak.upgrade() else { return };
                let position = slint::LogicalPosition::new(x, y);
                let button = PointerEventButton::Left;
                let w = window.window();
                w.dispatch_event(WindowEvent::PointerMoved { position });
                w.dispatch_event(WindowEvent::PointerPressed { position, button });
                w.dispatch_event(WindowEvent::PointerReleased { position, button });
            },
        );
    }
}

/// `tinypaw --first-run`: the card shown the first time TinyPaw starts. Its
/// buttons are answered by the running app.
pub fn first_run() -> ExitCode {
    use crate::features::system;
    use crate::platform::ipc;
    let window = match FirstRunWindow::new() {
        Ok(window) => window,
        Err(e) => {
            eprintln!("tinypaw: could not create the first-run window: {e}");
            return ExitCode::FAILURE;
        }
    };
    let registry = crate::registry::Registry::build(crate::features::BUILTINS).ok();
    let shortcuts = ipc::request("settings-state")
        .ok()
        .zip(registry)
        .and_then(|(reply, registry)| link::parse_state(&registry, &reply))
        .map(|state| state.shortcuts)
        .unwrap_or_default();
    let key = |id: &str| {
        shortcuts
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.binding.replace('+', " "))
    };
    let edit = match (key(system::UNDO), key(system::CLEAR)) {
        (Some(u), Some(c)) => format!("2.  {u} undoes, {c} clears."),
        (Some(u), None) => format!("2.  {u} undoes; Clear marks is under More."),
        _ => "2.  Undo and Clear marks are on the quick bar.".to_owned(),
    };
    let exit = match key(system::EXIT) {
        Some(e) => format!("3.  {e} hands the mouse back to your game."),
        None => "3.  End annotation hands the mouse back to your game.".to_owned(),
    };
    let tips: Vec<SharedString> = vec![
        "1.  Click to tap, drag to draw.".into(),
        edit.into(),
        exit.into(),
    ];
    window.set_tips(ModelRc::new(VecModel::from(tips)));
    window.set_shortcut(key(system::MASTER_TOGGLE).unwrap_or_default().into());
    let (w, h, rgba) = crate::branding::logo_rgba();
    let mut buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(w, h);
    buffer.make_mut_bytes().copy_from_slice(rgba);
    window.set_logo(slint::Image::from_rgba8(buffer));
    let answer = |action: &'static str| {
        move || {
            let _ = ipc::request(&format!("settings-action {action}"));
            let _ = slint::quit_event_loop();
        }
    };
    window.on_turn_on(answer("first-run.turn-on"));
    window.on_open_settings(answer("first-run.settings"));
    window.window().on_close_requested(|| {
        let _ = slint::quit_event_loop();
        CloseRequestResponse::HideWindow
    });
    if let Err(e) = window.show() {
        eprintln!("tinypaw: could not show the first-run window: {e}");
        return ExitCode::FAILURE;
    }
    match slint::run_event_loop() {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

enum Capture {
    Keep,
    Cancel,
    Binding(String),
}

/// What a key press means while a shortcut waits for a new combination.
fn capture(text: &str, ctrl: bool, alt: bool, shift: bool) -> Capture {
    use slint::platform::Key;
    let Some(key) = text.chars().next() else {
        return Capture::Keep;
    };
    let is = |k: Key| key == char::from(k);
    if is(Key::Escape) {
        return Capture::Cancel;
    }
    if is(Key::Alt) || is(Key::AltGr) {
        return Capture::Binding("Alt".into());
    }
    if is(Key::Control)
        || is(Key::ControlR)
        || is(Key::Shift)
        || is(Key::ShiftR)
        || is(Key::Meta)
        || is(Key::MetaR)
        || is(Key::CapsLock)
    {
        return Capture::Keep;
    }
    let function = [
        Key::F1,
        Key::F2,
        Key::F3,
        Key::F4,
        Key::F5,
        Key::F6,
        Key::F7,
        Key::F8,
        Key::F9,
        Key::F10,
        Key::F11,
        Key::F12,
        Key::F13,
        Key::F14,
        Key::F15,
        Key::F16,
        Key::F17,
        Key::F18,
        Key::F19,
        Key::F20,
        Key::F21,
        Key::F22,
        Key::F23,
        Key::F24,
    ]
    .iter()
    .position(|k| is(*k))
    .map(|n| format!("F{}", n + 1));
    let base = if let Some(name) = function {
        name
    } else if key == ' ' {
        "Space".into()
    } else if key.is_ascii_alphanumeric() {
        key.to_ascii_uppercase().to_string()
    } else if (key as u32) >= 1 && (key as u32) <= 26 && ctrl {
        // Some systems report Ctrl+A as a control character.
        char::from(b'A' + (key as u8 - 1)).to_string()
    } else {
        return Capture::Binding(String::new());
    };
    Capture::Binding(format!(
        "{}{}{}{}",
        if ctrl { "Ctrl+" } else { "" },
        if alt { "Alt+" } else { "" },
        if shift { "Shift+" } else { "" },
        base
    ))
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
                    .enumerate()
                    .map(|(i, (group, name))| NavEntry {
                        group: group.as_str().into(),
                        name: name.as_str().into(),
                        icon: pages::NAV.get(i).map_or("", |n| pages::icon(n.2)).into(),
                    })
                    .collect::<Vec<_>>(),
            )));
        }
        window.set_current_page(snapshot.current as i32);
        window.set_annotation_on(snapshot.annotation);
        window.set_page_title(snapshot.title.as_str().into());
        window.set_page_summary(snapshot.summary.as_str().into());
        let (id, label, on) = snapshot.switch.clone().unwrap_or_default();
        window.set_page_switch_id(id.into());
        window.set_page_switch_label(label.into());
        window.set_page_switch_on(on);
        window.set_notice(snapshot.notice.as_str().into());
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
                        tag: section.tag.as_str().into(),
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
        colors: ModelRc::new(VecModel::from(
            item.colors
                .iter()
                .map(|&[r, g, b, a]| slint::Color::from_argb_u8(a, r, g, b))
                .collect::<Vec<_>>(),
        )),
        pictures: ModelRc::new(VecModel::from(
            item.pictures
                .iter()
                .map(|t| {
                    if t.width == 0 {
                        return slint::Image::default();
                    }
                    let mut buffer =
                        slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(t.width, t.height);
                    buffer.make_mut_bytes().copy_from_slice(&t.rgba);
                    slint::Image::from_rgba8(buffer)
                })
                .collect::<Vec<_>>(),
        )),
        enabled: item.enabled,
        warning: item.warning,
        nested: item.nested,
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

#[cfg(test)]
mod tests {
    use super::*;
    use slint::platform::Key;

    fn binding(text: &str, ctrl: bool, alt: bool, shift: bool) -> Option<String> {
        match capture(text, ctrl, alt, shift) {
            Capture::Binding(b) => Some(b),
            _ => None,
        }
    }

    #[test]
    fn captured_keys_become_the_bindings_the_registry_accepts() {
        assert_eq!(
            binding("p", true, true, false).as_deref(),
            Some("Ctrl+Alt+P")
        );
        assert_eq!(binding("7", false, false, false).as_deref(), Some("7"));
        assert_eq!(
            binding(" ", false, false, true).as_deref(),
            Some("Shift+Space")
        );
        assert_eq!(
            binding(&char::from(Key::F5).to_string(), false, false, false).as_deref(),
            Some("F5")
        );
        assert_eq!(
            binding(&char::from(Key::F24).to_string(), true, false, false).as_deref(),
            Some("Ctrl+F24")
        );
        assert_eq!(
            binding(&char::from(Key::Alt).to_string(), false, true, false).as_deref(),
            Some("Alt")
        );
        // A control character with Ctrl held is that letter.
        assert_eq!(
            binding("\u{1}", true, false, false).as_deref(),
            Some("Ctrl+A")
        );
    }

    #[test]
    fn modifiers_alone_keep_waiting_and_escape_cancels() {
        for key in [Key::Control, Key::Shift, Key::Meta, Key::CapsLock] {
            assert!(matches!(
                capture(&char::from(key).to_string(), false, false, false),
                Capture::Keep
            ));
        }
        assert!(matches!(capture("", false, false, false), Capture::Keep));
        assert!(matches!(
            capture(&char::from(Key::Escape).to_string(), false, false, false),
            Capture::Cancel
        ));
    }
}
