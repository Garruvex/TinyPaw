//! Settings → Character: the character home, the Customize workspace
//! (artwork, behavior, advanced) and its previews.
//!
//! A child of `settings`, so it shares that module's helpers and types.

use super::*;
use iced::widget::{column, row};

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

/// Create from reference: choices, the requested files, Copy art prompt
/// and Import results.
fn reference_panel<'a>(
    r: &'a crate::mascot::reference::Request,
    thumb: Option<&'a iced::advanced::image::Handle>,
) -> Element<'a, Message> {
    use crate::mascot::reference::{CANVASES, Role};
    let small = |label: String, msg: Message| {
        button(text(label).size(12))
            .padding([5, 10])
            .style(theme::secondary)
            .on_press(msg)
    };
    let preview: Element<'a, Message> = match thumb {
        Some(h) => iced::widget::image(h.clone()).height(120).into(),
        None => hint("No preview for this file type."),
    };
    let canvas = pick_list(CANVASES.to_vec(), Some(r.canvas), |c| {
        Message::Ui(UiMsg::ReferenceCanvas(c))
    })
    .text_size(13)
    .padding([5, 8])
    .style(theme::select);
    let mut files = column![].spacing(6);
    for role in r.roles() {
        files = files.push(
            row![
                text(role.file()).size(13).width(90),
                text(if r.file_for(role).is_some() {
                    "✓ imported"
                } else {
                    ""
                })
                .size(12)
                .color(theme::MUTED)
                .width(Length::Fill),
                small(
                    "Copy prompt".into(),
                    Message::Ui(UiMsg::ReferenceCopy(Some(role)))
                ),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );
    }
    let mut results = column![].spacing(6);
    for (i, (path, role)) in r.results.iter().enumerate() {
        let mut choices: Vec<Option<Role>> = vec![None];
        choices.extend(r.roles().into_iter().map(Some));
        results = results.push(
            row![
                text(
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                )
                .size(13)
                .width(Length::Fill),
                pick_list(
                    choices.into_iter().map(RolePick).collect::<Vec<_>>(),
                    Some(RolePick(*role)),
                    move |p: RolePick| Message::Ui(UiMsg::ReferenceAssign(i, p.0)),
                )
                .text_size(13)
                .padding([5, 8])
                .style(theme::select),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );
    }
    let missing = r.missing();
    column![
        section(vec![
            h2("Create from reference"),
            row![preview, column![
                text(format!("Reference: {}", r.reference.file_name().unwrap_or_default().to_string_lossy())).size(13),
                hint("TinyPaw writes the request; your own image tool makes the art. The reference stays on this PC and is never put in an exported character."),
                iced::widget::text_input("Style, such as soft watercolour or flat vector", &r.style)
                    .on_input(|s| Message::Ui(UiMsg::ReferenceStyle(s)))
                    .size(13)
                    .padding([7, 9]),
                row![text("Canvas size").size(12).color(theme::MUTED), canvas, hint("pixels square; on-screen size is set later")]
                    .spacing(8)
                    .align_y(Alignment::Center),
            ].spacing(8)]
            .spacing(12)
            .into(),
        ]),
        section(vec![
            h2("Features to create"),
            hint("Each choice adds every required image and its mapping to the folder prompt."),
            checkbox(r.tracking).label("Create tracking eyes (background, pupil and mask for each eye, plus a separate head)").text_size(13).on_toggle(|on| Message::Ui(UiMsg::ReferenceTracking(on))).into(),
            checkbox(r.head).label("Create a head that bops to music (separate head and neck pivot)").text_size(13).on_toggle(|on| Message::Ui(UiMsg::ReferenceHead(on))).into(),
            checkbox(r.tail).label("Create a moving tail (separate tail and attachment pivot)").text_size(13).on_toggle(|on| Message::Ui(UiMsg::ReferenceTail(on))).into(),
            checkbox(r.eyes).label("Separate eyes (left and right eye images)").text_size(13).on_toggle(|on| Message::Ui(UiMsg::ReferenceEyes(on))).into(),
            checkbox(r.brows).label("Separate eyebrows (left and right eyebrow images)").text_size(13).on_toggle(|on| Message::Ui(UiMsg::ReferenceBrows(on))).into(),
            checkbox(r.blink && r.faces_possible())
                .label("Create blinking (closed eyes for the selected eye style)")
                .text_size(13)
                .on_toggle(|on| Message::Ui(UiMsg::ReferenceBlink(on)))
                .into(),
            checkbox(r.faces && r.faces_possible())
                .label("Create expressions when pressed and carried")
                .text_size(13)
                .on_toggle(|on| Message::Ui(UiMsg::ReferenceFaces(on)))
                .into(),
            hint("Blinking and expressions include their required head automatically. Tracking eyes use separate closed-eye images. Complete eye pictures switch off blinking and expressions."),
            hint("The prompt includes shared-canvas x/y placement, neck and tail pivots, eye travel and held-pose alignment. The art tool must measure and save the coordinates in character.json."),
            checkbox(r.held).label("Create a held pose (complete hanging character and pickup mapping)").text_size(13).on_toggle(|on| Message::Ui(UiMsg::ReferenceHeld(on))).into(),
        ]),
        section(vec![
            h2("Artwork to request"),
            hint(if r.fits_canvas() { "Every listed file is required for your selected features." } else { "Choose 1024 or 512 pixels. These feature images exceed the character memory limit at this canvas size." }),
            files.into(),
            row![
                button(text("Copy character folder prompt").font(theme::MEDIUM).size(13))
                    .padding([7, 14])
                    .style(theme::primary)
                    .on_press_maybe(r.fits_canvas().then_some(Message::Ui(UiMsg::ReferenceCopy(None)))),
                hint("Attach your reference in a tool that can create images and files. This prompt requests every PNG and its matching character.json."),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
        ]),
        section(vec![
            h2("Results"),
            small("Import folder…".into(), Message::Ui(UiMsg::ImportCharacter)).into(),
            hint("Extract the ZIP if needed, then choose the folder containing character.json."),
            small("Import individual PNGs…".into(), Message::Ui(UiMsg::ReferenceImport)).into(),
            results.into(),
            hint(if missing.is_empty() {
                "Every requested file is assigned. Open it to line the layers up, try it, and save.".to_owned()
            } else {
                format!("Still missing: {}.", missing.iter().map(|r| r.file()).collect::<Vec<_>>().join(", "))
            }),
            row![
                button(text("Cancel").size(13)).padding([7, 12]).style(theme::secondary).on_press(Message::Ui(UiMsg::ReferenceCancel)),
                button(text("Open in Arrange").font(theme::MEDIUM).size(13))
                    .padding([7, 14])
                    .style(theme::primary)
                    .on_press_maybe((missing.is_empty() && r.fits_canvas()).then_some(Message::Ui(UiMsg::ReferenceOpen))),
            ]
            .spacing(8)
            .into(),
        ]),
    ]
    .spacing(22)
    .into()
}

/// A result's role in the picker, or Not used.
#[derive(Debug, Clone, Copy, PartialEq)]
struct RolePick(Option<crate::mascot::reference::Role>);

impl std::fmt::Display for RolePick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(r) => write!(f, "{r}"),
            None => f.write_str("Not used"),
        }
    }
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
            c.name.clone(),
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

