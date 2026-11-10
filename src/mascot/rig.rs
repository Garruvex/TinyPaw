//! Character renderer. Bundled Yohta is cut from the approved concept sheet
//! (the source stays untouched; regions are decoded once and small masked eye
//! frames are cached). An imported character is one whole image, or layered:
//! a body plus moving parts and eyes, all registered on one canvas.
use super::motion::{Expression, MotionSpec, PartMotion, PartType, Pose, Preset};
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
    Layered {
        body: Piece,
        parts: Vec<Part>,
        eyes: Vec<LayerEye>,
    },
}

/// An image cropped to its visible pixels, placed on the canvas.
struct Piece {
    image: Handle,
    at: Point,
    size: Size,
}

struct Part {
    kind: PartType,
    piece: Piece,
    /// Head faces: blink, pressed, dragging.
    faces: [Option<Piece>; 3],
    pivot: Point,
    parent: Option<usize>,
    behind: bool,
    motion: MotionSpec,
}

struct LayerEye {
    frames: Vec<Handle>,
    closed: Option<Piece>,
    at: Point,
    size: Size,
    /// The head part the eye rides on.
    head: usize,
}

/// Decoded art for a layered character, every image canvas-sized.
pub struct LayeredArt {
    pub body: RgbaImage,
    /// Where the feet meet the ground, as fractions of the canvas.
    pub ground: (f32, f32),
    pub parts: Vec<PartArt>,
    pub eyes: Vec<EyeArt>,
}

pub struct PartArt {
    pub kind: PartType,
    pub image: RgbaImage,
    /// Attachment point, as fractions of the canvas.
    pub pivot: (f32, f32),
    pub parent: Option<usize>,
    pub behind: bool,
    pub motion: MotionSpec,
    /// Head faces: blink, pressed, dragging.
    pub faces: [Option<RgbaImage>; 3],
}

pub struct EyeArt {
    pub white: RgbaImage,
    pub pupil: RgbaImage,
    /// Alpha marks where the pupil may show.
    pub mask: RgbaImage,
    pub blink: Option<RgbaImage>,
    /// Furthest pupil travel, as fractions of the canvas.
    pub max_offset: (f32, f32),
}

/// The visible part of a canvas-sized image, or `None` if it's empty.
fn piece(image: &RgbaImage) -> Option<(Piece, RgbaImage)> {
    let (x, y, w, h) = visible(image)?;
    let cropped = image::imageops::crop_imm(image, x, y, w, h).to_image();
    Some((
        Piece {
            image: handle(cropped.clone()),
            at: Point::new(x as f32, y as f32),
            size: Size::new(w as f32, h as f32),
        },
        cropped,
    ))
}

