//! The Create from reference panel.

use super::*;
use iced::widget::{column, row};

/// Create from reference: choices, the requested files, Copy art prompt
/// and Import results.
pub(super) fn reference_panel<'a>(
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
            checkbox(r.dance).label("Create music gestures (separate arms, holding hand, phone and glow stick)").text_size(13).on_toggle(|on| Message::Ui(UiMsg::ReferenceDance(on))).into(),
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
            checkbox(r.extras.contains(&Role::HeadPetted)).label("Create a happy headpat head expression (includes a separate head)").text_size(13).on_toggle(|on| Message::Ui(UiMsg::ReferenceExtra(Role::HeadPetted,on))).into(),
            checkbox(r.extras.contains(&Role::Petted)).label("Create a happy headpat pose").text_size(13).on_toggle(|on| Message::Ui(UiMsg::ReferenceExtra(Role::Petted,on))).into(),
            hint("For seated dozing, reuse the blink artwork: no sleep or wake PNG is required. Add one sleep PNG only for a different position. TinyPaw adds breathing and Zzz; extra sleep positions can be added in Behavior."),
            checkbox(r.extras.contains(&Role::Sleep)).label("Optional: create a curled-up sleep pose").text_size(13).on_toggle(|on| Message::Ui(UiMsg::ReferenceExtra(Role::Sleep,on))).into(),
            checkbox(r.extras.contains(&Role::Wake)).label("Optional: create a drowsy wake pose (not required)").text_size(13).on_toggle(|on| Message::Ui(UiMsg::ReferenceExtra(Role::Wake,on))).into(),
            checkbox(r.extras.contains(&Role::Click)).label("Create a whole-character click pose").text_size(13).on_toggle(|on| Message::Ui(UiMsg::ReferenceExtra(Role::Click,on))).into(),
            checkbox(r.extras.contains(&Role::Drop)).label("Create a whole-character drop pose").text_size(13).on_toggle(|on| Message::Ui(UiMsg::ReferenceExtra(Role::Drop,on))).into(),
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
pub(super) struct RolePick(Option<crate::mascot::reference::Role>);

impl std::fmt::Display for RolePick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(r) => write!(f, "{r}"),
            None => f.write_str("Not used"),
        }
    }
}
