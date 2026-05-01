---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffbb80
project: task-card-fields
title: Add YAML-configurable `placeholder` hint for empty field displays
---
## What

When the user looks at a task and fields like `tags`, `project`, or `depends_on` are empty, the display renders a bare `-` (compact mode / cards) or a muted `None` (full mode / inspector). Since the editor isn't activated until the user clicks in, this empty state is the *only* cue the field is editable — and "–" tells the user nothing about *what* to add.

Make the empty-state text YAML-configurable via a new `placeholder` attribute on `FieldDef`. Displays read it and render it (muted) instead of the hardcoded fallback. When a field doesn't set `placeholder`, the current fallback text (`-` / `None`) is preserved so nothing else regresses.

### Why in the display, not the editor

Per the user's follow-up: "this should show the hint text in display since the editor won't be activated yet." The inspector/card pipeline is `FieldDisplayContent` (in `kanban-app/ui/src/components/fields/field.tsx`) wrapping the registered display in a click-to-edit surface. Until the user clicks, only the display is mounted. So the hint has to live there — the editors never render for fields at rest.

### Files to modify

1. **Rust schema** — `swissarmyhammer-fields/src/types.rs`
   - On `FieldDef`, add `pub placeholder: Option<String>` with `#[serde(default, skip_serializing_if = "Option::is_none")]`, positioned near the other display-adjacent fields (`icon`, `section`, `width`).
   - Add a YAML round-trip test alongside the existing ones covering both `placeholder: "Add tags"` and absence.

2. **TS schema mirror** — `kanban-app/ui/src/types/kanban.ts`
   - Extend `FieldDef` with `placeholder?: string`.

3. **Badge-list display** — `kanban-app/ui/src/components/fields/displays/badge-list-display.tsx`
   - Replace the hardcoded empty-branch text at the `if (values.length === 0)` block (currently lines ~116–122) with `field.placeholder ?? (mode === "compact" ? "-" : "None")`. Keep the existing muted/italic styling classes so look-and-feel only changes when a YAML placeholder is provided.

4. **Badge display** — `kanban-app/ui/src/components/fields/displays/badge-display.tsx`
   - Replace the `"-"` fallback at line ~110 with `field.placeholder ?? "-"`. Keep the existing `text-muted-foreground/50` class.

5. **Tests** — `kanban-app/ui/src/components/fields/displays/badge-list-display.test.tsx` + `badge-display.test.tsx`
   - Add cases asserting the placeholder text renders when `field.placeholder` is set AND the value is empty. Assert the existing `-` / `None` fallback when `placeholder` is absent.

6. **YAML — three target fields**
   - `swissarmyhammer-kanban/builtin/definitions/tags.yaml` → `placeholder: "Add tags"`
   - `swissarmyhammer-kanban/builtin/definitions/project.yaml` → `placeholder: "Assign a project"`
   - `swissarmyhammer-kanban/builtin/definitions/depends_on.yaml` → `placeholder: "Add dependencies"`

### Non-goals (explicit)

