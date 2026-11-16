# Authored pose sets and movement permissions

Design decision, 16 November 2026. This supersedes earlier requirements that every sitting or standing pose needs separate arms or supports waving.

## The authoring unit

A pose set is a cohesive authored appearance plus its approved movement. Posture describes the set; it does not promise a list of animations. A calm seated set, standing groove set, concert phone-wave set and curled sleep set can have different body, head, eye and hand artwork.

Each active set resolves one face/eye system. Compatible images may be explicitly reused, but neither shared anatomy nor separate limbs are mandatory across sets. Existing complete pictures remain valid sets.

| Hand option | Artwork | Movement |
| --- | --- | --- |
| Painted into the body | The original cohesive body includes its arms and paws | Nod, blink, breathing and compatible whole-body motion; no independent hand movement or waving |
| Separate movable hands | Matching body without those arms, plus authored arm/hand attachments | Only approved movements; adding an arm does not enable wave |
| Designed wave | A coherent body/shoulder/holding arm/grip/item combination tested as a set | The authored wave within its safe arc |

Changing the hand option is an artwork choice. It cannot cut painted arms out of an existing PNG or invent a matching body. The editor must require compatible body artwork and complete attachments before saving an incomplete configuration.

## Editor workflow

1. Choose or create a pose set. Show its assembled resting appearance continuously.
2. Select the body and its fixed or movable hand option. Hide inapplicable hand controls for fixed artwork.
3. Assign the set's compatible head, one eye pair and optional tail or other parts. Embedded facial features are not overlaid with a second pair.
4. Choose approved movements. Wave is a separate explicit opt-in and requires authored compatible attachments; unverified copied artwork starts with wave off.
5. Test rest, expressions, every enabled motion, its full arc, pickup and transitions. Missing assignments have specific feedback.
6. Save the whole character draft together. The existing artwork draft supports the current hand/motion options; unifying the older behavior save controls remains roadmap work.

A new custom set should start conservatively. Copying a set retains artwork for editing, gives its arms independent variant identities and turns wave off. Replacing a body must not silently make an incompatible arm eligible. Previously assigned hands may be retained inactive for undo or restoration; inactive artwork is not rendered.

## Playback path

Authored set -> validate artwork/attachments and enabled movements -> select an eligible set -> sample the shared beat -> apply that set's approved motion profile -> render the resolved set.

The beat provides timing and strength. It does not decide whether a limb can move. Excitement chooses among authored eligible sets, with dwell time and hysteresis. It does not transform arbitrary sitting artwork into a wave.

Explicit and Random style choices both honor the visible set's permissions. The current implementation keeps a fixed-hand seated set on Nod when Wave was requested; it never draws a generated replacement wand for that set. The preview and desktop use the same permissions. Eligible pose changes retain beat phase and ease between current visible states.

For future authoring, movement profiles should own resting angles, safe ranges, timing and joint limits. An item and its grip must remain in the same hand space. Geometry validation can check references and attachments; cohesive anatomy still requires an authored and visually tested set.

## Storage and migration

The current composition adapter stores `hands: embedded | movable` and an explicit `wave` permission on each recipe. Body variants may declare their matching hand mode and pose. Joint overrides belong to a recipe; image identities remain reusable when compatible. Old authored packs infer supported attachments for compatibility, while newly generated recipes explicitly declare their permissions.

Yohta sitting is restored to `sit/body.png`, with its painted paws, the plain head/eye pair and tail. Its separate arm/item assignments are inactive, and wave is disabled. The split body and experimental seated arms remain unused library artwork for a future intentionally authored set. Standing retains its independently authored attachments. Both complete sleeping pictures and pickup art remain available.

Updates use the existing validated staged pack transaction and preserve original source art, dialogue, reactions and overrides. No source images are deleted just because a motion is disabled.

## Delivery boundaries

Implemented now: restore fixed-paw sitting; optional hand modes; pose-local wave permission; structural validation; creator choices; consistent preview/desktop gating; explicit/Random fallback; save/reopen coverage.

The next architectural stage is arbitrary named pose sets with posture as metadata, rather than the existing sit/stand adapter. Then add per-set motion profiles, an eligible-motion picker, authored transition review, richer compatibility guidance and one artwork/behavior Save and Discard transaction. Those are deliberate design work, not assumed complete by the hand-mode fix.
