//! Repeatable CPU draw timings for annotation: a moving spotlight over a
//! screen of ink, and a stroke being drawn. Same damage tracking and
//! renderer as the overlay window.
use super::*;
use iced::advanced::{
    Renderer as _,
    graphics::{Viewport, damage, geometry::Renderer as _},
};
use std::time::Instant;

use crate::session::{Pt, Rgba, Stroke};

fn scribbles(width: f32, height: f32) -> Vec<Mark> {
    (0..24)
        .map(|k| {
            let y0 = height * (k as f32 + 0.5) / 24.0;
            Mark::Stroke(Stroke {
                output: OutputId(0),
                color: Rgba([0xFF, 0x5A, 0x5F, 0xFF]),
                width: 6.0,
                points: (0..600)
                    .map(|i| {
                        let t = i as f32 / 600.0;
                        Pt::new(
                            width * (0.05 + 0.9 * t),
                            y0 + (t * 40.0 + k as f32).sin() * 18.0,
                        )
                    })
                    .collect(),
                born: 0,
            })
        })
        .collect()
}

struct Bench {
    renderer: Renderer,
    pixels: tiny_skia::Pixmap,
    mask: tiny_skia::Mask,
    previous: Vec<iced_tiny_skia::Layer>,
    viewport: Viewport,
    bounds: Rectangle,
    elapsed: Vec<f64>,
    area: f32,
}

impl Bench {
    fn new(width: u32, height: u32) -> Self {
        let viewport = Viewport::with_physical_size(Size::new(width, height), 1.0);
        let renderer = Renderer::new(iced::Font::DEFAULT, 14.0.into());
        let previous = Vec::new();
        Self {
            bounds: Rectangle::with_size(viewport.logical_size()),
            renderer,
            pixels: tiny_skia::Pixmap::new(width, height).unwrap(),
            mask: tiny_skia::Mask::new(width, height).unwrap(),
            previous,
            viewport,
            elapsed: Vec::new(),
            area: 0.0,
        }
    }

    /// `draw` returns layers back to front, as the overlay's stack does.
    fn frame(&mut self, measure: bool, draw: impl FnOnce(&Renderer, Size) -> Vec<Vec<Geometry>>) {
        let started = Instant::now();
        self.renderer.reset(self.bounds);
        let bounds = self.bounds;
        for (i, layer) in draw(&self.renderer, bounds.size()).into_iter().enumerate() {
            let paint = |r: &mut Renderer| {
                for g in layer {
                    r.draw_geometry(g);
                }
            };
            if i == 0 {
                paint(&mut self.renderer);
            } else {
                self.renderer.with_layer(bounds, paint);
            }
        }
        let regions = if self.previous.is_empty() {
            vec![self.bounds]
        } else {
            damage::diff(
                &self.previous,
                self.renderer.layers(),
                |l| vec![l.bounds],
                iced_tiny_skia::Layer::damage,
            )
        };
        let regions = damage::group(regions, self.bounds);
        self.previous = self.renderer.layers().to_vec();
        self.renderer.draw(
            &mut self.pixels.as_mut(),
            &mut self.mask,
            &self.viewport,
            &regions,
            Color::TRANSPARENT,
        );
        if measure {
            self.elapsed.push(started.elapsed().as_secs_f64() * 1000.0);
            self.area += regions.iter().map(|r| r.width * r.height).sum::<f32>();
        }
    }

    fn report(mut self, label: &str) {
        let n = self.elapsed.len();
        self.elapsed.sort_by(f64::total_cmp);
        println!(
            "{}x{} {label}: median={:.3}ms p95={:.3}ms damaged={:.1}%",
            self.pixels.width(),
            self.pixels.height(),
            self.elapsed[n / 2],
            self.elapsed[n * 95 / 100],
            100.0 * self.area / n as f32 / self.bounds.width / self.bounds.height,
        );
    }
}

