//! Settings window: header with the master switch, a left navigation column
//! and one section at a time. Every control edits a registered setting through
//! the same typed update path as the CLI and quick bar.

use iced::widget::{
    Column, button, canvas, checkbox, column, container, pick_list, row, scrollable, slider, space,
    text, toggler, tooltip,
};
use iced::{Alignment, Color, Element, Length, Theme};

use super::icons::{Glyph, icon};
use super::{PreviewTile, UiMsg, theme};
use crate::app::Message;
use crate::features::{display, mascot, point, sound, system};
use crate::packs::{Draft, Pack};
use crate::registry::{Registry, ShortcutContext, Value};
use crate::session::{ActivationStyle, Session};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Pointer,
    Character,
    Annotation,
    Controls,
    Display,
    Sound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CharacterTab {
    /// The library and desktop preferences.
    #[default]
    Home,
    /// Customize → Artwork: arrange layers in the Arrange preview.
    Artwork,
    /// Customize → Behavior: reactions, lines, moving parts and music,
    /// tried in the Test character preview.
    Behavior,
    /// Customize → Advanced: resting point and stage size.
    Advanced,
}

/// The merged Windows header: sizes shared with the native hit test, and the
/// caption buttons TinyPaw draws in place of the system title bar.
pub mod caption {
    /// Header height in logical pixels.
    pub const HEADER: f32 = 44.0;
    /// Width of each caption button in logical pixels.
    pub const BUTTON_W: f32 = 46.0;
    /// Logical width left of the caption buttons that holds clickable
    /// controls (the Annotation switch); the rest of the header drags.
    pub const CONTROLS_W: f32 = 270.0;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Button {
        Minimize,
        Maximize,
        Close,
    }

    /// Live state of the merged header, when it is installed.
    #[derive(Debug, Clone, Copy, Default)]
    pub struct Frame {
        pub hover: Option<Button>,
        pub maximized: bool,
    }
}

/// Where the user asked to go while the character had unsaved changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leave {
    Page(Page),
    Character(&'static str),
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaveChoice {
    Save,
    Discard,
    Stay,
}

pub struct SettingsView<'a> {
    pub registry: &'a Registry,
    pub session: &'a Session,
    pub page: Page,
    pub character_tab: CharacterTab,
    pub displays: &'a [crate::platform::displays::Display],
    #[cfg(windows)]
    pub sources: &'a [crate::platform::windows::WindowSource],
    pub has_custom_sound: bool,
    /// Commands whose global shortcut is held by another app.
    pub taken: &'a [&'static str],
    pub notice: Option<&'a str>,
    pub packs: &'a [Pack],
    /// The pack the Point tool draws with.
    pub pack: Option<&'a Pack>,
    /// "Add your pointer" in progress.
    pub draft: Option<&'a Draft>,
    pub editing: bool,
    /// Set when the Windows merged header replaced the system title bar.
    pub frame: Option<caption::Frame>,
    /// Bundled Yohta first, then installed characters.
    pub characters: &'a [crate::mascot::pack::Character],
    /// The selected character's ID.
    pub character: &'static str,
    /// "Add character" in progress.
    pub character_draft: Option<&'a crate::mascot::pack::Draft>,
    pub character_busy: bool,
    /// A folder import waiting on Keep both or Replace: the character's name.
    pub character_conflict: Option<&'a str>,
    /// The Art tab's open character, while editing.
    pub art: Option<&'a crate::mascot::editor::Editor>,
    /// The Motion tab's live preview, on the Character page.
    pub motion: Option<&'a crate::mascot::preview::MotionPreview>,
    /// Create from reference in progress, with the reference's thumbnail.
    pub reference: Option<(&'a crate::mascot::reference::Request, Option<&'a iced::advanced::image::Handle>)>,
    /// The character draft has changes Save would write.
    pub unsaved: bool,
    /// Save / Discard / Stay is waiting for an answer.
    pub leaving: bool,
    /// Music response status and live level, for the meter.
    pub music: (String, f32),
    /// What music response listens to, and the choices for the picker.
    pub music_source: crate::mascot::music::Source,
    /// Sources to pick from with their live meters (0–1, decibel scale)
    /// and whether each is playing; then the input devices.
    pub music_rows: Vec<(crate::mascot::music::Source, f32, bool)>,
    pub music_inputs: Vec<crate::mascot::music::Source>,
    /// The Follow beat trial's status line.
    pub beat: String,
}

mod character;
use character::character_page;

fn h1<'a>(label: &'a str) -> Element<'a, Message> {
    text(label)
        .font(theme::HEADING)
        .size(22)
        .color(theme::TEXT)
        .into()
}

fn h2<'a>(label: &'a str) -> Element<'a, Message> {
    text(label)
        .font(theme::MEDIUM)
        .size(15)
        .color(theme::TEXT_SOFT)
        .into()
}

fn hint<'a>(label: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(label).size(12).color(theme::MUTED).into()
}

fn keycap<'a>(label: String) -> Element<'a, Message> {
    container(text(label.replace('+', " ")).size(12).color(theme::SUBTLE))
        .padding([3, 8])
        .style(theme::keycap)
        .into()
}

fn section<'a>(children: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    Column::with_children(children).spacing(10).into()
}

fn later<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    tooltip(
        content,
        container(text("Coming in a later alpha").size(12))
            .padding([6, 10])
            .style(theme::fill(theme::HOVER, 8.0)),
        tooltip::Position::Top,
    )
    .into()
}

pub fn view(v: SettingsView<'_>) -> Element<'_, Message> {
    let panel = iced::widget::keyed_column([(v.page, main(&v))])
        .width(Length::Fill)
        .height(Length::Fill);
    let body = row![nav(v.page), panel].height(Length::Fill);
    container(column![header(&v), body])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::fill(theme::SURFACE, 0.0))
        .into()
}

