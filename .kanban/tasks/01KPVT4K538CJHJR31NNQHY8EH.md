---
assignees:
- claude-code
position_column: todo
position_ordinal: de80
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

- [ ] Manual reproduction: open any inspector, press `j` repeatedly past the last field — focus stays on the last field, no visible movement, no leak to window-layer scopes
- [ ] Symmetric: press `k` repeatedly past the first inspector field — stays on first, no upward leak
- [ ] Symmetric: press `h` and `l` past the horizontal edges of the inspector — stays, no sideways leak
- [ ] The new Rust unit test `navigate_never_crosses_layer_boundary` (plus up/left/right variants) passes
- [ ] The new integration case in `nav_dispatch_integration.rs` fails on the current broken code and passes after the fix
- [ ] Diagnostic dumps captured in the task description before the fix lands
- [ ] `__spatial_dump` output proves the inspector layer is `active()` while the inspector is open (rules out hypothesis 1/2 or identifies them as the cause)
- [ ] All existing tests still green
- [ ] No new instrumentation left in production code

## Tests

- [ ] `cargo test -p swissarmyhammer-spatial-nav navigate_never_crosses_layer_boundary` — passes
- [ ] `cargo test -p kanban-app nav_dispatch_integration` — passes, including the new cross-layer case
- [ ] `cd kanban-app/ui && npm test` — all tests green
- [ ] Manual verification per acceptance criteria 1–3

## Workflow

- Use `/tdd`. Write the Rust unit test first — against the current code, it should FAIL (confirming the bug is in the algorithm or state layer) OR pass (confirming the bug is higher up — JS registration, or layer-stack state).
- If the Rust test fails: fix in `spatial_state.rs` / `spatial_nav.rs` and move on.
- If the Rust test passes: the bug is in the React-to-Rust layer (scope registration under the wrong layer_key, or the inspector's FocusLayer not becoming active). Capture the `__spatial_dump` and follow the hypothesis ladder above.
- Do not patch the symptom ("hardcode a check to see if the target is in the active layer and skip the emit"). Find the source. The invariant must be structural, not taped on.

