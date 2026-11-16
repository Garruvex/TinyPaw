# UI guide

How Settings and the character studio are built. It applies to everything under `ui/`. The reasons and the delivery order are in the [settings and multi-character plan](settings-and-multi-character-plan.md).

## Rules

1. Build pages only from the kit (`ui/kit.slint`) and `Tokens` (`ui/tokens.slint`).
2. No raw sizes, colours or stock widgets in pages or templates. A missing component or token is added to the kit or tokens first, then used. `cargo test` fails otherwise (`src/view/ui_guard_tests.rs`).
3. The only allowed raw value is a window's first size, marked with a `// guard:allow` comment on that line.
4. Colour tokens match `src/view/theme.rs`. Change both together; a test fails if they differ.
5. Every control saves at once on Settings pages. Studio tabs edit a draft and use `SaveBar`.
6. Put a touch area on a fixed surface, never inside the element being dragged. Redraw preview pictures when state changes, not on a timer (see the plan's step 0 results).

## Tokens

| Group | Tokens |
|---|---|
| Surface | `surface`, `header`, `nav`, `raised`, `control`, `hover`, `well`, `line`, `line-strong`, `track` |
| Text | `text`, `text-soft`, `subtle`, `muted`, `faint`, `disabled` |
| Accent | `cyan`, `cyan-text`, `cyan-bg` for selection and primary actions; `amber`, `amber-text`, `amber-bg` for the master toggle and warnings; `danger`, `danger-bg` for destructive actions |
| Type | `font-label` 12, `font-body` 13, `font-nav` 14, `font-h2` 15, `font-h1` 22 |
| Spacing | `space-1` 4, `space-2` 8, `space-3` 12, `space-4` 16, `space-5` 24 |
| Radius | `radius-small`, `radius-control`, `radius-card`, `radius-round` |
| Sizes | `h-tab`, `h-control`, `h-nav`, `h-row`, `nav-width`, `label-w`, `value-w` |

## Components

| Component | Use it for |
|---|---|
| `Page` | The body of a Settings page: title, optional summary, then sections |
| `Section` | A titled group of rows on a card |
| `SettingRow` | One setting: label and hint on the left, the control on the right |
| `Toggle` | An on or off setting |
| `Range` | A number in a range; the slider with our tokens |
| `Segmented` | A short list of exclusive choices (up to about five) |
| `Btn` | An action. `primary` once per view, for the main action |
| `NavItem`, `NavGroup` | The Settings navigation column |
| `Tab` | A strip of tabs in the studio |
| `Card` | A selectable choice with a picture or a short description |
| `ListRow` | A row in a list of things (characters on the desktop, reactions, packs) |
| `Notice` | An inline message; `warning` for things that need care |
| `SaveBar` | Studio drafts: Save and Revert |
| `PageTitle`, `SectionTitle`, `Hint` | Text styles. Do not set font sizes elsewhere |

Run `cd ui && cargo run --release` to open the gallery (`-- shell` opens the sample page in the Settings shell, `-- check` builds every window without a display). The gallery which shows every component in every state. Check it by eye after changing the kit.

## Templates

- `templates/settings_shell.slint`: the Settings window. Navigation on the left, one page on the right.
- `templates/studio_tab.slint`: one tab of the studio's right-hand pane.
- `pages/sample.slint`: a finished page to copy. It uses `Page`, `Section`, `SettingRow` and controls only.

## Wording

- Labels are short nouns or noun phrases: "Volume", not "Set the volume".
- Hints are one sentence that says what changes, ending with a full stop.
- Buttons are verbs: "Add to desktop", "Export".
- Use the same word for the same thing everywhere: character, instance, pack.

## Adding a page

1. Copy `pages/sample.slint` and rename the component.
2. Describe each setting as a `SettingRow` with a control from the kit.
3. Register the page in the registry (plan step 4) with its section and item IDs. Keep setting IDs stable.
4. Add it to the navigation entries.
5. Run `cargo test` and open the gallery if you touched the kit.

## Checklist

- [ ] Only kit components and tokens; no `std-widgets.slint`.
- [ ] No raw `px` sizes or `#` colours.
- [ ] Setting IDs are new or unchanged.
- [ ] Labels and hints follow the wording rules.
- [ ] Works at 100%, 150% and 200% display scale.
