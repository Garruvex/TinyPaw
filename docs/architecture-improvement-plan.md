# Architecture improvement plan

Reviewed 11 November 2026 against the current working tree, including pending character and audio changes. This supplements [next phase](next-phase.md) with implementation boundaries and acceptance criteria.

## Implementation status — 11 November 2026

The seven architecture delivery items are implemented in the working tree:

- Character installation uses unique staging folders, serialized commits, a backup journal, rollback on rename/final-load failure and startup recovery. Failed rollback retains the backup and reports its location. Replacement decisions reject an installation revision changed since preparation.
- PNG preparation, folder snapshots and installation run on workers. Duplicate decisions use the validated snapshot. Cancellation is accepted before commit; an accepted commit completes or rolls back. Completed installs always reconcile the catalog, while stale results do not select/show a character. Leaving Character settings cancels pending activation. Quit waits for workers and then flushes settings.
- One preference writer coalesces snapshots for 250 ms, preserves ordering, retries failed writes and flushes before Quit. Slider values still update immediately.
- Host Lens capabilities participate in shared command availability before session mutation. UI, wheel, tray, shortcuts and CLI use that check; native source loss still returns to Point.
- Standard setting sliders read registry ranges. Lens zoom reads its declared choices. Custom labelled controls and authoring controls retain their layouts.
- `src/platform/lens.rs` owns native Lens instances, source transitions, fallback and teardown. App passes intent, destinations and the current helper-window exclusions.
- Enumerated window identifiers and the selected dynamic Lens source are owned strings. Built-in command/setting IDs and existing saved settings remain compatible.

Validation adds coverage for rollback, interrupted swaps, failed rollback/recovery, source mutation, stale duplicate decisions, cancellation, decoded input budgets, unsupported command dispatch, preference coalescing and flush retry. The Windows tests require a writable real temp directory in this managed environment; its virtual sandbox temp folder refuses directory renames. Use `$env:TEMP='C:\Users\Dream\AppData\Local\Temp'; $env:TMP=$env:TEMP` before `cargo test` here.

The decoded RGBA input budget is provisionally 128 MiB across all declared images, with dimensions inspected before decoding. Peak process memory and import responsiveness still need measurement using representative large packs. Hardware visual checks (mixed DPI, focus, Lens fallback and Discord capture) remain open. The live music/BPM detector trial below remains a separate, unimplemented experimental milestone; no tracker dependency has been selected or shipped.

Automated validation: **109 tests passed**, strict Clippy (`--all-targets -- -D warnings`) passed, and the Windows release build passed. Rust formatting was applied with an explicit config path because config discovery is restricted in this environment.

The review sections below describe the original baseline and motivate these changes.

## Direction

Retain one Rust process, one Cargo package, the registry, and the session as the sole owner of annotation state. Keep App responsible for event routing and window lifecycle. Move filesystem transactions, save scheduling, and native Lens behavior into their existing subsystem homes as those paths are changed.

## Delivery order

| Order | Work | Acceptance criteria |
| --- | --- | --- |
| 1 | Make character replacement recoverable | A failed rename or final load preserves the previous installed pack. Startup recovers an interrupted replacement. |
| 2 | Move character preparation and installation off the UI thread | Large imports leave input and animation responsive. Duplicate decisions, cancellation, and late results have explicit behavior. |
| 3 | Coalesce preference writes | Sliders update immediately. One ordered writer saves the latest snapshot after a short debounce and flushes pending changes on orderly exit. Errors remain visible and retryable. |
| 4 | Add host capabilities to command availability | UI, shortcuts, and CLI reject unsupported Lens actions with the same reason before state changes. Runtime source loss still follows the existing recovery path. |
| 5 | Read standard setting controls from registry definitions | Sliders use declared constraints. Choices use declared options where applicable. Custom previews and page layouts remain supported. |
| 6 | Extract Lens lifecycle from App | A controller owns creation, source changes, fallback, exclusion lists, and cleanup. App routes intents and presents results. |
| 7 | Give dynamic sources owned identifiers | Refreshing the source list releases obsolete source strings. Built-in command and setting IDs remain static. |

## Character import review

### Current paths

The one-PNG path builds a Draft, accepts a name and ground anchor, and calls `characters::install` from `UiMsg::SaveCharacter`. Success adds the character, selects it, and enables its desktop window.

The folder path opens an asynchronous folder picker. `CharacterFolderPicked` calls `App::import_character`, which invokes `characters::import_folder` synchronously. The importer reads the manifest and validates every referenced image before asking about duplicates. Keep both allocates a new ID. Replace retains the imported ID. Replacing the bundled ID creates a separate ID. The choice reruns import against the source path.

Both paths write into a staging directory, validate the staged manifest and art, generate a thumbnail, rename into the installed directory, and load the installed character. App updates the catalog and selects it. A visible character with the replaced ID reloads its rig. Folder import does not enable a previously hidden desktop character, while the PNG action does. Decide the intended activation behavior and make the button labels describe it.

### Existing protections

The loader bounds manifest size, file size, canvas dimensions, asset count, parts, and eyes. Asset reads canonicalize paths and check containment. Folder import copies only manifest-referenced assets and validates the staged copies. Preserve these checks when introducing background work.

### Required corrections

