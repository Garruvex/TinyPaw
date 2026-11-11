use super::*;

#[test]
fn bundled_yohta_is_an_ordinary_pack_that_exports_and_copies() {
    let dir = scratch("bundled");
    let rig = bundled_rig();
    assert_eq!(rig.motion_parts().len(), 2, "head and tail");
    assert!(rig.eye_point().is_some(), "his eyes still track the cursor");
    assert!(Character::builtin().thumbnail().is_some());
    let lines = Character::builtin().pack_lines();
    assert_eq!(lines.total(), 12, "Yohta has a few things to say");
    assert!(lines.sounds().is_empty());
    // Export writes a folder Import folder accepts as another character.
    let out = dir.join("out");
    fs::create_dir_all(&out).unwrap();
    let folder = export(&Character::builtin(), &out).unwrap();
    assert!(folder.join("sit/eye-2-blink.png").exists());
    let root = dir.join("characters");
    // His ID is the built-in one, so importing asks first.
    assert!(matches!(
        import_folder_into(&root, &folder, &[], OnDuplicate::Ask).unwrap(),
        Imported::Duplicate { .. }
    ));
    let Imported::Installed(imported) =
        import_folder_into(&root, &folder, &[], OnDuplicate::Replace).unwrap()
    else {
        panic!("installed");
    };
    assert_ne!(
        imported.id, BUILTIN,
        "an import never replaces the built-in Yohta"
    );
    // An editable copy is an installed pack with its own ID.
    let copy = copy_builtin_into(&root, &[imported.id]).unwrap();
    assert!(!copy.is_builtin() && copy.id != imported.id);
    assert_eq!(copy.name, "Yohta (copy)");
    assert_eq!(copy.rig().unwrap().motion_parts().len(), 2);
    fs::remove_dir_all(&dir).unwrap();
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tinypaw-character-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn reference_prompt_folders_pass_import_with_every_requested_image_mapped() {
    use crate::mascot::reference::Request;
    let dir = scratch("reference-prompt");
    let source = dir.join("pack");
    fs::create_dir(&source).unwrap();
    let mut request = Request::new("My Cat.png".into());
    request.canvas = 512;
    for (layered, eyes, tracking) in [
        (false, false, false),
        (true, false, false),
        (true, true, false),
        (false, false, true),
        (true, false, true),
    ] {
        request.head = layered;
        request.tail = layered;
        request.brows = layered;
        request.blink = layered;
        request.faces = layered;
        request.held = layered;
        request.eyes = eyes;
        request.tracking = tracking;
        let brief = request.brief();
        let (_, json) = brief.rsplit_once("\n\n").unwrap();
        let manifest = parse_manifest(json).unwrap();
        let mut files = manifest.files();
        files.sort();
        let mut requested: Vec<_> = request.roles().iter().map(|r| r.file()).collect();
        requested.sort();
        assert_eq!(files, requested);
        fs::write(source.join("character.json"), json).unwrap();
        let png = RgbaImage::from_pixel(512, 512, image::Rgba([80, 120, 220, 255]));
        for file in files {
            png.save(source.join(file)).unwrap();
        }
        request.validate_folder(&source).unwrap();
        prepare_folder_into(&dir.join("library"), &source).unwrap();
        if tracking {
            let mut changed: Json = serde_json::from_str(json).unwrap();
            changed["poses"]["sit"]["eyes"][0]["mask"] = json!("body.png");
            fs::write(source.join("character.json"), changed.to_string()).unwrap();
            assert!(
                request.validate_folder(&source).is_err(),
                "a file in the wrong feature slot must fail"
            );
            fs::write(source.join("character.json"), json).unwrap();
            fs::remove_file(source.join("right_eye_pupil.png")).unwrap();
            assert!(
                request.validate_folder(&source).is_err(),
                "an omitted pupil must fail"
            );
        }
    }
    fs::remove_dir_all(dir).unwrap();
}

/// A 200 × 300 PNG with a 60 × 120 figure whose feet are at row 249.
fn figure(path: &Path) {
    let mut img = RgbaImage::new(200, 300);
    for y in 130..250 {
        for x in 70..130 {
            img.put_pixel(x, y, image::Rgba([80, 120, 220, 255]));
        }
    }
    img.save(path).unwrap();
}

#[test]
fn failed_final_load_restores_previous_installation() {
    let root = scratch("rollback");
    let target = root.join("cat");
    let stage = root.join(".staging-cat-test");
    fs::create_dir(&target).unwrap();
    fs::create_dir(&stage).unwrap();
    fs::write(target.join("version"), "old").unwrap();
    fs::write(stage.join("version"), "new").unwrap();
    let result: Result<(), _> = commit_stage(&root, &stage, &target, true, |_| {
        Err("injected load failure".into())
    });
    assert!(result.unwrap_err().contains("injected load failure"));
    assert_eq!(fs::read_to_string(target.join("version")).unwrap(), "old");
    assert!(recover_transactions(&root).is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn interrupted_swap_recovers_backup_and_committed_swap_keeps_new() {
    for committed in [false, true] {
        let root = scratch(if committed {
            "recovery-committed"
        } else {
            "recovery"
        });
        let journal = root.join(".staging-cat-test.transaction");
        fs::create_dir_all(journal.join("backup")).unwrap();
        fs::create_dir(root.join("cat")).unwrap();
        fs::write(journal.join("target"), "cat").unwrap();
        fs::write(journal.join("backup/version"), "old").unwrap();
        fs::write(root.join("cat/version"), "new").unwrap();
        if committed {
            fs::write(journal.join("committed"), "").unwrap();
        }
        assert!(recover_transactions(&root).is_empty());
        assert_eq!(
            fs::read_to_string(root.join("cat/version")).unwrap(),
            if committed { "new" } else { "old" }
        );
        assert!(!journal.exists());
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn destination_conflict_and_failed_stage_rename_preserve_old_pack() {
    let root = scratch("rename-failure");
    let target = root.join("cat");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("version"), "old").unwrap();
    let stage = root.join(".staging-missing");
    let result = commit_stage(&root, &stage, &target, true, |_| Ok(()));
    assert!(result.unwrap_err().contains("install"));
    assert_eq!(fs::read_to_string(target.join("version")).unwrap(), "old");
    assert!(commit_stage(&root, &stage, &target, false, |_| Ok(())).is_err());
    assert_eq!(fs::read_to_string(target.join("version")).unwrap(), "old");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn prepared_snapshot_survives_source_changes_and_rejects_stale_replacement() {
    let dir = scratch("snapshot");
    let (root, installed) = test_install_folder(&dir, OnDuplicate::Ask);
    let source = dir.join("fixture");
    let prepared = prepare_folder_into(&root, &source).unwrap();
    fs::write(source.join("sit/body.png"), b"changed after validation").unwrap();
    let Imported::Installed(copy) = prepared
        .install_into(
            &root,
            &[installed.id],
            OnDuplicate::KeepBoth,
            &AtomicU8::new(0),
        )
        .unwrap()
    else {
        panic!("expected installed snapshot");
    };
    assert!(copy.rig().is_ok());
    let Imported::Installed(_) = prepared
        .install_into(&root, &[], OnDuplicate::Replace, &AtomicU8::new(0))
        .unwrap()
    else {
        panic!("expected replacement");
    };
    let err = prepared
        .install_into(&root, &[], OnDuplicate::Replace, &AtomicU8::new(0))
        .unwrap_err();
    assert!(err.contains("changed while"), "{err}");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn cancellation_before_commit_preserves_the_installed_pack() {
    let dir = scratch("cancel");
    let (root, installed) = test_install_folder(&dir, OnDuplicate::Ask);
    let prepared = prepare_folder_into(&root, &dir.join("fixture")).unwrap();
    let before = revision(installed.folder.as_ref().unwrap()).unwrap();
    let err = prepared
        .install_into(&root, &[], OnDuplicate::Replace, &AtomicU8::new(2))
        .unwrap_err();
    assert!(err.contains("cancelled"), "{err}");
    assert_eq!(
        revision(installed.folder.as_ref().unwrap()).unwrap(),
        before
    );
    assert!(installed.rig().is_ok());
    fs::remove_dir_all(&dir).unwrap();
}

#[cfg(windows)]
#[test]
fn failed_rollback_keeps_backup_for_startup_recovery() {
    use std::os::windows::fs::OpenOptionsExt;
    let root = scratch("rollback-locked");
    let stage = root.join(".staging-cat-test");
    let target = root.join("cat");
    fs::create_dir(&stage).unwrap();
    fs::create_dir(&target).unwrap();
    fs::write(stage.join("version"), "new").unwrap();
    fs::write(target.join("version"), "old").unwrap();
    let mut locked = None;
    let result: Result<(), _> = commit_stage(&root, &stage, &target, true, |p| {
        locked = Some(
            fs::OpenOptions::new()
                .read(true)
                .share_mode(0)
                .open(p.join("version"))
                .unwrap(),
        );
        Err("injected failure".into())
    });
    let err = result.unwrap_err();
    assert!(err.contains("recovery failed"), "{err}");
    let journal = stage.with_extension("transaction");
    assert_eq!(
        fs::read_to_string(journal.join("backup/version")).unwrap(),
        "old"
    );
    drop(locked);
    assert!(recover_transactions(&root).is_empty());
    assert_eq!(fs::read_to_string(target.join("version")).unwrap(), "old");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn oversized_sources_and_aggregate_pack_memory_are_bounded() {
    let dir = scratch("image-budget");
    let path = dir.join("wide.png");
    RgbaImage::new(MAX_SOURCE_SIDE + 1, 1).save(&path).unwrap();
    assert!(draft_from_png(&path).unwrap_err().contains("at most"));
    let parts: Vec<_> = (0..9)
        .map(|n| json!({ "type": "other", "image": format!("part-{n}.png"), "pivot": [0.5, 0.5] }))
        .collect();
    let manifest = json!({ "schema_version": 1, "id": "cat", "name": "Cat", "canvas": [2048,2048],
        "default_pose": "sit", "poses": { "sit": { "body": "body.png", "ground_anchor": [0.5,0.9], "parts": parts } } });
    assert!(
        parse_manifest(&manifest.to_string())
            .unwrap_err()
            .contains("memory budget")
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn editor_sources_survive_import_export_and_missing_sources_are_rejected() {
    use crate::mascot::placement::{self, Placement, SourceRecord};
    let dir = scratch("editor-resources");
    let (root, installed) = test_install_folder(&dir, OnDuplicate::Ask);
    let source = installed.folder.as_ref().unwrap();
    let mut manifest = read_manifest(source).unwrap();
    let original = RgbaImage::from_pixel(12, 14, image::Rgba([80, 120, 200, 255]));
    let transform = Placement {
        x: 40.0,
        y: 45.0,
        scale: 1.2,
        angle: 15.0,
    };
    std::fs::create_dir_all(source.join("editor/sources")).unwrap();
    original
        .save(source.join("editor/sources/body.png"))
        .unwrap();
    placement::bake(&original, transform, manifest.canvas)
        .save(source.join(&manifest.default_pose().body))
        .unwrap();
    manifest.editor = vec![SourceRecord {
        name: None,
        target: manifest.default_pose().body.clone(),
        source: "editor/sources/body.png".into(),
        size: original.dimensions(),
        placement: transform,
    }];
    std::fs::write(source.join(MANIFEST), manifest.to_json().to_string()).unwrap();
    let Imported::Installed(copy) =
        import_folder_into(&root, source, &[installed.id], OnDuplicate::KeepBoth).unwrap()
    else {
        panic!("expected installed copy")
    };
    let copied = copy.folder.as_ref().unwrap();
    assert_eq!(
        read_manifest(copied).unwrap().editor[0].placement,
        transform
    );
    assert_eq!(
        read_png(copied, "editor/sources/body.png", (12, 14)).unwrap(),
        original
    );
    let exported = export(&copy, &dir.join("export")).unwrap();
    assert_eq!(
        read_manifest(&exported).unwrap().editor[0].placement,
        transform
    );
    std::fs::remove_file(exported.join("editor/sources/body.png")).unwrap();
    assert!(import_folder_into(&root, &exported, &[], OnDuplicate::KeepBoth).is_err());
    assert!(copy.rig().is_ok());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn manifest_rules() {
    let ok = r#"{"schema_version":1,"id":"cat","name":"Cat","canvas":[64,64],
        "default_pose":"sit","poses":{"sit":{"ground_anchor":[0.5,0.9],"body":"sit/body.png"}}}"#;
    assert!(parse_manifest(ok).is_ok());
    let bad = [
        ok.replace("\"schema_version\":1", "\"schema_version\":2"),
        ok.replace("\"cat\"", "\"Cat Id\""),
        ok.replace("[64,64]", "[64,4096]"),
        ok.replace("\"default_pose\":\"sit\"", "\"default_pose\":\"stand\""),
        ok.replace("[0.5,0.9]", "[0.5,1.5]"),
        ok.replace("sit/body.png", "../body.png"),
        ok.replace("sit/body.png", "C:/body.png"),
        ok.replace("\"sit\":{", "\"fly\":{"),
    ];
    for m in &bad {
        assert!(parse_manifest(m).is_err(), "{m}");
    }
    // The original spec's head/tail shorthand, and a part riding on it.
    let layered = ok.replace(
        "\"body\":\"sit/body.png\"",
        r#""body":"sit/body.png","head":{"image":"sit/head.png","pivot":[0.5,0.6],
           "expressions":{"pressed":"sit/head-pressed.png"}},
           "parts":[{"type":"ear","image":"sit/ear.png","pivot":[0.4,0.3],"parent":"head",
             "motion":{"preset":"floppy","strength":1.5}}],
           "eyes":[{"white":"sit/w.png","pupil":"sit/p.png","mask":"sit/m.png"}]"#,
    );
    let m = parse_manifest(&layered).unwrap();
    let pose = m.default_pose();
    assert_eq!(pose.parts.len(), 2);
    assert_eq!(pose.parts[1].parent, Some(0), "the ear rides on the head");
    assert!(pose.parts[1].behind, "ears default behind the head");
    assert_eq!(pose.parts[1].motion.preset, Preset::Floppy);
    assert_eq!(m.assets().len(), 7);
    // And it survives a round trip through to_json.
    let again = parse_manifest(&m.to_json().to_string()).unwrap();
    assert_eq!(again.assets(), m.assets());
    assert_eq!(again.default_pose().parts[1].parent, Some(0));

    let broken = [
        // A preset the type doesn't offer.
        layered.replace("\"floppy\"", "\"wag\""),
        // Strength out of range.
        layered.replace("1.5", "3.0"),
        // Riding on a head that isn't there.
        layered.replace(r#""head":{"#, r#""tail":{"#),
        // Faces on something other than the head.
        layered.replace(
            "\"type\":\"ear\"",
            "\"type\":\"ear\",\"expressions\":{\"pressed\":\"x.png\"}",
        ),
        // An unknown face.
        layered.replace(
            "\"pressed\":\"sit/head-pressed.png\"",
            "\"sleepy\":\"s.png\"",
        ),
    ];
    for m in &broken {
        assert!(parse_manifest(m).is_err(), "{m}");
    }
}

#[test]
fn a_pack_without_a_rest_mode_keeps_its_anchor_by_hand() {
    use super::super::rest::RestMode;
    let base = r#"{"schema_version":1,"id":"cat","name":"Cat","canvas":[64,64],
        "default_pose":"sit","poses":{"sit":{"ground_anchor":[0.5,0.9],"body":"sit/body.png"}}"#;
    let m = parse_manifest(&format!("{base}}}")).unwrap();
    assert_eq!(m.rest, RestMode::Manual);
    let auto = format!(r#"{base},"editor":{{"version":1,"layers":[],"rest_point":"auto"}}}}"#);
    let m = parse_manifest(&auto).unwrap();
    assert_eq!(m.rest, RestMode::Auto);
    // Written back the same way, and read again.
    let again = parse_manifest(&m.to_json().to_string()).unwrap();
    assert_eq!(again.rest, RestMode::Auto);
    assert_eq!(again.poses[0].ground, (0.5, 0.9));
}

#[test]
fn one_png_round_trip() {
    let dir = scratch("png");
    let png = dir.join("My Cat.png");
    figure(&png);
    let draft = draft_from_png(&png).unwrap();
    assert_eq!(draft.name, "My Cat");
    // Trimmed to the figure plus the margin.
    assert_eq!(
        (draft.width, draft.height),
        (60 + 2 * MARGIN, 120 + 2 * MARGIN)
    );
    let (gx, gy) = draft.ground;
    assert!((gx - 0.5).abs() < 0.02 && (gy - (124.0 / 128.0)).abs() < 0.01);

    let root = dir.join("characters");
    let c = install_into(&root, &draft, &["my-cat"]).unwrap();
    assert_eq!(c.id, "my-cat-2", "taken IDs are skipped");
    assert!(c.thumbnail().is_some());
    // A new one-image character rests automatically, written explicitly.
    let m = read_manifest(c.folder.as_ref().unwrap()).unwrap();
    assert_eq!(m.rest, super::super::rest::RestMode::Auto);
    // A part drawn on the original 200 × 300 canvas lines up with the
    // trimmed body; other sizes are refused with both sizes named.
    let m = read_manifest(c.folder.as_ref().unwrap()).unwrap();
    let src = m.source.expect("quick import records its cut");
    assert_eq!(src.size, (200, 300));
    let part = dir.join("tail.png");
    RgbaImage::from_pixel(200, 300, image::Rgba([1, 2, 3, 255]))
        .save(&part)
        .unwrap();
    let fitted = fit_png(&part, m.canvas, m.source.as_ref()).unwrap();
    assert_eq!(fitted.dimensions(), m.canvas);
    RgbaImage::new(50, 50).save(&part).unwrap();
    let err = fit_png(&part, m.canvas, m.source.as_ref()).unwrap_err();
    assert!(
        err.contains("50 × 50") && err.contains("200 × 300"),
        "{err}"
    );
    // The installed copy no longer needs the source file.
    fs::remove_file(&png).unwrap();
    let rig = c.rig().unwrap();
    assert!((rig.anchor().y - gy * draft.height as f32).abs() < 0.5);
    assert!(
        !fs::read_dir(&root)
            .unwrap()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().starts_with('.')),
        "no staging folder is left behind"
    );

    // Export, then import the export: a duplicate until told what to do.
    let out = dir.join("exported");
    fs::create_dir_all(&out).unwrap();
    let folder = export(&c, &out).unwrap();
    assert!(!folder.join(THUMB).exists(), "exports carry only pack data");
    let taken = [c.id];
    assert!(matches!(
        import_folder_into(&root, &folder, &taken, OnDuplicate::Ask).unwrap(),
        Imported::Duplicate { .. }
    ));
    let Imported::Installed(both) =
        import_folder_into(&root, &folder, &taken, OnDuplicate::KeepBoth).unwrap()
    else {
        panic!("installed");
    };
    assert_ne!(both.id, c.id);
    let Imported::Installed(same) =
        import_folder_into(&root, &folder, &taken, OnDuplicate::Replace).unwrap()
    else {
        panic!("installed");
    };
    assert_eq!(same.id, c.id);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn layered_folder_imports_and_renders() {
    let dir = scratch("layered");
    let src = dir.join("src");
    fs::create_dir_all(src.join("sit")).unwrap();
    let block = |x0: u32, y0: u32, x1: u32, y1: u32| {
        RgbaImage::from_fn(100, 100, |x, y| {
            let on = (x0..x1).contains(&x) && (y0..y1).contains(&y);
            image::Rgba(if on {
                [90, 120, 200, 255]
            } else {
                [0, 0, 0, 0]
            })
        })
    };
    for (name, img) in [
        ("body", block(30, 50, 70, 95)),
        ("head", block(20, 10, 80, 55)),
        ("tail", block(65, 60, 95, 70)),
        ("white", block(40, 25, 50, 35)),
        ("pupil", block(44, 29, 46, 31)),
        ("mask", block(40, 25, 50, 35)),
    ] {
        img.save(src.join(format!("sit/{name}.png"))).unwrap();
    }
    fs::write(
        src.join(MANIFEST),
        r#"{"schema_version":1,"id":"cat","name":"Cat","canvas":[100,100],"default_pose":"sit",
          "poses":{"sit":{"ground_anchor":[0.5,0.95],"body":"sit/body.png",
            "head":{"image":"sit/head.png","pivot":[0.5,0.5]},
            "tail":{"image":"sit/tail.png","pivot":[0.66,0.65],"motion":{"preset":"wag"}},
            "eyes":[{"white":"sit/white.png","pupil":"sit/pupil.png","mask":"sit/mask.png"}]}}}"#,
    )
    .unwrap();
    let root = dir.join("characters");
    let Imported::Installed(c) = import_folder_into(&root, &src, &[], OnDuplicate::Ask).unwrap()
    else {
        panic!("installed");
    };
    let rig = c.rig().unwrap();
    assert!(rig.eye_point().is_some());
    assert_eq!(rig.motion_parts().len(), 2);
    // Every named image was copied, and nothing else.
    assert_eq!(fs::read_dir(root.join("cat/sit")).unwrap().count(), 6);
    // A missing part image fails the import cleanly.
    fs::remove_file(src.join("sit/tail.png")).unwrap();
    let err = import_folder_into(&root, &src, &[], OnDuplicate::KeepBoth)
        .err()
        .unwrap();
    assert!(err.contains("tail.png"), "{err}");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn broken_imports_leave_the_installation_alone() {
    let dir = scratch("broken");
    let root = dir.join("characters");
    let src = dir.join("src");
    fs::create_dir_all(src.join("sit")).unwrap();
    figure(&src.join("sit/body.png"));
    let manifest = |canvas: &str| {
        format!(
            r#"{{"schema_version":1,"id":"fox","name":"Fox","canvas":{canvas},
            "default_pose":"sit","poses":{{"sit":{{"ground_anchor":[0.5,0.83],"body":"sit/body.png"}}}}}}"#
        )
    };
    // The art doesn't match the declared canvas.
    fs::write(src.join(MANIFEST), manifest("[100,100]")).unwrap();
    let err = import_folder_into(&root, &src, &[], OnDuplicate::Ask)
        .err()
        .unwrap();
    assert!(
        err.contains("200 × 300") && err.contains("100 × 100"),
        "{err}"
    );
    // A missing body.
    fs::write(src.join(MANIFEST), manifest("[200,300]")).unwrap();
    fs::remove_file(src.join("sit/body.png")).unwrap();
    assert!(import_folder_into(&root, &src, &[], OnDuplicate::Ask).is_err());
    assert!(
        !root.exists() || fs::read_dir(&root).unwrap().count() == 0,
        "nothing installed"
    );
    // Bundled Yohta can't be replaced; the import becomes a copy.
    figure(&src.join("sit/body.png"));
    fs::write(
        src.join(MANIFEST),
        manifest("[200,300]").replace("\"fox\"", "\"yohta\""),
    )
    .unwrap();
    let Imported::Installed(copy) =
        import_folder_into(&root, &src, &[], OnDuplicate::Replace).unwrap()
    else {
        panic!("installed");
    };
    assert_ne!(copy.id, BUILTIN);
    assert!(remove(&Character::builtin()).is_err());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn motion_overrides_round_trip_and_must_fit_the_rig() {
    let dir = scratch("motion");
    let specs = [
        MotionSpec {
            preset: Preset::Nod,
            strength: 1.4,
            speed: 0.6,
        },
        MotionSpec {
            preset: Preset::Wag,
            strength: 2.5,
            speed: 1.0,
        },
    ];
    let o = Overrides {
        motion: Some(specs.to_vec()),
        lines: None,
        bop: Some(Bop::Groove),
        reactions: None,
    };
    save_overrides_in(&dir, BUILTIN, &o).unwrap();
    assert_eq!(load_overrides_in(&dir, BUILTIN).bop, Some(Bop::Groove));
    let loaded = load_overrides_in(&dir, BUILTIN).motion.unwrap();
    assert_eq!(loaded[0], specs[0]);
    assert_eq!(loaded[1].strength, 2.0, "clamped to the slider range");
    assert!(!dir.join(".overrides/yohta.json.partial").exists());
    // Yohta's rig takes two specs; a single one, or a preset the part
    // can't use, is refused and the rig keeps its own motion.
    let mut rig = Rig::load();
    assert!(rig.set_motion(&loaded));
    assert_eq!(rig.motion()[0].preset, Preset::Nod);
    assert!(!rig.set_motion(&loaded[..1]));
    assert!(!rig.set_motion(&[specs[1], specs[1]]), "a head can't wag");
    assert!(save_overrides_in(&dir, "../escape", &o).is_err());
    // Lines save beside motion, and "no lines" is kept as a choice.
    let quiet = Overrides {
        motion: None,
        lines: Some(Lines::default()),
        bop: None,
        reactions: None,
    };
    save_overrides_in(&dir, BUILTIN, &quiet).unwrap();
    assert_eq!(load_overrides_in(&dir, BUILTIN), quiet);
    // Clearing everything removes the file.
    save_overrides_in(&dir, BUILTIN, &Overrides::default()).unwrap();
    assert!(!dir.join(".overrides/yohta.json").exists());
    fs::remove_dir_all(&dir).unwrap();
}

/// A short 16-bit mono WAV, written by hand so tests need no fixtures.
fn write_wav(path: &Path) {
    let samples: Vec<i16> = (0..800)
        .map(|n| ((n as f32 * 0.2).sin() * 8000.0) as i16)
        .collect();
    let data: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
    let mut wav = Vec::new();
    wav.extend(b"RIFF");
    wav.extend((36 + data.len() as u32).to_le_bytes());
    wav.extend(b"WAVEfmt ");
    wav.extend(16u32.to_le_bytes());
    wav.extend(1u16.to_le_bytes()); // PCM
    wav.extend(1u16.to_le_bytes()); // mono
    wav.extend(8000u32.to_le_bytes());
    wav.extend(16000u32.to_le_bytes());
    wav.extend(2u16.to_le_bytes());
    wav.extend(16u16.to_le_bytes());
    wav.extend(b"data");
    wav.extend((data.len() as u32).to_le_bytes());
    wav.extend(data);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, wav).unwrap();
}

#[test]
fn reaction_sounds_install_export_and_reimport() {
    use super::super::reactions::{Config, Event, Sound};
    let dir = scratch("reaction-sounds");
    let src = dir.join("src");
    fs::create_dir_all(src.join("sit")).unwrap();
    RgbaImage::from_fn(100, 100, |x, y| {
        let on = (30..70).contains(&x) && (50..95).contains(&y);
        image::Rgba(if on { [40, 80, 200, 255] } else { [0, 0, 0, 0] })
    })
    .save(src.join("sit/body.png"))
    .unwrap();
    write_wav(&src.join("sounds/boop.wav"));
    fs::write(
        src.join(MANIFEST),
        r#"{"schema_version":1,"id":"booper","name":"Booper","canvas":[100,100],
          "default_pose":"sit","poses":{"sit":{"ground_anchor":[0.5,0.95],"body":"sit/body.png"}},
          "reactions":{"click":{"sound":{"file":"sounds/boop.wav"},"visual":"compress"},
                       "drop":{"sound":{"builtin":"duck-toy"}}}}"#,
    )
    .unwrap();
    let root = dir.join("characters");
    let Imported::Installed(c) = import_folder_into(&root, &src, &[], OnDuplicate::Ask).unwrap()
    else {
        panic!("installed");
    };
    assert!(
        root.join("booper/sounds/boop.wav").exists(),
        "declared sounds are copied"
    );
    let pack = read_manifest(&root.join("booper")).unwrap().reactions;
    assert_eq!(
        pack.entry(Event::Click).unwrap().visual,
        Some(super::super::reactions::Visual::Compress)
    );
    // The user's layer: an imported WAV on Pick up, Drop turned off.
    let user_dir = dir.join("user");
    write_wav(&user_dir.join("reaction-7.wav"));
    let mut user = Config::default();
    let mut entry = super::super::reactions::Entry::default();
    entry.sound.enabled = true;
    entry.sound.sound = Some(Sound::File(SoundRef::User("reaction-7.wav".into())));
    user.set_entry(Event::Pickup, Some(entry));
    user.set_entry(Event::Drop, Some(Default::default()));
    let o = Overrides {
        reactions: Some(user),
        ..Overrides::default()
    };
    let out = dir.join("out");
    fs::create_dir_all(&out).unwrap();
    let folder = export_with(&c, &out, &o, |f| Some(user_dir.join(f))).unwrap();
    assert!(
        folder.join("sounds/reaction-7.wav").exists() && folder.join("sounds/boop.wav").exists()
    );
    // The exported folder reimports with the same effective sounds.
    let again = read_manifest(&folder).unwrap().reactions;
    let s = super::super::reactions::effective(&again, &Config::default());
    assert_eq!(
        s.get(Event::Click).sound,
        Some(Sound::File(SoundRef::Pack("sounds/boop.wav".into())))
    );
    assert_eq!(
        s.get(Event::Pickup).sound,
        Some(Sound::File(SoundRef::Pack("sounds/reaction-7.wav".into())))
    );
    assert!(!s.get(Event::Drop).enabled);
    // A pack naming a missing reaction sound fails its import.
    fs::remove_file(src.join("sounds/boop.wav")).unwrap();
    assert!(import_folder_into(&root, &src, &[], OnDuplicate::KeepBoth).is_err());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn held_poses_are_pack_images_that_install_and_export() {
    use super::super::reactions::{Art, Config, Event};
    let dir = scratch("held-pose");
    let src = dir.join("src");
    fs::create_dir_all(src.join("sit")).unwrap();
    let block = |x0: u32, x1: u32| {
        RgbaImage::from_fn(100, 100, |x, y| {
            let on = (x0..x1).contains(&x) && (50..95).contains(&y);
            image::Rgba(if on { [40, 80, 200, 255] } else { [0, 0, 0, 0] })
        })
    };
    block(30, 70).save(src.join("sit/body.png")).unwrap();
    fs::create_dir_all(src.join("reactions")).unwrap();
    block(20, 80).save(src.join("reactions/held.png")).unwrap();
    fs::write(
        src.join(MANIFEST),
        r#"{"schema_version":1,"id":"dangler","name":"Dangler","canvas":[100,100],
          "default_pose":"sit","poses":{"sit":{"ground_anchor":[0.5,0.95],"body":"sit/body.png"}},
          "reactions":{"pickup":{"visual":"none","held_pose":{"image":"reactions/held.png","place":[0.02,-0.04,1.0]}}}}"#,
    )
    .unwrap();
    let root = dir.join("characters");
    let Imported::Installed(c) = import_folder_into(&root, &src, &[], OnDuplicate::Ask).unwrap()
    else {
        panic!("installed");
    };
    assert!(root.join("dangler/reactions/held.png").exists());
    let held = super::super::reactions::held(&c.pack_reactions(), &Config::default()).unwrap();
    assert!(c.reaction_image(&held).is_some(), "decodes on the stage");
    // A user Use artwork picture travels with Export.
    let user = dir.join("user");
    fs::create_dir_all(&user).unwrap();
    block(10, 90).save(user.join("art-5.png")).unwrap();
    let mut layer = Config::default();
    let entry = super::super::reactions::Entry {
        visual: Some(super::super::reactions::Visual::Artwork),
        art: Some(Art::new(SoundRef::User("art-5.png".into()))),
        ..Default::default()
    };
    layer.set_entry(Event::Drop, Some(entry));
    let o = Overrides {
        reactions: Some(layer),
        ..Overrides::default()
    };
    let out = dir.join("out");
    fs::create_dir_all(&out).unwrap();
    let folder = export_with(&c, &out, &o, |f| Some(user.join(f))).unwrap();
    assert!(
        folder.join("reactions/art-5.png").exists() && folder.join("reactions/held.png").exists()
    );
    // A held pose on another stage size is refused.
    RgbaImage::new(50, 50)
        .save(src.join("reactions/held.png"))
        .unwrap();
    assert!(import_folder_into(&root, &src, &[], OnDuplicate::KeepBoth).is_err());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn line_sounds_travel_with_the_pack() {
    let dir = scratch("line-sounds");
    let src = dir.join("src");
    fs::create_dir_all(src.join("sit")).unwrap();
    RgbaImage::from_fn(100, 100, |x, y| {
        let on = (30..70).contains(&x) && (50..95).contains(&y);
        image::Rgba(if on { [40, 80, 200, 255] } else { [0, 0, 0, 0] })
    })
    .save(src.join("sit/body.png"))
    .unwrap();
    write_wav(&src.join("lines/hi.wav"));
    fs::write(
        src.join(MANIFEST),
        r##"{"schema_version":1,"id":"talker","name":"Talker","canvas":[100,100],
          "default_pose":"sit","poses":{"sit":{"ground_anchor":[0.5,0.95],"body":"sit/body.png"}},
          "lines":{"click":[{"text":"Hi!","sound":"lines/hi.wav"}]},
          "bubble":{"background":"#FFF7E0"}}"##,
    )
    .unwrap();
    let root = dir.join("characters");
    let Imported::Installed(c) = import_folder_into(&root, &src, &[], OnDuplicate::Ask).unwrap()
    else {
        panic!("installed");
    };
    assert!(
        root.join("talker/lines/hi.wav").exists(),
        "the sound was copied"
    );
    let lines = read_manifest(&root.join("talker")).unwrap().lines;
    assert_eq!(lines.get(super::super::lines::Trigger::Click).len(), 1);
    assert_eq!(lines.bubble.background, [255, 247, 224, 255]);
    // Export carries it too.
    let out = dir.join("out");
    fs::create_dir_all(&out).unwrap();
    let folder = export(&c, &out).unwrap();
    assert!(folder.join("lines/hi.wav").exists());
    // A missing or unreadable sound fails the import.
    fs::remove_file(src.join("lines/hi.wav")).unwrap();
    assert!(import_folder_into(&root, &src, &[], OnDuplicate::KeepBoth).is_err());
    fs::write(src.join("lines/hi.wav"), b"not a wav").unwrap();
    let err = import_folder_into(&root, &src, &[], OnDuplicate::KeepBoth)
        .err()
        .unwrap();
    assert!(err.contains("hi.wav"), "{err}");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn export_carries_motion_lines_and_sounds_and_imports_the_same() {
    use super::super::lines::{Line, Trigger};
    let dir = scratch("share");
    // A layered pack (head and tail) from the fixture art.
    let (root, plain) = test_install_folder(&dir, OnDuplicate::Ask);
    assert!(plain.rig().is_ok());
    let src = dir.join("layered");
    fs::create_dir_all(src.join("sit")).unwrap();
    let block = |x0: u32, y0: u32, x1: u32, y1: u32| {
        RgbaImage::from_fn(100, 100, |x, y| {
            let on = (x0..x1).contains(&x) && (y0..y1).contains(&y);
            image::Rgba(if on {
                [90, 120, 200, 255]
            } else {
                [0, 0, 0, 0]
            })
        })
    };
    block(30, 50, 70, 95)
        .save(src.join("sit/body.png"))
        .unwrap();
    block(20, 10, 80, 55)
        .save(src.join("sit/head.png"))
        .unwrap();
    block(65, 60, 95, 70)
        .save(src.join("sit/tail.png"))
        .unwrap();
    fs::write(
        src.join(MANIFEST),
        r#"{"schema_version":1,"id":"fox","name":"Fox","canvas":[100,100],"default_pose":"sit",
          "poses":{"sit":{"ground_anchor":[0.5,0.95],"body":"sit/body.png",
            "head":{"image":"sit/head.png","pivot":[0.5,0.5]},
            "tail":{"image":"sit/tail.png","pivot":[0.66,0.65]}}}}"#,
    )
    .unwrap();
    let Imported::Installed(fox) = import_folder_into(&root, &src, &[], OnDuplicate::Ask).unwrap()
    else {
        panic!("installed");
    };
    // The user's changes: new motion, a line with their own sound.
    let sounds = dir.join("user-sounds");
    write_wav(&sounds.join("line-1.wav"));
    let mut lines = Lines::default();
    lines.get_mut(Trigger::Click).push(Line {
        text: "Yip!".into(),
        sound: Some(SoundRef::User("line-1.wav".into())),
    });
    let motion = vec![
        MotionSpec {
            preset: Preset::Nod,
            strength: 1.5,
            speed: 0.5,
        },
        MotionSpec {
            preset: Preset::Wag,
            strength: 2.0,
            speed: 1.2,
        },
    ];
    let o = Overrides {
        motion: Some(motion.clone()),
        lines: Some(lines),
        bop: Some(Bop::RubberHose),
        reactions: None,
    };
    let out = dir.join("out");
    fs::create_dir_all(&out).unwrap();
    let folder = export_with(&fox, &out, &o, |f| Some(sounds.join(f))).unwrap();
    assert!(
        folder.join("lines/line-1.wav").exists(),
        "the user's sound travels"
    );

    // "Another PC": a fresh characters folder, no overrides there.
    let other = dir.join("other-pc");
    let Imported::Installed(copy) =
        import_folder_into(&other, &folder, &[], OnDuplicate::Ask).unwrap()
    else {
        panic!("installed");
    };
    let rig = copy.rig().unwrap();
    assert_eq!(rig.motion(), motion.as_slice(), "same motion");
    assert_eq!(rig.bop(), Bop::RubberHose, "same bop");
    let lines = read_manifest(copy.folder.as_ref().unwrap()).unwrap().lines;
    let click = &lines.get(Trigger::Click)[0];
    assert_eq!(click.text, "Yip!");
    assert_eq!(click.sound, Some(SoundRef::Pack("lines/line-1.wav".into())));
    assert!(other.join("fox/lines/line-1.wav").exists());

    // A single image's sway travels too.
    let o = Overrides {
        motion: Some(vec![MotionSpec {
            preset: Preset::Nod,
            strength: 0.4,
            speed: 1.0,
        }]),
        lines: None,
        bop: None,
        reactions: None,
    };
    let folder = export_with(&plain, &out, &o, |_| None).unwrap();
    let Imported::Installed(sprite) =
        import_folder_into(&other, &folder, &[], OnDuplicate::Ask).unwrap()
    else {
        panic!("installed");
    };
    assert_eq!(sprite.rig().unwrap().motion()[0].strength, 0.4);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn replacing_a_pack_keeps_the_users_overrides() {
    let dir = scratch("replace-keeps");
    let o = Overrides {
        motion: Some(vec![MotionSpec {
            preset: Preset::Nod,
            strength: 1.0,
            speed: 1.0,
        }]),
        lines: Some(Lines::default()),
        bop: None,
        reactions: None,
    };
    let (root, c) = test_install_folder(&dir, OnDuplicate::Ask);
    save_overrides_in(&root, c.id, &o).unwrap();
    let folder = dir.join("fixture");
    let Imported::Installed(again) =
        import_folder_into(&root, &folder, &[c.id], OnDuplicate::Replace).unwrap()
    else {
        panic!("installed");
    };
    assert_eq!(again.id, c.id);
    assert_eq!(
        load_overrides_in(&root, c.id),
        o,
        "replace keeps what the user changed"
    );
    fs::remove_dir_all(&dir).unwrap();
}
