//! Source-to-canvas placement shared by authoring, baking and editor metadata.
use image::{Rgba, RgbaImage};
use serde_json::{Value as Json, json};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// Canvas-space top-left before rotation about the source centre.
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub angle: f32,
}

impl Default for Placement {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            scale: 1.0,
            angle: 0.0,
        }
    }
}

impl Placement {
    pub fn centred(size: (u32, u32), canvas: (u32, u32)) -> Self {
        let scale = (canvas.0 as f32 / size.0 as f32)
            .min(canvas.1 as f32 / size.1 as f32)
            .min(1.0);
        Self {
            x: (canvas.0 as f32 - size.0 as f32 * scale) / 2.0,
            y: (canvas.1 as f32 - size.1 as f32 * scale) / 2.0,
            scale,
            angle: 0.0,
        }
    }

    pub fn map(self, point: (f32, f32), size: (u32, u32)) -> (f32, f32) {
        let (cx, cy) = (size.0 as f32 / 2.0, size.1 as f32 / 2.0);
        let (sin, cos) = self.angle.to_radians().sin_cos();
        let (x, y) = ((point.0 - cx) * self.scale, (point.1 - cy) * self.scale);
        (
            self.x + cx * self.scale + x * cos - y * sin,
            self.y + cy * self.scale + x * sin + y * cos,
        )
    }

    pub fn unmap(self, point: (f32, f32), size: (u32, u32)) -> (f32, f32) {
        let (cx, cy) = (size.0 as f32 / 2.0, size.1 as f32 / 2.0);
        let (sin, cos) = self.angle.to_radians().sin_cos();
        let (x, y) = (
            point.0 - self.x - cx * self.scale,
            point.1 - self.y - cy * self.scale,
        );
        (
            (x * cos + y * sin) / self.scale + cx,
            (-x * sin + y * cos) / self.scale + cy,
        )
    }

    pub fn valid(self, canvas: (u32, u32)) -> bool {
        [self.x, self.y, self.scale, self.angle]
            .iter()
            .all(|n| n.is_finite())
            && (0.01..=8.0).contains(&self.scale)
            && (-180.0..=180.0).contains(&self.angle)
            && self.x.abs() <= canvas.0 as f32 * 2.0
            && self.y.abs() <= canvas.1 as f32 * 2.0
    }
}

/// Bilinear sampling in premultiplied alpha avoids dark fringes at rotated edges.
/// Pixel centres use the same source coordinate convention as the preview.
pub fn bake(source: &RgbaImage, placement: Placement, canvas: (u32, u32)) -> RgbaImage {
    if placement == Placement::default() && source.dimensions() == canvas {
        return source.clone();
    }
    RgbaImage::from_fn(canvas.0, canvas.1, |x, y| {
        let (sx, sy) = placement.unmap((x as f32 + 0.5, y as f32 + 0.5), source.dimensions());
        let (sx, sy) = (sx - 0.5, sy - 0.5);
        let (ix, iy) = (sx.floor() as i32, sy.floor() as i32);
        let (fx, fy) = (sx - sx.floor(), sy - sy.floor());
        let mut sum = [0.0; 4];
        for (dx, dy, weight) in [
            (0, 0, (1.0 - fx) * (1.0 - fy)),
            (1, 0, fx * (1.0 - fy)),
            (0, 1, (1.0 - fx) * fy),
            (1, 1, fx * fy),
        ] {
            let (px, py) = (ix + dx, iy + dy);
            if px >= 0 && py >= 0 && px < source.width() as i32 && py < source.height() as i32 {
                let p = source.get_pixel(px as u32, py as u32);
                let alpha = p[3] as f32 / 255.0 * weight;
                for c in 0..3 {
                    sum[c] += p[c] as f32 * alpha;
                }
                sum[3] += alpha;
            }
        }
        if sum[3] <= 0.0 {
            Rgba([0, 0, 0, 0])
        } else {
            Rgba([
                (sum[0] / sum[3]).round() as u8,
                (sum[1] / sum[3]).round() as u8,
                (sum[2] / sum[3]).round() as u8,
                (sum[3] * 255.0).round() as u8,
            ])
        }
    })
}

#[derive(Debug, Clone)]
pub struct SourceRecord {
    pub target: String,
    pub source: String,
    pub size: (u32, u32),
    pub placement: Placement,
}

