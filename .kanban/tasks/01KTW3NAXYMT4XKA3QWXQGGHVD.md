---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kv982fzkzf3vzy33r4ra0q17
  text: |-
    Picked up. Research complete. Plan:
    - BINDING_TABLES (keybindings.ts) is already canonical: cua Mod+f→app.search, Mod+z→app.undo; vim Mod+r→app.redo, /+Mod+f→app.search; emacs Mod+f→nav.right (the conflict). mock-command-list.ts derives the global registry from BINDING_TABLES, so no manual mirror edit needed there.
    - Plugin metadata targets: app.undo {cua:Mod+z,vim:u,emacs:Ctrl+/}; app.redo {cua:Mod+Shift+Z,vim:Mod+r}; app.search {vim:/,cua:Mod+f} (DROP emacs — Mod+f is nav.right in emacs per conflict card 01KMT56FTBAP8PQ4QQND08MP97); app.quit {cua:Mod+q,vim:":q"} (menu-accel-only); file.openBoard {cua:Mod+o} (menu-accel-only).
    - Card I precedent: file.closeBoard already lowercased to Mod+w; ai-commands hoisted AI_COMMANDS const + ai mirror guard exist as the template.
    - TDD: update builtin_app_shell_commands_e2e.rs (assert_app_quit/search/undo/redo) + builtin_file_commands_e2e.rs (assert_open_board_metadata) FIRST → watch RED → then change plugin metadata. Hoist APP_COMMANDS / FILE_COMMANDS module-level consts so parseCommandTable can read them. Add app-shell + file mirror guard vitest files.
  timestamp: 2026-06-16T22:15:46.419195+00:00
- actor: claude-code
  id: 01kv98nfraatxdx416gsg2ze5h
  text: |-
    Implementation landed.

    Changes:
    - crates/swissarmyhammer-command-service/tests/integration/builtin_app_shell_commands_e2e.rs: updated assert_app_quit (Mod+Q→Mod+q), assert_app_search (dropped emacs, cua Mod+F→Mod+f), assert_app_undo (Mod+Z→Mod+z), assert_app_redo (vim Ctrl+R→Mod+r) + doc comments. Watched RED first.
    - crates/swissarmyhammer-command-service/tests/integration/builtin_file_commands_e2e.rs: assert_open_board_metadata Mod+O→Mod+o. Watched RED first.
    - builtin/plugins/app-shell-commands/commands/app.ts: hoisted module-level const APP_COMMANDS data table (parseable by parseCommandTable); appCommands() maps it binding each row's run→execute over the dispatch bundle. Canonicalized app.quit cua Mod+q, app.search {vim:/,cua:Mod+f} (emacs dropped), app.undo cua Mod+z, app.redo vim Mod+r.
    - builtin/plugins/file-commands/index.ts: hoisted const FILE_COMMANDS; file.openBoard cua Mod+o.
    - apps/kanban-app/ui/src/test/app-shell-plugin-commands-mirror.spatial.node.test.ts + file-plugin-commands-mirror.spatial.node.test.ts: new drift guards. Per-key check: canonical membership in BINDING_TABLES OR explicit commented allowlist (menu-accel-only: app.quit, file.newBoard, file.openBoard; reachable-but-untracked: app.help F1, app.command/app.palette Mod+Shift+P→app.palette.open, app.undo emacs Ctrl+/, vim chords :q / :). app.search uses MEMBERSHIP (vim binds both / and Mod+f).

    Discovery / gotcha: first FILE_COMMANDS restructure dropped a `.window` segment in the dispatch path (window.window.board.X instead of this→window.window.window.board.X) — caught by the file e2e ("server 'window' has no tool 'board'"), fixed.

    emacs app.search conflict (01KMT56FTBAP8PQ4QQND08MP97) resolved deliberately: emacs key DROPPED (not lowercased), leaving Mod+f as nav.right; emacs Find stays palette-only.

    RED→GREEN verified for guards: re-uppercasing app.search cua to Mod+F failed the app-shell guard (2 assertions); re-uppercasing file.openBoard to Mod+O failed the file guard (2 assertions). Reverted both.

    Results so far:
    - cargo nextest -p swissarmyhammer-command-service: 166/166 passed.
    - vitest unit project (incl. both new guards + all mirror guards): 32 files / 235 passed.
    - vitest browser (keybindings + app-shell + app-shell.nav/ai): 132 passed.
    - npx tsc --noEmit (apps/kanban-app/ui): clean (exit 0).
  timestamp: 2026-06-16T22:26:08.778837+00:00