/// Behavior → Moving parts: a preset, strength and speed for each part.
fn motion_section<'a>(m: &'a crate::mascot::preview::MotionPreview) -> Element<'a, Message> {
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
fn behavior_section<'a>(
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
                hint("Fall drops it straight down to the bottom of the screen it was released on. Picking it up mid-fall catches it."),
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
struct SoundPick(Option<Sound>);

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
fn character_sounds<'a>(
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
fn visual_row<'a>(m: &'a crate::mascot::preview::MotionPreview, e: Event) -> Element<'a, Message> {
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
    block.into()
}

/// Pick up → Held pose: keep the artwork, or show another PNG while carried.
fn held_row<'a>(m: &'a crate::mascot::preview::MotionPreview) -> Element<'a, Message> {
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
            hint("Line the held pose up so the part people grab sits where it does in the normal pose; the preview shows it while carried."),
        ]
        .spacing(6)
        .into(),
    }
}

/// Moves and resizes a replacement picture on the stage.
fn picture_controls<'a>(e: Event, held: bool, place: [f32; 3]) -> Element<'a, Message> {
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
        slider_row("Left/right", 0, -0.5..=0.5, 0.005),
        slider_row("Up/down", 1, -0.5..=0.5, 0.005),
        slider_row("Size", 2, 0.25..=2.0, 0.01),
    ]
    .spacing(4)
    .into()
}

/// An event's sound: picker, Import WAV…, Clear, Test, and its cooldown.
fn sound_row<'a>(
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
fn event_row<'a>(
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
fn speech_options<'a>(
    v: &SettingsView<'a>,
    m: &'a crate::mascot::preview::MotionPreview,
) -> Element<'a, Message> {
    use crate::mascot::lines::{MAX_WIDTH, MIN_WIDTH};
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
    ])
}

/// Idle lines' interval, shown only while idle lines can appear.
fn idle_interval<'a>(s: &crate::registry::SettingValues) -> Element<'a, Message> {
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
fn level_bar<'a>(level: f32, width: f32) -> Element<'a, Message> {
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

fn music_section<'a>(v: &SettingsView<'a>) -> Element<'a, Message> {
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
        hint("Nod is relaxed. Groove dips on the beat and leans side to side. Rubber hose hops every beat, squashing and stretching like an old cartoon. Compress briefly squashes the whole picture on each beat, so it suits a single PNG. Saved with the character's Save, and carried when you export it."),
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
        ]);
    }
    items.push(hint("Try it without music using Music in the preview."));
    section(items)
}

/// A setting value with its display label, for pick lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Choice(&'static str, &'static str);

impl Choice {
    fn of(value: &'static str, all: &[(&'static str, &'static str)]) -> Self {
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
fn motion_preview<'a>(m: &'a crate::mascot::preview::MotionPreview) -> Element<'a, Message> {
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
        canvas(MotionStage { preview: m })
            .width(Length::Fill)
            .height(300),
        row![
            test("Click", Test::Click),
            test("Drag ←", Test::DragLeft),
            test("Drag →", Test::DragRight),
            test("Speak", Test::Speak),
            test("Try beat", Test::Music),
        ]
        .spacing(6),
        hint(
            "Idle motion follows Settings → Character. Reduced motion stops it, as on the desktop."
        ),
    ]
    .spacing(10)
    .width(260)
    .into()
}

struct MotionStage<'a> {
    preview: &'a crate::mascot::preview::MotionPreview,
}

impl canvas::Program<Message> for MotionStage<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &(),
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
        let pad = 18.0;
        let k = ((size.width - pad * 2.0) / canvas_size.width)
            .min((size.height - pad * 2.0) / canvas_size.height);
        let feet = rig.anchor();
        // Stand the feet on a floor line near the bottom, centred.
        let origin = iced::Vector::new(
            size.width / 2.0 - feet.x * k,
            size.height - pad - feet.y * k,
        );
        // A live cursor test: tracking eyes follow the pointer over the
        // preview, counter-rotated into the head like on the desktop.
        if let (Some(eye), Some(at)) = (rig.eye_point(), cursor.position_in(bounds)) {
            let eyes = iced::Point::new(origin.x + eye.x * k, origin.y + eye.y * k);
            let g = crate::mascot::desktop::gaze_toward(at, eyes, 0.49 * canvas_size.height * k);
            let (sn, cs) = (-rig.head_angle(&pose)).sin_cos();
            pose.gaze = iced::Vector::new(g.x * cs - g.y * sn, g.x * sn + g.y * cs);
        }
        frame.translate(origin);
        frame.scale(k);
        if let Some((held, grip, angle)) = self.preview.hanging() {
            rig.draw_hanging(&mut frame, &held, grip, angle);
            return vec![frame.into_geometry()];
        }
        crate::mascot::motion::place(&mut frame, &pose, feet, canvas_size.height);
        match self.preview.picture() {
            Some(picture) => rig.draw_picture(&mut frame, &picture),
            None => rig.draw_pose(&mut frame, &pose),
        }
        vec![frame.into_geometry()]
    }
}

