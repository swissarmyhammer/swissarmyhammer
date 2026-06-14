---
depends_on:
- 01KTED5F8DQ2XH5BB0WK1MRR3P
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff9880
project: ui-command-cleanup
title: Card F — Move board.* (newTask/firstColumn/lastColumn) to a plugin
---
## What
Move the three `board.*` command DEFINITIONS out of `apps/kanban-app/ui/src/components/board-view.tsx` into a PLUGIN.

Sites in board-view.tsx:
- `makeNewTaskCommand` → `board.newTask`: resolves the focused column, dispatches `entity.addTask` + focus. The column-resolve + add + focus is WEBVIEW orchestration → handler bus (Card B); the underlying `entity.addTask` is already a plugin command and stays the dispatch target.
- `makeNavCommand` ×2 → `board.firstColumn` / `board.lastColumn` → backend op `spatial_navigate` (first/last). These have a real backend op, so route their execute to `spatial_navigate` (no bus needed) — mirror Card A's nav directional handling.

Approach:
- New plugin `builtin/plugins/board-commands/index.ts` (mirror `builtin/plugins/file-commands/index.ts`): `board.firstColumn`/`board.lastColumn` route to `spatial_navigate` (first/last); `board.newTask` is marked "handled in webview" (id/name/keys/scope, menu where applicable).
- In board-view.tsx, delete `makeNewTaskCommand` and the two `makeNavCommand` defs; register a webview handler for `board.newTask` (column-resolve + entity.addTask + focus). firstColumn/lastColumn need no handler — they execute server-side.

## Acceptance Criteria
- [x] `board.newTask`, `board.firstColumn`, `board.lastColumn` are plugin-defined; board-view.tsx no longer DEFINES them (`makeNewTaskCommand`/`makeNavCommand` removed).
- [x] firstColumn/lastColumn route to `spatial_navigate`; newTask runs column-resolve + entity.addTask + focus via the bus.
- [x] New-task and first/last-column behavior unchanged.
- [x] GUARD (presentation-only invariant): the `board.newTask` handler is orchestration only — it resolves the focused column and focuses the new card, and performs the durable add by dispatching `entity.addTask` through `useDispatchCommand` (NOT inline). board-view.tsx must NOT import `@/lib/mcp-transport`. `webview-command-bus.guard.node.test.ts` stays green. (firstColumn/lastColumn are backend-op routes, not bus handlers — they are exactly the right case to keep OFF the bus.)

## Tests
- [x] UI: extended `apps/kanban-app/ui/src/components/board-view.column-extremes.spatial.test.tsx` (first/last column → backend dispatch of board.* ids, no client-side spatial_navigate IPC). board.newTask coverage landed in the NEW sister file `board-view.new-task.spatial.test.tsx` (the bus handler mounts with `<BoardView>`, which `column-view.add-task-enter.spatial.test.tsx` does not render; that file stays green unchanged).
- [x] Plugin e2e: `crates/swissarmyhammer-command-service/tests/integration/builtin_board_commands_e2e.rs` — the three board.* ids registered with expected metadata, scope ["ui:board"], no menu; first/last drive the real focus kernel (`navigate focus`); newTask host dispatch is an inert no-op.
- [x] `webview-command-bus.guard.node.test.ts` green with board-view.tsx as a registration site.
- [x] Relevant vitest files green (88 browser tests across 11 files + 46 node guard/mirror tests; tsc clean; cargo nextest -p swissarmyhammer-command-service 127/127, full_baseline at 98 ids / 11 plugins).

