---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvzs68bcwr2wm6ghnm18ds33
  text: |-
    Picked up. Established browser-project baseline before any change: 2535 pass / 10 fail (npx vitest run --project browser). The 10 failures are the known pre-existing assertion failures tracked elsewhere (entity-card inspect context menu, 6x editor-save compact/full blur, grid-empty-state context menu, mention-view extraCommands, spatial-nav-end-to-end perspective.switch). Saved exact per-test pass/fail lists to scratch for after-comparison. Bar: the 2535-pass set must not regress; the 10 may remain failing.

    Prior art discovered: card 01KV6250AH0DPRMG9SJ6A45SPW already extracted the spatial-kernel echo contract into src/test/mock-spatial-kernel.ts (makeSpatialKernelMock + emitToListenerMap/emitToCallbackRecord). setup.ts already globally mocks @tauri-apps/plugin-log. This card extends that: extract the Tauri vi.hoisted bootstrap (setupSpatialMocks) + the helper functions (makeSpatialTestHelpers) used by the *enter-drill-in* family.
  timestamp: 2026-06-25T16:18:12.972362+00:00
- actor: claude-code
  id: 01kvzsmvrdybx96ndyjhpjdc6p
  text: |-
    Harness landed at src/test/spatial-nav-harness.ts with the three card-named exports: setupSpatialMocks() (the verbatim vi.hoisted Tauri bootstrap), makeSpatialTestHelpers({mockInvoke,listeners}) (registerScopeArgs/keyForMoniker/spatialDrillIn/OutCalls/spatialFocusCalls/dispatchPayloads/inspect/entityInspectDispatches/fireFocusChanged), and makeDefaultInvokeImpl({keymapMode,handleSpatialCommand}).

    Canonical file board-view.enter-drill-in.browser.test.tsx fully routed through it (bootstrap + helpers + defaultInvokeImpl). tsc --noEmit clean; that file's 9 browser tests still pass.

    vi.hoisted constraint solved: vi.hoisted cannot reference static imports, so callers use `await vi.hoisted(async () => { const {setupSpatialMocks} = await import("@/test/spatial-nav-harness"); return setupSpatialMocks(); })`. The vi.mock factories close over the returned mockInvoke/mockListen.

    SCOPE DISCOVERY (important for next agent): the "~20 files" share the bootstrap VERBATIM (one 19-file cluster + two 10-file clusters, differing only in the getCurrentWindow/plugin-log mock tails). But the HELPER bodies diverged after copy-paste: flushSetup has 3 genuinely-different timings (50ms setTimeout / double-tick / single-tick), and nav-bar/perspective-bar's fireFocusChanged drops the next_segment param. spatialFocusCalls/drill collectors differ only cosmetically (formatting + return-type annotations) — behaviorally identical. So: setupSpatialMocks is 100% safe to route everywhere the bootstrap matches; makeSpatialTestHelpers is safe only where the file's helpers are behaviorally identical to the canonical (fireFocusChanged = canonical variant, 50ms-or-compatible flushSetup stays local). Routing file-by-file with a full browser-project diff vs the 2535-pass baseline after each batch.
  timestamp: 2026-06-25T16:26:11.597737+00:00
- actor: claude-code
  id: 01kvzt0p14nc0rgtm6mhkdp394
  text: |-
    DONE — green, left in doing for review.

    Final scope: 43 test files routed through the new harness + 1 new harness module (44 files under apps/kanban-app/ui).
    - setupSpatialMocks() (the verbatim vi.hoisted Tauri bootstrap) now sourced from src/test/spatial-nav-harness.ts in 43 files: the canonical board-view.enter-drill-in.browser.test.tsx (fully routed: bootstrap + helpers + defaultInvokeImpl) and 42 sibling files via a mechanical, behavior-preserving bootstrap swap (the inline `type ListenCallback`+`vi.hoisted(()=>{...})` block → `await vi.hoisted(async () => { const {setupSpatialMocks}=await import("@/test/spatial-nav-harness"); return setupSpatialMocks(); })`). Each swapped file is +3/-22 lines; their local helpers/defaultInvokeImpl were intentionally left untouched (they diverged after copy-paste — flushSetup timings, fireFocusChanged signature — so unifying would risk behavior change).
    - makeSpatialTestHelpers() and makeDefaultInvokeImpl() consumed by the canonical file.
    - 12 files skipped by design: their bootstrap block is NOT verbatim (txn-batching/mcp-notifications/rust-engine-container use a different mockInvoke body; App/jump-to-overlay.occlusion/ai-panel-dock use a single-callback listener record; spatial-nav-end-to-end & the others differ). Leaving them avoids behavior change; they remain candidates for a follow-up if desired.

    VERIFICATION (really-done):
    - npx tsc --noEmit → exit 0.
    - Full browser vitest project: BEFORE 2535 pass / 10 fail; AFTER 2535 pass / 10 fail. comm-diff of passing-test sets: ZERO regressions, ZERO newly-failing, ZERO fixed (identical sets). The 10 pre-existing failures (entity-card inspect ctx menu, 6x editor-save blur, grid-empty-state ctx menu, mention-view extraCommands, spatial-nav-end-to-end perspective.switch) are untouched and tracked elsewhere.
    - Adversarial double-check agent: PASS, no findings (confirmed fresh-per-file mocks, proven async-hoist pattern, byte-equivalent helper/responder bodies, the one literal swap "notifications/focus/changed"→FOCUS_CHANGED_EVENT verified equal, no dangling ListenCallback, no plugin-log/window mock drift).

    Acceptance criteria:
    - [x] Shared harness module created with the three exports
    - [x] Spatial/browser test files import from it instead of re-declaring the bootstrap (43 files)
    - [x] All affected test files remain green; tsc --noEmit clean
  timestamp: 2026-06-25T16:32:38.948627+00:00
position_column: doing
position_ordinal: '80'
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