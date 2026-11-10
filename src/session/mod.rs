//! Annotation session: pure transition logic, independent of Iced and the
//! desktop backend. Events go in with an explicit timestamp; requested effects
//! and a declarative overlay state come out.

mod scene;
mod tool;

pub use scene::*;
pub use tool::*;

use crate::registry::SettingValues;

/// Milliseconds since the resident process started.
pub type Millis = u64;

/// With fading on, a stroke stays fully visible this long after release…
pub const FADE_HOLD_MS: Millis = 3000;
/// …then fades out over this long and is removed.
pub const FADE_OUT_MS: Millis = 400;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationStyle {
    /// On opens Point immediately.
    Toggle,
    /// On starts in Passthrough; a held shortcut points and draws.
    HoldToDraw,
}

impl ActivationStyle {
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "toggle" => Some(Self::Toggle),
            "hold" => Some(Self::HoldToDraw),
            _ => None,
        }
    }
}

/// Why TinyPaw owns input, so a stale key release cannot end a latched session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ownership {
    Latched,
    Held { hold: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Off,
    /// Enabled; the app underneath has input, committed marks stay visible.
    Passthrough,
    Annotating(Ownership),
}

/// Ties an asynchronous platform reply to the request that caused it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ticket {
    pub generation: u64,
    pub request: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Pending {
    ticket: Ticket,
    ownership: Ownership,
}

/// A still requested for Freeze, remembered with where Freeze started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Capture {
    ticket: Ticket,
    from: Mode,
}

/// The frozen scene: its own marks over one still frame. The pixels live
/// outside the session; `frame` identifies them.
#[derive(Debug)]
struct Frozen {
    scene: Scene,
    frame: u64,
    from: Mode,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // InputFailed/Interrupted: lock and display-loss hooks land in phase 4.
pub enum Event {
    MasterToggle,
    MasterOff,
    /// Enabled-only: annotate without holding a key.
    Latch,
    HoldDown,
    InteractDown,
    InteractUp,
    ButtonsReleased,
    HoldUp,
    InputAcquired(Ticket),
    InputFailed(Ticket),
    Pointer(PointerEvent),
    Undo,
    Clear,
    SelectTool {
        id: &'static str,
        factory: ToolFactory,
    },
    /// Freeze from Live, or Return Live when frozen.
    FreezeToggle,
    StillReady {
        ticket: Ticket,
        frame: u64,
    },
    StillFailed(Ticket),
    Text(TextInput),
    /// Lock, suspend or loss of the target display.
    Interrupted,
    SettingsChanged(SettingValues),
    /// Time passed; expires faded ink.
    Tick,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    PlayTap,
    /// Give focus back to whatever had it before TinyPaw took input.
    ReturnFocus,
    /// Take one still of the target display once the overlay shows no marks.
    CaptureStill(Ticket),
    /// Drop the frozen frame's pixels.
    ReleaseStill,
    Notice(&'static str),
}

/// What the platform layer should be showing right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayState {
    Hidden,
    /// Marks visible, input goes to the app underneath.
    ClickThrough,
    /// Overlay receives pointer and keyboard input.
    Interactive,
}

pub struct Session {
    mode: Mode,
    generation: u64,
    next_request: u64,
    pending: Option<Pending>,
    /// Physical state of the hold-to-draw binding.
    hold_down: bool,
    /// Set when enabled while the hold key was already down: needs a fresh press.
    hold_blocked: bool,
    hold_seq: u64,
    interacting: bool,
    interact_released: bool,
    /// Hold released while a button was down: swallow the button-up first.
    awaiting_button_up: bool,
    pointer: Option<(OutputId, Pt)>,
    live: Scene,
    frozen: Option<Frozen>,
    capture: Option<Capture>,
    default_tool: (&'static str, ToolFactory),
    tool_id: &'static str,
    tool: Box<dyn ToolState>,
    settings: SettingValues,
}

impl Session {
    pub fn new(tool_id: &'static str, factory: ToolFactory, settings: SettingValues) -> Self {
        Self {
            mode: Mode::Off,
            generation: 0,
            next_request: 0,
            pending: None,
            hold_down: false,
            hold_blocked: false,
            hold_seq: 0,
            interacting: false,
            interact_released: false,
            awaiting_button_up: false,
            pointer: None,
            live: Scene::default(),
            frozen: None,
            capture: None,
            default_tool: (tool_id, factory),
            tool_id,
            tool: factory(&settings),
            settings,
        }
    }

