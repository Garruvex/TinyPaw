//! Freeze: take one clean still of the target display and draw on it. Press
//! again (or Return Live) to discard the still and its marks.

use crate::registry::{CommandDef, Outcome, Registrar, ShortcutContext, Surfaces, when_enabled};
use crate::session::Event;

pub const TOGGLE: &str = "freeze.toggle";

pub fn register(r: &mut Registrar) {
    r.command(CommandDef {
        id: TOGGLE,
        label: "Freeze frame",
        cli: Some("freeze"),
        surfaces: Surfaces::TRAY.with(Surfaces::PICKER).with(Surfaces::CLI),
        group: "1-session",
        order: 3,
        shortcut: Some((ShortcutContext::Global, "Ctrl+Alt+F")),
        available: when_enabled,
        handler: |_| Outcome::Session(Event::FreezeToggle),
        on_release: None,
    });
}
