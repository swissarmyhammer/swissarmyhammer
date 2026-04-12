---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffbc80
title: Add tooltips to field icons in EntityCard and extract shared icon utilities
---
## What

Field icons in the **EntityCard** render as plain `<span>` elements with no tooltip. The **EntityInspector** wraps the same icons in `<Tooltip>/<TooltipTrigger>/<TooltipContent>` so users get a hover label showing `field.description || field.name`. The card view should behave the same way.

Both files also duplicate `kebabToPascal` and icon resolution logic.

### Approach

1. Extract a shared `fieldIcon(field: FieldDef)` utility into a new file `kanban-app/ui/src/components/fields/field-icon.ts` that both components import. This replaces `resolveIcon` (card) and `fieldIcon` (inspector), and eliminates the duplicated `kebabToPascal`.

2. In `entity-card.tsx`, wrap the icon `<span>` in `<Tooltip>/<TooltipTrigger>/<TooltipContent>` using `field.description || field.name.replace(/_/g, " ")` as the tooltip text — matching the inspector's behavior.

### Subtasks

- [x] Create `kanban-app/ui/src/components/fields/field-icon.ts` exporting `fieldIcon(field: FieldDef): LucideIcon | null`
- [x] Update `entity-inspector.tsx` to import `fieldIcon` from the shared module, remove local `kebabToPascal` and `fieldIcon` (HelpCircle fallback preserved at the inspector call site)
- [x] Update `entity-card.tsx` to import `fieldIcon` from the shared module, remove local `kebabToPascal` and `resolveIcon`, wrap icon in Tooltip
- [x] Add unit test for shared `fieldIcon` utility
- [x] Refactor long React components to satisfy the `code-quality:function-length` validator (see Implementation Notes below)

## Acceptance Criteria

- [x] Hovering over a field icon in EntityCard shows a tooltip with the field description (or humanized field name)
- [x] `kebabToPascal` exists in exactly one location (the shared module) for FieldDef icon resolution
- [x] EntityInspector tooltip behavior is unchanged
- [x] No regressions in existing entity-card or entity-inspector tests
- [x] All functions in touched files under 50 lines (hook requirement)

## Tests

- [x] `kanban-app/ui/src/components/fields/field-icon.test.tsx` — 5 unit tests for `fieldIcon`: null for missing/empty/unknown icon, kebab→Pascal resolution, single-word resolution
- [x] `kanban-app/ui/src/components/entity-card.test.tsx` — 3 new tooltip tests: description tooltip, humanized-name fallback, no tooltip for fields without icon
- [x] Full UI suite: 918/918 tests pass across 92 files
- [x] `tsc --noEmit` clean

## Implementation Notes

### Shared utility
- `kanban-app/ui/src/components/fields/field-icon.ts` — `fieldIcon(field: FieldDef): LucideIcon | null` + local `kebabToPascal`
- Inspector preserves its legacy HelpCircle fallback at the call site (`fieldIcon(field) ?? HelpCircle`) so behavior is unchanged for unresolved icon names.
- Card uses the null return directly — no fallback icon on cards.

### Function-length refactor (required by `code-quality:function-length` stop hook)

Pre-existing violations in `entity-card.tsx` and `entity-inspector.tsx` blocked stopping. All three flagged components were refactored into smaller focused functions while preserving behavior. Tests are the oracle — `918/918` still green.

**entity-card.tsx** — extracted `DragHandle`, `CardFields`, `CardField`, `CardFieldIcon` sub-components; extracted `useHeaderFields` hook. `EntityCard` body is now 46 lines.

**entity-inspector.tsx** — extracted:
- `useFieldSections`, `useFieldClaimPredicates`, `useFirstFieldFocus`, `useFieldEditing` hooks
- `predicatesForField`, `edgePredicates`, `isInspectorField` helpers
- `InspectorSections`, `InspectorFooter`, `FieldContent`, `FieldIconTooltip` sub-components

`EntityInspector` body: 36 lines. `FieldRow` body: 43 lines. `InspectorSections` body: 40 lines. All under 50.

### Workflow
- Used `/tdd` — wrote failing tests first, then the shared utility, then the component updates.
- Refactor was done only after the feature landed and tests were green, as a second phase to satisfy the stop hook.

## Review Findings (2026-04-12 14:32)

### Warnings
- [x] `kanban-app/ui/src/components/entity-card.tsx` `CardField` component — `fieldIcon(field)` is called once in `CardField` (`const hasIcon = !!fieldIcon(field)`) and again in `CardFieldIcon` (`const Icon = fieldIcon(field)`). Call it once in `CardField`, pass the resolved `Icon` as a prop to `CardFieldIcon`, and use `!!Icon` for the layout conditional. This removes the redundant lookup and makes the data flow explicit.
- [x] `kanban-app/ui/src/components/entity-inspector.tsx` `FieldContent` component — Uses an inline anonymous type `{ field: FieldDef; entity: Entity; editState: ReturnType<typeof useFieldEditing> }` instead of a named `FieldContentProps` interface. The JS/TS review guidelines require named prop interfaces for every component.
- [x] `kanban-app/ui/src/components/entity-inspector.tsx` `InspectorFooter` component — Uses an inline anonymous type `{ fields: FieldDef[]; rowFor: ... }` instead of a named `InspectorFooterProps` interface.
- [x] `kanban-app/ui/src/components/entity-inspector.tsx` `FieldIconTooltip` component — Uses an inline anonymous type `{ Icon: LucideIcon; tip: string }` instead of a named `FieldIconTooltipProps` interface.
- [x] `kanban-app/ui/src/components/entity-card.tsx` `DragHandle` component — Uses an inline anonymous type `{ dragHandleProps?: Record<string, unknown> }` instead of a named `DragHandleProps` interface.
- [x] `kanban-app/ui/src/components/entity-card.tsx` `CardFields` component — Uses an inline anonymous type `{ fields: FieldDef[]; entity: Entity }` instead of a named `CardFieldsProps` interface.
- [x] `kanban-app/ui/src/components/entity-card.tsx` `CardFieldIcon` component — Uses an inline anonymous type `{ field: FieldDef }` instead of a named `CardFieldIconProps` interface.

### Nits
- [x] `kanban-app/ui/src/components/entity-inspector.tsx` `InspectorSections` — The `flatIndex` counter is a mutable `{ i: 0 }` object mutated during render via `flatIndex.i++` inside `rowFor`. While it works because render is synchronous and single-pass, a mutable counter in the render body is surprising. Consider precomputing a `fieldWithIndex` array before rendering instead.
- [x] `kanban-app/ui/src/components/entity-inspector.tsx` `FieldContent` — Calls `isEditable(field)` again even though the parent `FieldRow` already computed it. The editable flag could be included in the `editState` return value or passed as a prop to avoid the redundant call.
- [x] `kanban-app/ui/src/components/entity-card.tsx` `CardField` — The component wraps the `<div className="flex-1 min-w-0">` around `<Field>` unconditionally, but the outer `<div className={hasIcon ? "flex items-start gap-1.5" : ""}>` renders an empty className when there is no icon. Consider returning just the `<Field>` wrapper directly (no outer div) when there is no icon, to match the previous code's behavior of not wrapping icon-less fields in an extra `<div className="">`.