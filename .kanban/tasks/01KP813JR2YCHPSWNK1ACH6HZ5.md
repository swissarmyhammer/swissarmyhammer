---
assignees:
- claude-code
position_column: todo
position_ordinal: '7e80'
title: Fix pre-existing React act() warnings in UI tests (131 instances)
---
What: kanban-app/ui vitest suite emits ~131 'An update to X inside a test was not wrapped in act(...)' warnings on stderr. Providers involved include UIStateProvider, SchemaProvider, InspectorPanel (pre-entity-cache work), EntityInspector, PerspectiveTabBar, FocusScope, and others.

Tests all pass (1092/1092) but stderr noise indicates post-mount state updates are firing outside the test's act() scope. These are pre-existing (verified by stashing current changes and seeing 133 warnings still present on main).

Acceptance Criteria:
- npm test (from kanban-app/ui) produces zero 'not wrapped in act' warnings on stderr
- All 1092 tests still pass

Approach:
- For tests that just call render() once and assert on DOM, wrap render in `await act(async () => { render(...) })` to flush post-mount effects inside scope.
- For tests that call async provider-loading hooks, use `findBy*` queries or `waitFor` so the loading-triggered state updates settle.

Tests: existing vitest suite; should pass cleanly with no stderr noise.