//! Ink geometry: smoothed stroke paths and the tiled spotlight mask.
//!
//! Both are shaped for the renderer's damage tracking, which compares the
//! primitives of a live frame one by one and repaints only those that
//! changed. A drawing stroke is split into fixed runs of points so only its
//! growing tail differs between frames, and the spotlight dims the screen
//! with fixed tiles so only the tiles around the moving hole change.

use std::cell::RefCell;

use iced::advanced::image;
use iced::widget::canvas::fill::Rule;
use iced::widget::canvas::{self, Frame, Path, path::Builder};
use iced::{Color, Point, Rectangle, Size};

use crate::session::Pt;

/// How far one smoothing pass may move a point. Pointer positions arrive in
/// whole pixels, so slow strokes follow a pixel staircase; this is enough to
/// iron that out without flattening deliberate corners of a quick stroke.
const MAX_NUDGE: f32 = 0.9;
const SMOOTH_PASSES: usize = 2;
/// Curve segments per sub-path of a split stroke.
const RUN: usize = 48;

/// Removes pointer quantisation noise. Ends stay put so a stroke starts at
/// the press and ends at the release.
pub fn smooth(points: &[Pt]) -> Vec<Point> {
    let mut out: Vec<Point> = points.iter().map(|p| Point::new(p.x, p.y)).collect();
    if out.len() < 3 {
        return out;
    }
    let mut prev = out.clone();
    for _ in 0..SMOOTH_PASSES {
        prev.copy_from_slice(&out);
        for i in 1..out.len() - 1 {
            let (a, p, b) = (prev[i - 1], prev[i], prev[i + 1]);
            let avg = Point::new((a.x + 2.0 * p.x + b.x) / 4.0, (a.y + 2.0 * p.y + b.y) / 4.0);
            let (dx, dy) = (avg.x - p.x, avg.y - p.y);
            let len = (dx * dx + dy * dy).sqrt();
            let k = if len > MAX_NUDGE {
                MAX_NUDGE / len
            } else {
                1.0
            };
            out[i] = Point::new(p.x + dx * k, p.y + dy * k);
        }
    }
    out
}

/// Bézier control points for the segment `p1 -> p2` of a centripetal
/// Catmull-Rom spline. The curve passes through every sample, so quick
/// corners keep their tips, and the centripetal form does not overshoot or
/// loop where slow, dense samples meet fast, sparse ones.
fn controls(p0: Point, p1: Point, p2: Point, p3: Point) -> (Point, Point) {
    const EPS: f32 = 1e-4;
    // With alpha = 1/2, d^(2 alpha) is the plain distance.
    let (l1, l2, l3) = (p0.distance(p1), p1.distance(p2), p2.distance(p3));
    let (d1, d2, d3) = (l1.sqrt(), l2.sqrt(), l3.sqrt());
    let c1 = if d1 < EPS {
        p1
    } else {
        let k = 3.0 * d1 * (d1 + d2);
        let m = 2.0 * l1 + 3.0 * d1 * d2 + l2;
        Point::new(
            (l1 * p2.x - l2 * p0.x + m * p1.x) / k,
            (l1 * p2.y - l2 * p0.y + m * p1.y) / k,
        )
    };
    let c2 = if d3 < EPS {
        p2
    } else {
        let k = 3.0 * d3 * (d3 + d2);
        let m = 2.0 * l3 + 3.0 * d3 * d2 + l2;
        Point::new(
            (l3 * p1.x - l2 * p3.x + m * p2.x) / k,
            (l3 * p1.y - l2 * p3.y + m * p2.y) / k,
        )
    };
    (c1, c2)
}

/// Appends segments `from..to` (segment `i` joins `p[i]` and `p[i + 1]`) of
/// the smooth curve through `p`.
fn segments(b: &mut Builder, p: &[Point], from: usize, to: usize) {
    let last = p.len() - 1;
    b.move_to(p[from]);
    for i in from..to.min(last) {
        let p0 = p[i.saturating_sub(1)];
        let p3 = p[(i + 2).min(last)];
        let (c1, c2) = controls(p0, p[i], p[i + 1], p3);
        b.bezier_curve_to(c1, c2, p[i + 1]);
    }
}

