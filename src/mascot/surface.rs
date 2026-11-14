//! What the character renderer draws on. The rig draws through [`Surface`], so
//! the same code serves an Iced canvas frame and a plain pixel buffer.
//!
//! [`Raster`] is the pixel-buffer backend (tiny-skia, no Iced renderer). The
//! stage engine draws character windows with it and the studio shows its
//! output as the preview, so both match. Its image placement follows Iced's
//! canvas (`Frame::draw_image` turns a transformed picture into an upright
//! rectangle plus one rotation), so a pose looks the same on either backend.

use std::collections::HashMap;

use iced::advanced::image::{Handle, Id};
use iced::widget::canvas::{self, Frame, Path};
use iced::{Color, Point, Rectangle, Size, Vector};
use tiny_skia::{FillRule, FilterQuality, Paint, PathBuilder, Pixmap, PixmapPaint, Transform};

/// The drawing operations the rig needs.
pub trait Surface: Sized {
    /// Runs `draw` and then restores the transform it started with.
    fn with_save(&mut self, draw: impl FnOnce(&mut Self));
    fn translate(&mut self, by: Vector);
    /// Turns by `angle` radians around the current origin.
    fn rotate(&mut self, angle: f32);
    fn scale(&mut self, by: f32);
    fn scale_nonuniform(&mut self, by: Vector);
    /// Draws a straight-alpha RGBA image into `bounds`.
    fn draw_image(&mut self, bounds: Rectangle, image: &Handle, opacity: f32);
    fn fill_rectangle(&mut self, top_left: Point, size: Size, color: Color);
    fn fill_circle(&mut self, center: Point, radius: f32, color: Color);
}

impl Surface for Frame {
    fn with_save(&mut self, draw: impl FnOnce(&mut Self)) {
        Frame::with_save(self, draw);
    }

    fn translate(&mut self, by: Vector) {
        Frame::translate(self, by);
    }

    fn rotate(&mut self, angle: f32) {
        Frame::rotate(self, angle);
    }

    fn scale(&mut self, by: f32) {
        Frame::scale(self, by);
    }

    fn scale_nonuniform(&mut self, by: Vector) {
        Frame::scale_nonuniform(self, by);
    }

    fn draw_image(&mut self, bounds: Rectangle, image: &Handle, opacity: f32) {
        Frame::draw_image(
            self,
            bounds,
            canvas::Image::new(image.clone()).opacity(opacity),
        );
    }

    fn fill_rectangle(&mut self, top_left: Point, size: Size, color: Color) {
        Frame::fill_rectangle(self, top_left, size, color);
    }

    fn fill_circle(&mut self, center: Point, radius: f32, color: Color) {
        Frame::fill(self, &Path::circle(center, radius), color);
    }
}

/// A pixel buffer the rig draws into: premultiplied RGBA, row-major.
pub struct Raster {
    pixels: Pixmap,
    transform: Transform,
    saved: Vec<Transform>,
    images: HashMap<Id, Option<Pixmap>>,
}

impl Raster {
    /// A transparent buffer, or `None` for an empty size.
    pub fn new(width: u32, height: u32) -> Option<Self> {
        Some(Self {
            pixels: Pixmap::new(width, height)?,
            transform: Transform::identity(),
            saved: Vec::new(),
            images: HashMap::new(),
        })
    }

    /// Clears to transparent and resets the transform. Decoded images are kept.
    pub fn clear(&mut self) {
        self.pixels.fill(tiny_skia::Color::TRANSPARENT);
        self.transform = Transform::identity();
        self.saved.clear();
    }

    pub fn width(&self) -> u32 {
        self.pixels.width()
    }

    pub fn height(&self) -> u32 {
        self.pixels.height()
    }

    /// Premultiplied RGBA bytes.
    pub fn data(&self) -> &[u8] {
        self.pixels.data()
    }

    /// The buffer as straight (unpremultiplied) RGBA, for showing in a toolkit
    /// that expects it.
    pub fn to_straight_rgba(&self) -> Vec<u8> {
        self.pixels
            .pixels()
            .iter()
            .flat_map(|p| {
                let c = p.demultiply();
                [c.red(), c.green(), c.blue(), c.alpha()]
            })
            .collect()
    }

    pub fn pixmap(&self) -> &Pixmap {
        &self.pixels
    }

    /// A point in the current transform's space, in device pixels.
    pub fn device(&self, p: Point) -> Point {
        let mut q = tiny_skia::Point { x: p.x, y: p.y };
        self.transform.map_point(&mut q);
        Point::new(q.x, q.y)
    }

