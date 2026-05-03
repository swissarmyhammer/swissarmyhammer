---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffff80
project: spatial-nav
title: 'Migrate remaining icon-button sites part 2: left-nav + perspective-tab-bar Filter/Group'
---
## What

Parent ref: ^01KQPZAFSPJEMHMKRSQGPD0JM6 — that task migrated 2 of the 5 remaining `<Pressable>` sites (`nav-bar.tsx::ui:navbar.search`, `board-selector.tsx::tear-off`). The sizing note on the parent explicitly authorised splitting and suggested the grouping `navbar/board-selector in one, perspective-tab-bar/left-nav in another`. This is the second half — 3 sites, with one of them requiring a structural reshape that pushed it out of the parent's 5-files-touched budget.

## Sites to migrate

### 1. `kanban-app/ui/src/components/left-nav.tsx::ScopedViewButton`

Currently:
- `<CommandScopeProvider moniker={moniker("view", view.id)} commands={activateCommands}>` — registers a `view.activate: Enter` CommandDef (manually-built activation binding).
- `<FocusScope moniker={asSegment(`view:${view.id}`)}>` — leaf for spatial-nav.
- Inner `<Tooltip><TooltipTrigger asChild><button onClick={() => dispatch(...)}>` host.

Migration: replace the inner `<FocusScope view:${id}>` + button with `<Pressable asChild moniker={asSegment(`ui:leftnav.view:${viewId}`)} ariaLabel={view.name} onPress={() => dispatch({ args: { view_id: view.id } })}>`. Drop the `activateCommands` array and the `view.activate` CommandDef — Pressable's `pressable.activate` Enter binding subsumes it (innermost-wins so they don't double-fire, but keeping both is dead weight). Keep the outer `CommandScopeProvider` — `useContextMenu` still reads its `moniker("view", view.id)` for right-click context-menu chains.

