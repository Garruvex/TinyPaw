use super::*;
use crate::features::{self, point, system};
use crate::registry::{Registry, Value};

const O: OutputId = OutputId(0);

fn registry() -> Registry {
    Registry::build(features::BUILTINS).expect("valid built-ins")
}

fn session_with(style: &'static str) -> Session {
    let reg = registry();
    let mut values = reg.defaults();
    reg.set(&mut values, system::ACTIVATION_STYLE, Value::Choice(style))
        .unwrap();
    Session::new(point::TOOL, point::new_state, values)
}

fn ptr(kind: PointerKind, x: f32, y: f32) -> Event {
    Event::Pointer(PointerEvent {
        kind,
        output: O,
        pos: Pt::new(x, y),
        at: 0,
        paw_angle: None,
    })
}

/// Completes the pending acquisition as the platform would.
fn ack(s: &mut Session) {
    let t = s.pending_ticket().expect("pending acquisition");
    s.handle(Event::InputAcquired(t), 0);
}

fn draw_line(s: &mut Session, from: (f32, f32), to: (f32, f32)) -> Vec<Effect> {
    let mut fx = s.handle(ptr(PointerKind::Down, from.0, from.1), 0);
    fx.extend(s.handle(
        ptr(
            PointerKind::Moved,
            (from.0 + to.0) / 2.0,
            (from.1 + to.1) / 2.0,
        ),
        0,
    ));
    fx.extend(s.handle(ptr(PointerKind::Moved, to.0, to.1), 0));
    fx.extend(s.handle(ptr(PointerKind::Up, to.0, to.1), 0));
    fx
}

#[test]
fn starts_off_and_hidden() {
    let s = session_with("toggle");
    assert_eq!(s.mode(), Mode::Off);
    assert_eq!(s.overlay_state(), OverlayState::Hidden);
}

#[test]
fn toggle_mode_acquires_input_then_points() {
    let mut s = session_with("toggle");
    s.handle(Event::MasterToggle, 0);
    assert_eq!(s.mode(), Mode::Passthrough);
    assert_eq!(s.overlay_state(), OverlayState::Interactive);
    assert!(s.tip(0).is_none(), "no paw until input is acknowledged");
    ack(&mut s);
    assert_eq!(s.mode(), Mode::Annotating(Ownership::Latched));
}

#[test]
fn stale_acknowledgement_is_ignored() {
    let mut s = session_with("toggle");
    s.handle(Event::MasterToggle, 0);
    let stale = s.pending_ticket().unwrap();
    s.handle(Event::MasterToggle, 0); // Off
    s.handle(Event::MasterToggle, 0); // On again, new generation
    s.handle(Event::InputAcquired(stale), 0);
    assert_eq!(s.mode(), Mode::Passthrough);
    ack(&mut s);
    assert!(s.is_pointing());
}

#[test]
fn tap_plays_sound_and_draws_nothing() {
    let mut s = session_with("toggle");
    s.handle(Event::MasterToggle, 0);
    ack(&mut s);
    s.handle(ptr(PointerKind::Down, 10.0, 10.0), 0);
    let fx = s.handle(ptr(PointerKind::Up, 11.0, 10.0), 5);
    assert_eq!(fx, vec![Effect::PlayTap]);
    assert!(s.scene().is_empty());
    assert!(s.animating(50));
    assert!(!s.animating(200));
}

#[test]
fn drag_draws_from_original_press_without_tap() {
    let mut s = session_with("toggle");
    s.handle(Event::MasterToggle, 0);
    ack(&mut s);
    let fx = draw_line(&mut s, (10.0, 10.0), (60.0, 10.0));
    assert!(!fx.contains(&Effect::PlayTap));
    assert_eq!(s.scene().marks().len(), 1);
    let Mark::Stroke(stroke) = &s.scene().marks()[0] else {
        panic!("expected a stroke")
    };
    assert_eq!(stroke.points[0], Pt::new(10.0, 10.0));
    assert_eq!(*stroke.points.last().unwrap(), Pt::new(60.0, 10.0));
    assert_eq!(stroke.width, 4.0);
    assert_eq!(stroke.color, Rgba::CYAN);
}

