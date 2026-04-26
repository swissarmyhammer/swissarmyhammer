---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffff8880
title: 'Grid view: empty state and grid body need context menu + prominent "New <EntityType>" button'
---
## What

When a grid view has no rows (e.g. a fresh Tags grid or a filter that matches nothing), the grid renders a plain "No rows to display" message with no affordance for adding the first entity. A `+` button (`AddEntityBar` in `kanban-app/ui/src/components/grid-view.tsx:576-601`) does exist below the table, but:

- It's a tiny `Plus` icon at `text-muted-foreground/50` opacity (line 591) — easy to miss even with content, almost invisible on an empty grid where the eye is drawn to the centered "No rows to display" message.
- Right-clicking anywhere on the empty grid area (the "No rows to display" div at `kanban-app/ui/src/components/data-table.tsx:217-223`) falls through to the OS default menu — no "New <EntityType>" option surfaces because there's no `onContextMenu` handler on the empty state.
- Right-clicking the whitespace between/below rows in a non-empty grid has the same gap: `EntityRow` (`data-table.tsx:551-575`) wires `onContextMenu`, but the outer scroll container at `data-table.tsx:226` does not, so only on-row clicks reach the context menu pipeline.

**Palette side is already correct** (verified):
- `emit_entity_add` at `swissarmyhammer-kanban/src/scope_commands.rs:404-439` emits `entity.add:{type}` for every `view:{id}` moniker in the scope chain when the view declares an `entity_type`. All four builtin grid views declare one (`swissarmyhammer-kanban/builtin/views/{tasks-grid,tags-grid,projects-grid,board}.yaml` line 5).
- `view:{id}` is injected into the scope chain by `ViewContainer` (`kanban-app/ui/src/components/view-container.tsx:50-53`) whenever a grid is rendered.
- Tests at `kanban-app/ui/src/components/command-palette.test.tsx:800+` prove the palette already shows "New Tag" / "New Task" / "New Project" per active view.
- So the palette requirement from the user's report is **already satisfied**. Call this out in the fix so the implementer doesn't re-plumb it.

**Remaining gaps to close**:
1. No context menu on the empty grid area.
2. No context menu on non-row whitespace in a non-empty grid.
3. No discoverable "+ New <EntityType>" button — the existing one is too faint and positioned where users don't look.

## Approach

### 1. Extract the empty state out of `DataTable` into `GridView`

`data-table.tsx` is a generic spreadsheet primitive that doesn't know entity types. The "No rows to display" branch (`data-table.tsx:217-223`) currently hardcodes a generic message and no affordances because it can't reach entity-type context. Move empty-state responsibility up to the grid layer:

- In `DataTable` (line 217-223): remove the `flatRows.length === 0` early return. Let the table render with its headers and empty body — the caller decides whether to render the table at all or substitute an empty state.
- In `GridView`/`GridBody` (`grid-view.tsx:620-644`): branch before `<DataTable>`. When `data.entities.length === 0`, render a new `GridEmptyState` component; otherwise render `<DataTable>` + `<AddEntityBar>` as today.

### 2. Build `GridEmptyState`

New component in `grid-view.tsx` (local, unexported):

- Display: a friendly centered block with
  - `Paperclip`/entity-appropriate `lucide` icon (or reuse the `Plus` already imported) at a larger size,
  - `No {entityType}s yet` text (pluralize trivially — `${entityType}s` is fine for the four builtin types: tasks, tags, projects; the column-grid view uses `column` which pluralizes the same),
  - A prominent `New {EntityType}` button using the project's standard `Button` primitive (`kanban-app/ui/src/components/ui/button.tsx` — same one used throughout). Variant: default (not muted). Clicking dispatches `entity.add:{entityType}` via `addNewEntity(dispatch, entityType)` (`grid-view.tsx:186` — existing helper, reuse it).
- Context menu: attach `onContextMenu={useContextMenu()}` to the empty-state wrapper `div`. The scope chain already contains `view:{id}`, so `useContextMenu()` will trigger `list_commands_for_scope` and render "New <EntityType>" in the native menu (same pipeline as right-click on a perspective tab or an entity row).
- Layout: `flex-1 flex items-center justify-center` so it fills the body, matching the current empty state's footprint.

### 3. Wire context menu on non-empty grid whitespace

In `DataTable` (line 226) — the scroll container `<div ref={tableContainerRef}>`:

