use super::*;
use crate::mascot::rig;

const GROUND: f32 = 807.0;

fn yohta(height: f32) -> Layout {
    layout(
        height,
        rig::HEIGHT,
        Size::new(rig::WIDTH, rig::HEIGHT),
        Point::new(rig::WIDTH * 0.5, GROUND),
        5,
        0.0,
    )
}
const WORK: (i32, i32, i32, i32) = (0, 0, 1920, 1040);

#[test]
fn layout_fits_character_and_menu_with_feet_on_the_anchor() {
    for height in [96.0, 160.0, 320.0] {
        let l = yohta(height);
        assert!((l.character.height - height).abs() < 0.01);
        for side in [Side::Left, Side::Right] {
            let m = l.menu(side);
            assert!(m.x >= 0.0 && m.y >= 0.0);
            assert!(m.x + m.width <= l.window.width + 0.01);
            assert!(m.y + m.height <= l.window.height + 0.01);
            // The menu never covers the character.
            assert!(m.x + m.width <= l.character.x || m.x >= l.character.x + l.character.width);
        }
        assert!(l.character.y + l.character.height <= l.window.height + 0.01);
        assert!(
            (l.menu(Side::Right).y + l.menu(Side::Right).height - l.anchor.y).abs() < 0.01
                || l.menu(Side::Right).y == 0.0
        );
    }
}

#[test]
fn clamping_keeps_the_character_on_the_work_area() {
    let l = yohta(160.0);
    for k in [1.0, 1.5, 2.0] {
        for wanted in [(-5000.0, -5000.0), (99999.0, 99999.0), (960.0, 500.0)] {
            let (x, y) = l.clamp(wanted, k, WORK);
            let (ox, oy) = l.origin((x, y), k);
            let left = ox as f32 + l.character.x * k;
            let right = ox as f32 + (l.character.x + l.character.width) * k;
            assert!(left >= -1.0 && right <= 1921.0, "{k} {wanted:?}");
            assert!(oy >= -1 && y <= 1040.0);
        }
        assert_eq!(l.clamp((960.0, 500.0), k, WORK), (960.0, 500.0));
    }
}

#[test]
fn origin_round_trips_the_anchor() {
    let l = yohta(200.0);
    let (ox, oy) = l.origin((1000.0, 900.0), 1.25);
    assert!((ox as f32 + l.anchor.x * 1.25 - 1000.0).abs() <= 1.0);
    assert!((oy as f32 + l.anchor.y * 1.25 - 900.0).abs() <= 1.0);
}

#[test]
fn default_spot_is_bottom_right_on_the_taskbar() {
    let l = yohta(160.0);
    let (x, y) = l.default_anchor(1.0, WORK);
    assert_eq!(y, 1040.0);
    assert!(x > 1700.0 && x < 1920.0);
}

#[test]
fn menu_flips_left_near_the_right_edge() {
    let l = yohta(160.0);
    assert_eq!(l.menu_side(400.0, 1.0, 1920), Side::Right);
    assert_eq!(l.menu_side(1850.0, 1.0, 1920), Side::Left);
}

#[test]
fn chosen_menu_side_holds_unless_it_leaves_the_screen() {
    let l = yohta(160.0);
    let work = (0, 1920);
    assert_eq!(l.chosen_side(MenuSide::Left, 960.0, 1.0, work), Side::Left);
    assert_eq!(
        l.chosen_side(MenuSide::Right, 960.0, 1.0, work),
        Side::Right
    );
    assert_eq!(l.chosen_side(MenuSide::Left, 60.0, 1.0, work), Side::Right);
    assert_eq!(
        l.chosen_side(MenuSide::Right, 1850.0, 1.0, work),
        Side::Left
    );
    assert_eq!(l.chosen_side(MenuSide::Auto, 1850.0, 1.0, work), Side::Left);
}

