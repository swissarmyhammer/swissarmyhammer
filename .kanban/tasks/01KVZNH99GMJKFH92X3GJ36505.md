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
position_column: doing
position_ordinal: '80'
title: Stale @tauri-apps/api import/module-resolution errors in browser vitest project (~7 files)
---
## What

During the `#ui` batch (2026-06-24/25), runs of the `apps/kanban-app/ui` **browser** vitest project surfaced ~7 test files failing to LOAD with stale `@tauri-apps/api` import / module-resolution errors (distinct from the assertion failures tracked separately). These look like a Vite dep-optimization (`node_modules/.vite`) cache artifact or an unpinned-transitive-dep issue rather than a source bug — but they cause real red in the suite and need to be ruled out / resolved so the browser suite is trustworthy.

Relevant context: this repo's `apps/kanban-app/ui` lockfile is gitignored and CI uses `npm install`, so freshly-published transitive deps can break a previously-green build with no code change (this is why radix-ui was pinned). A stale `@tauri-apps/api` optimize-deps cache is the most likely cause.

## Acceptance Criteria
- [ ] Identify the exact ~7 failing files and the precise import/module error message (re-run the browser suite to capture them).
- [ ] Determine root cause: stale Vite optimize-deps cache vs an actual unpinned/incompatible `@tauri-apps/api` version vs a real import path bug.
- [ ] Resolve it: e.g. clear `node_modules/.vite` + clean `npm install`, and/or pin the offending dep, and/or fix the import — and document which it was so it doesn't recur.
- [ ] After the fix, those ~7 files LOAD and run (their own assertions may then pass or fall under the separate assertion-failures task).

## Tests
- [ ] Repro: `cd apps/kanban-app/ui && npx vitest run --project browser 2>&1 | grep -iE 'tauri|failed to (load|resolve|import)'` — capture the failing files + errors.
- [ ] After resolving: the same command shows no `@tauri-apps/api` resolution errors; the previously-erroring files load.

## Workflow
- Start by ruling out the transient cache: `rm -rf apps/kanban-app/ui/node_modules/.vite` then re-run. If that fixes it, document it as a cache artifact and consider a stable mitigation (e.g. an `optimizeDeps` entry) rather than a code change. If it's a real dep/version problem, pin it. This is investigation-led — do not write production code blindly. #test-failure #ui