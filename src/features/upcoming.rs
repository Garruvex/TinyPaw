//! Tools from the first-release scope that are registered now so every
//! surface shows them, and refuse with a reason until their phase lands.

use crate::registry::{CommandDef, Outcome, Registrar, ShortcutContext, Surfaces, later_phase};
use crate::session::Event;

pub const TEXT: &str = "text.select";
pub const PIN: &str = "pin.select";
pub const SPOTLIGHT: &str = "spotlight.select";
pub const LENS: &str = "lens.select";
pub const FREEZE: &str = "freeze.toggle";

fn tool(r: &mut Registrar, id: &'static str, label: &'static str, order: u16) {
    r.command(CommandDef {
        id,
        label,
        cli: None,
        surfaces: Surfaces::PICKER,
        group: "tools",
        order,
        shortcut: None,
        available: later_phase,
        // Never reached: availability refuses first.
        handler: |_| Outcome::Session(Event::Undo),
        on_release: None,
    });
}

pub fn register(r: &mut Registrar) {
    tool(r, TEXT, "Text", 2);
    tool(r, PIN, "Pin", 3);
    tool(r, SPOTLIGHT, "Spotlight", 4);
    tool(r, LENS, "Lens", 5);
    r.command(CommandDef {
        id: FREEZE,
        label: "Freeze frame",
        cli: Some("freeze"),
        surfaces: Surfaces::TRAY.with(Surfaces::CLI),
        group: "1-session",
        order: 3,
        shortcut: Some((ShortcutContext::Global, "Ctrl+Alt+F")),
        available: later_phase,
        handler: |_| Outcome::Session(Event::Undo),
        on_release: None,
    });
}