Moniker change is intentional: `ui:leftnav.view:${viewId}` matches the `ui:navbar.*` / `ui:perspective-bar.*` chrome-namespace pattern (consistent with `inspectable.tsx`'s "UI chrome is not inspectable" rule). No spatial tests pin `segment === "view:..."` — only command-scope tests pin `view:v1` in `scopeChain`, and those reference the `CommandScopeProvider` moniker which stays unchanged.

### 2. `kanban-app/ui/src/components/perspective-tab-bar.tsx::FilterButton`

Blocker: `PerspectiveTabFocusable` currently wraps the whole tab in `<FocusScope moniker={asSegment(`perspective_tab:${id}`)}>` (a leaf scope). Adding a `<Pressable>` inside (which itself mounts a `<FocusScope>`) creates a Scope-inside-Scope violation that the kernel's iteration-3 `scope-not-leaf` enforcement will detect and log.

Required reshape:
- Promote `PerspectiveTabFocusable` from `<FocusScope perspective_tab:${id}>` to `<FocusZone perspective_tab:${id}>` (mirroring entity-card's iteration-2 promotion). Use `showFocusBar={false}` since the inner leaves carry the focus signal.
- Wrap the existing `<TabButton>` (the name button) in its own inner `<FocusScope moniker={asSegment(`perspective_tab.name:${id}`)}>` leaf so it stays focusable.
- Add `<Pressable asChild moniker={asSegment(`perspective_tab.filter:${id}`)} ariaLabel="Filter" onPress={onFocus}>` for FilterButton. (The inner button keeps its `e.stopPropagation()` so the tab's click-to-activate doesn't double-fire.)
- Add `<Pressable asChild moniker={asSegment(`perspective_tab.group:${id}`)} ariaLabel="Group" onPress={() => onOpenChange(true)}>` for GroupPopoverButton — but `GroupPopoverButton` is itself a `<PopoverTrigger asChild>` chain, so the Pressable goes inside the trigger's slot.

Cascading test updates needed (similar in scope to entity-card's iteration 2):
- `perspective-tab-bar.spatial-nav.test.tsx`: tab now registers as zone, not scope.
- `perspective-bar.spatial.test.tsx`: same.
- `perspective-tab-bar.context-menu.test.tsx`: scope chain still includes `perspective_tab:${id}` (zone monikers also enter the scope chain via FQM composition) — verify chain[0] still resolves correctly, may need to update which segment is innermost.
- `perspective-tab-bar.focus-indicator.browser.test.tsx`: leaf-data-segment selectors change.
- `perspective-tab-bar.no-inspect-on-dblclick.spatial.test.tsx`: target moniker changes.
- `spatial-nav-end-to-end.spatial.test.tsx`: family pinning `perspective_tab:default` register-as-scope must flip to register-as-zone.

### 3. `kanban-app/ui/src/components/perspective-tab-bar.tsx::GroupButton` (`GroupPopoverButton`)

Same reshape as #2 — covered by the same PerspectiveTabFocusable promotion.

## Acceptance Criteria

- [x] `left-nav.tsx::ScopedViewButton` migrated to `<Pressable asChild moniker={asSegment(`ui:leftnav.view:${viewId}`)}>`. The redundant `view.activate` CommandDef is removed; the outer `CommandScopeProvider` for `view:{id}` stays so right-click context-menu chains keep their scope.
- [x] `perspective-tab-bar.tsx::PerspectiveTabFocusable` promoted from `<FocusScope>` to `<FocusZone>` with `showFocusBar={false}`.
- [x] `TabButton` wrapped in inner `<FocusScope moniker={asSegment(`perspective_tab.name:${id}`)}>` leaf.
- [x] `FilterButton` migrated to `<Pressable asChild moniker={asSegment(`perspective_tab.filter:${id}`)}>` with `e.stopPropagation()` on the inner button.
- [x] `GroupPopoverButton` migrated to `<Pressable asChild moniker={asSegment(`perspective_tab.group:${id}`)}>` inside the existing `<PopoverTrigger asChild>` slot.
- [x] All cascading test updates done. Zero Scope-inside-Scope violations logged from the perspective-tab-bar.
- [x] No regressions across the rest of the suite.

## Tests

- [x] `left-nav.view-enter.spatial.test.tsx` — mirrors `nav-bar.inspect-enter.spatial.test.tsx` shape; seeds focus on `ui:leftnav.view:v1` leaf, dispatches Enter, asserts `view.set` IPC fires once with `view_id` arg.
- [x] `perspective-tab-bar.filter-enter.spatial.test.tsx` — seeds focus on `perspective_tab.filter:p1` leaf, dispatches Enter, asserts the filter editor receives focus (the `onFocus` callback fires).
- [x] `perspective-tab-bar.group-enter.spatial.test.tsx` — seeds focus on `perspective_tab.group:p1` leaf, dispatches Enter, asserts the group popover opens (`onOpenChange(true)` fires).
- [x] All existing perspective-tab-bar tests updated for the scope→zone reshape.
- [x] `cd kanban-app/ui && pnpm vitest run` and `pnpm tsc --noEmit` zero failures.
- [x] `cargo nextest run -p swissarmyhammer-focus -p kanban-app` zero failures.

## Workflow

- Use `/tdd`: write the migration test first (red), then perform the migration (green), then move to the next site.
- The perspective-tab-bar reshape is the bulk of the work; expect 4-7 cascading test files to need updates (mirroring entity-card iteration 2's scope: ~8 files).
- The `<Pressable>` primitive's API is settled — see `kanban-app/ui/src/components/pressable.tsx` and the docstring contract.

## References

- Reference harnesses: `nav-bar.inspect-enter.spatial.test.tsx`, `nav-bar.search-enter.spatial.test.tsx`, `perspective-tab-bar.add-enter.spatial.test.tsx`, `entity-card.inspect-enter.spatial.test.tsx`, `column-view.add-task-enter.spatial.test.tsx`, `board-selector.tear-off-enter.spatial.test.tsx` (all in repo).
- Reshape precedent: entity-card iteration 2 under `01KQJDYJ4SDKK2G8FTAQ348ZHG` — promote scope to zone, wrap inner controls in leaves, update all cascading tests.

## Review Findings (2026-05-03 16:37)

### Warnings

- [x] `kanban-app/ui/src/components/perspective-tab-bar.tsx:362` — `ScopedPerspectiveTab`'s docstring still describes the OLD shape: "adds a `<FocusScope moniker={asSegment(\`perspective_tab:${id}\`)}>` leaf when the spatial-nav stack is mounted. That makes each tab a peer leaf in the surrounding `ui:perspective-bar` zone." Post-reshape `PerspectiveTabFocusable` is a `<FocusZone>` (line 481), and tabs are sibling zones (not "peer leaves") containing inner name / filter / group leaves. Update the docstring to describe the zone wrapper and the inner leaves.
- [x] `kanban-app/ui/src/components/perspective-spatial-nav.guards.node.test.ts:62-66` — The "wraps each tab in FocusScope with moniker perspective_tab:${id}" guard test still asserts the OLD shape exists in source. It currently only "passes" because the stale docstring on line 362 of perspective-tab-bar.tsx happens to contain the regex match — this is a tautology, not a real guard. After the reshape the production binding is a `<FocusZone>` not a `<FocusScope>`. Either update the assertion to `<FocusZone\s+moniker=\{asSegment\(\`perspective_tab:\$\{id\}\`\)/` and add a positive assertion for `<FocusScope\s+moniker=\{asSegment\(\`perspective_tab\.name:\$\{id\}\`\)/`, or delete the test if the structural property is already covered by the runtime browser tests. Also fix the file-header doc on line 11 ("each tab is a `perspective_tab:{id}` FocusScope leaf") to reflect the new shape.
- [x] `kanban-app/ui/src/spatial-nav-end-to-end.spatial.test.tsx:650-686` — Test "clicking a perspective tab focuses that tab" still queries `[data-segment='perspective_tab:default']` and clicks it directly. The task description explicitly listed this file as a cascading update ("family pinning `perspective_tab:default` register-as-scope must flip to register-as-zone"), but only the registration-shape side was implicitly handled (via `getRegisteredFqBySegment` walking both zone and scope registrations); the click-target side was missed. The test now passes mechanically because FocusZone's own onClick still calls `focus(fq)` on the wrapper FQM, but it no longer exercises the realistic user path — a real click on the visible tab name lands on the inner `<FocusScope perspective_tab.name:default>` leaf, not the wrapper zone. Either split this test into two cases (clicking the wrapper zone vs clicking the inner name leaf), or refactor the test to click the inner `[data-segment='perspective_tab.name:default']` and assert focus claim flips on that leaf, mirroring how `perspective-bar.spatial.test.tsx` test #3 was rewritten.
- [x] `kanban-app/ui/src/components/perspective-tab-bar.context-menu.test.tsx:232-233, 267-268, 297` — Inline comments still say "(`perspective_tab:<id>` comes from the inner `<FocusScope>` leaf;)". Post-reshape, `perspective_tab:<id>` comes from the `<FocusZone>` wrapper, not the inner FocusScope leaf (which is `perspective_tab.name:<id>`). The assertions themselves are still correct because `useContextMenu` is captured at `PerspectiveTab`'s render scope (outside the inner name FocusScope), so `chain[0]` legitimately resolves to `perspective_tab:p2`. But the explanatory comments mislead the next reader about WHY the chain looks the way it does. Update the comments to reflect: "the FocusZone wrapper, not the inner name leaf — useContextMenu is captured outside the inner `<FocusScope perspective_tab.name:${id}>`".

### Nits

- [x] `kanban-app/ui/src/components/perspective-tab-bar.tsx:442-466` — The new `PerspectiveTabFocusable` docstring is excellent and explains the reshape clearly. Consider adding a one-line cross-reference here pointing readers at the `ScopedPerspectiveTab` docstring (line 356) — that one is currently the stale one and a forward-pointer would help future maintainers find both halves of the explanation.