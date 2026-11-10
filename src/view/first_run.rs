//! First-run card: one primary action beside the active shortcut.

use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element, Length};

use super::icons::{Glyph, icon};
use super::{UiMsg, theme};
use crate::app::Message;

pub fn view<'a>(shortcut: Option<String>) -> Element<'a, Message> {
    let logo = container(icon(Glyph::PawSolid, 20.0, theme::AMBER))
        .width(36)
        .height(36)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(theme::fill(theme::AMBER_BG, 10.0));
    let title = column![
        text("TinyPaw is ready").font(theme::HEADING).size(17),
        text("It lives in the tray and starts Off.").size(12).color(theme::MUTED),
    ]
    .spacing(2);
    let tips = column![
        text("1.  Click to tap, drag to draw.").size(13).color(theme::SUBTLE),
        text("2.  Ctrl Z undoes, X clears.").size(13).color(theme::SUBTLE),
        text("3.  Esc hands the mouse back to your game.").size(13).color(theme::SUBTLE),
    ]
    .spacing(6);
    let mut action = row![
        button(text("Turn on TinyPaw").font(theme::HEADING).size(14).width(Length::Fill).center())
            .width(Length::Fill)
            .padding([12, 16])
            .style(theme::primary)
            .on_press(Message::Ui(UiMsg::TurnOn)),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    if let Some(s) = shortcut {
        action = action.push(
            container(text(s.replace('+', " ")).size(12).color(theme::SUBTLE))
                .padding([4, 8])
                .style(theme::keycap),
        );
    }
    let settings = button(text("Open settings").size(13).color(theme::CYAN_TEXT))
        .padding([4, 0])
        .style(|_, _| button::Style::default())
        .on_press(Message::Ui(UiMsg::OpenSettings));

    container(
        column![
            row![logo, title].spacing(10).align_y(Alignment::Center),
            tips,
            action,
            settings
        ]
        .spacing(16),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(24)
    .style(theme::fill(theme::SURFACE, 0.0))
    .into()
}