    pub fn waiting_for_interact_resume(&self) -> bool {
        self.interacting && self.interact_released
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn is_enabled(&self) -> bool {
        self.mode != Mode::Off
    }

    pub fn pending_ticket(&self) -> Option<Ticket> {
        self.pending.map(|p| p.ticket)
    }

    /// The scene undo, clear and drawing act on: the frozen one while frozen.
    pub fn scene(&self) -> &Scene {
        self.frozen.as_ref().map_or(&self.live, |f| &f.scene)
    }

    /// The frozen frame being shown, if any.
    pub fn frozen_frame(&self) -> Option<u64> {
        self.frozen.as_ref().map(|f| f.frame)
    }

    /// True between a Freeze request and its still: the overlay shows nothing
    /// so marks are not baked into the capture.
    pub fn capture_ticket(&self) -> Option<Ticket> {
        self.capture.map(|c| c.ticket)
    }

    pub fn capturing(&self) -> bool {
        self.capture.is_some()
    }

    pub fn draft(&self) -> Option<&Mark> {
        self.tool.draft()
    }

    pub fn text_draft(&self) -> Option<TextDraft<'_>> {
        if self.is_pointing() {
            self.tool.text_draft()
        } else {
            None
        }
    }

    pub fn spotlight(&self) -> Option<(OutputId, Pt, f32)> {
        if !self.is_pointing() || self.capturing() {
            return None;
        }
        let (output, at) = self.pointer?;
        self.tool
            .spotlight(&self.settings)
            .map(|radius| (output, at, radius))
    }

    pub fn active_tool(&self) -> &'static str {
        self.tool_id
    }

    pub fn settings(&self) -> &SettingValues {
        &self.settings
    }

    pub fn activation_style(&self) -> ActivationStyle {
        ActivationStyle::from_id(self.settings.choice("system.activation_style"))
            .unwrap_or(ActivationStyle::Toggle)
    }

    pub fn overlay_state(&self) -> OverlayState {
        match self.mode {
            Mode::Off => OverlayState::Hidden,
            Mode::Annotating(_) => OverlayState::Interactive,
            Mode::Passthrough
                if self.pending.is_some() || self.awaiting_button_up || self.frozen.is_some() =>
            {
                OverlayState::Interactive
            }
            Mode::Passthrough if self.live.is_empty() => OverlayState::Hidden,
            Mode::Passthrough => OverlayState::ClickThrough,
        }
    }

    /// Annotating without a held key, so "Annotate now" has nothing to do.
    pub fn is_latched(&self) -> bool {
        self.mode == Mode::Annotating(Ownership::Latched)
    }

    /// Whether the tip and toolbar should be drawn.
    pub fn is_pointing(&self) -> bool {
        matches!(self.mode, Mode::Annotating(_))
    }

    pub fn tip(&self, now: Millis) -> Option<(OutputId, Tip)> {
        if !self.is_pointing() || self.capturing() {
            return None;
        }
        let (output, at) = self.pointer?;
        self.tool
            .tip(at, now, &self.settings)
            .map(|tip| (output, tip))
    }

    pub fn pointer(&self) -> Option<(OutputId, Pt)> {
        self.pointer
    }

    pub fn fading(&self) -> bool {
        self.settings.choice("ink.fade") == "3s"
    }

