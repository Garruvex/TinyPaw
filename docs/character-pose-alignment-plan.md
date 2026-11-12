# Character pose alignment and pickup

**Proposed for implementation.** A creator should be able to place the sitting and held poses side by side, match the apparent size of the head, choose exactly where the character hangs from the pointer, and test a pickup before saving. The desktop character size should be set once. A dangling body can be taller than a sitting body, and a curled sleeping pose can be shorter.

## What needs fixing

The Behavior editor already exposes horizontal offset, vertical offset and scale. The rig supports editable joints, and the manifest has a ground anchor. The missing pickup control is a grip on the held illustration. Today `Picture::new` estimates it from the top band of opaque art. A horn, raised hand or asymmetric ear can pull that estimate away from the intended grip.

The reaction-image importer trims transparent margins and bottom-centers the result, even when the PNG already matches the character canvas. This discards placement that the creator prompts explicitly ask artists to preserve. Existing fitted images may already have lost their original margins.

Swing clearance already accounts for enlarged held art. Keep its regression test and extend it to authored grips. Investigate the reported sitting-image residue independently. The software renderer checks an unrotated rectangle when deciding whether to redraw a rotated image. This is a testable suspect, and the desktop pose-selection path is another possible source of overlap.

## Delivery phases

Each phase ends in a separate commit with its acceptance checks. Use disposable character packs and preferences for verification.

### 1. Capture the failure and establish a rendering baseline

- Make a small redistributable fixture with a broad sitting silhouette, a long dangling pose, asymmetric ears and obvious transparent margins. Include a layered sitting pose and a flat fallback. Use Onyx for a local visual comparison when available.
- Compare a sequence of partial redraws against fresh transparent frames at the same positions and angles. Cover sit → held → swing left/right → drop → sit, rapid pickup during a reaction, and window move/resize. Include damage entirely inside a rotated tip outside the upright rectangle.
- Inspect which character image each frame submits. Fix the first proven difference in pose selection, damage bounds, clipping or surface presentation. If the native symptom persists after pixel comparisons pass, record a desktop reproduction separately.
- **Pass when** old sitting pixels become transparent on pickup, the swung silhouette matches a clean render, and the next sitting frame contains no held pixels. Preserve damage-driven redraw during normal idle use.

### 2. Define placement and grip once

- Extend the existing held-pose art record with an optional grip stored as two finite fractions in the inclusive range 0–1, measured from the top left of the full source image including padding. Keep the current `place` offset and uniform scale semantics. An absent grip uses the current estimate. Resetting the grip restores that fallback.
- Resolve source points into stage coordinates, then into the window using the desktop size and current pose transform. Use the same mapping and its inverse for drawing, grips, alpha hits, preview handles and transformed bounds. During carry the resolved grip stays under the pointer and is the pendulum origin. The existing ground anchor governs landing. Layer joint pivots keep their existing meaning.
- Trace pack loading, draft editing, per-character overrides, save, export and reimport. Distinguish inheriting the pack grip, overriding it locally, and deliberately choosing automatic grip. Validate malformed metadata through the existing pack and override validation paths.
- **Pass when** old packs render and swing as before, a custom off-center grip survives a full round trip, reset/inherit survives restart, and source-to-screen-to-source mapping recovers the selected pixel within rounding tolerance.

### 3. Preserve source geometry through import and creation

- For new whole-pose imports matching the shared canvas, preserve pixels and padding exactly. For differently sized art, retain the original source and calculate an initial aspect-preserving fit. Store enough placement information to represent non-square sources on non-square stages. Verify equivalent behavior in Creator, Settings replacement and pack import/export.
- Keep existing fitted PNGs and placements usable. Offer replacement from the original file to recover discarded padding. On replacement, show the new image with its proposed grip and placement before saving. Cancellation and failed imports must leave the previous working image and settings available.
- Update the creator’s per-image and pack prompts and manifest examples. Ask for the same head proportions as the approved resting reference, a full transparent canvas, a natural dangling silhouette, and a measured grip on the intended pickup point. Generate grip metadata only when held artwork is selected.
- **Pass when** a deliberately off-center shared-canvas PNG stays off-center through every entry point and round trip. A cropped rectangular PNG keeps its aspect ratio, and repeated edits/exports introduce no cumulative resizing or clipping.

