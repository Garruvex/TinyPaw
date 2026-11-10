//! Point: tap with the paw or laser, drag to draw.

use crate::features::system::{INK_COLOR, INK_WIDTH};
use crate::registry::SettingValues;
use crate::registry::{
    CommandDef, Constraint, Outcome, Registrar, SettingDef, Surfaces, ToolDef, Value,
    when_enabled,
};
use crate::session::{
    Effect, Event, Mark, Millis, OutputId, PointerEvent, PointerKind, Pt, Rgba, Stroke, Tip,
    ToolCx, ToolState,
};

pub const TOOL: &str = "point";
pub const SELECT: &str = "point.select";
pub const USE_PAW: &str = "point.paw";
pub const USE_LASER: &str = "point.laser";
pub const STYLE: &str = "point.style";
pub const PRESENTATION: &str = "point.presentation";
pub const SIZE: &str = "point.size";

/// Paw height in logical pixels that the artwork is drawn at by default.
pub const BASE_SIZE: f32 = 56.0;
/// Movement in logical pixels before a press becomes a stroke.
pub const DRAG_THRESHOLD: f32 = 4.0;
/// Length of the press-and-release tap animation.
pub const TAP_MS: Millis = 100;
/// Minimum spacing between recorded stroke points.
const MIN_SPACING: f32 = 0.75;
/// Time constant of the stick grip easing toward the pointer.
pub const GRIP_TAU_MS: f32 = 90.0;
/// The grip counts as settled once it is this close to its target.
const GRIP_SETTLED: f32 = 0.5;