    /// Opacity of a committed mark at `now` (1.0 unless it is fading out).
    pub fn mark_alpha(&self, mark: &Mark, now: Millis) -> f32 {
        let Some(born) = mark.born().filter(|_| self.fading()) else {
            return 1.0;
        };
        let age = now.saturating_sub(born);
        if crate::features::system::reduced(&self.settings) {
            return if age <= FADE_HOLD_MS { 1.0 } else { 0.0 };
        }
        if age <= FADE_HOLD_MS {
            1.0
        } else {
            1.0 - ((age - FADE_HOLD_MS) as f32 / FADE_OUT_MS as f32).min(1.0)
        }
    }

    /// True while any mark is visibly fading, so the view needs frames.
    pub fn fade_animating(&self, now: Millis) -> bool {
        self.fading()
            && !crate::features::system::reduced(&self.settings)
            && self
                .scene()
                .marks()
                .iter()
                .filter_map(Mark::born)
                .any(|born| {
                    let age = now.saturating_sub(born);
                    age > FADE_HOLD_MS && age <= FADE_HOLD_MS + FADE_OUT_MS
                })
    }

    /// When the next fade starts, if one is waiting.
    pub fn next_fade(&self, now: Millis) -> Option<Millis> {
        if !self.fading() {
            return None;
        }
        self.scene()
            .marks()
            .iter()
            .filter_map(Mark::born)
            .map(|born| born + FADE_HOLD_MS)
            .filter(|&t| t > now)
            .min()
    }

    pub fn animating(&self, now: Millis) -> bool {
        (self.is_pointing()
            && !crate::features::system::reduced(&self.settings)
            && self.tool.animating(now))
            || self.fade_animating(now)
    }

