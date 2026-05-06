---
assignees:
- claude-code
position_column: todo
position_ordinal: e680
project: spatial-nav
title: 'stateless: lock the contract — NavSnapshot, FocusOp, FocusState (types only)'
---
## Why this is card 2

Foundational. The prior attempt fell apart partly because data shapes drifted across 14 sub-tasks. This card freezes the wire shape and the operation enum BEFORE any implementation. Subsequent cards reference these types.

## What to define (no implementation)

### Rust — `swissarmyhammer-focus/src/stateless/types.rs` (new module)

```rust
pub struct NavSnapshot {
    pub layer_fq: FullyQualifiedMoniker,
    pub scopes: Vec<SnapshotScope>,
}

pub struct SnapshotScope {
    pub fq: FullyQualifiedMoniker,
    pub rect: Rect,
    pub parent_zone: Option<FullyQualifiedMoniker>,
    pub overrides: HashMap<Direction, Option<FullyQualifiedMoniker>>,
}

pub enum FocusOp {
    Cardinal { dir: Direction },
    EdgeFirst,
    EdgeLast,
    DrillIn,
    DrillOut,
    Click { fq: FullyQualifiedMoniker },
    FocusLost { lost: FullyQualifiedMoniker, lost_parent_zone: Option<FullyQualifiedMoniker>, lost_layer: FullyQualifiedMoniker },
    ClearFocus,
    PushLayer { fq: FullyQualifiedMoniker, allow_pierce_below: bool },
    PopLayer { fq: FullyQualifiedMoniker },
}

pub struct FocusState {
    pub focus_by_window: HashMap<WindowLabel, FullyQualifiedMoniker>,
    pub layers: HashMap<FullyQualifiedMoniker, FocusLayer>,
    pub last_focused_by_fq: HashMap<FullyQualifiedMoniker, FullyQualifiedMoniker>,
}

pub struct FocusDecision {
    pub next: FocusState,
    pub event: Option<FocusChangedEvent>,
}

pub fn decide(
    state: &FocusState,
    op: &FocusOp,
    snapshot: &NavSnapshot,
    window: &WindowLabel,
) -> FocusDecision { unimplemented!() }
```

`#[serde]` round-trips. `Hash`, `PartialEq`, `Clone`, `Debug` where reasonable. Re-export from crate root.

### TypeScript — wire-shape mirrors

`kanban-app/ui/src/lib/stateless-focus-types.ts` (new file): TS types that match the Rust serde output. Tests: serialise a Rust `NavSnapshot` and deserialise as TS, and vice versa.

### IPC surface (signatures only, not yet implemented)

`spatial_decide(window, op, snapshot) -> FocusDecisionEvent` — single Tauri command replacing the per-op family. Old commands stay during transition, will be removed in card 5.

## Out of scope

- `decide()` body (card 3)
- Replacing existing IPCs (card 5)
- React-side migration (card 4)

## Acceptance

- All types compile, serde round-trip in unit tests
- TS types align with Rust shapes (a small JSON roundtrip test)
- `cargo test -p swissarmyhammer-focus` and `pnpm -C kanban-app/ui test` green
- No existing kernel code is touched
#stateless-rebuild