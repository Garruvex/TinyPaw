//! Pointer drawing: a pack's PNG sprite, or the vector placeholder when no
//! pack is available. The claw tip sits at the exact input coordinate in every
//! frame; presses compress around it.

use iced::advanced::image::Handle;
use iced::widget::canvas::{self, Frame, LineCap, Path, Stroke};
use iced::{Color, Point, Radians, Rectangle, Size, Vector};

use crate::features::point::BASE_SIZE;
use crate::packs::Pack;
use crate::session::Tip;

/// Sprite height relative to the size setting: claw to wrist end.
const SPRITE_SCALE: f32 = 1.15;

const FUR: Color = Color::from_rgb8(0xF2, 0xA6, 0x5A);
const FUR_DARK: Color = Color::from_rgb8(0xD9, 0x83, 0x3A);
const OUTLINE: Color = Color::from_rgb8(0x2B, 0x1D, 0x14);
const BEAN: Color = Color::from_rgb8(0xF4, 0x9A, 0xA8);
const CLAW: Color = Color::from_rgb8(0xFA, 0xF4, 0xE8);
const ROD: Color = Color::from_rgb8(0x2E, 0x30, 0x36);
const ROD_LIGHT: Color = Color::from_rgb8(0x5A, 0x5E, 0x68);

/// Where the wrist sits in the paw's local, upright coordinates (tip at 0,0).
const WRIST: Vector = Vector::new(7.0, 56.0);

pub fn draw_tip(
    frame: &mut Frame,
    tip: Tip,
    area: Size,
    pack: Option<&Pack>,
    forearm_color: Option<[u8; 4]>,
) {
    match tip {
        Tip::Paw {
            at,
            press,
            scale,
            angle,
            stick,
            stretch,
            grip_x,
        } => {
            let at = Point::new(at.x, at.y);
            match pack {
                Some(pack) => {
                    let mut sprite = Sprite::new(pack, scale);
                    if press > 0.5
                        && let Some(pressed) = &pack.pressed
                    {
                        sprite.image = pressed;
                    }
                    if stick || stretch {
                        let forearm = stretch.then_some(
                            forearm_color
                                .or(pack.accent)
                                .map_or(FUR, crate::view::theme::rgba),
                        );
                        let texture = pack
                            .forearm
                            .as_ref()
                            .map(|arm| (arm, sprite.size.width * pack.joint_width));
                        draw_stick(
                            frame,
                            at,
                            grip_x,
                            press,
                            scale,
                            area,
                            sprite.wrist(),
                            forearm,
                            sprite.size.width * pack.joint_width,
                            texture,
                            |f, angle| sprite.draw(f, at, press, angle),
                        );
                    } else {
                        sprite.draw(frame, at, press, angle);
                    }
                }
                None if stick || stretch => draw_stick(
                    frame,
                    at,
                    grip_x,
                    press,
                    scale,
                    area,
                    WRIST * scale,
                    stretch.then_some(forearm_color.map_or(FUR, crate::view::theme::rgba)),
                    18.0 * scale,
                    None,
                    |f, angle| draw_paw(f, at, press, scale, angle),
                ),
                None => draw_paw(frame, at, press, scale, angle),
            }
        }
        Tip::Laser { at, press } => draw_laser(frame, Point::new(at.x, at.y), press),
    }
}

/// A pack's sprite sized for the current Size setting, positioned from its
/// hotspot so the tip lands on the pointer.
struct Sprite<'a> {
    image: &'a Handle,
    size: Size,
    hotspot: (f32, f32),
    wrist: Option<(f32, f32)>,
}

impl<'a> Sprite<'a> {
    fn new(pack: &'a Pack, scale: f32) -> Self {
        let height = BASE_SIZE * scale * SPRITE_SCALE;
        let width = height * pack.width as f32 / pack.height as f32;
        Self {
            image: &pack.image,
            size: Size::new(width, height),
            hotspot: pack.hotspot,
            wrist: pack.wrist,
        }
    }

    /// The wrist in tip-relative pixels before any press; a pack without
    /// one attaches at the bottom centre.
    fn wrist(&self) -> Vector {
        let (wx, wy) = self.wrist.unwrap_or((0.5, 1.0));
        Vector::new(
            (wx - self.hotspot.0) * self.size.width,
            (wy - self.hotspot.1) * self.size.height,
        )
    }

