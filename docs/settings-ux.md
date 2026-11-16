# Settings: scope, hierarchy and flow

A review of the Slint Settings window as it stands on `refactor/settings-registry`,
and the structure it should move to. All four steps in section 6 are done; what each left out is listed at the end.

## 1. What is wrong today

**Scope is not visible.** A setting can apply to the whole app, to one
character, or to one character standing on the desktop. The window does not
say which, and mixes them on one page.

| Problem | Where |
| --- | --- |
| The character cards select a "current" character, but selecting does nothing you can see. It only decides what "Add … to the desktop" adds. | Characters |
| Library, app-wide switches (Show, Frame rate) and per-instance sections sit on one page. | Characters |
| Each instance adds a five-row section. Ten characters make a 50-row page. | Characters |
| Waving hand and Music pose shape one character's body but "apply across characters". | Music response |
| About 25 controls on one page, including a trial feature with four engines. | Music response |

**Settings have no single home.**

| Setting | Appears on |
| --- | --- |
| Ink fade | Ink & text, and Pointer → Tap stamp |
| Tap sound preview | Sound, and Pointer → Custom pointers |
| "Change shortcuts" | Twice on Spotlight & lens |
| Start at sign-in | Shortcuts (it is not a shortcut) |
| Reduced motion | Display & motion, referenced from Characters |

**There is no front page.** The window opens on Pointer. The annotation
master switch, start at sign-in, activation style and which display to draw
on are the first things a new user needs, and they are spread over three
pages or missing from Settings.

**Rows are hard to scan.**

- Action rows have labels that do not name anything: "Another one",
  "Positions", "Hand", "Defaults".
- Most sections carry one or two full-sentence notes. The Music page has nine.
- Card rows are right-aligned, leaving the left half of the section empty.
- "Custom pointers" mixes file management, art prompts and a sound preview.
- Navigation group labels (Annotate, Desktop, App) are not aligned with the
  entries under them.

## 2. Scope model

Every setting belongs to exactly one of four scopes. The scope decides where
it lives.

| Scope | Means | Lives in |
| --- | --- | --- |
| **App** | One value for TinyPaw | Settings → General, Shortcuts, Sound |
| **Tool** | How one annotation tool behaves | Settings → Annotate pages; the quick bar edits the same values |
| **Character** | Part of what a character *is*: art, lines, reactions, sounds, bop style, hand position, music pose | Character studio only |
| **Instance** | One character standing on the desktop: shown, size, eyes, idle, place | Settings → On the desktop, and the character's right-click menu |

Rules that follow:

1. One setting, one home. Another page that needs it links there; it never
   repeats the control.
2. Settings never edits a character. It links to the studio.
3. The studio never holds an app or instance setting.
4. A page shows one scope. If it must show two, the second is a clearly
   titled section at the bottom.

## 3. Hierarchy

```
General                     app      (new; the page Settings opens on)
  TinyPaw          Annotation on · Start at sign-in
  Drawing          Activation style · Draw on (display) · Hide the quick bar
  Motion           Motion preference
  About            Version · Open data folder · Licences

ANNOTATE                    tool
  Pointer
    Pointer        cards (packs + Laser)
    Appearance     Style · Size · Paw angle · Forearm colour (when it applies)
    Tap stamp      Stamp            (Fade → link to Ink & text)
    Your pointers  Add · Import · Re-aim · Remove
    Make artwork   Copy pointer prompt · Copy forearm prompt   (collapsed)
  Ink & text
    Ink            Colour · Width · Fade
    Text labels    Size · Colour · Backing
  Spotlight & lens
    Spotlight      Size · Darkness
    Lens           Magnification · Size · Source · Refresh
                   (one "Change shortcuts" link at the foot of the page)

CHARACTERS                  instance + app
  On the desktop
    (top)          Show characters · Add a character…
    (list)         one row per instance: picture, name, Shown switch, ▸
                     ▸ expands to Size · Eyes follow · Idle motion · Remove
    All characters Frame rate · Reset positions
    Library        Open the Character studio…
  Music & dance
    Listening      React to music · Listen to · Hearing meter
    Movement       Strength · Stand at energy
    Dancing        Change every · Light-wave item · Styles in Random
    Follow beat    Follow beat    (trial; Sensitivity · Timing · Engine collapsed
                                   under "Advanced")

SYSTEM                      app
  Shortcuts        Available everywhere · While drawing · Restore defaults
  Sound            Tap sound · Volume · Mute taps · Laser taps · Mute everything
```