/// The Art tab: a character's body, moving parts, head faces and eyes.
fn art_page<'a>(e: &'a crate::mascot::editor::Editor, busy: bool) -> Element<'a, Message> {
    if busy {
        return column![
            h1("Preparing artwork…"),
            hint("Your draft stays available if preparation fails."),
            button("Cancel")
                .style(theme::secondary)
                .on_press(Message::Ui(UiMsg::ArtCancel))
        ]
        .spacing(12)
        .into();
    }
    use crate::mascot::editor::{Selected, Target};
    use crate::mascot::motion::PartType;
    let small = |label: String, msg: Message| {
        button(text(label).size(12))
            .padding([5, 10])
            .style(theme::secondary)
            .on_press(msg)
    };
    let thumb = |h: &iced::advanced::image::Handle| -> Element<'a, Message> {
        container(iced::widget::image(h.clone()).height(40).width(40))
            .width(48)
            .height(48)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(theme::fill(theme::WELL, 8.0))
            .into()
    };
    let seg = |label: &'static str, on: bool, msg: Message| -> Element<'a, Message> {
        button(text(label).size(12))
            .padding([5, 10])
            .style(theme::segment(on))
            .on_press(msg)
            .into()
    };

    // What the preview's clicks move.
    let mut targets = vec![seg(
        "Body",
        e.selected == Selected::Body,
        Message::Ui(UiMsg::ArtSelect(Selected::Body)),
    )];
    for (i, p) in e.parts.iter().enumerate() {
        targets.push(
            button(text(p.name.clone()).size(12))
                .padding([5, 10])
                .style(theme::segment(e.selected == Selected::Part(i)))
                .on_press(Message::Ui(UiMsg::ArtSelect(Selected::Part(i))))
                .into(),
        );
    }
    let targets = container(row(targets).spacing(4).wrap().vertical_spacing(4))
        .padding(4)
        .style(theme::fill(theme::WELL, 10.0));

    let head = e.head();
    let mut parts = column![].spacing(12);
    for (i, p) in e.parts.iter().enumerate() {
        let mut line = row![
            thumb(&p.layer.handle),
            iced::widget::text_input("Layer name", &p.name)
                .on_input(move |n| Message::Ui(UiMsg::ArtRename(i, n)))
                .size(13)
                .padding([6, 8])
                .width(150),
            pick_list(PartType::ALL, Some(p.kind), move |k| Message::Ui(
                UiMsg::ArtKind(i, k)
            ))
            .text_size(13)
            .padding([6, 10])
            .style(theme::select),
            container(
                row![
                    seg("Back", p.behind, Message::Ui(UiMsg::ArtBehind(i, true))),
                    seg("Front", !p.behind, Message::Ui(UiMsg::ArtBehind(i, false))),
                ]
                .spacing(2)
            )
            .padding(2)
            .style(theme::fill(theme::WELL, 8.0)),
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        if head.is_some() && p.kind != PartType::Head {
            line = line.push(hint("Moves with")).push(
                pick_list(
                    ["Character", "Head"],
                    Some(if p.on_head { "Head" } else { "Character" }),
                    move |w| Message::Ui(UiMsg::ArtOnHead(i, w == "Head")),
                )
                .text_size(13)
                .padding([5, 8])
                .style(theme::select),
            );
        }
        line = line
            .push(space().width(Length::Fill))
            .push(small("Remove".into(), Message::Ui(UiMsg::ArtRemovePart(i))));
        line = line
            .push(small(
                "Lower".into(),
                Message::Ui(UiMsg::ArtOrder(i, false)),
            ))
            .push(small("Raise".into(), Message::Ui(UiMsg::ArtOrder(i, true))));
        let mut block = column![line.wrap()].spacing(8);
        if p.kind == PartType::Head {
            let mut faces = row![hint("Faces:")].spacing(6).align_y(Alignment::Center);
            for (slot, name) in crate::mascot::editor::FACE_LABELS.iter().enumerate() {
                faces = faces.push(small(
                    if p.faces[slot].is_some() {
                        format!("{name} ✓")
                    } else {
                        format!("{name}…")
                    },
                    Message::Ui(UiMsg::ArtPick(Target::Face(i, slot))),
                ));
                if p.faces[slot].is_some() {
                    faces = faces.push(small(
                        format!("Arrange {name}"),
                        Message::Ui(UiMsg::ArtSelect(Selected::Face(i, slot))),
                    ));
                    faces = faces.push(
                        button(text("×").size(12))
                            .padding([5, 8])
                            .style(theme::tool_button(theme::Tone::Plain))
                            .on_press(Message::Ui(UiMsg::ArtRemoveFace(i, slot))),
                    );
                }
            }
            block = block.push(faces.wrap());
        }
        parts = parts.push(block);
    }

    let eyes_section = eyes_section(e, head.is_some());

    column![
        row![
            h1(if e.new_install { "Create from parts" } else { "Edit art" }),
            space().width(Length::Fill),
            button("Undo").style(theme::secondary).on_press_maybe(e.can_undo().then_some(Message::Ui(UiMsg::ArtUndo))),
            button("Redo").style(theme::secondary).on_press_maybe(e.can_redo().then_some(Message::Ui(UiMsg::ArtRedo))),
            button("Cancel")
                .style(theme::secondary)
                .on_press(Message::Ui(UiMsg::ArtCancel)),
            button(text("Save").font(theme::HEADING).size(13))
                .padding([8, 16])
                .style(theme::primary)
                .on_press_maybe(
                    (!e.name.trim().is_empty() && e.validation_error().is_none()).then_some(Message::Ui(UiMsg::ArtSave))
                ),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        iced::widget::text_input("Name", &e.name)
            .on_input(|n| Message::Ui(UiMsg::ArtName(n)))
            .size(14)
            .padding([9, 10])
            .style(|_, status| iced::widget::text_input::Style {
                background: theme::CONTROL.into(),
                border: iced::Border {
                    radius: 8.0.into(),
                    width: 1.0,
                    color: if matches!(status, iced::widget::text_input::Status::Focused { .. }) {
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
        section(vec![
            h2("Body"),
            row![
                thumb(&e.body.handle),
                small("Replace body…".into(), Message::Ui(UiMsg::ArtPick(Target::Body))),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
            hint("To give it a moving tail, use a body with the painted tail removed and add the tail as a part."),
        ]),
        section(vec![
            h2("Arrange"),
            targets.into(),
            joint_controls(e),
            placement_controls(e),
        ]),
        section(vec![
            h2("Moving parts"),
            parts.into(),
            small(
                "Add part PNGs…".into(),
                Message::Ui(UiMsg::ArtPick(Target::NewPart))
            )
            .padding([7, 12])
            .into(),
            hint("Ears and hair ride on the head. Back draws behind the body (or behind the head when on it)."),
        ]),
        eyes_section,
        hint(e.validation_error().unwrap_or_else(||"Drag to arrange; arrow keys nudge one pixel, Shift nudges ten. Artwork is resampled from its original source only when saved.".into())),
    ]
    .spacing(22)
    .max_width(640)
    .into()
}

/// "How are the eyes drawn?": keeps the current art and offers each
/// workflow with only its own controls. Tracking internals show only when
/// asked for.
fn eyes_section<'a>(e: &'a crate::mascot::editor::Editor, has_head: bool) -> Element<'a, Message> {
    use crate::mascot::editor::{EyeSlot, EyeStyle, Target};
    let style = e.eye_style();
    let choice = |label: &'static str, s: EyeStyle| -> Element<'a, Message> {
        button(text(label).size(12))
            .padding([5, 10])
            .style(theme::segment(style == s))
            .on_press(Message::Ui(UiMsg::ArtEyeStyle(s)))
            .into()
    };
    let small = |label: String, msg: Message| {
        button(text(label).size(12))
            .padding([5, 10])
            .style(theme::secondary)
            .on_press(msg)
    };
    let mut items: Vec<Element<'a, Message>> = vec![
        h2("Eyes"),
        text("How are the eyes drawn?").size(13).into(),
        row![
            choice("In the artwork", EyeStyle::InArt),
            choice("Separate eye images", EyeStyle::Separate),
            choice("Follow the cursor", EyeStyle::Tracking),
        ]
        .spacing(4)
        .wrap()
        .into(),
    ];
    match style {
        EyeStyle::InArt => items.push(hint(
            "Kept as drawn. Head faces (Eyes closed, Pressed, Being carried) can replace the head for reactions on the head's row above.",
        )),
        EyeStyle::Separate => {
            items.push(hint(if has_head {
                "Add complete eye and eyebrow PNGs. They're placed as still layers that move with the head; name them in the parts list."
            } else {
                "Add complete eye and eyebrow PNGs. They're placed as still layers; add a head part for them to move with it."
            }));
            items.push(
                small("Add eye or eyebrow PNGs…".into(), Message::Ui(UiMsg::ArtPick(Target::EyePart)))
                    .into(),
            );
        }
        EyeStyle::Tracking if !has_head => items.push(hint(
            "Eyes that follow the cursor ride on a head. Add a head part first, from Moving parts.",
        )),
        EyeStyle::Tracking => {
            items.push(hint(
                "Each eye needs three images on the same stage: the eye background, a moving pupil or iris, and a mask marking the area where the pupil is allowed to show. Blink is optional. Try it in Behavior: the Test character preview's eyes follow your pointer.",
            ));
            for (i, eye) in e.eyes.iter().enumerate() {
                let slot = |label: &str, set: bool, which: EyeSlot| {
                    small(
                        if set { format!("{label} ✓") } else { format!("{label} — missing") },
                        Message::Ui(UiMsg::ArtPick(Target::Eye(i, which))),
                    )
                };
                let complete = eye.white.is_some() && eye.pupil.is_some() && eye.mask.is_some();
                let mut images = row![
                    text(format!("Eye {}", i + 1)).size(13).width(48),
                    slot("Background", eye.white.is_some(), EyeSlot::White),
                    slot("Pupil", eye.pupil.is_some(), EyeSlot::Pupil),
                    slot("Pupil area", eye.mask.is_some(), EyeSlot::Mask),
                ]
                .spacing(6)
                .align_y(Alignment::Center);
                images = images.push(small(
                    if eye.blink.is_some() { "Blink ✓".into() } else { "Blink (optional)…".into() },
                    Message::Ui(UiMsg::ArtPick(Target::Eye(i, EyeSlot::Blink))),
                ));
                if eye.blink.is_some() {
                    images = images.push(
                        button(text("×").size(12))
                            .padding([5, 8])
                            .style(theme::tool_button(theme::Tone::Plain))
                            .on_press(Message::Ui(UiMsg::ArtRemoveBlink(i))),
                    );
                }
                images = images
                    .push(space().width(Length::Fill))
                    .push(small("Remove".into(), Message::Ui(UiMsg::ArtRemoveEye(i))));
                let mut block = column![images.wrap()].spacing(6);
                if let Some(pupil) = &eye.pupil {
                    block = block.push(
                        row![
                            text(format!("Iris / pupil size {:.0}%", pupil.placement.scale * 100.0)).size(12),
                            slider(0.01..=8.0, pupil.placement.scale, move |scale| Message::Ui(UiMsg::ArtPupilSize(i, scale)))
                                .step(0.01_f32)
                                .style(theme::range),
                        ]
                        .spacing(10)
                        .align_y(Alignment::Center),
                    );
                }
                if complete {
                    block = block.push(
                        row![
                            hint("Movement range"),
                            slider(0.0..=0.05, eye.travel, move |x| Message::Ui(UiMsg::ArtTravel(i, x)))
                                .step(0.001_f32)
                                .style(theme::range),
                        ]
                        .spacing(10)
                        .align_y(Alignment::Center),
                    );
                } else {
                    block = block
                        .push(hint("This eye can't follow the cursor until every required image is chosen. The character still saves with static eyes."))
                        .push(small("Keep these as static artwork".into(), Message::Ui(UiMsg::ArtStaticEye(i))))
                        .push(hint("Keeps the background and pupil as still head layers; the pupil area and blink are removed."));
                }
                items.push(block.into());
            }
            items.push(small("Add a tracking eye".into(), Message::Ui(UiMsg::ArtAddEye)).padding([7, 12]).into());
        }
    }
    section(items)
}

/// Customize → Advanced: the resting point and stage size. Neither is a
/// normal creation step.
fn advanced_page<'a>(e: &'a crate::mascot::editor::Editor) -> Element<'a, Message> {
    column![
        h1("Advanced"),
        section(vec![resting_point(e)]),
        section(vec![
            text("Stage").size(13).into(),
            hint(e.canvas_hint()),
            canvas_controls(e),
            hint(if e.clipped() {
                "Some artwork extends outside the stage and will be clipped when saved."
            } else {
                "Artwork fits inside the stage."
            }),
        ]),
    ]
    .spacing(22)
    .into()
}

/// Advanced → Resting point: Automatic, Adjust manually, Reset.
fn resting_point<'a>(e: &'a crate::mascot::editor::Editor) -> Element<'a, Message> {
    use crate::mascot::editor::RestAction;
    use crate::mascot::rest::RestMode;
    let state = match (e.adjusting_rest, e.rest) {
        (true, _) => "Click the preview where the character rests; arrow keys nudge it.",
        (false, RestMode::Auto) => "Automatic: estimated from the body's lowest visible art.",
        (false, RestMode::Manual) => "Set by hand. Reset to estimate it from the body again.",
    };
    let action = |label: &'static str, a: RestAction| {
        button(text(label).size(12))
            .padding([5, 10])
            .style(theme::secondary)
            .on_press(Message::Ui(UiMsg::ArtRest(a)))
    };
    let mut buttons = row![].spacing(6);
    buttons = if e.adjusting_rest {
        buttons.push(action("Done", RestAction::Done))
    } else {
        buttons.push(action("Adjust manually", RestAction::Adjust))
    };
    if e.rest == RestMode::Manual {
        buttons = buttons.push(action("Reset", RestAction::Reset));
    }
    column![
        text("Resting point").size(13),
        hint("Where the character rests when standing or bouncing."),
        buttons,
        hint(state),
    ]
    .spacing(6)
    .into()
}

