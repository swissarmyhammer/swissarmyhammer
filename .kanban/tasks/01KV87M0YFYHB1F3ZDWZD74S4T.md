---
assignees:
- claude-code
position_column: todo
position_ordinal: fa80
title: Extract shared spatial-nav browser-test harness (setupSpatialMocks/makeSpatialTestHelpers) — ~20 files duplicate the mock bootstrap verbatim
---
## What

The mock bootstrap in `apps/kanban-app/ui/src/components/board-view.enter-drill-in.browser.test.tsx:36` (lines ~36–255: listeners, `mockInvoke`/`mockListen`, Tauri API mocks, spatial kernel mock, default-invoke responses, and 15+ helper functions) is copied verbatim across ~20 spatial/browser test files with no parameterization.

Surfaced by the review engine while reviewing z3ax1jz (01KTSQ38PF0K5Q7DXR5Z3AX1JZ). It is a pre-existing duplication problem spanning the whole spatial-test family, NOT specific to the wire-shape repair that card delivered — so it was scoped out of that card and captured here.

## How

Extract a shared `src/test/spatial-nav-harness.ts` exporting:
- `setupSpatialMocks()`
- `makeSpatialTestHelpers()`
- a parameterized `defaultInvokeImpl` factory

Then replace the per-file copies with imports.

## Acceptance Criteria
- [ ] Shared harness module created with the three exports above
- [ ] Spatial/browser test files import from it instead of re-declaring the bootstrap
- [ ] All affected test files remain green; tsc --noEmit clean