//! Settings window: header with the master switch, a left navigation column
//! and one section at a time. Every control edits a registered setting through
//! the same typed update path as the CLI and quick bar.

use iced::widget::{
    Column, button, canvas, checkbox, column, container, pick_list, row, scrollable, slider,
    space, text, toggler, tooltip,
};
use iced::{Alignment, Color, Element, Length, Theme};

use super::icons::{Glyph, icon};
use super::{PreviewTile, UiMsg, theme};
use crate::app::Message;
use crate::features::{display, point, sound, system};
use crate::registry::{Registry, ShortcutContext, Value};
use crate::session::{ActivationStyle, Session};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    Controls,
    Display,
    #[default]
    Appearance,
    Sound,
}

pub struct SettingsView<'a> {
    pub registry: &'a Registry,
    pub session: &'a Session,
    pub page: Page,
    pub has_custom_sound: bool,
    /// Commands whose global shortcut is held by another app.
    pub taken: &'a [&'static str],
    pub notice: Option<&'a str>,
}

fn h1<'a>(label: &'a str) -> Element<'a, Message> {
    text(label).font(theme::HEADING).size(22).color(theme::TEXT).into()
}

fn h2<'a>(label: &'a str) -> Element<'a, Message> {
    text(label).font(theme::MEDIUM).size(13).color(theme::MUTED).into()
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
    let logo = container(icon(Glyph::PawSolid, 18.0, theme::AMBER))
        .width(30)
        .height(30)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(theme::fill(theme::AMBER_BG, 8.0));
    let switch = toggler(on)
        .label(if on { "On" } else { "Off" })
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
    right = right.push(
        container(switch)
            .padding([6, 12])
            .style(theme::outlined(theme::CONTROL, 999.0, theme::LINE)),
    );
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

