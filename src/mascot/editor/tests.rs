use super::*;
use crate::mascot::pack::{OnDuplicate, test_install_folder};

#[test]
fn saving_many_padded_layers_compacts_sources_and_preserves_placement() {
    let dir = std::env::temp_dir().join(format!("tinypaw-padded-save-{}", std::process::id()));
    let (root, character) = test_install_folder(&dir, OnDuplicate::Ask);
    let mut editor = Editor::open_in(&root, &character).unwrap();
    editor.canvas = (1024, 1024);
    let mut image = RgbaImage::new(1024, 1024);
    for y in 200..240 {
        for x in 300..330 {
            image.put_pixel(x, y, image::Rgba([120, 80, 220, 255]));
        }
    }
    let mut layer = Layer::new(image);
    layer.placement.angle = 90.0;
    let expected = placement::bake(&layer.image, layer.placement, editor.canvas);
    editor.body = layer.clone();
    editor.parts = [PartType::Head, PartType::Tail]
        .into_iter()
        .map(|kind| EditPart {
            name: kind.to_string(),
            kind,
            layer: layer.clone(),
            pivot: (0.5, 0.5),
            on_head: false,
            behind: kind.behind_by_default(),
            motion: MotionSpec::default_for(kind),
            faces: if kind == PartType::Head {
                [None, Some(layer.clone()), Some(layer.clone()), None]
            } else {
                [None, None, None, None]
            },
        })
        .collect();
    editor.eyes = vec![
        EditEye {
            white: Some(layer.clone()),
            pupil: Some(layer.clone()),
            mask: Some(layer.clone()),
            blink: Some(layer),
            travel: DEFAULT_TRAVEL,
        };
        2
    ];
    // Thirteen canvas layers require 156 MiB with untrimmed editable copies.
    let saved = editor.save_into(&root).unwrap();
    let manifest = read_manifest(saved.folder.as_ref().unwrap()).unwrap();
    assert_eq!(manifest.editor.len(), 13);
    assert!(manifest.editor.iter().all(|record| record.size == (30, 40)));
    let reopened = Editor::open_in(&root, &saved).unwrap();
    let rebaked = placement::bake(
        &reopened.body.image,
        reopened.body.placement,
        reopened.canvas,
    );
    for (actual, expected) in rebaked.pixels().zip(expected.pixels()) {
        assert!(actual[3].abs_diff(expected[3]) <= 1);
        for channel in 0..3 {
            let a = u16::from(actual[channel]) * u16::from(actual[3]) / 255;
            let b = u16::from(expected[channel]) * u16::from(expected[3]) / 255;
            assert!(a.abs_diff(b) <= 1);
        }
    }
    let saved_again = reopened.save_into(&root).unwrap();
    let saved_pixels = read_png(
        saved_again.folder.as_ref().unwrap(),
        "sit/body.png",
        editor.canvas,
    )
    .unwrap();
    assert!(
        saved_pixels == rebaked,
        "repeated saves preserve the compacted source"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn cropped_placement_round_trip_keeps_sources_and_baked_pixels() {
    let dir = std::env::temp_dir().join(format!("tinypaw-crop-roundtrip-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (root, c) = test_install_folder(&dir, OnDuplicate::Ask);
    let mut e = Editor::open_in(&root, &c).unwrap();
    let path = dir.join("head.png");
    let source = RgbaImage::from_pixel(20, 30, image::Rgba([120, 40, 220, 255]));
    source.save(&path).unwrap();
    e.set_image(Target::NewPart, &path).unwrap();
    assert_eq!(e.parts[0].layer.image.dimensions(), (20, 30));
    assert_eq!(e.parts[0].layer.placement.x, 40.0);
    e.checkpoint();
    let placement = Placement {
        x: 23.0,
        y: 18.0,
        scale: 1.3,
        angle: 25.0,
    };
    e.place(placement);
    let joint = e.parts[0].pivot;
    let expected = placement::bake(&source, placement, e.canvas);
    e.undo();
    assert_eq!(e.parts[0].layer.placement.angle, 0.0);
    e.redo();
    assert_eq!(e.parts[0].pivot, joint);
    let saved = e.save_into(&root).unwrap();
    std::fs::remove_file(path).unwrap();
    let reopened = Editor::open_in(&root, &saved).unwrap();
    assert_eq!(reopened.parts[0].layer.image.as_ref(), &source);
    assert_eq!(reopened.parts[0].layer.placement, placement);
    let m = read_manifest(saved.folder.as_ref().unwrap()).unwrap();
    assert_eq!(
        read_png(
            saved.folder.as_ref().unwrap(),
            &m.default_pose().parts[0].image,
            m.canvas
        )
        .unwrap(),
        expected
    );
    let export = pack::export(&saved, &dir.join("export")).unwrap();
    let exported = Editor::read(&export, saved.id).unwrap();
    assert_eq!(exported.parts[0].layer.placement, placement);
    assert_eq!(exported.parts[0].layer.image.as_ref(), &source);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn head_arrangement_moves_children_and_joints_while_pivot_mode_does_not() {
    let dir = std::env::temp_dir().join(format!("tinypaw-crop-joints-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (root, c) = test_install_folder(&dir, OnDuplicate::Ask);
    let mut e = Editor::open_in(&root, &c).unwrap();
    let head = dir.join("head.png");
    let eye = dir.join("eye.png");
    RgbaImage::from_pixel(30, 30, image::Rgba([80, 100, 120, 255]))
        .save(&head)
        .unwrap();
    RgbaImage::from_pixel(6, 4, image::Rgba([20, 30, 40, 255]))
        .save(&eye)
        .unwrap();
    e.set_image(Target::NewPart, &head).unwrap();
    e.set_image(Target::NewPart, &eye).unwrap();
    assert!(e.parts[1].on_head);
    let old = e.parts[1].layer.placement;
    let joint = e.parts[1].pivot;
    e.select(Selected::Part(0));
    e.mode = Mode::Arrange;
    e.checkpoint();
    e.translate(10.0, -5.0);
    assert!((e.parts[1].layer.placement.x - old.x - 10.0).abs() < 0.001);
    assert!((e.parts[1].pivot.0 - joint.0 - 0.1).abs() < 0.001);
    e.undo();
    assert_eq!(e.parts[1].layer.placement, old);
    e.mode = Mode::Pivot;
    e.click(0.4, 0.6);
    assert_eq!(e.parts[1].layer.placement, old);
    let ground = e.ground;
    e.resize_canvas(0, "200".into());
    assert_eq!(e.ground.0, ground.0 / 2.0);
    e.undo();
    assert_eq!(e.canvas, (100, 100));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn failed_sources_and_cancelled_save_preserve_draft_and_installation() {
    let dir = std::env::temp_dir().join(format!("tinypaw-crop-failure-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (root, c) = test_install_folder(&dir, OnDuplicate::Ask);
    let mut e = Editor::open_in(&root, &c).unwrap();
    let empty = dir.join("empty.png");
    RgbaImage::new(12, 14).save(&empty).unwrap();
    assert!(
        e.set_image(Target::NewPart, &empty)
            .unwrap_err()
            .contains("empty")
    );
    assert!(e.parts.is_empty());
    assert!(!e.can_undo());
    let original = std::fs::read(root.join(c.id).join(pack::MANIFEST)).unwrap();
    let job = std::sync::atomic::AtomicU8::new(2);
    assert!(
        e.save_into_guarded(&root, Some(&job))
            .unwrap_err()
            .contains("cancelled")
    );
    assert_eq!(
        std::fs::read(root.join(c.id).join(pack::MANIFEST)).unwrap(),
        original
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "local seven-PNG Yohta artwork acceptance"]
fn seven_yohta_crops_load_and_save() {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/mascot/yohta/separated-parts");
    let dir = std::env::temp_dir().join(format!("tinypaw-yohta-crops-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut e = Editor::from_body_into(&dir, &assets.join("body_with_paws.png"), &[]).unwrap();
    let names = [
        "eye_screen_left.png",
        "eyebrow_screen_right.png",
        "head.png",
        "tail.png",
        "eye_screen_right.png",
        "eyebrow_screen_left.png",
    ];
    let errors = e.set_images(
        Target::NewPart,
        names.into_iter().map(|n| assets.join(n)).collect(),
    );
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(e.parts.len(), 6);
    assert_eq!(
        e.parts
            .iter()
            .filter(|p| p.kind == PartType::Other && p.on_head)
            .count(),
        4
    );
    let saved = e.save_into(&dir).unwrap();
    let reopened = Editor::open_in(&dir, &saved).unwrap();
    assert_eq!(reopened.parts.len(), 6);
    assert_eq!(
        reopened.parts[0].layer.image.dimensions(),
        e.parts[0].layer.image.dimensions()
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn undo_restores_whole_gestures_and_joints_are_a_temporary_tool() {
    let dir = std::env::temp_dir().join(format!("tinypaw-gesture-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (root, c) = test_install_folder(&dir, OnDuplicate::Ask);
    let mut e = Editor::open_in(&root, &c).unwrap();
    let start = e.body.placement;
    // Holding an arrow: twenty nudges, one undo.
    for _ in 0..20 {
        e.checkpoint_gesture(Gesture::Nudge);
        e.nudge(1.0, 0.0);
    }
    // Typing a number: each keystroke edits, one undo.
    for text in ["1", "12", "12.", "12.5"] {
        e.field(1, text.into());
    }
    e.undo();
    assert_eq!(e.body.placement.y, start.y, "the typed value is one step");
    assert!((e.body.placement.x - start.x - 20.0).abs() < 1e-3);
    e.undo();
    assert_eq!(e.body.placement, start, "the held arrow is one step");
    // A joint is edited only inside Adjust joint.
    let head = dir.join("cat-head.png");
    RgbaImage::from_fn(100, 100, |x, y| {
        let on = (20..80).contains(&x) && (10..55).contains(&y);
        image::Rgba(if on { [200, 90, 40, 255] } else { [0, 0, 0, 0] })
    })
    .save(&head)
    .unwrap();
    e.set_image(Target::NewPart, &head).unwrap();
    let joint = e.parts[0].pivot;
    assert_eq!(e.joint_part(), None, "no joint tool by default");
    e.click(0.1, 0.1);
    assert_eq!(e.parts[0].pivot, joint, "an ordinary click edits no joint");
    e.adjust_joint(0);
    assert_eq!(e.joint_part(), Some(0));
    e.click(0.4, 0.6);
    assert_eq!(e.parts[0].pivot, (0.4, 0.6));
    e.select(Selected::Body);
    assert_eq!(
        e.joint_part(),
        None,
        "choosing another layer leaves the tool"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn reference_results_assemble_save_and_reimport_with_their_roles() {
    use super::super::reactions::{Config, held};
    let dir = std::env::temp_dir().join(format!("tinypaw-reference-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // What an image tool returns: three PNGs on one 64 × 64 stage.
    let block = |x0: u32, x1: u32, y0: u32, y1: u32| {
        RgbaImage::from_fn(64, 64, |x, y| {
            let on = (x0..x1).contains(&x) && (y0..y1).contains(&y);
            image::Rgba(if on { [200, 90, 40, 255] } else { [0, 0, 0, 0] })
        })
    };
    block(20, 44, 10, 60).save(dir.join("body.png")).unwrap();
    block(44, 58, 40, 50).save(dir.join("tail.png")).unwrap();
    block(18, 46, 4, 62).save(dir.join("held.png")).unwrap();
    let root = dir.join("characters");
    std::fs::create_dir_all(&root).unwrap();
    let mut e = Editor::from_body_into(&root, &dir.join("body.png"), &[]).unwrap();
    assert!(
        e.set_images(Target::NewPart, vec![dir.join("tail.png")])
            .is_empty()
    );
    e.set_image(Target::Held, &dir.join("held.png")).unwrap();
    for event in [
        super::super::reactions::Event::Headpat,
        super::super::reactions::Event::Sleep,
        super::super::reactions::Event::Wake,
    ] {
        e.set_reference_reaction(event, &dir.join("held.png"))
            .unwrap();
    }
    e.enable_reference_headpat();
    let saved = e.save_into(&root).unwrap();
    let folder = saved.folder.clone().unwrap();
    assert!(folder.join(HELD_FILE).exists());
    assert!(held(&saved.pack_reactions(), &Config::default()).is_some());
    assert!(
        saved
            .reaction_image(&held(&saved.pack_reactions(), &Config::default()).unwrap())
            .is_some()
    );
    assert_eq!(
        saved.rig().unwrap().motion_parts().len(),
        1,
        "the tail is a part"
    );
    // Opening and saving again keeps the held pose.
    let again = Editor::open_in(&root, &saved)
        .unwrap()
        .save_into(&root)
        .unwrap();
    assert!(held(&again.pack_reactions(), &Config::default()).is_some());
    let reactions = again.pack_reactions();
    assert!(reactions.companionship.as_ref().unwrap().petting);
    for event in [
        super::super::reactions::Event::Headpat,
        super::super::reactions::Event::Sleep,
        super::super::reactions::Event::Wake,
    ] {
        let art = super::super::reactions::art(&reactions, &Config::default(), event).unwrap();
        assert!(again.reaction_image(&art).is_some());
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn bundled_yohta_is_never_saved_over() {
    let dir = std::env::temp_dir().join(format!("tinypaw-builtin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (root, c) = test_install_folder(&dir, OnDuplicate::Ask);
    let mut e = Editor::open_in(&root, &c).unwrap();
    e.id = pack::BUILTIN;
    assert!(e.save_into(&root).unwrap_err().contains("copy"));
    assert!(!root.join(pack::BUILTIN).exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_resting_point_follows_the_body_until_set_by_hand() {
    let dir = std::env::temp_dir().join(format!("tinypaw-rest-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (root, c) = test_install_folder(&dir, OnDuplicate::Ask);
    let mut e = Editor::open_in(&root, &c).unwrap();
    // An installed pack opens in Arrange with the body selected and no
    // joint tool or resting-point guide active.
    assert_eq!(
        (e.mode, e.selected, e.adjusting_rest),
        (Mode::Arrange, Selected::Body, false)
    );
    // The fixture has no editor metadata: its anchor is kept by hand.
    assert_eq!(e.rest, RestMode::Manual);
    e.reset_rest();
    assert_eq!(e.rest, RestMode::Auto);
    let auto = e.auto_rest().unwrap();
    assert_eq!(e.ground, auto);
    // Moving the body moves the automatic point with it on save.
    e.translate(0.0, -10.0);
    let saved = e.save_into(&root).unwrap();
    let reopened = Editor::open_in(&root, &saved).unwrap();
    assert_eq!(reopened.rest, RestMode::Auto);
    assert!(
        (reopened.ground.1 - (auto.1 - 0.1)).abs() < 0.02,
        "{:?} {auto:?}",
        reopened.ground
    );
    // Adjust manually: clicks place it and it stays set by hand.
    let mut e = reopened;
    e.adjust_rest();
    e.click(0.3, 0.7);
    e.finish_rest();
    assert_eq!((e.rest, e.ground), (RestMode::Manual, (0.3, 0.7)));
    let saved = e.save_into(&root).unwrap();
    let reopened = Editor::open_in(&root, &saved).unwrap();
    assert_eq!(
        (reopened.rest, reopened.ground),
        (RestMode::Manual, (0.3, 0.7))
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn layers_have_readable_names_and_eye_images_ride_the_head() {
    assert_eq!(
        display_name(Path::new("eye_screen_left.png")),
        "Eye screen left"
    );
    assert_eq!(
        display_name(Path::new("Right-Eyebrow.PNG")),
        "Right eyebrow"
    );
    let dir = std::env::temp_dir().join(format!("tinypaw-names-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (root, c) = test_install_folder(&dir, OnDuplicate::Ask);
    let mut e = Editor::open_in(&root, &c).unwrap();
    assert_eq!(e.eye_style(), EyeStyle::InArt);
    let block = |x0: u32, y0: u32, x1: u32, y1: u32| {
        RgbaImage::from_fn(100, 100, |x, y| {
            let on = (x0..x1).contains(&x) && (y0..y1).contains(&y);
            image::Rgba(if on { [200, 90, 40, 255] } else { [0, 0, 0, 0] })
        })
    };
    let head = dir.join("cat-head.png");
    block(20, 10, 80, 55).save(&head).unwrap();
    let eye = dir.join("left_eye.png");
    block(35, 25, 45, 35).save(&eye).unwrap();
    e.set_image(Target::NewPart, &head).unwrap();
    e.set_image(Target::EyePart, &eye).unwrap();
    let p = &e.parts[1];
    assert_eq!(p.name, "Left eye");
    assert_eq!(
        (p.kind, p.on_head, p.motion.preset),
        (PartType::Other, true, super::super::motion::Preset::Still)
    );
    assert_eq!(e.eye_style(), EyeStyle::Separate);
    e.rename(0, "Cat head".into());
    // The draft rig matches what a save would install.
    let draft = e.draft_rig();
    let saved = e.save_into(&root).unwrap();
    let installed = saved.rig().unwrap();
    assert_eq!(draft.motion_parts().len(), installed.motion_parts().len());
    assert!(draft.anchor().distance(installed.anchor()) < 0.5);
    // Names come back from the editor metadata.
    let reopened = Editor::open_in(&root, &saved).unwrap();
    assert_eq!(
        reopened
            .parts
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>(),
        ["Cat head", "Left eye"]
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn edit_add_parts_and_save() {
    let dir = std::env::temp_dir().join(format!("tinypaw-editor-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (root, c) = test_install_folder(&dir, OnDuplicate::Ask);
    let mut e = Editor::open_in(&root, &c).unwrap();
    assert!(e.parts.is_empty());
    let block = |x0: u32, y0: u32, x1: u32, y1: u32| {
        RgbaImage::from_fn(100, 100, |x, y| {
            let on = (x0..x1).contains(&x) && (y0..y1).contains(&y);
            image::Rgba(if on { [200, 90, 40, 255] } else { [0, 0, 0, 0] })
        })
    };
    let head = dir.join("cat-head.png");
    block(20, 10, 80, 55).save(&head).unwrap();
    let tail = dir.join("cat-tail.png");
    block(65, 60, 95, 70).save(&tail).unwrap();
    let ear = dir.join("ear.png");
    block(20, 5, 30, 15).save(&ear).unwrap();
    e.set_image(Target::NewPart, &head).unwrap();
    e.set_image(Target::NewPart, &tail).unwrap();
    e.set_image(Target::NewPart, &ear).unwrap();
    assert_eq!(
        e.parts.iter().map(|p| p.kind).collect::<Vec<_>>(),
        [PartType::Head, PartType::Tail, PartType::Ear]
    );
    assert!(e.parts[2].on_head, "ears ride on the head");
    assert!(
        (e.parts[0].pivot.1 - 0.55).abs() < 0.01,
        "heads turn at the neck"
    );
    e.adjust_joint(1);
    e.click(0.66, 0.65);
    assert_eq!(e.parts[1].pivot, (0.66, 0.65));
    e.set_image(Target::Face(0, 1), &head).unwrap();
    assert!(
        e.set_image(Target::Face(1, 1), &head).is_err(),
        "tails have no faces"
    );

    // An eye without its mask can't be saved; with one, it can.
    e.add_eye();
    e.set_image(Target::Eye(0, EyeSlot::White), &ear).unwrap();
    e.set_image(Target::Eye(0, EyeSlot::Pupil), &ear).unwrap();
    assert!(e.save_into(&root).unwrap_err().contains("mask"));
    e.set_image(Target::Eye(0, EyeSlot::Mask), &ear).unwrap();
    let pupil_centre = e.eyes[0]
        .pupil
        .as_ref()
        .unwrap()
        .visible_centre(e.canvas)
        .unwrap();
    e.set_pupil_size(0, 1.5).unwrap();
    e.set_pupil_size(0, 2.0).unwrap();
    let pupil = e.eyes[0].pupil.as_ref().unwrap();
    let resized_centre = pupil.visible_centre(e.canvas).unwrap();
    assert!((pupil_centre.0 - resized_centre.0).abs() < 0.0001);
    assert!((pupil_centre.1 - resized_centre.1).abs() < 0.0001);
    let original_pixels = pupil.image.pixels().filter(|pixel| pixel[3] > 0).count();
    let baked = placement::bake(&pupil.image, pupil.placement, e.canvas);
    assert!(baked.pixels().filter(|pixel| pixel[3] > 0).count() >= original_pixels * 4);
    e.undo();
    assert_eq!(e.eyes[0].pupil.as_ref().unwrap().placement.scale, 1.0);
    e.redo();
    assert_eq!(e.eyes[0].pupil.as_ref().unwrap().placement.scale, 2.0);
    let saved = e.save_into(&root).unwrap();
    assert_eq!(saved.id, c.id, "saving keeps the character's identity");
    let rig = saved.rig().unwrap();
    assert_eq!(rig.motion_parts().len(), 3);
    assert!(rig.eye_point().is_some());

    // Reopening shows what was saved.
    let again = Editor::open_in(&root, &saved).unwrap();
    assert_eq!(
        again.eyes[0].pupil.as_ref().unwrap().placement,
        e.eyes[0].pupil.as_ref().unwrap().placement
    );
    assert_eq!(again.parts.len(), 3);
    assert_eq!(again.parts[1].pivot, (0.66, 0.65));
    assert!(again.parts[0].faces[1].is_some());
    assert!(again.parts[2].on_head);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn retyping_and_removing_the_head_unhooks_its_riders() {
    let dir = std::env::temp_dir().join(format!("tinypaw-editor2-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (root, c) = test_install_folder(&dir, OnDuplicate::Ask);
    let mut e = Editor::open_in(&root, &c).unwrap();
    let png = dir.join("x.png");
    RgbaImage::from_pixel(100, 100, image::Rgba([1, 1, 1, 255]))
        .save(&png)
        .unwrap();
    let head = dir.join("head.png");
    std::fs::copy(&png, &head).unwrap();
    let ear = dir.join("ear.png");
    std::fs::copy(&png, &ear).unwrap();
    e.set_image(Target::NewPart, &head).unwrap();
    e.set_image(Target::NewPart, &ear).unwrap();
    assert!(e.parts[1].on_head);
    // A second head isn't allowed.
    e.set_kind(1, PartType::Head);
    assert_eq!(e.parts[1].kind, PartType::Ear);
    e.remove_part(0);
    assert!(!e.parts[0].on_head);
    std::fs::remove_dir_all(&dir).unwrap();
}
