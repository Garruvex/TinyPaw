//! Overlay layers back to front: committed ink, draft, tip or wheel, then the
//! status chip and quick bar. Settings and first-run windows live in their own
//! modules and share the Graphite tokens.

pub mod first_run;
pub mod icons;
mod paw;
pub mod settings;
pub mod theme;
pub mod wheel;

use std::cell::Cell;

use iced::mouse;
use iced::widget::canvas::{self, Cache, Frame, Geometry, LineCap, LineJoin, Path, Stroke};
use iced::widget::{
    button, canvas as canvas_widget, column, container, mouse_area, row, space, stack, text,
    tooltip,
};
use iced::{Alignment, Color, Element, Length, Point, Rectangle, Renderer, Theme};

use crate::app::Message;
use crate::features::{display, point, sound, system, upcoming};
use crate::registry::{Registry, Value};
use crate::session::{Mark, Millis, OutputId, PointerKind, Session};
use icons::{Glyph, icon};
use theme::Tone;

/// Overlay-only UI state owned by the app (not session state).
#[derive(Debug, Default)]
pub struct OverlayUi {
    pub pointer_menu: bool,
    pub ink_menu: bool,
    pub wheel: Option<wheel::WheelState>,
}

impl OverlayUi {
    pub fn close_menus(&mut self) -> bool {
        let was = self.pointer_menu || self.ink_menu;
        self.pointer_menu = false;
        self.ink_menu = false;
        was
    }
}

#[derive(Debug, Clone)]
pub enum UiMsg {
    TogglePointerMenu,
    ToggleInkMenu,
    Nav(settings::Page),
    ImportWav,
    PreviewSound,
    TurnOn,
    OpenSettings,
}

/// Cached committed ink, invalidated by scene revision.
#[derive(Default)]
pub struct InkCache {
    cache: Cache,
    revision: Cell<Option<u64>>,
}

pub struct Overlay<'a> {
    pub registry: &'a Registry,
    pub session: &'a Session,
    pub ink: &'a InkCache,
    pub ui: &'a OverlayUi,
    pub output: OutputId,
    pub now: Millis,
}

fn available(registry: &Registry, session: &Session, id: &str) -> bool {
    registry
        .command(id)
        .is_some_and(|d| (d.available)(session).is_ok())
}

pub fn overlay(o: Overlay<'_>) -> Element<'_, Message> {
    let wheel_available: [bool; 8] =
        std::array::from_fn(|i| available(o.registry, o.session, wheel::ITEMS[i].command));
    let scene = canvas_widget(OverlayCanvas {
        session: o.session,
        ink: o.ink,
        wheel: o.ui.wheel,
        wheel_available,
        output: o.output,
        now: o.now,
    })
    .width(Length::Fill)
    .height(Length::Fill);

    let hidden = o.session.settings().bool(display::HIDE_BAR);
    if !o.session.is_pointing() || o.ui.wheel.is_some() || hidden {
        return scene.into();
    }

    let chip = container(status_chip())
        .width(Length::Fill)
        .align_x(Alignment::Center)
        .padding(20);

    let mut controls = column![].spacing(8).align_x(Alignment::Center);
    if o.ui.pointer_menu {
        controls = controls.push(pointer_menu(o.session));
    }
    if o.ui.ink_menu {
        controls = controls.push(ink_menu(o.session));
    }
    controls = controls.push(quick_bar(o.registry, o.session));
    let bar = container(controls)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::End)
        .padding(28);

    stack![scene, chip, bar].into()
}

fn status_chip<'a>() -> Element<'a, Message> {
    let dot = container(space())
        .width(8)
        .height(8)
        .style(theme::fill(theme::AMBER, 4.0));
    let content = row![
        dot,
        text("Annotating").font(theme::MEDIUM).size(13).color(theme::TEXT),
        text("·").size(13).color(theme::FAINT),
        text("Esc to exit").size(13).color(theme::SUBTLE),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    container(content)
        .padding([7, 14])
        .style(theme::panel(999.0))
        .into()
}

fn keycap<'a>(label: String) -> Element<'a, Message> {
    container(text(label).size(11).color(theme::SUBTLE))
        .padding([1, 6])
        .style(theme::keycap)
        .into()
}

/// Wraps a control in a tooltip showing its name and shortcut.
fn tip<'a>(
    content: impl Into<Element<'a, Message>>,
    label: String,
    shortcut: Option<String>,
) -> Element<'a, Message> {
    let mut body = row![text(label).size(12).color(theme::TEXT)]
        .spacing(8)
        .align_y(Alignment::Center);
    if let Some(s) = shortcut {
        body = body.push(keycap(s.replace('+', " ")));
    }
    tooltip(
        content,
        container(body)
            .padding([6, 10])
            .style(theme::fill(theme::HOVER, 8.0)),
        tooltip::Position::Top,
    )
    .gap(8)
    .into()
}

