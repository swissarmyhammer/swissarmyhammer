---
assignees:
- claude-code
position_column: todo
position_ordinal: e680
project: spatial-nav
title: 'Rebind ui.inspect Enter→Space — LIVE BROKEN: Space scrolls the grid instead of inspecting'
---
## What

Migrate `ui.inspect` from Enter to Space. Unit tests passed, live app does not.

## Live-app failures (2026-04-23 — user reports two concrete reproductions)

### Bug A — Grid: Space scrolls instead of inspecting

Focus a cell or row selector in the data-table grid. Press Space. The grid scrolls down. The inspector does not open. **Space's browser-default action is page-scroll; our keybinding handler is not intercepting it before the browser does.**

Hypothesis: `createKeyHandler` in `kanban-app/ui/src/lib/keybindings.ts` attaches its listener on `document` (or wherever the app-shell handler lives), not on the element that receives the Space keydown. The grid cell / row selector is a `<td>`, not a natively focusable element, so the browser's DOM focus is still on the nearest scroll container. Space fires on the scroll container and the browser handles it before our keybinding handler sees it — OR our handler sees it and resolves Space to `ui.inspect` but doesn't `preventDefault` in time / correctly.

Check `trySingleKey`'s `preventDefault` call: if it runs AFTER the browser has queued the scroll, preventDefault is too late. The correct fix is `preventDefault` at keydown before the browser's default-action phase fires — which means the listener must be on the bubble phase, on a node that's an ancestor of the keydown target.

The automated "Space on scrollable resolves and preventDefaults" test in `keybindings.test.ts` fakes this path — it calls `trySingleKey` directly, which always calls `preventDefault()`. The test doesn't verify that the keydown listener is attached to a node that receives the real browser event.

### Bug B — Board: column header doesn't drill to its cards

Focus a board column header. Press Enter. Nothing happens — focus does not move to the first card of that column.

This is exactly what the companion task `01KPX6FSPY6V15JATXMY6AGRER` is supposed to fix. That task is `BLOCKED` on this one (`01KPX6E0QPNRWZTQXGXX2MBEMV`). By moving Enter off `ui.inspect` before landing the drill-in replacement, we left column headers with Enter doing literally nothing. **The column header has no Enter binding at all** — the row-level `column.enterChildren.<id>` command was never added.

Between Bug A and Bug B, the user has no working keyboard affordance on a grid cell OR a column header. Every keyboard interaction on those scopes does either (a) nothing (Enter) or (b) the wrong thing (Space scrolls).

## Acceptance Criteria

- [ ] Pressing Space on a focused grid cell opens the inspector AND does NOT scroll the grid body
- [ ] Pressing Space on a focused row selector opens the inspector AND does NOT scroll
- [ ] Pressing Space on a focused card opens the inspector AND does NOT scroll the column
- [ ] Pressing Space on a focused toolbar Inspect button opens the board inspector
- [ ] Pressing Space on a focused column header opens the column entity inspector
- [ ] Pressing Enter on a focused column header moves focus to the first card in that column (this is the companion task's deliverable — ship both together, or alias Enter→Space as a temporary stop-gap)
- [ ] Pressing Enter on a focused grid cell / inspector field / LeftNav button / perspective tab still does its existing activation action (edit / switch)
- [ ] Automated test that would have caught Bug A: mount a scrollable container with a card inside, attach the real `createKeyHandler`, dispatch a Space keydown on the card's DOM node, assert (a) the container did NOT scroll AND (b) `ui.inspect` was dispatched
- [ ] Automated test that would have caught Bug B: focus a column header in a real board topology, press Enter, assert focus moved to the first card's moniker

## Tests (previous pass — all passed, none caught the real bug)

- [x] `keybindings.test.ts::normalizeKeyEvent(" ")` → `"Space"`
- [x] `keybindings.test.ts::Space preventDefaults when bound` — but stubs `trySingleKey` directly, doesn't exercise the real document-listener attachment
- [x] RowSelector / EntityCard / toolbar dispatch Space → `ui.inspect` in component tests — but those fake the invoke mock; no real browser scroll competition

## What needs to land in the next pass

1. **A real-browser test** (vitest-browser, not happy-dom) that mounts an `<AppShell>` with a scrollable column containing a card, focuses the card, dispatches a real keydown event with `key=" "`, and asserts the column's `scrollTop` didn't change AND `dispatch_command` was called with `ui.inspect`. If this test passes but the live app still fails, the gap between vitest-browser and Tauri's webview is the next hypothesis.

2. **The companion task's drill-in binding**, either as part of this commit or a stop-gap alias so Enter on a column header does SOMETHING (even if it's a temporary Enter→Space fallback).

3. **A check that the keydown listener runs on bubble phase at a node that's definitely an ancestor of the keydown target.** If the handler is attached on `document` but a parent captures+preventDefaults or stops propagation, we never see it. Grep the app for any `addEventListener("keydown"` other than ours — particularly in Radix components, CodeMirror editors, or the perspective bar.

## Workflow (next pass)

- Use `/tdd`. Write the browser test that fakes Bug A FIRST. If it passes, the bug is outside vitest-browser's reach (Tauri IPC timing, macOS WebKit quirks). If it fails, we have a reproducer.
- Before changing code, `grep -rn "addEventListener.*keydown" kanban-app/ui/src` to find every competing listener. The Space scroll might be native browser, not another JS listener — but rule out the JS ones first.
- If the fix requires `preventDefault()` earlier in event processing, move the `keydown` listener to capture phase on `document` so it runs before any bubble-phase scroll handler.
- Do NOT close this task by "making the automated tests pass" if the live app isn't fixed. The user's report is the definition of done.
