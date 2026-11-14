# Character and editor improvement plan

Updated 14 November 2026. Planned work.

This is the main roadmap for consolidating TinyPaw's character artwork, editor and creation prompts. The [artwork rules](character-artwork-rules.md) define the composition contract. This document defines delivery order and acceptance criteria.

The earlier creation, creator UI, behavior, sleep, alignment and bop plans provide historical decisions and supporting detail. For shared artwork, pose composition, editor organization and prompt requirements, this plan and the artwork rules take precedence.

## Outcome

A creator builds one character library, assigns its variants to poses, and chooses how those poses behave. Sitting and standing share a compatible head and eye set. Seated dozing and curled sleeping are two sleep compositions. Nod and the other music motions work on each compatible active composition.

Every required image is explained by a selected pose or capability. The editor, prompt generator, importer and renderer use the same requirements.

## Current baseline

The working tree already contains layered artwork, tracking eyes, head expressions, music motion, sitting and standing music poses, excitement-based posture selection, held and reaction pictures, two authored sleep pictures, alignment controls and pack import/export.

Those features currently use several representations.

| Area | Current issue | Intended result |
| --- | --- | --- |
| Standing music artwork | A separate body/head/arm record bypasses the general layered composition | Standing becomes an ordinary pose using shared parts |
| Eyes and expressions | Tracking eyes, painted eyes and full-head expression images overlap in purpose | One logical eye pair per layered composition, with compatible expression variants |
| Sleep and reactions | Whole-picture replacements duplicate existing parts | Reusable pose recipes and explicit complete-pose compatibility |
| Waving | Layer types and standing arm pictures have separate attachment conventions | One shoulder-to-hand-to-item attachment model |
| Authoring settings | Some hand and dance choices are shared app preferences | Character and pose authoring choices save with their owner |
| Installed artwork | Updating project art can leave the installed character missing new mappings | Validated pack updates carry every declared asset and reconcile the active character |
| Preview and desktop | Separate render paths and draft handling can disagree | Both consume the same resolved composition and current draft |
| Creation prompts | Features independently request overlapping files | A single deduplicated requirement list generates prompts and mappings |

These are review findings. The consolidation described below still needs implementation.

## Scope and rules

The [artwork rules](character-artwork-rules.md) own the part categories, reuse rules and pose recipes. Apply those rules consistently through the following model.

- The asset library owns each image and its variants.
- A pose references library entries and supplies their placements and attachments.
- Posture selects sitting, standing or a sleep composition.
- Motion selects Nod, Groove, Rubber hose, Compress or Light wave.
- Expression selects the active eye variant and additional face detail where needed.
- An item assignment selects a prop and compatible holding grip.
- Excitement chooses eligible awake postures and motion strength over time.
- Sleep behavior chooses an enabled sleep recipe after its quiet and proximity conditions are satisfied.

A new variant needs a visible reason such as a changed silhouette, viewing angle, facial detail or grip. Use animation for bobbing, breathing, sway, timing and item glow.

### Capability rules

| Capability | Requirements |
| --- | --- |
| Basic complete-picture character | One complete-pose image |
| Layered sitting or standing | Body, compatible head and eye pair, plus the selected optional parts |
| Tracking gaze | Eye backgrounds, pupils and masks in the active eye set |
| Blink or seated doze | Closed-eye variant compatible with the active head |
| Curled sleep | Compatible curled composition or an existing complete sleeping image |
| Nod | Whole-picture motion for a complete image, or head/body motion for a layered pose |
| Wave | Compatible shoulder, holding arm, grip placement and item |
| Pickup | Current composition with its carrying transform, or an assigned held composition |

The UI exposes the capabilities the selected artwork supports and identifies the missing assignments for an incomplete setup.

## Editor workflow

Use one character workspace and one draft with a persistent assembled preview.

