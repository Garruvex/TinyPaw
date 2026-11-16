//! Characters on the desktop: one entry per instance, so the same character
//! can appear more than once, each with its own place, size and flags. They
//! live in `instances.json` beside the preferences.
//!
//! The first run of this version builds the list from the old single
//! character settings, so an existing installation opens unchanged. After
//! that the list is the source of truth; `mascot.enabled` stays the master
//! switch, and `mascot.character` is only the character picked in the library.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Value as Json, json};

use crate::features::mascot as feature;
use crate::registry::SettingValues;

/// One character on the desktop.
#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    /// Stable for the life of the instance; never reused.
    pub id: u32,
    pub character: String,
    /// Its own shown flag. It is visible when this and the master switch are on.
    pub shown: bool,
    /// Where its feet rest, in physical desktop pixels.
    pub x: f64,
    pub y: f64,
    /// Visible resting height in logical pixels.
    pub height: f64,
    pub eyes: bool,
    pub idle: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Instances {
    list: Vec<Instance>,
    next: u32,
}

fn path() -> Option<PathBuf> {
    crate::preferences::data_dir().map(|d| d.join("instances.json"))
}

impl Instances {
    /// The saved list, or one built from the old single-character settings
    /// (and saved) when there is none yet.
    pub fn load(settings: &SettingValues) -> Self {
        let Some(path) = path() else {
            return Self::from_settings(settings);
        };
        Self::load_from(&path, settings)
    }

    fn load_from(path: &Path, settings: &SettingValues) -> Self {
        if let Ok(text) = fs::read_to_string(path) {
            if let Some(found) = Self::parse(&text) {
                return found;
            }
            // Unreadable: keep it for the user before starting over.
            let _ = fs::rename(path, path.with_extension("json.bad"));
        }
        let migrated = Self::from_settings(settings);
        if let Err(e) = migrated.save_to(path) {
            eprintln!("tinypaw: could not save {}: {e}", path.display());
        }
        migrated
    }

    /// The old single character as instance 1, shown.
    pub fn from_settings(s: &SettingValues) -> Self {
        Self {
            list: vec![Instance {
                id: 1,
                character: s.choice_text(feature::CHARACTER).to_owned(),
                shown: true,
                x: s.float(feature::X),
                y: s.float(feature::Y),
                height: s.float(feature::HEIGHT),
                eyes: s.bool(feature::EYES),
                idle: s.bool(feature::IDLE),
            }],
            next: 2,
        }
    }

    fn parse(text: &str) -> Option<Self> {
        let json: Json = serde_json::from_str(text).ok()?;
        let mut list: Vec<Instance> = Vec::new();
        for item in json.get("instances")?.as_array()? {
            // An entry that cannot be read is skipped, not fatal to the rest.
            let (Some(id), Some(character)) = (
                item.get("id").and_then(Json::as_u64),
                item.get("character").and_then(Json::as_str),
            ) else {
                continue;
            };
            let id = id as u32;
            if list.iter().any(|i| i.id == id) {
                continue;
            }
            list.push(Instance {
                id,
                character: character.to_owned(),
                shown: item.get("shown").and_then(Json::as_bool).unwrap_or(true),
                x: item
                    .get("x")
                    .and_then(Json::as_f64)
                    .unwrap_or(feature::UNPLACED),
                y: item
                    .get("y")
                    .and_then(Json::as_f64)
                    .unwrap_or(feature::UNPLACED),
                height: item
                    .get("height")
                    .and_then(Json::as_f64)
                    .unwrap_or(feature::DEFAULT_HEIGHT)
                    .clamp(feature::MIN_HEIGHT, feature::MAX_HEIGHT),
                eyes: item.get("eyes").and_then(Json::as_bool).unwrap_or(true),
                idle: item.get("idle").and_then(Json::as_bool).unwrap_or(true),
            });
        }
        let next = list.iter().map(|i| i.id).max().unwrap_or(0) + 1;
        Some(Self { list, next })
    }

    fn to_json(&self) -> Json {
        json!({
            "instances": self.list.iter().map(|i| json!({
                "id": i.id,
                "character": i.character,
                "shown": i.shown,
                "x": i.x,
                "y": i.y,
                "height": i.height,
                "eyes": i.eyes,
                "idle": i.idle,
            })).collect::<Vec<_>>()
        })
    }

    /// Writes the list beside the preferences, replacing the file atomically.
    pub fn save(&self) -> io::Result<()> {
        match path() {
            Some(path) => self.save_to(&path),
            None => Ok(()),
        }
    }

    fn save_to(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let temp = path.with_extension("json.tmp");
        fs::write(&temp, serde_json::to_string_pretty(&self.to_json())?)?;
        fs::rename(&temp, path)
    }

    #[cfg(test)]
    fn add_next(&self) -> u32 {
        self.next
    }

    pub fn list(&self) -> &[Instance] {
        &self.list
    }

