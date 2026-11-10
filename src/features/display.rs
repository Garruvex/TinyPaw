//! Target display and stream-facing visibility settings.

use crate::registry::{Constraint, Registrar, SettingDef, Value};

pub const TARGET: &str = "display.target";
pub const HIDE_BAR: &str = "display.hide_bar";

pub fn register(r: &mut Registrar) {
    r.setting(SettingDef {
        id: TARGET,
        label: "Draw on",
        group: "Display",
        order: 0,
        default: Value::Choice("primary"),
        // "all" joins the choices with multi-monitor support.
        constraint: Constraint::Choices(&["primary"]),
    });
    r.setting(SettingDef {
        id: HIDE_BAR,
        label: "Hide the quick bar while streaming",
        group: "Display",
        order: 1,
        default: Value::Bool(false),
        constraint: Constraint::None,
    });
}
