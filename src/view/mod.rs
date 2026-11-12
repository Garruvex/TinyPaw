//! Overlay layers back to front: committed ink, draft, tip or wheel, then the
//! status chip and quick bar. Settings and first-run windows live in their own
//! modules and share the Graphite tokens.

pub mod first_run;
pub mod icons;
mod ink;
pub(crate) mod paw;
pub mod settings;
pub mod theme;
pub mod wheel;

#[cfg(all(test, not(feature = "gpu")))]
mod interaction_tests;

#[cfg(all(test, not(feature = "gpu")))]
mod pointer_performance_tests;

#[cfg(all(test, not(feature = "gpu")))]
mod annotation_performance_tests;

use std::cell::Cell;

use iced::advanced::image;
use iced::alignment;
use iced::mouse;
use iced::widget::canvas::{self, Cache, Frame, Geometry, LineCap, LineJoin, Path, Stroke};
use iced::widget::{
    button, canvas as canvas_widget, column, container, mouse_area, pin as place, row, space,
    stack, text, text_input, tooltip,
};
use iced::{Alignment, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme};

use crate::app::Message;
use crate::features::{
    display, freeze, pin, point, sound, spotlight, system, text as text_tool, upcoming,
};
use crate::registry::{Registry, Value};
use crate::session::{Mark, Millis, OutputId, PointerKind, Session};

/// Widget ID of the label editor, so it can take keyboard focus.
pub const TEXT_EDITOR: &str = "tinypaw-text-editor";
use icons::{Glyph, icon};
use theme::Tone;

/// Overlay-only UI state owned by the app (not session state).
#[derive(Debug, Default)]
pub struct OverlayUi {
    pub pointer_menu: bool,
    pub ink_menu: bool,
    pub more_menu: bool,
    pub wheel: Option<wheel::WheelState>,
}

impl OverlayUi {
    pub fn close_menus(&mut self) -> bool {
        let was = self.pointer_menu || self.ink_menu || self.more_menu;
        self.pointer_menu = false;
        self.ink_menu = false;
        self.more_menu = false;
        was
    }
}

