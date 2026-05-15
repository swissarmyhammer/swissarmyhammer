---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffff8780
title: 'Perspective commands: resolve `perspective_id` from scope/UIState with first-perspective fallback; enforce "always a selected perspective" invariant'
---
## What

Two linked defects reported in one session:

1. **`perspective.clearFilter`, `perspective.clearGroup`, `perspective.sort.set`, `perspective.sort.clear`, `perspective.sort.toggle` fail with `MissingArg("perspective_id")` when invoked from the palette or a keybinding.** They only read the id from `ctx.args` (`swissarmyhammer-kanban/src/commands/perspective_commands.rs:204-207, 228-231, 257-260, 286-289, 351-354, 401-404`), with no fallback. The same goes for `perspective.filter` and `perspective.group` when invoked outside a scoped context.

2. **There is no invariant guaranteeing an active perspective is selected**, so `UIState.active_perspective_id(window_label)` can be empty even when perspectives exist. The frontend renders a fallback via `perspectives.find(...) ?? perspectives[0]` (`kanban-app/ui/src/lib/perspective-context.tsx:235-241`) — but this is UI-only; the backend store is still empty, so scope-chain lookups and commands dispatched without args have nothing to fall back to.

**Confirmed available inputs** (traced end-to-end):
- `ctx.resolve_entity_id("perspective")` (`swissarmyhammer-commands/src/context.rs:140-142`) — returns the first `perspective:{id}` moniker in the scope chain, innermost-first. Populated when the user right-clicks a perspective tab (via `ScopedPerspectiveTab` wrapper in `perspective-tab-bar.tsx:276-294`).
- `UIState::active_perspective_id(window_label)` (`swissarmyhammer-commands/src/ui_state.rs:911-922`) — per-window active id, returns `""` when unset.
- `KanbanContext::perspective_context().await` + `pctx.all()` filtered by `view` — full perspective list for the current view kind. Same path used by `cycle_perspective` at `perspective_commands.rs:551-562` and `resolve_view_kind` at 513-522.
- `ui.set_active_perspective(window_label, id)` (`ui_state.rs:496`) — writer for the active id.

**What's already correct (don't touch)**:
- The frontend filter formula bar and group popover pass `perspective_id` explicitly (`filter-editor.tsx:223-239`); those paths work. Keep them — they're the authoritative signal when present.
- `useAutoCreateDefaultPerspective` (`perspective-context.tsx:133-152`) already handles the "no perspectives exist for this view kind" case by dispatching `perspective.save { name: "Default", view: kind }`. Don't duplicate that.

## Approach

### Backend — resolve-or-default helper

In `swissarmyhammer-kanban/src/commands/perspective_commands.rs`, add one shared async helper and use it from the seven commands that currently read `ctx.arg("perspective_id")` directly:

```rust
/// Resolve the perspective_id a mutation command should act on, trying
/// (in order): explicit `args.perspective_id`, a `perspective:{id}` moniker
/// in the scope chain, UIState's active perspective for the current window,
/// and finally the first perspective whose `view` matches the active view
/// kind. Returns `MissingArg("perspective_id")` only if every fallback fails
/// (no perspectives registered for the view — the caller is responsible for
/// surfacing a useful error).
async fn resolve_perspective_id(
    ctx: &CommandContext,
    kanban: &Arc<KanbanContext>,
) -> swissarmyhammer_commands::Result<String> {
    if let Some(id) = ctx.arg("perspective_id").and_then(|v| v.as_str()) {
        return Ok(id.to_string());
    }
    if let Some(id) = ctx.resolve_entity_id("perspective") {
        return Ok(id.to_string());
    }
    let window_label = ctx.window_label_from_scope().unwrap_or("main");
    if let Some(ui) = ctx.ui_state.as_ref() {
        let active = ui.active_perspective_id(window_label);
        if !active.is_empty() {
            return Ok(active);
        }
    }
    // Final fallback: first perspective for the active view kind.
    let view_kind = resolve_view_kind(ctx, kanban).await;
    let pctx = kanban
        .perspective_context()
        .await
        .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
    let pctx = pctx.read().await;
    pctx.all()
        .iter()
        .find(|p| p.view == view_kind)
        .map(|p| p.id.clone())
        .ok_or_else(|| CommandError::MissingArg("perspective_id".into()))
}
```

Replace the body of each `let perspective_id = ctx.arg(...).ok_or(MissingArg(...))?;` block at lines 204-207, 228-231, 257-260, 286-289, 351-354, 401-404 (and line 173-176 in `SetFilterCmd`) with `let perspective_id = resolve_perspective_id(ctx, &kanban).await?;`.

