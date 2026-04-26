---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffbc80
title: Fix inspect from grid cell — cell moniker field suffix causes entity not found
---
## What

Double-clicking or inspecting from a grid cell dispatches `ui.inspect` with a cell moniker like `tag:tag-1.color`. The Rust `parse_moniker` (context.rs:154) splits on first `:` and returns `(\"tag\", \"tag-1.color\")`. The entity lookup fails because the real id is `tag-1`, not `tag-1.color`.

The scope chain from a grid cell should include the entity moniker from the row-level FocusScope: `[tag:tag-1.color, tag:tag-1, ...]`. `first_inspectable` (ui_commands.rs:17) scans the chain and should find `tag:tag-1`. But the problem is:

1. The cell FocusScope's `FocusScopeInner` double-click handler dispatches inspect with `target: moniker` (the CELL moniker `tag:tag-1.color`), and `InspectCmd` checks `ctx.target` FIRST before falling back to scope chain.
2. So the scope chain fallback never fires — the target is set but points to a non-existent entity.

**Two possible fixes (pick one):**

**Option A — Backend: strip field suffix from moniker in `parse_moniker`.**
If the id contains a `.`, split and use only the part before the first `.` as the id. This makes `parse_moniker(\"tag:tag-1.color\")` return `(\"tag\", \"tag-1\")`. Risk: entity IDs that legitimately contain `.` would break.

**Option B — Frontend: cell FocusScope should not dispatch inspect with cell moniker.**
The cell FocusScope has `commands={[]}` and `showFocusBar={false}` — it's for navigation only, not entity interaction. Its double-click should not fire inspect. The `EntityRow`'s double-click should handle inspect with the correct entity moniker. But currently the cell FocusScope's `FocusScopeInner` calls `e.stopPropagation()` on double-click, preventing EntityRow from seeing it.

**Recommended: Option B.** When `renderContainer={false}` or `showFocusBar={false}`, the FocusScope should not intercept double-click for inspect. This matches the intent: navigation-only scopes don't do entity interaction.

**Files to modify:**
- `kanban-app/ui/src/components/focus-scope.tsx` — `FocusScopeInner` double-click handler: skip dispatch when `showFocusBar={false}` (or when commands are empty)

## Acceptance Criteria
- [ ] Double-click a grid cell opens the inspector for that row's entity (not \"entity not found\")
- [ ] Double-click a card in board view still works (FocusScope with showFocusBar=true)
- [ ] Cell navigation still works (claimWhen predicates unaffected)

## Tests
- [ ] `cd kanban-app/ui && pnpm vitest run src/components/focus-scope.test.tsx` — all pass
- [ ] Add test: FocusScope with showFocusBar=false does not dispatch inspect on double-click
- [ ] Manual: grid view — double-click a tag row → inspector opens for that tag