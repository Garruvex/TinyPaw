//! Sprite-region renderer for the approved Yohta concept. The source remains
//! untouched; regions are decoded once, and small masked eye frames are cached.
use iced::advanced::image::Handle;
use iced::widget::canvas::{self, Frame};
use iced::{Point, Rectangle, Size, Vector};
use image::{Rgba, RgbaImage};
use std::collections::VecDeque;

const SOURCE: &[u8] = include_bytes!("../../assets/mascot/yohta/source/approved.png");
pub const WIDTH: f32 = 620.0;
pub const HEIGHT: f32 = 860.0;
const STEPS: i32 = 3;

pub struct Rig {
    head: Handle,
    body: Handle,
    tail: Handle,
    eyes: Vec<Eye>,
}

struct Eye {
    at: Point,
    size: Size,
    frames: Vec<Handle>,
    closed: Handle,
}

fn handle(pixels: RgbaImage) -> Handle {
    Handle::from_rgba(pixels.width(), pixels.height(), pixels.into_raw())
}

impl Rig {
    pub fn load() -> Self {
        let sheet = image::load_from_memory(SOURCE)
            .expect("embedded Yohta PNG")
            .to_rgba8();
        let crop = |x, y, w, h| image::imageops::crop_imm(&sheet, x, y, w, h).to_image();
        let mut head = crop(688, 60, 544, 520);
        isolate(&mut head, (250, 200));
        // Seed points select only the connected white eye sockets, preserving
        // the original outline, muzzle and highlights outside the socket.
        let eyes = vec![
            Eye::new(&head, (98, 355), (70, 317, 57, 73)),
            Eye::new(&head, (245, 355), (177, 309, 164, 90)),
        ];
        Self {
            head: handle(head),
            body: handle(crop(720, 638, 410, 350)),
            tail: handle(crop(1138, 600, 385, 390)),
            eyes,
        }
    }

    /// All positions are in rig coordinates. Rotation is around attachment
    /// points, rather than the centre of the source sprite rectangle.
    pub fn draw(
        &self,
        frame: &mut Frame,
        gaze: Vector,
        head_angle: f32,
        tail_angle: f32,
        blink: bool,
    ) {
        sprite(
            frame,
            &self.tail,
            Point::new(302.0, 452.0),
            Size::new(308.0, 312.0),
            Point::new(345.0, 510.0),
            tail_angle,
        );
        sprite(
            frame,
            &self.body,
            Point::new(91.0, 457.0),
            Size::new(410.0, 350.0),
            Point::new(295.0, 820.0),
            0.0,
        );
        frame.with_save(|f| {
            let pivot = Point::new(269.0, 532.0);
            f.translate(Vector::new(pivot.x, pivot.y));
            f.rotate(head_angle);
            f.translate(Vector::new(-pivot.x, -pivot.y));
            let origin = Point::new(0.0, 0.0);
            f.draw_image(
                Rectangle::new(origin, Size::new(544.0, 520.0)),
                canvas::Image::new(self.head.clone()),
            );
            for eye in &self.eyes {
                let index = gaze_index(gaze);
                let image = if blink {
                    &eye.closed
                } else {
                    &eye.frames[index]
                };
                f.draw_image(
                    Rectangle::new(eye.at, eye.size),
                    canvas::Image::new(image.clone()),
                );
            }
        });
    }
}

/// A source rectangle can include a few pixels from a neighbouring part.
/// Keep only the connected sprite under the seed, including its soft edge.
fn isolate(pixels: &mut RgbaImage, seed: (u32, u32)) {
    let (w, h) = pixels.dimensions();
    let mut keep = vec![false; (w * h) as usize];
    let mut queue = VecDeque::from([seed]);
    while let Some((x, y)) = queue.pop_front() {
        if x >= w || y >= h {
            continue;
        }
        let index = (y * w + x) as usize;
        if keep[index] || pixels.get_pixel(x, y)[3] < 8 {
            continue;
        }
        keep[index] = true;
        if x > 0 {
            queue.push_back((x - 1, y));
        }
        if y > 0 {
            queue.push_back((x, y - 1));
        }
        queue.extend([(x + 1, y), (x, y + 1)]);
    }
    for (index, pixel) in pixels.pixels_mut().enumerate() {
        if !keep[index] {
            *pixel = Rgba([0, 0, 0, 0]);
        }
    }
}