What moves:

| Setting | From | To |
| --- | --- | --- |
| Start at sign-in | Shortcuts | General |
| Activation style | Shortcuts | General |
| Draw on, Hide bar, Motion | Display & motion | General (the page goes away) |
| Ink fade on Pointer | Pointer | removed; link to Ink & text |
| Tap sound preview on Pointer | Pointer | removed; Sound has it |
| Character cards | Characters | the "Add a character…" picker |
| Waving hand, Music pose | Music response | Character studio (per character) |

Eight pages become seven. `mascot.character` stops being a setting the user
sees; the picker passes the chosen character straight to `instance.add`.

## 4. Flows

**First run.** Welcome window → "Turn on" → Settings opens on General.

**Opening Settings.** From the tray or the quick bar: General, or the page
that was open last. From a tool's quick-bar button: that tool's page. From a
character's right-click menu ("Settings for Yohta"): On the desktop with that
instance expanded.

**Putting a character on the desktop.**

1. On the desktop → "Add a character…".
2. A picker shows the library as cards. Its last card is "New character…",
   which opens the studio.
3. Choosing a card adds the instance, closes the picker, and expands the new
   row.

**Changing how a character looks or talks.** The instance row has "Edit Yohta
in the studio…". Settings stays open; the studio is its own window and owns
the unsaved-changes prompt.

**Changing a shortcut from a tool page.** One link at the foot of the page
opens Shortcuts scrolled to that tool's row.

## 5. Row and page rules

- A row label names the thing ("Positions" → "Character positions"); the
  button says what happens ("Reset").
- One note per section at most, one line. Longer help goes behind a "More"
  disclosure on that row.
- A section that only matters when a switch is on is hidden while it is off
  (Music already does this).
- Experimental settings carry a "Trial" tag and keep their fine tuning under
  "Advanced".
- Cards align left and wrap.
- Destructive actions (Remove, Restore defaults) sit last in their section and
  ask once.
- Every page fits one scope and at most five sections.

## 6. Order of work

| Step | Change | Touches |
| --- | --- | --- |
| A | New navigation, General page, remove duplicates, rename rows, trim notes | `pages.rs` only |
| B | On the desktop: instance list with expanding rows, Add picker | `pages.rs`, `kit.slint`, `item.slint`, `link.rs` |
| C | Music & dance: Advanced disclosure; move hand and pose to character overrides in the studio | `pages.rs`, mascot settings, studio |
| D | Kit polish: left-aligned cards, nav alignment, "More" disclosure, Trial tag | `kit.slint`, `tokens.slint` |

A is data only and can ship alone. B needs two new kit pieces (expanding row,
picker). C changes where settings are stored and needs a migration.

### Step A, as built

Done in `pages.rs`, plus one line in the shell so the first page has no group
label. Left for later because they need more than page data:

- "Annotation on" on General. It is a command today, not a registered setting.
- About: only the version is shown. Open data folder and Licences need actions.
- The Add picker. Until step B, On the desktop keeps the character cards in an
  "Add a character" section, next to the Add button they feed.
- "Advanced" disclosure for Follow beat, and the hand and pose move (step C).

A test now fails if any setting appears on two pages.

### Step B, as built

- On the desktop lists one row per character: picture, name, Shown switch and
  Edit. The row opens to Size, Eyes, Idle and Remove; one row is open at a time.
- "Add…" turns the page into the picker. Choosing a card adds that character
  (`settings-action instance.add.<character>`), returns to the list and opens
  the new row. The library selection (`mascot.character`) is no longer shown.
- Cards now start from the left. They do not wrap yet (step D).
- Which row is open and whether the picker is showing live in the Settings
  process (`link::View`), not in the app.
