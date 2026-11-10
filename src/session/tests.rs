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
    })
}

/// Completes the pending acquisition as the platform would.
fn ack(s: &mut Session) {
    let t = s.pending_ticket().expect("pending acquisition");
    s.handle(Event::InputAcquired(t), 0);
}

fn draw_line(s: &mut Session, from: (f32, f32), to: (f32, f32)) -> Vec<Effect> {
    let mut fx = s.handle(ptr(PointerKind::Down, from.0, from.1), 0);
    fx.extend(s.handle(ptr(PointerKind::Moved, (from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0), 0));
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
    let Mark::Stroke(stroke) = &s.scene().marks()[0];
    assert_eq!(stroke.points[0], Pt::new(10.0, 10.0));
    assert_eq!(*stroke.points.last().unwrap(), Pt::new(60.0, 10.0));
    assert_eq!(stroke.width, 4.0);
    assert_eq!(stroke.color, Rgba::CYAN);
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
        }),
        0,
    );
    assert_eq!(s.scene().marks().len(), 1);
    assert_eq!(s.draft().unwrap().output(), OutputId(1));
}
