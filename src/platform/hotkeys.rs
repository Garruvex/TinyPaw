//! Global shortcuts registered from the registry's Global-context bindings.

use std::collections::HashMap;
use std::str::FromStr;

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

use super::External;
use crate::registry::{Key, Registry, Shortcut, ShortcutContext};

pub struct Hotkeys {
    _manager: GlobalHotKeyManager,
    commands: HashMap<u32, &'static str>,
    /// Human-readable registration conflicts to report in the tray/settings.
    pub failures: Vec<String>,
    labels: HashMap<&'static str, String>,
}

impl Hotkeys {
    pub fn command(&self, id: u32) -> Option<&'static str> {
        self.commands.get(&id).copied()
    }

    pub fn label(&self, command: &str) -> Option<&str> {
        self.labels.get(command).map(String::as_str)
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
        Key::Escape => Code::Escape,
        Key::F(n) => Code::from_str(&format!("F{n}")).ok()?,
    };
    Some(HotKey::new((!mods.is_empty()).then_some(mods), code))
}

pub fn register(registry: &'static Registry) -> Result<Hotkeys, String> {
    let manager = GlobalHotKeyManager::new().map_err(|e| e.to_string())?;
    let mut commands = HashMap::new();
    let mut labels = HashMap::new();
    let mut failures = Vec::new();
    for (shortcut, def) in registry.shortcuts(ShortcutContext::Global) {
        let Some(hotkey) = to_hotkey(shortcut) else {
            failures.push(format!("{}: unsupported key", shortcut.label()));
            continue;
        };
        match manager.register(hotkey) {
            Ok(()) => {
                commands.insert(hotkey.id(), def.id);
                labels.insert(def.id, shortcut.label());
            }
            Err(e) => failures.push(format!("{} for {}: {e}", shortcut.label(), def.label)),
        }
    }
    GlobalHotKeyEvent::set_event_handler(Some(|e: GlobalHotKeyEvent| {
        super::send(External::Hotkey {
            id: e.id,
            pressed: e.state == HotKeyState::Pressed,
        });
    }));
    Ok(Hotkeys {
        _manager: manager,
        commands,
        failures,
        labels,
    })
}