#[test]
fn stretch_grip_animates_and_drag_stays_pressed_until_release() {
    let reg = registry();
    let mut values = reg.defaults();
    reg.set(&mut values, point::PRESENTATION, Value::Choice("stretch"))
        .unwrap();
    let mut s = Session::new(point::TOOL, point::new_state, values);
    s.handle(Event::MasterToggle, 0);
    ack(&mut s);
    s.handle(ptr(PointerKind::Moved, 10.0, 10.0), 0);
    s.handle(ptr(PointerKind::Down, 10.0, 10.0), 10);
    s.handle(ptr(PointerKind::Moved, 60.0, 10.0), 20);
    let press = |s: &Session, now| match s.tip(now) {
        Some((
            _,
            Tip::Paw {
                press,
                stretch: true,
                ..
            },
        )) => press,
        other => panic!("expected stretch paw, got {other:?}"),
    };
    assert_eq!(press(&s, 20), 1.0);
    assert!(s.animating(30));
    let fx = s.handle(ptr(PointerKind::Up, 60.0, 10.0), 40);
    assert!(!fx.contains(&Effect::PlayTap));
    assert_eq!(press(&s, 40), 1.0);
    assert!((press(&s, 90) - 0.5).abs() < 0.001);
    assert_eq!(press(&s, 140), 0.0);
    assert!(!s.animating(2000));
}

#[test]
fn undo_and_clear() {
    let mut s = session_with("toggle");
    s.handle(Event::MasterToggle, 0);
    ack(&mut s);
    draw_line(&mut s, (0.0, 0.0), (50.0, 0.0));
    draw_line(&mut s, (0.0, 20.0), (50.0, 20.0));
    s.handle(Event::Undo, 0);
    assert_eq!(s.scene().marks().len(), 1);
    draw_line(&mut s, (0.0, 40.0), (50.0, 40.0));
    s.handle(Event::Clear, 0);
    assert!(s.scene().is_empty());
    assert!(s.is_pointing(), "clear stays in annotation mode");
}

#[test]
fn off_clears_marks_and_returns_focus() {
    let mut s = session_with("toggle");
    s.handle(Event::MasterToggle, 0);
    ack(&mut s);
    draw_line(&mut s, (0.0, 0.0), (50.0, 0.0));
    let fx = s.handle(Event::MasterOff, 0);
    assert_eq!(fx, vec![Effect::ReturnFocus]);
    assert!(s.scene().is_empty());
    assert_eq!(s.overlay_state(), OverlayState::Hidden);
}

#[test]
fn off_mid_drag_discards_the_draft() {
    let mut s = session_with("toggle");
    s.handle(Event::MasterToggle, 0);
    ack(&mut s);
    s.handle(ptr(PointerKind::Down, 0.0, 0.0), 0);
    s.handle(ptr(PointerKind::Moved, 30.0, 0.0), 0);
    s.handle(Event::MasterOff, 0);
    assert!(s.draft().is_none());
    s.handle(Event::MasterToggle, 0);
    ack(&mut s);
    assert!(s.scene().is_empty());
}

#[test]
fn hold_to_draw_keeps_marks_click_through() {
    let mut s = session_with("hold");
    s.handle(Event::MasterToggle, 0);
    assert_eq!(s.mode(), Mode::Passthrough);
    assert_eq!(s.overlay_state(), OverlayState::Hidden);
    s.handle(Event::HoldDown, 0);
    s.handle(Event::HoldDown, 0); // auto-repeat ignored
    ack(&mut s);
    assert!(matches!(s.mode(), Mode::Annotating(Ownership::Held { .. })));
    draw_line(&mut s, (0.0, 0.0), (50.0, 0.0));
    let fx = s.handle(Event::HoldUp, 0);
    assert_eq!(fx, vec![Effect::ReturnFocus]);
    assert_eq!(s.mode(), Mode::Passthrough);
    assert_eq!(s.overlay_state(), OverlayState::ClickThrough);
    assert!(s.tip(0).is_none());
    // The next hold resumes the same session.
    s.handle(Event::HoldDown, 0);
    ack(&mut s);
    assert_eq!(s.scene().marks().len(), 1);
}

#[test]
fn hold_released_before_button_commits_and_swallows_button_up() {
    let mut s = session_with("hold");
    s.handle(Event::MasterToggle, 0);
    s.handle(Event::HoldDown, 0);
    ack(&mut s);
    s.handle(ptr(PointerKind::Down, 0.0, 0.0), 0);
    s.handle(ptr(PointerKind::Moved, 40.0, 0.0), 0);
    let fx = s.handle(Event::HoldUp, 0);
    assert!(fx.is_empty(), "input stays until the button is up");
    assert_eq!(s.scene().marks().len(), 1, "stroke committed at last point");
    assert_eq!(s.overlay_state(), OverlayState::Interactive);
    assert!(s.tip(0).is_none(), "paw hidden immediately");
    // A new hold cannot start before the button is up.
    s.handle(Event::HoldDown, 0);
    assert!(s.pending_ticket().is_none());
    s.handle(Event::HoldUp, 0);
    let fx = s.handle(ptr(PointerKind::Up, 40.0, 0.0), 0);
    assert_eq!(fx, vec![Effect::ReturnFocus]);
    assert_eq!(s.overlay_state(), OverlayState::ClickThrough);
    assert_eq!(s.scene().marks().len(), 1, "button-up drew nothing");
}

