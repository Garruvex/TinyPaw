# TinyPaw scope and architecture

Product scope, 10 November 2026. Stack selected. Capture behavior still requires live testing. Implementation and remaining release gates: [release validation](release-validation.md).

See the [system, engine and desktop layout](architecture.md) for the implementation map.

TinyPaw is a desktop helper for pointing, drawing, labeling and magnifying details for presentations and game streams. The first streaming workflow is a borderless game on a monitor shared in Discord. Press a shortcut, point with Yohta's paw or draw on the game, then press it again to resume play. A second participant must see the same paw and ink before this workflow is advertised. Users can install their own artwork and tap sound without compiling the app.

## First release

| Action | Expected behavior |
| --- | --- |
| Master on/off | Start Off. One rebindable global shortcut and one tray/menu-bar switch enable or disable TinyPaw as a whole. Show the current On or Off state in the tray/menu. On enters the selected activation style. Off ends the session, hides every overlay, clears marks, stops capture and tap playback, and returns input to the foreground app. Keep the resident process available for the next toggle. Quit is a separate tray action. |
| Point | Start with Yohta's paw. Switch to a small, high-contrast laser dot for precise targets or back to the selected paw. Keep both tips at the pointer, including during a tap. Switching pointer styles preserves the ink and focus. |
| Tap | Show a short paw press and play one preloaded sound. Consume the click so a slide or link underneath does not activate. |
| Draw | Once primary-button movement passes the drag threshold, start an ink stroke at the original press. Release ends the stroke. Classify a tap on release so drawing does not also play a tap sound. Both pointer styles draw with the same brush. |
| Text | Select Text, click a location, and type a short label. Enter places it and returns to Point. Give IME composition priority over Enter and Escape. Escape cancels a draft before it can exit the session. |
| Magnifier | Select Lens for a live enlarged crop next to the tip. Flip the lens near display edges. Switching back to Point closes the lens and keeps annotations. |
| Freeze | Press a rebindable shortcut to take one clean still of the target display and draw on it. Press it again to return live, which discards the still and its marks. Use it for games that pause, open a menu or move the camera while TinyPaw has focus. |
| Hold to interact | From Point, hold a rebindable key (try Alt) to operate the underlying app while notes stay visible. Release to resume pointing. Enable this on backends that pass the focus and release tests below. |
| Hold to draw | From gameplay, hold a configured shortcut to point and draw. Release it to resume gameplay. Retain committed marks as a click-through overlay, respecting ink fade. The next hold resumes the same session. |
| Undo | Undo the most recent visible stroke, label or numbered pin from the current live or frozen scene. Inside the text field, undo edits the draft. |
| Clear | Clear strokes, labels and pins in the current scene and reset its numbering. Stay in annotation mode. |
| Exit | Escape outside text entry while TinyPaw has focus, the Off command, or the exit control ends the session and clears annotations. Remove input surfaces and release focus so the game can regain mouse control. If a game requires a click to resume, document it for that game. |
| Personalize | Preview a paw over light and dark backgrounds. Choose its size, the ink color and width, and a tap WAV with volume and mute. Remember settings between launches. |

Start with a desktop-wide presentation mode. An always-visible paw that follows clicks while other apps receive them is a separate feature with different input requirements. Defer it alongside screenshot export, online pack browsing, and animated pack formats.

## Settings

