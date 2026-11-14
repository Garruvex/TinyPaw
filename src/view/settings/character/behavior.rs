//! Behavior tab: motion, music, sounds, speech and the motion preview.

use super::*;
use iced::widget::{column, row};

/// Behavior → Moving parts: a preset, strength and speed for each part.
pub(super) fn motion_section<'a>(
    m: &'a crate::mascot::preview::MotionPreview,
) -> Element<'a, Message> {
    let mut rows = column![].spacing(14);
    for (i, (label, spec)) in m.labels.iter().zip(&m.specs).enumerate() {
        let kind = m.kinds[i];
        let number = |value: f32| {
            text(format!("{value:.2}"))
                .size(12)
                .color(theme::MUTED)
                .width(34)
        };
        let speed: Element<'a, Message> = if spec.preset.uses_speed() {
            row![
                text("Speed").size(12).color(theme::MUTED).width(64),
                slider(0.0..=2.0, spec.speed, move |x| Message::Ui(
                    UiMsg::MotionSpeed(i, x)
                ))
                .step(0.05_f32)
                .style(theme::range),
                number(spec.speed),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .into()
        } else {
            hint("No idle motion, so no speed: it moves only when dragged or with music.")
        };
        rows = rows.push(
            column![
                row![
                    text(label.clone())
                        .font(theme::MEDIUM)
                        .size(13)
                        .width(Length::Fill),
                    pick_list(kind.presets(), Some(spec.preset), move |p| {
                        Message::Ui(UiMsg::MotionPreset(i, p))
                    })
                    .text_size(13)
                    .padding([6, 10])
                    .style(theme::select),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
                row![
                    text("Strength").size(12).color(theme::MUTED).width(64),
                    slider(0.0..=2.0, spec.strength, move |x| Message::Ui(
                        UiMsg::MotionStrength(i, x)
                    ))
                    .step(0.05_f32)
                    .style(theme::range),
                    number(spec.strength),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
                speed,
            ]
            .spacing(6),
        );
    }
    section(vec![
        row![
            h2("Moving parts"),
            space().width(Length::Fill),
            button(text("Reset to pack").size(12))
                .padding([5, 10])
                .style(theme::secondary)
                .on_press_maybe(m.customised().then_some(Message::Ui(UiMsg::MotionReset))),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
        hint(
            "Pick how each part moves. Strength scales idle, drag and music motion; Speed paces idle motion. Both run from 0 to 2; 1 is the preset as designed.",
        ),
        rows.into(),
    ])
}

/// Behavior, grouped by the moment the creator is thinking about. Each
/// event row is where its lines (and later its sound) are edited.
pub(super) fn behavior_section<'a>(
    v: &SettingsView<'a>,
    m: &'a crate::mascot::preview::MotionPreview,
) -> Element<'a, Message> {
    use crate::mascot::lines::Trigger;
    let s = v.session.settings();
    let mut own = column![event_row(m, Trigger::Idle)].spacing(16);
    own = own.push(idle_interval(s));
    own = own
        .push(event_row(m, Trigger::MusicStart))
        .push(event_row(m, Trigger::AnnotationEnd));
    let muted = s.bool(crate::features::sound::MUTE_ALL);
    column![
        character_sounds(m, muted),
        companionship_section(m, muted),
        section(vec![
            h2("When touched"),
            event_row(m, Trigger::Click),
            visual_row(m, Event::Click),
            sound_row(m, Event::Click, muted),
            event_row(m, Trigger::Pickup),
            held_row(m),
            sound_row(m, Event::Pickup, muted),
            event_row(m, Trigger::Drop),
            visual_row(m, Event::Drop),
            row![
                text("After release").size(12).color(theme::MUTED).width(84),
                pick_list(["Stay here", "Fall"], Some(if m.falls() { "Fall" } else { "Stay here" }), |c| {
                    Message::Ui(UiMsg::ReactionFall(c == "Fall"))
                })
                .text_size(13)
                .padding([5, 8])
                .style(theme::select),
                hint("Fall drops it to the bottom of the screen it was released on. Let go while moving, it is thrown: it glides to a stop, or with Fall arcs and bounces. Picking it up mid-flight catches it."),
            ]
            .spacing(6)
            .align_y(Alignment::Center)
            .into(),
            sound_row(m, Event::Drop, muted),
        ]),
        section(vec![h2("On its own"), own.into(), sound_row(m, Event::Idle, muted)]),
        speech_options(v, m),
        motion_section(m),
        music_section(v),
    ]
    .spacing(22)
    .into()
}

use crate::mascot::reactions::{BUILTIN_SOUNDS, Event, Sound};

/// A sound picker entry: None, a bundled clip, or the row's imported WAV.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SoundPick(Option<Sound>);

impl std::fmt::Display for SoundPick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match &self.0 {
            None => "None",
            Some(Sound::Builtin(id)) => BUILTIN_SOUNDS
                .iter()
                .find(|(b, _)| b == id)
                .map_or("Unknown", |(_, label)| label),
            Some(Sound::File(_)) => "Imported WAV",
        })
    }
}

/// Character-wide reaction sound settings, above the event rows.
pub(super) fn character_sounds<'a>(
    m: &'a crate::mascot::preview::MotionPreview,
    muted: bool,
) -> Element<'a, Message> {
    let s = m.sounds();
    let mut items: Vec<Element<'a, Message>> = vec![
        row![
            h2("Character sounds"),
            space().width(Length::Fill),
            button(text("Reset to pack").size(12))
                .padding([5, 10])
                .style(theme::secondary)
                .on_press_maybe(
                    m.reactions_customised()
                        .then_some(Message::Ui(UiMsg::ReactionsReset))
                ),
        ]
        .align_y(Alignment::Center)
        .into(),
        toggler(s.enabled)
            .label("Sounds on")
            .text_size(13)
            .on_toggle(|on| Message::Ui(UiMsg::ReactionSoundsOn(on)))
            .into(),
    ];
    if s.enabled {
        items.push(
            row![
                text("Volume").size(12).color(theme::MUTED).width(110),
                slider(0.0..=1.0, s.volume, |v| Message::Ui(UiMsg::ReactionVolume(
                    v
                )))
                .step(0.05_f32)
                .style(theme::range),
                text(format!("{:.0}%", s.volume * 100.0)).size(12).width(48),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
        );
        items.push(
            toggler(s.idle_enabled)
                .label("Idle sounds now and then")
                .text_size(13)
                .on_toggle(|on| Message::Ui(UiMsg::ReactionIdleOn(on)))
                .into(),
        );
        if s.idle_enabled {
            let (lo, hi) = s.idle_interval;
            items.push(
                row![
                    text("Every").size(12).color(theme::MUTED).width(110),
                    slider(10.0..=600.0, lo, move |x| Message::Ui(
                        UiMsg::ReactionIdleInterval(x, hi)
                    ))
                    .step(10.0_f32)
                    .style(theme::range),
                    text(format!("{lo:.0} s")).size(12).width(48),
                    text("to").size(12).color(theme::MUTED),
                    slider(10.0..=600.0, hi, move |x| Message::Ui(
                        UiMsg::ReactionIdleInterval(lo, x)
                    ))
                    .step(10.0_f32)
                    .style(theme::range),
                    text(format!("{hi:.0} s")).size(12).width(48),
                ]
                .spacing(8)
                .align_y(Alignment::Center)
                .into(),
            );
        }
    }
    items.push(hint(if muted {
        "Mute all sounds is on, so the character and Test are silent."
    } else {
        "One sound plays at a time; a new reaction replaces the last, and idle never interrupts. A reaction sound plays instead of that event's line sound."
    }));
    section(items)
}

