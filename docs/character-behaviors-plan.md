# Character behaviors plan

## Current composition and behavior roadmap

The [character and editor improvement plan](character-editor-improvement-plan.md) defines how posture, motion, expression, held items and sleep recipes use shared artwork. Its [artwork rules](character-artwork-rules.md) govern new image requirements. The dated status and decisions below preserve the earlier behavior review.

Proposed, 11 November 2026. Scope decisions for B1–B8 were agreed the same day.

**Status:** B1 is implemented but not yet checked natively. Each event entry takes a `visual` of `none` or `compress`. With no visual set, a click keeps the legacy squash, now played as a Compress after release so it never moves a held point; other events do nothing. New one-image characters and new from-parts characters are written with explicit `none` for Click, Pick up and Drop. Behavior → Click has a Visual picker. Compress is also a beat style (`Bop::Compress`, nothing but one bounded squash per beat), independent of the Click switch. Overlapping Compress pulses restart one envelope instead of stacking, and the preview's Music test is now labelled Try beat. B2 and B3 are implemented too, also not checked natively. Pick up has Held pose: Keep current artwork, or Choose PNG, then Left/right, Up/down and Size to line it up. Click and Drop have Use artwork. Pictures are canvas-sized PNGs on the character's own stage, so the carry contract holds unchanged. An imported PNG is trimmed, fitted and stood on the stage bottom, then saved in managed storage. In packs they live under `reactions/` and count as pack images for validation, copying, export and the image and memory limits. The desktop and the Test preview draw the held pose while carried and a Click or Drop picture for 0.7 s after its event. Clearing the held pose keeps Pick up's dialogue and sound. B6 is implemented too, not checked natively. Drop → After release offers Stay here (the default) or Fall (`after_release`). Fall drops the character straight down to the bottom of the usable area on the monitor it was released on, accelerating, with a small Compress on landing. Picking it up mid-fall stops the fall. Drop fires at release and landing never fires it again. The landing spot is saved, and reduced motion places the character there without falling. B5 (Head bob) is not implemented: it waits on the Follow beat trial, as agreed.

These features were split out of the [character creator UI and interaction plan](character-creator-ui-flow-plan.md) during review. That plan fixes and reorganises what exists. This one adds new behavior, and several items contradict the current scope, so each needed a decision before work starts. The feature descriptions below are carried over unchanged in intent.

Prerequisites from the UI plan: the shared draft (step 2), stable grabbing (step 3) and the Behavior section (step 4). Anything that deforms or replaces the picture while carried also needs expressive carry (step 6).

## Decisions