    fn draw(&self, frame: &mut Frame, tip: Point, press: f32, angle: f32) {
        let s = 1.0 - 0.06 * press.clamp(0.0, 1.0);
        frame.with_save(|f| {
            f.translate(Vector::new(tip.x, tip.y));
            f.rotate(Radians(angle));
            f.scale(s);
            let corner = Point::new(
                -self.hotspot.0 * self.size.width,
                -self.hotspot.1 * self.size.height,
            );
            f.draw_image(
                Rectangle::new(corner, self.size),
                canvas::Image::new(self.image.clone()),
            );
        });
    }
}

fn outline(width: f32) -> Stroke<'static> {
    Stroke::default().with_color(OUTLINE).with_width(width)
}

/// Classic Stick: a slim rod from an off-screen grip below and right of the
/// pointer, with the paw rotated along it. The grip follows `grip_x`, which
/// eases after the pointer; the claw stays on `tip` exactly. `wrist` is where
/// the rod attaches, tip-relative, in the paw's upright coordinates.
#[allow(clippy::too_many_arguments)]
fn draw_stick(
    frame: &mut Frame,
    tip: Point,
    grip_x: f32,
    press: f32,
    scale: f32,
    area: Size,
    wrist: Vector,
    forearm: Option<Color>,
    forearm_width: f32,
    texture: Option<(&crate::packs::Forearm, f32)>,
    draw_paw: impl FnOnce(&mut Frame, f32),
) {
    let grip = Point::new(
        (grip_x + area.width * 0.3).clamp(area.width * 0.2, area.width * 1.05),
        area.height * 1.12,
    );
    let dir = Vector::new(grip.x - tip.x, grip.y - tip.y);
    // Align the artwork's actual claw-to-wrist axis with the shaft.
    let angle = dir.y.atan2(dir.x) - wrist.y.atan2(wrist.x);
    let s = 1.0 - 0.06 * press.clamp(0.0, 1.0);
    let (sin, cos) = angle.sin_cos();
    let wrist = Point::new(
        tip.x + (wrist.x * cos - wrist.y * sin) * s,
        tip.y + (wrist.x * sin + wrist.y * cos) * s,
    );
    if let Some(color) = forearm {
        if let Some((arm, width)) = texture {
            draw_textured_forearm(frame, wrist, grip, tip.x - grip_x, scale, arm, width * s);
        } else {
            draw_forearm(
                frame,
                wrist,
                grip,
                tip.x - grip_x,
                scale,
                color,
                forearm_width * s,
            );
        }
        draw_paw(frame, angle);
        return;
    }
    let rod = Path::line(wrist, grip);
    frame.stroke(
        &rod,
        Stroke::default()
            .with_color(ROD)
            .with_width(7.0 * scale)
            .with_line_cap(LineCap::Round),
    );
    frame.stroke(
        &rod,
        Stroke::default()
            .with_color(ROD_LIGHT)
            .with_width(1.5 * scale)
            .with_line_cap(LineCap::Round),
    );
    draw_paw(frame, angle);
}

fn draw_textured_forearm(
    frame: &mut Frame,
    wrist: Point,
    grip: Point,
    lag: f32,
    scale: f32,
    arm: &crate::packs::Forearm,
    width: f32,
) {
    let direction = grip - wrist;
    let length = direction.x.hypot(direction.y).max(1.0);
    let normal = Vector::new(-direction.y / length, direction.x / length);
    let bend = normal * (lag * 0.2).clamp(-60.0 * scale, 60.0 * scale);
    let count = arm.strips.len();
    // Preserve the first wrist section's aspect ratio. Stretch the remaining limb.
    let native_length = width * arm.height as f32 * (1.0 - arm.anchor.1) / arm.width as f32;
    let collar = (native_length * 0.125).min(length * 0.25);
    let along = |t: f32| {
        if t <= 0.125 {
            collar * t / 0.125 / length
        } else {
            (collar + (length - collar) * (t - 0.125) / 0.875) / length
        }
    };
    let point = |t: f32| {
        let u = along(t);
        wrist + direction * u + bend * (16.0 * u * u * (1.0 - u) * (1.0 - u))
    };
    for (i, image) in arm.strips.iter().enumerate() {
        let start = point(i as f32 / count as f32);
        let end = point((i + 1) as f32 / count as f32);
        let delta = end - start;
        let height = delta.x.hypot(delta.y);
        frame.with_save(|f| {
            f.translate(Vector::new(start.x, start.y));
            f.rotate(Radians(
                delta.y.atan2(delta.x) - std::f32::consts::FRAC_PI_2,
            ));
            // Small overlap hides sampling seams and tucks the first strip under the paw.
            f.draw_image(
                Rectangle::new(
                    Point::new(-arm.anchor.0 * width, -1.0),
                    Size::new(width, height + 2.0),
                ),
                canvas::Image::new(image.clone()),
            );
        });
    }
}

