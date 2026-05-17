---
assignees:
- claude-code
position_column: todo
position_ordinal: '9480'
project: ai-panel
title: 'Flaky CodeBlock smoke test: Shiki async highlight races a fixed 50ms wait under full-suite load'
---
## What
`src/components/ai-elements/ai-elements.smoke.test.tsx` > `AI Elements: CodeBlock` > `mounts a CodeBlock with a copy button` fails intermittently under full-suite parallel load. The assertion at line 277 — `expect(container.textContent).toContain("ok")` — fails with `AssertionError: expected '' to contain 'ok'` because the highlighted source has not yet landed in the DOM.

## Root cause
The test renders a `<CodeBlock code='{"ok": true}' language="json">` and then waits a fixed `await flushActSettle(50)` (50ms) for Shiki to highlight asynchronously before asserting the highlighted text is present (see lines 274-277 and the in-test comment "Shiki highlights asynchronously — let the effect resolve before asserting"). Under full-suite parallel load (239 test files, browser + unit projects), the Shiki async highlight effect sometimes does not complete within that fixed 50ms window, so `container.textContent` is still empty when the assertion runs. It is a timing race, not a logic bug.

## Evidence (gathered 2026-05-17)
- In isolation (`vitest run src/components/ai-elements/ai-elements.smoke.test.tsx`): passes 3/3 runs, all 12 tests green.
- Under full-suite load (`vitest run`): CodeBlock FAILED in 3 runs, PASSED in 3 runs — roughly 50% intermittent failure rate. Same machine, no code changes between runs.

## Provenance
The file `ai-elements.smoke.test.tsx` was introduced by commit `448ee1ad4` ("feat(kanban-app): vendor AI Elements components into the UI") — completed task `01KRRN386C7THGV5T6RCA59H4F`. It is a pre-existing flake, NOT caused by the WebSocket ACP message stream task `01KRRN463S53X13YE1PQ1H8P53` (that task's `git diff HEAD` touches nothing under `src/components/`).
Not covered by the existing stale-path task `01KRS426Q36ZN3DYBX2S0AS82T`, which only tracks the slugify / editor-save / board-integration ENOENT failures.

## Acceptance Criteria
- [ ] The CodeBlock smoke test no longer races: replace the fixed `flushActSettle(50)` with a deterministic wait (e.g. `vi.waitFor`/`findBy*` polling until the highlighted token text appears, or await the Shiki highlighter promise directly).
- [ ] `vitest run` (full suite) passes the CodeBlock test in 20/20 consecutive runs.

## Tests
- [ ] `npx vitest run src/components/ai-elements/ai-elements.smoke.test.tsx` passes in isolation.
- [ ] Full-suite `npm test` in `apps/kanban-app/ui` passes the CodeBlock test reliably across repeated runs. #test-failure