#[derive(Debug, Clone)]
pub enum UiMsg {
    TogglePointerMenu,
    ToggleInkMenu,
    ToggleMoreMenu,
    Nav(settings::Page),
    CharacterTab(settings::CharacterTab),
    /// The answer to Save / Discard / Stay.
    Leave(settings::LeaveChoice),
    /// Save the character draft: artwork (if open) then behavior.
    CharacterSave,
    /// Drop the character draft's unsaved changes.
    CharacterDiscard,
    ImportWav,
    PreviewSound,
    TurnOn,
    OpenSettings,
    PickPack(&'static str),
    Rebind(&'static str),
    ResetShortcuts,
    RefreshSources,
    AddPointer,
    ImportPack,
    EditPack,
    RemovePack,
    AddForearm,
    AddPressed,
    AddPackTap,
    RemoveForearm,
    DraftWristX(f32),
    DraftWristY(f32),
    DraftJointWidth(f32),
    DraftForearmAnchor(f32, f32),
    CopyPointerPrompt,
    CopyForearmPrompt,
    DraftHotspot(f32, f32),
    DraftName(String),
    ForearmColor([u8; 4]),
    SaveDraft,
    CancelDraft,
    ResetMascotPosition,
    PickCharacter(&'static str),
    AddCharacter,
    ImportCharacter,
    ExportCharacter,
    RemoveCharacter,
    CharacterName(String),
    SaveCharacter,
    CancelCharacter,
    /// A folder import's ID is taken: `true` keeps both, `false` replaces.
    ResolveCharacter(bool),
    CancelCharacterImport,
    /// Open the selected character in the Art tab.
    EditArt,
    ArtFromParts,
    ArtRest(crate::mascot::editor::RestAction),
    ArtRename(usize, String),
    /// Adjust joint for a part, or Done.
    ArtJoint(Option<usize>),
    /// A reaction row's sound choice.
    ReactionSound(
        crate::mascot::reactions::Event,
        Option<crate::mascot::reactions::Sound>,
    ),
    ReactionImport(crate::mascot::reactions::Event),
    ReactionTest(crate::mascot::reactions::Event),
    ReactionCooldown(crate::mascot::reactions::Event, f32),
    ReactionSoundsOn(bool),
    ReactionVolume(f32),
    ReactionIdleOn(bool),
    ReactionIdleInterval(f32, f32),
    ReactionsReset,
    ReactionVisual(
        crate::mascot::reactions::Event,
        crate::mascot::reactions::Visual,
    ),
    /// Choose the PNG for an artwork reaction, or Pick up's held pose.
    ReactionPicture(crate::mascot::reactions::Event),
    ReactionHeldClear,
    /// Create from reference.
    ReferenceStart,
    ReferenceStyle(String),
    ReferenceCanvas(u32),
    ReferenceTail(bool),
    ReferenceHead(bool),
    ReferenceEyes(bool),
    ReferenceTracking(bool),
    ReferenceBrows(bool),
    ReferenceBlink(bool),
    ReferenceFaces(bool),
    ReferenceHeld(bool),
    ReferenceExtra(crate::mascot::reference::Role, bool),
    /// Copy the whole brief (`None`) or one file's prompt.
    ReferenceCopy(Option<crate::mascot::reference::Role>),
    ReferenceImport,
    ReferenceAssign(usize, Option<crate::mascot::reference::Role>),
    ReferenceOpen,
    ReferenceCancel,
    /// Drop → After release: fall to the screen edge (true) or stay.
    ReactionFall(bool),
    Companionship(crate::mascot::companionship::Settings),
    CompanionTest(crate::mascot::companionship::State),
    CompanionMouth,
    /// Align a picture: event, whether it's the held pose, x/y/scale, value.
    ReactionPlace(crate::mascot::reactions::Event, bool, usize, f32),
    ArtEyeStyle(crate::mascot::editor::EyeStyle),
    ArtField(usize, String),
    ArtCanvas(usize, String),
    ArtDragStart(crate::mascot::editor::Selected),
    ArtDrag(f32, f32),
    ArtNudge(f32, f32),
    ArtOrder(usize, bool),
    ArtUndo,
    ArtRedo,
    ArtZoom(f32),
    ArtBackground(crate::mascot::editor::Background),
    ArtSelect(crate::mascot::editor::Selected),
    /// Choose a PNG for this slot.
    ArtPick(crate::mascot::editor::Target),
    ArtKind(usize, crate::mascot::motion::PartType),
    ArtOnHead(usize, bool),
    ArtBehind(usize, bool),
    ArtRemovePart(usize),
    ArtRemoveFace(usize, usize),
    ArtAddEye,
    ArtStaticEye(usize),
    ArtRemoveEye(usize),
    ArtRemoveBlink(usize),
    ArtTravel(usize, f32),
    ArtPupilSize(usize, f32),
    ArtClick(f32, f32),
    ArtName(String),
    ArtSave,
    ArtCancel,
    /// Motion tab: a part's preset, strength and speed.
    MotionPreset(usize, crate::mascot::motion::Preset),
    MotionStrength(usize, f32),
    MotionSpeed(usize, f32),
    MotionTest(crate::mascot::preview::Test),
    /// Pick how the selected character bops to music; saved at once.
    MusicBop(crate::mascot::motion::Bop),
    MotionReset,
    /// Lines: edit, add or remove a line, and its sound.
    LineText(crate::mascot::lines::Trigger, usize, String),
    LineAdd(crate::mascot::lines::Trigger),
    LineRemove(crate::mascot::lines::Trigger, usize),
    LineSound(crate::mascot::lines::Trigger, usize),
    LineClearSound(crate::mascot::lines::Trigger, usize),
    BubbleBackground([u8; 4]),
    BubbleText([u8; 4]),
    BubbleWidth(f32),
    BubbleSide(crate::mascot::lines::BubbleSide),
    /// Where the bubble's tail aims, as canvas fractions; `None` follows
    /// the top of the art.
    BubbleAnchor(Option<(f32, f32)>),
    /// Start or stop picking the bubble's anchor on the preview.
    BubblePlace,
    MenuSide(crate::mascot::lines::MenuSide),
    MenuAlign(crate::mascot::lines::MenuAlign),
    LinesReset,
    /// Import… (`false`) or Export… (`true`): asks what.
    TransferOpen(bool),
    /// Import (`false`) or export (`true`) this.
    TransferPick(bool, settings::TransferScope),
    TransferFit(crate::mascot::pack::Fit),
    TransferMerge(crate::mascot::lines::Merge),
    /// Also take the bubble's look and the menu's placement.
    TransferLayout(bool),
    TransferConfirm,
    TransferCancel,
}

/// Each live or frozen scene has its own revision counter.
#[derive(Default)]
pub struct InkCache {
    cache: Cache,
    revision: Cell<Option<(Option<u64>, u64)>>,
}

pub struct Overlay<'a> {
    pub registry: &'a Registry,
    pub session: &'a Session,
    pub ink: &'a InkCache,
    pub ui: &'a OverlayUi,
    pub output: OutputId,
    pub now: Millis,
    /// The frozen frame, when the session shows one.
    pub still: Option<&'a image::Handle>,
    /// A short message for the status chip, such as a failed capture.
    pub notice: Option<&'a str>,
    /// The selected pointer pack's art, if it loaded.
    pub pack: Option<&'a crate::packs::Pack>,
    /// The clear Spotlight circle, from the tool or Spotlight anywhere.
    pub spotlight: Option<(OutputId, crate::session::Pt, f32)>,
}

/// Why a command can't run right now, or `None` when it can.
fn unavailable(registry: &Registry, session: &Session, id: &str) -> Option<&'static str> {
    match registry.command(id) {
        Some(d) => registry.availability(d.id, session).err().map(|why| why.0),
        None => Some("Not available"),
    }
}

pub fn overlay(o: Overlay<'_>) -> Element<'_, Message> {
    let wheel_blocked: wheel::Availability =
        std::array::from_fn(|i| unavailable(o.registry, o.session, wheel::ITEMS[i].command));
    let scene = canvas_widget(OverlayCanvas {
        session: o.session,
        ink: o.ink,
        wheel: o.ui.wheel,
        wheel_blocked,
        lens_hint: wheel::lens_hint(o.session.settings().choice(upcoming::SOURCE)),
        output: o.output,
        now: o.now,
        pack: o.pack,
    })
    .width(Length::Fill)
    .height(Length::Fill);
    let mask = canvas_widget(SpotlightMask {
        at: o
            .spotlight
            .filter(|(output, ..)| *output == o.output && !o.session.capturing())
            .map(|(_, at, radius)| {
                let dim = o.session.settings().float(spotlight::DIM) as f32;
                (Point::new(at.x, at.y), radius, dim)
            }),
    })
    .width(Length::Fill)
    .height(Length::Fill);

    // The still gets its own layer: within one canvas layer Iced paints images
    // above shapes, which would hide the marks drawn on the still.
    let mut layers: Vec<Element<'_, Message>> = Vec::with_capacity(5);
    if let Some(handle) = o.still.filter(|_| o.session.frozen_frame().is_some())
        && !o.session.capturing()
    {
        layers.push(
            iced::widget::image(handle.clone())
                .width(Length::Fill)
                .height(Length::Fill)
                .content_fit(iced::ContentFit::Fill)
                .into(),
        );
    }
    // Always present so the scene canvas keeps its place, and its state, in
    // the stack when Spotlight turns on or off.
    layers.push(mask.into());
    layers.push(scene.into());
    if let Some(draft) = o.session.text_draft().filter(|d| d.output == o.output) {
        layers.push(text_editor(
            draft.at,
            draft.text,
            draft.size,
            o.session.settings(),
        ));
    }

    let hidden = o.session.settings().bool(display::HIDE_BAR);
    if !o.session.is_pointing() || o.ui.wheel.is_some() || o.session.capturing() || hidden {
        return stack(layers).into();
    }

    let chip = container(status_chip(o.session.frozen_frame().is_some(), o.notice))
        .width(Length::Fill)
        .align_x(Alignment::Center)
        .padding(20);

    let mut controls = column![].spacing(8).align_x(Alignment::Center);
    if o.ui.pointer_menu {
        controls = controls.push(pointer_menu(o.session, o.pack));
    }
    if o.ui.ink_menu {
        controls = controls.push(ink_menu(o.session, o.pack));
    }
    if o.ui.more_menu {
        controls = controls.push(more_menu(o.registry, o.session));
    }
    controls = controls.push(quick_bar(o.registry, o.session, o.ui.more_menu));
    let bar = container(controls)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::End)
        .padding(28);

    layers.push(chip.into());
    layers.push(bar.into());
    stack(layers).into()
}

/// The label editor, placed where the label will sit and styled like one.
fn text_editor<'a>(
    at: crate::session::Pt,
    value: &str,
    size: f32,
    settings: &crate::registry::SettingValues,
) -> Element<'a, Message> {
    let color = theme::rgba(settings.color(text_tool::COLOR));
    let backing = settings.bool(text_tool::BACKING);
    let input = text_input("Type a label, Enter to place", value)
        .id(TEXT_EDITOR)
        .size(size)
        .font(theme::MEDIUM)
        .padding([4, 8])
        .width(Length::Fixed(
            label_width(value, size).max(size * 12.0) + 24.0,
        ))
        .on_input(Message::TextChanged)
        .on_submit(Message::TextSubmit)
        .style(move |_, status| text_input::Style {
            background: (if backing {
                theme::LABEL_BG
            } else {
                Color::TRANSPARENT
            })
            .into(),
            border: iced::Border {
                radius: 8.0.into(),
                width: 1.5,
                color: if matches!(status, text_input::Status::Focused { .. }) {
                    theme::CYAN_RING
                } else {
                    theme::LINE_STRONG
                },
            },
            icon: theme::MUTED,
            placeholder: theme::FAINT,
            value: color,
            selection: theme::CYAN_BG,
        });
    // The editor owns its area so typing clicks never start a new label.
    let editor = mouse_area(input).interaction(mouse::Interaction::Text);
    place(editor).x(at.x - 8.0).y(at.y - 4.0).into()
}

