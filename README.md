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

The build produces two programs. `tinypaw.exe` is the console CLI and the resident instance. `tinypawd.exe` is a small GUI-subsystem launcher that starts it without a console window; the Start-menu shortcut and the sign-in entry use it, and launching it while TinyPaw runs opens Settings.

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

## Windows installer

The installer is an [Inno Setup 6](https://jrsoftware.org/isinfo.php) script in [`installer/tinypaw.iss`](installer/tinypaw.iss). Install Inno Setup once:

```bash
winget install --id JRSoftware.InnoSetup -e --scope user
```

Then build the release binaries and the installer from the repository root:

```bash
cargo build --release
```

```bash
"$LOCALAPPDATA/Programs/Inno Setup 6/ISCC.exe" installer/tinypaw.iss
```

This writes `target/installer/TinyPaw-0.1.0-setup.exe`. Pass `/DAppVersion=x.y.z` to ISCC to stamp another version, or `/DBinDir=<folder>` to package binaries from elsewhere.

The installer is per-user and needs no admin rights:

- Files go to `%LOCALAPPDATA%\Programs\TinyPaw`, and a **TinyPaw** Start-menu shortcut runs `tinypawd.exe`.
- The **Add the tinypaw command to my PATH** task (on by default) appends that folder to the user `PATH`, so new terminals can run `tinypaw status`. If you untick it, run the CLI by full path, or add the folder yourself.
- **Settings > Controls > Start TinyPaw when I sign in** (off by default) writes a `TinyPaw` value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` pointing at `tinypawd.exe`. It is also available as `tinypaw set system.start_at_sign_in true`.
- Uninstall (Settings > Apps) quits the running instance and removes the files, shortcut, `PATH` entry and Run value. It then asks whether to delete `%APPDATA%\TinyPaw` (settings and imported pointers); the default is to keep it. A silent uninstall (`/VERYSILENT`) always keeps it.

## CLI

`tinypaw <command>` talks to the running instance: `status`, `toggle`, `off`, `latch`, `undo`, `clear`, `point`, `text`, `pin`, `spotlight`, `lens`, `freeze`, `paw`, `laser`, `mute`, `settings`, `hold-begin`, `hold-end`, `quit`, and `set <setting> <value>`. For example, to switch to Hold-to-draw:

```bash
tinypaw set system.activation_style hold
```

Set `TINYPAW_TRACE=1` to log every event the resident instance handles.