#[test]
fn hold_released_before_pending_acquire_cancels_it() {
    let mut s = session_with("hold");
    s.handle(Event::MasterToggle, 0);
    s.handle(Event::HoldDown, 0);
    let t = s.pending_ticket().unwrap();
    s.handle(Event::HoldUp, 0);
    s.handle(Event::InputAcquired(t), 0);
    assert_eq!(s.mode(), Mode::Passthrough);
    assert_eq!(s.overlay_state(), OverlayState::Hidden);
}

#[test]
fn master_off_during_hold_ignores_its_release_and_needs_fresh_press() {
    let mut s = session_with("hold");
    s.handle(Event::MasterToggle, 0);
    s.handle(Event::HoldDown, 0);
    ack(&mut s);
    s.handle(Event::MasterToggle, 0); // Off while held
    assert_eq!(s.mode(), Mode::Off);
    s.handle(Event::MasterToggle, 0); // On while key still down
    s.handle(Event::HoldDown, 0); // repeat of the old press
    assert!(s.pending_ticket().is_none());
    s.handle(Event::HoldUp, 0);
    assert_eq!(s.mode(), Mode::Passthrough);
    s.handle(Event::HoldDown, 0); // fresh press
    assert!(s.pending_ticket().is_some());
}

#[test]
fn hold_keys_do_nothing_while_off() {
    let mut s = session_with("hold");
    s.handle(Event::HoldDown, 0);
    assert!(s.pending_ticket().is_none());
    s.handle(Event::HoldUp, 0);
    assert_eq!(s.mode(), Mode::Off);
}

#[test]
fn latch_from_hold_survives_release() {
    let mut s = session_with("hold");
    s.handle(Event::MasterToggle, 0);
    s.handle(Event::HoldDown, 0);
    ack(&mut s);
    s.handle(Event::Latch, 0);
    s.handle(Event::HoldUp, 0);
    assert_eq!(s.mode(), Mode::Annotating(Ownership::Latched));
}

#[test]
fn failed_acquisition_stays_passthrough() {
    let mut s = session_with("toggle");
    s.handle(Event::MasterToggle, 0);
    let t = s.pending_ticket().unwrap();
    let fx = s.handle(Event::InputFailed(t), 0);
    assert_eq!(fx, vec![Effect::ReturnFocus]);
    assert_eq!(s.mode(), Mode::Passthrough);
    assert!(!s.is_pointing());
}

#[test]
fn interruption_ends_the_session() {
    let mut s = session_with("toggle");
    s.handle(Event::MasterToggle, 0);
    ack(&mut s);
    draw_line(&mut s, (0.0, 0.0), (50.0, 0.0));
    s.handle(Event::Interrupted, 0);
    assert_eq!(s.mode(), Mode::Off);
    assert!(s.scene().is_empty());
}

#[test]
fn stroke_ends_at_display_boundary() {
    let mut s = session_with("toggle");
    s.handle(Event::MasterToggle, 0);
    ack(&mut s);
    s.handle(ptr(PointerKind::Down, 0.0, 0.0), 0);
    s.handle(ptr(PointerKind::Moved, 30.0, 0.0), 0);
    s.handle(
        Event::Pointer(PointerEvent {
            kind: PointerKind::Moved,
            output: OutputId(1),
            pos: Pt::new(5.0, 0.0),
            at: 0,
            paw_angle: None,
        }),
        0,
    );
    assert_eq!(s.scene().marks().len(), 1);
    assert_eq!(s.draft().unwrap().output(), OutputId(1));
}

#[test]
fn faded_ink_expires_and_leaves_undo_history() {
    let reg = registry();
    let mut values = reg.defaults();
    reg.set(&mut values, system::INK_FADE, Value::Choice("3s"))
        .unwrap();
    let mut s = Session::new(point::TOOL, point::new_state, values);
    s.handle(Event::MasterToggle, 0);
    ack(&mut s);
    s.handle(ptr(PointerKind::Down, 0.0, 0.0), 100);
    s.handle(ptr(PointerKind::Moved, 40.0, 0.0), 4000); // held past 3 s
    s.handle(ptr(PointerKind::Up, 40.0, 0.0), 5000);
    let mark = s.scene().marks()[0].clone();
    assert_eq!(mark.born(), Some(5000), "fade starts on release");
    assert_eq!(s.mark_alpha(&mark, 7000), 1.0);
    assert!(s.next_fade(7000).is_some());
    assert!(s.mark_alpha(&mark, 8200) < 1.0);
    assert!(s.fade_animating(8200));
    s.handle(Event::Tick, 8500);
    assert!(s.scene().is_empty());
}

