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
        self.note(
            s,
            &format!(
                "Anywhere: tap {key} to toggle, or hold it. While drawing, scroll changes the {size} and Shift+scroll the {more}."
            ),
            false,
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
    let settings = b.cx.settings;
    b.section("", |s, b| {
        b.toggle(s, mascot::ENABLED, Some("Show characters"));
        b.note(
            s,
            "Drag a character to move it, click it to see it react, right-click for its menu.",
            false,
        );
    });
    // One section per character on the desktop; the same character can be there
    // more than once.
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
    for (instance, title) in b.cx.instances.iter().zip(titles) {
        let key = |field: &str| format!("instance.{}.{field}", instance.id);
        b.section(&title, |s, b| {
            let mut shown = Item::new(&key("shown"), Kind::Toggle, "Shown");
            shown.on = instance.shown;
            s.items.push(shown);
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
                "Remove from the desktop",
                "Remove",
            );
        });
    }
    if b.cx.instances.is_empty() {
        b.section("Nobody here yet", |s, b| {
            b.note(s, "Add a character to the desktop below.", false);
        });
    }
    b.section("Add a character", |s, b| {
        let current = settings.choice_text(mascot::CHARACTER);
        let mut item = Item::new(mascot::CHARACTER, Kind::Cards, "");
        let mut choices = Vec::new();
        for c in b.cx.characters {
            item.options.push(c.name.clone());
            item.pictures.push(c.thumb.clone());
            choices.push(Value::Choice(crate::platform::intern(&c.id)));
        }
        item.index =
            b.cx.characters
                .iter()
                .position(|c| c.id == current)
                .map_or(-1, |i| i as i32);
        s.items.push(item);
        b.extras.insert(
            mascot::CHARACTER.to_owned(),
            Extra {
                step: None,
                choices,
            },
        );
        let selected =
            b.cx.characters
                .iter()
                .find(|c| c.id == current)
                .map_or("the selected character", |c| c.name.as_str());
        b.action(
            s,
            "instance.add",
            &format!("Put {selected} on the desktop"),
            "Add",
        );
        b.action(
            s,
            "characters.edit",
            "Create and edit characters",
            "Open the studio…",
        );
    });
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
        b.range(s, mascot::MUSIC_STRENGTH, Some("Music strength"), 0.05, |v| {
            format!("{v:.2}")
        });
        b.choice(
            s,
            mascot::MUSIC_POSE,
            Some("Music pose"),
            vec![
                ("auto", "Automatic".to_owned()),
                ("sit", "Sitting".to_owned()),
                ("stand", "Standing".to_owned()),
            ],
        );
        b.range(s, mascot::EXCITEMENT, Some("Stand at energy"), 1.0, |v| {
            format!("{v:.0}%")
        });
        b.note(
            s,
            "Automatic stands up when the music stays lively and sits when it calms. Standing needs standing artwork.",
            false,
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
        b.note(
            s,
            "Random picks among the checked styles. Light wave needs a holding arm and an item.",
            false,
        );
    });
    b.section("Waving hand", |s, b| {
        b.range(s, mascot::WAVE_X, Some("Horizontal"), 1.0, |v| {
            format!("{v:.0}%")
        });
        b.range(s, mascot::WAVE_Y, Some("Vertical"), 1.0, |v| {
            format!("{v:.0}%")
        });
        b.range(s, mascot::WAVE_ANGLE, Some("Angle"), 1.0, |v| {
            format!("{v:.0}\u{b0}")
        });
        b.range(s, mascot::WAVE_RANGE, Some("Wave range"), 1.0, |v| {
            format!("{v:.0}%")
        });
        b.action(s, "music.reset-hand", "Hand position", "Reset");
        b.note(
            s,
            "Moves the waving arm and its item together, for every character.",
            false,
        );
    });
    b.section("Follow beat (trial)", |s, b| {
        b.toggle(s, mascot::BEAT, Some("Follow beat"));
        b.note(s, "Nods land on the beat instead of a steady bob.", false);
        if !settings.bool(mascot::BEAT) {
            return;
        }
        if !b.cx.music.beat.is_empty() {
            b.note(s, &format!("Beat: {}", b.cx.music.beat), false);
        }
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
        b.note(
            s,
            "Try another engine if the nod drifts off the beat. Changing it restarts the search.",
            false,
        );
    });
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
        let registry = Registry::build(features::BUILTINS).expect("valid built-ins");
        let settings = registry.defaults();
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
            },
        )
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
                    Kind::Toggle | Kind::Range if item.id.starts_with("instance.") => {}
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
    fn each_character_on_the_desktop_has_its_own_section_with_its_own_controls() {
        let built = build_page(PageId::Characters, false);
        let titles: Vec<_> = built.sections.iter().map(|s| s.title.as_str()).collect();
        assert!(titles.contains(&"Add a character") && titles.contains(&"Yohta"));
        let section = built.sections.iter().find(|s| s.title == "Yohta").unwrap();
        let ids: Vec<_> = section.items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "instance.1.shown",
                "instance.1.height",
                "instance.1.eyes",
                "instance.1.idle",
                "instance.remove.1"
            ]
        );
        assert_eq!(built.extras["instance.1.height"].step, Some(4.0));
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
