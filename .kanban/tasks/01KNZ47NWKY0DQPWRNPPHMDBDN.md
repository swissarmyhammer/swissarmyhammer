---
assignees:
- wballard
depends_on:
- 01KNZ44E91F4NYAGZX13H0FDAJ
position_column: todo
position_ordinal: bd80
project: pill-via-cm6
title: Migrate BadgeDisplay (scalar refs) to MentionView; drop dead select branch
---
## What

Rewrite `BadgeDisplay` for scalar reference fields (currently the only real consumer: `position_column`) to render via `MentionView` in single mode. Delete the `resolveSelectBadge` branch — it's dead code; no shipping field definition has `field.type.options`.

**Files to modify:**
- `kanban-app/ui/src/components/fields/displays/badge-display.tsx`:
  - Remove the entire `resolveSelectBadge` function and the `SelectOption` / options-based branch
  - Remove `resolveReferenceBadge` (the lookup it does now lives inside `MentionView`)
  - New implementation: if `field.type.entity` is set and `value` is a string id, render `<MentionView entityType={field.type.entity} id={value} />`
  - Empty-state: when `value` is empty, show the existing `-` dash
  - If `field.type.entity` is unset (shouldn't happen for any current field but is a defensive guard), render the raw value as a fallback plain span — or throw a dev-mode error (pick whichever fits the project's conventions)

**Files to verify:**
- `kanban-app/ui/src/components/fields/registrations/select.tsx` — still registers `BadgeDisplay` under the `badge` display key. No change needed to the registration wiring.

**Dead-code cleanup in this card:**
- Any imports that were only used by `resolveSelectBadge` (e.g. `SelectOption` type, `FieldDef` if it was only for options)
- The `BadgeResolution` intermediate type if no longer needed

## Acceptance Criteria
- [ ] `BadgeDisplay` renders `position_column` using `<MentionView>` in single mode
- [ ] `resolveSelectBadge` and related helpers are deleted
- [ ] Column pill shows the column's display name (via the CM6 widget) with its color
- [ ] Empty value renders as a `-` dash (unchanged)
- [ ] No behavioral regression in `position_column` rendering (same text, same color, same click behavior)

## Tests
- [ ] Update `kanban-app/ui/src/components/fields/displays/badge-display.test.tsx` — existing tests that use `resolveReferenceBadge` path get DOM assertion updates (widget inside CM6 contentDOM)
- [ ] Delete any existing tests that exercised the `resolveSelectBadge` path — it's dead code
- [ ] Add a test: render `BadgeDisplay` for a `position_column` field with a valid column id, assert the rendered DOM contains the column's name
- [ ] Add a test: render with an empty value, assert the dash is shown
- [ ] Run: `bun test badge-display` — all pass
- [ ] Smoke: `bun run dev`, confirm the column indicator still appears on task cards with the right color and name

## Workflow
- Use `/tdd` — update the test file first (delete dead-branch tests, add new single-mode test). Watch them fail. Then gut the implementation and rebuild it as a thin `MentionView` wrapper.
