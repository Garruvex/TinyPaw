//! Quick wheel: eight tools around the pointer. Hover follows the pointer's
//! angle from the centre, so a flick in a direction is enough.

use iced::alignment;
use iced::widget::canvas::{self, Frame, Path, Stroke};
use iced::widget::text::Alignment;
use iced::{Color, Point, Rectangle, Size};

use super::icons::{self, Glyph};
use super::theme;
use crate::features::{point, system, upcoming};

pub struct Item {
    pub glyph: Glyph,
    pub label: &'static str,
    pub hint: &'static str,
    pub command: &'static str,
}

pub const ITEMS: [Item; 8] = [
    Item { glyph: Glyph::Paw, label: "Paw", hint: "Tap and draw", command: point::USE_PAW },
    Item { glyph: Glyph::Laser, label: "Laser", hint: "Precise dot", command: point::USE_LASER },
    Item { glyph: Glyph::Text, label: "Text", hint: "Coming soon", command: upcoming::TEXT },
    Item { glyph: Glyph::Pin, label: "Pin", hint: "Coming soon", command: upcoming::PIN },
    Item { glyph: Glyph::Spotlight, label: "Spotlight", hint: "Coming soon", command: upcoming::SPOTLIGHT },
    Item { glyph: Glyph::Lens, label: "Lens", hint: "Coming soon", command: upcoming::LENS },
    Item { glyph: Glyph::Freeze, label: "Freeze", hint: "Coming soon", command: upcoming::FREEZE },
    Item { glyph: Glyph::Clear, label: "Clear", hint: "Remove all marks", command: system::CLEAR },
];

const OUTER: f32 = 210.0;
const INNER: f32 = 80.0;
const RING: f32 = 150.0;
const ITEM: f32 = 38.0;

#[derive(Debug, Clone, Copy, Default)]
pub struct WheelState {
    /// Fixed where the wheel opened; set by the first pointer report if unknown.
    pub center: Option<Point>,
    pub hover: Option<usize>,
}

impl WheelState {
    pub fn track(&mut self, p: Point) {
        let c = *self.center.get_or_insert(p);
        let (dx, dy) = (p.x - c.x, p.y - c.y);
        let dist = (dx * dx + dy * dy).sqrt();
        self.hover = if dist < INNER * 0.6 {
            None
        } else {
            let deg = dy.atan2(dx).to_degrees() + 90.0;
            Some(((deg / 45.0).round() as i32).rem_euclid(8) as usize)
        };
    }
}

fn item_center(c: Point, i: usize) -> Point {
    let a = (-90.0 + i as f32 * 45.0).to_radians();
    Point::new(c.x + RING * a.cos(), c.y + RING * a.sin())
}

fn label(frame: &mut Frame, content: &str, at: Point, size: f32, color: Color, font: iced::Font) {
    frame.fill_text(canvas::Text {
        content: content.to_owned(),
        position: at,
        color,
        size: size.into(),
        font,
        align_x: Alignment::Center,
        align_y: alignment::Vertical::Center,
        ..canvas::Text::default()
    });
}

/// Draws the wheel; `available[i]` dims items that cannot run yet.
pub fn draw(frame: &mut Frame, bounds: Rectangle, state: &WheelState, available: &[bool; 8]) {
    let size = bounds.size();
    frame.fill_rectangle(Point::ORIGIN, size, Color::from_rgba8(8, 10, 12, 0.42));
    let c = state
        .center
        .unwrap_or(Point::new(size.width / 2.0, size.height / 2.0));
    // Keep the whole wheel on screen.
    let c = Point::new(
        c.x.clamp(OUTER + 8.0, (size.width - OUTER - 8.0).max(OUTER + 8.0)),
        c.y.clamp(OUTER + 8.0, (size.height - OUTER - 8.0).max(OUTER + 8.0)),
    );

    let disc = Path::circle(c, OUTER);
    frame.fill(&Path::circle(Point::new(c.x, c.y + 10.0), OUTER), Color::from_rgba8(0, 0, 0, 0.3));
    frame.fill(&disc, theme::SURFACE_A);
    frame.stroke(&disc, Stroke::default().with_color(theme::LINE).with_width(1.0));
    let hole = Path::circle(c, INNER);
    frame.fill(&hole, theme::WELL);
    frame.stroke(&hole, Stroke::default().with_color(theme::LINE).with_width(1.0));

    for (i, item) in ITEMS.iter().enumerate() {
        let p = item_center(c, i);
        let hovered = state.hover == Some(i);
        let color = match (available[i], hovered) {
            (false, true) => theme::MUTED,
            (false, false) => theme::DISABLED,
            (true, true) => theme::CYAN_TEXT,
            (true, false) => Color::from_rgb8(0xD1, 0xD5, 0xDB),
        };
        if hovered {
            let ring = Path::circle(p, ITEM);
            let bg = if available[i] { theme::CYAN_BG } else { Color::from_rgba8(255, 255, 255, 0.06) };
            frame.fill(&ring, bg);
            if available[i] {
                frame.stroke(&ring, Stroke::default().with_color(theme::CYAN_RING).with_width(1.5));
            }
        }
        frame.draw_svg(
            Rectangle::new(Point::new(p.x - 12.0, p.y - 20.0), Size::new(24.0, 24.0)),
            iced::advanced::svg::Svg::new(icons::handle(item.glyph)).color(color),
        );
        label(frame, item.label, Point::new(p.x, p.y + 14.0), 12.0, color, theme::MEDIUM);
    }

    let (title, hint) = match state.hover {
        Some(i) => (
            ITEMS[i].label,
            if available[i] { ITEMS[i].hint } else { "Coming soon" },
        ),
        None => ("Pick a tool", "Point, then release"),
    };
    label(frame, title, Point::new(c.x, c.y - 9.0), 20.0, theme::TEXT, theme::HEADING);
    label(frame, hint, Point::new(c.x, c.y + 16.0), 12.0, theme::MUTED, theme::BODY);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_follows_direction_from_centre() {
        let mut w = WheelState::default();
        w.track(Point::new(500.0, 500.0)); // sets the centre
        assert_eq!(w.hover, None);
        w.track(Point::new(500.0, 400.0));
        assert_eq!(w.hover, Some(0), "up is the paw");
        w.track(Point::new(600.0, 500.0));
        assert_eq!(w.hover, Some(2), "right is text");
        w.track(Point::new(430.0, 430.0));
        assert_eq!(w.hover, Some(7), "up-left is clear");
        w.track(Point::new(505.0, 505.0));
        assert_eq!(w.hover, None, "back in the centre cancels");
    }
}
