//! Optional desktop character: visibility, size, motion and saved position.
//! The desktop window itself lives in `crate::mascot::desktop`.

use crate::registry::{
    CommandDef, Constraint, Outcome, Registrar, SettingDef, Surfaces, Value, always,
};

pub const ENABLED: &str = "mascot.enabled";
/// The old canvas-based size, read once to migrate to `HEIGHT`.
pub const SIZE: &str = "mascot.size";
/// The character's visible resting height in logical pixels, excluding
/// transparent stage padding.
pub const HEIGHT: &str = "mascot.height";
/// `SIZE` has been converted to `HEIGHT`.
pub const HEIGHT_MIGRATED: &str = "mascot.height_migrated";
pub const EYES: &str = "mascot.eyes";
pub const IDLE: &str = "mascot.idle";
/// Speech lines: shown at all, and Quiet (no idle or music lines).
pub const LINES: &str = "mascot.lines";
pub const QUIET_LINES: &str = "mascot.quiet_lines";
/// Character sounds (line sounds), separate from the pointer's tap volume.
pub const VOICE_VOLUME: &str = "mascot.voice_volume";
/// Music response: move with playback, and how strongly.
pub const MUSIC: &str = "mascot.music";
pub const MUSIC_STRENGTH: &str = "mascot.music_strength";
/// The Follow beat trial: on/off, how readily it commits, and a timing nudge.
pub const BEAT: &str = "mascot.follow_beat";
/// What music response listens to: `all`, `app:<exe>` or `in:<device>`.
pub const MUSIC_SOURCE: &str = "mascot.music_source";
pub const BEAT_SENSITIVITY: &str = "mascot.beat_sensitivity";
pub const BEAT_TIMING: &str = "mascot.beat_timing";
/// Idle lines come at a random time within this many minutes.
pub const IDLE_MIN: &str = "mascot.idle_lines_min";
pub const IDLE_MAX: &str = "mascot.idle_lines_max";
/// Animation frame rate: `30`, `60`, or `display` (one frame per refresh).
pub const FRAME_RATE: &str = "mascot.frame_rate";
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
/// Bundled Yohta's visible art is 786 of his 860 canvas pixels, so these
/// give him the same smallest, default and largest size as 96, 160 and 320.
pub const MIN_HEIGHT: f64 = 88.0;
pub const DEFAULT_HEIGHT: f64 = 146.0;
pub const MAX_HEIGHT: f64 = 292.0;
/// Saved position meaning "never placed": no display reaches it, so the
/// character starts at the default spot on the primary display.
pub const UNPLACED: f64 = -1.0e7;

/// The animation timer period for a `FRAME_RATE` choice, or `None` to step
/// once per display refresh.
pub fn frame_period(choice: &str) -> Option<std::time::Duration> {
    match choice {
        "30" => Some(std::time::Duration::from_micros(33_333)),
        "60" => Some(std::time::Duration::from_micros(16_667)),
        _ => None,
    }
}

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
        label: "Character canvas size (before visible height)",
        group: "Character",
        order: 90,
        default: Value::Float(160.0),
        constraint: Constraint::Range {
            min: MIN_SIZE,
            max: MAX_SIZE,
        },
    });
    r.setting(SettingDef {
        id: HEIGHT,
        label: "Character size",
        group: "Character",
        order: 1,
        default: Value::Float(DEFAULT_HEIGHT),
        constraint: Constraint::Range {
            min: MIN_HEIGHT,
            max: MAX_HEIGHT,
        },
    });
    r.setting(SettingDef {
        id: HEIGHT_MIGRATED,
        label: "Character size uses visible height",
        group: "Character",
        order: 91,
        default: Value::Bool(false),
        constraint: Constraint::None,
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
            constraint: Constraint::Range {
                min: 1.0,
                max: 30.0,
            },
        });
    }
    r.setting(SettingDef {
        id: MUSIC,
        label: "React to playback",
        group: "Character",
        order: 9,
        default: Value::Bool(false),
        constraint: Constraint::None,
    });
    r.setting(SettingDef {
        id: MUSIC_STRENGTH,
        label: "Music strength",
        group: "Character",
        order: 10,
        default: Value::Float(1.0),
        constraint: Constraint::Range { min: 0.0, max: 2.0 },
    });
    r.setting(SettingDef {
        id: BEAT,
        label: "Follow beat (trial)",
        group: "Character",
        order: 11,
        default: Value::Bool(false),
        constraint: Constraint::None,
    });
    r.setting(SettingDef {
        id: BEAT_SENSITIVITY,
        label: "Beat sensitivity",
        group: "Character",
        order: 12,
        default: Value::Choice("normal"),
        constraint: Constraint::Choices(&["low", "normal", "high"]),
    });
    r.setting(SettingDef {
        id: BEAT_TIMING,
        label: "Beat timing",
        group: "Character",
        order: 13,
        default: Value::Choice("default"),
        constraint: Constraint::Choices(&["earlier", "default", "later"]),
    });
    r.setting(SettingDef {
        id: MUSIC_SOURCE,
        label: "Listen to",
        group: "Character",
        order: 14,
        default: Value::Choice("all"),
        constraint: Constraint::None,
    });
    r.setting(SettingDef {
        id: FRAME_RATE,
        label: "Animation frame rate",
        group: "Character",
        order: 15,
        default: Value::Choice("display"),
        constraint: Constraint::Choices(&["30", "60", "display"]),
    });
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