#[test]
fn menu_lines_up_with_feet_middle_or_head() {
    let visible = (100.0, GROUND);
    let base = layout(
        160.0,
        GROUND - 100.0,
        Size::new(rig::WIDTH, rig::HEIGHT),
        Point::new(rig::WIDTH * 0.5, GROUND),
        3,
        104.0,
    );
    let at = |a| base.align_menu(a, visible).menu(Side::Right);
    let feet = at(MenuAlign::Feet);
    assert!((feet.y + feet.height - base.anchor.y).abs() < 0.01);
    let head = at(MenuAlign::Head);
    assert!((head.y - (base.character.y + 100.0 * base.scale)).abs() < 0.01);
    let middle = at(MenuAlign::Middle);
    assert!(head.y < middle.y && middle.y < feet.y);
    for a in MenuAlign::ALL {
        let m = at(a);
        assert!(
            m.y >= 0.0 && m.y + m.height <= base.window.height + 0.01,
            "{a:?}"
        );
    }
}

#[test]
fn bubble_follows_its_anchor_and_side() {
    use crate::mascot::lines::Bubble;
    let l = yohta(200.0);
    let top = 120.0;
    // By default the tail aims at the top of the visible art, not the
    // canvas, so padding above the head doesn't leave it floating.
    let SpeechPlace::Above { at, left, width } = speech_place(&l, Bubble::default(), top, None)
    else {
        panic!("above by default");
    };
    assert!((at.y - (l.character.y + top * l.scale)).abs() < 0.01);
    assert!(left >= 0.0 && left + width <= l.window.width + 0.01);
    let pinned = Bubble {
        anchor: Some((0.25, 0.5)),
        ..Bubble::default()
    };
    let SpeechPlace::Above { at, .. } = speech_place(&l, pinned, top, None) else {
        panic!("above");
    };
    assert!((at.x - (l.character.x + l.character.width * 0.25)).abs() < 0.01);
    let right = Bubble {
        side: BubbleSide::Right,
        ..pinned
    };
    match speech_place(&l, right, top, None) {
        SpeechPlace::Beside {
            edge,
            side,
            left,
            width,
            ..
        } => {
            assert_eq!(side, Side::Right);
            assert!(edge > at.x && left + width <= l.window.width + 0.01);
        }
        other => panic!("beside: {other:?}"),
    }
    assert!(
        matches!(
            speech_place(&l, right, top, Some(Side::Right)),
            SpeechPlace::Above { .. }
        ),
        "an open menu on that side moves the bubble above"
    );
    let left_side = Bubble {
        side: BubbleSide::Left,
        anchor: Some((0.0, 0.5)),
        ..Bubble::default()
    };
    match speech_place(&l, left_side, top, None) {
        SpeechPlace::Beside { left, edge, .. } => assert!(left >= 0.0 && edge <= at.x),
        other => panic!("beside: {other:?}"),
    }
}

#[test]
fn gaze_points_at_the_cursor_and_saturates() {
    let eyes = Point::new(100.0, 100.0);
    assert_eq!(gaze_toward(eyes, eyes, 50.0), Vector::ZERO);
    let near = gaze_toward(Point::new(125.0, 100.0), eyes, 50.0);
    assert!((near.x - 0.5).abs() < 1e-4 && near.y.abs() < 1e-4);
    let far = gaze_toward(Point::new(100.0, -900.0), eyes, 50.0);
    assert!((far.y + 1.0).abs() < 1e-4 && far.x.abs() < 1e-4);
}

#[test]
fn layout_follows_a_custom_canvas_and_off_centre_feet() {
    // A wide image whose feet sit left of centre.
    let l = layout(
        100.0,
        200.0,
        Size::new(400.0, 200.0),
        Point::new(100.0, 190.0),
        3,
        0.0,
    );
    assert!((l.character.width - 200.0).abs() < 0.01);
    assert!((l.anchor.x - (l.character.x + 50.0)).abs() < 0.01);
    assert!((l.anchor.y - (l.character.y + 95.0)).abs() < 0.01);
}

