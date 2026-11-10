//! Numbered pins: each tap places the next number. Pins share undo order with
//! ink and labels; undoing the last pin frees its number. Drags still draw.

use crate::features::point::PointTool;
use crate::features::system::INK_COLOR;
use crate::registry::{CommandDef, Outcome, Registrar, SettingValues, Surfaces, ToolDef, when_enabled};
use crate::session::{
    Effect, Event, Mark, Millis, Pin, PointerEvent, PointerKind, Pt, Rgba, Tip, ToolCx, ToolState,
};

pub const TOOL: &str = "pin";
pub const SELECT: &str = "pin.select";

pub fn register(r: &mut Registrar) {
    r.tool(ToolDef {
        id: TOOL,
        label: "Pin",
        select_command: SELECT,
        factory: new_state,
    });
    r.command(CommandDef {
        id: SELECT,
        label: "Numbered pin",
        cli: Some("pin"),
        surfaces: Surfaces::PICKER.with(Surfaces::CLI),
        group: "tools",
        order: 3,
        shortcut: None,
        available: when_enabled,
        handler: |_| {
            Outcome::Session(Event::SelectTool {
                id: TOOL,
                factory: new_state,
            })
        },
        on_release: None,
    });
}

pub fn new_state(_: &SettingValues) -> Box<dyn ToolState> {
    Box::new(PinTool::default())
}

#[derive(Debug, Default)]
pub struct PinTool {
    inner: PointTool,
}

impl ToolState for PinTool {
    fn pointer(&mut self, ev: &PointerEvent, cx: &mut ToolCx<'_>) {
        let before = cx.effects.len();
        self.inner.pointer(ev, cx);
        // Point classifies a tap on release; a tap here places a pin.
        let tapped = ev.kind == PointerKind::Up && cx.effects[before..].contains(&Effect::PlayTap);
        if tapped {
            let number = cx.scene.next_pin();
            cx.scene.push(Mark::Pin(Pin {
                output: ev.output,
                at: ev.pos,
                number,
                color: Rgba(cx.settings.color(INK_COLOR)),
            }));
        }
    }

    fn finish(&mut self, cx: &mut ToolCx<'_>) {
        self.inner.finish(cx);
    }

    fn draft(&self) -> Option<&Mark> {
        self.inner.draft()
    }

    fn button_down(&self) -> bool {
        self.inner.button_down()
    }

    fn tip(&self, at: Pt, now: Millis, settings: &SettingValues) -> Option<Tip> {
        self.inner.tip(at, now, settings)
    }

    fn animating(&self, now: Millis) -> bool {
        self.inner.animating(now)
    }
}
