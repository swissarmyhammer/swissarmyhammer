---
assignees:
- claude-code
depends_on:
- 01KQZJKQDWY3ZFMVW9GEH2VQ4C
position_column: todo
position_ordinal: f580
project: spatial-nav
title: 'stateless: card 5 — delete the old stateful kernel (per-op IPCs, SpatialRegistry state, BeamNavStrategy, React shims)'
---
## Why this is card 5

After card 4 lands, every production call site routes through `spatial_decide(snapshot)` and the old per-op IPCs are unreferenced by production code (still defined, still compiled, still tested). This card is the demolition: remove the dead state-bearing surface so the kernel has only one path. The motion-validation suites and the umbrella end-to-end test are the green-light gate — every deletion must keep them green.

This card produces a `-N lines / +0 lines` diff. There is no behavior change beyond what card 4 already shipped.

## What to delete

### A. Per-op Tauri commands

`kanban-app/src/commands.rs` — delete the 11 spatial command definitions and their inner helpers (lines ~2231–2693 today):

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

`kanban-app/src/main.rs` — delete the matching registrations (lines 76–86 today, keep only `commands::spatial_decide` from card 4).

The R/W lock around the registry that these commands shared (`AppState::spatial`) becomes dead — deletion follows in section C.

### B. React shims

`kanban-app/ui/src/lib/spatial-focus-context.tsx` — delete the per-op shim methods on `SpatialFocusActions`:

- `actions.navigate`
- `actions.drillIn`
- `actions.drillOut`
- `actions.focus` (replaced by `actions.click` / `decide({ Click })`)
- `actions.clearFocus` (replaced by `decide({ ClearFocus })`)
- `actions.pushLayer` / `actions.popLayer` (replaced by `decide({ PushLayer / PopLayer })`)

Plus the helpers in `kanban-app/ui/src/lib/scroll-on-edge.ts` if they reference the deleted shims rather than `actions.decide`.

`kanban-app/ui/src/components/app-shell.tsx::buildNavCommands` and `buildDrillCommands` — already migrated by card 4; this card removes any remaining feature-flag branches or transitional comments.

### C. `SpatialRegistry` state surface

`swissarmyhammer-focus/src/registry.rs`:

- Delete the `last_focused: Option<SegmentMoniker>` field on `FocusScope` (the new path stores `last_focused_by_fq` in `FocusState`).
- Delete `SpatialRegistry::record_focus` and any helpers that mutate per-scope state.
- Keep `children_of`, `ancestor_zones`, `first_child_by_top_left`, `last_child_by_bottom_right` — these are pure helpers that `decide()` reuses.
- Decide whether `SpatialRegistry` itself is still needed: if `decide()` reads only from `NavSnapshot`, the registry as a *runtime store* dies and only its *type aliases* (the `FocusScope` struct, `FocusLayer` struct) survive as snapshot-element shapes.

`swissarmyhammer-focus/src/state.rs` — delete `SpatialState` and `SpatialState::set_focus` if `FocusState` from the stateless module supersedes both. The `FocusChangedEvent` shape stays; check whether it moved to `stateless/types.rs` per card 2 — if so, delete the duplicate definition here.

`swissarmyhammer-focus/src/navigate.rs` — delete `BeamNavStrategy` and the `NavStrategy` trait once `decide()` covers cardinal nav. The score helpers (`score_candidate`, `pick_best_candidate`, half-plane filters) move *into* `stateless/decide.rs` (or a private helpers module) and are deleted from `navigate.rs`. Goal: `navigate.rs` becomes empty / removed; `mod navigate` line in `lib.rs` deleted.

### D. `AppState` field

`kanban-app/src/state.rs` — delete the `AppState::spatial: tokio::sync::Mutex<SpatialState>` field and the `AppState::registry: tokio::sync::Mutex<SpatialRegistry>` field if they are no longer constructed. The new `AppState::focus_state: tokio::sync::Mutex<FocusState>` field added by card 4 is the only stateful spatial slot.

### E. Tests against deleted symbols

Delete or migrate any test that names a deleted symbol:

- `kanban-app/tests/*.rs` — any test that calls `spatial_navigate` / `spatial_drill_in` / etc. directly. Most are covered by `spatial_decide_integration.rs` (card 4); delete the originals.
- `swissarmyhammer-focus/src/navigate.rs::tests` — the entire `tests` mod inside `navigate.rs` migrates to `stateless/decide.rs::tests` per card 3 and is deleted here once that migration is verified.
- `swissarmyhammer-focus/src/registry.rs::tests` — keep tests for the surviving pure helpers; delete tests that exercise `record_focus` or per-scope `last_focused` reads/writes.
- `kanban-app/ui/src/test/spatial-shadow-registry.ts` — drop the per-op handlers; keep only the `spatial_decide` handler from card 4.
- React tests that mock `actions.navigate` etc. — migrate to mock `actions.decide` (or delete if redundant with the motion-validation suites).

