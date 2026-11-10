//! Character packs: a folder holding `character.json` and PNG art, installed
//! as an independent copy under `%APPDATA%\TinyPaw\characters\<id>`. Bundled
//! Yohta and imported characters share one list and one selection setting.
//! Every pack is validated completely, and staged, before it can be used.
//!
//! This version renders one image per pose. The manifest is the version 1
//! format from `docs/mascot-feature.md`; layered parts (head, tail, eyes)
//! are rejected with a clear message until the renderer supports them.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use iced::advanced::image::Handle;
use image::RgbaImage;
use serde_json::{Value as Json, json};

use super::rig::Rig;

/// The bundled character's ID, also the fallback selection.
pub const BUILTIN: &str = "yohta";
const MANIFEST: &str = "character.json";
const THUMB: &str = "thumb.png";
/// Largest canvas side a pack may declare.
const MAX_SIDE: u32 = 2048;
/// Quick import scales art down to this height: plenty for 320 logical
/// pixels at 200 %, and far less memory than camera-sized PNGs.
const IMPORT_HEIGHT: u32 = 1024;
/// Largest PNG accepted for quick import, before scaling.
const MAX_SOURCE_SIDE: u32 = 8192;
/// Encoded bytes per image and per pack.
const MAX_FILE: u64 = 64 * 1024 * 1024;
const MAX_MANIFEST: u64 = 64 * 1024;
const THUMB_SIDE: u32 = 96;
/// Alpha at which a pixel counts as part of the character.
const SOLID: u8 = 32;
/// Transparent margin kept around trimmed quick-import art.
const MARGIN: u32 = 4;
const POSES: [&str; 2] = ["sit", "stand"];
/// Layered fields the renderer cannot draw yet.
const LAYERED: [&str; 4] = ["head", "tail", "eyes", "parts"];

/// An installed or bundled character, as listed in Settings.
#[derive(Debug, Clone)]
pub struct Character {
    /// Interned for the lifetime of the process, so settings can refer to it.
    pub id: &'static str,
    pub name: String,
    /// The installed folder; `None` for bundled Yohta.
    pub folder: Option<PathBuf>,
    thumb: Option<Handle>,
}