fn header<'a>(v: &SettingsView<'a>) -> Element<'a, Message> {
    let on = v.session.is_enabled();
    let logo = container(
        iced::widget::image(crate::branding::logo())
            .width(30)
            .height(30),
    )
    .width(30)
    .height(30)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center);
    let switch = toggler(on)
        .label("Annotation")
        .text_size(14)
        .size(22)
        .spacing(10)
        .font(theme::MEDIUM)
        .style(theme::master_toggler)
        .on_toggle(|_| Message::Command(system::MASTER_TOGGLE));
    let mut right = row![].spacing(14).align_y(Alignment::Center);
    if let Some(label) = v.registry.shortcut_label(system::MASTER_TOGGLE) {
        right = right.push(keycap(label));
    }
    right = right.push(container(switch).padding([6, 12]).style(theme::outlined(
        theme::CONTROL,
        999.0,
        theme::LINE,
    )));
    if let Some(frame) = v.frame {
        return framed_header(logo, right, frame);
    }
    container(
        row![
            row![logo, text("TinyPaw").font(theme::HEADING).size(17)]
                .spacing(10)
                .align_y(Alignment::Center),
            space().width(Length::Fill),
            right,
        ]
        .align_y(Alignment::Center),
    )
    .height(60)
    .padding([0, 20])
    .align_y(Alignment::Center)
    .style(theme::outlined(theme::HEADER, 0.0, theme::LINE))
    .into()
}

/// One row in place of the Windows title bar. Everything outside the switch
/// area and the caption buttons drags the window; the native hit test uses
/// the same `caption` sizes, so the two must change together.
fn framed_header<'a>(
    logo: impl Into<Element<'a, Message>>,
    controls: iced::widget::Row<'a, Message>,
    frame: caption::Frame,
) -> Element<'a, Message> {
    use caption::{BUTTON_W, Button, CONTROLS_W, HEADER};
    // Windows' own close-hover red, so the button reads as the system one.
    let close_red = Color::from_rgb8(0xC4, 0x2B, 0x1C);
    let cap = |b: Button, glyph: Glyph| -> Element<'a, Message> {
        let hovered = frame.hover == Some(b);
        let (bg, fg) = match (b, hovered) {
            (Button::Close, true) => (close_red, Color::WHITE),
            (_, true) => (theme::HOVER, theme::TEXT),
            _ => (Color::TRANSPARENT, theme::TEXT_SOFT),
        };
        container(icon(glyph, 14.0, fg))
            .width(BUTTON_W)
            .height(HEADER)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(theme::fill(bg, 0.0))
            .into()
    };
    container(
        row![
            space().width(14),
            logo.into(),
            space().width(10),
            text("TinyPaw Settings").font(theme::HEADING).size(14),
            space().width(Length::Fill),
            container(controls)
                .width(CONTROLS_W)
                .padding([0, 10])
                .align_x(Alignment::End),
            cap(Button::Minimize, Glyph::WinMinimize),
            cap(
                Button::Maximize,
                if frame.maximized {
                    Glyph::WinRestore
                } else {
                    Glyph::WinMaximize
                },
            ),
            cap(Button::Close, Glyph::Close),
        ]
        .align_y(Alignment::Center),
    )
    .height(HEADER)
    .style(theme::outlined(theme::HEADER, 0.0, theme::LINE))
    .into()
}

fn nav<'a>(page: Page) -> Element<'a, Message> {
    let item = |p: Page, glyph: Glyph, label: &'static str| {
        let on = page == p;
        button(
            row![
                icon(
                    glyph,
                    18.0,
                    if on { theme::CYAN_TEXT } else { theme::SUBTLE }
                ),
                text(label)
                    .size(14)
                    .font(if on { theme::MEDIUM } else { theme::BODY }),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .height(40)
        .padding([10, 12])
        .style(theme::nav_button(on))
        .on_press(Message::Ui(UiMsg::Nav(p)))
    };
    container(
        column![
            hint("Annotation"),
            item(Page::Pointer, Glyph::Pointer, "Pointer"),
            item(Page::Annotation, Glyph::Pen, "Drawing tools"),
            item(Page::Sound, Glyph::Sound, "Tap sounds"),
            space().height(10),
            hint("Desktop & input"),
            item(Page::Character, Glyph::Head, "Character"),
            item(Page::Display, Glyph::Display, "Display & motion"),
            item(Page::Controls, Glyph::Controls, "Drawing shortcuts"),
        ]
        .spacing(4),
    )
    .width(208)
    .height(Length::Fill)
    .padding([16, 12])
    .style(theme::outlined(theme::NAV, 0.0, theme::LINE))
    .into()
}

fn main<'a>(v: &SettingsView<'a>) -> Element<'a, Message> {
    let mut content: Vec<Element<'a, Message>> = Vec::new();
    if let Some(notice) = v.notice {
        content.push(
            container(text(notice).size(13).color(theme::AMBER_TEXT))
                .padding([8, 12])
                .style(theme::fill(theme::AMBER_BG, 8.0))
                .into(),
        );
    }
    let (body, side) = match v.page {
        Page::Pointer => pointer_page(v),
        Page::Annotation => (annotation_page(v), None),
        Page::Controls => (controls(v), None),
        Page::Display => (display_page(v), None),
        Page::Sound => (sound_page(v), None),
        Page::Character => character_page(v),
    };
    content.push(body);
    let pad = |left: f32, right: f32| iced::Padding {
        top: 24.0,
        right,
        bottom: 24.0,
        left,
    };
    // The side panel (preview or pointer editor) scrolls separately, so the
    // preview stays in view while the settings beside it scroll. The same
    // two-column structure is kept without one: switching between a page
    // with a side panel and one without (on the same Page) must not hand
    // one scroll area's widget state to a different widget.
    let has_side = side.is_some();
    let side = match side {
        Some(side) => container(side).padding(pad(14.0, 28.0)),
        None => container(space().width(0)),
    };
    row![
        scrollable(
            container(Column::with_children(content).spacing(16))
                .padding(pad(28.0, if has_side { 14.0 } else { 28.0 }))
                .width(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill),
        scrollable(side).width(Length::Shrink).height(Length::Fill),
    ]
    .into()
}

