//! Settings window: header with the master switch, a left navigation column
//! and one section at a time. Every control edits a registered setting through
//! the same typed update path as the CLI and quick bar.

use iced::widget::{
    Column, button, canvas, checkbox, column, container, pick_list, row, scrollable, slider, space,
    text, toggler, tooltip,
};
use iced::{Alignment, Color, Element, Length, Theme};

use super::icons::{Glyph, icon};
use super::{PreviewTile, UiMsg, theme};
use crate::app::Message;
use crate::features::{display, mascot, point, sound, system};
use crate::packs::{Draft, Pack};
use crate::registry::{Registry, ShortcutContext, Value};
use crate::session::{ActivationStyle, Session};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// In navigation order: Annotate, then Desktop, then App.
pub enum Page {
    #[default]
    Pointer,
    /// Ink and text labels.
    Ink,
    /// Spotlight and Lens, which also work outside annotation.
    Tools,
    Character,
    /// How drawing starts, and every shortcut.
    Shortcuts,
    Sound,
    Display,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CharacterTab {
    /// Pick, add, export or remove characters.
    #[default]
    Library,
    /// The selected character on the desktop: shown, size, eyes, idle.
    Desktop,
    /// Customize → Artwork: arrange layers in the Arrange preview.
    Artwork,
    /// Customize → Behavior: reactions, lines, moving parts and music,
    /// tried in the Test character preview.
    Behavior,
    /// Customize → Advanced: resting point and stage size.
    Advanced,
}

/// The merged Windows header: sizes shared with the native hit test, and the
/// caption buttons TinyPaw draws in place of the system title bar.
pub mod caption {
    /// Header height in logical pixels.
    pub const HEADER: f32 = 44.0;
    /// Width of each caption button in logical pixels.
    pub const BUTTON_W: f32 = 46.0;
    /// Logical width left of the caption buttons that holds clickable
    /// controls (the Annotation switch); the rest of the header drags.
    pub const CONTROLS_W: f32 = 270.0;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Button {
        Minimize,
        Maximize,
        Close,
    }

    /// Live state of the merged header, when it is installed.
    #[derive(Debug, Clone, Copy, Default)]
    pub struct Frame {
        pub hover: Option<Button>,
        pub maximized: bool,
    }
}

/// Where the user asked to go while the character had unsaved changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leave {
    Page(Page),
    Character(&'static str),
    Close,
}

/// What Import… and Export… move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferScope {
    /// A whole character folder.
    Character,
    /// Speech lines, the bubble and the menu, as a dialogue file.
    Dialogue,
    /// A starter dialogue file to fill in (export only).
    Template,
}