/// What an event does to the picture. Compress works with one PNG.
pub(super) fn visual_row<'a>(
    m: &'a crate::mascot::preview::MotionPreview,
    e: Event,
) -> Element<'a, Message> {
    use crate::mascot::reactions::Visual;
    let choices: Vec<Visual> = if e == Event::Click {
        vec![Visual::None, Visual::Compress, Visual::Artwork]
    } else {
        vec![Visual::None, Visual::Artwork]
    };
    let visual = m.visual(e);
    let mut block = column![
        row![
            text("Visual").size(12).color(theme::MUTED).width(48),
            pick_list(choices, Some(visual), move |v| Message::Ui(
                UiMsg::ReactionVisual(e, v)
            ))
            .text_size(13)
            .padding([5, 8])
            .style(theme::select),
            hint(match visual {
                Visual::Compress =>
                    "Briefly shortens and widens the picture after a click, then restores it.",
                Visual::Artwork if e == Event::Sleep =>
                    "Keeps the selected sleeping position throughout the nap.",
                Visual::None if e == Event::Sleep =>
                    "Dozes using existing blink artwork; no extra PNG needed.",
                Visual::Artwork =>
                    "Shows another whole-character PNG for a moment, then the normal one.",
                Visual::None => "The picture stays as it is.",
            }),
        ]
        .spacing(6)
        .align_y(Alignment::Center)
    ]
    .spacing(6);
    if let Some(art) = m.art(e) {
        block = block.push(picture_controls(e, false, art.place));
    }
    if e == Event::Sleep {
        let poses = m.sleep_poses();
        block = block.push(hint("No extra art is needed for dozing with a blink layer. Optional sleep PNGs add curled-up or lying-down positions. One pose is chosen per nap; wake artwork is optional."));
        if poses.len() < 8 {
            block =
                block.push(button("Add sleep pose…").on_press(Message::Ui(UiMsg::SleepPoseAdd)));
        }
        for (index, _) in poses.iter().enumerate() {
            block = block.push(
                row![
                    text(format!(
                        "Sleep pose {}{}",
                        index + 1,
                        if index == 0 { " (editing)" } else { "" }
                    ))
                    .size(12),
                    button("Edit placement & mouth")
                        .on_press(Message::Ui(UiMsg::SleepPoseEdit(index, false))),
                    button("Remove").on_press(Message::Ui(UiMsg::SleepPoseEdit(index, true))),
                ]
                .spacing(6),
            );
        }
    }
    block.into()
}

/// Pick up → Held pose: keep the artwork, or show another PNG while carried.
pub(super) fn held_row<'a>(m: &'a crate::mascot::preview::MotionPreview) -> Element<'a, Message> {
    let small = |label: &'static str, msg: Message| {
        button(text(label).size(12))
            .padding([5, 10])
            .style(theme::secondary)
            .on_press(msg)
    };
    match m.held() {
        None => row![
            text("Held pose").size(12).color(theme::MUTED).width(64),
            text("Keep current artwork").size(13),
            small("Choose PNG…", Message::Ui(UiMsg::ReactionPicture(Event::Pickup))),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
        Some(art) => column![
            row![
                text("Held pose").size(12).color(theme::MUTED).width(64),
                text("Custom PNG while carried").size(13),
                small("Change…", Message::Ui(UiMsg::ReactionPicture(Event::Pickup))),
                small("Clear", Message::Ui(UiMsg::ReactionHeldClear)),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            picture_controls(Event::Pickup, true, art.place),
            row![
                small(if m.placing_grip { "Cancel grip" } else { "Set grip on preview" }, Message::Ui(UiMsg::GripEdit)),
                small("Automatic grip", Message::Ui(UiMsg::GripSet(None))),
            ].spacing(6),
            hint("Choose Set grip, then click the pickup point on the preview. The circle marks where the character hangs."),
            hint("Line the held pose up so the part people grab sits where it does in the normal pose; the preview shows it while carried."),
        ]
        .spacing(6)
        .into(),
    }
}

/// Moves and resizes a replacement picture on the stage.
pub(super) fn picture_controls<'a>(e: Event, held: bool, place: [f32; 3]) -> Element<'a, Message> {
    let slider_row =
        |label: &'static str, which: usize, range: std::ops::RangeInclusive<f32>, step: f32| {
            row![
                text(label).size(12).color(theme::MUTED).width(64),
                slider(range, place[which], move |v| Message::Ui(
                    UiMsg::ReactionPlace(e, held, which, v)
                ))
                .step(step)
                .style(theme::range),
                text(if which == 2 {
                    format!("{:.0}%", place[2] * 100.0)
                } else {
                    format!("{:+.0}%", place[which] * 100.0)
                })
                .size(12)
                .width(48),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
        };
    column![
        button("Align poses…").on_press(Message::Ui(UiMsg::Align(
            crate::mascot::alignment::Action::Open(e)
        ))),
        row![
            button("Preview pose").on_press(Message::Ui(UiMsg::PlacementPreview(Some(e)))),
            button("Finish preview").on_press(Message::Ui(UiMsg::PlacementPreview(None))),
        ]
        .spacing(6),
        slider_row("Left/right", 0, -1.0..=1.0, 0.005),
        slider_row("Up/down", 1, -1.0..=1.0, 0.005),
        slider_row("Size", 2, 0.1..=4.0, 0.01),
    ]
    .spacing(4)
    .into()
}

/// An event's sound: picker, Import WAV…, Clear, Test, and its cooldown.
pub(super) fn sound_row<'a>(
    m: &'a crate::mascot::preview::MotionPreview,
    e: Event,
    muted: bool,
) -> Element<'a, Message> {
    let sounds = m.sounds();
    let t = sounds.get(e).clone();
    let current = SoundPick(t.sound.clone().filter(|_| t.enabled));
    let mut choices = vec![SoundPick(None)];
    choices.extend(
        BUILTIN_SOUNDS
            .iter()
            .map(|(id, _)| SoundPick(Some(Sound::Builtin(id)))),
    );
    if let Some(Sound::File(r)) = &t.sound {
        choices.push(SoundPick(Some(Sound::File(r.clone()))));
    }
    let small = |label: &'static str, msg: Option<Message>| {
        button(text(label).size(12))
            .padding([5, 10])
            .style(theme::secondary)
            .on_press_maybe(msg)
    };
    let mut line = row![
        text("Sound").size(12).color(theme::MUTED).width(48),
        pick_list(choices, Some(current), move |c: SoundPick| Message::Ui(
            UiMsg::ReactionSound(e, c.0)
        ))
        .text_size(13)
        .padding([5, 8])
        .style(theme::select),
        small("Import WAV…", Some(Message::Ui(UiMsg::ReactionImport(e)))),
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    if t.enabled {
        line = line
            .push(small(
                "Clear",
                Some(Message::Ui(UiMsg::ReactionSound(e, None))),
            ))
            .push(small(
                "Test",
                (!muted && sounds.enabled).then_some(Message::Ui(UiMsg::ReactionTest(e))),
            ));
    }
    let mut block = column![line.wrap()].spacing(6);
    if t.enabled && e != Event::Idle {
        let ms = t.cooldown_ms as f32;
        block = block.push(
            row![
                text("Cooldown").size(12).color(theme::MUTED).width(64),
                slider(0.0..=10_000.0, ms, move |x| Message::Ui(
                    UiMsg::ReactionCooldown(e, x)
                ))
                .step(50.0_f32)
                .style(theme::range),
                text(format!("{ms:.0} ms")).size(12).width(64),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );
    }
    block.into()
}

/// One event: a summary, a Try action, and its dialogue lines.
pub(super) fn event_row<'a>(
    m: &'a crate::mascot::preview::MotionPreview,
    t: crate::mascot::lines::Trigger,
) -> Element<'a, Message> {
    use crate::mascot::preview::Test;
    let small = |label: &'static str, msg: Message| {
        button(text(label).size(12))
            .padding([5, 10])
            .style(theme::secondary)
            .on_press(msg)
    };
    let count = m
        .lines
        .get(t)
        .iter()
        .filter(|l| !l.text.trim().is_empty())
        .count();
    let summary = match count {
        0 => "No lines".to_owned(),
        1 => "1 line".to_owned(),
        n => format!("{n} lines"),
    };
    let mut group = column![
        row![
            text(t.label())
                .font(theme::MEDIUM)
                .size(13)
                .width(Length::Fill),
            text(summary).size(12).color(theme::MUTED),
            small("Try", Message::Ui(UiMsg::MotionTest(Test::Say(t)))),
            small("Add line", Message::Ui(UiMsg::LineAdd(t))),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
    ]
    .spacing(6);
    {
        for (i, line) in m.lines.get(t).iter().enumerate() {
            let mut row_ = row![
                iced::widget::text_input("What it says; // splits it into parts", &line.text)
                    .on_input(move |text| Message::Ui(UiMsg::LineText(t, i, text)))
                    .size(13)
                    .padding([7, 9])
                    .width(Length::Fill)
                    .style(|_, status| iced::widget::text_input::Style {
                        background: theme::CONTROL.into(),
                        border: iced::Border {
                            radius: 8.0.into(),
                            width: 1.0,
                            color: if matches!(
                                status,
                                iced::widget::text_input::Status::Focused { .. }
                            ) {
                                theme::CYAN_RING
                            } else {
                                theme::LINE_STRONG
                            },
                        },
                        icon: theme::MUTED,
                        placeholder: theme::FAINT,
                        value: theme::TEXT,
                        selection: theme::CYAN_BG,
                    }),
                small(
                    if line.sound.is_some() {
                        "Sound ✓"
                    } else {
                        "Sound…"
                    },
                    Message::Ui(UiMsg::LineSound(t, i)),
                ),
            ]
            .spacing(6)
            .align_y(Alignment::Center);
            if line.sound.is_some() {
                row_ = row_.push(
                    button(text("No sound").size(12))
                        .padding([5, 8])
                        .style(theme::tool_button(theme::Tone::Plain))
                        .on_press(Message::Ui(UiMsg::LineClearSound(t, i))),
                );
            }
            row_ = row_.push(
                button(text("×").size(13))
                    .padding([5, 9])
                    .style(theme::tool_button(theme::Tone::Plain))
                    .on_press(Message::Ui(UiMsg::LineRemove(t, i))),
            );
            group = group.push(row_);
        }
    }
    group.into()
}

