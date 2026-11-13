//! Character renderer. A character is one whole image, or layered: a body
//! plus moving parts and eyes, all registered on one canvas. Bundled Yohta
//! is an ordinary layered pack (see `yohta_pack`).
use super::motion::{Bop, Expression, MotionSpec, PartMotion, PartType, Pose, Preset};
use iced::advanced::image::Handle;
use iced::widget::canvas::{self, Frame};
use iced::{Point, Rectangle, Size, Vector};
use image::{Rgba, RgbaImage};

/// Yohta's rig canvas.
pub const WIDTH: f32 = 620.0;
pub const HEIGHT: f32 = 860.0;
const STEPS: i32 = 3;
/// Rig pixels per silhouette cell. Coarse cells also pad the outline a
/// little, covering the small idle rotations of head and tail.
const CELL: f32 = 6.0;

#[derive(Debug, Clone)]
pub struct Rig {
    kind: Kind,
    /// Canvas size in rig pixels.
    size: Size,
    /// Where the feet meet the ground, in rig pixels.
    anchor: Point,
    /// Visible cells per layer, unposed: the body (or the whole sprite)
    /// first, then each part in order.
    silhouettes: Vec<Silhouette>,
    /// Motion per moving part, in `motion_parts` order.
    motion: Vec<MotionSpec>,
    /// How it bops to music.
    bop: Bop,
    /// Top and bottom of the visible artwork at rest, in rig pixels:
    /// what the character's Size measures, without transparent padding.
    visible: (f32, f32),
}

/// Top and bottom rows of pixels at least as opaque as hit testing needs,
/// in canvas pixels, over several canvas-sized images.
fn visible_rows<'a>(
    images: impl IntoIterator<Item = (&'a RgbaImage, Point, Size)>,
) -> Option<(f32, f32)> {
    let mut out: Option<(f32, f32)> = None;
    for (pixels, at, size) in images {
        let sy = size.height / pixels.height().max(1) as f32;
        let rows = (0..pixels.height())
            .filter(|&y| (0..pixels.width()).any(|x| pixels.get_pixel(x, y)[3] >= 32));
        let (mut top, mut bottom) = (None, None);
        for y in rows {
            top.get_or_insert(y);
            bottom = Some(y);
        }
        if let (Some(t), Some(b)) = (top, bottom) {
            let (t, b) = (at.y + t as f32 * sy, at.y + (b + 1) as f32 * sy);
            out = Some(out.map_or((t, b), |(ot, ob)| (ot.min(t), ob.max(b))));
        }
    }
    out
}

#[derive(Debug, Clone)]
enum Kind {
    /// One whole image: it moves as one piece.
    Sprite(Handle),
    Layered {
        body: Piece,
        parts: Vec<Part>,
        eyes: Vec<LayerEye>,
    },
}

/// An image cropped to its visible pixels, placed on the canvas.
#[derive(Debug, Clone)]
struct Piece {
    image: Handle,
    at: Point,
    size: Size,
}

#[derive(Debug, Clone)]
struct Part {
    kind: PartType,
    piece: Piece,
    /// Head faces: blink, pressed, dragging.
    faces: [Option<Piece>; 4],
    pivot: Point,
    parent: Option<usize>,
    behind: bool,
    motion: MotionSpec,
}

#[derive(Debug, Clone)]
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
    pub faces: [Option<RgbaImage>; 4],
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
    let mix =
        |t: u8, b: u8| ((f32::from(t) * ta + f32::from(b) * ba * (1.0 - ta)) / a).round() as u8;
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

/// A drawn layer: the body (or a one-image sprite), or a moving part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Body,
    Part(usize),
}

/// `q` turned by `-angle` around `pivot`: undoes a part's rotation.
fn unrotate(q: Point, pivot: Point, angle: f32) -> Point {
    let (s, c) = angle.sin_cos();
    let (x, y) = (q.x - pivot.x, q.y - pivot.y);
    Point::new(pivot.x + x * c + y * s, pivot.y - x * s + y * c)
}