fn draw_forearm(
    frame: &mut Frame,
    wrist: Point,
    grip: Point,
    lag: f32,
    scale: f32,
    color: Color,
    width: f32,
) {
    let direction = grip - wrist;
    let length = direction.x.hypot(direction.y).max(1.0);
    let normal = Vector::new(-direction.y / length, direction.x / length);
    // A bounded bow follows rapid reversals and settles with the eased grip.
    let bend = normal * (lag * 0.2).clamp(-60.0 * scale, 60.0 * scale);
    let near = wrist + direction * 0.3 + bend;
    let far = wrist + direction * 0.7 + bend;
    let narrow = normal * (width * 0.5);
    let wide = normal * (width * 1.2);
    let limb = Path::new(|b| {
        b.move_to(wrist + narrow);
        b.bezier_curve_to(near + narrow, far + wide, grip + wide);
        b.line_to(grip - wide);
        b.bezier_curve_to(far - wide, near - narrow, wrist - narrow);
        b.close();
    });
    frame.fill(&limb, color);
    frame.stroke(&limb, outline(2.0 * scale));
}

fn draw_paw(frame: &mut Frame, tip: Point, press: f32, scale: f32, angle: f32) {
    let s = scale * (1.0 - 0.06 * press.clamp(0.0, 1.0));
    frame.with_save(|f| {
        f.translate(Vector::new(tip.x, tip.y));
        f.rotate(Radians(angle));
        f.scale(s);

        // Soft drop shadow, lifted less while pressed.
        let lift = 4.0 * (1.0 - press);
        let shadow = Path::circle(Point::new(8.0 + lift, 40.0 + lift), 15.0);
        f.fill(
            &shadow,
            Color {
                a: 0.18,
                ..Color::BLACK
            },
        );

        // Wrist tuft.
        let wrist =
            Path::rounded_rectangle(Point::new(-6.0, 44.0), Size::new(26.0, 18.0), 8.0.into());
        f.fill(&wrist, FUR_DARK);
        f.stroke(&wrist, outline(2.0));

        // Palm and curled digits.
        let palm = Path::circle(Point::new(6.0, 37.0), 13.0);
        let curls = [
            Path::circle(Point::new(17.0, 26.0), 5.5),
            Path::circle(Point::new(20.0, 35.0), 5.5),
            Path::circle(Point::new(17.5, 44.0), 5.0),
        ];
        for c in &curls {
            f.fill(c, FUR);
            f.stroke(c, outline(2.0));
        }
        f.fill(&palm, FUR);
        f.stroke(&palm, outline(2.0));

        // Raised pointing digit; its claw tip is the origin.
        let digit =
            Path::rounded_rectangle(Point::new(-5.5, 4.0), Size::new(11.0, 30.0), 5.5.into());
        f.fill(&digit, FUR);
        f.stroke(&digit, outline(2.0));

        f.fill(&Path::circle(Point::new(6.0, 40.0), 5.5), BEAN);
        f.fill(&Path::circle(Point::new(0.0, 12.0), 2.6), BEAN);

        let claw = Path::new(|b| {
            b.move_to(Point::ORIGIN);
            b.line_to(Point::new(-3.0, 6.5));
            b.line_to(Point::new(3.0, 6.5));
            b.close();
        });
        f.fill(&claw, CLAW);
        f.stroke(&claw, outline(1.5));
    });
}

