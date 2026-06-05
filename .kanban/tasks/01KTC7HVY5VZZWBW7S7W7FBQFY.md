---
assignees:
- claude-code
depends_on:
- 01KTC7HAF68EAYYV9ZMEB0NGX2
position_column: todo
position_ordinal: '8380'
project: remove-prompts
title: Delete builtin/prompts and stop generating builtin_prompts.rs
---
## What
Delete the builtin prompt markdown and stop the build script from embedding them. Preserve the shared `builtin/_partials/` generation (used by skills/agents) — only the prompts source goes away.

Files to delete:
- `builtin/prompts/` directory (9 files: `are_rules_passing.md`, `example.md`, `read_changes.md`, `say-hello.md`, `scaffold-prompt.md`, plus `docs/`).

Files to edit in `crates/swissarmyhammer-prompts/build.rs`:
- Remove the first `BuiltinGenerator::new("prompts").source_dir("../../builtin/prompts")...generate();` block.
- KEEP the second block that generates partials from `builtin/_partials/` via `get_builtin_partials` — skills/agents depend on it.

Files to edit in `crates/swissarmyhammer-prompts/src/`:
- `lib.rs` — remove `include!(concat!(env!("OUT_DIR"), "/builtin_prompts.rs"));` (~line 45).
- `prompt_resolver.rs` — remove `include!(... builtin_prompts.rs)` (~line 6), remove `load_builtin_prompts()` (~line 131) calling `get_builtin_prompts()`, and remove the call to it inside `load_all_prompts` (~line 48). KEEP the partials include (~line 9) and the `_partials` registration loop (~lines 141-143).

Verify `get_builtin_prompts` has no other callers via `grep code "get_builtin_prompts"` before removing.

## Acceptance Criteria
- [ ] `builtin/prompts/` no longer exists.
- [ ] `build.rs` no longer generates `builtin_prompts.rs`; still generates `builtin_partials.rs`.
- [ ] `get_builtin_prompts` symbol is gone; `get_builtin_partials` still exists and is called.
- [ ] `cargo build -p swissarmyhammer-prompts` succeeds.

## Tests
- [ ] Update `crates/swissarmyhammer-prompts/tests/all_skills_render_test.rs` and `skills_rendering_test.rs` to no longer assume builtin prompts exist; they MUST still pass on partial rendering.
- [ ] Add a unit test in `prompt_resolver.rs` asserting that after `load_all_prompts`, the `_partials/` entries are present (e.g. `_partials/delegate-to-subagent`).
- [ ] `cargo test -p swissarmyhammer-prompts` is green.

## Workflow
- Use `/tdd` — write the partials-still-load assertion first, then remove builtin prompt loading.