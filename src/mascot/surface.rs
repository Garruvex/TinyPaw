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
