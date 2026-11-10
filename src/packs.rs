//! Pointer packs: a folder with a versioned JSON manifest and a transparent
//! PNG. Built-in packs and user packs go through the same loader; a pack is
//! validated completely before it can be selected.

use std::fs;
use std::path::{Path, PathBuf};

use iced::advanced::image::Handle;
use serde_json::{Value as Json, json};

/// Built-in packs, embedded so the release needs no asset folder.
const BUNDLED: &[(&str, &[u8])] = &[
    (
        include_str!("../assets/packs/yohta/manifest.json"),
        include_bytes!("../assets/packs/yohta/paw.png"),
    ),
    (
        include_str!("../assets/packs/moro/manifest.json"),
        include_bytes!("../assets/packs/moro/paw.png"),
    ),
    (
        include_str!("../assets/packs/moro-crayon/manifest.json"),
        include_bytes!("../assets/packs/moro-crayon/paw.png"),
    ),
    (
        include_str!("../assets/packs/zaeri/manifest.json"),
        include_bytes!("../assets/packs/zaeri/paw.png"),
    ),
];

pub const DEFAULT: &str = "yohta";

pub const POINTER_PROMPT: &str = "Create a desktop pointer using my attached character reference. Show the back of one right paw or claw. Preserve the reference's digit count, colours, markings and proportions. Raise the digit next to the thumb straight upward and curl the remaining digits inward. Make the pointing tip the highest part of the silhouette. Centre the wrist at the bottom with a clean, flat end suitable for joining a separate forearm. Keep the source artwork upright. The app applies rotation. Fit the entire paw inside a transparent portrait PNG with a small margin. Use a clear outline and a silhouette readable at cursor size. Omit the forearm, stick, arrows, lettering, cast shadow and glow.";

pub const FOREARM_PROMPT: &str = "Create a separate forearm asset that matches my attached character reference and pointer paw artwork. Preserve the character's colours, fur or scale texture, outline style and wrist width. Show only a straight upright forearm strip in a transparent portrait PNG. Place the narrow wrist joint at top centre and extend the limb downward, widening gently toward the bottom. Give both ends clean, flat cuts. Keep the middle simple so it can stretch and curve smoothly. Keep identifying wrist tufts and detailed distal markings on the separate paw artwork. Omit the paw, fingers, claws, shoulder, body, stick, arrows, lettering, cast shadow and glow.";
const MAX_SIDE: u32 = 4096;
/// Imported art is scaled so its height fits this, like the bundled sprites.
const SPRITE_HEIGHT: u32 = 256;
const ALPHA_SOLID: u8 = 128;

