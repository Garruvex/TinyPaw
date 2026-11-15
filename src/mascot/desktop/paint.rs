//! The speech bubble and the menu, drawn into a pixel buffer for the stage.
//! They match what the Iced view draws today (geometry, colours, pop and fade
//! timing); the Iced version goes when the app moves to the stage.

use iced::{Color, Point, Rectangle, Size, Vector};

use super::*;
use crate::mascot::surface::{Raster, Surface, round_rect_path};
use crate::mascot::typeset::{Face, IconCache, Style, Typeset};

/// Everything that decides what the stage draws, to skip frames where it did
/// not change.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneKey {
    pose: Pose,
    speech: Option<(String, usize)>,
    menu: Option<Side>,
    closing: bool,
    hover: Vec<f32>,
}

impl SceneKey {
    /// Whether drawing `other` would give the same picture as this one.
    pub fn looks_like(&self, other: &SceneKey) -> bool {
        self.speech == other.speech
            && self.menu == other.menu
            && self.closing == other.closing
            && self.hover.len() == other.hover.len()
            && self
                .hover
                .iter()
                .zip(&other.hover)
                .all(|(a, b)| (a - b).abs() <= 0.01)
            && self.pose.looks_like(&other.pose)
    }
}

/// Quarter-circle corner from `p0` to `p2` around corner point `p1`.
fn corner(b: &mut tiny_skia::PathBuilder, p0: Point, p1: Point, p2: Point) {
    let k = 0.4477;
    b.cubic_to(
        p0.x + (p1.x - p0.x) * k,
        p0.y + (p1.y - p0.y) * k,
        p2.x + (p1.x - p2.x) * k,
        p2.y + (p1.y - p2.y) * k,
        p2.x,
        p2.y,
    );
}

/// The menu's speech-bubble outline and its tail's tip, as `menu_bubble`
/// builds them for Iced.
fn menu_path(r: Rectangle, side: Side) -> Option<(tiny_skia::Path, Point)> {
    let (x, y, w, h) = (r.x + 1.5, r.y + 1.5, r.width - 3.0, r.height - 3.0);
    let rad = 16.0f32.min(h / 3.0);
    let low = y + h - rad - 2.0;
    let high = low - 16.0;
    let tip = match side {
        Side::Right => Point::new(x - MENU_TAIL, low + 8.0),
        Side::Left => Point::new(x + w + MENU_TAIL, low + 8.0),
    };
    let mut b = tiny_skia::PathBuilder::new();
    b.move_to(x + rad, y);
    b.line_to(x + w - rad, y);
    corner(
        &mut b,
        Point::new(x + w - rad, y),
        Point::new(x + w, y),
        Point::new(x + w, y + rad),
    );
    if side == Side::Left {
        b.line_to(x + w, high);
        b.line_to(tip.x, tip.y);
        b.line_to(x + w, low);
    }
    b.line_to(x + w, y + h - rad);
    corner(
        &mut b,
        Point::new(x + w, y + h - rad),
        Point::new(x + w, y + h),
        Point::new(x + w - rad, y + h),
    );
    b.line_to(x + rad, y + h);
    corner(
        &mut b,
        Point::new(x + rad, y + h),
        Point::new(x, y + h),
        Point::new(x, y + h - rad),
    );
    if side == Side::Right {
        b.line_to(x, low);
        b.line_to(tip.x, tip.y);
        b.line_to(x, high);
    }
    b.line_to(x, y + rad);
    corner(
        &mut b,
        Point::new(x, y + rad),
        Point::new(x, y),
        Point::new(x + rad, y),
    );
    b.close();
    Some((b.finish()?, tip))
}

impl Desktop {
    /// A physical screen point in the window's logical pixels.
    fn local_point(&self, p: (i32, i32)) -> Option<Point> {
        let (rect, k) = self.window_scale()?;
        Some(Point::new(
            (p.0 - rect.0) as f32 / k,
            (p.1 - rect.1) as f32 / k,
        ))
    }

    /// Whether a physical screen point is on the open menu.
    pub fn over_menu_screen(&self, p: (i32, i32)) -> bool {
        let (Some(side), Some(local)) = (self.menu, self.local_point(p)) else {
            return false;
        };
        self.layout.menu(side).contains(local)
    }