#[test]
fn keep_mode_never_fades() {
    let mut s = session_with("toggle");
    s.handle(Event::MasterToggle, 0);
    ack(&mut s);
    draw_line(&mut s, (0.0, 0.0), (50.0, 0.0));
    s.handle(Event::Tick, 60_000);
    assert_eq!(s.scene().marks().len(), 1);
    assert_eq!(s.next_fade(0), None);
}

fn stick_session() -> Session {
    let reg = registry();
    let mut values = reg.defaults();
    reg.set(&mut values, point::PRESENTATION, Value::Choice("stick"))
        .unwrap();
    let mut s = Session::new(point::TOOL, point::new_state, values);
    s.handle(Event::MasterToggle, 0);
    ack(&mut s);
    s
}

fn grip_x(s: &Session, now: Millis) -> (Pt, f32) {
    match s.tip(now) {
        Some((_, Tip::Paw { at, grip_x, .. })) => (at, grip_x),
        other => panic!("expected a paw tip, got {other:?}"),
    }
}

#[test]
fn stick_grip_eases_while_tip_stays_on_pointer() {
    let mut s = stick_session();
    s.handle(ptr(PointerKind::Moved, 100.0, 50.0), 0);
    assert_eq!(
        grip_x(&s, 0),
        (Pt::new(100.0, 50.0), 100.0),
        "first sample snaps"
    );
    assert!(!s.animating(0));

    s.handle(ptr(PointerKind::Moved, 500.0, 50.0), 1000);
    let (tip, early) = grip_x(&s, 1000);
    assert_eq!(tip, Pt::new(500.0, 50.0), "claw never lags");
    assert_eq!(early, 100.0);
    assert!(s.animating(1050));
    let (_, mid) = grip_x(&s, 1090);
    assert!(mid > 100.0 && mid < 500.0, "grip trails the pointer: {mid}");
    assert!(!s.animating(2000), "settles and stops requesting frames");
    assert!((grip_x(&s, 2000).1 - 500.0).abs() < 0.5);
}

#[test]
fn grip_easing_depends_on_elapsed_time_not_frame_count() {
    let mut s = stick_session();
    s.handle(ptr(PointerKind::Moved, 0.0, 0.0), 0);
    s.handle(ptr(PointerKind::Moved, 300.0, 0.0), 0);
    // Sampling more often must not change where the grip is at a given time.
    for t in (0..=60).step_by(6) {
        let _ = s.tip(t);
    }
    let a = grip_x(&s, 60).1;
    let mut t2 = stick_session();
    t2.handle(ptr(PointerKind::Moved, 0.0, 0.0), 0);
    t2.handle(ptr(PointerKind::Moved, 300.0, 0.0), 0);
    assert_eq!(a, grip_x(&t2, 60).1);
}

#[test]
fn furry_paw_movement_needs_no_frames() {
    let mut s = session_with("toggle");
    s.handle(Event::MasterToggle, 0);
    ack(&mut s);
    s.handle(ptr(PointerKind::Moved, 0.0, 0.0), 0);
    s.handle(ptr(PointerKind::Moved, 800.0, 0.0), 10);
    assert!(!s.animating(11));
}

fn select(s: &mut Session, id: &'static str, factory: ToolFactory) {
    s.handle(Event::SelectTool { id, factory }, 0);
}

fn tap(s: &mut Session, x: f32, y: f32) -> Vec<Effect> {
    let mut fx = s.handle(ptr(PointerKind::Down, x, y), 0);
    fx.extend(s.handle(ptr(PointerKind::Up, x, y), 0));
    fx
}

fn on(style: &'static str) -> Session {
    let mut s = session_with(style);
    s.handle(Event::MasterToggle, 0);
    if style == "toggle" {
        ack(&mut s);
    }
    s
}

#[test]
fn text_places_a_label_and_returns_to_point() {
    use crate::features::text;
    let mut s = on("toggle");
    select(&mut s, text::TOOL, text::new_state);
    tap(&mut s, 100.0, 100.0);
    assert!(s.text_draft().is_some());
    s.handle(Event::Text(TextInput::Changed("Loot here".into())), 0);
    s.handle(Event::Text(TextInput::Submit), 0);
    assert_eq!(s.active_tool(), point::TOOL);
    let Mark::Label(label) = &s.scene().marks()[0] else {
        panic!("expected a label")
    };
    assert_eq!(label.text, "Loot here");
}