Each row conflicts with, or extends, something already agreed. The outcomes are recorded in the [creation plan's Decisions](character-creation-plan.md#decisions). Deferred rows are not built.

| # | Feature | Conflicts with | Decision, 11 November 2026 |
| --- | --- | --- | --- |
| B1 | Click and beat **Compress** on one image | Nothing; today's press squash becomes a choice | **Accepted.** Smallest item, and it makes the single-PNG path expressive. |
| B2 | **Held pose PNG** while carried | [Character scope](mascot-feature.md): "do not add … pose switching" | **Accepted** as a narrow exception: one whole-character replacement, only while carried. |
| B3 | **Use artwork** for a reaction (one alternate PNG) | Same pose-switching rule | **Accepted** with B2; it is the same mechanism on Click and Drop. |
| B4 | **PNG frame sequences** | Creation plan, Out of scope: keyframe timelines and GIF/APNG parts | **Deferred.** It is animated-image support under another name; revisit after B2/B3 are used. |
| B5 | **Head bob** beat style | Depends on the beat trial (creation plan step 5), not yet judged | **Accepted**; ships only when Follow beat loses its trial label. |
| B6 | **Fall** to the screen edge after release | Creation plan, Out of scope: walking or roaming | **Accepted**, limited to one straight fall to the screen edge, off by default. |
| B7 | **Land on window tops** | Same, plus continuous window tracking while resting | **Deferred.** Highest cost and platform risk of the set. |
| B8 | **Limit pickup to an area** | Nothing | **Deferred** until a creator asks; any-pixel pickup covers the normal case. |

## Defaults for a new single-image character

**Choose PNG → Try character → Use character** stays the complete path. Try character opens with the picture at rest. A new single-image character starts with every response set to None and written explicitly, as the UI plan's storage rules require:

| Section | What the creator chooses | New single-PNG default |
| --- | --- | --- |
| Click, Pick up, Drop | Open an event and independently add a visual, dialogue line or sound | None for each. Ordinary carrying still works. |
| After release (B6) | Stay here or Fall | Stay here. |
| Music | Level response as shipped; a beat style under Follow beat | Off. |
| Moving parts | Add a part in Artwork, then choose what moves it | No parts required. An added tail starts at None. |

Loading an older pack preserves its previous effective behavior even where the old format omitted a field.

## B1 and B3: a reaction with one image

In **Behavior → Click → Visual**, offer **None**, **Compress**, or **Use artwork**. Compress briefly shortens and widens the original picture and then restores it. It works with one PNG. A short release fires Click once. Crossing the carry threshold fires Pick up once and suppresses Click. A real drag release fires Drop once. Optional held feedback can start on press only if it preserves the contact point.

Under **Behavior → Music → Follow beat**, the **Compress** style applies the same visible effect once per accepted beat. Click and music have independent switches. Silence or a missing beat leaves the image resting. A labelled **Try beat** pulse works without an audio source; it does not imply that live detection works on that device. Overlapping pulses restart a bounded envelope instead of multiplying scale. Carrying takes priority over any deformation that could move the held pixel. Level response is unaffected and remains the default music reaction.

**Use artwork** accepts one alternate transparent PNG. Preview its alignment over the resting artwork. Static poses use a common stage and registration so switching art cannot jump the held point. Dialogue and sound may accompany any visual choice, including None.

## B2: held pose PNG

Under **Behavior → Pick up**, show **Held pose** with choices **Keep current artwork** (default) and **Choose PNG**. For example, use a sitting cat as the normal image and a dangling cat as the held image. The held PNG replaces the entire character drawing, including its layered parts, for the duration of carrying. Pressing alone keeps the sitting pose. Crossing the drag threshold shows the dangling pose, which stays visible even when the pointer stops moving with the button down. Releasing restores the normal drawing or starts the configured Drop visual. A held pose never repeats pickup dialogue or audio.

After choosing the PNG, show it over a faded normal pose. Allow moving and resizing it so the pickup region lines up across the two poses, even when their crops differ. **Try pickup** lets the creator grab, hold still, move and release in the preview. Clearing the held PNG restores ordinary carrying and preserves pickup dialogue and sound.

The grabbed point must map between the two drawings. Store the alignment as one transform from the normal stage to the held image; the carry contract `t = c − F(g)` then applies with the held image's `F`.

## B4: frame sequences (deferred)

An ordered PNG sequence with one frame rate, a preview, and a managed copy of every frame. Play Click and Drop once, loop the held pose only while carried, and restore the resting art on completion, cancellation or release. Needs limits on frame count and decoded memory before it can be scoped.

## B5: Head bob beat style

For a character with a separate Head layer, suggest **Head bob** when Follow beat is enabled. The whole head travels straight down toward the shoulders, hits its lowest point on the beat, then springs back up and settles before the next hit. The head keeps its shape and angle. The torso and feet stay planted. Eyes, eyebrows and every other head attachment inherit the same offset once. The motion reference is the creator's description of the bob in [Mystery Skulls Animated – Ghost](https://www.youtube.com/watch?v=YlEb3L1PIco).

Starting numbers, to tune in the preview rather than leave to the reference alone:

| Value | Start at |
| --- | --- |
| Dip distance at full Strength | 12% of visible head height |
| Downstroke | Begins 60 ms before a predicted beat; lowest point lands on the Timing offset |
| Recovery | Back to rest within 45% of the beat interval, with one small overshoot |
| Fast tempo | Above 160 BPM, scale the dip down so recovery still completes |
| Unpredicted first hit | Dip on detection, recover promptly |

Put **Style**, **Strength** and **Try four beats** together under Music. The preview plays a short loop with a visible beat marker. Use the same pose evaluation on the desktop.

For a single PNG, suggest **Compress** instead and explain that adding separate head art unlocks Head bob. Preserve saved Nod, Groove and Rubber hose selections and their motion.

Rig change: `Pose` carries part angles only. Head bob needs a per-part translation applied before children, so head attachments inherit it once. Verify the feet stay fixed, face layers stay registered, and the head returns to its authored position on silence. Suspend the bob during pickup so it cannot move the held contact.

## B6 and B7: drop gravity and window collision

**Stay here** leaves the character where the user released it. **Fall** starts downward motion from that position. The first implementation lands on the release monitor's usable bottom edge using the automatic resting point. It exposes no mandatory ground-level setting. Pickup during a fall immediately cancels falling. Gravity is suspended throughout carrying. Release dialogue fires at release; landing never fires Drop a second time. Reduced motion places the character at the landing spot without the fall.

**Land on window tops** (B7, deferred) is a separate opt-in under Fall. Scope the first version to one-way top edges: moving down from above can land on an exposed window top beneath the character's body. Wall collisions, bouncing off corners and throwing are outside that first version. Ignore TinyPaw's own windows, hidden or minimized windows, and covered stretches of a top edge. A supporting window moving keeps the feet on it while that support remains valid. Closing or minimizing it resumes the fall. On platforms without reliable window geometry, offer the screen edge alone and explain availability. Use a sample platform in preview, clearly labelled.

Cost to settle before B7 is accepted: resting on a window means watching that window for as long as the character sits there. That must be event-driven, not a poll, and must stop when the character is hidden or disabled, in line with the idle-cost rules in the [reaction sound plan](character-reaction-sounds.md).

## B8: pickup area (deferred)

By default **any visible pixel is a handle**. The press location becomes the temporary pivot, including when a held PNG appears. There is nothing to place during creation. If creators need a specific handle such as a collar, **Limit pickup to an area** lets them paint or resize the allowed region, then try grabbing inside and outside it. A press outside the region can still click the character but cannot start carrying it. Even inside the region, preserve the actual contact point. A tail joint and the resting point are never pickup handles.

## Data

Follow the UI plan's storage rules: artwork in the pack, behavior choices in `.overrides/<id>.json`, merged into the pack on export. Proposed shape, to confirm against the loader before building:

```json
{
  "reactions": {
    "click":  { "visual": "compress" },
    "pickup": { "visual": "none", "held_pose": { "image": "held.png", "place": [0.02, -0.04, 1.0] } },
    "drop":   { "visual": { "image": "drop.png", "place": [0.0, 0.0, 1.0] }, "after_release": "stay" }
  },
  "bop": "head_bob"
}
```

- Every field is optional. A missing `reactions` object means legacy behavior, including the built-in press squash. A new character writes `"visual": "none"` explicitly.
- `enabled: false` on a reaction keeps its setup; removing the entry is what Clear does.
- Images named here are managed copies with the same size and count limits as other pack art, and count toward the decoded-memory limit.
- **Answered, 11 November 2026:** existing builds read `character.json` and overrides as loose JSON and ignore fields they do not know (`parse_manifest`, `load_overrides_in` in `src/mascot/pack.rs`). These stay optional schema-version-1 fields. An older build that saves overrides rewrites only the fields it knows, dropping `reactions`; note this in the release notes.

### One reaction object per event (agreed in phase 0)

The [reaction sound plan](character-reaction-sounds.md) proposed a separate `reaction_sounds` block. It merges into this `reactions` object so that one event row edits one entry, as the UI plan requires. The same shape is valid in `character.json` (author defaults) and in `.overrides/<id>.json` (user changes); an override entry replaces the pack's entry for that event as a whole.

```json
{
  "reactions": {
    "sound": { "enabled": true, "volume": 0.7, "idle_enabled": false, "idle_interval_seconds": [60, 300] },
    "click":  { "visual": "compress", "sound": { "builtin": "duck-toy" }, "gain": 1.0, "cooldown_ms": 250 },
    "pickup": { "visual": "none", "held_pose": { "image": "held.png", "place": [0.02, -0.04, 1.0] } },
    "drop":   { "visual": { "image": "drop.png", "place": [0.0, 0.0, 1.0] }, "after_release": "stay" },
    "idle":   { "enabled": false }
  }
}
```

- Event keys are `click`, `pickup`, `drop` and `idle`; the sound plan's `drag_start` and `drag_end` are `pickup` and `drop`.
- `sound`, `gain` and `cooldown_ms` follow the sound plan's validation rules unchanged. Dialogue lines stay in the existing `lines` block, keyed by the same triggers.
- `reactions.sound` holds the character-wide sound settings. On a receiving install `idle_enabled` is forced off until that user opts in.

## Delivery and acceptance

Only accepted rows are built, in this order:

| Order | Work | Acceptance gate |
| --- | --- | --- |
| 1 | B1: None defaults and independent click/beat Compress | A clean single-PNG import, save and reopen. Each switch alone, both together and both off behave as set. An existing animated pack keeps its motion. Compress never moves a held point. |
| 2 | B2 and B3: held pose and Use artwork | Short click, pickup, hold still, release and cancelled drag: the art restores and each eligible event fires once. A dialogue-only pickup works. The grabbed point does not jump when the held pose appears. Export and re-import keep the pose. |
| 3 | B5: Head bob | Feet fixed, face layers registered, rest position restored on silence, suspended while carried. Ships only once Follow beat is no longer a trial. |
| 4 | B6: Fall to the screen edge | Release over empty space, pickup mid-fall, mixed-DPI monitors and reopening a resting character. Reduced motion skips the fall. |

B4, B7 and B8 have no delivery slot until their decisions change.

## Headpat and sleep extension

The [headpat and sleeping implementation scope](character-headpat-sleep-plan.md), scoped 12 November 2026, adds button-free petting and opt-in sleep with touch, click/pickup, audio and optional random waking. It defines input filtering, state priority, creator assets, storage, delivery order and acceptance. The implementation is committed in phases, with automated checks and a Windows preview smoke check. The linked implementation record lists the remaining hardware release checks.
