---
assignees:
- claude-code
position_column: doing
position_ordinal: '80'
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

**Hypothesis 1 (most likely — prioritize first): Only the clicked scope — or no scopes at all — are registered.** Other cards/cells never called `spatial_register`. Symptom shape: `spatial_state.entries.len() ∈ {0, 1}` after the app loads and the user clicks one thing. Most-likely cause: a silent regression in `useRectObserver` / the FocusScope registration path — maybe the `layerKey` used at registration resolves to `null` or `""`, causing the `if (!layerKey || !spatial) return;` guard in `focus-scope.tsx` to skip `spatial_register` entirely. Start here.

**Hypothesis 2 (structurally unlikely after the recent fix — confirm via dump before chasing): Entries are registered, but with a `layer_key` that doesn't match `layer_stack.active().key`.** Symptom shape: `spatial_state.entries.len() > 1` but `spatial_search` returns empty because the active-layer filter (`spatial_state.rs:732-766`) culls every entry. **Caveat:** the current `FocusLayer` implementation in `kanban-app/ui/src/components/focus-layer.tsx` now uses `useLayerKeyAndPush` (a `useState` initializer that generates the ULID, invokes `spatial_push_layer` synchronously at render, and returns the same ULID from the context Provider). That makes "the layer key Rust received ≠ the layer key FocusScope sees via `useFocusLayerKey()`" structurally impossible — same constant returned from both sides of the same `useState` call. Do not spend time on H2 unless the dump shows `entries.len() > 1` AND the layer_keys visibly mismatch `layer_stack.active().key`.

**Hypothesis 3: Entries + layer match, but `parent_scope` or `overrides` shadow every candidate.** Much less likely given the beam-test fix and the override audit — check third.

### Diagnostic commands — do these in order

