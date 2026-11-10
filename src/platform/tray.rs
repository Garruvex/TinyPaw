//! Tray menu built from commands registered for the Tray surface.

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use super::External;
use crate::features::{mascot, system};
use crate::registry::{Registry, Surfaces};
use crate::session::Session;

pub struct Tray {
    icon: TrayIcon,
    master: Option<CheckMenuItem>,
    /// "Show character", checked while the desktop character is enabled.
    character: Option<CheckMenuItem>,
    items: Vec<(&'static str, MenuItem)>,
    on_icon: Icon,
    off_icon: Icon,
    shown: Option<bool>,
}

pub fn build(registry: &Registry) -> Result<Tray, String> {
    let menu = Menu::new();
    let mut master = None;
    let mut character = None;
    let mut items = Vec::new();
    let mut last_group = None;
    for def in registry.commands_for(Surfaces::TRAY) {
        if last_group.is_some_and(|g| g != def.group) {
            menu.append(&PredefinedMenuItem::separator())
                .map_err(|e| e.to_string())?;
        }
        last_group = Some(def.group);
        if def.id == system::MASTER_TOGGLE {
            let item = CheckMenuItem::with_id(def.id, def.label, true, false, None);
            menu.append(&item).map_err(|e| e.to_string())?;
            master = Some(item);
        } else if def.id == mascot::TOGGLE {
            let item = CheckMenuItem::with_id(def.id, def.label, true, false, None);
            menu.append(&item).map_err(|e| e.to_string())?;
            character = Some(item);
        } else {
            let item = MenuItem::with_id(def.id, def.label, true, None);
            menu.append(&item).map_err(|e| e.to_string())?;
            items.push((def.id, item));
        }
    }

    MenuEvent::set_event_handler(Some(|e: MenuEvent| {
        super::send(External::Menu(e.id.0));
    }));
    TrayIconEvent::set_event_handler(Some(|e: TrayIconEvent| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = e
        {
            super::send(External::TrayClick);
        }
    }));

    let off_icon = paw_icon(false)?;
    let on_icon = paw_icon(true)?;
    let icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .with_icon(off_icon.clone())
        .with_tooltip("TinyPaw: annotation off")
        .build()
        .map_err(|e| e.to_string())?;
    Ok(Tray {
        icon,
        master,
        character,
        items,
        on_icon,
        off_icon,
        shown: None,
    })
}

impl Tray {
    /// Mirrors the session's On/Off state and command availability.
    pub fn sync(&mut self, registry: &Registry, session: &Session, shortcut: Option<&str>) {
        let on = session.is_enabled();
        if let Some(master) = &self.master {
            // The menu toggles the check itself on click; always restore truth.
            master.set_checked(on);
        }
        if let Some(item) = &self.character {
            item.set_checked(session.settings().bool(mascot::ENABLED));
        }
        for (id, item) in &self.items {
            if let Some(def) = registry.command(id) {
                item.set_enabled((def.available)(session).is_ok());
            }
        }
        if self.shown == Some(on) {
            return;
        }
        self.shown = Some(on);
        let state = if on {
            "annotation on"
        } else {
            "annotation off"
        };
        let tip = match shortcut {
            Some(s) => format!("TinyPaw: {state} ({s})"),
            None => format!("TinyPaw: {state}"),
        };
        let _ = self.icon.set_tooltip(Some(tip));
        let _ = self.icon.set_icon(Some(if on {
            self.on_icon.clone()
        } else {
            self.off_icon.clone()
        }));
    }
}

/// A 32×32 placeholder paw print: warm when On, grey when Off.
fn paw_icon(on: bool) -> Result<Icon, String> {
    const N: usize = 32;
    let color: [u8; 3] = if on {
        [0xF2, 0x8C, 0x28]
    } else {
        [0x9A, 0x9A, 0x9A]
    };
    let blobs: [(f32, f32, f32); 5] = [
        (16.0, 21.0, 8.0),
        (7.0, 12.0, 3.6),
        (12.5, 7.0, 3.6),
        (19.5, 7.0, 3.6),
        (25.0, 12.0, 3.6),
    ];
    let mut rgba = vec![0u8; N * N * 4];
    for y in 0..N {
        for x in 0..N {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let coverage = blobs
                .iter()
                .map(|&(cx, cy, r)| {
                    (r + 0.5 - ((px - cx).powi(2) + (py - cy).powi(2)).sqrt()).clamp(0.0, 1.0)
                })
                .fold(0.0f32, f32::max);
            let i = (y * N + x) * 4;
            rgba[i..i + 3].copy_from_slice(&color);
            rgba[i + 3] = (coverage * 255.0) as u8;
        }
    }
    Icon::from_rgba(rgba, N as u32, N as u32).map_err(|e| e.to_string())
}
