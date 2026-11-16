//! Shared parts and pose assignments in the artwork workspace.
use super::*;
use crate::mascot::composition::Composition;
use crate::mascot::editor::{Editor, LibraryAction, Target};
use iced::widget::{checkbox, column, row};

pub(super) fn library_page(e: &Editor) -> Element<'_, Message> {
    let library = e.library.as_ref().unwrap();
    let composition = &library.composition;
    let action = |a| Message::Ui(UiMsg::ArtLibrary(a));
    let movable = composition.movable_hands(&library.pose);
    let wave = composition.wave_enabled(&library.pose);
    let mut assignments = column![].spacing(14);
    for (slot, kind) in [
        ("body", "body"),
        ("head", "head"),
        ("eyes", "eyes"),
        ("tail", "tail"),
        ("left_arm", "left_arm"),
        ("right_arm", "right_arm"),
        ("holding_arm", "holding_arm"),
        ("phone", "phone"),
        ("glow_stick", "glow_stick"),
        ("grip", "grip"),
        ("phone_glow", "phone_glow"),
        ("stick_glow", "stick_glow"),
    ] {
        if !movable
            && [
                "left_arm",
                "right_arm",
                "holding_arm",
                "phone",
                "glow_stick",
                "grip",
                "phone_glow",
                "stick_glow",
            ]
            .contains(&slot)
        {
            continue;
        }
        let value = &composition.poses[&library.pose]["slots"][slot];
        let selected = Composition::variant_id(value).unwrap_or("None").to_owned();
        let mut choices = vec!["None".to_owned()];
        choices.extend(
            composition
                .variants
                .iter()
                .filter(|(_, v)| v["kind"] == kind || (kind == "body" && v["kind"] == "complete"))
                .map(|(id, _)| id.clone()),
        );
        let slot_name = slot.to_owned();
        let mut block = column![
            row![
                text(slot.replace('_', " ")).size(13).width(110),
                pick_list(choices, Some(selected.clone()), move |id| action(
                    LibraryAction::Assign(
                        slot_name.clone(),
                        if id == "None" { String::new() } else { id }
                    )
                ))
                .text_size(13)
                .padding([6, 10])
                .style(theme::select),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
        ]
        .spacing(5);
        if selected != "None" {
            if slot != "body" && slot != "eyes" {
                let parents: Vec<String> = composition.poses[&library.pose]["slots"]
                    .as_object()
                    .unwrap()
                    .keys()
                    .filter(|parent| parent.as_str() != slot && parent.as_str() != "eyes")
                    .cloned()
                    .collect();
                let parent = value["parent"]
                    .as_str()
                    .unwrap_or(if slot == "eyes" { "head" } else { "body" })
                    .to_owned();
                let slot_name = slot.to_owned();
                block = block.push(
                    row![
                        text("Attached to").size(12).width(110),
                        pick_list(parents, Some(parent), move |parent| action(
                            LibraryAction::Parent(slot_name.clone(), parent)
                        ))
                        .text_size(12)
                        .style(theme::select)
                    ]
                    .spacing(8),
                );
            }
            let placement = value["placement"]
                .as_array()
                .cloned()
                .unwrap_or_else(|| vec![0.0.into(), 0.0.into(), 1.0.into(), 0.0.into()]);
            for (n, label, min, max) in [
                (0, "Left / right", -0.5, 0.5),
                (1, "Up / down", -0.5, 0.5),
                (2, "Size", 0.25, 2.0),
                (3, "Angle", -180.0, 180.0),
            ] {
                let slot = slot.to_owned();
                let value = placement[n]
                    .as_f64()
                    .unwrap_or(if n == 2 { 1.0 } else { 0.0 }) as f32;
                block = block.push(
                    row![
                        text(label).size(12).width(110).color(theme::MUTED),
                        slider(min..=max, value, move |value| action(
                            LibraryAction::Placement(slot.clone(), n, value)
                        ))
                        .step(if n == 3 { 1.0_f32 } else { 0.005_f32 })
                        .style(theme::range),
                        text(if n == 3 {
                            format!("{value:.0}°")
                        } else {
                            format!("{:.1}%", value * 100.0)
                        })
                        .size(12)
                        .width(62),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                );
            }
        }
        if selected != "None" && slot != "body" && slot != "eyes" {
            let pivot = value
                .get("pivot")
                .unwrap_or(&composition.variants[&selected]["pivot"]);
            if let Some(pivot) = pivot.as_array().filter(|p| p.len() == 2) {
                for (axis, label) in [(0, "Joint horizontal"), (1, "Joint vertical")] {
                    let slot = slot.to_owned();
                    block = block.push(
                        row![
                            text(label).size(12).width(110),
                            slider(
                                0.0..=1.0,
                                pivot[axis].as_f64().unwrap_or(0.5) as f32,
                                move |v| action(LibraryAction::Joint(slot.clone(), axis, v))
                            )
                            .step(0.001_f32)
                            .style(theme::range)
                        ]
                        .spacing(8),
                    );
                }
            }
        }
        assignments = assignments.push(block);
    }
    let variant = &composition.variants[&library.variant];
    let variant_index = composition
        .variants
        .keys()
        .position(|id| id == &library.variant)
        .unwrap();
    let mut images = column![].spacing(8);
    for (n, (pointer, file)) in Composition::image_fields(variant).into_iter().enumerate() {
        images = images
            .push(
                row![
                    text(pointer.trim_start_matches('/').replace('/', " · "))
                        .size(12)
                        .width(Length::Fill),
                    button("Replace PNG…")
                        .style(theme::secondary)
                        .on_press(Message::Ui(UiMsg::ArtPick(Target::LibraryImage(
                            variant_index,
                            n
                        )))),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .push(text(file).size(11).color(theme::MUTED));
    }
    if variant["kind"] == "eyes" && variant["eyes"][0]["image"].is_null() {
        for eye in 0..2 {
            for axis in 0..2 {
                let value = variant["eyes"][eye]["max_offset"][axis]
                    .as_f64()
                    .unwrap_or(0.01) as f32;
                images = images.push(
                    row![
                        text(format!(
                            "{} eye {} travel",
                            if eye == 0 { "Left" } else { "Right" },
                            if axis == 0 { "horizontal" } else { "vertical" }
                        ))
                        .size(12)
                        .width(160),
                        slider(0.0..=0.05, value, move |v| action(
                            LibraryAction::EyeTravel(eye, axis, v)
                        ))
                        .step(0.001_f32)
                        .style(theme::range)
                    ]
                    .spacing(8),
                );
            }
        }
    }
    let new_poses: Vec<String> = ["sit", "stand", "doze", "curled", "held"]
        .into_iter()
        .filter(|p| !composition.poses.contains_key(*p))
        .map(str::to_owned)
        .collect();
    let mut kinds = vec!["body", "complete", "eyes"];
    kinds.extend(
        crate::mascot::motion::PartType::ALL
            .into_iter()
            .map(|kind| kind.name()),
    );
    column![
        row![h1("Parts and poses"),space().width(Length::Fill),
            button("Undo").style(theme::secondary).on_press_maybe(e.can_undo().then_some(Message::Ui(UiMsg::ArtUndo))),
            button("Redo").style(theme::secondary).on_press_maybe(e.can_redo().then_some(Message::Ui(UiMsg::ArtRedo))),
            button("Cancel").style(theme::secondary).on_press(Message::Ui(UiMsg::ArtCancel)),
            button("Save").style(theme::primary).on_press_maybe(e.validation_error().is_none().then_some(Message::Ui(UiMsg::ArtSave))),
        ].spacing(8).align_y(Alignment::Center),
        iced::widget::text_input("Character name",&e.name).on_input(|name|Message::Ui(UiMsg::ArtName(name))),
        section(vec![h2("Pose"),row![
            pick_list(composition.poses.keys().cloned().collect::<Vec<_>>(),Some(library.pose.clone()),move |p|action(LibraryAction::Pose(p)))
                .text_size(13).padding([6,10]).style(theme::select),
            pick_list(new_poses,None::<String>,move |p|action(LibraryAction::NewPose(p))).placeholder("Copy into new pose…")
                .text_size(13).padding([6,10]).style(theme::select),
        ].spacing(8).into(),
        checkbox(movable).label("Separate movable hands for this pose").on_toggle(move |v|action(LibraryAction::Hands(v))).into(),
        if movable {checkbox(wave).label("This artwork is designed to wave").on_toggle(move |v|action(LibraryAction::Wave(v))).into()}
            else {hint("Hands are painted into the body. This pose can nod and breathe; waving is unavailable.")},
        hint("Choose body artwork matching the hand option. Movable arms do not automatically enable waving. Each pose keeps its own parts, joints and movement permissions."),
        hint("Pose assignments reuse library variants. Position and size belong to this pose. Eyes follow the head; the item and grip follow the holding arm. Arm artwork and joints belong to this pose."),
        assignments.into()]),
        section(vec![h2("Shared parts"),
            pick_list(composition.variants.keys().cloned().collect::<Vec<_>>(),Some(library.variant.clone()),move |v|action(LibraryAction::Variant(v)))
                .text_size(13).padding([6,10]).style(theme::select).into(),
            hint(format!("Used by {}",composition.uses(&library.variant).join(", "))),
            row![button("Make variant").style(theme::secondary).on_press(action(LibraryAction::MakeVariant)),
                pick_list(kinds,None::<&str>,move |kind|action(LibraryAction::NewVariant(kind.to_owned())))
                    .placeholder("Add part variant…").text_size(13).padding([6,10]).style(theme::select),
            ].spacing(8).into(),
            hint("Reuse artwork only when its anatomy fits. Sitting and standing arms should use their own variants. Make variant assigns an independent copy to the current pose. Joints are edited above for this pose only."),
            images.into()]),
        hint(e.validation_error().unwrap_or_else(||"All pose assignments are valid.".into())),
    ].spacing(20).into()
}
