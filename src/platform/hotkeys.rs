//! Global shortcuts registered from the registry's Global-context bindings.
//! On Linux the compositor owns global keys: a Niri binding runs
//! `tinypaw toggle`, so registration reports that instead of grabbing keys.

#[cfg(not(target_os = "linux"))]
pub use native::*;

#[cfg(target_os = "linux")]
pub use wayland::*;

#[cfg(target_os = "linux")]
mod wayland {
    use crate::registry::Registry;

    pub struct Hotkeys {
        pub failures: Vec<String>,
        pub taken: Vec<&'static str>,
    }

    impl Hotkeys {
        pub fn command(&self, _id: u32) -> Option<&'static str> {
            None
        }
    }

    pub fn register(_registry: &Registry) -> Result<Hotkeys, String> {
        Err("on Wayland, bind `tinypaw toggle` in your compositor (see docs/niri.md)".into())
    }
}

#[cfg(not(target_os = "linux"))]
mod native {
    use std::collections::HashMap;
    use std::str::FromStr;

    use global_hotkey::hotkey::{Code, HotKey, Modifiers};
    use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

    use super::super::External;
    use crate::registry::{Key, Registry, Shortcut, ShortcutContext};

    pub struct Hotkeys {
        _manager: GlobalHotKeyManager,
        commands: HashMap<u32, &'static str>,
        /// Human-readable registration conflicts to report in the tray/settings.
        pub failures: Vec<String>,
        /// Commands whose global shortcut another app already holds.
        pub taken: Vec<&'static str>,
    }

    impl Hotkeys {
        pub fn command(&self, id: u32) -> Option<&'static str> {
            self.commands.get(&id).copied()
        }
    }

    fn to_hotkey(s: &Shortcut) -> Option<HotKey> {
        let mut mods = Modifiers::empty();
        if s.ctrl {
            mods |= Modifiers::CONTROL;
        }
        if s.alt {
            mods |= Modifiers::ALT;
        }
        if s.shift {
            mods |= Modifiers::SHIFT;
        }
        let code = match s.key {
            Key::Char(c) if c.is_ascii_digit() => Code::from_str(&format!("Digit{c}")).ok()?,
            Key::Char(c) => Code::from_str(&format!("Key{c}")).ok()?,
            Key::Alt => return None,
            Key::Escape => Code::Escape,
            Key::Space => Code::Space,
            Key::F(n) => Code::from_str(&format!("F{n}")).ok()?,
        };
        Some(HotKey::new((!mods.is_empty()).then_some(mods), code))
    }

    pub fn register(registry: &Registry) -> Result<Hotkeys, String> {
        let manager = GlobalHotKeyManager::new().map_err(|e| e.to_string())?;
        let mut commands = HashMap::new();
        let mut failures = Vec::new();
        let mut taken = Vec::new();
        for (shortcut, def) in registry.shortcuts(ShortcutContext::Global) {
            if shortcut.key == Key::Alt {
                continue;
            }
            let Some(hotkey) = to_hotkey(shortcut) else {
                failures.push(format!("{}: unsupported key", shortcut.label()));
                continue;
            };
            match manager.register(hotkey) {
                Ok(()) => {
                    commands.insert(hotkey.id(), def.id);
                }
                Err(e) => {
                    failures.push(format!("{} for {}: {e}", shortcut.label(), def.label));
                    taken.push(def.id);
                }
            }
        }
        GlobalHotKeyEvent::set_event_handler(Some(|e: GlobalHotKeyEvent| {
            super::super::send(External::Hotkey {
                id: e.id,
                pressed: e.state == HotKeyState::Pressed,
            });
        }));
        Ok(Hotkeys {
            _manager: manager,
            commands,
            failures,
            taken,
        })
    }
}
