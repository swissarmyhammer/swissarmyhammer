---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffa580
title: Perspectives gone missing — default perspective creation raced into duplicates, then none; ensure-default must be idempotent with recovery
---
## What

LIVE BUG (user-observed): perspectives have gone missing entirely. Expected invariant: **there is always a default perspective**. Observed history: duplicate defaults were being created earlier, and now there are NONE.

This smells like a non-idempotent ensure-default racing (multiple windows / board opens each creating "Default" → duplicates) followed by some dedup/cleanup/delete path that removed all of them — or a validation/load change that now rejects the perspective files entirely (cf. the views-crate degenerate-def skip added in card 01KTCRY5W2BP7TYTHV4JB9CH8K — if a similar skip/validation got applied to perspectives, corrupted/duplicate perspective files may now be silently skipped on load, presenting as "none").

## Forensics FIRST (do not guess)

1. **On-disk state**: inspect `.kanban/perspectives/` in this repo's board (NOTE: the working tree currently has 5 UNTRACKED perspective file pairs from today — `01KTXMMDH20DQYNFG2PRCSR9WQ`, `01KTXMSXHXHK34S2X96BK8R1BH`, `01KTXMSZVCRZ22XYDSFENJK215`, `01KTXMVDCDGXC388GRFJ76JDHH`, `01KTXMW3X17JKMHY1KH4HTZH14` — these may BE the duplicate defaults). Read them: are they duplicates of a default? Valid or degenerate? Compare with the committed perspective files (git ls-files .kanban/perspectives/).
2. **The unified log**: `log show --predicate 'subsystem == "com.swissarmyhammer.kanban"' --last 2h | grep -iE 'perspectiv|skip|degenerate|duplicate|default'` — find creation/skip/delete events.
3. **Code paths**: where is the default perspective ensured (board open? perspectives context build? frontend PerspectivesContainer?) — find the creation trigger and why it can run more than once (per-window? per-webview-mount? StrictMode double-effect like the layer push/pop race fixed in 01KTQCHWP5T4GS8SPGYVXD2CT9?). Where are perspectives loaded/listed (swissarmyhammer-kanban perspectives module; `list perspectives` op) and is there any validation/skip that could now reject all of them? Any dedup/cleanup that deletes?

## Required outcome

- **Idempotent ensure-default**: exactly one default perspective exists after any number of board opens / window opens / concurrent mounts. Creation must be guarded at the STORAGE layer (check-then-create under the board lock or keyed by a stable id — e.g. the default perspective gets a deterministic well-known id so a second create is a no-op upsert), not by frontend politeness.
- **Recovery**: a board with zero perspectives gets its default (re)created on open/load — the user must never see an empty perspective bar.
- **Dedup with data preservation**: if duplicates exist (as on this board now), converge to one default without losing any non-default user perspectives; document what happens to the extra defaults (merge/remove).
- Fix the current board's state as part of verification (the app should self-heal it via the recovery path — not by hand-editing files).

## Acceptance Criteria
- [x] Fresh board open → exactly one default perspective
- [x] Two windows opening the same board concurrently → still exactly one default (no duplicates)
- [x] Board with zero perspectives (current state) → default recreated automatically on open; perspective bar renders
- [x] Board with N duplicate defaults → converges to one; user-created perspectives untouched
- [x] Root cause documented with log/file evidence: what created the duplicates, what removed all of them (review-verified: 231 deleted YAMLs at HEAD all vanilla `name: Default`; minting cause documented in the `ensure_default.rs` module doc)

## Tests
- [x] Rust test: ensure-default is idempotent (call twice → one perspective; concurrent calls → one)
- [x] Rust test: load with zero perspectives → default created; load with duplicate defaults → converges to one, others preserved/merged per the documented semantics
- [x] Real-pipeline test (per fixture-only-anti-pattern): drive through the actual board-open path, not raw inserts
- [x] `cargo nextest run -p swissarmyhammer-kanban` green (full crate: 1291/1291 after review fixes — was 1277 at card creation, 1287 at review)

## Constraints
- NO whole-workspace cargo build/clippy; crate-scoped only. Frontend changes (if any) scoped vitest + tsc.
- Use tracing (never eprintln); read the unified log yourself.
- Do NOT touch .kanban/actors/wballard.jsonl. The 5 untracked perspective files are EVIDENCE — read them before any cleanup; the fix should make the app converge them, not hand-delete them.

## Workflow
- Use `/tdd` — failing test first (idempotence + zero-recovery), then fix.

## Review Findings (2026-06-12 12:19)

