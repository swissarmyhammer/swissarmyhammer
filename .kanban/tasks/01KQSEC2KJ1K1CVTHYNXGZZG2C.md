---
assignees:
- claude-code
depends_on:
- 01KQSDP4ZJY5ERAJ68TFPVFRRE
- 01KQSEA6J8BCE1CAQ1S9XK7TFF
position_column: todo
position_ordinal: d180
project: spatial-nav
title: 'Spatial-nav follow-up B: unify Tauri IPC + React adapter for single FocusScope primitive'
---
## Reference

Parent: `01KQSDP4ZJY5ERAJ68TFPVFRRE`. Predecessor: sub-task A (`01KQSEA6J8BCE1CAQ1S9XK7TFF`) — kernel collapse must land first.

After this task lands, `cargo build --workspace` is clean again and `pnpm -C kanban-app/ui typecheck` passes for the adapter layer. React component callsites (`<FocusZone>` JSX) still use the old name — sub-task C handles those. Test files (~120 across Rust + TS) are still partially broken — sub-task D handles those.

## What

The kernel after sub-task A only accepts a single `register_scope` IPC. This sub-task rewrites the Tauri command bridge and the React adapter to match.

### Files to modify

- `kanban-app/src/commands.rs`
  - Delete `spatial_register_zone` Tauri command and its `spatial_register_zone_inner`. Keep only `spatial_register_scope` / `spatial_register_scope_inner`.
  - Delete or rewrite `spatial_register_zone_preserves_last_focused` test — `last_focused` is now a per-scope field, not zone-only. The same test premise reshapes as "registering a scope that previously had `last_focused` populated preserves it."
  - The `RegisterEntry` Rust type after sub-task A no longer has a `kind` discriminator; the wire format is whatever the kernel accepts (a flat struct with optional `last_focused` field). Adjust the deserialization shape accordingly.

- `kanban-app/src/main.rs`
  - The Tauri command registration list — drop `spatial_register_zone` from the `tauri::generate_handler!` invocation.

- `kanban-app/ui/src/lib/spatial-focus-context.tsx`
  - Currently has `useRegisterScope` and `useRegisterZone` as separate hooks plus a 2-section IPC dispatcher (`register_scope` / `register_zone`).
  - Collapse to a single `useRegisterScope` hook. The `SpatialFocusActions` interface's `registerScope` and `registerZone` methods either both become `registerScope` (preferred — one method) OR `registerZone` becomes a thin alias that delegates to `registerScope` for one release window.
  - Recommend: collapse to one method `registerScope`. Sub-task C updates every JSX callsite anyway, so the alias has no callers and just adds dead surface.
  - The dispatcher branch that called `invoke("spatial_register_zone", …)` is deleted; everything routes through `invoke("spatial_register_scope", …)`.
  - The `lastFocusedFq` parameter that was zone-only becomes optional on every register call; the kernel handles `None` correctly (per sub-task A).

- `kanban-app/ui/src/components/use-track-rect-on-ancestor-scroll.ts`
  - The hook today threads through `register_scope` vs `register_zone` (or `update_rect` against either). After this sub-task, only `update_rect` exists at the IPC layer (already singular) — but if the hook is parameterized by "which IPC to call to re-register," collapse that parameter.
  - Confirm: the staleness/timestamp threading from task `01KQQV2H8HW2BF3619DFXHX3RX` (sampledAtMs from gBCR) still works and lines up with the unified register surface.

### Out of scope for this sub-task

- React component JSX callsites (`<FocusZone>` → `<FocusScope>`) and `focus-zone.tsx` deletion — sub-task C.
- React-side test sweep (~120 files), kernel README rewrite — sub-task D.

### Decision dependencies (already approved by user)

1. Wire-format: `RegisterEntry` discriminator (`kind: "scope"|"zone"`) is dropped — implemented in sub-task A. This sub-task ensures the React side serializes the new shape correctly.
2. `last_focused` is optional on every `FocusScope`. Defaults to `None`. Set by the `setFocus` event chain when a child scope receives focus, just as it was for zones today.

## Acceptance Criteria

- [ ] `kanban-app/src/commands.rs` has no `spatial_register_zone` command — only `spatial_register_scope`.
- [ ] `kanban-app/src/main.rs` `tauri::generate_handler!` invocation drops `spatial_register_zone`.
- [ ] `kanban-app/ui/src/lib/spatial-focus-context.tsx` exports a single `useRegisterScope` hook (no `useRegisterZone`).
- [ ] `SpatialFocusActions` interface has a single `registerScope` method (no `registerZone`).
- [ ] All `invoke("spatial_register_zone", ...)` callsites are gone.
- [ ] `cargo build --workspace`: clean, zero warnings.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- [ ] `pnpm -C kanban-app/ui exec tsc --noEmit`: clean for `spatial-focus-context.tsx`, `use-track-rect-on-ancestor-scroll.ts`, and any other adapter-layer files. Component-layer files (`focus-zone.tsx`, all callsites) remain INTENTIONALLY broken — sub-task C picks them up.
- [ ] The `last_focused` preservation contract still holds end-to-end — verified by the (renamed/reshaped) preservation test from `commands.rs`.

## Tests

- [ ] Rewrite `spatial_register_zone_preserves_last_focused` in `kanban-app/src/commands.rs::tests` (or wherever it lives) as `spatial_register_scope_preserves_last_focused`. Same assertion: re-registering a scope with the same FQM preserves an existing `last_focused`. This test must pass.
- [ ] Add an adapter-layer typecheck test or smoke test confirming the React `useRegisterScope` hook calls `invoke("spatial_register_scope", { … })` exactly once and accepts an optional `lastFocusedFq` argument.
- [ ] `cargo nextest run -p kanban-app`: zero failures.
- [ ] `cargo build --workspace`: zero warnings (build-only check, since component layer still breaks vitest).

## Workflow

- Land sub-task A first; this one cannot start until the kernel surface settles.
- Build incrementally: rewrite `commands.rs`, fix `cargo build -p kanban-app`, then rewrite `spatial-focus-context.tsx`, then run typecheck on the adapter files only.
- Do NOT touch `<FocusZone>` JSX callsites — sub-task C handles those.
- Do NOT update test files outside the adapter layer — sub-task D handles those.
- If a hook signature change in `spatial-focus-context.tsx` cascades into component-layer typecheck failures, those failures are EXPECTED and are sub-task C's job to fix — don't try to absorb them here.
#spatial-nav-redesign