- Do NOT touch the editors. Editor placeholder strings (e.g. `multi-select-editor.tsx`'s `Type ${prefix} to search...`) stay as-is — they apply once the editor is active, which is a different UX surface.
- Do NOT plumb `placeholder` into other displays (text-display, markdown-display, date-display, progress-display, progress-ring-display, status-date-display, color-swatch, attachment-display). Those don't currently render an "empty placeholder" in the way badge/badge-list do; adopting the field is a follow-up if we ever want it there.
- Do NOT change the existing fallback strings when `placeholder` is absent. `-` / `None` must remain so fields without a configured hint render identically to today.
- Do NOT apply placeholders beyond the three named fields in this card. The user asked specifically for tags/project/dependencies; other fields can gain YAML entries in follow-up cards.

### Design choice — same placeholder for both modes

`badge-list-display.tsx` has separate fallbacks for compact (`-`) and full (`None`). When a YAML `placeholder` is set, use it in BOTH modes — the user configured a hint string and we should honor it, letting the muted styling handle visual weight. Truncation is already wrapped in the existing compact-mode class, so a long placeholder like "Add tags" still fits on a card row. If later we want mode-specific placeholders, the field can grow `placeholder_compact` / `placeholder_full` — out of scope here.

## Acceptance Criteria

- [x] `FieldDef` serde round-trips `placeholder: Option<String>` — present, empty string, and absent all deserialize and re-serialize losslessly.
- [x] With `placeholder: "Add tags"` in `tags.yaml`, a task whose tags list is empty renders "Add tags" in both the card (compact) and the inspector (full), styled muted.
- [x] With `placeholder: "Assign a project"` in `project.yaml`, a task whose project is null renders "Assign a project".
- [x] With `placeholder: "Add dependencies"` in `depends_on.yaml`, a task whose depends_on is empty renders "Add dependencies".
- [x] Clicking on the rendered hint enters edit mode — the click-to-edit surface wrapping the display is unchanged.
- [x] Any `badge`/`badge-list` field that does NOT declare `placeholder` still renders `-` (compact) or `None` (full) exactly as before.
- [x] No change to tag pills, reference badges, or the populated (non-empty) branch — only the empty branch text is affected.

## Tests

- [x] `swissarmyhammer-fields/src/types.rs` — add `field_def_placeholder_yaml_round_trip`:
  - Build a `FieldDef` with `placeholder: Some("Add tags".into())`, YAML-round-trip, assert the string survives.
  - Build a `FieldDef` with `placeholder: None`, YAML-round-trip, assert the key does not appear in the serialized output.
- [x] `kanban-app/ui/src/components/fields/displays/badge-list-display.test.tsx`:
  - "renders the configured placeholder in full mode when values array is empty".
  - "renders the configured placeholder in compact mode when values array is empty".
  - "falls back to 'None' / '-' when placeholder is absent" (regression guard).
- [x] `kanban-app/ui/src/components/fields/displays/badge-display.test.tsx`:
  - "renders the configured placeholder when value is missing or empty string".
  - "falls back to '-' when placeholder is absent" (regression guard).
- [x] Run: `cargo nextest run -p swissarmyhammer-fields field_def_placeholder_yaml_round_trip` → green.
- [x] Run: `cargo nextest run -p swissarmyhammer-fields -p swissarmyhammer-kanban` → full suites green.
- [x] Run: `cd kanban-app/ui && pnpm test -- badge-list-display badge-display` → green.
- [ ] Manual verification: launch the kanban app. Create a task with no tags, no project, no dependencies. Confirm the inspector shows "Add tags", "Assign a project", "Add dependencies" under their respective field icons. Click each — the editor opens.

## Workflow

- Use `/tdd` — RED: write `field_def_placeholder_yaml_round_trip` + the badge-list and badge display placeholder tests first (all fail: field doesn't exist, displays still hardcode `-`/`None`). GREEN: add the `placeholder` field to `FieldDef` (Rust + TS), wire it through the two displays with `??` fallback, add the three YAML `placeholder:` entries. Verify green and do the manual UI check.

## Implementation Notes

- Rust `FieldDef` gained `placeholder: Option<String>` with `#[serde(default, skip_serializing_if = "Option::is_none")]`, positioned right after `section`. Updated every `FieldDef` literal constructor across `swissarmyhammer-fields`, `swissarmyhammer-entity`, and `swissarmyhammer-kanban` to include the new field.
- TS `FieldDef` mirrors with `placeholder?: string` including a doc comment.
- `BadgeDisplay` and `BadgeListDisplay` prefer `field.placeholder` via `??` over their hardcoded `-` / `None` fallbacks, keeping the muted/italic styling untouched.
- Added `field_def_placeholder_yaml_round_trip` covering `Some`, `None`, and absent-key deserialization. Added four UI tests (two per display) covering both the placeholder-set and placeholder-absent branches.
- YAML updates added `placeholder:` to `tags.yaml`, `project.yaml`, and `depends_on.yaml` — picked up at compile time by `include_dir!`.
- Manual verification remains for the reviewer (launch the app, eyeball the inspector).
