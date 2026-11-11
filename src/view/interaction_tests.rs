//! Exercise real widget layouts and mouse press/release routing, including
//! canvases beside buttons. Command/state tests alone cannot catch a swallowed click.
use super::*;
use iced::advanced::{Layout, Shell, clipboard, layout, widget::Tree};

pub(super) fn click_layout(mut element: Element<'_, Message>, size: Size) -> Vec<Message> {
    let renderer = Renderer::new(iced::Font::DEFAULT, 14.0.into());
    let mut tree = Tree::new(element.as_widget());
    let node = element.as_widget_mut().layout(
        &mut tree,
        &renderer,
        &layout::Limits::new(Size::ZERO, size),
    );
    fn centres(layout: Layout<'_>, viewport: Rectangle, points: &mut Vec<Point>) {
        let bounds = layout.bounds();
        if bounds.width > 0.0 && bounds.height > 0.0 && viewport.contains(bounds.center()) {
            points.push(bounds.center());
        }
        for child in layout.children() {
            centres(child, viewport, points);
        }
    }
    let viewport = Rectangle::with_size(size);
    let mut points = Vec::new();
    centres(Layout::new(&node), viewport, &mut points);
    let mut messages = Vec::new();
    for point in points {
        for event in [
            mouse::Event::CursorMoved { position: point },
            mouse::Event::ButtonPressed(mouse::Button::Left),
            mouse::Event::ButtonReleased(mouse::Button::Left),
        ] {
            element.as_widget_mut().update(
                &mut tree,
                &iced::Event::Mouse(event),
                Layout::new(&node),
                mouse::Cursor::Available(point),
                &renderer,
                &mut clipboard::Null,
                &mut Shell::new(&mut messages),
                &viewport,
            );
        }
    }
    messages
}

fn pointing(registry: &Registry) -> Session {
    let tool = registry.default_tool();
    let mut session = Session::new(tool.id, tool.factory, registry.defaults());
    session.handle(crate::session::Event::MasterToggle, 0);
    session.handle(
        crate::session::Event::InputAcquired(session.pending_ticket().unwrap()),
        1,
    );
    session
}

#[test]
fn overlay_buttons_receive_complete_clicks_above_the_annotation_canvas() {
    let registry = Registry::build(crate::features::BUILTINS).unwrap();
    let session = pointing(&registry);
    let ink = InkCache::default();
    let ui = OverlayUi {
        more_menu: true,
        ..OverlayUi::default()
    };
    let messages = click_layout(
        overlay(Overlay {
            registry: &registry,
            session: &session,
            ink: &ink,
            ui: &ui,
            output: OutputId(0),
            now: 1,
            still: None,
            notice: None,
            pack: None,
        }),
        Size::new(1100.0, 900.0),
    );
    for expected in [
        text_tool::SELECT,
        pin::SELECT,
        system::UNDO,
        system::EXIT,
        spotlight::SELECT,
        upcoming::LENS,
        freeze::TOGGLE,
        system::CLEAR,
        sound::TOGGLE_MUTE,
    ] {
        assert!(
            messages
                .iter()
                .any(|m| matches!(m, Message::Command(id) if *id == expected)),
            "button did not emit {expected}: {messages:?}"
        );
    }
    for expected in ["TogglePointerMenu", "ToggleInkMenu", "ToggleMoreMenu"] {
        assert!(
            messages
                .iter()
                .any(|m| matches!(m, Message::Ui(ui) if format!("{ui:?}") == expected)),
            "button did not emit {expected}"
        );
    }
}

fn settings_view<'a>(
    registry: &'a Registry,
    session: &'a Session,
    packs: &'a [crate::packs::Pack],
    page: settings::Page,
) -> settings::SettingsView<'a> {
    settings::SettingsView {
        registry,
        session,
        page,
        character_tab: Default::default(),
        displays: &[],
        #[cfg(windows)]
        sources: &[],
        has_custom_sound: false,
        taken: &[],
        notice: None,
        packs,
        pack: packs.first(),
        draft: None,
        editing: false,
        frame: None,
        characters: &[],
        character: "yohta",
        character_draft: None,
        character_busy: false,
        character_conflict: None,
        art: None,
        motion: None,
        unsaved: false,
        leaving: false,
        music: ("Off".into(), 0.0),
        music_source: crate::mascot::music::Source::All,
        music_rows: vec![(crate::mascot::music::Source::All, 0.0, false)],
        music_inputs: Vec::new(),
        beat: "Off".into(),
    }
}

