---
assignees:
- claude-code
depends_on:
- 01KRE1VDTC4MNKN3YPR619NDQK
- 01KRE1SSN9AX8R67XC58HHQKKB
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffd880
title: Render perspective tab bar from command registry
---
## What

Integration point. `<PerspectiveTabBar>` stops hardcoding which buttons it shows — instead it queries the live command registry for every command whose `tab_button` is set AND whose scope chain matches the active perspective (including `view_kinds` filtering, which now finally does its job).

This task lands the registry-driven rendering path WITHOUT removing any hardcoded buttons yet. The existing `<FilterFocusButton>`, `<GroupPopoverButton>`, `<AddPerspectiveButton>` stay in place; the new render path produces zero buttons until commands are annotated (handled by individual migration tasks). Once a command flips on `tab_button`, the new path renders it; the per-command migration then removes the corresponding hardcoded button in a controlled handoff.

### Files to modify

- `kanban-app/ui/src/components/perspective-tab-bar.tsx`:
  - Add a `useScopedTabCommands(perspectiveId, activeView)` hook that:
    - Calls the existing `commands_for_scope` bridge (the same dispatch path the palette already uses) with a scope chain `["perspective:${perspectiveId}", "view:${activeView.id}", "board:${activeBoardId}"]` — same shape the palette uses for this surface.
    - Filters the returned list to commands with `tab_button != null`.
    - Returns the list, with backend-supplied `options` on each enum param already populated.
  - In the per-tab render, map `tabCommands` to `<CommandButton command={cmd} surface={"perspective_tab"} perspectiveId={p.id} />` rendered alongside the existing hardcoded buttons. Position the new buttons next to the hardcoded ones; the order will collapse once migrations delete the hardcoded counterparts.

- The bridge that reaches the Rust `commands_for_scope` from the UI — likely `kanban-app/ui/src/lib/commands.ts` or similar — must already expose this. If the existing palette wiring uses it, the hook just reuses that path. If not, this task includes a thin Tauri-invoke wrapper.

### Behavior

- Today, the tab bar has 3 hardcoded buttons (Filter, Group, Add) per perspective.
- After this task, it has 3 hardcoded buttons + 0 registry-rendered buttons (no commands carry `tab_button` yet) — net visual change: none.
- After Filter is migrated, it has 2 hardcoded buttons + 1 registry-rendered button (Filter). After all three are migrated, it has 0 hardcoded buttons + 3 registry-rendered buttons.
- During the transition, both render paths coexist deliberately. The migration tasks own the handoff per command.

### Out of scope

- Annotating individual commands with `tab_button` — separate per-command tasks.
- Deleting the hardcoded buttons — also per-command tasks (the deletion is the final step of each migration).

## Acceptance Criteria

- [x] `useScopedTabCommands` hook queries `commands_for_scope` with a scope chain that contains `perspective:`, `view:`, and `board:` monikers and returns only commands whose `tab_button != null`.
- [x] `<PerspectiveTabBar>` renders one `<CommandButton>` per item in `tabCommands`, alongside the existing hardcoded buttons.
- [x] When zero commands have `tab_button` set (current state of the YAMLs at task time), zero `<CommandButton>`s render and the tab bar is visually identical to today.
- [x] When a synthetic test fixture sets `tab_button` on a command, the corresponding `<CommandButton>` appears in the tab bar at runtime.
- [x] `pnpm -C kanban-app/ui test perspective-tab-bar` passes including a new fixture-driven case. (Project uses npm; verified with `npx vitest run perspective-tab-bar`: 13 files, 79 tests, all green. Full suite `npm test`: 226 files, 2137 tests, all green.)

## Tests

