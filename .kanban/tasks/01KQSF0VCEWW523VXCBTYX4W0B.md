---
assignees:
- claude-code
depends_on:
- 01KQQTXDHP3XBHZ8G40AC4FG4D
position_column: todo
position_ordinal: '7e80'
project: spatial-nav
title: nav.left from column collapses focus to engine root after geometric pick landed
---
## What

After component one of the spatial-nav redesign (`01KQQTXDHP3XBHZ8G40AC4FG4D`) landed and replaced `cardinal_cascade` with `geometric_pick`, the `target=None` / focus-collapse-to-engine-root class of bugs is supposed to be fixed. But it still reproduces for `nav.left` from a board column.

### Reproduce

```
2026-05-04 07:16:40 — cmd=ui.setFocus result={"ScopeChain":["column:review", "ui:board", "board:board", "view:01JM…", "ui:perspective", "perspective:01KN…", "perspective:01KN…", "board:board", "store:…", "mode:normal", "window:…", "engine"]}
                       (focus is on column:review, full chain present — kernel knows about it)

2026-05-04 07:16:45 — cmd=nav.left target=None
                       (the nav.left command fires with no target — RED FLAG)

2026-05-04 07:16:45 — cmd=ui.setFocus scope_chain=[] target=None
                       result: scope_chain=Some(["engine"])
                       (focus collapses to engine root)
```

### Why this is wrong

`geometric_pick` (verified in `swissarmyhammer-focus/src/navigate.rs:345`) honours the no-silent-dropout contract: it always returns a `FullyQualifiedMoniker`, never `None`. When the half-plane in direction D is empty, it returns the focused FQM (stay-put). The React glue should detect "result === focusedFq" and treat it as no-op — focus stays where it is.

The observed `target=None` and the focus-collapse-to-engine indicate one of:

1. **The kernel IS returning a real FQM**, but the React glue (`buildNavCommands` in `kanban-app/ui/src/components/app-shell.tsx`) is mishandling the result and dispatching `ui.setFocus(target=None)` instead of preserving the result.
2. **The IPC adapter drops the result** between Rust and React (e.g., wraps it in `Option` and serializes `Some(focused_fq)` as `None` when the result equals the input).
3. **A code path other than `geometric_pick` is firing** for `nav.left` — e.g., the override path (`check_override`) returns `Some(None)` for `column:review`'s `Direction::Left` override, and the React glue maps that to `target=None` instead of stay-put. Worth checking whether any column zone has an explicit `navOverride: { left: null }` wall.
4. **`ui.setFocus(target=None)` is the correct stay-put encoding**, but the kernel's `ui.setFocus` handler treats `target=None` as "clear focus" and emits `scope_chain=["engine"]`. That's a contract violation — `target=None` from a stay-put nav should be a no-op, not a clear.

Most likely #4 — the user-observable result (chain becomes `["engine"]`) matches "ui.setFocus(None) cleared focus to root."

### Files to read first

- `swissarmyhammer-focus/src/navigate.rs::geometric_pick` (line 345) and `BeamNavStrategy::next` (line 224). Confirm the return value when half-plane is empty (should be `focused_fq`).
- `kanban-app/ui/src/components/app-shell.tsx::buildNavCommands` — the four cardinal command builders. Trace what they do with the kernel's result.
- `kanban-app/ui/src/lib/spatial-focus-context.tsx` — IPC adapter for `spatial_navigate`. Check whether the result type is `Option<FullyQualifiedMoniker>` or `FullyQualifiedMoniker`.
- `swissarmyhammer-kanban/src/commands/ui_commands.rs` (and adjacent) — the Rust-side `ui.setFocus` handler. Check what `target=None` does. If it clears, the contract is broken.
- `kanban-app/src/commands.rs` — the Tauri bridge for `spatial_navigate`. Check the result type.

### Fix shape

Once root cause is pinned, the fix is one of:

- React glue: if the kernel result equals `focusedFq`, do NOT dispatch `ui.setFocus` at all (treat as no-op). The current `buildNavCommands` likely calls `setFocus(result)` unconditionally; needs an equality check.
- IPC adapter: preserve the FQM through serialization. If it's currently `Option<FQM>` to encode "no motion", change to always-`FQM` per the kernel's no-silent-dropout contract.
- `ui.setFocus` handler: accept `target=None` as no-op (preserve current focus), not as "clear to root." Or remove the ability to dispatch with `target=None` from the nav path.

### Files to modify (likely)

- `kanban-app/ui/src/components/app-shell.tsx` — `buildNavCommands`.
- `kanban-app/ui/src/lib/spatial-focus-context.tsx` — IPC adapter.
- `kanban-app/src/commands.rs` and/or `swissarmyhammer-kanban/src/commands/ui_commands.rs` — `ui.setFocus` handler semantics.

## Acceptance Criteria

- [ ] Pressing `ArrowLeft` from `column:review` (or any column at the visual left edge of the board) does NOT collapse focus to engine root. Focus either stays on `column:review` (true visual edge, stay-put) OR moves to a visibly-leftward target (LeftNav, perspective bar's filter editor, etc.).
- [ ] The `nav.left` command never dispatches `ui.setFocus` with `target=None` AND `scope_chain=[]`. Either it doesn't dispatch (no-op) or it dispatches with the real FQM.
- [ ] No regression in any other cardinal nav case — pressing arrow keys from any focused scope still works as the geometric model promises.
- [ ] `cargo test -p swissarmyhammer-focus` passes (the kernel side, if changes are needed there).
- [ ] `pnpm -C kanban-app/ui test` passes.

## Tests

- [ ] **End-to-end regression** in `kanban-app/ui/src/spatial-nav-end-to-end.spatial.test.tsx` (or a new file `nav-stay-put-on-edge.spatial.test.tsx`): mount the production-shaped harness, drive focus to a column at the left edge of the board, fire `keydown ArrowLeft`, assert focus does NOT change to engine root and the `ui.setFocus` IPC was either (a) not called OR (b) called with a real `target` FQM (not `None`).
- [ ] **Negative regression** in `kanban-app/ui/src/components/app-shell.test.tsx`: with a mocked kernel returning the focused FQM unchanged from `spatial_navigate` (stay-put encoding), assert the React `nav.left` command does NOT dispatch `ui.setFocus(target=None, scope_chain=[])`.
- [ ] **Kernel test** in `swissarmyhammer-focus/src/navigate.rs::tests`: confirm `geometric_pick` returns the focused FQM (not `None`, not a different FQM) when no candidate exists in the half-plane.
- [ ] If the `ui.setFocus` Rust handler is the culprit, add a unit test there pinning that `target=None` is not interpreted as "clear focus."
- [ ] Run `cargo test -p swissarmyhammer-focus && pnpm -C kanban-app/ui test app-shell spatial-nav-end-to-end` and confirm green.

## Workflow

- Use `/tdd`. Start with the negative-regression test in `app-shell.test.tsx` (RED — it reproduces the user's bug). Trace the path from `keydown` through `nav.left` execute → `actions.navigate` → IPC → kernel → result → `ui.setFocus`. Pin the root cause (one of the four candidates above). Fix at the appropriate layer. Confirm GREEN.