#[test]
fn the_bubble_band_sits_above_the_character() {
    let l = layout(
        100.0,
        100.0,
        Size::new(100.0, 100.0),
        Point::new(50.0, 95.0),
        3,
        BUBBLE_BAND,
    );
    assert!(l.character.y >= BUBBLE_BAND - 0.01);
    assert!((l.window.height - (l.character.y + 100.0)).abs() < 1.0);
}

#[test]
fn size_is_visible_height_so_padding_does_not_shrink_the_character() {
    // The same 100-pixel figure on a tight and a heavily padded canvas.
    let tight = layout(
        200.0,
        100.0,
        Size::new(60.0, 110.0),
        Point::new(30.0, 105.0),
        3,
        0.0,
    );
    let padded = layout(
        200.0,
        100.0,
        Size::new(600.0, 1100.0),
        Point::new(300.0, 900.0),
        3,
        0.0,
    );
    assert_eq!(tight.scale, padded.scale);
    assert!((tight.scale * 100.0 - 200.0).abs() < 1e-3);
    // Yohta: the new range shows him exactly as the old one did.
    let r = crate::mascot::rig::Rig::load();
    let ratio = f64::from(r.visible_height() / r.size().height);
    use crate::features::mascot as m;
    for (old, new) in [
        (m::MIN_SIZE, m::MIN_HEIGHT),
        (160.0, m::DEFAULT_HEIGHT),
        (m::MAX_SIZE, m::MAX_HEIGHT),
    ] {
        assert!((old * ratio - new).abs() < 1.0, "{old} × {ratio} vs {new}");
    }
}

#[test]
fn small_movements_stay_clicks() {
    assert!(!is_drag((3, 3), 1.0));
    assert!(is_drag((6, 0), 1.0));
    // Five logical pixels are ten physical ones at 200 %.
    assert!(!is_drag((8, 0), 2.0));
    assert!(is_drag((11, 0), 2.0));
}

/// Contact error: physical distance between the cursor and where the
/// grabbed window point ends up after a carry step.
fn contact_error(cursor: (i32, i32), grab: Point, k: f32) -> f32 {
    let o = carried_origin(cursor, grab, k);
    let held = (o.0 as f32 + grab.x * k, o.1 as f32 + grab.y * k);
    (held.0 - cursor.0 as f32).hypot(held.1 - cursor.1 as f32)
}

#[test]
fn the_grabbed_point_stays_under_the_cursor() {
    // Centre, head, tail and edge grabs, in window logical pixels.
    let grabs = [
        (310.0, 450.0),
        (250.0, 200.0),
        (420.0, 700.0),
        (213.7, 801.3),
    ];
    // A path across a negative-origin monitor and off the right edge.
    let path = [
        (-1500, 300),
        (-3, -40),
        (0, 0),
        (960, 540),
        (1919, 1079),
        (2600, 1500),
    ];
    for k in [1.0, 1.25, 1.5, 2.0] {
        for g in grabs {
            for c in path {
                let e = contact_error(c, Point::new(g.0, g.1), k);
                assert!(e <= 1.0, "{k} {g:?} {c:?}: {e}");
            }
        }
    }
    // Crossing from 100 % to 200 % mid-drag: the window's own size
    // gives the new scale, and the same logical grab point holds.
    let g = Point::new(250.0, 200.0);
    for k in [1.0, 2.0, 1.5] {
        assert!(contact_error((1900, 500), g, k) <= 1.0);
    }
}

