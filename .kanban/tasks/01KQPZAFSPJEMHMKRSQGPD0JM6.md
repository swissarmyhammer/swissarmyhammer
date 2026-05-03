---
assignees:
- claude-code
position_column: todo
position_ordinal: ca80
project: spatial-nav
title: Migrate remaining icon-button sites to &lt;Pressable&gt; (audit + sweep)
---
## What

Parent ref: ^01KQM9BGN0HFQSC168YD9G82Z2 — the `<Pressable>` primitive (FocusScope leaf + button + Enter/Space activation) was built and proven on FOUR reference sites under the parent task: `nav-bar.tsx::ui:navbar.inspect`, `perspective-tab-bar.tsx::AddPerspectiveButton`, `entity-card.tsx::InspectButton`, and `column-view.tsx::AddTaskButton`. The remaining icon-button sites the parent task enumerated still wrap a `<button onClick={…}>` directly — keyboard users either cannot focus them, or focus works but Enter is a no-op.

This task migrates every remaining site to `<Pressable>` (or `<Pressable asChild>` inside a `<TooltipTrigger asChild>`) so the contract — every actionable icon button activates identically through mouse and keyboard — holds across the UI.

## Sites to migrate

Each site replaces its `<FocusScope>?<button onClick=…>…</button></FocusScope>?` shape with a `<Pressable>` (or `<Tooltip><TooltipTrigger asChild><Pressable asChild …>…</Pressable></TooltipTrigger>…</Tooltip>` for tooltip-wrapped buttons):

1. **`kanban-app/ui/src/components/nav-bar.tsx::ui:navbar.search`** — currently a `<FocusScope>` wrapping a `<button onClick={dispatchSearch}>`. Migrate to `<Pressable asChild moniker={asSegment("ui:navbar.search")} ariaLabel="Search" onPress={dispatchSearch}>`.
2. ~~**`kanban-app/ui/src/components/entity-card.tsx::InspectButton`**~~ — REMOVED FROM SCOPE (2026-05-03). This site was migrated under the parent task `01KQM9BGN0HFQSC168YD9G82Z2` after its scope was expanded on reopen.
3. **`kanban-app/ui/src/components/perspective-tab-bar.tsx::FilterButton`** — currently a bare `<button>` (no FocusScope). Migrate with new moniker `perspective_tab.filter:${id}` (entity-disambiguated like `card.inspect:${id}`).
4. **`kanban-app/ui/src/components/perspective-tab-bar.tsx::GroupButton`** — parallel to FilterButton. Migrate with new moniker `perspective_tab.group:${id}`.
5. **`kanban-app/ui/src/components/left-nav.tsx`** — view-button click sites. Confirm via `Grep "ui:leftnav"` whether they're already `<FocusScope>` (they should be after commit `c01f3ed38`); if so, swap to `<Pressable>` to gain Enter activation. Each view button: `<Pressable moniker={asSegment(`ui:leftnav.view:${viewId}`)} ariaLabel={…} onPress={…}>`.
6. **`kanban-app/ui/src/components/board-selector.tsx`** — the tear-off "Open in new window" affordance. Currently `<FocusScope moniker={asSegment("board-selector.tear-off")}><Tooltip>...<Button onClick={dispatchNewWindow}>...</Button></Tooltip></FocusScope>`. Migrate to `<Pressable asChild>` chain. (Note: BoardSelector is being reshaped under task `01KQJDYJ4SDKK2G8FTAQ348ZHG`; coordinate ordering — this migration must follow that task to avoid merge conflicts.)

## Acceptance Criteria

- [ ] All five remaining sites listed above migrated to `<Pressable>` (entity-card InspectButton removed from scope on 2026-05-03 — landed under parent task `01KQM9BGN0HFQSC168YD9G82Z2`).
- [ ] Each migrated site keeps mouse / pointer activation working (existing onClick paths unchanged in observable behavior).
- [ ] Each migrated site gains Enter (vim/cua) and Space (cua) activation when its leaf is focused.
- [ ] No regressions: existing tests stay green.

## Tests

- [ ] For each site: add an `*.add-enter.spatial.test.tsx` or `*.inspect-enter.spatial.test.tsx`-style sibling test that mounts the surrounding component in the production-shaped provider stack, drives `focus-changed` to seed focus on the leaf's moniker, dispatches Enter, and asserts the expected `dispatch_command` IPC fires exactly once. Mirror the harness from `nav-bar.inspect-enter.spatial.test.tsx`, `perspective-tab-bar.add-enter.spatial.test.tsx`, `entity-card.inspect-enter.spatial.test.tsx`, and `column-view.add-task-enter.spatial.test.tsx` (filed under the parent task).
- [ ] Run the full suite: `cd kanban-app/ui && pnpm vitest run` and `pnpm tsc --noEmit`. Zero failures, zero warnings.

## Sizing note

Five sites in one task may still be close to the 5-files-touched limit. If so, split into two tasks: navbar/board-selector in one, perspective-tab-bar/left-nav in another. Use judgment based on file-count after exploring each migration.

## Workflow

- Use `/tdd`: for each site, write the migration test first (red), then perform the migration (green), then move to the next site.
- The `<Pressable>` primitive's API is settled — see `kanban-app/ui/src/components/pressable.tsx` and the docstring contract.
