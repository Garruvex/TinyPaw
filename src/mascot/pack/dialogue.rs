//! Dialogue files: a character's speech lines, bubble and menu placement on
//! their own, to share them or to set up many lines at once.
//!
//! A dialogue file uses the same `lines`, `bubble` and `menu` keys as
//! `character.json`, so a character's manifest is also a dialogue file.
//! Sounds are WAVs named relative to the file:
//!
//! ```json
//! { "schema_version": 1,
//!   "lines": { "click": ["Hi!", { "text": "That tickles", "sound": "lines/giggle.wav" }] },
//!   "bubble": { "anchor": [0.5, 0.15], "side": "above" },
//!   "menu": { "side": "auto", "align": "feet" } }
//! ```

use super::*;

/// The file name Export writes.
pub const DIALOGUE: &str = "lines.json";
/// Room for 200 lines of 280 characters, even outside ASCII.
const MAX_DIALOGUE: u64 = 256 * 1024;

/// A read dialogue file: its lines, with sounds still beside the file.
#[derive(Debug, Clone)]
pub struct Dialogue {
    pub lines: Lines,
    folder: PathBuf,
    /// Sounds the file names that are missing or unreadable; their lines
    /// come in without a sound.
    pub missing_sounds: usize,
}

pub fn read_dialogue(path: &Path) -> Result<Dialogue, String> {
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > MAX_DIALOGUE {
        return Err("the dialogue file is larger than 256 KiB".into());
    }
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let v: Json = serde_json::from_str(&text).map_err(|e| format!("not readable JSON ({e})"))?;
    if !v["schema_version"].is_null() && v["schema_version"].as_u64() != Some(1) {
        return Err("the dialogue file must use schema_version 1".into());
    }
    if !Lines::present(&v) {
        return Err("this file has no lines, bubble or menu".into());
    }
    let mut lines = Lines::parse(&v, false)?;
    let folder = path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let mut missing_sounds = 0;
    lines.map_sounds(|s| match s {
        SoundRef::Pack(p) if check_sound(&folder, p).is_ok() => Some(s.clone()),
        _ => {
            missing_sounds += 1;
            None
        }
    });
    Ok(Dialogue {
        lines,
        folder,
        missing_sounds,
    })
}

impl Dialogue {
    /// The lines with their sounds copied into `id`'s managed storage, so
    /// moving the file later changes nothing, and how many couldn't be.
    pub fn import_sounds(&self, id: &str) -> (Lines, usize) {
        self.import_sounds_with(|from| import_line_sound(id, from))
    }

    fn import_sounds_with(
        &self,
        mut copy: impl FnMut(&Path) -> Result<SoundRef, String>,
    ) -> (Lines, usize) {
        let mut lines = self.lines.clone();
        let mut failed = 0;
        let mut copied: Vec<(String, SoundRef)> = Vec::new();
        lines.map_sounds(|s| {
            let SoundRef::Pack(p) = s else {
                return None;
            };
            if let Some((_, r)) = copied.iter().find(|(from, _)| from == p) {
                return Some(r.clone());
            }
            match copy(&self.folder.join(p)) {
                Ok(r) => {
                    copied.push((p.clone(), r.clone()));
                    Some(r)
                }
                Err(_) => {
                    failed += 1;
                    None
                }
            }
        });
        (lines, failed)
    }
}

/// Writes `lines` as `<id>-lines/lines.json` inside `into`, with their
/// sounds in its `lines/` folder. Returns the folder written.
pub fn export_dialogue(c: &Character, lines: &Lines, into: &Path) -> Result<PathBuf, String> {
    export_dialogue_with(c.id, lines, into, |s| c.sound_path(s))
}

fn export_dialogue_with(
    id: &str,
    lines: &Lines,
    into: &Path,
    path_of: impl Fn(&SoundRef) -> Option<PathBuf>,
) -> Result<PathBuf, String> {
    let target = free_folder(into, &format!("{id}-lines"));
    fs::create_dir_all(&target).map_err(|e| e.to_string())?;
    let result = (|| {
        let mut out = lines.clone();
        let mut written: Vec<(SoundRef, String)> = Vec::new();
        let mut failed = None;
        out.map_sounds(|s| {
            if let Some((_, name)) = written.iter().find(|(r, _)| r == s) {
                return Some(SoundRef::Pack(name.clone()));
            }
            let from = path_of(s)?;
            let base = from.file_name()?.to_string_lossy().into_owned();
            let mut name = format!("lines/{base}");
            let mut n = 2;
            while written.iter().any(|(_, w)| *w == name) {
                name = format!("lines/{n}-{base}");
                n += 1;
            }
            let to = target.join(&name);
            if let Err(e) =
                fs::create_dir_all(target.join("lines")).and_then(|_| fs::copy(&from, &to))
            {
                failed = Some(format!("{}: {e}", from.display()));
                return None;
            }
            written.push((s.clone(), name.clone()));
            Some(SoundRef::Pack(name))
        });
        if let Some(e) = failed {
            return Err(e);
        }
        let mut v = json!({ "schema_version": 1 });
        out.write(&mut v, true);
        let text = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
        fs::write(target.join(DIALOGUE), text).map_err(|e| e.to_string())?;
        // What was written must read back the same.
        let check = read_dialogue(&target.join(DIALOGUE))?;
        if check.missing_sounds > 0 || check.lines != out {
            return Err("the written dialogue file doesn't read back the same".into());
        }
        Ok(())
    })();
    match result {
        Ok(()) => Ok(target),
        Err(e) => {
            let _ = fs::remove_dir_all(&target);
            Err(e)
        }
    }
}

