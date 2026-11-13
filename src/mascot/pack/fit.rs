//! Fitting pack images that don't match the declared canvas onto it at
//! import. Installed packs stay strict: every image is baked to canvas size,
//! with transparent pixels around the art, before the pack is validated.
//!
//! A manifest may say how with an optional `fit` block, which installed
//! packs never carry:
//!
//! ```json
//! "fit": { "default": "bottom",
//!          "images": { "sit/tail.png": { "x": 210, "y": 300, "scale": 1 } } }
//! ```

use super::*;
use crate::mascot::placement::{self, Placement};

/// Where an image smaller or larger than the canvas goes on it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Fit {
    /// Centred, standing on the canvas bottom.
    Bottom,
    Center,
    TopLeft,
    /// An exact top-left and scale, in canvas pixels.
    Exact(Placement),
}

impl Fit {
    /// The choices the import panel offers.
    pub const CHOICES: [Fit; 3] = [Fit::Bottom, Fit::Center, Fit::TopLeft];

    pub fn name(self) -> &'static str {
        match self {
            Fit::Bottom => "bottom",
            Fit::Center => "center",
            Fit::TopLeft => "top-left",
            Fit::Exact(_) => "exact",
        }
    }

    fn parse(v: &Json, canvas: (u32, u32)) -> Result<Self, String> {
        if let Some(s) = v.as_str() {
            return Self::CHOICES
                .into_iter()
                .find(|f| f.name() == s)
                .ok_or(format!(
                    "fit `{s}` must be bottom, center, top-left or a placement"
                ));
        }
        let num = |k: &str| v[k].as_f64().map(|n| n as f32);
        let p = Placement {
            x: num("x").unwrap_or(0.0),
            y: num("y").unwrap_or(0.0),
            scale: num("scale").unwrap_or(1.0),
            angle: 0.0,
        };
        if !v.is_object() || !p.valid(canvas) {
            return Err("a fit placement needs x, y and a scale between 0.01 and 8".into());
        }
        Ok(Fit::Exact(p))
    }

    /// The placement that puts a `size` image on `canvas`. Art is only
    /// ever scaled down, so it is never blurred by enlarging.
    pub fn placement(self, size: (u32, u32), canvas: (u32, u32)) -> Placement {
        let (w, h) = (size.0.max(1) as f32, size.1.max(1) as f32);
        let (cw, ch) = (canvas.0 as f32, canvas.1 as f32);
        let scale = (cw / w).min(ch / h).min(1.0);
        match self {
            Fit::Exact(p) => p,
            Fit::Center => Placement::centred(size, canvas),
            Fit::Bottom => Placement {
                x: (cw - w * scale) / 2.0,
                y: ch - h * scale,
                scale,
                angle: 0.0,
            },
            Fit::TopLeft => Placement {
                scale,
                ..Placement::default()
            },
        }
    }
}

impl std::fmt::Display for Fit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Fit::Bottom => "Centre, on the bottom",
            Fit::Center => "Centre",
            Fit::TopLeft => "Top-left corner",
            Fit::Exact(_) => "As the pack says",
        })
    }
}

/// The manifest's `fit` block: a default and per-image placements.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Rules {
    pub default: Option<Fit>,
    pub images: Vec<(String, Fit)>,
}

impl Rules {
    pub fn parse(v: &Json, canvas: (u32, u32)) -> Result<Self, String> {
        let mut out = Self::default();
        if v.is_null() {
            return Ok(out);
        }
        if !v["default"].is_null() {
            out.default = Some(Fit::parse(&v["default"], canvas)?);
        }
        if let Some(map) = v["images"].as_object() {
            for (asset, fit) in map {
                if !valid_asset(asset) {
                    return Err(format!("fit names `{asset}`, which isn't a pack PNG"));
                }
                out.images.push((asset.clone(), Fit::parse(fit, canvas)?));
            }
        } else if !v["images"].is_null() {
            return Err("fit images must map PNGs to placements".into());
        }
        Ok(out)
    }

    /// How `asset` is fitted: its own rule, else `choice`, else the
    /// manifest's default, else on the bottom.
    pub fn for_asset(&self, asset: &str, choice: Option<Fit>) -> Fit {
        self.images
            .iter()
            .find(|(a, _)| a == asset)
            .map(|(_, f)| *f)
            .or(choice)
            .or(self.default)
            .unwrap_or(Fit::Bottom)
    }
}

