---
assignees:
- claude-code
position_column: todo
position_ordinal: d480
project: ui-command-cleanup
title: 'Harden per-window op resolution: no silent default-to-main when the window: moniker is missing'
---
## What
The redundant `window_label` op parameter is ALREADY removed and per-window `ui_state` ops resolve the window from the scope chain's `window:<label>` moniker (commit `a2002c330`, 2026-06-05). This card is the REMAINING gap, not a duplicate.

`window_from_scope` in `crates/swissarmyhammer-ui-state/src/service.rs:60-65` ends in a **silent fallback**:
```rust
fn window_from_scope(scope_chain: &[String]) -> &str {
    scope_chain.iter()
        .find_map(|m| m.strip_prefix("window:"))
        .unwrap_or(DEFAULT_WINDOW_LABEL) // "main"
}
```
If any caller ever sends a scope chain that lacks a `window:` moniker, every per-window op silently writes to the `main` slot — which is the EXACT regression `a2002c330` fixed (palette/inspector state written to a window no board reads: `:` never opened the palette, inspect never opened the inspector). Today nothing guards against it; it would fail invisibly with no error and no test catching it.

## Goal
Make a missing `window:` moniker a loud, detectable failure on the per-window op path instead of a silent default-to-main, and add a systemic guarantee that per-window ops always carry the window.

## Approach (decide during impl)
- Change `window_from_scope` to return `Option<&str>` (or `Result`) for the **per-window mutation ops** (Inspect, InspectorClose/CloseAll/SetWidth, PaletteOpen/Close, ShowCommand/Palette/Search, Dismiss, StartRename, SetActiveView, SetAppMode). On `None`, return an rmcp error from `call_tool` rather than mutating `main`.
- Keep a tracing `error!`/`warn!` so the silent-main case is observable in the unified log (`subsystem == "com.swissarmyhammer.kanban"`).
- Confirm whether any LEGITIMATE caller has no window context (e.g. a global/no-board op). If such ops exist, they should not be per-window ops at all — split them so the per-window set is uniformly window-required.
- Verify the frontend always includes the `window:` moniker in the scope chain it sends (the producer side — `FocusedScopeContext` / `ui.setFocus` push). If focus can sit at a layer above the window moniker, fix the producer so the chain always roots at `window:<label>`.

## Acceptance Criteria
- [ ] A per-window op invoked with a scope chain lacking a `window:` moniker returns an error (or is otherwise rejected) — it does NOT mutate the `main` window.
- [ ] The silent `unwrap_or("main")` default is gone from the per-window mutation path (read-only/global helpers may keep an explicit, documented default if justified).
- [ ] The frontend-produced scope chain for per-window dispatches always contains a `window:<label>` moniker (producer-side guarantee), verified by a test.

## Tests
- [ ] `swissarmyhammer-ui-state` service test: drive each per-window op with a scope chain that has NO `window:` moniker → assert it errors and that neither the `main` slot nor any window slot was mutated.
- [ ] Positive test (already exists per a2002c330 — keep/extend): a non-"main" `window:` moniker flips the correct window's state and nothing lands on `main`.
- [ ] Frontend/producer test: the scope chain sent for a per-window command includes the `window:` moniker.
- [ ] Regression: fails before the hardening (silent main write), passes after.

## Workflow
- Use `/tdd` — write the missing-moniker rejection test first, then change the resolver.

## Related
- Builds on `a2002c330` (scope-chain window resolution).
- Connected to the focus/scope_chain authority question — if `scope_chain` is frontend-authoritative (`FRONTEND_AUTHORITATIVE_KINDS`), the producer guarantee above is where the window moniker must be enforced. #tech-debt