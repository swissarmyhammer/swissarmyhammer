---
assignees:
- claude-code
depends_on:
- 01KPEMFBBFRE1JWRJ9AXQFVSEB
position_column: todo
position_ordinal: f280
title: 'Commands: post-refactor review pass — clippy, dead code, TODO grep, final audit'
---
## What

Final cleanup pass after every card in the commands plan has landed. Not about testing correctness — that's G. This is about keeping the tree clean and catching scraps: orphaned code, stale TODOs, dead imports, redundant registrations.

### Scope

- Run `cargo clippy -p swissarmyhammer-commands -p swissarmyhammer-kanban -p kanban-app -- -D warnings` — no warnings.
- `grep -r 'TODO\\|FIXME\\|XXX' swissarmyhammer-commands/src swissarmyhammer-kanban/src kanban-app/src` scoped to files changed by this refactor — resolve or file follow-up cards.
- `cargo machete` (or equivalent unused-dep check) — no dead dependencies introduced.
- `cargo +nightly udeps` — no unused crates.
- Audit `register_commands()` total count matches the test.
- Grep for any orphan entries: commands declared in YAML but no Rust impl; Rust commands not registered; entity schema references to commands that no longer exist.
- Verify `builtin/commands/*.yaml` files are all loaded via `include_dir!` — no new file silently missing from the registry.
- Update CLAUDE.md or relevant memory files with the final rule set if any details drifted during implementation.

### Files potentially touched

- Fix-ups anywhere the audit finds them. Expected: small, scattered, low-risk.

### Subtasks

- [ ] Clippy clean across the three crates.
- [ ] TODO/FIXME grep — close or defer each.
- [ ] Unused-dep / unused-crate check.
- [ ] Confirm command count invariant.
- [ ] Orphan audit: YAML ↔ Rust registration ↔ entity schema references.
- [ ] Update relevant memory files or CLAUDE.md notes if implementation revealed rule refinements.

## Acceptance Criteria

- [ ] `cargo clippy --all-targets -- -D warnings` is green for the three crates.
- [ ] Zero `TODO` / `FIXME` / `XXX` in files touched by this plan, unless explicitly deferred via a new kanban card.
- [ ] Every YAML command has a registered Rust impl (existing `test_all_yaml_commands_have_rust_implementations` passes).
- [ ] No Rust command is registered but unreferenced by any YAML.
- [ ] Memory files reflect the as-implemented rules.

## Tests

- [ ] `cargo clippy --all-targets -- -D warnings` clean.
- [ ] `cargo nextest run -p swissarmyhammer-commands -p swissarmyhammer-kanban -p kanban-app` — all green.
- [ ] Manual: `grep -rn 'TODO\\|FIXME\\|XXX' <touched-files>` reviewed.

## Workflow

- Use `really-done` skill discipline — evidence before assertions.

#commands

Depends on: 01KPEMFBBFRE1JWRJ9AXQFVSEB (verification card must land first; this is the last pass)