1. Replacement currently removes the old directory before the final rename and load. Rename the old installation to a unique backup in the same parent, move the staged pack into place, and finish loading it before removing the backup. On failure restore the backup. If rollback fails, retain recovery files and report their locations. Recover interrupted transactions at startup using explicit transaction state.
2. The staging path is deterministic and removes a previous staging directory with the same ID. Use unique staging directories and serialize installation commits. Recheck destination conflicts at commit time so a stale catalog snapshot cannot overwrite a newer result.
3. File copying, decoding, thumbnail generation, and installation currently block App updates. Run preparation on a blocking worker. Give requests IDs and return typed outcomes for prepared, duplicate, installed, and failed states. Keep catalog and desktop changes on the UI thread.
4. Duplicate resolution currently rereads the source folder. Preserve validation on retry because source files may change. Prefer retaining a validated staged snapshot while awaiting a decision, then commit that exact snapshot. Cancel should discard its staging directory.
5. Define cancellation around the commit boundary. Before commit, cancellation discards work. Once commit starts, complete or roll back the filesystem transaction and reconcile the catalog even if the import page has closed. A late completion must not unexpectedly select or show a character after a newer user action.
6. Bound total decoded memory across a pack and inspect PNG dimensions before full decoding where supported. Existing per-file and image-count limits can still allow substantial aggregate work. Choose the budget from a measured supported pack and return a clear validation error when exceeded.

### Verification

Add targeted failure tests for replacement rename, final load, rollback, and startup recovery. Exercise duplicate keep-both and replace after a destination changes between preparation and commit. Verify a changed source cannot bypass staged validation. Check that removing or replacing the selected character refreshes the catalog, rig cache, desktop, and saved selection coherently.

Manually exercise PNG import, layered folder import, cancellation, duplicate choices, hidden versus visible activation, and restart. Measure responsiveness while importing the largest supported pack.

The review test run completed 90 of 93 tests successfully. Three character import tests failed with Windows access-denied errors. The failing filesystem operation remains undiagnosed. Resolve that failure and rerun the suite before using it as release evidence.

## Mascot and music boundary

Keep rig evaluation and motion in the mascot subsystem. Route clicks, gaze targets, and future music events into that subsystem as inputs. Let it own reaction state and animation deadlines. App should coordinate desktop windows and settings without accumulating animation rules.

### Live beat detection trial

Trial two behaviors on the same captured Windows playback stream. A tail flick can respond as soon as a drum or note starts. A regular head bob needs the timing of the underlying pulse, including beats with no prominent attack. A BPM estimate alone gives the interval but leaves the first bob unplaced.

| Candidate | Role in the trial | Integration cost |
| --- | --- | --- |
| microdsp spectral flux | First choice for reactive accents. Apply an adaptive peak threshold and a short cooldown to its onset-strength signal. | Rust library designed for embedded use. TinyPaw supplies peak selection and silence handling. [Documentation](https://docs.rs/microdsp/latest/microdsp/) |
| aubio Tempo | First choice to evaluate for Follow beat. Use beat timestamps, period and confidence to schedule the next bob. | Rust bindings to a native C library. Check Windows packaging and upstream license fit before choosing a release dependency. [Rust API](https://docs.rs/aubio/latest/aubio/struct.Tempo.html) · [Upstream](https://github.com/aubio/aubio) |
| BTrack | Second live tracker to compare if aubio loses the pulse on representative music. | Causal C++ implementation under GPLv3. A local benchmark can compare its output before any integration decision. [Upstream](https://github.com/adamstark/BTrack) |

Feed short timestamped mono blocks from output loopback to an analysis worker. Keep sample processing off the UI thread. Send the mascot the latest accent strength or predicted beat time. Fade to idle on silence or lost confidence, reset on device changes, and release capture when disabled. Both one-PNG and layered characters consume the same timing. The rig chooses which parts move.

Prototype BPM and beat tracking with aubio Tempo first. As playback progresses, update both the estimated interval between beats (tempo) and their position on the playback clock (phase). Predict the next beat from the last beat timestamp plus the current period. Schedule the start of the bob at the predicted beat plus the selected timing offset minus the time the rig takes to reach its lowest point. At 120 BPM, beats are 500 ms apart. For a predicted beat at 1,500 ms and a 40 ms descent, On beat starts at 1,460 ms and lands at 1,500 ms. The 40 ms illustrates animation travel time. Early and Late shift the landing while the tracker continues following tempo changes.

Register a saved Beat timing preference for Follow beat with Early, On beat (default), and Late. Apply a signed offset to the predicted beat timestamp and schedule the animation so its visible accent (the lowest point of a head bob) lands at that target. Early requires a confident prediction of the next beat. Discard stale predictions on loss of lock. Use the same scheduling in the simulated preview. Tune the preset distances by sight and verify their signed timing against a click track without changing the detected BPM.

Compare each candidate using live-sized chunks of the same audio. Use 80, 120 and 160 BPM clicks to measure timing, then hand-mark excerpts with syncopated drums, quiet beats and tempo changes. Include speech, game sounds and TinyPaw's own taps to count unwanted bobs. Match events one-to-one to marked beats within a trial tolerance of 70 ms. Report misses, extras, half- or double-time runs, time to lock and regain tempo, and p95 audio-to-visible-motion delay. Measure CPU, memory and packaged size on the Windows release build with music response on and off. Choose the shipping tracker from those results.

## Completion checks

Run the existing registry and session tests after dispatch changes. Verify that unsupported actions fail consistently through CLI and UI. Test rapid setting changes followed by shutdown for persistence ordering. Check idle redraw behavior and active animation latency after each worker or controller extraction.
