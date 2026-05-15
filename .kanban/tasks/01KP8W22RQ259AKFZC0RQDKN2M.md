---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffd180
title: Add display iconOverride to eliminate duplicate icons on status_date
---
## What

The `status_date` field renders **two icons**: the static `target` icon from YAML (rendered by `FieldRow`/`CardFieldIcon` in the tooltip position) and the kind-specific icon (CheckCircle, AlertTriangle, Play, Clock, PlusCircle) rendered inside `StatusDateDisplay`. The kind-specific icons are more informative — they should replace the static field icon in the tooltip position, and the display should stop rendering its own icon.

**Approach: `iconOverride` on `DisplayRegistration`**

Extend the existing `DisplayRegistration` interface (which already has `isEmpty`) with a new optional `iconOverride: (value: unknown) => LucideIcon | null` function. When a display registers an `iconOverride`, the parent layout (`FieldRow` in the inspector, `CardFieldIcon` on the card) calls it with the current value. If it returns a LucideIcon, that replaces the static YAML icon. If it returns null, the static icon is used as fallback. This is metadata-driven and general-purpose — any display can provide value-dependent icons.

### Files to modify

1. **`kanban-app/ui/src/components/fields/field.tsx`** — Add `iconOverride` to `DisplayRegistration` and `RegisterDisplayOptions`. Export `getDisplayIconOverride(displayName: string): ((value: unknown) => LucideIcon | null) | undefined` alongside existing `getDisplayIsEmpty`.

2. **`kanban-app/ui/src/components/entity-inspector.tsx`** — In `FieldRow`, import `getDisplayIconOverride`. After resolving the static icon, check if the display has an override: `const overrideIcon = getDisplayIconOverride(field.display ?? "")`. If it exists, call `overrideIcon(entity.fields[field.name])` to get a dynamic icon. Use it in place of the static icon in `FieldIconTooltip`. The `useFieldValue` subscription is in the `Field` child, so read the value from `entity.fields[field.name]` directly (FieldRow already receives the entity).

3. **`kanban-app/ui/src/components/entity-card.tsx`** — Same pattern in `CardFieldIcon` / `CardField`: call the display's `iconOverride` with the current value and use the returned icon instead of the static one.

4. **`kanban-app/ui/src/components/fields/displays/status-date-display.tsx`** — Export a `statusDateIconOverride(value: unknown): LucideIcon | null` function that uses `parseStatusDateValue` + `KIND_DESCRIPTORS` to return the kind's icon. Remove the `<Icon>` JSX from both compact and full modes — the display renders only the text phrase.

5. **`kanban-app/ui/src/components/fields/registrations/status-date.tsx`** — Pass `iconOverride: statusDateIconOverride` in the `registerDisplay` options.

## Acceptance Criteria

- [x] Inspector shows CheckCircle/AlertTriangle/Play/Clock/PlusCircle in the tooltip icon position (not the static `target` icon) based on the status_date's `kind`
- [x] Card shows the same kind-specific icon in the tooltip position
- [x] Only one icon renders per status_date field (no duplicate)
- [x] Tooltip still shows the field description on hover
- [x] Fields without an `iconOverride` registration continue using their static YAML icon (no regression)
- [x] The `iconOverride` API is general-purpose — not status_date-specific — any display can register one

## Tests

- [x] **`kanban-app/ui/src/components/fields/field.test.tsx`** — Add tests for `getDisplayIconOverride`: returns undefined for unregistered displays, returns the function when registered, returns undefined when registered without an override
- [x] **`kanban-app/ui/src/components/fields/displays/status-date-display.test.tsx`** — Update existing tests: the display no longer renders SVG icons inside itself (remove `svg` assertions from kind tests), add tests for the exported `statusDateIconOverride` function returning correct LucideIcon per kind
- [x] **`kanban-app/ui/src/components/entity-inspector.test.tsx`** — Verify that when a display has `iconOverride`, the dynamic icon appears in the FieldRow tooltip rather than the static one
- [x] Run `cd kanban-app/ui && npx vitest run` — all tests pass

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass. #field