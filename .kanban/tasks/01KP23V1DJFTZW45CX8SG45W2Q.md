---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffff9b80
title: Hide progress field row in inspector when total is 0
---
## What

When a task has 0 subtask checkboxes (i.e. `parse-body-progress` yields `{ total: 0, completed: 0, percent: 0 }`), the `EntityInspector` still renders the `progress` field row: a `HelpCircle` ("?") icon with tooltip, flex gap, and an empty content div — wasting vertical space. The underlying displays (`ProgressDisplay` in `kanban-app/ui/src/components/fields/displays/progress-display.tsx`, `ProgressRingDisplay` in `kanban-app/ui/src/components/fields/displays/progress-ring-display.tsx`) already return `null` when `total === 0`, but the outer `FieldRow` wrapper in `kanban-app/ui/src/components/entity-inspector.tsx` renders regardless.

Two secondary facts:
- The "?" icon is the `HelpCircle` fallback on line ~330 of `entity-inspector.tsx`: `const Icon = field.icon ? (fieldIcon(field) ?? HelpCircle) : null`. `bar-chart` (from `swissarmyhammer-kanban/builtin/definitions/progress.yaml`) maps to lucide `BarChart`, which may be unresolved in the installed lucide version — hence the fallback.
- Keyboard navigation uses `navigableFields` + `claimPredicates` indexed by flat position, so simply returning `null` from `FieldRow` would leave dangling predicates. Fields must be filtered *before* computing predicates.

### Approach — metadata-driven, reuses the display registry

Add an optional `isEmpty(value)` predicate to the display registry so each display owns its own notion of emptiness (no hardcoded field names in React). The inspector consults `isEmpty` before rendering a row AND uses it to filter `navigableFields` so keyboard nav stays coherent.

Only hide when the field is **non-editable** (`editor: none`) — editable fields with empty values must still be visible so the user can click to edit. Progress is computed (editor `none`), so it hides safely; text/markdown fields do not.

### Files to modify

1. `kanban-app/ui/src/components/fields/field.tsx`
   - Change `displayRegistry` to store `{ component, isEmpty? }` tuples.
   - Extend `registerDisplay(name, component, options?)` signature with `options.isEmpty?: (value: unknown) => boolean`.
   - Export a helper `getDisplayIsEmpty(name: string): ((v: unknown) => boolean) | undefined` for consumers (inspector).
   - Backwards compatibility: existing `registerDisplay(name, component)` calls keep working (options is optional).

2. `kanban-app/ui/src/components/fields/registrations/progress.tsx`
   - Register with `isEmpty: (v) => isProgressEmpty(v)` where `isProgressEmpty` checks that `v` is a non-null object whose `total` number is `0` (or non-numeric / missing).

3. `kanban-app/ui/src/components/fields/registrations/progress-ring.tsx`
   - Same `isEmpty` predicate — shape accepts both `{ total, completed }` and `{ total, done }`.
   - Extract the predicate to a small shared helper (e.g. `fields/displays/progress-empty.ts`) rather than duplicating.

4. `kanban-app/ui/src/components/entity-inspector.tsx`
   - Add a new `useVisibleFields(entity, fields)` helper that filters out fields where `resolveEditor(field) === "none"` AND `getDisplayIsEmpty(field.display)?.(entity.fields[field.name]) === true`.
   - Pipe the visible list into `useFieldSections`, `navigableFields`, and `claimPredicates` so keyboard nav indexes match.
   - Do NOT touch `entity-card.tsx` — cards are out of scope per user report ("this appears to be hiding properly on cards").

### Non-goals

- Do not change `HelpCircle` fallback behaviour or lucide icon mapping — hiding the row makes the fallback moot here.
- Do not fix the card. The user explicitly excluded it.
- Do not attempt a CSS-only `:empty` / `:has` solution — `FocusScope` has side-effects (focus registration, nav claims) that must be skipped, not just hidden.

## Acceptance Criteria

