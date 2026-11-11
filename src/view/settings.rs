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

pub struct SettingsView<'a> {
    pub registry: &'a Registry,
    pub session: &'a Session,
    pub page: Page,
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
    /// Music response status and live level, for the meter.
    pub music: (String, f32),
    /// What music response listens to, and the choices for the picker.
    pub music_source: crate::mascot::music::Source,
    pub music_sources: Vec<crate::mascot::music::Source>,
    /// The Follow beat trial's status line.
    pub beat: String,
}

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
        .size(13)
        .color(theme::MUTED)
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
    let body = row![nav(v.page), main(&v)].height(Length::Fill);
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
            item(Page::Pointer, Glyph::Pointer, "Pointer"),
            item(Page::Character, Glyph::Head, "Character"),
            item(Page::Annotation, Glyph::Pen, "Annotation"),
            item(Page::Controls, Glyph::Controls, "Controls"),
            item(Page::Display, Glyph::Display, "Display"),
            item(Page::Sound, Glyph::Sound, "Tap sounds"),
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
    let Some(side) = side else {
        return scrollable(
            container(Column::with_children(content).spacing(16))
                .padding(pad(28.0, 28.0))
                .width(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    };
    // The side panel (preview or pointer editor) scrolls separately, so the
    // preview stays in view while the settings beside it scroll.
    row![
        scrollable(
            container(Column::with_children(content).spacing(16))
                .padding(pad(28.0, 14.0))
                .width(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill),
        scrollable(container(side).padding(pad(14.0, 28.0)))
            .width(Length::Shrink)
            .height(Length::Fill),
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
        "Add yours".into(),
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
    let left = column![
        h1("Pointer"),
        section(vec![h2("Pointer"), pointers.into()]),
        row![
            button("Import folder").style(theme::secondary).on_press(Message::Ui(UiMsg::ImportPack)),
            button("Re-aim pointer").style(theme::secondary).on_press_maybe(v.pack.filter(|p| p.folder.is_some()).map(|_| Message::Ui(UiMsg::EditPack))),
            button("Remove pointer").style(theme::secondary).on_press_maybe(v.pack.filter(|p| p.folder.is_some()).map(|_| Message::Ui(UiMsg::RemovePack))),
        ].spacing(8),
        section(vec![
            h2("Create your own artwork"),
            hint("Attach your character reference in your image tool or send it with the prompt to an artist."),
            row![
                button(text("Copy pointer prompt").size(13))
                    .padding([7, 12])
                    .style(theme::secondary)
                    .on_press(Message::Ui(UiMsg::CopyPointerPrompt)),
                button(text("Copy forearm prompt").size(13))
                    .padding([7, 12])
                    .style(theme::secondary)
                    .on_press(Message::Ui(UiMsg::CopyForearmPrompt)),
            ].spacing(8).wrap().into(),
            hint("Use Add yours to import the paw, then optionally attach a matching forearm and align the wrist."),
        ]),
        section(vec![h2("Presentation"), presentation.into(), hint(pres_hint)]),
        if !laser && pres == "stretch" && v.pack.is_none_or(|p| p.forearm.is_none()) {
            forearm_colors(s, v.pack.and_then(|p| p.accent))
        } else {
            space().into()
        },
        section(vec![
            h2("Tap stamp"),
            stamps.into(),
            hint("Leave a little paw print when you tap with a paw pointer. Uses your ink colour and fade setting; drag to draw as usual."),
        ]),
        section(vec![size_row.into(), size_slider.into()]),
        if !laser && pres == "furry" { angle_control } else { space().into() },
        hint("Ink, labels, Spotlight and Lens are under Annotation."),
    ]
    .spacing(22)
    .width(Length::Fill);

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
            text("Try a tap")
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
        h1("Annotation"),
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
            h2("Advanced"),
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
        (ShortcutContext::Global, "Global"),
        (ShortcutContext::Overlay, "While drawing"),
    ] {
        for (shortcut, def) in v.registry.shortcuts(context) {
            rows = rows.push(
                container(
                    row![
                        text(def.label).size(14).width(Length::Fill),
                        if v.taken.contains(&def.id) {
                            text("In use by another app")
                                .size(12)
                                .color(theme::AMBER_TEXT)
                        } else {
                            text(scope).size(12).color(theme::FAINT)
                        },
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
        h1("Controls"),
        h2("Motion"),
        pick_list(
            [Motion("system"), Motion("full"), Motion("reduced")],
            Some(Motion(v.session.settings().choice(system::MOTION))),
            |m| Message::Set(system::MOTION, Value::Choice(m.0))
        )
        .padding([8, 12])
        .style(theme::select),
        section(vec![h2("When switched On"), styles.into()]),
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
fn character_page<'a>(
    v: &SettingsView<'a>,
) -> (Element<'a, Message>, Option<Element<'a, Message>>) {
    if let Some(e) = v.art {
        return (
            art_page(e, v.character_busy),
            Some(art_preview(e, v.character_busy)),
        );
    }
    let s = v.session.settings();
    let size = s.float(mascot::SIZE);
    let selected = v
        .characters
        .iter()
        .find(|c| c.id == v.character)
        .or(v.characters.first());
    let name = selected.map_or("the character", |c| c.name.as_str());

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
    let mut cards = row(v.characters.iter().map(|c| {
        let art: Element<'a, Message> = match c.thumbnail() {
            Some(thumb) => iced::widget::image(thumb).height(48).into(),
            None => icon(Glyph::Head, 28.0, theme::MUTED).into(),
        };
        card(
            art,
            c.name.clone(),
            c.id == v.character && v.character_draft.is_none(),
            Message::Ui(UiMsg::PickCharacter(c.id)),
        )
        .into()
    }))
    .spacing(8);
    cards = cards.push(card(
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
        "Add character".into(),
        v.character_draft.is_some(),
        Message::Ui(UiMsg::AddCharacter),
    ));
    let custom = selected.filter(|c| !c.is_builtin());
    let manage = row![
        button("From parts…")
            .style(theme::secondary)
            .on_press(Message::Ui(UiMsg::ArtFromParts)),
        button("Import folder")
            .style(theme::secondary)
            .on_press(Message::Ui(UiMsg::ImportCharacter)),
        // Yohta is built in, so editing him starts from a copy.
        button(if custom.is_some() { "Edit art" } else { "Edit a copy" })
            .style(theme::secondary)
            .on_press_maybe(selected.map(|_| Message::Ui(UiMsg::EditArt))),
        button("Export…")
            .style(theme::secondary)
            .on_press_maybe(selected.map(|_| Message::Ui(UiMsg::ExportCharacter))),
        button("Remove")
            .style(theme::secondary)
            .on_press_maybe(custom.map(|_| Message::Ui(UiMsg::RemoveCharacter))),
    ]
    .spacing(8);

    let mut page = column![
        h1("Character"),
        section(vec![
            h2("Characters"),
            cards.wrap().vertical_spacing(8).into(),
            manage.into(),
            hint("Add character takes one transparent PNG. Import folder takes a character.json folder with a body and optional moving parts, like one made with Export."),
        ]),
    ]
    .spacing(22)
    .max_width(640);

    if let Some(other) = v.character_conflict {
        page = page.push(
            container(
                column![
                    text(format!("A character called {other} is already installed."))
                        .size(13)
                        .color(theme::AMBER_TEXT),
                    row![
                        button("Keep both")
                            .style(theme::secondary)
                            .on_press(Message::Ui(UiMsg::ResolveCharacter(true))),
                        button("Replace")
                            .style(theme::secondary)
                            .on_press(Message::Ui(UiMsg::ResolveCharacter(false))),
                        button("Cancel")
                            .style(theme::secondary)
                            .on_press(Message::Ui(UiMsg::CancelCharacterImport)),
                    ]
                    .spacing(8),
                ]
                .spacing(10),
            )
            .padding([10, 12])
            .style(theme::fill(theme::AMBER_BG, 8.0)),
        );
    }

    let eyes: Element<'a, Message> = if selected.is_some_and(|c| c.is_builtin()) {
        check("Eyes follow the cursor", s.bool(mascot::EYES), mascot::EYES)
    } else {
        hint("Eyes follow the cursor when a character has separate eye art.")
    };
    page = page
        .push(section(vec![
            check(
                "Show on the desktop",
                s.bool(mascot::ENABLED),
                mascot::ENABLED,
            ),
            hint(format!(
                "Right-click {name} for its menu, click to see it react, and drag to move it. It stays while annotation is off and steps aside while you draw."
            )),
        ]))
        .push(section(vec![
            row![
                h2("Size"),
                space().width(Length::Fill),
                text(format!("{size:.0} px tall")).size(13)
            ]
            .into(),
            slider(v.registry.range(mascot::SIZE), size, |x| {
                Message::Set(mascot::SIZE, Value::Float(x))
            })
            .step(4.0)
            .style(theme::range)
            .into(),
        ]))
        .push(eyes)
        .push(check(
            "Idle motion and blinking",
            s.bool(mascot::IDLE),
            mascot::IDLE,
        ))
        .push(
            row![
                hint("Reduced motion also stops idle motion and blinking."),
                button(text("Motion settings").size(12).color(theme::CYAN_TEXT))
                    .padding([2, 4])
                    .style(theme::tool_button(theme::Tone::Plain))
                    .on_press(Message::Ui(UiMsg::Nav(Page::Controls))),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .push(
            button(text("Reset position").size(13))
                .padding([7, 12])
                .style(theme::secondary)
                .on_press_maybe(
                    s.bool(mascot::ENABLED)
                        .then_some(Message::Ui(UiMsg::ResetMascotPosition)),
                ),
        );
    if let Some(m) = v.motion {
        page = page
            .push(motion_section(m))
            .push(lines_section(v, m))
            .push(music_section(v));
    }

    let side = match (v.character_draft, v.motion) {
        (Some(d), _) => Some(new_character(d, v.character_busy)),
        (None, Some(m)) => Some(motion_preview(m)),
        (None, None) => None,
    };
    (page.into(), side)
}

/// The Motion tab: a preset, strength and speed for each moving part.
fn motion_section<'a>(m: &'a crate::mascot::preview::MotionPreview) -> Element<'a, Message> {
    let mut rows = column![].spacing(14);
    for (i, (label, spec)) in m.labels.iter().zip(&m.specs).enumerate() {
        let kind = m.kinds[i];
        let number = |value: f32| text(format!("{value:.2}")).size(12).color(theme::MUTED).width(34);
        rows = rows.push(
            column![
                row![
                    text(label.clone()).font(theme::MEDIUM).size(13).width(Length::Fill),
                    pick_list(kind.presets(), Some(spec.preset), move |p| {
                        Message::Ui(UiMsg::MotionPreset(i, p))
                    })
                    .text_size(13)
                    .padding([6, 10])
                    .style(theme::select),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
                row![
                    text("Strength").size(12).color(theme::MUTED).width(64),
                    slider(0.0..=2.0, spec.strength, move |x| Message::Ui(
                        UiMsg::MotionStrength(i, x)
                    ))
                    .step(0.05_f32)
                    .style(theme::range),
                    number(spec.strength),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
                row![
                    text("Speed").size(12).color(theme::MUTED).width(64),
                    slider(0.0..=2.0, spec.speed, move |x| Message::Ui(UiMsg::MotionSpeed(i, x)))
                        .step(0.05_f32)
                        .style(theme::range),
                    number(spec.speed),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            ]
            .spacing(6),
        );
    }
    section(vec![
        row![
            h2("Motion"),
            space().width(Length::Fill),
            button(text("Reset to pack").size(12))
                .padding([5, 10])
                .style(theme::secondary)
                .on_press_maybe(m.customised().then_some(Message::Ui(UiMsg::MotionReset))),
            button(text("Save motion").font(theme::MEDIUM).size(12))
                .padding([5, 12])
                .style(theme::primary)
                .on_press_maybe(m.dirty().then_some(Message::Ui(UiMsg::MotionSave))),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
        hint("Pick how each part moves. Strength and Speed run from 0 to 2; 1 is the preset as designed. Changes show in the preview straight away and on the desktop once saved."),
        rows.into(),
    ])
}

/// The Lines tab: what the character says, when, and how it sounds.
fn lines_section<'a>(
    v: &SettingsView<'a>,
    m: &'a crate::mascot::preview::MotionPreview,
) -> Element<'a, Message> {
    use crate::mascot::lines::{MAX_WIDTH, MIN_WIDTH, Trigger};
    let s = v.session.settings();
    let small = |label: &'static str, msg: Message| {
        button(text(label).size(12))
            .padding([5, 10])
            .style(theme::secondary)
            .on_press(msg)
    };
    let mut groups = column![].spacing(16);
    for t in Trigger::ALL {
        let mut group = column![
            row![
                text(t.label()).font(theme::MEDIUM).size(13).width(Length::Fill),
                small("Add line", Message::Ui(UiMsg::LineAdd(t))),
            ]
            .align_y(Alignment::Center)
        ]
        .spacing(6);
        for (i, line) in m.lines.get(t).iter().enumerate() {
            let mut row_ = row![
                iced::widget::text_input("What it says; // splits it into parts", &line.text)
                    .on_input(move |text| Message::Ui(UiMsg::LineText(t, i, text)))
                    .size(13)
                    .padding([7, 9])
                    .width(Length::Fill)
                    .style(|_, status| iced::widget::text_input::Style {
                        background: theme::CONTROL.into(),
                        border: iced::Border {
                            radius: 8.0.into(),
                            width: 1.0,
                            color: if matches!(
                                status,
                                iced::widget::text_input::Status::Focused { .. }
                            ) {
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
                small(
                    if line.sound.is_some() { "Sound ✓" } else { "Sound…" },
                    Message::Ui(UiMsg::LineSound(t, i)),
                ),
            ]
            .spacing(6)
            .align_y(Alignment::Center);
            if line.sound.is_some() {
                row_ = row_.push(
                    button(text("No sound").size(12))
                        .padding([5, 8])
                        .style(theme::tool_button(theme::Tone::Plain))
                        .on_press(Message::Ui(UiMsg::LineClearSound(t, i))),
                );
            }
            row_ = row_.push(
                button(text("×").size(13))
                    .padding([5, 9])
                    .style(theme::tool_button(theme::Tone::Plain))
                    .on_press(Message::Ui(UiMsg::LineRemove(t, i))),
            );
            group = group.push(row_);
        }
        groups = groups.push(group);
    }

    let swatches = |colours: [[u8; 4]; 4], current: [u8; 4], msg: fn([u8; 4]) -> UiMsg| {
        row(colours.into_iter().map(move |c| {
            button(space().width(24).height(24))
                .padding(0)
                .style(theme::swatch(theme::rgba(c), c == current))
                .on_press(Message::Ui(msg(c)))
                .into()
        }))
        .spacing(6)
    };
    let b = m.lines.bubble;
    let minutes = |x: f64| format!("{x:.0} min");
    section(vec![
        row![
            h2("Lines"),
            space().width(Length::Fill),
            button(text("Reset to pack").size(12))
                .padding([5, 10])
                .style(theme::secondary)
                .on_press_maybe(m.lines_customised().then_some(Message::Ui(UiMsg::LinesReset))),
            button(text("Save lines").font(theme::MEDIUM).size(12))
                .padding([5, 12])
                .style(theme::primary)
                .on_press_maybe(m.lines_dirty().then_some(Message::Ui(UiMsg::LinesSave))),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
        hint("A line is picked at random, never the same twice in a row, and shows in a bubble above the character. Try them with Speak in the preview."),
        check("Show speech lines", s.bool(mascot::LINES), mascot::LINES),
        check(
            "Quiet: no idle or music lines",
            s.bool(mascot::QUIET_LINES),
            mascot::QUIET_LINES,
        ),
        row![
            text("Idle lines every").size(12).color(theme::MUTED).width(110),
            slider(1.0..=30.0, s.float(mascot::IDLE_MIN), |x| Message::Set(
                mascot::IDLE_MIN,
                Value::Float(x)
            ))
            .step(1.0)
            .style(theme::range),
            text(minutes(s.float(mascot::IDLE_MIN))).size(12).width(48),
            text("to").size(12).color(theme::MUTED),
            slider(1.0..=30.0, s.float(mascot::IDLE_MAX), |x| Message::Set(
                mascot::IDLE_MAX,
                Value::Float(x)
            ))
            .step(1.0)
            .style(theme::range),
            text(minutes(s.float(mascot::IDLE_MAX))).size(12).width(48),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
        row![
            text("Character volume").size(12).color(theme::MUTED).width(110),
            slider(0.0..=1.0, s.float(mascot::VOICE_VOLUME), |x| Message::Set(
                mascot::VOICE_VOLUME,
                Value::Float(x)
            ))
            .step(0.05)
            .style(theme::range),
            text(format!("{:.0}%", s.float(mascot::VOICE_VOLUME) * 100.0))
                .size(12)
                .width(48),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
        hint("Line sounds play at Character volume, separate from the pointer's tap sound."),
        groups.into(),
        row![
            text("Bubble").size(12).color(theme::MUTED).width(110),
            swatches(
                [
                    [255, 255, 255, 255],
                    [255, 247, 224, 255],
                    [27, 27, 31, 255],
                    [34, 211, 238, 255],
                ],
                b.background,
                UiMsg::BubbleBackground,
            ),
            text("Text").size(12).color(theme::MUTED),
            swatches(
                [
                    [27, 27, 31, 255],
                    [255, 255, 255, 255],
                    [14, 116, 144, 255],
                    [180, 83, 9, 255],
                ],
                b.text,
                UiMsg::BubbleText,
            ),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into(),
        row![
            text("Bubble width").size(12).color(theme::MUTED).width(110),
            slider(MIN_WIDTH..=MAX_WIDTH, b.max_width, |x| Message::Ui(UiMsg::BubbleWidth(x)))
                .step(10.0_f32)
                .style(theme::range),
            text(format!("{:.0} px", b.max_width)).size(12).width(48),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
    ])
}

/// The Music tab: react to playback, how strongly, and a live meter.
fn music_section<'a>(v: &SettingsView<'a>) -> Element<'a, Message> {
    let s = v.session.settings();
    let (status, level) = (v.music.0.clone(), v.music.1);
    let on = s.bool(mascot::MUSIC);
    let meter = container(
        container(space())
            .width(Length::FillPortion((level.clamp(0.0, 1.0) * 1000.0) as u16 + 1))
            .height(8)
            .style(theme::fill(theme::CYAN_TEXT, 4.0)),
    )
    .width(Length::Fill)
    .height(8)
    .style(theme::fill(theme::WELL, 4.0));
    let meter = row![
        meter,
        space().width(Length::FillPortion(((1.0 - level.clamp(0.0, 1.0)) * 1000.0) as u16 + 1)),
    ];
    section(vec![
        h2("Music"),
        check("React to playback", on, mascot::MUSIC),
        hint("Moves with the loudness of what's playing on this PC. TinyPaw's own sounds are left out, and nothing is recorded."),
        row![
            text("Listen to").size(12).color(theme::MUTED).width(110),
            pick_list(v.music_sources.clone(), Some(v.music_source.clone()), |s| Message::Set(
                mascot::MUSIC_SOURCE,
                Value::Choice(crate::platform::intern(&s.id()))
            ))
            .text_size(13)
            .padding([6, 10])
            .style(theme::select),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into(),
        hint(match v.music_source {
            crate::mascot::music::Source::All => "All apps together. Pick one app so a game or a call doesn't count; the list shows apps that are playing.",
            crate::mascot::music::Source::App(_) => "Only this app counts, and Follow beat hears only its sound.",
            crate::mascot::music::Source::Input(_) => "For music that doesn't play on this PC. This uses the input device (Windows shows the microphone in use) while listening; nothing is recorded.",
        }),
        row![
            text("Playback").size(12).color(theme::MUTED).width(110),
            container(meter).width(Length::Fill),
            text(status).size(12).color(theme::MUTED),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into(),
        row![
            text("Music strength").size(12).color(theme::MUTED).width(110),
            slider(0.0..=2.0, s.float(mascot::MUSIC_STRENGTH), |x| Message::Set(
                mascot::MUSIC_STRENGTH,
                Value::Float(x)
            ))
            .step(0.05)
            .style(theme::range),
            text(format!("{:.2}", s.float(mascot::MUSIC_STRENGTH))).size(12).width(40),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
        row![
            text("Bop style").size(12).color(theme::MUTED).width(110),
            pick_list(
                crate::mascot::motion::Bop::ALL,
                v.motion.map(|m| m.rig().bop()),
                |b| Message::Ui(UiMsg::MusicBop(b)),
            )
            .text_size(13)
            .padding([6, 10])
            .style(theme::select),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into(),
        hint("Nod is relaxed. Groove dips on the beat and leans side to side. Rubber hose hops every beat, squashing and stretching like an old cartoon. Saved for this character, and carried when you export it."),
        check("Follow beat (trial)", s.bool(mascot::BEAT), mascot::BEAT),
        hint("Nods land on the beat instead of a steady bob. It listens to what's playing (TinyPaw's own sounds are held off while they play) and keeps nothing."),
        row![
            text("Beat").size(12).color(theme::MUTED).width(110),
            text(v.beat.clone()).size(12).color(theme::MUTED).width(Length::Fill),
        ]
        .align_y(Alignment::Center)
        .into(),
        row![
            text("Sensitivity").size(12).color(theme::MUTED).width(110),
            pick_list(
                [Choice("low", "Low"), Choice("normal", "Normal"), Choice("high", "High")],
                Some(Choice::of(s.choice(mascot::BEAT_SENSITIVITY), &[("low", "Low"), ("normal", "Normal"), ("high", "High")])),
                |c| Message::Set(mascot::BEAT_SENSITIVITY, Value::Choice(c.0)),
            )
            .text_size(13)
            .padding([6, 10])
            .style(theme::select),
            text("Timing").size(12).color(theme::MUTED),
            pick_list(
                [Choice("earlier", "Earlier"), Choice("default", "Default"), Choice("later", "Later")],
                Some(Choice::of(s.choice(mascot::BEAT_TIMING), &[("earlier", "Earlier"), ("default", "Default"), ("later", "Later")])),
                |c| Message::Set(mascot::BEAT_TIMING, Value::Choice(c.0)),
            )
            .text_size(13)
            .padding([6, 10])
            .style(theme::select),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into(),
        hint("Try it without music using Music in the preview."),
    ])
}

/// A setting value with its display label, for pick lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Choice(&'static str, &'static str);

impl Choice {
    fn of(value: &'static str, all: &[(&'static str, &'static str)]) -> Self {
        let label = all.iter().find(|(v, _)| *v == value).map_or(value, |(_, l)| l);
        Choice(value, label)
    }
}

impl std::fmt::Display for Choice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.1)
    }
}

/// The selected character, animated, with buttons to try its reactions.
fn motion_preview<'a>(m: &'a crate::mascot::preview::MotionPreview) -> Element<'a, Message> {
    use crate::mascot::preview::Test;
    let test = |label: &'static str, t: Test| {
        button(text(label).size(12).width(Length::Fill).center())
            .width(Length::Fill)
            .padding([7, 8])
            .style(theme::secondary)
            .on_press(Message::Ui(UiMsg::MotionTest(t)))
    };
    let b = m.lines.bubble;
    let bubble: Element<'a, Message> = match m.speech() {
        Some(said) => container(
            text(said.to_owned())
                .size(13)
                .color(Color::from_rgba8(b.text[0], b.text[1], b.text[2], 1.0)),
        )
        .padding([8, 12])
        .max_width(b.max_width.min(260.0))
        .style(move |_: &Theme| container::Style {
            background: Some(Color::from_rgba8(b.background[0], b.background[1], b.background[2], 1.0).into()),
            border: iced::Border {
                radius: 14.0.into(),
                width: 1.0,
                color: Color::from_rgba8(0, 0, 0, 0.18),
            },
            ..container::Style::default()
        })
        .into(),
        None => space().height(34).into(),
    };
    column![
        h2("Preview"),
        container(bubble).width(Length::Fill).center_x(Length::Fill),
        canvas(MotionStage { preview: m })
            .width(Length::Fill)
            .height(300),
        row![
            test("Click", Test::Click),
            test("Drag ←", Test::DragLeft),
            test("Drag →", Test::DragRight),
            test("Speak", Test::Speak),
            test("Music", Test::Music),
        ]
        .spacing(6),
        hint("Idle motion follows Settings → Character. Reduced motion stops it, as on the desktop."),
    ]
    .spacing(10)
    .width(260)
    .into()
}

struct MotionStage<'a> {
    preview: &'a crate::mascot::preview::MotionPreview,
}

impl canvas::Program<Message> for MotionStage<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        use iced::widget::canvas::{Frame, Path};
        let size = bounds.size();
        let mut frame = Frame::new(renderer, size);
        frame.fill(
            &Path::rounded_rectangle(iced::Point::ORIGIN, size, 12.0.into()),
            theme::WELL,
        );
        let rig = self.preview.rig();
        let pose = self.preview.pose();
        let canvas_size = rig.size();
        let pad = 18.0;
        let k = ((size.width - pad * 2.0) / canvas_size.width)
            .min((size.height - pad * 2.0) / canvas_size.height);
        let feet = rig.anchor();
        // Stand the feet on a floor line near the bottom, centred.
        let origin = iced::Vector::new(size.width / 2.0 - feet.x * k, size.height - pad - feet.y * k);
        frame.translate(origin);
        frame.scale(k);
        crate::mascot::motion::place(&mut frame, pose, feet, canvas_size.height);
        rig.draw_pose(&mut frame, pose);
        vec![frame.into_geometry()]
    }
}

/// The Art tab: a character's body, moving parts, head faces and eyes.
fn art_page<'a>(e: &'a crate::mascot::editor::Editor, busy: bool) -> Element<'a, Message> {
    if busy {
        return column![
            h1("Preparing artwork…"),
            hint("Your draft stays available if preparation fails."),
            button("Cancel")
                .style(theme::secondary)
                .on_press(Message::Ui(UiMsg::ArtCancel))
        ]
        .spacing(12)
        .into();
    }
    use crate::mascot::editor::{EyeSlot, FACE_NAMES, Selected, Target};
    use crate::mascot::motion::PartType;
    let small = |label: String, msg: Message| {
        button(text(label).size(12))
            .padding([5, 10])
            .style(theme::secondary)
            .on_press(msg)
    };
    let thumb = |h: &iced::advanced::image::Handle| -> Element<'a, Message> {
        container(iced::widget::image(h.clone()).height(40).width(40))
            .width(48)
            .height(48)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(theme::fill(theme::WELL, 8.0))
            .into()
    };
    let seg = |label: &'static str, on: bool, msg: Message| -> Element<'a, Message> {
        button(text(label).size(12))
            .padding([5, 10])
            .style(theme::segment(on))
            .on_press(msg)
            .into()
    };

    // What the preview's clicks move.
    let mut targets = vec![
        seg(
            "Body",
            e.selected == Selected::Body,
            Message::Ui(UiMsg::ArtSelect(Selected::Body)),
        ),
        seg(
            "Ground",
            e.selected == Selected::Ground,
            Message::Ui(UiMsg::ArtSelect(Selected::Ground)),
        ),
    ];
    for (i, p) in e.parts.iter().enumerate() {
        targets.push(
            button(text(format!("{} {}", i + 1, p.kind)).size(12))
                .padding([5, 10])
                .style(theme::segment(e.selected == Selected::Part(i)))
                .on_press(Message::Ui(UiMsg::ArtSelect(Selected::Part(i))))
                .into(),
        );
    }
    let targets = container(row(targets).spacing(4).wrap().vertical_spacing(4))
        .padding(4)
        .style(theme::fill(theme::WELL, 10.0));

    let head = e.head();
    let mut parts = column![].spacing(12);
    for (i, p) in e.parts.iter().enumerate() {
        let mut line = row![
            text(format!("{}", i + 1))
                .size(13)
                .color(theme::MUTED)
                .width(16),
            thumb(&p.layer.handle),
            pick_list(PartType::ALL, Some(p.kind), move |k| Message::Ui(
                UiMsg::ArtKind(i, k)
            ))
            .text_size(13)
            .padding([6, 10])
            .style(theme::select),
            container(
                row![
                    seg("Back", p.behind, Message::Ui(UiMsg::ArtBehind(i, true))),
                    seg("Front", !p.behind, Message::Ui(UiMsg::ArtBehind(i, false))),
                ]
                .spacing(2)
            )
            .padding(2)
            .style(theme::fill(theme::WELL, 8.0)),
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        if head.is_some() && p.kind != PartType::Head {
            line = line.push(
                checkbox(p.on_head)
                    .label("On head")
                    .text_size(13)
                    .size(15)
                    .on_toggle(move |on| Message::Ui(UiMsg::ArtOnHead(i, on))),
            );
        }
        line = line
            .push(space().width(Length::Fill))
            .push(small("Remove".into(), Message::Ui(UiMsg::ArtRemovePart(i))));
        line = line
            .push(small(
                "Lower".into(),
                Message::Ui(UiMsg::ArtOrder(i, false)),
            ))
            .push(small("Raise".into(), Message::Ui(UiMsg::ArtOrder(i, true))));
        let mut block = column![line.wrap()].spacing(8);
        if p.kind == PartType::Head {
            let mut faces = row![hint("Faces:")].spacing(6).align_y(Alignment::Center);
            for (slot, name) in FACE_NAMES.iter().enumerate() {
                faces = faces.push(small(
                    if p.faces[slot].is_some() {
                        format!("{name} ✓")
                    } else {
                        format!("{name}…")
                    },
                    Message::Ui(UiMsg::ArtPick(Target::Face(i, slot))),
                ));
                if p.faces[slot].is_some() {
                    faces = faces.push(small(
                        format!("Arrange {name}"),
                        Message::Ui(UiMsg::ArtSelect(Selected::Face(i, slot))),
                    ));
                    faces = faces.push(
                        button(text("×").size(12))
                            .padding([5, 8])
                            .style(theme::tool_button(theme::Tone::Plain))
                            .on_press(Message::Ui(UiMsg::ArtRemoveFace(i, slot))),
                    );
                }
            }
            block = block.push(faces.wrap());
        }
        parts = parts.push(block);
    }

    let mut eyes = column![].spacing(10);
    for (i, eye) in e.eyes.iter().enumerate() {
        let slot = |label: &str, set: bool, which: EyeSlot| {
            small(
                if set {
                    format!("{label} ✓")
                } else {
                    format!("{label}…")
                },
                Message::Ui(UiMsg::ArtPick(Target::Eye(i, which))),
            )
        };
        let mut images = row![
            text(format!("Eye {}", i + 1)).size(13).width(48),
            slot("White", eye.white.is_some(), EyeSlot::White),
            slot("Pupil", eye.pupil.is_some(), EyeSlot::Pupil),
            slot("Mask", eye.mask.is_some(), EyeSlot::Mask),
            slot("Blink", eye.blink.is_some(), EyeSlot::Blink),
        ]
        .spacing(6)
        .align_y(Alignment::Center);
        if eye.blink.is_some() {
            images = images.push(
                button(text("×").size(12))
                    .padding([5, 8])
                    .style(theme::tool_button(theme::Tone::Plain))
                    .on_press(Message::Ui(UiMsg::ArtRemoveBlink(i))),
            );
        }
        images = images
            .push(space().width(Length::Fill))
            .push(small("Remove".into(), Message::Ui(UiMsg::ArtRemoveEye(i))));
        eyes = eyes.push(
            column![
                images.wrap(),
                small("Keep static layers".into(),Message::Ui(UiMsg::ArtStaticEye(i))),
                hint("Static conversion keeps the white and pupil artwork as head layers; the tracking mask and blink are removed."),
                row![
                    hint("Pupil travel"),
                    slider(0.0..=0.05, eye.travel, move |x| Message::Ui(
                        UiMsg::ArtTravel(i, x)
                    ))
                    .step(0.001_f32)
                    .style(theme::range),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            ]
            .spacing(6),
        );
    }

    let eyes_section: Element<'a, Message> = if head.is_some() || !e.eyes.is_empty() {
        section(vec![
            h2("Eyes"),
            hint(
                "Complete eyes and eyebrows work as ordinary parts: choose Other, Still, and On head. Tracking is optional; it requires separate white, pupil and mask images. Blink is optional. Remove an unfinished tracking eye to keep using static layers.",
            ),
            eyes.into(),
            small("Add eye".into(), Message::Ui(UiMsg::ArtAddEye))
                .padding([7, 12])
                .into(),
        ])
    } else {
        hint(
            "Add a head part to give the character eyes that follow the cursor and faces for reactions.",
        )
    };

    column![
        row![
            h1(if e.new_install { "Create from parts" } else { "Edit art" }),
            space().width(Length::Fill),
            button("Undo").style(theme::secondary).on_press_maybe(e.can_undo().then_some(Message::Ui(UiMsg::ArtUndo))),
            button("Redo").style(theme::secondary).on_press_maybe(e.can_redo().then_some(Message::Ui(UiMsg::ArtRedo))),
            button("Cancel")
                .style(theme::secondary)
                .on_press(Message::Ui(UiMsg::ArtCancel)),
            button(text("Save").font(theme::HEADING).size(13))
                .padding([8, 16])
                .style(theme::primary)
                .on_press_maybe(
                    (!e.name.trim().is_empty() && e.validation_error().is_none()).then_some(Message::Ui(UiMsg::ArtSave))
                ),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        iced::widget::text_input("Name", &e.name)
            .on_input(|n| Message::Ui(UiMsg::ArtName(n)))
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
        section(vec![
            h2("Body"),
            row![
                thumb(&e.body.handle),
                small("Replace body…".into(), Message::Ui(UiMsg::ArtPick(Target::Body))),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
            hint(e.canvas_hint()),
            canvas_controls(e),
            hint("To give it a moving tail, use a body with the painted tail removed and add the tail as a part."),
        ]),
        section(vec![
            h2("Arrange and attach"),
            row![seg("Arrange artwork",e.mode==crate::mascot::editor::Mode::Arrange,Message::Ui(UiMsg::ArtMode(crate::mascot::editor::Mode::Arrange))),
                seg("Set attachment point",e.mode==crate::mascot::editor::Mode::Pivot,Message::Ui(UiMsg::ArtMode(crate::mascot::editor::Mode::Pivot)))].spacing(4).into(),
            targets.into(),
            placement_controls(e),
            hint("Ground is where the feet stand. A part turns around its pivot: the neck for a head, the root for a tail or ear."),
        ]),
        section(vec![
            h2("Moving parts"),
            parts.into(),
            small(
                "Add part PNGs…".into(),
                Message::Ui(UiMsg::ArtPick(Target::NewPart))
            )
            .padding([7, 12])
            .into(),
            hint("Ears and hair ride on the head. Back draws behind the body (or behind the head when on it)."),
        ]),
        eyes_section,
        hint(e.validation_error().unwrap_or_else(||"Drag to arrange; arrow keys nudge one pixel, Shift nudges ten. Artwork is resampled from its original source only when saved.".into())),
        hint("Motion presets, strength and speed come in the Motion tab, next."),
    ]
    .spacing(22)
    .max_width(640)
    .into()
}

fn canvas_controls<'a>(e: &'a crate::mascot::editor::Editor) -> Element<'a, Message> {
    if !e.can_resize_canvas() {
        return hint(
            "Canvas size is shared with other poses; resize is available for single-pose packs.",
        );
    }
    row![
        hint("Stage (1–2048 px)"),
        iced::widget::text_input("Width", &e.canvas_fields[0])
            .on_input(|s| Message::Ui(UiMsg::ArtCanvas(0, s)))
            .width(90)
            .padding(6)
            .size(13),
        text("×").size(13),
        iced::widget::text_input("Height", &e.canvas_fields[1])
            .on_input(|s| Message::Ui(UiMsg::ArtCanvas(1, s)))
            .width(90)
            .padding(6)
            .size(13)
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

fn placement_controls<'a>(e: &'a crate::mascot::editor::Editor) -> Element<'a, Message> {
    if e.mode != crate::mascot::editor::Mode::Arrange || e.selected_layer().is_none() {
        return hint("Choose a layer and Arrange artwork to move, scale or rotate it.");
    }
    let fields = row(["X (px)", "Y (px)", "Scale (%)", "Angle (°)"]
        .into_iter()
        .enumerate()
        .map(|(i, label)| {
            column![
                hint(label),
                iced::widget::text_input("0", &e.fields[i])
                    .on_input(move |s| Message::Ui(UiMsg::ArtField(i, s)))
                    .padding(6)
                    .size(13)
            ]
            .width(100)
            .into()
        }))
    .spacing(8)
    .wrap();
    column![
        fields,
        row![
            button("←").on_press(Message::Ui(UiMsg::ArtNudge(-1.0, 0.0))),
            button("↑").on_press(Message::Ui(UiMsg::ArtNudge(0.0, -1.0))),
            button("↓").on_press(Message::Ui(UiMsg::ArtNudge(0.0, 1.0))),
            button("→").on_press(Message::Ui(UiMsg::ArtNudge(1.0, 0.0))),
            hint("Nudge one canvas pixel")
        ]
        .spacing(6)
        .align_y(Alignment::Center)
    ]
    .spacing(8)
    .into()
}

/// The assembled character at rest, with the ground and pivot markers.
fn art_preview<'a>(e: &'a crate::mascot::editor::Editor, busy: bool) -> Element<'a, Message> {
    use crate::mascot::editor::Selected;
    let placing = match e.selected {
        Selected::Ground => "Placing: ground".to_owned(),
        Selected::Body => "Selected: body".to_owned(),
        Selected::Face(i, slot) => format!(
            "Selected: head {} {}",
            i + 1,
            crate::mascot::editor::FACE_NAMES[slot]
        ),
        Selected::Part(i) => format!(
            "Placing: pivot of {} {}",
            i + 1,
            e.parts.get(i).map_or("part".into(), |p| p.kind.to_string())
        ),
    };
    column![
        h2("Preview"),
        row![button("Light").style(theme::segment(e.background==crate::mascot::editor::Background::Light)).on_press(Message::Ui(UiMsg::ArtBackground(crate::mascot::editor::Background::Light))),
            button("Dark").style(theme::segment(e.background==crate::mascot::editor::Background::Dark)).on_press(Message::Ui(UiMsg::ArtBackground(crate::mascot::editor::Background::Dark))),
            button("Grid").style(theme::segment(e.background==crate::mascot::editor::Background::Checker)).on_press(Message::Ui(UiMsg::ArtBackground(crate::mascot::editor::Background::Checker)))].spacing(4),
        row![hint(format!("Zoom {:.0}%",e.zoom*100.0)),slider(0.5..=3.0,e.zoom,|z|Message::Ui(UiMsg::ArtZoom(z))).step(0.1_f32).style(theme::range)].spacing(8),
        canvas(ArtPreview { editor: e,busy })
            .width(Length::Fill)
            .height(340),
        hint(placing),
        hint("Drag visible artwork to arrange it. Attachment mode moves only the joint. Selecting a face previews that replacement."),
        hint(if e.clipped() { "Some source images extend outside the stage and will be clipped when saved." } else { "Artwork fits inside the stage." }),
    ]
    .spacing(10)
    .width(300)
    .into()
}

struct ArtPreview<'a> {
    editor: &'a crate::mascot::editor::Editor,
    busy: bool,
}

#[derive(Default)]
struct ArtPreviewState {
    drag: Option<iced::Point>,
}

impl ArtPreview<'_> {
    fn image_rect(&self, bounds: iced::Size) -> iced::Rectangle {
        let pad = 14.0;
        let (w, h) = (self.editor.canvas.0 as f32, self.editor.canvas.1 as f32);
        let fit = ((bounds.width - pad * 2.0) / w).min((bounds.height - pad * 2.0) / h)
            * self.editor.zoom;
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

impl canvas::Program<Message> for ArtPreview<'_> {
    type State = ArtPreviewState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &iced::Event,
        bounds: iced::Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        use crate::mascot::editor::{Mode, Selected};
        use iced::mouse::{Button, Event};
        if self.busy {
            state.drag = None;
            return None;
        }
        let r = self.image_rect(bounds.size());
        match event {
            iced::Event::Mouse(Event::ButtonReleased(Button::Left))
                if state.drag.take().is_some() =>
            {
                Some(canvas::Action::capture())
            }
            iced::Event::Mouse(Event::CursorMoved { .. }) if state.drag.is_some() => {
                let p = cursor.position()?;
                let local = iced::Point::new(p.x - bounds.x, p.y - bounds.y);
                let old = state.drag.replace(local)?;
                let scale = self.editor.canvas.0 as f32 / r.width;
                Some(
                    canvas::Action::publish(Message::Ui(UiMsg::ArtDrag(
                        (local.x - old.x) * scale,
                        (local.y - old.y) * scale,
                    )))
                    .and_capture(),
                )
            }
            iced::Event::Mouse(Event::ButtonPressed(Button::Left)) => {
                let p = cursor.position_in(bounds)?;
                if !r.contains(p) {
                    return Some(canvas::Action::capture());
                }
                let (x, y) = ((p.x - r.x) / r.width, (p.y - r.y) / r.height);
                let msg = if self.editor.mode == Mode::Pivot
                    || self.editor.selected == Selected::Ground
                {
                    UiMsg::ArtClick(x, y)
                } else {
                    let target = self.editor.hit_target(x, y)?;
                    state.drag = Some(p);
                    UiMsg::ArtDragStart(target)
                };
                Some(canvas::Action::publish(Message::Ui(msg)).and_capture())
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        use crate::mascot::editor::Selected;
        use iced::widget::canvas::{Frame, Path, Stroke};
        let e = self.editor;
        let size = bounds.size();
        let mut frame = Frame::new(renderer, size);
        use crate::mascot::editor::Background;
        let base = match e.background {
            Background::Light => Color::from_rgb8(244, 244, 245),
            Background::Dark => Color::from_rgb8(27, 38, 33),
            Background::Checker => Color::from_rgb8(190, 190, 196),
        };
        frame.fill_rectangle(iced::Point::ORIGIN, size, base);
        if e.background == Background::Checker {
            for y in 0..(size.height / 16.0).ceil() as u32 {
                for x in 0..(size.width / 16.0).ceil() as u32 {
                    if (x + y) % 2 == 0 {
                        frame.fill_rectangle(
                            iced::Point::new(x as f32 * 16.0, y as f32 * 16.0),
                            iced::Size::new(16.0, 16.0),
                            Color::from_rgb8(225, 225, 230),
                        );
                    }
                }
            }
        }
        let r = self.image_rect(size);
        let clip = r
            .intersection(&iced::Rectangle::new(iced::Point::ORIGIN, size))
            .unwrap_or_default();
        frame.with_clip(clip, |frame| {
            let fit = r.width / e.canvas.0 as f32;
            for (_, layer) in e.draw_layers() {
                let d = layer.image.dimensions();
                let p = layer.placement;
                frame.with_save(|f| {
                    f.translate(iced::Vector::new(
                        r.x + (p.x + d.0 as f32 * p.scale / 2.0) * fit,
                        r.y + (p.y + d.1 as f32 * p.scale / 2.0) * fit,
                    ));
                    f.rotate(iced::Radians(p.angle.to_radians()));
                    f.scale(fit * p.scale);
                    f.draw_image(
                        iced::Rectangle::new(
                            iced::Point::new(-(d.0 as f32) / 2.0, -(d.1 as f32) / 2.0),
                            iced::Size::new(d.0 as f32, d.1 as f32),
                        ),
                        canvas::Image::new(layer.handle.clone()),
                    );
                });
            }
        });
        frame.stroke(
            &Path::rectangle(r.position(), r.size()),
            Stroke::default().with_color(theme::MUTED).with_width(1.0),
        );
        if let Some(layer) = e.selected_layer() {
            let d = layer.image.dimensions();
            let corners = [
                (0.0, 0.0),
                (d.0 as f32, 0.0),
                (d.0 as f32, d.1 as f32),
                (0.0, d.1 as f32),
            ];
            let outline = Path::new(|path| {
                for (i, p) in corners.into_iter().enumerate() {
                    let p = layer.placement.map(p, d);
                    let p = iced::Point::new(
                        r.x + p.0 * r.width / e.canvas.0 as f32,
                        r.y + p.1 * r.height / e.canvas.1 as f32,
                    );
                    if i == 0 {
                        path.move_to(p);
                    } else {
                        path.line_to(p);
                    }
                }
                path.close();
            });
            frame.stroke(
                &outline,
                Stroke::default()
                    .with_color(theme::CYAN_TEXT)
                    .with_width(1.5),
            );
        }
        let at = |(x, y): (f32, f32)| iced::Point::new(r.x + x * r.width, r.y + y * r.height);
        let ring = Stroke::default().with_color(Color::WHITE).with_width(1.5);
        let ground = at(e.ground);
        let amber = Color::from_rgb8(0xF5, 0x9E, 0x0B);
        frame.stroke(
            &Path::line(
                iced::Point::new(r.x, ground.y),
                iced::Point::new(r.x + r.width, ground.y),
            ),
            Stroke::default()
                .with_color(Color { a: 0.7, ..amber })
                .with_width(1.0),
        );
        let ground_r = if e.selected == Selected::Ground {
            7.0
        } else {
            5.0
        };
        frame.fill(&Path::circle(ground, ground_r), amber);
        frame.stroke(&Path::circle(ground, ground_r), ring);
        for (i, p) in e.parts.iter().enumerate() {
            let on = e.selected == Selected::Part(i);
            let c = at(p.pivot);
            let radius = if on { 7.0 } else { 5.0 };
            let fill = if on {
                Color::from_rgb8(0x22, 0xD3, 0xEE)
            } else {
                Color::from_rgba8(0x22, 0xD3, 0xEE, 0.55)
            };
            frame.fill(&Path::circle(c, radius), fill);
            frame.stroke(&Path::circle(c, radius), ring);
            frame.fill_text(canvas::Text {
                content: format!("{}", i + 1),
                position: iced::Point::new(c.x + 9.0, c.y - 8.0),
                color: Color::WHITE,
                size: 12.0.into(),
                ..canvas::Text::default()
            });
        }
        vec![frame.into_geometry()]
    }
}

/// "Add character": name it, check it on light and dark, click its feet.
fn new_character<'a>(draft: &'a crate::mascot::pack::Draft, busy: bool) -> Element<'a, Message> {
    column![
        h2("New character"),
        canvas(GroundPicker { draft })
            .width(Length::Fill)
            .height(260),
        hint("Click where it stands. The marker is suggested from the bottom of the art."),
        iced::widget::text_input("Name", &draft.name)
            .on_input(|n| Message::Ui(UiMsg::CharacterName(n)))
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
        hint("A single image reacts and sways as one piece. For moving parts, import a character folder with separate head, tail or ear PNGs."),
        row![
            button(text("Cancel").size(13).width(Length::Fill).center())
                .width(Length::Fill)
                .padding([10, 12])
                .style(theme::secondary)
                .on_press(Message::Ui(UiMsg::CancelCharacter)),
            button(
                text(if busy { "Installing…" } else { "Use character" })
                    .font(theme::HEADING)
                    .size(13)
                    .width(Length::Fill)
                    .center()
            )
            .width(Length::Fill)
            .padding([10, 12])
            .style(theme::primary)
            .on_press_maybe(
                (!busy && !draft.name.trim().is_empty()).then_some(Message::Ui(UiMsg::SaveCharacter))
            ),
        ]
        .spacing(8),
    ]
    .spacing(10)
    .width(260)
    .into()
}

/// The draft's art on light and dark halves, with a ground marker that a
/// click moves.
struct GroundPicker<'a> {
    draft: &'a crate::mascot::pack::Draft,
}

impl GroundPicker<'_> {
    fn image_rect(&self, bounds: iced::Size) -> iced::Rectangle {
        let pad = 16.0;
        let (w, h) = (self.draft.width as f32, self.draft.height as f32);
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

impl canvas::Program<Message> for GroundPicker<'_> {
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
                let message = UiMsg::CharacterGround((p.x - r.x) / r.width, (p.y - r.y) / r.height);
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
        let mut frame = Frame::new(renderer, size);
        frame.fill(
            &Path::rounded_rectangle(iced::Point::ORIGIN, size, 12.0.into()),
            Color::from_rgb8(0xF4, 0xF4, 0xF5),
        );
        frame.fill_rectangle(
            iced::Point::new(size.width / 2.0, 0.0),
            iced::Size::new(size.width / 2.0, size.height),
            Color::from_rgb8(0x1B, 0x26, 0x21),
        );
        let r = self.image_rect(size);
        frame.draw_image(r, canvas::Image::new(self.draft.image.clone()));
        let (gx, gy) = self.draft.ground;
        let at = iced::Point::new(r.x + gx * r.width, r.y + gy * r.height);
        // The ground line, then the feet marker.
        let line = Stroke::default()
            .with_color(Color::from_rgba8(0xF5, 0x9E, 0x0B, 0.7))
            .with_width(1.0);
        frame.stroke(
            &Path::line(
                iced::Point::new(r.x, at.y),
                iced::Point::new(r.x + r.width, at.y),
            ),
            line,
        );
        frame.fill(&Path::circle(at, 6.0), Color::from_rgb8(0xF5, 0x9E, 0x0B));
        frame.stroke(
            &Path::circle(at, 6.0),
            Stroke::default().with_color(Color::WHITE).with_width(1.5),
        );
        vec![frame.into_geometry()]
    }
}

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
        h1("Display"),
        section(vec![
            h2("Draw on"),
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
            h2("Tap sound"),
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
        ]),
        check("Mute tap sound", s.bool(sound::MUTE), sound::MUTE),
        check(
            "Play a sound when tapping with the laser",
            s.bool(sound::LASER),
            sound::LASER
        ),
    ]
    .spacing(22)
    .max_width(560)
    .into()
}

