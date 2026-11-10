# Character creation plan

Proposed plan, 11 November 2026. It finishes the desktop character as a
creator tool: import art, set how each part moves, give the character speech
lines, and let it react to music. It builds on
[the character scope](mascot-feature.md), [the next-phase roadmap](next-phase.md)
(deliverables 2–4) and [the engine and artwork study](mascot-engine-study-2026-11-11.md).

Two decisions here change the earlier scope: speech lines are now in scope
(the scope deferred dialogue), and a left click reacts and speaks while a
right click opens the menu (agreed). See [Decisions](#decisions).

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

## Speech lines

- **Triggers:** Click, Drag (pickup), Drop, Idle (random, every 1–5 minutes by default), Music starts, Annotation ends. More triggers can come later.
- **Picking a line:** a random line from the trigger's list, never the same one twice in a row. `//` splits a line into parts shown one after another.
- **Bubble:** drawn in the character's own window, in a new slot above the head. It flips below or to the side near screen edges. It stays for its reading time, or until its sound ends.
- **Sound:** an optional WAV played through the existing tap-sound output at the tap volume. Mute tap sound mutes lines too.
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

1. **Left click — agreed 11 November 2026.** A left click reacts and speaks; a right click opens the menu (as in MateEngine), and the tray keeps the same entries. Escape or an outside click still closes the menu, and a drag never opens it. This replaces the next-phase plan's "keep left-click as the launcher gesture". It lands with step 2, when click reactions exist; until then left click keeps opening the menu so the character is never without one.
2. **Where behaviour lives — agreed.** Motion and lines live in the pack, so a shared character brings them; a user's edits are saved as overrides.
3. **Audio dependency — agreed.** Add the `windows` crate with only its audio features, alongside `windows-sys`, for per-session peak meters that leave out TinyPaw's own sounds. Stage B's loopback analysis still uses `cpal`.
4. **Petting — deferred.** Revisit once click and drag reactions feel right, with a rub gesture that can't fire during a drag.
5. **Scope change — done.** [The character scope](mascot-feature.md) now includes speech lines and the right-click menu; reminders and alarms stay out.

## Out of scope

Keyframe timelines, mesh deformation, a skeleton editor, walking or roaming,
GIF/APNG parts, props and dolls, reminders/alarms/to-do lines, multiple
characters at once, and parsing other apps' character files.