impl Character {
    pub fn builtin() -> Self {
        Self {
            id: BUILTIN,
            name: "Yohta".into(),
            folder: None,
            thumb: None,
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
        let Some(folder) = &self.folder else {
            return Ok(Rig::load());
        };
        let m = read_manifest(folder)?;
        let pose = m.default_pose();
        let art = read_png(folder, &pose.body, m.canvas)?;
        Ok(Rig::sprite(art, pose.ground))
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
struct Manifest {
    id: String,
    name: String,
    author: Option<String>,
    canvas: (u32, u32),
    default_pose: String,
    poses: Vec<Pose>,
}

#[derive(Debug)]
struct Pose {
    name: String,
    ground: (f32, f32),
    /// Relative path of the body image, using `/`.
    body: String,
}

impl Manifest {
    fn default_pose(&self) -> &Pose {
        self.poses
            .iter()
            .find(|p| p.name == self.default_pose)
            .expect("validated default pose")
    }

    fn to_json(&self) -> Json {
        let mut poses = serde_json::Map::new();
        for p in &self.poses {
            poses.insert(
                p.name.clone(),
                json!({ "ground_anchor": [p.ground.0, p.ground.1], "body": p.body }),
            );
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
        m
    }
}

fn valid_id(id: &str) -> bool {
    (1..=40).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// A path inside the pack: relative, `/`-separated, no `..`.
fn valid_asset(path: &str) -> bool {
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
        if let Some(field) = LAYERED.iter().find(|f| !pose[**f].is_null()) {
            return Err(format!(
                "the {pose_name} pose has a separate {field}. Moving parts arrive in a later \
                 update; for now import the whole character as its body image"
            ));
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
        poses.push(Pose {
            name: pose_name.clone(),
            ground,
            body: body.to_owned(),
        });
    }
    let default_pose = m["default_pose"].as_str().unwrap_or_default();
    if !poses.iter().any(|p| p.name == default_pose) {
        return Err("default_pose must name one of the poses".into());
    }
    Ok(Manifest {
        id: id.to_owned(),
        name: name.to_owned(),
        author,
        canvas: (w, h),
        default_pose: default_pose.to_owned(),
        poses,
    })
}

fn read_manifest(folder: &Path) -> Result<Manifest, String> {
    let path = folder.join(MANIFEST);
    let meta = fs::metadata(&path).map_err(|_| "this folder has no character.json".to_owned())?;
    if meta.len() > MAX_MANIFEST {
        return Err("character.json is too large".into());
    }
    parse_manifest(&fs::read_to_string(path).map_err(|e| e.to_string())?)
}

/// Reads one pack image, refusing paths that leave the folder (including
/// through links), oversized files and art that doesn't fit the canvas.
fn read_png(folder: &Path, asset: &str, canvas: (u32, u32)) -> Result<RgbaImage, String> {
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
}

/// Starts "Add character" from a transparent PNG: trims empty margins, scales
/// large art down and suggests a ground point under the feet.
pub fn draft_from_png(path: &Path) -> Result<Draft, String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > MAX_FILE {
        return Err("the PNG is larger than 64 MiB".into());
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
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
    let art = trim(&art).ok_or("the PNG is empty")?;
    let art = if art.height() > IMPORT_HEIGHT || art.width() > MAX_SIDE {
        let k = (IMPORT_HEIGHT as f32 / art.height() as f32)
            .min(MAX_SIDE as f32 / art.width() as f32);
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
    })
}

/// The visible art plus a small transparent margin, or `None` if empty.
fn trim(art: &RgbaImage) -> Option<RgbaImage> {
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
    Some(out)
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
fn unique_id(root: &Path, base: &str, taken: &[&str]) -> String {
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
pub fn install(draft: &Draft, taken: &[&str]) -> Result<Character, String> {
    install_into(&user_dir().ok_or("no per-user data folder")?, draft, taken)
}

fn install_into(root: &Path, draft: &Draft, taken: &[&str]) -> Result<Character, String> {
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
        }],
    };
    let art = draft.art.clone();
    stage_and_install(root, &manifest, None, |stage| {
        fs::create_dir_all(stage.join("sit")).map_err(|e| e.to_string())?;
        art.save_with_format(stage.join("sit/body.png"), image::ImageFormat::Png)
            .map_err(|e| e.to_string())
    })
}

/// Writes a pack into a hidden staging folder, validates it there, then moves
/// it into place. With `replace`, the installed folder of that ID is swapped
/// only after the new one has passed. A failure leaves nothing behind.
fn stage_and_install(
    root: &Path,
    manifest: &Manifest,
    replace: Option<&str>,
    write_art: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<Character, String> {
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let stage = root.join(format!(".staging-{}", manifest.id));
    let _ = fs::remove_dir_all(&stage);
    fs::create_dir(&stage).map_err(|e| e.to_string())?;
    let result = (|| {
        write_art(&stage)?;
        let text = serde_json::to_string_pretty(&manifest.to_json()).map_err(|e| e.to_string())?;
        fs::write(stage.join(MANIFEST), text).map_err(|e| e.to_string())?;
        // Validate exactly what will be installed, every pose.
        let checked = read_manifest(&stage)?;
        for pose in &checked.poses {
            read_png(&stage, &pose.body, checked.canvas)?;
        }
        let art = read_png(&stage, &checked.default_pose().body, checked.canvas)?;
        thumbnail(&art)
            .save_with_format(stage.join(THUMB), image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        let target = root.join(&manifest.id);
        if let Some(id) = replace {
            let old = root.join(id);
            if old.exists() {
                fs::remove_dir_all(&old).map_err(|e| e.to_string())?;
            }
        }
        fs::rename(&stage, &target).map_err(|e| e.to_string())?;
        load_folder(&target)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&stage);
    }
    result
}

// ---- Folder import, export, removal -----------------------------------------

/// What to do when an imported pack's ID is already installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnDuplicate {
    Ask,
    KeepBoth,
    Replace,
}

pub enum Imported {
    Installed(Character),
    /// The ID is taken; ask, then import again with a choice.
    Duplicate { name: String },
}

/// Imports a character folder, copying only the files its manifest names.
pub fn import_folder(
    source: &Path,
    taken: &[&str],
    on_duplicate: OnDuplicate,
) -> Result<Imported, String> {
    import_folder_into(
        &user_dir().ok_or("no per-user data folder")?,
        source,
        taken,
        on_duplicate,
    )
}

fn import_folder_into(
    root: &Path,
    source: &Path,
    taken: &[&str],
    on_duplicate: OnDuplicate,
) -> Result<Imported, String> {
    let mut m = read_manifest(source)?;
    // Check the art before asking anything.
    for pose in &m.poses {
        read_png(source, &pose.body, m.canvas)?;
    }
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
    let bodies: Vec<String> = m.poses.iter().map(|p| p.body.clone()).collect();
    stage_and_install(root, &m, replace.as_deref(), |stage| {
        for body in &bodies {
            let to = stage.join(body);
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(source.join(body), to).map_err(|e| e.to_string())?;
        }
        Ok(())
    })
    .map(Imported::Installed)
}

/// The installed folder, checked to sit directly in the characters folder.
fn installed_folder(c: &Character) -> Result<PathBuf, String> {
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
        for pose in &m.poses {
            read_png(folder, &pose.body, m.canvas)?;
            let to = target.join(&pose.body);
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(folder.join(&pose.body), to).map_err(|e| e.to_string())?;
        }
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
        let dir = std::env::temp_dir().join(format!(
            "tinypaw-character-{name}-{}",
            std::process::id()
        ));
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
        let layered = ok.replace("\"body\"", "\"head\":{\"image\":\"h.png\"},\"body\"");
        assert!(parse_manifest(&layered).unwrap_err().contains("later update"));
    }

    #[test]
    fn one_png_round_trip() {
        let dir = scratch("png");
        let png = dir.join("My Cat.png");
        figure(&png);
        let draft = draft_from_png(&png).unwrap();
        assert_eq!(draft.name, "My Cat");
        // Trimmed to the figure plus the margin.
        assert_eq!((draft.width, draft.height), (60 + 2 * MARGIN, 120 + 2 * MARGIN));
        let (gx, gy) = draft.ground;
        assert!((gx - 0.5).abs() < 0.02 && (gy - (124.0 / 128.0)).abs() < 0.01);

        let root = dir.join("characters");
        let c = install_into(&root, &draft, &["my-cat"]).unwrap();
        assert_eq!(c.id, "my-cat-2", "taken IDs are skipped");
        assert!(c.thumbnail().is_some());
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
        assert!(err.contains("200 × 300") && err.contains("100 × 100"), "{err}");
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
