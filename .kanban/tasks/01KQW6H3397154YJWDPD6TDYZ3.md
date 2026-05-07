---
assignees:
- wballard
depends_on:
- 01KQW6FSJ0PT783KHTNRBP6XR3
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffae80
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
- `swissarmyhammer-focus/src/divergence.rs` — DELETE (or rename to single-path harness) #stateless-nav

## Review Findings (2026-05-07 18:55)

Cutover deletions are complete and correct: all four Tauri commands gone, all three `check_*_divergence` helpers gone, `divergence` module gone from `lib.rs` and re-exports, the four entries gone from `tauri::generate_handler!`, the three actions (`registerScope` / `unregisterScope` / `updateRect`) gone from `SpatialFocusActions`, the IPC `useEffect` gone from `<FocusScope>`, the placeholder hooks gone from `<ColumnView>`. Diff is net-negative as expected (~1700 lines deleted across 11 files). `cargo check --workspace --all-targets` and `cargo clippy -p swissarmyhammer-focus` are clean. The `LARGE_COORD_BOUND` `#[cfg(debug_assertions)]` gate is correct (constant is only referenced inside a debug-only block in `validate_rect_invariants`; deletion would be wrong because it's still used in debug builds). `installRegistryHook` is correctly test-only — the global is a module-private `let` initialised to `null`, only `kanban-app/ui/src/test/setup.ts` installs it, production never touches it. The `Option<NavSnapshot>` semantics on `spatial_focus` and `spatial_navigate` are reasonable and documented (silent drop on the transient unmount window where the React-side registry has torn down). The `spatial_focus_lost` command takes a non-optional `NavSnapshot` matching the new contract.

The findings below are all stale documentation referencing deleted IPC commands / hooks / observers, plus one weak-test concern on the rewritten soak suite. The cutover itself is sound; the doc rot is the cleanup the cutover left behind.

### Warnings

- [x] `swissarmyhammer-focus/tests/spatial_nav_soak.rs:212-273, 412-432, 480-498, 559-750` — Most of the rewritten soak scenarios discard their results with `let _ = state.focus_with_snapshot(...)` / `let _ = state.navigate_with_snapshot(...)` / `let _ = state.focus_lost(...)` and only verify "doesn't panic". Scenario 2's doc claims it verifies the kernel "produces a focus event for the first focus and walk[s] ancestors without panicking" but the test never asserts an event is produced or that ancestors got walked. Scenarios 5 (inspector), 6 (modal), and every `run_scenario_*` helper in the cross-cutting test do the same. Only scenario 4 (filter-hide) and scenario 7 (bulk-delete) actually assert on outcomes. Recommend strengthening the discarded-result tests to assert at minimum `event.is_some()` where a transition is expected, or update the doc to honestly describe the test as a "no-panic survival suite" rather than a regression suite for behavioral correctness.

### Nits

- [x] `kanban-app/src/commands.rs:2182` — `with_spatial`'s doc references the deleted `[spatial_unregister_scope]` command as the lock-ordering rationale. Rewrite to describe the lock-ordering invariant in terms of the surviving snapshot-path commands (or in abstract terms — "any operation that holds both locks") instead of the deleted command.
- [x] `kanban-app/src/state.rs:441` — The doc comment on `spatial_state` says "Mutated by every `spatial_focus`, `spatial_navigate`, and `spatial_unregister_scope` command." Drop the deleted command and (optionally) add `spatial_focus_lost` / `spatial_clear_focus` to keep the list current.
- [x] `kanban-app/ui/src/lib/spatial-focus-context.tsx:380` — Comment in the deletion-listener says "the cached rect is refreshed at mount (initial seed alongside `registerSpatialScope`)". `registerSpatialScope` is gone; the seed now happens alongside `LayerScopeRegistry.add`. Update the reference.
- [x] `kanban-app/ui/src/lib/spatial-focus-context.tsx:387` — Same listener comment says "the surviving `spatial_unregister_scope` path will still drive fallback off the kernel's stored rect". That path was the whole point of step 11 to delete; nothing surviving drives fallback this way. Either drop the sentence or rewrite it to describe the new behavior (the IPC is just skipped — there is no fallback path).
- [x] `kanban-app/ui/src/lib/layer-scope-registry-context.tsx:1-50` — File header still describes the registry as "Step 1 of the spatial-nav redesign... stands the React-side registry up *alongside* the existing kernel sync... Both sources of truth coexist for now... the registry is purely additive in step 1" and lists "Removing the kernel sync (steps 10-12)" as out of scope. The kernel sync IS removed (step 11 = this task). Rewrite to describe the registry as the sole scope-tracking authority, drop the dual-source / step-numbered framing, and remove the "Out of scope" section that no longer makes sense post-cutover. Per doc-comment rules, also drop the references to card and step numbers.
- [x] `swissarmyhammer-focus/src/registry.rs:79-89` — `register_scope` doc enumerates "Virtualizer placeholder → real-mount swap" referring to a `usePlaceholderRegistration` hook in `column-view.tsx` and a `spatial_register_batch` mechanism — both deleted. Either drop the bullet or rewrite to describe the surviving same-shape re-register paths (placeholder-real swap is no longer one of them).
- [x] `swissarmyhammer-focus/src/registry.rs:673` — `update_rect` doc says "Called from the React side via `spatial_update_rect` when ResizeObserver fires." Both the Tauri command and the React-side ResizeObserver are deleted. The kernel-internal `update_rect` is now called only from inside the kernel (and from kernel tests). Either rewrite the doc to describe the surviving callers, or — given step 12 removes this method entirely — leave a one-line note that this is kernel-internal and slated for removal.
- [x] `swissarmyhammer-focus/src/registry.rs:1153` — `remove_layer` doc says "the React side unmounts those scopes first via `spatial_unregister_scope`". The IPC is gone; the React side unmounts via `LayerScopeRegistry.delete` and the kernel never sees the unmount. Update or drop.
- [x] `swissarmyhammer-focus/src/registry.rs:1248` — `apply_batch` doc points at the deleted `spatial_register_batch` adapter and the deleted virtualizer placeholder logic in `column-view.tsx`. The kernel `apply_batch` method itself is preserved per task scope (step 12 will remove it), but the React-side / Tauri-side justification is gone. Rewrite to describe `apply_batch` as a pure kernel utility, or leave a one-line note that it is slated for removal in step 12.
- [x] `swissarmyhammer-focus/src/registry.rs:270-273` — `validate_rect_invariants` doc says "Update fires from `ResizeObserver` and the ancestor-scroll listener, both of which run only after layout". Both are deleted. The validator's per-op handling is still useful for kernel-internal callers but the React-mechanism rationale is stale. Rewrite to describe what `update_rect` callers look like now (kernel-internal only) or drop the rationale.
- [x] `swissarmyhammer-focus/tests/focus_lost.rs:230-231, 276-277, 309-310, 328-330` — Test doc comments and the `coexistence_*_emits_once` test names frame the tests as guarding the dual-path coexistence between `spatial_focus_lost` and `spatial_unregister_scope`. The IPCs no longer coexist — the tests still exercise useful kernel idempotence between `state.focus_lost` and `state.handle_unregister`, but the framing is stale. Rename the tests (e.g. `focus_lost_idempotent_with_handle_unregister`) and rewrite the docs to describe the kernel-level invariant, dropping references to the deleted IPCs.
- [x] `kanban-app/ui/src/components/column-view.virtualized-nav.browser.test.tsx:216` — The `flushSetup` helper doc says only "Two-tick microtask flush so register effects settle." The two-tick choice is load-bearing — it's there because the dynamic-import `LayerScopeRegistry` mirror in `test/setup.ts` resolves the `import("@tauri-apps/api/core")` asynchronously, so a single tick won't drain the mirror queue. The WHY exists at one specific call site (line 468, inside the skipped test) but not at the helper definition where it would actually help readers. Add a one-sentence rationale to the helper doc so the "two ticks" isn't magic.
- [x] `kanban-app/ui/src/components/column-view.virtualized-nav.browser.test.tsx:454-460` — The skipped test's rationale comment references "spatial-nav step 11" — point-in-time / step-ID reference. Per doc-comment rules, drop the step number; describe the architectural reason ("the kernel-side scope replica is gone, so the kernel-simulator only sees the React-side `LayerScopeRegistry` view of mounted scopes") which the comment already does well in the rest of the block.
- [x] `kanban-app/ui/src/lib/spatial-focus-context.test.tsx:149` — Regression test comment opens with "The IPC umbilical is cut in step 11 of the spatial-nav redesign". Drop the step ID; the rest of the comment ("React no longer tells the kernel about scopes; the kernel sees scope state only via per-decision snapshots. The actions bag must therefore not surface entry points to the deleted IPCs.") is exactly the right level of context.