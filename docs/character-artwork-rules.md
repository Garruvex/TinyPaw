# Character artwork rules

Composition contract, corrected 16 November 2026. Implementation is tracked in the [character and editor improvement plan](character-editor-improvement-plan.md).

A character owns one library of reusable parts and their variants. A pose selects parts from that library and places them at named attachments. Behaviors animate the selected parts. Artwork requirements come from the selected poses and capabilities.

## Current hand rule

A pose may keep arms and paws painted into its body, or use separate movable hands. Fixed hands do not support Wave. Movable hands do not automatically enable Wave: that movement needs a cohesive authored set and explicit permission. Yohta sitting uses its original painted paws and supports Nod and blink; its experimental seated wave is disabled. See the [pose-set design](character-pose-set-design.md).

## Shared parts

| Part | Initial variants | Reuse rule |
| --- | --- | --- |
| Eyes | Open and closed, with optional happy or surprised variants | One logical left/right pair is active in each pose. Reuse between poses only when facial geometry fits. Tracking backgrounds, pupils, masks and closed lids belong to that pair. |
| Head | Front | Each pose selects one compatible head. Reuse is optional; add a variant when viewing angle, expression geometry or silhouette changes. |
| Body | Sitting, standing, curled | Contains the torso and legs. Separately animated head, arms and tail are omitted and their joints have painted overlap. |
| Arms | Relaxed left, relaxed right, holding right | Optional. Fixed poses keep arms/paws in the body. Movable poses use independently authored arms, shoulders and grips; holding replaces the relaxed arm only in a set designed for it. |
| Tail | Resting | Reuse by placement and rotation. Add a curled variant only if the existing tail cannot fit the sleeping silhouette. |
| Items | Phone, glow stick | Attach to the holding hand. Reuse the same item across compatible poses. |
| Grip fingers | Holding right, when needed | Share between items fitting the same hand within a pose; use a pose-specific variant when hand anatomy changes. Draw in front of the item. |
| Sleep | Seated doze and curled sleep pose recipes | Select a body, head, closed eyes, arms and tail. |

Variant names describe visible artwork. Behavior names refer to actions.

## Composition rules

1. Every reusable image has one asset identity. Poses reference it. Reusing a head in another pose does not create another head file.
2. Each layered pose resolves exactly one body, one head and one eye pair. A left/right pair is edited and switched together.
3. Heads used with separate eyes have plain face artwork beneath the eye layers. Closed eyes replace the open pair. Blink and sleep reuse the closed-eye variant.
4. Deduplication is a rule within each pose: exactly one active face and eye system. It does not require different postures to use identical artwork. Reuse a front head/eye pair only when compatible. Do not stack painted-in eyes beneath a second eye pair.
5. Each head variant declares a compatible eye set and its face placement. A different viewing angle can require a different eye-set variant. This still selects one pair at a time.
6. Every active arm belongs to one shoulder in its own pose. A fixed pose needs no arm layers. Each movable pose uses independently authored arms and editable shoulder/grip pivots. A holding arm replaces the relaxed arm on that shoulder.
7. An item attaches at a hand grip. The arm, item and foreground fingers follow the same transform chain; do not counter-rotate only the item away from its fingers. Include painted shoulder overlap and inspect the entire wave arc.
8. Each pose declares its ground point and attachment placements. Moving a body carries its head, eyes, arms and tail.
9. Changing a pose preserves character size and ground alignment. Transitions animate from the current visible state.
10. A new image is justified by a different silhouette, viewing angle, expression detail or grip. Timing, sway, bob, breathing and item glow come from animation.
11. Existing complete-pose PNGs remain usable as complete-pose variants. Their embedded eyes, head, limbs and tail are accounted for together, so those parts are not drawn again.

## Poses and behaviors

| Pose or behavior | Composition |
| --- | --- |
| Sitting awake | Sitting body + front head + open eyes + relaxed arms + resting tail |
| Standing awake | Standing body + compatible head and one eye pair + standing relaxed arms + compatible tail |
| Seated doze | Sitting composition + closed eyes + slow breathing |
| Curled sleep | Curled body + compatible head placement + closed eyes + compatible arms and tail |
| Nod | Animate the active head and body, with delayed tail and ear follow-through |
| Groove or bounce | Animate the active composition using the selected motion |
| Light wave | Active compatible pose + holding arm + selected item + grip fingers |
| Pickup | Hanging body variant with compatible head and attachments, or an existing complete held-pose image |

Posture, motion, expression and held item are separate choices.

- Posture selects sitting or standing.
- Motion selects Nod, Groove, Rubber hose, Compress or Light wave.
- Expression selects the eye variant and any additional face detail actually needed.
- Item selects the phone or glow stick.

Both sitting and standing can use the same motion. A wave is available when the active pose has the required shoulder, holding arm and grip placements. Excitement may choose posture and movement strength. Sleep behavior chooses a sleep recipe.

## Small initial artwork set

Start with a cohesive body, head and one compatible eye set for each selected pose. Keep painted paws unless that pose is deliberately authored for separate arms. Request holding arms and grips only for a designed holding/waving set. Reuse compatible heads, eyes, tails and items when their anatomy fits. A different arm silhouette is necessary artwork, not redundancy.

Add the curled body for the second sleep pose. Reuse the closed eyes for seated dozing. Add a sleep-specific head, tail or arm variant only where the curled composition needs a different silhouette.

This is a count of logical parts. A tracking eye set may contain several PNGs for its two backgrounds, pupils, masks and closed eyelids. The editor presents them as one eye set. With closed-eye layers, blinking and seated dozing do not need another full-head blink PNG. Full-head expression variants are reserved for an intentional change to facial detail or silhouette and replace the separate eye layers while active.

## Editor and prompts

The [character and editor improvement plan](character-editor-improvement-plan.md#editor-workflow) defines the editor workflow, ownership, migration and implementation order.

Creation prompts resolve the shared parts required by selected poses and capabilities. Each unique image is requested once. Both full-folder and individual-image imports use those assignments.

## Validation

- Referenced variants and attachment targets exist.
- Exactly one body, head and eye pair resolve for each layered pose.
- Every head and eye set has a compatible placement.
- Embedded parts in a complete-pose image are not overlaid with duplicates.
- Every holding pose maps its shoulder, grip and optional foreground fingers.
- Each selected capability has its required artwork.
- Unused variants remain in the library for later use. Export includes declared assets and their placements.
- Save, reload, export and import preserve shared identities and assignments.

## Implementation baseline

The versioned shared `composition` model now owns variants and pose assignments. Sitting and standing compile into ordinary layered rigs and can share a head and one eye-pair assignment. Complete pictures remain explicit recipes for existing sleep and held artwork. Legacy packs and the older standing-artwork record remain supported for compatibility.

See the [implementation status and remaining workflow work](character-editor-improvement-plan.md#implemented-baseline). The [current Yohta inventory](character-shared-artwork-inventory.md) records which artwork is reused and which complete pictures remain intentional.
