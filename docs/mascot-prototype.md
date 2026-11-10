# Yohta sitting mascot prototype

Feature scope and custom character imports are specified separately in
[Desktop character and custom character imports](mascot-feature.md).

Launch with `cargo run -- --mascot-preview`. This opens a normal preview window
without starting the resident instance, capture, global hooks, tray or audio.
The approved design is the large-headed, tiny-bodied anime chibi, turned about
45 degrees toward his right (the viewer's left), with yellow/orange crest,
cyan eyes, blue/navy fur, white chest and orange accents.

## What works

- Pupils follow the cursor inside the preview stage, with bounded, eased motion.
- Eye masks follow the artwork's white sockets; gaze is counter-rotated into
  the head's local coordinates so it stays directional during tilting.
- Blink overlays, subtle neck rotation and tail rotation share the rig.
- A simulated beat demonstrates head bobbing and a small whole-body bounce.
  It does not capture sound or detect beats.
- Clicking the mascot toggles a sample menu. Its buttons show selection
  feedback without dispatching commands to the resident app.
- Size (96–440 px) and background controls help evaluate desktop legibility.
- Turning idle motion and beat simulation off stops periodic updates once
  cursor tracking settles.

## Artwork and rendering

`assets/mascot/yohta/source/approved.png` is the approved concept sheet.
Its original bytes are retained. The renderer decodes it once at startup,
loads the head/body/tail sprite regions, and drops disconnected neighbouring
fragments from the head region. Neck overlap hides the internal sticker edge.
Eye frames use the connected white socket masks and cache seven horizontal
by seven vertical gaze directions per eye. No per-frame image decoding.

This is a prototype rig, not a finished layered illustration. Pupils and blink
fills are rendered procedurally within the masks. Artist-authored iris and
eyelid assets, stronger overlap artwork and separate PNG exports remain useful
production refinements. The standing pose has not been created.

The renderer lives in `src/mascot/rig.rs`; preview controls and timing live
in `src/mascot/mod.rs`. The annotation session owns neither of them.

## Validation

`cargo test --offline` validates rig loading, eye masks, bounded gaze,
motion disabling and fitting in a small viewport alongside the existing suite.
For a native renderer smoke check:

```powershell
target/debug/tinypaw.exe --mascot-preview --snapshot .work/yohta.png
target/debug/tinypaw.exe --mascot-preview --snapshot .work/yohta-blink.png --snapshot-scene blink
target/debug/tinypaw.exe --mascot-preview --snapshot .work/yohta-menu.png --snapshot-scene menu
```

The destination directory must exist. Snapshot mode captures the preview after
startup and exits. Additional scenes are `light` and `small`.

## Desktop integration

The same rig now runs in the resident app's desktop character window
(`src/mascot/desktop.rs`). See
[Stage 1 implementation](mascot-feature.md#stage-1-implementation). Still to
come: real music response, as an optional platform audio service that supplies
an amplitude envelope to the rig and releases audio resources when disabled;
and the standing pose, against the same eye/neck/tail contract.