- [x] Opening the inspector for a task whose body has no `- [ ]` / `- [x]` checkboxes renders NO element matching `[data-testid="field-row-progress"]` — no icon, no tooltip, no empty flex row.
- [x] Opening the inspector for a task with at least one checkbox renders `[data-testid="field-row-progress"]` with a visible `role="progressbar"` and the correct percent.
- [x] Opening the inspector for a `board` or `column` entity whose `percent_complete` has `total: 0` hides that row the same way.
- [x] Arrow-key navigation (ArrowDown/ArrowUp) in an inspector that has a hidden progress row skips directly between the fields on either side — no "stuck" state, no invisible stop.
- [x] Editable fields with empty values (e.g. a `title` field bound to empty string) still render their row and remain clickable to edit. Hiding logic only applies to `editor: "none"` fields.
- [x] `registerDisplay(name, component)` calls without options continue to work unchanged — no regression in any other field type.

## Tests

- [x] `kanban-app/ui/src/components/entity-inspector.test.tsx`: add a test group "hides empty computed fields" that
  - [x] Asserts `field-row-progress` is absent when the task's `progress` field value is `{ total: 0, completed: 0, percent: 0 }`.
  - [x] Asserts `field-row-progress` is present when the task's `progress` value is `{ total: 4, completed: 2, percent: 50 }`.
  - [x] Asserts ArrowDown from `title` lands on the field AFTER `progress` (e.g. `body`) when progress is hidden, and on `progress` when it is visible.
- [x] `kanban-app/ui/src/components/fields/displays/progress-ring-display.test.tsx`: add an `isProgressEmpty` test (or in the new shared helper file) covering `{ total: 0 }`, `{ total: 4 }`, `null`, `42`, `{}`.
- [x] `kanban-app/ui/src/components/fields/field.test.tsx` (create if absent, or add to the nearest existing registry test) — verify `registerDisplay` accepts the new `options.isEmpty` argument and `getDisplayIsEmpty` returns it.
- [x] Run: `cd kanban-app/ui && pnpm test -- entity-inspector progress-ring-display field` → all pass, no snapshot drift.
- [x] Run the existing full UI test suite `cd kanban-app/ui && pnpm test` → green, 0 regressions.

## Workflow

- Use `/tdd` — write the inspector visibility test first (RED), then the registry extension and `isEmpty` predicates, then the inspector filter (GREEN), then refactor to extract the shared `isProgressEmpty` helper.

## Notes (implementation)

- Added shared helper `kanban-app/ui/src/components/fields/displays/progress-empty.ts` exporting `isProgressEmpty(value)` + sibling test file covering `{ total: 0 }`, board/task shapes, `null`, `undefined`, non-object, empty object, non-numeric `total`.
- Extended `kanban-app/ui/src/components/fields/field.tsx`:
  - `displayRegistry` now stores `DisplayRegistration = { component, isEmpty? }`.
  - `registerDisplay(name, component, options?)` — third arg is optional, back-compatible.
  - Exported `getDisplayIsEmpty(name)` and types `DisplayRegistration`, `RegisterDisplayOptions`.
- Updated both progress registrations to pass `{ isEmpty: isProgressEmpty }`.
- Added `useVisibleFields(entity, fields)` hook in `entity-inspector.tsx` that filters non-editable fields whose display reports empty; sections / navigable fields / claim predicates all pipe through it.
- Pre-existing `editor-save.test.tsx` failures on this branch (about `due`, `scheduled`, `status_date` date fields) are unrelated — verified by stashing my changes and reproducing the failures on the baseline.

### Test results

- Full UI suite: 935 passing, 16 pre-existing unrelated failures (all in `editor-save.test.tsx` from sibling date-field work, verified by stash).
- Targeted files (`progress-empty` + `field.test` + `entity-inspector` + `progress-ring-display` + `progress-ring-integration` + `entity-card`): 54/54 pass.
- `pnpm exec tsc --noEmit`: clean.