/// Which rig cells hold visible artwork, for alpha-aware hit testing.
#[derive(Debug, Clone)]
struct Silhouette {
    columns: usize,
    rows: usize,
    cells: Vec<bool>,
    /// Lowest visible rig row: where the feet meet the ground.
    ground: f32,
    bounds: Option<Rectangle>,
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
            bounds: None,
        }
    }

    fn stamp(&mut self, pixels: &RgbaImage, (at, size): (Point, Size)) {
        let (sx, sy) = (
            size.width / pixels.width() as f32,
            size.height / pixels.height() as f32,
        );
        if let Some((x, y, w, h)) = visible(pixels) {
            let rect = Rectangle::new(
                Point::new(at.x + x as f32 * sx, at.y + y as f32 * sy),
                Size::new(w as f32 * sx, h as f32 * sy),
            );
            self.bounds = Some(self.bounds.map_or(rect, |old| old.union(&rect)));
        }
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

fn handle(pixels: RgbaImage) -> Handle {
    Handle::from_rgba(pixels.width(), pixels.height(), pixels.into_raw())
}

impl Rig {
    /// Bundled Yohta, from his embedded character pack.
    pub fn load() -> Self {
        super::pack::bundled_rig()
    }

    /// A character drawn from one image, standing at `ground` (fractions of
    /// the image from its top-left).
    pub fn sprite(pixels: RgbaImage, ground: (f32, f32)) -> Self {
        let size = Size::new(pixels.width() as f32, pixels.height() as f32);
        let mut silhouette = Silhouette::new(size);
        silhouette.stamp(&pixels, (Point::ORIGIN, size));
        Self {
            visible: visible_rows([(&pixels, Point::ORIGIN, size)]).unwrap_or((0.0, size.height)),
            silhouettes: vec![silhouette],
            anchor: Point::new(ground.0 * size.width, ground.1 * size.height),
            kind: Kind::Sprite(handle(pixels)),
            size,
            // The whole image sways a little, like a tilting head.
            motion: vec![MotionSpec::of(Preset::Tilt)],
            bop: Bop::default(),
        }
    }

    /// A character built from a body and moving parts on one canvas.
    pub fn layered(art: LayeredArt) -> Self {
        let size = Size::new(art.body.width() as f32, art.body.height() as f32);
        let stamped = |p: &Piece, pixels: &RgbaImage| {
            let mut s = Silhouette::new(size);
            s.stamp(pixels, (p.at, p.size));
            s
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
        let mut silhouettes = vec![stamped(&body, &body_pixels)];
        let mut shown = vec![(body_pixels.clone(), body.at, body.size)];
        let mut parts = Vec::new();
        for p in &art.parts {
            let Some((main, pixels)) = piece(&p.image) else {
                continue;
            };
            silhouettes.push(stamped(&main, &pixels));
            shown.push((pixels.clone(), main.at, main.size));
            parts.push(Part {
                kind: p.kind,
                piece: main,
                faces: [0, 1, 2, 3]
                    .map(|i| p.faces[i].as_ref().and_then(piece).map(|(face, _)| face)),
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
        let visible =
            visible_rows(shown.iter().map(|(p, at, s)| (p, *at, *s))).unwrap_or((0.0, size.height));
        Self {
            visible,
            anchor: Point::new(art.ground.0 * size.width, art.ground.1 * size.height),
            motion: parts.iter().map(|p| p.motion).collect(),
            bop: Bop::default(),
            kind: Kind::Layered { body, parts, eyes },
            size,
            silhouettes,
        }
    }

    /// The parts the animator moves, in the order of `Pose::angles`.
    pub fn motion_parts(&self) -> Vec<PartMotion> {
        let (kinds, parents): (Vec<PartType>, Vec<Option<usize>>) = match &self.kind {
            Kind::Sprite(_) => (vec![PartType::Head], vec![None]),
            Kind::Layered { parts, .. } => parts.iter().map(|p| (p.kind, p.parent)).unzip(),
        };
        kinds
            .into_iter()
            .zip(parents)
            .zip(&self.motion)
            .map(|((kind, parent), spec)| PartMotion {
                kind,
                spec: *spec,
                parent,
            })
            .collect()
    }

    /// What the Motion tab calls each moving part, in `motion_parts` order.
    pub fn motion_labels(&self) -> Vec<String> {
        match &self.kind {
            Kind::Sprite(_) => vec!["Whole character".into()],
            Kind::Layered { parts, .. } => parts
                .iter()
                .enumerate()
                .map(|(i, p)| format!("{} {}", i + 1, p.kind))
                .collect(),
        }
    }

    /// The current motion per part.
    pub fn motion(&self) -> &[MotionSpec] {
        &self.motion
    }

    pub fn bop(&self) -> Bop {
        self.bop
    }

    pub fn set_bop(&mut self, bop: Bop) {
        self.bop = bop;
    }

    /// Replaces the motion, part by part. A spec whose preset the part's type
    /// doesn't offer, or a list of the wrong length, is ignored.
    pub fn set_motion(&mut self, specs: &[MotionSpec]) -> bool {
        let parts = self.motion_parts();
        if specs.len() != parts.len()
            || specs
                .iter()
                .zip(&parts)
                .any(|(s, p)| !p.kind.presets().contains(&s.preset))
        {
            return false;
        }
        self.motion = specs
            .iter()
            .map(|s| MotionSpec {
                preset: s.preset,
                strength: s.strength.clamp(0.0, 2.0),
                speed: s.speed.clamp(0.0, 2.0),
            })
            .collect();
        true
    }

    /// The point between the eyes, in rig pixels, for gaze.
    pub fn eye_point(&self) -> Option<Point> {
        match &self.kind {
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

    pub fn visible_bounds(&self) -> Rectangle {
        self.silhouettes
            .iter()
            .filter_map(|s| s.bounds)
            .reduce(|a, b| a.union(&b))
            .unwrap_or(Rectangle::new(Point::ORIGIN, self.size))
    }
    pub fn pet_region(&self, p: Point, pose: &Pose) -> bool {
        let hit = self.hit(p, pose);
        if let Kind::Layered { parts, .. } = &self.kind {
            if let Some(head) = parts.iter().position(|p| p.kind == PartType::Head) {
                return matches!(hit,Some(Layer::Part(i)) if i==head || parts[i].parent==Some(head));
            }
        }
        hit.is_some()
    }

    /// The head's rotation in `pose`, which the gaze is counter-rotated by.
    pub fn head_angle(&self, pose: &Pose) -> f32 {
        let index = match &self.kind {
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
            // One piece: a gentle sway around the feet stands in for the
            // head's idle tilt.
            Kind::Sprite(image) => sprite(
                frame,
                image,
                Point::ORIGIN,
                self.size,
                self.anchor,
                angle(0) * 0.5,
            ),
            Kind::Layered { body, parts, eyes } => {
                let ctx = Layered {
                    parts,
                    eyes,
                    pose,
                    drop: self.head_drop(pose),
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

    /// How far the music head bob moves a head part down, in rig pixels.
    fn head_drop(&self, pose: &Pose) -> f32 {
        pose.head_drop * self.visible_height()
    }

    /// How far part `index` is shifted down by the head bob, in its
    /// parent's space: heads only.
    fn shift(parts: &[Part], index: usize, drop: f32) -> Vector {
        if parts[index].kind == PartType::Head {
            Vector::new(0.0, drop)
        } else {
            Vector::ZERO
        }
    }

    /// Whether a rig-space point lies on visible artwork, unposed.
    #[cfg(test)]
    pub fn opaque(&self, p: Point) -> bool {
        self.silhouettes.iter().any(|s| s.contains(p))
    }

    /// The front-most layer drawn at rig point `p` in `pose`, undoing each
    /// part's rotation the way `draw_pose` applies it. The whole-character
    /// lean, squash and bounce are the host's (see `motion::unplaced`).
    pub fn hit(&self, p: Point, pose: &Pose) -> Option<Layer> {
        let angle = |i: usize| pose.angles.get(i).copied().unwrap_or_default();
        let opaque =
            |layer: usize, p: Point| self.silhouettes.get(layer).is_some_and(|s| s.contains(p));
        match &self.kind {
            Kind::Sprite(_) => {
                opaque(0, unrotate(p, self.anchor, angle(0) * 0.5)).then_some(Layer::Body)
            }
            Kind::Layered { parts, .. } => {
                // Reverse draw order: front roots, the body, behind roots.
                let roots = |behind: bool| {
                    parts
                        .iter()
                        .enumerate()
                        .rev()
                        .filter(move |(_, c)| c.parent.is_none() && c.behind == behind)
                };
                let drop = self.head_drop(pose);
                roots(false)
                    .find_map(|(i, _)| self.hit_part(parts, i, p, &angle, drop))
                    .or_else(|| opaque(0, p).then_some(Layer::Body))
                    .or_else(|| {
                        roots(true).find_map(|(i, _)| self.hit_part(parts, i, p, &angle, drop))
                    })
            }
        }
    }

    /// Hit test within part `index` and its children, `p` in its parent's space.
    fn hit_part(
        &self,
        parts: &[Part],
        index: usize,
        p: Point,
        angle: &dyn Fn(usize) -> f32,
        drop: f32,
    ) -> Option<Layer> {
        let part = &parts[index];
        let local = unrotate(
            p - Self::shift(parts, index, drop),
            part.pivot,
            angle(index),
        );
        let children = |behind: bool| {
            parts
                .iter()
                .enumerate()
                .rev()
                .filter(move |(_, c)| c.parent == Some(index) && c.behind == behind)
        };
        children(false)
            .find_map(|(i, _)| self.hit_part(parts, i, local, angle, drop))
            .or_else(|| {
                self.silhouettes[index + 1]
                    .contains(local)
                    .then_some(Layer::Part(index))
            })
            .or_else(|| {
                children(true).find_map(|(i, _)| self.hit_part(parts, i, local, angle, drop))
            })
    }

    /// Where `draw_pose` draws a point of `layer`'s unposed artwork.
    #[cfg(test)]
    pub fn posed(&self, layer: Layer, p: Point, pose: &Pose) -> Point {
        let angle = |i: usize| pose.angles.get(i).copied().unwrap_or_default();
        let rotate = |q: Point, pivot: Point, a: f32| unrotate(q, pivot, -a);
        match (&self.kind, layer) {
            (Kind::Sprite(_), _) => rotate(p, self.anchor, angle(0) * 0.5),
            (Kind::Layered { .. }, Layer::Body) => p,
            (Kind::Layered { parts, .. }, Layer::Part(mut i)) => {
                let drop = self.head_drop(pose);
                let mut q = p;
                loop {
                    q = rotate(q, parts[i].pivot, angle(i)) + Self::shift(parts, i, drop);
                    match parts[i].parent {
                        Some(parent) => i = parent,
                        None => return q,
                    }
                }
            }
        }
    }

    /// The visible artwork's height at rest, in rig pixels, excluding
    /// transparent stage padding.
    pub fn visible_height(&self) -> f32 {
        (self.visible.1 - self.visible.0).max(1.0)
    }

    /// Canvas size in rig pixels.
    pub fn size(&self) -> Size {
        self.size
    }

    /// Where the feet meet the ground, in rig pixels.
    pub fn anchor(&self) -> Point {
        self.anchor
    }
}

/// A whole-character replacement picture (a held pose or artwork
/// reaction): canvas-sized, drawn at `place` = `[x, y, scale]`, with the
/// offset in canvas fractions.
#[derive(Debug, Clone)]
pub struct Picture {
    pub preserve_aspect: bool,
    pub image: Handle,
    pub place: [f32; 3],
    /// Where a held pose hangs from, in the picture's pixels: the top
    /// centre of its art, the scruff.
    pub hold: Point,
    /// The opaque art's box in the picture's pixels, for keeping a swing
    /// inside the window.
    pub art: Rectangle,
}

impl Picture {
    pub fn new(pixels: &RgbaImage, place: [f32; 3]) -> Self {
        let art = visible(pixels).map_or(
            Rectangle::new(
                Point::ORIGIN,
                Size::new(pixels.width() as f32, pixels.height() as f32),
            ),
            |(x, y, w, h)| {
                Rectangle::new(
                    Point::new(x as f32, y as f32),
                    Size::new(w as f32, h as f32),
                )
            },
        );
        Self {
            image: handle(pixels.clone()),
            preserve_aspect: false,
            place,
            hold: hold_point(pixels),
            art,
        }
    }

    pub fn size_in(&self, stage: Size) -> Size {
        if self.preserve_aspect {
            if let Some((w, h)) = image_size(&self.image) {
                let fit = (stage.width / w).min(stage.height / h);
                return Size::new(w * fit, h * fit);
            }
        }
        stage
    }

    pub fn source_fraction(&self, stage: Size, point: Point) -> Point {
        let size = self.size_in(stage);
        Point::new(
            (point.x - self.place[0] * stage.width) / (size.width * self.place[2]),
            (point.y - self.place[1] * stage.height) / (size.height * self.place[2]),
        )
    }

    /// `p`, in the picture's pixels, in the stage frame as placed.
    pub fn in_stage(&self, stage: Size, p: Point) -> Point {
        let [x, y, k] = self.place;
        let (pw, ph) = image_size(&self.image).unwrap_or((stage.width, stage.height));
        let size = self.size_in(stage);
        Point::new(
            x * stage.width + p.x * size.width * k / pw,
            y * stage.height + p.y * size.height * k / ph,
        )
    }

    /// Alpha hit testing in the same stage coordinates used for drawing.
    pub fn hit(&self, stage: Size, point: Point) -> bool {
        let fraction = self.source_fraction(stage, point);
        if let Handle::Rgba {
            width,
            height,
            pixels,
            ..
        } = &self.image
        {
            let px = fraction.x * *width as f32;
            let py = fraction.y * *height as f32;
            if px >= 0.0 && py >= 0.0 && px < *width as f32 && py < *height as f32 {
                return pixels[((py as u32 * width + px as u32) * 4 + 3) as usize] >= 32;
            }
        }
        false
    }

    /// Resolve an authored grip against the untrimmed source image.
    pub fn with_grip(mut self, grip: Option<[f32; 2]>) -> Self {
        if let Some([x, y]) = grip {
            if let Some((width, height)) = image_size(&self.image) {
                self.hold = Point::new(x * width, y * height);
            }
        }
        self
    }

    /// The hold point in the stage frame, with the picture placed.
    pub fn hold_in(&self, stage: Size) -> Point {
        self.in_stage(stage, self.hold)
    }
}

fn image_size(h: &Handle) -> Option<(f32, f32)> {
    match h {
        Handle::Rgba { width, height, .. } => Some((*width as f32, *height as f32)),
        _ => None,
    }
}

/// The scruff of a held pose: the centre of the top 4 % of its opaque art.
pub fn hold_point(pixels: &RgbaImage) -> Point {
    let (w, h) = pixels.dimensions();
    let Some(top) = (0..h).find(|&y| (0..w).any(|x| pixels.get_pixel(x, y)[3] >= 32)) else {
        return Point::new(w as f32 / 2.0, 0.0);
    };
    let band = ((h as f32 * 0.04).ceil() as u32).max(1);
    let (mut sum, mut n) = (0.0f64, 0u64);
    for y in top..(top + band).min(h) {
        for x in 0..w {
            if pixels.get_pixel(x, y)[3] >= 32 {
                sum += f64::from(x) + 0.5;
                n += 1;
            }
        }
    }
    Point::new(
        (sum / n.max(1) as f64) as f32,
        top as f32 + band as f32 / 2.0,
    )
}

impl Rig {
    /// Draws a held pose hanging from `pivot` (the grabbed point, in the
    /// stage frame), turned `angle` around it. Whatever was grabbed, the
    /// character hangs from its scruff under the cursor.
    pub fn draw_hanging(&self, frame: &mut Frame, picture: &Picture, pivot: Point, angle: f32) {
        let hold = picture.hold_in(self.size);
        frame.with_save(|f| {
            f.translate(Vector::new(pivot.x, pivot.y));
            f.rotate(angle);
            f.translate(Vector::new(-hold.x, -hold.y));
            self.draw_picture(f, picture);
        });
    }

    /// The middle of the visible art at rest, in rig pixels: above it a
    /// grabbed character hangs, below it it stands.
    pub fn visible_middle(&self) -> f32 {
        (self.visible.0 + self.visible.1) / 2.0
    }

    /// Top and bottom of the visible art at rest, in rig pixels.
    pub fn visible_rows(&self) -> (f32, f32) {
        self.visible
    }

    /// Draws `picture` in place of the character, on the same stage.
    pub fn draw_picture(&self, frame: &mut Frame, picture: &Picture) {
        self.draw_picture_alpha(frame, picture, 1.0);
    }

    pub fn draw_picture_alpha(&self, frame: &mut Frame, picture: &Picture, opacity: f32) {
        let [x, y, k] = picture.place;
        let size = picture.size_in(self.size);
        frame.draw_image(
            Rectangle::new(
                Point::new(x * self.size.width, y * self.size.height),
                Size::new(size.width * k, size.height * k),
            ),
            canvas::Image::new(picture.image.clone()).opacity(opacity),
        );
    }
}

fn place(frame: &mut Frame, p: &Piece) {
    frame.draw_image(
        Rectangle::new(p.at, p.size),
        canvas::Image::new(p.image.clone()),
    );
}

/// Draws a layered character's parts with their parents' transforms.
struct Layered<'a> {
    parts: &'a [Part],
    eyes: &'a [LayerEye],
    pose: &'a Pose,
    /// The music head bob, in rig pixels.
    drop: f32,
}

impl Layered<'_> {
    fn part(&self, frame: &mut Frame, index: usize) {
        let p = &self.parts[index];
        let angle = self.pose.angles.get(index).copied().unwrap_or_default();
        frame.with_save(|f| {
            f.translate(Rig::shift(self.parts, index, self.drop));
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
                Expression::Petted => p.faces[3].as_ref(),
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
        let Kind::Layered { eyes, parts, .. } = &rig.kind else {
            panic!("Yohta is a layered pack");
        };
        assert_eq!(eyes.len(), 2);
        assert_eq!(parts.len(), 2, "head and tail");
        assert!(
            eyes.iter()
                .all(|eye| eye.frames.len() == 49 && eye.closed.is_some())
        );
        assert_eq!(rig.size(), Size::new(WIDTH, HEIGHT));
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
                faces: [
                    None,
                    Some(block(20, 10, 80, 55, [250, 0, 0, 255])),
                    None,
                    None,
                ],
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
        assert_eq!(
            (body.at, body.size),
            (Point::new(30.0, 50.0), Size::new(40.0, 45.0))
        );
        assert_eq!(parts[0].pivot, Point::new(50.0, 50.0));
        assert!(parts[0].faces[1].is_some() && parts[0].faces[0].is_none());
        assert_eq!(eyes[0].frames.len(), 49);
        assert_eq!(rig.motion_parts().len(), 1);
        // The head counts for clicks, the empty corners don't.
        assert!(rig.opaque(Point::new(50.0, 20.0)));
        assert!(!rig.opaque(Point::new(3.0, 3.0)));
    }

    #[test]
    fn original_rectangular_art_keeps_aspect_and_inverse_mapping() {
        let pixels = RgbaImage::from_pixel(120, 40, image::Rgba([255; 4]));
        let mut picture = Picture::new(&pixels, [0.1, 0.2, 1.25]);
        picture.preserve_aspect = true;
        let stage = Size::new(200.0, 300.0);
        let size = picture.size_in(stage);
        assert!((size.width / size.height - 3.0).abs() < 0.001);
        let at = picture.in_stage(stage, Point::new(30.0, 20.0));
        let fraction = picture.source_fraction(stage, at);
        assert!((fraction.x - 0.25).abs() < 0.001);
        assert!((fraction.y - 0.5).abs() < 0.001);
        assert!(picture.hit(stage, at));
        assert!(!picture.hit(stage, Point::ORIGIN));
    }

    #[test]
    fn authored_grip_uses_source_padding_and_placement() {
        let pixels = RgbaImage::new(200, 100);
        let picture = Picture::new(&pixels, [0.1, -0.2, 1.25]).with_grip(Some([0.3, 0.4]));
        let grip = picture.hold_in(Size::new(400.0, 200.0));
        assert!((grip.x - 190.0).abs() < 0.001);
        assert!((grip.y - 60.0).abs() < 0.001);
    }

    #[test]
    fn a_held_pose_hangs_from_the_top_of_its_art() {
        let mut img = RgbaImage::new(100, 200);
        for y in 20..180 {
            for x in 40..60 {
                img.put_pixel(x, y, Rgba([90, 120, 200, 255]));
            }
        }
        let p = hold_point(&img);
        assert!(
            (p.x - 50.0).abs() < 0.5 && (p.y - 24.0).abs() < 1.0,
            "{p:?}"
        );
        // Placed at half size and shifted, the hold point follows.
        let pic = Picture::new(&img, [0.1, 0.0, 0.5]);
        let at = pic.hold_in(Size::new(100.0, 200.0));
        assert!(
            (at.x - 35.0).abs() < 0.5 && (at.y - 12.0).abs() < 1.0,
            "{at:?}"
        );
    }

    #[test]
    fn visible_height_ignores_padding() {
        let mut img = RgbaImage::new(40, 200);
        for y in 50..150 {
            for x in 10..30 {
                img.put_pixel(x, y, Rgba([90, 120, 200, 255]));
            }
        }
        assert_eq!(Rig::sprite(img, (0.5, 0.75)).visible_height(), 100.0);
        let yohta = Rig::load();
        assert!(yohta.visible_height() > 600.0 && yohta.visible_height() <= HEIGHT);
        println!("yohta visible/canvas = {}", yohta.visible_height() / HEIGHT);
    }

    #[test]
    fn hit_testing_follows_the_posed_head() {
        let rig = Rig::layered(layered_art());
        let rest = Pose {
            angles: vec![0.0],
            ..Pose::default()
        };
        let tilted = Pose {
            angles: vec![0.6],
            ..Pose::default()
        };
        // The head's top-left corner: on the head at rest, empty once the
        // head turns away from it.
        let corner = Point::new(22.0, 12.0);
        assert_eq!(rig.hit(corner, &rest), Some(Layer::Part(0)));
        assert_eq!(rig.hit(corner, &tilted), None);
        // Points of the head art are hit wherever the pose draws them.
        for p in [(25.0, 15.0), (75.0, 20.0), (50.0, 30.0)] {
            let drawn = rig.posed(Layer::Part(0), Point::new(p.0, p.1), &tilted);
            assert_eq!(
                rig.hit(drawn, &tilted),
                Some(Layer::Part(0)),
                "{p:?} at {drawn:?}"
            );
        }
        // The head is drawn over the body, so the overlap hits the head;
        // below it is body.
        assert_eq!(rig.hit(Point::new(50.0, 52.0), &rest), Some(Layer::Part(0)));
        assert_eq!(rig.hit(Point::new(50.0, 80.0), &tilted), Some(Layer::Body));
    }

    #[test]
    fn sprite_hit_testing_follows_its_sway() {
        let mut img = RgbaImage::new(40, 80);
        for y in 10..70 {
            for x in 10..30 {
                img.put_pixel(x, y, Rgba([90, 120, 200, 255]));
            }
        }
        let rig = Rig::sprite(img, (0.5, 70.0 / 80.0));
        let pose = Pose {
            angles: vec![0.4],
            ..Pose::default()
        };
        let top = rig.posed(Layer::Body, Point::new(12.0, 12.0), &pose);
        assert_eq!(rig.hit(top, &pose), Some(Layer::Body));
    }

    #[test]
    fn pupils_stay_inside_their_mask() {
        let mut art = layered_art();
        // A pupil far bigger than the socket, pushed hard right.
        art.eyes[0].pupil = RgbaImage::from_pixel(100, 100, Rgba([0, 0, 0, 255]));
        art.eyes[0].white = RgbaImage::new(100, 100);
        art.eyes[0].max_offset = (0.2, 0.2);
        let eye = LayerEye::new(&art.eyes[0], 0).unwrap();
        assert_eq!(
            (eye.at, eye.size),
            (Point::new(40.0, 25.0), Size::new(10.0, 10.0))
        );
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

#[cfg(test)]
mod reaction_hit_tests {
    use super::*;
    #[test]
    fn replacement_art_hits_its_placed_alpha() {
        let mut pixels = RgbaImage::new(100, 100);
        pixels.put_pixel(30, 40, Rgba([255, 255, 255, 255]));
        let picture = Picture::new(&pixels, [0.1, 0.2, 0.5]);
        let stage = Size::new(200.0, 300.0);
        let at = picture.in_stage(stage, Point::new(30.5, 40.5));
        assert!(picture.hit(stage, at));
        assert!(!picture.hit(stage, Point::ORIGIN));
        assert!(!picture.hit(stage, picture.in_stage(stage, Point::new(31.5, 40.5))));
    }
}