#[test]
fn escape_cancels_a_text_draft_without_placing_it() {
    use crate::features::text;
    let mut s = on("toggle");
    select(&mut s, text::TOOL, text::new_state);
    tap(&mut s, 10.0, 10.0);
    s.handle(Event::Text(TextInput::Changed("繁體中文".into())), 0);
    s.handle(Event::Text(TextInput::Cancel), 0);
    assert!(s.text_draft().is_none());
    assert!(s.scene().is_empty());
    assert!(s.is_enabled(), "cancelling the draft does not exit");
}

#[test]
fn clicking_elsewhere_places_the_current_label() {
    use crate::features::text;
    let mut s = on("toggle");
    select(&mut s, text::TOOL, text::new_state);
    tap(&mut s, 10.0, 10.0);
    s.handle(Event::Text(TextInput::Changed("one".into())), 0);
    tap(&mut s, 200.0, 10.0);
    assert_eq!(s.scene().marks().len(), 1);
    assert!(s.text_draft().is_some_and(|d| d.text.is_empty()));
}

#[test]
fn pins_number_in_order_and_undo_frees_the_last_number() {
    use crate::features::pin;
    let mut s = on("toggle");
    select(&mut s, pin::TOOL, pin::new_state);
    tap(&mut s, 10.0, 10.0);
    tap(&mut s, 50.0, 10.0);
    let numbers = |s: &Session| -> Vec<u32> {
        s.scene()
            .marks()
            .iter()
            .filter_map(|m| match m {
                Mark::Pin(p) => Some(p.number),
                _ => None,
            })
            .collect()
    };
    assert_eq!(numbers(&s), vec![1, 2]);
    s.handle(Event::Undo, 0);
    tap(&mut s, 90.0, 10.0);
    assert_eq!(numbers(&s), vec![1, 2], "number 2 is reused");
    s.handle(Event::Clear, 0);
    tap(&mut s, 90.0, 10.0);
    assert_eq!(numbers(&s), vec![1], "clear resets numbering");
}

#[test]
fn undo_runs_across_stroke_label_and_pin_in_order() {
    use crate::features::{pin, text};
    let mut s = on("toggle");
    draw_line(&mut s, (0.0, 0.0), (50.0, 0.0));
    select(&mut s, text::TOOL, text::new_state);
    tap(&mut s, 10.0, 40.0);
    s.handle(Event::Text(TextInput::Changed("hi".into())), 0);
    s.handle(Event::Text(TextInput::Submit), 0);
    select(&mut s, pin::TOOL, pin::new_state);
    tap(&mut s, 80.0, 80.0);
    let kinds = |s: &Session| -> Vec<&'static str> {
        s.scene()
            .marks()
            .iter()
            .map(|m| match m {
                Mark::Stroke(_) => "stroke",
                Mark::PawPrint(_) => "paw print",
                Mark::Label(_) => "label",
                Mark::Pin(_) => "pin",
            })
            .collect()
    };
    assert_eq!(kinds(&s), vec!["stroke", "label", "pin"]);
    s.handle(Event::Undo, 0);
    assert_eq!(kinds(&s), vec!["stroke", "label"]);
    s.handle(Event::Undo, 0);
    assert_eq!(kinds(&s), vec!["stroke"]);
}

#[test]
fn spotlight_shows_only_while_pointing() {
    use crate::features::spotlight;
    let mut s = session_with("hold");
    s.handle(Event::MasterToggle, 0);
    s.handle(Event::HoldDown, 0);
    ack(&mut s);
    select(&mut s, spotlight::TOOL, spotlight::new_state);
    s.handle(ptr(PointerKind::Moved, 300.0, 200.0), 0);
    assert_eq!(s.spotlight().map(|(_, _, r)| r), Some(160.0));
    s.handle(Event::HoldUp, 0);
    assert!(
        s.spotlight().is_none(),
        "mask hides when input returns to the game"
    );
}

fn capture_ticket(fx: &[Effect]) -> Ticket {
    fx.iter()
        .find_map(|e| match e {
            Effect::CaptureStill(t) => Some(*t),
            _ => None,
        })
        .expect("a capture request")
}

