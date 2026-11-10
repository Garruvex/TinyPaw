//! Character packs: a folder holding `character.json` and PNG art, installed
//! as an independent copy under `%APPDATA%\TinyPaw\characters\<id>`. Bundled
//! Yohta and imported characters share one list and one selection setting.
//! Every pack is validated completely, and staged, before it can be used.
//!
//! The manifest is the version 1 format from `docs/mascot-feature.md`: each
//! pose has a body and optional moving parts (head, tail, ears, wings, hair),
//! eyes and head faces, every image on the same canvas.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

static INSTALL_LOCK: Mutex<()> = Mutex::new(());
static NEXT_STAGE: AtomicU64 = AtomicU64::new(0);

use iced::advanced::image::Handle;
use image::RgbaImage;
use serde_json::{Value as Json, json};

use super::motion::{MotionSpec, PartType, Preset};
use super::rig::{EyeArt, LayeredArt, PartArt, Rig};

/// The bundled character's ID, also the fallback selection.
pub const BUILTIN: &str = "yohta";
pub(super) const MANIFEST: &str = "character.json";
const THUMB: &str = "thumb.png";
/// Largest canvas side a pack may declare.
const MAX_SIDE: u32 = 2048;
/// Quick import scales art down to this height: plenty for 320 logical
/// pixels at 200 %, and far less memory than camera-sized PNGs.
const IMPORT_HEIGHT: u32 = 1024;
/// Largest PNG accepted for quick import, before scaling.
pub(super) const MAX_SOURCE_SIDE: u32 = 8192;
/// Encoded bytes per image and per pack.
pub(super) const MAX_FILE: u64 = 64 * 1024 * 1024;
const MAX_MANIFEST: u64 = 64 * 1024;
const THUMB_SIDE: u32 = 96;
/// Alpha at which a pixel counts as part of the character.
pub(super) const SOLID: u8 = 32;
/// Transparent margin kept around trimmed quick-import art.
const MARGIN: u32 = 4;
const POSES: [&str; 2] = ["sit", "stand"];
const MAX_PARTS: usize = 16;
const MAX_EYES: usize = 4;
/// Image files per pack, across every pose.
const MAX_IMAGES: usize = 32;
/// Head faces, in the order the rig takes them.
const FACES: [&str; 3] = ["blink", "pressed", "dragging"];
/// Furthest a pupil may travel, as a fraction of the canvas.
pub(super) const MAX_GAZE: f32 = 0.1;

/// An installed or bundled character, as listed in Settings.
#[derive(Debug, Clone)]
pub struct Character {
    /// Interned for the lifetime of the process, so settings can refer to it.
    pub id: &'static str,
    pub name: String,
    /// The installed folder; `None` for bundled Yohta.
    pub folder: Option<PathBuf>,
    thumb: Option<Handle>,
    cached_rig: Option<Arc<Mutex<Option<Rig>>>>,
}

impl Character {
    pub fn builtin() -> Self {
        Self {
            id: BUILTIN,
            name: "Yohta".into(),
            folder: None,
            thumb: None,
            cached_rig: None,
        }
    }

    pub fn is_builtin(&self) -> bool {
        self.folder.is_none()
    }

    /// A small picture for lists. Yohta's is cut from his sheet on first use.
    pub fn thumbnail(&self) -> Option<Handle> {
        if self.is_builtin() {
            static YOHTA: OnceLock<Handle> = OnceLock::new();
            return Some(
                YOHTA
                    .get_or_init(|| handle(Rig::yohta_thumbnail(THUMB_SIDE)))
                    .clone(),
            );
        }
        self.thumb.clone()
    }

    /// Decodes the art for the desktop. Done when shown, released when hidden.
    pub fn rig(&self) -> Result<Rig, String> {
        if let Some(rig) = &self.cached_rig
            && let Some(rig) = rig.lock().unwrap_or_else(|e| e.into_inner()).take()
        {
            return Ok(rig);
        }
        let Some(folder) = &self.folder else {
            return Ok(Rig::load());
        };
        let m = read_manifest(folder)?;
        let pose = m.default_pose();
        let png = |asset: &str| read_png(folder, asset, m.canvas);
        let body = png(&pose.body)?;
        if pose.parts.is_empty() && pose.eyes.is_empty() {
            return Ok(Rig::sprite(body, pose.ground));
        }
        let mut parts = Vec::new();
        for part in &pose.parts {
            let mut faces = [None, None, None];
            for (slot, face) in faces.iter_mut().zip(&part.faces) {
                if let Some(asset) = face {
                    *slot = Some(png(asset)?);
                }
            }
            parts.push(PartArt {
                kind: part.kind,
                image: png(&part.image)?,
                pivot: part.pivot,
                parent: part.parent,
                behind: part.behind,
                motion: part.motion,
                faces,
            });
        }
        let mut eyes = Vec::new();
        for eye in &pose.eyes {
            eyes.push(EyeArt {
                white: png(&eye.white)?,
                pupil: png(&eye.pupil)?,
                mask: png(&eye.mask)?,
                blink: eye.blink.as_deref().map(png).transpose()?,
                max_offset: eye.max_offset,
            });
        }
        Ok(Rig::layered(LayeredArt {
            body,
            ground: pose.ground,
            parts,
            eyes,
        }))
    }

    pub fn release_prepared_rig(&self) {
        if let Some(rig) = &self.cached_rig {
            rig.lock().unwrap_or_else(|e| e.into_inner()).take();
        }
    }
}

fn handle(pixels: RgbaImage) -> Handle {
    Handle::from_rgba(pixels.width(), pixels.height(), pixels.into_raw())
}

pub fn user_dir() -> Option<PathBuf> {
    crate::preferences::data_dir().map(|d| d.join("characters"))
}

// ---- Manifest ---------------------------------------------------------------

#[derive(Debug)]
pub(super) struct Manifest {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) author: Option<String>,
    pub(super) canvas: (u32, u32),
    pub(super) default_pose: String,
    pub(super) poses: Vec<Pose>,
    /// How quick import cut the canvas from the original PNG, so later part
    /// art drawn on that original lines up.
    pub(super) source: Option<Source>,
    pub(super) editor: Vec<super::placement::SourceRecord>,
}

/// The original PNG's size and the visible box quick import kept from it
/// (before adding the margin and scaling to the canvas).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Source {
    pub(super) size: (u32, u32),
    pub(super) visible: (u32, u32, u32, u32),
}

#[derive(Debug, Clone)]
pub(super) struct Pose {
    pub(super) name: String,
    pub(super) ground: (f32, f32),
    /// Relative path of the body image, using `/`.
    pub(super) body: String,
    pub(super) parts: Vec<PartDef>,
    pub(super) eyes: Vec<EyeDef>,
}

#[derive(Debug, Clone)]
pub(super) struct PartDef {
    pub(super) kind: PartType,
    pub(super) image: String,
    pub(super) pivot: (f32, f32),
    /// Index of the part this rides on; only the head can carry parts.
    pub(super) parent: Option<usize>,
    pub(super) behind: bool,
    pub(super) motion: MotionSpec,
    /// Head faces: blink, pressed, dragging.
    pub(super) faces: [Option<String>; 3],
}

#[derive(Debug, Clone)]
pub(super) struct EyeDef {
    pub(super) white: String,
    pub(super) pupil: String,
    pub(super) mask: String,
    pub(super) blink: Option<String>,
    pub(super) max_offset: (f32, f32),
}

impl Pose {
    /// Every image this pose names.
    pub(super) fn assets(&self) -> Vec<&str> {
        let mut out = vec![self.body.as_str()];
        for p in &self.parts {
            out.push(&p.image);
            out.extend(p.faces.iter().flatten().map(String::as_str));
        }
        for e in &self.eyes {
            out.extend([e.white.as_str(), e.pupil.as_str(), e.mask.as_str()]);
            out.extend(e.blink.as_deref());
        }
        out
    }

