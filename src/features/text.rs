//! Text: click a spot, type a one-line label, Enter places it and returns to
//! Point. The editor owns keys while typing; IME composition finishes inside
//! the editor before Enter or Escape reach TinyPaw.

use crate::registry::{
    CommandDef, Constraint, Outcome, Registrar, SettingDef, SettingValues, ShortcutContext,
    Surfaces, ToolDef, Value, when_enabled,
};
use crate::session::{
    Event, Label, Mark, Millis, OutputId, PointerEvent, PointerKind, Pt, TextDraft, TextInput, Tip,
    ToolCx, ToolState,
};

pub const TOOL: &str = "text";
pub const SELECT: &str = "text.select";
pub const COLOR: &str = "text.color";
pub const BACKING: &str = "text.backing";
pub const SIZE: &str = "text.size";

/// Longest label accepted, in characters.
const MAX_CHARS: usize = 120;

pub fn register(r: &mut Registrar) {
    r.tool(ToolDef {
        id: TOOL,
        label: "Text",
        select_command: SELECT,
        factory: new_state,
    });
    r.command(CommandDef {
        id: SELECT,
        label: "Text",
        cli: Some("text"),
        surfaces: Surfaces::PICKER.with(Surfaces::CLI),
        group: "tools",
        order: 2,
        shortcut: Some((ShortcutContext::Overlay, "T")),
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
        id: COLOR,
        label: "Label colour",
        group: "Appearance",
        order: 21,
        default: Value::Color([255, 255, 255, 255]),
        constraint: Constraint::None,
    });
    r.setting(SettingDef {
        id: BACKING,
        label: "Label backing",
        group: "Appearance",
        order: 22,
        default: Value::Bool(true),
        constraint: Constraint::None,
    });
    r.setting(SettingDef {
        id: SIZE,
        label: "Label size",
        group: "Appearance",
        order: 20,
        default: Value::Float(18.0),
        constraint: Constraint::Range {
            min: 12.0,
            max: 48.0,
        },
    });
}

pub fn new_state(_: &SettingValues) -> Box<dyn ToolState> {
    Box::new(TextTool::default())
}

#[derive(Debug)]
struct Draft {
    output: OutputId,
    at: Pt,
    text: String,
    size: f32,
    history: Vec<String>,
    color: crate::session::Rgba,
    backing: bool,
}

#[derive(Debug, Default)]
pub struct TextTool {
    draft: Option<Draft>,
}

impl TextTool {
    fn commit(&mut self, cx: &mut ToolCx<'_>) {
        if let Some(d) = self.draft.take() {
            let text = d.text.trim().to_owned();
            if !text.is_empty() {
                cx.scene.push(Mark::Label(Label {
                    output: d.output,
                    at: d.at,
                    text,
                    size: d.size,
                    color: d.color,
                    backing: d.backing,
                }));
            }
        }
    }
}

impl ToolState for TextTool {
    fn pointer(&mut self, ev: &PointerEvent, cx: &mut ToolCx<'_>) {
        if ev.kind != PointerKind::Down {
            return;
        }
        // Clicking elsewhere places the current label and starts a new one.
        self.commit(cx);
        let size = cx.settings.float(SIZE) as f32;
        self.draft = Some(Draft {
            output: ev.output,
            // The click marks the middle of the first line.
            at: Pt::new(ev.pos.x, ev.pos.y - size * 0.7),
            text: String::new(),
            size,
            history: Vec::new(),
            color: crate::session::Rgba(cx.settings.color(COLOR)),
            backing: cx.settings.bool(BACKING),
        });
    }

    fn finish(&mut self, cx: &mut ToolCx<'_>) {
        self.commit(cx);
    }

    fn draft(&self) -> Option<&Mark> {
        None
    }

    fn button_down(&self) -> bool {
        false
    }

    fn tip(&self, _at: Pt, _now: Millis, _settings: &SettingValues) -> Option<Tip> {
        None // The view shows a text cursor instead.
    }

    fn animating(&self, _now: Millis) -> bool {
        false
    }

    fn text_draft(&self) -> Option<TextDraft<'_>> {
        self.draft.as_ref().map(|d| TextDraft {
            output: d.output,
            at: d.at,
            text: &d.text,
            size: d.size,
        })
    }

    fn text_input(&mut self, input: TextInput, cx: &mut ToolCx<'_>) -> bool {
        match input {
            TextInput::Undo => {
                if let Some(d) = &mut self.draft
                    && let Some(text) = d.history.pop()
                {
                    d.text = text;
                }
                false
            }
            TextInput::Changed(text) => {
                if let Some(d) = &mut self.draft {
                    // One line only; paste with newlines collapses to spaces.
                    if d.text != text {
                        d.history.push(d.text.clone());
                    }
                    d.text = text
                        .replace(['\r', '\n'], " ")
                        .chars()
                        .take(MAX_CHARS)
                        .collect();
                }
                false
            }
            TextInput::Submit => {
                let placed = self.draft.is_some();
                self.commit(cx);
                placed
            }
            TextInput::Cancel => {
                self.draft = None;
                false
            }
        }
    }
}