- Attach `onContextMenu={useContextMenu()}` so right-clicking whitespace between rows or below the last row surfaces the context menu with the view's `entity.add` (and whatever other commands are scoped to this chain).
- `EntityRow`'s `onContextMenu` at line 567-570 calls `e.stopPropagation()` implicitly via React event bubbling? Actually no — looking at line 567, it calls `setFocus` then `contextMenuHandler`, neither of which stops propagation. Add `e.stopPropagation()` inside the `EntityRow` handler so per-row context menus still pin to the row target and don't bubble up to the grid-level one (which would offer a different, less-specific command set).

### 4. Leave `AddEntityBar` as-is

- The existing `+` button at `grid-view.tsx:576-601` stays for non-empty grids. It's not the discoverability problem once there's actual content — it's fine as a secondary affordance. Don't touch its styling; that's a separate UX task if it proves to still be missed.

**Do NOT** duplicate the palette emission logic or create a new command — `entity.add:{type}` is already the right entry point, and both the "+ button" click and the empty-state button click already route through it (via `addNewEntity`, `grid.newBelow`, `grid.newAbove`).

## Acceptance Criteria

- [x] Opening a Tags / Projects / Tasks grid with zero matching rows shows a prominent "New <EntityType>" button centered in the view (not a barely-visible small icon at the bottom).
- [x] Clicking that button dispatches `entity.add:{entityType}` and adds a new row, which becomes visible immediately (existing entity-created event flow).
- [x] Right-clicking the empty-grid area shows a native context menu containing "New <EntityType>" (and whatever other view-scoped commands exist).
- [x] Right-clicking whitespace between rows or below the last row in a non-empty grid also shows the view-level context menu (same contents as the empty-state right-click).
- [x] Right-clicking a row still shows the row-specific context menu (per-entity commands like Delete, Archive, Copy — NOT the view-level menu). Per-row context menus do not bubble up and fire the grid-level handler. (Already true: `useContextMenu` calls `e.stopPropagation()`.)
- [x] The existing `AddEntityBar` "+" button at the bottom continues to work unchanged for non-empty grids.
- [x] Command palette behavior is unchanged (already shows "New Tag" / "New Task" / "New Project" when the matching view is active).
- [x] No regression in `grid-view.test.tsx` — the existing keyboard command dispatch paths (`grid.newBelow`, `grid.newAbove`) and per-column-header context menu all continue to work.

## Tests

- [x] New browser-mode test `kanban-app/ui/src/components/grid-empty-state.browser.test.tsx`:
  1. Render `GridView` for a tags view with `entities = []`.
  2. Assert a button with text `New Tag` is in the DOM and is visually prominent (not `text-muted-foreground/50`).
  3. Click it. Assert `dispatch_command` was called with `{ cmd: "entity.add:tag" }`.
  4. Fire a `contextmenu` event on the empty-state wrapper. Assert `invoke("list_commands_for_scope", ...)` is called with a scope chain containing `"view:<viewId>"`.
- [x] New browser-mode test in `kanban-app/ui/src/components/data-table.test.tsx` (or grid-view.test.tsx if preferred):
  1. Render a non-empty grid.
  2. Fire `contextmenu` on the outer scroll container (not a row).
  3. Assert handler is called. (Used `onContainerContextMenu` prop instead of asserting on `list_commands_for_scope` directly — this is the same contract at the DataTable layer; the grid-view layer wires it to `useContextMenu`.)
  4. Fire `contextmenu` on a specific `EntityRow`. Assert container handler is NOT called (per-row handler stops propagation via `useContextMenu`).
- [x] Existing tests still pass:
  - `kanban-app/ui/src/components/grid-view.test.tsx` — all cases (updated fixtures: fixtures rendered with empty entities now exercise the empty-state path; probe-based keyboard tests provide a stub entity so the DataTable mock still mounts).
  - `kanban-app/ui/src/components/data-table.test.tsx` — row-level context menu, header context menu, cell click.
  - `kanban-app/ui/src/components/command-palette.test.tsx:800+` — per-entity-type palette rendering (no change expected).
- [x] Run: `cd kanban-app/ui && npx vitest run grid-view grid-empty-state data-table command-palette` — all 61 tests passing across 5 files.

## Workflow

- Use `/tdd`. Start with `grid-empty-state.browser.test.tsx` asserting the prominent "New Tag" button. Make it pass by extracting empty-state handling into GridView + building `GridEmptyState`. Then add the grid-level context menu test and wire `onContextMenu` on the scroll container + empty state.
- Do NOT touch `emit_entity_add`, the command palette wiring, or the `entity.add:{type}` command definitions — those are correct. This task is entirely frontend, confined to `grid-view.tsx` and `data-table.tsx`.
- Do NOT introduce a new `view.addEntity` or similar command — the existing `entity.add:{type}` dynamic is the right abstraction. #ux #commands #frontend

## Review Findings (2026-04-22 13:45)

