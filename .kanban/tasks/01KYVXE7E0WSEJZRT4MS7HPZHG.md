---
assignees:
- claude-code
position_column: todo
position_ordinal: bf80
title: 'swissarmyhammer-skills: typed SkillError instead of Result&lt;_, String&gt;'
---
12 review findings in `crates/swissarmyhammer-skills`, split out of ^t7ebyn8. All pre-existing — untouched public signatures that entered review scope because the card added a constant above them, displacing their line numbers.

## Items

These public functions return `Result<_, String>` with no `# Errors` doc:

- `resolve_skill`
- `parse_skill_md`
- `parse_skill_md_with_path`
- `load_skill_from_dir`
- `load_skill_from_builtin`
- `SkillName::new`

Plus frontmatter/filename literals in the pre-existing `split_frontmatter` that should be named constants.

## Required change

1. Introduce a typed `SkillError` enum (thiserror, as the workspace does elsewhere) and replace the stringly-typed error channel. A `String` error cannot be matched on, so no caller can react to a specific failure — every consumer either propagates it or does a substring test.
2. Add `# Errors` doc sections naming each variant a function can return.
3. Name the frontmatter delimiter and filename literals.

Check the call sites before changing the signature — `resolve_skill` and `parse_skill_md` are used by shelltool-cli, code-context-cli, kanban-cli, and mirdan. This is a breaking change to a crate-public API and the blast radius is real; use `code_context get blastradius` first.

## Warning on line numbers

The review engine's cited lines track the pre-image and are offset by roughly 12–19 lines. Grep for the symbol.

## Acceptance

- No `Result<_, String>` remains in the crate's public API.
- Every public fallible function has an `# Errors` section naming its variants.
- `cargo nextest run -E 'rdeps(swissarmyhammer-skills)'`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` clean.

Related, do not duplicate: ^ksys4z5 covers the agent-side frontmatter round-trip. #refactor