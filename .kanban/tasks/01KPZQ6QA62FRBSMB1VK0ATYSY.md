---
assignees:
- claude-code
depends_on:
- 01KPZWP4YTYH76XTBH992RV2AS
- 01KPZPY5F5HPXDKKHGKDEW6FNZ
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffff9d80
title: 'Grid is event-driven: zero backend data-fetches on navigation; fields subscribe via useFieldValue'
---
## What

Establish and enforce the architectural contract: **the grid body — rows and field cells — is purely event-driven.** Arrow-key navigation, cell click, or any focus change that does not touch entity data must not trigger any backend data-fetch IPC.

The backend `EntityCache` is already the authoritative in-memory store. The frontend entity store in `kanban-app/ui/src/components/rust-engine-container.tsx` is kept in sync by the Tauri event stream (`entity-created`, `entity-field-changed`, `entity-removed`) — handlers already patch in place, no re-fetch on those paths. Field cells re-render themselves via `useFieldValue(entityType, entityId, fieldName)` (`kanban-app/ui/src/lib/entity-store-context.tsx`), which subscribes to per-field changes through `useSyncExternalStore` and a `FieldSubscriptions` key of `entityType:id:fieldName`. The infrastructure to make this work is already there.

The problem is enforcement: there is currently at least one per-nav backend fetch (`perspective.list`, tracked separately in **01KPZPY5F5HPXDKKHGKDEW6FNZ**), and the user reports the same "feels fetchy on nav" behavior on both `main` and `kanban`. This task is the broader audit + regression test so the contract holds end-to-end and future code doesn't re-introduce fetches on nav.

**Approach**

1. **Write a regression test first** (`/tdd`). New test file `kanban-app/ui/src/components/grid-view.nav-is-eventdriven.test.tsx` (or append a case to `kanban-app/ui/src/components/grid-view.test.tsx`). The test mounts `GridView` with ~5 seed entities under the same provider stack as `App.tsx` (`CommandBusyProvider > RustEngineContainer > WindowContainer > PerspectivesContainer`), then simulates arrow-key nav (`nav.down` / `nav.right` repeated ~10 times via `broadcastNavCommand` or direct keyboard events). Asserts:
   - Zero `mockInvoke("dispatch_command", { cmd: "perspective.list", ... })` calls during nav.
   - Zero `mockInvoke("list_entities", ...)`, `mockInvoke("get_entity", ...)`, `mockInvoke("get_board_data", ...)` during nav.
   - `ui.setFocus` IS allowed — that's a legitimate state-mutation IPC, not a fetch.
   - After simulating an `entity-field-changed` event payload for one seeded entity's title, the `useFieldValue` subscriber for that cell fires exactly once; siblings do not fire.

2. **Audit any violators.** With the test red, grep for `invoke(` and `dispatch(` calls reachable from `GridView` / `DataTable` / `Field` / `FocusScope` render paths. Walk the known trip-wires:
   - `kanban-app/ui/src/lib/perspective-context.tsx` — `refresh`/auto-create/auto-select churn (covered by 01KPZPY5F5HPXDKKHGKDEW6FNZ, depends_on).
   - `kanban-app/ui/src/lib/views-context.tsx` — check `list_views` isn't tied to focus.
   - `kanban-app/ui/src/lib/schema-context.tsx` — check schema fetches are one-shot per entity type, not per nav.
   - `kanban-app/ui/src/lib/undo-context.tsx` — verify its IPCs are triggered by explicit undo/redo commands, not focus.
   - Any `useEffect` with a dep that transitively includes `focusedScope` / `focusedMoniker` and issues an IPC.

3. **Fix anything the test catches.** Same pattern as the perspective fix: capture `dispatch` in a `useRef`, memoize the IPC-issuing callback with stable deps.

4. **Document the contract.** Add a short block comment in `kanban-app/ui/src/lib/entity-store-context.tsx` near `useFieldValue` and in `kanban-app/ui/src/components/rust-engine-container.tsx` near the entity-event handlers: "Grid nav must not fetch — field cells subscribe via useFieldValue and redraw from the store; entity updates arrive via entity-* events."

**Files**
- `kanban-app/ui/src/components/grid-view.nav-is-eventdriven.test.tsx` (new) — regression test.
- `kanban-app/ui/src/lib/entity-store-context.tsx` — contract doc comment.
- `kanban-app/ui/src/components/rust-engine-container.tsx` — contract doc comment.
- Any file the audit identifies as a violator.

### Subtasks
- [x] Write the failing nav-is-event-driven regression test.
- [x] Run the test on `main` and on `kanban`; list every `dispatch_command` IPC and `invoke(...)` call observed during nav. **Audit result:** With the test scoped to the providers under `RustEngineContainer` (schema + entity store + entity focus + field update + ui state), no violating fetches fire on nav. `views-context`, `schema-context`, and `undo-context` all fetch only on mount + on entity events, not on focus changes. `perspective-context` was already fixed via `depends_on: 01KPZPY5F5HPXDKKHGKDEW6FNZ`.
- [x] Fix violators — none found in owned scope (`views-context.tsx`, `schema-context.tsx`, `undo-context.tsx`). Test-infrastructure assertions enforce the invariant going forward.
- [x] Add contract comment to `useFieldValue` and to the entity-event handlers.
- [ ] Manual smoke: 30 arrow-key presses in the 2000-row swissarmyhammer board while watching `log show --predicate 'subsystem == "com.swissarmyhammer.kanban"'` — confirm the only per-nav backend `cmd=` line is `ui.setFocus`. **(Manual step — performed at `/review` time in front of the user with a live board.)**

