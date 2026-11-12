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
    save_to(&path, values)
}

fn save_to(path: &std::path::Path, values: &SettingValues) -> io::Result<()> {
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
            Value::DynamicChoice(c) => Json::String(c.clone()),
        };
        map.insert(id.to_owned(), json);
    }
    let text = serde_json::to_string_pretty(&Json::Object(map)).map_err(io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)
}

enum WriteRequest {
    Save(SettingValues),
    Flush(std::sync::mpsc::Sender<Result<(), String>>),
}

/// One ordered writer, shared by every setting surface. A failed snapshot
/// remains pending and retries without requiring the user to move a slider.
pub struct Writer {
    tx: std::sync::mpsc::Sender<WriteRequest>,
}

impl Writer {
    pub fn new() -> Self {
        Self::with_save(save)
    }

    fn with_save(write: impl Fn(&SettingValues) -> io::Result<()> + Send + 'static) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            use std::sync::mpsc::RecvTimeoutError;
            use std::time::{Duration, Instant};
            let mut pending = None;
            let mut deadline = Instant::now();
            loop {
                let request = if pending.is_some() {
                    rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
                } else {
                    rx.recv().map_err(|_| RecvTimeoutError::Disconnected)
                };
                match request {
                    Ok(WriteRequest::Save(values)) => {
                        pending = Some(values);
                        deadline = Instant::now() + Duration::from_millis(250);
                    }
                    Ok(WriteRequest::Flush(reply)) => {
                        let result = pending
                            .as_ref()
                            .map_or(Ok(()), &write)
                            .map_err(|e| e.to_string());
                        if result.is_ok() {
                            pending = None;
                        }
                        let _ = reply.send(result);
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        let result = write(pending.as_ref().unwrap()).map_err(|e| e.to_string());
                        if result.is_ok() {
                            pending = None;
                        } else {
                            deadline = Instant::now() + Duration::from_secs(2);
                        }
                        crate::platform::send(crate::platform::External::PreferencesSaved(result));
                    }
                    Err(RecvTimeoutError::Disconnected) => {
                        if let Some(values) = pending {
                            let _ = write(&values);
                        }
                        break;
                    }
                }
            }
        });
        Self { tx }
    }

    pub fn schedule(&self, values: &SettingValues) {
        let _ = self.tx.send(WriteRequest::Save(values.clone()));
    }

    pub fn flush(&self) -> Result<(), String> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.tx
            .send(WriteRequest::Flush(tx))
            .map_err(|e| e.to_string())?;
        rx.recv().map_err(|e| e.to_string())?
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    #[test]
    fn rapid_edits_flush_only_the_latest_snapshot() {
        let saved = Arc::new(Mutex::new(Vec::new()));
        let output = saved.clone();
        let writer = Writer::with_save(move |v| {
            output
                .lock()
                .unwrap()
                .push(v.float(crate::features::sound::VOLUME));
            Ok(())
        });
        let registry = Registry::build(crate::features::BUILTINS).unwrap();
        let mut values = registry.defaults();
        for i in 0..100 {
            registry
                .set(
                    &mut values,
                    crate::features::sound::VOLUME,
                    Value::Float(i as f64 / 100.0),
                )
                .unwrap();
            writer.schedule(&values);
        }
        writer.flush().unwrap();
        assert_eq!(*saved.lock().unwrap(), vec![0.99]);
    }

    #[test]
    fn failed_flush_retains_the_latest_snapshot_for_retry() {
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let writer = Writer::with_save(move |_| {
            if count.fetch_add(1, Ordering::SeqCst) == 0 {
                Err(io::Error::other("disk unavailable"))
            } else {
                Ok(())
            }
        });
        writer.schedule(&SettingValues::default());
        assert!(writer.flush().is_err());
        writer.flush().unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}
