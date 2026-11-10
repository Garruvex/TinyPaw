//! Overlay layers: committed ink (cached), draft stroke, tip, then controls.

mod paw;

use std::cell::Cell;

use iced::mouse;
use iced::widget::canvas::{self, Cache, Frame, Geometry, LineCap, LineJoin, Path, Stroke};
use iced::widget::{button, canvas as canvas_widget, container, mouse_area, row, stack, text};
use iced::{Alignment, Color, Element, Length, Point, Rectangle, Renderer, Theme};

use crate::app::Message;
use crate::registry::{Registry, Surfaces};
use crate::session::{Mark, Millis, OutputId, PointerKind, Session};

/// Cached committed ink, invalidated by scene revision.
#[derive(Default)]
pub struct InkCache {
    cache: Cache,
    revision: Cell<Option<u64>>,
}

pub fn overlay<'a>(
    registry: &'a Registry,
    session: &'a Session,
    ink: &'a InkCache,
    output: OutputId,
    now: Millis,
) -> Element<'a, Message> {
    let scene = canvas_widget(OverlayCanvas {
        session,
        ink,
        output,
        now,
    })
    .width(Length::Fill)
    .height(Length::Fill);

    if !session.is_pointing() {
        return scene.into();
    }

    stack![scene, toolbar(registry, session)].into()
}

fn toolbar<'a>(registry: &'a Registry, session: &'a Session) -> Element<'a, Message> {
    let buttons = registry
        .commands_for(Surfaces::TOOLBAR)
        .into_iter()
        .map(|def| {
            let enabled = (def.available)(session).is_ok();
            button(text(def.label).size(14))
                .padding([6, 14])
                .style(button::secondary)
                .on_press_maybe(enabled.then_some(Message::Command(def.id)))
                .into()
        });
    let strip = container(row(buttons).spacing(6))
        .padding(6)
        .style(|_: &Theme| container::Style {
            background: Some(Color::from_rgba8(0x16, 0x16, 0x1A, 0.88).into()),
            border: iced::border::rounded(10),
            ..container::Style::default()
        });
    // The strip owns its whole area so control clicks never start a stroke.
    let strip = mouse_area(strip).interaction(mouse::Interaction::Idle);
    container(strip)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::End)
        .padding(24)
        .into()
}

struct OverlayCanvas<'a> {
    session: &'a Session,
    ink: &'a InkCache,
    output: OutputId,
    now: Millis,
}

#[derive(Default)]
struct CanvasState {
    inside: bool,
    last: Option<Point>,
}

fn stroke_style(color: [u8; 4], width: f32) -> Stroke<'static> {
    Stroke::default()
        .with_color(Color::from_rgba8(color[0], color[1], color[2], color[3] as f32 / 255.0))
        .with_width(width)
        .with_line_cap(LineCap::Round)
        .with_line_join(LineJoin::Round)
}

fn draw_mark(frame: &mut Frame, mark: &Mark) {
    let Mark::Stroke(s) = mark;
    match s.points.as_slice() {
        [] => {}
        [p] => {
            let c = s.color.0;
            frame.fill(
                &Path::circle(Point::new(p.x, p.y), s.width / 2.0),
                Color::from_rgba8(c[0], c[1], c[2], c[3] as f32 / 255.0),
            );
        }
        [first, rest @ ..] => {
            let path = Path::new(|b| {
                b.move_to(Point::new(first.x, first.y));
                for p in rest {
                    b.line_to(Point::new(p.x, p.y));
                }
            });
            frame.stroke(&path, stroke_style(s.color.0, s.width));
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
        if self.ink.revision.get() != Some(scene.revision()) {
            self.ink.cache.clear();
            self.ink.revision.set(Some(scene.revision()));
        }
        let committed = self.ink.cache.draw(renderer, bounds.size(), |frame| {
            for mark in scene.marks().iter().filter(|m| m.output() == self.output) {
                draw_mark(frame, mark);
            }
        });

        let mut live = Frame::new(renderer, bounds.size());
        if let Some(draft) = self.session.draft().filter(|m| m.output() == self.output) {
            draw_mark(&mut live, draft);
        }
        if let Some((output, tip)) = self.session.tip(self.now)
            && output == self.output
        {
            paw::draw_tip(&mut live, tip);
        }
        vec![committed, live.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _state: &CanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if self.session.is_pointing() && cursor.is_over(bounds) {
            // The paw is drawn in the scene so the stream shows it even when
            // cursor capture is disabled.
            mouse::Interaction::Hidden
        } else {
            mouse::Interaction::None
        }
    }
}