    fn to_json(&self) -> Json {
        let mut pose = json!({
            "ground_anchor": [self.ground.0, self.ground.1],
            "body": self.body,
        });
        if !self.parts.is_empty() {
            pose["parts"] = Json::Array(
                self.parts
                    .iter()
                    .map(|p| {
                        let mut part = json!({
                            "type": p.kind.name(),
                            "image": p.image,
                            "pivot": [p.pivot.0, p.pivot.1],
                            "layer": if p.behind { "back" } else { "front" },
                            "motion": {
                                "preset": p.motion.preset.name(),
                                "strength": p.motion.strength,
                                "speed": p.motion.speed,
                            },
                        });
                        if p.parent.is_some() {
                            part["parent"] = json!("head");
                        }
                        let faces: serde_json::Map<String, Json> = FACES
                            .iter()
                            .zip(&p.faces)
                            .filter_map(|(k, v)| Some(((*k).to_owned(), json!(v.as_ref()?))))
                            .collect();
                        if !faces.is_empty() {
                            part["expressions"] = Json::Object(faces);
                        }
                        part
                    })
                    .collect(),
            );
        }
        if !self.eyes.is_empty() {
            pose["eyes"] = Json::Array(
                self.eyes
                    .iter()
                    .map(|e| {
                        let mut eye = json!({
                            "white": e.white,
                            "pupil": e.pupil,
                            "mask": e.mask,
                            "max_offset": [e.max_offset.0, e.max_offset.1],
                        });
                        if let Some(b) = &e.blink {
                            eye["blink"] = json!(b);
                        }
                        eye
                    })
                    .collect(),
            );
        }
        pose
    }
}

fn fraction_pair(v: &Json) -> Option<(f32, f32)> {
    match v.as_array()?.as_slice() {
        [x, y] => Some((x.as_f64()? as f32, y.as_f64()? as f32))
            .filter(|(x, y)| (0.0..=1.0).contains(x) && (0.0..=1.0).contains(y)),
        _ => None,
    }
}

fn asset_field(v: &Json, what: &str) -> Result<String, String> {
    v.as_str()
        .filter(|a| valid_asset(a))
        .map(str::to_owned)
        .ok_or(format!("{what} must be a PNG inside the pack folder"))
}

/// One part, from `parts` or the `head`/`tail` shorthand. `parent` is
/// resolved later, once every part is known.
fn parse_part(v: &Json, kind: Option<PartType>, n: usize) -> Result<(PartDef, bool), String> {
    let kind = match kind {
        Some(k) => k,
        None => v["type"].as_str().and_then(PartType::parse).ok_or(format!(
            "part {n} needs a type: head, tail, ear, wing, hair or other"
        ))?,
    };
    let what = format!("the {} part", kind.name());
    let image = asset_field(&v["image"], &format!("{what}'s image"))?;
    let pivot =
        fraction_pair(&v["pivot"]).ok_or(format!("{what} needs a pivot of two fractions"))?;
    let behind = match v["layer"].as_str() {
        None => kind.behind_by_default(),
        Some("back") => true,
        Some("front") => false,
        Some(other) => return Err(format!("{what}'s layer `{other}` must be back or front")),
    };
    let mut motion = MotionSpec::default_for(kind);
    let m = &v["motion"];
    if let Some(name) = m["preset"].as_str() {
        motion.preset = Preset::parse(name)
            .filter(|p| kind.presets().contains(p))
            .ok_or(format!("{what} can't use the `{name}` motion"))?;
    }
    for (field, value) in [
        ("strength", &mut motion.strength),
        ("speed", &mut motion.speed),
    ] {
        if let Some(x) = m[field].as_f64() {
            if !(0.0..=2.0).contains(&x) {
                return Err(format!("{what}'s {field} must be between 0 and 2"));
            }
            *value = x as f32;
        }
    }
    let mut faces = [None, None, None];
    if let Some(map) = v["expressions"].as_object() {
        if kind != PartType::Head {
            return Err(format!("only the head can have expressions, not {what}"));
        }
        for (key, value) in map {
            let slot = FACES.iter().position(|f| f == key).ok_or(format!(
                "unknown expression `{key}`; use blink, pressed or dragging"
            ))?;
            faces[slot] = Some(asset_field(value, &format!("the {key} face"))?);
        }
    }
    let on_head = match v["parent"].as_str() {
        None | Some("body") => false,
        Some("head") if kind != PartType::Head => true,
        Some(other) => return Err(format!("{what} can't ride on `{other}`; use head or body")),
    };
    Ok((
        PartDef {
            kind,
            image,
            pivot,
            parent: None,
            behind,
            motion,
            faces,
        },
        on_head,
    ))
}

fn parse_eye(v: &Json, n: usize) -> Result<EyeDef, String> {
    let what = format!("eye {n}");
    let max_offset = match v["max_offset"].as_array().map(Vec::as_slice) {
        None => (0.012, 0.009),
        Some([x, y]) => (
            x.as_f64().unwrap_or(-1.0) as f32,
            y.as_f64().unwrap_or(-1.0) as f32,
        ),
        Some(_) => (-1.0, -1.0),
    };
    if !(0.0..=MAX_GAZE).contains(&max_offset.0) || !(0.0..=MAX_GAZE).contains(&max_offset.1) {
        return Err(format!(
            "{what}'s max_offset must be two fractions up to {MAX_GAZE}"
        ));
    }
    Ok(EyeDef {
        white: asset_field(&v["white"], &format!("{what}'s white"))?,
        pupil: asset_field(&v["pupil"], &format!("{what}'s pupil"))?,
        mask: asset_field(&v["mask"], &format!("{what}'s mask"))?,
        blink: match &v["blink"] {
            Json::Null => None,
            b => Some(asset_field(b, &format!("{what}'s blink"))?),
        },
        max_offset,
    })
}

/// Parts, eyes and their links for one pose.
fn parse_layers(pose: &Json, pose_name: &str) -> Result<(Vec<PartDef>, Vec<EyeDef>), String> {
    let mut found: Vec<(PartDef, bool)> = Vec::new();
    for (key, kind) in [("head", PartType::Head), ("tail", PartType::Tail)] {
        if !pose[key].is_null() {
            found.push(parse_part(&pose[key], Some(kind), found.len() + 1)?);
        }
    }
    if let Some(list) = pose["parts"].as_array() {
        for v in list {
            found.push(parse_part(v, None, found.len() + 1)?);
        }
    } else if !pose["parts"].is_null() {
        return Err(format!("the {pose_name} pose's parts must be a list"));
    }
    if found.len() > MAX_PARTS {
        return Err(format!("a pose can have at most {MAX_PARTS} parts"));
    }
    let heads: Vec<usize> = found
        .iter()
        .enumerate()
        .filter(|(_, (p, _))| p.kind == PartType::Head)
        .map(|(i, _)| i)
        .collect();
    if heads.len() > 1 {
        return Err(format!("the {pose_name} pose has more than one head"));
    }
    let head = heads.first().copied();
    let mut parts = Vec::new();
    for (mut part, on_head) in found {
        if on_head {
            part.parent = Some(head.ok_or(format!(
                "the {} part rides on the head, but there is no head part",
                part.kind.name()
            ))?);
        }
        parts.push(part);
    }
    let eyes = match &pose["eyes"] {
        Json::Null => Vec::new(),
        Json::Array(list) => list
            .iter()
            .enumerate()
            .map(|(n, v)| parse_eye(v, n + 1))
            .collect::<Result<_, _>>()?,
        _ => return Err(format!("the {pose_name} pose's eyes must be a list")),
    };
    if eyes.len() > MAX_EYES {
        return Err(format!("a pose can have at most {MAX_EYES} eyes"));
    }
    if !eyes.is_empty() && head.is_none() {
        return Err("eyes need a head part to ride on".into());
    }
    Ok((parts, eyes))
}

impl Manifest {
    pub(super) fn default_pose(&self) -> &Pose {
        self.poses
            .iter()
            .find(|p| p.name == self.default_pose)
            .expect("validated default pose")
    }

    /// Every image the pack names, each once.
    pub(super) fn assets(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for p in &self.poses {
            for a in p.assets() {
                if !out.contains(&a) {
                    out.push(a);
                }
            }
        }
        out
    }

    pub(super) fn resources(&self) -> Vec<(&str, (u32, u32))> {
        let mut out: Vec<_> = self
            .assets()
            .into_iter()
            .map(|a| (a, self.canvas))
            .collect();
        out.extend(self.editor.iter().map(|r| (r.source.as_str(), r.size)));
        out
    }

    pub(super) fn validate_files(&self, folder: &Path) -> Result<(), String> {
        let mut encoded = 0u64;
        for (asset, dimensions) in self.resources() {
            encoded = encoded
                .checked_add(
                    fs::metadata(folder.join(asset))
                        .map_err(|_| format!("{asset} is missing"))?
                        .len(),
                )
                .ok_or("character file sizes overflow the supported budget")?;
            if encoded > 128 * 1024 * 1024 {
                return Err(
                    "character files and editor sources exceed the 128 MiB disk budget".into(),
                );
            }
            read_png(folder, asset, dimensions)?;
        }
        Ok(())
    }

