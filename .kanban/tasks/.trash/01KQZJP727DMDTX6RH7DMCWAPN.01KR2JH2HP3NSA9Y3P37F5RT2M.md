---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: f580
project: spatial-nav
title: 'stateless: card 5 — make swissarmyhammer-focus a pure stateless library (delete legacy state, registry, strategy, observer)'
---
## Why this is card 5

After card 4 lands, every production call site routes through `spatial_decide(snapshot)` and the old per-op IPCs are unreferenced by production code (still defined, still compiled, still tested). This card finishes the rebuild by **deleting every piece of state-bearing code from the `swissarmyhammer-focus` crate** so the crate becomes a pure stateless library.

**End-state contract:** `swissarmyhammer-focus` exports exactly two surface kinds:

1. **The decision function** — `swissarmyhammer_focus::stateless::decide(state, op, snapshot, window) -> FocusDecision`.
2. **Pure data shapes** — `NavSnapshot`, `SnapshotScope`, `FocusOp`, `FocusState`, `FocusDecision`, `FocusChangedEvent`, `FocusScope`, `FocusLayer`, `FocusOverrides`, `Direction`, `Rect`, `Pixels`, `FullyQualifiedMoniker`, `SegmentMoniker`, `LayerName`, `WindowLabel`. All `Serialize` + `Deserialize`. Zero behavior beyond what `derive` gives them.

Anything else — registries, strategies, observers, mutable state, traits — is gone. The consumer (`kanban-app`) holds the only `FocusState` instance, builds a `NavSnapshot` per call from `LayerScopeRegistry`, and calls `decide()`. There is no other path.

This card produces a `-N lines / +small lines` diff. There is no behavior change beyond what card 4 already shipped.

## What to delete

### A. Per-op Tauri commands (kanban-app side)

`kanban-app/src/commands.rs` — delete the 15 spatial command definitions and their inner helpers (lines ~2231–2693 today):

| Symbol | Line (HEAD) |
|---|---|
| `spatial_register_scope_inner` | 2231 |
| `spatial_unregister_scope_inner` | 2258 |
| `spatial_register_batch_inner` | 2277 |
| `spatial_push_layer_inner` | 2286 |
| `spatial_register_scope` | 2323 |
| `spatial_register_batch` | 2364 |
| `spatial_unregister_scope` | 2392 |
| `spatial_update_rect` | 2417 |
| `spatial_focus` | 2439 |
| `spatial_clear_focus` | 2475 |
| `spatial_navigate` | 2508 |
| `spatial_push_layer` | 2591 |
| `spatial_pop_layer` | 2617 |
| `spatial_drill_in` | 2653 |
| `spatial_drill_out` | 2693 |

`kanban-app/src/main.rs` — delete the matching registrations (lines 76–86 today; keep only `commands::spatial_decide` from card 4).

### B. React shims

`kanban-app/ui/src/lib/spatial-focus-context.tsx` — delete every per-op shim method on `SpatialFocusActions` (`actions.navigate`, `actions.drillIn`, `actions.drillOut`, `actions.focus`, `actions.clearFocus`, `actions.pushLayer`, `actions.popLayer`). After this card the only surface is `actions.decide(op)`.

`kanban-app/ui/src/lib/scroll-on-edge.ts` — update if it calls any deleted shim instead of `actions.decide`.

`kanban-app/ui/src/components/app-shell.tsx::buildNavCommands` and `buildDrillCommands` — already migrated by card 4; remove any remaining feature-flag branches or transitional comments.

### C. Whole-module deletions in `swissarmyhammer-focus`

Delete these files outright. Their entire contents are state-bearing, strategy-pluggable, or observer-pattern surfaces that the stateless `decide()` replaces:

| File | Reason |
|---|---|
| `swissarmyhammer-focus/src/state.rs` | `SpatialState` mutable focus tracker + `set_focus` mutator + `LostFocusContext` + `FallbackResolution` — `FocusState` from `stateless::types` supersedes. `FocusChangedEvent` moves to `stateless::types` (per card 2). |
| `swissarmyhammer-focus/src/navigate.rs` | `NavStrategy` trait + `BeamNavStrategy` + `pick_target_via_view` + `NavScopeView` — `decide()` covers cardinal nav directly; no pluggable strategy. |
| `swissarmyhammer-focus/src/registry.rs` | `SpatialRegistry` + `RegisterEntry` + `record_focus` mutator + `IndexedSnapshot` — replaced by `NavSnapshot` (snapshot-per-call, no long-lived registry). Pure helpers (`children_of`, `first_child_by_top_left`, `last_child_by_bottom_right`, `ancestor_zones`) move to `stateless/helpers.rs` and take `&NavSnapshot` instead of `&SpatialRegistry`. |
| `swissarmyhammer-focus/src/observer.rs` | `FocusEventSink` trait + `NoopSink` + `RecordingSink` — push-based event delivery for adapters. The stateless API returns `FocusDecision { event: Option<FocusChangedEvent> }`; consumers pull instead of push. No alternative observer pattern. |
| `swissarmyhammer-focus/src/snapshot.rs` | `IndexedSnapshot` + the legacy snapshot indexer that wraps a registry. The new `NavSnapshot` lives under `stateless/types.rs` (per card 2). |

