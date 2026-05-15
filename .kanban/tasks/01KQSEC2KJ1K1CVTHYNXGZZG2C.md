---
assignees:
- claude-code
depends_on:
- 01KQSDP4ZJY5ERAJ68TFPVFRRE
- 01KQSEA6J8BCE1CAQ1S9XK7TFF
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffff8980
project: spatial-nav
title: 'Spatial-nav follow-up B: unify Tauri IPC + React adapter for single FocusScope primitive'
---
## Reference

Parent: `01KQSDP4ZJY5ERAJ68TFPVFRRE`. Predecessor: sub-task A (`01KQSEA6J8BCE1CAQ1S9XK7TFF`) — kernel collapse must land first.

After this task lands, `cargo build --workspace` is clean again and `pnpm -C kanban-app/ui typecheck` passes for the adapter layer. React component callsites (`<FocusZone>` JSX) still use the old name — sub-task C handles those. Test files (~120 across Rust + TS) are still partially broken — sub-task D handles those.

## What

The kernel after sub-task A only accepts a single `register_scope` IPC. This sub-task rewrites the Tauri command bridge and the React adapter to match.

### Files modified

- `kanban-app/src/commands.rs`
  - Deleted `spatial_register_zone` Tauri command and its `spatial_register_zone_inner`. Kept only `spatial_register_scope` / `spatial_register_scope_inner`. The `_inner` now passes `last_focused: None` and lets the kernel's `register_scope` carry forward any prior value (the unified preservation path).
  - Renamed `spatial_register_zone_preserves_last_focused` test to `spatial_register_scope_preserves_last_focused` — re-registers a scope at the same FQM and asserts a prior `last_focused` survives the placeholder/real-mount swap.
  - `spatial_register_batch_inner` now returns `()` (was `Result<(), BatchRegisterError>`); after the kernel collapse `apply_batch` is infallible. The corresponding kind-mismatch test was deleted (no longer reachable). The remaining batch test was reshaped to use the flat `RegisterEntry` struct with no `kind` discriminator.
  - Dropped `BatchRegisterError` and `FocusZone` from the kernel-import line.

- `kanban-app/src/main.rs`
  - Dropped `commands::spatial_register_zone` from the `tauri::generate_handler!` invocation.

- `kanban-app/ui/src/lib/spatial-focus-context.tsx`
  - Collapsed `SpatialFocusActions.registerScope` and `registerZone` into a single `registerScope` method.
  - Deleted the `registerZone` implementation (which `invoke`d `spatial_register_zone`) and the corresponding entry in the returned actions bag.
  - Updated the `registerScope` doc to reflect the kernel's single registered primitive (no separate zone wire shape) and the `last_focused` server-owned-memory contract.

- `kanban-app/ui/src/components/use-track-rect-on-ancestor-scroll.ts`
  - No code change needed — the hook only calls `update_rect` (already singular at the IPC layer) and was not parameterized by zone-vs-scope. The `sampledAtMs` staleness threading is preserved untouched.

### Out of scope for this sub-task

- React component JSX callsites (`<FocusZone>` → `<FocusScope>`) and `focus-zone.tsx` deletion — sub-task C.
- React-side test sweep (~120 files), kernel README rewrite — sub-task D.

### Decision dependencies (already approved by user)

1. Wire-format: `RegisterEntry` discriminator (`kind: "scope"|"zone"`) is dropped — implemented in sub-task A. This sub-task ensures the React side serializes the new shape correctly.
2. `last_focused` is optional on every `FocusScope`. Defaults to `None`. Set by the `setFocus` event chain when a child scope receives focus, just as it was for zones today.

## Acceptance Criteria

