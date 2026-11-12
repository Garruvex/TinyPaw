//! Manifest parsing and validation for character packs.

use super::*;

// ---- Manifest ---------------------------------------------------------------

#[derive(Debug)]
pub(in crate::mascot) struct Manifest {
    pub(in crate::mascot) id: String,
    pub(in crate::mascot) name: String,
    pub(in crate::mascot) author: Option<String>,
    pub(in crate::mascot) canvas: (u32, u32),
    pub(in crate::mascot) default_pose: String,
    pub(in crate::mascot) poses: Vec<Pose>,
    /// How quick import cut the canvas from the original PNG, so later part
    /// art drawn on that original lines up.
    pub(in crate::mascot) source: Option<Source>,
    pub(in crate::mascot) editor: Vec<super::super::placement::SourceRecord>,
    /// Whether the resting point follows the body art, kept in the editor
    /// metadata. Packs without it are manual, so their anchor is kept.
    pub(in crate::mascot) rest: super::super::rest::RestMode,
    /// What the character says, and its bubble's look.
    pub(in crate::mascot) lines: Lines,
    /// How it bops to music, if not the default nod.
    pub(in crate::mascot) bop: Option<Bop>,
    /// The pack's reaction defaults (`reactions`); the user's changes are
    /// overrides on top.
    pub(in crate::mascot) reactions: super::super::reactions::Config,
}

/// The original PNG's size and the visible box quick import kept from it
/// (before adding the margin and scaling to the canvas).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::mascot) struct Source {
    pub(in crate::mascot) size: (u32, u32),
    pub(in crate::mascot) visible: (u32, u32, u32, u32),
}

#[derive(Debug, Clone)]
pub(in crate::mascot) struct Pose {
    pub(in crate::mascot) name: String,
    pub(in crate::mascot) ground: (f32, f32),
    /// Relative path of the body image, using `/`.
    pub(in crate::mascot) body: String,
    pub(in crate::mascot) parts: Vec<PartDef>,
    pub(in crate::mascot) eyes: Vec<EyeDef>,
    /// A body-only pose's whole-character sway, if not the default.
    pub(in crate::mascot) whole: Option<MotionSpec>,
}

#[derive(Debug, Clone)]
pub(in crate::mascot) struct PartDef {
    pub(in crate::mascot) kind: PartType,
    pub(in crate::mascot) image: String,
    pub(in crate::mascot) pivot: (f32, f32),
    /// Index of the part this rides on; only the head can carry parts.
    pub(in crate::mascot) parent: Option<usize>,
    pub(in crate::mascot) behind: bool,
    pub(in crate::mascot) motion: MotionSpec,
    /// Head faces: blink, pressed, dragging.
    pub(in crate::mascot) faces: [Option<String>; 4],
}

#[derive(Debug, Clone)]
pub(in crate::mascot) struct EyeDef {
    pub(in crate::mascot) white: String,
    pub(in crate::mascot) pupil: String,
    pub(in crate::mascot) mask: String,
    pub(in crate::mascot) blink: Option<String>,
    pub(in crate::mascot) max_offset: (f32, f32),
}

