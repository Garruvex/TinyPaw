# Desktop character and custom character imports

Status: proposed feature scope, 10 November 2026. This document defines future
work; it does not mean desktop integration or custom imports already exist.
The current [Yohta prototype](mascot-prototype.md) demonstrates the visual
direction in a separate preview window.

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
- A click opens the menu; dragging moves the character.
- Optional music response.
- Custom character import is part of the feature.

The numerical defaults and format below are proposed implementation decisions.
Custom art may use any style; the importer does not enforce Yohta's proportions.

## User experience

| Action | Behaviour |
| --- | --- |
| Enable | Show the selected character in a small transparent desktop window. Default disabled until the user chooses it. |
| Click | Open a compact menu beside the character. A second click, Escape or an outside click dismisses it. Closing the menu does not disable the character. |
| Drag | Move after approximately 5 logical pixels of movement. A drag never also opens the menu. Keep the character reachable on a connected display. |
| Menu | Offer the existing annotation On/Off, Settings and Mute controls, plus character/pose selection and Hide character. Dispatch app actions through the existing registry and respect their availability. |
| Look | Move pupils within their eye masks toward the cursor, with small limits and smooth settling. The head need not follow the cursor. |
| Idle | Occasional blinking, a gentle head tilt and tail sway. Avoid constant large bounces. |
| Music | Optionally bob the head and subtly compress/bounce the body in response to local playback. Stop and settle when playback is silent. |
| Pose | Choose Sitting or Standing if supplied by the pack. Changing pose preserves desktop position and display size without sliding the feet. |
| Hide | Remove the character window; retain its settings and installed artwork. The tray and keyboard shortcuts remain available. |
| Restart | Restore character, pose, size, position and motion choices. Recover to a visible display if the saved monitor is missing. |

Proposed defaults: sitting, about 160 logical pixels tall, eye tracking and subtle
idle motion enabled, music response disabled. Size range: 96–320 logical pixels.
Do not add automatic walking, pose switching, dialogue, reminders or pet-care
systems to the initial feature.

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

1. Choose **Add character** in Appearance.
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
