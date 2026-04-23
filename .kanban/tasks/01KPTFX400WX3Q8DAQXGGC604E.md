---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff8880
project: spatial-nav
title: 'FocusScope: replace push-based notifyClaim with pull-based useFocusedMoniker subscription'
---
## What

Orphaned focus bars — a scope visually shows `data-focused="true"` even after spatial focus has moved elsewhere — can happen because the focus-decoration state is derived from a **push-based** notification system instead of a **pull-based** subscription to the focused moniker. The user has to nav in-and-out of a cell or card to clear the stale visual.

### Root cause

`kanban-app/ui/src/lib/entity-focus-context.tsx`:
- `useFocusSetter` (lines 216-264) stores focused moniker but also **pushes** transitions via two `notifyClaim` calls:
  ```ts
  const prev = getFocusedMoniker();
  setFocusedMoniker(moniker);
  notifyClaim(prev, false, ...);   // tell old scope to un-focus
  notifyClaim(moniker, true, ...); // tell new scope to focus
  ```
- `notifyClaim` (lines 275-287) iterates `monikerToKeys` → `claimRegistry` and calls each scope's `setIsClaimed` with the new boolean.
- Every `FocusScope` subscribes by registering a `setIsClaimed` callback via `registerClaim` in `useClaimRegistration` (focus-scope.tsx:94-111).
- `useFocusDecoration` (focus-scope.tsx:207-226) writes `data-focused="true"` imperatively on the element **when `isClaimed` becomes true**, and removes it when `isClaimed` becomes false.

### Failure modes this produces

1. **Unmount / remount race** — scope A is focused, user clicks B. `prev = A` is captured. If A unmounts before `notifyClaim(prev, false)` runs (because React effects have different cleanup timing), the old callback is already gone, but A's DOM element may briefly survive with `data-focused="true"` until React fully tears down.
2. **Successor mismatch with Rust** — `setFocus(B)` optimistically fires `notifyClaim(B, true)`. Rust's eventual `focus-changed` might pick C (e.g. successor logic from `01KPRGGCB5NYPW28AJZNM3D0QT`). The listener fires `notifyClaim(C, true)` but **nothing un-notifies B** — both now show focused until the next user action.
3. **Duplicate mounts** — if two `FocusScope` instances share a moniker for any reason (key collisions, stale mount during transition), both register in `monikerToKeys` and both get notified. Both stay focused.
4. **Dropped notifications** — any code path that calls `setFocusedMoniker` directly (e.g. from `focus-changed` event handling) without also calling `notifyClaim` leaves stale state.

### Why pull-based is better

The user's framing: *"am I focused? is the current focus me?"* That's the idempotent question. Every FocusScope reads `useFocusedMoniker()` and computes `isFocused = focusedMoniker === myMoniker`. No transition notifications, no "clear the old one" step, no possibility of missing a notification.

- Single source of truth — the focused moniker store (`useFocusedMonikerStore`)
- Each scope re-evaluates on every focus change (via `useSyncExternalStore`)
- Stale state is impossible: if the store says B is focused, only B evaluates to true, everyone else is false

### Performance note (not a blocker)

Every scope re-evaluating on every focus change could be a concern with hundreds of scopes. Two mitigations:
- `useSyncExternalStore` with a referentially-stable snapshot — only the two scopes that change focused state actually update DOM (React bails on same-value state).
- Use `useSyncExternalStoreWithSelector` if needed to subscribe ONLY when `focusedMoniker === myMoniker` flips, not on every change. But profile first — likely unnecessary.

### Approach

Rewrite the focus-decoration path to be pull-based:

1. **In `useFocusDecoration`** (or a new hook `useIsFocused(moniker)`) — read the focused moniker via `useFocusedMoniker()`, compute `active = focusedMoniker === moniker`, imperatively write `data-focused` accordingly. The existing `useFocusDecoration` already accepts `active: boolean` — just change the source of `active`.

2. **Remove the two `notifyClaim` calls** from `useFocusSetter` (lines 235-246 of entity-focus-context.tsx). The store update via `setFocusedMoniker(moniker)` is now the single signal.

3. **Decide the fate of `registerClaim` / `unregisterClaim` / `claimRegistryRef`**:
   - If no other code path depends on the push-callback mechanism, delete it.
   - If something does depend on it (e.g. some imperative-side-effect-on-focus consumer), keep the registry but remove its role in driving visual state. Audit callers of `registerClaim` to confirm.

4. **Keep `syncSpatialFocus`** (line 253) — Rust's `spatial_focus` invoke is a separate concern (updates Rust state) and stays push-based because Rust doesn't subscribe to React state.

