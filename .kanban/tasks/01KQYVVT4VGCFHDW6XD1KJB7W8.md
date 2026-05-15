---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffa280
project: spatial-nav
title: rename showFocusBar → showFocus and audit all current showFocusBar={false} suppressions
---
## Why

The `showFocusBar` prop on `<FocusScope>` (and the same name plumbed through `<Field>`, `<MentionView>`, etc.) is a misnomer — it controls whether the dashed-border focus indicator paints, not a "bar". Rename to `showFocus`. While we're touching every callsite, audit each `={false}` suppression and either flip it or document the reason inline.

## Rename

- `kanban-app/ui/src/components/focus-scope.tsx` — prop name on the public component, the internal `SpatialFocusScopeBody`, the type definition, the default value (`true`), the conditional that controls `<FocusIndicator>` mounting, all docstrings.
- `kanban-app/ui/src/components/fields/field.tsx` — prop on `<Field>`, default (`false`), propagation to `<FocusScope>`, all docstrings.
- `kanban-app/ui/src/components/mention-view.tsx` — `MentionView` and `MentionViewCompact` prop, propagation to `<FocusScope>`, docstrings.
- All callsites that pass `showFocusBar` (or `showFocusBar={true}` / `showFocusBar={false}` / shorthand `showFocusBar`):
  - `entity-inspector.tsx`
  - `entity-card.tsx`
  - `column-view.tsx`
  - `board-selector.tsx`
  - `nav-bar.tsx`
  - `board-view.tsx`
  - `perspective-tab-bar.tsx`
  - `perspective-container.tsx`
  - `left-nav.tsx`
  - `grid-view.tsx`
- All test files referencing `showFocusBar` in code or comments (~20 files, see `grep -rn 'showFocusBar' kanban-app/ui` for the live list).

The rename is mechanical — sed-replace + tsc + `pnpm -C kanban-app/ui test`.

## Audit (each existing `={false}` site)

For each remaining `showFocus={false}` (post-rename), either keep with a one-line `// Why: ...` comment justifying suppression, or flip to default. Sites:

| File | Scope | Current rationale | Action |
|---|---|---|---|
| `nav-bar.tsx:67` | `ui:navbar` zone | viewport-spanning chrome | likely keep — confirm |
| `nav-bar.tsx:82` | `ui:navbar.board-selector` zone | container, not user-targeted | likely keep — confirm |
| `perspective-tab-bar.tsx:317` | `ui:perspective-bar` (bar container) | viewport-spanning | likely keep — confirm |
| `grid-view.tsx:946` | grid zone | viewport-sized | likely keep — confirm |
| `left-nav.tsx:54` | `ui:left-nav` | viewport-spanning | confirm — should the left-nav itself glow when focused, or only its inner items? |
| `board-view.tsx:1085` | board zone | board fills viewport | likely keep — confirm |
| `perspective-container.tsx:169` | perspective container | viewport-sized | likely keep — confirm |
| `fields/field.tsx:459` | `<Field>` default | callers opt in per surface | keep as default, document |

For each "keep", add a one-line comment `// showFocus=false: <reason>`. For any "flip", just remove the prop and accept the default.

## Tests

- `pnpm tsc --noEmit` clean after rename
- `pnpm -C kanban-app/ui test` green — every test mentioning `showFocusBar` in assertions or comments updated
- No production callsite still references `showFocusBar`
- No test asserts behavior that changes (the rename is semantics-preserving)

## Acceptance criteria

- Zero `showFocusBar` references in `kanban-app/ui` (production + tests)
- Every `showFocus={false}` site has a one-line justification comment OR has been removed
- All tests green
- Visual behavior unchanged for any site we kept; flipped sites paint the indicator

## Out of scope

- Refactoring the `<FocusIndicator>` component itself
- Adding new focus surfaces beyond what the audit identifies as "should-have"

## Files (search live, don't trust this list — `grep -rn 'showFocusBar' kanban-app/ui`)

Production: `focus-scope.tsx`, `fields/field.tsx`, `mention-view.tsx`, `entity-inspector.tsx`, `entity-card.tsx`, `column-view.tsx`, `board-selector.tsx`, `nav-bar.tsx`, `board-view.tsx`, `perspective-tab-bar.tsx`, `perspective-container.tsx`, `left-nav.tsx`, `grid-view.tsx`

Tests: ~20 files, all under `kanban-app/ui/src/components/`