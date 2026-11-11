//! Settings → Character: the character home, the Customize workspace
//! (artwork, behavior, advanced) and its previews.
//!
//! A child of `settings`, so it shares that module's helpers and types.

use super::*;

mod art;
mod behavior;
mod reference;
#[cfg(test)]
mod tests;

use art::*;
use behavior::*;
use iced::widget::{column, row};
use reference::*;

/// `mascot::FRAME_RATE` choices with their labels.
const FRAME_RATES: [(&str, &str); 3] = [
    ("30", "30 fps"),
    ("60", "60 fps"),
    ("display", "Match display"),
];

pub(super) fn character_page<'a>(
    v: &SettingsView<'a>,
) -> (Element<'a, Message>, Option<Element<'a, Message>>) {
    let (page, side) = character_body(v);
    if !v.leaving {
        return (page, side);
    }
    (column![leave_prompt(v), page].spacing(16).into(), side)
}

/// Save / Discard / Stay, while leaving with unsaved character changes.
fn leave_prompt<'a>(v: &SettingsView<'a>) -> Element<'a, Message> {
    let name = v
        .characters
        .iter()
        .find(|c| c.id == v.character)
        .map_or("this character", |c| c.name.as_str());
    let choice = |label: &'static str, c: LeaveChoice, primary: bool| {
        button(text(label).size(13))
            .padding([7, 14])
            .style(if primary {
                theme::primary
            } else {
                theme::secondary
            })
            .on_press_maybe((!v.character_busy).then_some(Message::Ui(UiMsg::Leave(c))))
    };
    container(
        column![
            text(format!("Save your changes to {name}?"))
                .size(13)
                .color(theme::AMBER_TEXT),
            row![
                choice("Save", LeaveChoice::Save, true),
                choice("Discard", LeaveChoice::Discard, false),
                choice("Stay", LeaveChoice::Stay, false),
            ]
            .spacing(8),
        ]
        .spacing(10),
    )
    .padding([10, 12])
    .style(theme::fill(theme::AMBER_BG, 8.0))
    .max_width(640)
    .into()
}

/// One Save and Discard for the behavior draft (motion, lines, bop).
fn save_bar<'a>(v: &SettingsView<'a>) -> Element<'a, Message> {
    if !v.unsaved {
        return hint("Changes show in the preview straight away and on the desktop once saved.");
    }
    container(
        row![
            text("Unsaved changes").size(13).width(Length::Fill),
            button(text("Discard").size(12))
                .padding([5, 10])
                .style(theme::secondary)
                .on_press(Message::Ui(UiMsg::CharacterDiscard)),
            button(text("Save").font(theme::MEDIUM).size(12))
                .padding([5, 14])
                .style(theme::primary)
                .on_press_maybe((!v.character_busy).then_some(Message::Ui(UiMsg::CharacterSave))),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([6, 10])
    .style(theme::fill(theme::WELL, 10.0))
    .into()
}

/// Library and On desktop, then the Customize sections. One draft spans
/// them all.
fn navigator<'a>(v: &SettingsView<'a>) -> Element<'a, Message> {
    let mut tabs = row![].spacing(4);
    for (tab, label) in [
        (CharacterTab::Library, "Library"),
        (CharacterTab::Desktop, "On desktop"),
        (CharacterTab::Artwork, "Artwork"),
        (CharacterTab::Behavior, "Behavior"),
        (CharacterTab::Advanced, "Advanced"),
    ] {
        tabs = tabs.push(
            button(text(label).size(13))
                .padding([7, 10])
                .style(theme::segment(v.character_tab == tab))
                .on_press(Message::Ui(UiMsg::CharacterTab(tab))),
        );
    }
    container(tabs.wrap())
        .padding(4)
        .style(theme::fill(theme::WELL, 10.0))
        .into()
}

/// The active preview mode, shown beside the canvas.
fn mode_label<'a>(mode: &'static str, what: &'static str) -> Element<'a, Message> {
    row![
        container(text(mode).size(12).font(theme::MEDIUM))
            .padding([3, 8])
            .style(theme::fill(theme::CYAN_BG, 6.0)),
        hint(what),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

/// Artwork or Advanced before the editor is open.
fn artwork_closed<'a>(v: &SettingsView<'a>) -> Element<'a, Message> {
    let selected = v.characters.iter().find(|c| c.id == v.character);
    let builtin = selected.is_none_or(|c| c.is_builtin());
    if v.character_busy {
        return section(vec![hint("Opening the artwork…")]);
    }
    if builtin {
        return section(vec![
            h2("Artwork"),
            hint(
                "Yohta is built in, so his artwork can't be changed. Duplicate him to make an editable copy; his behavior can be customized without one.",
            ),
            button(text("Duplicate to edit").size(13))
                .padding([7, 12])
                .style(theme::secondary)
                .on_press(Message::Ui(UiMsg::EditArt))
                .into(),
        ]);
    }
    section(vec![
        h2("Artwork"),
        button(text("Open artwork").size(13))
            .padding([7, 12])
            .style(theme::secondary)
            .on_press(Message::Ui(UiMsg::EditArt))
            .into(),
    ])
}

