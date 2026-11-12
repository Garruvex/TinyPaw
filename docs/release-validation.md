# Windows release validation — 10 November 2026

Implemented in this working tree: Stretch Paw asset/anchor import and bending; pressed poses and pack default tap audio; whole-pack import and installed-pack removal/re-aiming; live DWM window Lens with source selection, 2×/4× and size; hold to interact; shortcut rebinding and conflict rollback; named/all-monitor overlays; native lock/suspend/display notifications; Windows reduced-motion preference and override; label colour/backing/size, Spotlight size and draft text undo.

## Checks completed

- Continuation check: all 64 unit tests pass and `cargo build --release` succeeds. The executable is `target/release/tinypaw.exe` (9,054,208 bytes).
- Clippy and formatting passed in the earlier session. Rechecking in the current session is blocked by `Access is denied` during configuration discovery; this is not a fresh lint/format pass.
- Unit tests cover session cancellation, input handoff, gameplay-button release waits, draft undo and captured label styling, reduced motion, shortcut conflict rejection and saved key swaps, negative monitor selection, and self-contained pack imports with pressed PNG/WAV and path rejection.
- The Windows test build opened Settings, selected All displays, activated an overlay and returned Off with Escape. Lens with no source returned to Point and displayed a source-selection message.
- Test settings were isolated from the user's normal preferences.

## Release gates still open

- Confirm a moving borderless game is magnified live, with the paw/ink/lens excluded from its source and all three visible to a Discord monitor-share viewer. DWM renders a source-window crop directly, without a CPU frame-copy loop. Protected, minimised, exclusive-fullscreen and non-composited sources are unsupported; failed sources return to Point. A game may pause on focus loss.
- Exercise the source picker, source closure, edge flipping and the 2×/4×/size controls on the real game. Window handles are session-specific: refresh/reselect after the source is restarted.
- Verify global shortcut registration/rebinding/release, Alt+Tab, and mouse-button handoff outside the sandbox. Global registration is rejected with Access denied in this execution environment.
- Test multiple physical monitors at different scaling, left/above origins, hot-unplug and DPI changes. Native placement uses physical display rectangles; ink and text remain local logical coordinates per window.
- Verify actual lock, suspend/resume and display disconnection on hardware. These stop the session and require a fresh activation.
- Confirm reduced-motion follows Windows and both overrides, including a held pose and abrupt ink expiry.
- Import a pack through the file picker, move the original, restart and test pressed art and audio; re-aim and remove it.
- Obtain the approved Aamox pointer asset locally. Its Library download was unavailable during this task. Do not substitute new art without review.
- Confirm Teddy Squeak's exact clip, item or mod before producing an original sound.

## Commit blocker

The session can edit source and run builds/tests but `.git` is explicitly read-only. `git add` and `git commit` failed creating `.git/index.lock` with Permission denied. No feature commits were created. The working tree retains all changes; commits require a session with write access to Git metadata.
