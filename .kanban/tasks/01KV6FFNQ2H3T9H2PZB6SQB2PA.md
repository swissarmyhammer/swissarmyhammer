---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kv6jw4wv30nqq8te1fy0a34y
  text: |-
    Root-caused and fixed all 9 failures. Verified each file in ISOLATION (confirmed counts) and together (no ordering pollution). All 7 kernel-simulator consumer files green; tsc --noEmit clean.

    Two distinct root-cause classes, both = drift between the production focus/entity MCP IPC path (command_tool_call envelopes) and the test harness which still spoke the legacy spatial_*/get_entity command names.

    CLASS A — entity fetch via MCP not lowered by shared simulator (6 fails across 3 files):
    InspectorPanel fetches its entity via entity-mcp.ts::getEntity → callMcpTool("entity","get entity",{type,id}) → invoke("command_tool_call",{module:"entity",...}). The shared kernel-simulator.ts only lowered the focus module, so the entity fetch fell through to fallback unhandled → getEntity returned {} → entity had no entity_type/fields → <EntityInspector> rendered "Loading schema..." forever → field <FocusScope>s never registered → every downstream assertion (findBySegment / focusCalls) failed.
    - inspector.repeat-open-focus.browser.test.tsx (1 fail)
    - inspector.close-restores-focus.browser.test.tsx (2 fail)
    - inspector.boundary-nav.browser.test.tsx (3 fail)
    Fix (shared helper): added lowerEntityBridge() in test-helpers/kernel-simulator.ts mirroring the existing lowerFocusBridge — command_tool_call module "entity" op "get entity" lowers onto legacy get_entity {entityType,id} and re-envelopes the raw bag into {ok,entity} so each test's defaultInvokeImpl answers it. This is the correct fix location (shared harness drift), not the per-test fallbacks.

    CLASS B — per-test helpers/intercepts only match the legacy command name (3 fails across 3 files):
    - inspector.repeat-open-focus: ALSO had a second cause — its focusCalls() helper read args.fq, but the MCP envelope carries fq under args.params.fq, so it saw "?, ?". Fixed helper to read params.fq ?? fq. (file now passes with both A+B fixes)
    - column-view.virtualized-nav.browser.test.tsx (2 fail): spatialNavigateCalls() counted only cmd==="spatial_navigate"; production routes navigate through command_tool_call op "navigate focus" with args under params (focusedFq/focused_fq). Fixed helper to count both wire shapes and normalize.
    - entity-focus.kernel-projection.test.tsx (1 fail): the unknown-moniker test staged mockImplementationOnce throwing only on cmd==="spatial_focus"; production sends command_tool_call op "set focus" so the throw never fired and the permissive sim accepted the bogus moniker → store regressed. Fixed the intercept to also match the {module:"focus",op:"set focus",params:{fq}} envelope.

    EXTRA (same class, in-scope, no new card per card instructions): inspectors-container.auto-focus-on-mount.browser.test.tsx was also pre-existing red from the SAME focus-MCP migration (its inline mock + spatialFocusCalls only handled legacy spatial_focus). Fixed both to handle the command_tool_call set-focus envelope. Now green.

    Files changed:
    - apps/kanban-app/ui/src/test-helpers/kernel-simulator.ts (lowerEntityBridge + entity-module routing)
    - apps/kanban-app/ui/src/components/inspector.repeat-open-focus.browser.test.tsx (focusCalls reads params.fq)
    - apps/kanban-app/ui/src/components/column-view.virtualized-nav.browser.test.tsx (spatialNavigateCalls counts MCP envelope)
    - apps/kanban-app/ui/src/lib/entity-focus.kernel-projection.test.tsx (rejection intercept matches MCP envelope)
    - apps/kanban-app/ui/src/components/inspectors-container.auto-focus-on-mount.browser.test.tsx (mock + helper handle MCP envelope)

    No production assertions weakened/deleted.

    Before: 9 failing (the 5 named files). After (isolation): 5/5 named files green. Together with the other 2 consumer files + the extra sibling = 8 files / 50 tests pass, EXIT 0, fresh vite cache. tsc --noEmit EXIT 0.

    PRE-EXISTING failures OUT OF SCOPE (NOT introduced by this change; not kernel-simulator consumers): the full `vitest run` shows 6 unrelated test failures + 6 files failing at import. The import failures are a dependency-state issue — installed @tauri-apps/api/core.js does not export SERIALIZE_TO_IPC_FN (only referenced in JSDoc), so files importing it (avatar, board-selector, field editors via strict-dispatch-mock) fail to collect; this reproduces on a single file with a fresh cache and is unrelated to my edits. The 6 assertion failures (column-view "Do This Next", entity-card entity.inspect, grid-empty-state, mention-view extraCommands, spatial-nav-end-to-end Family 5) are context-menu/command-registration failures tied to the in-flight command-registration changes already present in the working tree (modified builtin/plugins/entity-commands, scope_commands.rs, app-shell-commands, etc. — none of which I touched). None import the kernel-simulator. These belong to other in-flight work, not this card.
  timestamp: 2026-06-15T21:26:49.499610+00:00
- actor: claude-code
  id: 01kv8kvt9ghca9tjrmrmf06bkv
  text: 'Finish loop: scoped review of the 5 changed files (kernel-simulator.ts + 4 test files) found 0 new in-scope findings — lowerEntityBridge lowering is correct, no weakened assertions; all surfaced items are the already-captured harness duplication (zd74s4t) or pre-existing clarity nits. Tests re-confirmed: combined browser run of all 7 kernel-simulator consumer files → 7 files / 43 tests pass, 0 failures, clean collection. Moved to done.'
  timestamp: 2026-06-16T16:22:36.080211+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffbd80
project: builtin-commands
title: Fix 9 remaining pre-existing browser-suite failures across kernel-simulator consumer files
---
## What

After yvrqky6 fixed the 6 `entity-inspector.test.tsx` failures (by updating the shared `apps/kanban-app/ui/src/test-helpers/kernel-simulator.ts` to mirror the production `focus` MCP IPC path), 9 tests still fail across other `installKernelSimulator` consumer files. These are PRE-EXISTING on HEAD with separate root causes (not introduced by yvrqky6 — that change was a net improvement, 15→9 failing, zero passing→failing regressions).

The browser suite is therefore not fully green. This card tracks driving the remaining 9 to green.

## Remaining failures (per reviewer's per-file audit at the time of yvrqky6)

- `inspector.repeat-open-focus` — 1 fail
- `inspector.close-restores-focus` — 2 fail
- `inspector.boundary-nav` — 3 fail
- `entity-focus.kernel-projection` — 1 fail
- `column-view.virtualized-nav` — 2 fail

## Acceptance Criteria

- Root-cause each remaining failure (likely further drift between the shared kernel-simulator and the production focus/spatial IPC path, but confirm per file — they may have distinct causes).
- Fix production code or the shared test helper as appropriate; do NOT weaken/delete assertions to force green unless a test genuinely asserts wrong behavior (justify if so).
- All 7 kernel-simulator consumer files fully green; no regressions elsewhere.
- `npx tsc --noEmit` clean.

## Tests

- Run each affected file in isolation first to confirm the failure and root cause (test ordering pollution was a factor in yvrqky6 — verify in isolation, not just in the full run).
- Follow TDD where a production fix is involved.

## Notes

Related to completed card yvrqky6 (shared `kernel-simulator.ts` focus-bridge fix). See also `6a45spw` (shared mock-spatial-kernel harness extraction) — these touch the same spatial/focus test-harness area and may share root causes or be worth coordinating.