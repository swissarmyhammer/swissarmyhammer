---
assignees:
- claude-code
position_column: todo
position_ordinal: e080
project: spatial-nav
title: 'Perspective tabs: h/l (nav.left/right) from a focused tab doesn''t reach adjacent tab, focus vanishes'
---
## What

With focus on a perspective tab (e.g. "Default" in the perspective tab bar), pressing `h` or `l` (nav.left / nav.right) does not move focus to the adjacent perspective tab. Instead focus is "just lost" — the focus bar disappears and no subsequent nav key does anything visible.

### Expected behavior

Perspective tabs sit horizontally in a flex container near the top of each view. Cardinal nav should treat them as a row:

- `l` / `nav.right` from tab N → tab N+1 (if one exists to the right)
- `h` / `nav.left` from tab N → tab N-1 (if one exists to the left)
- At the ends: either stay put (no movement, no event) OR go to the `+` add button / LeftNav button / toolbar — but **never** silently clear focus to nothing

### Current relevant code

`kanban-app/ui/src/components/perspective-tab-bar.tsx:309-327` — `ScopedPerspectiveTab` wraps each tab in:

```tsx
<FocusScope
  moniker={moniker("perspective", perspective.id)}
  commands={commands}    // perspective.activate.<id> bound to Enter
  renderContainer={false}
>
  <PerspectiveTab ... />
</FocusScope>
```

`PerspectiveTab` (same file, ~line 400-470) uses `useFocusScopeElementRef()` to attach the enclosing scope's ref to its root `<div>` at line 468 (`data-moniker={tabMoniker}`). `data-focused` is written by `useFocusDecoration` in the parent FocusScope.

This matches the pattern LeftNav and data-table cells use successfully.

### Why the symptom shape points to rect geometry, not plumbing

"Focus is lost" (visual vanishes, no neighboring tab gains focus) means one of:

1. **Beam test / scoring picks a scope whose rect has zero-size or is off-screen** — focus moves there, the moniker changes, but `data-focused` is set on a DOM node that isn't visible. Most likely: a tab's rect registered as `{x:0, y:0, w:0, h:0}` because `ResizeObserver` fired before layout settled, or because the parent flex container collapsed its first measurement.

2. **Beam test picks a scope that is not a tab** — the perspective bar contains more than just tabs. The `AddPerspectiveButton` (`+`) at ~line 318, a `FilterEditor` body, and the `PerspectiveTabBar` itself may also be registered as FocusScopes or may have child scopes that register rects overlapping the tab row. If one of those has a rect that scores better than the adjacent tab, `h`/`l` lands on it — which may be invisible/unfocusable or render no decoration.

3. **Adjacent tabs are registered but their `layer_key` doesn't match `layer_stack.active()`** — same failure shape as the earlier H2 hypothesis. Filter culls them, candidate pool for h/l is empty except for one unexpected thing, focus goes there.