fn icon_button<'a>(glyph: Glyph, tone: Tone, msg: Option<Message>) -> button::Button<'a, Message> {
    let color = match tone {
        Tone::Selected => theme::CYAN_TEXT,
        Tone::Warm => theme::AMBER_TEXT,
        _ if msg.is_none() => theme::DISABLED,
        _ => theme::TEXT_SOFT,
    };
    button(
        container(icon(glyph, 20.0, color))
            .width(44)
            .height(44)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center),
    )
    .padding(0)
    .style(theme::tool_button(tone))
    .on_press_maybe(msg)
}

fn divider<'a>() -> Element<'a, Message> {
    container(space())
        .width(1)
        .height(24)
        .style(theme::fill(theme::LINE_STRONG, 0.0))
        .into()
}

fn command_button<'a>(
    registry: &Registry,
    session: &Session,
    id: &'static str,
    glyph: Glyph,
    tone: Tone,
) -> Element<'a, Message> {
    let def = registry.command(id);
    let reason = def.and_then(|d| (d.available)(session).err());
    let msg = reason.is_none().then_some(Message::Command(id));
    let mut label = def.map_or(id, |d| d.label).to_owned();
    if let Some(why) = reason {
        label = format!("{label} · {why}");
    }
    tip(icon_button(glyph, tone, msg), label, registry.shortcut_label(id))
}