- [x] `kanban-app/src/commands.rs` has no `spatial_register_zone` command — only `spatial_register_scope`.
- [x] `kanban-app/src/main.rs` `tauri::generate_handler!` invocation drops `spatial_register_zone`.
- [x] `kanban-app/ui/src/lib/spatial-focus-context.tsx` exports a single `useRegisterScope` hook (no `useRegisterZone`). N/A — only methods exist on the actions interface (no separate hooks). `SpatialFocusActions.registerScope` is the single method.
- [x] `SpatialFocusActions` interface has a single `registerScope` method (no `registerZone`).
- [x] All `invoke("spatial_register_zone", ...)` callsites are gone.
- [x] `cargo build --workspace`: clean, zero warnings.
- [x] `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- [x] `pnpm -C kanban-app/ui exec tsc --noEmit`: clean for `spatial-focus-context.tsx`, `use-track-rect-on-ancestor-scroll.ts`, and any other adapter-layer files. Component-layer files (`focus-zone.tsx`, all callsites) remain INTENTIONALLY broken — sub-task C picks them up. Confirmed: 4 remaining errors all in `focus-zone.tsx` (component, sub-task C) and `scroll-on-edge.test.ts` (test, sub-task D).
- [x] The `last_focused` preservation contract still holds end-to-end — verified by `spatial_register_scope_preserves_last_focused` in `commands.rs`.

## Tests

- [x] Renamed `spatial_register_zone_preserves_last_focused` to `spatial_register_scope_preserves_last_focused` — re-registering a scope with the same FQM preserves any existing `last_focused`. Test passes.
- [x] Adapter-layer typecheck/smoke test confirms `registerScope` calls `invoke("spatial_register_scope", { … })` exactly once (existing test in `spatial-focus-context.test.tsx::"invokes spatial_register_scope with the full kernel-types record"`). The optional `lastFocusedFq` parameter does not exist as a separate argument — `last_focused` is server-owned memory and is intentionally not on the wire (per the kernel docs and the unified `RegisterEntry` shape from sub-task A). The optional argument that does exist on `registerScope` is `sampledAtMs` for the rect-staleness validator.
- [x] `cargo nextest run -p kanban-app`: 92 tests run, 92 passed, 0 failures.
- [x] `cargo build --workspace`: zero warnings.

## Workflow

- Land sub-task A first; this one cannot start until the kernel surface settles.
- Build incrementally: rewrite `commands.rs`, fix `cargo build -p kanban-app`, then rewrite `spatial-focus-context.tsx`, then run typecheck on the adapter files only.
- Do NOT touch `<FocusZone>` JSX callsites — sub-task C handles those.
- Do NOT update test files outside the adapter layer — sub-task D handles those.
- If a hook signature change in `spatial-focus-context.tsx` cascades into component-layer typecheck failures, those failures are EXPECTED and are sub-task C's job to fix — don't try to absorb them here.
#spatial-nav-redesign

## Review Findings (2026-05-04 08:26)

### Warnings
- [x] `kanban-app/ui/src/lib/focus-debug-context.tsx:4,22,77` — Three doc-comment references to `<FocusZone>` survive in this adapter-layer file. The same kind of doc-only stale-mention swept in `spatial-focus-context.tsx` (which the task explicitly accepts as in-scope). For consistency the FocusZone → FocusScope rename should land on every adapter-layer (`lib/`) file in this sub-task — leaving them for sub-task C means a non-adapter task is fixing adapter-layer prose. Suggestion: replace each `<FocusZone>` mention with `<FocusScope>`, or rephrase to "the spatial-nav primitive." Resolved: lines 4 and 77 now read `<FocusLayer>` + `<FocusScope>` (dropped the dead `<FocusZone>` clause); line 22 swapped to `<FocusScope>`. No `FocusZone` text remains in `focus-debug-context.tsx`.
- [x] `kanban-app/ui/src/lib/entity-focus-context.tsx:22` — Same issue — the docstring example uses `<FocusZone moniker="card:T1">`. Swap to `<FocusScope moniker="card:T1">` so the documented example still compiles after sub-task C deletes `<FocusZone>`. Resolved: example now reads `<FocusScope moniker="card:T1">`.
- [x] `kanban-app/ui/src/lib/scroll-on-edge.ts:242` — Same issue — JSDoc says `<FocusScope> / <FocusZone>`. After sub-task C the second clause becomes a dangling reference. Drop the `/ <FocusZone>` half (it is a single primitive after the collapse) so the comment reflects the post-collapse contract. Resolved: JSDoc now describes a single `<FocusScope>` primitive (no slash, no dangling reference).

Sweep verification: `grep FocusZone kanban-app/ui/src/lib/` returns zero matches.

### Nits
- [x] `kanban-app/ui/src/components/column-view.tsx:310-318,400` — `ScopeRegisterEntry` still carries `kind: "scope"` and the off-screen entries set it. Sub-task A removed the `kind` discriminator from the wire `RegisterEntry`; serde silently drops the extra field, so this works at runtime but is dead/misleading wire shape. Strictly sub-task C's territory (it is in `components/`, not adapter), but flagging here so the C-author drops `kind: "scope"` from the local interface rather than carrying the stale field forward. Acknowledged — out of scope for this sub-task per the reviewer's own note (file lives in `components/`, not the adapter `lib/`). Forwarded to sub-task C.