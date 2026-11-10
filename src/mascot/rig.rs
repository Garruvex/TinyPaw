//! Character renderer. Bundled Yohta is cut from the approved concept sheet
//! (the source stays untouched; regions are decoded once and small masked eye
//! frames are cached). An imported character is one whole image for now.
use iced::advanced::image::Handle;
use iced::widget::canvas::{self, Frame};
use iced::{Point, Rectangle, Size, Vector};
use image::{Rgba, RgbaImage};
use std::collections::VecDeque;

const SOURCE: &[u8] = include_bytes!("../../assets/mascot/yohta/source/approved.png");
/// Yohta's rig canvas.
pub const WIDTH: f32 = 620.0;
pub const HEIGHT: f32 = 860.0;
const STEPS: i32 = 3;
/// Rig pixels per silhouette cell. Coarse cells also pad the outline a
/// little, covering the small idle rotations of head and tail.
const CELL: f32 = 6.0;

// Where each sprite is drawn, in rig coordinates.
const TAIL_AT: (Point, Size) = (Point::new(302.0, 452.0), Size::new(308.0, 312.0));
const BODY_AT: (Point, Size) = (Point::new(91.0, 457.0), Size::new(410.0, 350.0));
const HEAD_AT: (Point, Size) = (Point::new(0.0, 0.0), Size::new(544.0, 520.0));

pub struct Rig {
    kind: Kind,
    /// Canvas size in rig pixels.
    size: Size,
    /// Where the feet meet the ground, in rig pixels.
    anchor: Point,
    silhouette: Silhouette,
}

enum Kind {
    Yohta {
        head: Handle,
        body: Handle,
        tail: Handle,
        eyes: Vec<Eye>,
    },
    /// One whole image: it moves as one piece.
    Sprite(Handle),
}

/// Which rig cells hold visible artwork, for alpha-aware hit testing.
struct Silhouette {
    columns: usize,
    rows: usize,
    cells: Vec<bool>,
    /// Lowest visible rig row: where the feet meet the ground.
    ground: f32,
}

impl Silhouette {
    fn new(size: Size) -> Self {
        let columns = (size.width / CELL).ceil() as usize;
        let rows = (size.height / CELL).ceil() as usize;
        Self {
            columns,
            rows,
            cells: vec![false; columns * rows],
            ground: 0.0,
        }
    }

    fn stamp(&mut self, pixels: &RgbaImage, (at, size): (Point, Size)) {
        let (sx, sy) = (
            size.width / pixels.width() as f32,
            size.height / pixels.height() as f32,
        );
        for (x, y, p) in pixels.enumerate_pixels() {
            if p[3] < 32 {
                continue;
            }
            let rx = at.x + x as f32 * sx;
            let ry = at.y + y as f32 * sy;
            let (c, r) = ((rx / CELL) as usize, (ry / CELL) as usize);
            if c < self.columns && r < self.rows {
                self.cells[r * self.columns + c] = true;
                self.ground = self.ground.max(ry + sy);
            }
        }
    }

    fn contains(&self, p: Point) -> bool {
        if p.x < 0.0 || p.y < 0.0 {
            return false;
        }
        let (c, r) = ((p.x / CELL) as usize, (p.y / CELL) as usize);
        c < self.columns && r < self.rows && self.cells[r * self.columns + c]
    }
}

struct Eye {
    at: Point,
    size: Size,
    frames: Vec<Handle>,
    closed: Handle,
}

fn handle(pixels: RgbaImage) -> Handle {
    Handle::from_rgba(pixels.width(), pixels.height(), pixels.into_raw())
}

impl Rig {
    pub fn load() -> Self {
        let sheet = image::load_from_memory(SOURCE)
            .expect("embedded Yohta PNG")
            .to_rgba8();
        let crop = |x, y, w, h| image::imageops::crop_imm(&sheet, x, y, w, h).to_image();
        let mut head = crop(688, 60, 544, 520);
        isolate(&mut head, (250, 200));
        // Seed points select only the connected white eye sockets, preserving
        // the original outline, muzzle and highlights outside the socket.
        let eyes = vec![
            Eye::new(&head, (98, 355), (70, 317, 57, 73)),
            Eye::new(&head, (245, 355), (177, 309, 164, 90)),
        ];
        let body = crop(720, 638, 410, 350);
        let tail = crop(1138, 600, 385, 390);
        let size = Size::new(WIDTH, HEIGHT);
        let mut silhouette = Silhouette::new(size);
        silhouette.stamp(&tail, TAIL_AT);
        silhouette.stamp(&body, BODY_AT);
        silhouette.stamp(&head, HEAD_AT);
        Self {
            anchor: Point::new(WIDTH * 0.5, silhouette.ground),
            kind: Kind::Yohta {
                head: handle(head),
                body: handle(body),
                tail: handle(tail),
                eyes,
            },
            size,
            silhouette,
        }
    }

