---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kv5zyn9976wb921p3q2b8v2b
  text: Picked up in /finish $builtin-commands (scoped-batch). Task was sitting in `review` with no prior comments — running /review to determine whether the field-pill spatial-nav fix is actually clean or needs implement work.
  timestamp: 2026-06-15T15:56:08.873954+00:00
- actor: claude-code
  id: 01kv60yxtt4b6cajxg3v3k8q3d
  text: |-
    Addressed all 3 Review Findings in entity-inspector.field-enter-drill.browser.test.tsx (test-quality refactors only, no behavior change):

    1. Blocker (filter-helper duplication): added shared `filterIpcCalls(cmd, op)` near the other filter helpers; replaced both inline `mockInvoke.mock.calls.filter(...)` sites with `filterIpcCalls("spatial_navigate", "navigate focus")` and `filterIpcCalls("spatial_drill_out", "drill_out layer")`.
    2. Warning (defaultInvokeImpl monolith): extracted `handleEntityCommand(cmd, args)` (entity/schema/ui-state/undo/dispatch) and `handleSpatialCommand(cmd, args)` (drill in/out, navigate, scope register/unregister, focus/clear-focus). They return an `UNHANDLED` sentinel so `defaultInvokeImpl` falls through correctly even when a handler intentionally returns `undefined`. defaultInvokeImpl is now ~15 lines: list-command answer, then entity, then spatial. Also collapsed a latent duplicate condition `cmd === "spatial_register_scope" || cmd === "spatial_register_scope"` into a single check (behavior-preserving — duplicate was a no-op).
    3. Warning (abbreviated `c`): the shared `filterIpcCalls` uses `ipcCall` as its callback parameter, subsuming both rename sites.

    Verified: `cd apps/kanban-app/ui && npx vitest run src/components/entity-inspector.field-enter-drill.browser.test.tsx` → Test Files 1 passed, Tests 6 passed (6), exit 0. Both named tests (right_from_first_pill_lands_on_second_pill, escape_from_pill_drills_back_to_field_zone) plus the other 4 stay green. Assertions unchanged. Flipped all 3 finding checkboxes to [x]. Moving back to review.
  timestamp: 2026-06-15T16:13:46.202717+00:00
- actor: claude-code
  id: 01kv6275zkha5rttz9r5ma7ncs
  text: |-
    Addressed Review Findings (2026-06-15 11:14).

    NITS (3) — fixed in entity-inspector.field-enter-drill.browser.test.tsx. Renamed the new helper params to descriptive names: handleEntityCommand(command, commandArgs), handleSpatialCommand(command, commandArgs), defaultInvokeImpl(command, commandArgs), updating all in-body references. Note: the review suggested `arguments`, but TS ES modules run in strict mode where `arguments` is a reserved binding name (SyntaxError as a param), so used the descriptive `commandArgs` instead. All 3 nits flipped to [x].

    BLOCKER + 2 WARNINGS (shared mock-spatial-kernel extraction) — DEFERRED, not done in this card. The blocker/warnings ask to extract UNHANDLED, handleEntityCommand, handleSpatialCommand, filterIpcCalls + supporting maps (drillInResponses, monikerToKey, currentFocusKey, listeners) into a shared test util because the scaffolding is duplicated near-verbatim (0.96–1.00 similarity) across 5+ sibling spatial test files (board-view.enter-drill-in, inspectable.space, entity-inspector.spatial-nav, field.spatial-nav, app-shell, perspective-tab-bar.filter-enter.spatial, etc.). That is a genuinely separate cross-file refactor spanning files unrelated to this card. This card's scope is strictly "fix 2 failing field-pill spatial-nav browser tests" — done (all 6 tests pass, production wiring fixed). Created follow-up card 6a45spw (01KV6250AH0DPRMG9SJ6A45SPW) in project builtin-commands, tagged frontend/refactor/test, capturing the duplicated scaffolding, the ~5 sibling files, and the target shared module path (apps/kanban-app/ui/src/test/mock-spatial-kernel.ts). Flipped the blocker/2 warnings to [x] with inline deferral notes pointing at 6a45spw.

    Verified after the nit renames: `cd apps/kanban-app/ui && npx vitest run src/components/entity-inspector.field-enter-drill.browser.test.tsx` → Test Files 1 passed, Tests 6 passed (6), exit 0. Assertions unchanged. Moving back to review.
  timestamp: 2026-06-15T16:35:45.267103+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffb180
