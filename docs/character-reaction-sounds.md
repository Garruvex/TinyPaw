# Character reaction sounds

Feature scope, 11 November 2026. **Increments 1–3 implemented and increment 4 partly; none checked by ear yet.**

Increment 1 status: `src/mascot/reactions.rs` holds typed `Event`s (Click, Pickup, Drop, Idle, named as in the shared `reactions` object) and the one `Voice`, with its cooldowns, idle deadline and quiet rules. `src/audio.rs` gains a separate stoppable character-voice player, so a new reaction replaces the last one and nothing queues. The device opens on demand and is released once neither taps nor the voice need it. `sound.mute_all` (Mute all sounds) appears on Tap sounds and on the Speech tab. Line sounds now play through the character voice, and a trigger's accepted reaction sound wins over its line sound. Increments 2–3 status: Behavior has a Character sounds block (on/off, volume, opt-in idle sounds with their interval, Reset to pack) and a sound line under the Click, Pick up, Drop and Idle rows. Each line has a picker (None, the four bundled clips, the imported WAV), Import WAV…, Clear, Test and, for interactive rows, a cooldown. They are part of the one character draft and Save. Sounds are stored in the shared `reactions` object, in the pack (author defaults, which never enable idle) and in the overrides (the user's layer, with `user:` WAVs beside the overrides). Imported WAVs are checked without truncation (2 MiB checked first, mono or stereo, 8–96 kHz, five seconds at most, finite samples). The character may use four files. Export folds the user's layer into the pack and copies its WAVs into `sounds/`. Import validates every declared sound, and pruning keeps both line and reaction sounds. Discard prunes WAVs a discarded draft imported. Increment 4's quiet rules and lazy idle deadline are in the engine. Not yet measured: CPU, memory and audio-resource use with idle sounds on and off, and recovery after losing the output device.

This is an independently shippable part of the [character creation plan](character-creation-plan.md). A character can make a sound when clicked, picked up or dropped without speech text, speech bubbles, music detection or a layered rig. A single-PNG character supports the entire feature.

## User outcome

Open **Settings → Character → Reactions → Sounds**, assign a sound to Click, Drag pickup or Drop, test it, and save. The same reaction works on the desktop after restarting TinyPaw or moving the original WAV. Exporting the character carries the configured sounds with it.

The Reactions section must be available for every character, including bundled Yohta and flat PNGs. It must not require opening Edit art or adding moving parts.

## Scope

| Included | Deferred |
| --- | --- |
| Click, drag pickup and drop sounds | Petting and hover sounds |
| Optional idle sounds, off by default | Music-start and annotation-end triggers |
| One bundled sound or imported WAV per trigger | Random sound pools and sequential playlists |
| Character enable/mute, volume, trigger cooldown and Test | Speech text, bubbles and the Lines editor |
| Shared preview/desktop reaction dispatch | Live music capture, BPM and beat detection |
| Managed audio copies, saved settings and folder sharing | MP3/OGG, recording, ZIP import and an audio editor |

## Controls and defaults

The character's Sounds section contains:

