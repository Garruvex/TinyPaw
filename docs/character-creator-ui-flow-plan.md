# Character creator UI and interaction plan

Proposed, 11 November 2026. Revised the same day after review. **Planning only; not implemented by this document.**

This refines the [character creation plan](character-creation-plan.md) and its [reaction sound feature](character-reaction-sounds.md). It is the intended replacement for the earlier creator layout, mandatory ground-point workflow and exposed generic pivot mode. Pack compatibility and existing artwork must be preserved.

This plan fixes and reorganises what already exists. New behaviors (falling, held poses, artwork reactions, beat styles, pickup areas) are in the [character behaviors plan](character-behaviors-plan.md), which needs its own scope decisions and starts after this plan's step 4.

## Goal

A creator should be able to import a picture, place separate artwork if needed, try the character and save. Every visible control must have a meaningful, observable effect for the selected artwork and current mode. Technical rig setup is optional advanced work.

The three essential decisions are:

1. **Ground is automatic.** A resting reference is useful internally, but setting a ground level is not a normal creation step and is never required to drag a character.
2. **A desktop drag holds the character where the mouse grabbed it.** The cursor is the temporary carrying pivot. Animation must not pull that contact point away from the cursor.
3. **Artwork placement, animation joints and desktop carrying are different interactions.** They must not share one ambiguous pivot control.

## Current findings

Review baseline: the working tree on 11 November 2026, not the earlier implementation summaries. Motion, Lines and Music sections now exist in the current view; their existence alone does not establish every setting's effect or platform support. Each row below was checked against the source on that date.