/// The import and export panel on the Character page.
#[derive(Debug, Clone)]
pub enum Transfer {
    /// Import… or Export… was pressed: what to move.
    Choose { export: bool },
    /// A character folder whose images don't all match its canvas: how to
    /// fit them before importing.
    Fit {
        path: std::path::PathBuf,
        found: crate::mascot::pack::Inspected,
        choice: crate::mascot::pack::Fit,
    },
    /// A dialogue file, waiting on how to combine it with the lines.
    Dialogue {
        dialogue: crate::mascot::pack::Dialogue,
        merge: crate::mascot::lines::Merge,
        layout: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaveChoice {
    Save,
    Discard,
    Stay,
}

pub struct SettingsView<'a> {
    pub registry: &'a Registry,
    pub session: &'a Session,
    pub page: Page,
    pub character_tab: CharacterTab,
    pub displays: &'a [crate::platform::displays::Display],
    #[cfg(windows)]
    pub sources: &'a [crate::platform::windows::WindowSource],
    pub has_custom_sound: bool,
    /// Commands whose global shortcut is held by another app.
    pub taken: &'a [&'static str],
    pub notice: Option<&'a str>,
    pub packs: &'a [Pack],
    /// The pack the Point tool draws with.
    pub pack: Option<&'a Pack>,
    /// "Add your pointer" in progress.
    pub draft: Option<&'a Draft>,
    pub editing: bool,
    /// Set when the Windows merged header replaced the system title bar.
    pub frame: Option<caption::Frame>,
    /// Bundled Yohta first, then installed characters.
    pub characters: &'a [crate::mascot::pack::Character],
    /// The selected character's ID.
    pub character: &'static str,
    /// "Add character" in progress.
    pub character_draft: Option<&'a crate::mascot::pack::Draft>,
    pub character_busy: bool,
    /// A folder import waiting on Keep both or Replace: the character's name.
    pub character_conflict: Option<&'a str>,
    /// The import and export panel, while open.
    pub transfer: Option<&'a Transfer>,
    /// The Art tab's open character, while editing.
    pub art: Option<&'a crate::mascot::editor::Editor>,
    /// The Motion tab's live preview, on the Character page.
    pub motion: Option<&'a crate::mascot::preview::MotionPreview>,
    /// Create from reference in progress, with the reference's thumbnail.
    pub reference: Option<(
        &'a crate::mascot::reference::Request,
        Option<&'a iced::advanced::image::Handle>,
    )>,
    /// The character draft has changes Save would write.
    pub unsaved: bool,
    /// Save / Discard / Stay is waiting for an answer.
    pub leaving: bool,
    /// Music response status and live level, for the meter.
    pub music: (String, f32),
    /// What music response listens to, and the choices for the picker.
    pub music_source: crate::mascot::music::Source,
    /// Sources to pick from with their live meters (0–1, decibel scale)
    /// and whether each is playing; then the input devices.
    pub music_rows: Vec<(crate::mascot::music::Source, f32, bool)>,
    pub music_inputs: Vec<crate::mascot::music::Source>,
    /// The Follow beat trial's status line.
    pub beat: String,
}

mod character;
mod controls;
mod display_tab;
mod ink;
mod pointer;
mod sound_tab;
use character::character_page;
use controls::*;
use display_tab::*;
use ink::*;
use pointer::*;
use sound_tab::*;

fn h1<'a>(label: &'a str) -> Element<'a, Message> {
    text(label)
        .font(theme::HEADING)
        .size(22)
        .color(theme::TEXT)
        .into()
}

fn h2<'a>(label: &'a str) -> Element<'a, Message> {
    text(label)
        .font(theme::MEDIUM)
        .size(15)
        .color(theme::TEXT_SOFT)
        .into()
}

/// How to use and adjust a Lens or Spotlight anywhere shortcut, with its
/// current binding.
fn anywhere_hint(
    registry: &crate::registry::Registry,
    id: &str,
    fallback: &str,
    size: &str,
    more: &str,
) -> String {
    let key = registry
        .shortcut_label(id)
        .unwrap_or_else(|| fallback.to_owned());
    format!(
        "Anywhere: tap {key} to toggle, or hold it to show until release.          While holding, Up/Down change the {size} and +/− the {more}.          In annotation, scroll for {size}, Shift+scroll for {more}."
    )
}

fn hint<'a>(label: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(label).size(12).color(theme::MUTED).into()
}

fn keycap<'a>(label: String) -> Element<'a, Message> {
    container(text(label.replace('+', " ")).size(12).color(theme::SUBTLE))
        .padding([3, 8])
        .style(theme::keycap)
        .into()
}

fn section<'a>(children: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    Column::with_children(children).spacing(10).into()
}

/// A library card's name, shortened to fit the 92 px card.
fn card_label(name: &str) -> String {
    const MAX: usize = 12;
    if name.chars().count() <= MAX {
        return name.to_owned();
    }
    let mut short: String = name.chars().take(MAX - 1).collect();
    short.push('…');
    short
}

/// An inline jump to the place a related setting lives.
fn link<'a>(label: &'a str, msg: Message) -> Element<'a, Message> {
    button(text(label).size(12).color(theme::CYAN_TEXT))
        .padding([2, 4])
        .style(theme::tool_button(theme::Tone::Plain))
        .on_press(msg)
        .into()
}