/// The pointer's look, plus the preview `main` keeps in view.
fn pointer_page<'a>(v: &SettingsView<'a>) -> (Element<'a, Message>, Option<Element<'a, Message>>) {
    let s = v.session.settings();
    let laser = s.choice(point::STYLE) == "laser";

    let card = |art: Element<'a, Message>, label: String, on: bool, msg: Message| {
        button(
            column![
                container(art)
                    .height(52)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center),
                text(label).font(theme::MEDIUM).size(12),
            ]
            .spacing(6)
            .width(Length::Fill)
            .align_x(Alignment::Center),
        )
        .width(92)
        .height(96)
        .padding([10, 6])
        .style(theme::card(on))
        .on_press(msg)
    };
    let current = v.pack.map(|p| p.id);
    let mut pointers = row(v.packs.iter().map(|p| {
        card(
            iced::widget::image(p.image.clone()).height(48).into(),
            p.name.clone(),
            !laser && current == Some(p.id),
            Message::Ui(UiMsg::PickPack(p.id)),
        )
        .into()
    }))
    .spacing(8);
    let laser_tint = Color::from_rgb8(0xF8, 0x71, 0x71);
    pointers = pointers.push(card(
        container(icon(Glyph::Laser, 22.0, laser_tint))
            .width(40)
            .height(40)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(theme::fill(
                Color {
                    a: 0.14,
                    ..laser_tint
                },
                20.0,
            ))
            .into(),
        "Laser".into(),
        laser,
        Message::Set(point::STYLE, Value::Choice("laser")),
    ));
    pointers = pointers.push(card(
        container(icon(Glyph::Plus, 22.0, theme::MUTED))
            .width(40)
            .height(40)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(theme::outlined(
                Color::TRANSPARENT,
                20.0,
                theme::LINE_STRONG,
            ))
            .into(),
        "Add pointer".into(),
        false,
        Message::Ui(UiMsg::AddPointer),
    ));
    let pointers = pointers.wrap().vertical_spacing(8);

    let pres = s.choice(point::PRESENTATION);
    let seg = |label: &'static str, id: &'static str, enabled: bool| -> Element<'a, Message> {
        let b = button(text(label).size(13))
            .padding([7, 12])
            .style(theme::segment(pres == id))
            .on_press_maybe(
                enabled.then_some(Message::Set(point::PRESENTATION, Value::Choice(id))),
            );
        if enabled { b.into() } else { later(b) }
    };
    let presentation = container(
        row![
            seg("Furry Paw", "furry", true),
            seg("Stretch Paw", "stretch", true),
            seg("Classic Stick", "stick", true),
        ]
        .spacing(4),
    )
    .padding(4)
    .style(theme::fill(theme::WELL, 10.0));
    let pres_hint = match pres {
        "stick" => "The paw on a slim rod from an off-screen grip, lower right.",
        "stretch" => {
            "A curved forearm follows the paw. Packs without forearm art use their accent colour."
        }
        _ => "A compact paw at the pointer.",
    };

    let mut stamps = row![].spacing(4);
    for (id, label) in [("off", "Off"), ("cat", "Cat"), ("dog", "Dog")] {
        stamps = stamps.push(
            button(text(label).size(13))
                .padding([7, 12])
                .style(theme::segment(s.choice(point::TAP_STAMP) == id))
                .on_press(Message::Set(point::TAP_STAMP, Value::Choice(id))),
        );
    }

    let size = s.float(point::SIZE);
    let size_row = row![
        h2("Size"),
        space().width(Length::Fill),
        text(format!("{size:.0} px")).size(13).color(theme::TEXT),
    ];
    let size_slider = slider(v.registry.range(point::SIZE), size, |x| {
        Message::Set(point::SIZE, Value::Float(x))
    })
    .step(4.0)
    .style(theme::range);

    let current_ink = s.color(system::INK_COLOR);
    let angle = s.float(point::ANGLE);
    let angle_control = section(vec![
        row![
            h2("Paw angle"),
            space().width(Length::Fill),
            text(format!("{angle:.0}°")).size(13).color(theme::TEXT),
        ]
        .into(),
        slider(v.registry.range(point::ANGLE), angle, |x| {
            Message::Set(point::ANGLE, Value::Float(x))
        })
        .step(5.0)
        .style(theme::range)
        .into(),
        hint("Positive angles reach from the lower right."),
    ]);
    let width = s.float(system::INK_WIDTH);
    let mut left = column![
        h1("Pointer"),
        section(vec![h2("Choose a pointer"), pointers.into()]),
    ]
    .spacing(22)
    .width(Length::Fill);

    if !laser {
        let mut appearance = column![
            h2("Paw appearance"),
            presentation,
            hint(pres_hint),
            size_row,
            size_slider
        ]
        .spacing(10);
        if pres == "furry" {
            appearance = appearance.push(angle_control);
        }
        if pres == "stretch" && v.pack.is_none_or(|p| p.forearm.is_none()) {
            appearance = appearance.push(forearm_colors(s, v.pack.and_then(|p| p.accent)));
        }
        left = left.push(appearance).push(section(vec![
            h2("Tap stamp"), stamps.into(),
            hint("Each tap leaves a paw print in your ink colour. Set its fade time in Drawing tools."),
        ]));
    }
    let mut actions = row![
        button("Import folder")
            .style(theme::secondary)
            .on_press(Message::Ui(UiMsg::ImportPack))
    ]
    .spacing(8);
    if !laser && v.pack.is_some_and(|p| p.folder.is_some()) {
        actions = actions
            .push(
                button("Re-aim pointer")
                    .style(theme::secondary)
                    .on_press(Message::Ui(UiMsg::EditPack)),
            )
            .push(
                button("Remove pointer")
                    .style(theme::secondary)
                    .on_press(Message::Ui(UiMsg::RemovePack)),
            );
    }
    left = left.push(section(vec![
        h2("Custom pointers"),
        actions.wrap().into(),
        hint("To create matching artwork, share a prompt and your character reference with an artist or image tool."),
        row![
            button("Copy pointer prompt").style(theme::secondary).on_press(Message::Ui(UiMsg::CopyPointerPrompt)),
            button("Copy forearm prompt").style(theme::secondary).on_press(Message::Ui(UiMsg::CopyForearmPrompt)),
        ].spacing(8).wrap().into(),
    ]));

    let ink = theme::rgba(current_ink);
    let tile = |bg: Color| {
        canvas(PreviewTile {
            forearm_color: point::forearm_color(s),
            background: bg,
            ink,
            width: width as f32,
            laser,
            stick: pres == "stick",
            stretch: pres == "stretch",
            scale: size as f32 / point::BASE_SIZE,
            angle: -(angle as f32).to_radians(),
            pack: v.pack.cloned(),
        })
        .width(Length::Fill)
        .height(150)
    };
    if let Some(draft) = v.draft {
        return (left.into(), Some(new_pointer(draft, v.editing, s)));
    }
    let preview = column![
        h2("Preview"),
        tile(Color::from_rgb8(0xF4, 0xF4, 0xF5)),
        tile(Color::from_rgb8(0x1B, 0x26, 0x21)),
        button(
            text("Preview tap sound")
                .font(theme::MEDIUM)
                .size(13)
                .width(Length::Fill)
                .center()
        )
        .width(Length::Fill)
        .padding([11, 12])
        .style(theme::secondary)
        .on_press(Message::Ui(UiMsg::PreviewSound)),
    ]
    .spacing(10)
    .width(260);

    (left.into(), Some(preview.into()))
}

