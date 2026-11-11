# TinyPaw
A paw version pointer to point and draw for presentation

TinyPaw is a desktop helper for pointing and drawing over presentations and game streams. See the [scope](docs/scope.md), [architecture](docs/architecture.md) and [measurements](docs/measurements.md).

The [next-phase plan](docs/next-phase.md) covers the menu redesign, desktop Lens, character imports, interaction and music response.

## Status: alpha (Windows)

Working on Windows:

- **Overlay:** transparent overlays on the primary display, a named display or all displays with your Yohta, Moro, Moro Crayon or Zaeri pointer art (Furry Paw, Stretch Paw or Classic Stick) or a laser dot, ink in six colours and four widths, optional fade after 3 seconds, and a tap animation. Stretch Paw bends the pack's forearm PNG, preserving a short wrist section. Packs without forearm art use a curved limb in their accent colour.
- **Paw-print stamps:** Settings > Pointer > Tap stamp offers Off, Cat or Dog. Tap with any paw pointer to leave a print in your ink colour; dragging still draws. Prints follow ink fading and support Undo and Clear.
- **Quick bar and wheel:** the Graphite quick bar holds Pointer, Text, Pin, ink, Undo and End annotation, with Spotlight, Lens, Freeze, Clear marks and tap mute under **More**. Tooltips show shortcuts. The quick wheel opens with a held **Ctrl+Alt+W**; its up-left wedge is Undo, so a mis-flick can't wipe your marks.
- **Settings window:** Pointer (with a live preview that stays in view), Ink & text and Spotlight & lens under Annotate; Character (Library, On desktop, Artwork, Behavior, Advanced) under Desktop; Shortcuts, Sound and Display & motion under App; and an Annotation switch in the header. Choices are saved to `%APPDATA%\TinyPaw\preferences.json`.
- **Sound:** Blackboard Knock, Bubble Pop, Rubber Bonk, Duck Toy, Mac Quack, Bonk and Dry Fart, plus a pack-provided tap or an imported WAV, with preview, volume and mute. All built-in sounds ship inside the executable.
- **Activation:** Toggle and Hold-to-draw styles, a tray menu, a first-run card, and a CLI that forwards to the running instance.
- **Tools:** Text labels (IME-aware: the first Enter accepts a candidate, the next places the label), numbered Pins, Spotlight (soft-edged, with adjustable size and darkness), and Freeze (**Ctrl+Alt+F**) to draw on a still of the screen, with Return Live. Lens magnifies 1.5×, 2×, 3×, 4× or 6× in a round lens beside the pointer. Under Annotation → Lens → Advanced choose **Desktop under pointer** (whatever app is under the paw; TinyPaw stays out of the lens but not out of a screen share) or **Lock to a window**. **Ctrl+Alt+Z** (Lens) and **Ctrl+Alt+X** (Spotlight) work anywhere, with annotation on or off: tap to toggle, or hold to show until you release. While holding either, **Up/Down** change the size and **+/−** the zoom (Lens) or darkness (Spotlight); the keys are only taken while you hold the shortcut. While annotating with Spotlight or Lens, scroll for size and Shift+scroll for zoom or darkness. Every change is saved.

- **Pointer packs:** built-in packs ship inside the executable. **Add yours** (Settings > Pointer) imports a transparent paw PNG. Click the tip, adjust the amber wrist marker with the position sliders, and optionally add a matching forearm PNG. Click its upper wrist joint, adjust the forearm width, and move or press in the pair preview before saving. Both images and their alignment are saved together in `%APPDATA%\TinyPaw\packs`. Copyable art prompts are on the Pointer page. To build a pack from large source art, run `python tools/prepare_pack.py`.

Settings > Controls lets you rebind shortcuts and choose Follow Windows, Full motion or Reduced motion. Hold **Alt** from Point to use the app underneath while committed marks remain visible; release Alt and all mouse buttons to resume. Hold to interact is disabled during a drag, Hold-to-draw and Freeze. Use the Annotate now tray action or `tinypaw latch` to resume explicitly.