fn status_chip<'a>(frozen: bool, notice: Option<&'a str>) -> Element<'a, Message> {
    let dot = container(space())
        .width(8)
        .height(8)
        .style(theme::fill(theme::AMBER, 4.0));
    let content: Element<'a, Message> = if let Some(notice) = notice {
        row![dot, text(notice).size(13).color(theme::AMBER_TEXT)]
            .spacing(10)
            .align_y(Alignment::Center)
            .into()
    } else if frozen {
        row![
            icon(Glyph::Freeze, 16.0, theme::CYAN_TEXT),
            text("Frozen")
                .font(theme::MEDIUM)
                .size(13)
                .color(theme::TEXT),
            text("·").size(13).color(theme::FAINT),
            button(text("Return live").font(theme::MEDIUM).size(13))
                .padding([2, 8])
                .style(theme::tool_button(Tone::Selected))
                .on_press(Message::Command(freeze::TOGGLE)),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into()
    } else {
        row![
            dot,
            text("Annotating")
                .font(theme::MEDIUM)
                .size(13)
                .color(theme::TEXT),
            text("·").size(13).color(theme::FAINT),
            text("Esc to exit").size(13).color(theme::SUBTLE),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into()
    };
    mouse_area(
        container(content)
            .padding([7, 14])
            .style(theme::panel(999.0)),
    )
    .interaction(mouse::Interaction::Idle)
    .into()
}

fn keycap<'a>(label: String) -> Element<'a, Message> {
    container(text(label).size(11).color(theme::SUBTLE))
        .padding([1, 6])
        .style(theme::keycap)
        .into()
}

/// Wraps a control in a tooltip showing its name and shortcut.
fn tip<'a>(
    content: impl Into<Element<'a, Message>>,
    label: String,
    shortcut: Option<String>,
) -> Element<'a, Message> {
    let mut body = row![text(label).size(12).color(theme::TEXT)]
        .spacing(8)
        .align_y(Alignment::Center);
    if let Some(s) = shortcut {
        body = body.push(keycap(s.replace('+', " ")));
    }
    tooltip(
        content,
        container(body)
            .padding([6, 10])
            .style(theme::fill(theme::HOVER, 8.0)),
        tooltip::Position::Top,
    )
    .gap(8)
    .into()
}

