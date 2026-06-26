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
- actor: claude-code
  id: 01kw09w4h4cs4gcw2kcsm8nfvr
  text: |-
    Targeted review-fix on the new harness src/test/spatial-nav-harness.ts. All edits behavior-preserving; left in doing for review.

    FIXED:
    1. MUST-FIX (blocker): keyForMoniker computed the SAME registerScopeArgs().find((a)=>a.segment===moniker) query twice (zone then scope) — the second was dead code returning an identical result. Confirmed via `git show ef627f6c0^:.../board-view.enter-drill-in.browser.test.tsx` that the double-find was copied VERBATIM from the pre-extraction helper. Collapsed to a single `const match = registerScopeArgs().find(...); return match?.fq as FullyQualifiedMoniker | undefined;`. Found/not-found results identical.
    2. cb→callback (+coupled cbs→callbacks) in mockListen (setupSpatialMocks).
    3. cmd→command in defaultInvokeImpl.
    4. env→envelope in defaultInvokeImpl.

    SKIPPED (documented):
    - null→undefined for fireFocusChanged params: DELIBERATELY NOT changed. The harness constructs a FocusChangedPayload whose prev_fq/next_fq are typed `FullyQualifiedMoniker | null` (non-optional, types/spatial.ts), and production emits JSON null for absent monikers. Switching the `= null` defaults / `| null` unions to optional undefined would (a) break tsc (undefined not assignable to `... | null`) and (b) change the emitted payload value null→undefined, diverging from production. Verified the only consumer is the canonical board-view.enter-drill-in.browser.test.tsx, whose call sites pass partial args and rely on the null default. Left as-is.
    - @/... path-alias imports: project-wide convention, out of scope.
    - Splitting makeSpatialTestHelpers for the 82-line function-length warning: conflicts with this card's consolidation goal, out of scope.

    VERIFICATION:
    - npx tsc --noEmit → exit 0.
    - Browser vitest project: 2535 passed / 10 failed — IDENTICAL to baseline (same 5 known-failing files: editor-save blur family, entity-card inspect, grid-empty-state, mention-view, spatial-nav-end-to-end). Zero previously-passing tests regressed.
    - Adversarial double-check agent: PASS — confirmed diff is exactly the 4 intended edits, no dangling cb/cbs/cmd/env in the renamed functions, exported API surface unchanged, no external caller needs updating, the skipped items correctly absent from the diff.
  timestamp: 2026-06-25T21:09:47.172321+00:00
- actor: claude-code
  id: 01kw0asyqwm9wg267rj3hhfbzb
  text: |-
    Closing to done. Card goal achieved and verified: shared spatial-nav harness (setupSpatialMocks / makeSpatialTestHelpers / makeDefaultInvokeImpl) extracted in apps/kanban-app/ui/src/test/spatial-nav-harness.ts and ~43 duplicating browser-test files routed through it. Behavior provably preserved — browser passing-test set byte-identical before/after (2535 pass / 10 pre-existing fail tracked separately), tsc --noEmit exit 0, double-check PASS.

    Review (run on the new harness file, since the 48-file delta exceeded the review-engine fan-out capacity): round 1 surfaced ONE genuine finding — a redundant duplicate `registerScopeArgs().find` in keyForMoniker (dead second branch, verbatim-copied from the original). FIXED (collapsed to a single find; behavior identical).

    Subsequent review rounds produced only style-validator churn that I decline as out-of-scope per "no bonus refactoring":
    - "Blockers" to factory-wrap spatialDrillIn/Out/FocusCalls and inspect/entityInspectDispatches — these are trivial ONE-LINE named accessors over the shared collectFocusTool/dispatchPayloads helpers; they are already DRY, and a make*Collector factory would reduce readability, not improve it.
    - function-length (85-line makeSpatialTestHelpers): this IS the consolidation factory; splitting it conflicts with the card's single-source-of-truth goal. (Repeat of a finding already declined the prior round.)
    - null defaults / `| null`: FocusChangedPayload types prev_fq/next_fq as `... | null` and production emits JSON null; switching to undefined breaks tsc and diverges from the wire contract. (Repeat of a declined finding; implementer verified.)
    - snake_case params prev_fq/next_fq/next_segment: intentionally mirror the backend FocusChangedPayload field names.
    - Array<X> vs X[], single-letter find-lambda, residual abbreviations: match surrounding codebase conventions; pure taste.

    Cheap localized renames that did improve the new code (cb→callback, cmd→command, env→envelope) were applied. Marking done — substantive work + the one real blocker are complete and verified.
  timestamp: 2026-06-25T21:26:04.284720+00:00
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffff380
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