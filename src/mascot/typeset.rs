//! Text and icons for the stage's speech bubble and menu. Glyphs are shaped
//! and rasterized with cosmic-text (the engine Iced uses) and icons with
//! resvg, then blended into a [`Raster`]. The font database starts with Segoe
//! UI only, so the stage does not hold every installed font in memory; the
//! full system set loads the first time a line needs another script.

use std::collections::HashMap;
use std::path::PathBuf;

use cosmic_text::{
    Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache, SwashContent, Weight, fontdb,
};
use iced::{Color, Point};

use super::surface::Raster;
use crate::view::icons::{self, Glyph};

/// Line height as a multiple of the font size (Iced's default).
const LINE: f32 = 1.3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Face {
    Regular,
    Semibold,
}

/// How to set a piece of text.
#[derive(Debug, Clone, Copy)]
pub struct Style {
    /// Logical pixels.
    pub size: f32,
    pub face: Face,
    /// Wraps lines at this width, in logical pixels.
    pub wrap: Option<f32>,
    pub color: Color,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key {
    text: String,
    /// Font size in thousandths of a pixel.
    size: u32,
    face: Face,
    /// Wrap width in tenths of a pixel; 0 for none.
    wrap: u32,
}

pub struct Typeset {
    fonts: FontSystem,
    swash: SwashCache,
    shaped: HashMap<Key, Buffer>,
    full: bool,
}

fn windows_fonts() -> Option<PathBuf> {
    std::env::var_os("WINDIR").map(|w| PathBuf::from(w).join("Fonts"))
}

impl Typeset {
    pub fn new() -> Self {
        let mut db = fontdb::Database::new();
        if let Some(dir) = windows_fonts() {
            for file in [
                "segoeui.ttf",
                "seguisb.ttf",
                "segoeuib.ttf",
                "seguisym.ttf",
                "seguiemj.ttf",
            ] {
                let path = dir.join(file);
                if path.exists() {
                    let _ = db.load_font_file(path);
                }
            }
        }
        db.set_sans_serif_family("Segoe UI");
        Self {
            fonts: FontSystem::new_with_locale_and_db("en-US".into(), db),
            swash: SwashCache::new(),
            shaped: HashMap::new(),
            full: false,
        }
    }

    /// Loads every installed font, once, when text needs one the small set
    /// lacks.
    fn need_full(&mut self, text: &str) {
        let latin = |c: char| {
            let c = c as u32;
            c < 0x250 || (0x2000..0x2070).contains(&c) || (0x2190..0x2200).contains(&c)
        };
        if !self.full && !text.chars().all(latin) {
            self.fonts.db_mut().load_system_fonts();
            self.shaped.clear();
            self.full = true;
        }
    }

    fn shape(&mut self, text: &str, size: f32, face: Face, wrap: Option<f32>) -> &Buffer {
        self.need_full(text);
        let key = Key {
            text: text.to_owned(),
            size: (size * 1000.0).round() as u32,
            face,
            wrap: wrap.map_or(0, |w| (w * 10.0).round() as u32),
        };
        if self.shaped.len() > 64 {
            self.shaped.clear();
        }
        let fonts = &mut self.fonts;
        self.shaped.entry(key).or_insert_with(|| {
            let mut buffer = Buffer::new(fonts, Metrics::new(size, size * LINE));
            buffer.set_size(fonts, wrap, None);
            let attrs = Attrs::new()
                .family(Family::Name("Segoe UI"))
                .weight(match face {
                    Face::Regular => Weight::NORMAL,
                    Face::Semibold => Weight::SEMIBOLD,
                });
            buffer.set_text(fonts, text, &attrs, Shaping::Advanced, None);
            buffer.shape_until_scroll(fonts, false);
            buffer
        })
    }

    /// Width and height of `text` at `size` logical pixels, wrapped at `wrap`.
    pub fn measure(&mut self, text: &str, size: f32, face: Face, wrap: Option<f32>) -> (f32, f32) {
        let buffer = self.shape(text, size, face, wrap);
        let mut width: f32 = 0.0;
        let mut height = 0.0;
        for run in buffer.layout_runs() {
            width = width.max(run.line_w);
            height = run.line_top + run.line_height;
        }
        (width, height)
    }

    /// Draws `text` with its top-left at `at`, in the raster's current
    /// transform space. Shaped at the device size so it stays sharp.
    pub fn draw(&mut self, raster: &mut Raster, text: &str, at: Point, style: Style) {
        let Style {
            size,
            face,
            wrap,
            color,
        } = style;
        let scale = raster.scale_factor();
        let origin = raster.device(at);
        let (origin_x, origin_y) = (origin.x.round(), origin.y.round());
        let device = size * scale;
        let wrap = wrap.map(|w| w * scale);
        self.shape(text, device, face, wrap);
        let key = Key {
            text: text.to_owned(),
            size: (device * 1000.0).round() as u32,
            face,
            wrap: wrap.map_or(0, |w| (w * 10.0).round() as u32),
        };
        // `shape` stored the buffer under this key; take it out to draw
        // while the swash cache borrows the font system.
        let Some(buffer) = self.shaped.remove(&key) else {
            return;
        };
        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                let physical = glyph.physical((origin_x, origin_y + run.line_y), 1.0);
                let Some(image) = self
                    .swash
                    .get_image(&mut self.fonts, physical.cache_key)
                    .as_ref()
                else {
                    continue;
                };
                let (w, h) = (image.placement.width, image.placement.height);
                let x = physical.x + image.placement.left;
                let y = physical.y - image.placement.top;
                match image.content {
                    SwashContent::Mask => raster.blend_mask(x, y, w, h, &image.data, color),
                    SwashContent::Color => {
                        raster.blend_rgba(x, y, w, h, &image.data, color.a);
                    }
                    SwashContent::SubpixelMask => {}
                }
            }
        }
        self.shaped.insert(key, buffer);
    }
}

/// Icon coverage masks, rendered once per size.
#[derive(Default)]
pub struct IconCache {
    masks: HashMap<(Glyph, u32), Vec<u8>>,
}

impl IconCache {
    /// Draws `glyph` tinted `color`, `size` logical pixels square with its
    /// top-left at `at`.
    pub fn draw(&mut self, raster: &mut Raster, glyph: Glyph, at: Point, size: f32, color: Color) {
        let px = (size * raster.scale_factor()).round().max(1.0) as u32;
        let origin = raster.device(at);
        let mask = self
            .masks
            .entry((glyph, px))
            .or_insert_with(|| render(glyph, px));
        raster.blend_mask(
            origin.x.round() as i32,
            origin.y.round() as i32,
            px,
            px,
            mask,
            color,
        );
    }
}

fn render(glyph: Glyph, px: u32) -> Vec<u8> {
    let source = icons::svg_source(glyph);
    let Ok(tree) = resvg::usvg::Tree::from_str(&source, &resvg::usvg::Options::default()) else {
        return vec![0; (px * px) as usize];
    };
    let Some(mut map) = tiny_skia::Pixmap::new(px, px) else {
        return vec![0; (px * px) as usize];
    };
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(px as f32 / 24.0, px as f32 / 24.0),
        &mut map.as_mut(),
    );
    map.pixels().iter().map(|p| p.alpha()).collect()
}
