---
assignees:
- claude-code
position_column: todo
position_ordinal: e880
project: spatial-nav
title: 'Virtualization: separate data-virtualization from render-virtualization — nav keypress must not dispatch any data-fetch command'
---
## What

A nav keypress should produce **exactly one** Rust command round-trip: `nav.<direction>` → `SpatialState::navigate` → `focus-changed` event. Nothing else. No entity fetches, no perspective refetches, no filter re-evaluations.

Log evidence from the running app contradicts this. After a click or focus change, the macOS unified log shows repeated command dispatches per event:

```
[FocusScope] focus → task:01KP3BVSFWNWW0H2GEWZDDAMS1
command  cmd=ui.setFocus  scope_chain=...
[filter-diag] perspective REFRESH (full refetch)
command  cmd=perspective.list  ...
[filter-diag] perspective REFRESH (full refetch)
command  cmd=perspective.list  ...
```

So focus changes are driving data refetches. The user's observation is correct: **the frontend is treating data as if it were virtualized**, re-fetching on state changes that have nothing to do with data.

### Architectural principle to enforce

Render-virtualization and data-virtualization are different concerns and must be kept separate:

| Concern | What gets virtualized | Who owns it |
|---|---|---|
| **Render virtualization** | Which entity rows/cards have DOM elements mounted in the viewport | `@tanstack/react-virtual`, the UI |
| **Data virtualization** | Which entity records are present in memory | **Forbidden.** All entity data for the active board is loaded once into the frontend store. |

The rule:
1. On board switch (or initial load), `list_entities` fetches every entity for the board into the frontend entity store.
2. Entity-change events from Rust (`entity-created`, `entity-removed`, `entity-field-changed`, `board-changed`) invalidate or update the cached record(s).
3. Everything else — focus change, cursor move, perspective switch, filter edit, nav keypress — reads from the store, never from Rust.
4. On keypress: **zero** IPC calls except the single `nav.<direction>` that mutates spatial focus state. Store is already populated; React re-renders from the updated focused moniker.

The user's exact framing: "get all the data and just virtualize the row rendering. entity change events then update the in-UI data causing a rerender."

### Diagnostic first

Before fixing, measure. Run the app and capture a log of a single keypress sequence. Count the IPC invokes per keypress. The expected count is 1 (`nav.<direction>`). Any other invoke is a bug to eliminate.

**Diagnostic command:**

```bash
log show --predicate 'subsystem == "com.swissarmyhammer.kanban"' --last 1m \
  --style compact \
  | grep -E "command  cmd=|command completed  cmd="
```

Run on a manual reproduction: focus a card, press `j` once, capture output. Paste into the task's description under a `## Diagnostic Evidence (YYYY-MM-DD HH:MM)` heading, clearly labeling the range of log lines that correspond to a single keypress.

### Known offender sites to audit

Based on `grep -rn` against the frontend, these sites are likely refetching on focus change and must be audited:

1. **`kanban-app/ui/src/lib/perspective-context.tsx`** — lines 98, 104 log `[filter-diag] perspective REFRESH (full refetch)`. Review what triggers the refresh. Per comments, intended triggers are `entity-created/removed` and `board-changed` — NOT focus change. But the log shows it firing after `ui.setFocus`. Something in its effect deps is changing with focus.

2. **`kanban-app/ui/src/components/perspective-container.tsx`** — line 104 calls `refreshEntities(boardPath, activeFilter)` inside a `useEffect` with deps `[activeFilter, boardPath, refreshEntities]`. If `activeFilter` or `refreshEntities` identity changes when focus changes, this refetches. Audit for referential stability.

3. **`kanban-app/ui/src/components/rust-engine-container.tsx`** — `useGuardedRefreshEntities` at line 226, `useEntityEventListeners` at line 235. Understand what triggers `refreshEntities`. Per the file header: "on board switch" — but if it's ALSO firing on other events, that's the leak.

4. **`kanban-app/ui/src/components/filter-editor.tsx`** — filter editor body is logging `FilterEditor RENDER` on every focus change (per earlier log evidence). If the filter editor's render recomputes something that triggers a fetch, that's a site. The editor should memoize against focus-independent inputs.

5. **`kanban-app/ui/src/lib/entity-focus-context.tsx`** — `ui.setFocus` dispatch itself. Does the Rust handler do any entity work beyond updating the focused scope chain? It shouldn't. Audit the Rust `SetFocusCmd` handler.

### Fix pattern

For each offender site:

1. Identify what triggers the refetch.
2. If the trigger is focus/scope-chain related, remove the trigger — the store already has the data.
3. If the trigger is legitimately entity-related (a CRUD event), make sure it only fires on the specific event shape it needs (field-changed, created, removed) and NOT on scope-chain updates.
4. Verify any memoized selector over entity data has stable referential identity when only focus changes (not entity data).

