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

mod dialogue;
mod fit;
mod manifest;
#[cfg(test)]
mod tests;

pub use dialogue::{DIALOGUE, Dialogue, export_dialogue, read_dialogue, write_template};
pub use fit::{Fit, Misfit};
pub(super) use manifest::*;

static INSTALL_LOCK: Mutex<()> = Mutex::new(());
static NEXT_STAGE: AtomicU64 = AtomicU64::new(0);

use iced::advanced::image::Handle;
use image::RgbaImage;
use serde_json::{Value as Json, json};

use super::lines::{Lines, SoundRef};
use super::motion::{Bop, MotionSpec, PartType, Preset};
use super::rig::{EyeArt, LayeredArt, PartArt, Rig};

/// The bundled character's ID, also the fallback selection.
pub const BUILTIN: &str = "yohta";

/// Bundled Yohta's pack (see `yohta_pack`), embedded so the release needs no
/// asset folder. It goes through the same parsing and checks as any pack.
const YOHTA_FILES: &[(&str, &[u8])] = &[
    (
        "character.json",
        include_bytes!("../../../assets/mascot/yohta/pack/character.json"),
    ),
    (
        "sit/body.png",
        include_bytes!("../../../assets/mascot/yohta/pack/sit/body.png"),
    ),
    (
        "sit/head.png",
        include_bytes!("../../../assets/mascot/yohta/pack/sit/head.png"),
    ),
    (
        "sit/tail.png",
        include_bytes!("../../../assets/mascot/yohta/pack/sit/tail.png"),
    ),
    (
        "sit/eye-1-white.png",
        include_bytes!("../../../assets/mascot/yohta/pack/sit/eye-1-white.png"),
    ),
    (
        "sit/eye-1-pupil.png",
        include_bytes!("../../../assets/mascot/yohta/pack/sit/eye-1-pupil.png"),
    ),
    (
        "sit/eye-1-mask.png",
        include_bytes!("../../../assets/mascot/yohta/pack/sit/eye-1-mask.png"),
    ),
    (
        "sit/eye-1-blink.png",
        include_bytes!("../../../assets/mascot/yohta/pack/sit/eye-1-blink.png"),
    ),
    (
        "sit/eye-2-white.png",
        include_bytes!("../../../assets/mascot/yohta/pack/sit/eye-2-white.png"),
    ),
    (
        "sit/eye-2-pupil.png",
        include_bytes!("../../../assets/mascot/yohta/pack/sit/eye-2-pupil.png"),
    ),
    (
        "sit/eye-2-mask.png",
        include_bytes!("../../../assets/mascot/yohta/pack/sit/eye-2-mask.png"),
    ),
    (
        "sit/eye-2-blink.png",
        include_bytes!("../../../assets/mascot/yohta/pack/sit/eye-2-blink.png"),
    ),
];
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
const FACES: [&str; 4] = ["blink", "pressed", "dragging", "petted"];
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

    /// A small picture for lists. Yohta's comes from his head art on first use.
    pub fn thumbnail(&self) -> Option<Handle> {
        if self.is_builtin() {
            static YOHTA: OnceLock<Option<Handle>> = OnceLock::new();
            return YOHTA
                .get_or_init(|| {
                    let m = bundled_manifest();
                    let head = m.default_pose().parts.first()?.image.clone();
                    let art = decode_png(bundled(&head).ok()?, &head, m.canvas).ok()?;
                    let (x, y, w, h) = visible_box(&art)?;
                    let face = image::imageops::crop_imm(&art, x, y, w, h).to_image();
                    Some(handle(thumbnail(&face)))
                })
                .clone();
        }
        self.thumb.clone()
    }

    /// The pack's own lines, before the user's changes.
    pub fn pack_lines(&self) -> Lines {
        match &self.folder {
            None => bundled_manifest().lines,
            Some(folder) => read_manifest(folder).map(|m| m.lines).unwrap_or_default(),
        }
    }

    /// What the character says: the user's lines if they changed them.
    pub fn lines(&self) -> Lines {
        load_overrides(self.id)
            .lines
            .unwrap_or_else(|| self.pack_lines())
    }

    /// The pack's own reaction defaults.
    pub fn pack_reactions(&self) -> super::reactions::Config {
        match &self.folder {
            None => bundled_manifest().reactions,
            Some(folder) => read_manifest(folder)
                .map(|m| m.reactions)
                .unwrap_or_default(),
        }
    }

    /// A reaction picture, decoded on the character's stage.
    pub fn reaction_image(&self, art: &super::reactions::Art) -> Option<RgbaImage> {
        let canvas = match &self.folder {
            None => bundled_manifest().canvas,
            Some(folder) => read_manifest(folder).ok()?.canvas,
        };
        let path = self.sound_path(&art.image)?;
        let bytes = fs::read(&path).ok()?;
        decode_png(&bytes, &path.to_string_lossy(), art.source_size.map(|[w,h]| (w,h)).unwrap_or(canvas)).ok()
    }

    /// What `e` does to the picture, pack under user.
    pub fn reaction_visual(&self, e: super::reactions::Event) -> super::reactions::Visual {
        let user = load_overrides(self.id).reactions.unwrap_or_default();
        super::reactions::visual(&self.pack_reactions(), &user, e)
    }

    /// What the character's reactions actually do: pack under user.
    pub fn reaction_sounds(&self) -> super::reactions::Sounds {
        let user = load_overrides(self.id).reactions.unwrap_or_default();
        super::reactions::effective(&self.pack_reactions(), &user)
    }

    /// The file a line or reaction sound plays from.
    pub fn sound_path(&self, sound: &SoundRef) -> Option<PathBuf> {
        match sound {
            SoundRef::Pack(p) => self.folder.as_ref().map(|f| f.join(p)),
            SoundRef::User(f) => user_dir().map(|r| user_sounds(&r, self.id).join(f)),
        }
    }

    /// The rig with the user's Motion tab settings and bop applied, if
    /// they still fit.
    pub fn rig_with_motion(&self) -> Result<Rig, String> {
        let mut rig = self.rig()?;
        let o = load_overrides(self.id);
        if let Some(specs) = o.motion {
            rig.set_motion(&specs);
        }
        if let Some(bop) = o.bop {
            rig.set_bop(bop);
        }
        Ok(rig)
    }

    /// The pack's own bop style, before the user's choice.
    pub fn pack_bop(&self) -> Bop {
        match &self.folder {
            None => bundled_manifest().bop,
            Some(folder) => read_manifest(folder).ok().and_then(|m| m.bop),
        }
        .unwrap_or_default()
    }

    /// How the character bops: the user's choice, else the pack's.
    pub fn bop(&self) -> Bop {
        load_overrides(self.id)
            .bop
            .unwrap_or_else(|| self.pack_bop())
    }

    /// Decodes the art for the desktop. Done when shown, released when hidden.
    pub fn rig(&self) -> Result<Rig, String> {
        if let Some(rig) = &self.cached_rig
            && let Some(rig) = rig.lock().unwrap_or_else(|e| e.into_inner()).take()
        {
            return Ok(rig);
        }
        let Some(folder) = &self.folder else {
            return Ok(bundled_rig());
        };
        let m = read_manifest(folder)?;
        build_rig(&m, |asset| read_png(folder, asset, m.canvas))
    }

    pub fn release_prepared_rig(&self) {
        if let Some(rig) = &self.cached_rig {
            rig.lock().unwrap_or_else(|e| e.into_inner()).take();
        }
    }
}