    /// How much the current transform scales lengths (it is uniform where
    /// text is drawn).
    pub fn scale_factor(&self) -> f32 {
        self.transform.sx.hypot(self.transform.ky)
    }

    pub fn fill_path(&mut self, path: &tiny_skia::Path, color: Color) {
        self.pixels.fill_path(
            path,
            &paint(color, true),
            FillRule::Winding,
            self.transform,
            None,
        );
    }

    /// Strokes `path` with round joins.
    pub fn stroke_path(&mut self, path: &tiny_skia::Path, color: Color, width: f32) {
        let stroke = tiny_skia::Stroke {
            width,
            line_join: tiny_skia::LineJoin::Round,
            ..tiny_skia::Stroke::default()
        };
        self.pixels
            .stroke_path(path, &paint(color, true), &stroke, self.transform, None);
    }

    pub fn fill_round_rect(&mut self, rect: Rectangle, radius: f32, color: Color) {
        if let Some(path) = round_rect_path(rect, radius) {
            self.fill_path(&path, color);
        }
    }

    /// Blends `color` through an 8-bit coverage mask whose top-left lands on
    /// device pixel (`x`, `y`): glyphs and tinted icons.
    pub fn blend_mask(
        &mut self,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        mask: &[u8],
        color: Color,
    ) {
        let (w, h) = (self.pixels.width() as i32, self.pixels.height() as i32);
        let stride = self.pixels.width() as usize;
        let data = self.pixels.data_mut();
        for row in 0..height as i32 {
            let py = y + row;
            if py < 0 || py >= h {
                continue;
            }
            for col in 0..width as i32 {
                let px = x + col;
                if px < 0 || px >= w {
                    continue;
                }
                let coverage = mask[(row as u32 * width + col as u32) as usize];
                if coverage == 0 {
                    continue;
                }
                let a = f32::from(coverage) / 255.0 * color.a;
                let at = (py as usize * stride + px as usize) * 4;
                let keep = 1.0 - a;
                for (i, c) in [color.r, color.g, color.b].into_iter().enumerate() {
                    data[at + i] = (c * a * 255.0 + f32::from(data[at + i]) * keep)
                        .round()
                        .min(255.0) as u8;
                }
                data[at + 3] = (a * 255.0 + f32::from(data[at + 3]) * keep)
                    .round()
                    .min(255.0) as u8;
            }
        }
    }

    /// Blends straight RGBA pixels (colour glyphs such as emoji) at device
    /// pixel (`x`, `y`).
    pub fn blend_rgba(
        &mut self,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        rgba: &[u8],
        opacity: f32,
    ) {
        let (w, h) = (self.pixels.width() as i32, self.pixels.height() as i32);
        let stride = self.pixels.width() as usize;
        let data = self.pixels.data_mut();
        for row in 0..height as i32 {
            let py = y + row;
            if py < 0 || py >= h {
                continue;
            }
            for col in 0..width as i32 {
                let px = x + col;
                if px < 0 || px >= w {
                    continue;
                }
                let s = ((row as u32 * width + col as u32) * 4) as usize;
                let a = f32::from(rgba[s + 3]) / 255.0 * opacity;
                if a <= 0.0 {
                    continue;
                }
                let at = (py as usize * stride + px as usize) * 4;
                let keep = 1.0 - a;
                for i in 0..3 {
                    data[at + i] = (f32::from(rgba[s + i]) * a + f32::from(data[at + i]) * keep)
                        .round()
                        .min(255.0) as u8;
                }
                data[at + 3] = (a * 255.0 + f32::from(data[at + 3]) * keep)
                    .round()
                    .min(255.0) as u8;
            }
        }
    }

    /// Decoded images are cached by handle; drop any not in `keep`.
    pub fn retain_images(&mut self, keep: impl Fn(&Id) -> bool) {
        self.images.retain(|id, _| keep(id));
    }
}

/// Straight RGBA handle to a premultiplied pixmap, the way Iced's renderer
/// prepares it.
fn decode(handle: &Handle) -> Option<Pixmap> {
    let Handle::Rgba {
        width,
        height,
        pixels,
        ..
    } = handle
    else {
        return None;
    };
    let mut map = Pixmap::new(*width, *height)?;
    for (out, src) in map.pixels_mut().iter_mut().zip(pixels.as_chunks::<4>().0) {
        *out = tiny_skia::ColorU8::from_rgba(src[0], src[1], src[2], src[3]).premultiply();
    }
    Some(map)
}

