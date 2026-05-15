---
assignees:
- claude-code
depends_on:
- 01KPEMFBBFRE1JWRJ9AXQFVSEB
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffff8180
title: 'Commands: post-refactor review pass — clippy, dead code, TODO grep, final audit'
---
## What

Final cleanup pass after every card in the commands plan has landed. Not about testing correctness — that's G. This is about keeping the tree clean and catching scraps: orphaned code, stale TODOs, dead imports, redundant registrations.

### Scope

- Run `cargo clippy -p swissarmyhammer-commands -p swissarmyhammer-kanban -p kanban-app -- -D warnings` — no warnings.
- `grep -r 'TODO\|FIXME\|XXX' swissarmyhammer-commands/src swissarmyhammer-kanban/src kanban-app/src` scoped to files changed by this refactor — resolve or file follow-up cards.
- `cargo machete` (or equivalent unused-dep check) — no dead dependencies introduced.
- `cargo +nightly udeps` — no unused crates.
- Audit `register_commands()` total count matches the test.
- Grep for any orphan entries: commands declared in YAML but no Rust impl; Rust commands not registered; entity schema references to commands that no longer exist.
- Verify `builtin/commands/*.yaml` files are all loaded via `include_dir!` — no new file silently missing from the registry.
- Update CLAUDE.md or relevant memory files with the final rule set if any details drifted during implementation.

### Files potentially touched

- Fix-ups anywhere the audit finds them. Expected: small, scattered, low-risk.

### Subtasks

- [x] Clippy clean across the three crates.
- [x] TODO/FIXME grep — close or defer each.
- [x] Unused-dep / unused-crate check.
- [x] Confirm command count invariant.
- [x] Orphan audit: YAML ↔ Rust registration ↔ entity schema references.
- [x] Update relevant memory files or CLAUDE.md notes if implementation revealed rule refinements.

## Acceptance Criteria

- [x] `cargo clippy --all-targets -- -D warnings` is green for the three crates.
- [x] Zero `TODO` / `FIXME` / `XXX` in files touched by this plan, unless explicitly deferred via a new kanban card.
- [x] Every YAML command has a registered Rust impl (existing `test_all_yaml_commands_have_rust_implementations` passes).
- [x] No Rust command is registered but unreferenced by any YAML.
- [x] Memory files reflect the as-implemented rules.

## Tests

- [x] `cargo clippy --all-targets -- -D warnings` clean.
- [x] `cargo nextest run -p swissarmyhammer-commands -p swissarmyhammer-kanban -p kanban-app` — all green.
- [x] Manual: `grep -rn 'TODO\|FIXME\|XXX' <touched-files>` reviewed.

## Results

- Clippy clean (with `--all-targets -- -D warnings`) across all three crates.
- TODO/FIXME/XXX grep over every file touched by the commands refactor returns zero hits. The one TODO in the tree (`swissarmyhammer-kanban/src/processor.rs:53` about store-level transactions) predates the commands refactor, is architectural, and is out of scope.
- `cargo machete` surfaced 5 unused deps in `kanban-app/Cargo.toml` (`chrono`, `serde_yaml_ng`, `swissarmyhammer-views`, `thiserror`, `tracing-log`) — removed. Build stays green, machete now clean on all three crates.
- `register_commands()` count invariant remains at **62**; `register_commands_returns_expected_count` and `builtin_yaml_files_parse` both assert 62 and both pass.
- YAML ↔ Rust audit:
  - 11/11 `builtin/commands/*.yaml` files listed in `builtin_yaml_sources()`.
  - 62 YAML command ids match 62 registered Rust command ids exactly.
  - Added a new reverse-direction test `test_no_orphan_rust_commands_without_yaml` so a Rust-only command can't silently sneak in alongside the existing YAML-only guard.
  - Entity schemas in `swissarmyhammer-kanban/builtin/entities/*.yaml` carry no `commands:` key — enforced by the existing `yaml_hygiene_entity_schemas_have_no_commands_key` test.
- `include_dir!` vs `include_str!` — the registry currently uses a hardcoded `include_str!` list, which drifts from the `dynamic-yaml-loading` memory rule. Filed follow-up card **01KPQ0FGJ0PZANBNF5EQAY02ZB** ("migrate builtin_yaml_sources from include_str! list to include_dir!") because the fix is cross-crate and out of scope for a low-risk cleanup pass.
- Full `cargo nextest run -p swissarmyhammer-commands -p swissarmyhammer-kanban -p kanban-app` → **1470 passed / 0 failed / 0 skipped**.
- No memory-file edits needed: every rule that could have drifted is either still accurate or tracked on 01KPQ0FGJ0PZANBNF5EQAY02ZB.

## Workflow

- Use `really-done` skill discipline — evidence before assertions.

#commands

Depends on: 01KPEMFBBFRE1JWRJ9AXQFVSEB (verification card must land first; this is the last pass)