fn icon_button<'a>(glyph: Glyph, tone: Tone, msg: Option<Message>) -> button::Button<'a, Message> {
    let color = match tone {
        Tone::Selected => theme::CYAN_TEXT,
        Tone::Warm => theme::AMBER_TEXT,
        _ if msg.is_none() => theme::DISABLED,
        _ => theme::TEXT_SOFT,
    };
    button(
        container(icon(glyph, 20.0, color))
            .width(44)
            .height(44)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center),
    )
    .padding(0)
    .style(theme::tool_button(tone))
    .on_press_maybe(msg)
}

fn divider<'a>() -> Element<'a, Message> {
    container(space())
        .width(1)
        .height(24)
        .style(theme::fill(theme::LINE_STRONG, 0.0))
        .into()
}

fn command_button<'a>(
    registry: &Registry,
    session: &Session,
    id: &'static str,
    glyph: Glyph,
    tone: Tone,
) -> Element<'a, Message> {
    let def = registry.command(id);
    let reason = def.and_then(|d| registry.availability(d.id, session).err());
    let msg = reason.is_none().then_some(Message::Command(id));
    let mut label = def.map_or(id, |d| d.label).to_owned();
    if let Some(why) = reason {
        label = format!("{label} · {why}");
    }
    tip(
        icon_button(glyph, tone, msg),
        label,
        registry.shortcut_label(id),
    )
}

fn tool_tone(session: &Session, tool: &str) -> Tone {
    if session.active_tool() == tool {
        Tone::Selected
    } else {
        Tone::Plain
    }
}

/// Tools that live in the More panel, with the glyph the More button borrows
/// while one of them is active.
const MORE_TOOLS: [(&str, Glyph); 2] = [
    (spotlight::TOOL, Glyph::Spotlight),
    (upcoming::TOOL, Glyph::Lens),
];

