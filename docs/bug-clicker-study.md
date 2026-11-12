# BUG Clicker: features and custom-character workflow

Research date: 11 November 2026. Reference screenshot: user-supplied Windows
v1.19.3 Reactions screen. This is research and a TinyPaw design recommendation,
not an implementation change or a verified specification of BUG Clicker's files.

## Evidence and limits

- The developer's [download repository](https://github.com/BUG-INVADE/Bug-Clicker-download)
  describes a Windows desktop character that responds to clicks with a sound and
  speech bubble. It documents portable storage beside the executable and automatic
  updates. The repository view exposed a README, not application source code.
- An [artist's own commission listing](https://vgen.co/Cat14c/service/desktop-pet-chibi-bugclicker/09c37c62-8d1d-48c4-900d-82fde81bc0ce)
  advertises a chibi character, four expressions (idle, pressed, headpat, blink),
  configured `.cvclk` delivery, and different source-file rights/options. This
  establishes an actual artist-facing workflow, not the application's internals.
- The supplied screenshot directly establishes the controls listed below.
- The developer site, https://bugndev.web.app/, could not be retrieved in this
  environment. No local BUG Clicker app, exported template, or import dialog was
  inspected during this study. File encryption and runtime internals remain
  unverified; no protected character file was decoded.

Search results prominently feature bugclicker.org and bug-clicker.com. Both
identify themselves as independent services, not the app developer. Their guides
are leads for further UI inspection, not authoritative technical specifications.
They conflict about compatibility and whether `.cvclk` is an archive, JSON, or a
protected format. Do not implement a parser from these descriptions.

## Features visible in the screenshot

| Area | Observed controls or information |
| --- | --- |
| Navigation | Characters, Customize, Reminders, Reactions, Share, About. Tab names alone do not establish every feature inside them. |
| Music mode | Separate options labelled "Nods on the beat" and "Right on every sound". |
| Audio source | Windows default output selected; source dropdown and refresh button. Helper text mentions recording devices such as Stereo Mix. |
| Feedback | Active status, 118 BPM, "on beat", volume meter and beat indicator. This is displayed status, not a test of detection accuracy. |
| Gain | Auto, As is, x10, x50; helper text says gain is for analysis and does not change what the user hears. |
| Sensitivity | Low, Normal, High; current numeric value 5/10. |
| Timing | Earlier, Default, Later. |
| Advanced settings | Fine-tune toggle, currently off. Contents when enabled are unknown. |

The design lesson is to show whether audio is arriving before asking a user to
tune motion. Separate amplitude response from actual beat tracking. The screenshot
does not establish which capture API, beat algorithm or animation engine is used.

## How custom characters are presented

The artist-facing evidence supports a small set of authored expressions and a
finished importable character file. This lets an artist sell a ready-to-use
character rather than ask the recipient to assemble artwork.

Unofficial guides describe the following sequence, which still needs a direct
check against the app:

1. Get the template from Character Studio.
2. Draw a body and aligned head variants on a shared transparent canvas.
3. Load separate PNGs or a PSD with named groups.
4. Configure reactions, dialogue and presentation in the app.
5. Export a `.cvclk`; the recipient imports it through Share and selects it in
   Characters.

The guides report group names `body`, `head1` through `head4`, plus extra hair
layers. Do not treat these names, their ordering, suggested canvas dimensions,
PSD support details, or optional-part rules as verified import contracts.

This suggests a combination of expression swaps and part movement, but the exact
transform hierarchy, pivots and reaction priorities require direct observation.
There is no evidence here that a skeleton, Live2D model or frame-by-frame animation
is required.

## Implications for TinyPaw

Compare with [the existing mascot proposal](mascot-feature.md). These are proposed
follow-ups; they do not silently expand its agreed implementation scope.

| Capability | Current TinyPaw proposal | Recommendation |
| --- | --- | --- |
| Entry-level import | One transparent PNG | Keep this as the minimum. |
| Independent motion | Body, head, tail, masked cursor eyes | Keep optional; custom creatures need not supply every part. |
| Expressions | Blink overlays; no general authored expression states | Add optional normal, pressed and happy/petted head images in a future schema revision. Use the normal head when a variant is absent. |
| Eye compatibility | Separate tracking eyes inherit head transform | Let each expression choose tracking eyes or eyes baked into its artwork. Hide tracking eyes on baked-eye expressions to avoid duplicate eyes. |
| Creator tools | PNG/folder import and alignment editor | Provide a labelled PNG template and live reaction preview first. Evaluate PSD import separately rather than making it a dependency. |
| Distribution | Folder import/export; archive support deferred | A single portable pack would make sharing simpler. Treat archive packaging as an explicit later scope decision. Keep the manifest documented and data-only. |
| Audio | Optional amplitude envelope; no exact beat promise | Begin with an input meter, silence status and strength control. Add tempo detection and timing adjustment only with measured behaviour. |
| Dialogue/reminders | Explicitly deferred | Keep deferred; they are separate systems from making a character animate. |

Our click currently opens a menu. A proposed pressed reaction should acknowledge
the same click without adding a second click action. If petting is added, define
gesture recognition so it does not trigger during a drag or intercept menu use.
One possible priority is drag/menu ownership, then pressed, then petted, then
blink/idle. Music should provide a bounded motion input rather than overwrite
the selected face. Validate this in the preview before changing the feature spec.

## Next direct checks

- Inspect Character Studio's Images, Motion and export screens, using the app's
  own template and an original test character.
- Confirm required/optional slots, PSD naming, pivots, extra-part ordering and
  whether expression images can have different dimensions.
- Observe click versus hold versus pet versus drag, including overlapping events.
- Export/import an original character and inspect the user-visible credit,
  duplicate-character and missing-image behaviour.
- Confirm whether `.cvpart`, creator locks, GIF/PNG capture, multiple simultaneous
  characters and keyboard response are present in v1.19.3. They are leads, not
  verified findings of this study.
- Measure idle CPU/memory and music response separately before making efficiency
  comparisons with TinyPaw.
