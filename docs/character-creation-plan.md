# Character creation plan

Proposed plan, 11 November 2026. It finishes the desktop character as a
creator tool: import art, set how each part moves, give the character speech
lines, and let it react to music. It builds on
[the character scope](mascot-feature.md), [the next-phase roadmap](next-phase.md)
(deliverables 2–4) and [the engine and artwork study](mascot-engine-study-2026-11-11.md).

Two decisions here change the earlier scope: speech lines are now in scope
(the scope deferred dialogue), and a left click reacts and speaks while a
right click opens the menu (agreed). See [Decisions](#decisions).

## UI and interaction revision

The [character creator UI and flow plan](character-creator-ui-flow-plan.md) supersedes the earlier layout, mandatory ground picker and global pivot-edit workflow below. It specifies automatic resting points, cursor-anchored carrying, contextual animation joints, guided eyes, a control-effect audit and a unified draft/save flow. It is proposed work; earlier implementation notes remain historical evidence.

New behaviors that came up in that work (compression reactions, held poses, Head bob, falling, window landing, pickup areas) are in the [character behaviors plan](character-behaviors-plan.md). Several contradict [Out of scope](#out-of-scope) below, so each waits on a decision recorded here.

## What BUG Clicker does, and what we take

From the developer's [release notes](https://github.com/BUG-INVADE/Bug-Clicker-download/releases)
(v1.18.2–v1.19.4). These describe documented behaviour, not its internals.

| Area | BUG Clicker | TinyPaw |
| --- | --- | --- |
| Motion | No keyframes. Each part has a **type** (ears, tail, wings, hair, antennae, props). A preset such as "Upright ears" or "Floppy ears" sets several values at once; a few sliders (drag swing 0–200) and an exact-number fine-tune sit behind it. Floppy ears and antennae swing like a springy pendulum. | Same model: part types, one-click presets, a strength and speed slider each, numbers under **Fine-tune**. A damped spring is the only "physics". |
| Expressions | Head image slots per state: idle, closed eyes (blink), pressed, petted, dragging (head4/head5), plus a dragging body. | Expression slots on the head: Idle, Blink, Pressed, Dragging, Talking; Petted later. A missing slot uses Idle. |
| Drag | Body swings with drag speed; tails trail opposite the drag; ears droop; wings flap. | Drag velocity feeds every part's motion as an input. |
| Lines | A Line Editor; idle lines; `//` splits a line into parts said in turn; an optional sound per line, with the bubble held until it ends; bubble styles with 9-slice images. | Lines by trigger, `//` parts, an optional WAV per line, one bubble style with colours first. |
| Music | "Nods on the beat" and "Right on every sound"; source picker, meter, BPM, gain (auto, ×10, ×50, later ×100), sensitivity, timing offset. Excludes its own sounds. | Level response first, beat second (as agreed). Meter and silence state before any tuning. |
| Left out | Reminders, alarms, to-do lines, props and dolls, GIF/APNG parts, outline/glow. | Out of scope for this plan. |

## Settings → Character, once finished

The Character page becomes a small editor around one pinned preview, like the
Pointer page.

```
┌ Characters ───────┬ Art · Motion · Lines · Music ──────────────┬ Preview ─────────┐
│ ● Yohta           │                                            │  [character]     │
│   My cat          │   (the selected tab's controls)            │  Light / Dark    │
│ + Add character   │                                            │  Test: Click ·   │
│ Import folder     │                                            │  Drag · Speak ·  │
│                   │                                            │  Music beat      │
│ Show on desktop ▣ │                                            │                  │
│ Size ─────●──     │                                            │                  │
└───────────────────┴────────────────────────────────────────────┴──────────────────┘
```

| Tab | Holds |
| --- | --- |
| **Art** | Canvas, ground point, and the part list: add a part image, pick its type, click its pivot. Expression slots for the head. Eyes: white, pupil, mask, blink. |
| **Motion** | Per part: a preset, then Strength and Speed. Per reaction (Idle, Click, Drag, Music): which parts move and how much. **Fine-tune** reveals exact numbers. |
| **Lines** | Lines grouped by trigger, each with optional sound; how often idle lines appear; bubble colours and size; Mute lines. |
| **Music** | React to playback on/off, live level meter and silence state, Strength; later Follow beat, Sensitivity and Timing. |

The preview's **Test** buttons fire the same reactions as the desktop, so a
creator can try each one without leaving Settings.

## Improvement: import and arrange cropped parts

Implemented authoring increment, 11 November 2026. This extends step 2 with cropped PNG placement while keeping installed runtime packs on schema v1.

### Current implementation

- **Settings → Character → From parts…** chooses the body first and creates a padded stage. **Edit art → Add part PNGs…** selects multiple differently sized PNGs. Recognized head/tail/ear filenames suggest roles; other art uses Other, with static eyes/eyebrows attached to the head when one exists. Roles remain editable.
- **Arrange artwork** drags the selected visible image, with proportional scale, angle and X/Y fields. **Set attachment point** moves only the joint. Arrow keys nudge one canvas pixel; Shift nudges ten. A drag stays captured across transparent gaps. Head arrangement carries its children, authored faces and joints with it.
- Body, part and authored face layers can be selected. Raise/Lower changes sibling draw order; Back/Front and On head retain their runtime meanings. The preview uses the same source-coordinate transform as baking, supports 50–300% zoom and Light/Dark/Grid backgrounds, and outlines the selected source rectangle.
- Single-pose stages can be explicitly resized from 1–2048 pixels per side without moving existing artwork in pixel coordinates. Multi-pose canvas resizing is withheld until every pose can be edited together. Clipping and invalid joints/fields are explained before saving. Undo/Redo retains the latest 30 editing states.
- Opening art, decoding selected files and baking/saving run on workers. A bad source reports its filename while valid sources remain in the draft. Saving uses the recoverable installation transaction. Cancelling before commit preserves the installed pack; a commit already started finishes or rolls back and reconciles the catalog.
- Save writes canvas-sized PNGs plus optional `character.json.editor` version-1 metadata with the original PNGs, source sizes and placements. Reopen uses these originals. Folder export/import preserves and validates them; runtime-only packs still reopen as aligned sources. Paths, dimensions and combined decoded storage are bounded before decoding.
- Complete eyes remain ordinary static layers. Optional tracking requires white/pupil/mask art; **Keep static layers** converts the white/pupil art to ordinary layers without requiring a complete tracking setup.

Automated checks cover source preservation after deleting originals, baked pixels versus placement, export/import metadata, invalid metadata and source files, cancelled save, head-child/joint movement, stage resizing, Undo/Redo and drag coordinates across zoom levels. The local seven-PNG Yohta load/save/reopen acceptance check also passes.

Validation on 11 November 2026: **123 regular tests passed**, plus the explicitly run seven-crop Yohta acceptance test. Strict Clippy passed. The release executable linked successfully; Cargo could not replace the normal `tinypaw.exe`, so the verified linked binary was copied to `target/release/tinypaw-character-creator.exe`. Native desktop visual checks remain open.

Still open in this increment: dropping files directly onto the editor, native visual verification at 100/150/200% display scaling, live motion/reaction tests in the pinned editor preview, and measured responsiveness/peak memory on the largest supported pack. Motion controls, lines and music remain subsequent deliverables.

The workflow and acceptance criteria below remain the target; references to the former canvas restriction describe the starting point.

### Problem and design decision

The separated Yohta sheet produces seven useful PNGs: head, body with paws,
tail, two complete eyes and two eyebrows. Their cropped dimensions differ.
The original `fit_png` path in `src/mascot/pack.rs` accepted only the character canvas
size or the original quick-import image size. The original editor therefore rejected
ordinary cropped parts and moved pivots rather than arranging artwork.
The extraction's `parts.json` records sheet coordinates and is not an importable
`character.json`; sheet positions are not assembled character positions.

Keep a shared canvas for installed packs and rendering. Accept differently sized
images in the creation editor and prepare the shared-canvas layers automatically
when saving. Users should not need an external drawing tool or hand-written
manifest merely to place an existing part.

### Creator workflow

1. Start with **Add character → From parts**, or **Edit art → Add parts** for an
   existing character. Select multiple PNGs or drop them on the editor. Choose
   the body explicitly; filename guesses for other roles remain editable.
2. Create a draft stage from the body, with transparent room around it. Preserve
   source proportions. Keep existing canvases unchanged when editing a pack;
   offer an explicit canvas-size control with a preview if more room is needed.
3. Place new cropped parts near the centre at their original pixel scale when
   they fit. If a source is too large, propose a proportional fit and show its
   scale. Never silently stretch it or assume source-sheet positions assemble it.
   Existing canvas-sized layers and original-source-sized layers retain their
   current alignment behaviour.
4. Select a layer to drag, resize proportionally and rotate it. Provide move,
   scale and angle fields, keyboard nudging, zoom, and light/dark/checkerboard
   backgrounds. Keep **Arrange artwork** and **Set attachment point** as distinct
   modes so moving a part does not accidentally move only its pivot.
5. Choose type, **On head**, and Back/Front. Support explicit ordering among
   siblings. Complete eyes and eyebrows can be ordinary front layers attached
   to the head, using the existing `other` type and Still preset.
6. Set attachment points at the neck, tail base or other joint. Then test head
   turns, tail motion, click and drag. Highlight clipping and let the creator
   adjust the stage or placement before saving. Warn about gaps exposed by
   movement; missing artwork behind overlaps still needs drawing.
7. **Save** prepares matching transparent layers and writes `character.json`
   automatically through the existing staged installation. Cancel leaves the
   installed character unchanged. Export produces a folder that Import folder
   accepts without manual changes.

### Eyes and expressions

Complete eye PNGs are valid static art and move with their head. Eye tracking
is an optional upgrade requiring separate white, pupil and mask images, with
an optional blink image. Explain those requirements beside **Enable tracking**;
an incomplete tracking setup must not prevent saving the static character.
Offer an explicit choice to keep it static or finish the tracking setup.

Expression replacements use the same arrangement tools. Start a replacement
with its base head's placement, but allow adjustment when its crop differs.
Do not claim to extract pupils or reconstruct hidden ears, paws or facial
art automatically. If a body already contains a head or tail, adding another
layer can duplicate it; show that in the preview and explain the needed artwork.

### Implementation boundaries

- Add a draft placement model in `src/mascot/editor.rs`: source image, translation,
  uniform scale, rotation, attachment point, parent and order. Retain original
  pixels during editing; resample from the source rather than from previous
  transformed results. Define attachment points in canvas coordinates and move
  them with the artwork during arrangement; pivot mode adjusts the joint alone.
- Keep strict canvas validation for prepared folder packs in
  `src/mascot/pack.rs`. Separate editor source loading from `fit_png`'s current
  alignment rules. Inspect dimensions and enforce source-size, file-size and
  aggregate decoded-memory limits before allocating image buffers. Keep the
  existing part/image limits, with a clear explanation when reached.
- Preview and save must use the same placement transform, clipping, layer order
  and parent rules. Bake placement into transparent canvas-sized PNGs at save;
  runtime rotation still uses the chosen joint. Rebuild the desktop input
  silhouette from the resulting art so empty space remains click-through.
- Preserve source images and draft transforms in optional, versioned editor
  metadata in managed storage. Keep them with editable exports so reopening
  permits further adjustment without repeated resampling. Runtime packs remain
  schema v1 and work without this metadata; packs without it reopen as aligned
  canvas-sized sources. Validate editor metadata and referenced sources too,
  and include their memory/disk cost in preparation limits.
- Keep preview edits isolated until Save. Use the existing installation
  transaction and preserve the previous pack if preparation or commit fails.
  Reuse the planned background preparation from the
  [architecture improvement plan](architecture-improvement-plan.md) for decoding
  and baking; avoid blocking Settings during large imports.
- Keep PNG-only input and folder export/import for this increment. ZIP import,
  automatic sheet segmentation and a new runtime pack schema are separate work.

### Delivery and acceptance

| Increment | Deliverable | Acceptance |
| --- | --- | --- |
| 1. Flexible source import | Multi-PNG selection, role assignment and draft placement | All seven files in `assets/mascot/yohta/separated-parts/` load without a canvas-mismatch error. A malformed, empty or oversized file reports its own reason without discarding valid draft work. |
| 2. Arrangement | Move, proportional resize, rotate, ordering, parent and separate pivot modes | Assemble Yohta with head, tail, complete eyes and eyebrows; the eyes and eyebrows follow head motion. Zoom and display scaling do not change placement or attachment points. |
| 3. Save and reopen | Canvas baking, generated manifest, editor metadata and compatible export | Save, restart and reopen preserve placement, order and pivots. Moving or deleting the original files does not break the character. An exported folder re-imports with the same appearance and reactions. |
| 4. Validation and polish | Tracking guidance, clipping feedback, undo/redo and failure handling | Static eyes save without tracking assets. Cancel, failed save and failed replacement preserve the installed character. Undo/redo restores a placement or pivot edit. Transparent gaps pass clicks to the desktop. |

Targeted automated checks should cover transform/pivot coordinates, legacy
canvas alignment, bake/preview agreement, metadata round trips and failed-save
preservation. Manually verify the seven Yohta crops at 100%, 150% and 200%
display scaling, including rotated edges, head-child layers, movement at overlap
seams and desktop click-through. These are planned checks, not completed tests.

## Data: one pack carries art, motion and lines

The pack (manifest v1 in the scope) grows so a shared character brings its
behaviour with it. Users' own changes are stored as overrides, never written
into an installed pack.

```json
{
  "schema_version": 1,
  "id": "my-cat",
  "name": "My cat",
  "canvas": [1024, 1024],
  "default_pose": "sit",
  "poses": {
    "sit": {
      "ground_anchor": [0.5, 0.95],
      "body": "sit/body.png",
      "parts": [
        { "type": "head", "image": "sit/head.png", "pivot": [0.43, 0.61],
          "expressions": { "pressed": "sit/head-pressed.png", "dragging": "sit/head-drag.png" } },
        { "type": "tail", "image": "sit/tail.png", "pivot": [0.65, 0.72],
          "motion": { "preset": "wag", "strength": 0.6, "speed": 0.5 } },
        { "type": "ear", "image": "sit/ear-left.png", "pivot": [0.38, 0.30], "parent": "head",
          "motion": { "preset": "floppy" } }
      ],
      "eyes": [ { "white": "…", "pupil": "…", "mask": "…", "blink": "…", "max_offset": [0.012, 0.009] } ]
    }
  },
  "lines": {
    "click": [ { "text": "Hi!" }, { "text": "That tickles // stop it", "sound": "lines/giggle.wav" } ],
    "idle":  [ { "text": "…" } ],
    "drag":  [ { "text": "Whoa!" } ]
  },
  "bubble": { "background": "#FFFFFF", "text": "#1B1B1F", "max_width": 220 }
}
```

- A single PNG is still a complete pack: `body` only, no parts, no lines.
- `parent` lets ears, hair and eyes ride on the head; one level of parents is enough.
- Motion values are optional; a missing value uses the preset, a missing preset uses the type's default.
- Validation rules and limits from the scope apply, plus: at most 16 parts, 200 lines, 280 characters per line, line sounds WAV only and 2 MiB each.

## Motion model

Each part has a **type** and a **preset**; the preset is a small function of
time and inputs. All presets share the same inputs, so the renderer has one
code path.

| Type | Presets | What drives it |
| --- | --- | --- |
| Body | Still, Breathe, Bounce | Music level, press (squash) |
| Head | Still, Tilt, Nod | Idle time, music, press, cursor (small turn toward it) |
| Tail | Sway, Wag, Hang | Idle time, drag (trails opposite), music |
| Ear | Upright (twitch), Floppy (spring) | Head motion, drag (droops), music |
| Wing | Fold, Flap | Drag (flaps), music |
| Hair | Spring | Head motion, drag |
| Eyes | Track cursor | Cursor, blink timer |

- **Inputs:** elapsed time, cursor direction, drag velocity, press, music level, and later beat pulses.
- **Spring presets** follow their parent with a damped spring, so floppy ears lag the head and settle. It's one small spring per part, not a physics engine.
- **Reactions** choose an expression and add short motion on top of the idle motion:

| Reaction | Default |
| --- | --- |
| Click | Pressed face, a squash on press, a line |
| Drag | Dragging face, lean with speed, tail trail, a line on pickup |
| Drop | Settle back to Idle |
| Idle | Blink and a gentle sway; a line now and then |
| Music | Bounce and nod by level, then by beat |

- **Priority** follows the engine study: hidden > drag > menu/press > music > idle. Blink never overrides a drag face.
- **Reduced motion** stops sway, bounce and spring motion. Expressions, lines and gaze snapping remain.
- **Rendering** stays in the existing Iced canvas rig. Transforms are world, then body, then part pivot, then child parts. The hit-test silhouette is rebuilt only when the artwork or expression changes.

## Reaction sounds

Character reaction audio is a separate, independently deliverable feature. See [Character reaction sounds](character-reaction-sounds.md) for its editor, gesture triggers, playback/mute policy, asset storage and acceptance criteria. Speech text and bubbles are not prerequisites. This feature remains unimplemented.

## Speech lines

- **Triggers:** Click, Drag (pickup), Drop, Idle (random, every 1–5 minutes by default), Music starts, Annotation ends. More triggers can come later.
- **Picking a line:** a random line from the trigger's list, never the same one twice in a row. `//` splits a line into parts shown one after another.
- **Bubble:** drawn in the character's own window, in a new slot above the head. It flips below or to the side near screen edges. It stays for its reading time, or until its sound ends.
- **Sound:** an optional WAV uses the shared audio engine in the character voice category. Character volume and Mute all sounds apply; pointer tap mute/volume remain pointer-only. See the [reaction sound mute policy](character-reaction-sounds.md#mute-and-volume-decisions). The Lines implementation must define arbitration with reaction clips so a trigger does not play both at once.
- **Quiet rules:**
  - **Mute lines** turns off idle and music lines but keeps click lines.
  - Lines don't show while annotation owns input.
  - Idle lines wait while a full-screen app is in front.
- **Accessibility:** bubble text follows the system text scale. Reduced motion shows the bubble without its pop-in.
- **Not included:** typing effects, multiple bubble styles, 9-slice images, reminders and alarms.

## Music response

**Stage A — level.**
- **What it measures:** Windows playback level for every audio session except TinyPaw's own, through each session's peak meter. That leaves out our taps and line sounds without guesswork and needs no audio samples.
- **Smoothing:** attack and release smoothing, separate start and stop thresholds, and a silence state.
- **Effect:** the level drives each part's music motion through the Strength slider.
- **Settings:** shows the live meter and "Silent / Hearing playback / Unavailable" before any tuning.
- **When it stops:** polling stops when disabled, hidden, or during annotation. A device change resets the envelope.

**Stage B — beat (trial).**
- **What it needs:** loopback samples of the default output (rodio's `cpal` can open WASAPI loopback). Onset detection on short frames, plus a tempo estimate that holds through off-beats.
- **Effect:** nods land on predicted beats.
- **Controls:** Follow beat, Sensitivity (Low, Normal, High) and Timing (Earlier, Default, Later).
- **Own sounds:** muted from the analysis by time window while TinyPaw is playing one, which is approximate. The UI says so.
- **Before it ships:** test against click tracks at 80, 120 and 160 BPM, a tempo change, speech and silence. Record missed, extra and late beats.

## Build order

Each step ships on its own and keeps the character working.

| Step | Delivers | Done when |
| --- | --- | --- |
| 1. Packs and one-PNG import | Character pack loader, bundled Yohta moved onto it, Add character from one PNG, Import folder, Characters list, remove/export. (Next-phase deliverable 2.) | A first-time user puts their own PNG on the desktop in under a minute; restart, moved source file, duplicate ID and failed import all behave as the scope says. |
| 2. Parts and Motion tab | Generic part list with types, pivots and parents; presets with Strength/Speed; Fine-tune; reactions for Idle, Click, Drag and Drop; expression slots. Yohta re-authored as parts. | A layered pack with head, tail and two floppy ears animates in preview and on the desktop with no seams; reduced motion stops it; CPU stays near the Stage 1 numbers. |
| 3. Lines | Bubble slot in the character window, Lines tab, triggers, `//` parts, line sounds, quiet rules. | Click, drag and idle lines appear without stealing focus or input; the bubble stays on screen at every edge; Mute lines works. |
| 4. Music level | Session level meter service, Music tab with meter and Strength, music motion per part, Music-starts trigger. | Playback moves the character, silence settles it, TinyPaw's own sounds don't, switching output devices recovers; zero cost when off. |
| 5. Beat trial | Loopback analysis, Follow beat and its controls. | Click-track results are recorded and judged good enough before the label ships. |
| 6. Sharing | Export includes motion and lines; import keeps the user's overrides; optional single-file pack. | A pack exported from one PC imports with identical behaviour on another. |

Steps 3 and 4 depend only on step 2's reaction plumbing and can be built in either order.

## Decisions

1. **Left click — agreed 11 November 2026.** A left click reacts and speaks; a right click opens the menu (as in MateEngine), and the tray keeps the same entries. Escape or an outside click still closes the menu, and a drag never opens it. This replaces the next-phase plan's "keep left-click as the launcher gesture". Landed with step 2a.
2. **Where behaviour lives — agreed.** Motion and lines live in the pack, so a shared character brings them; a user's edits are saved as overrides.
3. **Audio dependency — agreed.** Add the `windows` crate with only its audio features, alongside `windows-sys`, for per-session peak meters that leave out TinyPaw's own sounds. Stage B's loopback analysis still uses `cpal`.
4. **Petting — scoped 12 November 2026.** The [headpat and sleeping implementation scope](character-headpat-sleep-plan.md) replaces the earlier deferral with button-free rubbing, optional happy artwork and shared gesture recognition that cannot fire during a drag. The same scope adds opt-in sleep after inactivity/quiet and configurable wake causes. Implemented in phases with automated coverage and a Windows preview smoke check. Hardware release checks are recorded in that scope.
5. **Scope change — done.** [The character scope](mascot-feature.md) now includes speech lines and the right-click menu; reminders and alarms stay out.
6. **New behaviors — agreed 11 November 2026.** From the [character behaviors plan](character-behaviors-plan.md): click and beat Compress, a held pose PNG while carried, one alternate PNG for a reaction, the Head bob beat style, and an opt-in straight fall to the screen edge after release are in scope. Head bob ships only once Follow beat is no longer a trial. PNG frame sequences, landing on window tops and restricted pickup areas are deferred.

## Out of scope

Keyframe timelines, mesh deformation, a skeleton editor, walking or roaming,
GIF/APNG parts, props and dolls, reminders/alarms/to-do lines, multiple
characters at once, and parsing other apps' character files.

One exception, from decision 6: a single whole-character PNG may replace the
drawing while carried or for one reaction. Frame sequences stay out.
A released character may also fall straight to the screen edge when the
creator turns that on; it still never walks or roams.
