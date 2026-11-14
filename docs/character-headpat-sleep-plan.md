# Headpat and sleeping implementation scope

## Shared artwork and sleep recipes

The [character and editor improvement plan](character-editor-improvement-plan.md) governs shared artwork and editor integration. Seated dozing and curled sleeping are two pose recipes. Compatible recipes reuse closed eyes, head and other parts under the [artwork rules](character-artwork-rules.md). The sleep timing, wake behavior and interaction details below remain supporting requirements.

Scoped and implemented in phases on 12 November 2026. Automated checks and a Windows Settings preview smoke check have passed. Mixed-DPI hardware, audio-device transitions and performance measurements remain release checks.

This extends the [behavior plan](character-behaviors-plan.md) with petting and persistent sleep. It replaces the earlier deferral of petting for this implementation scope. Existing packs and their click, carry, drop, music and sound behavior must remain unchanged when these features are absent or disabled.

## User experience and defaults

**Headpat** means moving the cursor back and forth over the character without a mouse button held. A single passing hover is not a pat. Clicking remains Click; holding a button and moving remains Pick up. Headpat is optional and needs no additional art to work, but custom happy artwork is supported.

**Automatic sleep** is opt-in. Its default rule is **Sleep after 5 minutes with no mouse nearby and no external audio**. Both conditions must hold continuously; silence alone is insufficient. The delay is editable from 1 to 60 minutes. This is character-local attention, not whole-computer inactivity: typing or moving the mouse elsewhere does not keep the character awake.

Nearby means within the character's visible bounds plus a margin of 25% of its visible height (minimum 24 logical pixels). Cursor presence in that area resets the sleep countdown even without movement; use an extra 8-logical-pixel exit margin to prevent boundary jitter. Use stable resting/sleep-pose bounds so breathing and decorative effects cannot toggle proximity. A character menu, captured press, carry, flight/fall or direct interaction prevents sleep entry and starts a fresh countdown when it ends. Automatic idle dialogue, sounds and decorative reactions leave the countdown running. If sleep becomes due during an idle line or reaction, let that response finish, suppress further idle responses and enter sleep without another five-minute wait. TinyPaw's own audio is excluded from the quiet condition. No global keyboard monitoring or OS last-input integration is required.

Default wake choices:

| Cause | Default | Behavior |
| --- | --- | --- |
| Touch character | On | A moving cursor enters opaque artwork; sustained gentle touch or rubbing gradually wakes it. Coming near only keeps an awake character awake. |
| Click or pick up character | Always | Wake immediately and perform the original gesture once; no unresponsive delay. |
| Audio starts | On | Sustained external audio starts a brief drowsy wake, including when music animation is disabled. |
| Random wake | Off | Optional natural waking after a random 5–15 minute sleep; range editable. |

After waking, require a fresh full quiet-and-away interval before sleeping again. Remote desktop activity neither prevents sleep nor wakes it. A passing mouse near a sleeping character does not instantly wake it.

## Headpat recognition

- Hit-test the currently rendered character, including its pose. Default petting region is the head's visible pixels for a layered rig, or all visible pixels for a flat image. An optional rectangular region in normalized stage coordinates lets creators refine it; it is unrelated to pickup restrictions.
- Starting tuning values: accumulate at least 40 logical pixels of in-region travel and two direction reversals within 1.2 seconds. A reversal requires at least 8 logical pixels along the stroke direction; small jitter does not count. Scale distance thresholds to visible character height relative to the default 146 logical pixels, with sensible minimums. These numbers require native tuning before release.
- Enter Petting once when recognized. Continued strokes sustain it; stop after 0.8 seconds without a valid stroke. Brief exits pause accumulation; remaining outside for 0.2 seconds clears the stroke history. Never accumulate off-character travel.
- Any button down cancels recognition immediately. A press, drag, menu, fall/flight, hidden character or annotation input ownership blocks it. Once carried, reversals must never generate Headpat.
- Default feedback is a small, bounded happy head tilt where a separate head exists; a flat image may remain still. Optional replacement artwork provides the expression. No automatic squash that disturbs hit testing or placement.
- One Headpat start event per session may play an optional line/sound. Sustaining strokes never repeatedly restarts audio. Start with a 3-second re-entry cooldown; one-shot feedback, no looping purr in this version.

