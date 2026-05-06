---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffe80
project: spatial-nav
title: Migrate remaining icon-button sites to &lt;Pressable&gt; (audit + sweep)
---
## What

Parent ref: ^01KQM9BGN0HFQSC168YD9G82Z2 — the `<Pressable>` primitive (FocusScope leaf + button + Enter/Space activation) was built and proven on FOUR reference sites under the parent task: `nav-bar.tsx::ui:navbar.inspect`, `perspective-tab-bar.tsx::AddPerspectiveButton`, `entity-card.tsx::InspectButton`, and `column-view.tsx::AddTaskButton`. The remaining icon-button sites the parent task enumerated still wrap a `<button onClick={…}>` directly — keyboard users either cannot focus them, or focus works but Enter is a no-op.

This task migrates every remaining site to `<Pressable>` (or `<Pressable asChild>` inside a `<TooltipTrigger asChild>`) so the contract — every actionable icon button activates identically through mouse and keyboard — holds across the UI.

## Sites to migrate

Each site replaces its `<FocusScope>?<button onClick=…>…</button></FocusScope>?` shape with a `<Pressable>` (or `<Tooltip><TooltipTrigger asChild><Pressable asChild …>…</Pressable></TooltipTrigger>…</Tooltip>` for tooltip-wrapped buttons):

1. **`kanban-app/ui/src/components/nav-bar.tsx::ui:navbar.search`** — currently a `<FocusScope>` wrapping a `<button onClick={dispatchSearch}>`. Migrate to `<Pressable asChild moniker={asSegment("ui:navbar.search")} ariaLabel="Search" onPress={dispatchSearch}>`. **DONE in this task.**
2. ~~**`kanban-app/ui/src/components/entity-card.tsx::InspectButton`**~~ — REMOVED FROM SCOPE (2026-05-03). This site was migrated under the parent task `01KQM9BGN0HFQSC168YD9G82Z2` after its scope was expanded on reopen.
3. **`kanban-app/ui/src/components/perspective-tab-bar.tsx::FilterButton`** — currently a bare `<button>` (no FocusScope). Migrate with new moniker `perspective_tab.filter:${id}` (entity-disambiguated like `card.inspect:${id}`). **DEFERRED to follow-up `01KQQSVS4EBKKFN5SS7MW5P8CN`** — requires Scope→Zone reshape of `PerspectiveTabFocusable` plus cascading test updates (~6-8 files), pushing total file count well past the 5-file budget called out in the sizing note.
4. **`kanban-app/ui/src/components/perspective-tab-bar.tsx::GroupButton`** — parallel to FilterButton. **DEFERRED to follow-up `01KQQSVS4EBKKFN5SS7MW5P8CN`** — same reshape as #3.
5. **`kanban-app/ui/src/components/left-nav.tsx`** — view-button click sites. Confirm via `Grep "ui:leftnav"` whether they're already `<FocusScope>` (they should be after commit `c01f3ed38`); if so, swap to `<Pressable>` to gain Enter activation. Each view button: `<Pressable moniker={asSegment(`ui:leftnav.view:${viewId}`)} ariaLabel={…} onPress={…}>`. **DEFERRED to follow-up `01KQQSVS4EBKKFN5SS7MW5P8CN`** — grouped with perspective-tab-bar per the sizing note's suggested split.
6. **`kanban-app/ui/src/components/board-selector.tsx`** — the tear-off "Open in new window" affordance. Currently `<FocusScope moniker={asSegment("board-selector.tear-off")}><Tooltip>...<Button onClick={dispatchNewWindow}>...</Button></Tooltip></FocusScope>`. Migrate to `<Pressable asChild>` chain. (Note: BoardSelector is being reshaped under task `01KQJDYJ4SDKK2G8FTAQ348ZHG`; coordinate ordering — this migration must follow that task to avoid merge conflicts. Confirmed `01KQJDYJ...` is in `done` column on 2026-05-02 — unblocked.) **DONE in this task.**

## Split decision (2026-05-03)

The sizing note explicitly suggested `navbar/board-selector in one, perspective-tab-bar/left-nav in another`. After exploring all five sites:

- Sites 1 (navbar.search) + 6 (board-selector.tear-off) are simple Pressable swaps. Two component files + two new test files = 4 files, fits comfortably within the budget.
- Sites 3 + 4 (perspective-tab-bar Filter/Group) require promoting `PerspectiveTabFocusable` from `<FocusScope perspective_tab:${id}>` to `<FocusZone>` because adding a `<Pressable>` (which itself mounts a `<FocusScope>` leaf) inside an existing leaf scope would trigger the kernel's iteration-3 `scope-not-leaf` enforcement (added under `01KQJDYJ4SDKK2G8FTAQ348ZHG`). That reshape cascades through 6+ test files (mirroring entity-card iteration 2's scope), well past the budget.
- Site 5 (left-nav) is a clean Pressable swap on its own, but the sizing note groups it with perspective-tab-bar; following that grouping keeps the split coherent.

This task migrates sites 1 and 6. Follow-up task `01KQQSVS4EBKKFN5SS7MW5P8CN` covers sites 3, 4, and 5.

## Acceptance Criteria

- [x] Sites 1 (`nav-bar.tsx::ui:navbar.search`) and 6 (`board-selector.tsx::tear-off`) migrated to `<Pressable>` (the entity-card InspectButton was removed from scope on 2026-05-03 — landed under parent task `01KQM9BGN0HFQSC168YD9G82Z2`). Sites 3, 4, 5 deferred to follow-up `01KQQSVS4EBKKFN5SS7MW5P8CN` per the sizing note's authorised split.
- [x] Each migrated site keeps mouse / pointer activation working (existing onClick paths unchanged in observable behavior).
- [x] Each migrated site gains Enter (vim/cua) and Space (cua) activation when its leaf is focused.
- [x] No regressions: existing tests stay green.