fn canvas_controls<'a>(e: &'a crate::mascot::editor::Editor) -> Element<'a, Message> {
    if !e.can_resize_canvas() {
        return hint(
            "Canvas size is shared with other poses; resize is available for single-pose packs.",
        );
    }
    row![
        hint("Stage (1–2048 px)"),
        iced::widget::text_input("Width", &e.canvas_fields[0])
            .on_input(|s| Message::Ui(UiMsg::ArtCanvas(0, s)))
            .width(90)
            .padding(6)
            .size(13),
        text("×").size(13),
        iced::widget::text_input("Height", &e.canvas_fields[1])
            .on_input(|s| Message::Ui(UiMsg::ArtCanvas(1, s)))
            .width(90)
            .padding(6)
            .size(13)
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

/// Adjust joint: a temporary, labelled tool for where the selected part
/// turns. Ordinary dragging never edits a joint.
fn joint_controls<'a>(e: &'a crate::mascot::editor::Editor) -> Element<'a, Message> {
    use crate::mascot::editor::Selected;
    if let Some(i) = e.joint_part() {
        let name = e.parts.get(i).map_or("the part", |p| p.name.as_str());
        return row![
            container(text(format!("Adjusting the joint of {name}")).size(12).font(theme::MEDIUM))
                .padding([4, 8])
                .style(theme::fill(theme::AMBER_BG, 6.0)),
            button(text("Done").size(12))
                .padding([5, 12])
                .style(theme::primary)
                .on_press(Message::Ui(UiMsg::ArtJoint(None))),
            hint("Click where it turns, such as the neck or tail root. The outlines show its swing. Escape also finishes."),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .wrap()
        .into();
    }
    match e.selected {
        Selected::Part(i) if e.has_joint(i) => row![
            button(text("Adjust joint").size(12))
                .padding([5, 10])
                .style(theme::secondary)
                .on_press(Message::Ui(UiMsg::ArtJoint(Some(i)))),
            hint("Where this part turns when it moves."),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
        _ => space().height(0).into(),
    }
}

fn placement_controls<'a>(e: &'a crate::mascot::editor::Editor) -> Element<'a, Message> {
    if e.mode != crate::mascot::editor::Mode::Arrange || e.selected_layer().is_none() {
        return hint("Choose a layer and Arrange artwork to move, scale or rotate it.");
    }
    let fields = row(["X (px)", "Y (px)", "Scale (%)", "Angle (°)"]
        .into_iter()
        .enumerate()
        .map(|(i, label)| {
            column![
                hint(label),
                iced::widget::text_input("0", &e.fields[i])
                    .on_input(move |s| Message::Ui(UiMsg::ArtField(i, s)))
                    .padding(6)
                    .size(13)
            ]
            .width(100)
            .into()
        }))
    .spacing(8)
    .wrap();
    column![
        fields,
        row![
            button("←").on_press(Message::Ui(UiMsg::ArtNudge(-1.0, 0.0))),
            button("↑").on_press(Message::Ui(UiMsg::ArtNudge(0.0, -1.0))),
            button("↓").on_press(Message::Ui(UiMsg::ArtNudge(0.0, 1.0))),
            button("→").on_press(Message::Ui(UiMsg::ArtNudge(1.0, 0.0))),
            hint("Nudge one canvas pixel")
        ]
        .spacing(6)
        .align_y(Alignment::Center)
    ]
    .spacing(8)
    .into()
}