    /// A character drawn from one image, standing at `ground` (fractions of
    /// the image from its top-left).
    pub fn sprite(pixels: RgbaImage, ground: (f32, f32)) -> Self {
        let size = Size::new(pixels.width() as f32, pixels.height() as f32);
        let mut silhouette = Silhouette::new(size);
        silhouette.stamp(&pixels, (Point::ORIGIN, size));
        Self {
            anchor: Point::new(ground.0 * size.width, ground.1 * size.height),
            kind: Kind::Sprite(handle(pixels)),
            size,
            silhouette,
        }
    }

    /// Yohta's head, small, for character lists.
    pub fn yohta_thumbnail(side: u32) -> RgbaImage {
        let sheet = image::load_from_memory(SOURCE)
            .expect("embedded Yohta PNG")
            .to_rgba8();
        let mut head = image::imageops::crop_imm(&sheet, 688, 60, 544, 520).to_image();
        isolate(&mut head, (250, 200));
        let scale = side as f32 / head.width().max(head.height()) as f32;
        image::imageops::resize(
            &head,
            ((head.width() as f32 * scale).round() as u32).max(1),
            ((head.height() as f32 * scale).round() as u32).max(1),
            image::imageops::FilterType::Triangle,
        )
    }

    /// Whether a rig-space point lies on visible artwork.
    pub fn opaque(&self, p: Point) -> bool {
        self.silhouette.contains(p)
    }

    /// Canvas size in rig pixels.
    pub fn size(&self) -> Size {
        self.size
    }

    /// Where the feet meet the ground, in rig pixels.
    pub fn anchor(&self) -> Point {
        self.anchor
    }

    /// Whether the character has eyes that can follow the cursor.
    pub fn has_eyes(&self) -> bool {
        matches!(&self.kind, Kind::Yohta { eyes, .. } if !eyes.is_empty())
    }

    /// All positions are in rig coordinates. Rotation is around attachment
    /// points, rather than the centre of the source sprite rectangle.
    pub fn draw(
        &self,
        frame: &mut Frame,
        gaze: Vector,
        head_angle: f32,
        tail_angle: f32,
        blink: bool,
    ) {
        let (head, body, tail, eyes) = match &self.kind {
            Kind::Yohta {
                head,
                body,
                tail,
                eyes,
            } => (head, body, tail, eyes),
            // One piece: a gentle sway around the feet stands in for the
            // head's idle tilt.
            Kind::Sprite(image) => {
                sprite(
                    frame,
                    image,
                    Point::ORIGIN,
                    self.size,
                    self.anchor,
                    head_angle * 0.5,
                );
                return;
            }
        };
        sprite(
            frame,
            tail,
            TAIL_AT.0,
            TAIL_AT.1,
            Point::new(345.0, 510.0),
            tail_angle,
        );
        sprite(
            frame,
            body,
            BODY_AT.0,
            BODY_AT.1,
            Point::new(295.0, 820.0),
            0.0,
        );
        frame.with_save(|f| {
            let pivot = Point::new(269.0, 532.0);
            f.translate(Vector::new(pivot.x, pivot.y));
            f.rotate(head_angle);
            f.translate(Vector::new(-pivot.x, -pivot.y));
            f.draw_image(
                Rectangle::new(HEAD_AT.0, HEAD_AT.1),
                canvas::Image::new(head.clone()),
            );
            for eye in eyes {
                let index = gaze_index(gaze);
                let image = if blink {
                    &eye.closed
                } else {
                    &eye.frames[index]
                };
                f.draw_image(
                    Rectangle::new(eye.at, eye.size),
                    canvas::Image::new(image.clone()),
                );
            }
        });
    }
}

/// A source rectangle can include a few pixels from a neighbouring part.
/// Keep only the connected sprite under the seed, including its soft edge.
fn isolate(pixels: &mut RgbaImage, seed: (u32, u32)) {
    let (w, h) = pixels.dimensions();
    let mut keep = vec![false; (w * h) as usize];
    let mut queue = VecDeque::from([seed]);
    while let Some((x, y)) = queue.pop_front() {
        if x >= w || y >= h {
            continue;
        }
        let index = (y * w + x) as usize;
        if keep[index] || pixels.get_pixel(x, y)[3] < 8 {
            continue;
        }
        keep[index] = true;
        if x > 0 {
            queue.push_back((x - 1, y));
        }
        if y > 0 {
            queue.push_back((x, y - 1));
        }
        queue.extend([(x + 1, y), (x, y + 1)]);
    }
    for (index, pixel) in pixels.pixels_mut().enumerate() {
        if !keep[index] {
            *pixel = Rgba([0, 0, 0, 0]);
        }
    }
}

fn sprite(frame: &mut Frame, image: &Handle, at: Point, size: Size, pivot: Point, angle: f32) {
    frame.with_save(|f| {
        f.translate(Vector::new(pivot.x, pivot.y));
        f.rotate(angle);
        f.translate(Vector::new(-pivot.x, -pivot.y));
        f.draw_image(Rectangle::new(at, size), canvas::Image::new(image.clone()));
    });
}