#[cfg(test)]
mod character_arrangement_tests {
    use super::*;
    use canvas::Program;

    #[test]
    fn drag_stays_captured_across_transparent_space_and_zoom_preserves_canvas_units() {
        use crate::mascot::editor::{Editor, Selected};
        use iced::{Point, Rectangle, Size, mouse};
        let dir =
            std::env::temp_dir().join(format!("tinypaw-arrange-input-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("body.png");
        image::RgbaImage::from_pixel(8, 8, image::Rgba([100, 140, 220, 255]))
            .save(&file)
            .unwrap();
        let mut editor = Editor::from_body(&file, &[]).unwrap();
        editor.select(Selected::Body);
        let bounds = Rectangle::new(Point::new(30.0, 40.0), Size::new(300.0, 340.0));
        for zoom in [1.0, 1.5, 2.0] {
            editor.zoom = zoom;
            let preview = ArtPreview {
                editor: &editor,
                busy: false,
            };
            let r = preview.image_rect(bounds.size());
            let start = Point::new(bounds.x + r.center_x(), bounds.y + r.center_y());
            let mut state = ArtPreviewState::default();
            let press = preview
                .update(
                    &mut state,
                    &iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                    bounds,
                    mouse::Cursor::Available(start),
                )
                .unwrap()
                .into_inner();
            assert!(matches!(
                press.0,
                Some(Message::Ui(UiMsg::ArtDragStart(Selected::Body)))
            ));
            let moved = Point::new(start.x + 10.0 * r.width / editor.canvas.0 as f32, start.y);
            let action = preview
                .update(
                    &mut state,
                    &iced::Event::Mouse(mouse::Event::CursorMoved { position: moved }),
                    bounds,
                    mouse::Cursor::Available(moved),
                )
                .unwrap()
                .into_inner();
            let Some(Message::Ui(UiMsg::ArtDrag(dx, dy))) = action.0 else {
                panic!("expected drag")
            };
            assert!((dx - 10.0).abs() < 0.001 && dy.abs() < 0.001);
            assert_eq!(action.2, iced::event::Status::Captured);
            let outside = Point::new(bounds.x + bounds.width + 50.0, bounds.y);
            let release = preview
                .update(
                    &mut state,
                    &iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                    bounds,
                    mouse::Cursor::Available(outside),
                )
                .unwrap()
                .into_inner();
            assert_eq!(release.2, iced::event::Status::Captured);
            assert!(state.drag.is_none());
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