1. **Dump the SpatialState after click-to-focus** via `__spatial_dump` (debug-only Tauri command, already exists in `kanban-app/src/spatial.rs` per CLAUDE.md — use it, don't reinvent). Add `tracing::info!` inside `spatial_state.rs` only if the dump isn't granular enough. Required fields to observe:
   - Number of registered entries
   - For each entry: `key`, `moniker`, `layer_key`, `rect`
   - Current `focused_key`
   - `layer_stack` contents in order, and what `active()` returns
2. **Run the app, load a board with at least 2 cards, click card 1, capture the dump.** Compare to what React should have registered (grep the DOM for `data-moniker` attributes).
3. **Evidence gate (do not skip):** **paste the full `__spatial_dump` output into this task's description under a `## Diagnostic Evidence (YYYY-MM-DD HH:MM)` heading** before committing any fix. The dump is the authoritative record of which hypothesis is real. No fix lands without it being captured in the task.
4. **If Hypothesis 1** (`entries.len() ∈ {0, 1}`): only 1 entry → fix the missing registrations. Trace `spatial_register` invokes in the log (number should match `[data-moniker]` count in the DOM); compare to mounted FocusScope count.
5. **If Hypothesis 2** (`entries.len() > 1` AND layer_keys mismatch): fix the layer-key threading. Given the current structurally-consistent design, this would indicate something unexpected — audit the `useState` initializer path for a re-run.
6. **If Hypothesis 3** (all entries under active layer, still no candidate): audit overrides / parent_scope chain.

### Files to instrument first

- `/Users/wballard/github/swissarmyhammer/swissarmyhammer-navigation/kanban-app/src/spatial.rs` — `__spatial_dump` Tauri command already exists; verify and use it. If more granularity needed, add `tracing::info!` in `spatial_register` with `key`, `moniker`, `layer_key` as it lands
- `/Users/wballard/github/swissarmyhammer/swissarmyhammer-navigation/kanban-app/ui/src/components/focus-scope.tsx` — add `console.warn("[FocusScope] register", moniker, layerKey)` at the registration effect (per `frontend-logging` memory — read via `log show`)
- `/Users/wballard/github/swissarmyhammer/swissarmyhammer-navigation/kanban-app/ui/src/components/focus-layer.tsx` — add `console.warn("[FocusLayer] push", name, key)` inside `useLayerKeyAndPush`'s `useState` initializer

### Bail-out criterion

If `__spatial_dump` reveals structural corruption — e.g. `layer_stack` is empty despite a mounted `FocusLayer`, entries don't exist at all despite mounted FocusScopes, or `layer_key` values don't come from any known FocusLayer instance — **stop patching**. Revert the uncommitted diff (only the files tied to the broken hypothesis, per the earlier "don't revert all changes" instruction) and re-plan. This is the third iteration on the same symptom; the user has explicitly said regressions here are unacceptable. A broken foundation is not fixed surgically.

### Why the 1401 tests pass (testing-gap note)

The Tauri-boundary stub (`setup-tauri-stub.ts`) intercepts `invoke("spatial_register", ...)` and records the call — but it does not run the real `SpatialState` machinery. Tests assert that certain monikers were registered; they do NOT assert `spatial_search` returns the right candidate because the shim no longer exists. The algorithm lives in Rust and the frontend boundary doesn't cross into it. So every test can pass while the production `SpatialState` holds zero or one entry.

### New integration-test requirement

A `cargo test -p kanban-app nav_dispatch_integration` test must:
1. Build a real Tauri `AppHandle` + `AppState`
2. Invoke `spatial_register` for **at least three** scopes (so the candidate pool is non-empty under every direction)
3. Invoke `spatial_focus` on one of them
4. Dispatch `nav.down` through `dispatch_command`
5. Assert the returned `Value` is a moniker string (not `Value::Null`) AND that a `focus-changed` event was emitted
6. Re-run with zero focused scope — assert fallback-to-first selects a registered moniker

**Gap this test does NOT close:** it calls `dispatch_command` directly in-process, skipping the Tauri IPC boundary. Serde arg shape, `#[tauri::command]` codegen, window-label routing, event subscription through the webview — all still unprotected. Don't scope-creep into that territory here; just flag it so a follow-up can add a Tauri-IPC harness if the user wants one.

## Diagnostic Evidence (2026-04-22 15:55)

Ran the freshly-built binary (with the new permanent `tracing::info!` on `spatial_register` / `spatial_push_layer` / `spatial_remove_layer`) in the background with `./target/debug/kanban-app` and a freshly-built `ui/dist/`. Two windows opened (main + `board-01knhmgw4k017zrsydrzx6h613`) per persisted session state, both reaching the `[webPageID=8] UserActivity::Impl::endActivity: description=App nap disabled for page due to user activity` marker that signals the webview finished loading.

Captured `log show --predicate 'subsystem == "com.swissarmyhammer.kanban"' --info --debug --style compact --last 30s`. Filtered for spatial IPC:

```
# log show — last 30s since UI load
spatial_register | spatial_push_layer | spatial_focus | spatial_remove_layer
→ (no matches)
```

Total log lines in that window: **659 lines** — mostly entity-cache bridge startup, window creation, and WebKit process accounting. **Zero** `spatial_register`, `spatial_push_layer`, `spatial_focus`, or `focus-changed` entries. Zero `command cmd=…` entries from the frontend either.

Because remote shell sessions cannot drive clicks into the app, I could not capture a `__spatial_dump` after a click. But the absence of every single spatial-IPC trace during the UI-load window is itself dispositive:

- If `FocusLayer` at `app-shell.tsx:334` ever mounted, `spatial_push_layer` would have logged — it did not.
- If any `FocusScope` ever ran its `useRectObserver` effect with a non-null `layerKey`, `spatial_register` would have logged — it did not.

That matches **Hypothesis 1** (registrations never fire). `entries.len() == 0` at the point `NavigateCmd.execute` reads `focused_key`; `fallback_to_first` finds no `active_layer().key` (stack empty) so it returns `None`; `dispatch_command` sees `Value::Null`, matching the original `command completed cmd=nav.down result=null undoable=false` log.

### Integration-test artefact

Added a five-test suite in `kanban-app/src/spatial.rs::nav_dispatch_integration_tests` that builds a real `AppHandle<MockRuntime>`, registers three (and five, and two-windows) spatial entries via the same `spatial_register` IPC path the frontend uses, and dispatches each `nav.*` through `NavigateCmd::execute`. All five tests pass:

- `nav_down_with_focused_entry_returns_moniker_and_emits_event`
- `nav_down_without_focus_falls_back_to_first_in_layer`
- `nav_cardinal_directions_reach_neighbours_on_active_layer`
- `nav_routes_to_spatial_state_of_window_named_in_scope`
- `every_nav_direction_resolves_to_a_registered_navigate_cmd`

The tests exercise **exactly** the scenario the task describes as broken. They pass. Combined with the absent-IPC evidence above, that pins the defect to the React side: the Rust pipeline works end-to-end when the entries are registered, so the registrations themselves are what goes missing in the running app.

### Bail-out — stopping before surgical patching

Per the task's bail-out criterion: `layer_stack` is empty, entries don't exist, yet `FocusLayer` / `FocusScope` are visibly mounted in the React tree (the scope chain includes `task:01KMANBMZY49P0WVVVK443TVS8` from a click, which only populates if `useScopeRegistration` ran — meaning the component tree DID render). **That's structural corruption, not a surgical bug.** I'm stopping here and leaving the integration-test regression suite + new permanent `tracing::info!` in place so the next pass has both the failing-shape test and the observability to actually watch what registration does (or doesn't do) on the React side — the next diagnostic pass needs to focus on why `useRectObserver` never fires or why its `spatial_register` invoke never lands, NOT on patching Rust.

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
- [ ] Full `__spatial_dump` output is pasted into this task's description under a dated `## Diagnostic Evidence` heading (evidence gate — see Workflow)
- [ ] Instrumentation after the fix — keep the Rust `tracing::info!` in `spatial_register` with (key, moniker, layer_key); remove the JS `console.warn("[FocusScope] register …")` and `console.warn("[FocusLayer] push …")` breadcrumbs (high-volume, log-polluting)

## Tests

- [ ] New: `kanban-app/tests/nav_dispatch_integration.rs` — mount a real `AppHandle` + `AppState`, register **at least three** spatial entries, dispatch `nav.down` via `commands::dispatch_command`, assert the result is a moniker string (not `Value::Null`) AND a `focus-changed` event was emitted with the expected `next_key`
- [ ] Second case: `focused_key = None` at dispatch time, verify fallback-to-first fires and emits an event (this protects the app-boot-before-first-click path)
- [ ] Third case: entries registered with a layer key that matches the active layer — explicitly assert the pool filter doesn't cull them
- [ ] Run `cargo test -p kanban-app nav_dispatch_integration` — new tests green
- [ ] Run `cargo test -p swissarmyhammer-spatial-nav -p swissarmyhammer-kanban -p kanban-app` — all existing tests still green
- [ ] Run `cd kanban-app/ui && npm test` — all 1401 tests still green
- [ ] **Live verification** (per `always-verify` memory — do not skip): run the app, press `h/j/k/l`, confirm focus moves. Capture `log show --predicate 'subsystem == "com.swissarmyhammer.kanban"' --last 2m` output showing the full round-trip AND the `spatial_state.entries` count > 1. This is a manual check because no existing automated harness covers the full Tauri IPC + React listener path end-to-end (noted gap in the integration-test section above).

## Workflow

- Use `/tdd` but **start with the dump, not the test**. The first action is: build the app, click a card, dump `SpatialState`. What you see determines which hypothesis (1, 2, or 3) is real and which file the fix lands in.
- **Evidence gate:** before you write any fix, paste the full `__spatial_dump` output into this task's description under a `## Diagnostic Evidence (YYYY-MM-DD HH:MM)` heading. No fix commits without the evidence captured in the task.
- Only once the hypothesis is confirmed, write the failing integration test that reproduces the same shape, then fix, then re-run live.
- Per `always-verify` memory: check the macOS log yourself, do not ask the user to verify. Per `frontend-logging`: use `console.warn` for any JS instrumentation, read the OS log via `log show`.
- Remove JS `console.warn` breadcrumbs from production code before closing the task. Keep the Rust `tracing::info!` entry in `spatial_register` (key, moniker, layer_key) — low-volume, high-diagnostic-value, fires at mount only.
- **Bail-out:** if the dump reveals structural corruption rather than a surgical bug (see "Bail-out criterion" above), stop. Revert the files tied to the broken hypothesis and re-plan. Do not patch on top of a broken foundation.

## Review Findings (2026-04-22 21:15)

### Scope reviewed
The uncommitted changes to `kanban-app/src/spatial.rs` (~500 lines added — permanent `tracing::info!` on `spatial_register`/`spatial_push_layer`/`spatial_remove_layer`, generic `TauriSpatialNavigator<R: Runtime>`, exposed `tauri_integration_tests` helpers, and a new `nav_dispatch_integration_tests` module with five tests) plus the description update capturing the `## Diagnostic Evidence (2026-04-22 15:55)` section.

### Acceptance-contract status
The task is **not complete**. The primary user-facing acceptance criterion — "Pressing `h`/`j`/`k`/`l` ... in the running app produces a visible focus change" — is explicitly not met; the implementer's own evidence confirms `spatial_register` is never invoked from React in production. The bail-out clause the implementer invoked is legitimate (structural corruption, not a surgical bug), but invoking bail-out is a reason to re-plan and continue, not a reason to declare the task done. A review gate that lets an unmet user-facing acceptance criterion ship silently is a broken gate.

### Blockers

- [ ] `kanban-app/src/spatial.rs:bail-out` — Acceptance criterion "Pressing `h`/`j`/`k`/`l` produces a visible focus change" is unmet. Evidence (zero `spatial_register` / `spatial_push_layer` traces during app load) confirms Hypothesis 1 but no fix landed. Task must stay in `review` and get a follow-up pass that completes Hypothesis 1 — locate why `useRectObserver`'s `invoke("spatial_register", ...)` and `useLayerKeyAndPush`'s `invoke("spatial_push_layer", ...)` never reach Rust in production despite both being statically present in `focus-scope.tsx:132` and `focus-layer.tsx:45`.

### Warnings

- [ ] `kanban-app/ui/src/components/focus-layer.tsx:43-49` — First suspect for the Hypothesis 1 root cause: `useLayerKeyAndPush` calls `invoke("spatial_push_layer", …)` inside a `useState` initializer. React's Strict Mode double-invokes state initializers in development, and some bundler / HMR paths can also re-run them. If the initializer is running during a render that gets thrown away (Strict Mode, concurrent rendering, suspense), the `.catch(() => {})` swallows every Tauri IPC error silently — including "invoke is not available yet because the webview hasn't finished loading." The evidence log shows the webview reached `UserActivity::Impl::endActivity` but no spatial IPC ever fires, which is exactly the shape of this failure mode. Instrument both `useLayerKeyAndPush` and `useRectObserver` with a `console.warn` breadcrumb AND remove the bare `.catch(() => {})` so any real error surfaces in the OS log.

- [ ] `kanban-app/ui/src/components/focus-scope.tsx:132-144` — `invoke("spatial_register", { args: {...} })` is wrapped in `.catch(() => {})`. Per `always-verify` / `frontend-logging` memory rules, swallowing invoke errors silently is how this class of bug stays invisible. At minimum, log the error via `console.warn`; ideally surface it to the user. The current absence of any log entry for `spatial_register` during app load could be the invoke throwing (module not ready, command not registered under that exact name, serde shape mismatch) and nobody would know.

- [ ] `kanban-app/src/spatial.rs:nav_dispatch_integration_tests` — The integration tests are an excellent regression fence for the Rust half of the pipeline, but they share a known gap: they invoke Rust commands via `app.state::<AppState>()` directly rather than through the actual Tauri IPC envelope that the webview uses. Since the confirmed defect lives on the IPC boundary itself (React invoke calls never arrive), this test suite cannot catch the bug it was introduced to prevent from silently regressing again. Add a comment at the top of the `nav_dispatch_integration_tests` module spelling out that gap, and leave an open kanban task for "Tauri IPC harness that exercises `invoke()` from a headless webview" so the gap is tracked.

- [ ] `kanban-app/ui/src/components/focus-scope.tsx:98-103` — `useSpatialKeyBinding` does `registerSpatialKey(spatialKey, moniker)` in a `useEffect`, then `useRectObserver` (separate effect, same `layerKey` dep) calls `invoke("spatial_register", …)`. If `layerKey` is `null` at the moment the effect runs (because `FocusLayerContext.Provider` hasn't mounted yet, or mounted with a stale value because the `useState` initializer in its parent is being re-invoked by Strict Mode), the `if (!layerKey || !spatial) return;` guard at line 125 skips registration silently. Verify under React 18 Strict Mode whether the evidence-log "no spatial_register, ever" reproduces, because Strict Mode's double-mount pattern can leave the first render's cleanup fighting the second render's setup in ways that register/unregister pairs never complete.

- [ ] `kanban-app/src/spatial.rs:530-542` — Making `TauriSpatialNavigator` generic over `Runtime` is correct and well-documented, but `TauriSpatialNavigator<R: Runtime = tauri::Wry>` uses a default type parameter on a non-public type. If any consumer outside this file was constructing `TauriSpatialNavigator` via `::new(...)` before this change, the default means the existing call sites keep compiling — but any `impl`/`Arc<dyn>` site that named the type explicitly would now error. Verify `TauriSpatialNavigator` is referenced only inside this file (grep confirmed its `pub(crate)` visibility) and note in the doc-comment that the default `= tauri::Wry` exists so production code keeps reading cleanly.

### Nits

- [ ] `kanban-app/src/spatial.rs:nav_cardinal_directions_reach_neighbours_on_active_layer` — the five-entry cross-layout is inlined; `register_three_entries` is a helper for the 3-entry case. Consider extracting a `register_cross_layout` helper too, since the same geometry is likely to appear in future regression tests. Minor — only worth doing if a third consumer appears.

- [ ] `kanban-app/src/spatial.rs:nav_routes_to_spatial_state_of_window_named_in_scope` — the test asserts `b_state.focused_key() == Some("k2")`, which is correct, but a stronger assertion would also verify zero `focus-changed` events fired on window B's listener (currently only window A's event capture is wired up). Add a `capture_focus_events_on_window(&app, "B")` and assert `b_events.lock().unwrap().is_empty()` to pin down that cross-window routing truly stays isolated.

- [ ] `.kanban/tasks/01KPVDA8NYFFQ8R1D2G9YEATJ3.md` — the description update moved the task from `todo` to `review` before the acceptance criteria were satisfied. This is the symptom being addressed by this review (column-movement is the verdict; moving to `review` with unmet acceptance criteria forces a re-review pass). Not a code nit — a workflow nit for the next iteration: the bail-out criterion in a task is a signal to re-plan, not to advance the task to `review`.

### Directive to the next implementer

The next pass should:

1. **Complete Hypothesis 1**: locate why `useRectObserver`'s `invoke("spatial_register", ...)` at `focus-scope.tsx:132` and `useLayerKeyAndPush`'s `invoke("spatial_push_layer", ...)` at `focus-layer.tsx:45` do not reach Rust when the app boots in production. The static code path exists; something at the IPC / bundler / React-strict-mode / module-load-order layer is swallowing the call. Remove the bare `.catch(() => {})` swallows first so the real error surfaces.

2. **Do NOT revert** the permanent `tracing::info!` instrumentation or the `nav_dispatch_integration_tests` module. Both are load-bearing observability / regression fencing and should ship even if the eventual fix lands in React.

3. **Add a temporary `console.warn` breadcrumb** in both `useRectObserver` (just inside the effect body, before the guard clause) and `useLayerKeyAndPush` (just inside the `useState` initializer). Read the macOS log yourself via `log show --predicate 'subsystem == "com.swissarmyhammer.kanban"'` after a fresh app launch — per `frontend-logging` memory, never ask the user to check the browser console.

4. **Evidence gate on the fix**: once `h/j/k/l` produce visible focus movement, capture `log show` output showing the full round-trip (`spatial_push_layer → N × spatial_register → dispatch_command cmd=nav.down → spatial_state.navigate result=Some → emit focus-changed → [focus-changed] moniker=...`) and paste it into this task's description under a fresh `## Live Verification (YYYY-MM-DD HH:MM)` heading.

5. **Remove the `console.warn` breadcrumbs** before closing, per the existing acceptance criterion. Keep the Rust `tracing::info!` entries.