/// Colour controls apply only to the generated forearm, leaving paw art intact.
fn forearm_colors<'a>(
    s: &'a crate::registry::SettingValues,
    accent: Option<[u8; 4]>,
) -> Element<'a, Message> {
    let automatic = s.bool(point::FOREARM_AUTO_COLOR);
    let color = if automatic {
        accent.unwrap_or([0xF2, 0xA6, 0x5A, 255])
    } else {
        s.color(point::FOREARM_COLOR)
    };
    let palette = [
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
    let swatches = row(palette.into_iter().map(|c| {
        button(space().width(24).height(24))
            .padding(0)
            .style(theme::swatch(theme::rgba(c), c == color))
            .on_press(Message::Ui(UiMsg::ForearmColor(c)))
            .into()
    }))
    .spacing(6)
    .wrap();
    let mut controls = column![
        h2("Forearm colour"),
        hint("Used by Stretch Paw when no forearm PNG is attached."),
        check("Use pointer colour", automatic, point::FOREARM_AUTO_COLOR),
        swatches,
    ]
    .spacing(8);
    for (channel, label) in ["Red", "Green", "Blue"].into_iter().enumerate() {
        controls = controls.push(
            row![
                container(hint(label)).width(42),
                slider(0.0..=255.0, color[channel] as f32, move |value| {
                    let mut next = color;
                    next[channel] = value.round() as u8;
                    Message::Ui(UiMsg::ForearmColor(next))
                })
                .step(1.0_f32)
                .style(theme::range),
                text(color[channel].to_string()).size(12).width(28),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );
    }
    section(vec![controls.into()])
}

/// Ink, then each tool's options: what the quick bar's tools draw with.
fn annotation_page<'a>(v: &SettingsView<'a>) -> Element<'a, Message> {
    let s = v.session.settings();
    let current_ink = s.color(system::INK_COLOR);
    let width = s.float(system::INK_WIDTH);
    let swatches = row(theme::INKS.iter().map(|&(_, c)| {
        button(space().width(28).height(28))
            .padding(0)
            .style(theme::swatch(theme::rgba(c), c == current_ink))
            .on_press(Message::Set(system::INK_COLOR, Value::Color(c)))
            .into()
    }))
    .spacing(8);
    let widths = container(
        row(theme::WIDTHS.iter().map(|&w| {
            button(text(format!("{w:.0} px")).size(13))
                .padding([7, 12])
                .style(theme::segment((w - width).abs() < 0.01))
                .on_press(Message::Set(system::INK_WIDTH, Value::Float(w)))
                .into()
        }))
        .spacing(4),
    )
    .padding(4)
    .style(theme::fill(theme::WELL, 10.0));
    let fade = pick_list(
        [FadeChoice::Keep, FadeChoice::ThreeSeconds],
        Some(if s.choice(system::INK_FADE) == "3s" {
            FadeChoice::ThreeSeconds
        } else {
            FadeChoice::Keep
        }),
        |f| Message::Set(system::INK_FADE, Value::Choice(f.id())),
    )
    .text_size(13)
    .padding([8, 10])
    .style(theme::select);

    let mut ink_row = row![swatches].spacing(12).align_y(Alignment::Center);
    if let Some(accent) = v.pack.and_then(|p| p.accent) {
        ink_row = ink_row.push(super::match_pointer(accent, current_ink));
    }
    #[cfg(windows)]
    let source_picker: Element<'a, Message> = {
        use crate::features::upcoming::{DESKTOP, FOREGROUND, SOURCE};
        let source = s.choice_text(SOURCE);
        let desktop = source == DESKTOP;
        let mode = |label: &'static str, on: bool, value: &str| {
            button(text(label).size(13))
                .padding([7, 12])
                .style(theme::segment(on))
                .on_press(Message::Set(SOURCE, Value::DynamicChoice(value.to_owned())))
        };
        let mut picker = column![
            h2("Lens source"),
            container(
                row![
                    mode("Desktop under pointer", desktop, DESKTOP),
                    // Locking starts from the window you were using.
                    mode(
                        "Lock to a window",
                        !desktop,
                        if desktop { FOREGROUND } else { source }
                    ),
                ]
                .spacing(4)
            )
            .padding(4)
            .style(theme::fill(theme::WELL, 10.0)),
        ]
        .spacing(8);
        picker = if desktop {
            picker.push(hint(
                "Magnifies whatever app is under the paw. TinyPaw stays out of the lens but still shows in a screen share.",
            ))
        } else {
            picker
                .push(
                    pick_list(
                        v.sources.to_vec(),
                        v.sources.iter().find(|w| w.id == source).cloned(),
                        |w| Message::Set(SOURCE, Value::DynamicChoice(w.id)),
                    )
                    .placeholder("Window active before TinyPaw")
                    .padding([8, 12])
                    .text_size(13)
                    .width(Length::Fill)
                    .style(theme::select),
                )
                .push(
                    row![
                        button("Refresh windows")
                            .style(theme::secondary)
                            .on_press(Message::Ui(UiMsg::RefreshSources)),
                        button("Use foreground")
                            .style(theme::secondary)
                            .on_press(Message::Set(SOURCE, Value::Choice(FOREGROUND))),
                    ]
                    .spacing(8),
                )
                .push(hint(
                    "Shows one window wherever the paw is. Use a borderless or windowed game.",
                ))
        };
        picker.into()
    };
    #[cfg(not(windows))]
    let source_picker: Element<'a, Message> = hint("Lens capture is available on Windows.");
    let label_swatches = row([
        [255, 255, 255, 255],
        [34, 211, 238, 255],
        [255, 210, 80, 255],
        [255, 90, 100, 255],
        [0, 0, 0, 255],
    ]
    .into_iter()
    .map(|c| {
        button(space().width(28).height(28))
            .style(theme::swatch(
                theme::rgba(c),
                s.color(crate::features::text::COLOR) == c,
            ))
            .on_press(Message::Set(crate::features::text::COLOR, Value::Color(c)))
            .into()
    }))
    .spacing(8);
    column![
        h1("Drawing tools"),
        section(vec![
            h2("Ink"),
            ink_row.into(),
            row![
                widths,
                row![hint("Fade"), fade]
                    .spacing(8)
                    .align_y(Alignment::Center)
            ]
            .spacing(16)
            .align_y(Alignment::Center)
            .into(),
            hint("The quick bar's ink button edits these same values."),
        ]),
        section(vec![
            h2("Text labels"),
            hint("Size"),
            slider(
                v.registry.range(crate::features::text::SIZE),
                s.float(crate::features::text::SIZE),
                |x| { Message::Set(crate::features::text::SIZE, Value::Float(x)) }
            )
            .step(1.0)
            .style(theme::range)
            .into(),
            hint("Colour"),
            label_swatches.into(),
            check(
                "Dark label backing",
                s.bool(crate::features::text::BACKING),
                crate::features::text::BACKING,
            ),
        ]),
        section(vec![
            h2("Spotlight radius"),
            slider(
                v.registry.range(crate::features::spotlight::RADIUS),
                s.float(crate::features::spotlight::RADIUS),
                |x| { Message::Set(crate::features::spotlight::RADIUS, Value::Float(x)) }
            )
            .step(5.0)
            .style(theme::range)
            .into(),
        ]),
        section(vec![
            h2("Lens"),
            row![
                hint("Magnification"),
                pick_list(
                    v.registry.choices(crate::features::upcoming::ZOOM),
                    Some(s.choice(crate::features::upcoming::ZOOM)),
                    |z| Message::Set(crate::features::upcoming::ZOOM, Value::Choice(z))
                )
                .padding([8, 12])
                .style(theme::select),
            ]
            .spacing(12)
            .align_y(Alignment::Center)
            .into(),
            hint("Size"),
            slider(
                v.registry.range(crate::features::upcoming::SIZE),
                s.float(crate::features::upcoming::SIZE),
                |x| { Message::Set(crate::features::upcoming::SIZE, Value::Float(x)) }
            )
            .step(10.0)
            .style(theme::range)
            .into(),
            source_picker,
        ]),
    ]
    .spacing(22)
    .max_width(640)
    .into()
}