### Warnings
- [x] `kanban-app/ui/src/components/data-table.tsx:280-290, 306` — Right-clicking a **column header** now fires two context menus: the existing inline header handler (toggles grouping via `header.column.toggleGrouping()`) AND the newly-added container `onContextMenu={onContainerContextMenu}` on the outer scroll div. The header handler only calls `e.preventDefault()` and does NOT call `e.stopPropagation()`, so the contextmenu event bubbles up through React's synthetic event system to the container. Before this change there was no container-level handler, so header-right-click was benign; after this change it toggles grouping *and* pops the native view-scoped context menu (with "New <EntityType>" etc.). This is a visible regression vs. the acceptance criterion "No regression in ... per-column-header context menu all continue to work." Fix: add `e.stopPropagation()` to both branches of `handleHeaderContextMenu` (or, preferably, collapse the two identical branches into one — they already do the same thing). Add a data-table.test.tsx case asserting that right-click on a `TableHead` does NOT invoke `onContainerContextMenu`. **FIXED**: Collapsed the two identical branches into a single `handleHeaderContextMenu` handler and added `e.stopPropagation()`. Added `data-table.test.tsx` test "does not fire onContainerContextMenu when a column header is right-clicked".
- [x] `kanban-app/ui/src/components/data-table.test.tsx:230-253` — The "whitespace below the last row" test actually fires `contextmenu` directly on the scroll container element, not on whitespace between/below rows. It asserts that the container handler fires when its own element receives the event — which is trivially true of any React handler and doesn't prove the behavioural claim. To cover the real scenario: render, query an element that sits *inside* the container but not inside a `<tr>` (e.g. the `<table>` element itself, or a group-header `<tr>` which has no `onContextMenu`), fire contextmenu on it, and assert the container handler fires via bubbling. As written the test would still pass even if bubbling were broken. **FIXED**: Updated the test to fire contextmenu on the inner `<table>` element (inside the scroll container but NOT inside any `<tr>`), so it now exercises real event bubbling to `onContainerContextMenu`.
- [x] `kanban-app/ui/src/components/grid-view.tsx:697, 707` — `GridBody` calls `useContextMenu()` unconditionally at line 695 but only attaches the resulting handler to `DataTable` (via `onContainerContextMenu`) on the non-empty branch. On the empty branch, `GridEmptyState` creates its *own* `useContextMenu()` handler internally. That's not a bug, but it means two hooks compute identical scope chains on every render of an empty grid. Either (a) hoist the handler out of `GridEmptyState` and pass it in from `GridBody` (single call site), or (b) skip the `GridBody`-level call when empty by moving it inside a conditional branch — but that would violate hooks rules, so (a) is the cleaner fix. Low-priority but the duplication is a concept-level waste the code review guidance explicitly flags. **FIXED**: Added an `onContextMenu` prop to `GridEmptyState` and pass `containerContextMenu` from `GridBody` into it. Single call site for the hook.

### Nits
- [x] `kanban-app/ui/src/components/grid-view.tsx:612, 623` — `` `${entityType}s` `` pluralization is fine for the four builtin entity types today, but a filter that matches zero rows in a populated grid yields the slightly misleading "No tags yet" when there are tags that just don't match the filter. Consider "No {plural} match this filter" when `activePerspective?.filter` is non-empty, or a neutral "No {plural} to show" in all cases. Pre-existing user-facing copy concern — flag for a follow-up. **DEFERRED**: Filed follow-up task 01KPTZM5YG9Y0Z7TTHPWQNM4FG ("Grid empty-state copy: distinguish 'no entities yet' from 'filter matches nothing'").
- [x] `kanban-app/ui/src/components/grid-view.tsx:577-580` — `titleCaseEntityType` only upper-cases the first character. `VALID_ENTITY_TYPE` permits slugs with hyphens (`my-entity-type`) which would render as "My-entity-type" instead of "My Entity Type". Not an issue for the current builtin set (single-word), but tighten it or document the single-word assumption if a multi-word entity type is ever added. **FIXED**: `titleCaseEntityType` now splits on `-` and `_`, title-cases each word, and joins with spaces. Behavior for single-word types (task/tag/project/column) is unchanged; multi-word slugs render as "My Entity Type".
- [x] `kanban-app/ui/src/components/data-table.tsx:280-290` — `handleHeaderContextMenu`'s two ternary branches are byte-identical (both run `e.preventDefault(); header.column.toggleGrouping();`). Collapse to a single handler regardless of `perspectiveId`. Pre-existing dead-ternary, but worth fixing alongside the stopPropagation fix above since you'll be touching the same block. **FIXED**: Collapsed to a single handler (alongside the `stopPropagation` fix in Warning 1).