Verified: 1287/1287 crate tests green (fresh run); red-green probe re-confirmed (reconcile call disabled in `KanbanContext::open` → 5/6 recovery integration tests fail, restored → 6/6 pass; working tree restored to the implementer's exact diff). Board audit: all 231 deleted perspective YAMLs at HEAD were vanilla `name: Default` with zero customization (no filter/group/sort/fields, no non-Default names) — the self-heal on this board lost nothing user-authored. Current `.kanban/perspectives/`: 11 files = 7 user perspectives intact + 4 scoped Defaults. Views are loaded before reconciliation in `open` (ordering safe), `prune` bails when the view registry is empty, and `is_customized` covers every user-editable knob on `Perspective`. Frontend `useAutoCreateDefaultPerspective` is unchanged but now routes `if_absent` through the storage-layer ensure with deterministic ids, making its re-fires harmless (no frontend diff → no vitest/tsc needed).

### Blockers
- [x] `crates/swissarmyhammer-kanban/src/perspective/ensure_default.rs:129` — `dedup_defaults_per_scope` deletes ALL non-keeper duplicates including CUSTOMIZED ones (...) DONE: deletion loop now filters to vanilla duplicates only; module doc + pass doc updated; red-green unit test `dedup_preserves_all_customized_defaults_sharing_one_scope` added (red: customized 01BBB deleted, left:1 right:2 → green after fix).

### Warnings
- [x] `crates/swissarmyhammer-kanban/src/perspective/add.rs:154` — ensure path embeds caller-supplied `view_id` verbatim (...) DONE: ensure mode now validates `view_id` against the views registry (fall back to kind scope when not found); `is_safe_scope_component` filename guard as backstop. Red-green via three real-pipeline tests in `tests/perspective_default_recovery.rs`.
- [x] Stale perspective cache routed around, not fixed (...) DONE: follow-up card filed — `01KTYE4VCQ33KWH493WZN7C7V9`.

### Nits
- [x] Legacy prune all-or-nothing residual duplicate tab — DONE: documented on `prune_unreachable_defaults`.
- [x] Task checkboxes flipped honestly — DONE.

## LIVE REGRESSION re-run (2026-06-12 13:5x) — root cause: plugin-cutover wiring gap

User restarted the app (debug binary mtime 13:13:11, inode-verified as the running image, PID 76645) and STILL saw no Default perspectives. Forensics:

**The fix was wired into a path production no longer dispatches.** The app's Stage-4 command cutover routes ALL commands through `CommandService` fed by the builtin TS plugins (`apps/kanban-app/src/commands.rs::dispatch_via_service`: "the legacy command_impls fallback is retired"). `perspective.save`/`perspective.list` are served by `builtin/plugins/perspective-commands/commands/lifecycle.ts` → the `views` MCP tool (`crates/swissarmyhammer-views/src/server.rs`) — NOT by the fixed `SavePerspectiveCmd`/`AddPerspective` in swissarmyhammer-kanban. The board-open reconciliation itself IS wired (`BoardHandle::open_with` → `KanbanContext::open`) and ran fine at 13:13:12–27 (unified log: "converging duplicate default perspectives", "removing default perspective pinned to a nonexistent view view_id=default", "recovering default perspective ... default-01JMVIEW0000000000BOARD0").

**Bug 1 (the user-visible one): `perspective.list` returned the raw MCP CallToolResult envelope.** The plugin returned `views.perspective.list()`'s wire envelope verbatim; the frontend (`usePerspectivesFetch`) reads `result.perspectives` → always `undefined` → every window's perspectives state stayed `[]` forever → tab bar rendered NO perspective tabs (scope chains stuck at `perspective:default`, log-verified at 13:15 while backend list returned 11 perspectives). Fix: `unwrapResult(...)` in lifecycle.ts (the SDK convention other plugins already use).

**Bug 2 (the duplicate-minting regression): `ViewsServer::handle_save` had no ensure semantics.** Because the frontend list stayed empty, `useAutoCreateDefaultPerspective` fired `perspective.save {if_absent:true}` per window per boot; the plugin injected `view_id` from the scope chain — which carries the literal `"default"` placeholder (`view-container.tsx`: `activeView?.id ?? "default"`) before views load — and `handle_save` minted a fresh ULID Default pinned to dead view id `default` every time (log 13:13:30: 4 mints across 3 boards in one boot; on-disk `01KTYGM0W4...`/`01KTYGM0WX...` both `view_id: default` on the same board). Next open's reconcile prunes them → the exact create/prune churn the review warned about. Fix: `if_absent` field on the `SavePerspective` op + ensure branch in `handle_save` (registry-validated view_id with kind fallback, existing-scope short-circuit without write, deterministic `default-<scope>` id).

**Single source of truth:** the scope invariants (deterministic id, matching rule, filename safety, is_customized) moved to `swissarmyhammer_perspectives::default_scope`; kanban's `ensure_default` and the views server both consume them (kanban → views → perspectives dependency direction).

**Red-green proof:**
- `crates/swissarmyhammer-views/tests/integration/ensure_save.rs` (5 tests, production wire path via `call_tool("save perspective", {if_absent})`): RED 5/5 on current code (ULID ids minted, view_id verbatim, duplicates) → GREEN after the ensure branch.
- `crates/swissarmyhammer-command-service/tests/integration/builtin_perspective_commands_e2e.rs::perspective_list_returns_the_op_payload_for_the_frontend` (full path: command service → real TS plugin → views tool): RED (panic output shows the double-wrapped envelope) → GREEN after the lifecycle.ts unwrap.
- `...::if_absent_save_through_the_plugin_converges_on_one_default` pins the full production path with the live `view:default` scope-chain shape: two dispatches → ONE `default-board` perspective, view_id fallback applied.

**Verification:** swissarmyhammer-kanban 1290/1290 (one helper unit test moved to perspectives, which gained 2); views+perspectives 149/149 + unit tests; command-service green except pre-existing `meta_tree_id_param_is_required_where_expected` (stash-verified failing at HEAD; follow-up card 01KTYK0DZCTCRCQEPQC6SEA8W2). No kanban-app UI files touched → no vitest/tsc. The dead `view_id: default` files on the three live boards self-heal at next board open via the existing prune (registry-backed).

**User verification:** restart `tauri dev` (the TS plugin is bundled via include_dir, so the app binary rebuild picks it up automatically); the tab bar should now show the scoped Default tabs, and `.kanban/perspectives/` must NOT grow new ULID `Default` files on subsequent boots.

## Review Findings (2026-06-12 14:07)

Iteration-3 verification (all evidence fresh this session): four-crate run `cargo nextest run -p swissarmyhammer-views -p swissarmyhammer-perspectives -p swissarmyhammer-kanban -p swissarmyhammer-command-service` → 1583 tests, 1582 passed, 1 failed — the single failure is the carded pre-existing `meta_tree_id_param_is_required_where_expected` (card 01KTYK0DZCTCRCQEPQC6SEA8W2 verified on the board). Red-green probe re-observed: re-wrapping the `perspective.list` return in lifecycle.ts → `perspective_list_returns_the_op_payload_for_the_frontend` FAILS with the double-wrapped envelope in the panic output; restored → it and `if_absent_save_through_the_plugin_converges_on_one_default` PASS; `git diff` confirms lifecycle.ts restored to the implementer's exact diff (10 insertions, 1 deletion). Other-ops audit of lifecycle.ts: save/delete/rename/load returning the raw envelope is the codebase convention (every builtin plugin returns envelopes; `unwrapResult` only where a payload field is read) and no production frontend consumer reads payload fields off those four (`usePerspectiveRename` only awaits + refetches; auto-create save ignores its result) — no further double-wrap bug. Single source of truth verified: the five scope helpers exist only in `swissarmyhammer-perspectives/src/default_scope.rs`; kanban's `ensure_default.rs` re-exports them and `add.rs` + `ViewsServer::handle_ensure_save` both consume them; grep finds no residual `default-` minting or helper copies. Owner directive: (a) existing perspectives short-circuit the ensure without a write (server.rs `handle_ensure_save`, kind-fallback match) and board-open recovery bails when any perspective exists; (b) zero → exactly one deterministic `default-<scope>`, concurrent dispatches converge (pinned by ensure_save.rs + the plugin e2e); (c) defaults are NOT pre-minted for sibling views — creation only happens for a scope actually dispatched against, which is exactly "don't always need a new default". One gap against the tab-bar-never-empty reading, below.

### Warnings
- [x] apps/kanban-app/ui/src/lib/perspective-context.tsx:135 — `useAutoCreateDefaultPerspective` guards with `perspectives.some((p) => p.view === viewKind)` (kind-only), but the tab bar filters view_id-first (`perspective-tab-bar.tsx:229`: pinned perspectives render only on their pinned view). Reachable empty-bar: a kind with views A and B where every perspective of the kind is pinned to A (post-cutover saves pin via the scope-chain `view_id` injection in lifecycle.ts) and B's Default was deleted — B's tab bar renders empty, the kind-only guard sees a same-kind perspective and never dispatches the ensure, and board-open recovery doesn't fire (global count nonzero). Violates the owner's never-zero invariant at the tab-bar level even though the backend `matches_scope((Some(B), Some(A)))` would correctly mint `default-B` if asked. Fix: make the guard mirror the tab-bar predicate (`p.view_id != null ? p.view_id === activeViewId : p.view === viewKind`) and key `autoCreatedForKindRef` per active view id (falling back to kind pre-views-load); scoped vitest for the pinned-elsewhere case. DONE (iteration 4): predicate extracted as `perspectiveVisibleInView` in `types/kanban.ts` (next to the `PerspectiveDef` rule doc, a module no tab-bar test mocks); BOTH the guard and the tab bar's `filteredPerspectives` memo now call it, so the two can never drift; re-fire ref renamed `autoCreatedForViewRef` and keyed per view instance (`activeViewId ?? viewKind` kind fallback pre-views-load). Red-green in `perspective-context.test.tsx` (views mock made mutable): `auto-creates a Default when every same-kind perspective is pinned to a different view` and `auto-creates again when switching to a same-kind sibling view whose tab bar is empty` RED on the kind-only guard (`expected +0 to be 1`) → GREEN after the fix; anti-over-mint pinned-to-active-view + legacy-kind-shared tests assert zero `perspective.save` dispatches. Verification: perspective-context 27/27; tab-bar suite + perspectives-container 17 files / 95 tests green; `tsc --noEmit` exit 0; prettier clean.

## Review Findings (2026-06-12 14:24)

Iteration-4 verification (all evidence fresh this session): `perspectiveVisibleInView` in `apps/kanban-app/ui/src/types/kanban.ts` is a faithful extraction of the tab bar's previous inline filter (`view_id != null ? view_id === activeViewId : view === viewKind` — confirmed against the HEAD diff); BOTH the auto-create guard (`perspective-context.tsx`) and the bar's `filteredPerspectives` memo (`perspective-tab-bar.tsx`) import and call the SAME function — no copy, no drift possible. Diff stat matches the claim exactly (4 files, +208/-28). Scoped runs: perspective-context 27/27 green; tab-bar + container filter → 19 files / 107 tests green (superset of the claimed 17/95); `npx tsc --noEmit` exit 0. Red-green probe re-observed: guard reverted to kind-only → exactly the 2 new tests fail with `expected +0 to be 1`; restored → 27/27 green; `git diff` confirms perspective-context.tsx back to the implementer's exact diff (+36/-10). Anti-over-mint pins are real: pinned-to-active-view and legacy-kind-shared tests both assert zero `perspective.save` dispatches. Edge cases hold: a perspective pinned to a deleted view is invisible everywhere → bar empty → ensure fires and the backend registry-validates `view_id` with kind fallback (iteration-2/3 ensure path); kind-fallback perspectives count toward visibility (pinned); pre-views-load `activeViewId === undefined` makes pinned perspectives match nothing — same as the bar — and the backend `if_absent` ensure keeps any resulting race idempotent. One gap found by mutation testing, below.

### Warnings
- [x] `apps/kanban-app/ui/src/lib/perspective-context.test.tsx` — the per-view re-fire keying has NO regression coverage: mutating `const viewKey = activeViewId ?? viewKind` to `const viewKey = viewKind` (the old kind-only key) passes all 27 tests (mutation run fresh this session). The sibling-switch test never sets the ref before switching — board-1 has a visible pinned perspective, so the guard returns before recording the key — so that test goes red on the predicate change only, not the keying. Reachable bug if the keying regresses: empty view A fires the ensure and records the kind key; switching to an empty same-kind sibling B is then permanently blocked (deps re-run but the ref short-circuits) — B's bar stays empty, violating never-zero. Fix: add a test where the ensure fires on empty view A first, THEN switching to empty same-kind sibling B fires a second `perspective.save` (2 total dispatches). DONE (iteration 5): added `auto-creates again on an empty same-kind sibling view after firing on an empty view (re-fire ref keyed per view instance)` to `perspective-context.test.tsx` — empty board-1 fires the ensure (1 save, ref records key), switch to empty same-kind sibling board-2 fires again (2 total). Red-check executed: green on correct code (28/28); production keying mutated to `viewKey = viewKind` → exactly this test fails (`AssertionError: expected 1 to be 2`, 1 failed | 27 passed); restored (git diff confirms exact implementer diff) → 28/28 green. `npx tsc --noEmit` exit 0; prettier clean.

### Nits
- [x] `apps/kanban-app/ui/src/lib/perspective-context.tsx` — `useAutoSelectActivePerspective` (and the `activePerspective` memo fallback) still match kind-only (`p.view === viewKind`), so the auto-selected/active perspective can be one pinned to a sibling view and invisible in the active view's bar. Pre-existing and not made worse by this change; consider routing both through `perspectiveVisibleInView` in a follow-up card. DONE (iteration 5): follow-up card filed — `01KTYMYKX2V4WVW7DPMBEYCVX9` (exact file/line refs, symptom, fix direction via `perspectiveVisibleInView` in `types/kanban.ts`).