project: builtin-commands
title: Fix 2 failing field-pill spatial-nav browser tests (right/escape don't dispatch nav commands)
---
## Problem

Two browser tests in `apps/kanban-app/ui/src/components/entity-inspector.field-enter-drill.browser.test.tsx` fail (the other 4 in the file pass):

- **`right_from_first_pill_lands_on_second_pill`** (L607): after seeding the first tag pill (`tag:tag-bug`) as focused inside a field zone, firing `ArrowRight` should make the global `nav.right` command dispatch `spatial_navigate({ focusedFq: bugPill.fq, direction: "right" })`. **Asserts `navCalls.length === 1`; actual `0`.**
- **`escape_from_pill_drills_back_to_field_zone`** (L690): with the bug pill focused, firing `Escape` should make `nav.drillOut` dispatch `spatial_drill_out(pillFq)` and forward the kernel-returned field-zone moniker through `FocusActions.setFocus`. **Asserts `drillOutCalls.length === 1`; actual `0`.**

Zero dispatches means the keydown (`ArrowRight` / `Escape`) is no longer resolving through the keymap to the `nav.right` / `nav.drillOut` command closures **when focus is on a pill nested inside a field zone**. The "Enter on a field zone" cases in the same file still pass, so the field-zone scope itself works — the break is specific to the spatial-nav (arrow/escape) bindings reaching a pill leaf.

## Root-cause suspect

The file's last touch was commit `6dc3b7d07` *"refactor(commands)!: rename ui.\* command ids to app.\*; fold ui-commands into app-shell-commands"* (preceded by `a13f720ae` *"move grid.\* and field/pressable commands to builtin plugins"*). Strong suspicion: the keymap binding / command registration that routes `ArrowRight`→`nav.right` and `Escape`→`nav.drillOut` for a pill leaf inside the `ui:field` marker scope was lost or mis-wired in that rename/move (e.g. a stale `ui:*` id in the keymap chain walk, or the nav binding no longer applying within the field-zone marker subtree). Confirmed pre-existing (independent of the recently-landed `field.tsx` "Edit Field" work — reverting `field.tsx` to HEAD reproduces both failures identically).

## What

- Bisect to confirm whether `6dc3b7d07` / `a13f720ae` introduced the regression: `git stash` not needed — run the test at those SHAs (`git -C . show`/checkout in a scratch clone, or inspect the diff) to find where the `nav.right` / `nav.drillOut` binding stopped reaching a pill-in-field leaf.
- Fix the root cause in the command/keymap wiring (likely in `builtin/plugins/nav-commands/index.ts`, `builtin/plugins/app-shell-commands/`, or the frontend keymap chain-walk / `CommandScopeProvider` for the `ui:field` marker — `apps/kanban-app/ui/src/lib/command-scope.tsx` and the field-zone scope in `apps/kanban-app/ui/src/components/fields/field.tsx`). Do NOT weaken the test assertions or convert them to no-ops.
- The fix must restore: arrow-key spatial nav and Escape drill-out dispatching the correct `spatial_navigate` / `spatial_drill_out` MCP calls for a focused pill leaf nested in a field zone.

Note: as of the latest commits the broader `swissarmyhammer-command-service` suite is green; this is an isolated frontend test gap.

## Acceptance Criteria
- [ ] `right_from_first_pill_lands_on_second_pill` passes: one `spatial_navigate` call with `{ focusedFq: bugPill.fq, direction: "right" }`.
- [ ] `escape_from_pill_drills_back_to_field_zone` passes: one `spatial_drill_out` call with `{ fq: bugPill.fq }`, and the returned field-zone moniker is forwarded via `setFocus`/`spatial_focus`.
- [ ] The other 4 tests in the file still pass (no regression to the Enter/edit/drill-in cases).
- [ ] Assertions are unchanged (or strengthened) — the fix is in production wiring, not the test.

## Tests
- [ ] `cd apps/kanban-app/ui && npx vitest run src/components/entity-inspector.field-enter-drill.browser.test.tsx` → all 6 pass (currently 2 fail / 4 pass).
- [ ] If the root cause is a keymap/binding wiring bug shared by other surfaces, add/extend a focused unit test at the wiring layer (e.g. the keymap chain-walk for a marker-scoped command) so the regression can't silently return.
- [ ] Run the surrounding frontend suite for the touched area (`npx vitest run src/components/fields src/components/entity-inspector`) to confirm no collateral breakage.

## Workflow
- Use `/tdd` — the two failing tests already encode the expected behavior; run them red first, root-cause the binding break, then fix the wiring until both (and the other 4) are green. #Test-Failure #navigation #frontend

## Review Findings (2026-06-15 10:56)

### Blockers
- [x] `apps/kanban-app/ui/src/components/entity-inspector.field-enter-drill.browser.test.tsx:662` — Two filter helpers differ only by command type and op name — should be one parameterized function to avoid drift when the mock pattern evolves. Extract a shared helper: `function filterIpcCalls(cmd: string, op: string) { return mockInvoke.mock.calls.filter(c => c[0] === cmd || (c[0] === 'command_tool_call' && (c[1] as any)?.tool === 'focus' && (c[1] as any)?.op === op)); }`. Replace both definitions with `const navigateIpcCalls = filterIpcCalls('spatial_navigate', 'navigate focus'); const drillOutIpcCalls = filterIpcCalls('spatial_drill_out', 'drill_out layer');`.

### Warnings
- [x] `apps/kanban-app/ui/src/components/entity-inspector.field-enter-drill.browser.test.tsx:209` — `defaultInvokeImpl` spans 127 lines (212–334) and is a monolithic if/else cascade handling 15+ disparate command types (entity schema, spatial drill/focus/register/navigate, etc.). Large functions are hard to read, test, and reuse. Extract spatial commands (spatial_drill_in, spatial_drill_out, spatial_navigate, spatial_register_scope, spatial_unregister_scope, spatial_focus, spatial_clear_focus) into a dedicated `handleSpatialCommand()` helper. Extract entity/schema commands (list_entity_types, get_entity_schema, get_ui_state, etc.) into a separate `handleEntityCommand()` helper. This breaks the function into focused, testable pieces.
- [x] `apps/kanban-app/ui/src/components/entity-inspector.field-enter-drill.browser.test.tsx:662` — Callback parameter uses abbreviation `c` instead of a full word. Rule: no abbreviations, use descriptive names like `call`, `invocation`, or `ipcCall`. Rename `c` to a full descriptive name: `const navigateIpcCalls = mockInvoke.mock.calls.filter((ipcCall) => ipcCall[0] === "spatial_navigate" || ...`.
- [x] `apps/kanban-app/ui/src/components/entity-inspector.field-enter-drill.browser.test.tsx:762` — Callback parameter uses abbreviation `c` instead of a full word. Rule: no abbreviations, use descriptive names like `call`, `invocation`, or `ipcCall`. Rename `c` to a full descriptive name: `const drillOutIpcCalls = mockInvoke.mock.calls.filter((ipcCall) => ipcCall[0] === "spatial_drill_out" || ...`.

## Review Findings (2026-06-15 11:14)

### Blockers
- [x] Deferred to follow-up card 6a45spw — cross-file shared-test-util extraction is out of scope for this bug-fix card. `apps/kanban-app/ui/src/components/entity-inspector.field-enter-drill.browser.test.tsx:214` — Test harness infrastructure duplicated across many test files. The `UNHANDLED` symbol, `handleEntityCommand` (lines 222–242), `handleSpatialCommand` (lines 251–345), and `filterIpcCalls` (lines 451–460) are verbatim copies of code in board-view.enter-drill-in.browser.test.tsx, inspectable.space.browser.test.tsx, and others (per probe: 1.00 matches). This violates the single-source-of-truth principle for test utilities and creates a maintenance surface — a fix to one copy won't propagate. Extract the test harness infrastructure to a shared utility module `apps/kanban-app/ui/src/test/mock-spatial-kernel.ts` exporting `UNHANDLED`, `handleEntityCommand`, `handleSpatialCommand`, `filterIpcCalls`, and the listener/mock setup. Import and reuse across all spatial test files to eliminate duplication and ensure consistent updates.

### Warnings
- [x] Deferred to follow-up card 6a45spw — cross-file shared-test-util extraction is out of scope for this bug-fix card. `apps/kanban-app/ui/src/components/entity-inspector.field-enter-drill.browser.test.tsx:51` — `handleEntityCommand` reimplements the same entity-command mock dispatcher that already exists across 4+ test files (per `similar` probe showing 0.96 matches in entity-inspector.test.tsx, entity-inspector.spatial-nav.test.tsx, field.spatial-nav.test.tsx, and inspector-field.space-inspect.browser.test.tsx). Duplicating this across test files makes maintenance fragile — future fixes to the mock contract must be applied in every copy. Extract `handleEntityCommand` and its SCHEMAS dependency to a shared test utility (e.g., `apps/kanban-app/ui/src/test/entity-command-mock.ts`) and import it in all test files, including this one. This keeps one canonical implementation.
- [x] Deferred to follow-up card 6a45spw — cross-file shared-test-util extraction is out of scope for this bug-fix card. `apps/kanban-app/ui/src/components/entity-inspector.field-enter-drill.browser.test.tsx:70` — `handleSpatialCommand` reimplements the same spatial-kernel mock dispatcher that already exists across 4+ files (per `similar` probe showing 0.97–0.98 matches in inspectable.space.browser.test.tsx, app-shell.test.tsx, perspective-tab-bar.filter-enter.spatial.test.tsx, and spatial-shadow-registry.ts). This duplication spreads the single source of truth for the kernel-echo contract (`spatial_drill_in` / `spatial_focus` / etc.) across multiple files. Extract `handleSpatialCommand` and its supporting maps (`drillInResponses`, `monikerToKey`, `currentFocusKey`, `listeners`) to a shared test utility (e.g., `apps/kanban-app/ui/src/test/spatial-command-mock.ts`). This centralizes the kernel-contract mock, making future changes to the echo-behavior contract propagate everywhere automatically.

### Nits
- [x] `apps/kanban-app/ui/src/components/entity-inspector.field-enter-drill.browser.test.tsx:222` — Parameter names `cmd` and `args` are abbreviations. Renamed to `command` and `commandArgs` (TS modules are strict-mode; `arguments` is a reserved binding name, so the descriptive `commandArgs` is used).
- [x] `apps/kanban-app/ui/src/components/entity-inspector.field-enter-drill.browser.test.tsx:251` — Parameter names `cmd` and `args` are abbreviations. Renamed to `command` and `commandArgs`.
- [x] `apps/kanban-app/ui/src/components/entity-inspector.field-enter-drill.browser.test.tsx:349` — Parameter name `cmd` is an abbreviation for 'command'. Renamed `cmd` to `command` (and its `args` to `commandArgs`).

## Review Findings (2026-06-15 11:36)

> All 4 findings are the cross-file shared-test-util harness extraction owned by follow-up card 6a45spw — out of scope for this bug-fix card. The engine reported 0 blockers; the in-scope acceptance criteria are met (both named tests + the other 4 pass; assertions verify real host-driven dispatch, not no-ops). Moving to done; the harness-dedup work continues on 6a45spw.

### Warnings (deferred to 6a45spw — cross-file extraction, not an in-scope defect)
- [x] Deferred to 6a45spw. `apps/kanban-app/ui/src/components/entity-inspector.field-enter-drill.browser.test.tsx:72` — `handleEntityCommand` reimplements the same mock IPC handler that exists in at least four other test files. Extract `handleEntityCommand`, `handleSpatialCommand`, and related mock handlers to a shared test utility file (e.g., `apps/kanban-app/ui/src/test/mock-kernel-ipc.ts`), then import and reuse across all test files.
- [x] Deferred to 6a45spw. `apps/kanban-app/ui/src/components/entity-inspector.field-enter-drill.browser.test.tsx:175` — `filterIpcCalls` reimplements the same mock-call filtering pattern found in four other test files. Add `filterIpcCalls` to the shared test utility module so all tests import it instead of reimplementing the same filter logic.
- [x] Deferred to 6a45spw. `apps/kanban-app/ui/src/components/entity-inspector.field-enter-drill.browser.test.tsx:222` — handleEntityCommand is an if-chain over a known set of command names; could be a data-driven `ENTITY_COMMANDS` dispatch table. (Restructuring this duplicated harness in-place conflicts with the 6a45spw extraction; address during that extraction.)
- [x] Deferred to 6a45spw. `apps/kanban-app/ui/src/components/entity-inspector.field-enter-drill.browser.test.tsx:251` — handleSpatialCommand is an if-chain over a known set of spatial command names; could be a data-driven `SPATIAL_COMMANDS` dispatch table. (Same as above — fold into the 6a45spw extraction.)