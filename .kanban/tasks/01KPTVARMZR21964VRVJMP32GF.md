---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff8b80
project: spatial-nav
title: SpatialState::focus_first_in_layer — active-layer guard bails on legitimate calls, leaving boot with no focus
---
## What

The active-layer guard added to `SpatialState::focus_first_in_layer` in the current uncommitted diff is silently no-op'ing legitimate calls. This leaves the app booting with **no focused key**. Nav commands then fire against a null source and (regardless of what else is broken about the nav pipeline) never produce a visible focus change.

### The guard (swissarmyhammer-spatial-nav/src/spatial_state.rs:551-559, uncommitted)

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

The intent was to prevent an out-of-order RAF from a lower layer stealing focus after an inner layer has already become active. The guard as written also bails when:
- `layer_stack.active()` returns `None` (no layer on the stack yet)
- `active.key` simply differs from the caller's `layer_key` for any reason other than inner-layer-on-top

At boot, the expected sequence is:
1. Root `FocusLayer name="window"` mounts → `spatial_push_layer(window_key, "window")` invoked
2. FocusLayer's useEffect schedules a RAF that calls `focus_first_in_layer(window_key)` (this is the auto-focus-first-entry behavior from `01KPQWWCGQPN3PA2233CDE93V6`)
3. By the time the RAF runs, `layer_stack.active().key == window_key` should hold — no inner layer exists yet
4. The guard passes, first entry in window layer gets focus

Observed: the app starts with no focused key, click-to-focus works, nav keys are no-ops. The most likely explanation is that the guard is hitting the `_ => return None` arm at step 3 — either because `active()` returns `None` (layer push hadn't yet mutated state when the RAF fired) or because the key mismatch check is too strict for a correct initial-focus flow.

### User's directive

> "did you bother to make sure we always have a layer — `focus_first_in_layer_noop_when_not_active_layer` new code bails and whatever active is — almost certainly the problem"

### Diagnostic steps

1. **Instrument** `focus_first_in_layer` with tracing: log `layer_key`, `inner.layer_stack.active().map(|l| &l.key)`, and which match arm was taken. Run the app and read `log show --predicate 'subsystem == "com.swissarmyhammer.kanban"'` — confirm which arm bails at boot.

2. **Rust unit test** (add to `spatial_state.rs::tests`):

   ```rust
   #[test]
   fn focus_first_in_layer_at_boot_picks_first_entry() {
       let state = SpatialState::new();
       state.push_layer("window".into(), "window".into());
       reg(&state, "a", "window", 0.0, 0.0, 100.0, 40.0, None);
       // Immediately after push + registration, focus_first_in_layer must
       // succeed — the window layer IS the active layer, nothing above it.
       let event = state.focus_first_in_layer("window").expect(
           "focus_first_in_layer must succeed when named layer is active",
       );
       assert_eq!(event.next_key, Some("a".to_string()));
       assert_eq!(state.focused_key(), Some("a".to_string()));
   }
   ```

   This test should FAIL with the current guard if `layer_stack.active()` returns `None` after push, OR PASS if the stack is properly mutated before the guard runs. Either outcome is diagnostic:
   - Test fails → `push_layer` isn't setting up `active()` correctly; fix layer-stack init
   - Test passes → the guard isn't the only problem; look at whether `focus_first_in_layer` is even being called at the right time

3. **Layer-stack initialization audit**: trace `push_layer` in `spatial_state.rs` — does it push onto `layer_stack` immediately and make the new entry the active one? If there's any deferred mutation (e.g. `Vec::push` on a `Mutex<Vec<_>>` that hasn't been acquired yet), that would create a window where `active()` returns stale data.

4. **Remove or relax the guard** based on findings:
   - If the guard's premise is correct (out-of-order RAF from a lower layer) but the implementation is wrong, tighten the match: only bail when `active().key != layer_key AND active().key belongs to a layer ABOVE layer_key in the stack`. A call naming a layer that IS the active layer, OR a call naming the ONLY layer on the stack, must succeed.
   - If the premise is itself the bug (RAF ordering is fine, inner layers don't actually race the way the comment claims), remove the guard entirely.

### Relationship to other tasks

- **Upstream of nav failure**: boot with null focus means nav commands have no source moniker. The `broadcastNavCommand` side-channel (task `01KPTV64JF4QJE6GQK3DS0TK41`) routes a `null` key to Rust's `navigate()`, which should fall through via `fallback_to_first()` — but that path also uses the active layer filter. If the active layer is misidentified here too, nav silently no-ops. Fixing THIS guard may resolve nav end-to-end even before the side-channel rewrite.
- **Originating task**: `01KPQWWCGQPN3PA2233CDE93V6` (auto-focus first entry on layer push). That task's implementation added both the RAF and the guard. The guard is the regression.
- **Part of**: `01KPTFSDB4FKNDJ1X3DBP7ZGNZ` (multi-inspector layer isolation audit). The existing test `focus_first_in_layer_noop_when_not_active_layer` at `spatial_state.rs:304` was added to DOCUMENT the guard's behavior — but the guard's behavior is the bug, so that test is asserting the wrong thing. Replace it with a test that asserts the correct boot-time behavior.

## Acceptance Criteria

- [ ] New Rust unit test `focus_first_in_layer_at_boot_picks_first_entry` is added and passes
- [ ] The existing `focus_first_in_layer_noop_when_not_active_layer` test is either revised or deleted — if the guard is correct but narrower, the test must reflect the narrower semantics (e.g. "only bails when a DIFFERENT layer is on top," not "bails whenever active != layer_key")
- [ ] After app boot, a scope in the window layer has `data-focused="true"` — verify via DOM inspection or the macOS unified log showing `focus-changed` with a non-null `next_key`
- [ ] Opening an inspector on top of the window layer still causes the inspector's first field to gain focus (regression test for the original `01KPQWWCGQPN3PA2233CDE93V6` behavior)
- [ ] Out-of-order RAF scenario: if legit, add a test that reproduces it and still protects against down-stack focus steal; if not legit, remove the guard
- [ ] Full npm test suite and `cargo test -p swissarmyhammer-spatial-nav` green

## Tests

- [ ] Rust unit test `focus_first_in_layer_at_boot_picks_first_entry` in `swissarmyhammer-spatial-nav/src/spatial_state.rs::tests` — passes
- [ ] Rust unit test for the "inner layer has already pushed on top" scenario — only if the guard is kept. If kept, test: push window, push inspector, call `focus_first_in_layer(window_key)` → returns None. Test: push window, call `focus_first_in_layer(window_key)` with no inner push → returns Some with the first window-layer entry
- [ ] Revise `focus_first_in_layer_noop_when_not_active_layer` so its description matches whatever behavior is actually correct after this fix
- [ ] `cargo test -p swissarmyhammer-spatial-nav` — green
- [ ] Manual: launch app, observe that a card/cell has `data-focused="true"` immediately after the view loads (no click needed to get initial focus)

## Workflow

- Use `/tdd`. Write `focus_first_in_layer_at_boot_picks_first_entry` FIRST. Run it against the current code — confirm it fails (or passes, giving us the diagnostic).
- If it fails, fix the guard, re-run.
- If it passes, the guard isn't the sole issue — add tracing and run the app to find what else is blocking boot-time focus.
- Keep the fix narrow. Do NOT attempt to refactor the layer stack or the push-layer path beyond what this specific bug demands.

