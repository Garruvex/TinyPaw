//! Contract between the session and interactive tools registered by features.

use super::{Effect, Millis, OutputId, Pt, Scene};
use crate::registry::SettingValues;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerKind {
    Moved,
    Down,
    Up,
    Left,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerEvent {
    pub kind: PointerKind,
    pub output: OutputId,
    pub pos: Pt,
    pub at: Millis,
}

/// What the session lends a tool while it handles input.
pub struct ToolCx<'a> {
    pub scene: &'a mut Scene,
    pub effects: &'a mut Vec<Effect>,
    pub settings: &'a SettingValues,
    pub now: Millis,
}

/// Shared scene primitive for the moving tip. The view owns how each is painted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tip {
    /// `press` runs 0.0 (idle) to 1.0 (fully pressed); `scale` is the
    /// chosen size relative to the artwork; `stick` mounts it on a rod.
    Paw { at: Pt, press: f32, scale: f32, stick: bool },
    Laser { at: Pt, press: f32 },
}

/// Feature-owned state for the active tool.
pub trait ToolState {
    /// Handles one pointer event that arrived while this tool is active.
    fn pointer(&mut self, ev: &PointerEvent, cx: &mut ToolCx<'_>);

    /// Ends any unfinished gesture: commit strokes, cancel pending taps.
    fn finish(&mut self, cx: &mut ToolCx<'_>);

    /// The uncommitted mark being drawn, if any.
    fn draft(&self) -> Option<&super::Mark>;

    /// True while a primary button press is owned by this tool.
    fn button_down(&self) -> bool;

    /// The tip to draw at the pointer, if any.
    fn tip(&self, at: Pt, now: Millis, settings: &SettingValues) -> Option<Tip>;

    /// True while an animation needs frames.
    fn animating(&self, now: Millis) -> bool;
}

/// Creates the state for a tool when it is selected.
pub type ToolFactory = fn(&SettingValues) -> Box<dyn ToolState>;