/// The assembled character at rest, with the ground and pivot markers.
fn art_preview<'a>(e: &'a crate::mascot::editor::Editor, busy: bool) -> Element<'a, Message> {
    use crate::mascot::editor::Selected;
    let placing = match e.selected {
        _ if e.adjusting_rest => "Placing: resting point".to_owned(),
        Selected::Body => "Selected: body".to_owned(),
        Selected::Face(i, slot) => format!(
            "Selected: head {} {}",
            i + 1,
            crate::mascot::editor::FACE_NAMES[slot]
        ),
        Selected::Part(i) => format!(
            "Placing: pivot of {} {}",
            i + 1,
            e.parts.get(i).map_or("part".into(), |p| p.kind.to_string())
        ),
    };
    let mode = if e.adjusting_rest || e.mode == crate::mascot::editor::Mode::Pivot {
        mode_label("Arrange", "Clicks place the point being adjusted.")
    } else {
        mode_label("Arrange", "Drag artwork to move the selected layer.")
    };
    column![
        h2("Preview"),
        mode,
        row![button("Light").style(theme::segment(e.background==crate::mascot::editor::Background::Light)).on_press(Message::Ui(UiMsg::ArtBackground(crate::mascot::editor::Background::Light))),
            button("Dark").style(theme::segment(e.background==crate::mascot::editor::Background::Dark)).on_press(Message::Ui(UiMsg::ArtBackground(crate::mascot::editor::Background::Dark))),
            button("Grid").style(theme::segment(e.background==crate::mascot::editor::Background::Checker)).on_press(Message::Ui(UiMsg::ArtBackground(crate::mascot::editor::Background::Checker)))].spacing(4),
        row![hint(format!("Zoom {:.0}%",e.zoom*100.0)),slider(0.5..=3.0,e.zoom,|z|Message::Ui(UiMsg::ArtZoom(z))).step(0.1_f32).style(theme::range)].spacing(8),
        canvas(ArtPreview { editor: e,busy })
            .width(Length::Fill)
            .height(340),
        hint(placing),
        hint("Drag visible artwork to arrange it. Adjust joint moves only the joint. Selecting a face previews that replacement."),
        hint(if e.clipped() { "Some source images extend outside the stage and will be clipped when saved." } else { "Artwork fits inside the stage." }),
    ]
    .spacing(10)
    .width(300)
    .into()
}

