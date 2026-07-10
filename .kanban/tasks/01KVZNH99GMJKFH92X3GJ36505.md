---
comments:
- actor: claude-code
  id: 01kvzpt6hn5kdvx38rkryx2q69
  text: |-
    Reproduced on browser vitest project. Exactly 7 files fail to LOAD with: SyntaxError: The requested module '/node_modules/@tauri-apps/api/core.js?v=e88c855f' does not provide an export named 'SERIALIZE_TO_IPC_FN'. The 7: avatar.test.tsx, board-selector.test.tsx, fields/editors/{attachment-editor,comment-log-editor,single-select-editor}.test.tsx, fields/displays/{avatar-display,comment-log-display}.test.tsx. Full browser run: 12 files failed / 257 passed; 10 assertion failures belong to the separate task, the other 7 = these load failures.

    Root cause confirmed as STALE Vite optimize-deps cache: real node_modules/@tauri-apps/api/core.js (v2.11.0) DOES export SERIALIZE_TO_IPC_FN (line 283), but the vitest browser dep cache under node_modules/.vite/vitest/<hash>/deps holds a pre-bundled core that predates that export (the ?v=e88c855f query is the optimizer hash). Next: rm -rf node_modules/.vite and re-run to confirm the cache-clear fixes it.
  timestamp: 2026-06-25T15:36:40.757401+00:00
- actor: claude-code
  id: 01kvzpxyydjzrc7h854vrmf53m
  text: |-
    CACHE HYPOTHESIS DISPROVEN. rm -rf node_modules/.vite then re-ran: identical 7 files still fail with the same SERIALIZE_TO_IPC_FN error on a fresh cache. This is NOT a stale-cache artifact — it is a real source bug in those 7 test files.

    Root cause: each of the 7 uses the OLD hand-listed mock stub `vi.mock("@tauri-apps/api/core", () => ({ invoke: ... }))` returning ONLY invoke. @tauri-apps/api >=2.11 pulls submodules (e.g. dpi/window/event) that import named exports from core (SERIALIZE_TO_IPC_FN, Resource, Channel, ...); the hand-listed stub drops them, so the downstream import fails at load time.

    The fix is already established in the codebase: passing siblings like multi-select-editor.test.tsx use the spread+importActual pattern: vi.mock("@tauri-apps/api/core", async (importActual) => ({ ...(await importActual<typeof import("@tauri-apps/api/core")>()), invoke: ... })) and the same for /event. Converting the 7 stragglers to that pattern is the fix. Proceeding.
  timestamp: 2026-06-25T15:38:44.045096+00:00
- actor: claude-code
  id: 01kvzq3j7mc3bs9qdsrz5wr5t4
  text: |-
    FIXED + VERIFIED. Converted the @tauri-apps/api/core and /event vi.mock factories in all 7 files to the spread+importActual pattern (preserve real exports incl. SERIALIZE_TO_IPC_FN, override only invoke/listen), matching the established pattern in multi-select-editor.test.tsx. Added an explanatory comment to each.

    Files changed (all under apps/kanban-app/ui/src):
    - components/avatar.test.tsx
    - components/board-selector.test.tsx
    - components/fields/editors/attachment-editor.test.tsx
    - components/fields/editors/comment-log-editor.test.tsx
    - components/fields/editors/single-select-editor.test.tsx
    - components/fields/displays/avatar-display.test.tsx
    - components/fields/displays/comment-log-display.test.tsx

    Verification (browser vitest project, fresh runs):
    - Targeted run of the 7 files: 7 files passed, 103 tests passed, 0 SERIALIZE errors.
    - tsc --noEmit: exit 0.
    - Full browser suite BEFORE: 12 files failed / 257 passed (7 = these load failures + 5 with the 10 assertion failures). AFTER: 5 files failed / 264 passed; 0 SERIALIZE_TO_IPC_FN errors, 0 'Failed to import test file'. The 7 load-failure files now pass; remaining 5 failed files (entity-card, editor-save, grid-empty-state, mention-view, spatial-nav-end-to-end) are the SAME 10 assertion failures that belong to the separate assertion-failures task — no new failures introduced.

    Note for future: the task hypothesized a stale node_modules/.vite cache; that was DISPROVEN (rm -rf .vite did not change the result). Root cause was incomplete mock stubs, fixed at source. No optimizeDeps/pin change needed.
  timestamp: 2026-06-25T15:41:47.636157+00:00
