//! Global shortcuts registered from the registry's Global-context bindings.

use std::collections::HashMap;
use std::str::FromStr;

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

use super::External;
use crate::features::anywhere::Adjust;
use crate::registry::{Key, Registry, Shortcut, ShortcutContext};

/// Extra keys registered only while a Lens or Spotlight anywhere shortcut is
/// held, so they never take Ctrl+Alt+arrows or +/- from other apps otherwise.
const ADJUST: &[(Code, Adjust)] = &[
    (Code::ArrowUp, Adjust::Bigger),
    (Code::ArrowDown, Adjust::Smaller),
    (Code::Equal, Adjust::More),
    (Code::NumpadAdd, Adjust::More),
    (Code::Minus, Adjust::Less),
    (Code::NumpadSubtract, Adjust::Less),
];

pub struct Hotkeys {
    manager: GlobalHotKeyManager,
    commands: HashMap<u32, &'static str>,
    /// Registered adjust keys while armed.
    adjust: Vec<(HotKey, Adjust)>,
    /// Plain Escape, registered only while Lens or Spotlight anywhere shows,
    /// so it turns them off from any app without taking Escape otherwise.
    dismiss: Option<HotKey>,
    /// Human-readable registration conflicts to report in the tray/settings.
    pub failures: Vec<String>,
    /// Commands whose global shortcut another app already holds.
    pub taken: Vec<&'static str>,
}

impl Hotkeys {
    pub fn command(&self, id: u32) -> Option<&'static str> {
        self.commands.get(&id).copied()
    }

    pub fn adjust(&self, id: u32) -> Option<Adjust> {
        self.adjust
            .iter()
            .find(|(h, _)| h.id() == id)
            .map(|(_, a)| *a)
    }

    /// The press is the Escape that hides Lens and Spotlight anywhere.
    pub fn is_dismiss(&self, id: u32) -> bool {
        self.dismiss.is_some_and(|h| h.id() == id)
    }

    /// Registers plain Escape or releases it. If another app holds it,
    /// Escape only works from TinyPaw's own windows.
    pub fn arm_dismiss(&mut self, on: bool) {
        if on == self.dismiss.is_some() {
            return;
        }
        if on {
            let hotkey = HotKey::new(None, Code::Escape);
            if self.manager.register(hotkey).is_ok() {
                self.dismiss = Some(hotkey);
            }
        } else if let Some(hotkey) = self.dismiss.take() {
            let _ = self.manager.unregister(hotkey);
        }
    }

    pub fn adjust_armed(&self) -> bool {
        !self.adjust.is_empty()
    }

    /// Registers the adjust keys (Ctrl+Alt with Up, Down, + and -) or
    /// releases them. A key another app holds is skipped.
    pub fn arm_adjust(&mut self, on: bool) {
        if on == self.adjust_armed() {
            return;
        }
        if on {
            for &(code, action) in ADJUST {
                let hotkey = HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), code);
                if self.manager.register(hotkey).is_ok() {
                    self.adjust.push((hotkey, action));
                }
            }
        } else {
            for (hotkey, _) in self.adjust.drain(..) {
                let _ = self.manager.unregister(hotkey);
            }
        }
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
        super::send(External::Hotkey {
            id: e.id,
            pressed: e.state == HotKeyState::Pressed,
        });
    }));
    Ok(Hotkeys {
        manager,
        commands,
        adjust: Vec::new(),
        dismiss: None,
        failures,
        taken,
    })
}
