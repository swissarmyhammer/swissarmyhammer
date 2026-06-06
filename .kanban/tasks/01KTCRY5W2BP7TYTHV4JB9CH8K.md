---
assignees:
- claude-code
position_column: todo
position_ordinal: cb80
title: 'Bug: All views show the board icon (LayoutGrid / 4 squares) except the board view'
---
## What
Reported by user: every view in the left-nav shows the same "4 squares" icon (which makes no sense for non-board views) — only the board view shows a sensible icon.

The "4 squares" is lucide's `LayoutGrid`, which is the **fallback** icon in `viewIcon` (`apps/kanban-app/ui/src/components/left-nav.tsx:23–31`):
```
function viewIcon(view: ViewDef) {
  const name = view.icon ?? view.kind;        // left-nav.tsx:24
  if (name) {
    const key = kebabToPascal(name);          // e.g. "grid" -> "Grid"
    const Icon = icons[key as keyof typeof icons];
    if (Icon) return <Icon className="h-4 w-4" />;
  }
  return <LayoutGrid className="h-4 w-4" />;    // fallback — the 4 squares
}
```
So every non-board view falls through to `LayoutGrid` because `view.icon ?? view.kind` does not resolve to a valid lucide-react icon key after `kebabToPascal`. The board view only looks right by coincidence — either its `kind`/`icon` happens to map to a real lucide name, or its mapped name IS `LayoutGrid`.

Root-cause candidates:
1. **Backend doesn't populate `view.icon`** per view, so `viewIcon` falls back to `view.kind`, and the raw `kind` strings (e.g. `grid`, `table`, `board`) are not valid lucide PascalCase names (`Grid`, `Table`, `Board` are not all lucide icons — e.g. there is no `Board`; `Grid` is deprecated/renamed). Confirm what `ViewDef.icon` / `ViewDef.kind` actually contain at runtime (`apps/kanban-app/ui/src/types/kanban.ts`, and the backend view definitions).
2. **Missing kind→icon mapping**: there is no deliberate per-kind icon map; the code relies on `kind` strings accidentally matching lucide names. Likely needs an explicit `kind -> lucide name` map (board → Columns/LayoutGrid, grid/table → Table, etc.) and/or each built-in view YAML to declare a valid `icon`.

Reproduce: open a board with multiple view kinds; observe all left-nav view icons render as the 4-square LayoutGrid except board.

## Acceptance Criteria
- [ ] Each view kind renders a distinct, sensible icon (board, grid/table, and any other kinds differ).
- [ ] Icons resolve from an explicit, validated source (per-view `icon` and/or a kind→icon map) rather than accidental lucide name collisions.
- [ ] Root cause identified (missing backend `icon` vs. invalid `kind`→lucide assumption).

## Tests
- [ ] Unit test for `viewIcon` (extract if needed) asserting each known view kind maps to its expected (non-fallback) icon, and an unknown kind maps to the documented fallback.
- [ ] Test that built-in view definitions each declare an `icon` that resolves to a real lucide key (guard against silent fallback).
- [ ] Regression test failing before the fix, passing after.

## Workflow
- Use `/tdd` — failing test first, then fix. #bug