- [x] `kanban-app/ui/src/components/perspective-tab-bar.registry-driven.test.tsx` (new):
  - `renders_command_button_for_each_tab_button_tagged_command` — mount with a fixture `commands_for_scope` that returns one command with `tab_button: { icon: "filter" }`, assert one `<CommandButton>` renders.
  - `renders_zero_command_buttons_when_no_commands_have_tab_button` — assert the registry-rendered slot is empty when no command in scope carries `tab_button`. The hardcoded `Filter`, `Group`, `Add perspective` buttons stay (sanity asserted).
  - `respects_view_kinds_filter` — fixture returns a command with `view_kinds: [grid]` and `tab_button`, mount with active view kind `board`, assert the button does NOT render. The test mock simulates the backend's `filter_by_view_kind` pass by reading `view:{id}` from the scope chain and dropping commands whose `view_kinds` don't admit the view's kind — exercising the same contract a production run would.
  - Bonus test: `queries list_commands_for_scope with perspective/view/board monikers in the scope chain` — pins the exact scope chain shape (`perspective:p1`, `view:board-1`, `board:test-board`) so a future refactor that drops one of the segments is loud rather than silent.
- [x] Update `perspective-tab-bar.test.tsx` to NOT regress on the existing hardcoded buttons — they stay until their migrations land. (No update needed; existing assertions still pass.)
- [x] Run: `pnpm -C kanban-app/ui test perspective-tab-bar` — green. (79 tests passing across 13 files.)

## Workflow

- Use `/tdd` — write the three new test cases first against a mocked `commands_for_scope`, then wire the hook + rendering.
- Look at how the command palette consumes `commands_for_scope` today (search `kanban-app/ui/src` for the bridge) — match that pattern. Do NOT invent a parallel query path.
- The new buttons must use the same Pressable / spatial-nav moniker pattern as the existing hardcoded ones so keyboard navigation through the tab bar stays seamless during the transition. #command-driven-ui

## Implementation notes

- **Backend forwarding for `tab_button`.** Before this task, `swissarmyhammer-kanban::scope_commands::ResolvedCommand` did NOT carry the source `CommandDef::tab_button`. The frontend filter (`cmd.tab_button != null`) would have been a no-op against a wire format that always dropped the field. This task adds `tab_button: Option<TabButtonDef>` to `ResolvedCommand` and forwards `cmd_def.tab_button.clone()` from the three real `CommandDef`-backed emit sites (cross-cutting, scoped-registry, global-registry) inside `commands_for_scope`. The five synthetic / dynamic emit sites (`view.set` fan-out, `board.switch:{path}`, `window.focus:{label}`, `perspective.set` fan-out, `entity.add:{type}`) all carry `tab_button: None` because they don't originate from a `CommandDef` with a tab-button declaration. A new integration test file `swissarmyhammer-kanban/tests/tab_button_forwarding.rs` locks the contract end-to-end for both scoped-registry and global-registry forwarding plus the `tab_button == None` happy path.
- **Hook construction.** `useScopedTabCommands(perspectiveId, activeViewId, activeBoardId)` builds the scope chain explicitly rather than reading from `FocusedScopeContext` because the tab bar queries commands for EVERY perspective on the current view — not just the focused one. The chain is innermost-first: `["perspective:${perspectiveId}", "view:${activeViewId}", "board:${activeBoardId}"]`. When the active board id is `undefined` (no board loaded) the hook short-circuits to an empty list without invoking the backend.
- **Render placement.** The new `<RegistryTabButtons>` slot lives inside `<PerspectiveTab>` (the per-tab inner component), positioned after the existing `<FilterFocusButton>` and `<GroupPopoverButton>` so legacy + registry buttons sit visually adjacent. The slot is gated on `activeViewId != null` so the scope chain is well-formed before the bridge is called. The hardcoded buttons remain conditional on `isActive`; the registry buttons render on every tab so that a per-command migration can decide via YAML scope/availability whether the button shows on active or all tabs.
- **`useBoardData` over `useBoardContext`.** The tab bar's existing test harness mounts `<PerspectiveTabBar>` without a `<BoardContainer>` ancestor, so reading via `useBoardContext()` (which throws on missing context) would regress the entire test file. `useBoardData()` returns `BoardData | null` safely; the moniker is derived from `boardData?.board?.id`.
- **Test mock breaking change in `perspective-tab-bar.filter-enter.spatial.test.tsx`.** That existing test mocks `@/components/filter-editor` with `{ FilterEditor }` only. The new transitive import chain (`perspective-tab-bar → command-button → command-popover → filter-editor::FilterExpressionEditor`) makes the partial mock incomplete. The mock now also exports a stub `FilterExpressionEditor`. No behavioral change to that test — its assertion remains the FilterButton-Enter path it was written for.

