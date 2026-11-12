//! Tray menu built from commands registered for the Tray surface.
//! Not built on Linux yet: tray-icon needs GTK there. A StatusNotifierItem
//! tray (for Waybar and similar) is a follow-up; `tinypaw settings` and the
//! compositor binding cover the same commands meanwhile.

#[cfg(not(target_os = "linux"))]
pub use native::*;

#[cfg(target_os = "linux")]
pub use none::*;

#[cfg(target_os = "linux")]
mod none {
    use crate::registry::Registry;
    use crate::session::Session;

    pub struct Tray;

    pub fn build(_registry: &Registry) -> Result<Tray, String> {
        Err("no tray on Wayland yet; use `tinypaw settings`".into())
    }

    impl Tray {
        pub fn sync(&mut self, _registry: &Registry, _session: &Session, _shortcut: Option<&str>) {}
    }
}

#[cfg(not(target_os = "linux"))]
mod native {
    use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{
        Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    };

    use super::super::External;
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

        let off_icon = crate::branding::tray_icon(false)?;
        let on_icon = crate::branding::tray_icon(true)?;
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
                    item.set_enabled(registry.availability(def.id, session).is_ok());
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
}
