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
        return Err(format!("the image must be at most {MAX_SIDE}×{MAX_SIDE} pixels"));
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
    let m: Json = serde_json::from_str(manifest).map_err(|e| format!("manifest: {e}"))?;
    if m["schemaVersion"].as_u64() != Some(1) {
        return Err("unsupported manifest schemaVersion".into());
    }
    let id = m["id"].as_str().filter(|id| valid_id(id)).ok_or("invalid pack id")?;
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
    Ok(Pack {
        id: Box::leak(id.to_owned().into_boxed_str()),
        name: name.to_owned(),
        image: Handle::from_rgba(width, height, rgba),
        width,
        height,
        hotspot,
        wrist,
        accent,
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
                    problems.push(format!("{}: duplicate pack id `{}`", folder.display(), p.id));
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
    let idle = m["pointer"]["idle"].as_str().ok_or("manifest has no idle image")?;
    // Resolve inside the pack folder only.
    if idle.contains(['/', '\\']) || idle.contains("..") {
        return Err("the idle image must sit in the pack folder".into());
    }
    let png = fs::read(folder.join(idle)).map_err(|e| e.to_string())?;
    parse(&manifest, &png)
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
        image::imageops::resize(&img, nw, SPRITE_HEIGHT, image::imageops::FilterType::Lanczos3)
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
    })
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
    while taken.contains(&id.as_str()) || BUNDLED.iter().any(|(m, _)| m.contains(&format!("\"{id}\""))) {
        id = format!("{base}-{n}");
        n += 1;
    }
    let dir = root.join(&id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let img = image::RgbaImage::from_raw(draft.width, draft.height, draft.rgba.to_vec())
        .ok_or("bad image buffer")?;
    img.save_with_format(dir.join("paw.png"), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    let (hx, hy) = draft.hotspot;
    let manifest = json!({
        "schemaVersion": 1,
        "id": id,
        "name": name,
        "pointer": {
            "idle": "paw.png",
            "hotspot": [hx, hy],
            "wrist": [0.5, 1.0],
        },
    });
    let text = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    fs::write(dir.join("manifest.json"), text).map_err(|e| e.to_string())?;
    load_folder(&dir).inspect_err(|_| {
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
        assert!(draft.hotspot.1 > 0.15 && draft.hotspot.1 < 0.2, "tip guessed at the top of the shape");
        draft.hotspot = (0.5, 0.2);

        let root = src.join("packs");
        let pack = install_into(&root, &draft, &["big-paw"]).unwrap();
        assert_eq!(pack.id, "big-paw-2", "taken IDs are skipped");
        assert_eq!(pack.hotspot, (0.5, 0.2));
        // The installed copy no longer depends on the source file.
        std::fs::remove_file(&png_path).unwrap();
        let again = load_folder(&root.join("big-paw-2")).unwrap();
        assert_eq!(again.name, "Big Paw");
        std::fs::remove_dir_all(&src).unwrap();
    }

    #[test]
    fn opaque_png_is_rejected() {
        let mut img = image::RgbaImage::new(4, 4);
        img.pixels_mut().for_each(|p| *p = image::Rgba([1, 2, 3, 255]));
        let mut png = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        assert!(decode(&png).unwrap_err().contains("transparent"));
    }

    #[test]
    fn slugs_are_valid_ids() {
        assert_eq!(slug("My Cool Paw!"), "my-cool-paw");
        assert_eq!(slug("你好"), "pointer");
        assert!(valid_id(&slug("A  b--c")));
    }
}
