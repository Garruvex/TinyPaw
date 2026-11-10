//! Tools from the first-release scope that are registered now so every
//! surface shows them, and refuse with a reason until their phase lands.

use crate::registry::{CommandDef, Outcome, Registrar, Surfaces, later_phase};
use crate::session::Event;

/// Lens needs a live capture source that leaves the overlay out of the frame.
pub const LENS: &str = "lens.select";

pub fn register(r: &mut Registrar) {
    r.command(CommandDef {
        id: LENS,
        label: "Lens",
        cli: None,
        surfaces: Surfaces::PICKER,
        group: "tools",
        order: 5,
        shortcut: None,
        available: later_phase,
        // Never reached: availability refuses first.
        handler: |_| Outcome::Session(Event::Undo),
        on_release: None,
    });
}