fn nav<'a>(page: Page) -> Element<'a, Message> {
    let item = |p: Page, glyph: Glyph, label: &'static str| {
        let on = page == p;
        button(
            row![
                icon(glyph, 18.0, if on { theme::CYAN_TEXT } else { theme::SUBTLE }),
                text(label).size(14).font(if on { theme::MEDIUM } else { theme::BODY }),
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
    let tip = container(
        column![
            text("Streaming on Discord?").font(theme::MEDIUM).size(12).color(theme::TEXT),
            text("Share the game monitor, not the game window, so viewers see the paw and ink.")
                .size(12)
                .color(theme::MUTED),
        ]
        .spacing(2),
    )
    .padding(12)
    .style(theme::fill(theme::RAISED, 10.0));
    container(
        column![
            item(Page::Controls, Glyph::Controls, "Controls"),
            item(Page::Display, Glyph::Display, "Display"),
            item(Page::Appearance, Glyph::Paw, "Appearance"),
            item(Page::Sound, Glyph::Sound, "Sound"),
            space().height(Length::Fill),
            tip,
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
    content.push(match v.page {
        Page::Appearance => appearance(v),
        Page::Controls => controls(v),
        Page::Display => display_page(v),
        Page::Sound => sound_page(v),
    });
    scrollable(
        container(Column::with_children(content).spacing(16))
            .padding([24, 28])
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn appearance<'a>(v: &SettingsView<'a>) -> Element<'a, Message> {
    let s = v.session.settings();
    let laser = s.choice(point::STYLE) == "laser";

    let pointer_card = |glyph: Glyph, tint: Color, label: &'static str, on: bool, msg: Option<Message>| {
        button(
            column![
                container(icon(glyph, 22.0, tint))
                    .width(40)
                    .height(40)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .style(theme::fill(Color { a: 0.14, ..tint }, 20.0)),
                text(label).font(theme::MEDIUM).size(13),
            ]
            .spacing(8)
            .align_x(Alignment::Center),
        )
        .width(104)
        .height(96)
        .padding([16, 8])
        .style(theme::card(on))
        .on_press_maybe(msg)
    };
    let pointers = row![
        pointer_card(
            Glyph::PawSolid,
            theme::AMBER,
            "Yohta",
            !laser,
            Some(Message::Set(point::STYLE, Value::Choice("paw")))
        ),
        pointer_card(
            Glyph::Laser,
            Color::from_rgb8(0xF8, 0x71, 0x71),
            "Laser",
            laser,
            Some(Message::Set(point::STYLE, Value::Choice("laser")))
        ),
        later(pointer_card(Glyph::Plus, theme::MUTED, "Add yours", false, None)),
    ]
    .spacing(8);

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
            seg("Stretch Paw", "stretch", false),
            seg("Classic Stick", "stick", true),
        ]
        .spacing(4),
    )
    .padding(4)
    .style(theme::fill(theme::WELL, 10.0));
    let pres_hint = match pres {
        "stick" => "The paw on a slim rod from an off-screen grip, lower right.",
        _ => "A compact paw at the pointer. Stretch Paw needs forearm art in the pack.",
    };

    let size = s.float(point::SIZE);
    let size_row = row![
        h2("Size"),
        space().width(Length::Fill),
        text(format!("{size:.0} px")).size(13).color(theme::TEXT),
    ];
    let size_slider = slider(32.0..=96.0, size, |x| Message::Set(point::SIZE, Value::Float(x)))
        .step(4.0)
        .style(theme::range);

    let current = s.color(system::INK_COLOR);
    let width = s.float(system::INK_WIDTH);
    let swatches = row(theme::INKS.iter().map(|&(_, c)| {
        button(space().width(28).height(28))
            .padding(0)
            .style(theme::swatch(theme::rgba(c), c == current))
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

    let left = column![
        h1("Appearance"),
        section(vec![h2("Pointer"), pointers.into()]),
        section(vec![h2("Presentation"), presentation.into(), hint(pres_hint)]),
        section(vec![size_row.into(), size_slider.into()]),
        section(vec![
            h2("Ink"),
            swatches.into(),
            row![widths, row![hint("Fade"), fade].spacing(8).align_y(Alignment::Center)]
                .spacing(16)
                .align_y(Alignment::Center)
                .into(),
        ]),
    ]
    .spacing(22)
    .width(Length::Fill);

    let ink = theme::rgba(current);
    let tile = |bg: Color| {
        canvas(PreviewTile {
            background: bg,
            ink,
            width: width as f32,
            laser,
        })
        .width(Length::Fill)
        .height(150)
    };
    let preview = column![
        h2("Preview"),
        tile(Color::from_rgb8(0xF4, 0xF4, 0xF5)),
        tile(Color::from_rgb8(0x1B, 0x26, 0x21)),
        button(text("Try a tap").font(theme::MEDIUM).size(13).width(Length::Fill).center())
            .width(Length::Fill)
            .padding([11, 12])
            .style(theme::secondary)
            .on_press(Message::Ui(UiMsg::PreviewSound)),
    ]
    .spacing(10)
    .width(260);

    row![left, preview].spacing(28).into()
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
                            text("In use by another app").size(12).color(theme::AMBER_TEXT)
                        } else {
                            text(scope).size(12).color(theme::FAINT)
                        },
                        container(keycap(shortcut.label())).width(120).align_x(Alignment::End),
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
            rows = rows.push(container(space()).height(1).width(Length::Fill).style(theme::fill(theme::LINE, 0.0)));
        }
    }

    column![
        h1("Controls"),
        section(vec![h2("When switched On"), styles.into()]),
        section(vec![
            h2("Shortcuts"),
            rows.into(),
            hint("Rebinding shortcuts comes in a later alpha."),
        ]),
    ]
    .spacing(22)
    .max_width(680)
    .into()
}

fn option_card<'a>(label: &'static str, detail: &'static str, on: bool, enabled: bool) -> Element<'a, Message> {
    let dot = container(
        container(space())
            .width(8)
            .height(8)
            .style(theme::fill(if on { theme::CYAN } else { Color::TRANSPARENT }, 4.0)),
    )
    .width(18)
    .height(18)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .style(theme::outlined(
        Color::TRANSPARENT,
        9.0,
        if on { theme::CYAN } else { theme::FAINT },
    ));
    let card = button(
        row![
            dot,
            text(label).size(14).width(Length::Fill),
            text(detail).size(12).color(theme::MUTED),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding(14)
    .style(theme::card(on))
    .on_press_maybe(enabled.then_some(Message::Ui(UiMsg::Nav(Page::Display))));
    if enabled { card.into() } else { later(card) }
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
                    color: if checked { theme::CYAN } else { theme::LINE_STRONG },
                },
                text_color: Some(theme::TEXT),
            }
        })
        .on_toggle(move |b| Message::Set(id, Value::Bool(b)))
        .into()
}

fn display_page<'a>(v: &SettingsView<'a>) -> Element<'a, Message> {
    let s = v.session.settings();
    column![
        h1("Display"),
        section(vec![
            h2("Draw on"),
            option_card("Primary display", "Follows Windows' main display", true, true),
            option_card("All displays", "", false, false),
        ]),
        check(
            "Hide the quick bar while streaming",
            s.bool(display::HIDE_BAR),
            display::HIDE_BAR
        ),
        hint("Shortcuts and the tray keep working while the bar is hidden."),
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
        h1("Sound"),
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
            slider(0.0..=1.0, volume, |x| Message::Set(sound::VOLUME, Value::Float(x)))
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
