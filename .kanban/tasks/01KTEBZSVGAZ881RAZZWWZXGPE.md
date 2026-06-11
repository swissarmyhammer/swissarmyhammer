---
assignees:
- claude-code
depends_on:
- 01KTED9JYGWM815K2X41N4QDBY
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff9c80
project: ui-command-cleanup
title: Rename all ui.* commands to app.* — eliminate the "ui" command namespace
---
## What
Decision (user): **there is no `ui.*` command namespace. Every former `ui.*` command is renamed to `app.*`.** UI-surface commands are app commands. The command-id namespace is independent of which MCP server backs the command — renaming the id does NOT change which server answers it.

## IMPORTANT — fold into the ui-command-cleanup project (owner decision)
The `ui-command-cleanup` project (Cards A,C,D,E,F,G,H) MOVES many of these commands into plugins. Per "fold rename into each move", those moves adopt the `app.*` name AT MOVE TIME — so the moved commands never carry a `ui.*` id. THIS card only mops up the `ui.*` ids that the cleanup project does NOT move. Before working this card, check what the cleanup has already renamed:
- `app.palette.open` — done by Card A (01KTCQFH7AEQDZD0QETSMCMGP0).
- `app.inspect` / `app.inspector.close` (+ close_all/set_width) — done by Card G (01KTED8MS8917AJCDAVHKSZHK7).
- `app.entity.startRename` — done where that command is moved.
- `app.ai-panel.*` editor drill-ins — done by Card E (01KTED7PFKRS6GMAQKVDCQA07V).
This card = the remainder + the repo-wide guard test (no id starts with `ui.`).

## Rename map (uniform `app.*` — user: "all called app")
| Current id | New id | Backing MCP server (UNCHANGED) |
|---|---|---|
| `ui.inspect` | `app.inspect` | `ui_state` |
| `ui.inspector.close` | `app.inspector.close` | `ui_state` |
| `ui.inspector.close_all` | `app.inspector.close_all` | `ui_state` |
| `ui.inspector.set_width` | `app.inspector.set_width` | `ui_state` |
| `ui.palette.open` | `app.palette.open` | `ui_state` |
| `ui.palette.close` | `app.palette.close` | `ui_state` |
| `ui.mode.set` | `app.mode.set` | `ui_state` |
| `ui.entity.startRename` | `app.entity.startRename` | `ui_state` |
| `ui.setFocus` | `app.setFocus` | `ui_state` (routing unchanged — the command records the scope chain via ui_state `set scope_chain`; the spatial kernel stays the separate `focus` server) |

Also swept (Card E drill-ins still carried `ui.*` ids in the plugin): `ui.ai-panel.composer.drillIn` → `app.ai-panel.composer.drillIn`, `ui.ai-panel.elicitation.field.drillIn` → `app.ai-panel.elicitation.field.drillIn`.

`window.new` (also in this plugin) is NOT a `ui.*` command — it keeps `window.new` and its `window` server routing. Out of scope for the rename; only its plugin home moved.

## Approach
- Rename every remaining `ui.*` id to `app.*` and fold the `ui-commands` plugin registrations into `app-shell-commands` (single app-command plugin).
- Each command keeps its existing MCP-server call verbatim — pure namespace/registration move, zero behavior change.

## Blast radius (update together)
- `builtin/plugins/ui-commands/index.ts` → merged into `builtin/plugins/app-shell-commands/commands/ui.ts` (+ context.ts dispatch surfaces); the `ui-commands` bundle is deleted.
- Frontend keymap / scope references to `ui.*` ids in `apps/kanban-app/ui/src` (keybindings, scope-claim lookups, menu/context-menu wiring)
- `crates/swissarmyhammer-command-service/tests/integration/builtin_ui_commands_e2e.rs` merged into `builtin_app_shell_commands_e2e.rs` (deleted)
- Repo-wide grep for old ids (`ui\.inspect`, `ui\.inspector`, `ui\.palette`, `ui\.mode`, `ui\.entity`, `ui\.setFocus`)

## Acceptance Criteria
- [x] No registered command id begins with `ui.` — all are `app.*`. (Guard test `no_registered_command_id_starts_with_the_retired_ui_prefix` in full_baseline_e2e.rs.)
- [x] Each renamed command routes to the SAME MCP server as before: inspector/palette/mode/rename/inspect/setFocus → `ui_state` (verbatim pre-rename routing). (`ui_origin_commands_execute_against_their_backends` e2e.)
- [x] All keybindings, menu placements, and context-menu entries resolve under the new `app.*` ids — no dangling `ui.*` reference anywhere (remaining `ui.*` mentions are retired-id regression guards and historical "formerly" comments only).

