//! Shared choice types and the controls used across pages.

use super::*;
use iced::widget::{column, row};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FadeChoice {
    Keep,
    ThreeSeconds,
}

impl FadeChoice {
    pub(super) fn id(self) -> &'static str {
        match self {
            Self::Keep => "keep",
            Self::ThreeSeconds => "3s",
        }
    }
}

/// A motion preference value with its display label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Motion(pub(super) &'static str);

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

pub(super) fn controls<'a>(v: &SettingsView<'a>) -> Element<'a, Message> {
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
        h1("Shortcuts"),
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

pub(super) fn check<'a>(label: &'a str, on: bool, id: &'static str) -> Element<'a, Message> {
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