/// Vector pads stay crisp at every pointer size and display scale.
pub fn draw_print(frame: &mut Frame, print: &crate::session::PawPrint, alpha: f32) {
    use crate::session::PawPrintKind;
    let mut color = crate::view::theme::rgba(print.color.0);
    color.a *= alpha;
    frame.with_save(|f| {
        f.translate(Vector::new(print.at.x, print.at.y));
        f.rotate(Radians(print.angle));
        f.scale(print.size / 28.0);
        let dog = print.kind == PawPrintKind::Dog;
        let toes = if dog {
            [
                (-10.0, -3.0, -0.35),
                (-4.0, -9.0, -0.12),
                (4.0, -9.0, 0.12),
                (10.0, -3.0, 0.35),
            ]
        } else {
            [
                (-9.0, -3.0, -0.45),
                (-3.5, -9.0, -0.15),
                (3.5, -9.0, 0.15),
                (9.0, -3.0, 0.45),
            ]
        };
        for (x, y, angle) in toes {
            f.with_save(|f| {
                f.translate(Vector::new(x, y));
                f.rotate(Radians(angle));
                f.scale_nonuniform(Vector::new(if dog { 3.3 } else { 3.0 }, 4.0));
                f.fill(&Path::circle(Point::ORIGIN, 1.0), color);
            });
            if dog {
                let claw = Path::new(|b| {
                    b.move_to(Point::new(x - 1.1, y - 5.0));
                    b.quadratic_curve_to(Point::new(x, y - 9.0), Point::new(x + 1.1, y - 5.0));
                    b.close();
                });
                f.fill(&claw, color);
            }
        }
        let pad = Path::new(|b| {
            b.move_to(Point::new(0.0, if dog { 0.0 } else { 1.5 }));
            b.bezier_curve_to(
                Point::new(-4.0, -4.0),
                Point::new(-6.0, 2.0),
                Point::new(-8.0, 6.0),
            );
            b.bezier_curve_to(
                Point::new(-11.0, 12.0),
                Point::new(-4.0, 13.0),
                Point::new(0.0, 10.5),
            );
            b.bezier_curve_to(
                Point::new(4.0, 13.0),
                Point::new(11.0, 12.0),
                Point::new(8.0, 6.0),
            );
            b.bezier_curve_to(
                Point::new(6.0, 2.0),
                Point::new(4.0, -4.0),
                Point::new(0.0, if dog { 0.0 } else { 1.5 }),
            );
            b.close();
        });
        f.fill(&pad, color);
    });
}