## Tests
- [x] `builtin_ui_commands_e2e.rs` folded into `builtin_app_shell_commands_e2e.rs`, asserting the renamed `app.*` 33-id set and per-command server routing.
- [x] Guard test: assert NO registered command id starts with `ui.` (regression lock) — watched RED against the old bundle, GREEN after.
- [x] Frontend keymap/mirror tests: former `ui.*` shortcuts resolve under the new `app.*` ids; mirror guards re-pointed at `app-shell-commands/commands/ui.ts` — watched RED (ENOENT) before the merge, GREEN after.
- [x] Tests fail before the rename, pass after.

## Workflow
- Use `/tdd` — write the no-`ui.`-prefix guard test first, then migrate.

## Related
- ui-command-cleanup project (folds most renames in at move time). Palette consolidation `01KTCRQ6KJ67FJWYEZFQ6J7R13`.

## Implementation notes (done)
- Pre-existing failures observed, NOT caused by this card (verified by reverting this card's edits and re-running): `swissarmyhammer-kanban::filter_integration s17_tag_names_with_special_chars` (fails at HEAD), and 25 browser-mode vitest files (fail at HEAD; e.g. Tauri `SERIALIZE_TO_IPC_FN` import error). Zero new frontend failures vs the HEAD baseline; this card fixed the 2 mirror guards.
- `ideas/` design docs intentionally untouched (historical planning records).
- `apps/kanban-app/src/plugins.rs` test constants (`BUILTIN_COMMAND_PLUGINS` 11→10, baseline list) updated textually; kanban-app crate not compiled per constraints.

## Review Findings (2026-06-11 15:58)

### Warnings
- [x] `apps/kanban-app/ui/src/components/inspectors-container.test.tsx:67` — Silent string dispatch: a red-green probe reverting the `app.inspector.close` dispatch in `inspectors-container.tsx` back to the retired `ui.inspector.close` was caught by NOTHING — `inspectors-container.test.tsx` stayed 15/15 green and a 15-file scoped node/spatial sweep (mirror guards, palette, inspector, rename, ai-panel, keybindings, app-shell, focus guards) stayed 229/229 green. Cause: the `useDispatchCommand` mock's unknown-id fallback returns a silent no-op (`return vi.fn(() => Promise.resolve());`), so any retired/typo'd command id at a frontend dispatch site passes every node test; the browser tests that exercise the real dispatch path are all in the known-fail `SERIALIZE_TO_IPC_FN` set, so they cannot catch it either. Fix: make the unknown-id fallback in dispatch-capturing mocks throw (or validate ids against `src/test/mock-command-list.ts`) — here and in the sibling tests using the same mock pattern — so a dangling command id fails loudly. (The rename itself is complete — grep found zero live `ui.*` dispatches — this is a durability gap the rename exposed, not a rename miss.)
  - **Resolution (2026-06-11):** Added `src/test/strict-dispatch-mock.ts` (`strictUseDispatchCommand`): every id the rendered tree requests must be enumerated in the test's known map; any other id throws at hook/call time. Adopted in `inspectors-container.test.tsx` and the sibling silent-fallback mocks (`nav-bar.test.tsx`, `focus-indicator.single-variant.spatial.test.tsx`, `nav-bar.focus-indicator.browser.test.tsx`, `slide-panel.test.tsx`). Permanent synthetic negative lives in `src/test/strict-dispatch-mock.node.test.ts` (retired `ui.inspector.close` must throw; watched RED on missing module, then GREEN). Re-ran the actual probe: reverting `inspectors-container.tsx` to `ui.inspector.close` now fails 15/15 with `useDispatchCommand mock: unknown command id "ui.inspector.close"`; restored id → 15/15 green. Full sweep of the 6 touched test files: 52/52 green; `tsc --noEmit` clean.

### Nits
- [x] `crates/swissarmyhammer-command-service/tests/integration/full_baseline_e2e.rs:244` — Stale doc comment "The locked 62-id baseline." above `expected_command_ids()`; the baseline is 99 ids. Pre-existing at HEAD, but the surrounding plugin-count docs were updated 11→10 in this change — cheap to fix while here.
  - **Resolution (2026-06-11):** Counted 99 string literals in `expected_command_ids()`; comment updated to "The locked 99-id baseline." (textual change only, per the no-cargo constraint).