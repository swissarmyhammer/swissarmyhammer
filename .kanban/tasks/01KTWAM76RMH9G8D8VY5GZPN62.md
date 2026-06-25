---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvzat93dasfn5dmexncd4jek
  text: 'Picked up. Decision from user: BUILD the command (not leave deleted). Plan: register a real `group.toggleCollapse` plugin command with `keys: { vim: "z o" }`, route it through the webview command bus to grouped-board-view, toggle the focused group''s collapse. Starting research on current code.'
  timestamp: 2026-06-25T12:07:00.461475+00:00
- actor: claude-code
  id: 01kvzb2cg96nq6z2ezjefyc5z2
  text: |-
    Research done. Design:
    - Command id `group.toggleCollapse`, owned by `board-commands` plugin (the only place grouping exists). Webview-bus handled (no backend op), inert host execute — mirrors `board.newTask`. Scope-gated `["ui:board"]`, keys `{ vim: "z o" }` (chord).
    - Routing: chord `z o` resolves via the keymap chain-walk while `ui:board` is in the focused scope chain; `useDispatchCommand` consults the webview command bus first and runs the registered handler.
    - Per-group target: each GroupSection currently wraps a BoardView whose `board:<id>` FocusScope FQM is IDENTICAL across groups (group value is NOT in any column/card FQM — confirmed). So I wrap each GroupSection's content in a `<FocusScope moniker="group:<value>" showFocus={false} handleEvents={false}>` (same pattern perspective-container uses to wrap the board) so each group's inner FQM subtree becomes unique. Then GroupSection registers `group.toggleCollapse` via `useFocusedWebviewCommandHandlers("group:<value>", { "group.toggleCollapse": onToggleCollapsed })` — only the focused group's handler is live, exactly the Field/Pressable many-instance precedent.
    - Tests (TDD): (1) Rust command-service e2e (builtin_board_commands_e2e) extended to assert `group.toggleCollapse` registered with vim `z o`, scope ui:board, inert host dispatch. (2) vitest production-path test: render grouped board, focus inside a group, dispatch `group.toggleCollapse`, assert that group's collapsed state flips.
  timestamp: 2026-06-25T12:11:26.089839+00:00
- actor: claude-code
  id: 01kvzbs64bsesg34jkwdb1zjak
  text: |-
    Implementation landed (TDD RED->GREEN for both layers):
    - Plugin: `builtin/plugins/board-commands/index.ts` — added `group.toggleCollapse` { vim "z o", scope ui:board }, inert host execute (webview-bus shape).
    - React: `apps/kanban-app/ui/src/components/group-section.tsx` — each GroupSection wraps content in a `<FocusScope moniker="group:<value>" showFocus=false handleEvents=false>` (unique per-group FQM subtree) and registers `group.toggleCollapse` via `useFocusedWebviewCommandHandlers` gated to focus-within. Degrades to a plain div (with the same data-group-section/value attrs) outside the spatial stack, mirroring PerspectiveSpatialZone, so bare unit tests keep working.
    - Tests: Rust `builtin_board_commands_e2e` (registration+keys+inert dispatch, 3->4) + `full_baseline_e2e` (locked baseline 99->100). Frontend mirror guard + mock (BOARD_PLUGIN_COMMANDS 3->4). New production-path browser test `grouped-board-view.collapse-command.spatial.test.tsx`: real GroupedBoardView + real focus store + real useDispatchCommand -> dispatching toggle expands exactly the focused group (RED was "no group FocusScope rendered", GREEN passes, stable 3x), plus a no-op-when-unfocused case.
    - Stale-rlib gotcha hit on full_baseline (parallel tauri dev/sccache); fixed by touching lib.rs + test, reran green.
    Remaining: tsc, clippy, fmt, wider vitest sweep.
  timestamp: 2026-06-25T12:23:53.227048+00:00
