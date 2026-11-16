# Slint studio prototype (plan step 0)

Throwaway prototype for [the settings and multi-character plan](../../docs/settings-and-multi-character-plan.md). It is isolated from the main package: its own workspace, lockfile and `target` folder. Nothing in `src/` of TinyPaw depends on it.

It has a Settings Characters page and a Character studio preview pane, both in Slint 1.18.1 with the software renderer. The preview picture is drawn with tiny-skia into a pixel buffer (a stand-in for the shared character renderer) and handed to Slint every frame. The selection box over the draggable layer is drawn by Slint from the same layer position and zoom.

Run it to try the drag by hand. Click Customize to open the studio, then drag the green layer and move the zoom slider.

```bash
cd prototypes/slint-studio
cargo run --release
```

Run the measurements headlessly. `PROTO_SCALE` sets the display scale factor and `PROTO_SNAPSHOT_DIR` saves pictures of both windows.

```bash
PROTO_BENCH=1 PROTO_SCALE=1.5 cargo run --release
```

The bench reports startup, first render, memory, the cost of the renderer hand-off and of a whole-window software render during a 300-step drag, and how far the Slint box is from the Rust layer's edges at five zoom levels.

Not covered: real mouse drag feel, frame pacing on screen, idle CPU, and Wayland.