/// "Add your pointer": click the exact tip, name it, save.
fn new_pointer<'a>(
    draft: &'a Draft,
    editing: bool,
    settings: &'a crate::registry::SettingValues,
) -> Element<'a, Message> {
    if editing {
        return column![
            h2("Re-aim pointer"),
            canvas(HotspotPicker {
                draft,
                forearm: false
            })
            .width(Length::Fill)
            .height(180),
            hint("Click the exact pointing tip. Both idle and pressed art keep this anchor."),
            row![
                button("Cancel")
                    .padding([8, 12])
                    .style(theme::secondary)
                    .on_press(Message::Ui(UiMsg::CancelDraft)),
                button("Save click point")
                    .padding([8, 12])
                    .style(theme::primary)
                    .on_press(Message::Ui(UiMsg::SaveDraft))
            ]
            .spacing(8),
        ]
        .spacing(10)
        .width(260)
        .into();
    }
    let arm: Element<'a, Message> = if draft.forearm.is_some() {
        column![
            h2("Forearm attachment"),
            canvas(HotspotPicker {
                draft,
                forearm: true
            })
            .width(Length::Fill)
            .height(130),
            hint("Click the wrist joint at the top of the forearm."),
            button(text("Remove forearm").size(13))
                .style(theme::secondary)
                .on_press(Message::Ui(UiMsg::RemoveForearm)),
        ]
        .spacing(8)
        .into()
    } else {
        button(text("Add matching forearm PNG").size(13))
            .padding([8, 12])
            .style(theme::secondary)
            .on_press(Message::Ui(UiMsg::AddForearm))
            .into()
    };
    column![
        h2("New pointer"),
        canvas(HotspotPicker {
            draft,
            forearm: false
        })
        .width(Length::Fill)
        .height(150),
        hint(
            "Click the exact tip of the pointer. Light and dark halves show how it reads on both."
        ),
        button(if draft.pressed.is_some() {
            "Replace pressed pose"
        } else {
            "Add pressed pose PNG"
        })
        .padding([8, 12])
        .style(theme::secondary)
        .on_press(Message::Ui(UiMsg::AddPressed)),
        button(if draft.tap.is_some() {
            "Replace pack tap WAV"
        } else {
            "Add default tap WAV"
        })
        .padding([8, 12])
        .style(theme::secondary)
        .on_press(Message::Ui(UiMsg::AddPackTap)),
        h2("Paw wrist position"),
        hint("Horizontal"),
        slider(0.0..=1.0, draft.wrist.0, |x| Message::Ui(
            UiMsg::DraftWristX(x)
        ))
        .step(0.01_f32),
        hint("Vertical"),
        slider(0.0..=1.0, draft.wrist.1, |y| Message::Ui(
            UiMsg::DraftWristY(y)
        ))
        .step(0.01_f32),
        h2("Forearm width"),
        slider(0.05..=1.5, draft.joint_width, |w| Message::Ui(
            UiMsg::DraftJointWidth(w)
        ))
        .step(0.01_f32),
        arm,
        if draft.forearm.is_none() {
            forearm_colors(settings, None)
        } else {
            space().into()
        },
        h2("Test the pair"),
        canvas(PairPreview {
            pack: draft.preview(),
            forearm_color: point::forearm_color(settings),
        })
        .width(Length::Fill)
        .height(220),
        hint("Move and hold the mouse in the preview to check bending and pressing."),
        iced::widget::text_input("Name", &draft.name)
            .on_input(|n| Message::Ui(UiMsg::DraftName(n)))
            .size(14)
            .padding([9, 10])
            .style(|_, status| iced::widget::text_input::Style {
                background: theme::CONTROL.into(),
                border: iced::Border {
                    radius: 8.0.into(),
                    width: 1.0,
                    color: if matches!(status, iced::widget::text_input::Status::Focused { .. }) {
                        theme::CYAN_RING
                    } else {
                        theme::LINE_STRONG
                    },
                },
                icon: theme::MUTED,
                placeholder: theme::FAINT,
                value: theme::TEXT,
                selection: theme::CYAN_BG,
            }),
        row![
            button(text("Cancel").size(13).width(Length::Fill).center())
                .width(Length::Fill)
                .padding([10, 12])
                .style(theme::secondary)
                .on_press(Message::Ui(UiMsg::CancelDraft)),
            button(
                text("Save pointer")
                    .font(theme::HEADING)
                    .size(13)
                    .width(Length::Fill)
                    .center()
            )
            .width(Length::Fill)
            .padding([10, 12])
            .style(theme::primary)
            .on_press(Message::Ui(UiMsg::SaveDraft)),
        ]
        .spacing(8),
    ]
    .spacing(10)
    .width(260)
    .into()
}

