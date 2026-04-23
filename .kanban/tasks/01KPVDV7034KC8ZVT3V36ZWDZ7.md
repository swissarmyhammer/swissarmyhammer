---
assignees:
- claude-code
position_column: todo
position_ordinal: ff8280
title: Drag-to-attach trips spatial-nav `focus_first_in_layer` stale-RAF guard — focus lost after paste completes
---
## What

Dragging a file onto a task inspector attachment field still misbehaves after the previous drag-to-attach fix (`01KPTK87DSZTDK1H0C7FKAHQB8`) landed. The observable symptom is the spatial-nav focus-layer guard firing during the paste-complete sequence — the user reports seeing this code path:

```rust
// Only act on the active (topmost) layer. A call that names a
// non-active layer is an out-of-order RAF from a lower layer that
// was mounted before an inner one; honouring it would move focus
// *down* the stack.
match inner.layer_stack.active() {
    Some(active) if active.key == layer_key => {}
    _ => return None,
}
```

**That exact pattern no longer exists on the `navigation` branch HEAD** (`ed4918049` in the `/Users/wballard/github/swissarmyhammer/swissarmyhammer-navigation` worktree). It was replaced with a softer guard at `swissarmyhammer-spatial-nav/src/spatial_state.rs:574-590`:

```rust
pub fn focus_first_in_layer(&self, layer_key: &str) -> Option<FocusChanged> {
    let mut inner = self.inner.write().unwrap();
    if inner.layer_stack.has_layer_above(layer_key) {
        return None;
    }
    // ...
}
```

`has_layer_above` (same file, line 188-193) returns `false` when the named key isn't in the stack at all — it only bails when a strictly-higher layer exists. That's the correct semantics.

So either:
1. **The user's running build is stale** — built before the `has_layer_above` replacement landed. A clean rebuild from navigation HEAD would make the symptom quoted in the report disappear, but the drag-to-attach flow likely still fails because of cause #2.
2. **The softened guard still bails** during drag-to-attach because the attach flow is (legitimately) calling `focus_first_in_layer` for a layer that *is* strictly below an inner one at that moment — e.g., the inspector FocusLayer's RAF-deferred push fires after an inner FocusLayer (attachment chip's focus scope? quick-attach popover? field editor?) has already mounted. That's the exact "stale RAF" scenario the guard was designed to catch, and the guard's bail is correct — the bug is upstream, in whatever sequence schedules the RAF-push for the wrong layer at the wrong time.

**The task is to identify which case applies and fix it, end-to-end.**

## Approach

### Phase 1 — reproduce and pin down the runtime call path

1. Start from a clean build on the `navigation` branch worktree (`/Users/wballard/github/swissarmyhammer/swissarmyhammer-navigation`). `cargo build` the backend, `cd kanban-app/ui && bun run build` the frontend.
2. Open a task in the inspector. Drag a file from the Finder onto the attachment field.
3. Observe the result and capture the OS log:
   ```sh
   log show --predicate 'subsystem == "com.swissarmyhammer.kanban"' --last 5m --info --debug
   ```
4. The reference memory entry `./reference_oslog.md` covers where frontend `console.warn` lands. Add targeted `tracing::debug!` in `focus_first_in_layer` (entry + the `has_layer_above` bail branch + the "already focused in layer" branch + the success branch) logging `{layer_key, active_key, layer_stack_depth, has_above}` so the log names the exact call that bails.
5. Also log every `push_layer` and `remove_layer` call on the Tauri side (`kanban-app/src/spatial.rs:86-100`, `:1047+` area) with the layer `key` and `name` + caller identity (best-effort via `tracing::Span::current()` metadata). Correlate with the `focus_first_in_layer` calls.

### Phase 2 — based on what the trace shows

- **If the trace shows the bail comes from a layer that's been removed from the stack entirely**: this means the frontend is RAFing a `spatial_focus_first_in_layer` call for a layer it's already unmounted. Likely cause: `FocusLayer` effect cleanup order — the RAF is scheduled in the `useEffect` mount but not cancelled in the unmount cleanup. Fix in `kanban-app/ui/src/components/focus-layer.tsx` (exists on `navigation` branch — verify path): capture the RAF id in a ref, `cancelAnimationFrame(id)` in the cleanup. The `has_layer_above` guard actually doesn't catch this case (it returns false for "not in stack") — so the call goes through and focuses the wrong layer's top-left entry or silently does nothing. Either way the root cause is a lifecycle bug.

- **If the trace shows an inner layer is legitimately above the inspector layer at RAF time**: determine who pushed the inner layer and whether it SHOULD be there. Candidates to audit on `navigation`: `inspectors-container.tsx`, `entity-inspector.tsx`, `attachment-display.tsx`, any modal/popover components (quick-attach?) that wrap themselves in `FocusLayer`. Expect to find a layer that's being pushed transiently during the paste flow (e.g. an attachment preview or progress indicator) and either remove it (if not needed) or make its push/pop sync with the paste-complete event so the outer inspector layer's RAF never fires during the transient.

