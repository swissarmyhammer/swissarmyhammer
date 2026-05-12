---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffff580
project: skills-guide-review
title: Skill deploy pipeline must preserve `references/` subdirectory structure
---
## What

The skill deploy pipeline currently flattens subdirectory structure when copying bundled resource files. Skills that follow the Anthropic guide's `references/` layout (`coverage/` since prior work, `review/` as of task 01KPZY9P08XE6DSVM962JP95Z2) end up with their reference files deployed at the skill root instead of under `references/`, so the `[...](./references/FOO.md)` links in SKILL.md resolve to the wrong path after deployment.

### Root cause

Three cooperating layers drop the subdirectory:

1. `swissarmyhammer-skills/src/skill_loader.rs::load_skill_from_dir` uses `std::fs::read_dir` (non-recursive) and stores `path.file_name()` only, so files under `references/` are never loaded from disk.
2. `swissarmyhammer-skills/src/skill_loader.rs::load_skill_from_builtin` receives recursively-discovered paths like `review/references/RUST_REVIEW.md` but keeps only `name.rsplit('/').next()` — the subdirectory is thrown away.
3. `swissarmyhammer-cli/src/commands/skill.rs::deploy_single_skill` then writes each file as `skill_dir.join(filename)` with no `create_dir_all` for a parent, and `is_safe_name` (from `install/components/mod.rs`) rejects any filename containing `/`, so subdirectory-bearing names would be refused even if the earlier layers preserved them.

### Why this matters now

`builtin/skills/coverage/` already uses `references/` and is broken in deployment (the `.skills/coverage/` output has the files flat). `builtin/skills/review/` now has the same layout. Future skills following the guide will hit the same bug.

## Acceptance Criteria

- [x] `load_skill_from_dir` walks subdirectories (e.g. with `walkdir`) and stores resource keys as paths relative to the skill root (`references/RUST_REVIEW.md`, not just `RUST_REVIEW.md`).
- [x] `load_skill_from_builtin` strips only the leading skill-name segment and keeps the remaining relative path as the resource key.
- [x] `is_safe_name` (or the path check at the deploy site) accepts forward-slash-separated relative paths while still rejecting `..`, absolute paths, and backslashes. Prefer a separate `is_safe_relative_path` helper over relaxing `is_safe_name`, since other callers of `is_safe_name` still want the strict single-segment check.
- [x] `deploy_single_skill` calls `create_dir_all` on the parent of each resource file before writing.
- [x] After redeployment, `.skills/coverage/references/RUST_COVERAGE.md` and `.skills/review/references/RUST_REVIEW.md` both exist and the SKILL.md links resolve to them.

## Tests

- [x] Unit test in `swissarmyhammer-skills/src/skill_loader.rs` covering `load_skill_from_dir` against a temp skill dir that has a `references/` subdirectory — assert the resource map contains `references/helper.md`.
- [x] Unit test for `load_skill_from_builtin` with input `[("my-skill/SKILL.md", "..."), ("my-skill/references/helper.md", "...")]` — assert `resources.files` contains `references/helper.md`.
- [x] Unit test for the new relative-path safety check: accepts `references/foo.md`, rejects `../escape.md`, `/abs/path.md`, and `foo\bar.md`.
- [x] Integration-style test in `swissarmyhammer-cli/src/commands/skill.rs` (or `install/components/mod.rs`) that deploys a skill with a `references/` file to a temp directory and asserts the file lands at `<skill>/references/<name>`.

## Reference

- Anthropic guide, Chapter 2 — File structure (progressive disclosure via `references/`).
- Discovered while implementing task 01KPZY9P08XE6DSVM962JP95Z2 (the review-skill move). #skills-guide