struct ArtPreview<'a> {
    editor: &'a crate::mascot::editor::Editor,
    busy: bool,
}

#[derive(Default)]
struct ArtPreviewState {
    drag: Option<iced::Point>,
}

impl ArtPreview<'_> {
    fn image_rect(&self, bounds: iced::Size) -> iced::Rectangle {
        let pad = 14.0;
        let (w, h) = (self.editor.canvas.0 as f32, self.editor.canvas.1 as f32);
        let fit = ((bounds.width - pad * 2.0) / w).min((bounds.height - pad * 2.0) / h)
            * self.editor.zoom;
        let size = iced::Size::new(w * fit, h * fit);
        iced::Rectangle::new(
            iced::Point::new(
                (bounds.width - size.width) / 2.0,
                (bounds.height - size.height) / 2.0,
            ),
            size,
        )
    }
}

impl canvas::Program<Message> for ArtPreview<'_> {
    type State = ArtPreviewState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &iced::Event,
        bounds: iced::Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        use crate::mascot::editor::Mode;
        use iced::mouse::{Button, Event};
        if self.busy {
            state.drag = None;
            return None;
        }
        let r = self.image_rect(bounds.size());
        match event {
            iced::Event::Mouse(Event::ButtonReleased(Button::Left))
                if state.drag.take().is_some() =>
            {
                Some(canvas::Action::capture())
            }
            iced::Event::Mouse(Event::CursorMoved { .. }) if state.drag.is_some() => {
                let p = cursor.position()?;
                let local = iced::Point::new(p.x - bounds.x, p.y - bounds.y);
                let old = state.drag.replace(local)?;
                let scale = self.editor.canvas.0 as f32 / r.width;
                Some(
                    canvas::Action::publish(Message::Ui(UiMsg::ArtDrag(
                        (local.x - old.x) * scale,
                        (local.y - old.y) * scale,
                    )))
                    .and_capture(),
                )
            }
            iced::Event::Mouse(Event::ButtonPressed(Button::Left)) => {
                let p = cursor.position_in(bounds)?;
                if !r.contains(p) {
                    return Some(canvas::Action::capture());
                }
                let (x, y) = ((p.x - r.x) / r.width, (p.y - r.y) / r.height);
                let msg = if self.editor.mode == Mode::Pivot || self.editor.adjusting_rest {
                    UiMsg::ArtClick(x, y)
                } else {
                    let target = self.editor.hit_target(x, y)?;
                    state.drag = Some(p);
                    UiMsg::ArtDragStart(target)
                };
                Some(canvas::Action::publish(Message::Ui(msg)).and_capture())
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        use crate::mascot::editor::Selected;
        use iced::widget::canvas::{Frame, Path, Stroke};
        let e = self.editor;
        let size = bounds.size();
        let mut frame = Frame::new(renderer, size);
        use crate::mascot::editor::Background;
        let base = match e.background {
            Background::Light => Color::from_rgb8(244, 244, 245),
            Background::Dark => Color::from_rgb8(27, 38, 33),
            Background::Checker => Color::from_rgb8(190, 190, 196),
        };
        frame.fill_rectangle(iced::Point::ORIGIN, size, base);
        if e.background == Background::Checker {
            for y in 0..(size.height / 16.0).ceil() as u32 {
                for x in 0..(size.width / 16.0).ceil() as u32 {
                    if (x + y) % 2 == 0 {
                        frame.fill_rectangle(
                            iced::Point::new(x as f32 * 16.0, y as f32 * 16.0),
                            iced::Size::new(16.0, 16.0),
                            Color::from_rgb8(225, 225, 230),
                        );
                    }
                }
            }
        }
        let r = self.image_rect(size);
        let clip = r
            .intersection(&iced::Rectangle::new(iced::Point::ORIGIN, size))
            .unwrap_or_default();
        frame.with_clip(clip, |frame| {
            let fit = r.width / e.canvas.0 as f32;
            for (_, layer) in e.draw_layers() {
                let d = layer.image.dimensions();
                let p = layer.placement;
                frame.with_save(|f| {
                    f.translate(iced::Vector::new(
                        r.x + (p.x + d.0 as f32 * p.scale / 2.0) * fit,
                        r.y + (p.y + d.1 as f32 * p.scale / 2.0) * fit,
                    ));
                    f.rotate(iced::Radians(p.angle.to_radians()));
                    f.scale(fit * p.scale);
                    f.draw_image(
                        iced::Rectangle::new(
                            iced::Point::new(-(d.0 as f32) / 2.0, -(d.1 as f32) / 2.0),
                            iced::Size::new(d.0 as f32, d.1 as f32),
                        ),
                        canvas::Image::new(layer.handle.clone()),
                    );
                });
            }
        });
        frame.stroke(
            &Path::rectangle(r.position(), r.size()),
            Stroke::default().with_color(theme::MUTED).with_width(1.0),
        );
        if let Some(layer) = e.selected_layer() {
            let d = layer.image.dimensions();
            let corners = [
                (0.0, 0.0),
                (d.0 as f32, 0.0),
                (d.0 as f32, d.1 as f32),
                (0.0, d.1 as f32),
            ];
            let outline = Path::new(|path| {
                for (i, p) in corners.into_iter().enumerate() {
                    let p = layer.placement.map(p, d);
                    let p = iced::Point::new(
                        r.x + p.0 * r.width / e.canvas.0 as f32,
                        r.y + p.1 * r.height / e.canvas.1 as f32,
                    );
                    if i == 0 {
                        path.move_to(p);
                    } else {
                        path.line_to(p);
                    }
                }
                path.close();
            });
            frame.stroke(
                &outline,
                Stroke::default()
                    .with_color(theme::CYAN_TEXT)
                    .with_width(1.5),
            );
        }
        let at = |(x, y): (f32, f32)| iced::Point::new(r.x + x * r.width, r.y + y * r.height);
        let ring = Stroke::default().with_color(Color::WHITE).with_width(1.5);
        // The resting point's guide shows only while it is being corrected.
        if e.adjusting_rest {
            let ground = at(e.ground);
            let amber = Color::from_rgb8(0xF5, 0x9E, 0x0B);
            frame.stroke(
                &Path::line(
                    iced::Point::new(r.x, ground.y),
                    iced::Point::new(r.x + r.width, ground.y),
                ),
                Stroke::default()
                    .with_color(Color { a: 0.7, ..amber })
                    .with_width(1.0),
            );
            frame.fill(&Path::circle(ground, 7.0), amber);
            frame.stroke(&Path::circle(ground, 7.0), ring);
        }
        // The joint and its swing show only while Adjust joint is open.
        if let Some(i) = e.joint_part()
            && let Some(p) = e.parts.get(i)
        {
            let pivot = at(p.pivot);
            let d = p.layer.image.dimensions();
            let corners = [
                (0.0, 0.0),
                (d.0 as f32, 0.0),
                (d.0 as f32, d.1 as f32),
                (0.0, d.1 as f32),
            ]
            .map(|c| {
                let c = p.layer.placement.map(c, d);
                iced::Point::new(
                    r.x + c.0 * r.width / e.canvas.0 as f32,
                    r.y + c.1 * r.height / e.canvas.1 as f32,
                )
            });
            for swing in [-0.3f32, 0.3] {
                let (sn, cs) = swing.sin_cos();
                let turned = corners.map(|c| {
                    let (x, y) = (c.x - pivot.x, c.y - pivot.y);
                    iced::Point::new(pivot.x + x * cs - y * sn, pivot.y + x * sn + y * cs)
                });
                let outline = Path::new(|path| {
                    path.move_to(turned[0]);
                    for c in &turned[1..] {
                        path.line_to(*c);
                    }
                    path.close();
                });
                frame.stroke(
                    &outline,
                    Stroke::default()
                        .with_color(Color::from_rgba8(0x22, 0xD3, 0xEE, 0.6))
                        .with_width(1.0),
                );
            }
        }
        for (i, p) in e
            .parts
            .iter()
            .enumerate()
            .filter(|(i, _)| e.joint_part() == Some(*i))
        {
            let on = e.selected == Selected::Part(i);
            let c = at(p.pivot);
            let radius = if on { 7.0 } else { 5.0 };
            let fill = if on {
                Color::from_rgb8(0x22, 0xD3, 0xEE)
            } else {
                Color::from_rgba8(0x22, 0xD3, 0xEE, 0.55)
            };
            frame.fill(&Path::circle(c, radius), fill);
            frame.stroke(&Path::circle(c, radius), ring);
            frame.fill_text(canvas::Text {
                content: format!("{}", i + 1),
                position: iced::Point::new(c.x + 9.0, c.y - 8.0),
                color: Color::WHITE,
                size: 12.0.into(),
                ..canvas::Text::default()
            });
        }
        vec![frame.into_geometry()]
    }
}

