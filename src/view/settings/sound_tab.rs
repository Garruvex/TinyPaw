//! Sound page.

use super::*;
use iced::widget::{column, row};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SoundChoice(&'static str);

impl std::fmt::Display for SoundChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(sound::label(self.0))
    }
}

pub(super) fn sound_page<'a>(v: &SettingsView<'a>) -> Element<'a, Message> {
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
            row![
                hint("Volume"),
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
            check("Mute tap sounds", s.bool(sound::MUTE), sound::MUTE),
            check(
                "Play a sound when tapping with the laser",
                s.bool(sound::LASER),
                sound::LASER
            ),
        ]),
        section(vec![
            h2("All sounds"),
            check("Mute all sounds", s.bool(sound::MUTE_ALL), sound::MUTE_ALL),
            hint_link(
                "Also silences the desktop character's reactions and line sounds.",
                "Character sounds",
                Message::Ui(UiMsg::CharacterTab(CharacterTab::Behavior)),
            ),
        ]),
    ]
    .spacing(22)
    .max_width(560)
    .into()
}
