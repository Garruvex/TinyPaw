//! The pixel-buffer renderer against the Iced canvas path the desktop draws
//! with today. Each case draws the same pose both ways and compares pixels.

use iced::advanced::{
    Renderer as _,
    graphics::{Viewport, geometry::Renderer as _},
};
use iced::widget::canvas::Frame;
use iced::{Color, Font, Point, Rectangle, Renderer, Size, Vector};
use image::{Rgba, RgbaImage};

use super::motion::{self, Expression, Pose};
use super::render::{Look, Placement, render_character};
use super::rig::{Picture, Rig};
use super::surface::Raster;

/// The desktop's path: an Iced canvas frame drawn by Iced's tiny-skia renderer.
/// Returns RGBA bytes (Iced's pixmap holds BGRA).
fn through_iced(width: u32, height: u32, draw: impl FnOnce(&mut Frame)) -> Vec<u8> {
    let viewport = Viewport::with_physical_size(Size::new(width, height), 1.0);
    let bounds = Rectangle::with_size(viewport.logical_size());
    let mut renderer = Renderer::new(Font::DEFAULT, 14.0.into());
    renderer.reset(bounds);
    let mut frame = Frame::new(&renderer, bounds.size());
    draw(&mut frame);
    renderer.draw_geometry(frame.into_geometry());
    let mut pixels = tiny_skia::Pixmap::new(width, height).unwrap();
    let mut mask = tiny_skia::Mask::new(width, height).unwrap();
    renderer.draw(
        &mut pixels.as_mut(),
        &mut mask,
        &viewport,
        &[bounds],
        Color::TRANSPARENT,
    );
    let mut data = pixels.data().to_vec();
    for px in data.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
    }
    data
}

struct Diff {
    worst: u8,
    over: usize,
    pixels: usize,
}

fn compare(a: &[u8], b: &[u8], tolerance: u8) -> Diff {
    assert_eq!(a.len(), b.len());
    let mut diff = Diff {
        worst: 0,
        over: 0,
        pixels: a.len() / 4,
    };
    for (x, y) in a.as_chunks::<4>().0.iter().zip(b.as_chunks::<4>().0) {
        let d = x.iter().zip(y).map(|(p, q)| p.abs_diff(*q)).max().unwrap();
        diff.worst = diff.worst.max(d);
        if d > tolerance {
            diff.over += 1;
        }
    }
    diff
}

fn rig_pose(rig: &Rig) -> Pose {
    Pose {
        angles: vec![0.0; rig.motion_parts().len()],
        ..Pose::default()
    }
}

/// Draws `look` both ways at `scale`, with the rig's corner at `origin`.
fn both(rig: &Rig, pose: &Pose, look: Look<'_>, origin: Point, scale: f32) -> (Vec<u8>, Vec<u8>) {
    let size = rig.size();
    let (w, h) = (
        (size.width * scale + origin.x * 2.0).ceil() as u32,
        (size.height * scale + origin.y * 2.0).ceil() as u32,
    );
    let iced = through_iced(w, h, |f| {
        f.translate(Vector::new(origin.x, origin.y));
        f.scale(scale);
        match look {
            Look::Hanging {
                picture,
                pivot,
                angle,
            } => rig.draw_hanging(f, picture, pivot, angle),
            Look::Picture(picture) => {
                motion::place(f, pose, rig.anchor(), size.height);
                rig.draw_picture(f, picture);
            }
            Look::Posed => {
                motion::place(f, pose, rig.anchor(), size.height);
                rig.draw_pose(f, pose);
            }
        }
    });
    let mut raster = Raster::new(w, h).unwrap();
    render_character(&mut raster, rig, pose, look, Placement { origin, scale });
    (iced, raster.data().to_vec())
}

fn report(name: &str, d: &Diff) {
    println!(
        "{name}: worst {} over-tolerance {} of {}",
        d.worst, d.over, d.pixels
    );
}

fn block(x: u32, y: u32, color: [u8; 4]) -> RgbaImage {
    RgbaImage::from_fn(200, 280, |px, py| {
        if (x..x + 90).contains(&px) && (y..y + 120).contains(&py) {
            Rgba(color)
        } else {
            Rgba([0, 0, 0, 0])
        }
    })
}

#[test]
fn bundled_character_at_rest_matches_the_canvas() {
    let rig = Rig::load();
    let pose = rig_pose(&rig);
    let (a, b) = both(&rig, &pose, Look::Posed, Point::new(20.0, 10.0), 1.0);
    let d = compare(&a, &b, 2);
    report("rest", &d);
    assert!(
        a.as_chunks::<4>().0.iter().any(|p| p[3] > 0),
        "drew nothing"
    );
    assert!(
        d.over * 1000 <= d.pixels,
        "{} of {} differ",
        d.over,
        d.pixels
    );
}

