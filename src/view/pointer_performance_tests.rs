//! Repeatable CPU draw timings with the same damage tracking as the window.
use super::*;
use iced::advanced::{
    Renderer as _,
    graphics::{Viewport, damage, geometry::Renderer as _},
};
use std::time::Instant;

#[test]
#[ignore = "manual performance measurement; run with --release --ignored --nocapture"]
fn pointer_render_cost() {
    let pack = crate::packs::load_all().0.remove(0);
    for (width, height) in [(1920, 1080), (2560, 1440)] {
        let viewport = Viewport::with_physical_size(Size::new(width, height), 1.0);
        let bounds = Rectangle::with_size(viewport.logical_size());
        for (label, stick, stretch) in [
            ("furry", false, false),
            ("stick", true, false),
            ("stretch", false, true),
        ] {
            let mut renderer = Renderer::new(iced::Font::DEFAULT, 14.0.into());
            let mut pixels = tiny_skia::Pixmap::new(width, height).unwrap();
            let mut mask = tiny_skia::Mask::new(width, height).unwrap();
            let mut previous = renderer.layers().to_vec();
            let mut elapsed = Vec::new();
            let mut area = 0.0;
            for frame_index in 0..65 {
                let started = Instant::now();
                renderer.reset(bounds);
                let mut frame = canvas::Frame::new(&renderer, bounds.size());
                let x = width as f32 * 0.4 + (frame_index as f32 * 0.12).sin() * 200.0;
                paw::draw_tip(
                    &mut frame,
                    crate::session::Tip::Paw {
                        at: crate::session::Pt::new(x, 200.0),
                        press: 0.0,
                        scale: 1.0,
                        angle: -0.35,
                        stick,
                        stretch,
                        grip_x: x - 80.0,
                    },
                    bounds.size(),
                    Some(&pack),
                    None,
                );
                renderer.draw_geometry(frame.into_geometry());
                let regions = damage::diff(
                    &previous,
                    renderer.layers(),
                    |l| vec![l.bounds],
                    iced_tiny_skia::Layer::damage,
                );
                let regions = damage::group(regions, bounds);
                previous = renderer.layers().to_vec();
                renderer.draw(
                    &mut pixels.as_mut(),
                    &mut mask,
                    &viewport,
                    &regions,
                    Color::TRANSPARENT,
                );
                if frame_index >= 5 {
                    elapsed.push(started.elapsed().as_secs_f64() * 1000.0);
                    area += regions.iter().map(|r| r.width * r.height).sum::<f32>();
                }
            }
            elapsed.sort_by(f64::total_cmp);
            println!(
                "{width}x{height} {label}: median={:.3}ms p95={:.3}ms damaged={:.1}%",
                elapsed[30],
                elapsed[57],
                100.0 * area / 60.0 / (width * height) as f32
            );
        }
    }
}