/// Speech settings shared by every event: whether lines show, how loud,
/// and the bubble's look.
pub(super) fn speech_options<'a>(
    v: &SettingsView<'a>,
    m: &'a crate::mascot::preview::MotionPreview,
) -> Element<'a, Message> {
    use crate::mascot::lines::{BubbleSide, MAX_WIDTH, MIN_WIDTH, MenuAlign, MenuSide};
    let s = v.session.settings();

    let swatches = |colours: [[u8; 4]; 4], current: [u8; 4], msg: fn([u8; 4]) -> UiMsg| {
        row(colours.into_iter().map(move |c| {
            button(space().width(24).height(24))
                .padding(0)
                .style(theme::swatch(theme::rgba(c), c == current))
                .on_press(Message::Ui(msg(c)))
                .into()
        }))
        .spacing(6)
    };
    let b = m.lines.bubble;
    section(vec![
        row![
            h2("Speech lines"),
            space().width(Length::Fill),
            button(text("Import…").size(12))
                .padding([5, 10])
                .style(theme::secondary)
                .on_press(Message::Ui(UiMsg::TransferPick(
                    false,
                    super::TransferScope::Dialogue
                ))),
            button(text("Export…").size(12))
                .padding([5, 10])
                .style(theme::secondary)
                .on_press(Message::Ui(UiMsg::TransferOpen(true))),
            button(text("Reset to pack").size(12))
                .padding([5, 10])
                .style(theme::secondary)
                .on_press_maybe(
                    m.lines_customised()
                        .then_some(Message::Ui(UiMsg::LinesReset))
                ),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
        hint(
            "A line is picked at random, never the same twice in a row, and shows in a bubble above the character. Try each event's lines with its Try button.",
        ),
        check("Show speech lines", s.bool(mascot::LINES), mascot::LINES),
        check(
            "Quiet: no idle or music lines",
            s.bool(mascot::QUIET_LINES),
            mascot::QUIET_LINES,
        ),
        row![
            text("Character volume")
                .size(12)
                .color(theme::MUTED)
                .width(110),
            slider(0.0..=1.0, s.float(mascot::VOICE_VOLUME), |x| Message::Set(
                mascot::VOICE_VOLUME,
                Value::Float(x)
            ))
            .step(0.05)
            .style(theme::range),
            text(format!("{:.0}%", s.float(mascot::VOICE_VOLUME) * 100.0))
                .size(12)
                .width(48),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
        hint(
            "Line sounds play at Character volume, separate from the pointer's tap sound. One character sound plays at a time; a new reaction replaces the last.",
        ),
        check(
            "Mute all sounds",
            s.bool(crate::features::sound::MUTE_ALL),
            crate::features::sound::MUTE_ALL,
        ),
        row![
            text("Bubble").size(12).color(theme::MUTED).width(110),
            swatches(
                [
                    [255, 255, 255, 255],
                    [255, 247, 224, 255],
                    [27, 27, 31, 255],
                    [34, 211, 238, 255],
                ],
                b.background,
                UiMsg::BubbleBackground,
            ),
            text("Text").size(12).color(theme::MUTED),
            swatches(
                [
                    [27, 27, 31, 255],
                    [255, 255, 255, 255],
                    [14, 116, 144, 255],
                    [180, 83, 9, 255],
                ],
                b.text,
                UiMsg::BubbleText,
            ),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into(),
        row![
            text("Bubble width").size(12).color(theme::MUTED).width(110),
            slider(MIN_WIDTH..=MAX_WIDTH, b.max_width, |x| Message::Ui(
                UiMsg::BubbleWidth(x)
            ))
            .step(10.0_f32)
            .style(theme::range),
            text(format!("{:.0} px", b.max_width)).size(12).width(48),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
        h2("Placement"),
        row![
            text("Bubble opens").size(12).color(theme::MUTED).width(110),
            pick_list(BubbleSide::ALL, Some(b.side), |s| Message::Ui(
                UiMsg::BubbleSide(s)
            ))
            .text_size(13)
            .padding([6, 10])
            .style(theme::select),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
        row![
            text("Tail points at")
                .size(12)
                .color(theme::MUTED)
                .width(110),
            text(match b.anchor {
                None => "Top of the art".to_owned(),
                Some((x, y)) => format!("{:.0}% across, {:.0}% down", x * 100.0, y * 100.0),
            })
            .size(12)
            .width(Length::Fill),
            button(
                text(if m.placing_bubble {
                    "Click the preview…"
                } else {
                    "Pick on preview"
                })
                .size(12)
            )
            .padding([5, 10])
            .style(if m.placing_bubble {
                theme::primary
            } else {
                theme::secondary
            })
            .on_press(Message::Ui(UiMsg::BubblePlace)),
            button(text("Top of the art").size(12))
                .padding([5, 10])
                .style(theme::secondary)
                .on_press_maybe(
                    b.anchor
                        .is_some()
                        .then_some(Message::Ui(UiMsg::BubbleAnchor(None)))
                ),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
        row![
            text("Menu opens").size(12).color(theme::MUTED).width(110),
            pick_list(MenuSide::ALL, Some(m.lines.menu.side), |s| Message::Ui(
                UiMsg::MenuSide(s)
            ))
            .text_size(13)
            .padding([6, 10])
            .style(theme::select),
            text("lined up with").size(12).color(theme::MUTED),
            pick_list(MenuAlign::ALL, Some(m.lines.menu.align), |a| Message::Ui(
                UiMsg::MenuAlign(a)
            ))
            .text_size(13)
            .padding([6, 10])
            .style(theme::select),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
        hint(
            "Where the bubble and the right-click menu sit on the desktop. The tail points at the top of the visible art unless you pick a spot, such as the mouth. A side bubble moves above while the menu is open on that side, or when there's no room. A chosen menu side flips only when it would leave the screen. Saved with the character's Save, and carried in exports.",
        ),
    ])
}

/// Idle lines' interval, shown only while idle lines can appear.
pub(super) fn idle_interval<'a>(s: &crate::registry::SettingValues) -> Element<'a, Message> {
    if !s.bool(mascot::LINES) || s.bool(mascot::QUIET_LINES) {
        return hint("Idle lines are off while speech lines are hidden or Quiet is on.");
    }
    let minutes = |x: f64| format!("{x:.0} min");
    row![
        text("Idle lines every")
            .size(12)
            .color(theme::MUTED)
            .width(110),
        slider(1.0..=30.0, s.float(mascot::IDLE_MIN), |x| Message::Set(
            mascot::IDLE_MIN,
            Value::Float(x)
        ))
        .step(1.0)
        .style(theme::range),
        text(minutes(s.float(mascot::IDLE_MIN))).size(12).width(48),
        text("to").size(12).color(theme::MUTED),
        slider(1.0..=30.0, s.float(mascot::IDLE_MAX), |x| Message::Set(
            mascot::IDLE_MAX,
            Value::Float(x)
        ))
        .step(1.0)
        .style(theme::range),
        text(minutes(s.float(mascot::IDLE_MAX))).size(12).width(48),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

/// The Music tab: react to playback, how strongly, and a live meter.
/// A live meter `level` (0–1) full; `width` 0 fills the row.
pub(super) fn level_bar<'a>(level: f32, width: f32) -> Element<'a, Message> {
    let level = level.clamp(0.0, 1.0);
    let fill = container(space())
        .width(Length::FillPortion((level * 1000.0) as u16 + 1))
        .height(8)
        .style(theme::fill(theme::CYAN_TEXT, 4.0));
    let bar = container(row![
        fill,
        space().width(Length::FillPortion(((1.0 - level) * 1000.0) as u16 + 1)),
    ])
    .height(8)
    .style(theme::fill(theme::WELL, 4.0));
    if width > 0.0 {
        bar.width(width).into()
    } else {
        bar.width(Length::Fill).into()
    }
}

pub(super) fn music_section<'a>(v: &SettingsView<'a>) -> Element<'a, Message> {
    let s = v.session.settings();
    use crate::mascot::music::Source;
    let (status, level) = (v.music.0.clone(), v.music.1);
    let on = s.bool(mascot::MUSIC);
    let pick = |source: &Source| {
        Message::Set(
            mascot::MUSIC_SOURCE,
            Value::Choice(crate::platform::intern(&source.id())),
        )
    };
    // One row per app source, each with its own live meter, so it's clear
    // which one is making sound.
    let mut rows = column![].spacing(4);
    for (source, shown, playing) in &v.music_rows {
        let selected = *source == v.music_source;
        let note = match source {
            Source::App(_) if !playing => "not playing",
            _ => "",
        };
        rows = rows.push(
            button(
                row![
                    text(source.to_string()).size(13).width(Length::Fill),
                    text(note).size(11).color(theme::MUTED),
                    level_bar(*shown, 120.0),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .padding([7, 10])
            .style(theme::card(selected))
            .on_press(pick(source)),
        );
    }
    let chosen_input = match &v.music_source {
        Source::Input(_) => Some(v.music_source.clone()),
        _ => None,
    };
    let intro = vec![
        h2("Music response"),
        check("React to playback", on, mascot::MUSIC),
        hint(
            "Moves with the loudness of what's playing on this PC. TinyPaw's own sounds are left out, and nothing is recorded.",
        ),
    ];
    if !on {
        // Nothing below has an effect, or a meter to read, while off.
        return section(intro);
    }
    let follow = s.bool(mascot::BEAT);
    let mut items = intro;
    items.extend([
        text("Listen to").size(12).color(theme::MUTED).into(),
        rows.into(),
        row![
            text("Or an input").size(12).color(theme::MUTED).width(110),
            pick_list(v.music_inputs.clone(), chosen_input, move |s| pick(&s))
                .placeholder("Microphone or line-in…")
                .text_size(13)
                .padding([6, 10])
                .style(theme::select),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into(),
        hint(match v.music_source {
            Source::All => "All apps together. Pick one app so a game or a call doesn't count; apps show up here while they make sound.",
            Source::App(_) => "Only this app counts, and Follow beat hears only its sound.",
            Source::Input(_) => "For music that doesn't play on this PC. This uses the input device (Windows shows the microphone in use) while listening; nothing is recorded.",
        }),
        row![
            text("Hearing").size(12).color(theme::MUTED).width(110),
            level_bar(level, 0.0),
            text(status).size(12).color(theme::MUTED),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into(),
        hint("Quiet sources are turned up automatically, so the character moves fully even with low volume or a microphone."),
        row![
            text("Music strength").size(12).color(theme::MUTED).width(110),
            slider(0.0..=2.0, s.float(mascot::MUSIC_STRENGTH), |x| Message::Set(
                mascot::MUSIC_STRENGTH,
                Value::Float(x)
            ))
            .step(0.05)
            .style(theme::range),
            text(format!("{:.2}", s.float(mascot::MUSIC_STRENGTH))).size(12).width(40),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
        row![
            text("Bop style").size(12).color(theme::MUTED).width(110),
            pick_list(
                crate::mascot::motion::Bop::ALL,
                v.motion.map(|m| m.rig().bop()),
                |b| Message::Ui(UiMsg::MusicBop(b)),
            )
            .text_size(13)
            .padding([6, 10])
            .style(theme::select),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into(),
        row![
            text("Music pose").size(12).color(theme::MUTED).width(110),
            pick_list(["Automatic", "Sitting", "Standing"],
                Some(match s.choice(mascot::MUSIC_POSE) { "sit" => "Sitting", "stand" => "Standing", _ => "Automatic" }),
                |x| Message::Set(mascot::MUSIC_POSE, Value::Choice(match x { "Sitting" => "sit", "Standing" => "stand", _ => "auto" })))
                .text_size(13).padding([6, 10]).style(theme::select),
        ].spacing(10).align_y(Alignment::Center).into(),
        row![
            text("Stand at energy").size(12).color(theme::MUTED).width(110),
            slider(30.0..=90.0, s.float(mascot::EXCITEMENT), |x| Message::Set(mascot::EXCITEMENT, Value::Float(x)))
                .step(1.0).style(theme::range),
            text(format!("{:.0}%", s.float(mascot::EXCITEMENT))).size(12).width(48),
        ].spacing(8).align_y(Alignment::Center).into(),
        hint("Automatic keeps gentle bops seated, stands after sustained high activity, and sits again when the music calms. A higher threshold keeps it seated more often. Standing artwork is required. Choose Standing to adjust the waving hand."),
        row![
            text("Change every").size(12).color(theme::MUTED).width(110),
            pick_list(["8", "16", "32"], Some(s.choice(mascot::DANCE_EVERY)), |x| Message::Set(mascot::DANCE_EVERY, Value::Choice(x)))
                .text_size(13).padding([6, 10]).style(theme::select),
            text("beats in Random").size(12).color(theme::MUTED),
        ].spacing(10).align_y(Alignment::Center).into(),
        row![
            text("Light-wave item").size(12).color(theme::MUTED).width(110),
            pick_list(["Glow stick", "Phone", "Random"], Some(match s.choice(mascot::DANCE_ITEM) { "phone" => "Phone", "glow_stick" => "Glow stick", _ => "Random" }),
                |x| Message::Set(mascot::DANCE_ITEM, Value::Choice(match x { "Phone" => "phone", "Glow stick" => "glow_stick", _ => "random" })))
                .text_size(13).padding([6, 10]).style(theme::select),
        ].spacing(10).align_y(Alignment::Center).into(),
        row![
            text("Hand horizontal").size(12).color(theme::MUTED).width(110),
            slider(-15.0..=15.0, s.float(mascot::WAVE_X), |x| Message::Set(mascot::WAVE_X, Value::Float(x)))
                .step(1.0).style(theme::range),
            text(format!("{:.0}%", s.float(mascot::WAVE_X))).size(12).width(48),
            button("Reset").style(theme::secondary).on_press(Message::Set(mascot::WAVE_X, Value::Float(0.0))),
        ].spacing(8).align_y(Alignment::Center).into(),
        row![
            text("Hand vertical").size(12).color(theme::MUTED).width(110),
            slider(-15.0..=15.0, s.float(mascot::WAVE_Y), |x| Message::Set(mascot::WAVE_Y, Value::Float(x)))
                .step(1.0).style(theme::range),
            text(format!("{:.0}%", s.float(mascot::WAVE_Y))).size(12).width(48),
            button("Reset").style(theme::secondary).on_press(Message::Set(mascot::WAVE_Y, Value::Float(0.0))),
        ].spacing(8).align_y(Alignment::Center).into(),
        row![
            text("Hand angle").size(12).color(theme::MUTED).width(110),
            slider(-45.0..=45.0, s.float(mascot::WAVE_ANGLE), |x| Message::Set(mascot::WAVE_ANGLE, Value::Float(x)))
                .step(1.0).style(theme::range),
            text(format!("{:.0}°", s.float(mascot::WAVE_ANGLE))).size(12).width(48),
            button("Reset").style(theme::secondary).on_press(Message::Set(mascot::WAVE_ANGLE, Value::Float(0.0))),
        ].spacing(8).align_y(Alignment::Center).into(),
        row![
            text("Wave range").size(12).color(theme::MUTED).width(110),
            slider(0.0..=200.0, s.float(mascot::WAVE_RANGE), |x| Message::Set(mascot::WAVE_RANGE, Value::Float(x)))
                .step(1.0).style(theme::range),
            text(format!("{:.0}%", s.float(mascot::WAVE_RANGE))).size(12).width(48),
            button("Reset").style(theme::secondary).on_press(Message::Set(mascot::WAVE_RANGE, Value::Float(100.0))),
        ].spacing(8).align_y(Alignment::Center).into(),
        hint("Hand position moves the waving arm and item together. Positive horizontal moves right; positive vertical moves down. These adjustments apply across characters and save immediately."),
        hint("Random uses the checked styles and changes on a beat boundary. Light wave joins Random when the character has a holding arm and an item. An empty selection uses Nod. Items use the available artwork; characters without gesture artwork use the built-in glow stick. Interval, included styles and item apply to all characters."),
        check("Random · Nod", s.bool(mascot::DANCE_STYLES[0]), mascot::DANCE_STYLES[0]),
        check("Random · Groove", s.bool(mascot::DANCE_STYLES[1]), mascot::DANCE_STYLES[1]),
        check("Random · Rubber hose", s.bool(mascot::DANCE_STYLES[2]), mascot::DANCE_STYLES[2]),
        check("Random · Compress", s.bool(mascot::DANCE_STYLES[3]), mascot::DANCE_STYLES[3]),
        check("Random · Light wave", s.bool(mascot::DANCE_STYLES[4]), mascot::DANCE_STYLES[4]),
        hint("Nod dips its head and shoulders on each beat, recovering smoothly and rocking across two beats. Groove dips on the beat and leans side to side. Rubber hose hops every beat, squashing and stretching like an old cartoon. Compress briefly squashes the whole picture on each beat, so it suits a single PNG. Light wave raises its right hand and sweeps the selected item across two beats. Add Music gestures artwork through Create from reference for a matching hand and interchangeable items. The selected style previews on the desktop immediately. Save keeps it with this character; Discard restores the saved style. It is carried when you export it."),
        check("Follow beat (trial)", follow, mascot::BEAT),
        hint("Nods land on the beat instead of a steady bob. It listens to what's playing (TinyPaw's own sounds are held off while they play) and keeps nothing."),
    ]);
    if follow {
        items.extend([
            row![
                text("Beat").size(12).color(theme::MUTED).width(110),
                text(v.beat.clone())
                    .size(12)
                    .color(theme::MUTED)
                    .width(Length::Fill),
            ]
            .align_y(Alignment::Center)
            .into(),
            row![
                text("Sensitivity").size(12).color(theme::MUTED).width(110),
                pick_list(
                    [
                        Choice("low", "Low"),
                        Choice("normal", "Normal"),
                        Choice("high", "High")
                    ],
                    Some(Choice::of(
                        s.choice(mascot::BEAT_SENSITIVITY),
                        &[("low", "Low"), ("normal", "Normal"), ("high", "High")]
                    )),
                    |c| Message::Set(mascot::BEAT_SENSITIVITY, Value::Choice(c.0)),
                )
                .text_size(13)
                .padding([6, 10])
                .style(theme::select),
                text("Timing").size(12).color(theme::MUTED),
                pick_list(
                    [
                        Choice("earlier", "Earlier"),
                        Choice("default", "Default"),
                        Choice("later", "Later")
                    ],
                    Some(Choice::of(
                        s.choice(mascot::BEAT_TIMING),
                        &[
                            ("earlier", "Earlier"),
                            ("default", "Default"),
                            ("later", "Later")
                        ]
                    )),
                    |c| Message::Set(mascot::BEAT_TIMING, Value::Choice(c.0)),
                )
                .text_size(13)
                .padding([6, 10])
                .style(theme::select),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
            row![
                text("Engine").size(12).color(theme::MUTED).width(110),
                pick_list(
                    [
                        Choice("classic", "Classic"),
                        Choice("bass", "Bass-weighted"),
                        Choice("sticky", "Sticky tempo"),
                        Choice("cumulative", "Cumulative")
                    ],
                    Some(Choice::of(
                        s.choice(mascot::BEAT_ENGINE),
                        &[
                            ("classic", "Classic"),
                            ("bass", "Bass-weighted"),
                            ("sticky", "Sticky tempo"),
                            ("cumulative", "Cumulative")
                        ]
                    )),
                    |c| Message::Set(mascot::BEAT_ENGINE, Value::Choice(c.0)),
                )
                .text_size(13)
                .padding([6, 10])
                .style(theme::select),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
            hint("Ways to follow the beat, each adding to the one before. Classic: the original. Bass-weighted: kicks count more than hats. Sticky tempo: resists jumping to half or double time. Cumulative: carries the beat's phase forward between hits. Changing it restarts the search."),
        ]);
    }
    items.push(hint("Try it without music using Music in the preview."));
    section(items)
}

/// A setting value with its display label, for pick lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Choice(pub(super) &'static str, pub(super) &'static str);

impl Choice {
    pub(super) fn of(value: &'static str, all: &[(&'static str, &'static str)]) -> Self {
        let label = all
            .iter()
            .find(|(v, _)| *v == value)
            .map_or(value, |(_, l)| l);
        Choice(value, label)
    }
}

impl std::fmt::Display for Choice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.1)
    }
}

/// The selected character, animated, with buttons to try its reactions.
pub(super) fn motion_preview<'a>(
    m: &'a crate::mascot::preview::MotionPreview,
) -> Element<'a, Message> {
    use crate::mascot::preview::Test;
    let test = |label: &'static str, t: Test| {
        button(text(label).size(12).width(Length::Fill).center())
            .width(Length::Fill)
            .padding([7, 8])
            .style(theme::secondary)
            .on_press(Message::Ui(UiMsg::MotionTest(t)))
    };
    let b = m.lines.bubble;
    let bubble: Element<'a, Message> = match m.speech() {
        Some(said) => container(
            text(said.to_owned())
                .size(13)
                .color(Color::from_rgba8(b.text[0], b.text[1], b.text[2], 1.0)),
        )
        .padding([8, 12])
        .max_width(b.max_width.min(260.0))
        .style(move |_: &Theme| container::Style {
            background: Some(
                Color::from_rgba8(b.background[0], b.background[1], b.background[2], 1.0).into(),
            ),
            border: iced::Border {
                radius: 14.0.into(),
                width: 1.0,
                color: Color::from_rgba8(0, 0, 0, 0.18),
            },
            ..container::Style::default()
        })
        .into(),
        None => space().height(34).into(),
    };
    column![
        h2("Preview"),
        mode_label("Test character", "Reactions and gaze as on the desktop."),
        container(bubble).width(Length::Fill).center_x(Length::Fill),
        container(canvas(MotionStage { preview: m }).width(Length::Fill).height(if m.alignment.is_some() { 220 } else { 300 })).clip(true),
        alignment_controls(m),
        row![
            test("Click", Test::Click),
            test("Drag ←", Test::DragLeft),
            test("Drag →", Test::DragRight),
            test("Speak", Test::Speak),
            test("Try beat", Test::Music),
        ]
        .spacing(6),
        hint(
            "Idle motion follows Character → On desktop. Reduced motion stops it, as on the desktop."
        ),
    ]
    .spacing(10)
    .width(260)
    .into()
}

fn alignment_controls(m: &crate::mascot::preview::MotionPreview) -> Element<'_, Message> {
    use crate::mascot::alignment::Action;
    let Some(a) = &m.alignment else {
        return if m.held().is_some() {
            button("Align held pose")
                .on_press(Message::Ui(UiMsg::Align(Action::Open(Event::Pickup))))
                .into()
        } else {
            space().height(0).into()
        };
    };
    let message = |action| Message::Ui(UiMsg::Align(action));
    let number = |label: &'static str, index: usize| {
        column![
            text(label).size(11),
            iced::widget::text_input("0", &a.numbers[index])
                .on_input(move |value| Message::Ui(UiMsg::Align(Action::Number(index, value))))
                .size(12),
        ]
    };
    let mut controls = column![
        text("Align poses").size(15),
        text("Drag the pose to move it. Arrow keys nudge. Size changes the pose. Zoom changes this preview.").size(12),
        button(if a.side_by_side { "Use overlay" } else { "Show side by side" })
            .on_press(message(Action::Compare(!a.side_by_side))),
        text("Overlay opacity").size(12),
        slider(0.0..=1.0, a.opacity, move |value| message(Action::Opacity(value)))
            .step(0.01_f32),
        text(format!("Zoom {:.0}%", a.zoom * 100.0)).size(12),
        slider(0.1..=2.0, a.zoom, move |value| message(Action::Zoom(value)))
            .step(0.05_f32),
        row![number("Left/right %", 0), number("Up/down %", 1), number("Size %", 2)]
            .spacing(5),
        button("Match head size").on_press(message(Action::Match)),
        row![
            button("Reset").on_press(message(Action::Reset)),
            button("Cancel").on_press(message(Action::Finish(false))),
            button("Apply").on_press_maybe(a.error.is_none().then(|| message(Action::Finish(true)))),
        ].spacing(5),
    ].spacing(6);
    if a.matching {
        controls = controls.push(text(a.instruction()).size(13));
    }
    if let Some(error) = &a.error {
        controls = controls.push(text(error).size(12));
    }
    controls.into()
}

#[derive(Default)]
pub(super) struct PoseDrag {
    start: Option<(iced::Point, [f32; 3])>,
    focused: bool,
    gripping: bool,
}

pub(super) struct MotionStage<'a> {
    preview: &'a crate::mascot::preview::MotionPreview,
}

impl MotionStage<'_> {
    /// The rig's scale and top-left on the stage, feet on the floor line.
    fn fit(&self, size: iced::Size) -> (f32, iced::Vector) {
        let rig = self.preview.rig();
        let canvas_size = rig.size();
        let pad = 18.0;
        let side = self
            .preview
            .alignment
            .as_ref()
            .is_some_and(|a| a.side_by_side);
        let available_width = if side { size.width / 2.0 } else { size.width };
        let k = ((available_width - pad * 2.0) / canvas_size.width)
            .min((size.height - pad * 2.0) / canvas_size.height)
            * self.preview.alignment.as_ref().map_or(1.0, |a| a.zoom);
        let feet = rig.anchor();
        let origin = iced::Vector::new(
            (if side {
                size.width * 0.75
            } else {
                size.width / 2.0
            }) - feet.x * k,
            size.height - pad - feet.y * k,
        );
        (k, origin)
    }
}

impl canvas::Program<Message> for MotionStage<'_> {
    type State = PoseDrag;

    /// Drives the animation: each redraw advances the preview and asks for
    /// the next one, so only this canvas repaints per frame. While picking
    /// the bubble's anchor, a click on the stage sets it.
    fn update(
        &self,
        state: &mut PoseDrag,
        event: &canvas::Event,
        bounds: iced::Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        use iced::mouse::{Button, Event};
        let (k, origin) = self.fit(bounds.size());
        let point = cursor
            .position_in(bounds)
            .map(|p| iced::Point::new((p.x - origin.x) / k, (p.y - origin.y) / k));
        let button = match event {
            canvas::Event::Mouse(Event::ButtonPressed(_)) => Some(true),
            canvas::Event::Mouse(Event::ButtonReleased(_)) => Some(false),
            _ => None,
        };
        self.preview.companion_pointer(point, button);
        if let canvas::Event::Window(iced::window::Event::RedrawRequested(now)) = event {
            // The page shows the speech bubble, so it needs rebuilding when
            // the bubble appears or goes.
            if self.preview.advance(*now) {
                return Some(canvas::Action::publish(Message::MotionTick));
            }
            // Match the display with a short wait: an immediate redraw would
            // spin faster than the screen refreshes.
            let wait = self
                .preview
                .control()
                .period
                .unwrap_or(std::time::Duration::from_millis(7));
            return Some(canvas::Action::request_redraw_at(*now + wait));
        }
        if let Some(a) = &self.preview.alignment {
            use crate::mascot::alignment::Action;
            let publish =
                |action| canvas::Action::publish(Message::Ui(UiMsg::Align(action))).and_capture();
            if let canvas::Event::Mouse(Event::CursorMoved { .. }) = event
                && state.gripping
                    && let (Some(point), Some(picture)) = (point, self.preview.placement_picture())
                    {
                        let grip = picture.source_fraction(self.preview.rig().size(), point);
                        return Some(
                            canvas::Action::publish(Message::Ui(UiMsg::GripSet(Some([
                                grip.x.clamp(0.0, 1.0),
                                grip.y.clamp(0.0, 1.0),
                            ]))))
                            .and_capture(),
                        );
                    }
            if let canvas::Event::Mouse(Event::ButtonPressed(Button::Left)) = event {
                state.focused = point.is_some();
                if let Some(point) = point {
                    if a.matching {
                        let mut point = point;
                        if a.side_by_side && a.points.len() < 2 {
                            point.x += bounds.width / (2.0 * k);
                        }
                        return Some(publish(Action::Point(point)));
                    }
                    if a.event == crate::mascot::reactions::Event::Pickup
                        && let Some(picture) = self.preview.placement_picture()
                            && point.distance(picture.hold_in(self.preview.rig().size())) < 9.0 / k
                            {
                                state.gripping = true;
                                return Some(canvas::Action::capture());
                            }
                    if !self.preview.placing_grip {
                        state.start = Some((point, a.art.place));
                        return Some(canvas::Action::capture());
                    }
                }
            }
            if let canvas::Event::Mouse(Event::ButtonReleased(Button::Left)) = event {
                state.start = None;
                state.gripping = false;
            }
            if let canvas::Event::Mouse(Event::CursorMoved { .. }) = event
                && let (Some((start, original)), Some(point)) = (state.start, point) {
                    let stage = self.preview.rig().size();
                    return Some(publish(Action::Place([
                        (original[0] + (point.x - start.x) / stage.width).clamp(-1.0, 1.0),
                        (original[1] + (point.y - start.y) / stage.height).clamp(-1.0, 1.0),
                        original[2],
                    ])));
                }
            if state.focused
                && let canvas::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Named(key),
                    ..
                }) = event
                {
                    use iced::keyboard::key::Named;
                    let mut place = a.art.place;
                    let size = self.preview.rig().size();
                    match key {
                        Named::ArrowLeft => place[0] -= 1.0 / size.width,
                        Named::ArrowRight => place[0] += 1.0 / size.width,
                        Named::ArrowUp => place[1] -= 1.0 / size.height,
                        Named::ArrowDown => place[1] += 1.0 / size.height,
                        _ => return None,
                    }
                    place[0] = place[0].clamp(-1.0, 1.0);
                    place[1] = place[1].clamp(-1.0, 1.0);
                    return Some(publish(Action::Place(place)));
                }
        }
        if self.preview.placing_grip {
            if let canvas::Event::Mouse(Event::ButtonPressed(Button::Left)) = event
                && let (Some(point), Some(picture)) = (point, self.preview.placement_picture()) {
                    let size = self.preview.rig().size();
                    let fraction = picture.source_fraction(size, point);
                    let grip = [fraction.x, fraction.y];
                    if grip.iter().all(|v| (0.0..=1.0).contains(v)) {
                        return Some(
                            canvas::Action::publish(Message::Ui(UiMsg::GripSet(Some(grip))))
                                .and_capture(),
                        );
                    }
                }
            return None;
        }
        if self.preview.placing_mouth
            && let canvas::Event::Mouse(Event::ButtonPressed(Button::Left)) = event
                && let Some(p) = point {
                    let size = self.preview.rig().size();
                    let mut s = self.preview.companionship();
                    s.mouth = [
                        (p.x / size.width).clamp(0.0, 1.0),
                        (p.y / size.height).clamp(0.0, 1.0),
                    ];
                    return Some(
                        canvas::Action::publish(Message::Ui(UiMsg::Companionship(s))).and_capture(),
                    );
                }
        if !self.preview.placing_bubble {
            return None;
        }
        let canvas::Event::Mouse(Event::ButtonPressed(Button::Left)) = event else {
            return None;
        };
        let at = cursor.position_in(bounds)?;
        let (k, origin) = self.fit(bounds.size());
        let size = self.preview.rig().size();
        let (x, y) = (
            (at.x - origin.x) / k / size.width,
            (at.y - origin.y) / k / size.height,
        );
        ((0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y)).then(|| {
            canvas::Action::publish(Message::Ui(UiMsg::BubbleAnchor(Some((x, y))))).and_capture()
        })
    }

    fn mouse_interaction(
        &self,
        _state: &PoseDrag,
        bounds: iced::Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> iced::mouse::Interaction {
        if (self.preview.placing_bubble || self.preview.placing_mouth || self.preview.placing_grip)
            && cursor.is_over(bounds)
        {
            iced::mouse::Interaction::Crosshair
        } else {
            iced::mouse::Interaction::default()
        }
    }

    fn draw(
        &self,
        _state: &PoseDrag,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: iced::Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        use iced::widget::canvas::{Frame, Path};
        let size = bounds.size();
        let mut frame = Frame::new(renderer, size);
        frame.fill(
            &Path::rounded_rectangle(iced::Point::ORIGIN, size, 12.0.into()),
            theme::WELL,
        );
        let rig = self.preview.rig();
        let mut pose = self.preview.pose().clone();
        let canvas_size = rig.size();
        let feet = rig.anchor();
        // Stand the feet on a floor line near the bottom, centred.
        let (k, origin) = self.fit(size);
        // Where the bubble's tail aims, while it's being picked.
        let marker = self.preview.placing_bubble.then(|| {
            let (x, y) = self.preview.lines.bubble.anchor.unwrap_or((
                feet.x / canvas_size.width,
                rig.visible_rows().0 / canvas_size.height,
            ));
            iced::Point::new(
                origin.x + x * canvas_size.width * k,
                origin.y + y * canvas_size.height * k,
            )
        });
        let marker = if self.preview.placing_mouth {
            let m = self.preview.companionship().mouth;
            Some(iced::Point::new(
                origin.x + m[0] * canvas_size.width * k,
                origin.y + m[1] * canvas_size.height * k,
            ))
        } else {
            marker
        };
        // A live cursor test: tracking eyes follow the pointer over the
        // preview, counter-rotated into the head like on the desktop.
        if !matches!(
            self.preview.companion_state(),
            crate::mascot::companionship::State::Asleep
                | crate::mascot::companionship::State::Waking
        ) && let (Some(eye), Some(at)) = (rig.eye_point(), cursor.position_in(bounds))
        {
            let eyes = iced::Point::new(origin.x + eye.x * k, origin.y + eye.y * k);
            let g = crate::mascot::desktop::gaze_toward(at, eyes, 0.49 * canvas_size.height * k);
            let (sn, cs) = (-rig.head_angle(&pose)).sin_cos();
            pose.gaze = iced::Vector::new(g.x * cs - g.y * sn, g.x * sn + g.y * cs);
        }
        frame.translate(origin);
        frame.scale(k);
        if let Some(picture) = self.preview.placement_picture() {
            if let Some(a) = &self.preview.alignment {
                if !a.matching || a.points.len() < 2 {
                    frame.with_save(|frame| {
                        if a.side_by_side {
                            frame.translate(iced::Vector::new(-size.width / (2.0 * k), 0.0));
                        }
                        rig.draw_pose(frame, &crate::mascot::motion::Pose::default());
                    });
                }
                if !a.matching || a.points.len() >= 2 {
                    rig.draw_picture_alpha(
                        &mut frame,
                        &picture,
                        if a.matching || a.side_by_side {
                            1.0
                        } else {
                            a.opacity
                        },
                    );
                }
                let feet = rig.anchor();
                frame.stroke(
                    &canvas::Path::line(
                        iced::Point::new(0.0, feet.y),
                        iced::Point::new(canvas_size.width, feet.y),
                    ),
                    canvas::Stroke::default()
                        .with_color(iced::Color::from_rgb(0.4, 0.7, 0.5))
                        .with_width(1.0 / k),
                );
            } else {
                rig.draw_picture(&mut frame, &picture);
            }
            if self.preview.placement_preview == Some(Event::Pickup) {
                frame.stroke(
                    &canvas::Path::circle(picture.hold_in(canvas_size), 6.0 / k),
                    canvas::Stroke::default()
                        .with_color(iced::Color::from_rgb(1.0, 0.3, 0.4))
                        .with_width(2.0 / k),
                );
            }
            return vec![frame.into_geometry()];
        }
        if let Some((held, grip, angle)) = self.preview.hanging() {
            rig.draw_hanging(&mut frame, &held, grip, angle);
            return vec![frame.into_geometry()];
        }
        crate::mascot::motion::place(&mut frame, &pose, feet, canvas_size.height);
        match self.preview.picture() {
            Some(picture) => rig.draw_picture(&mut frame, &picture),
            None => rig.draw_pose(&mut frame, &pose),
        }
        let mut sleep_overlay = Frame::new(renderer, size);
        sleep_overlay.translate(origin);
        sleep_overlay.scale(k);
        crate::mascot::motion::place(&mut sleep_overlay, &pose, feet, canvas_size.height);
        crate::mascot::companionship::draw_zzz(
            &mut sleep_overlay,
            canvas_size,
            &self.preview.companionship(),
            self.preview.companion_state(),
            self.preview.stirring(),
            self.preview.time(),
            self.preview.control().reduced,
        );
        self.preview.draw_affection(&mut sleep_overlay);
        let mut out = vec![frame.into_geometry(), sleep_overlay.into_geometry()];
        if let Some(at) = marker {
            let mut ui = Frame::new(renderer, size);
            let ring = Path::circle(at, 7.0);
            ui.fill(&ring, iced::Color::from_rgba(0.13, 0.83, 0.93, 0.35));
            ui.stroke(
                &ring,
                canvas::Stroke::default()
                    .with_color(theme::CYAN_TEXT)
                    .with_width(2.0),
            );
            out.push(ui.into_geometry());
        }
        out
    }
}

fn companionship_section<'a>(
    m: &'a crate::mascot::preview::MotionPreview,
    muted: bool,
) -> Element<'a, Message> {
    use crate::mascot::companionship::State;
    let s = m.companionship();
    let toggle = |label: &'static str, value: bool, key: usize| {
        let settings = s.clone();
        checkbox(value)
            .label(label)
            .text_size(13)
            .on_toggle(move |on| {
                let mut settings = settings.clone();
                match key {
                    0 => settings.petting = on,
                    1 => settings.sleep = on,
                    2 => settings.touch_wake = on,
                    3 => settings.audio_wake = on,
                    4 => settings.random_wake = on,
                    5 => settings.zzz = on,
                    _ => settings.breathing = on,
                }
                Message::Ui(UiMsg::Companionship(settings))
            })
    };
    let mut timing = column![].spacing(6);
    for (key, values, label) in [
        (0, s.touch_seconds, "Gentle wake, seconds"),
        (1, s.random_seconds, "Random wake, minutes"),
    ] {
        let factor = if key == 0 { 1.0 } else { 60.0 };
        timing = timing.push(
            text(format!(
                "{label}: {:.1}–{:.1}",
                values[0] / factor,
                values[1] / factor
            ))
            .size(12),
        );
        for index in 0..2 {
            let current = s.clone();
            let range = if key == 0 { 0.2..=10.0 } else { 1.0..=120.0 };
            timing = timing.push(
                slider(range, values[index] / factor, move |value| {
                    let mut s = current.clone();
                    let range = if key == 0 {
                        &mut s.touch_seconds
                    } else {
                        &mut s.random_seconds
                    };
                    range[index] = value * factor;
                    if index == 0 {
                        range[1] = range[1].max(range[0]);
                    } else {
                        range[0] = range[0].min(range[1]);
                    }
                    Message::Ui(UiMsg::Companionship(s))
                })
                .step(if key == 0 { 0.1_f32 } else { 1.0_f32 })
                .style(theme::range),
            );
        }
    }
    let delay = s.clone();
    let mouth_x = s.clone();
    let mouth_y = s.clone();
    section(vec![
        h2("Headpat"),
        toggle("Enable headpat", s.petting, 0).into(),
        hint(
            "Rub back and forth without holding a button. The head sways with your strokes and hearts float up. Gentle touch gives a smaller lean.",
        ),
        button("Try headpat")
            .style(theme::secondary)
            .on_press(Message::Ui(UiMsg::CompanionTest(State::Petting)))
            .into(),
        event_row(m, crate::mascot::lines::Trigger::Headpat),
        visual_row(m, Event::Headpat),
        sound_row(m, Event::Headpat, muted),
        h2("Sleep & wake"),
        toggle("Sleep when quiet and the mouse stays away", s.sleep, 1).into(),
        text(format!("Sleep after {:.0} minutes", s.delay / 60.0))
            .size(13)
            .into(),
        slider(60.0..=3600.0, s.delay, move |v| {
            let mut s = delay.clone();
            s.delay = v;
            Message::Ui(UiMsg::Companionship(s))
        })
        .step(60.0_f32)
        .style(theme::range)
        .into(),
        hint(
            "Typing and mouse movement elsewhere let it sleep. Audio monitoring runs while sleep is enabled. Gentle touch wakes it gradually; clicks and pickup wake immediately.",
        ),
        toggle("Wake from gentle touch", s.touch_wake, 2).into(),
        toggle("Wake when audio starts", s.audio_wake, 3).into(),
        toggle("Wake randomly after sleeping", s.random_wake, 4).into(),
        toggle("Show Zzz from the mouth", s.zzz, 5).into(),
        toggle("Gentle breathing while asleep", s.breathing, 6).into(),
        timing.into(),
        button("Place mouth in preview…")
            .style(theme::secondary)
            .on_press(Message::Ui(UiMsg::CompanionMouth))
            .into(),
        hint("Mouth position on the shared canvas (left/right, then up/down)."),
        slider(0.0..=1.0, s.mouth[0], move |v| {
            let mut s = mouth_x.clone();
            s.mouth[0] = v;
            Message::Ui(UiMsg::Companionship(s))
        })
        .step(0.01_f32)
        .style(theme::range)
        .into(),
        slider(0.0..=1.0, s.mouth[1], move |v| {
            let mut s = mouth_y.clone();
            s.mouth[1] = v;
            Message::Ui(UiMsg::Companionship(s))
        })
        .step(0.01_f32)
        .style(theme::range)
        .into(),
        row![
            button("Sleep now")
                .style(theme::secondary)
                .on_press(Message::Ui(UiMsg::CompanionTest(State::Asleep))),
            button("Try wake")
                .style(theme::secondary)
                .on_press(Message::Ui(UiMsg::CompanionTest(State::Awake)))
        ]
        .spacing(8)
        .into(),
        hint(
            "Preview sleep and wake are simulated. A sleeping pose is optional; add artwork for a curled-up character.",
        ),
        event_row(m, crate::mascot::lines::Trigger::Sleep),
        visual_row(m, Event::Sleep),
        sound_row(m, Event::Sleep, muted),
        event_row(m, crate::mascot::lines::Trigger::Wake),
        visual_row(m, Event::Wake),
        sound_row(m, Event::Wake, muted),
    ])
}