#[test]
fn freeze_draws_on_a_separate_scene_and_return_live_restores_marks() {
    let mut s = on("toggle");
    draw_line(&mut s, (0.0, 0.0), (50.0, 0.0));
    let t = capture_ticket(&s.handle(Event::FreezeToggle, 0));
    assert!(s.capturing());
    assert!(s.tip(0).is_none(), "nothing drawn until the still is up");
    s.handle(
        Event::StillReady {
            ticket: t,
            frame: 7,
        },
        0,
    );
    assert_eq!(s.frozen_frame(), Some(7));
    assert!(s.scene().is_empty(), "frozen scene starts empty");
    draw_line(&mut s, (0.0, 20.0), (50.0, 20.0));
    assert_eq!(s.scene().marks().len(), 1);
    let fx = s.handle(Event::FreezeToggle, 0);
    assert!(fx.contains(&Effect::ReleaseStill));
    assert_eq!(s.frozen_frame(), None);
    assert_eq!(s.scene().marks().len(), 1, "live marks are back");
    assert!(s.is_pointing());
}

#[test]
fn freeze_from_passthrough_takes_input_only_after_the_still() {
    let mut s = session_with("hold");
    s.handle(Event::MasterToggle, 0);
    let t = capture_ticket(&s.handle(Event::FreezeToggle, 0));
    assert!(
        s.pending_ticket().is_none(),
        "no focus change before capture"
    );
    s.handle(
        Event::StillReady {
            ticket: t,
            frame: 1,
        },
        0,
    );
    assert_eq!(s.overlay_state(), OverlayState::Interactive);
    ack(&mut s);
    assert!(s.is_pointing());
    let fx = s.handle(Event::FreezeToggle, 0);
    assert!(fx.contains(&Effect::ReturnFocus));
    assert_eq!(s.mode(), Mode::Passthrough);
}

#[test]
fn late_still_after_off_is_released() {
    let mut s = on("toggle");
    let t = capture_ticket(&s.handle(Event::FreezeToggle, 0));
    s.handle(Event::MasterOff, 0);
    let fx = s.handle(
        Event::StillReady {
            ticket: t,
            frame: 3,
        },
        0,
    );
    assert!(fx.contains(&Effect::ReleaseStill));
    assert_eq!(s.frozen_frame(), None);
    assert_eq!(s.mode(), Mode::Off);
}

#[test]
fn failed_capture_keeps_the_live_scene() {
    let mut s = on("toggle");
    draw_line(&mut s, (0.0, 0.0), (50.0, 0.0));
    let t = capture_ticket(&s.handle(Event::FreezeToggle, 0));
    let fx = s.handle(Event::StillFailed(t), 0);
    assert!(matches!(fx.as_slice(), [Effect::Notice(_)]));
    assert!(!s.capturing());
    assert_eq!(s.scene().marks().len(), 1);
}

#[test]
fn frozen_scene_survives_a_hold_release() {
    let mut s = session_with("hold");
    s.handle(Event::MasterToggle, 0);
    s.handle(Event::HoldDown, 0);
    ack(&mut s);
    let t = capture_ticket(&s.handle(Event::FreezeToggle, 0));
    s.handle(
        Event::StillReady {
            ticket: t,
            frame: 2,
        },
        0,
    );
    s.handle(Event::HoldUp, 0);
    assert!(s.is_pointing(), "frozen annotation is latched");
    assert_eq!(s.frozen_frame(), Some(2));
}

#[test]
fn interact_keeps_ink_and_waits_for_gameplay_buttons() {
    let mut s = session_with("toggle");
    s.handle(Event::MasterToggle, 0);
    ack(&mut s);
    draw_line(&mut s, (0.0, 0.0), (20.0, 20.0));
    assert_eq!(s.handle(Event::InteractDown, 10), vec![Effect::ReturnFocus]);
    assert_eq!(s.overlay_state(), OverlayState::ClickThrough);
    assert!(s.tip(10).is_none());
    assert_eq!(s.scene().marks().len(), 1);
    s.handle(Event::ButtonsReleased, 11);
    assert!(s.pending_ticket().is_none());
    s.handle(Event::InteractUp, 12);
    assert!(s.pending_ticket().is_none());
    s.handle(Event::ButtonsReleased, 13);
    ack(&mut s);
    assert!(s.is_pointing());
    assert_eq!(s.scene().marks().len(), 1);
}

#[test]
fn interact_does_not_interrupt_drag_or_hold_to_draw() {
    let mut s = session_with("toggle");
    s.handle(Event::MasterToggle, 0);
    ack(&mut s);
    s.handle(ptr(PointerKind::Down, 10.0, 10.0), 1);
    assert!(s.handle(Event::InteractDown, 2).is_empty());
    assert!(s.is_pointing());
    let mut held = session_with("hold");
    held.handle(Event::MasterToggle, 0);
    held.handle(Event::HoldDown, 1);
    ack(&mut held);
    held.handle(Event::InteractDown, 2);
    assert!(held.is_pointing());
}