/// Shows the new pointer on light and dark halves with a crosshair at the
/// hotspot; a click inside the image moves the hotspot there.
struct HotspotPicker<'a> {
    draft: &'a Draft,
    forearm: bool,
}

impl HotspotPicker<'_> {
    fn image_rect(&self, bounds: iced::Size) -> iced::Rectangle {
        let pad = 16.0;
        let (w, h) = self
            .draft
            .forearm
            .as_ref()
            .filter(|_| self.forearm)
            .map_or((self.draft.width as f32, self.draft.height as f32), |a| {
                (a.width as f32, a.height as f32)
            });
        let fit = ((bounds.width - pad * 2.0) / w).min((bounds.height - pad * 2.0) / h);
        let size = iced::Size::new(w * fit, h * fit);
        iced::Rectangle::new(
            iced::Point::new(
                (bounds.width - size.width) / 2.0,
                (bounds.height - size.height) / 2.0,
            ),
            size,
        )
    }
}

impl canvas::Program<Message> for HotspotPicker<'_> {
    type State = ();

    fn update(
        &self,
        _state: &mut (),
        event: &iced::Event,
        bounds: iced::Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if let iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) =
            event
            && let Some(p) = cursor.position_in(bounds)
        {
            let r = self.image_rect(bounds.size());
            if r.contains(p) {
                let fx = (p.x - r.x) / r.width;
                let fy = (p.y - r.y) / r.height;
                let message = if self.forearm {
                    UiMsg::DraftForearmAnchor(fx, fy)
                } else {
                    UiMsg::DraftHotspot(fx, fy)
                };
                return Some(canvas::Action::publish(Message::Ui(message)).and_capture());
            }
        }
        None
    }

    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        use iced::widget::canvas::{Frame, Path, Stroke};
        let size = bounds.size();
        let mut back = Frame::new(renderer, size);
        back.fill(
            &Path::rounded_rectangle(iced::Point::ORIGIN, size, 12.0.into()),
            Color::from_rgb8(0xF4, 0xF4, 0xF5),
        );
        back.fill_rectangle(
            iced::Point::new(size.width / 2.0, 0.0),
            iced::Size::new(size.width / 2.0, size.height),
            Color::from_rgb8(0x1B, 0x26, 0x21),
        );
        let r = self.image_rect(size);
        let (image, anchor) = self
            .draft
            .forearm
            .as_ref()
            .filter(|_| self.forearm)
            .map_or((&self.draft.image, self.draft.hotspot), |a| {
                (&a.image, a.anchor)
            });
        back.draw_image(r, canvas::Image::new(image.clone()));
        // Crosshair in its own frame: images paint above shapes in one frame.
        let mut marks = Frame::new(renderer, size);
        let hx = r.x + anchor.0 * r.width;
        let hy = r.y + anchor.1 * r.height;
        let ring = Path::circle(iced::Point::new(hx, hy), 7.0);
        marks.stroke(
            &ring,
            Stroke::default().with_color(Color::BLACK).with_width(3.0),
        );
        marks.stroke(
            &ring,
            Stroke::default().with_color(theme::CYAN).with_width(1.5),
        );
        if !self.forearm {
            let wrist = iced::Point::new(
                r.x + self.draft.wrist.0 * r.width,
                r.y + self.draft.wrist.1 * r.height,
            );
            marks.stroke(
                &Path::circle(wrist, 5.0),
                Stroke::default().with_color(theme::AMBER).with_width(2.0),
            );
        }
        vec![back.into_geometry(), marks.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _state: &(),
        bounds: iced::Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> iced::mouse::Interaction {
        match cursor.position_in(bounds) {
            Some(p) if self.image_rect(bounds.size()).contains(p) => {
                iced::mouse::Interaction::Crosshair
            }
            _ => iced::mouse::Interaction::default(),
        }
    }
}

struct PairPreview {
    pack: Pack,
    forearm_color: Option<[u8; 4]>,
}

struct PairPreviewState {
    session: Session,
    started: std::time::Instant,
    // Only a press begun in this canvas owns the matching release.
    pressed: bool,
}

impl Default for PairPreviewState {
    fn default() -> Self {
        let registry = Registry::build(crate::features::BUILTINS).expect("valid built-ins");
        let mut settings = registry.defaults();
        registry
            .set(&mut settings, point::PRESENTATION, Value::Choice("stretch"))
            .unwrap();
        let mut session = Session::new(point::TOOL, point::new_state, settings);
        session.handle(crate::session::Event::MasterToggle, 0);
        let ticket = session.pending_ticket().unwrap();
        session.handle(crate::session::Event::InputAcquired(ticket), 0);
        Self {
            session,
            started: std::time::Instant::now(),
            pressed: false,
        }
    }
}