- actor: claude-code
  id: 01kv9a5jeyrvza454d4ynqpz0d
  text: |-
    Review findings worked (2026-06-16).

    Blockers:
    1. node: protocol prefixes — added node: to all six imports (fs/path/url) in both app-shell-plugin-commands-mirror.spatial.node.test.ts and file-plugin-commands-mirror.spatial.node.test.ts.
    2. Duplicated run→execute bind — extracted a shared bindCommandRun<Surface>(spec, surface) helper into the SDK (crates/swissarmyhammer-plugin/src/sdk/commands.ts), the natural home both bundles already import (@swissarmyhammer/plugin). TDD: added sdk.rs::bind_command_run_wraps_run_as_execute (real V8 isolate) + the mod.rs source-surface assertion, watched RED (export missing), implemented GREEN. Both app-shell-commands/commands/app.ts and file-commands/index.ts now map via bindCommandRun. SDK-test + both e2e green.
    3. Duplicated copy_dir_recursive() — PRE-EXISTING duplication across 10 integration test files; this card touched only 2. There IS a shared support module (tests/integration/support.rs) but consolidating forces edits to 8 unrelated test files = scope creep per the finding's CAUTION. Deferred to a new follow-up kanban task ^nznnrwc (project ui-command-cleanup) with the full file list + move plan. Box checked with this note.

    Warnings:
    4. Four parallel file board run functions — collapsed to data: FileCommandSpec now carries verb + passPath; a single boardRun() interprets them into one run code path. file e2e green (verified routing still hits window.window.window.board[verb]).
    5. expose_backends (~85 lines) — extracted generic async fn expose_backend<S: ServerHandler+Send+Sync+'static>(host, name, server); the five repeated wrap-and-expose blocks collapse to expose_backend(host, name, server).await. Clean extraction, ~50 lines removed. app-shell e2e green.
    6. 33 metadata assertion functions — judged NOT a clean collapse, left as-is. They are deliberately heterogeneous (already share assert_keymap/assert_drag/assert_ui_surface_command/assert_no_keys/assert_no_menu/assert_single_param for the repeated parts) and several carry irreducibly per-command logic (assert_inspect_applies_to drift guard, entity.inspect global-scope negative assertions, field.edit context_menu+params) plus load-bearing per-command doc comments (which conflict card, why a key was dropped vs lowercased). A 12-optional-field struct table would be a worse abstraction and would destroy that rationale. Box checked with this note.
  timestamp: 2026-06-16T22:52:24.414765+00:00