| Finding | Evidence / implication | Required change |
| --- | --- | --- |
| Ground appears beside Body and parts as a normal editing target | `src/view/settings.rs` exposes `Selected::Ground`, and one-PNG import asks the user to place the feet | Estimate it automatically; put correction under Advanced → Resting point |
| Ground has real internal uses | Desktop layout uses the rig anchor for positioning, bounds and menus; whole-character lean/squash are drawn around it | Preserve compatibility and internal placement logic, but stop presenting it as a prerequisite |
| Carrying and leaning use different references | `Press` stores cursor/window origin, dragging moves the window by cursor delta, and `motion::place` rotates lean around `rig.anchor()` | Window translation alone cannot keep a visibly rotating grabbed point pinned; solve its transformed position |
| A press squashes around the feet before the drag threshold | `Inputs.pressed` drives `squash` while the button is down and not yet dragging | Press feedback must preserve the contact point or be expression-only |
| The drag is clamped to the work area on every poll | `Desktop::drag_to` calls `Layout::clamp` | Stop clamping while held; recover on release |
| Hit testing ignores the current pose | `Desktop::over_character` tests the unanimated silhouette | Invert the real render transform |
| Art editing defaults to joint placement | `Editor::read` starts in Pivot mode with Ground selected | Open in Arrange mode with ordinary selection; no joint tool active by default |
| Size scales the canvas, not the artwork | `layout` uses `height / canvas.height` | Define Size as visible height, with a migration (see [Character size](#character-size)) |
| Eyes mix complete artwork and tracking assets | Complete eyes are generic Other parts; tracking presents White/Pupil/Mask/Blink together | Use a guided eye choice and hide tracking internals until requested |
| Current custom-character eye UI is misleading | The main page exposes a tracking toggle for bundled Yohta, but only a hint for custom art; desktop gaze is conditioned on runtime eye capability/options | Offer the same toggle for every supported rig; otherwise explain the missing capability |
| Behavior sections form a long page, while Edit art opens a separate view | The art view has its own static preview; behavior uses a saved-rig motion preview | One character workspace, shared draft and pinned preview with explicit Arrange/Test modes |
| Some copy is already stale | The art view says "Motion presets, strength and speed come in the Motion tab, next." although a Motion section exists | Remove stale roadmap copy from product controls and audit hints against actual behavior |

The exact render/input paths to review are `src/mascot/desktop.rs`, `src/mascot/motion.rs`, `src/mascot/rig.rs`, `src/mascot/editor.rs`, `src/mascot/preview.rs`, and `src/view/settings.rs`. No performance, device compatibility or native visual result is claimed by this review.

## User flow and layout

### Character home

Show the character list, selected name/thumbnail, Show on desktop, visible character height and **Customize**. Keep Add character, Import pack and selected-character export/remove actions discoverable without filling the page with every editor control.

**Add character** offers three clearly labelled paths:

| Path | Required input | First usable result |
| --- | --- | --- |
| One image | A transparent PNG and optional name | Preview → Use character; no ground, canvas, eye or pivot questions |
| Separate parts | Body first, then multiple PNGs | Assemble with direct manipulation; role suggestions remain editable |
| Existing pack | A pack folder | Validate, resolve identity conflicts if needed, preview and use |

One image is the default simple path: **Choose PNG → Try character → Use character**. Adding eyes, expressions, moving parts or sound is optional after the character works. Create and Customize open the same draft.

### Create from a character reference

Add **Create from reference** alongside the existing artwork import paths. Show the reference thumbnail, a free-text **Style** field, and a **Canvas size** preset with a sensible default. Size means artwork resolution; desktop display height remains adjustable later. Start with a complete single PNG. **Choose moving parts and poses** is optional. Its checklist shows the exact requested filenames as choices change.

| Creator selects | Artwork to request |
| --- | --- |
| Base character only, including click or beat compression | One complete transparent body PNG |
| Independent head bob | Body with the head removed, plus a separate head PNG with its face included |
| Moving tail | Separate tail PNG; omit the tail from the base artwork |
| Held pose | One additional whole-character PNG showing the carried pose |

Combine selected requirements into one consistent brief. For example, selecting head, tail and held pose yields `body.png`, `head.png`, `tail.png` and `held.png`. Keep eyes and eyebrows painted into the head unless explicitly requested as layers. Specify consistent markings, camera angle, scale, transparent background and stage alignment. Ask for enough painted overlap at the neck and tail base to prevent seams. Preserve the reference locally with the draft if needed; exclude it from exported packs by default.

First increment: **Copy art prompt** and **Import results**. Show the user which reference to attach to their chosen image tool. Allow copying one prompt per missing asset for tools that return a single image per request. Match returned PNGs to requested roles, allow reassignment, show missing roles, and open the assembled draft in Arrange. Let the creator overlay layers, correct alignment and try enabled poses before export. TinyPaw generates the manifest and exports through its validated folder-pack path. Offer held-pose export once the runtime and pack format support that slot.

A later **Generate artwork** action can submit the reference and brief to a connected image generator and populate the same review step. It requires a generation integration, per-file output handling and retrying individual assets while preserving accepted ones. Acceptance for either route: a base-only request yields one usable picture; a layered request yields exactly the requested roles; the exported folder reimports and reproduces the reviewed character and supported poses.

The base, head and tail requests need only this plan. The compression, head bob and held pose rows depend on the [character behaviors plan](character-behaviors-plan.md); hide each row until its behavior ships.

### Customize workspace

Use a compact section navigator with **Artwork**, **Behavior**, and relevant **Advanced** controls, alongside one pinned preview. Only ship a subpage when its controls and save path work; do not show enabled placeholders.

The preview has two explicit modes:

| Mode | Drag does | What appears |
| --- | --- | --- |
| Arrange | Moves the selected artwork layer, preserving the grab offset | Layer selection, proportional resize/rotate handles, ordering and optional guides |
| Test character | Carries the assembled character exactly as on the desktop | Live reactions, gaze and eligible test controls; no authoring handles |

Selecting artwork enters Arrange. Choosing a reaction test enters Test character. Show the active mode next to the canvas. Neither a hidden selection nor a previously used joint tool may change what a drag does.

The layer list uses readable names such as Body, Head, Tail, Left eye and Right eyebrow. Preserve imported filenames and editable display names in editor metadata. Runtime `other` remains a compatible implementation type; it is not the label a creator must decipher.

### Behavior section

Behavior regroups the Motion, Lines, Sounds and Music controls that exist today. It adds no new behavior. Group them by the moment the creator is thinking about:

| Group | Rows | Each row holds |
| --- | --- | --- |
| When touched | Click, Pick up, Drop | Its dialogue lines and, once the [reaction sound feature](character-reaction-sounds.md) ships, its sound; a summary and a Try action |
| On its own | Idle, Music starts, Annotation ends | The same, plus the idle interval on the Idle row |
| Moving parts | One row per part that has a motion preset | Preset, then Strength and Speed only when that preset consumes them |
| Music | React to playback, source, level meter and Strength; bop style; Follow beat (trial) with Sensitivity and Timing | As shipped; see below |

- All six existing line triggers get a row. None is dropped from the editor.
- The event row is the editing location for its dialogue and sound. A Lines or Sounds overview, if retained, opens the same draft entry. The reaction sound document's "Reactions → Sounds" location becomes these rows.
- Disabling a row preserves its setup for re-enabling. **Clear** removes that assignment. Opening a section or trying a preset never silently enables it.
- **Music keeps the shipped model.** Level response stays the default reaction, and the source picker (all apps, one app, or an input) stays where the level meter is. Follow beat keeps its trial label until the creation plan's step 5 results are judged. The Nod, Groove and Rubber hose styles are unchanged.
- **Tail setup.** **Artwork → Add part → Tail** opens a PNG picker. Place the tail against the body and click its base once to set the rotation joint. The Moving parts row then offers Sway, Wag or Hang with a live preview. If the tail is already painted into a flat picture, say at Add tail that independent motion needs a separate tail and a body with that tail removed.

## Storage: what Save writes

The plan adds settings, so it must say where each one lives. These rules extend the existing layout; they do not change the runtime pack schema.

| Data | Lives in | Written by |
| --- | --- | --- |
| Artwork, canvas, joints, parents, order, expressions, eyes | The installed pack (`character.json` and its PNGs) through the staged installation transaction | Save, for packs the user created or imported and edits in Customize |
| Source PNGs, placements, display names, resting-point mode (`auto` or `manual`) | `character.json.editor` metadata, version bumped when fields are added | Save |
| Motion presets, lines, bop style and later sounds | `.overrides/<id>.json` beside the packs, as today | Save |
| Show on desktop, Size, music source and levels, mute | App preferences | Immediately, debounced; not part of the draft |

- **One Save, two writes.** Save commits the pack transaction first, then the overrides. If the overrides write fails, report it and keep the draft open; the pack is already consistent. Cancel discards both halves.
- **Bundled Yohta** is not rewritten. Its artwork controls are read-only or offered as **Duplicate to edit**; its behavior edits go to overrides like any other character.
- **Absent means legacy.** A field missing from a pack or override keeps the behavior that pack had before this plan, including the built-in press squash and idle sway. New defaults apply only to characters created after the change, and are written explicitly at creation so they never depend on a missing field.
- **Export** merges overrides into the exported pack as it does today, and includes editor metadata.
- A pack without editor metadata loads with resting-point mode `manual`, so its saved anchor is preserved.

## Ground / resting point

The current anchor is a reference point, not a physics ground plane. Its useful jobs are stable placement, standing bounce/squash and alignment between poses. It does not justify a mandatory setup step or floor-snapping behavior.

- Remove Ground from the normal target list and remove the ground picker from one-image onboarding.
- Keep **Advanced → Resting point → Automatic / Adjust manually / Reset**. Explain it as “Where the character rests when standing or bouncing,” with a guide shown only during correction.
- Keep existing saved anchors. Do not change imported characters' desktop positions merely because the UI becomes automatic. An existing value is preserved until Reset or until the creator replaces or rearranges the body.
- Recompute an automatic point after body replacement or arrangement settles, not every animated frame. A manual point follows its body artwork transform and remains an override until reset.
- A pose change keeps the resting reference aligned. Resting-point changes must not introduce a desktop position jump when the same visible feet can be preserved.
- Carrying never rewrites `ground_anchor`, per-part joints or source placements.

### Automatic estimate

Use the body layer only; parts such as a separate tail are ignored.

1. Find the body's opaque bounding box, treating alpha below a small threshold as empty.
2. `y` is the lowest opaque row.
3. Take the band covering the lowest 8% of the box height. `x` is the centre of the opaque pixels in that band, limited to the middle 60% of the box width so one outlying foot or shadow cannot pull it to an edge.

A flat picture cannot tell a painted-in tail from feet. If the tail hangs lowest, the estimate rests on the tail; the limit on `x` reduces the sideways error, and Advanced → Adjust manually is the remedy. Do not claim more than this.

Fixtures for the automated check: flat Yohta, Yohta's separate body, a round floating sprite, a picture whose tail hangs below its feet, and a picture with wide transparent padding. The estimate for both Yohta fixtures must land within 2% of the canvas from the hand-set anchor; the others must stay inside the opaque box and be stable under added padding.

### Stage size

Stage size is advanced. Offer **Fit artwork with margin** first, with a clear preview of any clipping. Keep exact width/height under Advanced. Expanding or fitting the stage preserves visible artwork, relative joints and screen placement; it must not become another way to move the desktop character accidentally.

### Character size

Size becomes the visible resting-pose height in logical pixels, excluding transparent stage padding. Today it scales the whole canvas, so the stored number means something different.

- **Migration, once.** On first run after the change, convert the stored value using the active character: `new = old × visible height ÷ canvas height`. The active character keeps its exact on-screen size.
- **Other characters.** After the change, every character shows at the same visible height for the same Size. A character with wide padding will therefore appear larger than it did before when it is next selected. That is the intended effect; say so in the release notes.
- The slider range is re-derived so the smallest and largest on-screen results match today's for bundled Yohta.

## Desktop carrying: cursor holds the grabbed point

### Final interaction contract

On a valid left press, record the clicked point on the rendered character. Keep the existing five-logical-pixel threshold to distinguish a click from a drag. When that gesture becomes a drag:

- Preserve the original grab offset; do not snap the character's centre, feet, neck or saved pivot to the mouse.
- Use the cursor as the temporary whole-character carrying pivot. If lean is enabled, the character may rotate around the held point while the held point stays under the cursor.
- Freeze idle/music motion that would fight the gesture. Secondary parts may trail once they can do so without moving the held contact. If a grabbed part or its parent chain would move that contact, freeze or compensate that chain during the gesture.
- Keep the gesture across transparent gaps, outside the character window, and across monitors. Moving through a gap does not transfer the drag to the application underneath.
- Do not continuously clamp the character against a monitor edge while the button is held: clamping is another source of cursor/character separation. Permit partial off-screen movement during the drag, then apply minimal recovery on release if needed to leave the body/head reachable. Do not snap to an invented floor.
- On release, keep the final visible placement, then settle around the normal resting reference. Changing the animation reference must not cause a one-frame position jump. Release the temporary grab point.
- Capture loss, hiding, character replacement and cancellation stop the gesture safely. A cancelled drag does not fire a successful Drop sound/line.
- Reduced motion preserves the same positioning contract while suppressing lean, sway and settling animation.

Click feedback before the threshold must also preserve the grabbed contact, or use expression-only feedback. Do not squash an image away from the cursor and then start the drag from a different point.

### Geometry requirement

Store a source/local grabbed point and its relevant part/parent identity. Hit testing must invert the actual current render transform, including scale, root effects and part transforms; checking an unanimated box is insufficient.

For a held local point `g`, current visual transform `F`, screen cursor `c`, and root translation `t`, enforce:

`t = c - F(g)`

Here `F` excludes `t` and includes the current display scale. The same transform contract must drive drawing, hit testing and grab compensation.

**What "one pixel" means.** The drag is driven by a poll, so the window always trails a moving cursor between polls. The contract is therefore: in each poll, the window move and the drawn pose are computed from the same cursor sample, and for that sample `|c − (t + F(g))|` is at most one physical pixel after rounding. Lag between polls is measured separately: record the poll interval during a drag and note it in [measurements](measurements.md). It is not part of the pass/fail check.

**Room to rotate.** The window is sized for an upright character. Lean around a grab point near the head swings the feet sideways and can push them outside the window, where they would be cut off. Before enabling cursor-centred lean, size the window margin from the worst case (maximum lean, grab at the far corner), or cap lean for that grab so the silhouette stays inside. The added margin stays click-through.

The stored resting anchor may remain the persistence coordinate for pack compatibility. Derive it from the resulting final transform; never use it as the carrying pivot. A UI cleanup alone will not fix the current feet-based lean.

### Safe implementation order

1. First guarantee stable translation: retain the pose/contact snapshot during a drag and suppress whole-character lean, press squash and any other deformation that cannot yet preserve the contact. Stop clamping while held. Do not expose a lean-strength setting in this intermediate state.
2. Add cursor-centred carrying lean with per-poll contact compensation and the window margin above. Then re-enable secondary motion with the grabbed chain protected.
3. Blend back to standing on release, compensate the reference change, and verify mixed-DPI transitions and off-screen recovery.

Stage 1 is an acceptable stability fix; the expressive carrying feature is complete only after stage 2/3 acceptance passes.

**Stage 1 status, 11 November 2026: implemented, not yet checked on a native desktop.** `src/mascot/desktop.rs` keeps the window at `c − g·k` from the window's own scale each poll, with the pose frozen from press to release (faces and gaze still change). There is no clamp while held. Release recovers with `Layout::recover`: at least half the width stays inside the sides, with the old top and bottom limits. Restored positions use the same rule, and live motion eases back over 0.25 s. Hit testing inverts `motion::place` and each part's rotation (`Rig::hit`). Automated checks cover the per-poll contact error at 100–200 % and across a scale change, recovery, posed hit testing and the frozen pose. A known stage 1 trade-off: tails and ears no longer trail while carried, which returns in stage 2. The Settings preview still squashes and leans as before.

## Artwork placement and animation joints

Normal artwork editing uses select, move, proportional resize, rotate, duplicate/remove where supported, and layer order. Exact X/Y/scale/angle values are optional inspector controls, not the primary workflow.

An animation joint means “where this part turns,” such as a neck or tail root. Show **Adjust joint** only for a selected part with a rotation/spring behavior that uses it. Enter a temporary labelled joint-edit tool, show its effect in a small motion test, and exit with Done or Escape. Ordinary dragging must never leave this tool active silently.

Moving/rotating/resizing artwork carries its own joint and attached children consistently. Adjusting a joint alone changes no source pixels or placement. The cursor's desktop grab point is temporary and unrelated to this saved joint.

Replace **On head** with **Moves with: Character / Head**. Offer the Head choice only when a valid head exists, and explain removal/reparenting without discarding artwork. Use one understandable draw-order list; keep advanced Back/Front groups only if needed to represent the runtime's parent ordering accurately. Do not offer ordering operations whose groups prevent a visible result without explaining the constraint.

## Eyes and expressions

Start with “How are the eyes drawn?” and preserve the current image before offering any upgrade:

| Choice | Simple controls | Advanced controls, only on request |
| --- | --- | --- |
| Eyes already in the head image | Keep as drawn; optional head expression replacements | None required |
| Separate complete eye images | Add eye/eyebrow PNGs, name them and place them; attach to Head automatically when available | Optional parent/order correction |
| Eyes follow cursor | Guided setup with a visual explanation and live cursor test | Eye background, moving pupil/iris and eye-opening mask; movement range |

Explain the mask as **the area where the pupil is allowed to show**, using labelled examples. Do not ask for a mask when importing a complete static eye. Do not promise to extract a pupil from a finished eye image.

Blink is an optional separate step. Separate eyebrows are ordinary attached artwork, not a required tracking input. Offer static and tracking eyes in the same character when useful; choosing a starting workflow is not a destructive global conversion.

Show missing tracking inputs on their slots. A character with valid static eyes can still save. Offer **Keep these as static artwork** explicitly for incomplete tracking work, with a preview of what is kept or removed; never silently discard masks or blink images. A tracking toggle appears for custom and bundled art only when the rig can actually track.

Name expression slots by their visible action: Normal, Eyes closed, Pressed, Being carried. Use the base head's placement to initialize replacements and show a hold-to-compare test. Do not expose Talking unless a working speech feature consumes it.

## Control audit: remove settings that have no useful effect

Before redesigning a control, record its UI label, source of truth, consumer, applicability, persistence scope and preview effect. Classify it as Working, Conditional, Misleading or No consumer. This audit must cover current Motion/Lines/Music implementations rather than assume those sections are still missing. Keep the inventory as a table in `docs/character-control-audit.md` so later steps can cite it.

| Control / surface | Proposed treatment | Observable check |
| --- | --- | --- |
| Ground / required feet marker | Automatic; optional advanced resting correction | Normal import succeeds without editing it; bounce/pose alignment remains stable |
| Global Set attachment point mode | Replace with contextual Adjust joint | No ordinary layer drag unexpectedly edits a joint |
| Layer X/Y, scale and angle | Keep under a selected-layer inspector | Each accepted edit changes that layer in the current preview; invalid text cannot be silently saved as an older value |
| Stage width/height | Fit with margin first; exact sizes advanced | Transparent padding does not change displayed character size |
| Character size | Visible resting-pose height in logical pixels, migrated once | A 320 px setting does not become a tiny character because its PNG has wide margins; the active character does not change size on upgrade |
| Preview zoom/background | Keep as view-only controls, visually separated from character properties | Changing them affects neither exports nor desktop size/position |
| Preset Strength / Speed | Show only when the selected preset consumes them | Still hides irrelevant sliders; disabled motion explains why a test is static |
| Eyes follow cursor | Capability-driven for every character | Unsupported static eyes cannot appear to enable tracking |
| Pupil travel | Rename Movement range; show only with complete tracking setup | Live cursor test visibly changes the range |
| Parent and order | Clear labels; offer valid operations only | Changing them produces a predictable assembly/layering effect |
| Idle motion / reduced motion | Show effective state and its controlling setting | A global reduced-motion override is visible beside affected controls |
| Playback strength / beat sensitivity / timing | Reveal only with the relevant enabled mode and a functioning source; show silence, unavailable and trial states honestly | Current level/beat status and test behavior establish an effect; a missing source does not look like a broken slider |
| Reaction sounds vs line audio | Separate task labels and correct mute category, with shared sound policy | A sound can be assigned/tested without requiring speech text once the standalone feature ships |
| Test buttons | Run the same supported behavior and effective settings as the desktop | Tests identify temporary simulation explicitly and do not pretend to use live playback |
| Old “coming next” hints / empty controls | Remove from normal product flow | No enabled setting has no consumer or save path |

Lack of motion during silence is not proof a strength control is broken. The UI must distinguish a valid quiet state, disabled behavior, unsupported artwork and an actual failure.

## Drafts, preview and saving

- Use one selected-character draft across Artwork and Behavior. Switching sections preserves it. One Save commits the character edits as described under [Storage](#storage-what-save-writes); do not make users discover a separate Save in every section.
- The pinned preview uses that draft, not an older saved rig. Rebuild expensive art/rig data on workers by revision; discard stale results and keep the last valid preview during preparation.
- Draft tests never change the real desktop placement. Save applies the accepted character revision and refreshes its desktop rig without moving its visible placement unexpectedly.
- Display preferences such as Show on desktop and desktop Size remain immediate, debounced preferences, visually separate from authoring controls that require Save.
- Explicit Cancel discards the draft. Leaving or closing with unsaved work offers Save / Discard / Stay. Do not lose work silently or prompt when nothing changed.
- Use inline errors for the affected layer/slot. Show a concise overall status near Save for preparation or transaction failures. Keep valid draft work after a bad input.
- Undo/Redo covers user operations: one drag, one resize/rotation gesture, one joint adjustment, an import batch, reparenting and order changes. Do not require dozens of undos for one slider drag or group operation.

## Delivery plan and acceptance

One sequence. Steps 2 and 3 are independent of each other and can be built in either order; everything from step 4 on needs the draft from step 2.

| Order | Work | Acceptance gate |
| --- | --- | --- |
| 1 | Control inventory, truthful hints and automatic rest point | The audit table records each current control's consumer. Stale and no-consumer UI is removed. A one-image character is usable without ground or joint setup. Existing packs retain placement. The rest-point fixtures pass. |
| 2 | Shared draft and storage rules | One draft spans artwork and behavior edits. One Save writes pack then overrides; Cancel and a failed save leave the installed character unchanged. Save / Discard / Stay appears only with unsaved work. The preview renders the draft by revision. Bundled Yohta's pack is never rewritten. |
| 3 | Stable grabbing (carry stage 1) and shared transform helpers | Grab centre, head, tail and edge; the per-poll contact check passes through movement, 100/150/200% scale and monitor transitions. No press squash before the threshold, no clamp while held, no click-plus-drag duplicate, no feet-pivot drift. |
| 4 | Customize workspace, Behavior grouping and eye setup | Artwork opens in Arrange; Arrange and Test are labelled. All six triggers have a row that edits the same data as before. Static eyes are added and named without technical rig choices. Advanced tracking explains its inputs and shows a real test. |
| 5 | Contextual joints, gesture-grouped undo and the Size change | Adjusting a joint has a visible motion test; ordinary drag remains placement. Undo restores whole gestures. Preview, saved pack and desktop agree on parent, order, placement and effective motion. The active character's on-screen size is unchanged by the migration. |
| 6 | Expressive carry (stages 2 and 3) | Cursor-pivot lean keeps the contact check passing and never clips the silhouette. Release settles without a position jump. Off-screen recovery leaves the character reachable. |
| 7 | Create from reference: Copy art prompt and Import results | A base-only request yields one usable picture; a layered request yields exactly the requested roles; the exported folder reimports and reproduces the reviewed character. Rows for behaviors that have not shipped stay hidden. |

Step 7 needs steps 2 and 4 and can be built any time after them. The [character behaviors plan](character-behaviors-plan.md) can start after step 4. Its features that deform or replace the picture while carried also need step 6.

Status, 11 November 2026: steps 1, 2 and 3 are implemented, not yet checked natively. Step 2 keeps the behavior draft (motion, lines, bop) alive while the artwork editor is open. Save commits the pack, then writes the overrides in one file; Discard, Cancel and Save / Discard / Stay cover switching pages, switching characters and closing Settings. Step 4 is implemented too. Settings → Character has a Home / Artwork / Behavior / Advanced navigator. Artwork and Advanced open the editor with an Arrange preview (built-in Yohta offers Duplicate to edit). Behavior groups all six line triggers into When touched and On its own event rows, each with Try, followed by speech options, Moving parts and Music, beside a labelled Test character preview. That preview shows the unsaved artwork draft, rebuilt on a worker when Behavior opens; stale results are dropped. Eyes start from "How are the eyes drawn?" (in the artwork, separate eye images, or follow the cursor). Missing tracking inputs show on their slots, Pupil travel became Movement range, and tracking eyes follow the pointer in the Test preview. Layers have editable names, defaulting to the imported file name and stored in the editor metadata. Face slots read Eyes closed, Pressed and Being carried. Not done in step 4: the Test preview's Click and Drag still run the old squash-and-lean simulation rather than the desktop's frozen carry; that comes with step 6. Step 1's inventory is in [character-control-audit.md](character-control-audit.md). The resting-point mode is stored as an optional `rest_point` field inside version-1 editor metadata rather than by bumping the metadata version: an older build would reject a version-2 `editor` block, and with it the whole pack.

Required automated checks:

- Forward/inverse transforms and held-point error, including root lean, scale, squash, head/child transforms and rotation near image edges.
- The five-logical-pixel threshold, an initial animated press, transparent gaps, outside-window release, cancellation and no fake Drop event.
- A point held on a head or tail, with secondary motion enabled; protected chain behavior; no mutation of stored joints/resting point during a drag.
- 100%, 150% and 200% scale, non-zero/negative desktop origins and cross-monitor DPI changes; the per-poll contact error no greater than one physical pixel before release recovery.
- Release/reference blending without a screen-position jump, plus minimal off-screen recovery and restart after placement.
- Automatic/restored/manual resting points over the named fixtures, body replacement, stage fitting and pose alignment without a required setup question.
- Storage: absent fields keep legacy behavior; a new character's explicit defaults round-trip; a failed overrides write after a pack commit is reported and recoverable; the Size migration preserves the active character's pixel height.
- Visibility/applicability of each audited control and actual state changes, not merely that its button emits a message.
- Draft revision ordering, Save/Discard/Stay, cancellation/failure preservation, export/reopen, and gesture-grouped undo.

Manual usability check: give a first-time user a flat PNG and then the seven Yohta crops. They should put the flat image on the desktop without a ground/pivot explanation; assemble the crops using readable layer roles; distinguish Arrange from Test; and carry the character from whichever point they grab. Repeat over light/dark desktop content and mixed-DPI monitors. Record remaining confusing or ineffective controls before marking the UI work complete.

Native visual checks and the control inventory are release evidence still to collect. This plan does not mark any current control or drag fix complete merely by describing the desired behavior.