- actor: claude-code
  id: 01kvzc80zrev1jhfa8170j71cp
  text: |-
    Verification all green (fresh):
    - cargo nextest -p swissarmyhammer-command-service: 173 passed (incl. board_commands e2e + full_baseline 99->100).
    - vitest browser: grouped-board-view.collapse-command.spatial.test.tsx 3 passed (focused-group expand, ungrouped `group:` bucket, no-op-when-unfocused), stable 3x; group-section.test.tsx + keybindings.test.ts green; board-view spatial + grouped-board-view suite (9 files) green.
    - vitest unit guards: mirror + plugin-owned + webview-bus guard green.
    - tsc --noEmit clean; clippy clean on touched Rust files (the 5 nav_commands warnings are pre-existing, untouched file); prettier/fmt clean.
    - double-check agent: PASS. Confirmed no new scope-is-leaf violation (production already nests ui:perspective->board->column->task FocusScopes; scope-not-leaf is a dev-only same-rect warning), outer virtualizer unaffected (measureElement is on the wrapper div, not GroupSection root), z o correctly excluded from global table + binds via ui:board chain walk. Addressed its advisory finding by adding the empty-string ungrouped-bucket test.

    Command id: group.toggleCollapse, registered in builtin/plugins/board-commands/index.ts, keys { vim: "z o" }, scope ["ui:board"]. Routing: useDispatchCommand -> webview command bus -> focus-gated handler each GroupSection registers via useFocusedWebviewCommandHandlers("group:<value>", ...) -> onToggleCollapsed flips the focused group's collapse. Moving to review.
  timestamp: 2026-06-25T12:31:59.480981+00:00