### 4. Add an “Align poses” editor

- Open from held-art settings and the creator preview. Show the sitting reference and selected pose at the same desktop scale, with side-by-side and adjustable-opacity overlay views. Provide direct drag and uniform resize, numeric fine adjustment, a draggable grip, and a visible ground reference tied to the existing ground anchor.
- Offer optional **Match head size**. Mark the same two stable landmarks on each pose, such as the left and right cheek outlines. Multiply the current pose scale by the ratio of the two displayed distances and align their midpoints. Reject coincident or near-coincident points and explain when the needed scale exceeds the supported range. Manual alignment handles profile views and artwork with different proportions.
- Keep preview zoom independent of saved character and pose scale. Include keyboard nudging, reset placement, automatic grip, cancel and save. Let the creator try a real pickup, shake and release from the draft before saving. Share pose placement controls with whole-picture reactions such as sleep and drop. Show grip controls only for held art.
- **Pass when** moving or resizing the art carries its grip with it, preview zoom leaves saved geometry untouched, cancel restores the original, and reopening reproduces the saved alignment on the desktop. Verify this with both mouse and keyboard.

### 5. Verify native movement, boundaries and release

- Reuse the existing swing-room calculation with the resolved grip. Reserve the transformed visible-art envelope across the full supported swing. Include anti-aliased edges and ensure transparent margins stay click-through. Recompute on pose, size, grip and display-scale changes. Keep the cursor contact stable when the window origin or size changes.
- **Screen-edge behavior:** while held, keep the grip attached to the cursor. At the physical bottom edge a dangling body can extend offscreen. On release, settle at a reachable resting position inside the destination monitor’s work area. For characters larger than that area, keep the grip or another interactive portion reachable. Verify taskbars, negative monitor coordinates and monitor handoff.
- Exercise 100%, 125%, 150% and 200% display scaling, small/default/large character sizes, held art at 100% and 125%, centered and extreme grips, all four edges, reduced motion, and a crossing between monitors at different scales. Automate geometry and pixel comparisons. Record actual Windows checks separately, including which monitor configurations were available.
- **Release gate.** At a fixed cursor position the grip drifts by at most one physical pixel after layout settles. A free swing reaches its normal range away from screen edges. Reduced motion still honors the grip. Pickup/release cycles leave no ghosts, do not accumulate drift, and keep transparent regions click-through. Check frame time and idle redraw activity against phase 1 on the same machine before release.

## Completion evidence

Record the commit and checks for each phase here as implementation proceeds. Keep a before/after pickup sequence, pixel comparison results, an exported fixture that reopens with its authored grip, and the Windows display configurations actually tested. A single uniform resize can match head width but cannot repair inconsistent drawing proportions. The side-by-side preview should make it apparent when the remaining correction belongs in the artwork.

### Implementation record

- `264a004` fixes rotated raster and SVG culling and clipping during partial redraw. A new regression failed before the fix and passes at display scales 1, 1.25, 1.5 and 2. All four renderer tests pass. This establishes a renderer regression baseline. Native pickup sequences and pose-transition comparisons remain to be verified.
- `fd6cb30` adds optional source-relative grip metadata, validates coordinates, and resolves it in desktop and preview rendering. Held-pose settings can select a grip by clicking the preview, show its marker, and restore automatic grip. Existing pose-preview work in the checkout was integrated where required by these controls. The application suite passed with 240 tests and eight ignored. Added checks cover metadata round trips, invalid grips, placement with source padding, and preview grip/reset behavior. Native usability and pack export with an authored grip still need explicit verification.
- `c0293cf` preserves exact pixels and padding when a replacement reaction PNG matches the character canvas. A focused import regression passes. Differently sized imports still use the existing fitted output. Original-source retention, source replacement review and creator prompt updates remain pending.
- The overlay editor, head-landmark matching and final native screen-boundary validation remain pending. The implementation is incomplete.