5. **Keep `useFocusedMoniker()` as the canonical subscription hook** — existing consumers (e.g. `spatial-grid-fixture.tsx:FixtureCellDiv`) already use this pattern manually; the refactor just moves that pattern inside FocusScope itself.

### Files to modify

- `kanban-app/ui/src/components/focus-scope.tsx` — change how `useFocusDecoration`'s `active` prop is derived (from `useFocusedMoniker()` instead of claim callback state); possibly remove `useClaimRegistration` if its only purpose was driving visual state
- `kanban-app/ui/src/lib/entity-focus-context.tsx` — remove the two `notifyClaim` calls from `useFocusSetter`; delete `notifyClaim`, `claimRegistryRef`, `keyToMonikerRef`, `monikerToKeysRef`, `registerClaim`, `unregisterClaim` if unused after the refactor (audit first)
- `kanban-app/ui/src/components/focus-scope.test.tsx` — add regression tests covering the failure modes above

### Relationship to other tasks

- `01KPRGGCB5NYPW28AJZNM3D0QT` (always-something-focused invariant) — complementary. That task ensures the store has a valid moniker. This task ensures every scope's visual agrees with the store. Together: exactly one visual focus, always derived from the single source of truth.
- `01KPRGQ8WM2MC69WSRA5VZ9DZJ` (grid cursor as parallel state) — similar ethos: remove parallel state machines, derive from single source of truth.
- `01KPS22R2T4Q5QT9A71E7ZWAAP` and `01KPTFSDB4FKNDJ1X3DBP7ZGNZ` (inspector layer isolation) — may or may not be the same root cause. The "some fields work, some don't" in the inspector could be explained by this push-vs-pull bug: if `notifyClaim` is missing for some inspector field rows (e.g. when the inspector remounts on entity switch), their `data-focused` state drifts. Fixing push→pull could fix both.

### Out of scope

- Changing the Rust side (`spatial_focus` stays a push API from JS → Rust)
- Changing how claims interact with commands (command resolution via the scope chain is independent)
- Performance optimization beyond the note above — profile if tests surface regressions

## Acceptance Criteria

- [ ] After any sequence of `setFocus`, mount, unmount, and Rust `focus-changed` events, **exactly one** DOM element in the window has `data-focused="true"` (or zero, if no scope's moniker matches the focused store value)
- [ ] The reproduction case ("orphaned focus bar after rapid nav") no longer happens — verify by rapidly clicking between cells/cards/rows and observing at most one focus bar at any moment
- [ ] Removing `notifyClaim` does not break existing behavior — focus bar still appears on the correct scope on every focus change, including Rust-initiated `focus-changed` events
- [ ] A scope that unmounts while focused does not leave a ghost `data-focused` attribute on any surviving element
- [ ] Two scopes with the same moniker (if any exist) both show the focus bar, derived from the same store value (idempotence)

## Tests

- [ ] Add `kanban-app/ui/src/components/focus-scope.test.tsx` (or extend existing) cases:
  - `no_stale_data_focused_after_focus_moves_away` — render two scopes A and B, setFocus(A), setFocus(B), assert only B has `data-focused`
  - `no_stale_data_focused_after_focused_scope_unmounts` — render A focused, unmount A, assert no element has `data-focused` (this asserts cleanup path)
  - `data_focused_derives_from_store_not_notifications` — mock the store to return "C" directly without any notifyClaim call, assert C's element has `data-focused` (proves pull-based model works)
  - `rust_focus_changed_event_updates_visual_without_notifyClaim` — simulate a Rust `focus-changed` event that mutates the store, assert the target element has `data-focused` with no intermediate push
- [ ] Run `cd kanban-app/ui && npm test` — all 1301+ existing tests pass, 4+ new tests green
- [ ] Manual: rapidly press h/j/k/l across a dense grid for 30 seconds, watch for orphaned focus bars — none should appear. Click between various scopes (LeftNav, perspective, grid cell, row selector, inspector) in any order, verify single focus bar throughout.

## Workflow

- Use `/tdd` — write the four regression tests first (they should fail on the current push-based implementation), then refactor to pull-based.
- **Audit `registerClaim` callers** before deletion — if any consumer relies on the push callback for a non-visual reason, keep the registry but still remove `notifyClaim`'s role in visual state.
- If the refactor surfaces a performance issue (> 5ms per focus change with many scopes registered), profile and apply `useSyncExternalStoreWithSelector` targeted subscriptions. Don't pre-optimize.
- Do NOT touch Rust. The fix lives entirely in React.

