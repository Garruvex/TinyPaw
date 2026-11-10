//! Tap sound settings and the Mute control.

use crate::registry::{
    CommandDef, Constraint, Outcome, Registrar, SettingDef, ShortcutContext, Surfaces, Value,
    always,
};

pub const TAP: &str = "sound.tap";
pub const VOLUME: &str = "sound.volume";
pub const MUTE: &str = "sound.mute";
pub const LASER: &str = "sound.laser";
pub const TOGGLE_MUTE: &str = "sound.toggle_mute";

/// Stable sound IDs. `pack` follows the active pointer's own tap, falling back
/// to Blackboard Knock; `custom` is the imported WAV.
pub const CHOICES: &[&str] = &[
    "pack", "knock", "pop", "bonk", "duck-toy", "mac-quack", "bonk-clip", "dry-fart", "custom",
];

pub fn label(id: &str) -> &'static str {
    match id {
        "pack" => "Pack default (Blackboard Knock)",
        "knock" => "Blackboard Knock",
        "pop" => "Bubble Pop",
        "bonk" => "Rubber Bonk",
        "duck-toy" => "Duck Toy",
        "mac-quack" => "Mac Quack",
        "bonk-clip" => "Bonk",
        "dry-fart" => "Dry Fart",
        "custom" => "Imported WAV",
        _ => "Unknown",
    }
}

pub fn register(r: &mut Registrar) {
    r.setting(SettingDef {
        id: TAP,
        label: "Tap sound",
        group: "Sound",
        order: 0,
        default: Value::Choice("pack"),
        constraint: Constraint::Choices(CHOICES),
    });
    r.setting(SettingDef {
        id: VOLUME,
        label: "Volume",
        group: "Sound",
        order: 1,
        default: Value::Float(0.7),
        constraint: Constraint::Range { min: 0.0, max: 1.0 },
    });
    r.setting(SettingDef {
        id: MUTE,
        label: "Mute tap sound",
        group: "Sound",
        order: 2,
        default: Value::Bool(false),
        constraint: Constraint::None,
    });
    r.setting(SettingDef {
        id: LASER,
        label: "Play a sound when tapping with the laser",
        group: "Sound",
        order: 3,
        default: Value::Bool(false),
        constraint: Constraint::None,
    });
    r.command(CommandDef {
        id: TOGGLE_MUTE,
        label: "Mute",
        cli: Some("mute"),
        surfaces: Surfaces::TOOLBAR.with(Surfaces::CLI),
        group: "2-edit",
        order: 5,
        shortcut: Some((ShortcutContext::Overlay, "M")),
        available: always,
        handler: |s| Outcome::Set(MUTE, Value::Bool(!s.settings().bool(MUTE))),
        on_release: None,
    });
}
