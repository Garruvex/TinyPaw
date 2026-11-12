//! The automatic resting point: where a character rests when standing or
//! bouncing. It is estimated from the body art only, so a separate tail or
//! other part never moves it. A flat picture cannot tell a painted-in tail
//! from feet; Advanced → Resting point → Adjust manually is the remedy.

use image::RgbaImage;

/// Alpha below this counts as empty.
const SOLID: u8 = 32;
/// The band at the bottom of the opaque box the feet are looked for in, as
/// a share of the box height.
const BAND: f32 = 0.08;
/// How far from the box's sides the estimate may sit, as a share of its
/// width: it stays in the middle 60 %.
const INSET: f32 = 0.2;

/// Whether a resting point follows the art or was set by hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RestMode {
    /// Re-estimated whenever the body changes.
    Auto,
    /// Kept as saved until Reset. A pack without editor metadata is
    /// treated as manual, so its saved anchor is preserved.
    #[default]
    Manual,
}

impl RestMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Manual => "manual",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "auto" => Some(Self::Auto),
            "manual" => Some(Self::Manual),
            _ => None,
        }
    }
}

/// The resting point of `body`, as fractions of the image from its
/// top-left, or `None` when the image has no opaque pixels.
///
/// `y` is just below the lowest opaque row. `x` is the centre of the
/// opaque pixels in the lowest 8 % of the opaque box, limited to the
/// box's middle 60 % so one outlying foot or shadow cannot pull it to an
/// edge.
pub fn estimate(body: &RgbaImage) -> Option<(f32, f32)> {
    let (w, h) = body.dimensions();
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for (x, y, p) in body.enumerate_pixels() {
        if p[3] >= SOLID {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    if x0 > x1 {
        return None;
    }
    let (bw, bh) = ((x1 - x0 + 1) as f32, (y1 - y0 + 1) as f32);
    let band = ((bh * BAND).ceil() as u32).max(1);
    let (mut sum, mut count) = (0.0f64, 0u64);
    for y in (y1 + 1).saturating_sub(band)..=y1 {
        for x in x0..=x1 {
            if body.get_pixel(x, y)[3] >= SOLID {
                sum += f64::from(x) + 0.5;
                count += 1;
            }
        }
    }
    let centre = (sum / count.max(1) as f64) as f32;
    let (lo, hi) = (x0 as f32 + bw * INSET, x0 as f32 + bw * (1.0 - INSET));
    let x = centre.clamp(lo, hi);
    Some((x / w as f32, (y1 + 1) as f32 / h as f32))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;
    use std::path::Path;

    fn png(path: &str) -> RgbaImage {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
        image::open(&path)
            .unwrap_or_else(|e| panic!("{path:?}: {e}"))
            .to_rgba8()
    }

    /// The bundled anchor: centred, under the lowest solid row of any layer.
    const YOHTA_ANCHOR_X: f32 = 0.5;

    fn yohta_layers() -> [RgbaImage; 3] {
        [
            png("assets/mascot/yohta/pack/sit/tail.png"),
            png("assets/mascot/yohta/pack/sit/body.png"),
            png("assets/mascot/yohta/pack/sit/head.png"),
        ]
    }

    fn lowest(images: &[&RgbaImage]) -> f32 {
        let h = images[0].height();
        let bottom = (0..h)
            .rev()
            .find(|&y| {
                images
                    .iter()
                    .any(|i| (0..i.width()).any(|x| i.get_pixel(x, y)[3] >= SOLID))
            })
            .unwrap();
        (bottom + 1) as f32 / h as f32
    }

    #[test]
    fn yohta_estimates_land_on_the_hand_set_anchor() {
        let [tail, body, head] = yohta_layers();
        let anchor_y = lowest(&[&tail, &body, &head]);
        // Yohta's separate body.
        let (x, y) = estimate(&body).unwrap();
        assert!((x - YOHTA_ANCHOR_X).abs() <= 0.02, "body x {x}");
        assert!((y - anchor_y).abs() <= 0.02, "body y {y} vs {anchor_y}");
        // Flat Yohta: every layer painted into one picture.
        let mut flat = RgbaImage::new(body.width(), body.height());
        for layer in [&tail, &body, &head] {
            image::imageops::overlay(&mut flat, layer, 0, 0);
        }
        let (x, y) = estimate(&flat).unwrap();
        assert!((x - YOHTA_ANCHOR_X).abs() <= 0.02, "flat x {x}");
        assert!((y - anchor_y).abs() <= 0.02, "flat y {y} vs {anchor_y}");
    }

    fn fill(img: &mut RgbaImage, f: impl Fn(u32, u32) -> bool) {
        for (x, y, p) in img.enumerate_pixels_mut() {
            if f(x, y) {
                *p = Rgba([120, 90, 200, 255]);
            }
        }
    }

    /// The estimate stays inside the opaque box.
    fn inside(img: &RgbaImage, (x, y): (f32, f32), bbox: (u32, u32, u32, u32)) {
        let (px, py) = (x * img.width() as f32, y * img.height() as f32);
        assert!(
            px >= bbox.0 as f32 && px <= (bbox.2 + 1) as f32,
            "{px} in {bbox:?}"
        );
        assert!(
            py >= bbox.1 as f32 && py <= (bbox.3 + 1) as f32,
            "{py} in {bbox:?}"
        );
    }

    #[test]
    fn a_round_floating_sprite_rests_at_its_bottom_centre() {
        let mut img = RgbaImage::new(200, 200);
        fill(&mut img, |x, y| {
            let (dx, dy) = (x as f32 - 100.0, y as f32 - 80.0);
            dx * dx + dy * dy <= 50.0 * 50.0
        });
        let e = estimate(&img).unwrap();
        inside(&img, e, (50, 30, 150, 130));
        assert!(
            (e.0 - 0.5).abs() < 0.01 && (e.1 * 200.0 - 131.0).abs() < 1.0,
            "{e:?}"
        );
    }

    #[test]
    fn a_tail_below_the_feet_is_kept_off_the_edge() {
        // A body 60..140 wide standing at y 160, with a thin tail curling
        // down to y 190 at the far right.
        let mut img = RgbaImage::new(220, 220);
        fill(&mut img, |x, y| {
            (60..140).contains(&x) && (40..160).contains(&y)
        });
        fill(&mut img, |x, y| {
            (180..190).contains(&x) && (100..190).contains(&y)
        });
        let e = estimate(&img).unwrap();
        inside(&img, e, (60, 40, 189, 189));
        // It rests on the tail (the lowest art), but no further right than
        // the box's middle 60 %.
        let px = e.0 * 220.0;
        assert!(px <= 60.0 + 130.0 * 0.8 + 0.01, "{px}");
        assert_eq!(e.1 * 220.0, 190.0);
    }

    #[test]
    fn wide_transparent_padding_does_not_move_the_estimate() {
        let mut tight = RgbaImage::new(100, 100);
        fill(&mut tight, |x, y| {
            (20..80).contains(&x) && (10..90).contains(&y)
        });
        let mut padded = RgbaImage::new(500, 300);
        fill(&mut padded, |x, y| {
            (220..280).contains(&x) && (110..190).contains(&y)
        });
        let (a, b) = (estimate(&tight).unwrap(), estimate(&padded).unwrap());
        // The same point on the art, in pixels from the art's corner.
        let (ax, ay) = (a.0 * 100.0 - 20.0, a.1 * 100.0 - 10.0);
        let (bx, by) = (b.0 * 500.0 - 220.0, b.1 * 300.0 - 110.0);
        assert!((ax - bx).abs() < 0.01 && (ay - by).abs() < 0.01);
        inside(&padded, b, (220, 110, 279, 189));
    }

    #[test]
    fn empty_art_has_no_estimate() {
        assert_eq!(estimate(&RgbaImage::new(10, 10)), None);
        assert_eq!(RestMode::parse(RestMode::Auto.name()), Some(RestMode::Auto));
    }
}
