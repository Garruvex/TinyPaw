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

Run `cargo test --offline -p iced_tiny_skia --lib` from the repository root
to compare partial redraws and stroke clearing at fractional display scaling.

The root `Cargo.toml` applies the patch through `[patch.crates-io]`.