/// A rig from a manifest's default pose, reading images through `png`.
fn build_rig(m: &Manifest, png: impl Fn(&str) -> Result<RgbaImage, String>) -> Result<Rig, String> {
    let pose = m.default_pose();
    let body = png(&pose.body)?;
    if pose.parts.is_empty() && pose.eyes.is_empty() {
        let mut rig = Rig::sprite(body, pose.ground);
        if let Some(w) = pose.whole {
            rig.set_motion(&[w]);
        }
        rig.set_bop(m.bop.unwrap_or_default());
        return Ok(rig);
    }
    let mut parts = Vec::new();
    for part in &pose.parts {
        let mut faces = [None, None, None, None];
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
            blink: eye.blink.as_deref().map(&png).transpose()?,
            max_offset: eye.max_offset,
        });
    }
    let mut rig = Rig::layered(LayeredArt {
        body,
        ground: pose.ground,
        parts,
        eyes,
    });
    rig.set_bop(m.bop.unwrap_or_default());
    Ok(rig)
}

fn bundled(asset: &str) -> Result<&'static [u8], String> {
    YOHTA_FILES
        .iter()
        .find(|(name, _)| *name == asset)
        .map(|(_, bytes)| *bytes)
        .ok_or_else(|| format!("{asset} is missing from bundled Yohta"))
}