#[test]
fn carrying_is_not_clamped_but_release_recovers() {
    let l = yohta(160.0);
    let k = 1.0;
    let width = l.character.width * k;
    let before = (l.anchor.x - l.character.x) * k;
    // A third past the left edge, so most of it shows: kept.
    let partly = (before - width * 0.3, 600.0);
    assert_eq!(l.recover(partly, k, WORK), partly);
    // Far off the right: brought back until half its width shows.
    let (x, _) = l.recover((99999.0, 600.0), k, WORK);
    let right = x + (l.character.x + l.character.width - l.anchor.x) * k;
    assert!((right - (1920.0 + width * 0.5)).abs() < 0.01, "{right}");
    // Above the top or below the taskbar: the same limits as clamp.
    for wanted in [(960.0, -5000.0), (960.0, 99999.0)] {
        assert_eq!(l.recover(wanted, k, WORK).1, l.clamp(wanted, k, WORK).1);
    }
    // A spot on screen is left alone.
    assert_eq!(l.recover((960.0, 500.0), k, WORK), (960.0, 500.0));
}

#[test]
fn hit_testing_follows_the_drawn_lean() {
    let rig = Rig::load();
    let l = yohta(320.0);
    let rest = Pose::default();
    let leaning = Pose {
        lean: 0.15,
        squash: 1.0,
        ..Pose::default()
    };
    // A face point, drawn where the lean puts it.
    let face = Point::new(232.0, 360.0);
    let to_window = |pose: &Pose| {
        let d = super::super::motion::placed(pose, rig.anchor(), rig.size().height, face);
        Point::new(l.character.x + d.x * l.scale, l.character.y + d.y * l.scale)
    };
    assert!(over(&l, &rig, &rest, to_window(&rest)));
    assert!(over(&l, &rig, &leaning, to_window(&leaning)));
    // A corner of the window is never the character.
    assert!(!over(&l, &rig, &leaning, Point::new(1.0, 1.0)));
}

#[test]
fn a_carried_character_leans_around_the_grab_and_trails_unprotected_parts() {
    let (mut d, _) = Desktop::open("yohta", Rig::load(), 146.0, (500.0, 500.0), 5, false);
    let o = Options {
        eyes: false,
        idle: true,
        reduced: false,
        lines: false,
        quiet: true,
        idle_every: (60.0, 300.0),
        music: 0.0,
        beat: None,
    };
    d.step_motion(&o, 1.0);
    let l = d.layout;
    // Grab the head, about where the face is.
    let face = Point::new(232.0, 360.0);
    let grab = Point::new(
        l.character.x + face.x * l.scale,
        l.character.y + face.y * l.scale,
    );
    let (pivot, protected) = d.grab_chain(grab);
    assert!(protected.iter().any(|p| *p), "the head is protected");
    assert!(!protected.iter().all(|p| *p), "the tail may trail");
    let held = d.pose.clone();
    d.press = Some(Press {
        cursor: (0, 0),
        grab,
        pose: held.clone(),
        dragging: true,
        pivot,
        protected: protected.clone(),
        lean: 0.0,
        swing: 0.0,
        last_vx: 0.0,
    });
    let local = super::super::motion::unplaced(&held, d.rig.anchor(), d.rig.size().height, pivot);
    let mut leaned = 0.0f32;
    for _ in 0..40 {
        d.now += 0.016;
        d.last_anchor.0 -= 12.0; // carried to the right
        d.step_motion(&o, 1.0);
        let pose = &d.pose;
        leaned = leaned.max(pose.carry.map_or(0.0, |(_, a)| a.abs()));
        // The grabbed point is where it was, within a hundredth of a
        // rig pixel, and the protected chain hasn't turned.
        let drawn = super::super::motion::placed(pose, d.rig.anchor(), d.rig.size().height, local);
        assert!(drawn.distance(pivot) < 0.01, "{drawn:?} vs {pivot:?}");
        for (i, p) in protected.iter().enumerate() {
            if *p {
                assert_eq!(pose.angles[i], held.angles[i]);
            }
        }
    }
    assert!(leaned > 0.01, "it leans while carried");
    let trailing = d.pose.carry.map_or(0.0, |(_, a)| a);
    assert!(
        trailing > 0.0,
        "carried right by the head, the feet swing back to the left: {trailing}"
    );
    // The lean never pushes the character out of its window.
    assert!(leaned <= CARRY_LEAN + 1e-4);
    // Release eases the lean out around the same point.
    d.release(&o);
    d.now += 0.001;
    d.step_motion(&o, 1.0);
    assert!(
        d.pose
            .carry
            .is_some_and(|(at, a)| at == pivot && a.abs() <= leaned + 1e-4)
    );
}