    pub fn handle(&mut self, event: Event, now: Millis) -> Vec<Effect> {
        let mut effects = Vec::new();
        if self.fading() {
            let duration = FADE_HOLD_MS
                + if crate::features::system::reduced(&self.settings) {
                    0
                } else {
                    FADE_OUT_MS
                };
            let cutoff = now.saturating_sub(duration);
            self.live.expire(cutoff);
            if let Some(f) = &mut self.frozen {
                f.scene.expire(cutoff);
            }
        }
        match event {
            Event::Tick => {}
            Event::MasterToggle => {
                if self.is_enabled() {
                    self.turn_off(&mut effects);
                } else {
                    self.turn_on();
                }
            }
            Event::MasterOff | Event::Interrupted => {
                if self.is_enabled() {
                    self.turn_off(&mut effects);
                }
            }
            Event::Latch => match self.mode {
                Mode::Off => {}
                Mode::Passthrough => {
                    self.interacting = false;
                    self.interact_released = false;
                    if !self.awaiting_button_up {
                        self.request_input(Ownership::Latched);
                    }
                }
                Mode::Annotating(Ownership::Held { .. }) => {
                    self.mode = Mode::Annotating(Ownership::Latched);
                }
                Mode::Annotating(Ownership::Latched) => {}
            },
            Event::InteractDown => {
                if self.mode == Mode::Annotating(Ownership::Latched)
                    && self.tool_id == "point"
                    && !self.tool.button_down()
                    && self.frozen.is_none()
                    && !self.capturing()
                {
                    self.interacting = true;
                    self.interact_released = false;
                    self.pending = None;
                    self.release_to_passthrough(now, &mut effects);
                }
            }
            Event::InteractUp => {
                if self.interacting {
                    self.interact_released = true;
                }
            }
            Event::ButtonsReleased => {
                if self.interacting && self.interact_released && self.mode == Mode::Passthrough {
                    self.interacting = false;
                    self.interact_released = false;
                    self.request_input(Ownership::Latched);
                }
            }
            Event::HoldDown => {
                if self.hold_down {
                    return effects; // auto-repeat or duplicate edge
                }
                self.hold_down = true;
                if self.mode == Mode::Passthrough
                    && self.activation_style() == ActivationStyle::HoldToDraw
                    && !self.hold_blocked
                    && self.pending.is_none()
                    && !self.awaiting_button_up
                    && self.frozen.is_none()
                {
                    self.hold_seq += 1;
                    self.request_input(Ownership::Held {
                        hold: self.hold_seq,
                    });
                }
            }
            Event::HoldUp => {
                if !self.hold_down {
                    return effects; // unmatched edge
                }
                self.hold_down = false;
                self.hold_blocked = false;
                match (self.mode, self.pending) {
                    (_, Some(p)) if matches!(p.ownership, Ownership::Held { .. }) => {
                        self.pending = None;
                    }
                    (Mode::Annotating(Ownership::Held { .. }), _) => {
                        self.release_to_passthrough(now, &mut effects);
                    }
                    _ => {}
                }
            }
            Event::InputAcquired(ticket) => {
                if let Some(p) = self.pending
                    && p.ticket == ticket
                {
                    self.pending = None;
                    self.mode = Mode::Annotating(p.ownership);
                }
            }
            Event::InputFailed(ticket) => {
                if self.pending.map(|p| p.ticket) == Some(ticket) {
                    self.pending = None;
                    effects.push(Effect::ReturnFocus);
                }
            }
            Event::Pointer(ev) => self.pointer_event(ev, now, &mut effects),
            Event::Undo => {
                if self.is_enabled() && self.tool.draft().is_none() {
                    self.scene_mut().undo();
                }
            }
            Event::Clear => {
                if self.is_enabled() {
                    self.with_tool(&mut effects, now, |tool, cx| tool.finish(cx));
                    self.scene_mut().clear();
                }
            }
            Event::SelectTool { id, factory } => self.select_tool(id, factory, now, &mut effects),
            Event::Text(input) => {
                if self.is_pointing() {
                    let done =
                        self.with_tool(&mut effects, now, |tool, cx| tool.text_input(input, cx));
                    if done {
                        let (id, factory) = self.default_tool;
                        self.select_tool(id, factory, now, &mut effects);
                    }
                }
            }
            Event::FreezeToggle => self.freeze_toggle(now, &mut effects),
            Event::StillReady { ticket, frame } => {
                match self.capture {
                    Some(c) if c.ticket == ticket => {
                        self.capture = None;
                        self.frozen = Some(Frozen {
                            scene: Scene::default(),
                            frame,
                            from: c.from,
                        });
                        let (id, factory) = self.default_tool;
                        self.select_tool(id, factory, now, &mut effects);
                        match self.mode {
                            // Only now take input, so a game that pauses on
                            // focus loss was captured while still running.
                            Mode::Passthrough => self.request_input(Ownership::Latched),
                            // A frozen scene outlives the hold that started it.
                            Mode::Annotating(_) => self.mode = Mode::Annotating(Ownership::Latched),
                            Mode::Off => {}
                        }
                    }
                    // A late or cancelled still: let the shell drop it.
                    _ => effects.push(Effect::ReleaseStill),
                }
            }
            Event::StillFailed(ticket) => {
                if self.capture.map(|c| c.ticket) == Some(ticket) {
                    self.capture = None;
                    effects.push(Effect::Notice(
                        "Freeze needs a screen capture, which failed",
                    ));
                }
            }
            Event::SettingsChanged(values) => {
                self.settings = values;
            }
        }
        effects
    }

    fn scene_mut(&mut self) -> &mut Scene {
        match &mut self.frozen {
            Some(f) => &mut f.scene,
            None => &mut self.live,
        }
    }