/// A canvas image whose size differs from the canvas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Misfit {
    pub asset: String,
    pub size: (u32, u32),
}

/// `folder/asset`, refusing paths that leave the folder through links.
pub(super) fn contained(folder: &Path, asset: &str) -> Result<PathBuf, String> {
    let root = folder.canonicalize().map_err(|e| e.to_string())?;
    let path = folder
        .join(asset)
        .canonicalize()
        .map_err(|_| format!("{asset} is missing"))?;
    if !path.starts_with(&root) {
        return Err(format!("{asset} is outside the pack folder"));
    }
    Ok(path)
}

/// Canvas images (poses and reaction pictures) whose size isn't the canvas.
pub fn misfits(m: &Manifest, folder: &Path) -> Result<Vec<Misfit>, String> {
    let mut out = Vec::new();
    for asset in m.assets() {
        let path = contained(folder, asset)?;
        let size = image::ImageReader::open(&path)
            .and_then(|r| r.with_guessed_format())
            .map_err(|e| format!("{asset}: {e}"))?
            .into_dimensions()
            .map_err(|e| format!("{asset}: {e}"))?;
        let expected = m
            .resources()
            .into_iter()
            .find(|(name, _)| *name == asset)
            .map(|(_, size)| size)
            .unwrap_or(m.canvas);
        if size != expected {
            if expected != m.canvas {
                return Err(format!(
                    "{asset} does not match its declared original source size"
                ));
            }
            if size.0 > MAX_SOURCE_SIDE || size.1 > MAX_SOURCE_SIDE {
                return Err(format!(
                    "{asset} must be at most {MAX_SOURCE_SIDE} × {MAX_SOURCE_SIDE} pixels"
                ));
            }
            out.push(Misfit {
                asset: asset.to_owned(),
                size,
            });
        }
    }
    Ok(out)
}

/// Reads `asset` from `from`, puts it on the canvas and writes it to `to`.
pub(super) fn bake_asset(
    from: &Path,
    to: &Path,
    asset: &str,
    fit: Fit,
    canvas: (u32, u32),
) -> Result<(), String> {
    let path = contained(from, asset)?;
    if fs::metadata(&path).map_err(|e| e.to_string())?.len() > MAX_FILE {
        return Err(format!("{asset} is larger than 64 MiB"));
    }
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    check_source_dimensions(&bytes).map_err(|e| format!("{asset}: {e}"))?;
    let art = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .map_err(|e| format!("{asset} is not a readable PNG ({e})"))?
        .to_rgba8();
    let baked = placement::bake(&art, fit.placement(art.dimensions(), canvas), canvas);
    if !baked.pixels().any(|p| p[3] >= SOLID) {
        return Err(format!("{asset} lands outside the canvas when fitted"));
    }
    let target = to.join(asset);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    baked
        .save_with_format(target, image::ImageFormat::Png)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placements_keep_art_on_the_canvas() {
        let canvas = (400, 400);
        let p = Fit::Bottom.placement((100, 50), canvas);
        assert_eq!((p.x, p.y, p.scale), (150.0, 350.0, 1.0));
        let p = Fit::TopLeft.placement((800, 400), canvas);
        assert_eq!(
            (p.x, p.y, p.scale),
            (0.0, 0.0, 0.5),
            "large art scales down"
        );
        let p = Fit::Center.placement((100, 100), canvas);
        assert_eq!((p.x, p.y), (150.0, 150.0));

        let rules = Rules::parse(
            &json!({ "default": "center", "images": { "a.png": { "x": 5, "y": 6 } } }),
            canvas,
        )
        .unwrap();
        assert!(matches!(rules.for_asset("a.png", Some(Fit::Bottom)), Fit::Exact(p) if p.x == 5.0));
        assert_eq!(rules.for_asset("b.png", Some(Fit::TopLeft)), Fit::TopLeft);
        assert_eq!(rules.for_asset("b.png", None), Fit::Center);
        for bad in [
            json!({ "default": "middle" }),
            json!({ "images": { "../x.png": "center" } }),
            json!({ "images": { "a.png": { "scale": 0 } } }),
        ] {
            assert!(Rules::parse(&bad, canvas).is_err(), "{bad}");
        }
    }
}