#[test]
fn scaled_and_posed_character_matches_the_canvas() {
    let rig = Rig::load();
    let mut pose = rig_pose(&rig);
    for (i, angle) in pose.angles.iter_mut().enumerate() {
        *angle = 0.06 * (i as f32 + 1.0) * if i % 2 == 0 { 1.0 } else { -1.0 };
    }
    pose.gaze = Vector::new(0.7, -0.4);
    for scale in [0.5, 0.8] {
        let (a, b) = both(&rig, &pose, Look::Posed, Point::new(16.0, 12.0), scale);
        let d = compare(&a, &b, 2);
        report(&format!("posed {scale}"), &d);
        assert!(
            d.over * 1000 <= d.pixels,
            "{} of {} differ",
            d.over,
            d.pixels
        );
    }
}

#[test]
fn leaning_and_squashed_character_matches_the_canvas() {
    let rig = Rig::load();
    let mut pose = rig_pose(&rig);
    pose.lean = 0.12;
    pose.squash = 0.6;
    pose.bounce = 0.02;
    let (a, b) = both(&rig, &pose, Look::Posed, Point::new(40.0, 20.0), 0.7);
    let d = compare(&a, &b, 2);
    report("lean and squash", &d);
    assert!(
        d.over * 1000 <= d.pixels,
        "{} of {} differ",
        d.over,
        d.pixels
    );
}

#[test]
fn blinking_face_matches_the_canvas() {
    let rig = Rig::load();
    let mut pose = rig_pose(&rig);
    pose.expression = Expression::Blink;
    let (a, b) = both(&rig, &pose, Look::Posed, Point::new(0.0, 0.0), 0.6);
    let d = compare(&a, &b, 2);
    report("blink", &d);
    assert!(
        d.over * 1000 <= d.pixels,
        "{} of {} differ",
        d.over,
        d.pixels
    );
}

#[test]
fn reaction_pictures_match_the_canvas() {
    let rig = Rig::sprite(block(40, 120, [200, 90, 60, 255]), (0.5, 0.9));
    let pose = rig_pose(&rig);
    let held = Picture::new(&block(30, 40, [60, 120, 220, 230]), [0.1, 0.05, 0.8]);
    let (a, b) = both(
        &rig,
        &pose,
        Look::Picture(&held),
        Point::new(10.0, 10.0),
        1.0,
    );
    let d = compare(&a, &b, 2);
    report("picture", &d);
    assert!(
        d.over * 1000 <= d.pixels,
        "{} of {} differ",
        d.over,
        d.pixels
    );
    let look = Look::Hanging {
        picture: &held,
        pivot: Point::new(120.0, 80.0),
        angle: 0.3,
    };
    let (a, b) = both(&rig, &pose, look, Point::new(10.0, 10.0), 1.0);
    let d = compare(&a, &b, 2);
    report("hanging", &d);
    assert!(
        d.over * 1000 <= d.pixels,
        "{} of {} differ",
        d.over,
        d.pixels
    );
}

#[test]
fn raised_light_matches_the_canvas() {
    let rig = Rig::sprite(block(40, 120, [200, 90, 60, 255]), (0.5, 0.9));
    let mut pose = rig_pose(&rig);
    pose.light_lift = 0.9;
    pose.light_swing = 0.3;
    let (a, b) = both(&rig, &pose, Look::Posed, Point::new(0.0, 0.0), 2.0);
    let d = compare(&a, &b, 6);
    report("light wave", &d);
    assert!(
        d.over * 200 <= d.pixels,
        "{} of {} differ",
        d.over,
        d.pixels
    );
}

#[test]
fn straight_output_undoes_premultiplication() {
    let rig = Rig::sprite(block(40, 120, [200, 90, 60, 128]), (0.5, 0.9));
    let pose = rig_pose(&rig);
    let mut raster = Raster::new(200, 280).unwrap();
    render_character(
        &mut raster,
        &rig,
        &pose,
        Look::Posed,
        Placement {
            origin: Point::ORIGIN,
            scale: 1.0,
        },
    );
    let straight = raster.to_straight_rgba();
    let px = &straight[((200 * 180 + 80) * 4) as usize..][..4];
    assert!(px[3].abs_diff(128) <= 1, "{px:?}");
    assert!(
        px[0].abs_diff(200) <= 2 && px[1].abs_diff(90) <= 2,
        "{px:?}"
    );
}

#[test]
#[ignore = "manual measurement; run with --release --ignored --nocapture"]
fn render_cost() {
    let rig = Rig::load();
    let pose = rig_pose(&rig);
    for scale in [0.25_f32, 0.5, 1.0] {
        let size = rig.size();
        let (w, h) = (
            (size.width * scale).ceil() as u32,
            (size.height * scale).ceil() as u32,
        );
        let mut raster = Raster::new(w, h).unwrap();
        let at = Placement {
            origin: Point::ORIGIN,
            scale,
        };
        render_character(&mut raster, &rig, &pose, Look::Posed, at);
        let mut times = Vec::new();
        for _ in 0..200 {
            let started = std::time::Instant::now();
            render_character(&mut raster, &rig, &pose, Look::Posed, at);
            times.push(started.elapsed().as_secs_f64() * 1000.0);
        }
        times.sort_by(f64::total_cmp);
        println!(
            "{w}x{h}: median {:.3} ms, p95 {:.3} ms",
            times[100], times[190]
        );
    }
}
