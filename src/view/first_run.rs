//! First-run card: one primary action beside the active shortcut.

use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element, Length};

use super::{UiMsg, theme};
use crate::app::Message;
use crate::features::system;
use crate::registry::Registry;

/// The card; its tips name the keys currently bound, so a rebind shows here.
pub fn view<'a>(registry: &Registry) -> Element<'a, Message> {
    let key = |id: &str| registry.shortcut_label(id).map(|k| k.replace('+', " "));
    let shortcut = registry.shortcut_label(system::MASTER_TOGGLE);
    let edit = match (key(system::UNDO), key(system::CLEAR)) {
        (Some(u), Some(c)) => format!("2.  {u} undoes, {c} clears."),
        (Some(u), None) => format!("2.  {u} undoes; Clear marks is under More."),
        _ => "2.  Undo and Clear marks are on the quick bar.".to_owned(),
    };
    let exit = match key(system::EXIT) {
        Some(e) => format!("3.  {e} hands the mouse back to your game."),
        None => "3.  End annotation hands the mouse back to your game.".to_owned(),
    };
    let logo = container(iced::widget::image(crate::branding::logo()).width(36).height(36))
        .width(36)
        .height(36)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center);
    let title = column![
        text("TinyPaw is ready").font(theme::HEADING).size(17),
        text("It lives in the tray and starts Off.").size(12).color(theme::MUTED),
    ]
    .spacing(2);
    let tips = column![
        text("1.  Click to tap, drag to draw.").size(13).color(theme::SUBTLE),
        text(edit).size(13).color(theme::SUBTLE),
        text(exit).size(13).color(theme::SUBTLE),
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