## Sleep entry and wake recognition

- Track character proximity in logical coordinates across mixed-DPI monitors. Crossing the inner boundary counts as nearby; leaving the outer boundary counts as away. Reset proximity sampling safely on topology/pose changes; never interpret the character moving beneath a stationary cursor as intentional touch.
- Gentle touch wake requires intentional cursor entry into opaque artwork followed by sustained contact. At each sleep entry sample one touch threshold from 1–3 seconds and keep it fixed for that sleep; show partial stirring before fully awake. Valid rubbing strokes fill the same wake progress faster (approximately 0.6–1 second of sustained rubbing). A single pass does not wake. Leaving for more than 0.3 seconds clears progress. Reset gesture history at full wake so waking strokes cannot also fire Headpat feedback; continued fresh strokes may then establish Petting. Clicking/pickup bypass all delays.
- Reuse the existing audio meter and its raw, unscaled playback state. Do not infer silence from music animation strength, mute settings or a missing meter. Quiet and mouse-away must both hold for the full configured sleep delay. Audio wake needs 500 ms of sustained playback followed by a short 0.5–1.5 second drowsy transition; sample its duration once per wake attempt. Reuse the meter's noise gate and hysteresis rather than add an unrelated loudness threshold. Once qualified, finish waking even if audio briefly stops; click/pickup interrupts immediately.
- Default audio source is other-app playback on the default output, excluding TinyPaw. A microphone is never opened automatically. An optional explicit source selection may reuse the existing source picker; clearly show which source prevents sleep or wakes it.
- Audio capture has its own consumer lifetime: sleep/wake audio monitoring may keep the shared meter alive even with Music off. Stop it when no visible character or preview needs it. A device switch enters Unknown until the meter recovers. Unknown/unavailable is not silence: reset the continuous quiet-and-away countdown, retain existing sleep, and show status. Do not silently fall back to proximity-only sleep.
- Optional random wake samples one monotonic deadline per sleep entry. Do not re-roll each frame, persist deadlines across restarts, or wake hidden characters. Device resume/show starts awake with a fresh inactivity interval; no accumulated hidden-time surprise.

## State and behavior arbitration

Use a shared, deterministic controller driven by monotonic time and semantic inputs: Awake, Petting, FallingAsleep, Asleep and Waking. Desktop and preview use the same controller. FallingAsleep and Waking are brief transitions, not extra required images. Reduced motion skips visual easing but retains touch qualification; it does not turn passing hover into wake. Press/menu/carry/flight remain existing interaction ownership, not parallel sleep states.

Priority is: hide/cancel and input ownership, press/carry/flight, click/pickup wake and Click/Drop reactions, qualified gentle/audio wake, Petting, sleep, normal blink/idle/music. Click/pickup interrupts sleep immediately; gentle touch may take time to wake it. A decorative wake image must never delay pickup or lose a click. Sleep entry never generates Click, Pickup or Drop. Wake from click produces Wake plus exactly one Click; optional Wake feedback is suppressed for that input so sounds and speech do not double. The same applies to Pickup. Multiple wake causes in one update produce one Wake.

On button down, hit-test and capture the contact against the sleeping artwork before changing pose. Retain pointer capture and the original screen contact through wake, even when that pixel becomes transparent in the awake artwork. Resolve the press using the existing click/drag threshold and release rules. If it becomes a drag, map the captured contact into the awake or held pose and compensate translation so the contact stays under the cursor. Keep the normal resting placement on an ordinary wake click. Cancellation releases capture without generating a click or drop. This contract applies equally to head, body and tail contacts.

While asleep, stop cursor eye tracking, normal blinking, moving-part idle presets, music movement, idle dialogue and idle sounds. Optional breathing is one small bounded root effect, with no rotation or walking; disable it under reduced motion. No compulsory snoring. Wake restores the authored rig and effective user settings; do not overwrite saved motion settings to simulate sleep.