/// Writes a starter dialogue file into `into`. Returns the file written.
pub fn write_template(into: &Path) -> Result<PathBuf, String> {
    let mut path = into.join("lines-template.json");
    let mut n = 2;
    while path.exists() {
        path = into.join(format!("lines-template-{n}.json"));
        n += 1;
    }
    let text = serde_json::to_string_pretty(&super::super::lines::template())
        .map_err(|e| e.to_string())?;
    fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mascot::lines::Trigger;

    /// A tenth of a second of 16-bit mono PCM.
    fn wav(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let samples: Vec<u8> = (0..800i16)
            .flat_map(|i| ((i % 40 - 20) * 500).to_le_bytes())
            .collect();
        let mut b = Vec::new();
        b.extend(b"RIFF");
        b.extend((36 + samples.len() as u32).to_le_bytes());
        b.extend(b"WAVEfmt ");
        b.extend(16u32.to_le_bytes());
        b.extend(1u16.to_le_bytes());
        b.extend(1u16.to_le_bytes());
        b.extend(8000u32.to_le_bytes());
        b.extend(16000u32.to_le_bytes());
        b.extend(2u16.to_le_bytes());
        b.extend(16u16.to_le_bytes());
        b.extend(b"data");
        b.extend((samples.len() as u32).to_le_bytes());
        b.extend(samples);
        fs::write(path, b).unwrap();
    }

    #[test]
    fn dialogue_round_trips_with_sounds() {
        let dir = std::env::temp_dir().join(format!("tinypaw-dialogue-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let src = dir.join("src");
        wav(&src.join("lines/hi.wav"));
        fs::write(
            src.join("mine.json"),
            r#"{ "lines": { "click": ["Hi!", { "text": "Hello", "sound": "lines/hi.wav" }],
                            "drop": [{ "text": "Oof", "sound": "lines/missing.wav" }] },
                 "bubble": { "side": "left" } }"#,
        )
        .unwrap();
        let d = read_dialogue(&src.join("mine.json")).unwrap();
        assert_eq!(d.lines.get(Trigger::Click).len(), 2);
        assert_eq!(d.missing_sounds, 1, "a missing sound drops, the line stays");
        assert!(d.lines.get(Trigger::Drop)[0].sound.is_none());

        // Import copies each sound once, into managed storage.
        let store = dir.join("store");
        let mut copies = 0;
        let (lines, failed) = d.import_sounds_with(|from| {
            copies += 1;
            fs::create_dir_all(&store).unwrap();
            fs::copy(from, store.join("line-1.wav")).map_err(|e| e.to_string())?;
            Ok(SoundRef::User("line-1.wav".into()))
        });
        assert_eq!((failed, copies), (0, 1));
        assert_eq!(
            lines.get(Trigger::Click)[1].sound,
            Some(SoundRef::User("line-1.wav".into()))
        );

        // Export writes the user's sounds beside the file, and reads back.
        let out = dir.join("out");
        let folder = export_dialogue_with("fixture", &lines, &out, |s| match s {
            SoundRef::User(f) => Some(store.join(f)),
            SoundRef::Pack(p) => Some(src.join(p)),
        })
        .unwrap();
        let back = read_dialogue(&folder.join(DIALOGUE)).unwrap();
        assert_eq!(back.missing_sounds, 0);
        assert_eq!(
            back.lines.get(Trigger::Click)[1].sound,
            Some(SoundRef::Pack("lines/line-1.wav".into()))
        );
        assert_eq!(
            back.lines.bubble.side,
            crate::mascot::lines::BubbleSide::Left
        );

        let template = write_template(&dir).unwrap();
        assert_eq!(
            read_dialogue(&template).unwrap().lines.total(),
            Trigger::ALL.len()
        );
        for bad in [
            r#"{ "schema_version": 2, "lines": {} }"#,
            r#"{ "name": "x" }"#,
            "nope",
        ] {
            fs::write(dir.join("bad.json"), bad).unwrap();
            assert!(read_dialogue(&dir.join("bad.json")).is_err(), "{bad}");
        }
        let _ = fs::remove_dir_all(&dir);
    }
}