pub fn gaze_index(gaze: Vector) -> usize {
    let quantize = |v: f32| (v.clamp(-1.0, 1.0) * STEPS as f32).round() as i32 + STEPS;
    (quantize(gaze.y) * (STEPS * 2 + 1) + quantize(gaze.x)) as usize
}

impl Eye {
    fn new(head: &RgbaImage, seed: (u32, u32), region: (u32, u32, u32, u32)) -> Self {
        let (rx, ry, w, h) = region;
        let mut mask = vec![false; (w * h) as usize];
        let mut queue = VecDeque::from([seed]);
        while let Some((x, y)) = queue.pop_front() {
            if x < rx || y < ry || x >= rx + w || y >= ry + h {
                continue;
            }
            let index = ((y - ry) * w + x - rx) as usize;
            let p = head.get_pixel(x, y);
            if mask[index] || p[3] < 100 || p[0].min(p[1]).min(p[2]) < 185 {
                continue;
            }
            mask[index] = true;
            queue.extend([(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]);
        }
        assert!(
            mask.iter().filter(|&&v| v).count() > 300,
            "Yohta eye mask is empty"
        );
        let make = |gx: f32, gy: f32, closed: bool| {
            RgbaImage::from_fn(w, h, |x, y| {
                if !mask[(y * w + x) as usize] {
                    return Rgba([0, 0, 0, 0]);
                }
                let px = x as f32;
                let py = y as f32;
                if closed {
                    let curve = h as f32 * (0.60 + 0.10 * ((px / w as f32 - 0.5) * 2.0).powi(2));
                    return if (py - curve).abs() < 2.5 {
                        Rgba([12, 17, 48, 254])
                    } else {
                        Rgba([28, 36, 104, 254])
                    };
                }
                let cx = w as f32 * (0.50 + gx * 0.13);
                let cy = h as f32 * (0.48 + gy * 0.10);
                let dx = (px - cx) / (w as f32 * 0.31);
                let dy = (py - cy) / (h as f32 * 0.74);
                if dx * dx + dy * dy < 1.0 {
                    let pupil = ((px - cx) / (w as f32 * 0.085)).powi(2)
                        + ((py - cy) / (h as f32 * 0.49)).powi(2);
                    if pupil < 1.0 {
                        Rgba([8, 16, 28, 254])
                    } else {
                        Rgba([0, 229, 229, 254])
                    }
                } else {
                    *head.get_pixel(rx + x, ry + y)
                }
            })
        };
        let mut frames = Vec::new();
        for y in -STEPS..=STEPS {
            for x in -STEPS..=STEPS {
                frames.push(handle(make(
                    x as f32 / STEPS as f32,
                    y as f32 / STEPS as f32,
                    false,
                )));
            }
        }
        Self {
            at: Point::new(rx as f32, ry as f32),
            size: Size::new(w as f32, h as f32),
            frames,
            closed: handle(make(0.0, 0.0, true)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gaze_is_bounded_and_centre_is_stable() {
        assert_eq!(gaze_index(Vector::ZERO), 24);
        assert_eq!(gaze_index(Vector::new(-100.0, -100.0)), 0);
        assert_eq!(gaze_index(Vector::new(100.0, 100.0)), 48);
    }
    #[test]
    fn approved_sheet_has_valid_regions_and_eye_masks() {
        let rig = Rig::load();
        let Kind::Yohta { eyes, .. } = &rig.kind else {
            panic!("Yohta rig");
        };
        assert_eq!(eyes.len(), 2);
        assert!(eyes.iter().all(|eye| eye.frames.len() == 49));
        assert!(rig.has_eyes());
    }

    #[test]
    fn sprite_rig_uses_its_own_canvas_and_ground() {
        let mut img = RgbaImage::new(40, 80);
        for y in 10..70 {
            for x in 10..30 {
                img.put_pixel(x, y, Rgba([90, 120, 200, 255]));
            }
        }
        let rig = Rig::sprite(img, (0.5, 70.0 / 80.0));
        assert_eq!(rig.size(), Size::new(40.0, 80.0));
        assert!((rig.anchor().y - 70.0).abs() < 1e-4);
        assert!(rig.opaque(Point::new(20.0, 40.0)));
        assert!(!rig.opaque(Point::new(2.0, 2.0)));
        assert!(!rig.has_eyes());
    }
    #[test]
    fn silhouette_covers_the_character_and_not_the_corners() {
        let rig = Rig::load();
        // Face, chest and tail are solid; canvas corners are empty.
        assert!(rig.opaque(Point::new(232.0, 360.0)));
        assert!(rig.opaque(Point::new(295.0, 700.0)));
        assert!(!rig.opaque(Point::new(2.0, HEIGHT - 2.0)));
        assert!(!rig.opaque(Point::new(WIDTH - 2.0, 2.0)));
        assert!(!rig.opaque(Point::new(-5.0, 300.0)));
        assert!(rig.anchor().y > 700.0 && rig.anchor().y <= HEIGHT);
    }
}
