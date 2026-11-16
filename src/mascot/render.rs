//! Draws a character into a pixel buffer. The stage engine uses this for the
//! desktop character and the studio uses it for the preview, so they cannot
//! drift apart. It mirrors what the desktop canvas does for one frame.

use iced::Point;

use super::motion::{self, Pose};
use super::rig::{Picture, Rig};
use super::surface::{Raster, Surface};

/// What to draw in place of, or as, the character this frame.
#[derive(Clone, Copy)]
pub enum Look<'a> {
    /// The rig in its pose.
    Posed,
    /// A reaction picture in place of the rig.
    Picture(&'a Picture),
    /// A held picture hanging from `pivot`, turned `angle` radians.
    Hanging {
        picture: &'a Picture,
        pivot: Point,
        angle: f32,
    },
}

/// Where the rig sits in the buffer: the rig's top-left corner in pixels, and
/// pixels per rig pixel.
#[derive(Clone, Copy)]
pub struct Placement {
    pub origin: Point,
    pub scale: f32,
}

/// Clears `target` and draws one frame of `rig`.
pub fn render_character(
    target: &mut Raster,
    rig: &Rig,
    pose: &Pose,
    look: Look<'_>,
    at: Placement,
) {
    target.clear();
    target.translate(iced::Vector::new(at.origin.x, at.origin.y));
    target.scale(at.scale);
    match look {
        Look::Hanging {
            picture,
            pivot,
            angle,
        } => rig.draw_hanging(target, picture, pivot, angle),
        Look::Picture(picture) => {
            motion::place(target, pose, rig.anchor(), rig.size().height);
            rig.draw_picture(target, picture);
        }
        Look::Posed => {
            motion::place(target, pose, rig.anchor(), rig.size().height);
            rig.draw_pose(target, pose);
        }
    }
}