## Tests

- [x] For each migrated site: added an `*.add-enter.spatial.test.tsx` or `*.inspect-enter.spatial.test.tsx`-style sibling test that mounts the surrounding component in the production-shaped provider stack, drives `focus-changed` to seed focus on the leaf's moniker, dispatches Enter, and asserts the expected `dispatch_command` IPC fires exactly once. Mirror the harness from `nav-bar.inspect-enter.spatial.test.tsx`, `perspective-tab-bar.add-enter.spatial.test.tsx`, `entity-card.inspect-enter.spatial.test.tsx`, and `column-view.add-task-enter.spatial.test.tsx` (filed under the parent task). New tests added: `nav-bar.search-enter.spatial.test.tsx` (1 test), `board-selector.tear-off-enter.spatial.test.tsx` (1 test).
- [x] Run the full suite: `cd kanban-app/ui && pnpm vitest run` and `pnpm tsc --noEmit`. Zero failures, zero warnings. Verified: 200 test files / 1927 tests pass (1 skipped, pre-existing); `pnpm tsc --noEmit` exits 0; `cargo nextest run -p swissarmyhammer-focus -p kanban-app` 322/322 pass; `cargo clippy -p swissarmyhammer-focus -p kanban-app --all-targets -- -D warnings` clean.

## Sizing note

Five sites in one task may still be close to the 5-files-touched limit. If so, split into two tasks: navbar/board-selector in one, perspective-tab-bar/left-nav in another. Use judgment based on file-count after exploring each migration. **Action taken: followed the suggested split. Task migrates sites 1+6; follow-up `01KQQSVS4EBKKFN5SS7MW5P8CN` covers sites 3+4+5.**

## Workflow

- Use `/tdd`: for each site, write the migration test first (red), then perform the migration (green), then move to the next site.
- The `<Pressable>` primitive's API is settled — see `kanban-app/ui/src/components/pressable.tsx` and the docstring contract.

## Implementation Notes (2026-05-03)

### Site 1: `nav-bar.tsx::ui:navbar.search`

Replaced `<FocusScope moniker="ui:navbar.search" className="ml-auto"><Tooltip>...<button onClick={() => dispatchSearch().catch(...)}>...</button>...</Tooltip></FocusScope>` with the canonical Pressable chain. The `ml-auto` class previously sat on the FocusScope wrapper to push the search button to the right edge of the navbar's flex row; it now lives on a wrapping `<div className="ml-auto">` outside the `<Tooltip>` because Pressable spreads its className onto the inner `<button>` host (via Slot.Root mergeProps), not onto the outer `<FocusScope>` `<div>` it mounts. A multi-line comment in nav-bar.tsx documents this layout invariant. Removed the now-unused `FocusScope` import.

### Site 6: `board-selector.tsx::tear-off`

Replaced `<FocusScope moniker="board-selector.tear-off"><Tooltip>...<Button variant="ghost" size="icon" onClick={...}>...</Button>...</Tooltip></FocusScope>` with `<Tooltip>...<TooltipTrigger asChild><Pressable asChild moniker="board-selector.tear-off" ariaLabel="Open in new window" onPress={...}><button>...</button></Pressable>...</Tooltip>`. The plain `<button>` carries the same icon-button styling that `<Button variant="ghost" size="icon">` produced (h-6 w-6 rounded-md, ghost-style hover transitions) inlined — keeping the migration to a single button host element rather than nesting a Slot inside a Slot. Removed the now-unused `Button` import; kept the existing `FocusScope` import (still used by the `board-selector.dropdown` leaf above the tear-off).

### Test files added

- `kanban-app/ui/src/components/nav-bar.search-enter.spatial.test.tsx` — mirrors `nav-bar.inspect-enter.spatial.test.tsx` shape; pins Enter on `ui:navbar.search` leaf dispatches `app.search` exactly once.
- `kanban-app/ui/src/components/board-selector.tear-off-enter.spatial.test.tsx` — mirrors the same shape; pins Enter on `board-selector.tear-off` leaf dispatches `window.new` exactly once with `board_path` arg.

### Files touched (4 total, well under the 5-file budget)

1. `kanban-app/ui/src/components/nav-bar.tsx`
2. `kanban-app/ui/src/components/board-selector.tsx`
3. `kanban-app/ui/src/components/nav-bar.search-enter.spatial.test.tsx` (new)
4. `kanban-app/ui/src/components/board-selector.tear-off-enter.spatial.test.tsx` (new)

### Verification

- `cd kanban-app/ui && pnpm vitest run src/components/nav-bar.search-enter.spatial.test.tsx` → 1/1 pass.
- `cd kanban-app/ui && pnpm vitest run src/components/board-selector.tear-off-enter.spatial.test.tsx` → 1/1 pass.
- `cd kanban-app/ui && pnpm vitest run src/components/nav-bar src/components/board-selector` → 50/50 pass (was 41 before; +9 from the new test files mounting alongside existing nav-bar/board-selector tests; actually 8 navbar test files + 1 board-selector with the new file added each side, for 8 total; reconciled count = 50).
- `cd kanban-app/ui && pnpm vitest run src/components/focus` → 116/116 pass.
- `cd kanban-app/ui && pnpm vitest run` → 200 test files / 1927 tests pass / 1 skipped (pre-existing).
- `cd kanban-app/ui && pnpm tsc --noEmit` → exit 0.
- `cargo nextest run -p swissarmyhammer-focus -p kanban-app` → 322/322 pass.
- `cargo clippy -p swissarmyhammer-focus -p kanban-app --all-targets -- -D warnings` → clean.