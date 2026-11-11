# Patched iced_tiny_skia 0.14.1

Based on the crates.io release. TinyPaw's overlay and character windows are transparent, so any pixel
outside the repainted region stays on screen until something else covers it.

- `src/layer.rs` (`Layer::damage`): quad damage now includes the drop
  shadow (offset and blur radius). Before, a moved or hidden quick bar left
  its shadow behind.
- `src/engine.rs` clips shadow painting to the damaged pixels and redraws a
  shadow even when only its fringe intersects a damaged area. Repeated hover
  redraws previously darkened the shadow outside the toolbar.
- `src/primitive.rs` (`Primitive::visible_bounds`): stroke bounds now grow
  by half the stroke width (times the miter limit for miter joins) plus one
  pixel of anti-aliasing. Before, fading ink and the moving laser left their
  edges behind.

Performance patches (November 2026), measured with
`cargo test --release annotation_render_cost -- --ignored --nocapture`:

- `src/lib.rs` (`Renderer::draw`): each damaged region is rendered into a
  scratch pixmap of its own size and copied back. Upstream drew into the
  whole window with a window-sized clip mask, so every path was rasterized
  over its full extent and the mask (a full-screen memset) was rebuilt
  several times per layer. The mask is now rebuilt only when a group's clip
  differs from the current one.
- `src/engine.rs` (`covers`): no clip mask when the clip covers the whole
  target; masked drawing is slower.
- `src/layer.rs` (`Layer::damage`): live canvas groups are diffed primitive by
  primitive instead of always counting as changed, so one moving shape no
  longer repaints everything else in its frame.
- `src/geometry.rs` (`Frame::stroke`): strokes wider than a hairline are
  outlined once when the geometry is built and stored as fills, instead of
  re-running the stroker over the whole path on every repaint.
- `src/vector.rs` (`Pipeline::draw`): SVGs are placed with the transform's
  translation. Upstream ignored it because it never translated; with
  per-region pixmaps, icons were drawn shifted by the region's origin.
- `src/window/compositor.rs` (`present`): only damaged rectangles are copied
  to the window (`present_with_damage`), and an unchanged frame copies
  nothing. Upstream copied the full window every frame.

Run `cargo test --offline -p iced_tiny_skia --lib` from the repository root
to compare partial redraws and stroke clearing at fractional display scaling.

The root `Cargo.toml` applies the patch through `[patch.crates-io]`.