- actor: claude-code
  id: 01kv9c7j18dvgq2z2tp66xnvfa
  text: |-
    Review findings worked (2026-06-16 17:58 batch — 0 blockers, 5 warnings, 2 nits, all quality/clarity).

    Warnings:
    1. app.ts nine `app.*` run closures collapsed to data — the symmetric counterpart to file-commands' boardRun. AppCommandSpec now carries routing fields `dispatch`/`service`/`verb`/`passScope?`; a single `appRun(spec)` interpreter walks the dispatch Proxy `bundle[dispatch][server][tool][service][verb](...)` (server == tool == DISPATCH_TOOL[dispatch]), threading scope_chain only when passScope. appCommands strips the routing-only fields and maps bindCommandRun({...metadata, run: appRun(spec)}, dispatch). GOTCHA caught by the real-isolate e2e: my first interpreter had only ONE intermediate proxy hop, but the original paths have TWO (app.app.app.about.show — bundle.app then .app.app.about.show), giving "server 'app' has no tool 'about'". Fixed to index `surface[segment][segment][service][verb]`; the e2e (executes all 9 app.* against live backends) now passes.
    2. `e`→`entry` in app-shell mirror test (both pluginEntries.map call sites).
    3. UIState vs UiStateServer acronym casing — these are external PUBLIC type names of swissarmyhammer-ui-state (UIState struct, UiStateServer struct); the e2e only references them. ~1209 refs across ~250 files; crate-wide rename = deferred CAUTION scope. Folded into follow-up ^nznnrwc (extended its description + comment, with RFC-430 note). NOT silently ignored.
    4. `rawCtx`→`rawContext` in sdk/commands.ts (all 4 occurrences).
    5. `write_plugin` duplication — verified the finding's line refs were stale: it's in 4 files (sdk.rs/callbacks.rs/plugin_host.rs/event_subscription_e2e.rs), and the copies are NOT identical (sdk.rs writes entry.ts + captures __result; the rest write index.ts + Promise<void>). Needs a parameterized shared helper; consolidating edits 3 files unrelated to this card. Folded into follow-up ^nznnrwc with the parameterization plan. NOT silently ignored.

    Nits 6 & 7: added the `/** The three keymap modes tested: vim, cua, and emacs. */` doc comment to MODES in both mirror test files.

    Gate (all green, fresh this round):
    - cargo nextest -p swissarmyhammer-command-service: 166/166 passed.
    - cargo nextest -p swissarmyhammer-plugin --test sdk: 9/9 passed (incl. bind_command_run_wraps_run_as_execute).
    - scoped vitest: 2 mirror guards 6/6; keybindings + app-shell + app-shell.nav/ai 132/132.
    - npx tsc --noEmit (apps/kanban-app/ui): clean (exit 0).

    Note: a full `cargo nextest -p swissarmyhammer-plugin` (whole crate) shows 2 pre-existing FAILs (file_notes_e2e::file_notes_plugin_round_trips_through_files_tool, example_layering_e2e::committed_examples_coload_across_layers) + 3 hot_reload timeouts. These are CWD/files-handler-dependent example-plugin e2e tests, fail in isolation too, and touch NONE of my changed files (app.ts, sdk/commands.ts, the two ui mirror tests). Pre-existing, unrelated to this card's scope. The card's defined gate (command-service + sdk + scoped vitest + tsc) is fully green.
  timestamp: 2026-06-16T23:28:26.664770+00:00
- actor: wballard
  id: 01kv9crfhnn2972s07x5rrs8ev
  text: |-
    Finish orchestrator — iteration 3 disposition / 3-iteration guardrail applied.

    Review trajectory: round 1 = 8 blockers, round 2 = 0 blockers (5 warnings/2 nits, all addressed), round 3 = 0 genuine blockers. Advancing to done.

    Round-3 (18:29) findings dispositioned:
    - "Blocker" (file-plugin-commands-mirror.spatial.node.test.ts:65, relative import missing .js) = VERIFIED FALSE POSITIVE. Line 65 is `for (const mode of MODES) {`, a for-of loop, not an import. The file's only relative import (line 36, `./plugin-command-table`) is extensionless — the prevailing convention for relative TS imports in this vitest project (cf. `@/lib/keybindings`). node: prefixes (lines 32-34) and the MODES doc comment (line 56) are correctly present. Engine misattributed both line number and content.
    - "Warning" (canonical*Keys duplication across the app./file./ai. mirror guards) = deferred-scope cosmetic churn; the ai.* guard isn't even this card's file. Folded into follow-up ^nznnrwc.

    Substantive work complete: key canonicalization (app.quit/search/undo/redo, file.openBoard) + the two new mirror drift guards; emacs app.search conflict resolved deliberately (key dropped). All 4 acceptance criteria met. Gate green: cargo nextest -p swissarmyhammer-command-service 166/166; swissarmyhammer-plugin --test sdk 9/9; scoped vitest (mirror guards + keybindings + app-shell) green; tsc --noEmit clean. Follow-up ^nznnrwc carries the genuinely crate-wide consolidations (copy_dir_recursive, write_plugin, UIState rename).
  timestamp: 2026-06-16T23:37:41.173755+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffbf80
project: ui-command-cleanup
title: Canonicalize remaining app-shell/file plugin key literals; extend the mirror drift guards to those bundles
---
## What
Follow-up prescribed by the Card I review (01KTED9JYGWM815K2X41N4QDBY, warning W1): finish the key-canonicalization sweep that Card I applied to `file.closeBoard` and the `ai.*` keys, and extend the `*-plugin-commands-mirror` drift-guard pattern to the app-shell and file bundles.

