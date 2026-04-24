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
title: 'navOverride cleanup: layer-scoped, Option&lt;Moniker&gt; overrides, remove predicate code'
---
## What

After all views migrate to spatial navigation, clean up the predicate infrastructure and replace broadcast-based `claimWhen` with a simpler, targeted override model. Overrides are layer-scoped and use newtypes throughout — `Option<Moniker>` on the Rust side, branded `Moniker | null` on the TS side.

### Crate placement

Per the commit-`b81336d42` refactor pattern, the Rust side lives in `swissarmyhammer-kanban/src/focus/`:
- `overrides` field already on `Focusable` / `FocusZone` in `focus/scope.rs` (card `01KNQXW7HH...`)
- `check_override` implemented in `focus/navigate.rs` as rule-0 of the beam search (card `01KNQXXF5W...`)
- No new Tauri commands needed — overrides travel with `spatial_register_focusable` / `spatial_register_zone`
- Tests in `swissarmyhammer-kanban/tests/focus_overrides.rs`

React side lives in `kanban-app/ui/src/` — `navOverride` prop on the primitives and composite `FocusScope`.

### New override model

Old: pull-based predicates, walked on every keystroke — O(all predicates).
New: direct directive map on the focused scope — O(1) lookup.

**Interface:**

```typescript
// TypeScript (uses branded Moniker from card 01KNM3YHHFJ3...)
type NavOverride = Partial<Record<Direction, Moniker | null>>;

// Example
navOverride={{
  "right": Moniker("task:02"),   // go here instead of spatial nav
  "left":  null,                 // block this direction
  // "up" omitted → use spatial nav (default)
}}
```

- **`Moniker` value** — "go here instead of spatial nav"
- **`null` value** — "block this direction, no movement"
- **Missing key** — "use spatial nav default"

### Rust side — all newtyped

```rust
// Already on Focusable and FocusZone in card 01KNQXW7HH...
pub struct Focusable {
    // ...
    pub overrides: HashMap<Direction, Option<Moniker>>,
}

pub struct FocusZone {
    // ...
    pub overrides: HashMap<Direction, Option<Moniker>>,
}
```

The `navigate()` function gets an override rule-0 check before beam test. The override-resolution helper's signature:

```rust
fn check_override(
    &self,
    focused: &FocusScope,
    direction: Direction,
) -> Option<Option<Moniker>> {
    //  ^^^^^^^^^^^^^^^^^^^^^^^ outer Option = "did override apply?"
    //                          inner Option = "target or blocked?"

    let ov = focused.overrides().get(&direction)?;
    match ov {
        None => Some(None),  // explicit block
        Some(target_moniker) => {
            // Resolve only within the focused entry's layer — cross-layer overrides ignored.
            let target_in_layer = self.scopes_in_layer(focused.layer_key())
                .any(|s| s.moniker() == target_moniker);
            if target_in_layer {
                Some(Some(target_moniker.clone()))
            } else {
                None  // fall through to beam search
            }
        }
    }
}
```

- Outer `None` → override didn't apply; beam search runs.
- Outer `Some(None)` → explicit block; `navigate` returns `None`.
- Outer `Some(Some(moniker))` → override target; `navigate` returns `Some(moniker)`.

Note: the signature compares `Moniker == Moniker` — typed equality, not string equality.

### Layer scoping enforced at resolution

An override target moniker is resolved **only within the focused entry's layer** (`focused.layer_key()`). If the target exists but lives in a different layer, the override is **ignored** and spatial nav runs as usual. Cross-layer teleportation is never allowed, even via override.

### Clean up

1. Replace `claimWhen: ClaimPredicate[]` with `navOverride: NavOverride` on all component call sites
2. Delete predicate broadcast infrastructure from `entity-focus-context.tsx`:
   - Remove `ClaimPredicate` interface
   - Remove `claimPredicatesRef`, `registerClaimPredicates`, `unregisterClaimPredicates`
   - Remove the predicate walk from `broadcastNavCommand` (spatial nav is the only path; overrides run in Rust)
3. Confirm `useRestoreFocus` is gone (replaced by FocusLayer `last_focused` in card `01KNQXYC4RB...`)
4. Send overrides to Rust via the typed register commands — `overrides: HashMap<Direction, Option<Moniker>>` on the wire

### Subtasks
- [ ] Replace `claimWhen: ClaimPredicate[]` with `navOverride: NavOverride` on `Focusable`/`FocusZone` React primitives and `FocusScope` wrapper
- [ ] Wire `overrides: HashMap<Direction, Option<Moniker>>` into `Focusable` and `FocusZone` Rust structs (field already on them from card `01KNQXW7HH...` — confirm)
- [ ] Implement `check_override` returning `Option<Option<Moniker>>`
- [ ] Update `navigate` to invoke `check_override` as rule 0
- [ ] Delete `ClaimPredicate` type, predicate registry, broadcast walk
- [ ] Confirm `useRestoreFocus` removed

## Acceptance Criteria
- [ ] `navOverride` typed as `Partial<Record<Direction, Moniker | null>>` on TS side — no raw strings
- [ ] `overrides: HashMap<Direction, Option<Moniker>>` on Rust types — no `Option<String>`
- [ ] Override check is O(1) on the focused entry; no broadcast walk
- [ ] Same-layer override target → returns that moniker
- [ ] Cross-layer override target → override ignored; beam search runs
- [ ] `null` override → `navigate` returns `None` (blocked)
- [ ] Missing direction key → spatial nav default
- [ ] `ClaimPredicate` type deleted; predicate registry deleted
- [ ] `useRestoreFocus` gone
- [ ] `cargo test` and `pnpm vitest run` pass

## Tests
- [ ] Rust: override with same-layer target returns `Some(Some(Moniker(...)))`
- [ ] Rust: override with `None` value returns `Some(None)` (block)
- [ ] Rust: override with cross-layer moniker returns outer `None` (fall through)
- [ ] Rust: `navigate` with no override delegates to beam search
- [ ] React: `navOverride` typed; TS rejects a `string` where `Moniker` expected at call sites (compile-time check)
- [ ] Full spatial nav works without any `navOverride` in production code
- [ ] Run `cargo test -p swissarmyhammer-kanban` and `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.