/// "Add character": name it, check it on light and dark, use it.
fn new_character<'a>(draft: &'a crate::mascot::pack::Draft, busy: bool) -> Element<'a, Message> {
    column![
        h2("New character"),
        canvas(DraftPreview { draft })
            .width(Length::Fill)
            .height(260),
        iced::widget::text_input("Name", &draft.name)
            .on_input(|n| Message::Ui(UiMsg::CharacterName(n)))
            .size(14)
            .padding([9, 10])
            .style(|_, status| iced::widget::text_input::Style {
                background: theme::CONTROL.into(),
                border: iced::Border {
                    radius: 8.0.into(),
                    width: 1.0,
                    color: if matches!(status, iced::widget::text_input::Status::Focused { .. }) {
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
        hint("A single image reacts and sways as one piece. For moving parts, import a character folder with separate head, tail or ear PNGs."),
        row![
            button(text("Cancel").size(13).width(Length::Fill).center())
                .width(Length::Fill)
                .padding([10, 12])
                .style(theme::secondary)
                .on_press(Message::Ui(UiMsg::CancelCharacter)),
            button(
                text(if busy { "Installing…" } else { "Use character" })
                    .font(theme::HEADING)
                    .size(13)
                    .width(Length::Fill)
                    .center()
            )
            .width(Length::Fill)
            .padding([10, 12])
            .style(theme::primary)
            .on_press_maybe(
                (!busy && !draft.name.trim().is_empty()).then_some(Message::Ui(UiMsg::SaveCharacter))
            ),
        ]
        .spacing(8),
    ]
    .spacing(10)
    .width(260)
    .into()
}

/// The draft's art on light and dark halves.
struct DraftPreview<'a> {
    draft: &'a crate::mascot::pack::Draft,
}

impl DraftPreview<'_> {
    fn image_rect(&self, bounds: iced::Size) -> iced::Rectangle {
        let pad = 16.0;
        let (w, h) = (self.draft.width as f32, self.draft.height as f32);
        let fit = ((bounds.width - pad * 2.0) / w).min((bounds.height - pad * 2.0) / h);
        let size = iced::Size::new(w * fit, h * fit);
        iced::Rectangle::new(
            iced::Point::new(
                (bounds.width - size.width) / 2.0,
                (bounds.height - size.height) / 2.0,
            ),
            size,
        )
    }
}

impl canvas::Program<Message> for DraftPreview<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        use iced::widget::canvas::{Frame, Path};
        let size = bounds.size();
        let mut frame = Frame::new(renderer, size);
        frame.fill(
            &Path::rounded_rectangle(iced::Point::ORIGIN, size, 12.0.into()),
            Color::from_rgb8(0xF4, 0xF4, 0xF5),
        );
        frame.fill_rectangle(
            iced::Point::new(size.width / 2.0, 0.0),
            iced::Size::new(size.width / 2.0, size.height),
            Color::from_rgb8(0x1B, 0x26, 0x21),
        );
        let r = self.image_rect(size);
        frame.draw_image(r, canvas::Image::new(self.draft.image.clone()));
        vec![frame.into_geometry()]
    }
}

