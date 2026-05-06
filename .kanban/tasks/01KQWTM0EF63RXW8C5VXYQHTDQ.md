---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffff9c80
project: spatial-nav
title: 'spatial-nav redesign step 11 follow-up: migrate UI test infrastructure off the deleted IPCs'
---
## Parent

Follow-up to step 11 (`01KQW6H3397154YJWDPD6TDYZ3`) of the spatial-nav redesign (parent `01KQTC1VNQM9KC90S65P7QX9N1`).

## Goal

Migrate all UI test infrastructure that observes scope registration via the deleted IPCs (`spatial_register_scope`, `spatial_unregister_scope`, `spatial_update_rect`) onto a new observation point — the consumer-owned `LayerScopeRegistry`.

## Background

Step 11 deleted the three IPCs from production code (Rust kernel + frontend actions + `<FocusScope>` mount IPC effect). After step 11, `<FocusScope>` mount no longer fires `spatial_register_scope`; it registers only in the per-layer `LayerScopeRegistry`.

The existing UI test infrastructure observes scope registration via `mockInvoke.mock.calls.filter(c => c[0] === "spatial_register_scope")`. Approximately 295 tests across ~62 files depend on this pattern. With the IPC gone, all those tests fail because the assertions never see the expected calls.

## Approach

1. **Update `kanban-app/ui/src/test/spatial-shadow-registry.ts`** — replace its `mockInvoke`-based capture with a hook into `LayerScopeRegistry`. Provide a similar API surface (`registry`, `getRegisteredFqBySegment`) but populated from the React-side registry mounts.

2. **Update `kanban-app/ui/src/test-helpers/kernel-simulator.ts`** — same migration. The simulator currently records every `spatial_register_*` IPC; switch to subscribing to LayerScopeRegistry adds/deletes.

3. **Update individual component tests** that use `registerScopeArgs()` patterns (badge-list-nav, field.with-icon, column-view.scroll-rects, inspector.entity-zone-barrier, etc.) — replace with reading from the LayerScopeRegistry context. Most tests can continue to use a `registerScopeArgs()` wrapper that reads from the registry.

4. **Update the soak/end-to-end suites** (`spatial-nav-soak.spatial.test.tsx`, `spatial-nav-end-to-end.spatial.test.tsx`, `board-view.cross-column-nav.spatial.test.tsx`, `entity-card.in-zone-nav.spatial.test.tsx`) — these mount the production stack. They need the shadow registry to populate from LayerScopeRegistry instead of from `mockInvoke`.

## Files

- `kanban-app/ui/src/test/spatial-shadow-registry.ts`
- `kanban-app/ui/src/test-helpers/kernel-simulator.ts`
- ~62 test files under `kanban-app/ui/src/components/` and `kanban-app/ui/src/lib/`

## Acceptance criteria

- Every UI test that previously asserted on `spatial_register_scope` / `spatial_unregister_scope` / `spatial_update_rect` IPCs now asserts via the LayerScopeRegistry observation point
- `pnpm vitest run` passes with no `spatial_register_scope`-related failures
- `pnpm tsc --noEmit` is clean
- The `spatial-shadow-registry` and `kernel-simulator` helpers expose the same API surface as before (so tests don't need shape changes — only the helper's internals migrate)

## Out of scope

- Any production-side change (this is purely test infrastructure)
- Snapshot-path coverage gaps (covered separately)
#stateless-nav