# Mascot engine study and Yohta artwork plan

Research date: 11 November 2026. This adds implementation recommendations to
[the mascot feature scope](mascot-feature.md). It is not a claim that the custom
pack importer or live music response has shipped.

## Direction

Use a small 2D layered character in TinyPaw's existing resident process.
Character artwork, animation state and native desktop input are separate owners.
The same pack and rig must drive the editor preview and desktop character.

The two new artwork studies produced with this pass establish:

- Sitting and standing Yohta: a large angular head, tiny body, three-quarter
  view toward his right, yellow/orange crest, cyan eyes and blue/navy/white fur.
- Neutral, blink, pleased-click and surprised-drag head concepts.

These are appearance studies, not aligned animation assets. The running
prototype still uses its existing approved source sheet until production
layers are prepared and validated. A generated parts board must not be treated
as an automatically registered rig.

## What was studied

### MateEngine: source-backed findings

The source review used repository tree
`2c5ea6b8f4cf5e1773a0816b46d9267cda5174d4`, rather than relying on a feature
comparison or third-party summary. Relevant files:

- [AvatarAnimatorController.cs](https://github.com/shinyflvre/Mate-Engine/blob/2c5ea6b8f4cf5e1773a0816b46d9267cda5174d4/Assets/MATE%20ENGINE%20-%20Scripts/AvatarHandlers/AvatarAnimatorController.cs)
- [AvatarMouseTracking.cs](https://github.com/shinyflvre/Mate-Engine/blob/2c5ea6b8f4cf5e1773a0816b46d9267cda5174d4/Assets/MATE%20ENGINE%20-%20Scripts/AvatarHandlers/AvatarMouseTracking.cs)
- [AvatarWindowHandler.cs](https://github.com/shinyflvre/Mate-Engine/blob/2c5ea6b8f4cf5e1773a0816b46d9267cda5174d4/Assets/MATE%20ENGINE%20-%20Scripts/AvatarHandlers/AvatarWindowHandler.cs)
- [SettingsHandlerAudio.cs](https://github.com/shinyflvre/Mate-Engine/blob/2c5ea6b8f4cf5e1773a0816b46d9267cda5174d4/Assets/MATE%20ENGINE%20-%20Scripts/Settings/SettingsMenu/SettingsHandlerAudio.cs)

| Source observation | TinyPaw recommendation |
| --- | --- |
| Animator handling separates idle, dragging and dancing; movement/menu blocking can stop drag/dance behaviour. Idle/dance changes use timed transitions. | Explicit interaction ownership, with short settling transitions. A drag must never also count as a click. |
| Tracking permissions vary by animation state. Eye/head rotation uses local coordinates, bounds and smoothing. | Masked pupil tracking inherits the head transform. Disable inappropriate reactions during drag/menu ownership. |
| The reviewed animator checks allowed playback sessions against a peak-volume threshold. This is activity detection, not proof of beat synchronisation. | Implement amplitude response first and label it accurately. Treat later tempo tracking as a separate signal. |
| Window sitting handles desktop rectangles, hidden/minimised windows and snapping. | Keep placement/DPI recovery in the host. Defer window-edge sitting, which adds complexity unrelated to our core mascot. |
| Avatar runtime uses Unity/VRM and specialised avatar systems. | Borrow interaction principles while retaining TinyPaw's existing Rust/Iced 2D renderer. |

The public [README](https://github.com/shinyflvre/Mate-Engine) is useful for
product interaction, including its right-click/menu workflow. Its performance
claims are not our benchmarks. [Release notes](https://github.com/shinyflvre/Mate-Engine/releases)
also describe model-switching memory and positioning fixes: switching packs and
poses deserves explicit testing.

The repository's [published license](https://github.com/shinyflvre/Mate-Engine/blob/main/LICENSE.md)
contains source/distribution and asset restrictions. This study does not vendor
MateEngine code, shaders, models or animations. TinyPaw's implementation and art
remain its own.

### BUG Clicker: stronger evidence from developer releases

Its repository's current file tree exposed only a README, not application
source. However, the developer's release notes provide useful technical product
evidence. The following is documented behaviour, not a live test or a
reverse-engineered description of its renderer.

| Developer release | Verified documented behaviour | Lesson for TinyPaw |
| --- | --- | --- |
| [v1.19.4](https://github.com/BUG-INVADE/Bug-Clicker-download/releases/tag/v1.19.4) | A dedicated dragging face can be assigned through the image UI or a named image/PSD group. Its drag preview supports manually swinging the character. Fixes address drag continuity and resetting the head after release. | Named expression slots and an interactive drag preview are useful. Always settle back to the base pose after release. |
| [v1.19.3](https://github.com/BUG-INVADE/Bug-Clicker-download/releases/tag/v1.19.3) | Dragging can select a different face/body and use speed-dependent tilt. Parts can be fixed or hidden. Its music fix excludes the app's own sounds from beat detection. | Add an optional drag expression; keep it separate from blink/gaze. Exclude TinyPaw's own tap sound from audio analysis where possible. |
| [v1.19.2](https://github.com/BUG-INVADE/Bug-Clicker-download/releases/tag/v1.19.2) | Editor fixes cover part alignment under zoom and stale rectangular click maps after changing artwork. | Use one coordinate system for all layers and invalidate input masks whenever the visible silhouette changes. |

The earlier [BUG Clicker workflow study](bug-clicker-study.md) records additional
UI evidence and unanswered questions. The developer release notes above upgrade
some previous leads, notably PSD group assignment, dragging expressions and
part behaviour, to documented features. They still do not establish the entire
file schema or internal implementation. No protected file was decoded.

## The smallest useful runtime

Keep three independent pieces of state:

| State | Examples | Why separate |
| --- | --- | --- |
| Interaction | Idle, pressed, dragging, menu open, hidden | Owns input and whether movement is allowed. |
| Expression | Neutral, blink, pressed, optional drag | Selects artwork without replacing the entire animation system. |
| Motion inputs | Cursor target, playback level, elapsed time | Drives small transforms without stealing gesture ownership. |

Priority: hidden/interrupted, then active drag, then menu/press interaction,
then normal idle/music. Blinking is a temporary expression override; it must
not overwrite a drag face. Opening a menu stops decorative movement around the
click targets. Releasing a drag clears its velocity and settles the head/tail.

Use one transform chain: world placement, whole-body transform, attachment pivot,
part rotation, artwork. Eyes inherit the head transform and add a bounded pupil
offset inside their own masks. Convert cursor direction to head-local space.
Use elapsed-time easing so 30 Hz and 144 Hz hosts settle at the same speed.

For a speed-dependent drag reaction, calculate velocity from desktop displacement
and elapsed time, clamp it, and feed only a small tilt. It is a reaction to the
user moving the window, not an autonomous movement system.

## A workable Windows host

A small transparent top-level window is the first implementation target.
The visible sprite and its input silhouette must agree.

Microsoft documents per-pixel hit testing for layered windows in
[Window Features](https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features#layered-windows).
Do not assume that Iced's visually transparent swapchain automatically supplies
that behaviour: verify the actual window/rendering path on Windows.

If that path does not provide usable alpha input, a native window region is a
candidate. [SetWindowRgn](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowrgn)
also clips drawing, so it needs the complete current silhouette, including
moving parts. Rebuild only when the rasterised silhouette changes. Handle region
ownership and failures explicitly.

[WM_NCHITTEST](https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-nchittest)
documents that HTTRANSPARENT forwards within the same thread. It is not, by
itself, a general solution for clicking a different app behind the mascot.

Regardless of the chosen native method:

- Begin a gesture only on visible character pixels.
- Once dragging, retain capture through release even if the pointer stops or
  moves into an otherwise transparent area.
- Keep the menu in a separate small owned surface or include its full region;
  it must not disappear through the character's alpha mask.
- Recalculate coordinates for DPI/monitor changes; clamp the ground anchor to
  a display work area after topology changes.
- Test real clicks into another app through the empty sprite canvas. A
  screenshot alone cannot verify input passthrough.

## Music that remains small and predictable

Start with optional Windows playback metering and an amplitude envelope. If
samples are needed, Microsoft's
[WASAPI loopback documentation](https://learn.microsoft.com/en-us/windows/win32/coreaudio/loopback-recording)
describes capture of a playback endpoint. Neither design needs microphone input.

Show an input meter and silence/unavailable state. Apply attack/release smoothing
and separate start/stop thresholds to avoid jitter. Map level into bounded head
tilt and body bounce; silence returns smoothly to idle. Device changes should
reset the envelope and reconnect without moving the character.

Prefer playback-session metering excluding our process for a simple response.
If a whole-output sample stream includes TinyPaw's tap, an own-sound suppression
window is only an approximation; do not claim it fully isolates the source.
Beat detection and BPM display need separate tests before being advertised.
The existing preview's tempo slider remains explicitly a simulated beat.

## Turn the new Yohta studies into a usable pack

Use a 1024-square transparent canvas for each production pose, with the head at
the same authored scale. Align at the ground anchor, not at the image's outer
bounding box. The generated studies may need geometric adjustment to do this.

| Export | Artwork requirement |
| --- | --- |
| Body | Tiny torso/limbs with neck overlap; no duplicated head or tail. |
| Head base | Full crest/ears/muzzle with clean eye sockets; enough neck overlap for a few degrees of tilt. |
| Eye whites and masks | Same registration as the head; each mask defines a pupil's allowed area through alpha. |
| Irises/pupils | Separate movable elements, sized for the near/far eye perspective. |
| Blink overlays | Matching fur-backed closed eyelids, covering the sockets cleanly. |
| Tail | Complete root-to-tip image, drawn behind the body with a root pivot. |
| Optional expression faces | Click/drag variants registered to the same neck, ears and eye positions. |

Keep eyes in one of two explicit modes per expression: tracking overlays or
eyes painted into the expression. Never draw tracking pupils over a face that
already contains them. Blinking hides/replaces the normal eye group.

Quick-import users still need only one PNG. They get click/drag and whole-image
motion. Independent tracking and articulation require the matching layered
assets; a flattened character cannot supply those automatically.

An expression-slot extension is a recommendation for the pack design, not an
already-supported field in the proposed version 1 manifest. Finalise it with the
pack loader before claiming a file compatible.

## Build and verify in this order

1. Finish the native character surface and gesture ownership. Prove alpha input
   and drag continuity against another app.
2. Author one complete sitting Yohta pack. Check gaze extremes, blink coverage
   and neck/tail seams at 96, 160 and 320 logical pixels.
3. Run that pack through the same loader as a custom single-PNG and layered
   import. Keep the current character active on failed import.
4. Add the standing pose and optional reaction faces using the same pack contract.
5. Add playback metering, an input meter and amplitude response, followed by
   device-loss and own-sound tests.

Measure idle/animated CPU and decoded-asset memory. Do not infer efficiency from
another project's marketing or introduce a second engine merely to animate
five small image groups. The native integration, importer and audio adapter
remain distinct deliverables even when a preview looks finished.