## Review Findings (2026-05-12 10:18)

### Nits
- [x] `kanban-app/ui/src/components/perspective-tab-bar.registry-driven.test.tsx:419-421` — The bonus 4th test (`queries list_commands_for_scope with perspective/view/board monikers in the scope chain`) advertises that it "pins the exact scope chain shape", but the three assertions use `toContain` (set-membership), which does NOT pin order. A refactor that flips the chain to `["board:test-board", "view:board-1", "perspective:p1"]` (outermost-first) would still pass this test, even though it would break the innermost-first convention every other call site relies on. Strengthen the test by asserting on `chain` directly with `toEqual([...])` so order is locked in. **Addressed (2026-05-12):** swapped the three `toContain` assertions for a single `toEqual(["perspective:p1", "view:board-1", "board:test-board"])` so the test now fails loudly if any future refactor reorders or drops a segment. Comment in the test now spells out the innermost-first contract and shows a concrete outermost-first counter-example that the assertion rejects. Verified: `npx vitest run perspective-tab-bar.registry-driven` — 4/4 green; full `perspective-tab-bar` suite — 79/79 green across 13 files.
- [ ] `kanban-app/ui/src/components/perspective-tab-bar.tsx:369-383` — `<RegistryTabButtons>` does not pass `isActive` to `<CommandButton>`, so every registry-rendered button will render with the default non-highlight style. This is academic today (no command carries `tab_button` yet), but the upcoming per-command migration tasks (Filter, Group, Add) will each need to compute and thread an `isActive` value — those migrations cannot just flip on a YAML key. Consider adding a thin per-command predicate hook (or wiring `isActive` from a follow-up signal like `command.args` against perspective state) so migrations don't each invent their own active-state plumbing. **Deferred (2026-05-12, box left unchecked because no code changed):** Reviewer framed this as forward-looking ("Consider…"). With zero commands carrying `tab_button` today, there is no concrete consumer to shape the active-state API around. The three migration tasks (Filter, Group, Add) each have a structurally different active-state signal — Filter is "filter is non-empty", Group is "group popover is open", Add doesn't have a meaningful active state — so a single shared predicate hook would either be too narrow for two of them or too generic to add value. The right move is to let the first per-command migration land a concrete shape and then refactor a shared hook out of the second. Inventing the abstraction now would commit to a shape before we know which signals it actually needs to carry.
- [ ] `swissarmyhammer-kanban/src/scope_commands.rs:374-509` — The five synthetic emit sites (`emit_view_switch`, `emit_board_switch`, `emit_window_focus`, `emit_perspective_goto`, `emit_entity_add`) all hard-code `tab_button: None`. The doc on `ResolvedCommand::tab_button` (line 226-231) explains why — these rows don't originate from a `CommandDef` with a tab-button declaration. Of the five, `entity.add:{type}` is the most plausible future tab-button candidate (an "+ Add Task" affordance per view). If/when that migration lands, the synthetic emitter will need an upstream lookup (e.g. fetch the `entity.add` CommandDef from the registry and forward its `tab_button` to every fan-out row). Leaving as a design note for the migration owner — not a defect in this task. **Deferred (2026-05-12, box left unchecked because no code changed):** Reviewer explicitly tagged this as "Leaving as a design note for the migration owner — not a defect in this task." The hard-coded `tab_button: None` is correct for current behavior (synthetic rows have no `CommandDef` to forward from). The upstream-lookup refactor only makes sense once a concrete migration (likely `entity.add:{type}` → "+ Add Task") forces the question, and the right shape of that lookup depends on how the migration models per-entity-type tab buttons. Speculating now would invent a registry-lookup path that may not match the eventual shape.