- Debug builds accept `TINYPAW_UI_CLICKS` so a page can be clicked through
  without the mouse; that is how this step was checked.

Not done: "Edit in the studio" on a row, and opening Settings on a given
character from its right-click menu.

### Step C, as built

- Music pose and the waving hand (horizontal, vertical, angle, range) are now
  each character's own. They are edited on the studio's Music tab and saved at
  once to the character's overrides file as `music`, beside its bop style.
- No values are moved. A character without its own follows the old app-wide
  settings (`mascot.music_pose`, `mascot.wave_*`), which stay registered and
  keep whatever the user had, so nothing changes on upgrade. Settings no
  longer shows them; the command line can still set them.
- Music & dance drops the pose row and the Waving hand section and links to
  the studio instead. Stand at energy stays, as one value for everyone.
- Follow beat's Sensitivity, Timing and Engine sit under an "Advanced" row
  that shows or hides them.

Not done: an exported pack does not carry the character's music pose and
hand; they stay with the user's overrides. Setting a character back to the
app-wide values drops its own, so it follows them again.

### Step D, as built

- Cards wrap onto further rows when they do not fit one (pointers, the Add
  picker).
- Navigation group labels line up with the entries under them, with space
  above each group.
- A section can carry a tag; Follow beat shows "Trial" instead of having it in
  its title.
- A note can fold longer help behind "More" (`note_more` in `pages.rs`). The
  shortcut notes on Spotlight & lens and the Movement, Dancing and beat engine
  notes on Music & dance use it. A test fails if any note's visible line is
  longer than 150 characters.

Still open from the whole plan: "Annotation on" on General, the data folder
and licences under About, "Edit in the studio" on a character's row, opening
Settings on one character from its menu, and carrying a character's music
pose and hand in an exported pack.

### Restyle toward the scoping mockup

The steps above followed this document and not the mockup shown during
scoping, so the result looked unlike it. This pass closes the visible gap
while keeping the page structure:

- A header bar with the app name and the Annotation master switch
  (`annotation` in `settings-state`, `settings-action annotation.toggle`).
- Line icons beside each navigation entry (`pages::icon`, drawn as paths).
- Compact 44 px rows with hairlines between them, and cyan switches.
- Characters: the section is "On the desktop · N". Each row shows the
  picture, name, size slider and switch on one line; the dots open Eyes, Idle
  and Remove. The Library sits on the same page: clicking a card puts that
  character on the desktop, and the last card, New, opens the studio. The
  separate picker page from step B is gone.
- The navigation group is "Desktop" again, with Characters and Music & dance.

Not matched yet: which display a character is on, Hide all, a row per music
source with its own meter, inline Follow beat dropdowns, Export, Import and
Remove of library characters from Settings, and the Character studio layout.

### Music & dance, toward the mockup

- The page's master switch ("Characters move with playback") sits beside the
  title. Off, the page is that switch and one line.
- "Listen to" is one row per source with its own live level; the chosen one
  is marked Selected, the others Playing or Not playing. Clicking a row
  chooses it. `settings-state` now sends each source's level.
- Follow beat's Sensitivity and Timing are side-by-side buttons and Engine a
  dropdown, all visible; the Advanced fold from step C is gone.

Differs from the mockup: Sensitivity, Timing and Engine are three rows, not
three dropdowns on one line, and Stand at energy and the Dancing section are
kept (the mockup left them out).

### Character studio, toward the mockup

- A toolbar under the header holds the Character / Pointer switch and the
  character picker; "Unsaved changes", Discard and Save show there when there
  is something to save. The left "Editors" navigation and the in-page save bar
  are gone.
- The preview sits on the left and stays beside whichever tab is open. The
  pointer editor keeps its panel on the right.
- Six tabs: Library, Artwork, Reactions, Speech, Motion, Advanced. Behavior is
  split into the middle three. Lines for petting, sleep, wake and neighbours
  stay in Reactions beside those settings, which differs from the mockup.
- Each group sits on a card, as in Settings.
