# Shared character artwork inventory

Updated 16 November 2026.

| Logical part | Current Yohta source | Uses |
| --- | --- | --- |
| Front head | `sit/part-1-head.png`, existing expression variants | Sitting and standing; standing supplies a neck offset |
| Front eye pair | Two backgrounds, pupils, masks and closed lids in `sit/eye-*` | Sitting, standing, blink and gaze |
| Sitting body | `music/body.png` | Seated recipe; separated arms and tail |
| Standing body | `music/standing-body-shared.png` | Standing recipe; no embedded head, arms or tail |
| Tail | `sit/part-2-tail.png` | Both awake recipes |
| Sitting relaxed arms | `music/left_arm.png`, `music/right_arm.png` | Sitting shoulders only |
| Standing relaxed arms | `music/standing-left-arm-v2.png`, `music/standing-relaxed-arm.png` | Standing shoulders, with independent joint/scale |
| Sitting holding arm | `music/sitting-holding-arm-v2.png` | Seated wave with a rounded shoulder overlap |
| Standing holding arm | `music/standing-holding-arm-v2.png` | Standing wave, its own shorter arm and grip geometry |
| Grip fingers | `grip.sit` and `grip.stand` | Independent hand placements; compatible finger source reused, follows arm/item and draws in front |
| Items | `music/phone.png`, `music/glow_stick.png` | Shared between postures, attached at grip |
| Seated doze | `reactions/sleep.png` | Existing complete recipe, preserves authored appearance |
| Curled sleep | `reactions/sleep-curled.png` | Separate complete recipe with its own mouth/placement |
| Pickup | `held.png` | Existing complete recipe |
| Other reactions | Click, drop, headpat and wake pictures | Existing event assignments |

The previous standing head, torso and arm/item pictures remain archived source artwork. They are not overlaid on the shared awake recipes. Installed artwork can differ from this project pack; its canonical head, eyes, face geometry, dialogue and reaction pictures are retained during the local migration.

A complete-picture character uses one `complete` body variant and a recipe containing only that body slot. Legacy complete-picture imports can be converted without manufacturing separate eyes or limbs. Nod and whole-picture motion still work; tracking gaze requires an authored head and eye pair.

Original editable source PNGs and their placements remain pack resources. Shared replacement PNGs retain their original dimensions in editor source records, while normalized runtime images remain on the character canvas.

Posture-specific arms were produced with the built-in image tool. Exact prompts are retained in `pose-arm-prompts.json`; generated originals remain in `artwork-sources/`, and the runtime PNGs are normalized to the existing 1024 stage. Placement, scale and shoulder pivots are measured in each pose rather than copied from the other posture. Old complete-head expression images remain archived sources; they are no longer stacked into the awake face system.
