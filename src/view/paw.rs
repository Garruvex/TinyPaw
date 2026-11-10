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
/// Lean of the free-standing paw, as if reaching in from the lower right.
const FURRY_LEAN: f32 = 0.35;

pub fn draw_tip(frame: &mut Frame, tip: Tip, area: Size, pack: Option<&Pack>) {
    match tip {
        Tip::Paw {
            at,
            press,
            scale,
            stick,
            grip_x,
        } => {
            let at = Point::new(at.x, at.y);
            match pack {
                Some(pack) => {
                    let sprite = Sprite::new(pack, scale);
                    if stick {
                        draw_stick(frame, at, grip_x, press, scale, area, sprite.wrist(), |f, angle| {
                            sprite.draw(f, at, press, angle)
                        });
                    } else {
                        sprite.draw(frame, at, press, 0.0);
                    }
                }
                None if stick => draw_stick(frame, at, grip_x, press, scale, area, WRIST * scale, |f, angle| {
                    draw_paw(f, at, press, scale, angle)
                }),
                None => draw_paw(frame, at, press, scale, FURRY_LEAN),
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
    draw_paw: impl FnOnce(&mut Frame, f32),
) {
    let grip = Point::new(
        (grip_x + area.width * 0.3).clamp(area.width * 0.2, area.width * 1.05),
        area.height * 1.12,
    );
    let dir = Vector::new(grip.x - tip.x, grip.y - tip.y);
    // Rotate local "down" (0,1) onto the direction of the grip.
    let angle = dir.y.atan2(dir.x) - std::f32::consts::FRAC_PI_2;
    let s = 1.0 - 0.06 * press.clamp(0.0, 1.0);
    let (sin, cos) = angle.sin_cos();
    let wrist = Point::new(
        tip.x + (wrist.x * cos - wrist.y * sin) * s,
        tip.y + (wrist.x * sin + wrist.y * cos) * s,
    );
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

fn draw_paw(frame: &mut Frame, tip: Point, press: f32, scale: f32, angle: f32) {
    let s = scale * (1.0 - 0.06 * press.clamp(0.0, 1.0));
    frame.with_save(|f| {
        f.translate(Vector::new(tip.x, tip.y));
        f.rotate(Radians(angle));
        f.scale(s);

        // Soft drop shadow, lifted less while pressed.
        let lift = 4.0 * (1.0 - press);
        let shadow = Path::circle(Point::new(8.0 + lift, 40.0 + lift), 15.0);
        f.fill(&shadow, Color { a: 0.18, ..Color::BLACK });

        // Wrist tuft.
        let wrist = Path::rounded_rectangle(
            Point::new(-6.0, 44.0),
            Size::new(26.0, 18.0),
            8.0.into(),
        );
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
        let digit = Path::rounded_rectangle(
            Point::new(-5.5, 4.0),
            Size::new(11.0, 30.0),
            5.5.into(),
        );
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

fn draw_laser(frame: &mut Frame, at: Point, press: f32) {
    let pulse = 1.0 + 0.5 * press.clamp(0.0, 1.0);
    frame.fill(
        &Path::circle(at, 11.0 * pulse),
        Color::from_rgba8(0xFF, 0x20, 0x20, 0.28),
    );
    frame.fill(&Path::circle(at, 6.0), Color::from_rgb8(0xFF, 0x2A, 0x2A));
    frame.fill(&Path::circle(at, 2.5), Color::from_rgb8(0xFF, 0xE8, 0xE8));
}

/// The upright paw, scaled to fit a preview tile of `height` px.
pub fn draw_preview_paw(frame: &mut Frame, tip: Point, height: f32) {
    draw_paw(frame, tip, 0.0, height / 64.0, FURRY_LEAN);
}