    /// The menu row under a physical screen point.
    pub fn menu_row_at(&self, p: (i32, i32)) -> Option<usize> {
        let (side, local) = (self.menu?, self.local_point(p)?);
        let r = self.layout.menu(side);
        let row = ((local.y - r.y - MENU_PAD) / ROW_H).floor();
        (r.contains(local) && row >= 0.0 && (row as usize) < self.rows).then_some(row as usize)
    }

    /// Whether the left button is held on the character.
    pub fn pressing(&self) -> bool {
        self.press.is_some()
    }

    /// Whether anything about the scene moves without the pose changing:
    /// hearts rising, sleep marks drifting.
    pub fn has_overlay(&self) -> bool {
        self.companion.animates_overlay()
    }

    pub fn scene_key(&self, o: &Options) -> SceneKey {
        SceneKey {
            pose: self.pose.clone(),
            speech: self
                .speech
                .as_ref()
                .filter(|_| o.lines)
                .map(|s| (s.text().to_owned(), s.index)),
            menu: self.menu,
            closing: self.closing.is_some(),
            hover: self.hover.clone(),
        }
    }

    /// The open menu: its bubble (popping in or fading out) and its rows.
    pub fn paint_menu(
        &self,
        r: &mut Raster,
        text: &mut Typeset,
        icons: &mut IconCache,
        items: &[Item],
    ) {
        if let Some((side, scale, alpha)) = self.menu_look()
            && let Some((path, tip)) = menu_path(self.layout.menu(side), side)
        {
            r.with_save(|r| {
                r.translate(Vector::new(tip.x, tip.y));
                r.scale(scale);
                r.translate(Vector::new(-tip.x, -tip.y));
                r.with_save(|r| {
                    r.translate(Vector::new(0.0, 3.0));
                    r.fill_path(&path, Color::from_rgba(0.0, 0.0, 0.0, 0.28 * alpha));
                });
                r.fill_path(&path, Color::from_rgba8(0x1D, 0x20, 0x2C, 0.97 * alpha));
                r.stroke_path(&path, Color::from_rgba(1.0, 1.0, 1.0, 0.92 * alpha), 2.0);
            });
        }
        let Some(side) = self.menu else { return };
        let menu = self.layout.menu(side);
        for (i, item) in items.iter().enumerate().take(self.rows) {
            let row = Rectangle::new(
                Point::new(menu.x + MENU_PAD, menu.y + MENU_PAD + ROW_H * i as f32),
                Size::new(MENU_W - 2.0 * MENU_PAD, ROW_H),
            );
            let reveal = self.row_reveal(i);
            let enabled = item.on_press.is_some();
            let hover = if enabled {
                self.hover.get(i).copied().unwrap_or(0.0)
            } else {
                0.0
            };
            let (fg, glyph) = if enabled {
                (
                    mix(theme::TEXT_SOFT, Color::WHITE, hover),
                    mix(theme::TEXT_SOFT, theme::CYAN_TEXT, hover),
                )
            } else {
                (theme::DISABLED, theme::DISABLED)
            };
            if hover > 0.0 {
                r.fill_round_rect(
                    row,
                    10.0,
                    Color::from_rgba(1.0, 1.0, 1.0, 0.10 * hover * reveal),
                );
            }
            let left = row.x + 10.0 + (1.0 - reveal) * 10.0 + hover * 3.0;
            icons.draw(
                r,
                item.icon,
                Point::new(left, row.y + (ROW_H - 20.0) / 2.0),
                20.0,
                faded(glyph, reveal),
            );
            text.draw(
                r,
                item.label,
                Point::new(left + 20.0 + 12.0, row.y + (ROW_H - 15.0 * 1.3) / 2.0),
                Style {
                    size: 15.0,
                    face: Face::Semibold,
                    wrap: None,
                    color: faded(fg, reveal),
                },
            );
            if let Some(on) = item.checked {
                let pill = Rectangle::new(
                    Point::new(
                        row.x + row.width - 10.0 - 30.0,
                        row.y + (ROW_H - 18.0) / 2.0,
                    ),
                    Size::new(30.0, 18.0),
                );
                let track = match (on, enabled) {
                    (_, false) => theme::DISABLED,
                    (true, true) => theme::CYAN,
                    (false, true) => Color::from_rgb8(0x4A, 0x4E, 0x5A),
                };
                r.fill_round_rect(pill, 9.0, faded(track, reveal));
                let knob_x = if on {
                    pill.x + 30.0 - 3.0 - 12.0
                } else {
                    pill.x + 3.0
                };
                r.fill_round_rect(
                    Rectangle::new(Point::new(knob_x, pill.y + 3.0), Size::new(12.0, 12.0)),
                    6.0,
                    faded(Color::WHITE, reveal),
                );
            }
        }
    }

