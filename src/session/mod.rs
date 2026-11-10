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

#[derive(Debug, Clone)]
#[allow(dead_code)] // InputFailed/Interrupted: lock and display-loss hooks land in phase 4.
pub enum Event {
    MasterToggle,
    MasterOff,
    /// Enabled-only: annotate without holding a key.
    Latch,
    HoldDown,
    HoldUp,
    InputAcquired(Ticket),
    InputFailed(Ticket),
    Pointer(PointerEvent),
    Undo,
    Clear,
    SelectTool { id: &'static str, factory: ToolFactory },
    /// Lock, suspend or loss of the target display.
    Interrupted,
    SettingsChanged(SettingValues),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    PlayTap,
    /// Give focus back to whatever had it before TinyPaw took input.
    ReturnFocus,
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
    /// Hold released while a button was down: swallow the button-up first.
    awaiting_button_up: bool,
    pointer: Option<(OutputId, Pt)>,
    live: Scene,
    tool_id: &'static str,
    tool_factory: ToolFactory,
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
            awaiting_button_up: false,
            pointer: None,
            live: Scene::default(),
            tool_id,
            tool_factory: factory,
            tool: factory(&settings),
            settings,
        }
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

    pub fn scene(&self) -> &Scene {
        &self.live
    }

    pub fn draft(&self) -> Option<&Mark> {
        self.tool.draft()
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
            Mode::Passthrough if self.pending.is_some() || self.awaiting_button_up => {
                OverlayState::Interactive
            }
            Mode::Passthrough if self.live.is_empty() => OverlayState::Hidden,
            Mode::Passthrough => OverlayState::ClickThrough,
        }
    }

    /// Whether the tip and toolbar should be drawn.
    pub fn is_pointing(&self) -> bool {
        matches!(self.mode, Mode::Annotating(_))
    }

    pub fn tip(&self, now: Millis) -> Option<(OutputId, Tip)> {
        if !self.is_pointing() {
            return None;
        }
        let (output, at) = self.pointer?;
        self.tool.tip(at, now, &self.settings).map(|tip| (output, tip))
    }

    pub fn animating(&self, now: Millis) -> bool {
        self.is_pointing() && self.tool.animating(now)
    }

    pub fn handle(&mut self, event: Event, now: Millis) -> Vec<Effect> {
        let mut effects = Vec::new();
        match event {
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
                    if !self.awaiting_button_up {
                        self.request_input(Ownership::Latched);
                    }
                }
                Mode::Annotating(Ownership::Held { .. }) => {
                    self.mode = Mode::Annotating(Ownership::Latched);
                }
                Mode::Annotating(Ownership::Latched) => {}
            },
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
                    self.live.undo();
                }
            }
            Event::Clear => {
                if self.is_enabled() {
                    let mut cx = Self::cx(&mut self.live, &mut effects, &self.settings, now);
                    self.tool.finish(&mut cx);
                    self.live.clear();
                }
            }
            Event::SelectTool { id, factory } => {
                if self.tool_id != id {
                    let mut cx = Self::cx(&mut self.live, &mut effects, &self.settings, now);
                    self.tool.finish(&mut cx);
                    self.tool_id = id;
                    self.tool_factory = factory;
                    self.tool = factory(&self.settings);
                }
            }
            Event::SettingsChanged(values) => {
                self.settings = values;
            }
        }
        effects
    }

    fn cx<'a>(
        scene: &'a mut Scene,
        effects: &'a mut Vec<Effect>,
        settings: &'a SettingValues,
        now: Millis,
    ) -> ToolCx<'a> {
        ToolCx {
            scene,
            effects,
            settings,
            now,
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
        self.mode = Mode::Off;
        self.pending = None;
        self.awaiting_button_up = false;
        // An in-progress hold is invalidated; its key-up will be ignored.
        self.hold_blocked = self.hold_down;
        self.pointer = None;
        self.live.clear();
        self.tool = (self.tool_factory)(&self.settings);
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
        let mut cx = Self::cx(&mut self.live, effects, &self.settings, now);
        self.tool.finish(&mut cx);
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
            PointerKind::Left => self.pointer = None,
            _ => self.pointer = Some((ev.output, ev.pos)),
        }
        let mut cx = Self::cx(&mut self.live, effects, &self.settings, now);
        self.tool.pointer(&ev, &mut cx);
    }
}

#[cfg(test)]
mod tests;