| Area | Controls |
| --- | --- |
| Parts | Eyes, Head, Body, Arms, Tail and Items, each with variants and a list of poses using them |
| Poses | Sitting, Standing, Seated doze, Curled sleep and optional Pickup, each assigning existing variants |
| Behavior | Reactions, music posture rules, motion, expressions, sleep, dialogue and sounds |
| Arrange preview | Select and position parts, set attachments, align poses and inspect the resting reference |
| Test preview | Try the current composition and behavior, including seated/standing music, both sleep poses and pickup |

Selecting an existing variant shows where it is used. Editing a shared variant updates all its uses in the draft. **Make variant** creates a separately editable copy when the creator wants different artwork or geometry.

Treat the two eyes as one logical assignment. Keep backgrounds, pupils, masks and tracking limits in the eye-set editor. Head and eye compatibility stays visible when assigning a head variant.

Hand placement edits the active pose's shoulder, grip, offset, resting angle and swing range. Moving the arm carries its item and grip fingers. Alignment changes have an immediate observable effect in the preview.

Show the actual posture and motion during automatic or random playback so the creator can distinguish the active result from a configured choice.

Save commits the complete draft. Discard restores saved assignments and settings. Desktop draft preview follows the same Save and Discard behavior.

## Music and sleep behavior

Sitting and standing are eligible postures for the same music motions. Preserve beat phase through style changes and pose changes. Motion eases toward revised timing and strength targets.

Automatic excitement uses sustained normalized audio activity and separate stand-up and settle-down conditions. The thresholds and delays prevent short gaps from repeatedly changing posture. Provide explicit Sitting and Standing choices for authoring and testing.

A seated character continues to nod and sway while music plays. Wave becomes eligible in each posture that has compatible arm and grip assignments. Item selection and hand alignment are saved with the relevant character and pose.

Sleep starts after the configured quiet interval while the cursor is away and interactions allow it. Retain seated dozing and curled sleeping as independent enabled recipes. Choose a recipe for the nap, keep it stable through that nap, and avoid an immediate repeat when another recipe is available. Existing wake-by-touch and wake-by-audio choices continue to govern waking.

Open eyes, closed eyes and breathing are reused across compatible poses. Additional sleep artwork is justified by a changed body or head silhouette.

## Creation prompts and imports

Feature selection resolves a requirement graph before generating filenames or prompts.

1. Select the desired postures and capabilities.
2. Resolve the shared parts, expression variants and attachments they need.
3. Reuse compatible assignments already in the character library.
4. List the remaining unique images once.
5. Generate the complete folder prompt, individual-image prompts and matching manifest from that list.
6. Validate the returned files against the same requirements and show their assignments in the editor.

Selecting sitting and standing requests their shared head and eyes once. Selecting blink and seated sleep requests the closed-eye variant once. A shared item is requested once across compatible holding poses.

Each prompt specifies the approved reference, image size, source geometry, named attachments, overlap at joints and compatible face placement. The assembled preview verifies alignment. Preserve the original source and transparent padding through import and later edits.

## Storage and compatibility

| Data | Owner |
| --- | --- |
| Image identity, variants and source artwork | Character asset library |
| Part selections, placements, ground alignment and attachments | Character pose |
| Eye expressions and tracking geometry | Compatible eye set |
| Dance selection, posture rules and eligible styles | Character behavior |
| Hand alignment and grip/item placement | Character pose and holding assignment |
| Sleep recipes and selection rules | Character behavior |
| Audio input/source and app-wide listening controls | App preferences |

Version the representation and validate references on load. Old packs, complete-pose images, head expressions, source placements, dialogue, sounds and user overrides need a migration path through the existing staged save and installation flow.

Register existing complete sleep and held images as complete-pose variants. Preserve authored appearance while shared layered versions are prepared.

Choose a canonical front head and eye geometry for the current Yohta sitting and standing poses. Keep an intentionally different head as a named variant. Equal file content can share storage, while semantically different variants retain their identities.

