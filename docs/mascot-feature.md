# Desktop character and custom character imports

Status: feature scope, 10 November 2026. Stage 1 (desktop interaction) is
implemented on Windows as of 11 November 2026; see
[Stage 1 implementation](#stage-1-implementation). Custom imports (stage 2) and
the standing pose and music (stage 3) do not exist yet. The
[Yohta prototype](mascot-prototype.md) preview window remains for rig work.

See [MateEngine/BUG Clicker research and the Yohta artwork plan](mascot-engine-study-2026-11-11.md)
for the source review, Windows input strategy and production asset checklist.

## Goal

Add a small optional character to the desktop that animates, reacts to simple
interactions and opens TinyPaw's controls. Users can import their own character.
Use layered PNG artwork and lightweight transforms. Build our own implementation;
no BUG Clicker dependency or requirement to read its protected character files.

Agreed direction:

- Sitting and standing pose support.
- Large head, small body, anime-influenced styling, thick outlines and subtle
  pixel/graffiti accents for the bundled Yohta artwork.
- Eyes look toward the cursor. Head and tail can move independently.
- A left click reacts (and can speak); a right click opens the menu; dragging
  moves the character.
- Optional music response.
- Optional speech lines in a bubble, set per character.
- Custom character import is part of the feature.

The numerical defaults and format below are proposed implementation decisions.
Custom art may use any style; the importer does not enforce Yohta's proportions.

## User experience

| Action | Behaviour |
| --- | --- |
| Enable | Show the selected character in a small transparent desktop window. Default disabled until the user chooses it. |
| Click | A left click plays the character's click reaction (pressed face and a squash) and, once lines exist, may show a speech line. |
| Right click | Open a compact menu beside the character. Another right click, Escape or an outside click dismisses it. Closing the menu does not disable the character. |
| Drag | Move after approximately 5 logical pixels of movement. A drag never also opens the menu. Keep the character reachable on a connected display. |
| Menu | Offer the existing annotation On/Off, Settings and Mute controls, plus character/pose selection and Hide character. Dispatch app actions through the existing registry and respect their availability. |
| Look | Move pupils within their eye masks toward the cursor, with small limits and smooth settling. The head need not follow the cursor. |
| Idle | Occasional blinking, a gentle head tilt and tail sway. Avoid constant large bounces. |
| Music | Optionally bob the head and subtly compress/bounce the body in response to local playback. Stop and settle when playback is silent. |
| Lines | Optionally show a short speech line in a bubble on click, drag, drop, idle, music start or annotation end, with an optional sound. Idle lines can be muted. |
| Pose | Choose Sitting or Standing if supplied by the pack. Changing pose preserves desktop position and display size without sliding the feet. |
| Hide | Remove the character window; retain its settings and installed artwork. The tray and keyboard shortcuts remain available. |
| Restart | Restore character, pose, size, position and motion choices. Recover to a visible display if the saved monitor is missing. |

Proposed defaults: sitting, about 160 logical pixels tall, eye tracking and subtle
idle motion enabled, music response disabled. Size range: 96–320 logical pixels.
Do not add automatic walking, pose switching, reminders, alarms or pet-care
systems. Speech lines are in scope as planned in the
[character creation plan](character-creation-plan.md); conversations and
scripted dialogue are not.

### Relationship to annotation

The character is an optional launcher with its own visibility preference.
In the proposed feature, annotation Off leaves an enabled character available;
Quit closes it with the rest of the app. This deliberately extends the existing
first-release Off behaviour and must be reflected in help and UI wording when
implemented.

Initially hide the character and its menu while the annotation surface owns
input, then restore it at the same position when input is released. Keep
committed marks independent of the character. The annotation toolbar remains
the control surface while drawing. Do not make clicks on the character draw ink.

## Artwork and simple animation

Four logical groups are sufficient:

| Group | Contents and transform |
| --- | --- |
| Body | Base seated/standing artwork, including stationary limbs; optional small breathing or music bounce. |
| Head | Head, ears, hair and muzzle; rotates around a neck pivot. |
| Eyes | Eye whites, irises/pupils, masks and optional blink overlays. These inherit the head transform and also support independent pupil offsets. |
| Tail | Tail artwork; rotates around its attachment point behind the body. |

Use an elapsed-time-driven animation state with bounded offsets, rotation and
scale. Do not introduce a skeleton editor, mesh deformation, physics simulation
or a general scripting engine.

Drawing order: tail, body, head, eye whites, masked pupils, blink overlays.
Body bounce moves the whole group; the tail and head add their local rotations.
Apply the head transform to every eye component. Convert cursor direction into
head-local coordinates before calculating pupil movement.

Production artwork must extend behind neck and tail overlaps so rotation cannot
expose holes or interior sticker borders. Blink overlays cover the eye opening
with matching fur and a closed-eyelid line; scaling pupils is not a blink.

All assets for a pose share one transparent canvas and registration. Most pixels
of a part image can be empty. Coordinates are fractions of that canvas, measured
from its top-left. A neck or tail pivot is an attachment point, not the centre of
the visible part. A pose ground anchor keeps sitting/standing changes stable.

## Importing a custom character

### Quick import: one PNG

1. Choose **Add character** in Settings → Character.
2. Select a transparent PNG and give the character a display name.
3. Preview it at desktop size on light and dark backgrounds.
4. Set the ground anchor, with a bottom-centre suggestion.
5. Save to install it and select it.

One PNG must complete the workflow. It supports placement, clicking, dragging
and optional whole-image motion. Explain that independent eye/head/tail motion
requires separate artwork. Do not promise automatic cutting or invent missing
parts from a flattened illustration.

### Layered import

Offer **Import character folder** for a folder containing a manifest and PNGs.
Also let a user start with Quick import and add layers in the character editor.
The editor supplies previews and clickable controls for the ground anchor,
neck/tail pivots and eye movement limits. Users should not need to write JSON.

Import either or both poses. A sitting-only or standing-only character is valid.
The available pose picker reflects the pack's actual contents.

| Missing artwork | Fallback |
| --- | --- |
| Separate head | Render the complete character in the body image; skip independent head rotation. |
| Tail | Skip tail animation. A tail already painted into the body remains stationary. |
| Eye assets/mask | Skip cursor tracking for that eye. |
| Blink overlay | Keep that eye open. |
| Second pose | Offer only the supplied pose. |

An optional component that is absent uses its fallback. A component that is
declared but invalid fails validation with a useful error; do not silently hide
broken files.

### Proposed version 1 manifest

This is a new mascot schema, separate from the existing pointer pack manifest.
The full manifest is an interchange format; the editor produces it for the user.

```json
{
  "schema_version": 1,
  "id": "yohta",
  "name": "Yohta",
  "author": "Character artist",
  "canvas": [1024, 1024],
  "default_pose": "sit",
  "poses": {
    "sit": {
      "ground_anchor": [0.5, 0.95],
      "body": "sit/body.png",
      "head": {
        "image": "sit/head.png",
        "pivot": [0.43, 0.61]
      },
      "tail": {
        "image": "sit/tail.png",
        "pivot": [0.65, 0.72]
      },
      "eyes": [
        {
          "white": "sit/eye-near-white.png",
          "pupil": "sit/eye-near-pupil.png",
          "mask": "sit/eye-near-mask.png",
          "blink": "sit/eye-near-blink.png",
          "max_offset": [0.012, 0.009]
        }
      ]
    }
  }
}
```

Only the schema version, identity/name, canvas, default pose and at least one
pose are required. Each pose requires its ground anchor and body image. For a
static pack that image contains the whole character. Other groups are optional.
Eyes require a separate head and white/pupil/mask images; blink is optional.
Allow up to four eyes so custom creatures are not limited to feline anatomy.
The mask's alpha channel defines the visible pupil area; its RGB colour is
irrelevant. Clip the shifted pupil to the stationary mask.

Both poses use the declared canvas size but may have different pivots and eye
limits. Store animation timing/amplitude defaults in the app for version 1;
do not make custom character creators tune a physics rig.

### Validation and installation

- Accept local transparent PNG assets and a supported JSON schema. Start with
  single-image and folder import; archive import is deferred.
- Require image dimensions to match the declared canvas. Set a maximum of
  2048 pixels per dimension, 32 image files, 64 MiB total encoded assets and
  128 MiB total decoded RGBA data per pack.
- Validate finite normalized anchors, bounded eye offsets, a valid default
  pose, supported pose names and nonempty image/mask alpha.
- Resolve files strictly inside the selected folder. Reject absolute paths,
  traversal and symlinks escaping that folder. Packs contain data only: no
  scripts, executables, remote URLs, fonts or shaders.
- Fully validate and copy into a staging directory before installing. A failed
  import keeps the existing character and preferences active.
- Install independent copies under TinyPaw's character storage, separate from
  pointer packs. Moving or deleting the source folder must not break the import.
- Resolve duplicate pack IDs explicitly: replace the installed pack or install
  a separate copy. Preserve user overrides when replacing the same identity.
- Allow rename, preview, pose selection, alignment edits, removal and export
  of the installed TinyPaw-format folder. Never overwrite a source illustration.
- Keep user overrides separate from authored pack data. Removing an active
  pack selects bundled Yohta; never delete the source folder.

Bundled Yohta uses the same validated pack path as imported characters. The
current concept-sheet crop coordinates and procedural eyes are prototype
details, not the import format.

## Implementation boundaries

| Owner | Responsibility |
| --- | --- |
| Registered mascot feature | Declare visibility, character, pose, size, eye tracking, idle/music motion and annotation visibility policy. Expose commands/settings through the existing registry. |
| Shared pack loader/editor | Validate assets, install/export packs and save alignment overrides. Character files do not register executable features. |
| Shared rig/animation state | Compose groups and masks, calculate gaze and pose transforms, and apply reduced-motion rules. |
| Desktop host | Own the small transparent surface, alpha-aware input, click/drag gestures, focus, DPI, display recovery and the adjacent menu. |
| Optional audio adapter | Supply a small amplitude envelope for local playback; expose platform availability and release resources when disabled. |
| Preview | Use the same rig and animation state as the desktop, with a demo beat clearly labelled as simulated. |

Use the existing resident process and Iced renderer. Load the selected pack
lazily; decode/cache its assets once and release them when switching or hiding.
Do not keep every installed character decoded.

Poll cursor position only while visible tracking needs it. Use a capped update
rate during animation, redraw on changes and sleep when settled. Use
elapsed-time smoothing rather than frame-count-dependent movement.

Transparent pixels outside the character must not consume desktop clicks.
Only visible character/menu areas are interactive. Dragging temporarily owns
the gesture through release. Menu opening/dismissal must not steal a click
meant for the app underneath. Restore prior focus after menu actions as needed.
Place menus within the display work area, flipping sides near an edge.

Respect system/app reduced motion: disable decorative sway/bounce, let gaze
snap or be disabled, and keep menu/drag interactions usable. Hidden characters
stop polling, animation and music capture. Lock/suspend releases resources;
resume validates the selected display before restoring.

For music, start with a smoothed playback amplitude response and silence gate.
Do not promise exact beat detection. Capture no microphone input and retain no
audio. Windows is the first adapter; other backends show availability honestly.
Character use must remain functional without any audio backend.

## Delivery stages and acceptance

1. **Desktop interaction.** Bring Yohta's sitting rig into the resident app,
   behind an optional registered toggle. Implement click/menu, drag, position/
   size persistence, cursor eyes, blinking and subtle head/tail motion.
2. **Custom imports.** Implement single-PNG and layered folder import, alignment
   preview/editor, pack management and export. Move bundled Yohta onto the same
   format. Custom import is required to complete this feature.
3. **Pose and music polish.** Add matching standing Yohta artwork, stable pose
   switching and optional Windows playback response. Validate resource use,
   reduced motion and the full desktop workflow before advertising support.

### Stage 1 implementation

- **Feature.** `src/features/mascot.rs` registers `mascot.enabled` (default
  off), `mascot.size` (96–320, default 160), `mascot.eyes`, `mascot.idle` and
  the internal `mascot.x`/`mascot.y` ground anchor in physical desktop pixels.
  Commands: `mascot.toggle` (tray check item, `tinypaw mascot`) and
  `mascot.hide` (`tinypaw mascot-hide`). Settings has a Character page.
- **Window.** `src/mascot/desktop.rs` owns one transparent, topmost,
  taskbar-less Iced window laid out as `[menu slot | character | menu slot]`.
  The menu opens on the right, or on the left near the right edge of the work
  area, without resizing the window. The rig is decoded when the character is
  enabled and released when it is turned off.
- **Input.** The window stays click-through (winit mouse passthrough) except
  while the polled cursor is over the rig's silhouette, the open menu, or during
  a press. The silhouette is a coarse 6-rig-pixel grid stamped from the sprite
  alpha, which also pads the outline for the small idle rotations. A
  `WM_MOUSEACTIVATE` subclass returns `MA_NOACTIVATE`, so clicking Yohta never
  takes focus from the user's app. A press becomes a drag after 5 logical
  pixels; otherwise release toggles the menu. Escape or a click outside closes
  it. Menu entries dispatch through the registry and are disabled when
  unavailable.
- **Placement.** The ground anchor is clamped to the nearest monitor's work
  area: feet no lower than the taskbar, the window (and so the menu) no higher
  than the top edge, and the character's sides within the edges. An unplaced
  or off-screen anchor recovers to the bottom right of the primary display, or
  the nearest display. Display and DPI changes re-place it. Size changes keep
  the feet still.
- **Annotation.** While the annotation surface owns input
  (`OverlayState::Interactive`), the window is hidden and its menu closed. It
  returns at the same spot. Off and click-through marks leave it visible.
- **Motion.** Idle sway and blinking at about 30 fps while enabled. Reduced
  motion (app or Windows) stops sway and blinking and snaps the gaze. With idle
  off, polling drops to 10 Hz for hit testing and gaze.
- **Measured** (release build, 3440×1440 primary display, 11 November 2026):
  idle motion on, 2.8–3.8 % of one core; idle off, 0.6 %; character off,
  0 %, with private memory back from 13 MB to 5 MB.
- **Known limits.** Hit testing follows the polled cursor, so a click within
  about 33 ms (100 ms when still) of moving onto Yohta can still reach the app
  underneath. The vertical clamp keeps room for the menu above the feet, so
  Yohta cannot sit flush with the top edge. Non-Windows builds keep the settings
  but report that the character is unavailable.

### Stage 2 progress: character packs and one-PNG import

Implemented on Windows, 11 November 2026 (step 1 of the
[character creation plan](character-creation-plan.md)):

- **Packs.** `src/mascot/pack.rs` loads `character.json` folders from
  `%APPDATA%\TinyPaw\characters\<id>` using the version 1 manifest above.
  Every install is written to a hidden staging folder,
  validated there (canvas size, alpha, paths inside the folder, 64 MiB per
  file) and only then moved into place.
- **Selection.** `mascot.character` (default `yohta`) chooses the character;
  bundled Yohta is the first entry in the same list. A missing or broken
  pack shows Yohta and says why.
- **Add character.** One transparent PNG: empty margins are trimmed, art taller
  than 1024 px is scaled down, and the ground point is suggested from the
  bottom of the art. The draft previews on light and dark; clicking moves the
  ground point. **Use character** installs it, selects it and shows it.
- **Import folder / Export / Remove.** Import copies only the files the
  manifest names; a taken ID asks **Keep both** or **Replace** (Yohta is never
  replaced). Export writes a re-importable folder; Remove deletes TinyPaw's
  copy and falls back to Yohta.
- **Desktop.** The window layout, hit test and placement use each character's
  own canvas and ground point. A single image sways gently and squashes on
  press; reduced motion turns both off.

### Stage 2 progress: moving parts and reactions

Step 2a of the character creation plan, 11 November 2026. A pose can now
name separate PNGs for moving parts, eyes and head faces, all on the pose's
canvas. A body-only pose still renders as one image.

```json
"sit": {
  "ground_anchor": [0.5, 0.95],
  "body": "sit/body.png",
  "parts": [
    { "type": "head", "image": "sit/head.png", "pivot": [0.43, 0.61],
      "motion": { "preset": "tilt", "strength": 1, "speed": 1 },
      "expressions": { "blink": "sit/head-blink.png",
                       "pressed": "sit/head-pressed.png",
                       "dragging": "sit/head-drag.png" } },
    { "type": "tail", "image": "sit/tail.png", "pivot": [0.65, 0.72],
      "motion": { "preset": "wag" } },
    { "type": "ear", "image": "sit/ear.png", "pivot": [0.38, 0.30],
      "parent": "head", "layer": "back", "motion": { "preset": "floppy" } }
  ],
  "eyes": [
    { "white": "sit/eye-white.png", "pupil": "sit/eye-pupil.png",
      "mask": "sit/eye-mask.png", "blink": "sit/eye-blink.png",
      "max_offset": [0.012, 0.009] }
  ]
}
```

| Field | Rule |
| --- | --- |
| `type` | `head`, `tail`, `ear`, `wing`, `hair` or `other`. At most one head. |
| `pivot` | Attachment point, fractions of the canvas. The part rotates around it. |
| `parent` | `head` makes the part ride on the head (one level only); omitted or `body` is top level. |
| `layer` | `back` or `front` of its parent. Default: tails, wings and ears back; others front. |
| `motion` | `preset` per type (below); `strength` and `speed` 0–2, default 1. |
| `expressions` | Head only: `blink`, `pressed`, `dragging` faces. A missing face uses the head. Pressed and dragging faces have their eyes painted in, so tracking eyes hide under them. |
| `eyes` | Need a head. The pupil is clipped to the mask's alpha and travels up to `max_offset` (at most 0.1 of the canvas). |

The `head` and `tail` objects from the version 1 example above are accepted
as shorthand for parts. Limits: 16 parts, 4 eyes, 32 images per pack.

| Type | Presets (first is the default) |
| --- | --- |
| head | tilt, nod, still |
| tail | sway, wag, hang, still |
| ear | twitch, floppy, still |
| wing | flap, fold |
| hair | spring, still |
| other | still, sway, spring |

- **Motion.** `src/mascot/motion.rs` steps every part from elapsed time.
  Floppy, spring and hang parts follow a damped spring, lag behind a turning
  parent, and swing when the drag speeds up or stops.
- **Reactions.** A left click shows the pressed face for about a third of a
  second, with a squash while held. Dragging shows the dragging face; the
  character leans against the motion, tails trail and ears droop, then
  everything settles after the drop. Blinks use the blink face or eye blinks.
- **Menu.** The right button opens the menu; Escape or any outside click closes
  it. Left click no longer opens it.
- **Reduced motion** stops idle motion, springs, lean and squash; faces still
  change.
- **Art tab (2b).** **Edit art** on an installed character opens it in
  Settings: replace the body, add part PNGs (the type is guessed from the file
  name), set each part's type, *On head* and Back/Front, add the head's blink,
  pressed and dragging faces, and add eyes (white, pupil, mask, optional
  blink, pupil travel). A pinned preview shows the character at rest with the
  ground and numbered pivot markers; clicking moves the selected one. **Save**
  writes a fresh pack through the staged install and replaces the old copy
  only after it validates; the desktop reloads it at once.
- **Lining up parts.** Quick import records how it cut the canvas from the
  original PNG (`"source": {"size": [w, h], "visible": [x, y, w, h]}`), so a
  part drawn on that original canvas is cut and scaled the same way. Parts at
  the character's own canvas size are used as they are; other sizes are
  refused with both sizes named.
- **Planned import improvement.** The matching-canvas rule above describes the
  current editor and remains the prepared pack format. The
  [cropped-parts improvement plan](character-creation-plan.md#improvement-import-and-arrange-cropped-parts)
  adds differently sized PNG inputs, visual arrangement and automatic canvas
  preparation on Save, with complete eyes supported as static head layers.
- **Motion tab (2c).** The Character page lists each moving part (Yohta's
  head and tail, a single image as "Whole character", or a layered pack's
  parts) with its preset, Strength and Speed, showing exact values. A pinned
  preview animates the character with the same rig and animator as the desktop,
  with **Click**, **Drag ←** and **Drag →** tests. **Save motion** stores the
  choices in `characters/.overrides/<id>.json`, beside rather than inside the
  pack, so re-importing or replacing the pack keeps them; the desktop reloads
  at once. **Reset to pack** returns to the pack's own motion. Saved motion
  that no longer fits the parts is ignored. The preview runs only while the
  page is open.
- **Yohta as a pack (2d).** Bundled Yohta is an ordinary layered pack in
  `assets/mascot/yohta/pack/` (head, body, tail, and white, pupil, mask and
  blink images per eye), embedded in the executable and loaded through the
  same parsing and checks as any installed pack. `src/mascot/yohta_pack.rs`
  regenerates it from the approved sheet (`cargo test generate_yohta_pack --
  --ignored`), reproducing the earlier hand-built rig's crops, socket masks
  and procedural pupils exactly; the special-case rig is gone and the sheet
  is no longer embedded. **Export…** works for him, and **Edit a copy**
  installs "Yohta (copy)" for editing, since the built-in Yohta never changes.

### Speech lines (character creation step 3)

Implemented 11 November 2026.

- **Format.** A pack may carry `"lines"`, mapping triggers (`click`, `drag`
  for pick-up, `drop`, `idle`, `music`, `annotation_end`) to lists of
  `{"text": …, "sound": "lines/x.wav"}`, and a `"bubble"` with `background`
  and `text` colours (`#RRGGBB`) and `max_width` (120–400). Limits: 200
  lines, 280 characters each, WAV sounds of at most 2 MiB inside the pack.
  Sounds are validated, copied on import, written on export and kept by Art
  tab saves. Bundled Yohta ships a dozen lines.
- **Saying them.** `src/mascot/lines.rs` picks a line at random, never the
  same one twice in a row, and shows `//`-separated parts in turn for their
  reading time (1.5–8 s), or longer while the line's sound plays. Click,
  pick-up and drop fire from the character's own gestures; idle lines come at
  a random time in the chosen range and wait while another app is full
  screen, the character is busy or a line is showing; annotation ending
  fires when the character comes back.
- **Bubble.** Drawn in a band above the character inside its own window,
  centred over it and kept inside the window. It never takes clicks. The
  band exists only when the character has lines.
- **Settings.** The Character page's Lines section edits lines per trigger,
  each with an optional WAV (copied into managed storage), plus bubble
  colours and width, **Show speech lines**, **Quiet** (no idle or music
  lines; click, drag and drop lines stay), the idle interval and **Character
  volume**, which is separate from the pointer's tap sound. The preview's
  **Speak** shows the bubble. **Save lines** stores them as overrides beside
  the pack; **Reset to pack** returns to the pack's own.
- **Not yet.** Arbitration with the planned reaction sounds is left to that
  feature.

### Music response, level stage (character creation step 4)

Implemented 11 November 2026.

- **Meter.** `src/platform/music.rs` runs a background thread only while
  **React to playback** is on and the desktop character is showing (or the
  Character page is open, for the meter). Every 50 ms it reads the peak meter
  of each active audio session on the default output, skipping TinyPaw's own
  process, so taps and line sounds never count and no samples are captured.
  The default output is looked up again every 2 s, so device changes recover;
  an unreadable device reports Unavailable. Turning it off joins the thread.
- **Envelope.** `src/mascot/music.rs` smooths the loudest peak (fast attack,
  slower release), starts "playing" above one threshold and stops only after
  0.8 s below a lower one, so quiet passages don't flicker.
- **Motion.** The level, times **Music strength** (0–2), adds a relaxed
  two-per-second bob to each part through its own Strength, and a small body
  bounce. Reduced motion stops both. *Music starts* lines fire when playback
  begins.
- **Settings.** The Character page's Music section has **React to playback**,
  a live meter with its status (Off, Listening, Silent, Hearing playback,
  Unavailable) and **Music strength**. The preview's **Music** test simulates
  a few seconds of music without any audio.

### Follow beat trial (character creation step 5)

Implemented 11 November 2026, behind **Follow beat (trial)** (off by default).

- **Tracker.** `src/mascot/beat.rs` finds onsets as rises in high-passed
  loudness over 10 ms frames, the tempo from the autocorrelation of the last
  6 s of onsets (60–180 BPM, preferring moderate tempos so it doesn't lock to
  half or double time), and the phase from a comb over past onsets. It emits
  beats at predicted times, so it holds through off-beats and short gaps,
  and goes quiet when the pattern's confidence drops below the Sensitivity
  threshold.
- **Live input.** `src/platform/beat.rs` opens a WASAPI loopback stream of
  the default output with `cpal` (already used by rodio). The audio callback
  only downmixes and hands chunks to an analysis thread without blocking; a
  stalled stream is reopened, which follows device changes. It runs only
  alongside music response. Loopback hears TinyPaw too, so onsets are held
  off while TinyPaw plays a tap or line sound (approximate, as planned).
- **Motion.** With a followed beat, music nods use the beat's phase and the
  body hops on each beat; without one, the relaxed fixed bob continues.
  **Timing** shifts it 60 ms earlier or later; **Sensitivity** (Low, Normal,
  High) sets how clear the pattern must be.

**Trial results** (`cargo test beat_trial -- --nocapture`; synthetic tracks
at 44.1 kHz, fed in 10 ms chunks; a hit is within ±70 ms; scored after 4 s
to lock on, and 4 s after the tempo change):

| Signal | Beats | Hits | Missed | Extra | Mean offset |
| --- | --- | --- | --- | --- | --- |
| 80 BPM clicks | 21 | 21 | 0 | 0 | 0 ms |
| 120 BPM clicks | 32 | 32 | 0 | 1 | 0 ms |
| 160 BPM clicks | 43 | 43 | 0 | 0 | −3 ms |
| 120 BPM with off-beat hats | 32 | 32 | 0 | 1 | 0 ms |
| 100 → 140 BPM change | 24 | 24 | 0 | 0 | +1 ms |
| Speech-like bursts, 20 s | 0 | – | – | 0 | – |
| Silence, 20 s | 0 | – | – | 0 | – |

The test fails if a steady signal drops below 90 % hits, exceeds 10 % extra
beats or drifts beyond ±40 ms, or if silence or speech produce beats. Measured
confidence: speech-like bursts peak at 0.25, the click tracks never fall
below 0.45, hence Normal's threshold of 0.35. These are synthetic signals; real
music with soft attacks and loopback latency still needs listening checks,
which is why the option stays labelled a trial.

### Sharing (character creation step 6)

Implemented 11 November 2026.

- **Export carries your changes.** Export folds the user's overrides into the
  written pack: Motion tab settings into each part's `motion`, or into a new
  pose-level `"motion"` for a single image's sway, and the user's lines in
  place of the pack's, with their own WAVs copied into the folder's `lines/`
  and renamed to pack sounds. Pack sounds the exported lines no longer use
  are left out. What was written is validated like any import before Export
  reports success. Bundled Yohta exports the same way.
- **Import keeps your changes.** Overrides live beside the installed pack,
  keyed by ID, so Replace keeps them; motion that no longer fits the new
  parts is ignored rather than applied wrongly.
- **Checked by** `export_carries_motion_lines_and_sounds_and_imports_the_same`:
  a layered pack with changed motion and a line with the user's own sound,
  exported and imported into a fresh characters folder, loads with the same
  motion, line and sound.
- **Not done.** The plan's optional single-file pack. Folder export and
  import cover sharing today; an archive format (and ZIP import) is separate
  work, as the import-and-arrange plan also notes.

Completion checks:

- Yohta and an imported original character both work without code changes.
- One PNG installs successfully; missing optional layers degrade as specified.
- Layered eyes stay inside their masks during head motion and pupil extremes.
- Head/tail movement reveals no seams; switching pose preserves the ground
  anchor; dragging does not click or draw.
- The menu uses existing app commands, reflects availability and stays on screen.
- Blank pixels pass input through; annotation and desktop focus remain usable.
- Invalid or oversized packs cannot replace a working installation.
- Moving source files, restarting, changing DPI or disconnecting a display does
  not strand the mascot.
- Hide, reduced motion and music disable stop the relevant work; measure the
  memory/CPU cost against TinyPaw without the mascot enabled.
- No protected third-party file parsing is needed. Our PNGs, format, renderer
  and animation behaviour are sufficient.

### Bop styles

Music response has three bop styles, picked per character under Music →
**Bop style** and saved at once (with a short Music preview so the change
shows without audio):

| Style | Movement |
| --- | --- |
| **Nod** (default) | A small nod of each part and a light hop, as before. |
| **Groove** | A sharp dip (squash) on each beat that eases out, and a lean to one side on one beat and the other side on the next. Parts follow the lean a little late; no hopping. |
| **Rubber hose** | A hop every beat: squashed on landing, stretched at the top, rocking side to side, with ears, wings and hair flopping late. |

Two-beat moves count beats in the animator, so they alternate sides on a
followed beat or on the steady 120 BPM pace. Reduced motion turns all of it
off. The squash is now signed (negative stretches), and the desktop and the
Settings preview share one placement function, so the preview now shows the
music hop too. Without lines the desktop window keeps a little room above
the character for the hop.

A pack sets its own style with `"music": { "bop": "groove" }` (`nod`,
`groove` or `rubber_hose`; an unknown style bops the default way). The
user's choice is kept in the character's overrides as `"bop"`, removed again
when it matches the pack's, and baked into exports.
