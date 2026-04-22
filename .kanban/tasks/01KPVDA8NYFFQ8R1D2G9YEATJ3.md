---
assignees:
- claude-code
position_column: todo
position_ordinal: de80
project: spatial-nav
title: 'Nav keys: SpatialState::navigate returns Ok(None) for every nav.* — candidate pool is empty (registration or layer-key bug)'
---
## Runtime diagnosis — hypothesis narrowed (2026-04-22)

A second diagnostic pass ran with the tracing + log-capture workflow the original task called for. The live macOS unified log is unambiguous:

```
cmd=nav.down  scope_chain=Some([..., "window:main", "engine"])
command completed  cmd=nav.down  result=null  undoable=false
```

Every `nav.*` command:
1. ✅ Reaches Rust
2. ✅ Dispatches to `NavigateCmd.execute`
3. ✅ Resolves `SpatialNavigatorExt` from the context
4. ✅ Calls `TauriSpatialNavigator::navigate`
5. ✅ Calls `SpatialState::navigate`
6. ❌ Gets back `Ok(None)` — **no candidate found**
7. ❌ No `focus-changed` event emitted (the `Some(event) => …` arm is never taken)

The candidate pool is empty. `spatial_search` returns no entries when filtered by active layer + direction. Since click-to-focus works (Rust accepts `spatial_focus(key)` and knows the focused key), the break is specifically in **which scopes are registered in Rust's state**.

### The three remaining hypotheses (run in this order)

**Hypothesis 1: Only the clicked scope is registered.** Other cards/cells never called `spatial_register`. Symptom shape: `spatial_state.entries.len() == 1` after the app loads and the user clicks one thing. Most-likely cause: a silent regression in `useRectObserver` or the FocusScope registration path — maybe the `layerKey` used at registration resolves to `null` or `""`, which a later filter culls.

**Hypothesis 2: Entries are registered, but with a `layer_key` that doesn't match `layer_stack.active().key`.** Symptom shape: `spatial_state.entries.len() > 1` but `spatial_search` returns empty because the active-layer filter (`spatial_state.rs:732-766`) culls every entry. Most-likely cause: `useLayerRegistration`'s `useState` initializer pushes the window layer with one key, but `useFocusScopeElementRef` / the Rust-side `spatial_register` invoke carries a different layer key (stale ref, mismatched generation).

**Hypothesis 3: Entries + layer match, but `parent_scope` or `overrides` shadow every candidate.** Much less likely given the beam-test fix and the override audit, but check third.

### Diagnostic commands — do these in order