    /// Lends the active tool the current scene.
    fn with_tool<R>(
        &mut self,
        effects: &mut Vec<Effect>,
        now: Millis,
        f: impl FnOnce(&mut dyn ToolState, &mut ToolCx<'_>) -> R,
    ) -> R {
        let scene = match &mut self.frozen {
            Some(frozen) => &mut frozen.scene,
            None => &mut self.live,
        };
        let mut cx = ToolCx {
            scene,
            effects,
            settings: &self.settings,
            now,
        };
        f(self.tool.as_mut(), &mut cx)
    }

    fn select_tool(
        &mut self,
        id: &'static str,
        factory: ToolFactory,
        now: Millis,
        effects: &mut Vec<Effect>,
    ) {
        if self.tool_id != id {
            self.with_tool(effects, now, |tool, cx| tool.finish(cx));
            self.tool_id = id;
            self.tool = factory(&self.settings);
        }
    }

    fn reset_tool(&mut self) {
        let (id, factory) = self.default_tool;
        self.tool_id = id;
        self.tool = factory(&self.settings);
    }

    fn freeze_toggle(&mut self, now: Millis, effects: &mut Vec<Effect>) {
        if self.frozen.is_some() {
            self.return_live(now, effects);
        } else if self.capture.is_some() {
            // Pressed again before the still arrived: cancel.
            self.capture = None;
        } else if self.is_enabled() && !self.awaiting_button_up {
            self.with_tool(effects, now, |tool, cx| tool.finish(cx));
            self.next_request += 1;
            let ticket = Ticket {
                generation: self.generation,
                request: self.next_request,
            };
            self.capture = Some(Capture {
                ticket,
                from: self.mode,
            });
            effects.push(Effect::CaptureStill(ticket));
        }
    }

    /// Releases the still and its marks and returns to where Freeze started.
    fn return_live(&mut self, now: Millis, effects: &mut Vec<Effect>) {
        let Some(frozen) = self.frozen.take() else {
            return;
        };
        effects.push(Effect::ReleaseStill);
        self.reset_tool();
        let _ = now;
        if frozen.from == Mode::Passthrough {
            if self.pending.take().is_none() && self.is_pointing() {
                effects.push(Effect::ReturnFocus);
            }
            self.mode = Mode::Passthrough;
        }
    }

    fn turn_on(&mut self) {
        self.generation += 1;
        self.hold_blocked = self.hold_down;
        self.mode = Mode::Passthrough;
        if self.activation_style() == ActivationStyle::Toggle {
            self.request_input(Ownership::Latched);
        }
    }

    fn turn_off(&mut self, effects: &mut Vec<Effect>) {
        // Advance the generation first so late replies cannot restore the overlay.
        self.generation += 1;
        let had_input = matches!(self.mode, Mode::Annotating(_))
            || self.pending.is_some()
            || self.awaiting_button_up;
        self.interacting = false;
        self.interact_released = false;
        self.mode = Mode::Off;
        self.pending = None;
        self.awaiting_button_up = false;
        self.capture = None;
        if self.frozen.take().is_some() {
            effects.push(Effect::ReleaseStill);
        }
        // An in-progress hold is invalidated; its key-up will be ignored.
        self.hold_blocked = self.hold_down;
        self.pointer = None;
        self.live.clear();
        self.reset_tool();
        if had_input {
            effects.push(Effect::ReturnFocus);
        }
    }

    fn request_input(&mut self, ownership: Ownership) {
        self.next_request += 1;
        self.pending = Some(Pending {
            ticket: Ticket {
                generation: self.generation,
                request: self.next_request,
            },
            ownership,
        });
    }

    fn release_to_passthrough(&mut self, now: Millis, effects: &mut Vec<Effect>) {
        let button_down = self.tool.button_down();
        self.with_tool(effects, now, |tool, cx| tool.finish(cx));
        self.mode = Mode::Passthrough;
        if button_down {
            // Keep input until the matching button-up so the game never sees it.
            self.awaiting_button_up = true;
        } else {
            effects.push(Effect::ReturnFocus);
        }
    }

    fn pointer_event(&mut self, ev: PointerEvent, now: Millis, effects: &mut Vec<Effect>) {
        if self.awaiting_button_up {
            if ev.kind == PointerKind::Up {
                self.awaiting_button_up = false;
                effects.push(Effect::ReturnFocus);
            }
            return;
        }
        if !self.is_pointing() {
            return;
        }
        match ev.kind {
            PointerKind::Left if self.pointer.is_some_and(|(output, _)| output != ev.output) => {
                return;
            }
            PointerKind::Left => self.pointer = None,
            _ => self.pointer = Some((ev.output, ev.pos)),
        }
        if self.capturing() {
            return; // Nothing is drawn until the still is on screen.
        }
        self.with_tool(effects, now, |tool, cx| tool.pointer(&ev, cx));
    }
}

#[cfg(test)]
mod tests;
