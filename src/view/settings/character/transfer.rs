//! The Character page's import and export panel: one place to move whole
//! characters and their dialogue in and out.

use super::*;
use iced::widget::{column, row};

fn choice<'a>(title: &'a str, detail: &'a str, msg: UiMsg) -> Element<'a, Message> {
    button(
        column![
            text(title).font(theme::MEDIUM).size(13),
            text(detail).size(12).color(theme::MUTED),
        ]
        .spacing(3),
    )
    .width(Length::Fill)
    .padding([10, 12])
    .style(theme::secondary)
    .on_press(Message::Ui(msg))
    .into()
}

fn actions<'a>(confirm: Option<&'a str>) -> Element<'a, Message> {
    let mut r = row![].spacing(8);
    if let Some(label) = confirm {
        r = r.push(
            button(text(label).size(13))
                .padding([7, 14])
                .style(theme::primary)
                .on_press(Message::Ui(UiMsg::TransferConfirm)),
        );
    }
    r.push(
        button(text("Cancel").size(13))
            .padding([7, 14])
            .style(theme::secondary)
            .on_press(Message::Ui(UiMsg::TransferCancel)),
    )
    .into()
}

/// The open panel: what to move, then any question the import needs.
pub(super) fn transfer_panel<'a>(t: &'a Transfer, name: &'a str) -> Element<'a, Message> {
    use crate::mascot::lines::Merge;
    let body: Vec<Element<'a, Message>> = match t {
        Transfer::Choose { export: false } => vec![
            h2("Import"),
            choice(
                "Character folder…",
                "A folder with character.json and its PNGs, like one made with Export.",
                UiMsg::TransferPick(false, TransferScope::Character),
            ),
            choice(
                "Dialogue file…",
                "A lines.json (or another character's character.json): speech lines, the bubble and the menu, into the selected character.",
                UiMsg::TransferPick(false, TransferScope::Dialogue),
            ),
            actions(None),
        ],
        Transfer::Choose { export: true } => vec![
            h2("Export"),
            choice(
                "Character folder…",
                "Everything, with your changes, ready to share or import on another PC.",
                UiMsg::TransferPick(true, TransferScope::Character),
            ),
            choice(
                "Dialogue file…",
                "The saved speech lines, bubble and menu placement, with their sounds.",
                UiMsg::TransferPick(true, TransferScope::Dialogue),
            ),
            choice(
                "Dialogue template…",
                "A lines.json with one example line per event, to fill in with any text editor.",
                UiMsg::TransferPick(true, TransferScope::Template),
            ),
            actions(None),
        ],
        Transfer::Fit { found, choice, .. } => {
            let mut list = column![].spacing(2);
            for m in found.misfits.iter().take(8) {
                list = list.push(
                    text(format!("{} is {} × {}", m.asset, m.size.0, m.size.1))
                        .size(12)
                        .color(theme::TEXT_SOFT),
                );
            }
            if found.misfits.len() > 8 {
                list = list.push(hint(format!("and {} more", found.misfits.len() - 8)));
            }
            vec![
                h2("Fit the artwork"),
                text(format!(
                    "{}'s canvas is {} × {}, but some images aren't:",
                    found.name, found.canvas.0, found.canvas.1
                ))
                .size(13)
                .into(),
                list.into(),
                row![
                    text("Place them").size(12).color(theme::MUTED).width(110),
                    pick_list(crate::mascot::pack::Fit::CHOICES, Some(*choice), |f| {
                        Message::Ui(UiMsg::TransferFit(f))
                    })
                    .text_size(13)
                    .padding([6, 10])
                    .style(theme::select),
                ]
                .spacing(8)
                .align_y(Alignment::Center)
                .into(),
                hint(
                    "Each image is put on a full-size canvas with transparent space around it; large ones are scaled down. Images the pack's character.json places with \"fit\" keep that placement. Fine-tune parts later in Customize → Artwork.",
                ),
                actions(Some("Import")),
            ]
        }
        Transfer::Dialogue {
            dialogue,
            merge,
            layout,
        } => {
            let l = &dialogue.lines;
            vec![
                h2("Import dialogue"),
                text(format!(
                    "{} lines for {name}.{}",
                    l.total(),
                    if dialogue.missing_sounds > 0 {
                        format!(
                            " {} sounds are missing, so those lines come in silent.",
                            dialogue.missing_sounds
                        )
                    } else {
                        String::new()
                    }
                ))
                .size(13)
                .into(),
                row![
                    text("Lines").size(12).color(theme::MUTED).width(110),
                    pick_list([Merge::Replace, Merge::Add], Some(*merge), |m| {
                        Message::Ui(UiMsg::TransferMerge(m))
                    })
                    .text_size(13)
                    .padding([6, 10])
                    .style(theme::select),
                ]
                .spacing(8)
                .align_y(Alignment::Center)
                .into(),
                checkbox(*layout)
                    .label("Also use its bubble look and menu placement")
                    .text_size(13)
                    .on_toggle(|on| Message::Ui(UiMsg::TransferLayout(on)))
                    .into(),
                hint("The lines go into the Behavior draft, so you can try them before Save."),
                actions(Some("Import")),
            ]
        }
    };
    container(Column::with_children(body).spacing(10))
        .padding([12, 14])
        .style(theme::outlined(theme::HEADER, 10.0, theme::LINE_STRONG))
        .into()
}