fn assert_ui(messages: &[Message], expected: &[&str]) {
    for name in expected {
        assert!(
            messages
                .iter()
                .any(|m| matches!(m, Message::Ui(ui) if format!("{ui:?}") == *name)),
            "button did not emit {name}: {messages:?}"
        );
    }
}

#[test]
fn settings_navigation_and_page_actions_receive_clicks() {
    let registry = Registry::build(crate::features::BUILTINS).unwrap();
    let session = pointing(&registry);
    let packs = crate::packs::load_all().0;
    use settings::Page;
    for (page, expected) in [
        (
            Page::Pointer,
            vec![
                "AddPointer",
                "ImportPack",
                "CopyPointerPrompt",
                "CopyForearmPrompt",
                "PreviewSound",
            ],
        ),
        (
            Page::Character,
            vec!["AddCharacter", "ArtFromParts", "ImportCharacter"],
        ),
        (Page::Annotation, vec!["RefreshSources"]),
        (Page::Controls, vec!["ResetShortcuts"]),
        (Page::Display, vec![]),
        (Page::Sound, vec!["PreviewSound", "ImportWav"]),
    ] {
        let messages = click_layout(
            settings::view(settings_view(&registry, &session, &packs, page)),
            Size::new(1200.0, 2400.0),
        );
        assert_ui(&messages, &expected);
        for target in [
            Page::Pointer,
            Page::Character,
            Page::Annotation,
            Page::Controls,
            Page::Display,
            Page::Sound,
        ] {
            assert!(
                messages
                    .iter()
                    .any(|m| matches!(m, Message::Ui(UiMsg::Nav(p)) if *p == target)),
                "navigation to {target:?} failed on {page:?}"
            );
        }
    }
}

#[test]
fn character_tabs_show_the_selected_controls_and_keep_the_preview_interactive() {
    use crate::features::mascot;
    use crate::mascot::{preview::MotionPreview, rig::Rig};
    use settings::CharacterTab;

    let registry = Registry::build(crate::features::BUILTINS).unwrap();
    let session = pointing(&registry);
    let rig = Rig::load();
    let defaults = rig.motion().to_vec();
    let preview = MotionPreview::new(
        "yohta",
        rig,
        defaults,
        Default::default(),
        Default::default(),
        Default::default(),
        Default::default(),
        Default::default(),
    );
    let tabs = [CharacterTab::Home, CharacterTab::Behavior];
    for tab in tabs {
        let mut view = settings_view(&registry, &session, &[], settings::Page::Character);
        view.motion = Some(&preview);
        view.character_tab = tab;
        let messages = click_layout(settings::view(view), Size::new(1040.0, 4000.0));
        for target in [
            CharacterTab::Home,
            CharacterTab::Artwork,
            CharacterTab::Behavior,
            CharacterTab::Advanced,
        ] {
            assert!(
                messages
                    .iter()
                    .any(|m| matches!(m, Message::Ui(UiMsg::CharacterTab(t)) if *t == target))
            );
        }
        // Behavior has a row for every line trigger, each with Try.
        for t in crate::mascot::lines::Trigger::ALL {
            let row = messages.iter().any(|m| matches!(m, Message::Ui(UiMsg::LineAdd(x)) if *x == t));
            let try_ = messages.iter().any(|m| {
                matches!(m, Message::Ui(UiMsg::MotionTest(crate::mascot::preview::Test::Say(x))) if *x == t)
            });
            assert_eq!((row, try_), (tab == CharacterTab::Behavior, tab == CharacterTab::Behavior), "{t:?}");
        }
        assert!(
            messages
                .iter()
                .any(|m| matches!(m, Message::Ui(UiMsg::MotionTest(_))))
        );
        for (id, owner) in [
            (mascot::ENABLED, CharacterTab::Home),
            (mascot::LINES, CharacterTab::Behavior),
            (mascot::MUSIC, CharacterTab::Behavior),
        ] {
            assert_eq!(
                messages
                    .iter()
                    .any(|m| matches!(m, Message::Set(key, _) if *key == id)),
                tab == owner
            );
        }
    }
}