fn draw_laser(frame: &mut Frame, at: Point, press: f32) {
    let pulse = 1.0 + 0.5 * press.clamp(0.0, 1.0);
    frame.fill(
        &Path::circle(at, 11.0 * pulse),
        Color::from_rgba8(0xFF, 0x20, 0x20, 0.28),
    );
    frame.fill(&Path::circle(at, 6.0), Color::from_rgb8(0xFF, 0x2A, 0x2A));
    frame.fill(&Path::circle(at, 2.5), Color::from_rgb8(0xFF, 0xE8, 0xE8));
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::advanced::{graphics::geometry::Renderer as _, renderer::Headless};
    use iced::futures::FutureExt;
    use iced::{Font, Pixels, Renderer};

    #[test]
    fn stamps_render_with_distinct_claws_and_transparent_gaps() {
        use crate::session::{OutputId, PawPrint, PawPrintKind, Pt, Rgba};
        let mut renderer =
            <Renderer as Headless>::new(Font::DEFAULT, Pixels(16.0), Some("tiny-skia"))
                .now_or_never()
                .flatten()
                .expect("tiny-skia renderer");
        let mut frame = Frame::new(&renderer, Size::new(240.0, 120.0));
        for (x, kind) in [(60.0, PawPrintKind::Cat), (180.0, PawPrintKind::Dog)] {
            draw_print(
                &mut frame,
                &PawPrint {
                    output: OutputId(0),
                    at: Pt::new(x, 60.0),
                    kind,
                    color: Rgba([230, 120, 150, 255]),
                    size: 56.0,
                    angle: 0.0,
                    born: 0,
                },
                1.0,
            );
        }
        renderer.draw_geometry(frame.into_geometry());
        let shot = renderer.screenshot(Size::new(240, 120), 1.0, Color::WHITE);
        let pixel = |x: usize, y: usize| &shot[(240 * y + x) * 4..][..3];
        assert_eq!(pixel(60, 72), [230, 120, 150], "cat central pad");
        assert_eq!(pixel(180, 72), [230, 120, 150], "dog central pad");
        assert_eq!(pixel(60, 40), [255, 255, 255], "gap between toes");
        assert_eq!(pixel(52, 28), [255, 255, 255], "cat has no claw");
        assert_ne!(pixel(172, 28), [255, 255, 255], "dog has a claw");
        if let Some(path) = std::env::var_os("TINYPAW_STAMP_PREVIEW") {
            image::save_buffer(path, &shot, 240, 120, image::ColorType::Rgba8).unwrap();
        }
    }

    #[test]
    fn paw_rotation_keeps_the_hotspot_fixed_in_all_presentations() {
        // Red claw at the left, green wrist at the right.
        let pixels: Vec<u8> = (0..16 * 8)
            .flat_map(|i| {
                if i % 16 < 8 {
                    [255, 0, 0, 255]
                } else {
                    [0, 255, 0, 255]
                }
            })
            .collect();
        let pack = Pack {
            id: "sideways",
            name: "Sideways".into(),
            image: Handle::from_rgba(16, 8, pixels),
            width: 16,
            height: 8,
            hotspot: (0.125, 0.5),
            wrist: Some((0.875, 0.5)),
            accent: None,
            forearm: None,
            joint_width: 0.4,
            pressed: None,
            tap: None,
            folder: None,
        };
        for (stick, stretch, textured) in [
            (false, false, false),
            (true, false, false),
            (false, true, false),
            (false, true, true),
        ] {
            let mut pack = pack.clone();
            if textured {
                let rgba = std::sync::Arc::new([0, 0, 255, 255].repeat(16 * 8));
                let mut arm = crate::packs::Forearm {
                    image: Handle::from_rgba(16, 8, rgba.to_vec()),
                    width: 16,
                    height: 8,
                    rgba,
                    anchor: (0.5, 0.0),
                    strips: Vec::new(),
                };
                arm.set_anchor((0.5, 0.0));
                pack.forearm = Some(arm);
            }
            let mut renderer =
                <Renderer as Headless>::new(Font::DEFAULT, Pixels(16.0), Some("tiny-skia"))
                    .now_or_never()
                    .flatten()
                    .expect("synchronous tiny-skia renderer");
            let area = Size::new(200.0, 200.0);
            let mut frame = Frame::new(&renderer, area);
            let pose = Tip::Paw {
                at: crate::session::Pt::new(100.0, 40.0),
                press: 1.0,
                scale: 1.0,
                angle: std::f32::consts::FRAC_PI_2,
                stick,
                stretch,
                grip_x: 40.0, // puts the grip directly below the tip
            };
            let override_color = stretch.then_some([230, 40, 90, 255]);
            draw_tip(&mut frame, pose, area, Some(&pack), override_color);
            renderer.draw_geometry(frame.into_geometry());
            let shot = renderer.screenshot(Size::new(200, 200), 1.0, Color::WHITE);
            let pixel = |y: usize| &shot[(200 * y + 100) * 4..][..3];
            assert_eq!(pixel(40), [255, 0, 0], "claw at the input coordinate");
            assert_eq!(pixel(130), [0, 255, 0], "wrist above the shaft");
            assert_eq!(
                pixel(170) != [255, 255, 255],
                stick || stretch,
                "attachment extends beyond the wrist"
            );
            if textured {
                assert_eq!(
                    pixel(170),
                    [0, 0, 255],
                    "forearm texture is drawn beneath the paw"
                );
            } else if stretch {
                assert_eq!(
                    pixel(170),
                    [230, 40, 90],
                    "generated forearm uses the chosen colour"
                );
            }
        }
    }

    #[test]
    fn generated_forearm_uses_the_pack_width_without_a_texture() {
        let mut pack = crate::packs::load_all().0.remove(0);
        pack.forearm = None;
        let mut widths = Vec::new();
        for joint_width in [0.2, 1.0] {
            pack.joint_width = joint_width;
            let mut renderer =
                <Renderer as Headless>::new(Font::DEFAULT, Pixels(16.0), Some("tiny-skia"))
                    .now_or_never()
                    .flatten()
                    .unwrap();
            let area = Size::new(200.0, 200.0);
            let mut frame = Frame::new(&renderer, area);
            draw_tip(
                &mut frame,
                Tip::Paw {
                    at: crate::session::Pt::new(100.0, 25.0),
                    press: 0.0,
                    scale: 1.0,
                    angle: 0.0,
                    stick: false,
                    stretch: true,
                    grip_x: 40.0,
                },
                area,
                Some(&pack),
                Some([230, 40, 90, 255]),
            );
            renderer.draw_geometry(frame.into_geometry());
            let shot = renderer.screenshot(Size::new(200, 200), 1.0, Color::WHITE);
            widths.push(
                (0..200)
                    .filter(|x| shot[(200 * 170 + x) * 4..][..3] == [230, 40, 90])
                    .count(),
            );
        }
        assert!(
            widths[0] > 0 && widths[1] > widths[0] * 2,
            "width slider did not resize the generated limb: {widths:?}"
        );
    }
}