#[derive(Debug, Clone)]
pub struct Pack {
    /// Interned for the lifetime of the process, so settings can refer to it.
    pub id: &'static str,
    pub name: String,
    pub image: Handle,
    pub width: u32,
    pub height: u32,
    /// Pointing tip as fractions of width and height from the top-left.
    pub hotspot: (f32, f32),
    pub wrist: Option<(f32, f32)>,
    pub accent: Option<[u8; 4]>,
    pub forearm: Option<Forearm>,
    /// Forearm image width relative to paw image width.
    pub joint_width: f32,
    pub pressed: Option<Handle>,
    pub tap: Option<crate::audio::Clip>,
    pub folder: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct Forearm {
    pub image: Handle,
    pub width: u32,
    pub height: u32,
    pub rgba: std::sync::Arc<Vec<u8>>,
    pub anchor: (f32, f32),
    pub strips: Vec<Handle>,
}

impl Forearm {
    pub fn set_anchor(&mut self, anchor: (f32, f32)) {
        self.anchor = (anchor.0.clamp(0.0, 1.0), anchor.1.clamp(0.0, 0.8));
        let image =
            image::RgbaImage::from_raw(self.width, self.height, self.rgba.to_vec()).unwrap();
        let top = (self.anchor.1 * self.height as f32) as u32;
        let count = (self.height - top).min(32);
        self.strips = (0..count)
            .map(|i| {
                let y = top + i * (self.height - top) / count;
                let end = top + (i + 1) * (self.height - top) / count;
                let strip = image::imageops::crop_imm(&image, 0, y, self.width, end - y).to_image();
                Handle::from_rgba(self.width, end - y, strip.into_raw())
            })
            .collect();
    }
}

pub fn forearm_from_file(path: &Path) -> Result<Forearm, String> {
    let draft = draft_from_file(path)?;
    let mut arm = Forearm {
        image: draft.image,
        width: draft.width,
        height: draft.height,
        rgba: draft.rgba,
        anchor: (0.5, 0.0),
        strips: Vec::new(),
    };
    arm.set_anchor(guess_hotspot(arm.width, arm.height, &arm.rgba));
    Ok(arm)
}

/// Decoded pixels kept while a new pointer is being set up.
#[derive(Debug, Clone)]
pub struct Draft {
    pub name: String,
    pub image: Handle,
    pub width: u32,
    pub height: u32,
    pub rgba: std::sync::Arc<Vec<u8>>,
    pub hotspot: (f32, f32),
    pub wrist: (f32, f32),
    pub joint_width: f32,
    pub forearm: Option<Forearm>,
    pub pressed: Option<(Handle, std::sync::Arc<Vec<u8>>)>,
    pub tap: Option<(crate::audio::Clip, PathBuf)>,
}

impl Draft {
    pub fn preview(&self) -> Pack {
        Pack {
            id: "preview",
            name: self.name.clone(),
            image: self.image.clone(),
            width: self.width,
            height: self.height,
            hotspot: self.hotspot,
            wrist: Some(self.wrist),
            accent: None,
            forearm: self.forearm.clone(),
            joint_width: self.joint_width,
            pressed: self.pressed.as_ref().map(|(h, _)| h.clone()),
            tap: self.tap.as_ref().map(|(c, _)| c.clone()),
            folder: None,
        }
    }
}

pub fn user_dir() -> Option<PathBuf> {
    crate::preferences::data_dir().map(|d| d.join("packs"))
}

fn parse_color(text: &str) -> Option<[u8; 4]> {
    let hex = text.trim_start_matches('#');
    let n = u32::from_str_radix(hex, 16).ok()?;
    match hex.len() {
        6 => Some([(n >> 16) as u8, (n >> 8) as u8, n as u8, 0xFF]),
        8 => Some(n.to_be_bytes()),
        _ => None,
    }
}

fn fraction_pair(v: &Json) -> Option<(f32, f32)> {
    let a = v.as_array()?;
    let (x, y) = (a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32);
    ((0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y)).then_some((x, y))
}

fn valid_id(id: &str) -> bool {
    (1..=40).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Decodes a PNG and checks it can serve as a pointer.
fn decode(png: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .map_err(|e| format!("not a readable PNG ({e})"))?
        .to_rgba8();
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
        return Err(format!(
            "the image must be at most {MAX_SIDE}×{MAX_SIDE} pixels"
        ));
    }
    let alpha = img.pixels().map(|p| p.0[3]);
    let (mut clear, mut solid) = (false, false);
    for a in alpha {
        clear |= a < 255;
        solid |= a >= ALPHA_SOLID;
    }
    if !clear {
        return Err("the PNG needs a transparent background".into());
    }
    if !solid {
        return Err("the PNG is empty".into());
    }
    Ok((w, h, img.into_raw()))
}

/// Parses a manifest and its PNG into a validated pack.
pub fn parse(manifest: &str, png: &[u8]) -> Result<Pack, String> {
    parse_assets(manifest, png, None)
}

fn parse_assets(manifest: &str, png: &[u8], arm_png: Option<&[u8]>) -> Result<Pack, String> {
    let m: Json = serde_json::from_str(manifest).map_err(|e| format!("manifest: {e}"))?;
    if m["schemaVersion"].as_u64() != Some(1) {
        return Err("unsupported manifest schemaVersion".into());
    }
    let id = m["id"]
        .as_str()
        .filter(|id| valid_id(id))
        .ok_or("invalid pack id")?;
    let name = m["name"]
        .as_str()
        .map(str::trim)
        .filter(|n| !n.is_empty() && n.chars().count() <= 40)
        .ok_or("the pack needs a name of up to 40 characters")?;
    let hotspot = fraction_pair(&m["pointer"]["hotspot"]).ok_or("invalid hotspot")?;
    let wrist = match &m["pointer"]["wrist"] {
        Json::Null => None,
        v => Some(fraction_pair(v).ok_or("invalid wrist anchor")?),
    };
    if wrist.is_some_and(|w| (w.0 - hotspot.0).abs() < 1e-3 && (w.1 - hotspot.1).abs() < 1e-3) {
        return Err("hotspot and wrist must differ".into());
    }
    let accent = m["accent"].as_str().and_then(parse_color);
    let (width, height, rgba) = decode(png)?;
    let joint_width = match &m["pointer"]["jointWidth"] {
        Json::Null => 0.4,
        value => value.as_f64().ok_or("invalid joint width")? as f32,
    };
    if !(0.05..=1.5).contains(&joint_width) {
        return Err("joint width must be between 0.05 and 1.5".into());
    }
    let forearm = if !m["pointer"]["forearm"].is_null() {
        if wrist.is_none() {
            return Err("a forearm needs a paw wrist anchor".into());
        }
        let (w, h, rgba) = decode(arm_png.ok_or("missing forearm image")?)?;
        let anchor = match &m["pointer"]["forearmAnchor"] {
            Json::Null => (0.5, 0.0),
            value => fraction_pair(value).ok_or("invalid forearm anchor")?,
        };
        if anchor.1 > 0.8 {
            return Err("forearm anchor must be in the upper 80%".into());
        }
        let mut arm = Forearm {
            image: Handle::from_rgba(w, h, rgba.clone()),
            width: w,
            height: h,
            rgba: std::sync::Arc::new(rgba),
            anchor,
            strips: Vec::new(),
        };
        arm.set_anchor(anchor);
        Some(arm)
    } else {
        None
    };
    Ok(Pack {
        id: Box::leak(id.to_owned().into_boxed_str()),
        name: name.to_owned(),
        image: Handle::from_rgba(width, height, rgba),
        width,
        height,
        hotspot,
        wrist,
        accent,
        forearm,
        joint_width,
        pressed: None,
        tap: None,
        folder: None,
    })
}

/// Bundled packs, then user packs. A broken user pack is reported and skipped.
pub fn load_all() -> (Vec<Pack>, Vec<String>) {
    let mut packs = Vec::new();
    let mut problems = Vec::new();
    for (manifest, png) in BUNDLED {
        match parse(manifest, png) {
            Ok(p) => packs.push(p),
            Err(e) => problems.push(format!("built-in pack: {e}")),
        }
    }
    if let Some(dir) = user_dir()
        && let Ok(entries) = fs::read_dir(&dir)
    {
        for entry in entries.flatten() {
            let folder = entry.path();
            match load_folder(&folder) {
                Ok(p) if packs.iter().any(|q| q.id == p.id) => {
                    problems.push(format!(
                        "{}: duplicate pack id `{}`",
                        folder.display(),
                        p.id
                    ));
                }
                Ok(p) => packs.push(p),
                Err(e) => problems.push(format!("{}: {e}", folder.display())),
            }
        }
    }
    (packs, problems)
}

fn load_folder(folder: &Path) -> Result<Pack, String> {
    let manifest = fs::read_to_string(folder.join("manifest.json")).map_err(|e| e.to_string())?;
    let m: Json = serde_json::from_str(&manifest).map_err(|e| format!("manifest: {e}"))?;
    let idle = m["pointer"]["idle"]
        .as_str()
        .ok_or("manifest has no idle image")?;
    // Resolve inside the pack folder only.
    if idle.is_empty() || idle.contains(['/', '\\', ':']) || idle.contains("..") {
        return Err("the idle image must sit in the pack folder".into());
    }
    let png = fs::read(folder.join(idle)).map_err(|e| e.to_string())?;
    let arm_png = match &m["pointer"]["forearm"] {
        Json::Null => None,
        value => {
            let file = value.as_str().ok_or("invalid forearm filename")?;
            if file.is_empty() || file.contains(['/', '\\', ':']) || file.contains("..") {
                return Err("the forearm image must sit in the pack folder".into());
            }
            Some(fs::read(folder.join(file)).map_err(|e| e.to_string())?)
        }
    };
    let mut pack = parse_assets(&manifest, &png, arm_png.as_deref())?;
    if let Some(file) = asset_name(&m["pointer"]["pressed"])? {
        let (w, h, rgba) = decode(&fs::read(folder.join(file)).map_err(|e| e.to_string())?)?;
        if (w, h) != (pack.width, pack.height) {
            return Err("pressed pose must use the idle canvas dimensions".into());
        }
        pack.pressed = Some(Handle::from_rgba(w, h, rgba));
    }
    if let Some(file) = asset_name(&m["sounds"]["tap"])? {
        pack.tap = Some(crate::audio::decode_wav(&folder.join(file))?);
    }
    pack.folder = Some(folder.to_path_buf());
    Ok(pack)
}

fn asset_name(value: &Json) -> Result<Option<&str>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let file = value.as_str().ok_or("invalid asset filename")?;
    if file.is_empty() || file.contains(['/', '\\', ':']) || file.contains("..") {
        return Err("assets must sit in the pack folder".into());
    }
    Ok(Some(file))
}

