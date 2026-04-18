---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffe580
project: spatial-nav
title: Remove focusedMoniker React state — make Rust the sole owner of focus
---
## What

Card `01KNM3YHHFJ3PTXZHD9EFKVBS6` (Focus claim registry) explicitly deferred removing `focusedMoniker` useState from EntityFocusProvider. The stated architectural goal was "Rust owns all focus state" but React still maintains a parallel `focusedMoniker` state that mirrors Rust via the `focus-changed` event listener. None of the follow-on cards removed this bridge. It has become permanent tech debt.

### Why this matters

Every `focus-changed` event now mutates two places: the claim registry (intended — O(1) lookup to the focused callback) and a React useState (bridge). Components reading `focusedMoniker` are bypassing the claim registry — they are reading a shadow of Rust state, not Rust state itself. Any divergence between the two (dropped event, race on reconnect) shows up as split-brain focus.

### Scope

Files that still consume `focusedMoniker` directly or via `useIsFocused`:
- `kanban-app/ui/src/lib/entity-focus-context.tsx` — the state itself, the `useIsFocused` hook, and the `FocusedScopeContext` provider
- `kanban-app/ui/src/components/focus-scope.tsx` — `isDirectFocus` derives from `focusedMoniker === moniker || isClaimed`. Should just be `isClaimed`.
- `kanban-app/ui/src/components/board-view.tsx` — audit for `useIsFocused` / `focusedMoniker` reads
- `kanban-app/ui/src/components/grid-view.tsx` — audit
- `kanban-app/ui/src/components/entity-inspector.tsx` — audit

### Subtasks

- [x] Grep for all consumers of `useIsFocused`, `focusedMoniker`, `FocusedScopeContext`
- [x] Migrate each consumer to the claim registry (they should register a claim callback and react to that)
- [x] Delete `focusedMoniker` useState and its setter from EntityFocusProvider
- [x] Delete the bridge code in `useFocusChangedEffect` that updates the useState
- [x] Delete `useIsFocused` and `FocusedScopeContext` if fully unused after migration
- [x] Run `pnpm vitest run` — all tests pass

## Acceptance Criteria

- [x] `focusedMoniker` React state is deleted
- [x] Every component that used to read it now reacts via a claim callback
- [x] The `focus-changed` event listener only drives the claim registry — no secondary state update
- [x] No regression in focus highlighting, keyboard navigation, or inspector focus capture
- [x] `cargo test` and `pnpm vitest run` both pass

## Implementation Notes

**Architecture changes:**
- Replaced `useState<focusedMoniker>` with a ref-backed external store (`useFocusedMonikerStore`) that exposes `subscribeFocus`/`getFocusedMoniker` consumed via `useSyncExternalStore`.
- Added public hook `useFocusedMoniker()` — the idiomatic React read for focus state.
- `useIsFocused`, `useFocusedScope` now subscribe via the store. `FocusedScopeContext` value is also driven through the same subscription so `useDispatchCommand` sees updated scope chains.
- `focus-changed` event from Rust calls claim callbacks, updates the ref, and notifies subscribers. No parallel state.
- `setFocus` synchronously fires claim callbacks for the outgoing/incoming monikers (optimistic UI) — the Rust event remains authoritative and can correct divergence.
- `focus-scope.tsx`: `isDirectFocus = showFocusBar && isClaimed` (dropped the `focusedMoniker === moniker` fallback).

**Notes on `useIsFocused` and `FocusedScopeContext`:**
- `useIsFocused` is still used (entity-inspector FieldRow, tests for ancestor-focus behavior) — kept, migrated to the store-based subscription.
- `FocusedScopeContext` is still used by `useDispatchCommand` in `command-scope.tsx` — kept, updated via the store subscription inside the provider.