When the fallback kicks in (i.e. args/scope didn't carry the id), also persist the choice: right after resolving, if the result was NOT from `ctx.arg` or scope chain, call `ui.set_active_perspective(window_label, &perspective_id)`. This makes the fallback self-healing — after the first such command, subsequent palette invocations find a non-empty active id without re-running the lookup. Implement this by making `resolve_perspective_id` return `(String, ResolvedFrom)` so the caller knows whether to persist, or by calling `set_active_perspective` unconditionally (idempotent when already equal — see `ui_state.rs:500-503`).

### Frontend — enforce "always a selected perspective"

In `kanban-app/ui/src/lib/perspective-context.tsx`, add a sibling hook to `useAutoCreateDefaultPerspective` (line 133-152):

```tsx
/**
 * Keep UIState.active_perspective_id in sync with a real perspective for
 * the current view kind. If the stored id is empty or refers to a
 * perspective that no longer exists (deleted, or view-kind mismatch),
 * dispatch `ui.perspective.set` for the first matching perspective.
 */
function useAutoSelectActivePerspective(
  loaded: boolean,
  perspectives: PerspectiveDef[],
  active_perspective_id: string,
  viewKind: string,
  dispatch: (cmd: string, opts?: { args?: Record<string, unknown> }) => Promise<unknown>,
) {
  useEffect(() => {
    if (!loaded) return;
    const matching = perspectives.filter((p) => p.view === viewKind);
    if (matching.length === 0) return; // let useAutoCreateDefaultPerspective handle this
    const stillValid = matching.some((p) => p.id === active_perspective_id);
    if (stillValid) return;
    dispatch("ui.perspective.set", {
      args: { perspective_id: matching[0].id },
    }).catch(console.error);
  }, [loaded, perspectives, active_perspective_id, viewKind, dispatch]);
}
```

Wire it inside `PerspectiveProvider` between the existing `useAutoCreateDefaultPerspective` call and the return (around line 223-224). This runs after the auto-create hook has had a chance to add a "Default" — the chain is: no perspectives → create Default → list updates → select Default.

Keep the existing `activePerspective` memo at line 235-241 as a belt-and-suspenders fallback for the render that happens *during* the dispatch round-trip.

## Acceptance Criteria

- [ ] Invoking `perspective.clearFilter`, `perspective.clearGroup`, `perspective.sort.set`, `perspective.sort.clear`, `perspective.sort.toggle`, `perspective.filter`, or `perspective.group` from the command palette (no args, no scope chain perspective) succeeds and operates on the active perspective for the current window/view.
- [ ] Right-clicking a perspective tab and invoking one of those commands operates on that perspective (scope chain `perspective:{id}` takes precedence over UIState active).
- [ ] Passing an explicit `perspective_id` arg always takes precedence (frontend filter/group editors unchanged).
- [ ] After the backend resolves a perspective via the UIState / first-perspective fallback, `UIState.active_perspective_id(window_label)` is updated so subsequent commands find it set (idempotent `set_active_perspective` confirms this).
- [ ] On app boot with existing perspectives but empty `active_perspective_id`, the frontend dispatches `ui.perspective.set` for the first matching perspective within one render cycle. The perspective tab bar reflects the selection without a flicker of "no tab selected."
- [ ] When switching views, if the newly-active view's kind has no matching active perspective, one is auto-selected.
- [ ] When no perspectives exist for the view kind, `useAutoCreateDefaultPerspective` creates "Default" first, then `useAutoSelectActivePerspective` selects it on the next render.
- [ ] Returning `MissingArg("perspective_id")` from the resolver only happens when the active view kind has truly zero perspectives AND `useAutoCreateDefaultPerspective` has not yet completed — rare, transient, and recoverable.

## Tests

- [ ] New Rust test `resolve_perspective_id_prefers_explicit_arg` in `swissarmyhammer-kanban/src/commands/perspective_commands.rs` tests module:
  1. Build a `CommandContext` with `args.perspective_id = "arg-id"`, scope chain containing `perspective:scope-id`, UIState active = `ui-id`.
  2. Assert resolver returns `"arg-id"`.
- [ ] New Rust test `resolve_perspective_id_falls_back_to_scope_chain`:
  1. No arg, scope chain containing `perspective:scope-id`, UIState active = `ui-id`.
  2. Assert returns `"scope-id"`.
- [ ] New Rust test `resolve_perspective_id_falls_back_to_uistate`:
  1. No arg, scope chain without perspective, UIState active = `ui-id`.
  2. Assert returns `"ui-id"`.
- [ ] New Rust test `resolve_perspective_id_falls_back_to_first_for_view_kind`:
  1. No arg, no scope perspective, UIState active = `""`; perspective context has two perspectives — one for `board`, one for `grid`; active view kind = `board`.
  2. Assert returns the board perspective's id AND `UIState.active_perspective_id` is now equal to that id (self-healing check).
- [ ] New Rust test `clear_filter_works_from_palette_with_no_args`:
  1. Set up a perspective with a filter. Set it as UIState active.
  2. Create `CommandContext` with empty args and no scope perspective.
  3. Execute `ClearFilterCmd`. Assert filter is cleared on disk and no error.
- [ ] New browser/vitest test in `kanban-app/ui/src/lib/perspective-context.test.tsx` for `useAutoSelectActivePerspective`:
  1. Mount `PerspectiveProvider` with `loaded=true`, perspectives=`[{id:"p1", view:"board"}]`, UIState active_perspective_id=`""`, viewKind=`"board"`.
  2. Assert `ui.perspective.set` is dispatched with `perspective_id: "p1"` within one effect cycle.
  3. Also cover: when active id refers to a deleted perspective, auto-selects the first remaining; when active id references a different view kind, re-selects for current view.
- [ ] Update existing test `filter-editor.test.tsx` at lines 120/220/356 — still passes `perspective_id` explicitly and still works (no regression).
- [ ] Run: `cargo nextest run -p swissarmyhammer-kanban perspective_commands` and `cd kanban-app/ui && bun test perspective-context` — all passing.

## Workflow

- Use `/tdd`. Start by writing `clear_filter_works_from_palette_with_no_args` — the failing test reproduces the user's bug. Add the `resolve_perspective_id` helper and swap in seven call sites. Then write the resolver unit tests. Then add the frontend auto-select hook and its test.
- Do NOT remove the UI-level `perspectives[0]` memo fallback at `perspective-context.tsx:237-238`; that's the synchronous render path that covers the few ms while the new hook's dispatch is round-tripping.
- Do NOT modify commands whose id is already correctly sourced (e.g., the filter formula bar keeps passing `perspective_id` explicitly). #bug #perspectives #commands