### Mouth-origin Zzz effect

Show a small optional `Z`, `z`, `z` trail emerging from the mouth while asleep: one glyph approximately every 1.2 seconds, floating upward/outward and fading over 2–3 seconds, with at most three live glyphs. This is a procedural decoration, not baked into sleep.png, a speech bubble, or an audio effect. As gentle wake progress builds, reduce emission; stop and clear the trail on full wake, click/pickup, hide or cancellation. Reduced motion shows a static small `Zzz` beside the mouth instead of floating particles.

At intentional touch onset, immediately begin fading the current Zzz trail and pause new emission to show stirring throughout the wake delay. If no glyph is currently visible, show a faint mouth-origin Zzz that begins fading on that update. With reduced motion, use a dimmer static Zzz during contact. If contact ends before waking, restore normal emission or the static indicator as wake progress clears. A brief pass can show this stirring feedback while the character remains asleep. This feedback works with a single-image character and needs no wake artwork.

Provide a visual Mouth / sleep effect origin marker for the sleeping pose, stored as normalized stage coordinates. With no marker, use an approximate upper-face origin and allow correction; never claim automatic mouth detection. A layered head origin follows its actual head transform. Whole-character sleep artwork has its own origin because its mouth may move. Include the trail in render/window bounds while keeping its entire area click-through and excluding it from proximity, touch/pickup hit tests and visible-height calculations. If the desktop edge leaves no space above, choose an inward trail direction and clamp only decoration bounds; do not move the character. Sleep now preview shows both the origin and desktop-edge behavior.

If external audio starts with audio wake disabled, the character stays asleep and music motion remains suppressed. If random wake occurs while still quiet/inactive, the fresh full inactivity delay allows a visible awake period.

## Artwork, creator and settings

Add Behavior → Headpat and Behavior → Sleep & wake. Include gesture instructions, enable switches, quiet-and-away delay, proximity preview, wake choices, gentle-wake timing, audio source/status, Zzz toggle/origin, optional reaction sound/line, and Try headpat / Sleep now / Try wake. Preview proximity and audio are labelled simulations and never use desktop-wide activity to unexpectedly interrupt an authoring test. Real cursor strokes in the preview use the same recognizer. The 1–3 second gentle-wake variation is the default; offer a fixed delay for creators who prefer consistent timing.

Optional creator assets:

| Choice | Files | Contract |
| --- | --- | --- |
| Headpat, head expression | `head_petted.png` | Requires separate head; replaces head only. |
| Headpat, complete pose | `petted.png` | Replaces entire character; choose this or head expression. |
| Sleeping pose | `sleep.png` | Complete character at rest, shared canvas and resting alignment; no baked-in Zzz. Include measured mouth origin in metadata. |
| Wake reaction | `wake.png` | Optional complete character, brief display before normal artwork. |

No new PNG is mandatory: a layered head can express petting and an existing blink face may represent closed eyes during sleep. A flat image without a sleep PNG stays still; explain that adding sleep artwork makes sleep visible. Do not require tracking eyes to supply a closed-eye asset. Reuse only compatible existing blink artwork; never paint eyes over a complete replacement pose.

Prompts must request all selected filenames, consistent scale/padding, measured placement and complete manifest mappings. Head variants explicitly include or exclude painted eyes/brows according to the renderer's overlay policy. Baked-eye headpat artwork suppresses tracking eyes; separately authored brow layers remain unless explicitly hidden. Define this policy in metadata, not by guessing from pixels. Whole-character poses suppress every ordinary layer. Preserve the normal resting point/desktop placement through pose changes, fit the union of supported visible bounds to avoid clipping, and update posed hit testing. Individual-image prompts ask for the approved base/head as an alignment attachment.

The creator may emit suggested delays and wake policies as author defaults, but importing a pack never silently enables automatic sleep, random waking, microphone monitoring or unprompted sound on the receiving installation. Present those choices for local opt-in. Headpat remains separately enableable. Settings participate in the existing shared draft, Save/Discard/Stay and export path.

## Storage and implementation boundaries