/// Bounding box of pixels with any alpha.
fn visible(image: &RgbaImage) -> Option<(u32, u32, u32, u32)> {
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for (x, y, p) in image.enumerate_pixels() {
        if p[3] > 0 {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    (x0 <= x1).then(|| (x0, y0, x1 - x0 + 1, y1 - y0 + 1))
}

/// `top` over `bottom`, both straight (unpremultiplied) RGBA.
fn over(top: Rgba<u8>, bottom: Rgba<u8>) -> Rgba<u8> {
    let ta = f32::from(top[3]) / 255.0;
    let ba = f32::from(bottom[3]) / 255.0;
    let a = ta + ba * (1.0 - ta);
    if a <= 0.0 {
        return Rgba([0, 0, 0, 0]);
    }
    let mix = |t: u8, b: u8| {
        ((f32::from(t) * ta + f32::from(b) * ba * (1.0 - ta)) / a).round() as u8
    };
    Rgba([
        mix(top[0], bottom[0]),
        mix(top[1], bottom[1]),
        mix(top[2], bottom[2]),
        (a * 255.0).round() as u8,
    ])
}

impl LayerEye {
    /// Precomposites the white with the pupil, clipped to the mask, at each
    /// gaze step, so drawing is one image per eye.
    fn new(art: &EyeArt, head: usize) -> Option<Self> {
        let (w, h) = art.white.dimensions();
        let (mx, my, mw, mh) = visible(&art.mask)?;
        // The frame covers the mask and the white around it.
        let (x, y, fw, fh) = match visible(&art.white) {
            Some((wx, wy, ww, wh)) => {
                let (x0, y0) = (mx.min(wx), my.min(wy));
                let (x1, y1) = ((mx + mw).max(wx + ww), (my + mh).max(wy + wh));
                (x0, y0, x1 - x0, y1 - y0)
            }
            None => (mx, my, mw, mh),
        };
        let (ox, oy) = (art.max_offset.0 * w as f32, art.max_offset.1 * h as f32);
        let mut frames = Vec::with_capacity(((STEPS * 2 + 1) * (STEPS * 2 + 1)) as usize);
        for gy in -STEPS..=STEPS {
            for gx in -STEPS..=STEPS {
                let dx = (gx as f32 / STEPS as f32 * ox).round() as i64;
                let dy = (gy as f32 / STEPS as f32 * oy).round() as i64;
                let frame = RgbaImage::from_fn(fw, fh, |fx, fy| {
                    let (cx, cy) = (x + fx, y + fy);
                    let white = *art.white.get_pixel(cx, cy);
                    let (px, py) = (i64::from(cx) - dx, i64::from(cy) - dy);
                    if px < 0 || py < 0 || px >= i64::from(w) || py >= i64::from(h) {
                        return white;
                    }
                    let mut pupil = *art.pupil.get_pixel(px as u32, py as u32);
                    let clip = u32::from(art.mask.get_pixel(cx, cy)[3]);
                    pupil[3] = (u32::from(pupil[3]) * clip / 255) as u8;
                    over(pupil, white)
                });
                frames.push(handle(frame));
            }
        }
        Some(Self {
            frames,
            closed: art.blink.as_ref().and_then(piece).map(|(p, _)| p),
            at: Point::new(x as f32, y as f32),
            size: Size::new(fw as f32, fh as f32),
            head,
        })
    }
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

    /// A character built from a body and moving parts on one canvas.
    pub fn layered(art: LayeredArt) -> Self {
        let size = Size::new(art.body.width() as f32, art.body.height() as f32);
        let mut silhouette = Silhouette::new(size);
        let stamp = |s: &mut Silhouette, p: &Piece, pixels: &RgbaImage| {
            s.stamp(pixels, (p.at, p.size));
        };
        let (body, body_pixels) = piece(&art.body).unwrap_or_else(|| {
            (
                Piece {
                    image: handle(RgbaImage::new(1, 1)),
                    at: Point::ORIGIN,
                    size: Size::new(1.0, 1.0),
                },
                RgbaImage::new(1, 1),
            )
        });
        stamp(&mut silhouette, &body, &body_pixels);
        let mut parts = Vec::new();
        for p in &art.parts {
            let Some((main, pixels)) = piece(&p.image) else {
                continue;
            };
            stamp(&mut silhouette, &main, &pixels);
            parts.push(Part {
                kind: p.kind,
                piece: main,
                faces: [0, 1, 2].map(|i| {
                    p.faces[i].as_ref().and_then(piece).map(|(face, _)| face)
                }),
                pivot: Point::new(p.pivot.0 * size.width, p.pivot.1 * size.height),
                parent: p.parent,
                behind: p.behind,
                motion: p.motion,
            });
        }
        let head = parts.iter().position(|p| p.kind == PartType::Head);
        let eyes: Vec<LayerEye> = head
            .map(|head| {
                art.eyes
                    .iter()
                    .filter_map(|e| LayerEye::new(e, head))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            anchor: Point::new(art.ground.0 * size.width, art.ground.1 * size.height),
            kind: Kind::Layered { body, parts, eyes },
            size,
            silhouette,
        }
    }

    /// The parts the animator moves, in the order of `Pose::angles`.
    pub fn motion_parts(&self) -> Vec<PartMotion> {
        let preset = |kind: PartType, preset: Preset| PartMotion {
            kind,
            spec: MotionSpec {
                preset,
                strength: 1.0,
                speed: 1.0,
            },
            parent: None,
        };
        match &self.kind {
            Kind::Yohta { .. } => vec![
                preset(PartType::Head, Preset::Tilt),
                preset(PartType::Tail, Preset::Sway),
            ],
            // The whole image sways a little, like a tilting head.
            Kind::Sprite(_) => vec![preset(PartType::Head, Preset::Tilt)],
            Kind::Layered { parts, .. } => parts
                .iter()
                .map(|p| PartMotion {
                    kind: p.kind,
                    spec: p.motion,
                    parent: p.parent,
                })
                .collect(),
        }
    }

    /// The point between the eyes, in rig pixels, for gaze.
    pub fn eye_point(&self) -> Option<Point> {
        match &self.kind {
            Kind::Yohta { .. } => Some(Point::new(232.0, 360.0)),
            Kind::Sprite(_) => None,
            Kind::Layered { eyes, .. } if !eyes.is_empty() => {
                let n = eyes.len() as f32;
                let sum = eyes.iter().fold(Vector::ZERO, |acc, e| {
                    acc + Vector::new(e.at.x + e.size.width / 2.0, e.at.y + e.size.height / 2.0)
                });
                Some(Point::new(sum.x / n, sum.y / n))
            }
            Kind::Layered { .. } => None,
        }
    }

    /// The head's rotation in `pose`, which the gaze is counter-rotated by.
    pub fn head_angle(&self, pose: &Pose) -> f32 {
        let index = match &self.kind {
            Kind::Yohta { .. } => Some(0),
            Kind::Sprite(_) => None,
            Kind::Layered { parts, .. } => parts.iter().position(|p| p.kind == PartType::Head),
        };
        index
            .and_then(|i| pose.angles.get(i))
            .copied()
            .unwrap_or_default()
    }

    /// Draws one frame. Lean and squash are applied by the host around the
    /// feet; this draws the character itself in rig coordinates.
    pub fn draw_pose(&self, frame: &mut Frame, pose: &Pose) {
        let angle = |i: usize| pose.angles.get(i).copied().unwrap_or_default();
        match &self.kind {
            Kind::Yohta { .. } | Kind::Sprite(_) => self.draw(
                frame,
                pose.gaze,
                angle(0),
                angle(1),
                pose.expression == Expression::Blink,
            ),
            Kind::Layered { body, parts, eyes } => {
                let ctx = Layered {
                    parts,
                    eyes,
                    pose,
                };
                for (i, p) in parts.iter().enumerate() {
                    if p.parent.is_none() && p.behind {
                        ctx.part(frame, i);
                    }
                }
                place(frame, body);
                for (i, p) in parts.iter().enumerate() {
                    if p.parent.is_none() && !p.behind {
                        ctx.part(frame, i);
                    }
                }
            }
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
            Kind::Layered { .. } => return,
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

fn place(frame: &mut Frame, p: &Piece) {
    frame.draw_image(Rectangle::new(p.at, p.size), canvas::Image::new(p.image.clone()));
}

/// Draws a layered character's parts with their parents' transforms.
struct Layered<'a> {
    parts: &'a [Part],
    eyes: &'a [LayerEye],
    pose: &'a Pose,
}

impl Layered<'_> {
    fn part(&self, frame: &mut Frame, index: usize) {
        let p = &self.parts[index];
        let angle = self.pose.angles.get(index).copied().unwrap_or_default();
        frame.with_save(|f| {
            f.translate(Vector::new(p.pivot.x, p.pivot.y));
            f.rotate(angle);
            f.translate(Vector::new(-p.pivot.x, -p.pivot.y));
            let children = |f: &mut Frame, behind: bool| {
                for (i, c) in self.parts.iter().enumerate() {
                    if c.parent == Some(index) && c.behind == behind {
                        self.part(f, i);
                    }
                }
            };
            children(f, true);
            // Head faces replace the head art; a missing face uses it.
            let face = match self.pose.expression {
                Expression::Blink => p.faces[0].as_ref(),
                Expression::Pressed => p.faces[1].as_ref(),
                Expression::Dragging => p.faces[2].as_ref(),
                Expression::Idle => None,
            };
            place(f, face.unwrap_or(&p.piece));
            // Tracking eyes sit on the plain head and its blink face only;
            // a pressed or dragging face has its eyes painted in.
            let painted = face.is_some() && self.pose.expression != Expression::Blink;
            if !painted {
                for e in self.eyes.iter().filter(|e| e.head == index) {
                    match (&e.closed, self.pose.expression) {
                        (Some(closed), Expression::Blink) => place(f, closed),
                        // A blink face already covers the eyes.
                        (None, Expression::Blink) if face.is_some() => {}
                        _ => f.draw_image(
                            Rectangle::new(e.at, e.size),
                            canvas::Image::new(e.frames[gaze_index(self.pose.gaze)].clone()),
                        ),
                    }
                }
            }
            children(f, false);
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
        assert!(rig.eye_point().is_some());
    }

    /// A 100 × 100 canvas: body block, head block with a pivot, one eye.
    fn layered_art() -> LayeredArt {
        let block = |x0: u32, y0: u32, x1: u32, y1: u32, c: [u8; 4]| {
            RgbaImage::from_fn(100, 100, |x, y| {
                if (x0..x1).contains(&x) && (y0..y1).contains(&y) {
                    Rgba(c)
                } else {
                    Rgba([0, 0, 0, 0])
                }
            })
        };
        LayeredArt {
            body: block(30, 50, 70, 95, [40, 80, 200, 255]),
            ground: (0.5, 0.95),
            parts: vec![PartArt {
                kind: PartType::Head,
                image: block(20, 10, 80, 55, [60, 110, 230, 255]),
                pivot: (0.5, 0.5),
                parent: None,
                behind: false,
                motion: MotionSpec::default_for(PartType::Head),
                faces: [None, Some(block(20, 10, 80, 55, [250, 0, 0, 255])), None],
            }],
            eyes: vec![EyeArt {
                white: block(40, 25, 50, 35, [255, 255, 255, 255]),
                pupil: block(44, 29, 46, 31, [0, 0, 0, 255]),
                mask: block(40, 25, 50, 35, [0, 0, 0, 255]),
                blink: None,
                max_offset: (0.03, 0.02),
            }],
        }
    }

    #[test]
    fn layered_rig_crops_parts_and_composites_eyes() {
        let rig = Rig::layered(layered_art());
        assert_eq!(rig.size(), Size::new(100.0, 100.0));
        assert!((rig.anchor().y - 95.0).abs() < 1e-4);
        assert!(rig.eye_point().is_some());
        let eye = rig.eye_point().unwrap();
        assert!((eye.x - 45.0).abs() < 1e-3 && (eye.y - 30.0).abs() < 1e-3);
        let Kind::Layered { body, parts, eyes } = &rig.kind else {
            panic!("layered");
        };
        assert_eq!((body.at, body.size), (Point::new(30.0, 50.0), Size::new(40.0, 45.0)));
        assert_eq!(parts[0].pivot, Point::new(50.0, 50.0));
        assert!(parts[0].faces[1].is_some() && parts[0].faces[0].is_none());
        assert_eq!(eyes[0].frames.len(), 49);
        assert_eq!(rig.motion_parts().len(), 1);
        // The head counts for clicks, the empty corners don't.
        assert!(rig.opaque(Point::new(50.0, 20.0)));
        assert!(!rig.opaque(Point::new(3.0, 3.0)));
    }

    #[test]
    fn pupils_stay_inside_their_mask() {
        let mut art = layered_art();
        // A pupil far bigger than the socket, pushed hard right.
        art.eyes[0].pupil = RgbaImage::from_pixel(100, 100, Rgba([0, 0, 0, 255]));
        art.eyes[0].white = RgbaImage::new(100, 100);
        art.eyes[0].max_offset = (0.2, 0.2);
        let eye = LayerEye::new(&art.eyes[0], 0).unwrap();
        assert_eq!((eye.at, eye.size), (Point::new(40.0, 25.0), Size::new(10.0, 10.0)));
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
        assert!(rig.eye_point().is_none());
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
