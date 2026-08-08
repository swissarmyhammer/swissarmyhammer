---
assignees:
- claude-code
position_column: todo
position_ordinal: ff9c80
title: 'mirdan: unify the two YAML frontmatter parsers in list.rs and mcp_config.rs'
---
The review engine reports this on `crates/mirdan/src/list.rs`, quoted word for word:

> `crates/mirdan/src/list.rs:518` — parse_frontmatter reimplements frontmatter parsing logic that already exists as parse_yaml_frontmatter in the same crate (mcp_config.rs:202). Both functions parse YAML delimited by --- markers; this logic should be unified instead of duplicated. Have parse_frontmatter call mcp_config::parse_yaml_frontmatter and convert Result to Option, or extract the core parsing into a shared helper that both callers use with their preferred error handling.

## Why this is a separate card

This is pre-existing duplication, not new. Both functions last changed in commit 375d20b16 on 2026-05-15, months before ^qh5fnpd opened. It surfaced during ^qh5fnpd only because that card's verification used whole-file review (`review file`), which reads the whole file, and not the diff scope the review step used.

^qh5fnpd was scoped to one finding — moving `merge_targets` to a shared module — and was told not to refactor anything that finding did not name. So this one is filed here instead.

## Subtasks

- [ ] Read both functions: `parse_frontmatter` in `crates/mirdan/src/list.rs` and `parse_yaml_frontmatter` in `crates/mirdan/src/mcp_config.rs`.
- [ ] Decide which keeps the canonical body. The two differ in error handling — one returns `Option`, the other returns `Result` — so the shared helper must serve both without forcing either caller to change its contract.
- [ ] Route both call sites through the one implementation.
- [ ] Run `cargo nextest run -p mirdan`, `cargo fmt`, and `cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] Verify with `{"op": "review file", "path": "crates/mirdan/src/list.rs"}` that the duplication row is gone. #mirdan