- actor: wballard
  id: 01kvzcye2xd55k95vdp4dzwdtk
  text: |-
    Review resolution — reviewer certified the FEATURE CORRECT (the high-risk FocusScope-per-group change verified sound: it's an OUTER prefix segment, composes the inner `board:<id>` scope + columns/cards underneath without shadowing, cross-group/cross-column nav preserved, passive `showFocus/handleEvents={false}` frame degrading to plain `<div>` per the `PerspectiveSpatialZone` precedent; `group.toggleCollapse` registered once, scope `ui:board`, `keys.vim "z o"`, focus-gated so only the focused group's handler fires; production-path spatial test genuine; mouse toggle untouched). No correctness/security/nav-regression findings.

    In-scope DOC-DRIFT fixed:
    - builtin/plugins/board-commands/index.ts: data-table comment "three commands" → "four" (group.toggleCollapse is the 4th entry). Header/description already named the new command; log.info already said 4.
    - full_baseline_e2e.rs: stale prose counts corrected to 100 (lines ~12, 26 said 77; ~114, 255 said 99) — the assertion + TOTAL were already 100.

    Test-quality findings WAIVED as optional polish (engine over-escalates; none affect correctness, all on test code): duplicated inert-webview-command assertion in builtin_board_commands_e2e.rs (the engine's "blocker"); dual-maintained command-ID lists in that test; magic `20`ms delay + fixture-geometry literals.

    Verified state holds: command-service 173, vitest browser suites green (collapse-command, group-section, board-view spatial, keybindings), tsc clean, fmt/clippy clean on touched files. Doc edits are comment-only. Moving to done.
  timestamp: 2026-06-25T12:44:13.789505+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffe880
project: ui-command-cleanup
title: task.toggleCollapse does not exist — vim `z o` chord dropped during Card J migration; decide owner or delete for good
---
## What

While migrating `SEQUENCE_TABLES` chords into plugin command `keys` (Card J, `01KTED9Z9936CVM6P8YPCZ5WRS`), the vim `z o` → `task.toggleCollapse` entry was found to be DEAD: no plugin, YAML, webview-bus handler, or React `CommandDef` anywhere registers a command with id `task.toggleCollapse` (the only collapse logic is `grouped-board-view.tsx`'s local React `toggleCollapsed` state, which is not command-driven). Pressing `z o` dispatched an id the command service does not know — a guaranteed error/no-op.

Card J therefore dropped the `z o` binding instead of migrating it (the other three chords — `g g`, `g t`/`g Shift+T`, `d d` — moved into nav-commands / perspective-commands / entity-commands as chords).

## Decide

- If collapse-toggle-by-key is wanted: create a real `task.toggleCollapse` (or better-named, e.g. `group.toggleCollapse`) command in the owning plugin, route its execution to the grouped-board-view via the webview command bus, and declare `keys: { vim: "z o" }` (chord schema is now first-class).
- If not wanted: nothing to do — this card just documents why `z o` disappeared.

## Acceptance Criteria
- [ ] Either a working, plugin-registered collapse-toggle command with the `z o` chord and a production-path test, or an explicit decision recorded that the binding stays deleted. #ui

## Review Findings (2026-06-25 06:33)

Reviewed: working tree vs HEAD `ced8f50f`. Feature BUILD of `group.toggleCollapse` (vim `z o`, scope `ui:board`) with the load-bearing FocusScope-per-group structural change. Engine fleet + direct verification of the structural concern.

### In-scope structural verification (load-bearing — PASS, no blocker)
- [x] FocusScope-per-group is correct: each `GroupSection` wraps content in `<FocusScope moniker="group:<value>" showFocus={false} handleEvents={false}>`; the inner `board:<id>` scope + columns/cards compose UNDER this new outer segment (it ADDS a unique prefix, does NOT replace or shadow `board:<id>`). Cross-group/cross-column keyboard nav inside groups is preserved — cards/columns keep their own nested FocusScopes. `showFocus={false} handleEvents={false}` makes it a passive FQM frame, not a focus/click target. Matches the cited `PerspectiveSpatialZone` precedent including the plain-`<div>` degradation outside the spatial provider stack (`group-section.tsx` lines 179-217).
- [x] Command correctness: `group.toggleCollapse` registered ONCE in `BOARD_COMMANDS` table, `scope: ["ui:board"]`, `keys: { vim: "z o" }` (space-separated chord), inert host-execute branch SHARED with `board.newTask` (`builtin/plugins/board-commands/index.ts` lines 156-160, 216-226). Consistent with sibling.
- [x] Focus-gating: `useFocusedWebviewCommandHandlers(groupSegment, handlers)` keys the handler to this group's segment — only the focused group's handler is live, so a dispatch flips exactly the focused group (no-op when unfocused). Mouse header-button toggle (`onClick={onToggleCollapsed}`) is untouched and shares the same callback — no regression to existing collapse behavior.

### Blockers
- [ ] `crates/swissarmyhammer-command-service/tests/integration/builtin_board_commands_e2e.rs` — The `board.newTask` inert-webview-handled test is verbatim near-identical to the new `group.toggleCollapse` test: save focused state, execute command, assert ok=true, assert no `event` key, assert focused state unchanged. Extract a helper `assertInertWebviewCommand(service, commandId, commandName, state)` running all five assertions, call it twice. (NOTE: test-quality duplication in test code, not a product blocker — the feature itself is correct and green.)

### Warnings
- [ ] `crates/swissarmyhammer-command-service/tests/integration/builtin_board_commands_e2e.rs` — Command IDs maintained in two places (BOARD_IDS constant + board_metadata), requiring manual sync. Define one canonical table (id/name/keys) and derive BOARD_IDS from it.
- [ ] `crates/swissarmyhammer-command-service/tests/integration/full_baseline_e2e.rs` — Stale drift-guard PROSE (the assertion is correctly 100; only the comments drifted): line 12 `"all 77 commands wire through the new path"`, line 26 `77-id baseline`, line 114 `the 99 ids asserted`, line 255 `The locked 99-id baseline`. The actual count assertion (line ~496) is correctly `100`, so the guard FUNCTIONS — but the prose was not updated when 99→100 with `group.toggleCollapse`. Update all four strings to 100. (In-scope under focus point 4: leave no drift-guard prose stale.)
- [ ] `builtin/plugins/board-commands/index.ts` — Minor doc drift the engine missed: the module header (line ~7 "the three original"), the plugin `description` (lines 178-179 "new task, first/last column"), and the `load()` doc-comment (lines 181-184 "the four board-view commands" is correct, but the description prop still omits group.toggleCollapse) are inconsistent — some say three/omit the new command while line 230's `log.info` correctly says 4. Sync the prose to four commands incl. `group.toggleCollapse`.

### Nits
- [ ] `apps/kanban-app/ui/src/components/grouped-board-view.collapse-command.spatial.test.tsx` — Hardcoded `20` ms async-delay literal repeated 3×. Extract `const ASYNC_STABILIZATION_DELAY_MS = 20;`.
- [ ] `crates/swissarmyhammer-command-service/tests/integration/builtin_board_commands_e2e.rs` — Test-fixture geometry magic numbers (`450.0` board width, `250.0` height, `10.0` y-offset, `150.0`/`300.0` column x-positions) are unexplained. Extract named constants (BOARD_WIDTH, BOARD_HEIGHT, COLUMN_Y_OFFSET, MIDDLE_COLUMN_X, LAST_COLUMN_X) with a comment on the column-spacing strategy.

### Out-of-scope / pre-existing (disregard — confirmed not introduced by this task)
- `kanban-app::ai_panel_e2e` (GPU), `swissarmyhammer-plugin` `file_notes_e2e` / `example_layering_e2e` (CWD-isolation), pre-existing clippy in `builtin_nav_commands_e2e.rs` (untouched file). Not attributable to this diff.

Engine counts: 1 blocker, 3 warnings, 2 nits (test-quality + doc-drift only; no product/correctness/security findings). Structural FocusScope-per-group concern verified directly — PASS.