//! Optional desktop character: visibility, size, motion and saved position.
//! The desktop window itself lives in `crate::mascot::desktop`.

use crate::registry::{
    CommandDef, Constraint, Outcome, Registrar, SettingDef, Surfaces, Value, always,
};

pub const ENABLED: &str = "mascot.enabled";
pub const SIZE: &str = "mascot.size";
pub const EYES: &str = "mascot.eyes";
pub const IDLE: &str = "mascot.idle";
/// Speech lines: shown at all, and Quiet (no idle or music lines).
pub const LINES: &str = "mascot.lines";
pub const QUIET_LINES: &str = "mascot.quiet_lines";
/// Character sounds (line sounds), separate from the pointer's tap volume.
pub const VOICE_VOLUME: &str = "mascot.voice_volume";
/// Idle lines come at a random time within this many minutes.
pub const IDLE_MIN: &str = "mascot.idle_lines_min";
pub const IDLE_MAX: &str = "mascot.idle_lines_max";
/// The selected character's ID: bundled `yohta` or an installed pack.
pub const CHARACTER: &str = "mascot.character";
/// Ground anchor (bottom centre of the character) in physical desktop pixels.
/// Internal: written when a drag ends, not shown in Settings.
pub const X: &str = "mascot.x";
pub const Y: &str = "mascot.y";

pub const TOGGLE: &str = "mascot.toggle";
pub const HIDE: &str = "mascot.hide";

pub const MIN_SIZE: f64 = 96.0;
pub const MAX_SIZE: f64 = 320.0;
/// Saved position meaning "never placed": no display reaches it, so the
/// character starts at the default spot on the primary display.
pub const UNPLACED: f64 = -1.0e7;

pub fn register(r: &mut Registrar) {
    r.setting(SettingDef {
        id: ENABLED,
        label: "Show character on the desktop",
        group: "Character",
        order: 0,
        default: Value::Bool(false),
        constraint: Constraint::None,
    });
    r.setting(SettingDef {
        id: CHARACTER,
        label: "Character",
        group: "Character",
        order: 0,
        default: Value::Choice("yohta"),
        // Installed IDs aren't known at registration.
        constraint: Constraint::None,
    });
    r.setting(SettingDef {
        id: SIZE,
        label: "Character size",
        group: "Character",
        order: 1,
        default: Value::Float(160.0),
        constraint: Constraint::Range {
            min: MIN_SIZE,
            max: MAX_SIZE,
        },
    });
    r.setting(SettingDef {
        id: EYES,
        label: "Eyes follow the cursor",
        group: "Character",
        order: 2,
        default: Value::Bool(true),
        constraint: Constraint::None,
    });
    r.setting(SettingDef {
        id: IDLE,
        label: "Idle motion and blinking",
        group: "Character",
        order: 3,
        default: Value::Bool(true),
        constraint: Constraint::None,
    });
    for (id, label, order, default) in [
        (LINES, "Show speech lines", 4, true),
        (QUIET_LINES, "Quiet: no idle or music lines", 5, false),
    ] {
        r.setting(SettingDef {
            id,
            label,
            group: "Character",
            order,
            default: Value::Bool(default),
            constraint: Constraint::None,
        });
    }
    r.setting(SettingDef {
        id: VOICE_VOLUME,
        label: "Character volume",
        group: "Character",
        order: 6,
        default: Value::Float(0.7),
        constraint: Constraint::Range { min: 0.0, max: 1.0 },
    });
    for (id, label, order, default) in [
        (IDLE_MIN, "Idle lines at least every (minutes)", 7, 1.0),
        (IDLE_MAX, "Idle lines at most every (minutes)", 8, 5.0),
    ] {
        r.setting(SettingDef {
            id,
            label,
            group: "Character",
            order,
            default: Value::Float(default),
            constraint: Constraint::Range { min: 1.0, max: 30.0 },
        });
    }
    for (id, order) in [(X, 90), (Y, 91)] {
        r.setting(SettingDef {
            id,
            label: "Character position",
            group: "Platform",
            order,
            default: Value::Float(UNPLACED),
            constraint: Constraint::Range {
                min: UNPLACED,
                max: 1.0e7,
            },
        });
    }
    r.command(CommandDef {
        id: TOGGLE,
        label: "Show character",
        cli: Some("mascot"),
        surfaces: Surfaces::TRAY.with(Surfaces::CLI),
        group: "7-character",
        order: 0,
        shortcut: None,
        available: always,
        handler: |s| Outcome::Set(ENABLED, Value::Bool(!s.settings().bool(ENABLED))),
        on_release: None,
    });
    r.command(CommandDef {
        id: HIDE,
        label: "Hide character",
        cli: Some("mascot-hide"),
        surfaces: Surfaces::CLI,
        group: "7-character",
        order: 1,
        shortcut: None,
        available: always,
        handler: |_| Outcome::Set(ENABLED, Value::Bool(false)),
        on_release: None,
    });
}
