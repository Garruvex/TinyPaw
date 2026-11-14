# Character artwork rules

Composition contract, 14 November 2026. Implementation is tracked in the [character and editor improvement plan](character-editor-improvement-plan.md).

A character owns one library of reusable parts and their variants. A pose selects parts from that library and places them at named attachments. Behaviors animate the selected parts. Artwork requirements come from the selected poses and capabilities.

## Shared parts

| Part | Initial variants | Reuse rule |
| --- | --- | --- |
| Eyes | Open and closed, with optional happy or surprised variants | One logical left/right pair is active. Sitting and standing share the same set. Tracking backgrounds, pupils and masks belong to this set. |
| Head | Front | Sitting and standing share the same head. Add a variant when the viewing angle or silhouette changes. |
| Body | Sitting, standing, curled | Contains the torso and legs. Separately animated head, arms and tail are omitted and their joints have painted overlap. |
| Arms | Relaxed left, relaxed right, holding right | A pose selects an arm for each shoulder. Holding replaces the relaxed right arm. |
| Tail | Resting | Reuse by placement and rotation. Add a curled variant only if the existing tail cannot fit the sleeping silhouette. |
| Items | Phone, glow stick | Attach to the holding hand. Reuse the same item across compatible poses. |
| Grip fingers | Holding right, when needed | Share between items that fit the same grip. Draw in front of the item. |
| Sleep | Seated doze and curled sleep pose recipes | Select a body, head, closed eyes, arms and tail. |

Variant names describe visible artwork. Behavior names refer to actions.

## Composition rules

1. Every reusable image has one asset identity. Poses reference it. Reusing a head in another pose does not create another head file.
2. Each layered pose resolves exactly one body, one head and one eye pair. A left/right pair is edited and switched together.
3. Heads used with separate eyes have plain face artwork beneath the eye layers. Closed eyes replace the open pair. Blink and sleep reuse the closed-eye variant.
4. The front head and its eye set are shared between sitting and standing. Keep facial geometry consistent when requesting artwork.
5. Each head variant declares a compatible eye set and its face placement. A different viewing angle can require a different eye-set variant. This still selects one pair at a time.
6. Every active arm belongs to one shoulder. A holding arm replaces the relaxed arm on that shoulder.
7. An item attaches at a hand grip. The arm, item and foreground fingers follow the same transform chain.
8. Each pose declares its ground point and attachment placements. Moving a body carries its head, eyes, arms and tail.
9. Changing a pose preserves character size and ground alignment. Transitions animate from the current visible state.
10. A new image is justified by a different silhouette, viewing angle, expression detail or grip. Timing, sway, bob, breathing and item glow come from animation.
11. Existing complete-pose PNGs remain usable as complete-pose variants. Their embedded eyes, head, limbs and tail are accounted for together, so those parts are not drawn again.

## Poses and behaviors

| Pose or behavior | Composition |
| --- | --- |
| Sitting awake | Sitting body + front head + open eyes + relaxed arms + resting tail |
| Standing awake | Standing body + the same front head and eyes + relaxed arms + the same tail |
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

Start with the shared front head and its eye set, sitting and standing bodies, relaxed arms, one holding arm, one tail and the desired items.

Add the curled body for the second sleep pose. Reuse the closed eyes for seated dozing. Add a sleep-specific head, tail or arm variant only where the curled composition needs a different silhouette.

This is a count of logical parts. A tracking eye set may contain several PNGs for its two backgrounds, pupils, masks and closed eyelids. The editor presents them as one eye set.

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

The current format has general layered poses, head expression replacements, whole-picture reactions and an additional standing music-artwork record. The standing renderer does not yet use the seated tracking-eye pair. Existing seated and standing artwork also has different face proportions.

The [main improvement plan](character-editor-improvement-plan.md#delivery-order) consolidates these paths, migrates the current character and verifies the editor, runtime, prompts and import/export together.