    fn to_json(&self) -> Json {
        let mut poses = serde_json::Map::new();
        for p in &self.poses {
            poses.insert(p.name.clone(), p.to_json());
        }
        let mut m = json!({
            "schema_version": 1,
            "id": self.id,
            "name": self.name,
            "canvas": [self.canvas.0, self.canvas.1],
            "default_pose": self.default_pose,
            "poses": poses,
        });
        if let Some(author) = &self.author {
            m["author"] = json!(author);
        }
        if !self.editor.is_empty() {
            m["editor"] = super::placement::to_json(&self.editor);
        }
        if let Some(src) = &self.source {
            let (x, y, w, h) = src.visible;
            m["source"] = json!({ "size": [src.size.0, src.size.1], "visible": [x, y, w, h] });
        }
        m
    }
}

impl Source {
    /// Cuts `art` (the original PNG's size) the way quick import cut the
    /// body: the visible box, the margin, then scaled to `canvas`.
    fn apply(&self, art: &RgbaImage, canvas: (u32, u32)) -> RgbaImage {
        let (x, y, w, h) = self.visible;
        let mut cut = RgbaImage::new(w + MARGIN * 2, h + MARGIN * 2);
        image::imageops::replace(
            &mut cut,
            &image::imageops::crop_imm(art, x, y, w, h).to_image(),
            i64::from(MARGIN),
            i64::from(MARGIN),
        );
        if cut.dimensions() == canvas {
            cut
        } else {
            image::imageops::resize(
                &cut,
                canvas.0,
                canvas.1,
                image::imageops::FilterType::Lanczos3,
            )
        }
    }
}

/// Reads a PNG the user picked for an existing character and fits it to the
/// canvas: as is when it matches, or cut like the body when it matches the
/// original PNG quick import started from.
pub(super) fn fit_png(
    path: &Path,
    canvas: (u32, u32),
    source: Option<&Source>,
) -> Result<RgbaImage, String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > MAX_FILE {
        return Err("the PNG is larger than 64 MiB".into());
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    check_source_dimensions(&bytes)?;
    let art = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .map_err(|e| format!("not a readable PNG ({e})"))?
        .to_rgba8();
    if art.dimensions() == canvas {
        return Ok(art);
    }
    if let Some(src) = source
        && art.dimensions() == src.size
    {
        return Ok(src.apply(&art, canvas));
    }
    let original = source
        .map(|s| format!(", or the original {} × {}", s.size.0, s.size.1))
        .unwrap_or_default();
    Err(format!(
        "this PNG is {} × {}. Draw parts on the character's canvas: {} × {}{original}",
        art.width(),
        art.height(),
        canvas.0,
        canvas.1
    ))
}

fn valid_id(id: &str) -> bool {
    (1..=40).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// A path inside the pack: relative, `/`-separated, no `..`.
pub(super) fn valid_asset(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 200
        && !path.contains(['\\', ':'])
        && path.ends_with(".png")
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn parse_manifest(text: &str) -> Result<Manifest, String> {
    let m: Json = serde_json::from_str(text).map_err(|e| format!("character.json: {e}"))?;
    if m["schema_version"].as_u64() != Some(1) {
        return Err("character.json must use schema_version 1".into());
    }
    let id = m["id"].as_str().unwrap_or_default();
    if !valid_id(id) {
        return Err("the id must be 1–40 lowercase letters, digits or dashes".into());
    }
    let name = m["name"].as_str().unwrap_or_default().trim();
    if name.is_empty() || name.chars().count() > 60 {
        return Err("the name must be 1–60 characters".into());
    }
    let author = m["author"].as_str().map(|a| a.chars().take(80).collect());
    let side = |v: &Json| {
        v.as_u64()
            .filter(|n| (1..=u64::from(MAX_SIDE)).contains(n))
            .map(|n| n as u32)
    };
    let canvas = match m["canvas"].as_array().map(Vec::as_slice) {
        Some([w, h]) => (side(w), side(h)),
        _ => (None, None),
    };
    let (Some(w), Some(h)) = canvas else {
        return Err(format!(
            "canvas must be [width, height], each 1–{MAX_SIDE} pixels"
        ));
    };
    let Some(poses_json) = m["poses"].as_object().filter(|p| !p.is_empty()) else {
        return Err("the character needs at least one pose".into());
    };
    let mut poses = Vec::new();
    for (pose_name, pose) in poses_json {
        if !POSES.contains(&pose_name.as_str()) {
            return Err(format!("unknown pose `{pose_name}`; use sit or stand"));
        }
        let ground = pose["ground_anchor"]
            .as_array()
            .and_then(|a| match a.as_slice() {
                [x, y] => Some((x.as_f64()? as f32, y.as_f64()? as f32)),
                _ => None,
            })
            .filter(|(x, y)| (0.0..=1.0).contains(x) && (0.0..=1.0).contains(y))
            .ok_or(format!(
                "the {pose_name} pose needs a ground_anchor of two fractions"
            ))?;
        let body = pose["body"]
            .as_str()
            .filter(|b| valid_asset(b))
            .ok_or(format!(
                "the {pose_name} pose needs a body PNG inside the pack folder"
            ))?;
        let (parts, eyes) = parse_layers(pose, pose_name)?;
        poses.push(Pose {
            name: pose_name.clone(),
            ground,
            body: body.to_owned(),
            parts,
            eyes,
        });
    }
    let default_pose = m["default_pose"].as_str().unwrap_or_default();
    if !poses.iter().any(|p| p.name == default_pose) {
        return Err("default_pose must name one of the poses".into());
    }
    let source = match &m["source"] {
        Json::Null => None,
        v => {
            let num = |j: &Json| j.as_u64().and_then(|n| u32::try_from(n).ok());
            let size = match v["size"].as_array().map(Vec::as_slice) {
                Some([w, h]) => num(w).zip(num(h)),
                _ => None,
            };
            let visible = match v["visible"].as_array().map(Vec::as_slice) {
                Some([x, y, w, h]) => (|| Some((num(x)?, num(y)?, num(w)?, num(h)?)))(),
                _ => None,
            };
            match (size, visible) {
                (Some(size), Some(visible))
                    if size.0 <= MAX_SOURCE_SIDE
                        && size.1 <= MAX_SOURCE_SIDE
                        && visible.2 > 0
                        && visible.3 > 0
                        && visible.0 + visible.2 <= size.0
                        && visible.1 + visible.3 <= size.1 =>
                {
                    Some(Source { size, visible })
                }
                _ => return Err("source must give the original size and visible box".into()),
            }
        }
    };
    let mut manifest = Manifest {
        id: id.to_owned(),
        name: name.to_owned(),
        author,
        canvas: (w, h),
        default_pose: default_pose.to_owned(),
        poses,
        source,
        editor: Vec::new(),
    };
    manifest.editor = super::placement::parse(&m["editor"], manifest.canvas, &manifest.assets())?;
    if manifest.assets().len() > MAX_IMAGES {
        return Err(format!("a character can use at most {MAX_IMAGES} images"));
    }
    // Bound decoded RGBA storage across the entire rig, not just each file.
    let decoded = u64::from(manifest.canvas.0)
        * u64::from(manifest.canvas.1)
        * 4
        * manifest
            .poses
            .iter()
            .map(|p| p.assets().len() as u64)
            .sum::<u64>();
    let editor_bytes: u64 = manifest
        .editor
        .iter()
        .map(|r| u64::from(r.size.0) * u64::from(r.size.1) * 8)
        .sum();
    if decoded + editor_bytes > 128 * 1024 * 1024 {
        return Err("character art exceeds the 128 MiB decoded memory budget".into());
    }
    Ok(manifest)
}

pub(super) fn read_manifest(folder: &Path) -> Result<Manifest, String> {
    let path = folder.join(MANIFEST);
    let meta = fs::metadata(&path).map_err(|_| "this folder has no character.json".to_owned())?;
    if meta.len() > MAX_MANIFEST {
        return Err("character.json is too large".into());
    }
    parse_manifest(&fs::read_to_string(path).map_err(|e| e.to_string())?)
}

/// Reads one pack image, refusing paths that leave the folder (including
/// through links), oversized files and art that doesn't fit the canvas.
pub(super) fn read_png(
    folder: &Path,
    asset: &str,
    canvas: (u32, u32),
) -> Result<RgbaImage, String> {
    let root = folder.canonicalize().map_err(|e| e.to_string())?;
    let path = folder
        .join(asset)
        .canonicalize()
        .map_err(|_| format!("{asset} is missing"))?;
    if !path.starts_with(&root) {
        return Err(format!("{asset} is outside the pack folder"));
    }
    if fs::metadata(&path).map_err(|e| e.to_string())?.len() > MAX_FILE {
        return Err(format!("{asset} is larger than 64 MiB"));
    }
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let dimensions =
        image::ImageReader::with_format(std::io::Cursor::new(&bytes), image::ImageFormat::Png)
            .into_dimensions()
            .map_err(|e| format!("{asset}: {e}"))?;
    if dimensions != canvas {
        return Err(format!(
            "{asset} is {} × {}. This character uses a {} × {} canvas",
            dimensions.0, dimensions.1, canvas.0, canvas.1
        ));
    }
    let art = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .map_err(|e| format!("{asset} is not a readable PNG ({e})"))?
        .to_rgba8();
    if art.dimensions() != canvas {
        return Err(format!(
            "{asset} is {} × {}. This character uses a {} × {} canvas",
            art.width(),
            art.height(),
            canvas.0,
            canvas.1
        ));
    }
    if !art.pixels().any(|p| p[3] >= SOLID) {
        return Err(format!("{asset} is empty"));
    }
    Ok(art)
}

fn check_source_dimensions(bytes: &[u8]) -> Result<(), String> {
    let (w, h) =
        image::ImageReader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Png)
            .into_dimensions()
            .map_err(|e| format!("not a readable PNG ({e})"))?;
    if w > MAX_SOURCE_SIDE || h > MAX_SOURCE_SIDE {
        return Err(format!(
            "the PNG must be at most {MAX_SOURCE_SIDE} × {MAX_SOURCE_SIDE} pixels"
        ));
    }
    Ok(())
}

fn thumbnail(art: &RgbaImage) -> RgbaImage {
    let scale = THUMB_SIDE as f32 / art.width().max(art.height()) as f32;
    image::imageops::resize(
        art,
        ((art.width() as f32 * scale).round() as u32).max(1),
        ((art.height() as f32 * scale).round() as u32).max(1),
        image::imageops::FilterType::Triangle,
    )
}

// ---- Installed list ---------------------------------------------------------

/// Bundled Yohta first, then installed characters by name. Packs that fail
/// validation are skipped and reported.
pub fn load_all() -> (Vec<Character>, Vec<String>) {
    let mut list = vec![Character::builtin()];
    let mut problems = Vec::new();
    if let Some(root) = user_dir() {
        problems.extend(recover_transactions(&root));
    }
    if let Some(root) = user_dir()
        && let Ok(entries) = fs::read_dir(&root)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            let hidden = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_none_or(|n| n.starts_with('.'));
            if hidden || !path.is_dir() {
                continue;
            }
            match load_folder(&path) {
                Ok(c) => list.push(c),
                Err(e) => problems.push(format!("{}: {e}", path.display())),
            }
        }
    }
    list[1..].sort_by_key(|c| c.name.to_lowercase());
    (list, problems)
}

