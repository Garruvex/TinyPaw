//! The Settings pages, defined once: each page is sections of items that name
//! registered settings, with the wording and choice labels the registry does
//! not hold. The window renders whatever this produces, so adding a setting
//! to a page is one line here.

use std::collections::HashMap;

use super::DisplayInfo;
use super::link::{ShortcutInfo, WindowInfo};
use super::model::{Extra, Item, Kind, Section};
use crate::features::{display, sound, spotlight, system, text, upcoming};
use crate::registry::{Registry, SettingValues, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageId {
    Pointer,
    Ink,
    Tools,
    Characters,
    Shortcuts,
    Sound,
    Display,
}

/// Navigation order: group, name, page.
pub const NAV: &[(&str, &str, PageId)] = &[
    ("Annotate", "Pointer", PageId::Pointer),
    ("Annotate", "Ink & text", PageId::Ink),
    ("Annotate", "Spotlight & lens", PageId::Tools),
    ("Desktop", "Characters", PageId::Characters),
    ("App", "Shortcuts", PageId::Shortcuts),
    ("App", "Sound", PageId::Sound),
    ("App", "Display & motion", PageId::Display),
];

/// The page a name on the command line means, or the first.
pub fn index_of(name: &str) -> usize {
    let id = match name {
        "ink" => PageId::Ink,
        "tools" | "spotlight" | "lens" => PageId::Tools,
        "characters" | "character" => PageId::Characters,
        "shortcuts" => PageId::Shortcuts,
        "sound" => PageId::Sound,
        "display" => PageId::Display,
        _ => PageId::Pointer,
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
                "Anywhere: tap {key} to toggle, or hold it to show until release. While holding, Up/Down change the {size} and +/\u{2212} the {more}. In annotation, scroll for {size}, Shift+scroll for {more}."
            ),
            false,
        );
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
        PageId::Display => {
            display_page(&mut b);
            (
                "Display & motion",
                "Where TinyPaw draws, and how much it moves.",
            )
        }
        PageId::Shortcuts => {
            shortcuts_page(&mut b);
            (
                "Shortcuts",
                "How drawing starts, and the keys for each command.",
            )
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
        PageId::Pointer | PageId::Characters => {
            b.section("Not moved yet", |s, b| {
                b.note(
                    s,
                    "This page is still in the classic Settings window.",
                    false,
                );
                b.action(
                    s,
                    "open-classic",
                    "Classic Settings",
                    "Open classic Settings",
                );
            });
            let name = NAV.iter().find(|n| n.2 == page).map_or("", |n| n.1);
            (name, "")
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
        b.action(s, "sound.preview", "Preview", "Play");
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
        b.note(
            s,
            "Also silences the desktop character's reactions and line sounds.",
            false,
        );
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
        b.action(s, "page:shortcuts", "Shortcuts", "Change shortcuts");
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
        b.action(s, "page:shortcuts", "Shortcuts", "Change shortcuts");
    });
    b.section("Lens source", |s, b| lens_source(s, b));
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
    b.action(s, "lens.refresh", "Window list", "Refresh windows");
    let note = if current == upcoming::DESKTOP {
        "Magnifies whatever app is under the paw. TinyPaw stays out of the lens but still shows in a screen share."
    } else {
        "Shows one window wherever the paw is. Use a borderless or windowed game."
    };
    b.note(s, note, false);
}

#[cfg(not(windows))]
fn lens_source(s: &mut Section, b: &mut Builder<'_>) {
    b.note(s, "Lens capture is available on Windows.", false);
}

fn shortcuts_page(b: &mut Builder<'_>) {
    b.section("How drawing starts", |s, b| {
        b.choice(
            s,
            system::ACTIVATION_STYLE,
            Some("Activation style"),
            vec![
                ("toggle", "Toggle".to_owned()),
                ("hold", "Hold to draw".to_owned()),
            ],
        );
        b.note(
            s,
            "Toggle: the shortcut opens Point straight away. Press it again to give the game its mouse back.",
            false,
        );
        b.note(
            s,
            "Hold to draw: the game keeps input. Hold the draw key to point and draw; let go and your marks stay.",
            false,
        );
    });
    b.section("Startup", |s, b| {
        b.toggle(s, system::START_AT_SIGN_IN, None);
    });
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
        b.action(
            s,
            "shortcuts.reset",
            "Defaults",
            "Restore default shortcuts",
        );
    });
}

fn display_page(b: &mut Builder<'_>) {
    b.section("Annotation display", |s, b| {
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
    });
    b.section("Animation preference", |s, b| {
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
    });
    b.section("Streaming", |s, b| {
        b.toggle(s, display::HIDE_BAR, None);
        b.note(
            s,
            "Shortcuts and the tray keep working while the bar is hidden.",
            false,
        );
        b.note(
            s,
            "Streaming on Discord? Share the game monitor, not the game window, so viewers see the paw and ink.",
            false,
        );
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
        for page in [PageId::Sound, PageId::Display, PageId::Ink, PageId::Tools] {
            let built = build_page(page, true);
            for item in built.sections.iter().flat_map(|s| &s.items) {
                match item.kind {
                    Kind::Toggle | Kind::Range | Kind::Choice => {
                        assert!(
                            registry.setting(&item.id).is_some(),
                            "{} is not registered",
                            item.id
                        );
                    }
                    Kind::Action | Kind::Note | Kind::Shortcut => {}
                    Kind::Segments | Kind::Swatches => {
                        assert!(registry.setting(&item.id).is_some(), "{}", item.id);
                    }
                }
                if matches!(item.kind, Kind::Choice | Kind::Segments | Kind::Swatches) {
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