#[test]
fn interact_release_after_off_cannot_reacquire() {
    let mut s = session_with("toggle");
    s.handle(Event::MasterToggle, 0);
    ack(&mut s);
    s.handle(Event::InteractDown, 1);
    s.handle(Event::MasterOff, 2);
    s.handle(Event::InteractUp, 3);
    s.handle(Event::ButtonsReleased, 4);
    assert_eq!(s.mode(), Mode::Off);
    assert!(s.pending_ticket().is_none());
}

#[test]
fn interact_works_from_every_idle_live_tool_and_resumes_that_tool() {
    let reg = registry();
    for command in ["point", "text", "pin", "spotlight", "lens"] {
        let mut s = session_with("toggle");
        s.handle(Event::MasterToggle, 0);
        ack(&mut s);
        let def = reg.command_by_cli(command).unwrap();
        reg.dispatch(def.id, &mut s, 1).unwrap();
        let tool = s.active_tool();
        s.handle(Event::InteractDown, 2);
        assert_eq!(s.mode(), Mode::Passthrough, "Alt did not release {tool}");
        s.handle(Event::InteractUp, 3);
        s.handle(Event::ButtonsReleased, 4);
        ack(&mut s);
        assert!(s.is_pointing());
        assert_eq!(s.active_tool(), tool);
    }
}

#[test]
fn interact_cancels_an_in_flight_input_acquisition() {
    let mut s = session_with("toggle");
    s.handle(Event::MasterToggle, 0);
    let stale = s.pending_ticket().unwrap();
    s.handle(Event::InteractDown, 1);
    assert_eq!(s.overlay_state(), OverlayState::Hidden);
    s.handle(Event::InputAcquired(stale), 2);
    assert!(
        !s.is_pointing(),
        "late acknowledgement must not steal input back from Alt"
    );
    s.handle(Event::InteractUp, 3);
    s.handle(Event::ButtonsReleased, 4);
    ack(&mut s);
    assert!(s.is_pointing());
}

#[test]
fn text_undo_and_style_are_independent_of_annotation_history() {
    let reg = registry();
    let mut s = session_with("toggle");
    let mut settings = s.settings().clone();
    reg.set(
        &mut settings,
        crate::features::text::COLOR,
        Value::Color([255, 90, 100, 255]),
    )
    .unwrap();
    reg.set(
        &mut settings,
        crate::features::text::BACKING,
        Value::Bool(false),
    )
    .unwrap();
    s.handle(Event::SettingsChanged(settings), 0);
    s.handle(Event::MasterToggle, 0);
    ack(&mut s);
    draw_line(&mut s, (0.0, 0.0), (30.0, 30.0));
    reg.dispatch(crate::features::text::SELECT, &mut s, 1)
        .unwrap();
    s.handle(ptr(PointerKind::Down, 80.0, 80.0), 2);
    s.handle(Event::Text(TextInput::Changed("Hello".into())), 3);
    s.handle(Event::Text(TextInput::Changed("Hello world".into())), 4);
    s.handle(Event::Text(TextInput::Undo), 5);
    assert_eq!(s.text_draft().unwrap().text, "Hello");
    assert_eq!(s.scene().marks().len(), 1);
    s.handle(Event::Text(TextInput::Submit), 6);
    let Mark::Label(label) = &s.scene().marks()[1] else {
        panic!("label")
    };
    assert_eq!(label.text, "Hello");
    assert_eq!(label.color, Rgba([255, 90, 100, 255]));
    assert!(!label.backing);
}

#[test]
fn reduced_motion_snaps_grip_and_expires_ink_without_fading() {
    let reg = registry();
    let mut s = session_with("toggle");
    let mut settings = s.settings().clone();
    reg.set(&mut settings, system::MOTION, Value::Choice("reduced"))
        .unwrap();
    reg.set(&mut settings, point::PRESENTATION, Value::Choice("stretch"))
        .unwrap();
    reg.set(&mut settings, system::INK_FADE, Value::Choice("3s"))
        .unwrap();
    s.handle(Event::SettingsChanged(settings), 0);
    s.handle(Event::MasterToggle, 0);
    ack(&mut s);
    draw_line(&mut s, (0.0, 0.0), (100.0, 100.0));
    let (_, Tip::Paw { grip_x, at, .. }) = s.tip(1).unwrap() else {
        panic!("paw")
    };
    assert_eq!(grip_x, at.x);
    assert!(!s.animating(1));
    assert_eq!(s.mark_alpha(&s.scene().marks()[0], 3001), 0.0);
    s.handle(Event::Tick, 3001);
    assert!(s.scene().is_empty());
}