#[test]
fn a_released_character_falls_once_and_a_pickup_catches_it() {
    let (mut d, _) = Desktop::open("yohta", Rig::load(), 146.0, (500.0, 300.0), 5, false);
    let o = Options {
        eyes: false,
        idle: false,
        reduced: false,
        lines: false,
        quiet: true,
        idle_every: (60.0, 300.0),
        music: 0.0,
        beat: None,
    };
    d.falling = Some(Fall {
        speed: 0.0,
        land: 700.0,
    });
    let mut saves = Vec::new();
    for _ in 0..200 {
        d.now += 0.016;
        if let Some(spot) = d.step_fall(1.0) {
            saves.push(spot);
        }
        d.step_motion(&o, 1.0);
    }
    assert_eq!(saves, [(500.0, 700.0)], "lands once, exactly on the edge");
    assert!(d.take_events().is_empty(), "landing is not a Drop");
    // Picking it up mid-fall stops the fall at once.
    d.falling = Some(Fall {
        speed: 0.0,
        land: 900.0,
    });
    let _ = d.update(Msg::Pressed, &o);
    assert!(d.falling.is_none());
    // Off unless the character asks for it.
    d.set_fall(false);
    assert!(!d.fall_on_release);
}

#[test]
fn a_held_pose_swings_from_its_scruff_when_shaken_and_settles() {
    let (mut d, _) = Desktop::open("yohta", Rig::load(), 146.0, (500.0, 500.0), 5, false);
    let o = Options {
        eyes: false,
        idle: false,
        reduced: false,
        lines: false,
        quiet: true,
        idle_every: (60.0, 300.0),
        music: 0.0,
        beat: None,
    };
    // A tall figure hanging down the middle of the stage.
    let size = d.rig.size();
    let mut img = image::RgbaImage::new(size.width as u32, size.height as u32);
    for y in 100..700 {
        for x in 280..340 {
            img.put_pixel(x, y, image::Rgba([90, 120, 200, 255]));
        }
    }
    d.set_pictures(Pictures {
        held: Some(super::super::rig::Picture::new(&img, [0.0, 0.0, 1.0])),
        ..Pictures::default()
    });
    d.step_motion(&o, 1.0);
    let l = d.layout;
    let pivot = Point::new(size.width / 2.0, d.rig.visible_rows().0);
    let grab = Point::new(
        l.character.x + pivot.x * l.scale,
        l.character.y + pivot.y * l.scale,
    );
    d.press = Some(Press {
        cursor: (0, 0),
        grab,
        pose: d.pose.clone(),
        dragging: true,
        pivot,
        protected: vec![true; 2],
        lean: 0.0,
        swing: 0.0,
        last_vx: 0.0,
    });
    let (mut most, mut least) = (0.0f32, 0.0f32);
    // Shake side to side, then hold still.
    for n in 0..120 {
        d.now += 0.016;
        let dx = if n < 60 {
            if (n / 8) % 2 == 0 { 14.0 } else { -14.0 }
        } else {
            0.0
        };
        d.last_anchor.0 = d.anchor.0 - dx;
        d.step_motion(&o, 1.0);
        let a = d.pose.carry.map_or(0.0, |(_, a)| a);
        most = most.max(a);
        least = least.min(a);
        assert!(a.abs() <= HANG_SWING + 1e-4);
    }
    assert!(
        most > 0.05 && least < -0.05,
        "it swings both ways: {least}..{most}"
    );
    for _ in 0..300 {
        d.now += 0.016;
        d.last_anchor = d.anchor;
        d.step_motion(&o, 1.0);
    }
    assert!(
        d.pose.carry.map_or(0.0, |(_, a)| a).abs() < 0.02,
        "and settles hanging straight"
    );
}

