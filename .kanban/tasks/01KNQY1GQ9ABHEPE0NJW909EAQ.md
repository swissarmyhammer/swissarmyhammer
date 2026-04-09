---
assignees:
- claude-code
depends_on:
- 01KNQXZ81QBSS1M9WFD7VQJNAJ
- 01KNQXZZ9VQBHFX091P0K4F4YC
- 01KNQY0P9J03T24FSM8AVPFPZ9
position_column: todo
position_ordinal: a780
project: spatial-nav
title: 'Clean up: rename claimWhen to navOverride, remove bulk predicate code'
---
## What

After all views are migrated to spatial navigation, clean up the predicate infrastructure and replace the broadcast-based `claimWhen` with a simpler, targeted override model.

### New override model

The old `claimWhen` was pull-based: every scope registers predicates, `broadcastNavCommand` walks all of them on every keystroke — O(all predicates). The new model is **targeted**: overrides live on the focused scope only, checked as a lookup table before spatial nav.

**Interface change:**

```typescript
// Old: predicate function evaluated globally
claimWhen={[{ command: "nav.right", when: (f) => f === "task:01" }]}

// New: directive map on the focused scope
navOverride={{ "nav.right": "task:02", "nav.left": null }}
```

- **String value**: "go here instead of spatial nav"
- **`null` value**: "block this direction, don't navigate"
- **Missing key**: "use spatial nav (default)"

**Rust-side**: The override map is stored in `SpatialEntry.overrides: HashMap<Direction, Option<String>>`. The `navigate()` function checks the focused entry's overrides first. If an override exists for the requested direction, return it (or None for null/blocked). Only if no override exists does the beam test + scoring run.

**React-side**: `navOverride` prop on FocusScope is a `Record<string, string | null>`. FocusScope sends it to Rust via `spatial_register`. No more predicate registry, no more `registerClaimPredicates` / `unregisterClaimPredicates` / `claimPredicatesRef`.

This matches how every other system works: Android's `nextFocusRight` is a property on the view, UWP's `XYFocusRight` is a property on the element. Not a global listener.

### Clean up

1. **Replace `claimWhen` with `navOverride`** — new prop type is `Record<string, string | null>`, not `ClaimPredicate[]`
2. **Delete predicate broadcast infrastructure** from `entity-focus-context.tsx`:
   - Remove `ClaimPredicate` interface
   - Remove `claimPredicatesRef`, `registerClaimPredicates`, `unregisterClaimPredicates`
   - Remove the predicate walk from `broadcastNavCommand` (spatial nav is the only path now, with overrides checked in Rust)
3. **Remove `useRestoreFocus`** — FocusLayer's focus memory replaces this
4. **Send overrides to Rust** via `spatial_register` — the override map travels with the spatial entry

### Subtasks
- [ ] Replace `claimWhen: ClaimPredicate[]` with `navOverride: Record<string, string | null>` on FocusScope
- [ ] Add `overrides: HashMap<Direction, Option<String>>` to `SpatialEntry` in Rust
- [ ] Update `navigate()` to check focused entry's overrides before beam test
- [ ] Delete `ClaimPredicate` type, predicate registry, and broadcast walk from entity-focus-context
- [ ] Remove `useRestoreFocus` hook (replaced by FocusLayer focus memory)

## Acceptance Criteria
- [ ] `claimWhen` prop removed, replaced by `navOverride` with simple directive map
- [ ] Override check is O(1) — lookup on focused entry, no broadcast walk
- [ ] `navOverride={{ "nav.right": "task:02" }}` sends focus to task:02 on nav.right
- [ ] `navOverride={{ "nav.left": null }}` blocks nav.left (no movement)
- [ ] Missing direction key → spatial nav (default behavior)
- [ ] Overrides stored in Rust `SpatialEntry`, checked in `navigate()` before beam test
- [ ] `ClaimPredicate` type deleted, predicate registry deleted
- [ ] `useRestoreFocus` deleted
- [ ] `cargo test` passes, `pnpm vitest run` passes

## Tests
- [ ] `Rust unit tests` — navigate with override returns override target
- [ ] `Rust unit tests` — navigate with null override returns None (blocked)
- [ ] `Rust unit tests` — navigate with no override for direction falls through to spatial
- [ ] `focus-scope.test.tsx` — navOverride prop accepted and sent to Rust
- [ ] All navigation works via spatial nav with no overrides in production code
- [ ] Run `cargo test` and `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.