fn bundled_manifest() -> Manifest {
    let text = std::str::from_utf8(bundled(MANIFEST).expect("bundled manifest"))
        .expect("bundled manifest is UTF-8");
    parse_manifest(text).expect("bundled Yohta's manifest is valid")
}

/// Bundled Yohta's rig, decoded and checked like any installed pack.
pub(super) fn bundled_rig() -> Rig {
    let m = bundled_manifest();
    build_rig(&m, |asset| decode_png(bundled(asset)?, asset, m.canvas))
        .expect("bundled Yohta's art is valid")
}

/// Writes bundled Yohta's files into `to`, as a pack folder.
fn write_bundled(to: &Path, m: &Manifest) -> Result<(), String> {
    for asset in m.assets() {
        let target = to.join(asset);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(target, bundled(asset)?).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Installs an editable copy of bundled Yohta, which stays built in.
pub fn copy_builtin(taken: &[&str]) -> Result<Character, String> {
    let root = user_dir().ok_or("no per-user data folder")?;
    copy_builtin_into(&root, taken)
}

fn copy_builtin_into(root: &Path, taken: &[&str]) -> Result<Character, String> {
    let mut m = bundled_manifest();
    m.id = unique_id(root, BUILTIN, taken);
    m.name = "Yohta (copy)".into();
    let manifest = &m;
    stage_and_install(root, manifest, None, |stage| write_bundled(stage, manifest))
}

/// Bounding box of pixels at least `SOLID`, or `None` if empty.
fn visible_box(art: &RgbaImage) -> Option<(u32, u32, u32, u32)> {
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for (x, y, p) in art.enumerate_pixels() {
        if p[3] >= SOLID {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    (x0 <= x1).then(|| (x0, y0, x1 - x0 + 1, y1 - y0 + 1))
}

fn handle(pixels: RgbaImage) -> Handle {
    Handle::from_rgba(pixels.width(), pixels.height(), pixels.into_raw())
}

pub fn user_dir() -> Option<PathBuf> {
    crate::preferences::data_dir().map(|d| d.join("characters"))
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
        ground: super::rest::estimate(&art).unwrap_or((0.5, 1.0)),
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

pub(super) fn slug(name: &str) -> String {
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
            whole: None,
        }],
        source: Some(draft.source),
        editor: Vec::new(),
        rest: super::rest::RestMode::Auto,
        lines: Lines::default(),
        bop: None,
        reactions: super::reactions::new_character(),
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

/// What a picked character folder holds that import must ask about: its
/// name, and the images whose size isn't its canvas.
#[derive(Debug, Clone)]
pub struct Inspected {
    pub name: String,
    pub canvas: (u32, u32),
    pub misfits: Vec<Misfit>,
    /// The manifest's own `fit` default, if it gives one.
    pub suggested: Option<Fit>,
}

pub fn inspect_folder(source: &Path) -> Result<Inspected, String> {
    let m = read_manifest(source)?;
    let rules = fit_rules(source, m.canvas)?;
    // Images the manifest places itself need no question.
    let misfits = fit::misfits(&m, source)?
        .into_iter()
        .filter(|f| !rules.images.iter().any(|(a, _)| *a == f.asset))
        .collect();
    Ok(Inspected {
        name: m.name,
        canvas: m.canvas,
        misfits,
        suggested: rules.default,
    })
}

fn fit_rules(source: &Path, canvas: (u32, u32)) -> Result<fit::Rules, String> {
    let text = fs::read_to_string(source.join(MANIFEST)).map_err(|e| e.to_string())?;
    let v: Json = serde_json::from_str(&text).map_err(|e| format!("character.json: {e}"))?;
    fit::Rules::parse(&v["fit"], canvas)
}

/// Copies and validates a folder for import. Images that don't match the
/// canvas are baked onto it with `fit` (or the manifest's own rules).
pub fn prepare_folder(source: &Path, fit: Option<Fit>) -> Result<Arc<PreparedFolder>, String> {
    let root = user_dir().ok_or("no per-user data folder")?;
    prepare_folder_into(&root, source, fit)
}

fn prepare_folder_into(
    root: &Path,
    source: &Path,
    choice: Option<Fit>,
) -> Result<Arc<PreparedFolder>, String> {
    let m = read_manifest(source)?;
    let rules = fit_rules(source, m.canvas)?;
    let misfits = fit::misfits(&m, source)?;
    if !misfits.is_empty() {
        return prepare_fitted(root, source, m, &rules, choice, &misfits);
    }
    let prepared = new_prepared(root, &m)?;
    let assets: Vec<String> = m.files();
    // Validate containment before copying, and validate the actual copied bytes.
    m.validate_files(source)?;
    copy_assets(source, prepared.path(), &assets)?;
    fs::write(prepared.path.join(MANIFEST), m.to_json().to_string()).map_err(|e| e.to_string())?;
    m.validate_files(prepared.path())?;
    Ok(prepared)
}

/// A hidden folder for one prepared import, removed when dropped.
fn new_prepared(root: &Path, m: &Manifest) -> Result<Arc<PreparedFolder>, String> {
    let expected_revision = revision(&root.join(&m.id))?;
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let path = root.join(format!(
        ".prepared-{}-{}",
        std::process::id(),
        NEXT_STAGE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).map_err(|e| e.to_string())?;
    Ok(Arc::new(PreparedFolder {
        path,
        id: m.id.clone(),
        expected_revision,
    }))
}

/// Like the plain copy, but bakes each misfit onto the canvas first. Every
/// file is checked to stay in the folder; the result is validated whole.
fn prepare_fitted(
    root: &Path,
    source: &Path,
    m: Manifest,
    rules: &fit::Rules,
    choice: Option<Fit>,
    misfits: &[Misfit],
) -> Result<Arc<PreparedFolder>, String> {
    let prepared = new_prepared(root, &m)?;
    let mut encoded = 0u64;
    for asset in m.files() {
        let from = fit::contained(source, &asset)?;
        encoded += fs::metadata(&from).map_err(|e| e.to_string())?.len();
        if encoded > 128 * 1024 * 1024 {
            return Err("character files exceed the 128 MiB disk budget".into());
        }
        if misfits.iter().any(|f| f.asset == asset) {
            let fit = rules.for_asset(&asset, choice);
            fit::bake_asset(source, prepared.path(), &asset, fit, m.canvas)?;
        } else {
            let target = prepared.path().join(&asset);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(from, target).map_err(|e| e.to_string())?;
        }
    }
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
    let assets: Vec<String> = m.files();
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
    fs::remove_dir_all(installed_folder(c)?).map_err(|e| e.to_string())?;
    let _ = save_overrides(c.id, &Overrides::default());
    if let Some(root) = user_dir() {
        let _ = fs::remove_dir_all(user_sounds(&root, c.id));
    }
    Ok(())
}

// ---- Motion overrides --------------------------------------------------------

/// The user's Motion tab settings for a character, kept beside (not inside)
/// its installed pack so replacing or re-importing the pack keeps them.
fn motion_file(root: &Path, id: &str) -> PathBuf {
    root.join(".overrides").join(format!("{id}.json"))
}

/// What the user changed for a character, kept beside its pack.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Overrides {
    pub motion: Option<Vec<MotionSpec>>,
    pub lines: Option<Lines>,
    pub bop: Option<Bop>,
    /// The user's reaction layer; `user:` sounds live beside the overrides.
    pub reactions: Option<super::reactions::Config>,
}

pub fn load_overrides(id: &str) -> Overrides {
    user_dir()
        .map(|root| load_overrides_in(&root, id))
        .unwrap_or_default()
}

fn load_overrides_in(root: &Path, id: &str) -> Overrides {
    let path = motion_file(root, id);
    let Some(v) = fs::metadata(&path)
        .ok()
        .filter(|m| m.len() <= MAX_MANIFEST)
        .and_then(|_| fs::read_to_string(&path).ok())
        .and_then(|t| serde_json::from_str::<Json>(&t).ok())
        .filter(|v| v["schema_version"].as_u64() == Some(1))
    else {
        return Overrides::default();
    };
    let motion = v["motion"].as_array().and_then(|list| {
        list.iter()
            .map(|m| {
                Some(MotionSpec {
                    preset: Preset::parse(m["preset"].as_str()?)?,
                    strength: (m["strength"].as_f64()? as f32).clamp(0.0, 2.0),
                    speed: (m["speed"].as_f64()? as f32).clamp(0.0, 2.0),
                })
            })
            .collect()
    });
    // Broken saved lines fall back to the pack's rather than failing.
    let lines = Lines::present(&v)
        .then(|| Lines::parse(&v, true).ok())
        .flatten();
    let bop = v["bop"].as_str().and_then(Bop::parse);
    // Broken saved reactions fall back to the pack's rather than failing.
    let reactions = (!v["reactions"].is_null())
        .then(|| super::reactions::Config::parse(&v["reactions"], true).ok())
        .flatten();
    Overrides {
        motion,
        lines,
        bop,
        reactions,
    }
}

pub fn save_overrides(id: &str, o: &Overrides) -> Result<(), String> {
    save_overrides_in(&user_dir().ok_or("no per-user data folder")?, id, o)
}

fn save_overrides_in(root: &Path, id: &str, o: &Overrides) -> Result<(), String> {
    if !(valid_id(id) || id == BUILTIN) {
        return Err("unknown character".into());
    }
    let path = motion_file(root, id);
    if *o == Overrides::default() {
        return match fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        };
    }
    fs::create_dir_all(path.parent().expect("overrides folder")).map_err(|e| e.to_string())?;
    let mut json = json!({ "schema_version": 1 });
    if let Some(specs) = &o.motion {
        json["motion"] = specs
            .iter()
            .map(|s| json!({ "preset": s.preset.name(), "strength": s.strength, "speed": s.speed }))
            .collect();
    }
    if let Some(lines) = &o.lines {
        // Saved even when empty: "no lines" is a choice that overrides the pack.
        lines.write(&mut json, true);
    }
    if let Some(bop) = o.bop {
        json["bop"] = json!(bop.name());
    }
    if let Some(r) = &o.reactions {
        json["reactions"] = r.to_json();
    }
    // Write then rename, so a crash never leaves half a file.
    let partial = path.with_extension("json.partial");
    fs::write(
        &partial,
        serde_json::to_vec_pretty(&json).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(&partial, &path).map_err(|e| e.to_string())
}

/// Saved motion for `id`, if any. The rig checks it still fits its parts.
pub fn load_motion(id: &str) -> Option<Vec<MotionSpec>> {
    load_overrides(id).motion
}

/// Where the user's own line sounds for `id` are kept.
fn user_sounds(root: &Path, id: &str) -> PathBuf {
    root.join(".overrides").join(format!("{id}-sounds"))
}

/// Copies a WAV the user picked for a line into managed storage, so moving
/// or deleting the original doesn't silence it.
pub fn import_line_sound(id: &str, from: &Path) -> Result<SoundRef, String> {
    let root = user_dir().ok_or("no per-user data folder")?;
    if fs::metadata(from).map_err(|e| e.to_string())?.len() > super::lines::MAX_SOUND {
        return Err("line sounds must be 2 MiB or smaller".into());
    }
    crate::audio::decode_wav(from).map_err(|e| format!("not a readable WAV ({e})"))?;
    let dir = user_sounds(&root, id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let file = loop {
        let name = format!("line-{n}.wav");
        if !dir.join(&name).exists() {
            break name;
        }
        n += 1;
    };
    fs::copy(from, dir.join(&file)).map_err(|e| e.to_string())?;
    Ok(SoundRef::User(file))
}

/// Copies a WAV picked for a reaction into managed storage, after checking
/// it against the reaction limits, so moving the original changes nothing.
pub fn import_reaction_sound(id: &str, from: &Path) -> Result<SoundRef, String> {
    let root = user_dir().ok_or("no per-user data folder")?;
    crate::audio::check_reaction_wav(from)?;
    let dir = user_sounds(&root, id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let file = loop {
        let name = format!("reaction-{n}.wav");
        if !dir.join(&name).exists() {
            break name;
        }
        n += 1;
    };
    fs::copy(from, dir.join(&file)).map_err(|e| e.to_string())?;
    Ok(SoundRef::User(file))
}

/// Copies a PNG picked for a held pose or artwork reaction into managed
/// storage, fitted onto the character's stage: scaled to the stage height
/// at most, centred and standing on the stage's bottom, so it starts
/// roughly registered with the normal pose and can be aligned from there.
pub fn import_reaction_image(
    id: &str,
    from: &Path,
    canvas: (u32, u32),
) -> Result<super::reactions::Art, String> {
    let root = user_dir().ok_or("no per-user data folder")?;
    if fs::metadata(from).map_err(|e| e.to_string())?.len() > MAX_FILE {
        return Err("the PNG is larger than 64 MiB".into());
    }
    let bytes = fs::read(from).map_err(|e| e.to_string())?;
    check_source_dimensions(&bytes)?;
    let art = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .map_err(|e| format!("not a readable PNG ({e})"))?
        .to_rgba8();
    if trim(&art).is_none() { return Err("that PNG is empty".into()); }
    let dimensions = art.dimensions();
    let dir = user_sounds(&root, id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let file = loop {
        let name = format!("art-{n}.png");
        if !dir.join(&name).exists() {
            break name;
        }
        n += 1;
    };
    art
        .save_with_format(dir.join(&file), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    let mut result = super::reactions::Art::new(SoundRef::User(file));
    result.source_size = Some([dimensions.0, dimensions.1]);
    let fit = (canvas.0 as f32 / dimensions.0 as f32).min(canvas.1 as f32 / dimensions.1 as f32);
    result.place = [(1.0 - dimensions.0 as f32 * fit / canvas.0 as f32) / 2.0,
        1.0 - dimensions.1 as f32 * fit / canvas.1 as f32, 1.0];
    Ok(result)
}

/// Deletes the user's sounds that neither the saved lines nor the saved
/// reactions use any more.
pub fn prune_line_sounds(id: &str, keep: &Lines, reactions: Option<&super::reactions::Config>) {
    let Some(root) = user_dir() else { return };
    let used: Vec<&str> = keep
        .sounds()
        .into_iter()
        .chain(reactions.map(|r| r.files()).unwrap_or_default())
        .chain(reactions.map(|r| r.images()).unwrap_or_default())
        .filter_map(|s| match s {
            SoundRef::User(f) => Some(f.as_str()),
            SoundRef::Pack(_) => None,
        })
        .collect();
    if let Ok(entries) = fs::read_dir(user_sounds(&root, id)) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !used.contains(&name.as_str()) {
                let _ = fs::remove_file(e.path());
            }
        }
    }
}

/// Writes the character as a folder inside `into`, ready to share or
/// re-import. Returns the folder written.
/// Writes the character as a folder inside `into`, ready to share or
/// re-import, including the user's Motion and Lines changes and their line
/// sounds, so it behaves the same on another PC. Returns the folder written.
pub fn export(c: &Character, into: &Path) -> Result<PathBuf, String> {
    let root = user_dir();
    export_with(c, into, &load_overrides(c.id), |file| {
        root.as_ref().map(|r| user_sounds(r, c.id).join(file))
    })
}

fn export_with(
    c: &Character,
    into: &Path,
    overrides: &Overrides,
    user_sound: impl Fn(&str) -> Option<PathBuf>,
) -> Result<PathBuf, String> {
    let mut m = match &c.folder {
        Some(folder) => {
            let m = read_manifest(folder)?;
            m.validate_files(folder)?;
            m
        }
        None => bundled_manifest(),
    };
    // The pack's own files, before overrides change which sounds it names.
    let pack_files = m.files();
    let mut user_files: Vec<(PathBuf, String)> = Vec::new();
    bake(&mut m, overrides, &mut user_files, &user_sound)?;
    let target = free_folder(into, &m.id);
    fs::create_dir_all(&target).map_err(|e| e.to_string())?;
    let result = (|| {
        match &c.folder {
            Some(folder) => copy_assets(folder, &target, &pack_files)?,
            None => write_bundled(&target, &m)?,
        }
        for (from, to) in &user_files {
            let to = target.join(to);
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(from, to).map_err(|e| e.to_string())?;
        }
        // Drop pack sounds the exported lines no longer use.
        let used = m
            .sounds()
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        for f in &pack_files {
            if super::lines::valid_pack_sound(f) && !used.contains(f) {
                let _ = fs::remove_file(target.join(f));
            }
        }
        let text = serde_json::to_string_pretty(&m.to_json()).map_err(|e| e.to_string())?;
        fs::write(target.join(MANIFEST), text).map_err(|e| e.to_string())?;
        // What was written must load like any pack.
        read_manifest(&target)?.validate_files(&target)
    })();
    match result {
        Ok(()) => Ok(target),
        Err(e) => {
            let _ = fs::remove_dir_all(&target);
            Err(e)
        }
    }
}

/// Folds the user's overrides into a manifest: motion into each part (or
/// the single image's sway), and their lines in place of the pack's, with
/// their own sounds renamed into the pack's `lines/` folder.
fn bake(
    m: &mut Manifest,
    o: &Overrides,
    user_files: &mut Vec<(PathBuf, String)>,
    user_sound: &impl Fn(&str) -> Option<PathBuf>,
) -> Result<(), String> {
    let default = m.default_pose.clone();
    let pose = m
        .poses
        .iter_mut()
        .find(|p| p.name == default)
        .expect("validated default pose");
    if let Some(specs) = &o.motion {
        if !pose.parts.is_empty()
            && specs.len() == pose.parts.len()
            && pose
                .parts
                .iter()
                .zip(specs)
                .all(|(p, s)| p.kind.presets().contains(&s.preset))
        {
            for (p, s) in pose.parts.iter_mut().zip(specs) {
                p.motion = *s;
            }
        } else if pose.parts.is_empty()
            && let [s] = specs.as_slice()
            && PartType::Head.presets().contains(&s.preset)
        {
            pose.whole = Some(*s);
        }
    }
    if let Some(lines) = &o.lines {
        let mut lines = lines.clone();
        for t in super::lines::Trigger::ALL {
            for line in lines.get_mut(t) {
                if let Some(SoundRef::User(file)) = &line.sound {
                    let from = user_sound(file).ok_or("no per-user data folder")?;
                    let to = format!("lines/{file}");
                    if !user_files.iter().any(|(_, t)| *t == to) {
                        user_files.push((from, to.clone()));
                    }
                    line.sound = Some(SoundRef::Pack(to));
                }
            }
        }
        m.lines = lines;
    }
    if let Some(bop) = o.bop {
        m.bop = Some(bop);
    }
    if let Some(user) = &o.reactions {
        let mut merged = m.reactions.clone();
        if user.companionship.is_some() {
            merged.companionship = user.companionship.clone();
        }
        if user.settings.is_some() {
            merged.settings = user.settings;
        }
        for e in super::reactions::Event::ALL {
            if let Some(entry) = user.entry(e) {
                merged.set_entry(e, Some(entry.clone()));
            }
        }
        let mut failed = None;
        merged.map_files(|r| match r {
            SoundRef::User(file) => {
                let to = format!("sounds/{file}");
                match user_sound(file) {
                    Some(from) => {
                        if !user_files.iter().any(|(_, t)| *t == to) {
                            user_files.push((from, to.clone()));
                        }
                    }
                    None => failed = Some("no per-user data folder"),
                }
                SoundRef::Pack(to)
            }
            pack => pack.clone(),
        });
        merged.map_images(|r| match r {
            SoundRef::User(file) => {
                let to = format!("reactions/{file}");
                match user_sound(file) {
                    Some(from) => {
                        if !user_files.iter().any(|(_, t)| *t == to) {
                            user_files.push((from, to.clone()));
                        }
                    }
                    None => failed = Some("no per-user data folder"),
                }
                SoundRef::Pack(to)
            }
            pack => pack.clone(),
        });
        if let Some(e) = failed {
            return Err(e.into());
        }
        m.reactions = merged;
    }
    Ok(())
}

/// `into/<id>`, or `into/<id>-2` and so on if that exists.
fn free_folder(into: &Path, id: &str) -> PathBuf {
    let mut target = into.join(id);
    let mut n = 2;
    while target.exists() {
        target = into.join(format!("{id}-{n}"));
        n += 1;
    }
    target
}
