//! Saved user choices: one JSON object keyed by stable setting ID, validated
//! against the registry on load and written atomically.

use std::fs;
use std::io;
use std::path::PathBuf;

use serde_json::{Map, Value as Json};

use crate::registry::{Registry, SettingValues, Value};

/// Per-user data folder, e.g. `%APPDATA%\TinyPaw`.
pub fn data_dir() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("TinyPaw"))
}

fn file() -> Option<PathBuf> {
    data_dir().map(|d| d.join("preferences.json"))
}

pub struct Loaded {
    pub values: SettingValues,
    /// No preferences file existed: show the first-run card.
    pub first_run: bool,
}

/// Loads saved values over the registry defaults. Unknown IDs and values that
/// no longer validate are ignored, so the defaults apply instead.
pub fn load(registry: &Registry) -> Loaded {
    let mut values = registry.defaults();
    let Some(path) = file() else {
        return Loaded {
            values,
            first_run: true,
        };
    };
    let Ok(text) = fs::read_to_string(&path) else {
        return Loaded {
            values,
            first_run: true,
        };
    };
    if let Ok(Json::Object(map)) = serde_json::from_str::<Json>(&text) {
        for (id, json) in map {
            if id == crate::features::upcoming::SOURCE {
                continue;
            }
            let text = match json {
                Json::Bool(b) => b.to_string(),
                Json::Number(n) => n.to_string(),
                Json::String(s) => s,
                _ => continue,
            };
            if let Ok(value) = registry.parse_value(&id, &text) {
                let _ = registry.set(&mut values, &id, value);
            }
        }
    }
    Loaded {
        values,
        first_run: false,
    }
}

pub fn save(values: &SettingValues) -> io::Result<()> {
    let path = file().ok_or_else(|| io::Error::other("no per-user data folder"))?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut map = Map::new();
    for (id, value) in values.iter() {
        // Native window handles are valid only for this process session.
        if id == crate::features::upcoming::SOURCE {
            continue;
        }
        let json = match value {
            Value::Bool(b) => Json::Bool(*b),
            Value::Float(f) => serde_json::Number::from_f64(*f).map_or(Json::Null, Json::Number),
            Value::Color([r, g, b, a]) => Json::String(format!("#{r:02x}{g:02x}{b:02x}{a:02x}")),
            Value::Choice(c) => Json::String((*c).to_owned()),
        };
        map.insert(id.to_owned(), json);
    }
    let text = serde_json::to_string_pretty(&Json::Object(map)).map_err(io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, &path)
}

pub fn load_shortcuts(registry: &mut Registry) {
    let Some(path) = data_dir().map(|d| d.join("shortcuts.json")) else {
        return;
    };
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    let Ok(map) = serde_json::from_str::<std::collections::BTreeMap<String, String>>(&text) else {
        return;
    };
    // Validate the complete set against defaults before exposing it.
    let _ = registry.restore_bindings(&map);
}

pub fn save_shortcuts(registry: &Registry) -> io::Result<()> {
    let path = data_dir()
        .ok_or_else(|| io::Error::other("no per-user data folder"))?
        .join("shortcuts.json");
    let map: std::collections::BTreeMap<_, _> = [
        crate::registry::ShortcutContext::Global,
        crate::registry::ShortcutContext::Overlay,
    ]
    .into_iter()
    .flat_map(|c| registry.shortcuts(c))
    .map(|(s, d)| (d.id, s.label()))
    .collect();
    fs::create_dir_all(path.parent().unwrap())?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(&map)?)?;
    fs::rename(tmp, path)
}
