---
assignees:
- claude-code
position_column: todo
position_ordinal: '9480'
title: Keyboard navigation for grouped board view with expand/collapse commands
---
## What

Add keyboard navigation to the `GroupedBoardView` and `GroupSection` components, and introduce Rust-side `group.expand`, `group.collapse`, and `group.toggleCollapse` commands so expand/collapse is part of the standard command infrastructure. **Group collapse state must live in UIState on the Rust side** — not in local React state — so it is fully testable via Rust unit tests.

### Current state

- **Individual `BoardView`** has full pull-based keyboard nav via `FocusScope` + `ClaimPredicate` system (`kanban-app/ui/src/components/board-view.tsx`). Arrow keys, `gg`/`G`, `0`/`$` all work within a single board.
- **`GroupedBoardView`** (`kanban-app/ui/src/components/grouped-board-view.tsx`) renders a vertical stack of `GroupSection` components, each containing a full `BoardView`.
- **`GroupSection`** (`kanban-app/ui/src/components/group-section.tsx`) has local `collapsed` state toggled by clicking the header. No keyboard nav, no FocusScope, no commands.
- **UIState pattern** (`swissarmyhammer-commands/src/ui_state.rs`): all UI state that affects view rendering lives in `WindowState` on the Rust side (e.g. `active_perspective_id`, `active_view_id`, `inspector_stack`). Group collapse state must follow this pattern.
- **Keybindings** live in `kanban-app/ui/src/lib/keybindings.ts`. Vim already has `zo` → `task.toggleCollapse`. Group collapse should follow the same pattern.
- **Rust command YAML** definitions are in `swissarmyhammer-commands/builtin/commands/`.

### Approach

**Rust side — UIState** (`swissarmyhammer-commands/src/ui_state.rs`):
- Add `collapsed_groups: HashSet<String>` to `WindowState` (transient, `#[serde(skip)]` — no need to persist across restarts). The set holds group value strings that are collapsed.
- Add methods on `UIState`: `toggle_group_collapse(window, group_value)`, `expand_all_groups(window)`, `collapse_all_groups(window, group_values)` — each returns `UIStateChange`.

**Rust side — Commands** (`swissarmyhammer-commands/builtin/commands/group.yaml` + `swissarmyhammer-kanban/src/commands/`):
- `group.toggleCollapse` — reads `group_value` arg, calls `UIState::toggle_group_collapse`. Returns `UIStateChange`.
- `group.expandAll` — calls `UIState::expand_all_groups`. Returns `UIStateChange`.
- `group.collapseAll` — reads the current group values from context, calls `UIState::collapse_all_groups`. Returns `UIStateChange`.

**Frontend side** — changes to three files:
1. `kanban-app/ui/src/components/group-section.tsx` — remove local `useState(false)` for collapsed. Read collapse state from UIState (via the `ui-state-changed` event pattern used by `perspective-context.tsx`). Wrap the group header in a `FocusScope` with moniker `group:{bucket.value}`, register `claimWhen` predicates for `nav.up`/`nav.down` to navigate between group headers.
2. `kanban-app/ui/src/components/grouped-board-view.tsx` — provide cross-group navigation context (adjacent group monikers) similar to how `BoardView` provides `leftColumnTaskMonikers`/`rightColumnTaskMonikers`. Dispatch `group.toggleCollapse`/`expandAll`/`collapseAll` via the command system.
3. `kanban-app/ui/src/lib/keybindings.ts` — add vim bindings: `zo` → `group.toggleCollapse`, `zO` → `group.expandAll`, `zC` → `group.collapseAll`.

### Navigation model

- `nav.up` / `nav.down` — when focused on a group header, moves to the adjacent group header. When focused on a task card inside a group, normal within-board nav applies; moving past the last/first card in a group should jump to the next/previous group header.
- `nav.left` / `nav.right` — within a group section, works as normal column nav. At the group header level, these could be no-ops or could focus the first task in the group.
- `Enter` on a group header — toggle expand/collapse (or inspect, TBD).
- `zo` / space on a group header — toggle collapse.

### Files to modify
- `swissarmyhammer-commands/src/ui_state.rs` (add `collapsed_groups` to `WindowState`, add toggle/expand/collapse methods)
- `swissarmyhammer-commands/builtin/commands/group.yaml` (new)
- `swissarmyhammer-kanban/src/commands/mod.rs` (register new command impls)
- `kanban-app/ui/src/components/group-section.tsx`
- `kanban-app/ui/src/components/grouped-board-view.tsx`
- `kanban-app/ui/src/lib/keybindings.ts`

## Acceptance Criteria
- [ ] Group collapse state lives in `UIState::WindowState::collapsed_groups`, not React local state
- [ ] `group.toggleCollapse` command updates `UIState` and returns `UIStateChange`
- [ ] `group.expandAll` / `group.collapseAll` commands update `UIState` and return `UIStateChange`
- [ ] Group section headers are focusable via keyboard (visible focus indicator on the group header)
- [ ] `nav.up` / `nav.down` (arrow keys, `j`/`k`) navigates between group headers when a group header is focused
- [ ] Navigating past the last card in a group moves focus to the next group header (and vice versa)
- [ ] Commands appear in the command palette when a grouped board view is active
- [ ] Vim keybindings: `zo` toggles group, `zO` expands all, `zC` collapses all
- [ ] Collapsed groups skip their internal cards during nav.up/nav.down traversal

## Tests
- [ ] `swissarmyhammer-commands/src/ui_state.rs` — Unit tests: `toggle_group_collapse` adds/removes from set, `expand_all_groups` clears set, `collapse_all_groups` populates set, all return correct `UIStateChange`
- [ ] `swissarmyhammer-kanban/src/commands/` — Unit tests for group command impls: toggleCollapse calls UIState correctly, expandAll/collapseAll work
- [ ] `kanban-app/ui/src/components/group-section.test.tsx` — test that GroupSection renders a FocusScope with the correct moniker, that claimWhen predicates claim focus correctly for adjacent group navigation
- [ ] `kanban-app/ui/src/components/grouped-board-view.test.tsx` — test that GroupedBoardView passes correct adjacent-group monikers to each GroupSection
- [ ] `kanban-app/ui/src/lib/keybindings.test.ts` — test that `zo`, `zO`, `zC` sequences resolve to group commands in vim mode

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.