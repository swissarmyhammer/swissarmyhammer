---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffb80
title: Focus markers not showing in the second window of the same board — likely focus-changed window_label mislabel exposed by the new window filter
---
## What

LIVE BUG (user-observed, ping-pong from `01KTQ3J9SDV7GBJ1XHZN1T2GRE` / `01KTQ6QZNB3VN4MAND7VPASM21`): with two windows open on the SAME board (swissarmyhammer board), focus markers/boxes no longer render in the SECOND window. The uncommitted working-tree fixes added (a) a window filter on the webview's `focus-changed` listener (`apps/kanban-app/ui/src/lib/spatial-focus-context.tsx` — drops events whose `window_label` ≠ this window's label) and (b) the `ui_focus_owned_by_window` ownership check in `crates/swissarmyhammer-focus/src/server.rs`.

**Leading hypothesis**: the strict filter exposed a latent mislabel. Before the filter, the second window rendered focus because it accepted EVERY window's events (including its own, however labeled). Now, if any focus-changed event destined for the second window carries the WRONG `window_label` (e.g. the first window's label, or a label derived from something board-scoped instead of the committed FQ's root segment), the second window drops its own events → no markers. The first window keeps working because its label happens to match.

## Diagnose via the live log FIRST (never ask the user)

```
log show --predicate 'subsystem == "com.swissarmyhammer.kanban"' --last 30m | grep -iE 'focus-changed|window_label|focus_from|reconcile|emit|drop|ignor'
```

Find a focus commit initiated IN the second window (click/jump there) and trace: (1) what FQ was committed (root segment = second window's label?), (2) what `window_label` the emitted FocusChangedEvent carried, (3) whether the second window's webview received and accepted or dropped it. Add temporary console.warn instrumentation in the webview if needed (frontend logging goes to the unified log).

## Suspects (in order)

1. **`FocusChangedEvent.window_label` minting** in the kernel/server (`crates/swissarmyhammer-focus/src/state.rs` `navigate`/`focus`/`focus_from`/`reconcile_slot`, event construction + `forward_event` in `server.rs`): is the event's `window_label` derived from the committed FQ's ROOT SEGMENT (correct, per the fq-chain rule) or from something stale (the request's `window` arg, a slot key, a layer side field, the previous focus's window)? Any clear/blur event for the OLD focus when focus moves WITHIN one window — or when a jump clears prior focus — must also carry the right label for the window that owns the cleared FQ.
2. **Event fan-out** (`emit_to` targets in the Tauri layer, `apps/kanban-app/src/commands.rs` / wherever focus events bridge to webviews): are focus-changed events emitted to the specific owning window, all windows, or just one? If emitted only to ONE window per board (or to the window that issued the command rather than the FQ's owner), the second window never receives its events at all (filter irrelevant).
3. **The webview filter comparison** (`spatial-focus-context.tsx`): exact string compare of `event.window_label` vs `getCurrentWindow().label` — verify the event payload field name/shape matches what the filter reads (a typo'd field path makes the filter drop EVERYTHING — but then the first window would break too, so less likely).
4. **Registry/layer state in the second window**: if the second window's layer push or scope registration regressed, markers can't render even with correct events — check for "unregistered layer" warnings in the log.

## Required outcome

Every FocusChangedEvent carries the `window_label` of the window that OWNS the affected FQ (its root segment) — minted from the fully-qualified chain, never from the request arg or side state — and is delivered to that window. The webview filter stays (it is correct); fix the labeling/routing so legitimate events pass it. No special cases.

## Acceptance Criteria
- [ ] Two windows, same board: focus markers render correctly in BOTH windows; clicking/jumping/navigating in either window shows focus in that window only
- [ ] Three-window configuration (two same board + one different) from the prior task still works: drill/Escape and focus in all three
- [ ] Root cause named with log evidence: the exact mislabeled/misrouted event identified
- [ ] FocusChangedEvent.window_label provably equals the affected FQ's root segment in all paths (focus, navigate, drill, jump-clear, blur/clear of prior focus)

## Tests
- [ ] Rust test pinning the invariant: every event emitted by focus/navigate/drill/clear paths carries window_label == affected fq.root_segment() — including the cross-window jump case where the PRIOR focus in window A is cleared while new focus commits in window A (and any same-window move emitting clear+set). Must FAIL on current code if the mislabel is in the kernel/server; place in crates/swissarmyhammer-focus tests.
- [ ] If the bug is in event fan-out (Tauri bridge), a test at that seam proving focus events for window X are emitted to window X (pattern: the existing emit_to loop tests for ui-state-changed in apps/kanban-app/src/commands.rs, if any).
- [ ] If the bug is the webview filter/payload shape, a vitest in spatial-focus-context.responders.test.tsx proving the second window accepts its own correctly-labeled events (and the payload field path matches production).
- [ ] `cargo nextest run -p swissarmyhammer-focus` and `-p swissarmyhammer-command-service` green (two pre-existing keybinding e2e failures, card 01KTPDTH772HSEV5F7R1DKYDNJ, out of scope; 2 pre-existing vitest failures on card 01KTQ8KRJYX1DPHN76TZ654ZX2 out of scope); touched vitest files green.

## Constraints
- NO whole-workspace cargo build/clippy/run — `tauri dev` hot-reloads; crate-scoped nextest only.
- Read the unified log yourself; instrument with console.warn/tracing if needed (tracing crate on Rust side, never eprintln).
- Window identity from the fully-qualified scope chain ONLY. Keep the filter and ownership check; fix the labeling, not the guards.
- Do NOT revert the uncommitted working-tree fixes from the three prior tasks — build on top.

## Workflow
- Use `/tdd` — reproduce with a failing test at the seam the log implicates, then fix.