fn sprite(frame: &mut Frame, image: &Handle, at: Point, size: Size, pivot: Point, angle: f32) {
    frame.with_save(|f| {
        f.translate(Vector::new(pivot.x, pivot.y));
        f.rotate(angle);
        f.translate(Vector::new(-pivot.x, -pivot.y));
        f.draw_image(Rectangle::new(at, size), canvas::Image::new(image.clone()));
    });
}

pub fn gaze_index(gaze: Vector) -> usize {
    let quantize = |v: f32| (v.clamp(-1.0, 1.0) * STEPS as f32).round() as i32 + STEPS;
    (quantize(gaze.y) * (STEPS * 2 + 1) + quantize(gaze.x)) as usize
}

impl Eye {
    fn new(head: &RgbaImage, seed: (u32, u32), region: (u32, u32, u32, u32)) -> Self {
        let (rx, ry, w, h) = region;
        let mut mask = vec![false; (w * h) as usize];
        let mut queue = VecDeque::from([seed]);
        while let Some((x, y)) = queue.pop_front() {
            if x < rx || y < ry || x >= rx + w || y >= ry + h {
                continue;
            }
            let index = ((y - ry) * w + x - rx) as usize;
            let p = head.get_pixel(x, y);
            if mask[index] || p[3] < 100 || p[0].min(p[1]).min(p[2]) < 185 {
                continue;
            }
            mask[index] = true;
            queue.extend([(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]);
        }
        assert!(
            mask.iter().filter(|&&v| v).count() > 300,
            "Yohta eye mask is empty"
        );
        let make = |gx: f32, gy: f32, closed: bool| {
            RgbaImage::from_fn(w, h, |x, y| {
                if !mask[(y * w + x) as usize] {
                    return Rgba([0, 0, 0, 0]);
                }
                let px = x as f32;
                let py = y as f32;
                if closed {
                    let curve = h as f32 * (0.60 + 0.10 * ((px / w as f32 - 0.5) * 2.0).powi(2));
                    return if (py - curve).abs() < 2.5 {
                        Rgba([12, 17, 48, 254])
                    } else {
                        Rgba([28, 36, 104, 254])
                    };
                }
                let cx = w as f32 * (0.50 + gx * 0.13);
                let cy = h as f32 * (0.48 + gy * 0.10);
                let dx = (px - cx) / (w as f32 * 0.31);
                let dy = (py - cy) / (h as f32 * 0.74);
                if dx * dx + dy * dy < 1.0 {
                    let pupil = ((px - cx) / (w as f32 * 0.085)).powi(2)
                        + ((py - cy) / (h as f32 * 0.49)).powi(2);
                    if pupil < 1.0 {
                        Rgba([8, 16, 28, 254])
                    } else {
                        Rgba([0, 229, 229, 254])
                    }
                } else {
                    *head.get_pixel(rx + x, ry + y)
                }
            })
        };
        let mut frames = Vec::new();
        for y in -STEPS..=STEPS {
            for x in -STEPS..=STEPS {
                frames.push(handle(make(
                    x as f32 / STEPS as f32,
                    y as f32 / STEPS as f32,
                    false,
                )));
            }
        }
        Self {
            at: Point::new(rx as f32, ry as f32),
            size: Size::new(w as f32, h as f32),
            frames,
            closed: handle(make(0.0, 0.0, true)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gaze_is_bounded_and_centre_is_stable() {
        assert_eq!(gaze_index(Vector::ZERO), 24);
        assert_eq!(gaze_index(Vector::new(-100.0, -100.0)), 0);
        assert_eq!(gaze_index(Vector::new(100.0, 100.0)), 48);
    }
    #[test]
    fn approved_sheet_has_valid_regions_and_eye_masks() {
        let rig = Rig::load();
        assert_eq!(rig.eyes.len(), 2);
        assert!(rig.eyes.iter().all(|eye| eye.frames.len() == 49));
    }
}
