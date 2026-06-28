---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kv6erqc6xcwmtkchmbb2zpr7
  text: 'Picked up. Confirmed red: `npx vitest run src/components/entity-inspector.test.tsx` → 6 failed | 21 passed. All 6 fail on the title field zone never getting `data-focused="true"` (the default mount auto-focus).'
  timestamp: 2026-06-15T20:15:03.046297+00:00
- actor: claude-code
  id: 01kv6es6g3dy0ja505wvz5422t
  text: |-
    ROOT CAUSE (production wiring is correct; the test harness was stale):

    The card's suspected cause (stale FQM window-rooting) was wrong. Real cause: the kernel simulator (`src/test-helpers/kernel-simulator.ts`, used by `entity-inspector.test.tsx`) only intercepts the LEGACY Tauri command names — `spatial_focus`, `spatial_navigate`, `spatial_push_layer`, etc. But production focus IPC was migrated to the in-process `focus` MCP server: `focus-mcp.ts::setFocus` calls `callMcpTool("focus", "set focus", …)`, which lowers onto `invoke("command_tool_call", { module: "focus", tool: "focus", op: "set focus", params })` (`mcp-transport.ts`).

    On mount, `EntityInspector`'s `useFirstFieldFocus` dispatches `nav.focus` → the webview-bus handler in `SpatialFocusProvider` runs `actions.focus(fq)` → `setFocus` → `callMcpTool("focus","set focus",…)` → `command_tool_call`. The simulator did NOT handle `command_tool_call`, so it fell through to the test's `fallbackInvoke` (returns "ok", emits NO `focus-changed`). The entity-focus bridge (`entity-focus-context.tsx`) is the sole upstream of `store.set`, and it only writes on `focus-changed` — so the title zone's `data-focused` never flipped.

    Proof it was the harness, not pollution: in the FULL run, "first visible field has data-focused by default" passed only because an earlier test left `currentFocus` populated; run in isolation it failed identically (`expected null to be 'true'`). The sibling browser tests (field-vertical-nav / field-enter-drill) pass because they seed focus by directly invoking the `focus-changed` listeners (`fireFocusChanged`), bypassing the `set focus` round-trip entirely.

    FIX: Taught `installKernelSimulator` to lower the production `command_tool_call` bridge (module `focus`) back onto its existing `spatial_*` op handlers — exactly the wiring production uses. Added `lowerFocusBridge(op, params)` mapping each focus op (`set focus`→`spatial_focus`, `clear focus`→`spatial_clear_focus`, `navigate focus`→`spatial_navigate`, `push/pop layer`, `drill_in/out layer`) to the legacy command + args, wrapping pop/drill returns into the `{ ok, next_fq }` envelope the focus-mcp wrappers read. Extracted the legacy branch logic into `dispatchSpatial(cmd, a)` so one cascade implementation serves both wire shapes. No test assertions weakened; no production code changed.

    File: apps/kanban-app/ui/src/test-helpers/kernel-simulator.ts
  timestamp: 2026-06-15T20:15:18.531036+00:00
- actor: claude-code
  id: 01kv6eserc62y4yrfda67c555v
  text: |-
    VERIFIED GREEN:
    - `npx vitest run src/components/entity-inspector.test.tsx` → 27 passed (27), 0 failed (was 6 failed | 21 passed). All 6 target failures fixed, zero regression to the other 21.
    - Isolated re-run of "first visible field has data-focused attribute by default" now passes on its own (no longer depends on test-ordering pollution).
    - `npx tsc --noEmit` → exit 0, clean.

    No-regression check on the 7 other files that consume `installKernelSimulator`: those failures PRE-EXIST on HEAD and have separate root causes out of this card's scope. The fix is a net improvement, never a regression — measured by swapping HEAD's kernel-simulator back in:
    - HEAD (unmodified simulator): 15 failed | 18 passed across those 7 files.
    - With this fix: 9 failed | 34 passed across those 7 files.
    Restored my version after the comparison; `git diff --stat HEAD` shows only kernel-simulator.ts changed.

    Moving to review.
  timestamp: 2026-06-15T20:15:26.988633+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffb680
project: builtin-commands
title: Fix 6 pre-existing entity-inspector.test.tsx browser failures (data-focused not set on title zone)
---
## Problem

`apps/kanban-app/ui/src/components/entity-inspector.test.tsx` has 6 pre-existing failing browser tests (`6 failed | 21 passed`). All fail on the same assertion shape:

```
expect(titleFocusScope!.getAttribute("data-focused")).toBe("true")
// Expected: "true"  Received: null
```

e.g. at the `declarative sections` group ("dates section's field rows render as zones…", and the multi-section variant). The title field zone (`[data-segment='field:task:test-id.title']`) is not getting `data-focused="true"` when the test seeds the title as focused.

## Evidence it is pre-existing / unrelated

Discovered while verifying task 01KV3P5FHF2AFVR948CTZAV13S (field-pill spatial-nav tests). Stashing that task's only changed file (the field-enter-drill test) and re-running `entity-inspector.test.tsx` reproduces the identical `6 failed | 21 passed`. So these failures are independent of the field-enter-drill migration.

## Likely root cause (suspect)

Same architectural shift as commit `f6a56d7c1` ("host-driven nav/drill") and the window-rooting change it flagged as KNOWN FOLLOW-UP #1: "UI spatial fixture tests that hardcode `/window/...` focus FQM roots will FAIL until updated — the window-rooting change moves roots from `/window/...` to `/<window-label>/window/...`." These `entity-inspector.test.tsx` cases likely seed focus via a stale FQM root or pre-host-driven focus path, so the entity-focus bridge never mirrors the moniker into the store and the zone wrapper never flips `data-focused`.

## What to do
- Run `cd apps/kanban-app/ui && npx vitest run src/components/entity-inspector.test.tsx` red first.
- Root-cause why the title zone's `data-focused` is never set — compare against the now-passing `entity-inspector.field-enter-drill.browser.test.tsx` and `entity-inspector.field-vertical-nav.browser.test.tsx`, which exercise the host-driven focus-changed → store-mirror → `data-focused` path correctly.
- Fix the test seeding (or production wiring if a real bug) so the host-driven focus contract is honored. Do not weaken assertions.

## Acceptance Criteria
- [ ] `npx vitest run src/components/entity-inspector.test.tsx` → all pass.
- [ ] No regression to the surrounding suite.

#Test-Failure #frontend #navigation