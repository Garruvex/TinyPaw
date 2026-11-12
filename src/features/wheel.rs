//! Quick wheel: hold the binding, point at a tool, release to pick.

use crate::registry::{
    CommandDef, Outcome, Registrar, ShellAction, ShortcutContext, Surfaces, when_enabled,
};

pub const OPEN: &str = "wheel.open";

pub fn register(r: &mut Registrar) {
    r.command(CommandDef {
        id: OPEN,
        label: "Quick wheel",
        cli: None,
        surfaces: Surfaces::NONE,
        group: "1-session",
        order: 5,
        shortcut: Some((ShortcutContext::Global, "Ctrl+Alt+W")),
        available: when_enabled,
        handler: |_| Outcome::Shell(ShellAction::WheelOpen),
        on_release: Some(|_| Outcome::Shell(ShellAction::WheelClose)),
    });
}