#[cfg(test)]
mod character_arrangement_tests {
    use super::*;
    use canvas::Program;

    #[test]
    fn drag_stays_captured_across_transparent_space_and_zoom_preserves_canvas_units() {
        use crate::mascot::editor::{Editor, Selected};
        use iced::{Point, Rectangle, Size, mouse};
        let dir =
            std::env::temp_dir().join(format!("tinypaw-arrange-input-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("body.png");
        image::RgbaImage::from_pixel(8, 8, image::Rgba([100, 140, 220, 255]))
            .save(&file)
            .unwrap();
        let mut editor = Editor::from_body(&file, &[]).unwrap();
        editor.select(Selected::Body);
        let bounds = Rectangle::new(Point::new(30.0, 40.0), Size::new(300.0, 340.0));
        for zoom in [1.0, 1.5, 2.0] {
            editor.zoom = zoom;
            let preview = ArtPreview {
                editor: &editor,
                busy: false,
            };
            let r = preview.image_rect(bounds.size());
            let start = Point::new(bounds.x + r.center_x(), bounds.y + r.center_y());
            let mut state = ArtPreviewState::default();
            let press = preview
                .update(
                    &mut state,
                    &iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                    bounds,
                    mouse::Cursor::Available(start),
                )
                .unwrap()
                .into_inner();
            assert!(matches!(
                press.0,
                Some(Message::Ui(UiMsg::ArtDragStart(Selected::Body)))
            ));
            let moved = Point::new(start.x + 10.0 * r.width / editor.canvas.0 as f32, start.y);
            let action = preview
                .update(
                    &mut state,
                    &iced::Event::Mouse(mouse::Event::CursorMoved { position: moved }),
                    bounds,
                    mouse::Cursor::Available(moved),
                )
                .unwrap()
                .into_inner();
            let Some(Message::Ui(UiMsg::ArtDrag(dx, dy))) = action.0 else {
                panic!("expected drag")
            };
            assert!((dx - 10.0).abs() < 0.001 && dy.abs() < 0.001);
            assert_eq!(action.2, iced::event::Status::Captured);
            let outside = Point::new(bounds.x + bounds.width + 50.0, bounds.y);
            let release = preview
                .update(
                    &mut state,
                    &iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                    bounds,
                    mouse::Cursor::Available(outside),
                )
                .unwrap()
                .into_inner();
            assert_eq!(release.2, iced::event::Status::Captured);
            assert!(state.drag.is_none());
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