1. **Dump the SpatialState after click-to-focus** via a new Tauri command `__spatial_dump` (already exists per CLAUDE.md — use it, don't reinvent). Add tracing inside `spatial_state.rs` if the dump isn't granular enough. Required fields to observe:
   - Number of registered entries
   - For each entry: `key`, `moniker`, `layer_key`, `rect`
   - Current `focused_key`
   - `layer_stack` contents in order, and what `active()` returns
2. **Run the app, load a board with at least 2 cards, click card 1, capture the dump.** Compare to what React should have registered (grep the DOM for `data-moniker` attributes).
3. **If Hypothesis 1**: only 1 entry → fix the missing registrations (trace `spatial_register` invokes in log; compare to mounted FocusScope count).
4. **If Hypothesis 2**: N entries but `active().key != entry.layer_key` for most → fix the layer-key threading. Most likely the `FocusLayerKeyContext` value at the FocusScope's render point differs from what `useLayerRegistration` pushed.
5. **If Hypothesis 3**: all entries under active layer → audit overrides / parent_scope chain.

### Files to instrument first

- `/Users/wballard/github/swissarmyhammer/swissarmyhammer-navigation/kanban-app/src/spatial.rs` — already has `__spatial_dump` Tauri command (verify), add `tracing::info!` in `spatial_register` with `key`, `moniker`, `layer_key` as it lands
- `/Users/wballard/github/swissarmyhammer/swissarmyhammer-navigation/kanban-app/ui/src/components/focus-scope.tsx` — add `console.warn("[FocusScope] register", moniker, layerKey)` at the registration effect (per `frontend-logging` memory — read via `log show`)
- `/Users/wballard/github/swissarmyhammer/swissarmyhammer-navigation/kanban-app/ui/src/components/focus-layer.tsx` — add `console.warn("[FocusLayer] push", name, key)` at the useState initializer

### Why the 1401 tests pass

The tauri-boundary stub (`setup-tauri-stub.ts`) intercepts `invoke("spatial_register", ...)` and records the call — but it does not run the real `SpatialState` machinery. Tests assert that certain monikers were registered; they do NOT assert `spatial_search` returns the right candidate because the shim no longer exists. The algorithm lives in Rust and the frontend boundary doesn't cross into it. So every test can pass while the production `SpatialState` holds zero or one entry.

### New integration-test requirement

A `cargo test -p kanban-app nav_dispatch_integration` test must:
1. Build a real Tauri `AppHandle` + `AppState`
2. Invoke `spatial_register` for **at least three** scopes (so the candidate pool is non-empty under every direction)
3. Invoke `spatial_focus` on one of them
4. Dispatch `nav.down` through `dispatch_command`
5. Assert the returned `Value` is a moniker string (not `Value::Null`) AND that a `focus-changed` event was emitted
6. Re-run with zero focused scope — assert fallback-to-first selects a registered moniker

This test covers the exact runtime gap that let the break ship.

---

## Original task body (preserved below)

### What

User report: nav keys (`h/j/k/l`, arrow keys, emacs equivalents) do not cause any visible navigation in the running app. Click-to-focus works. The recent architectural overhaul (commands → Rust → navigate → focus-changed) is not observably functional, even though all 1401 UI tests and the Rust suite pass.

### Architecture snapshot (already verified by code inspection)

- `swissarmyhammer-commands/builtin/commands/nav.yaml` — declares `nav.up/down/left/right/first/last/rowStart/rowEnd` with keybindings, no client-side `execute`
- `swissarmyhammer-kanban/src/commands/mod.rs:161-190` — registers each `nav.*` id to an `Arc<NavigateCmd(Direction::...)>`
- `swissarmyhammer-kanban/src/commands/nav_commands.rs:41-53` — `NavigateCmd.execute` calls `ctx.require_extension::<SpatialNavigatorExt>()` and delegates to `navigate(window_label, direction)`
- `kanban-app/src/spatial.rs:518-549` — `TauriSpatialNavigator::navigate` reads `spatial_state.focused_key()` then calls `spatial_state.navigate(from_key, direction)`; on `Some(event)`, calls `window.emit_to(window.label(), "focus-changed", &event)`
- `kanban-app/src/commands.rs:1613-1616` — `SpatialNavigatorExt` unconditionally installed on every `CommandContext` during dispatch
- `kanban-app/ui/src/lib/entity-focus-context.tsx:269-306` — `useFocusChangedEffect` listens on `getCurrentWebviewWindow()`; emit + listen labels must match

### Out of scope

- Redesigning the dispatch pipeline
- Adding new commands or keybindings
- Touching the React→Rust command-dispatch side (clicks work; that side of the loop is fine)

## Acceptance Criteria

- [ ] After app load, `__spatial_dump` (or equivalent) shows N entries with `N >= count-of-visible-FocusScopes-in-DOM` — not 0 or 1
- [ ] Every registered entry's `layer_key` matches `layer_stack.active().key` (for window-layer scopes) or an inspector/modal layer key that is somewhere in `layer_stack`
- [ ] Pressing `h`/`j`/`k`/`l` (vim), `ArrowUp`/`Down`/`Left`/`Right` (cua), or `Ctrl+p/n/b/f` (emacs) in the running app produces a visible focus change — the focus-bar moves to a different scope
- [ ] `G`/`End`/`Alt+>` moves focus to the last registered scope in the active layer
- [ ] The macOS unified log shows, in order: `dispatch_command cmd=nav.<dir>`, `spatial_state.navigate … result=Some(event)` (not None), `emit focus-changed window=<label>`, and on the JS side, `[focus-changed] moniker=<resolved>` — no silent step
- [ ] Nav works both with an initial focus set (click first, then nav) AND on a fresh load with no click (fallback-to-first path)
- [ ] New integration test in `kanban-app/tests/` fails on the current broken codebase and passes after the fix
- [ ] All existing tests still green (`npm test` and `cargo test -p swissarmyhammer-spatial-nav -p swissarmyhammer-kanban -p kanban-app`)
- [ ] No permanent instrumentation left in code paths where it adds noise

## Tests

- [ ] New: `kanban-app/tests/nav_dispatch_integration.rs` — mount a real `AppHandle` + `AppState`, register **at least three** spatial entries, dispatch `nav.down` via `commands::dispatch_command`, assert the result is a moniker string (not `Value::Null`) AND a `focus-changed` event was emitted with the expected `next_key`
- [ ] Second case: `focused_key = None` at dispatch time, verify fallback-to-first fires and emits an event
- [ ] Third case: entries registered with a layer key that matches the active layer — explicitly assert the pool filter doesn't cull them
- [ ] Run `cargo test -p kanban-app nav_dispatch_integration` — new tests green
- [ ] Run `cargo test -p swissarmyhammer-spatial-nav -p swissarmyhammer-kanban -p kanban-app` — all existing tests still green
- [ ] Run `cd kanban-app/ui && npm test` — all 1401 tests still green
- [ ] **Live verification** (per `always-verify` memory — do not skip): run the app, press `h/j/k/l`, confirm focus moves. Capture `log show --predicate 'subsystem == "com.swissarmyhammer.kanban"' --last 2m` output showing the full round-trip AND the `spatial_state.entries` count > 1.

## Workflow

- Use `/tdd` but **start with the dump, not the test**. The first action is: build the app, click a card, dump `SpatialState`. What you see determines which hypothesis (1, 2, or 3) is real and which file the fix lands in.
- Only once the hypothesis is confirmed, write the failing integration test that reproduces the same shape, then fix, then re-run live.
- Per `always-verify` memory: check the macOS log yourself, do not ask the user to verify. Per `frontend-logging`: use `console.warn` for any JS instrumentation, read the OS log via `log show`.
- Remove temporary `console.warn` breadcrumbs from production code before closing the task. Keep Rust `tracing::info!` entries that add permanent observability value (registration count, layer active, nav result).