- **Character sounds:** on/off, plus a volume slider from 0–100%. Default volume is 70%. A character with no assigned sounds remains silent.
- **Idle sounds:** a separate opt-in switch, off by default, with a random interval of 60–300 seconds. Show the interval controls only when enabled.
- One row for each trigger: enabled switch, sound picker, **Import WAV…**, **Clear** and **Test**. Interactive rows also have a cooldown control from 0–10,000 ms, default 250 ms. Clearing a sound disables that row without removing any other assignment.
- A **Save** action and **Cancel**. Draft changes and imported clips can be tested before saving; Cancel preserves the installed configuration. Switching sections retains the draft. Leaving or closing follows the shared Save / Discard / Stay flow in the [creator UI plan](character-creator-ui-flow-plan.md#drafts-preview-and-saving); leaving stops its test voice.
- An unavailable or invalid sound is explained on its own row. It must not silently substitute a different sound.

The picker offers **None**, the concrete bundled sounds already available to TinyPaw, and the trigger's imported WAV. Exclude the pointer-specific `pack` and global `custom` aliases: an assignment must identify an exact bundled clip or a managed character asset.

**Test** uses the draft's volume and selected sound, bypasses event eligibility and cooldown, and never moves the desktop character. It respects all sound mutes and annotation quiet rules. When muted, disable Test and explain which mute applies. A hidden desktop character can still be tested explicitly from Settings.

## Trigger contract

| Trigger | Emit exactly once when | Do not emit when |
| --- | --- | --- |
| Click | Left button releases after a valid short character press, without crossing the drag threshold | On initial press, right click, a cancelled press, or a gesture that became a drag |
| Drag pickup | A captured character gesture first crosses the existing five-logical-pixel drag threshold | On every pointer update, merely holding the button, or moving across transparent gaps during the same drag |
| Drop | An actual character drag completes on left-button release | After a click, capture loss, hiding the character, cancellation, or resetting its position |
| Idle | An eligible visible character reaches its next randomly scheduled idle deadline | While hidden, annotation owns input, dragging/pressed, a character menu is open, or a foreground fullscreen app suppresses idle activity |

Click sound intentionally waits for the gesture to resolve, so starting a drag cannot also produce a click sound. Keep capture across transparent gaps and release outside the visible silhouette. Gesture cancellation must clear state without inventing a drop.

Only the existing character gesture owner emits these events. A character gesture must not also emit a pointer tap sound or annotate the desktop. Right click continues to open the character menu silently.

Use the same typed events (`Click`, `DragStart`, `DragEnd`, `Idle`) for the desktop and explicit reaction tests. A future motion/reaction preview may consume the same events; its implementation is not a prerequisite for the sound editor.

## Playback policy

1. Allow one character voice at a time. Do not build a queue of delayed reactions.
2. An eligible interactive event replaces the current character voice, including an idle voice. Stop the old voice before starting the replacement. Use a short fade-out of up to 10 ms where supported, without overlapping voices.
3. An idle event never interrupts another voice. If busy, skip it and schedule a fresh interval.
4. Enforce each trigger's cooldown using a monotonic clock. Start cooldown only after playback is accepted; muted, unavailable, suppressed or rejected events do not consume it. Repeated events during cooldown are discarded.
5. Hiding, removing or switching the character stops its voice and clears pending idle work. Entering annotation stops character audio. Resuming eligibility schedules a fresh idle interval; missed events never play in a burst.
6. Reduced motion affects animation, not sound eligibility. Sound remains independently controllable.
7. Audio-device failure leaves character interaction working. Show a concise status, keep assignments, and allow retry through Test or a later eligible event. Do not retry or open the device every frame.

Pointer taps and character voices are distinct playback categories. They may share an output engine, but they must not share trigger state, cooldowns, selected clips or cancellation tokens.

### Mute and volume decisions

| Control | Effect |
| --- | --- |
| Existing **Mute tap sound** / `sound.mute` | Pointer taps only; preserve its saved meaning and shortcut |
| New **Mute all sounds** / proposed `sound.mute_all` | Pointer taps, character reactions and future character speech |
| Character sounds off | All reactions for that character, including idle and Test |
| Trigger disabled or None | That trigger only |
| Idle sounds off | Automatic idle playback only |
| Existing `sound.volume` | Pointer tap volume; preserve its saved meaning |
| Character volume | Multiplies the assigned trigger's gain for desktop and preview playback |

Expose the shared Mute all sounds control on both sound surfaces. Keep **Settings → Tap sounds** focused on pointer sounds and provide a link to Character reactions. Changing pointer sound selection or mute must not silently change a character's configuration.

This supersedes the creation plan's earlier assumption that pointer tap mute and volume would also govern character audio. Future speech uses the character voice category and shared mute policy; speech/reaction arbitration will be specified with the Lines feature.

## Persistence and sharing

**Superseded shape, 11 November 2026:** sounds live in the shared per-event `reactions` object described in the [behaviors plan](character-behaviors-plan.md#one-reaction-object-per-event-agreed-in-phase-0). The block below shows the original proposal; its fields and rules carry over, renamed as described there.

Add an optional `reaction_sounds` block to the version-1 character manifest. Its absence means no assigned reaction sounds. This is a proposed loader extension; current loaders do not implement or preserve it yet.

```json
{
  "reaction_sounds": {
    "enabled": true,
    "volume": 0.7,
    "idle_enabled": false,
    "idle_interval_seconds": [60, 300],
    "triggers": {
      "click": {
        "enabled": true,
        "sound": { "builtin": "duck-toy" },
        "gain": 1.0,
        "cooldown_ms": 250
      },
      "drag_start": {
        "enabled": true,
        "sound": { "file": "sounds/pickup.wav" },
        "gain": 0.8,
        "cooldown_ms": 250
      },
      "drag_end": { "enabled": false },
      "idle": { "enabled": false }
    }
  }
}
```

- A sound reference contains exactly one of `builtin` or `file`. Missing trigger entries are disabled. `enabled: true` requires a valid sound reference. Volumes and gains are finite values from 0–1.
- Pack defaults travel with the character. User changes are per-character overrides, following the existing creation-plan decision to keep author defaults separate. Implement a dedicated validated override store; it does not exist yet.
- Copy imported WAVs into managed draft/override storage before accepting them. Store relative managed references, never an absolute source path. Do not reuse or overwrite the pointer's `sounds/custom.wav`.
- Save configuration and audio as one recoverable transaction. A failed decode, save, replacement or rollback preserves the last usable configuration or retains clearly reported recovery files. Cancel cleans up unused draft copies.
- Replacing a character with the same ID preserves its overrides. If an override references a now-missing pack sound, disable and explain that assignment until repaired. Keep both creates a new identity without inheriting unrelated local overrides.
- Export the effective character configuration and its referenced WAVs, rewriting override paths into the exported folder. Include bundled clip IDs, not copies of bundled audio. Omit global mutes, device state, active voices, cooldown clocks and idle deadlines.
- Import validates the whole block and every referenced WAV before committing, and copies only declared assets. Removing a character removes its managed sound overrides, without touching original user files or pointer sounds.
- On a receiving install, automatic idle sounds remain off until that user opts in. Export/import equivalence is judged with the same user mute, volume and idle opt-in settings.

## Validation and resource limits

- WAV only: mono/stereo PCM or IEEE float, sample rate 8–96 kHz, at most five seconds and 2 MiB encoded per file. Reject overlong audio rather than silently truncating it. Reject malformed headers, unsupported formats and non-finite samples.
- At most four unique referenced WAVs per character, 8 MiB encoded in total and 16 MiB decoded float samples. Reuse a decoded clip when multiple triggers reference the same file.
- Include audio in the character transaction's existing aggregate storage and decoded-memory budgets, alongside images and optional editor sources. Audio limits do not waive the pack's total limits.
- Validate duration/sample counts with bounded decoding. Check file size before reading the full payload. Canonicalize file references and reject traversal, absolute paths and links escaping managed storage or the pack.
- Validate trigger names, reference types, IDs, enabled flags, cooldowns, gains and interval ordering. Unknown trigger names and unknown declared bundled clips are errors. A missing optional block remains valid.
- Perform copying, validation and decoding on workers. Use request IDs and the existing pre-commit cancellation boundary. A late save result reconciles storage/catalog state without selecting or showing a character after a newer action.

## Implementation boundaries

- Add a character reaction/sound subsystem under `src/mascot/`; it owns trigger eligibility, cooldowns, idle deadlines and the current character voice request. App routes events and presents errors.
- Extend `src/audio.rs` with cancellable, categorized playback and completion/failure reporting. The current mixer-only tap playback and five-second truncating decoder are insufficient for this contract. Keep pointer tap behavior working during the change.
- Audio lifetime cannot depend solely on Annotation On/Off: a visible character must be able to react while annotation is off. Open output on demand for a reaction and release that usage after playback; retain an output device only while another active category needs it.
- Share immutable decoded clips between previews and desktop playback. Cache by character/config revision, invalidate after a successful save or switch, and release unused character audio when disabled or hidden, except during an explicit test.
- Register global controls through the existing registry. Route per-character draft controls through one validated character configuration API used by Settings, Test, persistence and export.
- Extend pack loading, installation, replacement, export and removal to handle declared WAVs and overrides. Keep preview edits isolated until Save.
- Idle uses the next eligible deadline, not a permanent UI frame loop. When disabled or hidden, schedule no idle work and hold no reaction-only output resources.

## Delivery and acceptance

| Increment | Deliverable | Done when |
| --- | --- | --- |
| 1. Gesture events and playback | Typed triggers, single voice, mute/volume/cooldown policy | A click emits one click; a drag emits one pickup and one drop with no click. Cancellation/right click emit none. Rapid clicks never create overlap or a backlog. |
| 2. Reaction sound editor | Bundled picker, per-trigger WAV import, Test, Save/Cancel, optional idle | A flat PNG and layered character can use the same settings. Test and desktop obey the same volume/mutes. An invalid WAV leaves valid draft assignments intact. |
| 3. Persistence and sharing | Pack extension, overrides, managed audio transactions and export/import | Restart works after deleting original WAVs. Replacement, Keep both, removal and folder sharing preserve the specified identity and ownership rules. Failed saves preserve the previous sound. |
| 4. Idle and resource verification | Quiet rules, deadlines, device loss and lifecycle | Hidden/disabled/annotation states stay quiet; resuming creates no catch-up burst. No reaction-only audio stream, polling or frame subscription remains when off. |

Required automated checks:

- Gesture threshold, short click, release outside the silhouette, transparent-gap drag, lost capture and cancelled drag.
- Cooldown boundaries with a fake monotonic clock; accepted versus suppressed events; interruption, idle priority and no queued playback.
- The mute matrix, volume calculation, explicit Test, and isolation from pointer settings and tap events.
- Missing/malformed/overlong WAVs, duration and decoded limits, invalid references, path escape and aggregate pack budgets.
- Restart/source independence, draft cancellation, failed transactional save/rollback, stale worker completion, overrides and export/import round trips.

Manual checks on Windows: flat and layered characters; annotation off/on; rapid clicks and repeated drags; right-click menu; idle in fullscreen; hide/switch/remove; output-device loss/recovery; restart; and export/import into a clean profile. Confirm sound timing by ear and measure enabled-idle versus disabled CPU, memory and audio-resource use.

The feature is complete only when the editor, desktop playback, persistence and sharing pass these checks. Shipping the artwork editor or motion reactions alone does not complete character reaction sounds.
