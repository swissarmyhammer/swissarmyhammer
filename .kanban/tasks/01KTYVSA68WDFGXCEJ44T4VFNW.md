---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffa980
title: Delete fails on a perspective tab
---
## What

LIVE BUG (user-observed 2026-06-12): deleting a perspective from its tab fails. (User report verbatim: "delete fails on perspective tab".)

## Root cause (reconstructed from code; macOS log was empty for the window)

`perspective.delete` routed to the `views` server's `delete perspective` op, which only mutates STORAGE (`PerspectiveContext::delete`) and holds no `UIState`. So after deleting the ACTIVE perspective, the dispatching window's `active_perspective_id` still pointed at the just-deleted id — a dangling selection / "empty bar" that violates the never-zero invariant. The plain non-active delete worked; the failure was the active-perspective case with no selection fallback. (The just-landed switch/next/prev work moved ACTIVATION to the `entity` server, which holds KanbanContext + UIState; delete had not followed.)

## Fix

- Made the shared `DeletePerspectiveCmd` (swissarmyhammer-kanban) UIState-aware: after a successful delete, when the deleted id was the window's active selection, re-select a surviving perspective belonging to the active view via the existing atomic `switch_to_perspective`; when no survivor exists, clear the active id. No-op when no UIState in scope.
- Added a `delete perspective` op + handler to the `entity` MCP server (swissarmyhammer-entity-mcp), reusing `DeletePerspectiveCmd` — same backend split as switch/next/prev (views = resolution/storage, entity = per-window UIState write).
- Re-routed the plugin's `perspective.delete` (builtin/plugins/perspective-commands) from `views` to `entity`, threading the full `scope_chain` (perspective target + window moniker).
- Updated the entity `_meta` snapshot to include the new op.

## Delete-of-default semantics

Deleting the default is ALLOWED and never errors. The delete is a storage mutation + selection fallback; the never-zero recovery (the `if_absent` ensure path on save / open-reconciliation) recreates a Default afterward. Distinguished from the bug: the delete itself succeeds.

## Acceptance Criteria
- [x] Deleting a non-default perspective from the tab succeeds; tab disappears; if it was active, selection falls back sanely
- [x] Deleting the default perspective: allowed-and-recreated (never errors); documented above
- [x] Root cause documented (log empty → reconstructed from code)
- [x] Errors in the delete path are surfaced (command-error → mcp error, no silent swallow)

## Tests
- [x] Red-first test at the failing seam: `perspective_delete_of_active_falls_back_to_a_survivor` (e2e through the real plugin) — failed `left != right` on the dangling active id, now green
- [x] `perspective_delete_from_tab_scope_succeeds` pins the basic tab-scope delete
- [x] Crate-scoped suites green: entity-mcp (16/16), kanban+views (1373/1373), command-service (148/149; the one failure is the pre-known carded meta_tree `unregister.id` case). Tab-bar vitest 106/106. tsc exit 0.

## Constraints
- NO whole-workspace builds; no kanban-app crate compile. Never touch .kanban/actors/wballard.jsonl. Did not touch swissarmyhammer-ui-state.

## Workflow
- /tdd.

## Review Findings (2026-06-13 07:46)

Verified end-to-end: route, fallback logic, tests re-run, red-green probe (neutered `reselect_after_delete` → `perspective_delete_of_active_falls_back_to_a_survivor` went red with `left == right` on the dangling id; restored, probe fully reverted, `cargo check -p swissarmyhammer-kanban` clean). Suites confirmed fresh: entity-mcp 16/16, views 80/80, kanban 1293/1293 (= 1373 combined), command-service 148/149 (sole failure `meta_tree::meta_tree_id_param_is_required_where_expected` = `unregister.id required flag`, unrelated, carded 01KTCBG5GP4FS50ZFPKSSN2H6Q), tab-bar vitest 106/106, tsc exit 0. Entity meta_snapshot has the new `("perspective","delete","delete perspective")` row; views meta drops next/prev/switch and asserts 15. Window moniker required on the entity delete op (no silent main fallback) and `window-container.tsx` wraps all dispatch in `window:${WINDOW_LABEL}`, so the hardening cannot break production deletes.

### Warnings
- [x] `crates/swissarmyhammer-kanban/src/commands/perspective_commands.rs:496` — `reselect_after_delete` calls `switch_to_perspective(...)` (which produces a `UIStateChange::PerspectiveSwitch`) but discards its return value; `handle_delete_perspective` (entity server) then returns the kanban delete op's `{deleted,id,name}` result, which has no `change` field. `apps/kanban-app/src/commands.rs::ui_state_change_kind` only emits `ui-state-changed` when it can unwrap a `UIStateChange` from `structuredContent.change`/`change`/raw — so a delete of the ACTIVE perspective writes the new selection server-side but fires NO `ui-state-changed`/`perspective_switch` event. The app only updates because the independent frontend `useAutoSelectActivePerspective` re-runs on the perspective LIST change and dispatches its own repair `perspective.switch`. The backend reselect is therefore effectively invisible to the UI (works for headless/agent dispatch only). This is the same missing-emit class the keystone comment at `commands.rs:2063` warns about. Either return/forward the `PerspectiveSwitch` change from `handle_delete_perspective` (e.g. `{ ok, change }` with the reselect's change) so the emit fires, or document that the frontend list-reconciliation is the intended UI driver and the backend reselect is server-state-only.
  - FIXED: `reselect_after_delete` now RETURNS the `switch_to_perspective` change (`Value`, or `Null` when there is no survivor / not the active perspective / no UIState). `DeletePerspectiveCmd::execute` folds that change into the delete result as a `change` field. `handle_delete_perspective` extracts it and returns `{ ok: true, change }` — the exact envelope switch/next/prev ride — so the host's `ui-state-changed` emit fires for the new selection. Pinned by `perspective_delete_of_active_forwards_the_reselect_change` (asserts `structuredContent.result.structuredContent.change.PerspectiveSwitch.perspective_id == survivor`).
