# Making the mascot bop land on the beat

## Character and editor integration

The [character and editor improvement plan](character-editor-improvement-plan.md) defines reusable sitting and standing compositions, character-owned motion choices and shared preview behavior. The timing and listening comparisons below support that consolidation. Their starting-point descriptions record the earlier implementation baseline.

The goal is for a viewer to hear a kick and see the character land its gesture at the same moment. Two things can go wrong independently. The tracker can choose the wrong pulse, or the character can follow the right pulse and reach its most noticeable pose late.

## Starting point

TinyPaw uses CPAL 0.18.2 for microphone, line-in and default-output capture on Windows. Per-app capture uses Windows process loopback. Rodio handles sound playback. The beat tracker consumes mono samples and the animator consumes beat phase and loudness. Both are implemented in Rust in this project. Retain those interfaces so future capture backends can feed the same tracker. A macOS or Linux port would need separate verification for input devices, desktop playback and individual apps. [Capture routing](../src/platform/beat.rs) · [Dependencies](../Cargo.toml)

The existing click trial catches every scored beat at 80, 120 and 160 BPM, with one extra at 120 BPM. The scorer allows ±70 ms per hit and skips four seconds at startup and after a tempo change. Its mean signed error can hide alternating early and late gestures. The measurements cover the detector, while a viewer also experiences capture delays, animation-clock smoothing and the shape of the gesture. [Trial and baseline results](mascot-feature.md) · [Scoring code](../src/mascot/beat.rs)

## 1. Carry audio timing through to the animation

The live worker receives a chunk, asks the tracker for beats, and records the current wall-clock instant if it found any. A kick near the start of a large chunk therefore gets a later timestamp than the same kick delivered in a small chunk. Worker backlog adds another variable delay. This is the first change to make because every style uses that timing. [Live worker](../src/platform/beat.rs)

- Give each chunk a starting sample position and a time reference at capture. Prefer a device capture timestamp where available. Where only packet arrival time is available, account for the packet duration and measure the remaining uncertainty for that source.
- Map the most recent emitted beat's position to a monotonic instant and publish it together with its period. Advance sample positions across dropped chunks so the worker can detect a gap. Reset tracking on a discontinuity or stream restart and let the existing animation clock ease back into lock. Apply the user's Timing adjustment once when calculating phase.
- Replay identical audio at 44.1 and 48 kHz with 10, 20 and 50 ms chunks and injected worker delays. Compare phase at the same display instant after the relevant audio has arrived. The proposed target is agreement within one 10 ms analysis frame after lock. Exercise buffer drops, device changes and both Timing directions.

## 2. Give the Nod a clear landing

At phase zero the current head angle passes through neutral. Its first tilt extreme comes a quarter cycle later, or 125 ms at 120 BPM. For a viewer following the head, that difference can be larger than the tracker error. [Nod and body curves](../src/mascot/motion.rs)

- Compare two versions of Nod on the same character with identical audio, strength and beat timestamps. In the candidate, the head dips into its lowest-looking pose on the beat, then recovers. Let the ears and tail follow through. Check the silhouette on a character with a separate head and one whose motion relies on the body hop.
- Record audible clicks and the desktop mascot together at a known frame rate. Mark the frame of the visible accent and compare it with each click. Report median signed offset and 95th-percentile absolute offset in milliseconds, alongside the recording frame duration. Also try a musical excerpt by ear. Accept the candidate if it brings the accent closer and viewers consistently recognize its landing in a side-by-side comparison.
- Verify the settings preview, reduced motion, the steady 120 BPM fallback and transitions into and out of beat lock. Check Groove's dip and Rubber Hose's landing separately before adjusting either style.

## 3. Use real music to decide whether the detector needs work

Build a small repeatable listening set with permission-cleared clips and hand-marked beats. Include a steady drum loop, soft bass kicks under bright off-beat hats, a sparse acoustic passage, a tempo change and spoken audio. Mark passages with an ambiguous pulse so half-time interpretations can be reviewed by ear.

Run the existing and candidate trackers on identical samples. Record missed and extra beats, time to first sustained lock, time to recover after a tempo change, and stretches spent on an off-beat or at half or double tempo. Keep the existing click, silence and speech checks. For a live check, play the same clip through whole-output and per-app capture, then exercise microphone input against an audible click.

If bright subdivisions repeatedly pull the tracker away from the kick, prototype separate low- and high-frequency onset scores. If the tracker finds the pulse but switches between half and double tempo, first try favoring the established period while its evidence stays strong. Change one mechanism at a time. Consider an external beat-tracking library only if it improves the documented failure cases enough to justify its measured CPU cost and added dependency.

The first reviewable milestone is a before-and-after recording of the current Nod with the timestamp fix alone, followed by the same recording with the revised pose. Those comparisons show how much each change contributes before tuning the detector.
