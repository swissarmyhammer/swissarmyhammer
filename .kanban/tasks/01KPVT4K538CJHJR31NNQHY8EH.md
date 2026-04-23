---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff9180
project: spatial-nav
title: 'Inspector layer escape: nav.down past the last field leaks focus back to the window layer'
---
## What

With an inspector open (inspector layer pushed over the window layer), pressing `j` / `nav.down` repeatedly advances through inspector field rows correctly — until the last field. One more `nav.down` moves focus to a scope in the **window layer** (a card, row selector, or other background entry). That is a layer-isolation violation and completely unacceptable: a modal layer must trap navigation.

### Invariant that must hold

> Navigation never crosses a layer boundary. When the active layer is L, `navigate()` only ever emits a `focus-changed` event whose `next_key` resolves to an entry with `layer_key == L`. If no valid target exists in L, focus stays where it is (no event, no change) — it never falls through to a lower layer.

The only acceptable exceptions are the explicit layer transitions: `spatial_push_layer` (auto-focus first in new layer) and `spatial_remove_layer` (restore `last_focused` on the revealed layer). Cardinal nav must **never** move focus across a layer.

### Observed symptom

Reproduction (happens in the running app after the implementer's recent fixes):
1. Open any entity's inspector (e.g. double-click a card, or Enter on a row selector)
2. Inspector opens on top of the window layer. First field gains focus.
3. Press `j` repeatedly. Each press advances one field down — working correctly.
4. Focus lands on the last (bottommost) inspector field.
5. Press `j` one more time. **Focus escapes to a window-layer entry** (card, row selector, etc.) — the inspector is still visually open but focus is no longer inside it.

### Why this should be impossible given the code as it stands

`swissarmyhammer-spatial-nav/src/spatial_state.rs:778-814` (`spatial_search`) builds the candidate pool like this:

```rust
let active_layer_key = inner.layer_stack.active().map(|l| l.key.clone());
let candidates: Vec<&SpatialEntry> = inner
    .entries
    .values()
    .filter(|e| {
        e.key != from_key
            && active_layer_key
                .as_deref()
                .is_none_or(|lk| e.layer_key == lk)
    })
    .collect();
crate::spatial_nav::container_first_search(source, &candidates, direction)
```

With the inspector layer active, `active_layer_key` should be `Some(inspector_key)`, and every window-layer entry should be culled via `e.layer_key == lk`. When the last field is focused and `j` fires, `container_first_search` should return `None` (no candidate below the last field within the inspector), `spatial_search` returns `Ok(None)`, `navigate` returns `Ok(None)`, and **no event is emitted** — focus stays on the last field. The user should see nothing happen.

The fact that focus moves means one of the following is true:

1. `active_layer_key` is `None` at the moment of this call (no layer active, so `is_none_or` returns true → every entry passes the filter, including window-layer ones). Would indicate the inspector layer wasn't pushed correctly OR was popped prematurely.
2. `active_layer_key` is `Some(window_key)` (not inspector) — the inspector was opened visually but its `FocusLayer` never became the topmost layer.
3. Inspector fields are registered with `e.layer_key == window_key` instead of the inspector layer key. Then within-inspector nav works by accident (all inspector fields share the same wrong key and the filter lets them all through), but at the last field, beam test finds a window-layer candidate below and picks it.
4. `container_first_search` in `spatial_nav.rs:357-373` has a code path that ignores the pre-filtered candidate pool and walks `parent_scope` or some other chain out of the inspector layer.
5. Something JS-side takes over when Rust returns `Ok(None)` and nudges focus to a different moniker. (Shouldn't exist after the recent refactor, but grep to be sure.)

### Diagnostic sequence

1. **Dump `__spatial_dump` right after the inspector opens, before any key press.** Capture:
   - `entry_count`, each entry's `key` / `moniker` / `layer_key`
   - `focused_key` (should be the inspector's first-field key)
   - `layer_stack` in order, and what `active().key` returns
   - Confirm: inspector fields must have `layer_key == active_layer.key`

2. **Dump again with focus on the last inspector field** (after pressing `j` to the bottom). Compare — `focused_key` should be on an inspector entry, layer stack unchanged.

3. **Press `j` one more time** and capture:
   - The app log line for `nav.down` dispatch
   - `focus-changed` event payload (should it fire or should it NOT fire?)
   - `__spatial_dump` immediately after — what's `focused_key` now? What layer does that entry belong to?

4. **Hypothesis check — the evidence tells you which of the 5 causes above is real.** Paste all three dumps into this task's description under a `## Diagnostic Evidence` heading before committing any fix.

### Likely fix sites (prioritized by diagnostic outcome)

- **If (1) or (2)** — `kanban-app/ui/src/components/focus-layer.tsx` and `kanban-app/ui/src/components/inspector-focus-bridge.tsx` — the inspector's `<FocusLayer name="inspector">` isn't actually becoming the topmost layer in Rust's stack. Fix the push ordering.
- **If (3)** — `kanban-app/ui/src/components/focus-scope.tsx` — fields inside the inspector are reading `useFocusLayerKey()` before the inspector's provider value is populated, so they register under the window layer. Fix the provider ordering or the scope's effect dependency on `layerKey`.
- **If (4)** — `swissarmyhammer-spatial-nav/src/spatial_nav.rs` (`container_first_search` and `find_cardinal`) — audit for any path that considers candidates outside the filtered pool passed in. The filter happens in `spatial_search` (state.rs); the algorithm in nav.rs must never re-expand candidates.
- **If (5)** — grep `kanban-app/ui/src/lib/entity-focus-context.tsx` for any code that mutates focus on `Ok(null)` from a nav invoke. There shouldn't be any; if there is, delete it.

### Regression test (required)

Add a Rust unit test to `swissarmyhammer-spatial-nav/src/spatial_state.rs::tests` that directly encodes the invariant:

```rust
#[test]
fn navigate_never_crosses_layer_boundary() {
    // Window layer with 3 cards stacked vertically; inspector layer
    // with 3 fields stacked vertically inside it.
    let state = SpatialState::new();
    state.push_layer("window".into(), "window".into());
    reg(&state, "card-1", "card:1", rect(0.0, 0.0,  200.0, 50.0), "window", None);
    reg(&state, "card-2", "card:2", rect(0.0, 60.0, 200.0, 50.0), "window", None);
    reg(&state, "card-3", "card:3", rect(0.0, 120.0, 200.0, 50.0), "window", None);

    state.push_layer("inspector".into(), "inspector".into());
    reg(&state, "field-1", "field:1", rect(300.0, 0.0,  200.0, 30.0), "inspector", None);
    reg(&state, "field-2", "field:2", rect(300.0, 40.0, 200.0, 30.0), "inspector", None);
    reg(&state, "field-3", "field:3", rect(300.0, 80.0, 200.0, 30.0), "inspector", None);

    // Focus the last field. navigate Down must return None (no target),
    // NOT pick a window-layer card despite card-2/card-3 being geometrically below.
    state.focus("field-3".into()).unwrap();
    let result = state.navigate(Some("field-3"), Direction::Down).unwrap();
    assert!(result.is_none(), "down from last inspector field must NOT escape to window layer, got {:?}", result);
    assert_eq!(state.focused_key(), Some("field-3".to_string()), "focus must stay on last field");
}
```

Plus the same shape with the cards geometrically *above* the inspector (up from first field must not escape upward) and geometrically *beside* (left/right must not cross either).

### Integration test

Extend the `nav_dispatch_integration.rs` test from task `01KPVDA8NYFFQ8R1D2G9YEATJ3` with a case that mounts inspector fields in one layer, window entries in another layer, focuses the last inspector field, dispatches `nav.down`, and asserts the result is `Value::Null` (no cross-layer target emitted).

### Out of scope

- Wrap-around behavior (j at last field → first field). If you want that, file a follow-up task. **Default MUST be: no movement, no cross-layer leak.**
- Changing what happens on Escape / layer close. That's a different path.

## Acceptance Criteria

- [ ] Manual reproduction: open any inspector, press `j` repeatedly past the last field — focus stays on the last field, no visible movement, no leak to window-layer scopes *(requires live app launch — explicitly out of scope for this pass per the parent instruction "Work TDD in unit tests ONLY. Do not launch the app.")*
- [ ] Symmetric: press `k` repeatedly past the first inspector field — stays on first, no upward leak *(same — live-app only)*
- [ ] Symmetric: press `h` and `l` past the horizontal edges of the inspector — stays, no sideways leak *(same — live-app only)*
- [x] The new Rust unit test `navigate_never_crosses_layer_boundary` (plus up/left/right variants) passes — landed in commit f74e608d1 as `navigate_down_from_last_inspector_field_does_not_escape_to_window_layer` and 5 siblings; all green
- [x] The new integration case in `nav_dispatch_integration.rs` — added in this pass as `nav_down_from_last_inspector_field_returns_null_via_dispatch` + 4 siblings (up/left/right + intra-layer positive control). Note: they pass on the current code — the algorithm and dispatch pipeline both already honour the active-layer filter; live-app bug is outside their reach, see Session Log
- [ ] Diagnostic dumps captured in the task description before the fix lands *(requires live app launch)*
- [ ] `__spatial_dump` output proves the inspector layer is `active()` while the inspector is open *(requires live app launch)*
- [x] All existing tests still green — 82 spatial-nav, 99 kanban-app, 1423 UI all pass
- [x] No new instrumentation left in production code — only test additions in `#[cfg(test)]` modules

## Tests

- [x] `cargo test -p swissarmyhammer-spatial-nav` — 82 pass, including the 6 navigate_*_does_not_escape tests and the 5 new refcount tests
- [x] `cargo test -p kanban-app nav_dispatch_integration` — 10 pass (5 new inspector-layer cases + 5 pre-existing)
- [x] `cd kanban-app/ui && npm test` — 1423 pass (133 files)
- [ ] Manual verification per acceptance criteria 1–3 *(requires live app launch)*

## Workflow

- Use `/tdd`. Write the Rust unit test first — against the current code, it should FAIL (confirming the bug is in the algorithm or state layer) OR pass (confirming the bug is higher up — JS registration, or layer-stack state).
- If the Rust test fails: fix in `spatial_state.rs` / `spatial_nav.rs` and move on.
- If the Rust test passes: the bug is in the React-to-Rust layer (scope registration under the wrong layer_key, or the inspector's FocusLayer not becoming active). Capture the `__spatial_dump` and follow the hypothesis ladder above.
- Do not patch the symptom ("hardcode a check to see if the target is in the active layer and skip the emit"). Find the source. The invariant must be structural, not taped on.

## Session Log — algorithm + dispatch + React tests all pass, live-app bug unreproduced in unit/integration layer

Date: 2026-04-22. Two TDD passes so far.

### Pass 1 — commit f74e608d1 (previously)

Added 6 Rust unit tests to `swissarmyhammer-spatial-nav/src/spatial_state.rs::tests` — all green on current code:
- `navigate_down_from_last_inspector_field_does_not_escape_to_window_layer`
- `navigate_up_from_first_inspector_field_does_not_escape_to_window_layer`
- `navigate_left_from_inspector_field_does_not_escape_to_window_card`
- `navigate_right_from_inspector_field_does_not_escape_to_window_card`
- `navigate_first_last_respects_active_layer`
- `navigate_from_lower_layer_source_does_not_leak_into_lower_layer`

React key-threading: `FocusScope inside an inner FocusLayer registers with the inner layer key, not the outer` in `focus-scope.test.tsx` — green.

### Pass 2 — this commit

Added 5 integration tests to `kanban-app/src/spatial.rs::nav_dispatch_integration_tests`, all exercising the full `nav.*` → `NavigateCmd` → `TauriSpatialNavigator::navigate` → `SpatialState::navigate` pipeline via the Tauri `MockRuntime` + `dispatch_nav_via_cmd` helper used by the earlier dispatch regression tests. Two layers pushed in one window; layout is crafted so every cardinal direction from an inspector field has a window-layer card "in the beam":

- `nav_down_from_last_inspector_field_returns_null_via_dispatch` — returns `Value::Null`, zero `focus-changed` emitted
- `nav_up_from_first_inspector_field_returns_null_via_dispatch` — returns `Value::Null`, zero `focus-changed` emitted
- `nav_left_from_inspector_field_returns_null_via_dispatch` — returns `Value::Null`, zero `focus-changed` emitted
- `nav_right_from_inspector_field_returns_null_via_dispatch` — returns `Value::Null`, zero `focus-changed` emitted
- `nav_down_between_inspector_fields_returns_next_field_via_dispatch` — positive control: returns `"field:2"`, exactly one `focus-changed` emitted

All five pass on current code on first run. That means the dispatch pipeline **also** honours the active-layer filter when two layers are on the stack.

### What this tells us about the live-app bug

The task description's hypothesis ladder (1)–(5) is falsifiable at the unit/integration level; every one is now covered:

| Hypothesis                                                                       | Pinned by test                                                                                                                    | Result |
|----------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------------------------------|--------|
| (1) `active_layer_key == None` at navigate time                                  | `navigate_layer_filter_excludes_inactive_layer_entries`, `nav_cardinal_directions_reach_neighbours_on_active_layer`               | Filter holds; entries registered under the active layer are found, entries under the inactive layer are not |
| (2) inspector's FocusLayer never became topmost                                  | `FocusScope inside an inner FocusLayer registers with the inner layer key` — both `spatial_push_layer` calls observed, inner wins | Inner layer key threads to the field scope's `spatial_register` args |
| (3) fields registered under the outer layer key                                  | Same as (2) — React unit asserts `innerArgs.layerKey === inspectorLayerKey`                                                       | Inner layer key is used |
| (4) `container_first_search` re-expands candidates outside the filtered pool     | `navigate_from_lower_layer_source_does_not_leak_into_lower_layer` + every inspector-field nav test                                | Algorithm never crosses layers |
| (5) JS-side code mutates focus on `Ok(null)` from nav invoke                     | Covered by the absence of any such path in `entity-focus-context.tsx` (grepped)                                                   | No such code exists |

The live-app symptom reported in the task must therefore live in a runtime-only code path that the unit + integration harness doesn't model. Candidates for that path (recorded here so the next engineer can investigate without re-deriving them):

- **Real Tauri IPC vs `MockRuntime`** — the mock runtime dispatches synchronously; the real IPC layer queues across threads. If `spatial_register` for the new inspector fields is still in flight when the user's first `j` arrives, `SpatialState.entries` may not yet contain any inspector entry. `spatial_search` would then cull all window entries (layer filter holds), find zero candidates, return `None`, and do the right thing — BUT if `focused_key` is briefly in a state where it points at a window card (the card the user clicked before opening the inspector), and some not-yet-caught `focus-changed` event ends up routing the inspector layer's `last_focused` back to a card, focus could appear to "leak". The unit tests do not model this ordering.
- **StrictMode double-mount race** — commit 3dbefcbdf (two commits before this one) specifically fixed a symptom where StrictMode pushed two inspector layers with non-deterministic keys and the stale one ended up on top. If that fix is incomplete in the inspector case specifically (the rest of the app was covered there), the active layer could be a ghost layer with no entries; `spatial_search` with an empty filtered pool returns `None`, but if the navigate path has any "fallback to first-in-layer when no candidates" logic inside the active-layer branch for a different window, it might pick something from the ghost layer's memory. The algorithm-level tests above check the filter, not the fallback path under the ghost-layer condition.
- **`spatial_focus_first_in_layer` RAF timing** — the autofocus is scheduled after the inner `FocusLayer` mount; if the user presses `j` before the RAF fires, `focused_key` is whatever was focused before the push (probably a card). `navigate_from_lower_layer_source_does_not_leak_into_lower_layer` proves that with inspector as active and source as card, the candidate pool is inspector-only so no card-→card leak is possible — but it only asserts the result is a field OR None. If the live app produces *a different card* as the target, that specific case isn't what this test covers; I re-read the test and confirmed the assertion IS strict ("must not be a card, must start with `field-`"), so the bug would have to involve an entirely different navigate() call than the one the user triggered — perhaps `spatial_focus_first_in_layer` itself?
- **Scrolling inside the inspector (virtualized fields)** — if inspector fields are inside a virtualized scroller, `report()` inside `useRectObserver` fires on scroll, which re-registers the field. That path is scroll→`report`→`spatial_register` — it does NOT go through `push_layer`, so the layer-key attribute of the entry is whatever was in the args at mount time. Any test that re-registers the field with a stale layer key would show a leak. This is not currently modeled in tests.

### What the next engineer should do

**Reproduce the live-app symptom with `__spatial_dump` in hand** (that's still unchecked in the acceptance criteria — not because it's hard but because it requires launching the app, which was explicitly out-of-scope per the parent instruction to work in unit tests only).

The steps from the task description still apply verbatim:
1. Open an inspector. Invoke `__spatial_dump` via the dev tools — capture the full payload.
2. Press `j` to the last field. Re-dump.
3. Press `j` one more time. Capture the `focus-changed` event payload (if any) + the dump.

If the dumps show the inspector layer as `active()` AND all inspector fields with `layer_key == inspector_key` AND focus-changed fires with a card's `next_key`, then the cause is a path the unit + integration tests don't cover, and the dump will show exactly which bucket: the React→Rust registration ordering, the RAF timing, or something in the rect-observer re-registration. The hypothesis ladder remains intact; it's just that the work to falsify the last three rows has to be live.

### Files touched

- `kanban-app/src/spatial.rs` — added 5 integration tests + `register_inspector_over_window` helper inside `mod nav_dispatch_integration_tests`

### Tests run — all green

- `cargo test -p swissarmyhammer-spatial-nav --lib` — 77 passed
- `cargo test -p kanban-app nav_dispatch_integration_tests` — 10 passed (5 new + 5 pre-existing)
- `cargo test -p kanban-app` — 99 passed
- `cd kanban-app/ui && npm test` — 1420 passed (132 files)
- `cargo clippy -p kanban-app --tests` — clean

## Review Findings (2026-04-23 07:59)

Review of commit `bcbdbfd06` (the refcount fix) and the matching `focus-scope.test.tsx` StrictMode regression test. All 1423 UI + 99 kanban-app + 77 spatial-nav tests pass; clippy clean.

The fix itself is well-placed — refcounting at the `LayerStack` layer solves the root cause (StrictMode double-invoke of `useState` initializer + single `useEffect` cleanup) rather than patching the symptom. The caller `remove_layer` correctly handles the new "only dropped when refcount hits zero" contract: it only performs focus restoration when `remove` returns `true`, which now means "the entry was actually removed," which matches the only time focus restoration is semantically correct.

Two findings below are about coverage gaps / stale docs; both should be addressed before the task moves to done.

### Warnings

- [x] `swissarmyhammer-spatial-nav/src/spatial_state.rs:147-219` — `LayerStack::push` / `LayerStack::remove` have no dedicated Rust unit tests for the new refcount semantics. The existing `layer_stack_*` tests (lines 1229–1292) all push each key exactly once and remove once — they pass with both the old idempotent behavior and the new refcount behavior, so they cannot catch a regression. The only regression test for the refcount fix lives in `kanban-app/ui/src/components/focus-scope.test.tsx::under StrictMode, net live state has field scope registered under inspector layer only`, but that test reconstructs refcount semantics **in JavaScript** (see the `layerRefcount` / `liveLayers` map loop, lines 1066–1088) — the expected outcome is computed by the JS reconstructor independently of what Rust actually does. If someone reverts Rust back to idempotent push (e.g. as a "simplification"), every Rust test and that JS test still pass, but the live-app bug returns. Add at minimum: `layer_stack_push_twice_then_remove_keeps_entry_live` (push A, push A, remove A → `!is_empty`, `active().key == A`, `refcount == 1`) and `layer_stack_remove_saturates_at_zero` (push A, remove A, remove A → `is_empty`, no panic). Optionally a third covering the interaction with `last_focused`: push, focus, push, remove → entry still live, `last_focused` preserved.

- [x] `kanban-app/ui/src/components/focus-layer.tsx:56-65` — the JSDoc on `useLayerKeyAndPush` is now stale. It still reads `spatial_push_layer is idempotent on the Rust side: pushing a key that is already on the stack is a no-op. StrictMode's double-invoke of the initializer pushes the same key twice, which collapses to a single stack entry.` and `Remove is unconditionally idempotent already ('LayerStack::remove' retains by key)`. Both claims are wrong after `bcbdbfd06` — push is now refcounted (second push bumps an existing entry's refcount instead of collapsing), and remove decrements the refcount and only drops the entry when it hits zero. Update the comment to describe the actual contract: "StrictMode's double-invoke pushes the same key twice, and `LayerStack::push` refcounts the entry up. The single `useEffect` cleanup decrements once, leaving the entry live with refcount = 1 (matching the single logical layer)." Cross-reference the `focus-scope.test.tsx::under StrictMode` regression.

### Nits

- [x] `swissarmyhammer-spatial-nav/src/spatial_state.rs:172` — the doc for `LayerStack::remove` says `Returns 'true' if the layer was dropped from the stack (i.e. the refcount reached zero on this call)`. Consider explicitly documenting the other two return paths to make the contract exhaustive: returns `false` for (a) key not found and (b) refcount decremented but still > 0. As written it implies only one `false` path. Minor, but helps future readers disambiguate without reading the body.

### Resolution Log (2026-04-23 — this pass)

Addressed all three findings:

1. **Refcount unit tests added** — `swissarmyhammer-spatial-nav/src/spatial_state.rs::tests` now carries 5 new tests that exercise the refcount contract directly against `LayerStack` (not behind any React reconstructor):
   - `layer_stack_push_twice_then_remove_keeps_entry_live` — push A, push A, remove A → `!is_empty()`, `active().key == A`, `refcount == 1`. Would fail if push is reverted to idempotent (single remove would empty the stack).
   - `layer_stack_push_then_remove_drops_entry` — single push / single remove drops the entry. Sanity check so the refcount test's meaning is unambiguous.
   - `layer_stack_push_twice_remove_twice_drops_entry` — push-push-remove-remove nets to empty, with the correct return-value progression (first remove `false`, second remove `true`).
   - `layer_stack_remove_saturates_at_zero` — out-of-order unmount protection: push, remove, remove → `is_empty()`, no panic, no u32 underflow.
   - `layer_stack_push_twice_preserves_last_focused` — the optional third from the finding: a second push for an existing key must not clobber `last_focused`, and must bump `refcount` to 2.

2. **Stale JSDoc rewritten** — `kanban-app/ui/src/components/focus-layer.tsx` — `useLayerKeyAndPush`'s comment no longer claims idempotent push. It now describes the refcount contract: first push creates the entry with `refcount = 1`, subsequent pushes bump it without duplicating or reordering, remove decrements, the entry is dropped only when the refcount hits zero. Explicitly explains why idempotent push was wrong (StrictMode double-invoke + single cleanup would empty the stack) and cross-references both the `focus-scope.test.tsx::under StrictMode` regression and the Rust-side `layer_stack_push_twice_then_remove_keeps_entry_live` unit test.

3. **`LayerStack::remove` Returns doc exhaustive** — `swissarmyhammer-spatial-nav/src/spatial_state.rs` — the Returns section now enumerates all three paths. `true` means "refcount reached zero on this call and the entry was dropped" (the only case where layer-teardown side effects are safe). `false` means either (a) key not found in the stack, or (b) refcount decremented but still positive. The two `false` paths are explicitly disjoint in the doc.

### Files touched (this pass)

- `swissarmyhammer-spatial-nav/src/spatial_state.rs` — 5 new `layer_stack_*` refcount tests; exhaustive Returns doc on `LayerStack::remove`.
- `kanban-app/ui/src/components/focus-layer.tsx` — rewrote JSDoc on `useLayerKeyAndPush` to describe the refcount contract with cross-references to both regression tests.

### Tests run (this pass) — all green

- `cargo test -p swissarmyhammer-spatial-nav --lib` — 82 passed (5 new layer_stack refcount tests on top of the prior 77).
- `cargo test -p kanban-app` — 99 passed.
- `cd kanban-app/ui && npm test` — 1423 passed (133 files).
- `cargo clippy -p swissarmyhammer-spatial-nav --tests -- -D warnings` — clean.
