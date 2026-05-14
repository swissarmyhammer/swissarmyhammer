---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffff180
project: skills-guide-review
title: Verify no skill description exceeds 1024 chars or contains `<`/`>`
---
## What

The Anthropic guide is explicit: description MUST be under 1024 characters and MUST NOT contain XML angle brackets. `code-context` and `lsp` use YAML folded scalars — they risk drifting past the limit.

Add an automated check to the repo's build/validate step to fail CI if any skill violates these constraints.

## Acceptance Criteria

- [x] A validation script or test asserts each `builtin/skills/*/SKILL.md` `description` length ≤ 1024 chars and has no `<`/`>` characters.
- [x] Runs as part of existing test/lint pipeline.

## Tests

- [x] Introduce a deliberately over-long description in a fixture skill and verify the check fails.
- [x] Introduce a description containing `<` and verify the check fails.

## Reference

Anthropic guide, Chapter 2 — Field requirements / description; Security restrictions. #skills-guide

## Implementation Notes

- Added `validate_description(&str) -> Result<(), String>` and `MAX_DESCRIPTION_CHARS = 1024` in `swissarmyhammer-skills/src/validation.rs`. Uses `chars().count()` (not bytes) so UTF-8 multi-byte glyphs (e.g. em-dashes) are counted correctly.
- Wired the check into `validate_frontmatter` so every load path (builtin, user, local) enforces the constraints — not just the test.
- Unit tests for `validate_description` cover: short ASCII, boundary at exactly 1024 chars, over-length rejection, `<` rejection, `>` rejection, UTF-8 char-vs-byte counting, and frontmatter-level propagation.
- Added integration test file `swissarmyhammer-skills/tests/builtin_description_compliance.rs` with two tests:
  - `all_builtin_skill_descriptions_comply_with_anthropic_guide` iterates every resolved builtin skill and asserts `validate_description` passes.
  - `validate_all_sources_reports_no_errors_for_builtin_descriptions` catches the harder failure mode where a skill fails to load entirely.
- `cargo test -p swissarmyhammer-skills` → 113 unit + 2 integration tests pass, clippy clean.