impl canvas::Program<Message> for PairPreview {
    type State = PairPreviewState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &iced::Event,
        bounds: iced::Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        use crate::session::{Event, OutputId, PointerEvent, PointerKind, Pt};
        let now = state.started.elapsed().as_millis() as u64;
        let position = cursor.position_in(bounds);
        let kind =
            match event {
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { .. })
                    if position.is_some() =>
                {
                    Some(PointerKind::Moved)
                }
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { .. })
                    if state.session.pointer().is_some() =>
                {
                    Some(PointerKind::Left)
                }
                iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                    iced::mouse::Button::Left,
                )) if position.is_some() => Some(PointerKind::Down),
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                    iced::mouse::Button::Left,
                )) if state.pressed => Some(PointerKind::Up),
                iced::Event::Mouse(iced::mouse::Event::CursorLeft) => Some(PointerKind::Left),
                _ => None,
            };
        if let Some(kind) = kind {
            match kind {
                PointerKind::Down => state.pressed = true,
                PointerKind::Up | PointerKind::Left => state.pressed = false,
                _ => {}
            }
            let p = position.unwrap_or(iced::Point::new(bounds.width * 0.4, 30.0));
            state.session.handle(
                Event::Pointer(PointerEvent {
                    kind,
                    output: OutputId(0),
                    pos: Pt::new(p.x, p.y),
                    at: now,
                }),
                now,
            );
            if matches!(kind, PointerKind::Up | PointerKind::Left) {
                state.session.handle(Event::Clear, now);
            }
            return Some(canvas::Action::request_redraw().and_capture());
        }
        state
            .session
            .animating(now)
            .then(canvas::Action::request_redraw)
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &iced::Renderer,
        _: &Theme,
        bounds: iced::Rectangle,
        _: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        frame.fill_rectangle(iced::Point::ORIGIN, bounds.size(), theme::WELL);
        let now = state.started.elapsed().as_millis() as u64;
        let tip = state
            .session
            .tip(now)
            .map(|(_, tip)| tip)
            .unwrap_or(crate::session::Tip::Paw {
                at: crate::session::Pt::new(bounds.width * 0.4, 30.0),
                press: 0.0,
                scale: 1.0,
                angle: 0.0,
                stick: false,
                stretch: true,
                grip_x: bounds.width * 0.4,
            });
        super::paw::draw_tip(
            &mut frame,
            tip,
            bounds.size(),
            Some(&self.pack),
            self.forearm_color,
        );
        vec![frame.into_geometry()]
    }
}

#[cfg(test)]
mod pointer_preview_tests {
    use super::*;
    use iced::widget::canvas::Program;

    #[test]
    fn preview_leaves_button_clicks_alone_but_finishes_its_own_press() {
        let preview = PairPreview {
            pack: crate::packs::load_all().0.remove(0),
            forearm_color: None,
        };
        let mut state = PairPreviewState::default();
        let bounds = iced::Rectangle::new(iced::Point::ORIGIN, iced::Size::new(260.0, 220.0));
        let outside = iced::mouse::Cursor::Available(iced::Point::new(150.0, 300.0));
        let inside = iced::mouse::Cursor::Available(iced::Point::new(100.0, 100.0));
        let down = iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left));
        let up = iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
            iced::mouse::Button::Left,
        ));
        let status = |action: Option<canvas::Action<Message>>| {
            action.map_or(iced::event::Status::Ignored, |a| a.into_inner().2)
        };

        // Save and Cancel are outside the preview and must get both events.
        assert_eq!(
            status(preview.update(&mut state, &down, bounds, outside)),
            iced::event::Status::Ignored
        );
        assert_eq!(
            status(preview.update(&mut state, &up, bounds, outside)),
            iced::event::Status::Ignored
        );
        assert_eq!(
            status(preview.update(&mut state, &down, bounds, inside)),
            iced::event::Status::Captured
        );
        assert_eq!(
            status(preview.update(&mut state, &up, bounds, outside)),
            iced::event::Status::Captured
        );
        assert_eq!(
            status(preview.update(&mut state, &up, bounds, outside)),
            iced::event::Status::Ignored
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FadeChoice {
    Keep,
    ThreeSeconds,
}

impl FadeChoice {
    fn id(self) -> &'static str {
        match self {
            Self::Keep => "keep",
            Self::ThreeSeconds => "3s",
        }
    }
}

/// A motion preference value with its display label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Motion(&'static str);

impl std::fmt::Display for Motion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.0 {
            "full" => "Full motion",
            "reduced" => "Reduced motion",
            _ => "Follow Windows",
        })
    }
}

impl std::fmt::Display for FadeChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Keep => "Keep until cleared",
            Self::ThreeSeconds => "After 3 seconds",
        })
    }
}

fn controls<'a>(v: &SettingsView<'a>) -> Element<'a, Message> {
    let style = v.session.activation_style();
    let card = |label: &'static str, desc: &'static str, id: &'static str, on: bool| {
        button(
            column![
                text(label).font(theme::HEADING).size(14).color(theme::TEXT),
                text(desc).size(12).color(theme::MUTED),
            ]
            .spacing(6),
        )
        .width(Length::Fill)
        .padding(14)
        .style(theme::card(on))
        .on_press(Message::Set(system::ACTIVATION_STYLE, Value::Choice(id)))
    };
    let styles = row![
        card(
            "Toggle",
            "The shortcut opens Point straight away. Press it again to give the game its mouse back.",
            "toggle",
            style == ActivationStyle::Toggle,
        ),
        card(
            "Hold to draw",
            "The game keeps input. Hold the draw key to point and draw; let go and your marks stay.",
            "hold",
            style == ActivationStyle::HoldToDraw,
        ),
    ]
    .spacing(10);

    let mut rows = column![].spacing(0);
    for (context, scope) in [
        (ShortcutContext::Global, "Available everywhere"),
        (ShortcutContext::Overlay, "While drawing"),
    ] {
        rows = rows.push(container(hint(scope)).padding([8, 4]));
        for (shortcut, def) in v.registry.shortcuts(context) {
            rows = rows.push(
                container(
                    row![
                        text(def.label).size(14).width(Length::Fill),
                        v.taken
                            .contains(&def.id)
                            .then(|| text("In use by another app")
                                .size(12)
                                .color(theme::AMBER_TEXT)),
                        container(keycap(shortcut.label()))
                            .width(120)
                            .align_x(Alignment::End),
                        button("Change")
                            .padding([7, 12])
                            .style(theme::secondary)
                            .on_press(Message::Ui(UiMsg::Rebind(def.id))),
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center),
                )
                .height(46)
                .padding([0, 4])
                .align_y(Alignment::Center)
                .style(|_: &Theme| container::Style {
                    border: iced::Border {
                        width: 0.0,
                        ..iced::Border::default()
                    },
                    ..container::Style::default()
                }),
            );
            rows = rows.push(
                container(space())
                    .height(1)
                    .width(Length::Fill)
                    .style(theme::fill(theme::LINE, 0.0)),
            );
        }
    }

    column![
        h1("Drawing shortcuts"),
        section(vec![h2("How drawing starts"), styles.into()]),
        section(vec![
            h2("Shortcuts"),
            rows.into(),
            button("Restore default shortcuts")
                .padding([8, 12])
                .style(theme::secondary)
                .on_press(Message::Ui(UiMsg::ResetShortcuts))
                .into(),
        ]),
    ]
    .spacing(22)
    .max_width(680)
    .into()
}