4. **Every tab shares the SAME `data-moniker`** (shouldn't — each is `perspective:<id>` keyed by distinct id) OR multiple instances of the same scope register with different spatial keys and the beam-test "finds" a duplicate. Would show up in `__spatial_dump` as two entries with identical monikers but different keys.

5. **The tab's FocusScope is nested inside another FocusScope** whose rect swallows its children, and beam test resolves to the parent instead of the adjacent sibling. Audit: is `<PerspectiveTabBar>` itself wrapped in a FocusScope that it shouldn't be? Is the `<CommandScopeProvider>` somewhere introducing a `parent_scope` that redirects h/l to the bar's container rect?

### Diagnostic sequence

1. **Focus a perspective tab via click.** Verify `data-focused="true"` on that tab's `<div>`.

2. **`__spatial_dump`** — capture all entries. For each perspective tab, confirm:
   - `moniker` is `perspective:<id>` with distinct id per tab
   - `layer_key == layer_stack.active().key`
   - `rect` has non-zero width and height
   - `rect.y` is similar across all tabs (they're in the same horizontal row)
   - `rect.x` differs in the expected left-to-right order

3. **Press `l`** and capture:
   - App log `nav.right` dispatch
   - `focus-changed` event payload — what's the `next_key`?
   - Resolve `next_key` via the dump's entries — is the target an adjacent tab, the `+` button, a filter input, or something else entirely?

4. **Paste all output into this task's description under a `## Diagnostic Evidence` heading before committing any fix.**

The dump tells you which of the 5 hypotheses is real.

### Likely fix directions by hypothesis

- **(1) Zero/bad rects** — investigate why `ResizeObserver.observe()` reports a valid rect on initial mount but a zero rect persists in Rust. Likely a timing issue where the first `report()` runs before layout completes and there's no re-report on the next frame. Fix in `kanban-app/ui/src/components/focus-scope.tsx` (`useRectObserver`) — add a `requestAnimationFrame` retry if the first measurement is zero, OR ensure `ResizeObserver` observes the correct element.

- **(2) Wrong target** — audit `kanban-app/ui/src/components/perspective-tab-bar.tsx` and any parent container for rogue FocusScopes or rect-producing elements. Either make the non-tab element not a spatial target (`spatial={false}`) or give it a shape/position that doesn't outscore the adjacent tab.

- **(3) Layer mismatch** — same fix as task `01KPVT4K538CJHJR31NNQHY8EH`. Should already be addressed by the recent architectural pass; revisit only if the dump proves it.

- **(4) Duplicate entries** — find the double-mount in React. Likely a re-keyed render that re-registers a new spatial key for the same moniker without unregistering the old one. Fix the effect cleanup.

- **(5) Parent scope shadowing** — audit `parent_scope` threading. The tab's `FocusScope` should have the tab bar as its parent scope, but `container_first_search` should still pick sibling tabs before falling back to the parent. If the parent is the one winning, the bar itself is registering a spatial rect it shouldn't.

### Regression test (required)

Add a Rust unit test to `swissarmyhammer-spatial-nav/src/spatial_state.rs::tests`:

```rust
#[test]
fn navigate_right_from_tab_reaches_adjacent_tab_not_parent_or_sibling_control() {
    // Three perspective tabs in a row, plus a "+" button to the right
    // of the last tab, plus a filter-editor rect to the right of "+".
    let state = SpatialState::new();
    state.push_layer("window".into(), "window".into());
    reg(&state, "t1", "perspective:p1", rect( 10.0, 40.0, 80.0, 28.0), "window", None);
    reg(&state, "t2", "perspective:p2", rect(100.0, 40.0, 80.0, 28.0), "window", None);
    reg(&state, "t3", "perspective:p3", rect(190.0, 40.0, 80.0, 28.0), "window", None);
    reg(&state, "add",  "perspective:add", rect(280.0, 40.0, 28.0, 28.0), "window", None);
    reg(&state, "flt",  "filter:body",     rect(320.0, 40.0, 200.0, 28.0), "window", None);

    // From t1, nav.right lands on t2 — not t3, not add, not flt
    state.focus("t1".into()).unwrap();
    let ev = state.navigate(Some("t1"), Direction::Right).unwrap().unwrap();
    assert_eq!(ev.next_key, Some("t2".to_string()));

    // From t2, nav.left lands on t1
    state.focus("t2".into()).unwrap();
    let ev = state.navigate(Some("t2"), Direction::Left).unwrap().unwrap();
    assert_eq!(ev.next_key, Some("t1".to_string()));

    // From t3, nav.right lands on `add` (adjacent, same layer, in-beam)
    state.focus("t3".into()).unwrap();
    let ev = state.navigate(Some("t3"), Direction::Right).unwrap().unwrap();
    assert_eq!(ev.next_key, Some("add".to_string()));
}
```

If this test passes, the bug is NOT in the Rust algorithm — it's in the React-side rect registration or layer assignment for the real perspective tabs. The dump then tells you which.

### Files likely touched

- `kanban-app/ui/src/components/perspective-tab-bar.tsx` — if any rogue FocusScope or rect-registering child needs `spatial={false}` or removal
- `kanban-app/ui/src/components/focus-scope.tsx` — if `useRectObserver` needs a zero-rect retry
- `swissarmyhammer-spatial-nav/src/spatial_state.rs` — only if the algorithm test fails (unlikely given prior work)

### Out of scope

- Changing what happens at tab-bar edges (wrap, or leak to toolbar). Default must be: stay put or land on the `+` button if it's adjacent. No invisible focus.
- Wiring keybindings to `perspective.next` / `perspective.prev` commands — the user wants plain h/l to work through spatial nav, not a semantic shortcut.

## Acceptance Criteria

- [ ] Manual: focus a perspective tab (click), press `l` repeatedly — focus moves tab→tab across the row, then lands on the `+` button (if present) or stays at the last tab; never goes blank
- [ ] Manual: symmetric for `h` — walks left through tabs, stops at first tab or steps over to an adjacent visible element (LeftNav)
- [ ] At no point during tab navigation does the focus bar disappear without a visible target gaining it
- [ ] `__spatial_dump` output captured in the task description before the fix
- [ ] Rust unit test `navigate_right_from_tab_reaches_adjacent_tab_not_parent_or_sibling_control` (plus a `nav.left` variant) passes
- [ ] If the fix lands in React: a vitest-browser test in `kanban-app/ui/src/test/` asserts `nav.right` from a perspective tab dispatches `dispatch_command` (via the real dispatcher, no shim) and the resulting `focus-changed` event's `next_key` resolves to the adjacent tab's moniker
- [ ] All existing tests still green

## Tests

- [ ] `cargo test -p swissarmyhammer-spatial-nav navigate_right_from_tab_reaches_adjacent_tab_not_parent_or_sibling_control` — passes
- [ ] `cargo test -p swissarmyhammer-spatial-nav -p swissarmyhammer-kanban -p kanban-app` — green
- [ ] `cd kanban-app/ui && npm test` — green
- [ ] Manual verification per acceptance criteria 1–3

## Workflow

- Use `/tdd`. Write the Rust unit test first — if it passes against current code, the bug is in the React layer (rect registration, layer_key threading, or sibling scopes that outscore tabs). If it fails, the algorithm needs work.
- Capture the `__spatial_dump` in the task description as the authoritative record of state when the bug reproduces. No fix commits without the dump.
- Fix at the root cause site the dump points to. Do not patch the symptom (e.g. "force nav to pick monikers starting with `perspective:` when in the perspective bar"). The general beam test must produce the right answer for geometrically-aligned sibling scopes.
- If the fix turns out to be "some other FocusScope is registering a rect that outscores the tabs," the right fix is to either remove that scope's spatial registration (`spatial={false}`) or restructure the layout so it doesn't compete. Not to hack around it.

