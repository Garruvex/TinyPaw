//! Spotlight: dims everything except a clear circle around the pointer. Taps
//! and ink work as in Point. As a tool the mask disappears whenever input
//! goes back to the app underneath; Spotlight anywhere (Ctrl+Alt+X) keeps it
//! over the desktop with annotation on or off.

use crate::features::anywhere::Target;
use crate::features::point::PointTool;
use crate::registry::{
    CommandDef, Constraint, Edge, Outcome, Registrar, SettingDef, SettingValues, ShellAction,
    ShortcutContext, Surfaces, ToolDef, Value, always, when_enabled,
};
use crate::session::{Event, Mark, Millis, PointerEvent, Pt, Tip, ToolCx, ToolState};

pub const TOOL: &str = "spotlight";
pub const SELECT: &str = "spotlight.select";
pub const RADIUS: &str = "spotlight.radius";
/// How dark the area outside the circle is, 0 to 1.
pub const DIM: &str = "spotlight.dim";
/// Tap toggles, hold shows it until release, with annotation on or off.
pub const ANYWHERE: &str = "spotlight.anywhere";
pub const ANYWHERE_BEGIN: &str = "spotlight.anywhere_begin";
pub const ANYWHERE_END: &str = "spotlight.anywhere_end";

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
        shortcut: Some((ShortcutContext::Overlay, "S")),
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
        constraint: Constraint::Range {
            min: 60.0,
            max: 400.0,
        },
    });
    r.setting(SettingDef {
        id: DIM,
        label: "Spotlight darkness",
        group: "Appearance",
        order: 31,
        default: Value::Float(0.55),
        constraint: Constraint::Range {
            min: 0.3,
            max: 0.85,
        },
    });
    r.command(CommandDef {
        id: ANYWHERE,
        label: "Spotlight anywhere",
        cli: None,
        surfaces: Surfaces::NONE,
        group: "tools",
        order: 8,
        // Beside Ctrl+Alt+Z for the lens.
        shortcut: Some((ShortcutContext::Global, "Ctrl+Alt+X")),
        available: always,
        handler: |_| Outcome::Shell(ShellAction::Anywhere(Target::Spotlight, Edge::Press)),
        on_release: Some(|_| {
            Outcome::Shell(ShellAction::Anywhere(Target::Spotlight, Edge::Release))
        }),
    });
    r.command(CommandDef {
        id: ANYWHERE_BEGIN,
        label: "Show spotlight",
        cli: Some("spotlight-begin"),
        surfaces: Surfaces::CLI,
        group: "tools",
        order: 9,
        shortcut: None,
        available: always,
        handler: |_| Outcome::Shell(ShellAction::Anywhere(Target::Spotlight, Edge::Show)),
        on_release: None,
    });
    r.command(CommandDef {
        id: ANYWHERE_END,
        label: "End spotlight",
        cli: Some("spotlight-end"),
        surfaces: Surfaces::CLI,
        group: "tools",
        order: 10,
        shortcut: None,
        available: always,
        handler: |_| Outcome::Shell(ShellAction::Anywhere(Target::Spotlight, Edge::Hide)),
        on_release: None,
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