/// A rounded rectangle path, corners drawn as quarter circles.
pub fn round_rect_path(rect: Rectangle, radius: f32) -> Option<tiny_skia::Path> {
    let r = radius.min(rect.width / 2.0).min(rect.height / 2.0).max(0.0);
    let (x0, y0, x1, y1) = (rect.x, rect.y, rect.x + rect.width, rect.y + rect.height);
    let k = r * 0.4477;
    let mut b = PathBuilder::new();
    b.move_to(x0 + r, y0);
    b.line_to(x1 - r, y0);
    b.cubic_to(x1 - k, y0, x1, y0 + k, x1, y0 + r);
    b.line_to(x1, y1 - r);
    b.cubic_to(x1, y1 - k, x1 - k, y1, x1 - r, y1);
    b.line_to(x0 + r, y1);
    b.cubic_to(x0 + k, y1, x0, y1 - k, x0, y1 - r);
    b.line_to(x0, y0 + r);
    b.cubic_to(x0, y0 + k, x0 + k, y0, x0 + r, y0);
    b.close();
    b.finish()
}

fn paint(color: Color, anti_alias: bool) -> Paint<'static> {
    let mut paint = Paint {
        anti_alias,
        ..Paint::default()
    };
    paint.set_color_rgba8(
        (color.r * 255.0).round() as u8,
        (color.g * 255.0).round() as u8,
        (color.b * 255.0).round() as u8,
        (color.a.clamp(0.0, 1.0) * 255.0).round() as u8,
    );
    paint
}

impl Surface for Raster {
    fn with_save(&mut self, draw: impl FnOnce(&mut Self)) {
        self.saved.push(self.transform);
        draw(self);
        if let Some(back) = self.saved.pop() {
            self.transform = back;
        }
    }

    fn translate(&mut self, by: Vector) {
        self.transform = self.transform.pre_translate(by.x, by.y);
    }

    fn rotate(&mut self, angle: f32) {
        self.transform = self
            .transform
            .pre_concat(Transform::from_rotate(angle.to_degrees()));
    }

    fn scale(&mut self, by: f32) {
        self.transform = self.transform.pre_scale(by, by);
    }

    fn scale_nonuniform(&mut self, by: Vector) {
        self.transform = self.transform.pre_scale(by.x, by.y);
    }

    fn draw_image(&mut self, bounds: Rectangle, image: &Handle, opacity: f32) {
        // Iced maps three corners through the transform and keeps an upright
        // rectangle plus the rotation, dropping any shear. Do the same so a
        // squashed, leaning pose matches.
        let map = |x: f32, y: f32| {
            let mut p = tiny_skia::Point { x, y };
            self.transform.map_point(&mut p);
            Point::new(p.x, p.y)
        };
        let (placed, rotation) = Rectangle::with_vertices(
            map(bounds.x, bounds.y),
            map(bounds.x + bounds.width, bounds.y),
            map(bounds.x, bounds.y + bounds.height),
        );
        let Some(source) = self
            .images
            .entry(image.id())
            .or_insert_with(|| decode(image))
            .as_ref()
        else {
            return;
        };
        let (sx, sy) = (
            placed.width / source.width() as f32,
            placed.height / source.height() as f32,
        );
        let center = placed.center();
        let transform = Transform::identity()
            .post_rotate_at(f32::from(rotation).to_degrees(), center.x, center.y)
            .pre_translate(placed.x, placed.y)
            .pre_scale(sx, sy);
        self.pixels.draw_pixmap(
            0,
            0,
            source.as_ref(),
            &PixmapPaint {
                quality: FilterQuality::Bilinear,
                opacity,
                ..PixmapPaint::default()
            },
            transform,
            None,
        );
    }

    fn fill_rectangle(&mut self, top_left: Point, size: Size, color: Color) {
        let Some(rect) =
            tiny_skia::Rect::from_xywh(top_left.x, top_left.y, size.width, size.height)
        else {
            return;
        };
        self.pixels.fill_path(
            &PathBuilder::from_rect(rect),
            &paint(color, false),
            FillRule::Winding,
            self.transform,
            None,
        );
    }

    fn fill_circle(&mut self, center: Point, radius: f32, color: Color) {
        let Some(path) = PathBuilder::from_circle(center.x, center.y, radius) else {
            return;
        };
        self.pixels.fill_path(
            &path,
            &paint(color, true),
            FillRule::Winding,
            self.transform,
            None,
        );
    }
}