Since Card I deleted `app-shell.tsx`'s static scope defs, the plugin registry metadata is the ONLY key source for the webview hotkey path: `extractKeymapBindings` matches the declared string LITERALLY against `normalizeKeyEvent` output, which emits lowercase letters for unshifted chords. The following declared keys are therefore structurally unreachable from a real keydown (today the macOS chords ride native menu accelerators, which parse letters case-insensitively):

- `builtin/plugins/app-shell-commands/commands/app.ts` — `app.quit` cua `Mod+Q`; `app.search` cua `Mod+F` + emacs `Mod+F`; `app.undo` cua `Mod+Z`; `app.redo` vim `Ctrl+R`
- `builtin/plugins/file-commands/index.ts` — `file.openBoard` cua `Mod+O`

## Acceptance Criteria
- [x] No plugin-declared key literal in app-shell-commands / file-commands is unreachable by `normalizeKeyEvent` unless explicitly allowlisted as menu-accelerator-only with a comment.
- [x] The app.search emacs conflict (01KMT56FTBAP8PQ4QQND08MP97) is resolved deliberately, not by silent lowercasing.
- [x] New mirror guards fail RED on an uppercase regression (verify by temporarily re-uppercasing one key).
- [x] `cargo nextest run -p swissarmyhammer-command-service` green; scoped vitest (new guards + keybindings + app-shell suites) green; `npx tsc --noEmit` clean.

## Workflow
- `/tdd` — failing expectations first, then metadata changes.

## Review Findings (2026-06-16 17:27)

### Blockers
- [x] `app-shell-plugin-commands-mirror.spatial.node.test.ts` — `fs`/`path`/`url` imports now use the `node:` protocol prefix (`node:fs`, `node:path`, `node:url`).
- [x] `file-plugin-commands-mirror.spatial.node.test.ts` — `fs`/`path`/`url` imports now use the `node:` protocol prefix.
- [x] Duplicated `run`→`execute` bind pattern — extracted a shared `bindCommandRun<Surface>(spec, surface)` helper into the SDK (`crates/swissarmyhammer-plugin/src/sdk/commands.ts`), the natural home both bundles already import from (`@swissarmyhammer/plugin`). `app-shell-commands/commands/app.ts` and `file-commands/index.ts` both map via it. TDD: `sdk.rs::bind_command_run_wraps_run_as_execute` (real V8 isolate) + the `mod.rs` source-surface assertion, RED→GREEN.
- [x] Duplicated `copy_dir_recursive()` test util — PRE-EXISTING duplication across 10 integration test files; this card touched only 2 (`builtin_app_shell_commands_e2e.rs`, `builtin_file_commands_e2e.rs`). A shared support module exists (`tests/integration/support.rs`) but consolidating forces edits to 8 unrelated test files = scope creep per the finding's own CAUTION. Deferred to follow-up task `^nznnrwc` (project `ui-command-cleanup`) with the full file list + move plan. NOT silently ignored.

### Warnings
- [x] Four parallel `run` functions in `file-commands/index.ts` — collapsed to data: `FileCommandSpec` carries `verb` + `passPath`; a single `boardRun()` interprets them into one `run` code path. file e2e green.
- [x] `expose_backends` length — extracted generic `async fn expose_backend<S: ServerHandler + Send + Sync + 'static>(host, name, server)`; the five repeated wrap-and-expose blocks now call it. ~50 lines removed. app-shell e2e green.
- [x] 33 parallel per-command metadata assertion functions — judged NOT a clean collapse, left as-is. They are deliberately heterogeneous: the repeated parts already share `assert_keymap` / `assert_drag` / `assert_ui_surface_command` / `assert_no_keys` / `assert_no_menu` / `assert_single_param`, and several rows carry irreducibly per-command logic (`assert_inspect_applies_to` drift guard, `entity.inspect` global-scope negative assertions, `field.edit` context_menu + params) plus load-bearing per-command doc comments (which conflict card, why a key was dropped vs lowercased). A 12-optional-field struct table would be a worse abstraction and destroy that rationale.

## Review Findings (2026-06-16 17:58)

