//! Builds bundled Yohta's character pack from the approved concept sheet, so
//! he loads, edits and exports like any other character. Run when the art
//! changes:
//!
//! ```text
//! cargo test generate_yohta_pack -- --ignored
//! ```
//!
//! It reproduces the earlier hand-built rig exactly: the same crops and
//! positions, the same socket masks, and pupils and closed eyes drawn with
//! the same shapes and colours, now as ordinary pack layers.

#[cfg(test)]
mod generate {
    use std::collections::VecDeque;
    use std::path::Path;

    use image::{Rgba, RgbaImage, imageops};
    use serde_json::json;

    /// The pack's canvas, the earlier rig's coordinate space.
    const W: u32 = 620;
    const H: u32 = 860;

    /// One eye: a seed inside its white socket and the socket's box on the
    /// head crop (x, y, width, height).
    type Socket = ((u32, u32), (u32, u32, u32, u32));
    const EYES: [Socket; 2] = [
        ((98, 355), (70, 317, 57, 73)),
        ((245, 355), (177, 309, 164, 90)),
    ];

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

    fn placed(art: &RgbaImage, x: i64, y: i64) -> RgbaImage {
        let mut canvas = RgbaImage::new(W, H);
        imageops::replace(&mut canvas, art, x, y);
        canvas
    }