- [x] `crates/swissarmyhammer-views/src/operations.rs:114` + `crates/swissarmyhammer-views/src/server.rs:297,637` — the views `delete perspective` op/handler is now an orphaned production path. Before this change it was the SOLE production caller's target (`views.views.views.perspective.delete` in the old `lifecycle.ts`); the reroute moved `perspective.delete` to `entity`, leaving nothing in production routing to views delete (only `views_e2e::list_rename_delete_lifecycle` + the views meta_snapshot pin reference it). This is the identical dead-path condition as next/prev/switch, which this same change DELETED from views. Treat it consistently: remove the views `delete perspective` op + handler + its `views_e2e` coverage + meta_snapshot row (drop to 14 ops), OR card the decision with a rationale for keeping it as views-CRUD-completeness (note load/save/rename/list still route to views, so only delete is orphaned).
  - FIXED (deleted consistently): grep-confirmed nothing in production (frontend/plugin/other crates) routes to the views delete op — the only references were inside swissarmyhammer-views itself. Removed: `DeletePerspective` op struct (operations.rs), its `VIEWS_OPERATIONS` entry, the `DeletePerspective` import, `handle_delete` + the `"delete perspective"` dispatch arm (server.rs), and the delete portion of `views_e2e::list_rename_delete_lifecycle` (renamed `list_rename_lifecycle`; list+rename still route to views so they stay). Views meta_snapshot dropped 15→14 ops. `-p swissarmyhammer-views` green 80/80.

### Nits
- [x] `crates/swissarmyhammer-command-service/tests/integration/builtin_perspective_commands_e2e.rs:1214` — stale comment: "The plugin port resolves the id off that moniker and routes to the views `delete perspective` op." Delete now routes to the `entity` server, not views. Update the comment block (lines ~1211–1216) to match the new route.
  - FIXED: comment now says the plugin routes to the `entity` server's `delete perspective` op (not views), with the per-window UIState rationale.
- [x] `crates/swissarmyhammer-kanban/src/commands/perspective_commands.rs:498` — the no-survivor branch (`None => ui.set_active_perspective(window_label, "")`, i.e. delete the only perspective for a view → active id cleared) is documented in the task semantics but has no test. Add an e2e pin asserting the active id is cleared (and, if intended, that never-zero recovery recreates a Default end-to-end). Currently both delete e2e tests exercise only the survivor-present path.
  - FIXED: added `perspective_delete_of_the_only_perspective_clears_the_active_id` (e2e through the real plugin) — switches to the only perspective, deletes it via tab scope, asserts the window's active id is cleared (`""`). Never-zero recovery is external (save/open reconciliation), so the test asserts only the clear.

## Review Findings (2026-06-13 13:10)

Iteration 2 verification. Re-verified all four prior items in code (not just marked) — all HONEST and clean:
- W1 (reselect forwarding): `reselect_after_delete` returns the switch change (perspective_commands.rs:483-523); `DeletePerspectiveCmd::execute` folds it as `change` (429-432); entity `handle_delete_perspective` returns `{ok, change}` identical to switch/next/prev (server.rs:668-694); frontend `ui_state_change_kind` unwraps `structuredContent.change` first and recognizes `PerspectiveSwitch` (commands.rs:2068-2093) — emit now fires on the same path. Pinned + red-first credible by `perspective_delete_of_active_forwards_the_reselect_change` (e2e:1323-1353). Non-active edge returns Null (492-494). CLEAN.
- W2 (orphaned views delete deleted): whole-repo grep shows zero production routes to a views delete op; `lifecycle.ts:102` routes to entity. operations.rs `VIEWS_OPERATIONS` is exactly 14 with no `DeletePerspective`; server.rs has zero dangling refs/handler/dispatch arm; meta_snapshot pins 14 and cross-checks the wire op enum. CLEAN.
- NIT 1: e2e comment (1211-1217) correctly says routes to `entity`, not views. CLEAN.
- NIT 2: `perspective_delete_of_the_only_perspective_clears_the_active_id` (e2e:1361-1385) asserts active id == "". CLEAN.

Fresh re-run `cargo nextest run -p swissarmyhammer-entity-mcp -p swissarmyhammer-views -p swissarmyhammer-kanban -p swissarmyhammer-command-service`: 1540 run, 1539 passed, 1 failed. Sole failure is the carded `meta_tree::meta_tree_id_param_is_required_where_expected` (01KTCBG5GP4FS50ZFPKSSN2H6Q). No other failures. All 4 delete tests pass.

### Nits
- [x] `crates/swissarmyhammer-views/src/operations.rs:18-21` — stale module-level doc comment introduced/left by the W2 deletion: it still said "The **fifteen** operations group into six sub-domains" and listed `delete` in the lifecycle sub-domain (`load, save, delete, rename, list`). The crate is now 14 ops with no delete op.
  - FIXED: module doc now reads "The fourteen operations group into six sub-domains" and the lifecycle line lists `load, save, rename, list` (no `delete`), matching the verified VIEWS_OPERATIONS slice (14 ops; lifecycle = LoadPerspective, SavePerspective, RenamePerspective, ListPerspective) and the meta_snapshot. `cargo check -p swissarmyhammer-views` exit 0.