### Warnings
- [x] `app-shell-plugin-commands-mirror.spatial.node.test.ts` — arrow param `e` renamed to `entry` (both `pluginEntries.map(...)` call sites).
- [x] `builtin/plugins/app-shell-commands/commands/app.ts` — the nine `app.*` `run` closures collapsed to data, the symmetric counterpart to file-commands' `boardRun`. Added routing fields to `AppCommandSpec` (`dispatch: 'app'|'store'|'uiState'`, `service`, `verb`, `passScope?`); a single `appRun(spec)` interpreter walks the dispatch Proxy `bundle[dispatch][server][tool][service][verb](...)` (server == tool == the `DISPATCH_TOOL[dispatch]` value) threading `scope_chain` only when `passScope`. `appCommands` strips the routing-only fields and maps `bindCommandRun({...metadata, run: appRun(spec)}, dispatch)`. The real-isolate app-shell e2e (executes all nine `app.*` against live backends) is the regression guard — caught an initial missing proxy hop, fixed, now green.
- [x] `crates/swissarmyhammer-command-service/tests/integration/builtin_app_shell_commands_e2e.rs` — `UIState` vs `UiStateServer` acronym inconsistency. These are the actual PUBLIC type names of the external `swissarmyhammer-ui-state` crate (`UIState` struct in `src/state.rs`, `UiStateServer` struct in `src/service.rs`); the e2e test only references them — the inconsistency cannot be fixed in this test file without renaming the source structs crate-wide. `UIState` alone appears in ~1209 references across ~250 files — a crate-wide rename through many unrelated consumers, exactly this project's deferred CAUTION scope. Folded into follow-up `^nznnrwc` (with the RFC-430 PascalCase recommendation). NOT silently ignored.
- [x] `crates/swissarmyhammer-plugin/src/sdk/commands.ts` — parameter `rawCtx` renamed to `rawContext` (all four occurrences: two doc-comment mentions + the `bindCommandRun` `execute` body). sdk.rs (9 tests, incl. `bind_command_run_wraps_run_as_execute`) green.
- [x] `crates/swissarmyhammer-plugin/tests/sdk.rs` — `write_plugin` duplication. Verified state (finding line refs were stale): `write_plugin` lives in 4 files (`sdk.rs`, `callbacks.rs`, `plugin_host.rs`, `event_subscription_e2e.rs`), NOT the cited 6, and the copies are NOT byte-identical — `sdk.rs` writes `entry.ts` + captures `globalThis.__result`, the other three write `index.ts` + `load(): Promise<void>`. Consolidation needs a parameterized helper in a shared module and forces edits to 3 files unrelated to this card (which touched only `sdk.rs`) — this project's deferred CAUTION scope. Folded into follow-up `^nznnrwc` with the full parameterization plan. NOT silently ignored.

### Nits
- [x] `apps/kanban-app/ui/src/test/app-shell-plugin-commands-mirror.spatial.node.test.ts` — added `/** The three keymap modes tested: vim, cua, and emacs. */` doc comment to the `MODES` constant.
- [x] `apps/kanban-app/ui/src/test/file-plugin-commands-mirror.spatial.node.test.ts` — added the same `MODES` doc comment.

## Review Findings (2026-06-16 18:29)

### Blockers
- [ ] `apps/kanban-app/ui/src/test/file-plugin-commands-mirror.spatial.node.test.ts:65` — Relative import missing `.js` extension. ESM-first requires all relative imports to use full paths with explicit `.js` extensions. Add `.js` extension: `import { parseCommandTable } from "./plugin-command-table.js";`.

### Warnings
- [ ] `apps/kanban-app/ui/src/test/app-shell-plugin-commands-mirror.spatial.node.test.ts:49` — Near-match function `canonicalAppKeys` reimplements the same logic as `canonicalFileKeys` (file-plugin-commands-mirror.spatial.node.test.ts) and `canonicalAiKeys` (ai-plugin-commands-mirror.spatial.node.test.ts), differing only in the prefix filter. Rule of Three applies: three instances of the same pattern should be extracted into a parameterized helper instead of duplicated. Extract into a shared helper (e.g., in `plugin-command-table.ts`) parameterized by prefix: `function canonicalIdsByPrefix(prefix: string): Record<string, Record<string, Set<string>>> { ... }`. Each test calls it with `"app."`, `"file."`, or `"ai."` as needed. Reduces duplication and locks the three instances to a single canonical implementation.