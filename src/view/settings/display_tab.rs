//! Display page.

use super::*;
use iced::widget::column;

/// The characters, the selected one's desktop options, and the Add
/// character draft beside them while one is in progress.
pub(super) fn display_page<'a>(v: &SettingsView<'a>) -> Element<'a, Message> {
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