    /// The white socket pixels connected to `seed`, inside `region`.
    fn socket(head: &RgbaImage, seed: (u32, u32), region: (u32, u32, u32, u32)) -> Vec<bool> {
        let (rx, ry, w, h) = region;
        let mut mask = vec![false; (w * h) as usize];
        let mut queue = VecDeque::from([seed]);
        while let Some((x, y)) = queue.pop_front() {
            if x < rx || y < ry || x >= rx + w || y >= ry + h {
                continue;
            }
            let i = ((y - ry) * w + x - rx) as usize;
            let p = head.get_pixel(x, y);
            if mask[i] || p[3] < 100 || p[0].min(p[1]).min(p[2]) < 185 {
                continue;
            }
            mask[i] = true;
            queue.extend([(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]);
        }
        mask
    }

    #[test]
    #[ignore = "writes assets/mascot/yohta/pack from the approved sheet"]
    fn generate_yohta_pack() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/mascot/yohta");
        let sheet = image::open(root.join("source/approved.png"))
            .expect("approved sheet")
            .to_rgba8();
        let crop = |x, y, w, h| imageops::crop_imm(&sheet, x, y, w, h).to_image();
        let mut head = crop(688, 60, 544, 520);
        isolate(&mut head, (250, 200));
        let body = crop(720, 638, 410, 350);
        // The earlier rig drew the tail crop into a 308 × 312 box.
        let tail = imageops::resize(
            &crop(1138, 600, 385, 390),
            308,
            312,
            imageops::FilterType::Lanczos3,
        );

        let out = root.join("pack");
        let sit = out.join("sit");
        std::fs::create_dir_all(&sit).unwrap();
        let save = |img: &RgbaImage, name: &str| img.save(sit.join(name)).unwrap();
        let head_c = placed(&head, 0, 0);
        let body_c = placed(&body, 91, 457);
        let tail_c = placed(&tail, 302, 452);
        save(&head_c, "head.png");
        save(&body_c, "body.png");
        save(&tail_c, "tail.png");

        let mut eyes = Vec::new();
        for (n, (seed, region)) in EYES.into_iter().enumerate() {
            let mask = socket(&head, seed, region);
            assert!(mask.iter().filter(|&&m| m).count() > 300, "eye {n} socket");
            let (rx, ry, w, h) = region;
            let inside = |x: u32, y: u32| {
                x >= rx
                    && y >= ry
                    && x < rx + w
                    && y < ry + h
                    && mask[((y - ry) * w + x - rx) as usize]
            };
            // Socket shape, its white art, the iris and pupil, and the lid.
            let mask_c = RgbaImage::from_fn(W, H, |x, y| {
                if inside(x, y) {
                    Rgba([255, 255, 255, 255])
                } else {
                    Rgba([0, 0, 0, 0])
                }
            });
            let white_c = RgbaImage::from_fn(W, H, |x, y| {
                if inside(x, y) {
                    *head.get_pixel(x, y)
                } else {
                    Rgba([0, 0, 0, 0])
                }
            });
            let (cx, cy) = (rx as f32 + w as f32 * 0.50, ry as f32 + h as f32 * 0.48);
            let pupil_c = RgbaImage::from_fn(W, H, |x, y| {
                let (px, py) = (x as f32, y as f32);
                let dx = (px - cx) / (w as f32 * 0.31);
                let dy = (py - cy) / (h as f32 * 0.74);
                if dx * dx + dy * dy >= 1.0 {
                    return Rgba([0, 0, 0, 0]);
                }
                let pupil = ((px - cx) / (w as f32 * 0.085)).powi(2)
                    + ((py - cy) / (h as f32 * 0.49)).powi(2);
                if pupil < 1.0 {
                    Rgba([8, 16, 28, 254])
                } else {
                    Rgba([0, 229, 229, 254])
                }
            });
            let blink_c = RgbaImage::from_fn(W, H, |x, y| {
                if !inside(x, y) {
                    return Rgba([0, 0, 0, 0]);
                }
                let (lx, ly) = ((x - rx) as f32, (y - ry) as f32);
                let curve = h as f32 * (0.60 + 0.10 * ((lx / w as f32 - 0.5) * 2.0).powi(2));
                if (ly - curve).abs() < 2.5 {
                    Rgba([12, 17, 48, 254])
                } else {
                    Rgba([28, 36, 104, 254])
                }
            });
            let base = format!("eye-{}", n + 1);
            for (img, part) in [
                (&white_c, "white"),
                (&pupil_c, "pupil"),
                (&mask_c, "mask"),
                (&blink_c, "blink"),
            ] {
                save(img, &format!("{base}-{part}.png"));
            }
            eyes.push(json!({
                "white": format!("sit/{base}-white.png"),
                "pupil": format!("sit/{base}-pupil.png"),
                "mask": format!("sit/{base}-mask.png"),
                "blink": format!("sit/{base}-blink.png"),
                // The earlier rig moved the iris 13 % of the socket across
                // and 10 % down at full gaze.
                "max_offset": [0.13 * w as f32 / W as f32, 0.10 * h as f32 / H as f32],
            }));
        }

        // The feet: the lowest solid row of any layer.
        let bottom = (0..H)
            .rev()
            .find(|&y| {
                (0..W).any(|x| {
                    [&head_c, &body_c, &tail_c]
                        .iter()
                        .any(|l| l.get_pixel(x, y)[3] >= 32)
                })
            })
            .unwrap();
        let manifest = json!({
            "schema_version": 1,
            "id": "yohta",
            "name": "Yohta",
            "author": "TinyPaw",
            "canvas": [W, H],
            "default_pose": "sit",
            "poses": { "sit": {
                "ground_anchor": [0.5, (bottom + 1) as f32 / H as f32],
                "body": "sit/body.png",
                "parts": [
                    { "type": "head", "image": "sit/head.png",
                      "pivot": [269.0 / W as f32, 532.0 / H as f32], "layer": "front",
                      "motion": { "preset": "tilt", "strength": 1.0, "speed": 1.0 } },
                    { "type": "tail", "image": "sit/tail.png",
                      "pivot": [345.0 / W as f32, 510.0 / H as f32], "layer": "back",
                      "motion": { "preset": "sway", "strength": 1.0, "speed": 1.0 } }
                ],
                "eyes": eyes,
            }},
            "lines": {
                "click": [
                    { "text": "Hey!" },
                    { "text": "That tickles!" },
                    { "text": "Mrrp?" },
                ],
                "drag": [{ "text": "Whoa, where are we going?" }, { "text": "Wheee!" }],
                "drop": [{ "text": "Nice spot." }, { "text": "Here? Okay." }],
                "idle": [
                    { "text": "*yawns*" },
                    { "text": "Need a paw pointing at something?" },
                    { "text": "Still here! // Just resting my eyes." },
                ],
                "annotation_end": [{ "text": "Nice drawing!" }, { "text": "All done?" }],
            },
        });
        std::fs::write(
            out.join("character.json"),
            serde_json::to_string_pretty(&manifest).unwrap() + "\n",
        )
        .unwrap();
    }
}