/// The stroke as one path, or as consecutive runs (`split`) with local
/// bounds whose earlier members stay identical while points are added. Runs
/// overlap only at their shared end, which round caps hide for opaque ink.
pub fn stroke_paths(points: &[Pt], split: bool) -> Vec<Path> {
    let p = smooth(points);
    let segs = p.len().saturating_sub(1);
    if segs == 0 {
        return Vec::new();
    }
    if !split || segs <= RUN {
        return vec![Path::new(|b| segments(b, &p, 0, segs))];
    }
    (0..segs)
        .step_by(RUN)
        .map(|from| Path::new(|b| segments(b, &p, from, from + RUN)))
        .collect()
}

/// Edge of a spotlight tile. A multiple of 8 so tile edges land on whole
/// physical pixels at 100–200% display scaling and adjacent tiles meet
/// without anti-aliased seams.
const TILE: f32 = 128.0;
const SNAP: f32 = 8.0;
/// The feather image is rendered at this many pixels per logical pixel so it
/// stays smooth on high-DPI displays.
const FEATHER_DENSITY: f32 = 2.0;

fn rect(b: &mut Builder, r: Rectangle) {
    b.rectangle(r.position(), r.size());
}

/// Radial alpha ramp for the inside edge of the spotlight: clear at
/// `outer - feather`, rising smoothly to `dim` at `outer`, clear again
/// outside where the dimming geometry takes over. Cached because it only
/// changes when the size or darkness setting does.
type FeatherKey = (u32, u32, u32);

fn feather_image(outer: f32, feather: f32, dim: f32) -> image::Handle {
    thread_local! {
        static LAST: RefCell<Option<(FeatherKey, image::Handle)>> =
            const { RefCell::new(None) };
    }
    let key = (outer.to_bits(), feather.to_bits(), dim.to_bits());
    LAST.with_borrow_mut(|last| {
        if let Some((k, handle)) = last
            && *k == key
        {
            return handle.clone();
        }
        let side = (2.0 * outer * FEATHER_DENSITY).ceil().max(1.0) as u32;
        let c = side as f32 / 2.0;
        let inner = outer - feather;
        let alpha = |d: f32| -> f32 {
            if d >= outer || d <= inner {
                0.0
            } else {
                let t = (d - inner) / feather;
                dim * t * t * (3.0 - 2.0 * t)
            }
        };
        let mut rgba = vec![0u8; (side * side * 4) as usize];
        for y in 0..side {
            for x in 0..side {
                let dx = (x as f32 + 0.5 - c) / FEATHER_DENSITY;
                let dy = (y as f32 + 0.5 - c) / FEATHER_DENSITY;
                let a = alpha((dx * dx + dy * dy).sqrt());
                rgba[((y * side + x) * 4 + 3) as usize] = (a * 255.0).round() as u8;
            }
        }
        let handle = image::Handle::from_rgba(side, side, rgba);
        *last = Some((key, handle.clone()));
        handle
    })
}

/// The parts of `tile` outside `cut`, as up to four rectangles.
fn tile_minus(tile: Rectangle, cut: Option<Rectangle>) -> impl Iterator<Item = Rectangle> {
    let parts = match cut {
        None => [Some(tile), None, None, None],
        Some(c) => {
            let (l, t) = (tile.x, tile.y);
            let (r, b) = (tile.x + tile.width, tile.y + tile.height);
            let (cl, ct) = (c.x, c.y);
            let (cr, cb) = (c.x + c.width, c.y + c.height);
            let part = |x0: f32, y0: f32, x1: f32, y1: f32| {
                (x1 > x0 && y1 > y0)
                    .then(|| Rectangle::new(Point::new(x0, y0), Size::new(x1 - x0, y1 - y0)))
            };
            [
                part(l, t, r, ct),
                part(l, cb, r, b),
                part(l, ct, cl, cb),
                part(cr, ct, r, cb),
            ]
        }
    };
    parts.into_iter().flatten()
}

