//! Lens: magnifies the desktop under the pointer, or one locked window.
//! Either way TinyPaw's own surfaces stay out of the magnified image.
use crate::registry::{
    CommandDef, Constraint, Outcome, Registrar, SettingDef, ShellAction, ShortcutContext, Surfaces,
    ToolDef, Value, always, when_enabled,
};
use crate::session::Event;
pub const LENS: &str = "lens.select";
/// Shows or hides the desktop lens anywhere, with annotation on or off.
pub const ANYWHERE: &str = "lens.anywhere";
pub const TOOL: &str = "lens";
pub const SOURCE: &str = "lens.source";
pub const ZOOM: &str = "lens.zoom";
pub const SIZE: &str = "lens.size";

/// `SOURCE` values besides a window ID: the desktop under the pointer, or the
/// window that was active before TinyPaw.
pub const DESKTOP: &str = "desktop";
pub const FOREGROUND: &str = "foreground";
pub fn register(r: &mut Registrar) {
    r.tool(ToolDef {
        id: TOOL,
        label: "Lens",
        select_command: LENS,
        factory: crate::features::point::new_state,
    });
    r.command(CommandDef {
        id: LENS,
        label: "Lens",
        cli: Some("lens"),
        surfaces: Surfaces::PICKER.with(Surfaces::CLI),
        group: "tools",
        order: 5,
        shortcut: Some((ShortcutContext::Overlay, "L")),
        available: when_enabled,
        handler: |_| {
            Outcome::Session(Event::SelectTool {
                id: TOOL,
                factory: crate::features::point::new_state,
            })
        },
        on_release: None,
    });
    r.command(CommandDef {
        id: ANYWHERE,
        label: "Lens anywhere",
        cli: Some("lens-anywhere"),
        surfaces: Surfaces::CLI,
        group: "tools",
        order: 6,
        shortcut: Some((ShortcutContext::Global, "Ctrl+Alt+L")),
        available: always,
        handler: |_| Outcome::Shell(ShellAction::ToggleLens),
        on_release: None,
    });
    r.setting(SettingDef {
        id: SOURCE,
        label: "Lens source",
        group: "Appearance",
        order: 39,
        default: Value::Choice("foreground"),
        constraint: Constraint::None,
    });
    r.setting(SettingDef {
        id: ZOOM,
        label: "Lens magnification",
        group: "Appearance",
        order: 40,
        default: Value::Choice("2"),
        constraint: Constraint::Choices(&["2", "4"]),
    });
    r.setting(SettingDef {
        id: SIZE,
        label: "Lens size",
        group: "Appearance",
        order: 41,
        default: Value::Float(240.0),
        constraint: Constraint::Range {
            min: 120.0,
            max: 480.0,
        },
    });
}