### F. README + module docs

`swissarmyhammer-focus/README.md` — card `01KQZF3KW7QGRR8VN5SB6F5RAF` already plans this rewrite; this card is its prerequisite. Verify after the deletions that no surviving symbol in the README points at a deleted thing (e.g., `BeamNavStrategy` or `SpatialState`).

`swissarmyhammer-focus/src/navigate.rs` module doc-comment, `swissarmyhammer-focus/src/state.rs` doc-comment — delete with the modules.

## What stays

- `swissarmyhammer-focus/src/scope.rs` — `FocusScope` struct (the snapshot element shape).
- `swissarmyhammer-focus/src/layer.rs` — `FocusLayer` struct.
- `swissarmyhammer-focus/src/types.rs` — `FullyQualifiedMoniker`, `SegmentMoniker`, `Rect`, `Direction`, `WindowLabel`.
- `swissarmyhammer-focus/src/stateless/*` — the new home for the kernel.
- `swissarmyhammer-focus/src/registry.rs::children_of` and the topmost-leftmost / bottommost-rightmost helpers — these are pure functions over a snapshot once `decide()` consumes them via the snapshot accessor, but in card 3 they may still take `&SpatialRegistry`. Decide whether they relocate to a `helpers.rs` module or stay; keep the import surface stable.

## Out of scope

- Algorithm changes — kernel semantics are frozen by the time this card starts; card 1 + card 3 own them.
- React-side migration — already complete after card 4.
- The README rewrite itself — card `01KQZF3KW7QGRR8VN5SB6F5RAF` runs after this one.

## Acceptance Criteria

- [ ] `cargo nextest run -p swissarmyhammer-focus -p kanban-app` green.
- [ ] `cd kanban-app/ui && bun test` green; the eight motion-validation suites + `spatial-nav-end-to-end.spatial.test.tsx` all pass.
- [ ] `grep -rn "spatial_navigate\|spatial_drill_in\|spatial_drill_out\|spatial_focus\b\|spatial_register_scope\|spatial_register_batch\|spatial_unregister_scope\|spatial_update_rect\|spatial_clear_focus\|spatial_push_layer\|spatial_pop_layer" --include='*.rs' --include='*.ts' --include='*.tsx'` returns **zero matches** outside test fixtures explicitly testing the absence.
- [ ] `grep -rn "BeamNavStrategy\|NavStrategy\b\|SpatialRegistry::record_focus\|FocusScope.*last_focused\b" --include='*.rs'` returns **zero matches**.
- [ ] `grep -rn "actions\.\(navigate\|drillIn\|drillOut\|focus\|clearFocus\|pushLayer\|popLayer\)\(" --include='*.tsx' --include='*.ts'` returns **zero matches** outside test fixtures.
- [ ] `swissarmyhammer-focus/src/navigate.rs` deleted (or reduced to a re-export shim flagged for next-release removal — prefer outright deletion).
- [ ] `swissarmyhammer-focus/src/state.rs` deleted (or reduced to type aliases re-exporting from `stateless/types.rs`).
- [ ] `swissarmyhammer-focus/src/lib.rs` no longer declares `mod navigate;` or `mod state;`.
- [ ] Diff size is net-negative: `git diff --shortstat <pre-card-5> HEAD` shows more deletions than insertions.

## Tests

- [ ] Existing motion-validation suites unchanged in assertions — they still target `spatial_decide` from card 4.
- [ ] New regression test `swissarmyhammer-focus/tests/no_legacy_symbols.rs`: a compile-time test that fails the build if any deleted symbol is re-introduced. Implementation: a `use` block listing only the surviving public symbols (`stateless::decide`, `stateless::FocusOp`, etc.); the test compiles ⇔ those symbols exist and the file does not import any of the deleted names.
- [ ] Test command: `cargo nextest run -p swissarmyhammer-focus -p kanban-app && cd kanban-app/ui && bun test` — all green.

## Workflow

- Use `/tdd` — write the grep-based negative assertions as a test in `swissarmyhammer-focus/tests/no_legacy_symbols.rs` first; let it fail because the symbols still exist; then walk sections A–F deleting until it passes. Re-run the motion-validation + end-to-end suite after each section to confirm no behavior regression.

#stateless-rebuild