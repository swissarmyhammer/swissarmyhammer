---
assignees:
- claude-code
position_column: todo
position_ordinal: 7f80
title: 'Fix perspective.next/prev: derive view_kind from UIState instead of requiring it as arg'
---
## What

`perspective.next` and `perspective.prev` commands fail silently when triggered via keybinding (vim `gt`/`gT`, CUA `Mod+]`/`Mod+[`) or command palette because neither dispatch path passes the required `view_kind` arg. The backend's `cycle_perspective` function at `swissarmyhammer-kanban/src/commands/perspective_commands.rs:489` calls `ctx.require_arg_str("view_kind")` which errors out, and the result goes to `.catch(console.error)` in the frontend.

The keybinding handler at `kanban-app/ui/src/components/app-shell.tsx:47` dispatches commands by ID only — there's no mechanism to inject per-command args. The command palette (`command-palette.tsx:222`) also dispatches with just `target`, no `args`.

### Fix — derive view_kind from the scope chain

The scope chain already contains `view:{viewId}` (set by `ViewContainer` at `view-container.tsx:50`). The command should extract the view ID from the scope chain, look up the `ViewDef` via `KanbanContext.views()` → `ViewsContext::get_by_id(id)`, and read its `kind` field.

**`swissarmyhammer-kanban/src/commands/perspective_commands.rs` — `cycle_perspective` function (~line 479-525)**
- Replace `ctx.require_arg_str("view_kind")` with: try `ctx.arg_str("view_kind")` first (explicit arg), then fall back to deriving from scope chain:
  1. Find `view:{id}` moniker in `ctx.scope_chain` (same pattern as `window_label_from_scope` in `swissarmyhammer-commands/src/context.rs:71`)
  2. Get `KanbanContext` via `ctx.require_extension::<KanbanContext>()`
  3. Look up view via `kanban.views()` → `ViewsContext::get_by_id(view_id)` → read `.kind`
  4. Fall back to `"board"` if no view moniker found

**`swissarmyhammer-commands/builtin/commands/perspective.yaml` — lines 118-134**
- Remove `view_kind` from `params` on both `perspective.next` and `perspective.prev` (it's now derived)

## Acceptance Criteria
- [ ] `gt` / `gT` (vim) and `Mod+]` / `Mod+[` (CUA) cycle between perspectives for the current view kind
- [ ] The UI updates (tab bar reflects new active perspective, view content changes)
- [ ] Passing `view_kind` explicitly still works (backwards compatible for programmatic callers)

## Tests
- [ ] Add test: `test_next_perspective_derives_view_kind_from_scope_chain` — put `view:{board-view-id}` in scope chain, call `NextPerspectiveCmd.execute` without `view_kind` arg, assert perspective switches
- [ ] Add test: `test_next_perspective_explicit_view_kind_overrides_scope` — provide both scope chain view moniker AND explicit `view_kind` arg, assert explicit arg wins
- [ ] Run `cargo nextest run -p swissarmyhammer-kanban perspective` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#bug