---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffcb80
title: Fix pre-existing React act() warnings in UI tests (557 instances)
---
What: kanban-app/ui vitest suite emits ~557 'An update to X inside a test was not wrapped in act(...)' warnings on stderr (count grew 4x since this task was written; was 131). Providers involved include UIStateProvider, SchemaProvider, InspectorPanel (pre-entity-cache work), EntityInspector, PerspectiveTabBar, FocusScope, and others.

Tests all pass (2079/2079) but stderr noise indicates post-mount state updates are firing outside the test's act() scope.

Acceptance Criteria:
- pnpm test (from kanban-app/ui) produces zero 'not wrapped in act' warnings on stderr
- All 2079 tests still pass

Approach:
- For tests that just call render() once and assert on DOM, wrap render in `await act(async () => { render(...) })` to flush post-mount effects inside scope.
- For tests that call async provider-loading hooks, use `findBy*` queries or `waitFor` so the loading-triggered state updates settle.

Verify command:
`cd kanban-app/ui && pnpm test 2>&1 | grep -c "not wrapped in act"` — should print `0` when done.

Tests: existing vitest suite; should pass cleanly with no stderr noise.

## Review Findings (2026-05-10 19:01)

### Warnings
- [x] `kanban-app/ui/src/test/setup.ts` (the `__actWarnDiagInstalled` block, ~lines 17-50 of the diff) — Temporary diagnostic must be removed now that warnings are eliminated. The block's own comment reads `// Temporary diagnostic: attribute "not wrapped in act" warnings to the currently-running test. ... Removed once act() warnings are eliminated.` The acceptance criteria are now met (0 warnings, 2079/2079 passing), so the diagnostic should be deleted along with the `beforeEach`/`afterEach` import. Leaving it in place wraps every test's `console.error` and pollutes test setup with state that has no remaining purpose; it also installs a global `console.error` patch that future contributors will have to reason around when adding their own diagnostics. Suggested fix: remove the entire `if (!g.__actWarnDiagInstalled)` block and revert the `import { afterEach, beforeEach } from "vitest";` line added at the top.

### Nits
- [x] `kanban-app/ui/src/components/app-layout.test.tsx`, `kanban-app/ui/src/components/app-shell.test.tsx`, `kanban-app/ui/src/components/board-view.cross-column-nav.spatial.test.tsx` — These three files inline the `let result!: ReturnType<typeof render>; await act(async () => { result = render(...); }); return result;` pattern instead of using the new `renderInAct` helper from `@/test/act-render`. The helper exists precisely to remove this boilerplate (and most other modified files in this change already use it). Converting these three to `return await renderInAct(<Tree/>)` keeps the project's test-helper usage consistent and shrinks each `renderXxx` factory by ~5 lines. Not load-bearing — the inline pattern is semantically identical — but it leaves the suite with two ways to do the same thing.
- [x] `kanban-app/ui/src/components/board-view.cross-column-nav.spatial.test.tsx` — Local `pressKey(key)` helper (added lines ~10-25 of the file's diff) duplicates `pressKeyInAct(key, 80)` from `@/test/act-render`. The shared helper already supports the 80ms `settleMs` parameter that this local helper hardcodes. Replace `await pressKey("{ArrowRight}")` with `await pressKeyInAct("{ArrowRight}", 80)` and drop the local function. (Note: the diff also drops the subsequent `await flushSetup()` after each press, which the helper subsumes via `settleMs` — verify the test still passes after the swap; if it relies on flushSetup's additional rAF settle on top of the 80ms timer, keep the explicit flushSetup or bump the settleMs.)
- [x] `kanban-app/ui/src/components/fields/text-editor.test.tsx` — The three smoke tests (`renders with minimal props (value)`, `renders with placeholder and onChange`, `renders with singleLine flag`) replaced `expect(() => render(...)).not.toThrow()` with a manual try/catch + `expect(thrown).toBeNull()`. The idiomatic vitest equivalent for an async expression that should not reject is `await expect(renderInAct(<Tree/>)).resolves.toBeDefined();` (or `.not.toThrow` from `expect.poll` — but `.resolves` is simplest). The current form is correct but more verbose than necessary.