/// Copy only validated manifest assets, never arbitrary directory contents.
pub fn import_folder(source: &Path, taken: &[&str]) -> Result<Pack, String> {
    import_folder_into(&user_dir().ok_or("no per-user data folder")?, source, taken)
}

fn import_folder_into(root: &Path, source: &Path, taken: &[&str]) -> Result<Pack, String> {
    let pack = load_folder(source)?;
    if taken.contains(&pack.id) {
        return Err("a pointer with this id is already installed".into());
    }
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let target = root.join(pack.id);
    fs::create_dir(&target).map_err(|e| e.to_string())?;
    let result = (|| {
        let manifest = fs::read(source.join("manifest.json")).map_err(|e| e.to_string())?;
        let m: Json = serde_json::from_slice(&manifest).map_err(|e| e.to_string())?;
        for v in [
            &m["pointer"]["idle"],
            &m["pointer"]["pressed"],
            &m["pointer"]["forearm"],
            &m["sounds"]["tap"],
        ] {
            if let Some(file) = asset_name(v)? {
                fs::copy(source.join(file), target.join(file)).map_err(|e| e.to_string())?;
            }
        }
        fs::write(target.join("manifest.json"), manifest).map_err(|e| e.to_string())?;
        load_folder(&target)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&target);
    }
    result
}