/// A listing check: the manifest and its folder name. The art itself is
/// validated at install and when the character is shown.
fn load_folder(folder: &Path) -> Result<Character, String> {
    let m = read_manifest(folder)?;
    if folder.file_name().and_then(|n| n.to_str()) != Some(m.id.as_str()) || m.id == BUILTIN {
        return Err("folder name and id differ".into());
    }
    let thumb = fs::read(folder.join(THUMB))
        .ok()
        .and_then(|b| image::load_from_memory_with_format(&b, image::ImageFormat::Png).ok())
        .map(|i| handle(i.to_rgba8()));
    Ok(Character {
        id: Box::leak(m.id.into_boxed_str()),
        name: m.name,
        folder: Some(folder.to_path_buf()),
        thumb,
        cached_rig: None,
    })
}

// ---- Quick import: one PNG --------------------------------------------------

/// "Add character" in progress: trimmed art and a ground point. Nothing is
/// written until [`install`].
#[derive(Debug, Clone)]
pub struct Draft {
    pub name: String,
    pub image: Handle,
    pub width: u32,
    pub height: u32,
    art: Arc<RgbaImage>,
    /// Where the feet meet the ground, as fractions from the top-left.
    pub ground: (f32, f32),
    source: Source,
}

/// Starts "Add character" from a transparent PNG: trims empty margins, scales
/// large art down and suggests a ground point under the feet.
pub fn draft_from_png(path: &Path) -> Result<Draft, String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > MAX_FILE {
        return Err("the PNG is larger than 64 MiB".into());
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    check_source_dimensions(&bytes)?;
    let art = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .map_err(|e| format!("not a readable PNG ({e})"))?
        .to_rgba8();
    if art.width() > MAX_SOURCE_SIDE || art.height() > MAX_SOURCE_SIDE {
        return Err(format!(
            "the PNG must be at most {MAX_SOURCE_SIDE} × {MAX_SOURCE_SIDE} pixels"
        ));
    }
    if art.pixels().all(|p| p[3] == 255) {
        return Err("the PNG needs a transparent background".into());
    }
    let size = art.dimensions();
    let (art, visible) = trim(&art).ok_or("the PNG is empty")?;
    let art = if art.height() > IMPORT_HEIGHT || art.width() > MAX_SIDE {
        let k =
            (IMPORT_HEIGHT as f32 / art.height() as f32).min(MAX_SIDE as f32 / art.width() as f32);
        image::imageops::resize(
            &art,
            ((art.width() as f32 * k).round() as u32).max(1),
            ((art.height() as f32 * k).round() as u32).max(1),
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        art
    };
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("My character")
        .chars()
        .take(60)
        .collect();
    Ok(Draft {
        name,
        image: Handle::from_rgba(art.width(), art.height(), art.as_raw().clone()),
        width: art.width(),
        height: art.height(),
        ground: guess_ground(&art),
        art: Arc::new(art),
        source: Source { size, visible },
    })
}

/// The visible art plus a small transparent margin, and the visible box, or
/// `None` if empty.
fn trim(art: &RgbaImage) -> Option<(RgbaImage, (u32, u32, u32, u32))> {
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for (x, y, p) in art.enumerate_pixels() {
        if p[3] >= SOLID {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    if x0 > x1 {
        return None;
    }
    let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
    let mut out = RgbaImage::new(w + MARGIN * 2, h + MARGIN * 2);
    image::imageops::replace(
        &mut out,
        &image::imageops::crop_imm(art, x0, y0, w, h).to_image(),
        i64::from(MARGIN),
        i64::from(MARGIN),
    );
    Some((out, (x0, y0, w, h)))
}

/// The middle of the lowest few rows of visible art: usually the feet.
fn guess_ground(art: &RgbaImage) -> (f32, f32) {
    let (w, h) = art.dimensions();
    let Some(bottom) = (0..h)
        .rev()
        .find(|&y| (0..w).any(|x| art.get_pixel(x, y)[3] >= SOLID))
    else {
        return (0.5, 1.0);
    };
    let band = (h / 25).max(1);
    let (mut sum, mut count) = (0.0f64, 0u32);
    for y in bottom.saturating_sub(band)..=bottom {
        for x in 0..w {
            if art.get_pixel(x, y)[3] >= SOLID {
                sum += f64::from(x) + 0.5;
                count += 1;
            }
        }
    }
    let x = if count == 0 {
        0.5
    } else {
        (sum / f64::from(count)) as f32 / w as f32
    };
    (x, (bottom + 1) as f32 / h as f32)
}

fn slug(name: &str) -> String {
    let mut s: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    let s = s.trim_matches('-').chars().take(32).collect::<String>();
    if s.is_empty() { "character".into() } else { s }
}

/// An ID no installed or bundled character uses.
pub(super) fn unique_id(root: &Path, base: &str, taken: &[&str]) -> String {
    let base = slug(base);
    let mut id = base.clone();
    let mut n = 2;
    while id == BUILTIN || taken.contains(&id.as_str()) || root.join(&id).exists() {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

/// Installs a quick-import draft as a new character and returns it.
pub fn install(draft: &Draft, taken: &[&str], job: &AtomicU8) -> Result<Character, String> {
    install_into_guarded(
        &user_dir().ok_or("no per-user data folder")?,
        draft,
        taken,
        || begin_commit(job),
    )
}

#[cfg(test)]
fn install_into(root: &Path, draft: &Draft, taken: &[&str]) -> Result<Character, String> {
    install_into_guarded(root, draft, taken, || Ok(()))
}

fn install_into_guarded(
    root: &Path,
    draft: &Draft,
    taken: &[&str],
    gate: impl FnOnce() -> Result<(), String>,
) -> Result<Character, String> {
    let name = draft.name.trim();
    if name.is_empty() {
        return Err("give the character a name".into());
    }
    let (gx, gy) = draft.ground;
    if !(0.0..=1.0).contains(&gx) || !(0.0..=1.0).contains(&gy) {
        return Err("click where the character stands".into());
    }
    let manifest = Manifest {
        id: unique_id(root, name, taken),
        name: name.chars().take(60).collect(),
        author: None,
        canvas: (draft.width, draft.height),
        default_pose: "sit".into(),
        poses: vec![Pose {
            name: "sit".into(),
            ground: draft.ground,
            body: "sit/body.png".into(),
            parts: Vec::new(),
            eyes: Vec::new(),
        }],
        source: Some(draft.source),
        editor: Vec::new(),
    };
    let art = draft.art.clone();
    stage_and_install_guarded(
        root,
        &manifest,
        None,
        |stage| {
            fs::create_dir_all(stage.join("sit")).map_err(|e| e.to_string())?;
            art.save_with_format(stage.join("sit/body.png"), image::ImageFormat::Png)
                .map_err(|e| e.to_string())
        },
        gate,
    )
}

/// Writes a pack into a hidden staging folder, validates it there, then moves
/// it into place. With `replace`, the installed folder of that ID is swapped
/// only after the new one has passed. A failure leaves nothing behind.
pub(super) fn stage_and_install(
    root: &Path,
    manifest: &Manifest,
    replace: Option<&str>,
    write_art: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<Character, String> {
    stage_and_install_guarded(root, manifest, replace, write_art, || Ok(()))
}

pub(super) fn stage_and_install_guarded(
    root: &Path,
    manifest: &Manifest,
    replace: Option<&str>,
    write_art: impl FnOnce(&Path) -> Result<(), String>,
    gate: impl FnOnce() -> Result<(), String>,
) -> Result<Character, String> {
    let _guard = INSTALL_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    fs::create_dir_all(root).map_err(|e| format!("create {}: {e}", root.display()))?;
    let recovery_errors = recover_transactions_locked(root);
    if !recovery_errors.is_empty() {
        return Err(format!(
            "Previous character recovery needs attention: {}",
            recovery_errors.join("; ")
        ));
    }
    let stage = root.join(format!(
        ".staging-{}-{}-{}-{}",
        manifest.id,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        NEXT_STAGE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&stage).map_err(|e| format!("create {}: {e}", stage.display()))?;
    let result = (|| {
        write_art(&stage).map_err(|e| format!("stage artwork: {e}"))?;
        let text = serde_json::to_string_pretty(&manifest.to_json()).map_err(|e| e.to_string())?;
        fs::write(stage.join(MANIFEST), text).map_err(|e| e.to_string())?;
        // Validate exactly what will be installed, every pose.
        let checked = read_manifest(&stage).map_err(|e| format!("stage manifest: {e}"))?;
        checked
            .validate_files(&stage)
            .map_err(|e| format!("validate staged character: {e}"))?;
        let art = read_png(&stage, &checked.default_pose().body, checked.canvas)?;
        thumbnail(&art)
            .save_with_format(stage.join(THUMB), image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        let target = root.join(&manifest.id);
        // The exact cancellation boundary: once accepted, always finish or roll back.
        gate()?;
        fs::write(
            stage.join(".revision"),
            format!(
                "{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos(),
                NEXT_STAGE.fetch_add(1, Ordering::Relaxed)
            ),
        )
        .map_err(|e| e.to_string())?;
        commit_stage(root, &stage, &target, replace.is_some(), |p| {
            let mut c = load_folder(p).map_err(|e| format!("load installed: {e}"))?;
            c.cached_rig = Some(Arc::new(Mutex::new(Some(
                c.rig().map_err(|e| format!("load installed rig: {e}"))?,
            ))));
            Ok(c)
        })
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&stage);
    }
    result
}

/// A journal directory owns its backup. Its presence means the swap has not
/// committed; recovery always restores the previous version. Only a successful
/// final load creates `committed`, after which recovery keeps the new version.
fn commit_stage<T>(
    root: &Path,
    stage: &Path,
    target: &Path,
    replace: bool,
    load: impl FnOnce(&Path) -> Result<T, String>,
) -> Result<T, String> {
    if target.exists() && !replace {
        return Err("a character with this ID was installed meanwhile; try Keep both".into());
    }
    let journal = stage.with_extension("transaction");
    fs::create_dir(&journal).map_err(|e| format!("create journal {}: {e}", journal.display()))?;
    let id = target
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("invalid destination")?;
    fs::write(journal.join("target"), id).map_err(|e| format!("write journal: {e}"))?;
    let backup = journal.join("backup");
    let result = (|| {
        if target.exists() {
            fs::rename(target, &backup).map_err(|e| format!("backup {}: {e}", target.display()))?;
        } else {
            fs::write(journal.join("new"), b"").map_err(|e| e.to_string())?;
        }
        fs::rename(stage, target).map_err(|e| format!("install {}: {e}", target.display()))?;
        let value = load(target)?;
        fs::write(journal.join("committed"), b"").map_err(|e| e.to_string())?;
        Ok(value)
    })();
    match result {
        Ok(value) => {
            let _ = fs::remove_dir_all(&journal);
            Ok(value)
        }
        Err(error) => {
            if let Err(rollback) = recover_transaction(root, &journal) {
                return Err(format!(
                    "{error}; recovery failed: {rollback}. Keep {} for recovery",
                    journal.display()
                ));
            }
            Err(error)
        }
    }
}

fn recover_transaction(root: &Path, journal: &Path) -> Result<(), String> {
    let id = fs::read_to_string(journal.join("target")).map_err(|e| e.to_string())?;
    if !valid_id(&id) || id == BUILTIN {
        return Err("invalid transaction destination".into());
    }
    let target = root.join(id);
    let backup = journal.join("backup");
    if !journal.join("committed").exists() && backup.exists() {
        if target.exists() {
            fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
        }
        fs::rename(&backup, &target).map_err(|e| e.to_string())?;
    } else if !journal.join("committed").exists() && journal.join("new").exists() && target.exists()
    {
        fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
    }
    fs::remove_dir_all(journal).map_err(|e| e.to_string())
}

fn recover_transactions(root: &Path) -> Vec<String> {
    let _guard = INSTALL_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    recover_transactions_locked(root)
}

fn recover_transactions_locked(root: &Path) -> Vec<String> {
    let mut errors = Vec::new();
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(".staging-") && n.ends_with(".transaction"))
                && let Err(e) = recover_transaction(root, &path)
            {
                errors.push(format!("{}: {e}", path.display()));
            }
        }
    }
    errors
}

/// Installs a plain 100 × 100 character into `dir/characters` for tests.
#[cfg(test)]
pub(super) fn test_install_folder(dir: &Path, on_duplicate: OnDuplicate) -> (PathBuf, Character) {
    let src = dir.join("fixture");
    fs::create_dir_all(src.join("sit")).unwrap();
    RgbaImage::from_fn(100, 100, |x, y| {
        let on = (30..70).contains(&x) && (50..95).contains(&y);
        image::Rgba(if on { [40, 80, 200, 255] } else { [0, 0, 0, 0] })
    })
    .save(src.join("sit/body.png"))
    .unwrap();
    fs::write(
        src.join(MANIFEST),
        r#"{"schema_version":1,"id":"fixture","name":"Fixture","canvas":[100,100],
          "default_pose":"sit","poses":{"sit":{"ground_anchor":[0.5,0.95],"body":"sit/body.png"}}}"#,
    )
    .unwrap();
    let root = dir.join("characters");
    let Ok(Imported::Installed(c)) = import_folder_into(&root, &src, &[], on_duplicate) else {
        panic!("fixture installs");
    };
    (root, c)
}

// ---- Folder import, export, removal -----------------------------------------

/// What to do when an imported pack's ID is already installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnDuplicate {
    Ask,
    KeepBoth,
    Replace,
}

#[derive(Debug, Clone)]
pub enum Imported {
    Installed(Character),
    /// The ID is taken; ask, then import again with a choice.
    Duplicate {
        name: String,
    },
}

/// An immutable, validated copy retained while a duplicate decision is open.
#[derive(Debug)]
pub struct PreparedFolder {
    path: PathBuf,
    id: String,
    expected_revision: Option<Vec<u8>>,
}

impl Drop for PreparedFolder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

impl PreparedFolder {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn install(
        &self,
        taken: &[&str],
        choice: OnDuplicate,
        job: &AtomicU8,
    ) -> Result<Imported, String> {
        let root = user_dir().ok_or("no per-user data folder")?;
        self.install_into(&root, taken, choice, job)
    }

    fn install_into(
        &self,
        root: &Path,
        taken: &[&str],
        choice: OnDuplicate,
        job: &AtomicU8,
    ) -> Result<Imported, String> {
        import_folder_into_guarded(root, self.path(), taken, choice, || {
            if choice == OnDuplicate::Replace
                && self.id != BUILTIN
                && revision(&root.join(&self.id))? != self.expected_revision
            {
                return Err("The installed character changed while this import was open. Import again before replacing it.".into());
            }
            begin_commit(job)
        })
    }
}

pub fn prepare_folder(source: &Path) -> Result<Arc<PreparedFolder>, String> {
    let root = user_dir().ok_or("no per-user data folder")?;
    prepare_folder_into(&root, source)
}

fn prepare_folder_into(root: &Path, source: &Path) -> Result<Arc<PreparedFolder>, String> {
    let m = read_manifest(source)?;
    let expected_revision = revision(&root.join(&m.id))?;
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let path = root.join(format!(
        ".prepared-{}-{}",
        std::process::id(),
        NEXT_STAGE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).map_err(|e| e.to_string())?;
    let prepared = Arc::new(PreparedFolder {
        path,
        id: m.id.clone(),
        expected_revision,
    });
    let assets = m
        .resources()
        .into_iter()
        .map(|(a, _)| a.to_owned())
        .collect::<Vec<_>>();
    // Validate containment before copying, and validate the actual copied bytes.
    m.validate_files(source)?;
    copy_assets(source, prepared.path(), &assets)?;
    fs::write(prepared.path.join(MANIFEST), m.to_json().to_string()).map_err(|e| e.to_string())?;
    m.validate_files(prepared.path())?;
    Ok(prepared)
}

/// Imports a character folder, copying only the files its manifest names.
#[cfg(test)]
fn import_folder_into(
    root: &Path,
    source: &Path,
    taken: &[&str],
    on_duplicate: OnDuplicate,
) -> Result<Imported, String> {
    import_folder_into_guarded(root, source, taken, on_duplicate, || Ok(()))
}

fn import_folder_into_guarded(
    root: &Path,
    source: &Path,
    taken: &[&str],
    on_duplicate: OnDuplicate,
    gate: impl FnOnce() -> Result<(), String>,
) -> Result<Imported, String> {
    let mut m = read_manifest(source)?;
    // Check the art before asking anything.
    m.validate_files(source)?;
    let exists = m.id == BUILTIN || taken.contains(&m.id.as_str()) || root.join(&m.id).exists();
    let mut replace = None;
    if exists {
        match on_duplicate {
            OnDuplicate::Ask => return Ok(Imported::Duplicate { name: m.name }),
            OnDuplicate::KeepBoth => m.id = unique_id(root, &m.id, taken),
            // The bundled character is never replaced.
            OnDuplicate::Replace if m.id == BUILTIN => m.id = unique_id(root, &m.id, taken),
            OnDuplicate::Replace => replace = Some(m.id.clone()),
        }
    }
    let assets: Vec<String> = m
        .resources()
        .into_iter()
        .map(|(a, _)| a.to_owned())
        .collect();
    stage_and_install_guarded(
        root,
        &m,
        replace.as_deref(),
        |stage| copy_assets(source, stage, &assets),
        gate,
    )
    .map(Imported::Installed)
}

fn begin_commit(job: &AtomicU8) -> Result<(), String> {
    job.compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
        .map(|_| ())
        .map_err(|_| "Import cancelled".into())
}

fn revision(folder: &Path) -> Result<Option<Vec<u8>>, String> {
    if !folder.exists() {
        return Ok(None);
    }
    let file = if folder.join(".revision").exists() {
        folder.join(".revision")
    } else {
        folder.join(MANIFEST)
    };
    fs::read(file).map(Some).map_err(|e| e.to_string())
}

/// Copies the named images, and nothing else, keeping their sub-folders.
fn copy_assets(from: &Path, to: &Path, assets: &[String]) -> Result<(), String> {
    for asset in assets {
        let target = to.join(asset);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::copy(from.join(asset), target).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// The installed folder, checked to sit directly in the characters folder.
pub(super) fn installed_folder(c: &Character) -> Result<PathBuf, String> {
    let root = user_dir()
        .ok_or("no per-user data folder")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let folder = c
        .folder
        .as_ref()
        .ok_or("Yohta is built in and can't be removed")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if folder.parent() != Some(root.as_path())
        || folder.file_name().and_then(|n| n.to_str()) != Some(c.id)
    {
        return Err("this character is outside TinyPaw's character folder".into());
    }
    Ok(folder)
}

/// Deletes TinyPaw's installed copy; the original source is never touched.
pub fn remove(c: &Character) -> Result<(), String> {
    fs::remove_dir_all(installed_folder(c)?).map_err(|e| e.to_string())
}

/// Writes the character as a folder inside `into`, ready to share or
/// re-import. Returns the folder written.
pub fn export(c: &Character, into: &Path) -> Result<PathBuf, String> {
    let folder = c
        .folder
        .as_ref()
        .ok_or("Yohta can be exported once his art moves to the pack format")?;
    let m = read_manifest(folder)?;
    let mut target = into.join(&m.id);
    let mut n = 2;
    while target.exists() {
        target = into.join(format!("{}-{n}", m.id));
        n += 1;
    }
    fs::create_dir_all(&target).map_err(|e| e.to_string())?;
    let result = (|| {
        let assets: Vec<String> = m
            .resources()
            .into_iter()
            .map(|(a, _)| a.to_owned())
            .collect();
        m.validate_files(folder)?;
        copy_assets(folder, &target, &assets)?;
        let text = serde_json::to_string_pretty(&m.to_json()).map_err(|e| e.to_string())?;
        fs::write(target.join(MANIFEST), text).map_err(|e| e.to_string())
    })();
    match result {
        Ok(()) => Ok(target),
        Err(e) => {
            let _ = fs::remove_dir_all(&target);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("tinypaw-character-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A 200 × 300 PNG with a 60 × 120 figure whose feet are at row 249.
    fn figure(path: &Path) {
        let mut img = RgbaImage::new(200, 300);
        for y in 130..250 {
            for x in 70..130 {
                img.put_pixel(x, y, image::Rgba([80, 120, 220, 255]));
            }
        }
        img.save(path).unwrap();
    }

    #[test]
    fn failed_final_load_restores_previous_installation() {
        let root = scratch("rollback");
        let target = root.join("cat");
        let stage = root.join(".staging-cat-test");
        fs::create_dir(&target).unwrap();
        fs::create_dir(&stage).unwrap();
        fs::write(target.join("version"), "old").unwrap();
        fs::write(stage.join("version"), "new").unwrap();
        let result: Result<(), _> = commit_stage(&root, &stage, &target, true, |_| {
            Err("injected load failure".into())
        });
        assert!(result.unwrap_err().contains("injected load failure"));
        assert_eq!(fs::read_to_string(target.join("version")).unwrap(), "old");
        assert!(recover_transactions(&root).is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn interrupted_swap_recovers_backup_and_committed_swap_keeps_new() {
        for committed in [false, true] {
            let root = scratch(if committed {
                "recovery-committed"
            } else {
                "recovery"
            });
            let journal = root.join(".staging-cat-test.transaction");
            fs::create_dir_all(journal.join("backup")).unwrap();
            fs::create_dir(root.join("cat")).unwrap();
            fs::write(journal.join("target"), "cat").unwrap();
            fs::write(journal.join("backup/version"), "old").unwrap();
            fs::write(root.join("cat/version"), "new").unwrap();
            if committed {
                fs::write(journal.join("committed"), "").unwrap();
            }
            assert!(recover_transactions(&root).is_empty());
            assert_eq!(
                fs::read_to_string(root.join("cat/version")).unwrap(),
                if committed { "new" } else { "old" }
            );
            assert!(!journal.exists());
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn destination_conflict_and_failed_stage_rename_preserve_old_pack() {
        let root = scratch("rename-failure");
        let target = root.join("cat");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("version"), "old").unwrap();
        let stage = root.join(".staging-missing");
        let result = commit_stage(&root, &stage, &target, true, |_| Ok(()));
        assert!(result.unwrap_err().contains("install"));
        assert_eq!(fs::read_to_string(target.join("version")).unwrap(), "old");
        assert!(commit_stage(&root, &stage, &target, false, |_| Ok(())).is_err());
        assert_eq!(fs::read_to_string(target.join("version")).unwrap(), "old");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn prepared_snapshot_survives_source_changes_and_rejects_stale_replacement() {
        let dir = scratch("snapshot");
        let (root, installed) = test_install_folder(&dir, OnDuplicate::Ask);
        let source = dir.join("fixture");
        let prepared = prepare_folder_into(&root, &source).unwrap();
        fs::write(source.join("sit/body.png"), b"changed after validation").unwrap();
        let Imported::Installed(copy) = prepared
            .install_into(
                &root,
                &[installed.id],
                OnDuplicate::KeepBoth,
                &AtomicU8::new(0),
            )
            .unwrap()
        else {
            panic!("expected installed snapshot");
        };
        assert!(copy.rig().is_ok());
        let Imported::Installed(_) = prepared
            .install_into(&root, &[], OnDuplicate::Replace, &AtomicU8::new(0))
            .unwrap()
        else {
            panic!("expected replacement");
        };
        let err = prepared
            .install_into(&root, &[], OnDuplicate::Replace, &AtomicU8::new(0))
            .unwrap_err();
        assert!(err.contains("changed while"), "{err}");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn cancellation_before_commit_preserves_the_installed_pack() {
        let dir = scratch("cancel");
        let (root, installed) = test_install_folder(&dir, OnDuplicate::Ask);
        let prepared = prepare_folder_into(&root, &dir.join("fixture")).unwrap();
        let before = revision(installed.folder.as_ref().unwrap()).unwrap();
        let err = prepared
            .install_into(&root, &[], OnDuplicate::Replace, &AtomicU8::new(2))
            .unwrap_err();
        assert!(err.contains("cancelled"), "{err}");
        assert_eq!(
            revision(installed.folder.as_ref().unwrap()).unwrap(),
            before
        );
        assert!(installed.rig().is_ok());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn failed_rollback_keeps_backup_for_startup_recovery() {
        use std::os::windows::fs::OpenOptionsExt;
        let root = scratch("rollback-locked");
        let stage = root.join(".staging-cat-test");
        let target = root.join("cat");
        fs::create_dir(&stage).unwrap();
        fs::create_dir(&target).unwrap();
        fs::write(stage.join("version"), "new").unwrap();
        fs::write(target.join("version"), "old").unwrap();
        let mut locked = None;
        let result: Result<(), _> = commit_stage(&root, &stage, &target, true, |p| {
            locked = Some(
                fs::OpenOptions::new()
                    .read(true)
                    .share_mode(0)
                    .open(p.join("version"))
                    .unwrap(),
            );
            Err("injected failure".into())
        });
        let err = result.unwrap_err();
        assert!(err.contains("recovery failed"), "{err}");
        let journal = stage.with_extension("transaction");
        assert_eq!(
            fs::read_to_string(journal.join("backup/version")).unwrap(),
            "old"
        );
        drop(locked);
        assert!(recover_transactions(&root).is_empty());
        assert_eq!(fs::read_to_string(target.join("version")).unwrap(), "old");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn oversized_sources_and_aggregate_pack_memory_are_bounded() {
        let dir = scratch("image-budget");
        let path = dir.join("wide.png");
        RgbaImage::new(MAX_SOURCE_SIDE + 1, 1).save(&path).unwrap();
        assert!(draft_from_png(&path).unwrap_err().contains("at most"));
        let parts: Vec<_> = (0..9).map(|n| json!({ "type": "other", "image": format!("part-{n}.png"), "pivot": [0.5, 0.5] })).collect();
        let manifest = json!({ "schema_version": 1, "id": "cat", "name": "Cat", "canvas": [2048,2048],
            "default_pose": "sit", "poses": { "sit": { "body": "body.png", "ground_anchor": [0.5,0.9], "parts": parts } } });
        assert!(
            parse_manifest(&manifest.to_string())
                .unwrap_err()
                .contains("memory budget")
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn editor_sources_survive_import_export_and_missing_sources_are_rejected() {
        use crate::mascot::placement::{self, Placement, SourceRecord};
        let dir = scratch("editor-resources");
        let (root, installed) = test_install_folder(&dir, OnDuplicate::Ask);
        let source = installed.folder.as_ref().unwrap();
        let mut manifest = read_manifest(source).unwrap();
        let original = RgbaImage::from_pixel(12, 14, image::Rgba([80, 120, 200, 255]));
        let transform = Placement {
            x: 40.0,
            y: 45.0,
            scale: 1.2,
            angle: 15.0,
        };
        std::fs::create_dir_all(source.join("editor/sources")).unwrap();
        original
            .save(source.join("editor/sources/body.png"))
            .unwrap();
        placement::bake(&original, transform, manifest.canvas)
            .save(source.join(&manifest.default_pose().body))
            .unwrap();
        manifest.editor = vec![SourceRecord {
            target: manifest.default_pose().body.clone(),
            source: "editor/sources/body.png".into(),
            size: original.dimensions(),
            placement: transform,
        }];
        std::fs::write(source.join(MANIFEST), manifest.to_json().to_string()).unwrap();
        let Imported::Installed(copy) =
            import_folder_into(&root, source, &[installed.id], OnDuplicate::KeepBoth).unwrap()
        else {
            panic!("expected installed copy")
        };
        let copied = copy.folder.as_ref().unwrap();
        assert_eq!(
            read_manifest(copied).unwrap().editor[0].placement,
            transform
        );
        assert_eq!(
            read_png(copied, "editor/sources/body.png", (12, 14)).unwrap(),
            original
        );
        let exported = export(&copy, &dir.join("export")).unwrap();
        assert_eq!(
            read_manifest(&exported).unwrap().editor[0].placement,
            transform
        );
        std::fs::remove_file(exported.join("editor/sources/body.png")).unwrap();
        assert!(import_folder_into(&root, &exported, &[], OnDuplicate::KeepBoth).is_err());
        assert!(copy.rig().is_ok());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn manifest_rules() {
        let ok = r#"{"schema_version":1,"id":"cat","name":"Cat","canvas":[64,64],
            "default_pose":"sit","poses":{"sit":{"ground_anchor":[0.5,0.9],"body":"sit/body.png"}}}"#;
        assert!(parse_manifest(ok).is_ok());
        let bad = [
            ok.replace("\"schema_version\":1", "\"schema_version\":2"),
            ok.replace("\"cat\"", "\"Cat Id\""),
            ok.replace("[64,64]", "[64,4096]"),
            ok.replace("\"default_pose\":\"sit\"", "\"default_pose\":\"stand\""),
            ok.replace("[0.5,0.9]", "[0.5,1.5]"),
            ok.replace("sit/body.png", "../body.png"),
            ok.replace("sit/body.png", "C:/body.png"),
            ok.replace("\"sit\":{", "\"fly\":{"),
        ];
        for m in &bad {
            assert!(parse_manifest(m).is_err(), "{m}");
        }
        // The original spec's head/tail shorthand, and a part riding on it.
        let layered = ok.replace(
            "\"body\":\"sit/body.png\"",
            r#""body":"sit/body.png","head":{"image":"sit/head.png","pivot":[0.5,0.6],
               "expressions":{"pressed":"sit/head-pressed.png"}},
               "parts":[{"type":"ear","image":"sit/ear.png","pivot":[0.4,0.3],"parent":"head",
                 "motion":{"preset":"floppy","strength":1.5}}],
               "eyes":[{"white":"sit/w.png","pupil":"sit/p.png","mask":"sit/m.png"}]"#,
        );
        let m = parse_manifest(&layered).unwrap();
        let pose = m.default_pose();
        assert_eq!(pose.parts.len(), 2);
        assert_eq!(pose.parts[1].parent, Some(0), "the ear rides on the head");
        assert!(pose.parts[1].behind, "ears default behind the head");
        assert_eq!(pose.parts[1].motion.preset, Preset::Floppy);
        assert_eq!(m.assets().len(), 7);
        // And it survives a round trip through to_json.
        let again = parse_manifest(&m.to_json().to_string()).unwrap();
        assert_eq!(again.assets(), m.assets());
        assert_eq!(again.default_pose().parts[1].parent, Some(0));

        let broken = [
            // A preset the type doesn't offer.
            layered.replace("\"floppy\"", "\"wag\""),
            // Strength out of range.
            layered.replace("1.5", "3.0"),
            // Riding on a head that isn't there.
            layered.replace(r#""head":{"#, r#""tail":{"#),
            // Faces on something other than the head.
            layered.replace(
                "\"type\":\"ear\"",
                "\"type\":\"ear\",\"expressions\":{\"pressed\":\"x.png\"}",
            ),
            // An unknown face.
            layered.replace(
                "\"pressed\":\"sit/head-pressed.png\"",
                "\"sleepy\":\"s.png\"",
            ),
        ];
        for m in &broken {
            assert!(parse_manifest(m).is_err(), "{m}");
        }
    }

    #[test]
    fn one_png_round_trip() {
        let dir = scratch("png");
        let png = dir.join("My Cat.png");
        figure(&png);
        let draft = draft_from_png(&png).unwrap();
        assert_eq!(draft.name, "My Cat");
        // Trimmed to the figure plus the margin.
        assert_eq!(
            (draft.width, draft.height),
            (60 + 2 * MARGIN, 120 + 2 * MARGIN)
        );
        let (gx, gy) = draft.ground;
        assert!((gx - 0.5).abs() < 0.02 && (gy - (124.0 / 128.0)).abs() < 0.01);

        let root = dir.join("characters");
        let c = install_into(&root, &draft, &["my-cat"]).unwrap();
        assert_eq!(c.id, "my-cat-2", "taken IDs are skipped");
        assert!(c.thumbnail().is_some());
        // A part drawn on the original 200 × 300 canvas lines up with the
        // trimmed body; other sizes are refused with both sizes named.
        let m = read_manifest(c.folder.as_ref().unwrap()).unwrap();
        let src = m.source.expect("quick import records its cut");
        assert_eq!(src.size, (200, 300));
        let part = dir.join("tail.png");
        RgbaImage::from_pixel(200, 300, image::Rgba([1, 2, 3, 255]))
            .save(&part)
            .unwrap();
        let fitted = fit_png(&part, m.canvas, m.source.as_ref()).unwrap();
        assert_eq!(fitted.dimensions(), m.canvas);
        RgbaImage::new(50, 50).save(&part).unwrap();
        let err = fit_png(&part, m.canvas, m.source.as_ref()).unwrap_err();
        assert!(
            err.contains("50 × 50") && err.contains("200 × 300"),
            "{err}"
        );
        // The installed copy no longer needs the source file.
        fs::remove_file(&png).unwrap();
        let rig = c.rig().unwrap();
        assert!((rig.anchor().y - gy * draft.height as f32).abs() < 0.5);
        assert!(
            !fs::read_dir(&root)
                .unwrap()
                .flatten()
                .any(|e| e.file_name().to_string_lossy().starts_with('.')),
            "no staging folder is left behind"
        );

        // Export, then import the export: a duplicate until told what to do.
        let out = dir.join("exported");
        fs::create_dir_all(&out).unwrap();
        let folder = export(&c, &out).unwrap();
        assert!(!folder.join(THUMB).exists(), "exports carry only pack data");
        let taken = [c.id];
        assert!(matches!(
            import_folder_into(&root, &folder, &taken, OnDuplicate::Ask).unwrap(),
            Imported::Duplicate { .. }
        ));
        let Imported::Installed(both) =
            import_folder_into(&root, &folder, &taken, OnDuplicate::KeepBoth).unwrap()
        else {
            panic!("installed");
        };
        assert_ne!(both.id, c.id);
        let Imported::Installed(same) =
            import_folder_into(&root, &folder, &taken, OnDuplicate::Replace).unwrap()
        else {
            panic!("installed");
        };
        assert_eq!(same.id, c.id);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn layered_folder_imports_and_renders() {
        let dir = scratch("layered");
        let src = dir.join("src");
        fs::create_dir_all(src.join("sit")).unwrap();
        let block = |x0: u32, y0: u32, x1: u32, y1: u32| {
            RgbaImage::from_fn(100, 100, |x, y| {
                let on = (x0..x1).contains(&x) && (y0..y1).contains(&y);
                image::Rgba(if on {
                    [90, 120, 200, 255]
                } else {
                    [0, 0, 0, 0]
                })
            })
        };
        for (name, img) in [
            ("body", block(30, 50, 70, 95)),
            ("head", block(20, 10, 80, 55)),
            ("tail", block(65, 60, 95, 70)),
            ("white", block(40, 25, 50, 35)),
            ("pupil", block(44, 29, 46, 31)),
            ("mask", block(40, 25, 50, 35)),
        ] {
            img.save(src.join(format!("sit/{name}.png"))).unwrap();
        }
        fs::write(
            src.join(MANIFEST),
            r#"{"schema_version":1,"id":"cat","name":"Cat","canvas":[100,100],"default_pose":"sit",
              "poses":{"sit":{"ground_anchor":[0.5,0.95],"body":"sit/body.png",
                "head":{"image":"sit/head.png","pivot":[0.5,0.5]},
                "tail":{"image":"sit/tail.png","pivot":[0.66,0.65],"motion":{"preset":"wag"}},
                "eyes":[{"white":"sit/white.png","pupil":"sit/pupil.png","mask":"sit/mask.png"}]}}}"#,
        )
        .unwrap();
        let root = dir.join("characters");
        let Imported::Installed(c) =
            import_folder_into(&root, &src, &[], OnDuplicate::Ask).unwrap()
        else {
            panic!("installed");
        };
        let rig = c.rig().unwrap();
        assert!(rig.eye_point().is_some());
        assert_eq!(rig.motion_parts().len(), 2);
        // Every named image was copied, and nothing else.
        assert_eq!(fs::read_dir(root.join("cat/sit")).unwrap().count(), 6);
        // A missing part image fails the import cleanly.
        fs::remove_file(src.join("sit/tail.png")).unwrap();
        let err = import_folder_into(&root, &src, &[], OnDuplicate::KeepBoth)
            .err()
            .unwrap();
        assert!(err.contains("tail.png"), "{err}");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn broken_imports_leave_the_installation_alone() {
        let dir = scratch("broken");
        let root = dir.join("characters");
        let src = dir.join("src");
        fs::create_dir_all(src.join("sit")).unwrap();
        figure(&src.join("sit/body.png"));
        let manifest = |canvas: &str| {
            format!(
                r#"{{"schema_version":1,"id":"fox","name":"Fox","canvas":{canvas},
                "default_pose":"sit","poses":{{"sit":{{"ground_anchor":[0.5,0.83],"body":"sit/body.png"}}}}}}"#
            )
        };
        // The art doesn't match the declared canvas.
        fs::write(src.join(MANIFEST), manifest("[100,100]")).unwrap();
        let err = import_folder_into(&root, &src, &[], OnDuplicate::Ask)
            .err()
            .unwrap();
        assert!(
            err.contains("200 × 300") && err.contains("100 × 100"),
            "{err}"
        );
        // A missing body.
        fs::write(src.join(MANIFEST), manifest("[200,300]")).unwrap();
        fs::remove_file(src.join("sit/body.png")).unwrap();
        assert!(import_folder_into(&root, &src, &[], OnDuplicate::Ask).is_err());
        assert!(
            !root.exists() || fs::read_dir(&root).unwrap().count() == 0,
            "nothing installed"
        );
        // Bundled Yohta can't be replaced; the import becomes a copy.
        figure(&src.join("sit/body.png"));
        fs::write(
            src.join(MANIFEST),
            manifest("[200,300]").replace("\"fox\"", "\"yohta\""),
        )
        .unwrap();
        let Imported::Installed(copy) =
            import_folder_into(&root, &src, &[], OnDuplicate::Replace).unwrap()
        else {
            panic!("installed");
        };
        assert_ne!(copy.id, BUILTIN);
        assert!(remove(&Character::builtin()).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }
}
