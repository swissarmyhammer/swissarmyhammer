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
So every non-board view falls through to `LayoutGrid` because `view.icon ?? view.kind` does not resolve to a valid lucide-react icon key after `kebabToPascal`. The board view only looks right by coincidence.

Root-cause candidates:
1. **Backend doesn't populate `view.icon`** per view, so `viewIcon` falls back to `view.kind`, and the raw `kind` strings (e.g. `grid`, `table`, `board`) are not valid lucide PascalCase names. Confirm what `ViewDef.icon` / `ViewDef.kind` contain at runtime (`apps/kanban-app/ui/src/types/kanban.ts`, and the backend view definitions).
2. **Missing kind→icon mapping** — the code relies on `kind` strings accidentally matching lucide names.

## Architecture steer (dedup review)
Resolve the icon from VIEW/SERVICE METADATA, not a UI-hardcoded `kind→icon` map. The per-view `icon` should be supplied by the views service / `ViewDef` (single source of truth); the UI's `viewIcon` stays a dumb lookup of that provided icon with one documented fallback. **Prefer fixing the data** — each built-in view definition declares a valid lucide `icon` — over adding a `kind→lucide` switch in `left-nav.tsx`. A hardcoded map in React is exactly the presentation/control logic we are removing from the UI (target: UI displays + routes only). If a kind-based default is unavoidable, it belongs in the view service metadata, not the component.

## Acceptance Criteria
- [ ] Each view kind renders a distinct, sensible icon (board, grid/table, and any other kinds differ).
- [ ] The icon is supplied by view/service metadata (each built-in ViewDef declares a valid lucide `icon`); `viewIcon` is a dumb lookup + single documented fallback, with NO hardcoded kind→icon map in the component.
- [ ] Root cause identified (missing service-provided `icon` vs. invalid `kind`→lucide assumption).

## Tests
- [ ] Unit test for `viewIcon` asserting a provided valid `icon` renders it, and an unknown/empty icon maps to the documented fallback.
- [ ] Test that every built-in view definition declares an `icon` that resolves to a real lucide key (guard against silent fallback) — at the service/metadata layer.
- [ ] Regression test failing before the fix, passing after.

## Workflow
- Use `/tdd` — failing test first, then fix. #bug