- Add a shared controller/gesture module under `src/mascot/` with injectable clock, proximity/touch, audio status and seeded random inputs. Keep platform sampling outside it. Use injected randomness for reproducible touch/audio wake durations and random deadlines.
- Extend reaction events with Headpat, Sleep and Wake. Audit fixed-size event arrays, parsers, sound limits, dispatch, cooldowns, dialogue trigger keys and every exhaustive match. Existing speech uses `drag` for Pickup while reactions use `pickup`; preserve that compatibility explicitly.
- Add optional head-expression metadata for Petting rather than changing the meaning/order of the existing blink/pressed/dragging slots. Extend pack loading, rig rendering, editor controls and saved metadata together. No implicit fallback to the pressed face.
- Add optional `petting` and `sleep` settings blocks and corresponding reaction entries to packs/overrides. Final field names and serialization examples must be settled against current parsers before implementation. Missing blocks mean disabled. Validate timing ranges, coordinates, finite numbers and allowed asset paths.
- Count every added PNG/WAV against actual pack count and decoded-memory limits. Extend request size estimates, file validation, managed copying, cleanup, export, folder import and individual-PNG assembly together. Multiple new sounds may exceed the current four-file limit; choose a documented bounded increase or explain the limit beside assignment controls.
- Treat temporary sleep/gesture state as runtime state; do not persist it. Author settings and artwork persist. Keep installed characters unchanged on failed save and preserve the unsaved draft.
- Reuse existing Windows cursor sampling; add no extra global polling thread for petting. Sleeping can use low-frequency activity checks plus timers; only breathing/transition needs animation frames. Unsupported platforms report availability and keep click/carry functional.

## Delivery sequence and acceptance

1. **Controller and inputs:** specify serialization, implement proximity filtering and audio consumer lifetime, and verify the state controller with a fake clock. Cover boundary jitter, a cursor parked nearby, remote typing/movement not blocking sleep, unavailable audio, device recovery, fresh wake delays and deterministic sampled wake durations/deadlines.
2. **Headpat:** desktop and preview recognition, happy feedback and optional artwork/sound/line. Verify a passing hover never pats, rubbing does, exit stops it, drag/menu never pats, and sounds fire once per session/cooldown. Check head/body/transparent regions and 100/150/200% DPI.
3. **Sleep and wake:** optional pose, full quiet-and-away countdown, gradual touch/rub/audio wake, immediate click/pickup and optional random wake. Verify passing hover stays asleep, interrupted touch clears progress, simultaneous causes, no lost or duplicate gestures, no own-sound wake, music disabled/strength zero, no idle chatter while asleep, reduced motion, hide/show and suspend/resume. Sleep cannot begin in flight or during carry. Verify mouth-origin Zzz registration, emission/live-count bounds, click-through, DPI, edge clipping and complete cleanup after wake.

   Regression cases must include repeated idle lines and sounds throughout the sleep countdown, with sleep occurring after the currently playing response finishes. Grab the tip of a curled sleeping tail, wake and continue directly into a drag across changed artwork bounds, verifying one Pickup, no Click and continuous cursor contact. Also verify an ordinary wake click preserves placement, cancellation releases capture, and a single-image character shows immediate Zzz stirring and restores normal sleep feedback after an interrupted touch, including when no glyph was visible and under reduced motion.
4. **Creator and portability:** prompt options, exact asset/mapping coverage, folder and individual-image imports, shared draft and exports. Round-trip a flat character and a layered tracking-eye character with separate brows; check fallback art, eye suppression, alignment and memory limits.
5. **Native acceptance:** tune gesture/movement thresholds with real mice and touchpads, mixed-DPI displays and light/dark desktops. Measure idle/asleep CPU, meter lifetime, wake latency and headpat responsiveness. Automated passing alone does not establish native usability.

First-release exclusions: animated frame sequences, looping audio, persistent moods/needs, automatic schedules, system sleep/lock control, roaming, and window-top sleeping. Yawns/stretching can follow as bounded idle artwork after these state transitions are proven.

## Implementation record, 12 November 2026

