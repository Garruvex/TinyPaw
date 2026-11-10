# Pointer settings and rendering review — 11 November 2026

## Changes

- Pointer → Stretch Paw offers **Forearm colour** when the selected pack has no forearm PNG. **Use pointer colour** retains its accent (or the original tan fallback); swatches and RGB sliders select a custom colour. This is a saved appearance preference shared by generated forearms. Paw artwork and textured forearms retain their original colours.
- The same colour controls and live result appear while adding a pointer without a forearm PNG.
- Forearm width now sizes a generated forearm as well as a textured one. Previously the generated limb always used fixed dimensions, so the import slider appeared ineffective without an image.
- Development builds optimize `tiny-skia`, `tiny-skia-path`, and `iced_tiny_skia`. App code remains debuggable while CPU rasterization avoids the unoptimized-library slowdown.
- The previously saved Yohta Simple pack was copied from the temporary interaction-review profile into normal per-user pack storage without overwriting existing packs.

## Measurement

Run `cargo test --release pointer_render_cost -- --ignored --nocapture`, or omit `--release` to compare development builds. The ignored benchmark uses the production software renderer and damage grouping, a moving paw, and a persistent pixel buffer. It excludes toolbar layout, window presentation, compositor latency, input delivery, and other monitors; these are **CPU draw times, not measured on-screen FPS**.

Baseline at 2560×1440, median milliseconds per frame:

| Presentation | Original development libraries | Optimized development libraries | Release |
|---|---:|---:|---:|
| Furry Paw | 2.457 | 0.115 | 0.113 |
| Classic Stick | 37.882 | 2.577 | 1.010 |
| Stretch Paw, generated forearm | 53.105 | 4.330 | 1.497 |

These baseline measurements predate making the generated limb respect the width slider. Geometry and position change the actual cost. Classic Stick and Stretch repaint roughly 23% of this test screen, versus 0.2% for the compact paw: the long diagonal limb has a large rectangular damage area. That amplifies the cost of unoptimized path rasterization. The temporary interaction-review window was a development executable, explaining a large portion of the observed choppiness.

No frame-rate cap or animation easing constant was changed. Motion still uses elapsed time and stops requesting frames when settled. Textured arms, full-window presentation, multiple monitors and actual screen FPS remain separate live performance checks.

## Verification

Regression checks exercise complete mouse clicks through the real toolbar/settings widget layouts, including colour controls and pointer Save/Cancel beside the preview. Pixel checks verify custom generated colour, unchanged texture colour, generated width, and the pointing-tip anchor. Existing session tests cover elapsed-time grip motion and reduced motion.
