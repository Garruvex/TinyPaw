# TinyPaw on Niri: build and manual test script

This port draws through wlr-layer-shell overlays (see [architecture: Linux on Niri](architecture.md#linux-on-niri-layer-shell-route)). It has been compiled and unit-tested for `x86_64-unknown-linux-gnu`, but nobody has run it on a real Niri session yet. Work through this script on real hardware and record the results at the bottom.

Not ported yet: the tray icon, Freeze and Lens capture, and reduced-motion detection. Freeze falls back to the live scene, and Lens has no capture source.

## 1. Build

```sh
# Debian/Ubuntu names; on Arch: wayland libxkbcommon alsa-lib pkgconf
sudo apt install build-essential pkg-config libwayland-dev libxkbcommon-dev libasound2-dev
cargo build --release
install -Dm755 target/release/tinypaw ~/.local/bin/tinypaw
```

Check the build:

```sh
cargo test                      # must pass, including platform::wayland::tests
cargo clippy --all-targets
```

## 2. Niri configuration

Add this to `~/.config/niri/config.kdl`. Use the absolute path if `~/.local/bin` is not on the PATH Niri sees.

```kdl
spawn-at-startup "tinypaw"

binds {
    // Master toggle. TinyPaw also ignores repeats.
    Mod+Alt+P repeat=false { spawn "tinypaw" "toggle"; }
    // Recovery: always ends the session, safe to press twice.
    Mod+Alt+Escape repeat=false { spawn "tinypaw" "off"; }
    // Niri binds fire on press only, so hold-to-draw uses two keys.
    Mod+Alt+D repeat=false { spawn "tinypaw" "hold-begin"; }
    Mod+Alt+Shift+D repeat=false { spawn "tinypaw" "hold-end"; }
    // Latch annotating until Escape.
    Mod+Alt+L repeat=false { spawn "tinypaw" "latch"; }
    Mod+Alt+Comma repeat=false { spawn "tinypaw" "settings"; }
}
```

**Do not** add a `layer-rule` with `block-out-from "screencast"` or `"screen-capture"` for namespace `tinypaw`. That would hide the marks from the Discord viewer, which is the whole point. To check the rules Niri applied, run `niri msg layers`. Each selected output should show a `tinypaw` surface on the Overlay layer while TinyPaw is On.

Reload with `niri msg action load-config-file` (or restart Niri), then confirm only one instance is running:

```sh
tinypaw            # second start prints: TinyPaw is already running …
tinypaw status     # ok: TinyPaw is Off
```

To get a trace for any failure below, start it as `TINYPAW_TRACE=1 tinypaw 2>&1 | tee /tmp/tinypaw.log`.

## 3. Core checks

Each step lists the expected result. Note any deviation.

1. **Settings without overlays.** Run `tinypaw settings`. A normal window opens. Display shows one entry per monitor with its connector name (`DP-1`, `eDP-1`, …). The output at the global origin is marked primary.
2. **Toggle.** Press Mod+Alt+P. `tinypaw status` reports On (passthrough). Run `niri msg layers` and confirm a `tinypaw` overlay on each selected output.
3. **Click-through in passthrough.** Click, scroll and type into the window under the overlay. Everything reaches that window, including at pixels where nothing is drawn.
4. **Annotate and Escape.** Press Mod+Alt+L and draw a stroke. Only the overlay under the pointer has keyboard focus: typed letters do not reach the app below. Press Escape. The session returns to passthrough, the stroke stays visible, and keyboard focus returns to the app below without a click.
5. **Hold edges.** Press Mod+Alt+D, draw, then press Mod+Alt+Shift+D. You are back in passthrough. Press hold-end again, and press hold-begin twice in a row: neither causes a stuck or extra state.
6. **Off recovery.** While annotating, press Mod+Alt+Escape. All overlays close and `tinypaw status` reports Off. Press it again: still Off, with no error.
7. **Multiple outputs.** Set Display to *All displays* and draw on each output. Each output keeps its own marks, and the paw appears only on the output under the pointer.
8. **Hotplug.** With TinyPaw On, unplug or disable an output (`niri msg output <name> off`). The session ends and settings say the displays changed. Re-enable the output and choose a display again. TinyPaw turns On normally.
9. **Fractional scale.** Set an output to `scale 1.5` in the Niri config. Strokes land under the claw, and clicks in step 3 still pass through.
10. **Overview and workspaces.** Open the Niri overview while On. The overlay stays on top (overlay layer) and does not move with workspaces.

## 4. Fullscreen-slide test

1. Open a slide deck that has a hyperlink and start its fullscreen show, for example LibreOffice Impress *Slide Show → Start from First Slide*, or a browser deck in F11 fullscreen. Confirm with `niri msg windows` that the window is fullscreen.
2. Press Mod+Alt+P, then Mod+Alt+L. The paw and toolbar draw **above** the fullscreen slide. Draw a circle around the link.
3. Press Escape. The circle stays visible. Click the link. It opens normally on the first click, with no swallowed click and no extra click.
4. Press Mod+Alt+L again and tap the link once while annotating. It does **not** open, because TinyPaw owns input.
5. Advance slides with the keyboard while in passthrough. Keys reach the presentation.
6. Press Mod+Alt+Escape and exit the slideshow. No TinyPaw surface is left behind (`niri msg layers`).

## 5. Remote Discord viewer test

Needs a second Discord account on another device, which is the viewer.

1. On Niri, open Discord (desktop or browser) and start *Share screen*. Pick the **monitor** that will show the slide or game in the portal dialog, not a single window. Niri screencasts go through xdg-desktop-portal-gnome, and `xdg-desktop-portal-gnome` must be running.
2. Put the fullscreen slide (section 4) or a game on that monitor. Toggle TinyPaw On, latch, and draw a stroke and a pin.
3. **Viewer:** the paw, stroke and pin appear in the stream, aligned with the content, within about a second of being drawn locally.
4. Press Escape. The viewer still sees the marks in passthrough. Undo (Ctrl+Z while annotating) and Clear (X) disappear for the viewer too.
5. Press Mod+Alt+Escape. The viewer sees the marks vanish, and no black or frozen rectangle remains where the overlay was.
6. Note any capture border, flicker or lag that the viewer sees. Also note whether the game paused, muted or kept receiving mouse input while TinyPaw had focus.

## 6. Unsupported compositor

On a Wayland session without layer-shell (for example GNOME/Mutter):

1. Start `tinypaw`. Settings open as a normal window and explain that layer-shell is missing. This message appears before any activation.
2. `tinypaw toggle` prints `error: This compositor does not offer layer-shell…` and exits non-zero. `tinypaw status` still reports Off.
3. In an X11-only session, `tinypaw` exits with *needs a Wayland session such as Niri*.

## Results

| Date | Niri version | GPU / outputs | Sections passed | Notes |
| --- | --- | --- | --- | --- |
| | | | | |
