---
assignees:
- claude-code
depends_on:
- 01KPZWP4YTYH76XTBH992RV2AS
- 01KPZPY5F5HPXDKKHGKDEW6FNZ
position_column: todo
position_ordinal: ff8e80
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
- [ ] Write the failing nav-is-event-driven regression test.
- [ ] Run the test on `main` and on `kanban`; list every `dispatch_command` IPC and `invoke(...)` call observed during nav.
- [ ] Fix violators (may require `depends_on: 01KPZPY5F5HPXDKKHGKDEW6FNZ` for the perspective one).
- [ ] Add contract comment to `useFieldValue` and to the entity-event handlers.
- [ ] Manual smoke: 30 arrow-key presses in the 2000-row swissarmyhammer board while watching `log show --predicate 'subsystem == "com.swissarmyhammer.kanban"'` — confirm the only per-nav backend `cmd=` line is `ui.setFocus`.

## Acceptance Criteria
- [ ] New regression test passes: arrow-key nav in a mounted `GridView` produces ZERO `perspective.list` / `list_entities` / `get_entity` / `get_board_data` IPCs.
- [ ] Emitting a mock `entity-field-changed` event for a seeded entity's field causes exactly that field's `useFieldValue` subscriber to fire — no sibling cells re-render.
- [ ] Manual smoke: `log show ...` during sustained arrow-key nav on the 2000-row swissarmyhammer board shows only `cmd=ui.setFocus` lines per keystroke — no `cmd=perspective.list`, no `cmd=list_entities`, no `cmd=get_entity`.
- [ ] Nav-bar progress bar does not relight on arrow-key nav once initial load has settled (already-stated acceptance from the dependency task; restated here as the user-facing signal that this contract holds).
- [ ] No change to the event-driven update path: `entity-created` / `entity-field-changed` / `entity-removed` still patch the store in place via `handleEntityCreated` / `handleEntityFieldChanged` / `handleEntityRemoved`.

## Tests
- [ ] `kanban-app/ui/src/components/grid-view.nav-is-eventdriven.test.tsx` — new test asserting no fetch IPCs fire on nav, plus the per-field subscriber assertion.
- [ ] Test command: `cd kanban-app/ui && npm test -- grid-view.nav-is-eventdriven`. Expected: green.
- [ ] Run full UI test suite to confirm no regression: `cd kanban-app/ui && npm test`. Expected: green.
- [ ] Manual smoke described above in Acceptance Criteria.

## Workflow
- Use `/tdd` — the failing regression test drives both the violators' fixes and the contract itself.
- Depends on **01KPZPY5F5HPXDKKHGKDEW6FNZ** for the perspective.list refetch fix; prefer landing that first, then this as the enforcement layer. #performance #events #architecture #frontend