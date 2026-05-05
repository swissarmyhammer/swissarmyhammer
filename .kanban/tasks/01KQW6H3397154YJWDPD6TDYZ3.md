---
assignees:
- wballard
depends_on:
- 01KQW6FSJ0PT783KHTNRBP6XR3
position_column: todo
position_ordinal: da80
project: spatial-nav
title: 'spatial-nav redesign step 11: cutover (2/4) — delete spatial_register_scope, spatial_unregister_scope, spatial_update_rect IPCs'
---
## Parent

Implementation step for **01KQTC1VNQM9KC90S65P7QX9N1**. Second of four cutover steps.

## Goal

Cut the IPC umbilical between React's per-scope mount/unmount and the Rust kernel's scope replica. After this step, React no longer tells the kernel about scopes; the kernel only sees scope state via per-decision snapshots.

## What to delete

### Tauri commands

In `kanban-app/src/commands.rs`:

- Delete the `spatial_register_scope` command and its `_inner` helper
- Delete the `spatial_unregister_scope` command and its `_inner` helper at line 2394
- Delete the `spatial_update_rect` command and its `_inner` helper
- Remove from the `tauri::generate_handler!` macro list
- Update `kanban-app/src/lib.rs` if these are re-exported

### Frontend actions

In `kanban-app/ui/src/lib/spatial-focus-context.tsx`:

- Delete `registerScope` from `SpatialFocusActions`
- Delete `unregisterScope` from `SpatialFocusActions`
- Delete `updateRect` from `SpatialFocusActions`
- Remove their implementations (lines 382–431 area)
- Update the type definition for `SpatialFocusActions`

### `<FocusScope>` registration effect

In `kanban-app/ui/src/components/focus-scope.tsx`:

- Delete the entire useEffect block that calls `registerSpatialScope` / `unregisterScope` (lines 345–403)
- The component now only registers in `LayerScopeRegistry` (the useEffect added in step 1)

After this step, `<FocusScope>` does NOT touch IPC at all on mount/unmount. The only IPC paths it interacts with are click → `focus(fq, snapshot)` and the `useFocusClaim` subscription (a separate concern, unchanged).

## What still works

- Nav, click focus, focus restoration: all running on snapshot path
- All tests from steps 6–9 stay green; the dual-source diagnostic in step 9 had its registry-path branch made redundant — now there's no second path to compare to

### Diagnostic cleanup

The `compare_paths` harness from step 9 has nothing left to compare. Either:

(a) Delete it (recommended — it was a transition aid)
(b) Keep it as `assert_no_divergence_between(snapshot_path, ???)` — but there's no other path. So really, delete.

Step 9's soak tests stay; they now run only the snapshot path and still cover the production scenarios.

## Tests

- All e2e nav, focus, focus-lost, layer-pop tests still pass
- New regression: assert that `spatial_register_scope` etc. are not in the Tauri command surface (compile-time via removed handler entries; runtime sanity test that calling them from JS errors with "command not found")
- `cargo build` produces no `unused_*` warnings related to the removed paths

## Out of scope

- Removing `SpatialRegistry::scopes` itself (step 12)

## Acceptance criteria

- The three commands no longer exist in Rust
- The three actions no longer exist in TS
- `<FocusScope>` mount/unmount does not touch IPC
- All tests green
- The original symptom (overlap warning during drag) cannot fire from the rect-update path because the path is gone

## Files

- `kanban-app/src/commands.rs` — delete commands
- `kanban-app/src/lib.rs` — adjust handler list / re-exports
- `kanban-app/ui/src/lib/spatial-focus-context.tsx` — delete actions
- `kanban-app/ui/src/components/focus-scope.tsx` — delete IPC useEffect
- `swissarmyhammer-focus/src/divergence.rs` — DELETE (or rename to single-path harness) #01KQTC1VNQM9KC90S65P7QX9N1