- actor: claude-code
  id: 01kvzr6562dj4c9tccx9gsadvn
  text: |-
    Reworked the Review Findings (2026-06-25 09:44) checklist — TEST-ONLY hygiene cleanups, no production code. All 9 boxes (1 blocker, 4 warnings, 4 nits) flipped to [x].

    What was done:
    1. (blocker + warning #1) Consolidated the verbatim-duplicated `vi.mock("@tauri-apps/plugin-log", ...)` 6-method stub into a single global `vi.mock` in src/test/setup.ts (the project's existing vitest setupFiles entry, shared by both unit and browser projects). Removed the local block from all 6 files (avatar, avatar-display, comment-log-display, attachment-editor, comment-log-editor, single-select-editor), each replaced with a pointer comment. Safe because ~145 test files already mock it with this identical pure stub; the one file that asserts on a plugin-log spy (lib/render-profiler.test.tsx) keeps its own local mock (per-file override) and was not touched. Did NOT consolidate the api/core or api/event mocks — those differ per file (invoke return values) so they stay local.
    2. (warnings #2-4) Extracted `makeActor` to new src/test/entity-fixtures.ts with `overrides: Record<string, unknown> = {}` default; imported it in avatar.test.tsx, avatar-display.test.tsx, comment-log-display.test.tsx and removed the 3 local copies. comment-log-display dropped its now-unused `Entity` type import.
    3. (warning + 4 nits) Named-constant magic timeouts: single-select-editor.test.tsx — BLUR_COMMIT_TIMEOUT_MS(100)+BLUR_TEST_SLACK_MS(50) replacing the 150 at advanceTimersByTime, ASYNC_SETTLE_WAIT_MS(50), AUTOCOMPLETE_DEBOUNCE_WAIT_MS(400, // ~150ms implementation + 250ms slack), AUTOCOMPLETE_SOURCE_WAIT_MS(250); avatar.test.tsx — TOOLTIP_ANIMATION_WAIT_MS(100).

    Did NOT touch the spatial-nav mock bootstrap (installRegistryHook/callMockInvoke in setup.ts) or board-selector.test.tsx, per scope.

    Verification (fresh):
    - npx tsc --noEmit => exit 0.
    - Targeted browser run of the 6 touched files + board-selector + multi-select-editor => 8 files / 126 tests passed.
    - Full browser suite (npx vitest run --project browser) => 5 files failed / 264 passed (269); 10 tests / 2535 passed. The 6 touched files all pass; the 5 remaining failures (spatial-nav-end-to-end, entity-card, grid-empty-state, mention-view, editor-save = the same 10 assertion failures) belong to the SEPARATE assertion-failures card — no new failures introduced.
    - No prettier/eslint configured in apps/kanban-app/ui (no scripts, no config, no bins); the project's gate is `tsc --noEmit && vitest run`.
    - Adversarial double-check: PASS, no findings (independently re-ran tsc + targeted suite, confirmed the global plugin-log mock is safe and the 5 failing files are pre-existing assertion mismatches, not load/mock errors).

    Files changed (all under apps/kanban-app/ui/src):
    - test/setup.ts (added global plugin-log vi.mock + `import { vi }`)
    - test/entity-fixtures.ts (NEW — shared makeActor)
    - components/avatar.test.tsx
    - components/fields/displays/avatar-display.test.tsx
    - components/fields/displays/comment-log-display.test.tsx
    - components/fields/editors/attachment-editor.test.tsx
    - components/fields/editors/comment-log-editor.test.tsx
    - components/fields/editors/single-select-editor.test.tsx

    Leaving the task in `doing` (green, ready for /review).
  timestamp: 2026-06-25T16:00:41.154433+00:00
- actor: claude-code
  id: 01kvzrnhth3gqxsyyxag5yj1w6
  text: 'Addressed the 3 nits from "Review Findings (2026-06-25 10:01)": added one-line JSDoc to renderDisplay (comment-log-display.test.tsx), renderEditor (attachment-editor.test.tsx), and renderSingleSelect (single-select-editor.test.tsx), matching the style of neighboring documented helpers (avatar-display.test.tsx::renderDisplay, getCmView/settle). Documentation-only, no behavior change. Verified: `npx tsc --noEmit` exit 0; `npx vitest run --project browser` on the 3 touched files = 3 files / 46 tests passed, exit 0 (the [FocusLayer] push failed console.error lines are pre-existing tauri-window-mock noise, unrelated). Flipped the 3 checklist boxes. Leaving in doing for review.'
  timestamp: 2026-06-25T16:09:05.617173+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffef80
title: Stale @tauri-apps/api import/module-resolution errors in browser vitest project (~7 files)
---
## What

During the `#ui` batch (2026-06-24/25), runs of the `apps/kanban-app/ui` **browser** vitest project surfaced ~7 test files failing to LOAD with stale `@tauri-apps/api` import / module-resolution errors (distinct from the assertion failures tracked separately). These look like a Vite dep-optimization (`node_modules/.vite`) cache artifact or an unpinned-transitive-dep issue rather than a source bug — but they cause real red in the suite and need to be ruled out / resolved so the browser suite is trustworthy.

Relevant context: this repo's `apps/kanban-app/ui` lockfile is gitignored and CI uses `npm install`, so freshly-published transitive deps can break a previously-green build with no code change (this is why radix-ui was pinned). A stale `@tauri-apps/api` optimize-deps cache is the most likely cause.

## Acceptance Criteria
- [x] Identify the exact ~7 failing files and the precise import/module error message (re-run the browser suite to capture them).
- [x] Determine root cause: stale Vite optimize-deps cache vs an actual unpinned/incompatible `@tauri-apps/api` version vs a real import path bug.
- [x] Resolve it: e.g. clear `node_modules/.vite` + clean `npm install`, and/or pin the offending dep, and/or fix the import — and document which it was so it doesn't recur.
- [x] After the fix, those ~7 files LOAD and run (their own assertions may then pass or fall under the separate assertion-failures task).

## Tests
- [x] Repro: `cd apps/kanban-app/ui && npx vitest run --project browser 2>&1 | grep -iE 'tauri|failed to (load|resolve|import)'` — capture the failing files + errors.
- [x] After resolving: the same command shows no `@tauri-apps/api` resolution errors; the previously-erroring files load.

## Workflow
- Start by ruling out the transient cache: `rm -rf apps/kanban-app/ui/node_modules/.vite` then re-run. If that fixes it, document it as a cache artifact and consider a stable mitigation (e.g. an `optimizeDeps` entry) rather than a code change. If it's a real dep/version problem, pin it. This is investigation-led — do not write production code blindly. #test-failure #ui

## Review Findings (2026-06-25 09:44)

### Blockers
- [x] `apps/kanban-app/ui/src/components/avatar.test.tsx:13` — @tauri-apps/plugin-log mock is verbatim duplication across 6 test files (all except board-selector). This identical 8-line block repeats unchanged. Consolidate to vitest globalSetup or a shared test setup module.

### Warnings
- [x] `apps/kanban-app/ui/src/components/avatar.test.tsx:16` — The @tauri-apps/plugin-log mock pattern is duplicated identically across 6 test files (all except board-selector.test.tsx). Repeated setup code should be expressed once. Include @tauri-apps/plugin-log mock setup in the shared test utilities module alongside @tauri-apps/api/event.
- [x] `apps/kanban-app/ui/src/components/fields/displays/avatar-display.test.tsx:64` — The `makeActor` helper (lines 64–72) is identical to the one defined in avatar.test.tsx (lines 43–53). This utility should be extracted to a shared test fixtures file instead of being redefined in every test file that needs it. Create a shared test fixture file (e.g., `apps/kanban-app/ui/src/test/entity-fixtures.ts`) with `export function makeActor(...)` and import it in both files.
- [x] `apps/kanban-app/ui/src/components/fields/displays/comment-log-display.test.tsx:56` — The `makeActor` helper (lines 56–62) reimplements the same utility that already exists in avatar.test.tsx (lines 43–53). Although this version omits the `overrides` parameter, the core logic is identical and should call the shared version or a parameterized variant. Extract `makeActor` to a shared test fixture file. If the overrides parameter is rarely needed, the shared version could have it default to `{}`, or keep two versions in the fixture file (e.g., `makeActor` and `makeActorWithFields`).
- [x] `apps/kanban-app/ui/src/components/fields/editors/single-select-editor.test.tsx:310` — Unexplained numeric literal `150` (milliseconds) configures fake-timer advancement; comment documents 100ms implementation timeout but code advances 150ms without explaining the 50ms slack value. Clarify the intent: `const BLUR_COMMIT_TIMEOUT_MS = 100; const BLUR_TEST_SLACK_MS = 50; vi.advanceTimersByTime(BLUR_COMMIT_TIMEOUT_MS + BLUR_TEST_SLACK_MS);` or update the comment to explain the 150ms clearly.

### Nits
- [x] `apps/kanban-app/ui/src/components/avatar.test.tsx:120` — Unexplained numeric literal `100` (milliseconds) configures test timing behavior; hardcoded timeouts should be named constants for maintainability and clarity. Extract to a named constant: `const TOOLTIP_ANIMATION_WAIT_MS = 100;` and use it in the setTimeout call.
- [x] `apps/kanban-app/ui/src/components/fields/editors/single-select-editor.test.tsx:155` — Unexplained numeric literal `50` (milliseconds) configures test timing behavior in the settle() helper; should be a named constant. Extract to a named constant at module scope: `const ASYNC_SETTLE_WAIT_MS = 50;` and reference it in the setTimeout call.
- [x] `apps/kanban-app/ui/src/components/fields/editors/single-select-editor.test.tsx:190` — Unexplained numeric literal `400` (milliseconds) configures test timeout; comment claims '~150ms + slack' but does not justify why 400 specifically. Extract to a named constant with clarifying comment: `const AUTOCOMPLETE_DEBOUNCE_WAIT_MS = 400; // ~150ms implementation + 250ms slack` and use it in the setTimeout.
- [x] `apps/kanban-app/ui/src/components/fields/editors/single-select-editor.test.tsx:213` — Unexplained numeric literal `250` (milliseconds) configures test timeout; comment explains the intent (wait for async source) but not the specific value choice. Extract to a named constant: `const AUTOCOMPLETE_SOURCE_WAIT_MS = 250;` and use it in the setTimeout.

## Review Findings (2026-06-25 10:01)

### Nits
- [x] `apps/kanban-app/ui/src/components/fields/displays/comment-log-display.test.tsx:65` — Test helper function `renderDisplay` lacks documentation. It has complex parameters and wraps multiple React providers; similar functions in other test files (avatar.test.tsx::renderAvatar, avatar-display.test.tsx::renderDisplay) are documented. Add a JSDoc comment above `renderDisplay()` explaining its purpose, the `value` parameter, and the `options` object, matching the style used in avatar-display.test.tsx.
- [x] `apps/kanban-app/ui/src/components/fields/editors/attachment-editor.test.tsx:88` — Test helper function `renderEditor` lacks documentation. It wraps the editor in multiple providers with complex parameters. Add a JSDoc comment above `renderEditor()` explaining its purpose and the `props` parameter structure.
- [x] `apps/kanban-app/ui/src/components/fields/editors/single-select-editor.test.tsx:155` — Test helper function `renderSingleSelect` lacks documentation. It has complex parameters and wraps the component in multiple providers; the same file documents other test helpers (getCmView, settle). Add a JSDoc comment above `renderSingleSelect()` explaining its purpose and parameters, following the pattern used for getCmView() and settle() in the same file.