//! Spotlight: dims everything except a clear circle around the pointer. Taps
//! and ink work as in Point. The mask disappears whenever input goes back to
//! the app underneath, because only a pointing session draws it.

use crate::features::point::PointTool;
use crate::registry::{
    CommandDef, Constraint, Outcome, Registrar, SettingDef, SettingValues, Surfaces, ToolDef,
    Value, when_enabled,
};
use crate::session::{Event, Mark, Millis, PointerEvent, Pt, Tip, ToolCx, ToolState};

pub const TOOL: &str = "spotlight";
pub const SELECT: &str = "spotlight.select";
pub const RADIUS: &str = "spotlight.radius";

pub fn register(r: &mut Registrar) {
    r.tool(ToolDef {
        id: TOOL,
        label: "Spotlight",
        select_command: SELECT,
        factory: new_state,
    });
    r.command(CommandDef {
        id: SELECT,
        label: "Spotlight",
        cli: Some("spotlight"),
        surfaces: Surfaces::PICKER.with(Surfaces::CLI),
        group: "tools",
        order: 4,
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
    r.setting(SettingDef {
        id: RADIUS,
        label: "Spotlight size",
        group: "Appearance",
        order: 30,
        default: Value::Float(160.0),
        constraint: Constraint::Range { min: 60.0, max: 400.0 },
    });
}

pub fn new_state(_: &SettingValues) -> Box<dyn ToolState> {
    Box::new(SpotlightTool::default())
}

#[derive(Debug, Default)]
pub struct SpotlightTool {
    inner: PointTool,
}

impl ToolState for SpotlightTool {
    fn pointer(&mut self, ev: &PointerEvent, cx: &mut ToolCx<'_>) {
        self.inner.pointer(ev, cx);
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

    fn spotlight(&self, settings: &SettingValues) -> Option<f32> {
        Some(settings.float(RADIUS) as f32)
    }
}