Import each non-default global hand adjustment into the selected character's corresponding pose during migration. Record that migration so it runs once. Newly authored adjustments use character/pose ownership.

Use the existing import transaction, revision checks, cancellation and recovery mechanisms. Pack updates validate all declared files, commit their manifest and artwork together, and refresh the active character after the commit. Export carries the shared identities, pose assignments, sources, behavior and overrides needed to reproduce the result.

## Delivery order

### 1. Inventory and contract

Create an inventory for the current Yohta artwork and one complete-picture character. Record every image, its embedded parts, variants, pose uses and attachment geometry.

Define the asset, eye-set, pose and capability records against the artwork rules. Identify the canonical head/eye pair and the existing seated, standing, dozing, curled and held compositions.

Acceptance criteria

- Every existing visible pose and reaction has a documented destination.
- Every requested image has an explicit role.
- Shared and embedded parts are identifiable.
- The minimal required artwork list is reproducible from selected capabilities.

### 2. Shared storage and rendering

Implement the library and pose resolver. Adapt existing general parts, standing music art and complete-picture replacements into that model.

Use one resolved composition for desktop rendering, Settings preview, eye tracking, bounds, hit testing, attachment transforms and carrying.

Acceptance criteria

- Sitting and standing use the same selected front head and eye set.
- Nod and gaze work in both postures.
- Blink and seated doze use the same closed-eye assignment.
- Complete-pose compatibility accounts for embedded parts.
- Shoulder, hand, item and grip transforms agree in drawing and hit testing.
- Pose transitions keep the intended character scale and ground placement.

### 3. Unified editor

Add Parts and Poses editing to the shared workspace. Reuse existing arrangement, source placement, joint, alignment and behavior controls where their semantics match the new ownership.

Acceptance criteria

- A shared edit updates every affected pose in the draft.
- Make variant allows an independent edit.
- The preview identifies the selected layer, active pose and missing capability assignments.
- Hand controls work for each compatible pose.
- Save, Discard and restart reproduce the expected draft or saved result.

### 4. Prompt and import consolidation

Derive all creation prompts and import assignments from the capability resolver. Integrate complete-folder and individual-PNG workflows.

Acceptance criteria

- A sitting-plus-standing request includes one shared head and one eye set.
- Blink-plus-doze includes one closed-eye variant.
- Shared images have one assignment across all their uses.
- Missing or incompatible references produce actionable feedback.
- Declared artwork reaches the installed pack and active renderer together.

### 5. Current character migration and validation

Convert the current character's shared front head/eyes and awake poses. Retain both existing sleep recipes and the held artwork. Apply the character/pose ownership migration for existing settings.

Acceptance criteria

- Sitting and standing both support the selected music motion.
- Both sleep poses remain available, stable during a nap and recover correctly on wake.
- Nod, wave, random styles, excitement and item changes retain their expected timing.
- Existing dialogue, sounds, reactions, artwork alignment and pickup contact survive migration.
- Save, reload, export and reimport retain shared references and variant selections.
- Reduced motion, transparent hit testing and transformed bounds remain correct.
- Rendered comparisons and native desktop checks cover posture, sleep, carrying and expression transitions.

## First milestone

One migrated Yohta character uses a shared front head and eye set across sitting and standing, supports Nod and gaze in both, and exposes those assignments in the editor. It saves and reimports correctly while retaining the existing two sleeping pictures and held pose.

That provides an end-to-end baseline for further variant cleanup.

## Supporting plans

- [Artwork composition rules](character-artwork-rules.md)
- [Creator interaction and draft workflow](character-creator-ui-flow-plan.md)
- [Pose alignment and pickup geometry](character-pose-alignment-plan.md)
- [Headpat and sleep behavior](character-headpat-sleep-plan.md)
- [Beat timing and motion validation](bop-improvement-plan.md)
- [Earlier character creation plan](character-creation-plan.md)
- [Earlier behavior decisions](character-behaviors-plan.md)
