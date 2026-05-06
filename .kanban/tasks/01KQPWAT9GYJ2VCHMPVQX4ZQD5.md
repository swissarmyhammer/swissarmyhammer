---
assignees:
- claude-code
attachments:
- 01KQSDH59T9CTS8X40MPN62N6Z-01KQSDH59M1ANQK3086NA8XCDX-image-j9jBODrxZuGJaiYxWB9gLwDmHggV83.png
position_column: todo
position_ordinal: c880
title: Perspectives are scoped by view kind, not view id — all grid views share one pool
---
## What

Reported behavior: switching between two grid-kind views (e.g. a tasks grid and a tags grid) shows the **same** perspective tabs in the formula bar. Perspectives saved while one grid is active appear in every other grid view of the same kind.

### Root cause

`Perspective.view` stores the view **kind** string (`"board"`, `"grid"`), not the view's id. Every consumer filters by kind:

- Frontend: `kanban-app/ui/src/components/perspective-tab-bar.tsx:161–165`
  ```ts
  const viewKind = activeView?.kind ?? "board";
  const filteredPerspectives = useMemo(
    () => perspectives.filter((p) => p.view === viewKind),
    [perspectives, viewKind],
  );
  ```
- Backend: `swissarmyhammer-kanban/src/commands/perspective_commands.rs` filters by `view_kind` at ~7 sites (lines 63, 300, 331, 354, 382, 559, 578, 639).
- Backend: `swissarmyhammer-kanban/src/dynamic_sources.rs:113` — `gather_perspectives(view_kind)`.
- Data model: `swissarmyhammer-kanban/src/perspective/add.rs:28` — `Perspective::new(id, name, view: String)` where `view` is a kind.
- Type: `kanban-app/ui/src/types/kanban.ts:55` — `PerspectiveDef.view: string` documented as the view kind.
- On-disk: `.kanban/perspectives/*.yaml` files store the kind string in their `view:` field.

The expected behavior is **per-view-id scoping**: a perspective belongs to one specific view (by id), and switching to a different view of the same kind shows a different perspective set.

### This is bigger than a single task — recommend `/plan`

Implementing this correctly requires:

1. Changing `Perspective.view` semantics from "kind" to "view id" (or adding a new `view_id` field alongside).
2. A data migration for existing `.kanban/perspectives/*.yaml` files (currently store `view: grid` etc.) — decide whether to assign each existing perspective to a specific view id or keep a "shared by kind" fallback.
3. Backend filter changes at every site listed above (`perspective_commands.rs`, `dynamic_sources.rs`).
4. Frontend filter change.
5. Updated type docstrings and the perspective YAML schema in `swissarmyhammer-kanban/builtin/commands/perspective.yaml`.
6. Test fixture updates in `swissarmyhammer-kanban/tests/dynamic_sources_headless.rs:469` and the frontend perspective tab bar tests.
7. Decisions about backwards compatibility — what happens when an old `view: "grid"` value is loaded against the new id-based filter?

That's 7+ files and a data-format decision. **Use `/plan`** to break this into:

- A small task that introduces the new field shape and migration path (data-only).
- A backend task that switches filter call sites once the field is in place.
- A frontend task that switches `perspective-tab-bar.tsx`'s filter and updates tests.
- A migration task for existing `.kanban/perspectives/*.yaml` files.

Each of those fits the per-task sizing limits; the bundled change does not.

## Acceptance Criteria (for the planned epic)

- [ ] Two grid-kind views with different ids show **different** perspective tab sets in the formula bar — perspectives saved against one grid id do not appear when the other grid is active.
- [ ] Existing `.kanban/perspectives/*.yaml` files keep working through the migration (either auto-migrated or interpreted via a documented compatibility rule).
- [ ] Backend filters target view id, not view kind, at every callsite enumerated above.
- [ ] `pnpm -C kanban-app/ui test` and `cargo test -p swissarmyhammer-kanban` pass with updated assertions.

## Tests

- [ ] **Frontend regression** in `kanban-app/ui/src/components/perspective-tab-bar.test.tsx` (or a new spatial test file): mount `<PerspectiveTabBar>` with two `kind: "grid"` views with different ids and a perspective whose `view` is set to view A's id only. Switch active view A → B and assert the perspective is visible only when A is active.
- [ ] **Backend regression** in `swissarmyhammer-kanban/tests/dynamic_sources_headless.rs`: add a test that registers two grid-kind views with different ids, creates a perspective for view A's id, and asserts `gather_perspectives(view_b_id)` returns an empty list while `gather_perspectives(view_a_id)` returns the perspective.
- [ ] **Migration regression**: when loading a perspective whose `view:` field is the legacy kind string, the system either auto-migrates it to a specific view id or surfaces a clear, documented behaviour. Pin that behaviour with a unit test in `swissarmyhammer-kanban/src/perspective/add.rs` (or wherever the loader lives).
- [ ] Run `cargo test -p swissarmyhammer-kanban` and `pnpm -C kanban-app/ui test perspective-tab-bar` and confirm green.

## Workflow

- This task is a **placeholder** — assign it for `/plan` to expand into the four sub-tasks above. Do NOT attempt the bundled change in a single PR.