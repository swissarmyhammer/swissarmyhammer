---
assignees:
- claude-code
position_column: todo
position_ordinal: '9380'
project: cli-schema-gen
title: Shared schema-driven CLI generator in swissarmyhammer-operations
---
## What
Promote kanban-cli's private `cli_gen.rs` logic into a reusable, public API in `crates/swissarmyhammer-operations` so all four CLIs (kanban-cli, sah, code-context-cli, shelltool-cli) can build their command tree from the same schema-driven generator. `swissarmyhammer-operations` is the natural home: it already owns the `Operation` trait and `generate_mcp_schema` (`crates/swissarmyhammer-operations/src/schema.rs`), and kanban-cli already depends on it (`apps/kanban-cli/Cargo.toml:27`).

Create a new module `crates/swissarmyhammer-operations/src/cli_gen.rs` (add `pub mod cli_gen;` to `crates/swissarmyhammer-operations/src/lib.rs`) by moving these from `apps/kanban-cli/src/cli_gen.rs` verbatim, generalized to be tool-agnostic:
- `build_commands_from_schema(schema: &Value) -> Vec<clap::Command>` (kanban-cli `cli_gen.rs:58`) — reads `x-operation-schemas` for per-op precise args, falls back to global `properties`, groups by noun, builds noun→verb→args clap tree.
- `extract_noun_verb_arguments(matches, schema) -> Result<Map, String>` (kanban-cli `cli_gen.rs:320`) — navigates noun→verb matches back into a `{ "op": "verb noun", ...args }` JSON object.
- Supporting helpers: `precompute_args`, `build_clap_arg`, `ArgMeta`/`ArgMetaType`, `build_noun_command`, `build_verb_command`, `build_arguments_from_matches`, `extract_value_from_matches`, the `intern`/`STRING_CACHE` interner, and `schema_has_type`/`primary_type`.

This card moves the engine only; it does NOT yet migrate any CLI (kanban-cli keeps its copy compiling until card E swaps it). Keep the public surface minimal: `build_commands_from_schema` + `extract_noun_verb_arguments` are the two entry points each CLI needs.

Add a `clap` dependency to `crates/swissarmyhammer-operations/Cargo.toml` (the generator returns `clap::Command`). Verify this does not create a cycle — `swissarmyhammer-operations` is a leaf-ish crate; confirm `clap` is a workspace dep and adding it here is acceptable.

Note: this card intentionally moves the EXISTING full-schema reader. It does not depend on card A (wire shape) — it reads `x-operation-schemas` from whatever full schema it is handed.

## Acceptance Criteria
- [ ] `swissarmyhammer_operations::cli_gen::build_commands_from_schema` and `::extract_noun_verb_arguments` are public and documented.
- [ ] Given a tool's full schema (with `x-operation-schemas`), the generator produces a noun→verb clap tree with per-op-scoped args (required flags correct per op, not the global union).
- [ ] `cargo build -p swissarmyhammer-operations` succeeds; no dependency cycle introduced.

## Tests
- [ ] Port kanban-cli's `cli_gen` unit tests (`apps/kanban-cli/src/cli_gen.rs:411+` — `build_commands_produces_noun_verb_tree`, `board_noun_has_expected_verbs`, `board_init_has_scoped_args`, and the remaining arg-extraction tests) into `crates/swissarmyhammer-operations/src/cli_gen.rs` `#[cfg(test)]`. Since operations can't depend on kanban, drive the tests with a small in-crate mock `Operation` set fed through `generate_mcp_schema` (reuse the existing mock pattern in `schema.rs:260+`), asserting noun/verb structure and per-op required-arg scoping.
- [ ] `cargo nextest run -p swissarmyhammer-operations cli_gen` passes.

## Workflow
- Use `/tdd` — port the failing tests first (they fail until the module exists), then move the implementation to make them pass.