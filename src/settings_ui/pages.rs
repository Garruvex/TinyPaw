//! The Settings pages, defined once: each page is sections of items that name
//! registered settings, with the wording and choice labels the registry does
//! not hold. The window renders whatever this produces, so adding a setting
//! to a page is one line here.

use std::collections::HashMap;

use super::DisplayInfo;
use super::link::{CharacterInfo, InstanceInfo, MusicInfo, PackInfo, ShortcutInfo, WindowInfo};
use super::model::{Extra, Item, Kind, Section, Thumb};
use crate::features::{display, mascot, point, sound, spotlight, system, text, upcoming};
use crate::registry::{Registry, SettingValues, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageId {
    General,
    Pointer,
    Ink,
    Tools,
    Characters,
    Music,
    Shortcuts,
    Sound,
}

/// The pointer cards, one of them chosen. Not a setting of its own: choosing a
/// card sets the style and the pack together.
pub const POINTER: &str = "point.pointer";

/// The picker's cards: choosing one puts that character on the desktop.
pub const ADD: &str = "instance.add";
/// Opens the picker, and leaves it without adding.
pub const PICK: &str = "ui:pick";
pub const PICK_CANCEL: &str = "ui:cancel";
/// Shows or hides a page's fine tuning.
pub const ADVANCED: &str = "ui:advanced";

/// Navigation order: group, name, page. Each page holds one scope (see
/// docs/settings-ux.md): the app, an annotation tool, or the characters on
/// the desktop. The first page has no group.
pub const NAV: &[(&str, &str, PageId)] = &[
    ("", "General", PageId::General),
    ("Annotate", "Pointer", PageId::Pointer),
    ("Annotate", "Ink & text", PageId::Ink),
    ("Annotate", "Spotlight & lens", PageId::Tools),
    ("Characters", "On the desktop", PageId::Characters),
    ("Characters", "Music & dance", PageId::Music),
    ("System", "Shortcuts", PageId::Shortcuts),
    ("System", "Sound", PageId::Sound),
];

/// The page a name on the command line means, or the first.
pub fn index_of(name: &str) -> usize {
    let id = match name {
        "pointer" => PageId::Pointer,
        "ink" => PageId::Ink,
        "tools" | "spotlight" | "lens" => PageId::Tools,
        "characters" | "character" => PageId::Characters,
        "music" => PageId::Music,
        "shortcuts" => PageId::Shortcuts,
        "sound" => PageId::Sound,
        _ => PageId::General,
    };
    NAV.iter().position(|n| n.2 == id).unwrap_or(0)
}

/// What a page needs to know about the app beyond the settings.
pub struct Context<'a> {
    pub registry: &'a Registry,
    pub settings: &'a SettingValues,
    pub displays: &'a [DisplayInfo],
    pub shortcuts: &'a [ShortcutInfo],
    pub windows: &'a [WindowInfo],
    pub packs: &'a [PackInfo],
    pub characters: &'a [CharacterInfo],
    pub instances: &'a [InstanceInfo],
    pub music: &'a MusicInfo,
    pub has_custom_sound: bool,
    /// The character on the desktop whose row is open.
    pub expanded: Option<u32>,
    /// The page shows the picker for adding a character.
    pub picking: bool,
    /// Fine tuning is unfolded.
    pub advanced: bool,
}

pub struct Built {
    pub title: &'static str,
    pub summary: String,
    pub sections: Vec<Section>,
    pub extras: HashMap<String, Extra>,
}

struct Builder<'a> {
    cx: &'a Context<'a>,
    sections: Vec<Section>,
    extras: HashMap<String, Extra>,
}

impl<'a> Builder<'a> {
    fn new(cx: &'a Context<'a>) -> Self {
        Self {
            cx,
            sections: Vec::new(),
            extras: HashMap::new(),
        }
    }

    fn section(&mut self, title: &str, items: impl FnOnce(&mut Section, &mut Self)) {
        let mut section = Section {
            title: title.to_owned(),
            tag: String::new(),
            items: Vec::new(),
        };
        items(&mut section, self);
        self.sections.push(section);
    }

