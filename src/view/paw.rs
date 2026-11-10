//! Placeholder pointer art drawn with vector primitives. The claw tip sits at
//! the exact input coordinate in every frame; presses compress around it.

use iced::widget::canvas::{Frame, Path, Stroke};
use iced::{Color, Point, Radians, Size, Vector};

use crate::session::Tip;

const FUR: Color = Color::from_rgb8(0xF2, 0xA6, 0x5A);
const FUR_DARK: Color = Color::from_rgb8(0xD9, 0x83, 0x3A);
const OUTLINE: Color = Color::from_rgb8(0x2B, 0x1D, 0x14);
const BEAN: Color = Color::from_rgb8(0xF4, 0x9A, 0xA8);
const CLAW: Color = Color::from_rgb8(0xFA, 0xF4, 0xE8);

pub fn draw_tip(frame: &mut Frame, tip: Tip) {
    match tip {
        Tip::Paw { at, press } => draw_paw(frame, Point::new(at.x, at.y), press),
        Tip::Laser { at, press } => draw_laser(frame, Point::new(at.x, at.y), press),
    }
}

fn outline(width: f32) -> Stroke<'static> {
    Stroke::default().with_color(OUTLINE).with_width(width)
}

fn draw_paw(frame: &mut Frame, tip: Point, press: f32) {
    let scale = 1.0 - 0.06 * press.clamp(0.0, 1.0);
    frame.with_save(|f| {
        f.translate(Vector::new(tip.x, tip.y));
        // Lean the upright paw a little, as if reaching in from the lower right.
        f.rotate(Radians(0.35));
        f.scale(scale);

        // Soft drop shadow, lifted less while pressed.
        let lift = 4.0 * (1.0 - press);
        let shadow = Path::circle(Point::new(8.0 + lift, 40.0 + lift), 15.0);
        f.fill(&shadow, Color { a: 0.18, ..Color::BLACK });

        // Wrist tuft.
        let wrist = Path::rounded_rectangle(Point::new(-6.0, 44.0), Size::new(26.0, 18.0), 8.0.into());
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
        let digit = Path::rounded_rectangle(Point::new(-5.5, 4.0), Size::new(11.0, 30.0), 5.5.into());
        f.fill(&digit, FUR);
        f.stroke(&digit, outline(2.0));

        let bean = Path::circle(Point::new(6.0, 40.0), 5.5);
        f.fill(&bean, BEAN);
        let toe_bean = Path::circle(Point::new(0.0, 12.0), 2.6);
        f.fill(&toe_bean, BEAN);

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
