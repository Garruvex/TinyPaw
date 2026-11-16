//! Shared controls for the classic character editor.

use super::*;

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