The entity store (`rust-engine-container.tsx`'s `entitiesByType`) is the single source of truth. Nav keys must never read from Rust for entity data — they only mutate focus state in Rust.

### Files likely touched

- `kanban-app/ui/src/lib/perspective-context.tsx` — tighten refresh triggers
- `kanban-app/ui/src/components/perspective-container.tsx` — audit the refreshEntities effect
- `kanban-app/ui/src/components/rust-engine-container.tsx` — ensure `useEntityEventListeners` reacts only to entity events, not to focus events
- `kanban-app/ui/src/components/filter-editor.tsx` — memoize render to not re-run on focus change
- `kanban-app/ui/src/lib/entity-focus-context.tsx` — verify `setFocus` dispatches ONLY `ui.setFocus` (not a cascade of refetches)
- `swissarmyhammer-kanban/src/commands/ui_commands.rs` (`SetFocusCmd` handler) — ensure it returns only the scope chain, no entity work

### Regression test

Add a vitest-browser test (plus a supporting Rust integration test) that encodes the principle:

**`kanban-app/ui/src/test/spatial-nav-no-refetch.test.tsx`** (new) — mounts a board with ≥5 entities inside the fixture shell with a Tauri-boundary stub that counts invokes. The test:

1. Waits for initial load to complete (counts invokes as a baseline).
2. Focuses a cell (one expected invoke: `ui.setFocus`).
3. Resets the invoke counter.
4. Dispatches `nav.down` once.
5. Asserts **exactly one** subsequent invoke: `nav.down`. Zero `list_entities`, zero `perspective.list`, zero `refreshEntities`, zero additional `ui.setFocus`.
6. Simulates five more `nav.down` keypresses. Asserts **exactly five** invokes total, all `nav.down`, still zero data-fetch commands.

This test is the structural gate. Any future regression that re-introduces a refetch on focus change will fail this test immediately.

### Out of scope

- Paging / windowing entity data at the store layer. The user's directive is: load all entities. If a board has hundreds of thousands of entities and this becomes a scaling problem, that's a separate future concern — and solved by a different mechanism (incremental entity events from Rust as the user scrolls large record sets), not by per-keypress refetches.
- Changing the UI virtualization patterns in `column-view.tsx` or `data-table.tsx`. Those operate at the render layer and are correct in principle (once `01KPVTKZ1VGDSBB0HPYTTAHJNH` and `01KPVTP70YQRRNFYK8PP4636MV` land).
- Filter evaluation location. Filter evaluation lives backend-side (`list_entities` with filter args); that's correct. Re-running the filter on focus change is the bug — re-running on entity change is fine.

## Acceptance Criteria

- [ ] Diagnostic dump in the task description shows a single nav keypress produces exactly one Rust invoke: `nav.<direction>`
- [ ] After the fix, the macOS log shows NO `perspective.list`, `list_entities`, or other data-fetch commands firing in response to focus-change events
- [ ] Filter editor does not re-render when only focus changes (memoized)
- [ ] `useEntityEventListeners` reacts only to entity CRUD events, not to any focus/scope-chain event
- [ ] Board switch still loads entities (one-time fetch on mount / board change)
- [ ] Entity CRUD events still invalidate/update the store correctly (existing behavior preserved)
- [ ] Nav key latency measurably improves: from keypress to `data-focused` attribute on the new scope should be well under 16ms (one frame) on the happy path
- [ ] The new regression test passes; reintroducing a focus-triggered refetch causes the test to fail

## Tests

- [ ] New `kanban-app/ui/src/test/spatial-nav-no-refetch.test.tsx` — invoke-count test described above
- [ ] Rust integration test in `kanban-app/tests/` (or `swissarmyhammer-kanban/tests/`) that dispatches `ui.setFocus` via the command registry and asserts the handler does NOT touch the entity store (no read, no write)
- [ ] Update `kanban-app/ui/src/lib/perspective-context.test.tsx` (or create) to assert the perspective refresh effect fires only on `entity-created` / `entity-removed` / `board-changed` events, NOT on focus change or scope-chain change
- [ ] Run `cd kanban-app/ui && npm test` — green
- [ ] Run `cargo test -p swissarmyhammer-kanban` — green
- [ ] Manual verification: press and hold `j` on a board with many cards — focus bar moves smoothly frame-by-frame, no stutter. Log during the hold shows only `nav.down` invokes, no refetches.

## Workflow

- Use `/tdd`. Write the invoke-count test FIRST against the current code. It should fail, producing a list of every command that fires on a keypress — that list IS the diagnostic output.
- Paste the diagnostic into the task description under `## Diagnostic Evidence` before committing any fix.
- Fix one offender at a time; re-run the invoke-count test after each fix. The test stays failing until the keypress produces exactly one invoke.
- Do not paper over the symptom (e.g. "debounce the refetch"). The architecture requires that focus events never trigger fetches at all. If debouncing is the first instinct, stop and find the structural reason the fetch is being triggered.
- Leave the Rust entity store alone. This is a frontend-layer caching / subscription bug.

