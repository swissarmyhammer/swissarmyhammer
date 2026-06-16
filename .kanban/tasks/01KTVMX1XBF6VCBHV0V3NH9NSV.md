---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kv8arxc99c6kw9djrzqp9hnq
  text: 'Finish loop: verified 5/5 browser tests pass + tsc --noEmit clean. Scoped review found 0 blockers; the card''s actual contribution (setFocusFqOf helper) is clean, and all flagged items are pre-existing clarity nits in shared scaffolding (confirmed via git show on 66e47ccfb). Moved to done.'
  timestamp: 2026-06-16T13:43:43.753535+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffbc80
project: ui-command-cleanup
title: Fix pre-existing perspective-tab-bar.filter-migration.test.tsx failures (filter button click → empty spatial_focus fq)
---
## What
Two tests in `apps/kanban-app/ui/src/components/perspective-tab-bar.filter-migration.test.tsx` fail on branch `plugin`:

- `filter_button_click_dispatches_nav_focus_with_filter_editor_fq` — "spatial_focus.fq must end with filter_editor:p1 (got )"
- `filter_button_click_targets_the_currently_active_perspective` — "spatial_focus.fq must end with filter_editor:p2 when p2 is active (got )"

A `spatial_focus` IPC fires but its `fq` payload is empty, suggesting `FilterEditorDrillOutWiring`'s FQM-ref handoff to the Filter tab button yields null/empty at click time in this fixture (or the last matching IPC call has a different wire shape than the test unwraps).

## Evidence it pre-dates Card E (01KTED7PFKRS6GMAQKVDCQA07V)
Verified during Card E implementation (2026-06-11): restoring `perspective-tab-bar.tsx` to its HEAD version (before any Card E edits) reproduces the exact same 2 failures, so the regression was already present in the working tree / branch before the editor drill-in move. The test file itself is unmodified in git.

## Done means
- Root cause identified (empty `fq` in the `spatial_focus` IPC, or stale test unwrap shape).
- Both tests green without weakening their assertions.

## Resolution (2026-06-12)
Root cause: **stale test unwrap shape** (the second hypothesis), not an empty fq from production.

The MCP-transport cutover moved the focus commit from `invoke("spatial_focus", { fq, snapshot })` to `invoke("command_tool_call", { module: "focus", tool: "focus", op: "set focus", params: { fq, snapshot, window } })` (see `apps/kanban-app/ui/src/lib/focus-mcp.ts::setFocus` and `mcp-transport.ts::callMcpTool`). The codemod that swept the test suite updated this file's call *filter* to match the new `command_tool_call`/`set focus` shape, but left the `fq` unwrap reading the legacy top-level field (`(lastCall[1] as { fq?: string })?.fq`). Under the new shape the FQM lives at `params.fq`, so the unwrap always yielded `""` and `endsWith("filter_editor:pN")` failed.

Fix (test-only): added a local `setFocusFqOf()` helper that unwraps `bag.fq ?? bag.params.fq` (tolerating both transport vintages, mirroring `focus-scope.test.tsx`'s helper) and pointed both assertions at it. Assertions unchanged in strength — they now verify the real fq (`…filter_editor:p1` / `…filter_editor:p2`), which an empty production fq would still fail.

Verification: `npx vitest run src/components/perspective-tab-bar.filter-migration.test.tsx` → 5 passed (was 2 failed / 3 passed); `npx tsc --noEmit` → exit 0, no errors. Production code untouched.