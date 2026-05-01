---
assignees:
- claude-code
depends_on:
- 01KP8W22RQ259AKFZC0RQDKN2M
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffd480
title: Add display tooltipOverride to allow value-dependent tooltip text
---
## What

After the `iconOverride` work (depends on 01KP8W22RQ259AKFZC0RQDKN2M), the status_date tooltip still shows the static YAML description ("Smart status date — the most salient date given the task's current state"). With a dynamic icon indicating state, the tooltip should match — e.g. "Completed 3 days ago" or "Overdue by 5 days".

**Approach: `tooltipOverride` on `DisplayRegistration`**

Same pattern as `iconOverride`: add an optional `tooltipOverride: (value: unknown) => string | null` to `DisplayRegistration` and `RegisterDisplayOptions`. When registered and returning a non-null string, the parent layout (`FieldIconTooltip` in inspector, `CardFieldIcon` on card) uses it as the tooltip text instead of `field.description || fieldLabel(field)`. Null falls back to the static text.

### Files to modify

1. **`kanban-app/ui/src/components/fields/field.tsx`** — Add `tooltipOverride` to `DisplayRegistration` and `RegisterDisplayOptions`. Export `getDisplayTooltipOverride(displayName: string): ((value: unknown) => string | null) | undefined` alongside `getDisplayIconOverride` and `getDisplayIsEmpty`.

2. **`kanban-app/ui/src/components/entity-inspector.tsx`** — In `FieldRow`, import `getDisplayTooltipOverride`. After resolving the static `tip`, check for an override: `const overrideTip = getDisplayTooltipOverride(field.display ?? "")`. If it exists and returns a non-null string for the current value, pass that to `FieldIconTooltip` instead of the static tip.

3. **`kanban-app/ui/src/components/entity-card.tsx`** — Same pattern in `CardFieldIcon` / `CardField`: call `tooltipOverride` with the current value and use the returned string as the tooltip.

4. **`kanban-app/ui/src/components/fields/displays/status-date-display.tsx`** — Export a `statusDateTooltipOverride(value: unknown): string | null` function that returns the composed status phrase (reuse `composeStatusPhrase` — may need to extract it or make it callable from the override). Returns null for invalid values.

5. **`kanban-app/ui/src/components/fields/registrations/status-date.tsx`** — Pass `tooltipOverride: statusDateTooltipOverride` in the `registerDisplay` options.

## Acceptance Criteria

- [x] Inspector tooltip on the status_date icon shows "Completed 3 days ago" / "Overdue by 5 days" / etc. instead of the static YAML description
- [x] Card tooltip shows the same dynamic text
- [x] Fields without a `tooltipOverride` continue showing `field.description || fieldLabel(field)` (no regression)
- [x] The `tooltipOverride` API is general-purpose — any display can register one

## Tests

- [x] **`kanban-app/ui/src/components/fields/field.test.tsx`** — Add tests for `getDisplayTooltipOverride`: returns undefined for unregistered displays, returns the function when registered, returns undefined when registered without an override
- [x] **`kanban-app/ui/src/components/fields/displays/status-date-display.test.tsx`** — Add tests for the exported `statusDateTooltipOverride` function: returns correct phrase per kind, returns null for invalid values
- [x] **`kanban-app/ui/src/components/entity-inspector.test.tsx`** — Verify that when a display has `tooltipOverride`, the dynamic text appears in the tooltip
- [x] Run `cd kanban-app/ui && npx vitest run` — all tests pass

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass. #field