fn check<'a>(label: &'a str, on: bool, id: &'static str) -> Element<'a, Message> {
    checkbox(on)
        .label(label)
        .text_size(14)
        .size(16)
        .spacing(12)
        .style(|_, status| {
            let checked = matches!(
                status,
                checkbox::Status::Active { is_checked: true }
                    | checkbox::Status::Hovered { is_checked: true }
            );
            checkbox::Style {
                background: (if checked { theme::CYAN } else { theme::CONTROL }).into(),
                icon_color: theme::WELL,
                border: iced::Border {
                    radius: 4.0.into(),
                    width: 1.0,
                    color: if checked {
                        theme::CYAN
                    } else {
                        theme::LINE_STRONG
                    },
                },
                text_color: Some(theme::TEXT),
            }
        })
        .on_toggle(move |b| Message::Set(id, Value::Bool(b)))
        .into()
}

/// The characters, the selected one's desktop options, and the Add
/// character draft beside them while one is in progress.
fn display_page<'a>(v: &SettingsView<'a>) -> Element<'a, Message> {
    let s = v.session.settings();
    // The cards and the list share one selection; only a named display that
    // has gone away needs saying.
    let target = s.choice(display::TARGET);
    let missing: Element<'a, Message> =
        if target != "primary" && target != "all" && !v.displays.iter().any(|d| d.name == target) {
            text(format!("{target} isn't connected right now."))
                .size(12)
                .color(theme::AMBER_TEXT)
                .into()
        } else {
            space().into()
        };
    let share_tip = container(
        column![
            text("Streaming on Discord?")
                .font(theme::MEDIUM)
                .size(12)
                .color(theme::TEXT),
            text("Share the game monitor, not the game window, so viewers see the paw and ink.")
                .size(12)
                .color(theme::MUTED),
        ]
        .spacing(2),
    )
    .padding(12)
    .style(theme::fill(theme::RAISED, 10.0));
    column![
        h1("Display & motion"),
        section(vec![
            h2("Annotation display"),
            button("Primary display")
                .padding(12)
                .style(theme::card(s.choice(display::TARGET) == "primary"))
                .on_press(Message::Set(display::TARGET, Value::Choice("primary")))
                .into(),
            button("All displays")
                .padding(12)
                .style(theme::card(s.choice(display::TARGET) == "all"))
                .on_press(Message::Set(display::TARGET, Value::Choice("all")))
                .into(),
            pick_list(
                v.displays.to_vec(),
                v.displays
                    .iter()
                    .find(|d| d.name == s.choice(display::TARGET))
                    .cloned(),
                |d| Message::Set(display::TARGET, Value::Choice(d.name))
            )
            .placeholder("Choose a named display")
            .padding(12)
            .style(theme::select)
            .into(),
            missing,
        ]),
        section(vec![
            h2("Animation preference"),
            pick_list(
                [Motion("system"), Motion("full"), Motion("reduced")],
                Some(Motion(v.session.settings().choice(system::MOTION))),
                |m| Message::Set(system::MOTION, Value::Choice(m.0))
            )
            .padding([8, 12])
            .style(theme::select)
            .into(),
        ]),
        section(vec![
            h2("Streaming"),
            check(
                "Hide the quick bar while streaming",
                s.bool(display::HIDE_BAR),
                display::HIDE_BAR,
            ),
            hint("Shortcuts and the tray keep working while the bar is hidden."),
            share_tip.into(),
        ]),
    ]
    .spacing(22)
    .max_width(640)
    .into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SoundChoice(&'static str);

impl std::fmt::Display for SoundChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(sound::label(self.0))
    }
}

fn sound_page<'a>(v: &SettingsView<'a>) -> Element<'a, Message> {
    let s = v.session.settings();
    let options: Vec<SoundChoice> = sound::CHOICES
        .iter()
        .filter(|&&id| id != "custom" || v.has_custom_sound)
        .map(|&id| SoundChoice(id))
        .collect();
    let picker = pick_list(options, Some(SoundChoice(s.choice(sound::TAP))), |c| {
        Message::Set(sound::TAP, Value::Choice(c.0))
    })
    .width(Length::Fill)
    .text_size(14)
    .padding([10, 12])
    .style(theme::select);
    let volume = s.float(sound::VOLUME);
    column![
        h1("Tap sounds"),
        section(vec![
            h2("Sound selection"),
            row![
                picker,
                button(text("Play").font(theme::MEDIUM).size(13))
                    .padding([10, 16])
                    .style(theme::secondary)
                    .on_press(Message::Ui(UiMsg::PreviewSound)),
            ]
            .spacing(8)
            .into(),
            button(
                row![
                    icon(Glyph::Plus, 16.0, theme::CYAN_TEXT),
                    text("Import a WAV…").size(13).color(theme::CYAN_TEXT)
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .padding([6, 8])
            .style(theme::tool_button(theme::Tone::Plain))
            .on_press(Message::Ui(UiMsg::ImportWav))
            .into(),
        ]),
        section(vec![
            row![
                h2("Volume"),
                space().width(Length::Fill),
                text(format!("{:.0}%", volume * 100.0)).size(13)
            ]
            .into(),
            slider(v.registry.range(sound::VOLUME), volume, |x| Message::Set(
                sound::VOLUME,
                Value::Float(x)
            ))
            .step(0.05)
            .style(theme::range)
            .into(),
            check("Mute tap sounds", s.bool(sound::MUTE), sound::MUTE),
            check("Mute all sounds", s.bool(sound::MUTE_ALL), sound::MUTE_ALL),
            hint("Mute all sounds also silences the desktop character's reactions and line sounds; Mute tap sounds is pointer-only."),
            check(
                "Play a sound when tapping with the laser",
                s.bool(sound::LASER),
                sound::LASER
            ),
        ]),
    ]
    .spacing(22)
    .max_width(560)
    .into()
}