| Phase | Delivered | Verification |
| --- | --- | --- |
| 1 | Shared deterministic controller, settings validation and quiet/proximity inputs | Fake-clock checks cover countdown, unavailable audio, deferred idle responses, rubbing, touch and audio wake. |
| 2 | Reaction events, saved behavior controls, desktop integration and shared audio-meter lifetime | Full existing suite passes with the new event arrays and dialogue triggers. |
| 3 | Headpat head-expression slot, held-contact preservation, sleep/wake artwork, Zzz, breathing and real preview cursor input | Alpha-aware replacement-art checks and native Sleep now / Try wake checks. |
| 4 | Creator choices, per-image alignment instructions, reaction mappings, individual-PNG assembly and folder validation | Generated folder fixtures validate every new role. Individual-PNG results survive save/reopen with their reaction images. |
| 5 | Timing controls, visual mouth placement, reduced-motion handling and regression fixes | Full suite passes. Native preview checks found and resolved Zzz layering and clicks outside the preview waking it. |

The final storage block is `reactions.companionship`. It contains `petting`, `sleep`, `delay_seconds`, `touch_wake`, `audio_wake`, `random_wake`, `touch_seconds`, `random_seconds`, `breathing`, `zzz` and normalized `mouth`. Petting, sleep and wake artwork use their named reaction entries. Head-only happy art uses the appended `petted` head expression. Existing expression slot positions retain their meanings. Reaction WAV assignments are bounded at seven distinct files.

Sleep monitoring uses the selected music source while music response is enabled. With music response off it monitors other apps on the default output. Native source availability is reported by the existing Music meter. Automatic sleep and random waking require local activation after pack import. Sleep/wake timing edits share the ordinary Save/Discard behavior draft.

Headpat currently uses the head and its attachments when a head exists, or visible pixels for a flat character. A custom painted or rectangular petting region remains an advanced follow-up. Mouth positioning uses a point in the shared sleeping canvas, with direct preview placement and fine adjustment. The Zzz uses cached procedural image glyphs so it composites visibly above character artwork in the software renderer.

Native evidence covers the Windows settings layout, sleep with closed eyes, animated Zzz, mouth placement and explicit wake restoring open eyes. Sustained real audio, mouse/touchpad threshold tuning, repeated desktop pickup from changed sleep artwork, mixed-DPI monitors, suspend/resume and idle CPU measurements still require the release playtest. The native checks used isolated temporary application data.


## Optional sleeping positions, 13 November 2026

Seated dozing reuses the existing head and blink artwork; it requires no additional sleep or wake PNG. The shared controller lowers the head during sleep entry and lifts it during waking. A flat character without closed-eye artwork cannot invent closed eyes; a single optional sleeping PNG supplies that expression or a different body position.

Behavior > Sleep & wake offers Add sleep pose, with up to eight complete sleeping PNGs. Each nap selects one available pose and retains it for the nap, avoiding the previous choice when alternatives exist. A single pose needs no randomization setup. The first pose is the one currently being edited; selecting another pose for editing swaps it into that position, preserving both images' placement and mouth origins. Each pose can be removed independently. Existing packs remain valid.

The existing `reactions.sleep.visual` remains the primary image. Extra images are stored in `reactions.sleep.sleep_poses`, an array of `{ "art": { "image": "curled.png", "place": [0, 0, 1] }, "mouth": [0.5, 0.35] }`. All images participate in validation, file loading and export. The primary mouth remains `reactions.companionship.mouth`; extra poses retain their own stage-normalized mouth. Changing a pose does not add required animation frames or a required wake image. Additional positions return to the normal rig for waking rather than using a potentially mismatched wake image.

Creation prompts explicitly request one optional curled-up still, consistent character scale and floor baseline, with no baked-in breathing, Zzz or motion marks. The wake prompt is separately optional and asks for the sleep image as a reference. TinyPaw changes between complete body poses; it does not synthesize an articulated sit-to-lie transition.

Automated checks cover per-nap selection, avoiding repeats, image-reference export, metadata round trips, editing/removing poses and existing sleep input interruption. Native visual timing and the new pose controls still need an in-app playtest.
