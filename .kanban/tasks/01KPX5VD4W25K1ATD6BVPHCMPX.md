---
assignees:
- claude-code
position_column: todo
position_ordinal: e380
project: spatial-nav
title: 'Board column header: add a keybinding to drill into the column''s cards (recommended: Enter)'
---
## What

When a board column header is focused, there is no obvious key to move focus into the first card of that column. The user can `j` (down) but that uses pure geometric beam-test — depending on which column header is focused and which cards are geometrically below, it may land on a card in a different column, or miss entirely if the first card's rect is too far below. This is a UX gap: "enter the container" has no dedicated verb.

### Options

Five options considered, with tradeoffs. **Recommendation: Option B (Enter on container = drill-in).** Reasoning at the end.

#### Option A — User's proposal: `nav.down` with fallback-to-first-nested

When `j` is pressed and geometric beam test returns no in-layer candidate below, fall back to "first child scope" via the existing `parent_scope` relationship (inverted: find entries whose `parent_scope == focused_key`, pick the top-left of those).

- Pros: reuses an existing key, no new binding
- Cons: overloads `nav.down` with hierarchical semantics. If a column has `parent_scope == board_root` and the first card has `parent_scope == column`, then:
  - Focus on board root → `j` could mean "find first child scope" (column). That conflicts with "find card below board-root" (a column again, spatially).
  - Focus on column → `j` falls through to "first child of column" (first card). Works.
  - The rule becomes: "if no spatial candidate, treat as hierarchical." That's a two-modes-in-one-key design. Surprising when the user's column has a card directly below geometrically — they get the card — vs. when it doesn't — they still get a card but via a different mechanism.
- Verdict: cleverness that can produce non-obvious results.

#### Option B — Enter on a container = drill-in (recommended)

Extend the existing "Enter activates" pattern (task `01KPRS0WVK7YMS20PEY12ZY70W`):

| Scope | Enter action |
|---|---|
| LeftNav button | switch view |
| Perspective tab | switch perspective |
| Grid cell | edit |
| Inspector field | edit |
| Row selector | inspect entity |
| Card | inspect entity |
| **Column header** | **focus first card in column** ← new |

- Pros: consistent verb — Enter "activates" the focused scope. For a container, "activate" is "enter." Mirrors file-manager conventions (Enter on a folder opens it).
- Cons: none structural. Only risk: users who expect Enter on a column header to rename/edit the column title. That conflict is already resolvable — double-click or F2 can handle rename. If the current behavior is "Enter renames," change it; naming a column is less common than drilling in.
- Verdict: consistent with existing pattern, no overload of geometric nav.

#### Option C — Tab / Shift-Tab for hierarchy

Tab on a container drills in; Shift-Tab pops out to parent. Common in nested tree views.

- Pros: orthogonal to h/j/k/l spatial nav; clear separation of hierarchical vs. spatial navigation.
- Cons: Tab is already heavily overloaded (form field advancement, focus cycling, inspector.nextField). Introducing another meaning for Tab creates context-dependent behavior. And in the kanban app there's no existing "pop out to parent" equivalent — would have to invent one.
- Verdict: more complexity than it solves.

#### Option D — vim convention `l` drills in, `h` pops out

Mirrors how `h/l` sometimes collapse/expand nodes in vim tree plugins (NerdTree, etc).

- Pros: reuses spatial keys; thematic if you think of containers as "rightward" in hierarchy.
- Cons: conflicts with the pure-spatial meaning of `h/l` (horizontal motion). If a column header has another column header to its right, pressing `l` should go there, not drill in. Adding a "if container then drill, else move horizontal" rule is the same overload problem as Option A.
- Verdict: same flaw as Option A, different key.

#### Option E — Remove column headers from spatial nav entirely

Column headers become pure decoration; only cards are focusable. Pressing `j` from anywhere above a column lands on the first card; `k` from the first card leaves the board entirely.

- Pros: simplest model; no "container" concept needed at this layer.
- Cons: loses header-specific actions (sort, collapse, rename). User would need a menu or command palette to operate on a column without a focus target.
- Verdict: too restrictive. The user presumably wants to focus headers for some purpose (naming, sorting, etc).

### Recommendation: Option B

Add an `entity.activate` or `container.enter` command (actual id can match the board-specific site — e.g. a new `column.enterChildren` or reuse a generic pattern), bound to `Enter` on the column header's `FocusScope.commands`. The execute handler calls `setFocus(firstCardMoniker)` where `firstCardMoniker` is the moniker of the first card in the column's task list.