    fn label(&self, id: &str) -> &'static str {
        self.cx.registry.setting(id).map_or("", |d| d.label)
    }

    fn toggle(&self, s: &mut Section, id: &'static str, label: Option<&str>) {
        let mut item = Item::new(id, Kind::Toggle, label.unwrap_or_else(|| self.label(id)));
        item.on = self.cx.settings.bool(id);
        s.items.push(item);
    }

    fn range(
        &mut self,
        s: &mut Section,
        id: &'static str,
        label: Option<&str>,
        step: f64,
        show: fn(f64) -> String,
    ) {
        let range = self.cx.registry.range(id);
        let value = self.cx.settings.float(id);
        let mut item = Item::new(id, Kind::Range, label.unwrap_or_else(|| self.label(id)));
        item.value = value as f32;
        item.min = *range.start() as f32;
        item.max = *range.end() as f32;
        item.text = show(value);
        s.items.push(item);
        self.extras.insert(
            id.to_owned(),
            Extra {
                step: Some(step),
                choices: Vec::new(),
            },
        );
    }

    /// A choice among `options` (value and label); the current value is
    /// whichever matches the setting, or none.
    fn choice(
        &mut self,
        s: &mut Section,
        id: &'static str,
        label: Option<&str>,
        options: Vec<(&'static str, String)>,
    ) {
        let current = self.cx.settings.choice_text(id);
        let mut item = Item::new(id, Kind::Choice, label.unwrap_or_else(|| self.label(id)));
        item.index = options
            .iter()
            .position(|(value, _)| *value == current)
            .map_or(-1, |i| i as i32);
        item.options = options.iter().map(|(_, text)| text.clone()).collect();
        s.items.push(item);
        self.extras.insert(
            id.to_owned(),
            Extra {
                step: None,
                choices: options.into_iter().map(|(v, _)| Value::Choice(v)).collect(),
            },
        );
    }

    /// A choice among numbers, shown as side-by-side buttons.
    fn segments(
        &mut self,
        s: &mut Section,
        id: &'static str,
        label: &str,
        options: &[(f64, String)],
    ) {
        let current = self.cx.settings.float(id);
        let mut item = Item::new(id, Kind::Segments, label);
        item.index = options
            .iter()
            .position(|(v, _)| (v - current).abs() < 0.01)
            .map_or(-1, |i| i as i32);
        item.options = options.iter().map(|(_, t)| t.clone()).collect();
        s.items.push(item);
        self.extras.insert(
            id.to_owned(),
            Extra {
                step: None,
                choices: options.iter().map(|(v, _)| Value::Float(*v)).collect(),
            },
        );
    }

    /// A choice among colours.
    fn swatches(&mut self, s: &mut Section, id: &'static str, label: &str, colors: &[[u8; 4]]) {
        let current = self.cx.settings.color(id);
        let mut item = Item::new(id, Kind::Swatches, label);
        item.index = colors
            .iter()
            .position(|c| *c == current)
            .map_or(-1, |i| i as i32);
        item.colors = colors.to_vec();
        s.items.push(item);
        self.extras.insert(
            id.to_owned(),
            Extra {
                step: None,
                choices: colors.iter().map(|c| Value::Color(*c)).collect(),
            },
        );
    }

    /// How the "anywhere" shortcut for a tool is used, with its current keys.
    fn anywhere_note(&self, s: &mut Section, id: &str, size: &str, more: &str) {
        let key = self
            .cx
            .shortcuts
            .iter()
            .find(|c| c.id == id)
            .map_or("its shortcut", |c| c.binding.as_str());
        self.note_more(
            s,
            &format!("Anywhere: tap {key} to toggle, or hold it to show until you let go."),
            &format!(
                "While holding, Up and Down change the {size} and + and \u{2212} the {more}. While drawing, scroll changes the {size} and Shift+scroll the {more}."
            ),
        );
    }

    /// A choice shown as side-by-side buttons.
    fn segment_choice(
        &mut self,
        s: &mut Section,
        id: &'static str,
        label: &str,
        options: Vec<(&'static str, String)>,
    ) {
        self.choice(s, id, Some(label), options);
        if let Some(item) = s.items.last_mut() {
            item.kind = Kind::Segments;
        }
    }

    fn action(&self, s: &mut Section, id: &str, label: &str, caption: &str) {
        let mut item = Item::new(id, Kind::Action, label);
        item.text = caption.to_owned();
        s.items.push(item);
    }

    /// A one-line note with longer help folded behind "More".
    fn note_more(&self, s: &mut Section, text: &str, more: &str) {
        self.note(s, text, false);
        if let Some(item) = s.items.last_mut() {
            item.hint = more.to_owned();
        }
    }

    fn note(&self, s: &mut Section, text: &str, warning: bool) {
        let mut item = Item::new("", Kind::Note, "");
        item.text = text.to_owned();
        item.warning = warning;
        s.items.push(item);
    }
}

fn percent(v: f64) -> String {
    format!("{:.0}%", v * 100.0)
}

pub fn build(page: PageId, cx: &Context<'_>) -> Built {
    let mut b = Builder::new(cx);
    let (title, summary) = match page {
        PageId::Sound => {
            sound_page(&mut b);
            ("Sound", "Tap sounds and the master mute.")
        }
        PageId::General => {
            general_page(&mut b);
            (
                "General",
                "How TinyPaw starts, where it draws, and how much it moves.",
            )
        }
        PageId::Shortcuts => {
            shortcuts_page(&mut b);
            ("Shortcuts", "The keys for each command.")
        }
        PageId::Ink => {
            ink_page(&mut b);
            (
                "Ink & text",
                "What the drawing tools draw with. The quick bar's ink button edits the same values.",
            )
        }
        PageId::Tools => {
            tools_page(&mut b);
            (
                "Spotlight & lens",
                "In annotation from the quick bar, or anywhere by shortcut.",
            )
        }
        PageId::Pointer => {
            pointer_page(&mut b);
            ("Pointer", "How the paw looks while you draw.")
        }
        PageId::Characters if cx.picking => {
            picker_page(&mut b);
            (
                "Add a character",
                "Choose who to put on the desktop. The same character can be there more than once.",
            )
        }
        PageId::Characters => {
            characters_page(&mut b);
            ("On the desktop", "The characters standing on your desktop.")
        }
        PageId::Music => {
            music_page(&mut b);
            (
                "Music & dance",
                "Characters move with what's playing on this PC. Nothing is recorded.",
            )
        }
    };
    Built {
        title,
        summary: summary.to_owned(),
        sections: b.sections,
        extras: b.extras,
    }
}

fn sound_page(b: &mut Builder<'_>) {
    b.section("Tap sound", |s, b| {
        let options = sound::CHOICES
            .iter()
            .filter(|&&id| id != "custom" || b.cx.has_custom_sound)
            .map(|&id| (id, sound::label(id).to_owned()))
            .collect();
        b.choice(s, sound::TAP, None, options);
        b.action(s, "sound.preview", "Hear the tap sound", "Play");
        b.action(s, "sound.import", "Your own sound", "Import a WAV…");
        b.range(s, sound::VOLUME, None, 0.05, percent);
        b.toggle(s, sound::MUTE, Some("Mute tap sounds"));
        b.toggle(
            s,
            sound::LASER,
            Some("Play a sound when tapping with the laser"),
        );
    });
    b.section("All sounds", |s, b| {
        b.toggle(s, sound::MUTE_ALL, Some("Mute all sounds"));
        b.note(s, "Also silences the characters on the desktop.", false);
    });
}

/// Colours offered for text labels.
const LABEL_COLORS: [[u8; 4]; 5] = [
    [255, 255, 255, 255],
    [34, 211, 238, 255],
    [255, 210, 80, 255],
    [255, 90, 100, 255],
    [0, 0, 0, 255],
];

fn ink_page(b: &mut Builder<'_>) {
    b.section("Ink", |s, b| {
        let inks: Vec<[u8; 4]> = crate::view::theme::INKS.iter().map(|&(_, c)| c).collect();
        b.swatches(s, system::INK_COLOR, "Colour", &inks);
        let widths: Vec<(f64, String)> = crate::view::theme::WIDTHS
            .iter()
            .map(|&w| (w, format!("{w:.0} px")))
            .collect();
        b.segments(s, system::INK_WIDTH, "Width", &widths);
        b.choice(
            s,
            system::INK_FADE,
            Some("Fade"),
            vec![
                ("keep", "Keep marks".to_owned()),
                ("3s", "Fade after 3 seconds".to_owned()),
            ],
        );
        b.note(s, "Pointer tap stamps share the fade.", false);
    });
    b.section("Text labels", |s, b| {
        b.range(s, text::SIZE, Some("Size"), 1.0, |v| format!("{v:.0}"));
        b.swatches(s, text::COLOR, "Colour", &LABEL_COLORS);
        b.toggle(s, text::BACKING, Some("Dark label backing"));
    });
}

fn tools_page(b: &mut Builder<'_>) {
    b.section("Spotlight", |s, b| {
        b.range(s, spotlight::RADIUS, Some("Size"), 5.0, |v| {
            format!("{v:.0} px")
        });
        b.range(s, spotlight::DIM, Some("Darkness"), 0.05, percent);
        b.anywhere_note(s, spotlight::ANYWHERE, "size", "darkness");
    });
    b.section("Lens", |s, b| {
        let zooms =
            b.cx.registry
                .choices(upcoming::ZOOM)
                .iter()
                .map(|&z| (z, format!("{z}\u{d7}")))
                .collect();
        b.choice(s, upcoming::ZOOM, Some("Magnification"), zooms);
        b.range(s, upcoming::SIZE, Some("Size"), 10.0, |v| {
            format!("{v:.0} px")
        });
        b.anywhere_note(s, upcoming::ANYWHERE, "size", "zoom");
    });
    b.section("Lens source", |s, b| lens_source(s, b));
    b.section("", |s, b| {
        b.action(
            s,
            "page:shortcuts",
            "Spotlight and lens shortcuts",
            "Change shortcuts",
        );
    });
}

#[cfg(windows)]
fn lens_source(s: &mut Section, b: &mut Builder<'_>) {
    let current = b.cx.settings.choice_text(upcoming::SOURCE).to_owned();
    let mut options = vec![
        (
            upcoming::DESKTOP.to_owned(),
            "Desktop under pointer".to_owned(),
        ),
        (
            upcoming::FOREGROUND.to_owned(),
            "Window active before TinyPaw".to_owned(),
        ),
    ];
    options.extend(b.cx.windows.iter().map(|w| (w.id.clone(), w.title.clone())));
    if !options.iter().any(|(id, _)| *id == current) {
        options.push((current.clone(), "A window that is not open now".to_owned()));
    }
    let mut item = Item::new(upcoming::SOURCE, Kind::Choice, "Magnify");
    item.index = options
        .iter()
        .position(|(id, _)| *id == current)
        .map_or(-1, |i| i as i32);
    item.options = options.iter().map(|(_, t)| t.clone()).collect();
    s.items.push(item);
    b.extras.insert(
        upcoming::SOURCE.to_owned(),
        Extra {
            step: None,
            choices: options
                .into_iter()
                .map(|(id, _)| Value::DynamicChoice(id))
                .collect(),
        },
    );
    b.action(s, "lens.refresh", "Open windows", "Refresh list");
    let note = if current == upcoming::DESKTOP {
        "Magnifies whatever is under the paw. TinyPaw stays out of the lens."
    } else {
        "Shows one window wherever the paw is. Use a borderless or windowed game."
    };
    b.note(s, note, false);
}

#[cfg(not(windows))]
fn lens_source(s: &mut Section, b: &mut Builder<'_>) {
    b.note(s, "Lens capture is available on Windows.", false);
}

/// Colours for the generated forearm.
const FOREARM_COLORS: [[u8; 4]; 10] = [
    [36, 39, 60, 255],
    [52, 54, 130, 255],
    [91, 155, 255, 255],
    [255, 255, 255, 255],
    [90, 90, 98, 255],
    [242, 166, 90, 255],
    [170, 100, 64, 255],
    [244, 154, 168, 255],
    [164, 112, 220, 255],
    [104, 184, 128, 255],
];

fn pointer_page(b: &mut Builder<'_>) {
    let settings = b.cx.settings;
    let laser = settings.choice(point::STYLE) == "laser";
    let current = settings.choice(point::PACK);
    let pack =
        b.cx.packs
            .iter()
            .find(|p| p.id == current)
            .or_else(|| b.cx.packs.first());
    b.section("Choose a pointer", |s, b| {
        let mut item = Item::new(POINTER, Kind::Cards, "");
        let mut choices: Vec<Value> = Vec::new();
        for p in b.cx.packs {
            item.options.push(p.name.clone());
            item.pictures.push(p.thumb.clone());
            choices.push(Value::Choice(crate::platform::intern(&p.id)));
        }
        item.options.push("Laser".into());
        item.pictures.push(Thumb::none());
        choices.push(Value::Choice("laser"));
        item.index = if laser {
            choices.len() as i32 - 1
        } else {
            pack.and_then(|p| b.cx.packs.iter().position(|q| q.id == p.id))
                .map_or(-1, |i| i as i32)
        };
        s.items.push(item);
        b.extras.insert(
            POINTER.to_owned(),
            Extra {
                step: None,
                choices,
            },
        );
    });
    if !laser {
        let presentation = settings.choice(point::PRESENTATION);
        b.section("Paw appearance", |s, b| {
            b.segment_choice(
                s,
                point::PRESENTATION,
                "Style",
                vec![
                    ("furry", "Furry Paw".to_owned()),
                    ("stretch", "Stretch Paw".to_owned()),
                    ("stick", "Classic Stick".to_owned()),
                ],
            );
            b.note(
                s,
                match presentation {
                    "stick" => "The paw on a slim rod from an off-screen grip, lower right.",
                    "stretch" => {
                        "A curved forearm follows the paw. Packs without forearm art use their accent colour."
                    }
                    _ => "A compact paw at the pointer.",
                },
                false,
            );
            b.range(s, point::SIZE, None, 4.0, |v| format!("{v:.0} px"));
            if presentation == "furry" {
                b.range(s, point::ANGLE, Some("Paw angle"), 5.0, |v| format!("{v:.0}\u{b0}"));
            }
        });
        if presentation == "stretch" && pack.is_none_or(|p| !p.forearm) {
            b.section("Forearm colour", |s, b| {
                b.toggle(s, point::FOREARM_AUTO_COLOR, Some("Use pointer colour"));
                // With the automatic colour on, the swatch shown is the one in use.
                let auto = settings.bool(point::FOREARM_AUTO_COLOR);
                let shown = if auto {
                    pack.and_then(|p| p.accent)
                        .unwrap_or([0xF2, 0xA6, 0x5A, 255])
                } else {
                    settings.color(point::FOREARM_COLOR)
                };
                let mut item = Item::new(point::FOREARM_COLOR, Kind::Swatches, "Colour");
                item.colors = FOREARM_COLORS.to_vec();
                item.index = FOREARM_COLORS
                    .iter()
                    .position(|c| *c == shown)
                    .map_or(-1, |i| i as i32);
                s.items.push(item);
                b.extras.insert(
                    point::FOREARM_COLOR.to_owned(),
                    Extra {
                        step: None,
                        choices: FOREARM_COLORS.iter().map(|c| Value::Color(*c)).collect(),
                    },
                );
            });
        }
        b.section("Tap stamp", |s, b| {
            b.segment_choice(
                s,
                point::TAP_STAMP,
                "Stamp",
                vec![
                    ("off", "Off".to_owned()),
                    ("cat", "Cat".to_owned()),
                    ("dog", "Dog".to_owned()),
                ],
            );
            b.note(
                s,
                "Each tap leaves a paw print in your ink colour, and fades as ink does.",
                false,
            );
            b.action(s, "page:ink", "Colour and fade", "Ink & text");
        });
    }
    b.section("Your pointers", |s, b| {
        b.action(s, "pointer.add", "From a transparent PNG", "Add pointer…");
        b.action(
            s,
            "pointer.import",
            "From a pointer pack folder",
            "Import folder…",
        );
        if !laser && let Some(p) = pack.filter(|p| p.custom) {
            b.action(
                s,
                "pointer.reaim",
                &format!("Aim {}", p.name),
                "Re-aim pointer",
            );
            b.action(
                s,
                "pointer.remove",
                &format!("Delete {}", p.name),
                "Remove pointer",
            );
        }
    });
    b.section("Make artwork", |s, b| {
        b.note(
            s,
            "Give a prompt and your character reference to an artist or image tool.",
            false,
        );
        b.action(s, "pointer.prompt", "Pointer art", "Copy prompt");
        b.action(s, "pointer.arm-prompt", "Forearm art", "Copy prompt");
    });
}

const FRAME_RATES: [(&str, &str); 3] = [
    ("30", "30 fps"),
    ("60", "60 fps"),
    ("display", "Match display"),
];

fn characters_page(b: &mut Builder<'_>) {
    b.section("", |s, b| {
        b.toggle(s, mascot::ENABLED, Some("Show characters"));
        b.action(s, PICK, "Add a character", "Add\u{2026}");
    });
    // One row per character on the desktop; the same character can be there
    // more than once. The open row is followed by its settings.
    let thumb_of = |id: &str| {
        b.cx.characters
            .iter()
            .find(|c| c.id == id)
            .map_or_else(Thumb::none, |c| c.thumb.clone())
    };
    let name_of = |id: &str| {
        b.cx.characters
            .iter()
            .find(|c| c.id == id)
            .map_or(id.to_owned(), |c| c.name.clone())
    };
    let titles: Vec<String> =
        b.cx.instances
            .iter()
            .map(|i| {
                let name = name_of(&i.character);
                let same: Vec<_> =
                    b.cx.instances
                        .iter()
                        .filter(|o| o.character == i.character)
                        .collect();
                match same.iter().position(|o| o.id == i.id) {
                    Some(n) if same.len() > 1 => format!("{name} \u{b7} {}", n + 1),
                    _ => name,
                }
            })
            .collect();
    let range = b.cx.registry.range(mascot::HEIGHT);
    if b.cx.instances.is_empty() {
        b.section("Nobody here yet", |s, b| {
            b.note(s, "Add a character to put it on the desktop.", false);
        });
    } else {
        b.section("Characters", |s, b| {
            for (instance, title) in b.cx.instances.iter().zip(titles) {
                let key = |field: &str| format!("instance.{}.{field}", instance.id);
                let open = b.cx.expanded == Some(instance.id);
                let mut entry = Item::new(&key("shown"), Kind::Entry, &title);
                entry.on = instance.shown;
                entry.index = i32::from(open);
                entry.pictures.push(thumb_of(&instance.character));
                if !instance.shown {
                    entry.hint = "Hidden".into();
                }
                s.items.push(entry);
                if !open {
                    continue;
                }
                let first = s.items.len();
                let mut size = Item::new(&key("height"), Kind::Range, "Size");
                size.value = instance.height as f32;
                size.min = *range.start() as f32;
                size.max = *range.end() as f32;
                size.text = format!("{:.0} px", instance.height);
                s.items.push(size);
                b.extras.insert(
                    key("height"),
                    Extra {
                        step: Some(4.0),
                        choices: Vec::new(),
                    },
                );
                let mut eyes = Item::new(&key("eyes"), Kind::Toggle, "Eyes follow the cursor");
                eyes.on = instance.eyes;
                s.items.push(eyes);
                let mut idle = Item::new(&key("idle"), Kind::Toggle, "Idle motion and blinking");
                idle.on = instance.idle;
                s.items.push(idle);
                b.action(
                    s,
                    &format!("instance.remove.{}", instance.id),
                    "Take it off the desktop",
                    "Remove",
                );
                for item in &mut s.items[first..] {
                    item.nested = true;
                }
            }
            b.note(
                s,
                "Drag a character to move it, click it to see it react, right-click for its menu.",
                false,
            );
        });
    }
    b.section("All characters", |s, b| {
        b.choice(
            s,
            mascot::FRAME_RATE,
            Some("Frame rate"),
            FRAME_RATES.map(|(v, l)| (v, l.to_owned())).to_vec(),
        );
        b.note(
            s,
            "Higher is smoother but uses more CPU while characters move.",
            false,
        );
        b.action(s, "character.reset", "Character positions", "Reset");
    });
    b.section("Library", |s, b| {
        b.action(
            s,
            "characters.edit",
            "Create and edit characters",
            "Open the studio\u{2026}",
        );
    });
}

/// The page while adding a character: the library as cards. Choosing one adds
/// it and returns to the list.
fn picker_page(b: &mut Builder<'_>) {
    b.section("Choose a character", |s, b| {
        let mut item = Item::new(ADD, Kind::Cards, "");
        let mut choices = Vec::new();
        for c in b.cx.characters {
            item.options.push(c.name.clone());
            item.pictures.push(c.thumb.clone());
            choices.push(Value::Choice(crate::platform::intern(&c.id)));
        }
        // Nothing is selected: a click adds.
        item.index = -1;
        s.items.push(item);
        b.extras.insert(
            ADD.to_owned(),
            Extra {
                step: None,
                choices,
            },
        );
    });
    b.section("", |s, b| {
        b.action(
            s,
            "characters.edit",
            "Make a new character",
            "Open the studio\u{2026}",
        );
        b.action(s, PICK_CANCEL, "Back to the list", "Cancel");
    });
}

fn music_page(b: &mut Builder<'_>) {
    let settings = b.cx.settings;
    b.section("Listening", |s, b| {
        b.toggle(s, mascot::MUSIC, Some("React to playback"));
        if !settings.bool(mascot::MUSIC) {
            return;
        }
        let current = settings.choice_text(mascot::MUSIC_SOURCE).to_owned();
        let mut options: Vec<(String, String)> = b
            .cx
            .music
            .sources
            .iter()
            .map(|(id, label, playing)| {
                let label = if *playing || id == "all" || id.starts_with("in:") {
                    label.clone()
                } else {
                    format!("{label} (not playing)")
                };
                (id.clone(), label)
            })
            .collect();
        if !options.iter().any(|(id, _)| *id == current) {
            options.push((current.clone(), current.clone()));
        }
        let mut item = Item::new(mascot::MUSIC_SOURCE, Kind::Choice, "Listen to");
        item.index = options
            .iter()
            .position(|(id, _)| *id == current)
            .map_or(-1, |i| i as i32);
        item.options = options.iter().map(|(_, l)| l.clone()).collect();
        s.items.push(item);
        b.extras.insert(
            mascot::MUSIC_SOURCE.to_owned(),
            Extra {
                step: None,
                choices: options
                    .iter()
                    .map(|(id, _)| Value::Choice(crate::platform::intern(id)))
                    .collect(),
            },
        );
        b.note(
            s,
            if current == "all" {
                "All apps together. Pick one app so a game or a call doesn't count; apps show up here while they make sound."
            } else if current.starts_with("in:") {
                "For music that doesn't play on this PC. This uses the input device (Windows shows the microphone in use) while listening; nothing is recorded."
            } else {
                "Only this app counts, and Follow beat hears only its sound."
            },
            false,
        );
        let mut meter = Item::new("music.level", Kind::Meter, "Hearing");
        meter.value = b.cx.music.level;
        meter.text = b.cx.music.status.clone();
        s.items.push(meter);
    });
    if !settings.bool(mascot::MUSIC) {
        return;
    }
    b.section("Movement", |s, b| {
        b.range(
            s,
            mascot::MUSIC_STRENGTH,
            Some("Music strength"),
            0.05,
            |v| format!("{v:.2}"),
        );
        b.range(s, mascot::EXCITEMENT, Some("Stand at energy"), 1.0, |v| {
            format!("{v:.0}%")
        });
        b.note_more(
            s,
            "A character on Automatic stands up above this energy and sits when the music calms.",
            "It stands after the music stays lively for a while, not on one loud moment. A higher value keeps it seated more often. Standing needs standing artwork.",
        );
        b.action(
            s,
            "characters.edit",
            "Each character's pose and waving hand",
            "Open the studio\u{2026}",
        );
    });
    b.section("Dancing", |s, b| {
        b.choice(
            s,
            mascot::DANCE_EVERY,
            Some("Change every"),
            vec![
                ("8", "8 beats".to_owned()),
                ("16", "16 beats".to_owned()),
                ("32", "32 beats".to_owned()),
            ],
        );
        b.choice(
            s,
            mascot::DANCE_ITEM,
            Some("Light-wave item"),
            vec![
                ("glow_stick", "Glow stick".to_owned()),
                ("phone", "Phone".to_owned()),
                ("random", "Random".to_owned()),
            ],
        );
        for (id, label) in [
            (mascot::DANCE_STYLES[0], "Random \u{b7} Nod"),
            (mascot::DANCE_STYLES[1], "Random \u{b7} Groove"),
            (mascot::DANCE_STYLES[2], "Random \u{b7} Rubber hose"),
            (mascot::DANCE_STYLES[3], "Random \u{b7} Compress"),
            (mascot::DANCE_STYLES[4], "Random \u{b7} Light wave"),
        ] {
            b.toggle(s, id, Some(label));
        }
        b.note_more(
            s,
            "Random picks among the checked styles. Light wave needs a holding arm and an item.",
            "The style changes on a beat boundary. With nothing checked, Random uses Nod. Characters without gesture artwork use the built-in glow stick. These apply to every character.",
        );
    });
    b.section("Follow beat", |s, b| {
        b.toggle(s, mascot::BEAT, Some("Follow beat"));
        b.note(s, "Nods land on the beat instead of a steady bob.", false);
        if !settings.bool(mascot::BEAT) {
            return;
        }
        if !b.cx.music.beat.is_empty() {
            b.note(s, &format!("Beat: {}", b.cx.music.beat), false);
        }
        // Fine tuning stays folded away until asked for.
        b.action(
            s,
            ADVANCED,
            "Advanced",
            if b.cx.advanced { "Hide" } else { "Show" },
        );
        if !b.cx.advanced {
            return;
        }
        let first = s.items.len();
        b.choice(
            s,
            mascot::BEAT_SENSITIVITY,
            Some("Sensitivity"),
            vec![
                ("low", "Low".to_owned()),
                ("normal", "Normal".to_owned()),
                ("high", "High".to_owned()),
            ],
        );
        b.choice(
            s,
            mascot::BEAT_TIMING,
            Some("Timing"),
            vec![
                ("earlier", "Earlier".to_owned()),
                ("default", "Default".to_owned()),
                ("later", "Later".to_owned()),
            ],
        );
        b.choice(
            s,
            mascot::BEAT_ENGINE,
            Some("Engine"),
            vec![
                ("classic", "Classic".to_owned()),
                ("bass", "Bass-weighted".to_owned()),
                ("sticky", "Sticky tempo".to_owned()),
                ("cumulative", "Cumulative".to_owned()),
            ],
        );
        b.note_more(
            s,
            "Try another engine if the nod drifts off the beat. Changing it restarts the search.",
            "Classic is the original. Bass-weighted counts kicks more than hats. Sticky tempo resists jumping to half or double time. Cumulative carries the beat's phase forward between hits.",
        );
        for item in &mut s.items[first..] {
            item.nested = true;
        }
    });
    if let Some(section) = b.sections.last_mut() {
        section.tag = "Trial".into();
    }
}

fn shortcuts_page(b: &mut Builder<'_>) {
    for (global, title) in [(true, "Available everywhere"), (false, "While drawing")] {
        b.section(title, |s, b| {
            for shortcut in b.cx.shortcuts.iter().filter(|c| c.global == global) {
                let mut item = Item::new(&shortcut.id, Kind::Shortcut, &shortcut.label);
                item.text = shortcut.binding.clone();
                if shortcut.taken {
                    item.hint = "In use by another app".into();
                    item.warning = true;
                }
                s.items.push(item);
            }
        });
    }
    b.section("", |s, b| {
        b.action(s, "shortcuts.reset", "All shortcuts", "Restore defaults");
    });
}

fn general_page(b: &mut Builder<'_>) {
    b.section("TinyPaw", |s, b| {
        b.toggle(s, system::START_AT_SIGN_IN, None);
    });
    b.section("Drawing", |s, b| {
        let hold = b.cx.settings.choice(system::ACTIVATION_STYLE) == "hold";
        b.segment_choice(
            s,
            system::ACTIVATION_STYLE,
            "Start drawing by",
            vec![
                ("toggle", "Toggle".to_owned()),
                ("hold", "Hold to draw".to_owned()),
            ],
        );
        b.note(
            s,
            if hold {
                "The game keeps its input. Hold the draw key to point and draw; your marks stay when you let go."
            } else {
                "The shortcut opens Point straight away. Press it again to give the game its mouse back."
            },
            false,
        );
        let mut options = vec![
            ("primary", "Primary display".to_owned()),
            ("all", "All displays".to_owned()),
        ];
        options.extend(
            b.cx.displays
                .iter()
                .map(|d| (crate::platform::intern(&d.name), d.text.clone())),
        );
        b.choice(s, display::TARGET, Some("Draw on"), options);
        let target = b.cx.settings.choice_text(display::TARGET);
        if target != "primary" && target != "all" && !b.cx.displays.iter().any(|d| d.name == target)
        {
            b.note(s, &format!("{target} isn't connected right now."), true);
        }
        b.toggle(s, display::HIDE_BAR, None);
        b.note(
            s,
            "When streaming, share the monitor, not the game window, so viewers see the paw and ink.",
            false,
        );
    });
    b.section("Motion", |s, b| {
        b.choice(
            s,
            system::MOTION,
            Some("Motion"),
            vec![
                ("system", "Follow Windows".to_owned()),
                ("full", "Full motion".to_owned()),
                ("reduced", "Reduced motion".to_owned()),
            ],
        );
        b.note(
            s,
            "Reduced motion also stops the characters' idle motion and blinking.",
            false,
        );
    });
    b.section("About", |s, b| {
        b.note(s, concat!("TinyPaw ", env!("CARGO_PKG_VERSION")), false);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features;
    use crate::registry::Registry;

    fn shortcuts() -> Vec<ShortcutInfo> {
        vec![
            ShortcutInfo {
                id: "system.master_toggle".into(),
                label: "Annotation enabled".into(),
                binding: "Ctrl+Alt+P".into(),
                global: true,
                taken: false,
            },
            ShortcutInfo {
                id: "edit.undo".into(),
                label: "Undo".into(),
                binding: "Ctrl+Z".into(),
                global: false,
                taken: true,
            },
        ]
    }

    fn build_page(page: PageId, custom_sound: bool) -> Built {
        build_view(page, custom_sound, None, false)
    }

    fn build_view(page: PageId, custom_sound: bool, expanded: Option<u32>, picking: bool) -> Built {
        build_with(page, custom_sound, expanded, picking, None)
    }

    /// As `build_view`, with settings changed first and fine tuning unfolded.
    fn build_with(
        page: PageId,
        custom_sound: bool,
        expanded: Option<u32>,
        picking: bool,
        sets: Option<&[(&'static str, Value)]>,
    ) -> Built {
        let registry = Registry::build(features::BUILTINS).expect("valid built-ins");
        let mut settings = registry.defaults();
        for (id, value) in sets.unwrap_or_default() {
            registry
                .set(&mut settings, id, value.clone())
                .expect("valid");
        }
        let displays = [DisplayInfo {
            name: "Main".into(),
            text: "Main — 1920 × 1080 (primary)".into(),
        }];
        build(
            page,
            &Context {
                registry: &registry,
                settings: &settings,
                displays: &displays,
                shortcuts: &shortcuts(),
                instances: &[InstanceInfo {
                    id: 1,
                    character: "yohta".into(),
                    shown: true,
                    height: 146.0,
                    eyes: true,
                    idle: true,
                }],
                characters: &[CharacterInfo {
                    id: "yohta".into(),
                    name: "Yohta".into(),
                    builtin: true,
                    thumb: Thumb::none(),
                }],
                music: &MusicInfo::default(),
                packs: &[PackInfo {
                    id: "yohta".into(),
                    name: "Yohta".into(),
                    accent: Some([1, 2, 3, 255]),
                    custom: false,
                    forearm: false,
                    thumb: Thumb::none(),
                }],
                windows: &[WindowInfo {
                    id: "w1".into(),
                    title: "Notepad".into(),
                }],
                has_custom_sound: custom_sound,
                expanded,
                picking,
                advanced: sets.is_some(),
            },
        )
    }

    #[test]
    fn music_keeps_what_is_shared_and_leaves_pose_and_hand_to_the_studio() {
        let on = [
            (mascot::MUSIC, Value::Bool(true)),
            (mascot::BEAT, Value::Bool(true)),
        ];
        let ids = |built: &Built| -> Vec<String> {
            built
                .sections
                .iter()
                .flat_map(|s| &s.items)
                .map(|i| i.id.clone())
                .collect()
        };
        let open = build_with(PageId::Music, false, None, false, Some(&on));
        let shown = ids(&open);
        for gone in [
            mascot::MUSIC_POSE,
            mascot::WAVE_X,
            mascot::WAVE_Y,
            mascot::WAVE_ANGLE,
            mascot::WAVE_RANGE,
        ] {
            assert!(!shown.iter().any(|i| i == gone), "{gone} is still on Music");
        }
        for kept in [
            mascot::MUSIC_STRENGTH,
            mascot::EXCITEMENT,
            mascot::BEAT_ENGINE,
        ] {
            assert!(shown.iter().any(|i| i == kept), "{kept} is missing");
        }
        assert!(shown.iter().any(|i| i == "characters.edit"));
        // The beat's fine tuning is nested under Advanced.
        let tuning = open
            .sections
            .iter()
            .flat_map(|s| &s.items)
            .find(|i| i.id == mascot::BEAT_ENGINE)
            .unwrap();
        assert!(tuning.nested);
    }

    #[test]
    fn every_navigation_entry_builds_a_page() {
        for (_, name, page) in NAV {
            let built = build_page(*page, false);
            assert!(!built.sections.is_empty(), "{name} has no sections");
        }
    }

    #[test]
    fn page_items_name_registered_settings_and_carry_how_to_set_them() {
        let registry = Registry::build(features::BUILTINS).expect("valid built-ins");
        for page in [
            PageId::Pointer,
            PageId::Sound,
            PageId::General,
            PageId::Ink,
            PageId::Tools,
            PageId::Characters,
            PageId::Music,
        ] {
            let built = build_page(page, true);
            for item in built.sections.iter().flat_map(|s| &s.items) {
                match item.kind {
                    // One character on the desktop is not a registered setting.
                    Kind::Toggle | Kind::Range | Kind::Entry
                        if item.id.starts_with("instance.") => {}
                    Kind::Entry => panic!("{} is not a character on the desktop", item.id),
                    Kind::Toggle | Kind::Range | Kind::Choice => {
                        assert!(
                            registry.setting(&item.id).is_some(),
                            "{} is not registered",
                            item.id
                        );
                    }
                    Kind::Action | Kind::Note | Kind::Shortcut => {}
                    Kind::Cards | Kind::Meter => {}
                    Kind::Segments | Kind::Swatches => {
                        assert!(registry.setting(&item.id).is_some(), "{}", item.id);
                    }
                }
                if matches!(
                    item.kind,
                    Kind::Choice | Kind::Segments | Kind::Swatches | Kind::Cards
                ) {
                    let extra = &built.extras[&item.id];
                    let shown = if item.kind == Kind::Swatches {
                        item.colors.len()
                    } else {
                        item.options.len()
                    };
                    assert_eq!(extra.choices.len(), shown, "{}", item.id);
                    assert!(item.index >= 0, "{} shows no current choice", item.id);
                }
                if item.kind == Kind::Range {
                    assert!(built.extras[&item.id].step.is_some());
                    assert!(item.min < item.max);
                }
            }
        }
    }

    #[test]
    fn the_pointer_page_offers_each_pack_and_the_laser_and_hides_paw_settings_for_the_laser() {
        let built = build_page(PageId::Pointer, false);
        let cards = &built.sections[0].items[0];
        assert_eq!(cards.kind, Kind::Cards);
        assert_eq!(cards.options, ["Yohta", "Laser"]);
        assert_eq!(cards.index, 0);
        let titles: Vec<_> = built.sections.iter().map(|s| s.title.as_str()).collect();
        assert!(titles.contains(&"Paw appearance") && titles.contains(&"Tap stamp"));
        // The default presentation shows the paw angle; the default pack is not custom.
        let ids: Vec<_> = built
            .sections
            .iter()
            .flat_map(|s| &s.items)
            .map(|i| i.id.as_str())
            .collect();
        assert!(ids.contains(&point::ANGLE));
        assert!(!ids.contains(&"pointer.reaim"));
    }

    #[test]
    fn each_character_on_the_desktop_is_one_row_that_opens_to_its_own_controls() {
        let ids = |built: &Built| -> Vec<String> {
            built
                .sections
                .iter()
                .find(|s| s.title == "Characters")
                .expect("the list")
                .items
                .iter()
                .filter(|i| i.kind != Kind::Note)
                .map(|i| i.id.clone())
                .collect()
        };
        let closed = build_page(PageId::Characters, false);
        assert_eq!(ids(&closed), ["instance.1.shown"]);
        let row = &closed.sections[1].items[0];
        assert_eq!(
            (row.kind, row.label.as_str(), row.on),
            (Kind::Entry, "Yohta", true)
        );
        assert_eq!(row.index, 0);

        let open = build_view(PageId::Characters, false, Some(1), false);
        assert_eq!(
            ids(&open),
            [
                "instance.1.shown",
                "instance.1.height",
                "instance.1.eyes",
                "instance.1.idle",
                "instance.remove.1"
            ]
        );
        let list = &open.sections[1].items;
        assert_eq!(list[0].index, 1);
        assert!(!list[0].nested && list[1..5].iter().all(|i| i.nested));
        assert_eq!(open.extras["instance.1.height"].step, Some(4.0));
    }

    #[test]
    fn adding_a_character_is_a_picker_of_the_library_with_nothing_selected() {
        let list = build_page(PageId::Characters, false);
        assert!(
            list.sections
                .iter()
                .flat_map(|s| &s.items)
                .any(|i| i.id == PICK)
        );
        let picker = build_view(PageId::Characters, false, None, true);
        assert_eq!(picker.title, "Add a character");
        let cards = &picker.sections[0].items[0];
        assert_eq!((cards.id.as_str(), cards.kind), (ADD, Kind::Cards));
        assert_eq!(cards.options, ["Yohta"]);
        assert_eq!(cards.index, -1);
        assert_eq!(picker.extras[ADD].choices, [Value::Choice("yohta")]);
        assert!(
            picker
                .sections
                .iter()
                .flat_map(|s| &s.items)
                .any(|i| i.id == PICK_CANCEL)
        );
    }

    #[test]
    fn the_ink_page_marks_the_current_colour_and_width() {
        let built = build_page(PageId::Ink, false);
        let item = |id: &str| {
            built
                .sections
                .iter()
                .flat_map(|s| &s.items)
                .find(|i| i.id == id)
                .expect(id)
        };
        let ink = item(system::INK_COLOR);
        assert_eq!(ink.kind, Kind::Swatches);
        assert_eq!(ink.colors.len(), crate::view::theme::INKS.len());
        // The default ink is the first swatch and the default width is 4 px.
        assert_eq!(ink.index, 0);
        assert_eq!(
            item(system::INK_WIDTH).options[item(system::INK_WIDTH).index as usize],
            "4 px"
        );
        assert_eq!(item(system::INK_FADE).options.len(), 2);
    }

    #[cfg(windows)]
    #[test]
    fn the_lens_source_lists_open_windows_after_the_two_built_in_sources() {
        let built = build_page(PageId::Tools, false);
        let source = built
            .sections
            .iter()
            .flat_map(|s| &s.items)
            .find(|i| i.id == upcoming::SOURCE)
            .expect("lens source");
        assert_eq!(source.options[2], "Notepad");
        assert_eq!(
            built.extras[upcoming::SOURCE].choices[2],
            Value::DynamicChoice("w1".into())
        );
        // Choosing a window is accepted as the setting's value.
        let registry = Registry::build(features::BUILTINS).expect("valid built-ins");
        let mut values = registry.defaults();
        registry
            .set(
                &mut values,
                upcoming::SOURCE,
                Value::DynamicChoice("w1".into()),
            )
            .expect("a window ID is a valid lens source");
        assert_eq!(values.choice_text(upcoming::SOURCE), "w1");
        // The default source is the foreground window.
        assert_eq!(
            source.options[source.index as usize],
            "Window active before TinyPaw"
        );
    }

    #[test]
    fn the_shortcuts_page_lists_each_shortcut_in_its_scope_and_flags_taken_keys() {
        let built = build_page(PageId::Shortcuts, false);
        let section = |title: &str| {
            built
                .sections
                .iter()
                .find(|s| s.title == title)
                .expect(title)
        };
        let global = section("Available everywhere");
        let overlay = section("While drawing");
        assert_eq!(global.items.len(), 1);
        assert_eq!(global.items[0].kind, Kind::Shortcut);
        assert_eq!(global.items[0].text, "Ctrl+Alt+P");
        assert!(!global.items[0].warning);
        assert_eq!(overlay.items[0].hint, "In use by another app");
        assert!(overlay.items[0].warning);
    }

    #[test]
    fn settings_opens_on_general_which_holds_the_app_wide_settings_once() {
        assert_eq!(NAV[0].2, PageId::General);
        assert_eq!(index_of(""), 0);
        assert_eq!(index_of("display"), 0);
        let ids = |page| -> Vec<String> {
            build_page(page, false)
                .sections
                .iter()
                .flat_map(|s| &s.items)
                .map(|i| i.id.clone())
                .collect()
        };
        let general = ids(PageId::General);
        for id in [
            system::START_AT_SIGN_IN,
            system::ACTIVATION_STYLE,
            display::TARGET,
            display::HIDE_BAR,
            system::MOTION,
        ] {
            assert!(general.iter().any(|i| i == id), "{id} is not on General");
        }
        // One home each: no setting shows on two pages.
        let mut seen: HashMap<String, &str> = HashMap::new();
        for (_, name, page) in NAV {
            for id in ids(*page) {
                if id.is_empty() || id.starts_with("page:") {
                    continue;
                }
                if let Some(other) = seen.insert(id.clone(), name) {
                    assert_eq!(other, *name, "{id} is on {other} and {name}");
                }
            }
        }
    }

    #[test]
    fn long_help_folds_behind_more_and_the_trial_is_tagged() {
        let on = [
            (mascot::MUSIC, Value::Bool(true)),
            (mascot::BEAT, Value::Bool(true)),
        ];
        let music = build_with(PageId::Music, false, None, false, Some(&on));
        let beat = music.sections.last().expect("follow beat");
        assert_eq!(
            (beat.title.as_str(), beat.tag.as_str()),
            ("Follow beat", "Trial")
        );
        assert!(music.sections.iter().filter(|s| !s.tag.is_empty()).count() == 1);
        // A note shows one line; the rest waits behind More.
        for page in NAV.iter().map(|n| n.2) {
            let built = build_with(page, false, None, false, Some(&on));
            for note in built
                .sections
                .iter()
                .flat_map(|s| &s.items)
                .filter(|i| i.kind == Kind::Note)
            {
                assert!(
                    note.text.chars().count() <= 150,
                    "a note is too long to scan: {}",
                    note.text
                );
            }
        }
        let folded = music
            .sections
            .iter()
            .flat_map(|s| &s.items)
            .filter(|i| i.kind == Kind::Note && !i.hint.is_empty())
            .count();
        assert!(folded >= 3);
    }

    #[test]
    fn the_imported_sound_is_offered_only_when_there_is_one() {
        let without = build_page(PageId::Sound, false);
        let with = build_page(PageId::Sound, true);
        let count = |b: &Built| b.sections[0].items[0].options.len();
        assert_eq!(count(&with), count(&without) + 1);
    }

    #[test]
    fn setting_values_round_trip_through_the_command_line_text() {
        let registry = Registry::build(features::BUILTINS).expect("valid built-ins");
        let mut values = registry.defaults();
        for (id, value) in [
            (sound::VOLUME, Value::Float(0.35)),
            (sound::MUTE, Value::Bool(true)),
            (display::TARGET, Value::Choice("all")),
        ] {
            let text = super::super::link::value_text(&value);
            let parsed = registry.parse_value(id, &text).expect("parses back");
            registry.set(&mut values, id, parsed).expect("valid");
            assert_eq!(
                values.iter().find(|(i, _)| *i == id).map(|(_, v)| v),
                Some(&value)
            );
        }
    }

    #[test]
    fn the_state_the_app_reports_parses_back_into_settings() {
        let registry = Registry::build(features::BUILTINS).expect("valid built-ins");
        let mut values = registry.defaults();
        registry
            .set(&mut values, sound::VOLUME, Value::Float(0.4))
            .unwrap();
        let reply = format!(
            "ok: {}",
            serde_json::json!({
                "values": crate::preferences::to_json(&values),
                "displays": [{ "name": "Main", "text": "Main" }],
                "custom_sound": true,
                "focus": true,
            })
        );
        let state = super::super::link::parse_state(&registry, &reply).expect("parses");
        assert!((state.values.float(sound::VOLUME) - 0.4).abs() < 1e-9);
        assert!(state.custom_sound && state.focus);
        assert_eq!(state.displays.len(), 1);
    }
}
