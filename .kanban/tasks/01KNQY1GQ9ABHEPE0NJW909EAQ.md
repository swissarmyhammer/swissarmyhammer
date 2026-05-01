---
assignees:
- claude-code
depends_on:
- 01KNQXZ81QBSS1M9WFD7VQJNAJ
- 01KNQXZZ9VQBHFX091P0K4F4YC
- 01KNQY0P9J03T24FSM8AVPFPZ9
- 01KQ20MX70NFN2ZVM2YN0A4KQ0
- 01KQ20NMRQQSXVRHP4RHE56B0K
- 01KQ20Q2PNNR9VMES60QQSVXTS
- 01KQ20QW3KF0SMV98ZB8859PTM
- 01KPZS32YN7CRNM0TH7GR28M86
- 01KQ2E7RPBPJ8T8KZX39N2SZ0A
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffb680
project: spatial-nav
title: 'navOverride cleanup: layer-scoped, Option&lt;Moniker&gt; overrides, remove predicate code'
---
## What

After all views migrate to spatial navigation, clean up the predicate infrastructure and replace broadcast-based `claimWhen` with a simpler, targeted override model. Overrides are layer-scoped and use newtypes throughout — `Option<Moniker>` on the Rust side, branded `Moniker | null` on the TS side.

### Crate placement

Per the commit-`b81336d42` refactor pattern, the Rust side lives in `swissarmyhammer-focus/src/`:
- `overrides` field already on `Focusable` / `FocusZone` in `focus/scope.rs` (card `01KNQXW7HH...`)
- `check_override` implemented in `focus/navigate.rs` as rule-0 of the beam search (card `01KNQXXF5W...`)
- No new Tauri commands needed — overrides travel with `spatial_register_focusable` / `spatial_register_zone`
- Tests in `swissarmyhammer-focus/tests/overrides.rs`

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
- [x] Replace `claimWhen: ClaimPredicate[]` with `navOverride: NavOverride` on `Focusable`/`FocusZone` React primitives and `FocusScope` wrapper
- [x] Wire `overrides: HashMap<Direction, Option<Moniker>>` into `Focusable` and `FocusZone` Rust structs (field already on them from card `01KNQXW7HH...` — confirm)
- [x] Implement `check_override` returning `Option<Option<Moniker>>`
- [x] Update `navigate` to invoke `check_override` as rule 0
- [x] Delete `ClaimPredicate` type, predicate registry, broadcast walk
- [x] Confirm `useRestoreFocus` removed

## Acceptance Criteria
- [x] `navOverride` typed as `Partial<Record<Direction, Moniker | null>>` on TS side — no raw strings
- [x] `overrides: HashMap<Direction, Option<Moniker>>` on Rust types — no `Option<String>`
- [x] Override check is O(1) on the focused entry; no broadcast walk
- [x] Same-layer override target → returns that moniker
- [x] Cross-layer override target → override ignored; beam search runs
- [x] `null` override → `navigate` returns `None` (blocked)
- [x] Missing direction key → spatial nav default
- [x] `ClaimPredicate` type deleted; predicate registry deleted
- [x] `useRestoreFocus` gone
- [x] `cargo test` and `pnpm vitest run` pass

## Tests
- [x] Rust: override with same-layer target returns `Some(Some(Moniker(...)))`
- [x] Rust: override with `None` value returns `Some(None)` (block)
- [x] Rust: override with cross-layer moniker returns outer `None` (fall through)
- [x] Rust: `navigate` with no override delegates to beam search
- [x] React: `navOverride` typed; TS rejects a `string` where `Moniker` expected at call sites (compile-time check)
- [x] Full spatial nav works without any `navOverride` in production code
- [x] Run `cargo test -p swissarmyhammer-kanban` and `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Review Findings (2026-04-26 12:36)

### Nits
- [ ] `kanban-app/ui/src/hooks/use-board-nav.ts` — Docstring on `useBoardNav` says "Navigation is now fully pull-based via claimWhen predicates on each card and column header FocusScope." This contradicts the new reality after the predicate registry was deleted in this task. Update the docstring to describe the post-cleanup model: nav is driven by the Rust spatial-nav kernel via `useSpatialFocusActions().navigate`; this hook now only manages edit-mode state.
- [ ] `kanban-app/ui/src/hooks/use-grid.ts` — Two docstrings reference the deleted `claimWhen` predicate model: the `cursor` field comment ("navigation is pull-based via claimWhen") and the `useGrid` function comment ("that is handled by claimWhen predicates on each cell's FocusScope"). Both should be updated to describe spatial nav as the source of cursor movement and clarify that this hook only owns mode + visual selection state.
- [ ] `kanban-app/ui/src/components/inspector-focus-bridge.tsx` — File-level docstring says "vim/arrow/tab keys still broadcast nav commands (nav.up, nav.down, nav.first, nav.last) via `broadcastNavCommand`, but each field row is now a `<FocusScope kind="zone">` ...". This is now misleading — `broadcastNavCommand` is a no-op stub per the cleanup; the inspector's `nav.*` command handlers are dead branches that exist only to keep keymaps registered. Either update the docstring to call out the migration state explicitly, or follow up by rewiring the inspector's nav commands to `useSpatialFocusActions().navigate` so the documented behavior is restored.