impl Pose {
    /// Every image this pose names.
    pub(in crate::mascot) fn assets(&self) -> Vec<&str> {
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

    pub(in crate::mascot) fn to_json(&self) -> Json {
        let mut pose = json!({
            "ground_anchor": [self.ground.0, self.ground.1],
            "body": self.body,
        });
        if let Some(w) = &self.whole {
            pose["motion"] = json!({
                "preset": w.preset.name(),
                "strength": w.strength,
                "speed": w.speed,
            });
        }
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

pub(in crate::mascot) fn fraction_pair(v: &Json) -> Option<(f32, f32)> {
    match v.as_array()?.as_slice() {
        [x, y] => Some((x.as_f64()? as f32, y.as_f64()? as f32))
            .filter(|(x, y)| (0.0..=1.0).contains(x) && (0.0..=1.0).contains(y)),
        _ => None,
    }
}

pub(in crate::mascot) fn asset_field(v: &Json, what: &str) -> Result<String, String> {
    v.as_str()
        .filter(|a| valid_asset(a))
        .map(str::to_owned)
        .ok_or(format!("{what} must be a PNG inside the pack folder"))
}

/// One part, from `parts` or the `head`/`tail` shorthand. `parent` is
/// resolved later, once every part is known.
pub(in crate::mascot) fn parse_part(
    v: &Json,
    kind: Option<PartType>,
    n: usize,
) -> Result<(PartDef, bool), String> {
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
    let mut faces = [None, None, None, None];
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

pub(in crate::mascot) fn parse_eye(v: &Json, n: usize) -> Result<EyeDef, String> {
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
/// A body-only pose's `motion`: the head presets, which the whole image
/// uses for its sway.
pub(in crate::mascot) fn parse_whole(
    v: &Json,
    pose_name: &str,
) -> Result<Option<MotionSpec>, String> {
    if v.is_null() {
        return Ok(None);
    }
    let mut spec = MotionSpec::default_for(PartType::Head);
    if let Some(name) = v["preset"].as_str() {
        spec.preset = Preset::parse(name)
            .filter(|p| PartType::Head.presets().contains(p))
            .ok_or(format!(
                "the {pose_name} pose can't use the `{name}` motion"
            ))?;
    }
    for (field, value) in [("strength", &mut spec.strength), ("speed", &mut spec.speed)] {
        if let Some(x) = v[field].as_f64() {
            if !(0.0..=2.0).contains(&x) {
                return Err(format!(
                    "the {pose_name} pose's {field} must be between 0 and 2"
                ));
            }
            *value = x as f32;
        }
    }
    Ok(Some(spec))
}

pub(in crate::mascot) fn parse_layers(
    pose: &Json,
    pose_name: &str,
) -> Result<(Vec<PartDef>, Vec<EyeDef>), String> {
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
    pub(in crate::mascot) fn default_pose(&self) -> &Pose {
        self.poses
            .iter()
            .find(|p| p.name == self.default_pose)
            .expect("validated default pose")
    }

    /// Every image the pack names, each once.
    pub(in crate::mascot) fn assets(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for p in &self.poses {
            for a in p.assets() {
                if !out.contains(&a) {
                    out.push(a);
                }
            }
        }
        // Held poses and artwork reactions: canvas-sized, like any layer,
        // so they count against the image and memory limits too.
        for r in self.reactions.images() {
            if let SoundRef::Pack(a) = r
                && !out.contains(&a.as_str())
            {
                out.push(a);
            }
        }
        out
    }

    pub(in crate::mascot) fn resources(&self) -> Vec<(&str, (u32, u32))> {
        let mut out: Vec<_> = self
            .assets()
            .into_iter()
            .map(|a| (a, self.canvas))
            .collect();
        out.extend(self.editor.iter().map(|r| (r.source.as_str(), r.size)));
        out
    }

    /// Line sounds inside the pack, each once.
    pub(in crate::mascot) fn sounds(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for s in self
            .lines
            .sounds()
            .into_iter()
            .chain(self.reactions.files())
        {
            if let SoundRef::Pack(p) = s
                && !out.contains(&p.as_str())
            {
                out.push(p);
            }
        }
        out
    }

    /// Every file to copy with the pack.
    pub(in crate::mascot) fn files(&self) -> Vec<String> {
        self.resources()
            .into_iter()
            .map(|(a, _)| a)
            .chain(self.sounds())
            .map(str::to_owned)
            .collect()
    }

    pub(in crate::mascot) fn validate_files(&self, folder: &Path) -> Result<(), String> {
        for sound in self.sounds() {
            check_sound(folder, sound)?;
        }
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

    pub(in crate::mascot) fn to_json(&self) -> Json {
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
        if !self.editor.is_empty() || self.rest == super::super::rest::RestMode::Auto {
            m["editor"] = super::super::placement::to_json(&self.editor);
            // An optional field: older builds read version-1 editor
            // metadata and ignore it.
            m["editor"]["rest_point"] = json!(self.rest.name());
        }
        if let Some(src) = &self.source {
            let (x, y, w, h) = src.visible;
            m["source"] = json!({ "size": [src.size.0, src.size.1], "visible": [x, y, w, h] });
        }
        self.lines.write(&mut m, false);
        if !self.reactions.is_empty() {
            m["reactions"] = self.reactions.to_json();
        }
        if let Some(bop) = self.bop {
            m["music"] = json!({ "bop": bop.name() });
        }
        m
    }
}

/// A line sound inside `folder`: contained, small, and a readable WAV.
pub(in crate::mascot) fn check_sound(folder: &Path, asset: &str) -> Result<(), String> {
    let root = folder.canonicalize().map_err(|e| e.to_string())?;
    let path = folder
        .join(asset)
        .canonicalize()
        .map_err(|_| format!("{asset} is missing"))?;
    if !path.starts_with(&root) {
        return Err(format!("{asset} is outside the pack folder"));
    }
    if fs::metadata(&path).map_err(|e| e.to_string())?.len() > super::super::lines::MAX_SOUND {
        return Err(format!("{asset} is larger than 2 MiB"));
    }
    crate::audio::decode_wav(&path)
        .map(|_| ())
        .map_err(|e| format!("{asset} is not a readable WAV ({e})"))
}

impl Source {
    /// Cuts `art` (the original PNG's size) the way quick import cut the
    /// body: the visible box, the margin, then scaled to `canvas`.
    pub(in crate::mascot) fn apply(&self, art: &RgbaImage, canvas: (u32, u32)) -> RgbaImage {
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
pub(in crate::mascot) fn fit_png(
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

pub(in crate::mascot) fn valid_id(id: &str) -> bool {
    (1..=40).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// A path inside the pack: relative, `/`-separated, no `..`.
pub(in crate::mascot) fn valid_asset(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 200
        && !path.contains(['\\', ':'])
        && path.ends_with(".png")
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

pub(in crate::mascot) fn parse_manifest(text: &str) -> Result<Manifest, String> {
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
        let whole = parse_whole(&pose["motion"], pose_name)?;
        if whole.is_some() && !parts.is_empty() {
            return Err(format!(
                "the {pose_name} pose's motion is for a single image; give each part its own"
            ));
        }
        poses.push(Pose {
            whole,
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
        rest: m["editor"]["rest_point"]
            .as_str()
            .and_then(super::super::rest::RestMode::parse)
            .unwrap_or_default(),
        lines: Lines::parse(&m, false)?,
        // A style this version doesn't know bops the default way.
        bop: m["music"]["bop"].as_str().and_then(Bop::parse),
        reactions: super::super::reactions::Config::parse(&m["reactions"], false)?,
    };
    manifest.editor =
        super::super::placement::parse(&m["editor"], manifest.canvas, &manifest.assets())?;
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

pub(in crate::mascot) fn read_manifest(folder: &Path) -> Result<Manifest, String> {
    let path = folder.join(MANIFEST);
    let meta = fs::metadata(&path).map_err(|_| "this folder has no character.json".to_owned())?;
    if meta.len() > MAX_MANIFEST {
        return Err("character.json is too large".into());
    }
    parse_manifest(&fs::read_to_string(path).map_err(|e| e.to_string())?)
}

/// Reads one pack image, refusing paths that leave the folder (including
/// through links), oversized files and art that doesn't fit the canvas.
pub(in crate::mascot) fn read_png(
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
    decode_png(&bytes, asset, canvas)
}

/// Decodes one pack image from bytes: the canvas size is checked before the
/// pixels are decoded, and empty art is refused.
pub(in crate::mascot) fn decode_png(
    bytes: &[u8],
    asset: &str,
    canvas: (u32, u32),
) -> Result<RgbaImage, String> {
    let dimensions =
        image::ImageReader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Png)
            .into_dimensions()
            .map_err(|e| format!("{asset}: {e}"))?;
    if dimensions != canvas {
        return Err(format!(
            "{asset} is {} × {}. This character uses a {} × {} canvas",
            dimensions.0, dimensions.1, canvas.0, canvas.1
        ));
    }
    let art = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
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

pub(in crate::mascot) fn check_source_dimensions(bytes: &[u8]) -> Result<(), String> {
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
