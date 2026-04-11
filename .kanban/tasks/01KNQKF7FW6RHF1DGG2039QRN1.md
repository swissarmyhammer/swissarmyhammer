---
assignees:
- claude-code
depends_on:
- 01KNMJYX1RR52N3AS3M2MD5A8D
position_column: done
position_ordinal: ffffffffffffffffffffff9b80
title: 'Add test: every YAML-defined command must have a registered Rust implementation'
---
## What

Add a test that cross-references YAML command definitions against the Rust command registration map, ensuring every YAML-defined command ID has a corresponding `Arc<dyn Command>` registered. This prevents the `perspective.rename` class of bug — a command defined in YAML but with no backend implementation, silently failing when the frontend dispatches it.

### Approach

In `swissarmyhammer-kanban/src/commands/mod.rs`, add a test that:
1. Loads all builtin YAML sources via `swissarmyhammer_commands::registry::builtin_yaml_sources()`
2. Parses each YAML to extract command IDs (deserialize as `Vec<CommandDef>`, read `.id` field)
3. Calls `register_commands()` to get the Rust implementation map
4. For each YAML command ID, asserts that the Rust map contains an entry

Commands that are intentionally handled client-side (if any) can be listed in an explicit allowlist with a comment explaining why.

### Files to modify

- **`swissarmyhammer-kanban/src/commands/mod.rs`** — Add `test_all_yaml_commands_have_rust_implementations`

### Reference

- `builtin_yaml_sources()` at `swissarmyhammer-commands/src/registry.rs` — returns `Vec<(&str, &str)>` of (name, yaml_content)
- `CommandDef` struct in `swissarmyhammer-commands` — has an `id: String` field
- `register_commands()` at `swissarmyhammer-kanban/src/commands/mod.rs` — returns `HashMap<String, Arc<dyn Command>>`
- Current YAML command count: ~63 IDs across 8 YAML files
- Current registered Rust count: 62 (after adding RenamePerspectiveCmd)

## Acceptance Criteria
- [ ] Test exists and passes when all YAML commands have Rust impls
- [ ] Test FAILS if a YAML command ID is missing from `register_commands()`
- [ ] Test lists the missing command IDs in the failure message
- [ ] Any intentional client-only commands are in an explicit allowlist with comments

## Tests
- [ ] `cargo nextest run -p swissarmyhammer-kanban test_all_yaml_commands_have_rust_implementations` — passes
- [ ] Temporarily remove a registration from `register_commands()`, verify test fails with clear message
- [ ] `cargo nextest run -p swissarmyhammer-kanban` — all pass

## Workflow
- Use `/tdd` — write the test first (it should pass if card 1 is done), verify it catches gaps by temporarily breaking registration.