| Section | Controls and proposed defaults |
| --- | --- |
| Activation style | When switched On, Toggle mode opens Point immediately. Hold-to-draw mode starts ready in Passthrough, with the game retaining input until a fresh hold. Use a distinct hold binding. Offer a Latch annotation action for typing or Freeze without holding a key. |
| Controls | Configurable global toggle. Try Ctrl+Alt+P on Windows and Ctrl+Option+P on macOS. Report registration conflicts and allow rebinding from settings. On Niri show the compositor binding for `tinypaw toggle`. Configurable Freeze toggle. Try Ctrl+Alt+F on Windows and Ctrl+Option+F on macOS, and `tinypaw freeze` on Niri. Outside text entry, Ctrl/Cmd+Z undoes the latest annotation and X clears annotations. While typing, the editor owns ordinary keys, including X and Space. The explicit Off command always ends the session. |
| Display | Start with the primary display where the desktop identifies one, otherwise show a display picker. Offer one named monitor or all monitors. Remember the selection. If the selected monitor disappears during use, end the session and prompt for a target on next activation. |
| Paw | Ship Yohta (default), Moro, Zaeri and Aamox in the pointer picker. Under Appearance, choose the character and its presentation separately. Preview movement and taps on light and dark backgrounds. Keep Add your pointer, folder import and size here. Measure size in logical pixels. |
| Ink | Choose color and width. Start with a high-contrast cyan stroke at 4 logical pixels. Keep strokes until cleared by default. Offer “Fade after 3 seconds” for live callouts. Start the timer on release for each stroke and remove expired strokes from undo history. |
| Sound | Choose Pack default, a named sound from the shared tap collection, or an imported WAV. Preview, volume and mute use the live audio path. A named choice follows the user across pointer changes. |
| Text | Start with one-line labels and white text on a dark backing. Offer text size, color and backing. Labels persist until undo, clear or exit, independently of ink fading. |
| Lens | Default to 2×, offer 4× and adjustable lens size. Prompt for the capture source only when the desktop requires it for magnification. |

Keep the drawing surface clear while pointing. Group Point, Text and Lens in the compact bottom strip. Point contains installed paws and the laser dot. Keep Mute, Undo, Clear and Exit available. It can be hidden while streaming. Muting there uses the same preference as Settings and preserves the chosen WAV and volume. Exclude control clicks from the drawing gesture handler. Apply preferences immediately, persist them locally, and show the Discord sharing instructions in settings before a stream.

### Pointer visuals and TinyHand comparison