fn quick_bar<'a>(registry: &'a Registry, session: &'a Session) -> Element<'a, Message> {
    let laser = session.settings().choice(point::STYLE) == "laser";
    let pointer = button(
        row![
            icon(if laser { Glyph::Laser } else { Glyph::Paw }, 20.0, theme::CYAN_TEXT),
            icon(Glyph::ChevronUp, 14.0, theme::CYAN_TEXT),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .height(44)
    .padding([12, 10])
    .style(theme::tool_button(Tone::Selected))
    .on_press(Message::Ui(UiMsg::TogglePointerMenu));
    let pointer = tip(pointer, "Point: paw or laser".into(), None);

    let color = theme::rgba(session.settings().color(system::INK_COLOR));
    let width = session.settings().float(system::INK_WIDTH);
    let ink = button(
        row![
            container(space())
                .width(18)
                .height(18)
                .style(theme::outlined(color, 9.0, Color::from_rgba8(255, 255, 255, 0.18))),
            text(format!("{width:.0} px")).size(13).color(theme::SUBTLE),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .height(44)
    .padding([12, 10])
    .style(theme::tool_button(Tone::Plain))
    .on_press(Message::Ui(UiMsg::ToggleInkMenu));
    let ink = tip(ink, "Ink colour and width".into(), None);

    let muted = session.settings().bool(sound::MUTE);
    let mute = tip(
        icon_button(
            if muted { Glyph::Mute } else { Glyph::Sound },
            if muted { Tone::Warm } else { Tone::Plain },
            Some(Message::Command(sound::TOGGLE_MUTE)),
        ),
        if muted { "Unmute tap sound" } else { "Mute tap sound" }.into(),
        None,
    );

    let exit = button(
        row![
            icon(Glyph::Close, 18.0, theme::TEXT_SOFT),
            text("Exit").font(theme::MEDIUM).size(14),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .height(44)
    .padding([12, 14])
    .style(theme::tool_button(Tone::Exit))
    .on_press(Message::Command(system::EXIT));
    let exit = tip(exit, "Exit".into(), registry.shortcut_label(system::EXIT));

    let bar = row![
        pointer,
        command_button(registry, session, upcoming::TEXT, Glyph::Text, Tone::Plain),
        command_button(registry, session, upcoming::PIN, Glyph::Pin, Tone::Plain),
        command_button(registry, session, upcoming::SPOTLIGHT, Glyph::Spotlight, Tone::Plain),
        command_button(registry, session, upcoming::LENS, Glyph::Lens, Tone::Plain),
        divider(),
        ink,
        divider(),
        command_button(registry, session, system::UNDO, Glyph::Undo, Tone::Plain),
        command_button(registry, session, system::CLEAR, Glyph::Clear, Tone::Plain),
        mute,
        divider(),
        exit,
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    // The bar owns its whole area so clicks between buttons never draw.
    mouse_area(container(bar).padding(6).style(theme::panel(16.0)))
        .interaction(mouse::Interaction::Idle)
        .into()
}

fn pointer_menu<'a>(session: &Session) -> Element<'a, Message> {
    let laser = session.settings().choice(point::STYLE) == "laser";
    let choice = |glyph, label: &'static str, on: bool, id: &'static str| {
        button(
            row![
                icon(glyph, 20.0, if on { theme::CYAN_TEXT } else { theme::TEXT_SOFT }),
                text(label).font(theme::MEDIUM).size(14),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .height(44)
        .padding([12, 14])
        .style(theme::tool_button(if on { Tone::Selected } else { Tone::Plain }))
        .on_press(Message::Command(id))
    };
    mouse_area(
        container(
            row![
                choice(Glyph::Paw, "Yohta", !laser, point::USE_PAW),
                choice(Glyph::Laser, "Laser", laser, point::USE_LASER),
            ]
            .spacing(4),
        )
        .padding(6)
        .style(theme::panel(12.0)),
    )
    .interaction(mouse::Interaction::Idle)
    .into()
}

fn ink_menu<'a>(session: &Session) -> Element<'a, Message> {
    let current = session.settings().color(system::INK_COLOR);
    let width = session.settings().float(system::INK_WIDTH);
    let swatches = row(theme::INKS.iter().map(|&(_, c)| {
        button(space().width(28).height(28))
            .padding(0)
            .style(theme::swatch(theme::rgba(c), c == current))
            .on_press(Message::Set(system::INK_COLOR, Value::Color(c)))
            .into()
    }))
    .spacing(8);
    let widths = row(theme::WIDTHS.iter().map(|&w| {
        button(text(format!("{w:.0} px")).size(13))
            .padding([7, 12])
            .style(theme::segment((w - width).abs() < 0.01))
            .on_press(Message::Set(system::INK_WIDTH, Value::Float(w)))
            .into()
    }))
    .spacing(4);
    mouse_area(
        container(
            column![
                swatches,
                container(widths).padding(4).style(theme::fill(theme::WELL, 10.0)),
            ]
            .spacing(10)
            .align_x(Alignment::Center),
        )
        .padding(10)
        .style(theme::panel(12.0)),
    )
    .interaction(mouse::Interaction::Idle)
    .into()
}

struct OverlayCanvas<'a> {
    session: &'a Session,
    ink: &'a InkCache,
    wheel: Option<wheel::WheelState>,
    wheel_available: [bool; 8],
    output: OutputId,
    now: Millis,
}

#[derive(Default)]
struct CanvasState {
    inside: bool,
    last: Option<Point>,
}

fn stroke_style(color: Color, width: f32) -> Stroke<'static> {
    Stroke::default()
        .with_color(color)
        .with_width(width)
        .with_line_cap(LineCap::Round)
        .with_line_join(LineJoin::Round)
}

fn draw_mark(frame: &mut Frame, mark: &Mark, alpha: f32) {
    let Mark::Stroke(s) = mark;
    let mut color = theme::rgba(s.color.0);
    color.a *= alpha;
    match s.points.as_slice() {
        [] => {}
        [p] => frame.fill(&Path::circle(Point::new(p.x, p.y), s.width / 2.0), color),
        [first, rest @ ..] => {
            let path = Path::new(|b| {
                b.move_to(Point::new(first.x, first.y));
                for p in rest {
                    b.line_to(Point::new(p.x, p.y));
                }
            });
            frame.stroke(&path, stroke_style(color, s.width));
        }
    }
}

impl canvas::Program<Message> for OverlayCanvas<'_> {
    type State = CanvasState;

    fn update(
        &self,
        state: &mut CanvasState,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let iced::Event::Mouse(event) = event else {
            return None;
        };
        let pos = cursor.position_in(bounds);
        let msg = match (event, pos) {
            (mouse::Event::CursorMoved { .. }, Some(p)) => {
                if state.inside && state.last == Some(p) {
                    return None; // Coalesce repeated positions.
                }
                state.inside = true;
                state.last = Some(p);
                Message::Pointer(PointerKind::Moved, p)
            }
            (mouse::Event::CursorMoved { .. } | mouse::Event::CursorLeft, None) if state.inside => {
                state.inside = false;
                Message::Pointer(PointerKind::Left, state.last.unwrap_or(Point::ORIGIN))
            }
            (mouse::Event::ButtonPressed(mouse::Button::Left), Some(p)) => {
                state.last = Some(p);
                Message::Pointer(PointerKind::Down, p)
            }
            // Always report the release so a swallowed button-up is observed.
            (mouse::Event::ButtonReleased(mouse::Button::Left), p) => {
                let p = p.or(state.last).unwrap_or(Point::ORIGIN);
                Message::Pointer(PointerKind::Up, p)
            }
            _ => return None,
        };
        Some(canvas::Action::publish(msg).and_capture())
    }

    fn draw(
        &self,
        _state: &CanvasState,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let scene = self.session.scene();
        let mine = |m: &&Mark| m.output() == self.output;
        let mut layers = Vec::with_capacity(2);
        let mut live = Frame::new(renderer, bounds.size());

        if self.session.fading() {
            // Fading marks change every frame; skip the cache while fading.
            for mark in scene.marks().iter().filter(mine) {
                draw_mark(&mut live, mark, self.session.mark_alpha(mark, self.now));
            }
        } else {
            if self.ink.revision.get() != Some(scene.revision()) {
                self.ink.cache.clear();
                self.ink.revision.set(Some(scene.revision()));
            }
            layers.push(self.ink.cache.draw(renderer, bounds.size(), |frame| {
                for mark in scene.marks().iter().filter(mine) {
                    draw_mark(frame, mark, 1.0);
                }
            }));
        }

        if let Some(draft) = self.session.draft().filter(|m| m.output() == self.output) {
            draw_mark(&mut live, draft, 1.0);
        }
        match &self.wheel {
            Some(state) => wheel::draw(&mut live, bounds, state, &self.wheel_available),
            None => {
                if let Some((output, tip)) = self.session.tip(self.now)
                    && output == self.output
                {
                    paw::draw_tip(&mut live, tip, bounds.size());
                }
            }
        }
        layers.push(live.into_geometry());
        layers
    }

    fn mouse_interaction(
        &self,
        _state: &CanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if self.wheel.is_some() {
            mouse::Interaction::Idle
        } else if self.session.is_pointing() && cursor.is_over(bounds) {
            // The paw is drawn in the scene so the stream shows it even when
            // cursor capture is disabled.
            mouse::Interaction::Hidden
        } else {
            mouse::Interaction::None
        }
    }
}

/// Preview tile shared by settings: one stroke and the pointer on a backdrop.
pub struct PreviewTile {
    pub background: Color,
    pub ink: Color,
    pub width: f32,
    pub laser: bool,
}

impl<Message> canvas::Program<Message> for PreviewTile {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let size = bounds.size();
        frame.fill(
            &Path::rounded_rectangle(Point::ORIGIN, size, 12.0.into()),
            self.background,
        );
        let (w, h) = (size.width, size.height);
        let curve = Path::new(|b| {
            b.move_to(Point::new(w * 0.15, h * 0.68));
            b.bezier_curve_to(
                Point::new(w * 0.27, h * 0.25),
                Point::new(w * 0.6, h * 0.2),
                Point::new(w * 0.72, h * 0.47),
            );
        });
        frame.stroke(&curve, stroke_style(self.ink, self.width));
        let tip = Point::new(w * 0.74, h * 0.5);
        if self.laser {
            paw::draw_tip(
                &mut frame,
                crate::session::Tip::Laser {
                    at: crate::session::Pt::new(tip.x, tip.y),
                    press: 0.0,
                },
                size,
            );
        } else {
            paw::draw_preview_paw(&mut frame, tip, h * 0.42);
        }
        vec![frame.into_geometry()]
    }
}
