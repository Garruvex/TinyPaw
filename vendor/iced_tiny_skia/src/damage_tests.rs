use crate::core::{Color, Font, Pixels, Point, Rectangle, Renderer as _, Shadow, Size, Vector, renderer};
use crate::{Renderer, graphics::Viewport};

#[test]
fn partial_redraw_matches_a_fresh_transparent_frame() {
    let size = Size::new(180, 135);
    let viewport = Viewport::with_physical_size(size, 1.25);
    let screen = Rectangle::with_size(viewport.logical_size());
    let mut renderer = Renderer::new(Font::DEFAULT, Pixels(16.0));
    renderer.reset(screen);
    renderer.fill_quad(
        renderer::Quad {
            bounds: Rectangle::new(Point::new(32.0, 30.0), Size::new(80.0, 36.0)),
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.35),
                offset: Vector::new(0.0, 8.0),
                blur_radius: 12.0,
            },
            ..Default::default()
        },
        Color::WHITE,
    );

    let mut pixels = tiny_skia::Pixmap::new(size.width, size.height).unwrap();
    let mut mask = tiny_skia::Mask::new(size.width, size.height).unwrap();
    let mut draw = |pixels: &mut tiny_skia::Pixmap, damage: &[Rectangle]| {
        renderer.draw(&mut pixels.as_mut(), &mut mask, &viewport, damage, Color::TRANSPARENT);
    };
    draw(&mut pixels, &[screen]);
    let expected = pixels.clone();
    // Hover damage inside the panel, followed by damage exclusively in its shadow.
    for origin in [Point::new(42.5, 35.5), Point::new(55.5, 67.5)] {
        draw(&mut pixels, &[Rectangle::new(origin, Size::new(8.0, 5.0))]);
        assert!(pixels.data() == expected.data(), "partial redraw at {origin:?}");
    }
}

#[test]
fn clearing_a_stroke_erases_every_painted_pixel() {
    let size = Size::new(180, 135);
    let viewport = Viewport::with_physical_size(size, 1.25);
    let screen = Rectangle::with_size(viewport.logical_size());
    for (x, y) in [(95.5, 12.5), (16.5, 78.5), (95.5, 78.5)] {
        let mut renderer = Renderer::new(Font::DEFAULT, Pixels(16.0));
        renderer.reset(screen);
        let mut path = tiny_skia::PathBuilder::new();
        path.move_to(16.5, 12.5);
        path.line_to(x, y);
        let stroke = crate::Primitive::Stroke {
            path: path.finish().unwrap(),
            paint: tiny_skia::Paint::default(),
            stroke: tiny_skia::Stroke {
                width: 8.0,
                line_cap: tiny_skia::LineCap::Round,
                line_join: tiny_skia::LineJoin::Round,
                ..Default::default()
            },
        };
        let (layer, transform) = renderer.layers.current_mut();
        layer.draw_primitive_group(vec![stroke], screen, transform);
        let previous = renderer.layers().to_vec();
        let mut pixels = tiny_skia::Pixmap::new(size.width, size.height).unwrap();
        let mut mask = tiny_skia::Mask::new(size.width, size.height).unwrap();
        renderer.draw(&mut pixels.as_mut(), &mut mask, &viewport, &[screen], Color::TRANSPARENT);
        assert!(pixels.data().iter().any(|&v| v != 0));

        renderer.reset(screen);
        let dirty = crate::graphics::damage::diff(
            &previous, renderer.layers(), |layer| vec![layer.bounds], crate::Layer::damage,
        );
        let dirty = crate::graphics::damage::group(dirty, screen);
        renderer.draw(&mut pixels.as_mut(), &mut mask, &viewport, &dirty, Color::TRANSPARENT);
        assert!(pixels.data().iter().all(|&v| v == 0), "stroke ending at ({x}, {y})");
    }
}
