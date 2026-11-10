# TinyPaw measurements

Release-build results recorded against the targets in [scope](scope.md#footprint-animation-and-ease-of-use). Add a row each time a tool lands, and keep the earlier rows for comparison.

## Phase 1 vertical slice, 10 November 2026

Machine: Windows 11 Home 10.0.26200, 1920×1080 primary display at 100% scaling. Build: `cargo build --release` (thin LTO, stripped). Memory is the process working set and private bytes as reported by PowerShell `Get-Process`. Activation time is measured from sending `tinypaw toggle` until `tinypaw status` reports annotating. It includes two CLI process launches, about 70–80 ms of the total.

| Scenario | wgpu renderer | tiny-skia renderer (chosen) | Target |
| --- | --- | --- | --- |
| Idle in tray, Off, cold start | 13 MiB WS / 4 MiB private, 0.000% CPU over 10 s | 13 MiB WS / 4 MiB private, 0.000% CPU over 10 s | < 50 MiB, < 0.1% CPU |
| Annotating, overlay open | 169 MiB WS / 228 MiB private | 28 MiB WS / 15 MiB private | — |
| Idle after Off (overlay used once) | 127 MiB WS / 183 MiB private, 0.000% CPU | 18 MiB WS / 5 MiB private, 0.000% CPU | < 50 MiB, < 0.1% CPU |
| First activation | 818 ms | 143 ms | Visible paw < 100 ms p95 |
| Later activations | 639 ms, 684 ms | 142 ms, 151 ms | Visible paw < 100 ms p95 |
| Overlay focused after `toggle` (one CLI launch) | — | 78–91 ms | — |
| CPU while drawing a ~1.2 s stroke and tap | — | 31 ms total | p95 update within 3.1 ms at 160 Hz |
| Release executable | 10.8 MiB | 4.4 MiB | < 25 MiB compressed download |

**Decision.** Use tiny-skia for the overlay. wgpu creates a GPU surface each time the overlay opens, which cost 640–820 ms per activation. It also kept about 180 MiB private memory after Off. tiny-skia meets the idle footprint target, and its transparency and zero-alpha rendering work on Windows 11. The `gpu` Cargo feature keeps wgpu available for later comparisons.

**Not yet measured.** Per-frame update and submission times, presented frame intervals and missed refreshes at 160 Hz and 60 Hz, game p95 frame time with TinyPaw Off, drawing, and Lens. Mixed-refresh and mixed-DPI displays. Activation time excluding CLI launch overhead; measure it from the hotkey with a frame timestamp.

## Interface build (Graphite quick bar, wheel, settings), 10 November 2026

Same machine and method as above, tiny-skia renderer. The build adds SVG icons, rodio tap playback and native file dialogs.

| Scenario | Result | Target |
| --- | --- | --- |
| Idle in tray, Off, cold start | 12 MiB WS / 2 MiB private, 0.000% CPU over 10 s | < 50 MiB, < 0.1% CPU |
| Annotating, quick bar shown | 28 MiB WS / 14 MiB private | — |
| Idle after Off | 19 MiB WS / 4 MiB private, 0.000% CPU over 10 s | < 50 MiB, < 0.1% CPU |
| Activation (incl. CLI polling) | 106 ms, 69 ms, 104 ms | Visible paw < 100 ms p95 |
| Release executable | 7.5 MiB | < 25 MiB compressed download |

The audio device opens on the first tap after switching On and closes on Off, so the idle process holds no audio stream. Ctrl+Alt+Space was already registered by another app on the test machine, so the wheel defaults to Ctrl+Alt+W. Settings > Controls now marks any shortcut another app holds.

## Phase 1 behavior checks (scripted on Windows 11)

Passed with synthetic input on the primary display:

- Toggle mode: Ctrl+Alt+P and `tinypaw toggle` open Point on a transparent topmost overlay. Drag draws from the original press, a click taps without drawing, Ctrl+Z undoes, X clears, and Escape ends the session. Focus returns to the previously active app.
- Hold-to-draw mode: Ctrl+Alt+D held acquires input. Releasing it commits the stroke, hides the paw and toolbar, and leaves the mark click-through, with focus back on the previous app. Releasing the key before the mouse button keeps input until the button-up, which is swallowed, and only then returns focus.
- CLI: `status`, refusal of unavailable commands (`undo` while Off exits 1 with a reason), single-instance detection and `quit`.

Still required before advertising the Discord workflow, per the build order:

- Remote-viewer test in a Discord monitor share using a second account or device, with a borderless game.
- Game focus behavior: record whether each test game pauses, mutes, opens a menu or keeps mouse input while TinyPaw has focus. A pointer-lock game exits mid-drag, and the next gameplay click and camera movement must behave normally.
- Clicking a slide hyperlink while active, then opening it normally after exit.
- Mixed-DPI and negative monitor origins. Phase 1 covers the primary display only.
- Repeating the slice on Niri.