/// A hint followed by a link, on one line.
fn hint_link<'a>(
    label: impl text::IntoFragment<'a>,
    link_label: &'a str,
    msg: Message,
) -> Element<'a, Message> {
    row![hint(label), link(link_label, msg)]
        .spacing(6)
        .align_y(Alignment::Center)
        .wrap()
        .into()
}

/// When ink and tap stamps fade; one setting shared by both.
fn fade_picker<'a>(s: &crate::registry::SettingValues) -> Element<'a, Message> {
    pick_list(
        [FadeChoice::Keep, FadeChoice::ThreeSeconds],
        Some(if s.choice(system::INK_FADE) == "3s" {
            FadeChoice::ThreeSeconds
        } else {
            FadeChoice::Keep
        }),
        |f| Message::Set(system::INK_FADE, Value::Choice(f.id())),
    )
    .text_size(13)
    .padding([8, 10])
    .style(theme::select)
    .into()
}

fn later<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    tooltip(
        content,
        container(text("Coming in a later alpha").size(12))
            .padding([6, 10])
            .style(theme::fill(theme::HOVER, 8.0)),
        tooltip::Position::Top,
    )
    .into()
}

pub fn view(v: SettingsView<'_>) -> Element<'_, Message> {
    let panel = iced::widget::keyed_column([(v.page, main(&v))])
        .width(Length::Fill)
        .height(Length::Fill);
    let body = row![nav(v.page), panel].height(Length::Fill);
    container(column![header(&v), body])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::fill(theme::SURFACE, 0.0))
        .into()
}

fn header<'a>(v: &SettingsView<'a>) -> Element<'a, Message> {
    let on = v.session.is_enabled();
    let logo = container(
        iced::widget::image(crate::branding::logo())
            .width(30)
            .height(30),
    )
    .width(30)
    .height(30)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center);
    let switch = toggler(on)
        .label("Annotation")
        .text_size(14)
        .size(22)
        .spacing(10)
        .font(theme::MEDIUM)
        .style(theme::master_toggler)
        .on_toggle(|_| Message::Command(system::MASTER_TOGGLE));
    let mut right = row![].spacing(14).align_y(Alignment::Center);
    if let Some(label) = v.registry.shortcut_label(system::MASTER_TOGGLE) {
        right = right.push(keycap(label));
    }
    right = right.push(container(switch).padding([6, 12]).style(theme::outlined(
        theme::CONTROL,
        999.0,
        theme::LINE,
    )));
    if let Some(frame) = v.frame {
        return framed_header(logo, right, frame);
    }
    container(
        row![
            row![logo, text("TinyPaw").font(theme::HEADING).size(17)]
                .spacing(10)
                .align_y(Alignment::Center),
            space().width(Length::Fill),
            right,
        ]
        .align_y(Alignment::Center),
    )
    .height(60)
    .padding([0, 20])
    .align_y(Alignment::Center)
    .style(theme::outlined(theme::HEADER, 0.0, theme::LINE))
    .into()
}

/// One row in place of the Windows title bar. Everything outside the switch
/// area and the caption buttons drags the window; the native hit test uses
/// the same `caption` sizes, so the two must change together.
fn framed_header<'a>(
    logo: impl Into<Element<'a, Message>>,
    controls: iced::widget::Row<'a, Message>,
    frame: caption::Frame,
) -> Element<'a, Message> {
    use caption::{BUTTON_W, Button, CONTROLS_W, HEADER};
    // Windows' own close-hover red, so the button reads as the system one.
    let close_red = Color::from_rgb8(0xC4, 0x2B, 0x1C);
    let cap = |b: Button, glyph: Glyph| -> Element<'a, Message> {
        let hovered = frame.hover == Some(b);
        let (bg, fg) = match (b, hovered) {
            (Button::Close, true) => (close_red, Color::WHITE),
            (_, true) => (theme::HOVER, theme::TEXT),
            _ => (Color::TRANSPARENT, theme::TEXT_SOFT),
        };
        container(icon(glyph, 14.0, fg))
            .width(BUTTON_W)
            .height(HEADER)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(theme::fill(bg, 0.0))
            .into()
    };
    container(
        row![
            space().width(14),
            logo.into(),
            space().width(10),
            text("TinyPaw Settings").font(theme::HEADING).size(14),
            space().width(Length::Fill),
            container(controls)
                .width(CONTROLS_W)
                .padding([0, 10])
                .align_x(Alignment::End),
            cap(Button::Minimize, Glyph::WinMinimize),
            cap(
                Button::Maximize,
                if frame.maximized {
                    Glyph::WinRestore
                } else {
                    Glyph::WinMaximize
                },
            ),
            cap(Button::Close, Glyph::Close),
        ]
        .align_y(Alignment::Center),
    )
    .height(HEADER)
    .style(theme::outlined(theme::HEADER, 0.0, theme::LINE))
    .into()
}

