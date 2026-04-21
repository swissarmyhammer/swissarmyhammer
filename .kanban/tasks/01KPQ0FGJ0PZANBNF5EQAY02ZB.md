---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff8980
title: 'Commands: migrate builtin_yaml_sources from include_str! list to include_dir! (memory-rule drift)'
---
## What

`swissarmyhammer-commands/src/registry.rs::builtin_yaml_sources()` hardcodes every YAML file in the `builtin/commands/` directory with `include_str!`. The `dynamic-yaml-loading` memory rule explicitly calls this pattern out:

> Use `include_dir!` or build-time codegen for YAML collections, never hardcoded `include_str!` lists.

The hardcoded list currently matches the file set (11 of 11), and the `builtin_yaml_files_parse` + `test_all_yaml_commands_have_rust_implementations` + new `test_no_orphan_rust_commands_without_yaml` tests together guarantee the invariant at test time — but a new contributor adding a YAML file will still silently miss the registry unless they also update the hardcoded list. The memory rule exists precisely to prevent that silent-miss hazard.

Same pattern appears in at least two sibling crates per the comment in `registry.rs::load_yaml_dir`:

> Note: identical copies exist in `swissarmyhammer-fields` and `swissarmyhammer-views`.

So the fix is cross-crate.

## Acceptance Criteria

- [ ] `builtin_yaml_sources()` (and equivalent functions in `swissarmyhammer-fields` / `swissarmyhammer-views`) uses `include_dir!` or a build-time codegen step — no hardcoded `include_str!` list of YAML files.
- [ ] Adding a new `builtin/commands/*.yaml` file makes it available to the registry with no code change to the Rust file listing.
- [ ] Existing `builtin_yaml_files_parse`, `test_all_yaml_commands_have_rust_implementations`, `test_no_orphan_rust_commands_without_yaml` all still pass.
- [ ] No new clippy warnings.

## Why

Dropped out of the commands-post-refactor review pass (01KPG7JPJYHE65Q88KG44T842F). This is architectural drift, not a cleanup fix-up — the change touches three crates and their build machinery, so it is out of scope for the audit card and deserves its own card.

## Files to touch

- `swissarmyhammer-commands/src/registry.rs` — `builtin_yaml_sources()` and test IDs list.
- `swissarmyhammer-commands/Cargo.toml` — add `include_dir` dep.
- `swissarmyhammer-fields/src/...` — matching change to its YAML loader.
- `swissarmyhammer-views/src/...` — matching change to its YAML loader.

#commands
#tech-debt

## Review Findings (2026-04-20 22:00)

Scope clarification from implementer confirmed: `swissarmyhammer-fields` and `swissarmyhammer-views` take YAML sources as parameters — they have no embedded list to migrate. The root `builtin/` trees (definitions, entities, views, actors) already live in `swissarmyhammer-kanban/src/defaults.rs` via `include_dir!`. The only drift was in `swissarmyhammer-commands/src/registry.rs`.

The fix also silently repairs a pre-existing latent bug on `main`: the old hardcoded list had **8 entries**, not 11 — `column.yaml`, `tag.yaml`, and `task.yaml` (6 commands: `column.reorder`, `tag.update`, `task.move`, `task.delete`, `task.untag`, `task.doThisNext`) were embedded in the crate but never exposed through `builtin_yaml_sources()`. The hygiene tests (`test_all_yaml_commands_have_rust_implementations`, `test_no_orphan_rust_commands_without_yaml`) both derive their YAML-id set from `builtin_yaml_sources()`, so they were checking a subset against itself and could not catch the miss. Post-fix, all 11 files / 62 commands are registered and the tests guard the full set.

Tests: 175 `swissarmyhammer-commands` lib tests pass; 416 `swissarmyhammer-kanban` lib command tests pass; clippy clean on the commands crate with `-D warnings`.

### Nits
- [x] `swissarmyhammer-commands/src/registry.rs:192` — `include_dir!` is recursive but `file_stem()` uses only the basename. If `builtin/commands/` ever grows a subdirectory, two files with the same stem (e.g. `commands/foo.yaml` and `commands/sub/foo.yaml`) would both be emitted with key `"foo"`, letting the later one silently shadow the earlier via the `HashMap` insert in `merge_yaml_value`. Non-issue today (directory is flat) — either keep the layout flat by convention, or filter with `file.path().parent() == Some(Path::new(""))` if you want to enforce it at the loader.

  Resolved: `builtin_yaml_sources()` now filters on `file.path().parent() == Some(Path::new(""))`, enforcing the flat layout at the loader. Confirmed via a standalone Rust check that `include_dir!` emits root-level paths with `parent() == Some("")` and nested paths with `parent() == Some("sub")`, so a future accidental subdirectory will simply be skipped instead of silently shadowing a root file. 175 commands lib tests still pass; clippy clean with `-D warnings`.