pub fn register(r: &mut Registrar) {
    r.tool(ToolDef {
        id: TOOL,
        label: "Point",
        select_command: SELECT,
        factory: new_state,
    });
    r.command(CommandDef {
        id: SELECT,
        label: "Point",
        cli: Some("point"),
        surfaces: Surfaces::PICKER.with(Surfaces::CLI),
        group: "tools",
        order: 0,
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
    r.command(CommandDef {
        id: USE_PAW,
        label: "Paw",
        cli: Some("paw"),
        surfaces: Surfaces::PICKER.with(Surfaces::CLI),
        group: "tools",
        order: 0,
        shortcut: None,
        available: |_| Ok(()),
        handler: |_| Outcome::Set(STYLE, Value::Choice("paw")),
        on_release: None,
    });
    r.command(CommandDef {
        id: USE_LASER,
        label: "Laser",
        cli: Some("laser"),
        surfaces: Surfaces::PICKER.with(Surfaces::CLI),
        group: "tools",
        order: 1,
        shortcut: None,
        available: |_| Ok(()),
        handler: |_| Outcome::Set(STYLE, Value::Choice("laser")),
        on_release: None,
    });
    r.setting(SettingDef {
        id: STYLE,
        label: "Pointer",
        group: "Appearance",
        order: 0,
        default: Value::Choice("paw"),
        constraint: Constraint::Choices(&["paw", "laser"]),
    });
    r.setting(SettingDef {
        id: PRESENTATION,
        label: "Presentation",
        group: "Appearance",
        order: 1,
        default: Value::Choice("furry"),
        // "stretch" joins once a pack supplies forearm art.
        constraint: Constraint::Choices(&["furry", "stick"]),
    });
    r.setting(SettingDef {
        id: SIZE,
        label: "Size",
        group: "Appearance",
        order: 2,
        default: Value::Float(BASE_SIZE as f64),
        constraint: Constraint::Range { min: 32.0, max: 96.0 },
    });
}

pub fn new_state(_: &SettingValues) -> Box<dyn ToolState> {
    Box::new(PointTool::default())
}

#[derive(Debug, Default)]
enum Gesture {
    #[default]
    Idle,
    Pressed {
        output: OutputId,
        start: Pt,
    },
    Drawing(Mark),
}

#[derive(Debug, Default)]
pub struct PointTool {
    gesture: Gesture,
    tap_at: Option<Millis>,
    grip: Option<Grip>,
}

/// Exponential approach of the stick grip toward the latest pointer x,
/// evaluated from elapsed time so it looks the same at 60 and 160 Hz.
#[derive(Debug, Clone, Copy)]
struct Grip {
    from: f32,
    target: f32,
    since: Millis,
}

impl Grip {
    fn at(&self, now: Millis) -> f32 {
        let t = now.saturating_sub(self.since) as f32 / GRIP_TAU_MS;
        self.target + (self.from - self.target) * (-t).exp()
    }

    fn settled(&self, now: Millis) -> bool {
        (self.at(now) - self.target).abs() < GRIP_SETTLED
    }

    /// Retargets from wherever the grip is now; the first sample snaps.
    fn follow(grip: &mut Option<Grip>, x: f32, now: Millis) {
        let from = grip.map_or(x, |g| g.at(now));
        *grip = Some(Grip { from, target: x, since: now });
    }
}

fn new_stroke(output: OutputId, points: Vec<Pt>, settings: &SettingValues) -> Mark {
    Mark::Stroke(Stroke {
        output,
        color: Rgba(settings.color(INK_COLOR)),
        width: settings.float(INK_WIDTH) as f32,
        points,
        born: 0,
    })
}

impl PointTool {
    fn commit(&mut self, cx: &mut ToolCx<'_>) {
        if let Gesture::Drawing(mut mark) = std::mem::take(&mut self.gesture) {
            mark.set_born(cx.now);
            cx.scene.push(mark);
        }
    }
}

impl ToolState for PointTool {
    fn pointer(&mut self, ev: &PointerEvent, cx: &mut ToolCx<'_>) {
        // Only the stick has a grip; other presentations never pay for frames.
        let stick = cx.settings.choice(STYLE) == "paw"
            && cx.settings.choice(PRESENTATION) == "stick";
        match ev.kind {
            PointerKind::Left => self.grip = None,
            _ if stick => Grip::follow(&mut self.grip, ev.pos.x, cx.now),
            _ => self.grip = None,
        }
        match ev.kind {
            PointerKind::Down => {
                if matches!(self.gesture, Gesture::Idle) {
                    self.gesture = Gesture::Pressed {
                        output: ev.output,
                        start: ev.pos,
                    };
                }
            }
            PointerKind::Moved => match &mut self.gesture {
                Gesture::Idle => {}
                Gesture::Pressed { output, start } => {
                    if *output == ev.output && start.distance(ev.pos) > DRAG_THRESHOLD {
                        // The stroke starts at the original press.
                        self.gesture = Gesture::Drawing(new_stroke(
                            *output,
                            vec![*start, ev.pos],
                            cx.settings,
                        ));
                    }
                }
                Gesture::Drawing(Mark::Stroke(stroke)) => {
                    if stroke.output != ev.output {
                        // End at the display boundary, continue on the new target.
                        self.commit(cx);
                        self.gesture =
                            Gesture::Drawing(new_stroke(ev.output, vec![ev.pos], cx.settings));
                    } else if stroke
                        .points
                        .last()
                        .is_none_or(|last| last.distance(ev.pos) >= MIN_SPACING)
                    {
                        stroke.points.push(ev.pos);
                    }
                }
            },
            PointerKind::Up => match std::mem::take(&mut self.gesture) {
                Gesture::Idle => {}
                Gesture::Pressed { .. } => {
                    self.tap_at = Some(cx.now);
                    cx.effects.push(Effect::PlayTap);
                }
                Gesture::Drawing(mut mark) => {
                    let Mark::Stroke(stroke) = &mut mark;
                    if stroke.output == ev.output
                        && stroke.points.last().is_some_and(|l| *l != ev.pos)
                    {
                        stroke.points.push(ev.pos);
                    }
                    mark.set_born(cx.now);
                    cx.scene.push(mark);
                }
            },
            PointerKind::Left => self.finish(cx),
        }
    }

    fn finish(&mut self, cx: &mut ToolCx<'_>) {
        match self.gesture {
            Gesture::Drawing(_) => self.commit(cx),
            _ => self.gesture = Gesture::Idle,
        }
    }

    fn draft(&self) -> Option<&Mark> {
        match &self.gesture {
            Gesture::Drawing(mark) => Some(mark),
            _ => None,
        }
    }

    fn button_down(&self) -> bool {
        !matches!(self.gesture, Gesture::Idle)
    }

    fn tip(&self, at: Pt, now: Millis, settings: &SettingValues) -> Option<Tip> {
        let press = match (&self.gesture, self.tap_at) {
            (Gesture::Pressed { .. }, _) => 1.0,
            (_, Some(t)) if now.saturating_sub(t) < TAP_MS => {
                // Release half of the press: 1.0 back to 0.0 over TAP_MS.
                1.0 - now.saturating_sub(t) as f32 / TAP_MS as f32
            }
            _ => 0.0,
        };
        Some(match settings.choice(STYLE) {
            "laser" => Tip::Laser { at, press },
            _ => Tip::Paw {
                at,
                press,
                scale: settings.float(SIZE) as f32 / BASE_SIZE,
                stick: settings.choice(PRESENTATION) == "stick",
                grip_x: self.grip.map_or(at.x, |g| g.at(now)),
            },
        })
    }

    fn animating(&self, now: Millis) -> bool {
        self.tap_at.is_some_and(|t| now.saturating_sub(t) < TAP_MS)
            || self.grip.is_some_and(|g| !g.settled(now))
    }
}
