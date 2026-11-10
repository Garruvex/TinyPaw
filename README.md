# TinyPaw
A paw version pointer to point and draw for presentation

TinyPaw is a desktop helper for pointing and drawing over presentations and game streams. See the [scope](docs/scope.md), [architecture](docs/architecture.md) and [measurements](docs/measurements.md).

## Status: alpha (Windows)

Working on Windows:

- **Overlay:** a transparent overlay on the primary display with a placeholder Yohta paw or a laser dot, ink in six colours and four widths, optional fade after 3 seconds, and a tap animation.
- **Quick bar and wheel:** the Graphite quick bar holds the tools, ink, Undo, Clear, Mute and Exit, with shortcut tooltips. The quick wheel opens with a held **Ctrl+Alt+W**.
- **Settings window:** Controls, Display, Appearance and Sound sections with a live preview and a master switch. Choices are saved to `%APPDATA%\TinyPaw\preferences.json`.
- **Sound:** three synthesized tap sounds (Blackboard Knock, Bubble Pop, Rubber Bonk) or an imported WAV, with volume and mute.
- **Activation:** Toggle and Hold-to-draw styles, a tray menu, a first-run card, and a CLI that forwards to the running instance.
- **Tools:** Text labels (IME-aware: the first Enter accepts a candidate, the next places the label), numbered Pins, Spotlight, and Freeze (**Ctrl+Alt+F**) to draw on a still of the screen, with Return Live.

Lens appears in the interface but is not built yet. Pointer packs and Add your pointer, Stretch Paw, shortcut rebinding, multiple monitors, Niri and macOS also come later. The interface design is the Graphite canvas (TinyPaw Interface artifact).

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
| Ctrl+Alt+D (held) | Point and draw in Hold-to-draw mode |
| Ctrl+Alt+W (held) | Quick wheel: point at a tool, release to pick |
| Ctrl+Alt+F | Freeze the screen and draw on the still; again to return live |

## CLI

`tinypaw <command>` talks to the running instance: `status`, `toggle`, `off`, `latch`, `undo`, `clear`, `point`, `text`, `pin`, `spotlight`, `freeze`, `paw`, `laser`, `mute`, `settings`, `hold-begin`, `hold-end`, `quit`, and `set <setting> <value>`. For example, to switch to Hold-to-draw:

```bash
tinypaw set system.activation_style hold
```

Set `TINYPAW_TRACE=1` to log every event the resident instance handles.