#[test]
fn stamp_keeps_the_rendered_release_angle() {
    let reg = registry();
    let mut settings = reg.defaults();
    reg.set(&mut settings, point::TAP_STAMP, Value::Choice("cat"))
        .unwrap();
    let mut s = Session::new(point::TOOL, point::new_state, settings.clone());
    s.handle(Event::MasterToggle, 0);
    ack(&mut s);
    s.handle(ptr(PointerKind::Down, 30.0, 40.0), 10);
    let Event::Pointer(mut release) = ptr(PointerKind::Up, 30.0, 40.0) else {
        unreachable!()
    };
    release.paw_angle = Some(-0.7);
    s.handle(Event::Pointer(release), 20);
    reg.set(&mut settings, point::ANGLE, Value::Float(45.0))
        .unwrap();
    s.handle(Event::SettingsChanged(settings), 30);
    s.handle(ptr(PointerKind::Moved, 100.0, 200.0), 40);
    let Mark::PawPrint(print) = &s.scene().marks()[0] else {
        panic!("expected stamp")
    };
    assert_eq!(print.angle, -0.7);
}

#[test]
fn paw_stamps_commit_once_and_share_ink_history_and_fade() {
    for (id, kind) in [("cat", PawPrintKind::Cat), ("dog", PawPrintKind::Dog)] {
        let reg = registry();
        let mut settings = reg.defaults();
        reg.set(&mut settings, point::TAP_STAMP, Value::Choice(id))
            .unwrap();
        reg.set(&mut settings, system::INK_FADE, Value::Choice("3s"))
            .unwrap();
        let mut s = Session::new(point::TOOL, point::new_state, settings);
        s.handle(Event::MasterToggle, 0);
        ack(&mut s);
        s.handle(ptr(PointerKind::Down, 30.0, 40.0), 10);
        let fx = s.handle(ptr(PointerKind::Up, 31.0, 40.0), 20);
        assert_eq!(fx, vec![Effect::PlayTap]);
        let Mark::PawPrint(print) = &s.scene().marks()[0] else {
            panic!("expected stamp")
        };
        assert_eq!(print.at, Pt::new(30.0, 40.0));
        assert_eq!(print.kind, kind);
        assert_eq!(print.color, Rgba::CYAN);
        assert_eq!(print.born, 20);
        assert_eq!(print.output, O);
        s.handle(ptr(PointerKind::Up, 31.0, 40.0), 30);
        assert_eq!(s.scene().marks().len(), 1, "duplicate release cannot stamp");
        draw_line(&mut s, (60.0, 40.0), (100.0, 40.0));
        assert_eq!(s.scene().marks().len(), 2);
        assert!(matches!(s.scene().marks()[1], Mark::Stroke(_)));
        s.handle(Event::Undo, 30);
        assert!(matches!(s.scene().marks()[0], Mark::PawPrint(_)));
        s.handle(Event::Undo, 30);
        assert!(s.scene().is_empty());
        tap(&mut s, 30.0, 40.0);
        s.handle(Event::Tick, 4000);
        assert!(s.scene().is_empty(), "stamp expires with ink");
        tap(&mut s, 30.0, 40.0);
        s.handle(Event::Clear, 4010);
        assert!(s.scene().is_empty());
    }
}

#[test]
fn stamps_ignore_laser_cancelled_press_and_release_on_another_output() {
    let reg = registry();
    let mut settings = reg.defaults();
    reg.set(&mut settings, point::TAP_STAMP, Value::Choice("cat"))
        .unwrap();
    let mut s = Session::new(point::TOOL, point::new_state, settings.clone());
    s.handle(Event::MasterToggle, 0);
    ack(&mut s);
    s.handle(ptr(PointerKind::Down, 10.0, 10.0), 0);
    s.handle(ptr(PointerKind::Left, 10.0, 10.0), 1);
    s.handle(ptr(PointerKind::Up, 10.0, 10.0), 2);
    assert!(s.scene().is_empty());
    s.handle(ptr(PointerKind::Down, 10.0, 10.0), 3);
    s.handle(
        Event::Pointer(PointerEvent {
            kind: PointerKind::Up,
            output: OutputId(1),
            pos: Pt::new(10.0, 10.0),
            at: 4,
            paw_angle: None,
        }),
        4,
    );
    assert!(s.scene().is_empty());
    reg.set(&mut settings, point::STYLE, Value::Choice("laser"))
        .unwrap();
    s.handle(Event::SettingsChanged(settings), 5);
    tap(&mut s, 10.0, 10.0);
    assert!(s.scene().is_empty());
}