#[test]
#[ignore = "manual performance measurement; run with --release --ignored --nocapture"]
fn annotation_render_cost() {
    for (width, height) in [(1920u32, 1080u32), (2560, 1440)] {
        let (w, h) = (width as f32, height as f32);
        let marks = scribbles(w, h);
        let cache = Cache::new();
        let ink = |renderer: &Renderer, size: Size| {
            cache.draw(renderer, size, |f| {
                for m in &marks {
                    draw_mark(f, m, 1.0);
                }
            })
        };

        // Spotlight following the pointer across a screen of ink.
        let mut bench = Bench::new(width, height);
        for i in 0..125 {
            let at = Point::new(w * 0.5 + (i as f32 * 0.07).sin() * w * 0.3, h * 0.5);
            bench.frame(i >= 5, |r, size| {
                vec![ink::spotlight(r, size, at, 160.0, 0.55), vec![ink(r, size)]]
            });
        }
        bench.report("spotlight 160px over ink");

        let mut bench = Bench::new(width, height);
        for i in 0..125 {
            let at = Point::new(w * 0.5 + (i as f32 * 0.07).sin() * w * 0.3, h * 0.5);
            bench.frame(i >= 5, |r, size| {
                vec![ink::spotlight(r, size, at, 160.0, 0.55)]
            });
        }
        bench.report("spotlight 160px, no ink");

        // A long stroke being drawn, one new point per frame.
        let mut bench = Bench::new(width, height);
        let mut stroke = Stroke {
            output: OutputId(0),
            color: Rgba([0x4C, 0xC9, 0xF0, 0xFF]),
            width: 6.0,
            points: Vec::new(),
            born: 0,
        };
        for i in 0..605 {
            let t = i as f32 / 600.0;
            stroke.points.push(Pt::new(
                w * (0.1 + 0.8 * t),
                h * 0.5 + (t * 25.0).sin() * h * 0.3,
            ));
            let draft = Mark::Stroke(stroke.clone());
            bench.frame(i >= 5, |r, size| {
                let mut live = Frame::new(r, size);
                draw_mark(&mut live, &draft, 1.0);
                vec![vec![ink(r, size), live.into_geometry()]]
            });
        }
        bench.report("drawing a 600-point stroke over ink");
    }
}

/// Writes old (raw polyline) and new (smoothed curve) renderings of the same
/// pointer-like stroke to `TINYPAW_STROKE_PNG`, zoomed 4x, for eyeballing.
#[test]
#[ignore = "manual visual check; set TINYPAW_STROKE_PNG and run with --ignored"]
fn stroke_quality_snapshot() {
    let Some(out) = std::env::var_os("TINYPAW_STROKE_PNG") else {
        return;
    };
    let (w, h) = (360u32, 220u32);
    // A slow loop and a quick zigzag, sampled like a mouse: whole pixels,
    // dense when slow and sparse when fast.
    let mut points = Vec::new();
    for i in 0..260 {
        let t = i as f32 / 260.0 * std::f32::consts::TAU;
        points.push(Pt::new(
            (90.0 + 60.0 * t.cos()).round(),
            (100.0 + 60.0 * t.sin()).round(),
        ));
    }
    for i in 0..9 {
        points.push(Pt::new(
            190.0 + i as f32 * 18.0,
            if i % 2 == 0 { 60.0 } else { 150.0 },
        ));
    }
    let render = |smooth: bool| {
        let viewport = Viewport::with_physical_size(Size::new(w, h), 1.0);
        let mut renderer = Renderer::new(iced::Font::DEFAULT, 14.0.into());
        let bounds = Rectangle::with_size(viewport.logical_size());
        renderer.reset(bounds);
        let mut f = Frame::new(&renderer, bounds.size());
        let color = Color::from_rgb8(0x4C, 0xC9, 0xF0);
        if smooth {
            for path in ink::stroke_paths(&points, true) {
                f.stroke(&path, stroke_style(color, 4.0));
            }
        } else {
            let path = Path::new(|b| {
                b.move_to(Point::new(points[0].x, points[0].y));
                for p in &points[1..] {
                    b.line_to(Point::new(p.x, p.y));
                }
            });
            f.stroke(&path, stroke_style(color, 4.0));
        }
        renderer.draw_geometry(f.into_geometry());
        let mut pixels = tiny_skia::Pixmap::new(w, h).unwrap();
        pixels.fill(tiny_skia::Color::from_rgba8(0x16, 0x1A, 0x1D, 0xFF));
        let mut mask = tiny_skia::Mask::new(w, h).unwrap();
        renderer.draw(
            &mut pixels.as_mut(),
            &mut mask,
            &viewport,
            &[bounds],
            Color::from_rgb8(0x16, 0x1A, 0x1D),
        );
        pixels
    };
    let (old, new) = (render(false), render(true));
    let zoom = 4u32;
    let mut img = ::image::RgbaImage::new(w * zoom, h * zoom * 2 + 8);
    for (row, pix) in [(0u32, &old), (h * zoom + 8, &new)] {
        for y in 0..h * zoom {
            for x in 0..w * zoom {
                let p = pix.pixel(x / zoom, y / zoom).unwrap().demultiply();
                // Renderer pixels are BGRA.
                img.put_pixel(
                    x,
                    row + y,
                    ::image::Rgba([p.blue(), p.green(), p.red(), 255]),
                );
            }
        }
    }
    img.save(out).unwrap();
}