The macOS renderer puts a virtual grip 12% of the display height below the bottom edge. It aims the grip roughly 30% of a display width to the right of the cursor, clamps its horizontal position and eases it toward that target. This creates the impression of a pointing stick reaching in from the lower right. The entry point on the bottom or right edge moves as the cursor moves. The upright source art is rotated at runtime. [TinyHand arm renderer](https://github.com/nilbuild/tinyhand/blob/c5fb36b7c4d7c87480958872a724b30a52427a47/Sources/TinyHand/ArmView.swift)

| Upstream behavior checked in source | TinyPaw decision |
| --- | --- |
| The wrist joins a dark rod with a subtle highlight. Hand and rod are separate layers. Press shortens the hand by up to 6% and reduces its apparent lift through the shadow. | Offer this as Classic Stick. Start Yohta in Furry Paw and also offer Stretch Paw. Keep the claw on target and make the shadow optional across presentations. |
| Laser uses a red halo, a bright inner dot and a small press pulse. Laser sound defaults off independently of the hands. | Render the laser without a rod. Keep its center crisp on a stream and let users enable laser tap sound independently of the master mute. |
| Ink has rounded ends and joins, smoothed curves and an optional color that matches the pointer. Finished strokes are cached. | Use rounded strokes and keep the existing fixed-color default. Add Match pointer as an ink choice, using an optional pack accent with cyan fallback. Preserve the stroke's resolved color if the user switches paws. |
| A compact dark toolbar shows pinned styles (up to eight). The settings allow a quick ink-color control and a choice of ink fade times. | Put Yohta, Moro, Zaeri and Aamox thumbnails at the top of the pointer picker with Laser and imported packs accessible there. Keep the drawing strip compact. Additional favorites and fade durations can follow the streaming playtest. |
| The macOS app itself offers a hold-to-interact modifier and a choice between showing the cursor and keeping the hand visible. | Keep our planned hold-to-interact and inverse hold-to-draw modes. Use the normal cursor in passthrough for the initial game tests. |

Sources: [hand composition](https://github.com/nilbuild/tinyhand/blob/c5fb36b7c4d7c87480958872a724b30a52427a47/Sources/TinyHand/HandMaker.swift), [ink](https://github.com/nilbuild/tinyhand/blob/c5fb36b7c4d7c87480958872a724b30a52427a47/Sources/TinyHand/Ink.swift), [toolbar](https://github.com/nilbuild/tinyhand/blob/c5fb36b7c4d7c87480958872a724b30a52427a47/Sources/TinyHand/Toolbar.swift) and [preferences](https://github.com/nilbuild/tinyhand/blob/c5fb36b7c4d7c87480958872a724b30a52427a47/Sources/TinyHand/Preferences.swift).

### Paw presentations

| Appearance choice | On screen |
| --- | --- |
| Furry Paw | Proposed Yohta default. A compact pointing paw with a short tufted wrist follows the cursor. Most of the game stays visible. A single imported PNG works in this mode. |
| Stretch Paw | A small paw leads a long cartoon forearm reaching from below and to the right. On a quick reversal the forearm bows briefly, then settles. Stretch the middle of the limb while preserving the wrist tuft, finger shapes, markings near the claw and the exact pointing tip. |
| Classic Stick | Mount the same paw on a slim rod extending toward the moving offscreen grip described above. |

Register one presentation selector in Point. Offer stretch when the pack supplies a wrist anchor and forearm asset, and stick when it supplies a wrist anchor. Start a newly imported pack with Furry Paw. Show any unavailable choice with its asset requirement. Laser remains a separate pointer and remembers the last paw presentation on return.

### Helper tool behavior

**Text.** Use an inline Iced editor and store committed labels alongside strokes, in order and in output-local coordinates. Return to Point after placement. When composing with a Traditional Chinese IME, Enter first accepts a candidate; the next Enter places the completed label. Consume editor input so it cannot trigger drawing shortcuts or game actions.

**Lens.** Prototype Windows Graphics Capture, ScreenCaptureKit on macOS, and the ScreenCast portal with PipeWire on Niri. Sample a selected game window where it can be aligned to the overlay, or an overlay-free frame from the target display. Map the pointer through source bounds and display scaling before sampling. Verify the lens never magnifies itself and its rendered output remains visible in Discord's monitor share. Stop capture and discard frames on leaving Lens, source loss, lock or session exit. If capture is denied or unavailable, return to Point with annotations intact.

**Hold to interact.** Enter only from Point with no drag in progress. Hide the paw and toolbar, keep marks fixed on screen, make the overlay input-transparent and release focus. The desktop delivers clicks directly. Observe key-up even after focus changes and wait for held mouse buttons to be released before taking input back. Offer rebinding where Alt activates a menu or game binding. If the backend cannot reliably detect release, offer an explicit resume shortcut. Keep the global exit command available throughout.

### Accepted tools and delivery order

| Idea | Use during a stream |
| --- | --- |
| Spotlight | First overlay-tool milestone alongside pins. Draw a dim mask with a clear circle around the paw. Hide the mask whenever the user returns control to the foreground app. |
| Numbered pins | Each click places the next number on the target display. Undoing the last pin makes its number available again. Share the label renderer and undo order with ink and text. |
| Freeze frame | Deliver before Lens, on Windows first. A toggle shortcut takes a one-shot still of the target display, marks it Frozen, and draws into a separate annotation list. Capture before TinyPaw takes focus, so a game that pauses or opens a menu on focus loss stays out of the still. Hide live marks for one frame so they are not baked into it. Show no paw or ink until the still is on screen. Return Live discards the still and its marks, restoring the live scene. The game and its audio can continue underneath. This is the fallback for games that pause or move the camera while drawing. It cannot help in exclusive fullscreen. A hold binding can follow once key-release detection is proven. |

## Footprint, animation and ease of use

These are proposed prototype targets. Record actual release-build measurements before choosing renderer settings or publishing performance claims.

| Scenario | Target and check |
| --- | --- |
| Idle in tray | With settings closed and TinyPaw Off, schedule zero redraws, pointer polls or capture frames. Over 60 seconds after settling, target under 0.1% of one logical CPU core on average and under 50 MiB resident memory. Measure both after cold start and after using Lens and switching Off. Report the operating system and memory metric. |
| Distribution | Aim for a compressed download under 25 MiB per desktop build including the four starter pointers and shared tap sounds. Record installed size separately. Inspect enabled Cargo features, assets and the renderer before optimizing individual gestures. |
| Start pointing | From the resident Off state in Toggle mode, target a visible paw within 100 ms at p95 on the test machine. Record the first activation separately so lazy initialization costs are visible. |
| Draw while streaming | On the 160 Hz game display, the frame interval is 6.25 ms. Target p95 overlay CPU update and submission within half that interval while drawing. Also measure presented frame intervals and missed refreshes, since CPU timing alone cannot establish smoothness. Compare the same repeatable game scene and Discord monitor share with TinyPaw Off, drawing and Lens active. Record the game's p95 frame time and overlay CPU, GPU and memory for each run. Repeat at 60 Hz and with mixed-refresh displays. |

**Rendering and memory.** Update the paw to the latest pointer position at the next presentation opportunity. Preserve the input points needed for a continuous stroke while coalescing redundant redraw requests. Cache completed ink separately from the moving tip. Redraw only the output whose view changed. A static mark in Passthrough can stay on screen without an animation loop. Load the active pack on demand. Keep Lens capture bounded to the latest frame and release capture buffers and unused surfaces on Off. Benchmark the default Iced renderer with the smallest usable feature set first. If it misses either footprint or frame-pacing targets, compare renderer options on the same workload before committing.

**Motion.** Keep the tip anchored under the pointer. Redraw for grip settling, tap feedback and an optional ink fade only as long as each effect is active. Try a 100 ms press-and-release animation and interrupt it immediately on a new tap. Use elapsed time so it looks consistent at 60 and 160 Hz. Respect reduced-motion preferences where available and offer an in-app override. With reduced motion, snap the grip, use a static pressed pose and remove expired ink without a gradual fade. Exit and input handoff act immediately.

**First use.** Offer one primary action, “Turn on TinyPaw,” beside the active shortcut. Start with Yohta and Point selected. Click to tap and drag to draw with the same tool. Keep advanced tools in one picker and expose the current tool, Undo and Exit directly; keep existing controls such as Clear and Mute available without opening Settings. With a fresh install, ask a first-time user to turn it on, circle a target, undo and return to their game within 30 seconds without opening Settings. Repeat using a mouse alone via the tray and toolbar.

## Implementation

Use Rust and Iced for the application. Keep the annotation session independent of Iced and of the desktop backend. The overlay receives pointer events and renders the paw as an image in the scene so the stream can include it even when cursor capture is disabled.

### Application structure

Start with one Cargo package and one resident desktop process. Use modules within the package; additional crates can wait until a second consumer needs them. A short-lived CLI caller sends commands to the resident instance. Overlays, settings, tray and capture jobs share that instance. Discord receives the resulting screen through its normal share workflow.

All user-facing commands, tools and settings use the [feature registration contract](architecture.md#feature-registry-and-system-boundary). System controls use the same contract. The shell supplies layout slots, native integration and services. Built-ins declare what appears in those slots and how each option behaves.

| Rust module | Owns | Boundary |
| --- | --- | --- |
| `registry`, `features` | Validated definitions, dispatch indexes and built-in feature handlers | Register each option once. Settings, shortcuts, CLI and toolbar use the registered IDs. Only the active tool receives drawing input. |
| `app` | Iced event loop, settings, tray and command dispatch | Normalizes input into session events. Runs requested side effects and feeds success, failure or cancellation back to the session. |
| `session` | Input ownership, active tool, unfinished gesture, text draft and live/frozen annotations | Pure Rust transition logic. Holds the single chronological undo order across outputs for each scene. Receives events such as pointer movement in logical coordinates and emits requests such as acquire input or play tap. |
| `view` | Paw, ink, text, pins, spotlight, lens, toolbar and settings preview | Draws a snapshot of session state. The scene and preview share drawing code. Give ink a path cache and draw the moving pointer separately. |
| `platform` | Target displays, overlay surfaces, focus, shortcuts, session-lock events and input transparency | Owns native handles and converts input to output-local logical coordinates. Reports which functions work on the running desktop. Starts and stops input capture independently from keeping marks visible. |
| `capture` | Live lens frames and one-shot Freeze frames | Supplies pixels, source bounds and scale for the selected target. Keep the most recent frame only for the lens. Tag results by session and request so a late frame cannot reopen a canceled lens or freeze the wrong screen. |
| `packs`, `preferences`, `audio` | Validated PNG/WAV assets and hotspots, saved user choices and preloaded tap playback | Validate before switching a pack, save preferences locally, and keep drawing if audio is unavailable. Marks and captured frames are session-only. |

### Input ownership and event flow

The master switch gates all annotation actions. Off disarms the draw/interact bindings, drops pending transitions and clears the session. Keep the master shortcut and tray switch available. Where a compositor owns a hold binding, ignore its commands while Off. On enters Annotating or Passthrough according to the activation style. Passthrough leaves only committed live marks visible and gives the app underneath normal input. Track the active tool and Live or Frozen scene separately.

| User action | Session transition |
| --- | --- |
| Hold-to-draw key down | From enabled Passthrough, request target overlay input and enter Point once acquired. Ignore repeat events. If a gameplay drag is already in progress, let that gesture finish before taking ownership. |
| Drag, then release hold key | Finalize any stroke at its last accepted point and cancel an uncompleted tap. Hide the paw, spotlight and toolbar. If TinyPaw owns a button-down, consume its matching button-up before returning input to the game. Leave committed marks visible in Passthrough. If there are no marks, the overlay can be hidden. |
| Hold to interact from latched annotation | Temporarily enter Passthrough and preserve marks. Key-up restores pointing once gameplay buttons are up. Disable this inverse binding during a hold-to-draw gesture. |
| Master toggle or latch | From every enabled state, including a hold, text draft or pending capture, the master toggle switches Off. Invalidate the current hold and ignore its eventual key-up. Re-enabling while a hold key is down requires release and a fresh press before it can draw. Latch annotation is a separate enabled-only action. Escape outside text entry or the explicit Off command ends the session. |
| A transition fails or is interrupted | Invalidate its pending request. Unmap partial overlays, drop input ownership and stop capture on explicit off, lock, suspend or lost target. If hold release cannot be observed after a focus change, offer Toggle on that backend. |

Use paired press/release events for hold modes. On Niri expose idempotent local commands for hold begin and end where the compositor can bind both edges. Associate an end with its originating hold and ignore unmatched or duplicate edges. Never synthesize a gameplay click when handing input back.

### Library choices

| Work | Implementation |
| --- | --- |
| Overlay and preview | Iced image and Canvas widgets render poses and stroke paths. Share the view with the settings window. Start with borderless transparent windows on Windows and macOS, then use platform APIs where placement or input behavior requires them. |
| Niri | Use the Linux-only iced_exwlshell runtime for layer surfaces, settings windows and output notifications. Pin compatible Iced and shell crate versions together. |
| Activation | Use global-hotkey on Windows, macOS and X11, a Niri binding to a single-instance local command on Wayland, and tray-icon where supported. All entry points request the same session transition. Ignore key auto-repeat. |
| Packs and sound | Use serde_json for manifests, Iced's image loader for PNGs, rodio for predecoded audio and rfd for importing a pack folder or sound file. |

Keep gestures, strokes, animation timing, hotspots and pack loading in shared modules. Give the presentation module output-local pointer events and have it return drawing state and tap events. Decode the selected WAV when the style or sound changes. Redraw when input or animation changes the scene and let the process sleep while idle. Roll back partial activation if the backend cannot create the target surfaces or accept input. Shut down through a single path on Escape, toggle, lock, suspend or lost target.

Keep window placement, keyboard focus and pointer delivery behind platform-specific modules. Only build the Wayland module for Linux targets that enable it.

| Component | Responsibility |
| --- | --- |
| Desktop integration | Create a transparent surface per output, receive input on active surfaces, activate and dismiss the session, and handle shortcut conflicts, focus, display changes and session lock. Convert native events to output-local logical coordinates. |
| Shared views | Render the paw and ink from session state. Reuse the paw view in settings so scale, hotspot and press behavior match the live overlay. Only the display carrying the pointer draws the paw. |
| Packs and preferences | Resolve bundled assets, user-installed packs, and per-user overrides. Persist preferences separately so edits or updates to a pack preserve the user's sound and size choices. |

Use each overlay's own pointer events during presentation mode. This makes observing every click across the desktop unnecessary for the core feature. Test transparent areas for input explicitly. A platform that hit-tests invisible pixels differently may need a native input surface distinct from its visual surface.

### Platform targets

| Target | First implementation and proof |
| --- | --- |
| Windows | Prototype transparent topmost windows using the chosen toolkit. Check whether its transparency backend passes clicks on zero-alpha pixels through to the slide. Add Win32 input handling if necessary. Register the activation hotkey. Test mixed display scaling and negative monitor origins. |
| Linux on Niri/Wayland | Request an **overlay-layer** surface per output through the Wayland client API and layer-shell protocol bindings. Give active surfaces pointer input and arrange keyboard focus for Escape. Provide a single-instance `tinypaw toggle` command for a Niri shortcut and a `tinypaw off` recovery command. Test over a fullscreen slide on a real Niri session. |
| macOS | Use the same scene with native window-level, Spaces, focus restoration and shortcut handling as needed. Test a fullscreen presentation in its own Space, with an external display. Confirm permission requirements on a clean install. |
| Other Linux desktops | Validate X11 separately. Detect layer-shell support at runtime on Wayland. On an unsupported compositor, keep settings and preview usable and explain the limitation before activation. Track compatible desktops individually. |

## Discord game streaming

The v1 toggle controls local annotation. The streamer selects the capture source in Discord once and can then turn TinyPaw on and off throughout the same stream. On a multi-monitor setup, select the game monitor in both apps and keep chat and settings on another display. While annotating, the mouse belongs to TinyPaw. The game may continue running or pause when it loses focus.

| Discord source and game mode | Scope and verification |
| --- | --- |
| Game monitor shared, borderless or windowed game | Primary release target. The viewer should see the game, paw, tap animation and ink, then see clean gameplay after exit. Everything visible on the shared monitor can appear. |
| Game application shared directly | A separate desktop overlay can be visible to the streamer and omitted from the game capture. Treat this as an unsupported route for viewer-visible ink in v1. The shortcut cannot change Discord's capture source. |
| Exclusive fullscreen | Compatibility test only. Try borderless if the overlay is hidden or the game minimizes when focus changes. |
| TinyPaw Stream Output (deferred) | The user would select a game window in TinyPaw, then share a dedicated TinyPaw Stream Output window in Discord. Viewers would see the selected game and annotations together. Prototype only if the first monitor-share playtest shows a need for game-only sharing, such as wanting to Alt+Tab privately. See the [architecture decision](architecture.md#future-stream-output). |

For the first live test, use a second Discord account/device or a remote participant. Record the OS, Discord client/build, game, game display mode and selected capture source. Verify remote paw/ink visibility, absence of a duplicate cursor, and correct placement on mixed-scale monitors. Test a game that locks the mouse: activate, drag from the center, exit mid-drag, and confirm the first gameplay click and camera movement behave normally. Listen for game audio and the optional tap sound separately. Repeat with a direct game share as a control. Local previews or OBS recordings alone cannot establish a Discord pass.

## Style and sound format

A pack is a directory containing a versioned JSON manifest and local media. The four bundled pointers and user-created packs go through the same loader. Use transparent PNGs for the first version. One idle image is sufficient. An optional pressed image uses the same canvas and anchor. Without it, animate a small compression around the anchor.

```json
{
  "schemaVersion": 1,
  "id": "yohta",
  "name": "Yohta",
  "pointer": {
    "idle": "paw.png",
    "pressed": "paw-press.png",
    "hotspot": [0.40, 0.08],
    "wrist": [0.50, 0.90],
    "forearm": "forearm.png",
    "forearmAnchor": [0.50, 0.02],
    "jointWidth": 0.40
  },
  "sounds": { "tap": "tap.wav" }
}
```

The hotspot is the pointing claw or pad. The optional wrist anchor is where a forearm or rod attaches. Both use fractions of the image width and height from the top-left and must be distinct to define the pointing axis. Set final values against the artwork. A stretch forearm is a separate upright transparent strip with the wrist end at top center. Keep identifying tufts and distal markings in the paw image; place stretchable fur in the strip. With one PNG, suggest a wrist position for optional stick use and allow adjustment in the preview. A hotspot-only pack uses its original orientation. An optional pack accent supplies Match pointer ink. The settings sound picker overrides the pack default independently of the visual style. Include a restore-default action. Resolve assets within the pack directory, validate the manifest and images before switching, and keep the current pack active if loading fails.

### Add your pointer

Paired PNG import is available in Appearance. The forearm anchor uses fractions of its own image dimensions. Its vertical position must be between 0 and 0.8, leaving a section below the joint to stretch. Joint width scales the full forearm image width relative to the paw image width and accepts 0.05 to 1.5. Omitted values default to a top-centre forearm anchor and a width of 0.4. The renderer retains the first eighth of the source as a short wrist section and bends the remaining texture using up to 32 strips. Packs without a forearm image use the procedural accent-colour limb.

User-supplied artwork and custom pointer packs are original TinyPaw v1 requirements. TinyHand's HandMaker builds its art prompt from a style description. It expects an upright hand with a single upper tip and a centered wrist, validates the silhouette and transparency on import, then attaches the stick. TinyPaw uses the upright pose as its generation guide and allows users to set the hotspot manually for other poses. [TinyHand prompt and importer](https://github.com/nilbuild/tinyhand/blob/c5fb36b7c4d7c87480958872a724b30a52427a47/Sources/TinyHand/HandMaker.swift#L4-L26)

In Appearance, open **Add your pointer**. Offer an optional character/style field and a copyable prompt beside the PNG chooser. Insert the user's description into the prompt or fall back to the attached character reference. The prompt works as an artist brief or in the user's preferred image tool. Creation and import work locally from an existing PNG.

> Create a desktop pointer from my character reference. Character or style: [description]. Show the back of one right paw or claw, preserving the reference's digit count, markings, colors and proportions. Raise the digit next to the thumb straight upward and curl the remaining digits inward. Make the pointing tip the highest part of the silhouette. Center the wrist at the bottom and give it a clean end. Fit the entire paw inside a transparent portrait PNG with a small margin. Keep the silhouette readable at cursor size and omit the arm, stick, lettering, cast shadow and glow.

Give the new pointer a name, select the idle PNG, then click the exact tip in the preview to set the hotspot. Optionally add a pressed pose on the same canvas and a default tap WAV. Let the user try pointing and tapping on light and dark backgrounds before saving. Write a validated pack to app-managed storage and add it to the picker only after the save succeeds. A single PNG must complete the workflow.

### Starter sound direction

| Sound | Brief |
| --- | --- |
| Blackboard Knock | A short, hollow classroom pointer knock against a chalkboard. Use as the proposed default tap. |
| Teddy Squeak | Provisional direction for the user's Minecraft teddy reference. Confirm the exact cue from a clip or item/mod name before producing an original sound. |
| Bubble Pop | A soft, quick pop with little tail that stays pleasant over repeated taps. |
| Rubber Bonk | A low, springy comic tap for a heavier paw. |

Use original recordings or cleared assets for the bundled collection. Keep its clips short, trim leading silence, and match their perceived volume so switching to a joke sound does not produce a volume jump. Store shared clips once. In Pack default mode, play the pack's own valid tap if present, otherwise use Blackboard Knock. Keep preview, override and mute independent of the selected character.

## Build order and acceptance

1. **Prove the Discord game workflow on Windows, then Niri.** Use a placeholder paw, one stroke color, a toggle and Escape. Run the remote-viewer test above. Also test tapping a slide hyperlink while active and opening it normally after exit. Use the results to settle surface and focus handling before commissioning final assets.
   Record whether the game pauses, mutes, opens a menu or keeps receiving mouse input while TinyPaw has focus. Use that result to list which games need Freeze.
   Measure idle, first activation and drawing on this first vertical slice. Rerun the same workload as each tool lands; add the capture measurements when Lens is available.
   After the Discord playtest, record whether sharing the game monitor worked for the streamer when switching apps. Use that result to decide whether to schedule Stream Output after v1.
   In Hold to draw, release the shortcut before and after the mouse button, tap it twice quickly, and interrupt a pending activation. Confirm that the viewer sees the marks, input returns once, and the game receives no accidental shot or click.
   Switch the master Off during a hold, a text draft and pending Lens capture. Confirm the game and remote feed clear, the hold keys cannot reactivate TinyPaw, and stale callbacks cannot recreate the overlay. Switch On from the shortcut and tray in each activation style and verify the displayed status.
2. **Finish the pointer and sound workflow.** Prove both poses with Yohta, then load Moro, Zaeri and Aamox through the same pack path. Import a single PNG through Add your pointer, set its hotspot, move the source file, and verify the installed pointer still works after restart. Choose a shared sound, switch characters, mute and unmute, then return to Pack default. Verify the preview and live overlay agree.
   Try all three Yohta presentations against a small map marker. Reverse direction, tap, and draw across a scaled display. Check that the same pixel stays under the claw, there is no gap at the wrist, reduced motion suppresses the elastic bend, and redraws stop once settled.
   During a monitor share, circle a target, switch to Laser, mute and clear with X. Confirm the viewer sees the original circle until cleared, the game receives none of the control clicks, and the next stroke still works. With fading enabled, hold a drag longer than three seconds and verify the stroke stays visible until release and fades after the configured delay.
3. **Complete the helper tools.** Ship Text, Spotlight and pins on the shared annotation scene. Test undo across a stroke, label and pin, pin-number reuse, and spotlight hiding during passthrough. Validate Freeze first on Windows using a one-shot display still, then Lens. Measure the time until the still appears (try 150 ms), compare HDR colors against the live game, and confirm no capture border reaches the Discord viewer. Verify Freeze separately on Niri and macOS, where the one-shot capture route and its permission prompt are unproven. Verify returning Live disposes of the frozen frame and restores earlier live marks.
4. **Validate and package each platform.** Run the same interaction script on macOS and the other targeted desktops. Test monitor entry and removal, lock/unlock, shortcut collisions, repeated toggling and exiting during a drag. Ship each build with its verified desktop and capture modes.
   Test English and Traditional Chinese labels, candidate acceptance, cancellation, typing X and mixed label/stroke undo. Have the Discord viewer read a label and a magnified moving detail. Check mixed-DPI alignment, capture denial and source closure. Test holding and releasing Alt across a slide advance, a pointer-lock game, Alt+Tab and session lock. Record Lens and hold-to-interact support per desktop.

## References

- [TinyHand upstream](https://github.com/nilbuild/tinyhand) and [indexed README](https://githubhelp.com/nilbuild/tinyhand)
- [Windows screen capture](https://learn.microsoft.com/en-us/windows/apps/develop/media-authoring-processing/screen-capture), [ScreenCaptureKit](https://developer.apple.com/documentation/screencapturekit/capturing-screen-content-in-macos), [ScreenCast portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html) and [Niri screencasting](https://niri-wm.github.io/niri/Screencasting.html)
- [TinyHand GNOME port feature description](https://extensions.gnome.org/review/78109)
- [Iced drawing](https://docs.rs/iced/latest/iced/widget/canvas/), [window options](https://docs.rs/iced/latest/iced/window/struct.Settings.html), [Wayland shell integration](https://docs.rs/iced_exwlshell/latest/iced_exwlshell/) and [migration notes](https://github.com/waycrate/exwlshelleventloop#migrating)
- [Global shortcuts](https://docs.rs/global-hotkey/latest/global_hotkey/), [tray integration](https://docs.rs/tray-icon/latest/tray_icon/) and [native file dialogs](https://docs.rs/rfd/latest/rfd/)
- [Discord screen sharing](https://support.discord.com/hc/en-us/articles/360040816151-Go-Live-and-Screen-Share) and [Windows game capture methods](https://support.discord.com/hc/en-us/articles/9410427556375--Windows-Capturing-Application-Window-for-Screen-Share-and-Go-Live)
- [Windows layered-window hit testing](https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features#layered-windows)
- [rodio playback](https://docs.rs/rodio/latest/rodio/)
- [Niri layer-shell behavior](https://niri-wm.github.io/niri/Layer%E2%80%90Shell-Components.html) and [key bindings](https://niri-wm.github.io/niri/Configuration%3A-Key-Bindings.html)
- [Wayland layer-shell protocol](https://wayland.app/protocols/wlr-layer-shell-unstable-v1)