## Implementation notes (Card F, 2026-06-11)
- Plugin mirrors the grid-commands/nav-commands template: ONE `BOARD_COMMANDS` data table; firstColumn/lastColumn execute → `focus.navigate({ window, direction })` host-driven (the same wire shape as nav.first/nav.last); newTask host execute inert.
- Scope gating: all three carry `scope: ["ui:board"]`; board-view mounts the constant `BOARD_COMMAND_SCOPE = "ui:board"` marker via `CommandScopeProvider` (the board's spatial moniker `board:<id>` is dynamic — the Card D/E marker pattern; singleton surface, so `registerWebviewCommandHandler` directly, not the focus-gated hook).
- New mirror `BOARD_PLUGIN_COMMANDS` in `mock-command-list.ts` + drift guard `board-plugin-commands-mirror.spatial.node.test.ts`; plugin-ownership guard `board-commands.plugin-owned.node.test.ts`.
- `plugins.rs` doc/test constants updated to 11 plugins; ARCHITECTURE.md marker paragraph notes the dynamic-moniker singleton case.
- Pre-existing failing suites encountered during blast-radius runs (all verified failing on pure git-HEAD files, NOT Card F): board-view.enter-drill-in (01KTSQ38PF...), inspectable.space (01KTSSNDN6...), focus-scope/attachment-display (01KTS1C4EX...); NEW card 01KTVRE0T89FGBH0XYF55VGYTQ filed for entity-inspector.field-enter-drill (2) + column-view.test (2) + column-view.virtualized-nav (3).

## Workflow
- Use `/tdd` — failing tests first, then implement. Automated tests only.

## Review Findings (2026-06-11 11:50)

### Blockers
- [x] `apps/kanban-app/ui/src/components/board-commands.plugin-owned.node.test.ts:48` — `collectSourceFiles` is a 4th byte-identical copy of the helper already in `grid-commands.plugin-owned.node.test.ts`, `editor-drill-in-commands.plugin-owned.node.test.ts`, and `lib/webview-command-bus.guard.node.test.ts` (verified by shasum: all four hash `c9c873da…`), and the surrounding scan/describe scaffold is near-verbatim from the grid guard (the only real variation axis is the `board.` id-prefix regex). The rule of three was already met before this card; copies of scan plumbing drift in lockstep (e.g. a new directory to skip must be edited in four places). The project already has the precedent and the home for shared guard helpers: `src/test/plugin-command-table.ts` (extracted for the mirror guards). Extract `collectSourceFiles` plus a prefix-parameterized `definesPluginCommand(source, prefix)` detector into `src/test/` and import it here (migrating the grid/drill-in/bus-guard copies can ride along or be follow-up, but this card must not add the 4th copy). Keep the per-file detector unit-proof tests — they can exercise the shared helper.

### Blocker resolution (2026-06-11 12:05)
- New shared helper `apps/kanban-app/ui/src/test/plugin-owned-guard.ts` (style matches `plugin-command-table.ts`): exports `SRC_ROOT`, `collectSourceFiles(dir)`, regex-parameterized `definesPluginCommand(source, idPattern)`, and `findCommandDefinitionOffenders(idPattern)` for the scan scaffold. A `makePluginOwnedGuard` factory was rejected — it would only move `describe`/`it` strings into the helper with no real variation-axis gain.
- All 4 consumers migrated, local copies deleted: board/grid/editor-drill-in plugin-owned guards now use thin `defines*Command` wrappers over `definesPluginCommand` with their id pattern (`board\.` / `grid\.` / drill-in three-id alternation); `webview-command-bus.guard.node.test.ts` keeps its unique detectors but imports shared `collectSourceFiles`/`SRC_ROOT`. Zero copies remain (the differing walker in `no-tauri-change-listeners.node.test.ts` is a non-identical variant, out of scope per the finding).
- Detector teeth re-proved: shared quote class now includes backticks (drill-in form, strictly stronger for board/grid); red probe with synthetic `{ id: "board.newTask" }` file → board guard failed (1 failed | 2 passed), probe removed → green.
- Fresh verification: `npx vitest run --project unit` → 24 files / 205 tests passed, 0 failed (includes all 4 guards + all mirror node tests); `npx tsc --noEmit` → exit 0, clean.

Everything else verified clean: dispatch target `entity.add:task` is real (dispatch layer rewrites `entity.add:{type}` → `entity.add` + `entity_type`, `entity_commands.rs`) and pinned by `board-view.new-task.spatial.test.tsx`; first/last kernel semantics unchanged (both the retired inline path and the new host-driven path feed the same `SpatialState::navigate`/`edge_command` — the corrected e2e fixture was wrong, production parity holds); keys parity 1:1 vs HEAD; `isEditableTarget` guard unchanged; deletions clean; fresh runs: cargo nextest 127/127, unit guards 17/17, browser 6/6, tsc clean, red-green probe on the bus registration verified (RED 2 failed → restore → GREEN 2 passed, checksum-identical restore).