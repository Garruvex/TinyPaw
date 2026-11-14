//! Art tab: layers, eyes, placement and the artwork preview.

use super::*;
use iced::widget::{column, row};

/// The Art tab: a character's body, moving parts, head faces and eyes.
pub(super) fn art_page<'a>(
    e: &'a crate::mascot::editor::Editor,
    busy: bool,
) -> Element<'a, Message> {
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
pub(super) fn eyes_section<'a>(
    e: &'a crate::mascot::editor::Editor,
    has_head: bool,
) -> Element<'a, Message> {
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
                if let Some(white) = &eye.white {
                    block = block.push(
                        row![
                            text(format!("Whole eye size {:.0}%", white.placement.scale * 100.0)).size(12),
                            slider(0.01..=8.0, white.placement.scale, move |scale| Message::Ui(UiMsg::ArtEyeSize(i, scale)))
                                .step(0.01_f32)
                                .style(theme::range),
                        ]
                        .spacing(10)
                        .align_y(Alignment::Center),
                    );
                }
                for (label, slot, layer) in [
                    ("Iris / pupil", EyeSlot::Pupil, &eye.pupil),
                    ("Background", EyeSlot::White, &eye.white),
                    ("Pupil area", EyeSlot::Mask, &eye.mask),
                    ("Blink", EyeSlot::Blink, &eye.blink),
                ] {
                    let Some(layer) = layer else { continue };
                    let scale = layer.placement.scale;
                    block = block.push(
                        row![
                            text(format!("{label} size {:.0}%", scale * 100.0)).size(12),
                            slider(0.01..=8.0, scale, move |scale| Message::Ui(UiMsg::ArtEyeLayerSize(i, slot, scale)))
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
pub(super) fn advanced_page<'a>(e: &'a crate::mascot::editor::Editor) -> Element<'a, Message> {
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
pub(super) fn resting_point<'a>(e: &'a crate::mascot::editor::Editor) -> Element<'a, Message> {
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

pub(super) fn canvas_controls<'a>(e: &'a crate::mascot::editor::Editor) -> Element<'a, Message> {
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
pub(super) fn joint_controls<'a>(e: &'a crate::mascot::editor::Editor) -> Element<'a, Message> {
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

pub(super) fn placement_controls<'a>(e: &'a crate::mascot::editor::Editor) -> Element<'a, Message> {
    if e.mode != crate::mascot::editor::Mode::Arrange || e.selected_layer().is_none() {
        return hint("Choose a layer and Arrange artwork to move, scale or rotate it.");
    }
    let p = e.selected_layer().unwrap().placement;
    let limits = [2.0 * e.canvas.0 as f32, 2.0 * e.canvas.1 as f32];
    let sliders = [
        (-limits[0]..=limits[0], p.x),
        (-limits[1]..=limits[1], p.y),
        (1.0..=800.0, p.scale * 100.0),
        (-180.0..=180.0, p.angle),
    ];
    let fields = row(["X (px)", "Y (px)", "Scale (%)", "Angle (°)"]
        .into_iter()
        .enumerate()
        .map(|(i, label)| {
            column![
                hint(label),
                iced::widget::slider(sliders[i].0.clone(), sliders[i].1, move |value| {
                    Message::Ui(UiMsg::ArtField(i, format!("{value:.0}")))
                })
                .step(1.0_f32)
                .style(theme::range),
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
pub(super) fn art_preview<'a>(
    e: &'a crate::mascot::editor::Editor,
    busy: bool,
) -> Element<'a, Message> {
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

pub(super) struct ArtPreview<'a> {
    pub(super) editor: &'a crate::mascot::editor::Editor,
    pub(super) busy: bool,
}

#[derive(Default)]
pub(super) struct ArtPreviewState {
    pub(super) drag: Option<iced::Point>,
}

impl ArtPreview<'_> {
    pub(super) fn image_rect(&self, bounds: iced::Size) -> iced::Rectangle {
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
pub(super) fn new_character<'a>(
    draft: &'a crate::mascot::pack::Draft,
    busy: bool,
) -> Element<'a, Message> {
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
pub(super) struct DraftPreview<'a> {
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