#[test]
fn gestures_emit_each_event_once_and_cancellation_none() {
    let (mut d, _) = Desktop::open("yohta", Rig::load(), 160.0, (500.0, 500.0), 5, false);
    let o = Options {
        eyes: false,
        idle: false,
        reduced: false,
        lines: false,
        quiet: true,
        idle_every: (60.0, 300.0),
        music: 0.0,
        beat: None,
    };
    let press = |dragging| Press {
        cursor: (0, 0),
        grab: Point::ORIGIN,
        pose: Pose::default(),
        dragging,
        pivot: Point::ORIGIN,
        protected: Vec::new(),
        lean: 0.0,
        swing: 0.0,
        last_vx: 0.0,
    };
    // A short press is one Click.
    d.press = Some(press(false));
    d.release(&o);
    assert_eq!(d.take_events(), [Event::Click]);
    assert!(d.take_events().is_empty(), "taken once");
    // A drag's release is one Drop and no Click (Pickup comes from the
    // poll that crosses the threshold).
    d.press = Some(press(true));
    d.release(&o);
    assert_eq!(d.take_events(), [Event::Drop]);
    // Releasing with no press, or hiding mid-drag, invents nothing.
    assert_eq!(d.release(&o), None);
    d.press = Some(press(true));
    let _ = d.set_hidden(true);
    assert_eq!(d.release(&o), None);
    assert!(d.take_events().is_empty());
}

#[test]
fn a_held_press_keeps_its_pose_and_eases_out_on_release() {
    let (mut d, _) = Desktop::open("yohta", Rig::load(), 160.0, (500.0, 500.0), 5, false);
    let o = Options {
        eyes: false,
        idle: true,
        reduced: false,
        lines: false,
        quiet: true,
        idle_every: (60.0, 300.0),
        music: 1.0,
        beat: None,
    };
    for i in 1..=10 {
        d.now = i as f32 * 0.033;
        d.step_motion(&o, 1.0);
    }
    let held = d.pose.clone();
    // Reduced motion holds the whole pose, pressed or carried.
    let reduced = Options { reduced: true, ..o };
    d.press = Some(Press {
        cursor: (0, 0),
        grab: Point::ORIGIN,
        pose: held.clone(),
        dragging: false,
        pivot: Point::ORIGIN,
        protected: vec![true; held.angles.len()],
        lean: 0.0,
        swing: 0.0,
        last_vx: 0.0,
    });
    for (dragging, o) in [(false, o), (true, reduced)] {
        d.press.as_mut().unwrap().dragging = dragging;
        for i in 0..30 {
            d.now += 0.033;
            d.last_anchor.0 -= 40.0 * i as f32;
            d.step_motion(&o, 1.0);
            // No squash, lean, bounce or part motion while held.
            assert_eq!(
                (
                    d.pose.angles.clone(),
                    d.pose.lean,
                    d.pose.squash,
                    d.pose.bounce
                ),
                (held.angles.clone(), held.lean, held.squash, held.bounce)
            );
            assert!(d.pose.carry.is_none_or(|(_, a)| a == 0.0));
        }
        let face = if dragging {
            Expression::Dragging
        } else {
            Expression::Pressed
        };
        assert_eq!(d.pose.expression, face);
    }
    // Release: the first frame starts from the held geometry.
    d.release(&o);
    d.now += 0.001;
    d.step_motion(&o, 1.0);
    assert!((d.pose.lean - held.lean).abs() < 0.01 && (d.pose.bounce - held.bounce).abs() < 0.01);
    for _ in 0..20 {
        d.now += 0.033;
        d.step_motion(&o, 1.0);
    }
    assert!(d.released.is_none(), "back on live motion");
}
