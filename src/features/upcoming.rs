//! Live window lens. DWM samples the source window before TinyPaw is composed.
use crate::registry::{
    CommandDef, Constraint, Outcome, Registrar, SettingDef, Surfaces, ToolDef, Value, when_enabled,
};
use crate::session::Event;
pub const LENS: &str = "lens.select";
pub const TOOL: &str = "lens";
pub const SOURCE: &str = "lens.source";
pub const ZOOM: &str = "lens.zoom";
pub const SIZE: &str = "lens.size";
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
        shortcut: None,
        available: when_enabled,
        handler: |_| {
            Outcome::Session(Event::SelectTool {
                id: TOOL,
                factory: crate::features::point::new_state,
            })
        },
        on_release: None,
    });
    r.setting(SettingDef {
        id: SOURCE,
        label: "Lens source window",
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