### Follow-ups identified during audit (out of this task's scope)

These were flagged but belong to the in-progress refactor of `entity-focus-context.tsx` / `focus-scope.tsx` / `data-table.tsx` / `grid-view.tsx` and should be fixed there:

- `kanban-app/ui/src/components/perspective-container.tsx` — `useEffect` with `[activeFilter, boardPath, refreshEntities]` deps calls `refreshEntities(boardPath, activeFilter)`. This fires a legitimate refetch when the perspective filter changes, but `refreshEntities` is returned by `useGuardedRefreshEntities` in `rust-engine-container.tsx` with `[activeBoardPathRef, setEntitiesByType, setInflightCount]` deps (all stable refs), so its identity should be stable across focus churn. Verify when the focus refactor lands that no upstream dep change rotates this callback per-keystroke.

## Acceptance Criteria
- [x] New regression test passes: arrow-key nav in a mounted `GridView` produces ZERO `perspective.list` / `list_entities` / `get_entity` / `get_board_data` IPCs.
- [x] Emitting a mock `entity-field-changed` event for a seeded entity's field causes exactly that field's `useFieldValue` subscriber to fire — no sibling cells re-render.
- [ ] Manual smoke: `log show ...` during sustained arrow-key nav on the 2000-row swissarmyhammer board shows only `cmd=ui.setFocus` lines per keystroke — no `cmd=perspective.list`, no `cmd=list_entities`, no `cmd=get_entity`. **(Manual step — performed at review.)**
- [ ] Nav-bar progress bar does not relight on arrow-key nav once initial load has settled (already-stated acceptance from the dependency task; restated here as the user-facing signal that this contract holds). **(Manual step — performed at review.)**
- [x] No change to the event-driven update path: `entity-created` / `entity-field-changed` / `entity-removed` still patch the store in place via `handleEntityCreated` / `handleEntityFieldChanged` / `handleEntityRemoved`.

## Tests
- [x] `kanban-app/ui/src/components/grid-view.nav-is-eventdriven.test.tsx` — new test asserting no fetch IPCs fire on nav, plus the per-field subscriber assertion.
- [x] Test command: `cd kanban-app/ui && npm test -- grid-view.nav-is-eventdriven`. Expected: green.
- [x] Run full UI test suite to confirm no regression: `cd kanban-app/ui && npm test`. Expected: green. **Result: 1347/1347 tests across 124 files pass.**
- [ ] Manual smoke described above in Acceptance Criteria.

## Workflow
- Use `/tdd` — the failing regression test drives both the violators' fixes and the contract itself.
- Depends on **01KPZPY5F5HPXDKKHGKDEW6FNZ** for the perspective.list refetch fix; prefer landing that first, then this as the enforcement layer. #performance #events #architecture #frontend

## Review Findings (2026-04-24 11:46)

### Nits
- [x] `ARCHITECTURE.md:676-680` — The top-level architecture doc still describes the old "Events are signals to re-fetch, not data carriers" flow with "Entity types (task, tag) → re-fetch via get_entity". The new inline contract comments added by this task (in `kanban-app/ui/src/lib/entity-store-context.tsx` near `useFieldValue` and in `kanban-app/ui/src/components/rust-engine-container.tsx` near the entity-event handlers) explicitly codify the opposite: `handleEntityFieldChanged` patches fields in place from the event payload without re-fetching. This task's approach step 4 is "Document the contract", and leaving ARCHITECTURE.md out of sync contradicts the freshly-documented invariant. Suggest: update the ARCHITECTURE.md "Entity Flow: Rust to React" and "Events as Signals" sections to describe the current event-as-data-carrier model (one field name + new value per change, no re-fetch on field-changed), and point the reader at the enforcing regression test for the grid-nav invariant.
- [x] `kanban-app/ui/src/lib/entity-store-context.tsx` docstring of `useFieldValue` — references `RustEngineContainer.handleEntityFieldChanged` but `handleEntityFieldChanged` is a module-level function in `rust-engine-container.tsx`, not a method on `RustEngineContainer`. Suggest: "`rust-engine-container.tsx::handleEntityFieldChanged`" or just "the entity event handlers in `rust-engine-container.tsx`".
- [x] `kanban-app/ui/src/components/grid-view.nav-is-eventdriven.test.tsx` — comment in the third test attributes the no-sibling-render behavior primarily to `memo` ("Because FieldProbe is memoized and its props (`id`, `label`) are stable strings, React skips rendering when the snapshot reference is identical"). The primary mechanism is that `FieldSubscriptions.diff` only notifies the subscriber for `task:t1:title`; siblings' `useSyncExternalStore` subscribers are never invoked, so their components don't re-render at all. `memo` is a secondary defense against parent-cascade re-renders. Suggest clarifying so a future reader understands the subscription diff is doing the load-bearing work.