# TinyPaw
A paw version pointer to point and draw for presentation

TinyPaw is a desktop helper for pointing and drawing over presentations and game streams. See the [scope](docs/scope.md), [architecture](docs/architecture.md) and [measurements](docs/measurements.md).

## Status: alpha (Windows)

Working on Windows:

- **Overlay:** transparent overlays on the primary display, a named display or all displays with your Yohta, Moro, Moro Crayon or Zaeri pointer art (Furry Paw, Stretch Paw or Classic Stick) or a laser dot, ink in six colours and four widths, optional fade after 3 seconds, and a tap animation. Stretch Paw bends the pack's forearm PNG, preserving a short wrist section. Packs without forearm art use a curved limb in their accent colour.
- **Quick bar and wheel:** the Graphite quick bar holds the tools, ink, Undo, Clear, Mute and Exit, with shortcut tooltips. The quick wheel opens with a held **Ctrl+Alt+W**.
- **Settings window:** Controls, Display, Appearance and Sound sections with a live preview and a master switch. Choices are saved to `%APPDATA%\TinyPaw\preferences.json`.
- **Sound:** three synthesized tap sounds (Blackboard Knock, Bubble Pop, Rubber Bonk) a pack-provided tap or an imported WAV, with volume and mute.
- **Activation:** Toggle and Hold-to-draw styles, a tray menu, a first-run card, and a CLI that forwards to the running instance.
- **Tools:** Text labels (IME-aware: the first Enter accepts a candidate, the next places the label), numbered Pins, Spotlight, and Freeze (**Ctrl+Alt+F**) to draw on a still of the screen, with Return Live. Lens shows a live 2× or 4× window crop; choose its source under Appearance or activate from the target game window.

- **Pointer packs:** built-in packs ship inside the executable. **Add yours** (Settings > Appearance) imports a transparent paw PNG. Click the tip, adjust the amber wrist marker with the position sliders, and optionally add a matching forearm PNG. Click its upper wrist joint, adjust the forearm width, and move or press in the pair preview before saving. Both images and their alignment are saved together in `%APPDATA%\TinyPaw\packs`. Copyable art prompts are available in Appearance. To build a pack from large source art, run `python tools/prepare_pack.py`.

Settings > Controls lets you rebind shortcuts and choose Windows, full or reduced motion. Hold **Alt** from Point to use the app underneath while committed marks remain visible; release Alt and all mouse buttons to resume. Hold to interact is disabled during a drag, Hold-to-draw and Freeze. Use the Annotate now tray action or `tinypaw latch` to resume explicitly.

Appearance exposes label size, colour and backing, Spotlight radius, Lens source/zoom/size, pressed-pose PNG and pack tap WAV import, whole-folder import, and removal/re-aiming of installed pointers. Ctrl+Z in a text draft edits the draft; the annotation history stays intact.

Windows lock, suspend and display topology/DPI changes end the session and release capture. Re-select a missing monitor under Display before activation. Source-window capture avoids recursive magnification and leaves the overlays visible to monitor sharing; do not mark TinyPaw capture-excluded.

Aamox's approved pointer art and the exact Teddy Squeak reference are still outstanding. Niri and macOS native capture/input integrations remain unimplemented. Game/Discord capture, mixed-DPI hardware, lock/suspend and global-key release behavior still require the live release playtest in [release validation](docs/release-validation.md).

## Build and run

```bash
cargo build --release
```

```bash
target/release/tinypaw
```

TinyPaw starts Off in the tray. Left-click the tray icon or press **Ctrl+Alt+P** to switch it On.

| Input | Action |
| --- | --- |
| Ctrl+Alt+P | Master On/Off |
| Click | Tap with the paw |
| Drag | Draw |
| Ctrl+Z / X | Undo / Clear |
| Escape | Exit (switches Off and clears marks) |
| Alt (held, from Point) | Interact with the app underneath; marks stay visible |
| Ctrl+Alt+D (held) | Point and draw in Hold-to-draw mode |
| Ctrl+Alt+W (held) | Quick wheel: point at a tool, release to pick |
| Ctrl+Alt+F | Freeze the screen and draw on the still; again to return live |

## CLI

### Experimental Yohta mascot preview

Run `cargo run -- --mascot-preview` (or `target/debug/tinypaw.exe --mascot-preview`) to try the approved sitting mascot in a separate preview window. Move the cursor inside the preview to steer the eyes and click Yohta to open the sample menu. Controls include blinking/idle motion, a simulated music beat with tempo, size, and light/dark backgrounds.

The menu is a demonstration; desktop dragging, real music detection, the standing pose, and annotation-app integration are still pending. See [mascot prototype](docs/mascot-prototype.md) and the proposed [desktop character and custom import feature](docs/mascot-feature.md).

`tinypaw <command>` talks to the running instance: `status`, `toggle`, `off`, `latch`, `undo`, `clear`, `point`, `text`, `pin`, `spotlight`, `lens`, `freeze`, `paw`, `laser`, `mute`, `settings`, `hold-begin`, `hold-end`, `quit`, and `set <setting> <value>`. For example, to switch to Hold-to-draw:

```bash
tinypaw set system.activation_style hold
```

Set `TINYPAW_TRACE=1` to log every event the resident instance handles.