- **If the trace shows `focus_first_in_layer` never fires but a downstream caller treats `None` as an error**: search for `focus_first_in_layer` call sites in the frontend (grep `"spatial_focus_first_in_layer"` across `kanban-app/ui/src`) and audit each `.then()` / `await` chain. Returning `None` is a valid signal ("no focus change happened"); any code that throws on `None` is wrong. Fix: make the caller tolerate a null result without surfacing a toast.

### Phase 3 — add a regression test

Once the root cause is named, add a regression test at the appropriate layer:

- Rust, if the guard logic itself was wrong: new test in `swissarmyhammer-spatial-nav/src/spatial_state.rs` tests module named `focus_first_in_layer_does_not_bail_when_layer_key_not_in_stack` (or similar).
- Frontend, if the lifecycle/sequence was wrong: browser test `attachment-drop-preserves-focus.browser.test.tsx` that drags a file onto the attachment field, awaits the paste, and asserts focus stays on the inspector entity (not lost, not on a random entry).

## Acceptance Criteria

- [ ] After a clean build of `navigation` HEAD, dragging a file onto a task inspector's attachment field succeeds, the attachment appears in the list, and inspector focus is preserved (the focused entity moniker does not change to an unrelated scope, and no error boundary fires).
- [ ] The OS log clearly shows the push/pop layer sequence during drag-to-attach, with no unmatched `has_layer_above == true` bail from a layer that was already unmounted.
- [ ] No `focus_first_in_layer` caller treats `None` as an error (every caller tolerates the no-op outcome gracefully).
- [ ] A regression test at the correct layer (Rust unit if guard-level, browser if lifecycle-level) captures the fixed behavior.
- [ ] The previous fix `01KPTK87DSZTDK1H0C7FKAHQB8` stays intact — attachments continue to be added via `PasteMatrix::AttachmentOntoTaskHandler`, not via corrupt `onCommit([...paths])`.

## Tests

- [ ] Phase 1 trace artifact: attach the captured `log show` output or `tracing::debug!` session log as a task attachment showing the full drag-to-attach call sequence (push_layer, register, focus_first_in_layer with args, bail/success, unregister, remove_layer).
- [ ] One of:
  - Rust unit test `focus_first_in_layer_does_not_bail_when_layer_key_not_in_stack` in `swissarmyhammer-spatial-nav/src/spatial_state.rs` — asserts the guard tolerates a stale key without bailing falsely. (Already covered by current `has_layer_above` semantics — verify; add if missing.)
  - Browser test `kanban-app/ui/src/components/attachment-drop-preserves-focus.browser.test.tsx` — render inspector, fire drag-drop with a file, await paste, assert the focused moniker is still `task:<id>` (or the attachment field's focus scope).
- [ ] Existing tests still pass:
  - `swissarmyhammer-spatial-nav` full test suite (focus_first_in_layer tests at `spatial_state.rs:2055+`, layer-stack tests).
  - `kanban-app/src/spatial.rs` Tauri-command tests.
  - `kanban-app/ui/src/components/fields/displays/attachment-display.test.tsx` and any new tests from `01KPTK87DSZTDK1H0C7FKAHQB8`.
- [ ] Run: `cargo nextest run -p swissarmyhammer-spatial-nav` and `cd kanban-app/ui && bun test attachment` — all passing.

## Workflow

- Use `/tdd` for the regression test once the root cause is identified. Do NOT speculate on which Phase-2 branch applies before capturing the Phase-1 trace — the three outcomes have different fixes, and picking wrong wastes time.
- This task lives on the `navigation` branch. Make sure the worktree is `/Users/wballard/github/swissarmyhammer/swissarmyhammer-navigation` and HEAD is `ed4918049` (or newer) before starting.
- Do NOT revert the `has_layer_above` softening — it's correctly more permissive than the old `match` pattern and is not the regression source.
- If Phase 1 reveals the user's running build was simply stale (old `match` pattern still compiled in), the fix is literally "rebuild" and the task is a no-op for the code. In that case, close the task noting the rebuild resolution and file a follow-up to improve the dev-loop so stale builds are harder to run undetected — don't leave this ticket open as "fixed by rebuild" without at least that follow-up.

## Related

- `01KPTK87DSZTDK1H0C7FKAHQB8` — the previous drag-to-attach bug (data corruption via `onCommit([...paths])`). Must stay fixed.
- Spatial-nav project: `.kanban/projects/spatial-nav.yaml` and cards `01KNM3YHHFJ3PTXZHD9EFKVBS6`, `01KNQXW7HHHB8HW76K3PXH3G34` — architecture background for the layer stack and spatial registry. #bug #drag-and-drop #blocker