fn quick_bar<'a>(
    registry: &'a Registry,
    session: &'a Session,
    more_open: bool,
) -> Element<'a, Message> {
    let laser = session.settings().choice(point::STYLE) == "laser";
    let pointing = matches!(session.active_tool(), point::TOOL);
    let pointer_tone = if pointing {
        Tone::Selected
    } else {
        Tone::Plain
    };
    let pointer_color = if pointing {
        theme::CYAN_TEXT
    } else {
        theme::TEXT_SOFT
    };
    let pointer = button(
        row![
            icon(
                if laser { Glyph::Laser } else { Glyph::Paw },
                20.0,
                pointer_color
            ),
            icon(Glyph::ChevronUp, 14.0, pointer_color),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .height(44)
    .padding([12, 10])
    .style(theme::tool_button(pointer_tone))
    .on_press(Message::Ui(UiMsg::TogglePointerMenu));
    let pointer = tip(pointer, "Point: paw or laser".into(), None);

    let color = theme::rgba(session.settings().color(system::INK_COLOR));
    let width = session.settings().float(system::INK_WIDTH);
    let ink = button(
        row![
            container(space())
                .width(18)
                .height(18)
                .style(theme::outlined(
                    color,
                    9.0,
                    Color::from_rgba8(255, 255, 255, 0.18)
                )),
            text(format!("{width:.0} px")).size(13).color(theme::SUBTLE),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .height(44)
    .padding([12, 10])
    .style(theme::tool_button(Tone::Plain))
    .on_press(Message::Ui(UiMsg::ToggleInkMenu));
    let ink = tip(ink, "Ink colour and width".into(), None);

    // The More button shows the tool it is holding, so an active Spotlight or
    // Lens is still visible on the bar.
    let held = MORE_TOOLS
        .iter()
        .find(|(tool, _)| session.active_tool() == *tool)
        .map(|&(tool, glyph)| {
            (
                registry
                    .command(tool_command(tool))
                    .map_or(tool, |d| d.label),
                glyph,
            )
        });
    let more_on = more_open || held.is_some();
    let more_color = if more_on {
        theme::CYAN_TEXT
    } else {
        theme::TEXT_SOFT
    };
    let more = button(
        row![
            icon(held.map_or(Glyph::More, |(_, g)| g), 20.0, more_color),
            icon(Glyph::ChevronUp, 14.0, more_color),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .height(44)
    .padding([12, 10])
    .style(theme::tool_button(if more_on {
        Tone::Selected
    } else {
        Tone::Plain
    }))
    .on_press(Message::Ui(UiMsg::ToggleMoreMenu));
    let more = tip(
        more,
        held.map_or_else(
            || "More: Spotlight, Lens, Freeze, Clear marks".to_owned(),
            |(label, _)| format!("More · {label} is active"),
        ),
        None,
    );

    let end_label = registry
        .command(system::EXIT)
        .map_or("End annotation", |d| d.label);
    let end = button(
        row![
            icon(Glyph::Close, 18.0, theme::TEXT_SOFT),
            text(end_label).font(theme::MEDIUM).size(14),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .height(44)
    .padding([12, 14])
    .style(theme::tool_button(Tone::Exit))
    .on_press(Message::Command(system::EXIT));
    let end = tip(
        end,
        format!("{end_label} · clears marks"),
        registry.shortcut_label(system::EXIT),
    );

    let bar = row![
        pointer,
        command_button(
            registry,
            session,
            text_tool::SELECT,
            Glyph::Text,
            tool_tone(session, text_tool::TOOL)
        ),
        command_button(
            registry,
            session,
            pin::SELECT,
            Glyph::Pin,
            tool_tone(session, pin::TOOL)
        ),
        divider(),
        ink,
        divider(),
        command_button(registry, session, system::UNDO, Glyph::Undo, Tone::Plain),
        more,
        divider(),
        end,
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    // The bar owns its whole area so clicks between buttons never draw.
    mouse_area(container(bar).padding(6).style(theme::panel(16.0)))
        .interaction(mouse::Interaction::Idle)
        .into()
}

/// The command that selects a More-panel tool.
fn tool_command(tool: &str) -> &'static str {
    if tool == spotlight::TOOL {
        spotlight::SELECT
    } else {
        upcoming::LENS
    }
}

/// Labelled tools that don't need a slot on the bar.
fn more_menu<'a>(registry: &'a Registry, session: &'a Session) -> Element<'a, Message> {
    let entry = |id: &'static str, glyph: Glyph, tone: Tone, label: Option<String>| {
        let def = registry.command(id);
        let reason = def.and_then(|d| registry.availability(d.id, session).err());
        let label = label.unwrap_or_else(|| def.map_or(id, |d| d.label).to_owned());
        let color = match tone {
            _ if reason.is_some() => theme::DISABLED,
            Tone::Selected => theme::CYAN_TEXT,
            Tone::Warm => theme::AMBER_TEXT,
            _ => theme::TEXT_SOFT,
        };
        let mut body = row![
            icon(glyph, 20.0, color),
            text(label).font(theme::MEDIUM).size(14),
            space().width(Length::Fill),
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        if let Some(why) = &reason {
            body = body.push(text(why.to_string()).size(12).color(theme::FAINT));
        } else if let Some(key) = registry.shortcut_label(id) {
            body = body.push(keycap(key.replace('+', " ")));
        }
        button(body)
            .width(Length::Fill)
            .height(44)
            .padding([12, 14])
            .style(theme::tool_button(tone))
            .on_press_maybe(reason.is_none().then_some(Message::Command(id)))
            .into()
    };
    let muted = session.settings().bool(sound::MUTE);
    let items: Vec<Element<'a, Message>> = vec![
        entry(
            spotlight::SELECT,
            Glyph::Spotlight,
            tool_tone(session, spotlight::TOOL),
            None,
        ),
        entry(
            upcoming::LENS,
            Glyph::Lens,
            tool_tone(session, upcoming::TOOL),
            None,
        ),
        entry(
            freeze::TOGGLE,
            Glyph::Freeze,
            if session.frozen_frame().is_some() {
                Tone::Selected
            } else {
                Tone::Plain
            },
            None,
        ),
        container(space())
            .width(Length::Fill)
            .height(1)
            .style(theme::fill(theme::LINE_STRONG, 0.0))
            .into(),
        entry(system::CLEAR, Glyph::Clear, Tone::Plain, None),
        entry(
            sound::TOGGLE_MUTE,
            if muted { Glyph::Mute } else { Glyph::Sound },
            if muted { Tone::Warm } else { Tone::Plain },
            Some(
                if muted {
                    "Unmute tap sound"
                } else {
                    "Mute tap sound"
                }
                .into(),
            ),
        ),
    ];
    mouse_area(
        container(
            iced::widget::Column::with_children(items)
                .spacing(2)
                .width(248),
        )
        .padding(6)
        .style(theme::panel(12.0)),
    )
    .interaction(mouse::Interaction::Idle)
    .into()
}

fn pointer_menu<'a>(session: &Session, pack: Option<&crate::packs::Pack>) -> Element<'a, Message> {
    let laser = session.settings().choice(point::STYLE) == "laser";
    let tint = |on: bool| {
        if on {
            theme::CYAN_TEXT
        } else {
            theme::TEXT_SOFT
        }
    };
    let choice = |art: Element<'a, Message>, label: String, on: bool, id: &'static str| {
        button(
            row![
                container(art)
                    .width(20)
                    .height(20)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center),
                text(label).font(theme::MEDIUM).size(14),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .height(44)
        .padding([12, 14])
        .style(theme::tool_button(if on {
            Tone::Selected
        } else {
            Tone::Plain
        }))
        .on_press(Message::Command(id))
    };
    mouse_area(
        container(
            row![
                // The installed paw's own name and thumbnail.
                match pack {
                    Some(p) => choice(
                        iced::widget::image(p.image.clone()).height(20).into(),
                        p.name.clone(),
                        !laser,
                        point::USE_PAW,
                    ),
                    None => choice(
                        icon(Glyph::Paw, 20.0, tint(!laser)).into(),
                        "Paw".into(),
                        !laser,
                        point::USE_PAW,
                    ),
                },
                choice(
                    icon(Glyph::Laser, 20.0, tint(laser)).into(),
                    "Laser".into(),
                    laser,
                    point::USE_LASER
                ),
            ]
            .spacing(4),
        )
        .padding(6)
        .style(theme::panel(12.0)),
    )
    .interaction(mouse::Interaction::Idle)
    .into()
}

fn ink_menu<'a>(session: &Session, pack: Option<&crate::packs::Pack>) -> Element<'a, Message> {
    let current = session.settings().color(system::INK_COLOR);
    let width = session.settings().float(system::INK_WIDTH);
    let mut swatches = row(theme::INKS.iter().map(|&(_, c)| {
        button(space().width(28).height(28))
            .padding(0)
            .style(theme::swatch(theme::rgba(c), c == current))
            .on_press(Message::Set(system::INK_COLOR, Value::Color(c)))
            .into()
    }))
    .spacing(8)
    .align_y(Alignment::Center);
    if let Some(accent) = pack.and_then(|p| p.accent) {
        swatches = swatches.push(match_pointer(accent, current));
    }
    let widths = row(theme::WIDTHS.iter().map(|&w| {
        button(text(format!("{w:.0} px")).size(13))
            .padding([7, 12])
            .style(theme::segment((w - width).abs() < 0.01))
            .on_press(Message::Set(system::INK_WIDTH, Value::Float(w)))
            .into()
    }))
    .spacing(4);
    mouse_area(
        container(
            column![
                swatches,
                container(widths)
                    .padding(4)
                    .style(theme::fill(theme::WELL, 10.0)),
            ]
            .spacing(10)
            .align_x(Alignment::Center),
        )
        .padding(10)
        .style(theme::panel(12.0)),
    )
    .interaction(mouse::Interaction::Idle)
    .into()
}

struct OverlayCanvas<'a> {
    session: &'a Session,
    ink: &'a InkCache,
    wheel: Option<wheel::WheelState>,
    wheel_blocked: wheel::Availability,
    lens_hint: &'static str,
    output: OutputId,
    now: Millis,
    pack: Option<&'a crate::packs::Pack>,
}

/// The Spotlight dimming, on its own layer under the marks: it draws its soft
/// edge as an image, and Iced paints images above shapes within a layer.
struct SpotlightMask {
    at: Option<(Point, f32, f32)>,
}

impl<Message> canvas::Program<Message> for SpotlightMask {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let Some((at, radius, dim)) = self.at else {
            return Vec::new();
        };
        ink::spotlight(renderer, bounds.size(), at, radius, dim)
    }
}

#[derive(Default)]
struct CanvasState {
    inside: bool,
    last: Option<Point>,
    shift: bool,
}

fn stroke_style(color: Color, width: f32) -> Stroke<'static> {
    Stroke::default()
        .with_color(color)
        .with_width(width)
        .with_line_cap(LineCap::Round)
        .with_line_join(LineJoin::Round)
}

/// Rough single-line width; wide (CJK) characters take a full em.
fn label_width(text: &str, size: f32) -> f32 {
    text.chars()
        .map(|c| {
            if (c as u32) >= 0x1100 {
                size
            } else {
                size * 0.56
            }
        })
        .sum()
}

fn draw_label(frame: &mut Frame, l: &crate::session::Label) {
    let pad = (8.0, 4.0);
    let size = Size::new(
        label_width(&l.text, l.size) + pad.0 * 2.0,
        l.size * 1.4 + pad.1 * 2.0,
    );
    let corner = Point::new(l.at.x - pad.0, l.at.y - pad.1);
    if l.backing {
        frame.fill(
            &Path::rounded_rectangle(corner, size, 8.0.into()),
            theme::LABEL_BG,
        );
    }
    frame.fill_text(canvas::Text {
        content: l.text.clone(),
        position: Point::new(l.at.x, l.at.y + l.size * 0.7),
        color: theme::rgba(l.color.0),
        size: l.size.into(),
        font: theme::MEDIUM,
        align_y: alignment::Vertical::Center,
        ..canvas::Text::default()
    });
}

fn draw_pin(frame: &mut Frame, p: &crate::session::Pin) {
    let c = Point::new(p.at.x, p.at.y);
    frame.fill(
        &Path::circle(Point::new(c.x, c.y + 2.0), 15.0),
        Color::from_rgba8(0, 0, 0, 0.35),
    );
    frame.fill(&Path::circle(c, 14.0), theme::rgba(p.color.0));
    frame.fill_text(canvas::Text {
        content: p.number.to_string(),
        position: c,
        color: Color::from_rgb8(0x0B, 0x12, 0x15),
        size: 15.0.into(),
        font: theme::HEADING,
        align_x: iced::widget::text::Alignment::Center,
        align_y: alignment::Vertical::Center,
        ..canvas::Text::default()
    });
}

/// "Match pointer": ink in the pack's accent. The stroke keeps the resolved
/// colour if the pointer changes later.
pub fn match_pointer<'a>(accent: [u8; 4], current: [u8; 4]) -> Element<'a, Message> {
    tip(
        button(
            row![
                container(space().width(16).height(16)).style(theme::outlined(
                    theme::rgba(accent),
                    8.0,
                    theme::LINE_STRONG
                )),
                text("Match").size(12),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .padding([5, 8])
        .style(theme::segment(accent == current))
        .on_press(Message::Set(system::INK_COLOR, Value::Color(accent))),
        "Ink matches the pointer".into(),
        None,
    )
}

fn draw_mark(frame: &mut Frame, mark: &Mark, alpha: f32) {
    let s = match mark {
        Mark::Stroke(s) => s,
        Mark::PawPrint(p) => return paw::draw_print(frame, p, alpha),
        Mark::Label(l) => return draw_label(frame, l),
        Mark::Pin(p) => return draw_pin(frame, p),
    };
    let mut color = theme::rgba(s.color.0);
    color.a *= alpha;
    match s.points.as_slice() {
        [] => {}
        [p] => frame.fill(&Path::circle(Point::new(p.x, p.y), s.width / 2.0), color),
        points => {
            // Opaque ink is drawn in short runs (their round caps overlap
            // invisibly). A repaint then rasterizes only the runs it touches,
            // and the runs of a stroke still being drawn stay identical
            // except at its growing end, so only that end repaints.
            for path in ink::stroke_paths(points, color.a >= 1.0) {
                frame.stroke(&path, stroke_style(color, s.width));
            }
        }
    }
}

impl canvas::Program<Message> for OverlayCanvas<'_> {
    type State = CanvasState;

    fn update(
        &self,
        state: &mut CanvasState,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let event = match event {
            iced::Event::Mouse(event) => event,
            iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(m)) => {
                state.shift = m.shift();
                return None;
            }
            _ => return None,
        };
        let pos = cursor.position_in(bounds);
        let msg = match (event, pos) {
            (mouse::Event::CursorMoved { .. }, Some(p)) => {
                if state.inside && state.last == Some(p) {
                    return None; // Coalesce repeated positions.
                }
                state.inside = true;
                state.last = Some(p);
                Message::Pointer(self.output, PointerKind::Moved, p)
            }
            (mouse::Event::CursorMoved { .. } | mouse::Event::CursorLeft, None) if state.inside => {
                state.inside = false;
                Message::Pointer(
                    self.output,
                    PointerKind::Left,
                    state.last.unwrap_or(Point::ORIGIN),
                )
            }
            (mouse::Event::ButtonPressed(mouse::Button::Left), Some(p)) => {
                state.last = Some(p);
                Message::Pointer(self.output, PointerKind::Down, p)
            }
            // Over a Spotlight or Lens: size, or zoom/darkness with Shift.
            (mouse::Event::WheelScrolled { delta }, Some(_)) => {
                let y = match delta {
                    mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => {
                        *y
                    }
                };
                if y == 0.0 {
                    return None;
                }
                Message::ToolWheel(y > 0.0, state.shift)
            }
            // Always report the release so a swallowed button-up is observed.
            (mouse::Event::ButtonReleased(mouse::Button::Left), p) => {
                let p = p.or(state.last).unwrap_or(Point::ORIGIN);
                Message::Pointer(self.output, PointerKind::Up, p)
            }
            _ => return None,
        };
        Some(canvas::Action::publish(msg).and_capture())
    }

    fn draw(
        &self,
        _state: &CanvasState,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let scene = self.session.scene();
        let mine = |m: &&Mark| m.output() == self.output;
        let mut layers = Vec::with_capacity(4);
        if self.session.capturing() {
            // Show nothing so no marks are baked into the still.
            return layers;
        }

        // Back to front (the still and spotlight mask are layers below):
        // marks, draft, tip.
        let mut live = Frame::new(renderer, bounds.size());

        if self.session.fading() {
            // Fading marks change every frame; skip the cache while fading.
            for mark in scene.marks().iter().filter(mine) {
                draw_mark(&mut live, mark, self.session.mark_alpha(mark, self.now));
            }
        } else {
            let revision = (self.session.frozen_frame(), scene.revision());
            if self.ink.revision.get() != Some(revision) {
                self.ink.cache.clear();
                self.ink.revision.set(Some(revision));
            }
            layers.push(self.ink.cache.draw(renderer, bounds.size(), |frame| {
                for mark in scene.marks().iter().filter(mine) {
                    draw_mark(frame, mark, 1.0);
                }
            }));
        }

        if let Some(draft) = self.session.draft().filter(|m| m.output() == self.output) {
            draw_mark(&mut live, draft, 1.0);
        }
        match &self.wheel {
            Some(state) => wheel::draw(
                &mut live,
                bounds,
                state,
                &self.wheel_blocked,
                self.lens_hint,
            ),
            None => {
                if let Some((output, tip)) = self.session.tip(self.now)
                    && output == self.output
                {
                    paw::draw_tip(
                        &mut live,
                        tip,
                        bounds.size(),
                        self.pack,
                        point::forearm_color(self.session.settings()),
                    );
                }
            }
        }
        layers.push(live.into_geometry());
        layers
    }

    fn mouse_interaction(
        &self,
        _state: &CanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if self.wheel.is_some() {
            mouse::Interaction::Idle
        } else if self.session.active_tool() == text_tool::TOOL
            && self.session.is_pointing()
            && cursor.is_over(bounds)
        {
            mouse::Interaction::Text
        } else if self.session.is_pointing() && cursor.is_over(bounds) {
            // The paw is drawn in the scene so the stream shows it even when
            // cursor capture is disabled.
            mouse::Interaction::Hidden
        } else {
            mouse::Interaction::None
        }
    }
}

/// Preview tile shared by settings: one stroke and the pointer on a backdrop.
pub struct PreviewTile {
    pub forearm_color: Option<[u8; 4]>,
    pub background: Color,
    pub ink: Color,
    pub width: f32,
    pub laser: bool,
    pub stick: bool,
    pub stretch: bool,
    pub scale: f32,
    pub angle: f32,
    pub pack: Option<crate::packs::Pack>,
}

impl<Message> canvas::Program<Message> for PreviewTile {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let size = bounds.size();
        frame.fill(
            &Path::rounded_rectangle(Point::ORIGIN, size, 12.0.into()),
            self.background,
        );
        let (w, h) = (size.width, size.height);
        let tip = Point::new(w * 0.56, h * 0.25);
        let curve = Path::new(|b| {
            b.move_to(Point::new(w * 0.15, h * 0.68));
            b.bezier_curve_to(
                Point::new(w * 0.27, h * 0.25),
                Point::new(w * 0.6, h * 0.2),
                tip,
            );
        });
        frame.stroke(&curve, stroke_style(self.ink, self.width));
        let at = crate::session::Pt::new(tip.x, tip.y);
        let pose = if self.laser {
            crate::session::Tip::Laser { at, press: 0.0 }
        } else {
            crate::session::Tip::Paw {
                at,
                press: 0.0,
                scale: self.scale,
                angle: self.angle,
                stick: self.stick,
                stretch: self.stretch,
                grip_x: tip.x,
            }
        };
        paw::draw_tip(
            &mut frame,
            pose,
            size,
            self.pack.as_ref(),
            self.forearm_color,
        );
        vec![frame.into_geometry()]
    }
}