Annotation holds ink, label size, colour and backing, Spotlight size and darkness, and Lens source/zoom/size. Pointer holds pressed-pose PNG and pack tap WAV import, whole-folder import, and removal/re-aiming of installed pointers. Ctrl+Z in a text draft edits the draft; the annotation history stays intact.

Windows lock, suspend and display topology/DPI changes end the session and release capture. Re-select a missing monitor under Display before activation. Source-window capture avoids recursive magnification and leaves the overlays visible to monitor sharing; do not mark TinyPaw capture-excluded.

Aamox's approved pointer art and the exact Teddy Squeak reference are still outstanding. Niri and macOS native capture/input integrations remain unimplemented. Game/Discord capture, mixed-DPI hardware, lock/suspend and global-key release behavior still require the live release playtest in [release validation](docs/release-validation.md).

## Build and run

```bash
cargo build --release
```

```bash
target/release/tinypaw
```

TinyPaw starts Off in the tray. Left-click the tray icon, tick **Annotation enabled** in its menu, or press **Ctrl+Alt+P** to switch it On. The tray menu also has Annotate now, Show character, Settings and Quit TinyPaw; Freeze is on the quick bar's More panel and **Ctrl+Alt+F**.

| Input | Action |
| --- | --- |
| Ctrl+Alt+P | Master On/Off |
| Click | Tap with the paw |
| Drag | Draw |
| Ctrl+Z / X | Undo / Clear marks |
| P / R | Paw / Laser |
| T / N / S / L | Text / Pin / Spotlight / Lens |
| M | Mute or unmute the tap sound |
| Escape | End annotation (switches Off and clears marks) |
| Alt (held, from Point) | Interact with the app underneath; marks stay visible |
| Ctrl+Alt+D (held) | Point and draw in Hold-to-draw mode |
| Ctrl+Alt+W (held) | Quick wheel: point at a tool, release to pick |
| Ctrl+Alt+F | Freeze the screen and draw on the still; again to return live |
| Ctrl+Alt+Z | Lens anywhere, annotating or not: tap toggles, hold shows until release |
| Ctrl+Alt+X | Spotlight anywhere, the same way; clicks still reach the app underneath |
| Ctrl+Alt+Up/Down, +/− (while holding Z or X) | Size; zoom (Lens) or darkness (Spotlight) |

## CLI

### Desktop character (Windows, experimental)

Turn on Yohta under **Settings → Character → On desktop**, with **Show character** in the tray menu, or with `tinypaw mascot` (`tinypaw mascot-hide` hides him). He sits on the desktop and his eyes follow the cursor. Right-click him for a menu with Annotation enabled, Character settings… and Hide character; eye tracking is in Character → On desktop and tap mute in Sound. Click him and he reacts; drag him and he leans with the motion while his tail trails. Settings → Character can add your own character from one PNG, or import a folder with separate moving parts. Blank pixels around him pass clicks through to the app underneath, and clicking him never takes keyboard focus. He stays while TinyPaw is Off and steps aside while the annotation surface owns input. Size, motion and position are saved.

`cargo run -- --mascot-preview` still opens the standalone rig preview with a simulated music beat. Custom character imports, the standing pose and real music response are still pending; see [mascot prototype](docs/mascot-prototype.md) and [desktop character and custom import feature](docs/mascot-feature.md).

`tinypaw <command>` talks to the running instance: `status`, `toggle`, `off`, `latch`, `undo`, `clear`, `point`, `text`, `pin`, `spotlight`, `spotlight-begin`, `spotlight-end`, `lens`, `lens-begin`, `lens-end`, `freeze`, `paw`, `laser`, `mute`, `settings`, `hold-begin`, `hold-end`, `quit`, and `set <setting> <value>`. For example, to switch to Hold-to-draw:

```bash
tinypaw set system.activation_style hold
```

Set `TINYPAW_TRACE=1` to log every event the resident instance handles.