This extends the existing per-scope Enter-activate pattern cleanly:
- Consumers that focus a "container" scope (column header, and later potentially other containers) get a predictable "dive in" verb
- Zero overload of `j/k/h/l` (they stay purely spatial)
- Matches the column-view.tsx file-manager convention

### Mechanism

The column header's `FocusScope` already wraps the header DOM. Search `kanban-app/ui/src/components/column-view.tsx` for the header's `<FocusScope>`. Add to its `commands` array:

```tsx
const firstCardMoniker = column.tasks[0]?.moniker;
const commands: CommandDef[] = useMemo(() => {
  if (!firstCardMoniker) return [];  // empty column → no drill-in
  return [{
    id: `column.enterChildren.${column.id}`,
    name: `Focus first card in ${column.name}`,
    keys: { vim: "Enter", cua: "Enter", emacs: "Enter" },
    execute: () => setFocus(firstCardMoniker),
    contextMenu: false,
  }];
}, [firstCardMoniker, column.id, column.name, setFocus]);
```

For an empty column, no Enter binding is provided — Enter on an empty column header stays unassigned (falls through scope chain to any outer handler, or does nothing).

### Out of scope

- Pop-out-to-parent keybinding (e.g. Escape on a card = focus back to column header). If you want that, file a follow-up. Default: h/j/k/l geometric nav handles it — press `k` from the first card goes back to the header via beam test.
- Generalizing to all containers (inspector sections, grid column headers, etc.). This task is specifically for board column headers. Each container type can opt in later via the same pattern.
- Renaming the column via Enter. If Enter is currently bound to rename, move rename to `F2` or `R` (not part of this task unless a rename-Enter binding actually exists; check first).

### If the user prefers Option A instead

The implementation pivot is: modify `SpatialState::navigate` in `swissarmyhammer-spatial-nav/src/spatial_state.rs` so that when `spatial_search` returns `Ok(None)` for a direction, fall through to `find_first_child(focused_key)` — an entry whose `parent_scope == focused_key`, picked top-left. Still only within the active layer. This is a larger, algorithm-level change and should be re-tasked if pursued; the current task's tests would need to be rewritten.

## Acceptance Criteria

- [ ] Pressing Enter on a focused board column header with at least one card moves focus to the first card in that column's task list
- [ ] Pressing Enter on a focused board column header with zero cards does not crash; either does nothing or falls through to whatever the scope chain resolves (document which in the task's completion notes)
- [ ] `k` (up) from the first card lands back on the column header via normal beam test (no special handling needed)
- [ ] Other Enter-activate bindings still work: LeftNav button, perspective tab, grid cell, inspector field, row selector, card, toolbar inspect button
- [ ] If the column header previously had Enter bound to a rename action, that binding is moved to `F2` or a different key AND the previous binding's tests are updated to reflect the new key
- [ ] No regression in existing board or column-view tests
- [ ] All tests green

## Tests

- [ ] Add a vitest-browser test in `kanban-app/ui/src/components/column-view.test.tsx` (or a new `spatial-nav-column-drill.test.tsx` fixture): render a board column with 3 cards, focus the column header, dispatch Enter, assert focus moved to the first card's moniker
- [ ] Add a test for the empty-column case: render a column with zero cards, focus the header, dispatch Enter, assert no crash and (per the acceptance criteria decision) either focus is unchanged or the fall-through handler runs
- [ ] Add a regression test: Enter on a grid cell still edits (not drill-in), Enter on an inspector field still edits, Enter on a card still inspects — the new column binding does not leak semantics elsewhere
- [ ] Run `cd kanban-app/ui && npm test` — all tests green
- [ ] Manual: load a board, click a column header, press Enter — focus moves to the first card. Press `k` — focus returns to the header.

## Workflow

- Use `/tdd`. Write the failing "Enter on column header drills into first card" test first, then implement the `commands` prop on the header's FocusScope.
- Before adding the binding, grep for any existing Enter handler on the column header (double-click rename, keyboard rename, etc). If one exists, decide: move it to F2, or make Enter a multi-step interaction. Document the decision in the task's completion notes.
- Keep the scope narrow. Do NOT implement Option A, C, D, or E — they're documented as alternatives but not part of this task. If the user requests a pivot, revise the task first.

