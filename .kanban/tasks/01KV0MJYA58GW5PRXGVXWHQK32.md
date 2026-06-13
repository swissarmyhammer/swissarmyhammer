---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffab80
title: Perspective filter change doesn't refresh the view until click-away/back — needs event-driven debounced refresh on filter change
---
## What

LIVE BUG (user-observed 2026-06-13): when the perspective filter is edited, the board view does NOT re-filter until the user clicks away and back. The refresh must be driven by a backend event when the filter changes — **debounced** — not hardwired into the UI.

## Expected

- Editing a perspective's filter recomputes `filtered_task_ids` and the view updates automatically (debounced), with no click-away/back required.
- The refresh is **event-driven**: the filter mutation (a `perspective.*` filter command) produces a change/event that flows through the existing `{ok,change}` → `ui-state-changed` per-window emit path, and the UI re-renders off that event — NOT off a hardcoded UI side-effect or polling.

## Design / investigate (LIKELY SAME ROOT CAUSE as the switch/next/prev/delete fixes)

- This is almost certainly the same architecture seam just fixed for perspective switch/next/prev (01KTYQY0ZB62KHN6BPK3FBMBD7) and delete (01KTYVSA68WDFGXCEJ44T4VFNW): those commands had been routed to the **views server's resolution-only ops** which compute a result and discard it (no `UIState`, no emit). The fix moved them onto the **entity server** (`crates/swissarmyhammer-entity-mcp`) which holds `KanbanContext + UIState`, so they recompute `filtered_task_ids` for the active window and return `{ok, change}` → the host emits `ui-state-changed` per window. **The perspective FILTER command is very likely the last one still on that dead views path.** Check `builtin/plugins/perspective-commands` filter command routing FIRST.
- If so: route the filter mutation through the entity server (or wherever the recompute+emit happens), recompute `filtered_task_ids`, return the change so the per-window emit fires. Reuse the shared filter-eval + `filtered_task_ids` write path and `perspectiveVisibleInView`.
- **Debounce** belongs on the filter INPUT (rapid keystrokes → one settled dispatch, not one per character). Prefer debouncing the dispatch at the filter editor (presentation-layer), then the backend emit drives the refresh — trailing-edge coalesce. Decide and document.

## Acceptance Criteria
- [ ] Editing the active perspective's filter updates the view automatically (debounced), no click-away/back
- [ ] The refresh is driven by a backend change/event (`ui-state-changed`) via the per-window emit path — not a UI-local hack or poll
- [ ] Rapid typing produces a debounced/coalesced refresh, not one dispatch per keystroke
- [ ] Other windows on the same board re-filter from the same event
- [ ] Root cause documented (was the filter command on the views resolution-only path like switch/delete were?)

## Tests
- [ ] vitest red-first: filter edit → debounced dispatch → view re-renders off the emitted change (red: requires click-away)
- [ ] vitest: rapid typing coalesces to one (or trailing) dispatch, not per-keystroke
- [ ] e2e through the real plugin: the filter command returns `{ok,change}` and recomputes `filtered_task_ids`
- [ ] existing perspective switch/delete/select suites stay green
- [ ] `cargo nextest -p swissarmyhammer-command-service` (and `-p swissarmyhammer-entity-mcp` / `-p swissarmyhammer-views` if the command moves crates) green except carded meta_tree; tsc + touched vitest green

## Constraints
- NO whole-workspace cargo build/clippy; no kanban-app crate compile. Never touch .kanban/actors/wballard.jsonl.
- Reuse existing change-emit + filter-eval machinery; presentation-layer debounce, event-driven refresh.

## Workflow
- /tdd — failing test first at the seam the routing investigation implicates.