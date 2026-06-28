---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffac80
title: Cut/copy/paste shown on views & perspectives that don't support them — gate command availability through the plugin system, hide when unsupported
---
## What

Views and perspectives expose cut/copy/paste commands that are not supported for their entity type. Today they show up anyway (and presumably no-op or error). They should NOT be displayed when the focused entity type doesn't support them.

The gating must come from the **plugin system** — i.e. the command's availability/applicability for the focused entity type is declared/resolved by the plugin that owns the command (the same metadata-driven path that decides which commands apply to a scope), NOT a hardcoded React check that special-cases "view" / "perspective".

## Expected

- For a focused entity type that does not support cut/copy/paste (views, perspectives), those commands are simply absent from the command surface (palette / context menu / keybinding availability) — not shown-and-disabled, not shown-and-erroring.
- For entity types that DO support them (tasks/cards), they continue to appear and work.
- Availability is computed from plugin/command metadata for the focused scope, not branched on entity-type strings in the UI.

## Acceptance Criteria
- [x] Cut/copy/paste do not appear on the command surface when a view or perspective is focused
- [x] Cut/copy/paste still appear and work for entity types that support them (tasks/cards)
- [x] Availability is resolved from plugin/command metadata for the focused scope — no hardcoded entity-type branch in React
- [x] No regression to internal-drag (task.move) vs external-drag (paste) dispatch separation

## Tests
- [x] vitest red-first: with a view/perspective focused, the command surface (palette/menu) does NOT include cut/copy/paste
- [x] vitest: with a task/card focused, cut/copy/paste ARE present
- [x] command-metadata/applicability test proving the gate is driven by declared capability, not a UI string check
- [x] tsc + touched vitest green; relevant cargo nextest packages green if command YAML/declaration changes touch a Rust crate

## Review Findings (2026-06-13 14:53)

Overall: the fix is well-designed and metadata-driven. The findings below are a data-drift warning and two nits — all addressed in the 2026-06-13 review-fix pass.

### Warnings
- [x] **Declared ⟺ enforced drift guard.** `builtin/plugins/entity-commands/index.ts` `CLIPBOARD_ENTITY_TYPES` (list-time gate) and `crates/swissarmyhammer-kanban/src/commands/clipboard_commands.rs` `COPYABLE_ENTITY_TYPES` (dispatch-time `available()` gate) were two independent source-of-truth lists, kept in lockstep only by prose comments plus a third hand-maintained literal in the e2e. **FIXED (option a):** made `COPYABLE_ENTITY_TYPES` the canonical `pub` constant and rewrote `builtin_entity_commands_e2e::assert_clipboard_applies_to` to assert set-equality between the TS-surfaced `applies_to` (read through the real registered metadata via `list command`) and the Rust `COPYABLE_ENTITY_TYPES` directly — no third literal. The two lists can no longer diverge without a RED test. Red-green proven: dropping `actor` from the Rust constant turned the guard RED (`left` includes actor, `right` does not); restoring it → GREEN.

### Nits
- [x] **`caption.rs` INSPECTABLE_ENTITY_PREFIXES vs clipboard set divergence.** Documented the intentional divergence: `INSPECTABLE_ENTITY_PREFIXES` is deliberately a subset of `COPYABLE_ENTITY_TYPES` (omits `actor`/`project`, which are reached only via explicit context-menu target, never as bare scope-chain leaves), with an explicit note that if actor/project ever become palette-focusable their prefixes must be added.
- [x] **`list_applies_to.rs` reduced fixture.** Added a comment that the fixture's `copyable` set is a deliberately REDUCED fixture (missing actor/project), NOT the canonical production list, which is pinned against the Rust constant by `builtin_entity_commands_e2e::assert_clipboard_applies_to`.

### Verification evidence (review-fix pass, 2026-06-13)
- [x] Drift-guard red-green: mutated Rust `COPYABLE_ENTITY_TYPES` (dropped `actor`) → `assert_clipboard_applies_to` RED (`left: [actor, attachment, board, column, project, tag, task]` vs `right: [attachment, board, column, project, tag, task]`); restored → GREEN.
- [x] `cargo nextest run -p swissarmyhammer-command-service` → 155 run: 154 passed, 1 failed. The single failure is the carded pre-existing `meta_tree::meta_tree_id_param_is_required_where_expected` (unrelated; no changed file touches the unregister id-required schema).
- [x] `cargo nextest run -p swissarmyhammer-command-service --test list_applies_to` → 4/4 passed.
- [x] `cargo nextest run -p swissarmyhammer-command-service entity_commands_plugin_registers_and_executes` (loads the bundle from disk, surfaces applies_to, runs the drift guard) → 1/1 passed.
- [x] `cargo nextest run -p swissarmyhammer-kanban clipboard` → 50/50 passed.
- [x] tsc/vitest: builtin plugins have no standalone tsconfig — `index.ts` (doc-comment-only change) and the SDK are typechecked through the Rust e2e harness (`builtin_entity_commands_e2e`, green). No `.tsx`/UI files changed, so the app UIs are not recompiled (per constraints).

### Files changed (review-fix pass)
- `crates/swissarmyhammer-kanban/src/commands/clipboard_commands.rs` — `COPYABLE_ENTITY_TYPES` made `pub` + canonical-source doc.
- `crates/swissarmyhammer-command-service/tests/integration/builtin_entity_commands_e2e.rs` — `assert_clipboard_applies_to` now asserts set-equality vs the Rust `COPYABLE_ENTITY_TYPES`.
- `crates/swissarmyhammer-command-service/src/caption.rs` — documented the intentional INSPECTABLE_ENTITY_PREFIXES vs clipboard-set divergence.
- `crates/swissarmyhammer-command-service/tests/list_applies_to.rs` — comment marking the reduced fixture as non-canonical.
- `builtin/plugins/entity-commands/index.ts` — doc comment noting the lockstep is now enforced by the drift guard.