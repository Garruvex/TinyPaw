# TinyPaw
A paw version pointer to point and draw for presentation

TinyPaw is a desktop helper for pointing and drawing over presentations and game streams. See the [scope](docs/scope.md), [architecture](docs/architecture.md) and [measurements](docs/measurements.md).

## Status: alpha, Phase 1 (Windows vertical slice)

Working on Windows: a tray icon, the global On/Off shortcut, a transparent overlay on the primary display, a placeholder paw, cyan ink, tap animation, Undo, Clear and Exit. Toggle and Hold-to-draw activation styles are supported, along with a CLI that forwards to the running instance. Sound, pointer packs, settings UI, Niri and macOS come in later phases.

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

## CLI

`tinypaw <command>` talks to the running instance: `status`, `toggle`, `off`, `latch`, `undo`, `clear`, `point`, `hold-begin`, `hold-end`, `quit`, and `set <setting> <value>`. For example, to switch to Hold-to-draw:

```bash
tinypaw set system.activation_style hold
```

Settings are not saved between launches yet.

Set `TINYPAW_TRACE=1` to log every event the resident instance handles.
