//! System controls registered through the same contract as tools.

use crate::registry::{
    CommandDef, Constraint, Outcome, Registrar, SettingDef, ShellAction, ShortcutContext,
    Surfaces, Value, always, when_enabled,
};
use crate::session::Event;

pub const MASTER_TOGGLE: &str = "system.master_toggle";
pub const OFF: &str = "system.off";
pub const EXIT: &str = "system.exit";
pub const LATCH: &str = "system.latch";
pub const HOLD: &str = "system.hold";
pub const HOLD_END: &str = "system.hold_end";
pub const UNDO: &str = "system.undo";
pub const CLEAR: &str = "system.clear";
pub const QUIT: &str = "system.quit";
pub const SETTINGS: &str = "system.settings";

pub const ACTIVATION_STYLE: &str = "system.activation_style";
pub const INK_COLOR: &str = "ink.color";
pub const INK_WIDTH: &str = "ink.width";
pub const INK_FADE: &str = "ink.fade";

pub fn register(r: &mut Registrar) {
    r.command(CommandDef {
        id: MASTER_TOGGLE,
        label: "TinyPaw On",
        cli: Some("toggle"),
        surfaces: Surfaces::TRAY.with(Surfaces::CLI),
        group: "0-master",
        order: 0,
        shortcut: Some((ShortcutContext::Global, "Ctrl+Alt+P")),
        available: always,
        handler: |_| Outcome::Session(Event::MasterToggle),
        on_release: None,
    });
    r.command(CommandDef {
        id: OFF,
        label: "Off",
        cli: Some("off"),
        surfaces: Surfaces::CLI,
        group: "0-master",
        order: 1,
        shortcut: None,
        available: always,
        handler: |_| Outcome::Session(Event::MasterOff),
        on_release: None,
    });
    r.command(CommandDef {
        id: LATCH,
        label: "Annotate now",
        cli: Some("latch"),
        surfaces: Surfaces::TRAY.with(Surfaces::CLI),
        group: "1-session",
        order: 0,
        shortcut: None,
        available: when_enabled,
        handler: |_| Outcome::Session(Event::Latch),
        on_release: None,
    });
    r.command(CommandDef {
        id: HOLD,
        label: "Hold to draw",
        cli: Some("hold-begin"),
        surfaces: Surfaces::CLI,
        group: "1-session",
        order: 1,
        shortcut: Some((ShortcutContext::Global, "Ctrl+Alt+D")),
        // Key state is tracked even while Off so a key already held when
        // switching On needs a fresh press.
        available: always,
        handler: |_| Outcome::Session(Event::HoldDown),
        on_release: Some(|_| Outcome::Session(Event::HoldUp)),
    });
    r.command(CommandDef {
        id: HOLD_END,
        label: "End hold",
        cli: Some("hold-end"),
        surfaces: Surfaces::CLI,
        group: "1-session",
        order: 2,
        shortcut: None,
        available: always,
        handler: |_| Outcome::Session(Event::HoldUp),
        on_release: None,
    });
    r.command(CommandDef {
        id: UNDO,
        label: "Undo",
        cli: Some("undo"),
        surfaces: Surfaces::TOOLBAR.with(Surfaces::CLI),
        group: "2-edit",
        order: 0,
        shortcut: Some((ShortcutContext::Overlay, "Ctrl+Z")),
        available: when_enabled,
        handler: |_| Outcome::Session(Event::Undo),
        on_release: None,
    });
    r.command(CommandDef {
        id: CLEAR,
        label: "Clear",
        cli: Some("clear"),
        surfaces: Surfaces::TOOLBAR.with(Surfaces::CLI),
        group: "2-edit",
        order: 1,
        shortcut: Some((ShortcutContext::Overlay, "X")),
        available: when_enabled,
        handler: |_| Outcome::Session(Event::Clear),
        on_release: None,
    });
    r.command(CommandDef {
        id: EXIT,
        label: "Exit",
        cli: None,
        surfaces: Surfaces::TOOLBAR,
        group: "3-exit",
        order: 0,
        shortcut: Some((ShortcutContext::Overlay, "Escape")),
        available: when_enabled,
        handler: |_| Outcome::Session(Event::MasterOff),
        on_release: None,
    });
    r.command(CommandDef {
        id: SETTINGS,
        label: "Settings…",
        cli: Some("settings"),
        surfaces: Surfaces::TRAY.with(Surfaces::CLI),
        group: "8-app",
        order: 0,
        shortcut: None,
        available: always,
        handler: |_| Outcome::Shell(ShellAction::OpenSettings),
        on_release: None,
    });
    r.command(CommandDef {
        id: QUIT,
        label: "Quit TinyPaw",
        cli: Some("quit"),
        surfaces: Surfaces::TRAY.with(Surfaces::CLI),
        group: "9-quit",
        order: 0,
        shortcut: None,
        available: always,
        handler: |_| Outcome::Shell(ShellAction::Quit),
        on_release: None,
    });

    r.setting(SettingDef {
        id: ACTIVATION_STYLE,
        label: "Activation style",
        group: "Controls",
        order: 0,
        default: Value::Choice("toggle"),
        constraint: Constraint::Choices(&["toggle", "hold"]),
    });
    r.setting(SettingDef {
        id: INK_COLOR,
        label: "Ink color",
        group: "Appearance",
        order: 10,
        default: Value::Color([0x22, 0xD3, 0xEE, 0xFF]),
        constraint: Constraint::None,
    });
    r.setting(SettingDef {
        id: INK_WIDTH,
        label: "Ink width",
        group: "Appearance",
        order: 11,
        default: Value::Float(4.0),
        constraint: Constraint::Range { min: 1.0, max: 32.0 },
    });
    r.setting(SettingDef {
        id: INK_FADE,
        label: "Fade",
        group: "Appearance",
        order: 12,
        default: Value::Choice("keep"),
        constraint: Constraint::Choices(&["keep", "3s"]),
    });
}
