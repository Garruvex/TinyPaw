//! Ink and Tools pages.

use super::*;
use iced::widget::{column, row};

/// Ink and text labels: what the quick bar's drawing tools draw with.
pub(super) fn ink_page<'a>(v: &SettingsView<'a>) -> Element<'a, Message> {
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
    let mut ink_row = row![swatches].spacing(12).align_y(Alignment::Center);
    if let Some(accent) = v.pack.and_then(|p| p.accent) {
        ink_row = ink_row.push(super::super::match_pointer(accent, current_ink));
    }
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
        h1("Ink & text"),
        section(vec![
            h2("Ink"),
            ink_row.into(),
            row![
                widths,
                row![hint("Fade"), fade_picker(s)]
                    .spacing(8)
                    .align_y(Alignment::Center)
            ]
            .spacing(16)
            .align_y(Alignment::Center)
            .wrap()
            .into(),
            hint("The quick bar's ink button edits these same values. Pointer tap stamps share the fade."),
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
    ]
    .spacing(22)
    .max_width(640)
    .into()
}

/// Spotlight and Lens: in annotation from the quick bar, or anywhere by
/// shortcut.
pub(super) fn tools_page<'a>(v: &SettingsView<'a>) -> Element<'a, Message> {
    let s = v.session.settings();
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
    let shortcuts = || link("Change shortcuts", Message::Ui(UiMsg::Nav(Page::Shortcuts)));
    column![
        h1("Spotlight & lens"),
        section(vec![
            h2("Spotlight"),
            hint("Size"),
            slider(
                v.registry.range(crate::features::spotlight::RADIUS),
                s.float(crate::features::spotlight::RADIUS),
                |x| { Message::Set(crate::features::spotlight::RADIUS, Value::Float(x)) }
            )
            .step(5.0)
            .style(theme::range)
            .into(),
            hint("Darkness"),
            slider(
                v.registry.range(crate::features::spotlight::DIM),
                s.float(crate::features::spotlight::DIM),
                |x| { Message::Set(crate::features::spotlight::DIM, Value::Float(x)) }
            )
            .step(0.05)
            .style(theme::range)
            .into(),
            hint(anywhere_hint(
                v.registry,
                crate::features::spotlight::ANYWHERE,
                "Ctrl+Alt+X",
                "size",
                "darkness"
            )),
            shortcuts(),
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
            hint(anywhere_hint(
                v.registry,
                crate::features::upcoming::ANYWHERE,
                "Ctrl+Alt+Z",
                "size",
                "zoom"
            )),
            shortcuts(),
        ]),
        section(vec![h2("Lens source"), source_picker]),
    ]
    .spacing(22)
    .max_width(640)
    .into()
}