fn character_body<'a>(
    v: &SettingsView<'a>,
) -> (Element<'a, Message>, Option<Element<'a, Message>>) {
    let tab = v.character_tab;
    if let Some(e) = v.art
        && matches!(tab, CharacterTab::Artwork | CharacterTab::Advanced)
    {
        let body = if tab == CharacterTab::Artwork {
            art_page(e, v.character_busy)
        } else {
            advanced_page(e)
        };
        return (
            column![navigator(v), body]
                .spacing(16)
                .max_width(640)
                .into(),
            Some(art_preview(e, v.character_busy)),
        );
    }
    if let Some((r, thumb)) = v.reference {
        return (
            column![h1("Character"), reference_panel(r, thumb)]
                .spacing(22)
                .max_width(640)
                .into(),
            None,
        );
    }
    if matches!(tab, CharacterTab::Artwork | CharacterTab::Advanced) && v.character_draft.is_none()
    {
        return (
            column![h1("Character"), navigator(v), artwork_closed(v)]
                .spacing(22)
                .max_width(640)
                .into(),
            v.motion.map(motion_preview),
        );
    }
    let s = v.session.settings();
    let size = s.float(mascot::HEIGHT);
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
            card_label(&c.name),
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
    let add = row![
        button("From parts…")
            .style(theme::secondary)
            .on_press(Message::Ui(UiMsg::ArtFromParts)),
        button("Import folder")
            .style(theme::secondary)
            .on_press(Message::Ui(UiMsg::ImportCharacter)),
        button("From reference…")
            .style(theme::secondary)
            .on_press(Message::Ui(UiMsg::ReferenceStart)),
    ]
    .spacing(8)
    .wrap();
    let manage = row![
        button("Export…")
            .style(theme::secondary)
            .on_press_maybe(selected.map(|_| Message::Ui(UiMsg::ExportCharacter))),
        button("Remove")
            .style(theme::secondary)
            .on_press_maybe(custom.map(|_| Message::Ui(UiMsg::RemoveCharacter))),
    ]
    .spacing(8)
    .wrap();

    let side = match (v.character_draft, v.motion) {
        (Some(d), _) => Some(new_character(d, v.character_busy)),
        (None, Some(m)) => Some(motion_preview(m)),
        (None, None) => None,
    };
    // The tabs sit directly under the title on every tab; they hide only
    // while Add character's draft is open beside the library.
    let mut page = column![h1("Character")].spacing(22).max_width(640);
    if v.character_draft.is_none() {
        page = page.push(navigator(v));
    }
    if let Some(m) = v.motion
        && tab == CharacterTab::Behavior
    {
        return (
            page.push(save_bar(v)).push(behavior_section(v, m)).into(),
            side,
        );
    }
    if tab == CharacterTab::Behavior {
        page = page.push(hint(
            "Behavior opens once the character's preview is ready.",
        ));
        return (page.into(), side);
    }
    if tab == CharacterTab::Library || v.character_draft.is_some() {
        page = page
            .push(section(vec![
                h2("Characters"),
                cards.wrap().vertical_spacing(8).into(),
            ]))
            .push(section(vec![
                h2("More ways to add"),
                add.into(),
                hint("Add character takes one transparent PNG. Import folder takes a character.json folder with a body and optional moving parts, like one made with Export."),
            ]))
            .push(section(vec![
                h2("Selected character"),
                manage.into(),
                if custom.is_none() {
                    hint(format!("{name} is built in, so it can't be removed."))
                } else {
                    hint(format!("Export saves {name} as a folder you can share or import later."))
                },
            ]));
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
        return (page.into(), side);
    }

    // On desktop.
    let tracks = v.motion.is_some_and(|m| m.rig().eye_point().is_some());
    let eyes: Element<'a, Message> = if tracks {
        check("Eyes follow the cursor", s.bool(mascot::EYES), mascot::EYES)
    } else {
        hint_link(
            format!(
                "{name}'s eyes are part of its artwork, so they can't follow the cursor. Tracking eyes need a head with separate white, pupil and mask art."
            ),
            "Artwork → Eyes",
            Message::Ui(UiMsg::CharacterTab(CharacterTab::Artwork)),
        )
    };
    page = page
        .push(section(vec![
            h2("Visibility"),
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
            slider(v.registry.range(mascot::HEIGHT), size, |x| {
                Message::Set(mascot::HEIGHT, Value::Float(x))
            })
            .step(4.0)
            .style(theme::range)
            .into(),
        ]))
        .push(section(vec![
            h2("Liveliness"),
            eyes,
            check(
                "Idle motion and blinking",
                s.bool(mascot::IDLE),
                mascot::IDLE,
            ),
            hint_link(
                "Reduced motion also stops idle motion and blinking.",
                "Motion settings",
                Message::Ui(UiMsg::Nav(Page::Display)),
            ),
            row![
                text("Frame rate").size(13).width(Length::Fill),
                pick_list(FRAME_RATES.map(|(v, l)| Choice(v, l)), {
                    Some(Choice::of(s.choice(mascot::FRAME_RATE), &FRAME_RATES))
                }, |c| Message::Set(mascot::FRAME_RATE, Value::Choice(c.0)))
                .text_size(13)
                .padding([6, 10])
                .style(theme::select),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
            hint("Higher is smoother but uses more CPU while the character moves."),
        ]))
        .push(
            button(text("Reset position").size(13))
                .padding([7, 12])
                .style(theme::secondary)
                .on_press_maybe(
                    s.bool(mascot::ENABLED)
                        .then_some(Message::Ui(UiMsg::ResetMascotPosition)),
                ),
        );
    (page.into(), side)
}
