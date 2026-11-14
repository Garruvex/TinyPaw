# TinyPaw next phase

Proposed roadmap, 11 November 2026. This review covers the Windows alpha working tree. It uses the current views, command registrations and desktop host as the baseline. The [character scope](mascot-feature.md) defines the pack format and limits. This plan settles where those features belong in the app and the order to ship them.

## Character and editor consolidation

Added 14 November 2026. The [character and editor improvement plan](character-editor-improvement-plan.md) is the current roadmap for shared artwork, pose variants, character editing and creation prompts. Its first milestone migrates Yohta to one shared head and eye set across sitting and standing, with working Nod, gaze and save/import/export, while retaining both sleep poses and held artwork. The earlier deliverables below provide the original roadmap context.

## What to build first

| Order | Deliverable | Ship when |
| --- | --- | --- |
| 1 | Menu cleanup and a native desktop Lens trial | Users can find a pointer, a character and the control to stop drawing. Lens follows the paw from one app to another on a real Windows desktop. Gate the Lens default separately so it cannot hold up the menu fixes. |
| 2 | Import one PNG and optionally add a pressed expression | A first-time user can install, click and move a character using just its normal image. Adding a second full-character PNG gives it a custom pressed expression that survives restart. |
| 3 | Character reactions and layered packs | A flat PNG and a layered Yohta work in the same preview and resident renderer. Users can add a tail or head to an installed character visually. |
| 4 | Music response | Optional Windows playback drives visible motion, settles in silence and releases its audio resources when switched off. Gate beat sync separately. |

## Menu and icon review