fn nav<'a>(page: Page) -> Element<'a, Message> {
    let item = |p: Page, glyph: Glyph, label: &'static str| {
        let on = page == p;
        button(
            row![
                icon(
                    glyph,
                    18.0,
                    if on { theme::CYAN_TEXT } else { theme::SUBTLE }
                ),
                text(label)
                    .size(14)
                    .font(if on { theme::MEDIUM } else { theme::BODY }),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .height(40)
        .padding([10, 12])
        .style(theme::nav_button(on))
        .on_press(Message::Ui(UiMsg::Nav(p)))
    };
    container(
        column![
            hint("Annotate"),
            item(Page::Pointer, Glyph::Pointer, "Pointer"),
            item(Page::Ink, Glyph::Pen, "Ink & text"),
            item(Page::Tools, Glyph::Lens, "Spotlight & lens"),
            space().height(10),
            hint("Desktop"),
            item(Page::Character, Glyph::Head, "Character"),
            space().height(10),
            hint("App"),
            item(Page::Shortcuts, Glyph::Controls, "Shortcuts"),
            item(Page::Sound, Glyph::Sound, "Sound"),
            item(Page::Display, Glyph::Display, "Display & motion"),
        ]
        .spacing(4),
    )
    .width(208)
    .height(Length::Fill)
    .padding([16, 12])
    .style(theme::outlined(theme::NAV, 0.0, theme::LINE))
    .into()
}

fn main<'a>(v: &SettingsView<'a>) -> Element<'a, Message> {
    let mut content: Vec<Element<'a, Message>> = Vec::new();
    if let Some(notice) = v.notice {
        content.push(
            container(text(notice).size(13).color(theme::AMBER_TEXT))
                .padding([8, 12])
                .style(theme::fill(theme::AMBER_BG, 8.0))
                .into(),
        );
    }
    let (body, side) = match v.page {
        Page::Pointer => pointer_page(v),
        Page::Ink => (ink_page(v), None),
        Page::Tools => (tools_page(v), None),
        Page::Shortcuts => (controls(v), None),
        Page::Display => (display_page(v), None),
        Page::Sound => (sound_page(v), None),
        Page::Character => character_page(v),
    };
    content.push(body);
    let pad = |left: f32, right: f32| iced::Padding {
        top: 24.0,
        right,
        bottom: 24.0,
        left,
    };
    // The side panel (preview or pointer editor) scrolls separately, so the
    // preview stays in view while the settings beside it scroll. The same
    // two-column structure is kept without one: switching between a page
    // with a side panel and one without (on the same Page) must not hand
    // one scroll area's widget state to a different widget.
    let has_side = side.is_some();
    let side = match side {
        Some(side) => container(side).padding(pad(14.0, 28.0)),
        None => container(space().width(0)),
    };
    row![
        scrollable(
            container(Column::with_children(content).spacing(16))
                .padding(pad(28.0, if has_side { 14.0 } else { 28.0 }))
                .width(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill),
        scrollable(side).width(Length::Shrink).height(Length::Fill),
    ]
    .into()
}
