//! What the Settings window shows, as plain data that can cross to its thread.

use std::collections::HashMap;

use crate::registry::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum Kind {
    Toggle = 0,
    Range = 1,
    Choice = 2,
    Action = 3,
    Note = 4,
    /// A command and its key combination, with a button to change it.
    Shortcut = 5,
    /// A choice shown as a few side-by-side buttons.
    Segments = 6,
    /// A choice among colours.
    Swatches = 7,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// The setting's ID, or the action's.
    pub id: String,
    pub kind: Kind,
    pub label: String,
    pub hint: String,
    pub on: bool,
    pub value: f32,
    pub min: f32,
    pub max: f32,
    /// A range's value as shown, a button's caption, or a note's text.
    pub text: String,
    pub options: Vec<String>,
    pub index: i32,
    /// A swatch row's colours, as RGBA.
    pub colors: Vec<[u8; 4]>,
    pub enabled: bool,
    pub warning: bool,
}

impl Item {
    pub fn new(id: &str, kind: Kind, label: &str) -> Self {
        Self {
            id: id.to_owned(),
            kind,
            label: label.to_owned(),
            hint: String::new(),
            on: false,
            value: 0.0,
            min: 0.0,
            max: 1.0,
            text: String::new(),
            options: Vec::new(),
            index: 0,
            colors: Vec::new(),
            enabled: true,
            warning: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub title: String,
    pub items: Vec<Item>,
}

/// How to turn what the window reports back into a setting value.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Extra {
    /// A range snaps to multiples of this, when set.
    pub step: Option<f64>,
    /// A choice's values, in the order its options are listed.
    pub choices: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// Navigation entries: group and name.
    pub nav: Vec<(String, String)>,
    pub current: usize,
    pub title: String,
    pub summary: String,
    /// Why a change was refused, shown above the page.
    pub notice: String,
    pub sections: Vec<Section>,
    pub extras: HashMap<String, Extra>,
}