/// Dims everything but a circle whose inside edge fades smoothly.
///
/// The dimmed area is a fixed grid of tiles, each its own geometry so it
/// stays identical between frames unless the hole overlaps it, then the
/// hole's own square and a cached radial image for the soft edge. Moving the
/// pointer therefore repaints only around the old and new hole, never the
/// whole display. Tiles are plain rectangles drawn without anti-aliasing, so
/// they meet without seams at any display scale; only the circle is
/// anti-aliased.
///
/// Within a layer images paint above shapes, so these must have a layer of
/// their own beneath the ink.
pub fn spotlight(
    renderer: &iced::Renderer,
    size: Size,
    at: Point,
    radius: f32,
    dim: f32,
) -> Vec<canvas::Geometry> {
    let feather = (radius * 0.18).clamp(8.0, 36.0);
    let outer = radius + feather / 2.0;
    let shade = Color::from_rgba(0.0, 0.0, 0.0, dim);
    let screen = Rectangle::new(Point::ORIGIN, size);
    let lo = |v: f32| (v / SNAP).floor() * SNAP;
    let hi = |v: f32| (v / SNAP).ceil() * SNAP;
    let x0 = lo(at.x - outer - 1.0);
    let y0 = lo(at.y - outer - 1.0);
    let hole = Rectangle::new(
        Point::new(x0, y0),
        Size::new(hi(at.x + outer + 1.0) - x0, hi(at.y + outer + 1.0) - y0),
    );

    let cols = (size.width / TILE).ceil() as usize;
    let rows = (size.height / TILE).ceil() as usize;
    let mut layers = Vec::with_capacity(cols * rows + 1);
    for row in 0..rows {
        for col in 0..cols {
            let tile = Rectangle::new(
                Point::new(col as f32 * TILE, row as f32 * TILE),
                Size::new(TILE, TILE),
            );
            let Some(tile) = tile.intersection(&screen) else {
                continue;
            };
            let mut frame = Frame::new(renderer, size);
            for part in tile_minus(tile, tile.intersection(&hole)) {
                frame.fill_rectangle(part.position(), part.size(), shade);
            }
            layers.push(frame.into_geometry());
        }
    }

    let mut frame = Frame::new(renderer, size);
    frame.fill(
        &Path::new(|b| {
            rect(b, hole);
            b.circle(at, outer.max(0.0));
        }),
        canvas::Fill {
            style: canvas::Style::Solid(shade),
            rule: Rule::EvenOdd,
        },
    );
    frame.draw_image(
        Rectangle::new(
            Point::new(at.x - outer, at.y - outer),
            Size::new(2.0 * outer, 2.0 * outer),
        ),
        canvas::Image::new(feather_image(outer, feather, dim)),
    );
    layers.push(frame.into_geometry());
    layers
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(v: &[(f32, f32)]) -> Vec<Pt> {
        v.iter().map(|&(x, y)| Pt::new(x, y)).collect()
    }

    #[test]
    fn smoothing_keeps_ends_and_irons_out_a_pixel_staircase() {
        let stair: Vec<Pt> = (0..40).map(|i| Pt::new(i as f32, (i / 2) as f32)).collect();
        let s = smooth(&stair);
        assert_eq!(s[0], Point::new(0.0, 0.0));
        assert_eq!(*s.last().unwrap(), Point::new(39.0, 19.0));
        // Interior points sit close to the ideal line y = x / 2.
        let worst = s[3..37]
            .iter()
            .map(|p| (p.y - (p.x / 2.0 - 0.25)).abs())
            .fold(0.0, f32::max);
        assert!(worst < 0.2, "staircase residue {worst}");
    }

    #[test]
    fn smoothing_does_not_flatten_a_deliberate_corner() {
        let s = smooth(&pts(&[(0.0, 0.0), (50.0, 0.0), (50.0, 50.0)]));
        assert!(s[1].x > 48.0 && s[1].y < 2.0);
    }

    #[test]
    fn earlier_runs_of_a_growing_stroke_stay_identical() {
        let mut points: Vec<Pt> = (0..200)
            .map(|i| Pt::new(i as f32 * 3.0, (i as f32 * 0.2).sin() * 30.0))
            .collect();
        let before = stroke_paths(&points, true);
        points.extend((200..230).map(|i| Pt::new(i as f32 * 3.0, 5.0)));
        let after = stroke_paths(&points, true);
        assert!(after.len() >= before.len());
        let stable = before.len() - 1;
        for i in 0..stable {
            assert!(
                before[i].raw().iter().eq(after[i].raw().iter()),
                "run {i} changed"
            );
        }
    }
}