pub fn to_json(records: &[SourceRecord]) -> Json {
    json!({ "version": 1, "layers": records.iter().map(|r| json!({
        "target": r.target, "source": r.source, "size": [r.size.0,r.size.1],
        "placement": [r.placement.x,r.placement.y,r.placement.scale,r.placement.angle]
    })).collect::<Vec<_>>() })
}

pub fn parse(
    value: &Json,
    canvas: (u32, u32),
    targets: &[&str],
) -> Result<Vec<SourceRecord>, String> {
    if value.is_null() {
        return Ok(Vec::new());
    }
    if value["version"].as_u64() != Some(1) {
        return Err("unsupported editor metadata version".into());
    }
    let list = value["layers"]
        .as_array()
        .ok_or("editor layers must be a list")?;
    if list.len() > 32 {
        return Err("editor metadata has too many source images".into());
    }
    let mut records: Vec<SourceRecord> = Vec::new();
    let mut bytes = 0u64;
    for item in list {
        let target = item["target"]
            .as_str()
            .filter(|t| targets.contains(t))
            .ok_or("editor target is not a declared pack image")?;
        let source = item["source"]
            .as_str()
            .filter(|s| {
                super::pack::valid_asset(s)
                    && s.starts_with("editor/sources/")
                    && !targets.contains(s)
            })
            .ok_or("invalid editor source path")?;
        if records
            .iter()
            .any(|r| r.target == target || r.source == source)
        {
            return Err("duplicate editor source or target".into());
        }
        let size = item["size"]
            .as_array()
            .filter(|a| a.len() == 2)
            .ok_or("invalid editor source dimensions")?;
        let side = |v: &Json| {
            v.as_u64()
                .filter(|s| (1..=8192).contains(s))
                .map(|s| s as u32)
                .ok_or("editor source side must be 1–8192 pixels")
        };
        let size = (side(&size[0])?, side(&size[1])?);
        bytes += u64::from(size.0) * u64::from(size.1) * 8;
        if bytes > 128 * 1024 * 1024 {
            return Err("editor sources exceed the 128 MiB decoded memory budget".into());
        }
        let p = item["placement"]
            .as_array()
            .filter(|a| a.len() == 4)
            .ok_or("invalid editor placement")?;
        let number = |v: &Json| {
            v.as_f64()
                .map(|n| n as f32)
                .ok_or("invalid editor placement number")
        };
        let placement = Placement {
            x: number(&p[0])?,
            y: number(&p[1])?,
            scale: number(&p[2])?,
            angle: number(&p[3])?,
        };
        if !placement.valid(canvas) {
            return Err("editor placement is outside supported limits".into());
        }
        records.push(SourceRecord {
            target: target.into(),
            source: source.into(),
            size,
            placement,
        });
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editor_metadata_rejects_unsafe_paths_sizes_and_transforms() {
        let r = SourceRecord {
            target: "body.png".into(),
            source: "editor/sources/body.png".into(),
            size: (16, 20),
            placement: Placement::default(),
        };
        let good = to_json(&[r]);
        assert_eq!(parse(&good, (100, 100), &["body.png"]).unwrap().len(), 1);
        for (key, value) in [
            ("source", json!("../secret.png")),
            ("size", json!([999999, 999999])),
            ("placement", json!([0, 0, 0, 0])),
            ("target", json!("missing.png")),
        ] {
            let mut bad = good.clone();
            bad["layers"][0][key] = value;
            assert!(parse(&bad, (100, 100), &["body.png"]).is_err());
        }
    }

    #[test]
    fn placement_inverse_and_bake_agree() {
        let p = Placement {
            x: 8.0,
            y: 10.0,
            scale: 2.0,
            angle: 90.0,
        };
        let point = (1.5, 2.5);
        let mapped = p.map(point, (4, 6));
        let back = p.unmap(mapped, (4, 6));
        assert!((back.0 - point.0).abs() < 0.001 && (back.1 - point.1).abs() < 0.001);
        let source = RgbaImage::from_pixel(2, 2, Rgba([255, 80, 40, 255]));
        let baked = bake(
            &source,
            Placement {
                x: 3.0,
                y: 4.0,
                ..Placement::default()
            },
            (10, 10),
        );
        assert_eq!(*baked.get_pixel(3, 4), Rgba([255, 80, 40, 255]));
        assert_eq!(baked.get_pixel(2, 4)[3], 0);
    }
}