fn installed_folder(pack: &Pack) -> Result<PathBuf, String> {
    let root = user_dir()
        .ok_or("no per-user data folder")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let folder = pack
        .folder
        .as_ref()
        .ok_or("bundled pointers cannot be removed")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if folder.parent() != Some(root.as_path())
        || folder.file_name().and_then(|n| n.to_str()) != Some(pack.id)
    {
        return Err("pointer is outside installed packs".into());
    }
    Ok(folder)
}

pub fn remove(pack: &Pack) -> Result<(), String> {
    fs::remove_dir_all(installed_folder(pack)?).map_err(|e| e.to_string())
}

pub fn edit_draft(pack: &Pack) -> Result<Draft, String> {
    let folder = installed_folder(pack)?;
    let m: Json = serde_json::from_str(
        &fs::read_to_string(folder.join("manifest.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let file = asset_name(&m["pointer"]["idle"])?.ok_or("missing idle")?;
    let mut d = draft_from_file(&folder.join(file))?;
    d.name = pack.name.clone();
    d.hotspot = pack.hotspot;
    d.wrist = pack.wrist.unwrap_or((0.5, 1.0));
    d.forearm = pack.forearm.clone();
    d.joint_width = pack.joint_width;
    Ok(d)
}

pub fn reaim(pack: &Pack, draft: &Draft) -> Result<Pack, String> {
    let folder = installed_folder(pack)?;
    let path = folder.join("manifest.json");
    let mut m: Json = serde_json::from_str(&fs::read_to_string(&path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    m["pointer"]["hotspot"] = json!([draft.hotspot.0, draft.hotspot.1]);
    m["pointer"]["wrist"] = json!([draft.wrist.0, draft.wrist.1]);
    m["pointer"]["jointWidth"] = json!(draft.joint_width);
    let png = fs::read(folder.join(asset_name(&m["pointer"]["idle"])?.ok_or("missing idle")?))
        .map_err(|e| e.to_string())?;
    let arm = asset_name(&m["pointer"]["forearm"])?
        .map(|n| fs::read(folder.join(n)).map_err(|e| e.to_string()))
        .transpose()?;
    let text = serde_json::to_string_pretty(&m).map_err(|e| e.to_string())?;
    parse_assets(&text, &png, arm.as_deref())?;
    let tmp = folder.join("manifest.tmp");
    fs::write(&tmp, text).map_err(|e| e.to_string())?;
    fs::rename(tmp, path).map_err(|e| e.to_string())?;
    load_folder(&folder)
}

/// Topmost solid pixel, as a fraction: the pointing tip of upright art.
fn guess_hotspot(w: u32, h: u32, rgba: &[u8]) -> (f32, f32) {
    for y in 0..h {
        let row = &rgba[(y * w * 4) as usize..((y + 1) * w * 4) as usize];
        let xs: Vec<u32> = (0..w)
            .filter(|x| row[(*x * 4 + 3) as usize] >= ALPHA_SOLID)
            .collect();
        if let (Some(first), Some(last)) = (xs.first(), xs.last()) {
            return (
                ((first + last) as f32 / 2.0 + 0.5) / w as f32,
                (y as f32 + 0.5) / h as f32,
            );
        }
    }
    (0.5, 0.0)
}

/// Starts "Add your pointer" from a PNG: validates it, scales it to sprite
/// size and guesses the hotspot. Nothing is written until [`install`].
pub fn draft_from_file(path: &Path) -> Result<Draft, String> {
    let png = fs::read(path).map_err(|e| e.to_string())?;
    let (w, h, rgba) = decode(&png)?;
    let img = image::RgbaImage::from_raw(w, h, rgba).ok_or("bad image buffer")?;
    let img = if h > SPRITE_HEIGHT {
        let nw = ((w as f32 * SPRITE_HEIGHT as f32 / h as f32).round() as u32).max(1);
        image::imageops::resize(
            &img,
            nw,
            SPRITE_HEIGHT,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        img
    };
    let (w, h) = img.dimensions();
    let rgba = img.into_raw();
    let hotspot = guess_hotspot(w, h, &rgba);
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("My pointer")
        .chars()
        .take(40)
        .collect();
    Ok(Draft {
        name,
        image: Handle::from_rgba(w, h, rgba.clone()),
        width: w,
        height: h,
        rgba: std::sync::Arc::new(rgba),
        hotspot,
        wrist: (0.5, 1.0),
        joint_width: 0.4,
        forearm: None,
        pressed: None,
        tap: None,
    })
}

pub fn set_pressed(draft: &mut Draft, path: &Path) -> Result<(), String> {
    let (w, h, rgba) = decode(&fs::read(path).map_err(|e| e.to_string())?)?;
    if (w as f32 / h as f32 - draft.width as f32 / draft.height as f32).abs() > 0.01 {
        return Err("pressed image must use the same canvas shape as idle".into());
    }
    let image = image::RgbaImage::from_raw(w, h, rgba).ok_or("invalid image")?;
    let image = image::imageops::resize(
        &image,
        draft.width,
        draft.height,
        image::imageops::FilterType::Lanczos3,
    );
    let mut png = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut png, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    draft.pressed = Some((
        Handle::from_rgba(draft.width, draft.height, image.into_raw()),
        std::sync::Arc::new(png.into_inner()),
    ));
    Ok(())
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
    if s.is_empty() { "pointer".into() } else { s }
}

/// Writes a validated pack to app storage, so it keeps working if the source
/// file moves. Returns the installed pack, loaded back through the loader.
pub fn install(draft: &Draft, taken: &[&str]) -> Result<Pack, String> {
    install_into(&user_dir().ok_or("no per-user data folder")?, draft, taken)
}

fn install_into(root: &Path, draft: &Draft, taken: &[&str]) -> Result<Pack, String> {
    let name = draft.name.trim();
    if name.is_empty() {
        return Err("give the pointer a name".into());
    }
    let base = slug(name);
    let mut id = base.clone();
    let mut n = 2;
    while root.join(&id).exists()
        || taken.contains(&id.as_str())
        || BUNDLED
            .iter()
            .any(|(m, _)| m.contains(&format!("\"{id}\"")))
    {
        id = format!("{base}-{n}");
        n += 1;
    }
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let dir = root.join(&id);
    fs::create_dir(&dir).map_err(|e| e.to_string())?;
    let result = (|| {
        let img = image::RgbaImage::from_raw(draft.width, draft.height, draft.rgba.to_vec())
            .ok_or("bad image buffer")?;
        img.save_with_format(dir.join("paw.png"), image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        let (hx, hy) = draft.hotspot;
        let mut manifest = json!({
            "schemaVersion": 1,
            "id": id,
            "name": name,
            "pointer": {
                "idle": "paw.png",
                "hotspot": [hx, hy],
                "wrist": [draft.wrist.0, draft.wrist.1],
                "jointWidth": draft.joint_width,
            },
        });
        if let Some((_, png)) = &draft.pressed {
            fs::write(dir.join("pressed.png"), png.as_slice()).map_err(|e| e.to_string())?;
            manifest["pointer"]["pressed"] = json!("pressed.png");
        }
        if let Some((_, path)) = &draft.tap {
            crate::audio::decode_wav(path)?;
            fs::copy(path, dir.join("tap.wav")).map_err(|e| e.to_string())?;
            manifest["sounds"]["tap"] = json!("tap.wav");
        }
        if let Some(arm) = &draft.forearm {
            let image = image::RgbaImage::from_raw(arm.width, arm.height, arm.rgba.to_vec())
                .ok_or("bad forearm buffer")?;
            image
                .save_with_format(dir.join("forearm.png"), image::ImageFormat::Png)
                .map_err(|e| e.to_string())?;
            manifest["pointer"]["forearm"] = json!("forearm.png");
            manifest["pointer"]["forearmAnchor"] = json!([arm.anchor.0, arm.anchor.1]);
        }
        let text = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
        fs::write(dir.join("manifest.json"), text).map_err(|e| e.to_string())?;
        load_folder(&dir)
    })();
    result.inspect_err(|_| {
        let _ = fs::remove_dir_all(&dir);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_packs_load_with_tips_at_the_top() {
        for (manifest, png) in BUNDLED {
            let p = parse(manifest, png).unwrap();
            assert!(p.hotspot.1 < 0.1, "{} tip is near the top", p.id);
            assert!(p.wrist.is_some());
            assert_eq!(p.height, 256);
        }
    }

    #[test]
    fn rejects_bad_manifests() {
        let png = BUNDLED[0].1;
        let bad = [
            r#"{"schemaVersion":2,"id":"a","name":"A","pointer":{"hotspot":[0,0]}}"#,
            r#"{"schemaVersion":1,"id":"Bad Id","name":"A","pointer":{"hotspot":[0,0]}}"#,
            r#"{"schemaVersion":1,"id":"a","name":"","pointer":{"hotspot":[0,0]}}"#,
            r#"{"schemaVersion":1,"id":"a","name":"A","pointer":{"hotspot":[1.5,0]}}"#,
            r#"{"schemaVersion":1,"id":"a","name":"A","pointer":{"hotspot":[0.5,0.5],"wrist":[0.5,0.5]}}"#,
        ];
        for m in bad {
            assert!(parse(m, png).is_err(), "{m}");
        }
        let ok = r#"{"schemaVersion":1,"id":"a","name":"A","pointer":{"hotspot":[0.5,0]}}"#;
        assert!(parse(ok, png).is_ok());
        assert!(parse(ok, b"not a png").is_err());
    }

    #[test]
    fn hotspot_guess_finds_the_topmost_pixel() {
        let (w, h) = (4u32, 4u32);
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        // One solid pixel at (2, 1).
        rgba[((w + 2) * 4 + 3) as usize] = 255;
        let (x, y) = guess_hotspot(w, h, &rgba);
        assert!((x - 2.5 / 4.0).abs() < 1e-6 && (y - 1.5 / 4.0).abs() < 1e-6);
    }

    #[test]
    fn add_your_pointer_round_trip() {
        // Import a large PNG, move the hotspot, install, load it back.
        let src = std::env::temp_dir().join(format!("tinypaw-test-{}", std::process::id()));
        std::fs::create_dir_all(&src).unwrap();
        let png_path = src.join("Big Paw.png");
        let mut img = image::RgbaImage::new(300, 600);
        for y in 100..600 {
            for x in 100..200 {
                img.put_pixel(x, y, image::Rgba([200, 80, 160, 255]));
            }
        }
        img.save(&png_path).unwrap();

        let mut draft = draft_from_file(&png_path).unwrap();
        assert_eq!(draft.height, SPRITE_HEIGHT, "scaled to sprite size");
        assert_eq!(draft.name, "Big Paw");
        assert!(
            draft.hotspot.1 > 0.15 && draft.hotspot.1 < 0.2,
            "tip guessed at the top of the shape"
        );
        draft.hotspot = (0.5, 0.2);

        let root = src.join("packs");
        let pack = install_into(&root, &draft, &["big-paw"]).unwrap();
        assert_eq!(pack.id, "big-paw-2", "taken IDs are skipped");
        assert_eq!(pack.hotspot, (0.5, 0.2));
        // The installed copy no longer depends on the source file.
        std::fs::remove_file(&png_path).unwrap();
        let again = load_folder(&root.join("big-paw-2")).unwrap();
        assert_eq!(again.name, "Big Paw");
        assert!(
            again.forearm.is_none(),
            "legacy single-image packs remain supported"
        );
        let arm_path = src.join("Arm.png");
        img.save(&arm_path).unwrap();
        draft.forearm = Some(forearm_from_file(&arm_path).unwrap());
        draft.forearm.as_mut().unwrap().set_anchor((0.4, 0.2));
        draft.wrist = (0.6, 0.9);
        draft.joint_width = 0.7;
        let pair = install_into(&root, &draft, &[]).unwrap();
        assert_ne!(pair.id, again.id, "existing folders cannot be overwritten");
        fs::remove_file(&arm_path).unwrap();
        let reloaded = load_folder(&root.join(pair.id)).unwrap();
        assert_eq!(reloaded.wrist, Some((0.6, 0.9)));
        assert_eq!(reloaded.joint_width, 0.7);
        let arm = reloaded.forearm.unwrap();
        assert_eq!(arm.anchor, (0.4, 0.2));
        assert!(!arm.strips.is_empty());
        assert_eq!(arm.rgba, draft.forearm.as_ref().unwrap().rgba);
        let manifest_path = root.join(pair.id).join("manifest.json");
        let mut manifest: Json =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest["pointer"]["forearm"] = json!("../Arm.png");
        fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert!(
            load_folder(&root.join(pair.id))
                .unwrap_err()
                .contains("pack folder")
        );
        manifest["pointer"]["forearm"] = json!("forearm.png");
        fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        draft.hotspot = draft.wrist;
        assert!(install_into(&root, &draft, &[]).is_err());
        assert_eq!(
            fs::read_dir(&root).unwrap().count(),
            2,
            "failed installs leave no partial pack"
        );
        std::fs::remove_dir_all(&src).unwrap();
    }

    #[test]
    fn opaque_png_is_rejected() {
        let mut img = image::RgbaImage::new(4, 4);
        img.pixels_mut()
            .for_each(|p| *p = image::Rgba([1, 2, 3, 255]));
        let mut png = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        assert!(decode(&png).unwrap_err().contains("transparent"));
    }

    #[test]
    fn slugs_are_valid_ids() {
        assert_eq!(slug("My Cool Paw!"), "my-cool-paw");
        assert_eq!(slug("你好"), "pointer");
        assert!(valid_id(&slug("A  b--c")));
    }

    #[test]
    fn pressed_pose_and_pack_sound_survive_folder_import() {
        let root = std::env::temp_dir().join(format!("tinypaw-extras-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let png = root.join("paw.png");
        fs::write(&png, BUNDLED[0].1).unwrap();
        let mut draft = draft_from_file(&png).unwrap();
        set_pressed(&mut draft, &png).unwrap();
        // A short mono PCM WAV, decoded without opening an audio device.
        let wav = root.join("tap.wav");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&44u32.to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&44100u32.to_le_bytes());
        bytes.extend_from_slice(&88200u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&8u32.to_le_bytes());
        bytes.extend_from_slice(&[0, 0, 1, 0, 0, 0, 255, 255]);
        fs::write(&wav, bytes).unwrap();
        draft.tap = Some((crate::audio::decode_wav(&wav).unwrap(), wav.clone()));
        let source_root = root.join("source");
        let pack = install_into(&source_root, &draft, &[]).unwrap();
        assert!(pack.pressed.is_some());
        assert!(pack.tap.is_some());
        let destination = root.join("installed");
        let imported = import_folder_into(&destination, &source_root.join(pack.id), &[]).unwrap();
        fs::remove_dir_all(&source_root).unwrap();
        fs::remove_file(&png).unwrap();
        fs::remove_file(&wav).unwrap();
        let reloaded = load_folder(&destination.join(imported.id)).unwrap();
        assert!(reloaded.pressed.is_some());
        assert!(reloaded.tap.is_some());
        assert!(
            import_folder_into(&destination, &destination.join(imported.id), &[imported.id])
                .is_err()
        );
        let path = destination.join(imported.id).join("manifest.json");
        let mut m: Json = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        m["sounds"]["tap"] = json!("../private.wav");
        fs::write(&path, serde_json::to_vec(&m).unwrap()).unwrap();
        assert!(
            load_folder(&destination.join(imported.id))
                .unwrap_err()
                .contains("pack folder")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
