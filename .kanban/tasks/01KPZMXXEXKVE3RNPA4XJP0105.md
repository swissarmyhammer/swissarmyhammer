---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffff9680
title: Replace perspective.goto / view.switch indirection with direct .set commands
---
## What

PR #40 review comment from @wballard on `kanban-app/src/commands.rs:1320`:

> seems like trash, why not just make the correct perspective.set command to begin with?

Ref: https://github.com/swissarmyhammer/swissarmyhammer/pull/40#discussion_r3137358696

Currently `match_dynamic_prefix` in `kanban-app/src/commands.rs` rewrites:

- `view.switch:{id}` → `view.set` with `view_id: {id}`
- `perspective.goto:{id}` → `perspective.set` with `perspective_id: {id}`

The rewrite layer exists because the **dynamic palette entries** are emitted as `view.switch:{id}` and `perspective.goto:{id}` in `swissarmyhammer-kanban/src/scope_commands.rs` (`emit_view_switch`, `emit_perspective_goto`), then translated back into the canonical `*.set` commands.

Reviewer's critique: drop the indirection. Have the palette emit dynamic entries that dispatch `view.set` / `perspective.set` directly with the right args pre-filled. No rewrite needed.

### Files to modify

- `swissarmyhammer-kanban/src/scope_commands.rs` — `emit_view_switch` / `emit_perspective_goto` emit entries whose `cmd` is the real `view.set` / `perspective.set` and whose `args` carry the resolved `{view_id, ...}` / `{perspective_id, ...}`.
- `kanban-app/src/commands.rs::match_dynamic_prefix` — delete the two rewrite branches; `board.switch:*` stays (different — it resolves a file path into `file.switchBoard`).
- Any test exercising `view.switch:*` / `perspective.goto:*` as command ids.

## Acceptance Criteria

- [x] `rg 'view\.switch:|perspective\.goto:'` across the codebase returns only palette-display plumbing, with no live command-dispatch sites; or zero hits if the dynamic emitter now produces the canonical id directly.
- [x] `match_dynamic_prefix` no longer rewrites `view.switch:*` or `perspective.goto:*`; the function body keeps only the `board.switch:*` path (and any non-moved rewrites).
- [x] Clicking a view icon in the left-nav and invoking "Switch to X" from the palette both still trigger `view.set` with the right `view_id` in args.
- [x] Clicking a perspective tab and invoking "Go to X" from the palette both still trigger `perspective.set` with the right `perspective_id`.
- [x] `cargo test -p swissarmyhammer-kanban -p kanban-app` passes.

## Tests

- [x] Update existing `emit_view_switch` / `emit_perspective_goto` tests in `swissarmyhammer-kanban/src/scope_commands.rs` to assert the emitted entries carry `cmd: "view.set"` / `cmd: "perspective.set"` with pre-filled args, not the old `view.switch:*` / `perspective.goto:*` ids.
- [x] Delete or update the `match_dynamic_prefix_view_switch_rewrites_to_view_set` / `match_dynamic_prefix_perspective_goto_rewrites_to_perspective_set` tests in `kanban-app/src/commands.rs` — the rewrite branch is gone, so the test's premise goes away.
- [x] Frontend: `kanban-app/ui/src/lib/views-context.tsx` and `perspective-context.tsx` already dispatch `view.set` / `perspective.set`; verify their tests still pass unchanged.

## Workflow

Use `/tdd`. Start by flipping the palette-emission test to assert the canonical command id, watch it fail, update `emit_view_switch` / `emit_perspective_goto` to produce it, then drop the now-dead rewrite branches. #commands #refactor #architecture

## Implementation Notes

Done. Summary of the change:

1. `ResolvedCommand` gained an `args: Option<serde_json::Value>` field. Cross-emitter dedup (`SeenKey`) now keys on `(id, target, args)`; `dedupe_by_id` (the final pass) now keys on `(id, args)` so fan-out rows that share an id but differ by args stay distinct instead of collapsing to one entry.
2. `emit_view_switch` now emits rows with `id == "view.set"` and `args == { "view_id": view.id }`. `emit_perspective_goto` mirrors that with `perspective.set` + `perspective_id`.
3. `match_dynamic_prefix` lost its `view.switch:*` and `perspective.goto:*` branches — only `board.switch:*` and `entity.add:*` remain.
4. Frontend `CommandPalette` forwards `cmd.args` through the dispatch path; `left-nav.tsx` now dispatches `view.set` with `{ view_id }` directly; `views-container.tsx` keeps the `view.switch:{id}` id as a client-side scope-map key (for React / dedup uniqueness) but the execute handler dispatches `view.set` with args.
5. Updated tests: `view_switch_commands_emit_canonical_view_set_with_args` and `perspective_goto_commands_emit_canonical_perspective_set_with_args` assert the new wire format; `match_dynamic_prefix_no_longer_rewrites_view_switch_or_perspective_goto` is the regression guard.

Full cargo + vitest + clippy + tsc all clean.

## Review Findings (2026-04-24 09:26)

### Nits
- [x] `kanban-app/ui/src/lib/context-menu.ts:14-22` — The `args` field's doc comment on `ResolvedCommand` claims "the field is forwarded here too so the context-menu → dispatch pipeline stays symmetric with the palette path", but the actual `ContextMenuItem` shape (both TS line 27-33 and Rust `kanban-app/src/commands.rs:2036`) has no `args` field, and the loop at lines 63-80 does not propagate it. All current fan-out rows are `context_menu: false` so this never fires in production today, but the comment misleads and a future fan-out row opted into the context menu would silently drop its `args`. Fix: either (a) correct the comment to state args is intentionally dropped because all current fan-out rows are palette-only, or (b) actually add `args` to `ContextMenuItem` on both sides and forward it through `show_context_menu` so the symmetry claim becomes true.

Resolved via **Option A** (2026-04-24): corrected the doc comment on `ResolvedCommand.args` in `kanban-app/ui/src/lib/context-menu.ts` to accurately state that the field is intentionally dropped in the context-menu path because all current fan-out rows are `context_menu: false` (palette-only). Added a companion comment below `ContextMenuItem` pointing back at the explanation, and documented the concrete steps needed if a future fan-out row opts into the context menu (add `args` to `ContextMenuItem` on both TS and Rust sides, forward through `show_context_menu`). Option B (making the symmetry real) was considered but not warranted: no current or planned fan-out row needs the context-menu surface, and the updated comment acts as the breadcrumb for whoever adds that need later. Verified via `pnpm tsc --noEmit` (clean) and `pnpm test --run` (1322/1322 tests pass).