    #[cfg(test)]
    pub fn get(&self, id: u32) -> Option<&Instance> {
        self.list.iter().find(|i| i.id == id)
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut Instance> {
        self.list.iter_mut().find(|i| i.id == id)
    }

    /// Another instance of `character`, unplaced, at the size, eyes and idle
    /// defaults. Returns its ID.
    pub fn add(&mut self, character: &str, defaults: &SettingValues) -> u32 {
        let id = self.next;
        self.next += 1;
        self.list.push(Instance {
            id,
            character: character.to_owned(),
            shown: true,
            x: feature::UNPLACED,
            y: feature::UNPLACED,
            height: defaults.float(feature::HEIGHT),
            eyes: true,
            idle: true,
        });
        id
    }

    pub fn remove(&mut self, id: u32) -> bool {
        let before = self.list.len();
        self.list.retain(|i| i.id != id);
        self.list.len() != before
    }

    /// Where an instance came to rest.
    pub fn place(&mut self, id: u32, x: f64, y: f64) {
        if let Some(i) = self.get_mut(id) {
            (i.x, i.y) = (x, y);
        }
    }

    /// Every instance back to the default spot.
    pub fn unplace_all(&mut self) {
        for i in &mut self.list {
            (i.x, i.y) = (feature::UNPLACED, feature::UNPLACED);
        }
    }

    /// Points every instance of `from` at `to`, e.g. after a character is
    /// removed or replaced.
    pub fn retarget(&mut self, from: &str, to: &str) -> bool {
        let mut changed = false;
        for i in self.list.iter_mut().filter(|i| i.character == from) {
            i.character = to.to_owned();
            changed = true;
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{Registry, Value};

    fn defaults() -> (Registry, SettingValues) {
        let registry = Registry::build(crate::features::BUILTINS).expect("built-ins");
        let values = registry.defaults();
        (registry, values)
    }

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("tinypaw-instances-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join("instances.json")
    }

    #[test]
    fn an_existing_installation_becomes_one_instance_with_its_old_settings() {
        let (registry, mut values) = defaults();
        registry
            .set(&mut values, feature::CHARACTER, Value::Choice("moro"))
            .unwrap();
        registry
            .set(&mut values, feature::X, Value::Float(640.0))
            .unwrap();
        registry
            .set(&mut values, feature::Y, Value::Float(900.0))
            .unwrap();
        registry
            .set(&mut values, feature::HEIGHT, Value::Float(200.0))
            .unwrap();
        registry
            .set(&mut values, feature::EYES, Value::Bool(false))
            .unwrap();
        let path = temp("migrate");
        let found = Instances::load_from(&path, &values);
        assert_eq!(found.list().len(), 1);
        let first = &found.list()[0];
        assert_eq!(
            (
                first.id,
                first.character.as_str(),
                first.x,
                first.y,
                first.height,
                first.eyes
            ),
            (1, "moro", 640.0, 900.0, 200.0, false)
        );
        assert!(first.shown && first.idle);
        // The migration is saved, so a second start reads the file and the
        // settings no longer matter.
        let (_, other) = defaults();
        assert_eq!(Instances::load_from(&path, &other), found);
    }

    #[test]
    fn instances_of_one_character_are_separate_and_ids_are_never_reused() {
        let (_, values) = defaults();
        let mut all = Instances::from_settings(&values);
        let second = all.add("yohta", &values);
        let third = all.add("yohta", &values);
        assert_eq!((second, third), (2, 3));
        all.place(second, 10.0, 20.0);
        all.get_mut(third).unwrap().height = 120.0;
        assert_eq!(all.get(1).unwrap().x, feature::UNPLACED);
        assert_eq!(all.get(second).unwrap().x, 10.0);
        assert_eq!(all.get(third).unwrap().height, 120.0);
        assert!(all.remove(second));
        assert!(!all.remove(second));
        assert_eq!(all.add("yohta", &values), 4);
    }

    #[test]
    fn the_list_round_trips_and_bad_entries_are_ignored() {
        let (_, values) = defaults();
        let mut all = Instances::from_settings(&values);
        all.add("moro", &values);
        all.get_mut(2).unwrap().shown = false;
        let path = temp("round-trip");
        all.save_to(&path).unwrap();
        assert_eq!(
            Instances::parse(&fs::read_to_string(&path).unwrap()),
            Some(all.clone())
        );
        // A duplicate ID and an entry without a character are skipped.
        let text = r#"{"instances":[{"id":1,"character":"a"},{"id":1,"character":"b"},{"id":5}]}"#;
        let found = Instances::parse(text).unwrap();
        assert_eq!(found.list().len(), 1);
        assert_eq!(found.list()[0].character, "a");
        assert_eq!(found.add_next(), 2);
        assert!(Instances::parse("not json").is_none());
    }

    #[test]
    fn retargeting_and_unplacing_touch_only_what_they_name() {
        let (_, values) = defaults();
        let mut all = Instances::from_settings(&values);
        all.add("moro", &values);
        all.place(1, 5.0, 6.0);
        assert!(all.retarget("moro", "yohta"));
        assert!(all.list().iter().all(|i| i.character == "yohta"));
        all.unplace_all();
        assert!(all.list().iter().all(|i| i.x == feature::UNPLACED));
    }
}
