# Character control audit

Recorded 11 November 2026 against the working tree, as step 1 of the [character creator UI plan](character-creator-ui-flow-plan.md#control-audit-remove-settings-that-have-no-useful-effect). Later steps cite this table and update it when a control changes.

Classes: **Working**: has a consumer and a visible effect in every state it is shown. **Conditional**: has an effect only in some states, and is now shown only in those. **Misleading**: shown where it does nothing or claims more than it does. **No consumer**: nothing reads it.

"Fixed in step 1" means the change shipped with this audit. Rows marked with a later step are known issues left for the step that owns them.

## Character page (Settings → Character)

| Control | Source of truth | Consumer | Applies when | Saved in | Preview effect | Class | Treatment |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Character cards / Add character | Installed packs, `mascot.character` | App rig loading | Always | Preferences | Preview switches character | Working | Keep |
| From parts… / Import folder / Export… / Remove | Pack store | `pack.rs` transactions | Remove: custom characters only | Packs | n/a | Working | Keep; regroup under Character home in step 4 |
| Edit art / Edit a copy | Pack store | Art editor | Always; Yohta edits a copy | Pack | Opens editor | Working | Becomes Customize → Artwork in step 4 |
| Show on the desktop | `mascot.enabled` | Desktop window | Always | Preferences | None (desktop only) | Working | Keep |
| Size | `mascot.size` | `desktop::layout` | Always | Preferences | None | Misleading | Scales the canvas, so padded art shows small. Becomes visible height in step 5 |
| Eyes follow the cursor | `mascot.eyes` | Desktop gaze (`rig.eye_point()`) | Rig has tracking eyes | Preferences | Desktop only | Was misleading | **Fixed in step 1:** shown for any character whose rig can track. Otherwise a hint explains which art is missing |
| Idle motion and blinking | `mascot.idle` | Animator `Inputs.idle` | Not reduced motion | Preferences | Preview idles | Working | The hint and link to Motion settings explain reduced motion |
| Reset position | Desktop anchor | `Desktop::reset_position` | Character shown | Preferences | None | Working | Disabled while the character is hidden |

## Motion tab

| Control | Source of truth | Consumer | Applies when | Saved in | Preview effect | Class | Treatment |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Preset (per part) | Overrides `motion` | Animator | Always | `.overrides/<id>.json` | Live | Working | Keep |
| Strength | Overrides `motion` | Animator: idle, drag and music angle | Always | Overrides | Live | Working | The hint now says it also scales drag and music |
| Speed | Overrides `motion` | Animator idle angle only | Preset has idle motion | Overrides | Live | Was misleading | **Fixed in step 1:** hidden for Still, Hang, Fold, Floppy and Spring, with a reason (`Preset::uses_speed`) |
| Reset to pack / Save motion | Overrides | `save_overrides` | Customised / dirty | Overrides | n/a | Working | Joins the single Save in step 2 |

## Speech tab

| Control | Source of truth | Consumer | Applies when | Saved in | Preview effect | Class | Treatment |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Show speech lines | `mascot.lines` | `Desktop::say` | Always | Preferences | Speak test | Working | Keep |
| Quiet: no idle or music lines | `mascot.quiet_lines` | `Desktop::say` | Lines shown | Preferences | None | Working | Keep |
| Idle lines every … to … | `mascot.idle_min/max` | Idle scheduling | Lines shown and not Quiet | Preferences | None | Was conditional | **Fixed in step 1:** shown only while idle lines can appear |
| Character volume | `mascot.voice_volume` | Line sound playback | A line has a sound | Preferences | Speak test sound | Working | Shared with reaction sounds in phase 4 |
| Per-trigger lines (6 triggers), Sound…, No sound, × | Overrides `lines` | Picker / desktop | Always | Overrides | Speak test | Working | Become Behavior event rows in step 4; all six triggers kept |
| Bubble / Text colours, Bubble width | Overrides `bubble` | Desktop bubble | Lines shown | Overrides | Speak test | Working | Keep |
| Reset to pack / Save lines | Overrides | `save_overrides` | Customised / dirty | Overrides | n/a | Working | Joins the single Save in step 2 |

## Music tab

| Control | Source of truth | Consumer | Applies when | Saved in | Preview effect | Class | Treatment |
| --- | --- | --- | --- | --- | --- | --- | --- |
| React to playback | `mascot.music` | Music meter, animator `Inputs.music` | Always | Preferences | Music test | Working | Keep |
| Listen to (source rows), Or an input | `mascot.music_source` | `platform::music` | React on | Preferences | Meters | Was conditional | **Fixed in step 1:** hidden while React is off, since there is nothing to meter |
| Hearing meter | Live level | Display only | React on | none | n/a | Working | Same gating |
| Music strength | `mascot.music_strength` | Animator `Inputs.music` | React on | Preferences | Music test | Was conditional | Same gating |
| Bop style | Overrides `bop` | Animator | React on | Overrides | Music test | Was conditional | Same gating |
| Follow beat (trial) | `mascot.beat` | Beat tracker | React on | Preferences | None | Working | Keeps its trial label until judged |
| Beat status, Sensitivity, Timing | `mascot.beat_*` | Beat tracker | Follow beat on | Preferences | None | Was conditional | **Fixed in step 1:** shown only with Follow beat on |

## Preview (beside Motion / Speech / Music)

| Control | Consumer | Class | Treatment |
| --- | --- | --- | --- |
| Click, Drag ←, Drag →, Speak, Music | `preview::Test` simulation | Working | The preview still squashes on Click and leans on Drag, unlike the desktop after phase 1. Test character mode in step 4 makes them match |

## Art editor

| Control | Source of truth | Consumer | Class | Treatment |
| --- | --- | --- | --- | --- |
| Ground target and its marker | Pose `ground_anchor` | Desktop layout, lean and squash | Was misleading | **Fixed in step 1:** removed from the target list. Advanced → Resting point (Automatic, Adjust manually, Reset) replaces it, and its guide shows only while adjusting |
| Opens in Set attachment point with Ground selected | Editor state | Preview clicks | Was misleading | **Fixed in step 1:** opens in Arrange with the body selected |
| Arrange artwork / Set attachment point | `Editor.mode` | Preview clicks | Misleading | A global joint mode. Replaced by contextual Adjust joint in step 5 |
| Layer X / Y / Scale / Angle, nudge arrows | Layer placement | Bake on save, preview | Working | Becomes the selected-layer inspector in step 4 |
| Stage width × height | `canvas` | Bake, layout | Misleading | Padding changes desktop size until the Size change in step 5. Fit with margin comes then too |
| Preview zoom, Light / Dark / Grid | View only | Preview | Working | View-only; never saved |
| Part type, Back / Front, On head, Lower / Raise | Pose parts | Rig | Working | On head becomes Moves with in step 5 |
| Faces (Blink, Pressed, Dragging) | Head faces | Rig expressions | Working | Renamed to visible actions in step 4 |
| Eyes: White, Pupil, Mask, Blink, Pupil travel, Keep static layers | Pose eyes | Rig | Working | Guided eye setup in step 4; Pupil travel becomes Movement range |
| "Motion presets, strength and speed come in the Motion tab, next." | none | none | No consumer | **Removed in step 1** (stale roadmap copy) |

## Add character (one image)

| Control | Class | Treatment |
| --- | --- | --- |
| Click-where-it-stands ground picker | Was misleading | **Fixed in step 1:** removed. The resting point is estimated automatically (`rest::estimate`) and saved as `rest_point: "auto"` |
| Name, Cancel, Use character | Working | Keep |

## Still to collect

Native visual checks on light and dark desktops, and at mixed DPI, are release evidence still to collect. The table records source and consumer from the code only.