#[test]
fn one_save_and_the_leave_prompt_appear_only_with_unsaved_work() {
    use crate::mascot::{preview::MotionPreview, rig::Rig};
    use settings::{CharacterTab, LeaveChoice};

    let registry = Registry::build(crate::features::BUILTINS).unwrap();
    let session = pointing(&registry);
    let rig = Rig::load();
    let defaults = rig.motion().to_vec();
    let preview = MotionPreview::new("yohta", rig, defaults, Default::default(), Default::default(), Default::default(), Default::default(), Default::default());
    let messages = |unsaved: bool, leaving: bool| {
        let mut view = settings_view(&registry, &session, &[], settings::Page::Character);
        view.motion = Some(&preview);
        view.character_tab = CharacterTab::Behavior;
        view.unsaved = unsaved;
        view.leaving = leaving;
        click_layout(settings::view(view), Size::new(1040.0, 1200.0))
    };
    let has = |m: &[Message], f: &dyn Fn(&UiMsg) -> bool| {
        m.iter().any(|m| matches!(m, Message::Ui(u) if f(u)))
    };
    let clean = messages(false, false);
    assert!(!has(&clean, &|u| matches!(u, UiMsg::CharacterSave | UiMsg::CharacterDiscard)));
    assert!(!has(&clean, &|u| matches!(u, UiMsg::Leave(_))));
    let dirty = messages(true, false);
    assert!(has(&dirty, &|u| matches!(u, UiMsg::CharacterSave)));
    assert!(has(&dirty, &|u| matches!(u, UiMsg::CharacterDiscard)));
    assert!(!has(&dirty, &|u| matches!(u, UiMsg::Leave(_))));
    let leaving = messages(true, true);
    for c in [LeaveChoice::Save, LeaveChoice::Discard, LeaveChoice::Stay] {
        assert!(has(&leaving, &|u| matches!(u, UiMsg::Leave(x) if *x == c)), "{c:?}");
    }
}

#[test]
fn pointer_editor_save_and_cancel_work_beside_the_live_pair_preview() {
    let registry = Registry::build(crate::features::BUILTINS).unwrap();
    let session = pointing(&registry);
    let packs = crate::packs::load_all().0;
    let rgba = std::sync::Arc::new(vec![0, 0, 0, 0, 0, 100, 255, 255]);
    let draft = crate::packs::Draft {
        name: "Review pointer".into(),
        image: image::Handle::from_rgba(2, 1, rgba.to_vec()),
        width: 2,
        height: 1,
        rgba,
        hotspot: (0.75, 0.0),
        wrist: (0.5, 1.0),
        joint_width: 0.4,
        forearm: None,
        pressed: None,
        tap: None,
    };
    let mut view = settings_view(&registry, &session, &packs, settings::Page::Pointer);
    view.draft = Some(&draft);
    let messages = click_layout(settings::view(view), Size::new(1200.0, 2400.0));
    assert_ui(
        &messages,
        &[
            "SaveDraft",
            "CancelDraft",
            "AddForearm",
            "AddPressed",
            "AddPackTap",
        ],
    );
}

#[test]
fn first_run_buttons_receive_clicks() {
    let registry = Registry::build(crate::features::BUILTINS).unwrap();
    let messages = click_layout(first_run::view(&registry), Size::new(600.0, 400.0));
    assert_ui(&messages, &["TurnOn", "OpenSettings"]);
}

#[test]
fn generated_forearm_colour_controls_receive_clicks() {
    let registry = Registry::build(crate::features::BUILTINS).unwrap();
    let mut session = pointing(&registry);
    let mut values = session.settings().clone();
    registry
        .set(&mut values, point::PRESENTATION, Value::Choice("stretch"))
        .unwrap();
    session.handle(crate::session::Event::SettingsChanged(values), 2);
    let packs = crate::packs::load_all().0;
    let messages = click_layout(
        settings::view(settings_view(
            &registry,
            &session,
            &packs,
            settings::Page::Pointer,
        )),
        Size::new(1200.0, 2400.0),
    );
    assert!(
        messages
            .iter()
            .any(|m| matches!(m, Message::Ui(UiMsg::ForearmColor(_))))
    );
    assert!(messages.iter().any(
        |m| matches!(m, Message::Set(id, Value::Bool(false)) if *id == point::FOREARM_AUTO_COLOR)
    ));
    let mut values = session.settings().clone();
    registry
        .set(
            &mut values,
            point::FOREARM_COLOR,
            Value::Color([20, 30, 80, 255]),
        )
        .unwrap();
    assert_eq!(
        point::forearm_color(&values),
        None,
        "auto retains the pack colour"
    );
    registry
        .set(&mut values, point::FOREARM_AUTO_COLOR, Value::Bool(false))
        .unwrap();
    assert_eq!(point::forearm_color(&values), Some([20, 30, 80, 255]));
}