### D. Field deletions on surviving structs

`swissarmyhammer-focus/src/scope.rs::FocusScope` — delete the `last_focused: Option<SegmentMoniker>` field. The new path stores per-FQ remembered focus in `FocusState::last_focused_by_fq`, not on the scope struct.

`swissarmyhammer-focus/src/layer.rs::FocusLayer` — delete the `last_focused: Option<FullyQualifiedMoniker>` field for the same reason. (If this field doesn't exist today, this bullet is a no-op; verify on read.)

After both deletions, `FocusScope` and `FocusLayer` are pure shape structs containing only `fq`, `rect`, `parent_zone`, `overrides`, `layer_fq` (scope) and `fq`, `parent` (layer). They are snapshot elements, not state.

### E. `lib.rs` module + re-export pruning

`swissarmyhammer-focus/src/lib.rs` after this card declares only:

```rust
pub mod layer;     // FocusLayer struct
pub mod scope;     // FocusScope struct
pub mod stateless; // decide(), types, helpers
pub mod types;     // newtypes (FullyQualifiedMoniker, etc.)

pub use layer::FocusLayer;
pub use scope::{FocusOverrides, FocusScope};
pub use stateless::{
    decide,
    types::{FocusChangedEvent, FocusDecision, FocusOp, FocusState, NavSnapshot, SnapshotScope},
};
pub use types::{Direction, FullyQualifiedMoniker, LayerName, Pixels, Rect, SegmentMoniker, WindowLabel};
```

Delete the `pub mod navigate;`, `pub mod observer;`, `pub mod registry;`, `pub mod snapshot;`, `pub mod state;` lines and their re-exports. Update the crate-level doc comment to drop the `# Modules` paragraph and to describe the crate as a stateless decision library.

### F. `AppState` field deletion

`kanban-app/src/state.rs` — delete the `AppState::spatial: Mutex<SpatialState>` and `AppState::registry: Mutex<SpatialRegistry>` fields. The new `AppState::focus_state: Mutex<FocusState>` field added by card 4 is the only stateful spatial slot left, and lives in the **consumer crate**, not in `swissarmyhammer-focus`.

### G. Tests against deleted symbols

- `kanban-app/tests/*.rs` — delete any test calling a deleted command directly. The replacement is `kanban-app/tests/spatial_decide_integration.rs` from card 4.
- `swissarmyhammer-focus/src/navigate.rs::tests` — entire `tests` mod migrates to `stateless/decide.rs::tests` per card 3 and is deleted with `navigate.rs`.
- `swissarmyhammer-focus/src/registry.rs::tests` — keep tests for the surviving pure helpers (move them with the helpers to `stateless/helpers.rs::tests`); delete tests that exercise `record_focus` or per-scope `last_focused`.
- `swissarmyhammer-focus/src/state.rs::tests` — delete with the module.
- `swissarmyhammer-focus/src/observer.rs::tests` — delete with the module.
- `kanban-app/ui/src/test/spatial-shadow-registry.ts` — drop the per-op handlers; keep only the `spatial_decide` handler from card 4.
- React tests that mock `actions.navigate` etc. — migrate to `actions.decide` mocks (or delete if redundant with the eight motion-validation suites).

### H. Cargo.toml description

`swissarmyhammer-focus/Cargo.toml` — update `description`:

```toml
description = "Stateless spatial focus and keyboard navigation kernel — pure decision function over a per-call snapshot"
```

(Today: `"Spatial focus and keyboard navigation engine — generic, no domain dependencies"` — keep the *generic* idea, but lead with *stateless*.)

## What stays

- `swissarmyhammer-focus/src/scope.rs` — `FocusScope` struct (snapshot element shape) **with `last_focused` field deleted per section D**.
- `swissarmyhammer-focus/src/layer.rs` — `FocusLayer` struct **with `last_focused` field deleted per section D**.
- `swissarmyhammer-focus/src/types.rs` — newtypes + `Direction` + `Rect`. Untouched.
- `swissarmyhammer-focus/src/stateless/*` — the new home for everything decision-related.

## Out of scope

- Algorithm changes — kernel semantics are frozen by the time this card starts; cards 1 + 3 own them.
- React-side dispatch migration — already complete after card 4.
- The README rewrite itself — card `01KQZF3KW7QGRR8VN5SB6F5RAF` runs after this one.

## Acceptance Criteria

- [ ] `cargo nextest run -p swissarmyhammer-focus -p kanban-app` green.
- [ ] `cd kanban-app/ui && bun test` green; the eight motion-validation suites + `spatial-nav-end-to-end.spatial.test.tsx` all pass.
- [ ] `swissarmyhammer-focus/src/{state,navigate,registry,observer,snapshot}.rs` files **do not exist** on disk.
- [ ] `swissarmyhammer-focus/src/lib.rs` declares only `mod layer; mod scope; mod stateless; mod types;` (asserted by `tests/lib_surface.rs`).
- [ ] Public re-exports listed in `lib.rs` exactly match the allowlist in section E (asserted by `tests/lib_surface.rs`).
- [ ] `grep -rn "BeamNavStrategy\|NavStrategy\b\|SpatialRegistry\|SpatialState\|FocusEventSink\|RecordingSink\|NoopSink\|IndexedSnapshot\|RegisterEntry\|LostFocusContext\|FallbackResolution\|record_focus" swissarmyhammer-focus/ kanban-app/` returns **zero matches**.
- [ ] `grep -rn "spatial_navigate\|spatial_drill_in\|spatial_drill_out\|spatial_focus\b\|spatial_register_scope\|spatial_register_batch\|spatial_unregister_scope\|spatial_update_rect\|spatial_clear_focus\|spatial_push_layer\|spatial_pop_layer" --include='*.rs' --include='*.ts' --include='*.tsx'` returns **zero matches** outside test fixtures explicitly testing the absence.
- [ ] `grep -rn "actions\.\(navigate\|drillIn\|drillOut\|focus\|clearFocus\|pushLayer\|popLayer\)\(" --include='*.tsx' --include='*.ts'` returns **zero matches** outside test fixtures.
- [ ] `swissarmyhammer-focus` crate has zero `Mutex`/`RwLock`/`OnceCell`/`Lazy`/`static mut`/`RefCell`/`Cell`/`parking_lot::`/`tokio::sync::` usages anywhere in `src/` (asserted by extending `tests/stateless_is_pure.rs` from card 3 to walk the **whole crate**, not just `src/stateless/`).
- [ ] `swissarmyhammer-focus/Cargo.toml` description leads with the word "Stateless".
- [ ] Diff is net-negative: `git diff --shortstat <pre-card-5> HEAD` shows more deletions than insertions across `swissarmyhammer-focus/` and `kanban-app/`.

## Tests

- [ ] Existing motion-validation suites unchanged in assertions — they still target `spatial_decide` from card 4.
- [ ] New `swissarmyhammer-focus/tests/lib_surface.rs`: parses `src/lib.rs` (via `include_str!`) and asserts the set of `pub mod` and `pub use` lines exactly equals the allowlist from section E. Any new `pub mod foo;` introduced later requires a deliberate update to this allowlist — drift is loud.
- [ ] Extend `swissarmyhammer-focus/tests/stateless_is_pure.rs` (introduced by card 3) to walk **all** of `swissarmyhammer-focus/src/**/*.rs`, not just `src/stateless/`. The crate is pure end-to-end.
- [ ] New `swissarmyhammer-focus/tests/no_legacy_symbols.rs`: a `use` block listing only the surviving public symbols. The test compiles ⇔ those symbols exist; any future re-introduction of `BeamNavStrategy`, `SpatialRegistry`, `SpatialState`, etc. fails this test by name shadowing.
- [ ] Test command: `cargo nextest run -p swissarmyhammer-focus -p kanban-app && cd kanban-app/ui && bun test` — all green.

## Workflow

- Use `/tdd` — write `tests/lib_surface.rs` and the extended `tests/stateless_is_pure.rs` first; let them fail because the deleted modules still exist; then walk sections A–H deleting until they pass. Re-run the eight motion-validation suites + the end-to-end test after each section to confirm zero behavior regression.

#stateless-rebuild #stateless-nav