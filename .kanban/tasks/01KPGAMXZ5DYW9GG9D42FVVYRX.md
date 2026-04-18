---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffe380
title: Decide on board-view "New Task" keybinding (was Mod+N pre-entity.add)
---
## What

Surfaced as a nit in the review of 01KPENGNDX2526DZCRYT9N1E9P. The retired `task.add` command carried `keys: { cua: Mod+N, vim: a }` and `board.yaml`'s retired `board.newCard` carried the same `Mod+N`. The new unified `entity.add` declares no keys and `emit_entity_add` in `swissarmyhammer-kanban/src/scope_commands.rs` builds `ResolvedCommand { keys: None }` for every dynamic `entity.add:{type}`. As a result, neither `Mod+N` nor `vim a` now creates a task on the board view.

`Mod+N` itself is now bound to `file.newBoard` in `kanban-app/ui/src/components/app-shell.tsx`. The column `+` button and the palette still work as entry points on the board view. Grid views are unaffected because `tasks-grid.yaml` / `tags-grid.yaml` / `projects-grid.yaml` declare their own `grid.newBelow` (vim `o`, cua `Mod+Enter`) and `grid.newAbove` keybindings that the frontend wires to `entity.add:{entityType}`.

### Decision needed

Either:

1. **Add a per-view command in `board.yaml`** (e.g. `board.newCard`) that the frontend dispatches as `entity.add:task`, with whatever keys are appropriate (`vim a`, or a non-conflicting cua key). This restores the previously-working keystroke.

2. **Accept the column `+` button and palette as the sole entry points on the board view.** Drop the keystroke for good and document that grid views are the keystroke-driven creation surface.

Both are defensible. Flagged because the entity.add unification dropped a previously-working keystroke without a replacement.

## Acceptance Criteria

- [x] Decision documented in this task as a comment.
- If option 1, the binding is implemented and a test covers it.
- [x] If option 2, no code change — close as "decided".

## Tests

- If implementing option 1: a frontend or Rust test that verifies the binding dispatches `entity.add:task` from board scope.

#commands #ux

## Decision (2026-04-18)

**Option 2 (with refinement) — accept current state. No code change.**

### Why

The framing in the original task overstates the regression. Empirical state of the board view today:

1. **The board view already has a working "New Task" keystroke**: `kanban-app/ui/src/components/board-view.tsx` defines an inline `board.newTask` `CommandDef` via `makeNewTaskCommand` with `keys: { vim: "o", cua: "Mod+Enter" }`. This command:
   - Resolves the focused column from `focusedMonikerRef` (column moniker, or task moniker → its home column).
   - Dispatches `entity.add:task` with the resolved `column` override (no override when nothing is focused, so `AddEntity` falls back to the lowest-order column).
   - Refocuses the new task on success and surfaces a toast on failure.
   - Is registered via the board's `CommandScopeProvider`, so `extractScopeBindings` in `kanban-app/ui/src/lib/keybindings.ts` picks up the per-mode keys at runtime.

2. **The board's keystroke is symmetric with the grid's**: `grid.newBelow` is also `vim: o` / `cua: Mod+Enter`. A user who learns one transfers to the other.

3. **`Mod+N` is now `file.newBoard` and that is the right call**: "New Board" is the strongest claim on `Mod+N` in a board-management app — it parallels "New File" in a CUA editor. Reclaiming `Mod+N` for "New Task" would re-create the conflict that the entity.add unification removed.

4. **`vim a` is the only key actually lost.** Adding it would be cosmetic — vim users already have `o` for the same action on this view. Vim's `a` (append at cursor) is also not a particularly natural fit for "create a new task in the focused column"; `o` (open new line below) maps better.

5. **Adding `board.newCard` to `board.yaml` would be infrastructure for nothing.** `ViewDef.commands` is parsed in `swissarmyhammer-views/src/types.rs` but the frontend does not consume it — no React file in `kanban-app/ui/src` reads `view.commands`. The grid views' YAML `commands:` blocks are similarly inert metadata; what actually drives `grid.newBelow` is the inline `buildGridEditCommands` in `grid-view.tsx`, which by convention uses matching ids. Wiring a YAML→runtime path just to have somewhere to declare `board.newCard` would introduce a parallel command-source plumbing for one binding that is already covered by the existing inline `board.newTask`.

6. **The column `+` button and `app.palette` (`Mod+Shift+P`) → "New Task" remain working entry points** for users who don't memorise the keystroke.

### What is documented going forward

- Board-view "New Task" keystroke: `o` (vim) / `Mod+Enter` (cua), defined in `makeNewTaskCommand` in `kanban-app/ui/src/components/board-view.tsx`.
- `Mod+N` is global `file.newBoard`.
- The legacy `task.add` `Mod+N` / `vim a` keys are not restored.

### What this card does NOT promise

It does not commit to keeping `ViewDef.commands` permanently inert. If a future card decides to make YAML the source-of-truth for view commands (so the palette, help, and any future tooling can introspect them without parsing TSX), `board.newTask` is a natural first migration target. That is a separate, larger refactor — out of scope here.

## Resolution

Closed as decided per option 2 of the original task. No code change. Symmetric "New Task" keystroke is already in place via the inline `board.newTask` command.