| Surface | Finding in the current app | Proposed change |
| --- | --- | --- |
| Annotation bar | Eleven controls share the row, including tap mute. Exit turns annotation off and clears marks. The paw picker always calls the paw Yohta. Lens lacks the selected styling used by the other tools. | Keep Pointer, Text, Pin, Ink, Undo and **End annotation** directly available. Group Spotlight, Lens, Freeze and Clear marks in **More** with labels. Show the active tool on the More button when appropriate. Keep Return live directly accessible during Freeze. Use the installed paw's display name beside its thumbnail. Explain mark clearing in the End annotation tooltip. |
| Quick wheel | The eight directions support muscle memory. Lens and unavailable actions can both say “Coming soon.” Clear is easy to flick into accidentally. | Keep the existing tool directions. Replace only the Clear wedge with reversible Undo. Keep Clear marks in More and its current keyboard binding. Give Lens the hint “Magnify under the pointer” in desktop mode. Show the actual reason when a command is unavailable. |
| Tray | Annotation On, Annotate now, Freeze, Show character, Settings and Quit share a menu. “TinyPaw On” sounds like it also controls the mascot. | Keep **Annotation enabled**, **Show character**, **Settings** and **Quit TinyPaw**. Keep **Annotate now** as the recovery action when enabled and waiting for input. Move Freeze to the annotation surfaces and retain its global shortcut. Keep the tray left-click activation gesture. |
| Character menu | Five fixed rows include tap mute and eye tracking. Settings opens the general window. | Keep left-click as the launcher gesture until click reactions exist; then right-click opens the menu and left-click reacts ([character creation plan](character-creation-plan.md#decisions)). Show **Annotation enabled**, **Character settings…** and **Hide character**. Add a pose choice only for packs with multiple poses. Put eye tracking in Character settings and tap mute in Tap sounds. Derive menu height and edge placement from visible rows. |
| Settings and first run | Appearance mixes pointer packs, ink, text, Lens and Spotlight. Two paw icons mean different things. Streaming advice appears on every page. The reduced-motion hint points to Appearance while the setting lives in Controls. First-run tips hard-code editing keys. | Use the destinations below. Label the header switch **Annotation**. Put the streaming tip next to display sharing controls, link motion help to the actual setting, and render shortcut hints from current bindings. Keep onboarding to one primary action. |

### One header for Settings

On Windows, Settings currently stacks a system title bar above a 60-pixel TinyPaw header. Merge them into one row in the menu overhaul. Put the paw and **TinyPaw Settings** on the left, leave a broad draggable area in the middle, and place the **Annotation** switch before the standard minimize, maximize/restore and close buttons. Prefer Windows-drawn caption buttons in an extended frame. Keep the switch outside the drag region.

Scope the frame integration to the Settings window. Its window helper is also used for the fixed-size first-run card. Keep the native frame as the initial path on macOS and Linux, with one compact annotation control in the settings content. This keeps the layout portable while allowing each desktop to handle window management.

Accept the Windows header when dragging, double-click maximize/restore, edge resizing, the system menu, maximize-hover Snap Layouts and keyboard close work at normal and mixed DPI. Closing Settings should leave TinyPaw available in the tray. [Windows custom frame guidance](https://learn.microsoft.com/en-us/windows/win32/dwm/customframe) · [Snap Layouts](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/ui/apply-snap-layout-menu)

### Settings destinations

| Destination | Visible on entry | Reveal when relevant |
| --- | --- | --- |
| Pointer | Active pointer thumbnail/name, Add pointer, size and presentation | Re-aim, pressed image, forearm, pack tap, artwork prompts, folder import and removal in the selected pointer's editor or Add flow. Show joint controls for compatible art. |
| Character | Current character and preview, Show on desktop, Add character, size | Eye tracking for supported art, idle motion, music response, pose when multiple exist. Keep edit, export, remove and reset position beside the selection. |
| Annotation | Ink colour, width and fade | Expand Text for label size/colour/backing, Spotlight for radius, Lens for size/zoom and advanced source selection. The bar edits the same live ink values. |
| Controls | Activation style, shortcut bindings, global motion preference | Conflicts and restore defaults alongside bindings. Use “Follow Windows,” “Full motion” and “Reduced motion” as display labels for existing values. |
| Display | Monitor selection and quick-bar visibility | Show share guidance beside bar visibility. Use one selection indicator and retain a meaningful missing-monitor message. |
| Tap sounds | Mute, sound, volume and Play | WAV import and laser tap preference. Music response lives beside the character preview. |

Keep the existing vector icon set and its 24-unit grid. Use a pointer arrow for Pointer navigation, a character head for Character, a pen for Annotation, keyboard for Controls, monitor for Display and speaker for Tap sounds. Reserve a music note for character playback response. Pair icons with text in menus and navigation. Compact toolbar buttons get an accessible name, shortcut tooltip and visible focus. Use a selected shape or check in addition to colour. Verify at 100%, 150% and 200% scaling on light and dark desktop content.

Implement menu labels, icons, shortcuts and availability from shared command presentation keyed by existing registry IDs. Keep surface ordering explicit so the wheel retains fixed directions. Settings pages can compose registered settings into task-specific editors. Preserve settings keys, bindings, CLI names and old pointer packs. Check that the toolbar and More panel consume mouse events across their entire bounds so selecting a tool cannot leave ink. Relevant code is in [the toolbar](../src/view/mod.rs), [wheel](../src/view/wheel.rs), [icons](../src/view/icons.rs), [settings](../src/view/settings.rs), [tray](../src/platform/tray.rs) and [resident app](../src/app.rs).

## Make Lens work wherever the paw points

Today the Windows host registers a live DWM thumbnail of the previously focused or explicitly chosen window. It crops that one window as the paw moves. This naturally leaves TinyPaw's marks out of the sampled image, but moving onto another application still samples the original source. A missing source currently sends the user back to Point. [Windows Lens implementation](../src/platform/windows.rs) · [DWM thumbnails](https://learn.microsoft.com/en-us/windows/win32/dwm/thumbnail-ovw)

Make **Desktop under pointer** the proposed default for new installs. Prototype the windowed Windows Magnification API in a small non-activating lens host. Set its source rectangle in desktop coordinates around the actual tip and apply the current 2× or 4× scale. Windows automatically excludes the magnification window. Supply an exclusion list for TinyPaw's overlay and other visible helper windows, updating it as windows are recreated. Scope that filtering to the magnifier so the user can still share the annotated monitor. [Magnification overview](https://learn.microsoft.com/en-us/windows/win32/winauto/magapi/magapi-intro) · [Source rectangle](https://learn.microsoft.com/en-us/windows/win32/api/magnification/nf-magnification-magsetwindowsource) · [Window filters](https://learn.microsoft.com/en-us/windows/win32/api/magnification/nf-magnification-magsetwindowfilterlist)

Keep **Lock to a window** under Advanced using the existing DWM path, and honour saved named-window choices. When desktop magnification fails to initialise, offer the window mode with a short reason. Release the native control on tool exit. Before changing the default, follow a moving target across two overlapping apps, a menu, a borderless game and mixed-DPI displays. Check zoom alignment, focus, no repeated lens/ink, frame pacing and the receiving end of a Discord monitor share. Record exclusive-fullscreen and protected-content results separately. These are proposed checks, not verified compatibility.

## Add a character from one PNG

1. Open **Settings → Character → Add character**. Drop one transparent PNG on the Character page or choose **Select PNG**. Put **Import pack folder** alongside it as a secondary action. Keep **Add pointer** on the Pointer page with its tip/wrist workflow.
2. Immediately preview at desktop size against light and dark backgrounds. Fill the name from the filename, suggest a ground point at the bottom centre of the visible artwork, and let the user click to adjust it. Name, pose and pivot editing are optional on this path.
3. Choose **Use character**. Copy the assets into managed character storage, select the new pack and show it on the desktop at the existing size and anchor. The button makes showing it explicit. Cancel discards the draft and preserves the previous selection and visibility.
4. Show what the art supports in the preview. A flat PNG can react to a click, drag and bounce as one piece. Offer **Add moving parts** after installation. Users can stop after step 3 and come back later.

### How a single PNG responds

Treat the image as one sprite. On import, decode it once, cache a mask from its alpha channel and use its visible bounds to propose a pivot near the feet. Use the imported canvas dimensions for layout. The desktop host already distinguishes a click from a drag. Give it the selected sprite's mask and pivot in place of Yohta's fixed rig dimensions. Transform the pointer back into the sprite's local coordinates before checking its mask so the clickable silhouette moves with the artwork.

| Try it in the import preview | Response with only one PNG |
| --- | --- |
| Click the character | On press, briefly squash the whole image around the feet. A short release opens the menu. In the preview, use a sample menu so testing a click keeps the import draft open. |
| Grab and move | After five logical pixels, keep the original grab point under the pointer. Release places the character. The preview confines the drag to its stage. Keep the gesture captured if the pointer crosses a transparent gap. |
| Click the space between a curled tail and the body | That transparent gap lets the click through on the desktop. Test the same shape in preview and against an app behind the real mascot. |
| Try a music pulse | Preview a simulated whole-body bounce around the same pivot. When live playback response ships, its level drives that motion. Reduced motion keeps the sprite still and retains click/menu and placement. |

### Optional second PNG for a pressed expression

Offer **Add pressed image** in the import preview and later under **Character → Edit character**. For example, a creator can draw Yohta sitting normally, duplicate the canvas, change his face to a smug grin and export both full-character images. The two images share canvas dimensions, placement and ground anchor. A press swaps the entire sprite in place. The artist can keep the eyes and tail painted into both drawings.

| Artwork supplied | Press and hold | Release or drag |
| --- | --- | --- |
| Normal PNG only | Apply the default squash when motion is enabled. | Restore normal scale. A short release opens the menu. |
| Normal and pressed PNGs | Show the authored pressed image at its original scale. Reduced motion still allows this static expression change. | Restore the normal image on release, cancellation or transition to drag. Only a short release opens the menu. |

Let the creator press and hold in the preview to compare expressions before saving. Keep the gesture captured even if the pressed image has transparent pixels where the press began. Store the optional image with the character, include it in folder export/import, and allow replacement or removal. Removing it restores the one-PNG response. An invalid replacement leaves the last saved expression usable.

Verify both paths in preview and on the desktop, including a drag started from a press, release outside the silhouette, reopening after the source PNGs are moved, and a wrong-size replacement. A character with no pressed image remains valid.

### Adding independent movement

The layer editor starts from the installed character or a new draft made from separate PNGs. Accept cropped parts of different sizes and let the creator move, resize and rotate them, then set attachment points in a separate mode. Prepare matching canvas-sized layers and generate the manifest on Save. Existing aligned layers keep their placement. For example, adding a tail requires a body with the painted tail removed and a separate tail PNG; the editor handles its canvas preparation. Preview a sway immediately so the artist can see a doubled tail or a gap at the root. Complete eyes can remain static layers attached to the head; white/pupil/mask assets are required only for tracking. Blink, optional expressions and a second pose belong behind further controls. Include a downloadable or exportable example folder made from the same assets TinyPaw loads. See the [cropped-parts improvement plan](character-creation-plan.md#improvement-import-and-arrange-cropped-parts) for delivery order and acceptance checks.

Use the existing pointer import's file chooser, draft/preview interaction and data-directory conventions wherever they fit. Give characters their own loader and storage. Validate the [versioned pack rules and limits](mascot-feature.md#validation-and-installation) before decoding large images or replacing files. Explain errors at the slot, e.g. “Tail is 900 × 900. This pose uses a 1024 × 1024 canvas.” Missing optional art disables the matching motion. Reject invalid declared art. Stage and validate the full replacement before switching active packs. On duplicate identity, offer **Keep both** or **Replace**. Preserve the active pack on failure and fall back to bundled Yohta if the active custom pack is removed.

**Import acceptance.** Give someone a transparent PNG and ask them to put it on the desktop without coaching. Target under one minute and zero required alignment edits. Then move the source file and restart. Repeat with a layered folder, a missing declared image, a duplicate ID, a cancelled replacement and removal of the active pack. Verify selection, source files and prior artwork at each step.

## Character interaction and music

Build on the resident character's existing gaze, blink, idle tail movement, menu and five-logical-pixel drag threshold. The preview currently simulates a beat. Live playback analysis is a separate deliverable. [Desktop character](../src/mascot/desktop.rs) · [Prototype](mascot-prototype.md) · [Engine and artwork study](mascot-engine-study-2026-11-11.md)

| Interaction | Proposed behaviour | Acceptance |
| --- | --- | --- |
| Click and drag | A short press gives a small squash or an authored expression, then release opens the existing menu. Crossing the drag threshold takes over, with a bounded lean and a short settle on release. | Drag never opens the menu or inks the desktop. Test a fast first click from outside the silhouette into it. The current polled hit test can miss this transition. Verify clicks through blank pixels with another app behind it. |
| Pose and parts | Share pose evaluation, silhouettes, masks and pivots between import preview and desktop. Align pose swaps at the feet. | Single PNG works immediately. Layered gaze stays within the eyes on head tilt, tail roots stay covered and pose swaps preserve placement. Check menu and drag ownership while animation is active. |
| React to music | Opt-in **React to playback** with a live level meter and one strength control. Feed a smoothed Windows output level into the rig. A flat PNG bobs as one piece. A layered pack can nod its head with a smaller body bounce. | Quiet playback settles cleanly. Switching audio outputs reconnects. TinyPaw's taps alone do not sustain motion. Metering and extra animation work stop when disabled or the desktop character is hidden, except during an explicit live preview. |
| Follow the beat | After level response works, trial a tracker that predicts the next beat. Add **Beat timing** with **Early**, **On beat** (default), and **Late** for the visible bounce. See the [beat detection trial](architecture-improvement-plan.md#live-beat-detection-trial). Retain a labelled simulated beat for authoring without audio. | Test known click tracks at 80, 120 and 160 BPM, tempo changes, speech and silence. Record missed, extra and late accents before choosing whether to ship. |

Keep playback analysis in a platform service separate from tap-sound output. Start with output-session levels excluding TinyPaw where available. Pass only motion signals to the rig. Reduced motion suppresses bounce, sway and drag lean. Pause music-driven motion during drag or menu use and return smoothly to idle or playback. Measure a release build with the character off, idle, responding to playback and hidden during annotation against the [recorded desktop character baseline](mascot-feature.md#stage-1-implementation). A standing Yohta pose can arrive when matching artwork is ready. It does not block single-image import or the desktop Lens trial.
