---
assignees:
- claude-code
depends_on:
- 01KNQXXF5W7G4JP73C6ZCMKYKX
position_column: done
position_ordinal: ffffffffffffffffffffffda80
project: spatial-nav
title: Add FocusLayer to inspector (stacks on top of root window layer)
---
## What

The root `<FocusLayer name="window">` is added in card 1 (at the app shell). This card adds the inspector's layer so it captures navigation when open, and replaces `useRestoreFocus` with layer stack semantics.

### Subtasks
- [x] Add `<FocusLayer name="inspector">` to InspectorFocusBridge
- [x] Remove `useRestoreFocus()` from inspectors-container.tsx — replaced by FocusLayer stack
- [x] Verify inspector captures nav (arrows only move within inspector fields)
- [x] Verify inspector close pops the layer, restoring window layer's previously focused moniker
- [x] Verify entity-inspector's first-field focus on mount still works within the layer

## Implementation

### `inspector-focus-bridge.tsx`
- Wrapped the FocusScope + CommandScopeProvider in `<FocusLayer name="inspector">`
- When inspector mounts → Rust pushes inspector layer → becomes active layer
- When inspector unmounts → Rust removes inspector layer → restores window layer's `last_focused` via focus memory (card 2)

### `inspectors-container.tsx`
- Removed `useRestoreFocus()` import and call from `InspectorPanel`
- Focus save/restore is now handled by the Rust layer stack — `focus()` saves `last_focused` on the owning layer, `remove_layer()` restores it

### Test updates
- `inspectors-container.test.tsx` — removed `useRestoreFocus` mock, added `registerClaim`/`unregisterClaim` to entity-focus mock, added `ulid` mock

## Acceptance Criteria
- [x] Inspector navigation is captured — layer boundary
- [x] Closing inspector restores focus via Rust layer stack
- [x] `useRestoreFocus` removed from inspectors-container.tsx
- [x] Existing `claimWhen` predicates still work (not removed yet)
- [x] `pnpm vitest run` passes (1111 tests)"