    /// The speech bubble and its tail.
    pub fn paint_speech(&self, r: &mut Raster, text: &mut Typeset, o: &Options) {
        let Some(speech) = self.speech.as_ref().filter(|_| o.lines) else {
            return;
        };
        let l = &self.layout;
        let b = self.lines.bubble;
        let place = speech_place(l, b, self.rig.visible_rows().0, self.menu);
        let width = match place {
            SpeechPlace::Above { width, .. } | SpeechPlace::Beside { width, .. } => width,
        };
        let (pop, alpha) = if self.reduced {
            (1.0, 1.0)
        } else {
            let t = (self.now - self.speech_at) / SPEECH_POP;
            (ease_out_back(t), (t * 3.0).min(1.0))
        };
        let lift = ((1.0 - pop) * 10.0).max(0.0);
        let fill = faded(
            Color::from_rgb8(b.background[0], b.background[1], b.background[2]),
            alpha,
        );
        let ink = faded(Color::from_rgb8(b.text[0], b.text[1], b.text[2]), alpha);
        let line = faded(ink, 0.28);

        let wrap = (width - 30.0).max(1.0);
        let (tw, th) = text.measure(speech.text(), 15.0, Face::Semibold, Some(wrap));
        let (bw, bh) = ((tw.ceil() + 30.0).min(width), th + 20.0);
        let grow = SPEECH_TAIL * pop.clamp(0.0, 1.0);
        let (rect, base, tip, normal) = match place {
            SpeechPlace::Beside {
                left,
                width,
                edge,
                at,
                side,
            } => {
                let bottom = (at.y + SIDE_DROP + lift).min(l.window.height).max(1.0);
                let x = match side {
                    Side::Left => left + width - bw,
                    Side::Right => left,
                };
                let base = Point::new(edge, at.y + lift);
                let (tip, normal) = match side {
                    Side::Left => (Point::new(edge + grow, base.y), Vector::new(1.0, 0.0)),
                    Side::Right => (Point::new(edge - grow, base.y), Vector::new(-1.0, 0.0)),
                };
                (
                    Rectangle::new(Point::new(x, bottom - bh), Size::new(bw, bh)),
                    base,
                    tip,
                    normal,
                )
            }
            SpeechPlace::Above { left, width, at } => {
                let bottom = (at.y - SPEECH_TAIL - lift).max(1.0);
                let centre = left + width / 2.0;
                let base = Point::new(centre, bottom);
                let tip = Point::new(at.x.clamp(centre - 12.0, centre + 12.0), base.y + grow);
                (
                    Rectangle::new(
                        Point::new(left + (width - bw) / 2.0, bottom - bh),
                        Size::new(bw, bh),
                    ),
                    base,
                    tip,
                    Vector::new(0.0, 1.0),
                )
            }
        };
        r.fill_round_rect(rect, 16.0, fill);
        let inset = Rectangle::new(
            Point::new(rect.x + 0.75, rect.y + 0.75),
            Size::new(rect.width - 1.5, rect.height - 1.5),
        );
        if let Some(border) = round_rect_path(inset, 15.25) {
            r.stroke_path(&border, line, 1.5);
        }
        text.draw(
            r,
            speech.text(),
            Point::new(rect.x + 15.0, rect.y + 10.0),
            Style {
                size: 15.0,
                face: Face::Semibold,
                wrap: Some(wrap),
                color: ink,
            },
        );
        // The tail covers the bubble's edge so the two read as one outline.
        let across = Vector::new(-normal.y, normal.x) * 8.0;
        let back = normal * -2.0;
        let (one, two) = (base - across, base + across);
        let mut shape = tiny_skia::PathBuilder::new();
        shape.move_to((one + back).x, (one + back).y);
        shape.line_to(tip.x, tip.y);
        shape.line_to((two + back).x, (two + back).y);
        shape.close();
        if let Some(path) = shape.finish() {
            r.fill_path(&path, fill);
        }
        let mut edges = tiny_skia::PathBuilder::new();
        edges.move_to(one.x, one.y);
        edges.line_to(tip.x, tip.y);
        edges.line_to(two.x, two.y);
        if let Some(path) = edges.finish() {
            r.stroke_path(&path, line, 1.5);
        }
    }
}
