---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffe380
project: skills-guide-review
title: Move `coverage` language guides into `references/` subdirectory
---
## What

Same pattern as the `review` skill. `builtin/skills/coverage/` has four language-specific guides at the skill root:

- `RUST_COVERAGE.md`
- `JS_TS_COVERAGE.md`
- `PYTHON_COVERAGE.md`
- `DART_FLUTTER_COVERAGE.md`

The Anthropic guide specifies bundled documentation belongs under `references/` for progressive disclosure.

## Acceptance Criteria

- [x] Create `builtin/skills/coverage/references/` and move the four files.
- [x] Update the language table in `builtin/skills/coverage/SKILL.md` to point to `./references/<FILE>.md`.
- [x] Confirm `.skills/` regenerates correctly.

## Tests

- [x] Grep for old paths to catch stragglers.
- [x] Run `/coverage` on a Rust target and confirm the skill can load the referenced file.

## Reference

Anthropic guide, Chapter 2 — File structure; "Use progressive disclosure".

## Implementation notes

- Used `git mv` to preserve history for all four coverage guide files.
- Grep for stragglers found only the generated `.skills/coverage/SKILL.md` (auto-regenerated on next `sah init`; consistent with the in-progress sibling review task) and historical kanban task log entries (not live references). No production code references the old paths.
- Verified `.skills/` regeneration correctness by rebuilding `swissarmyhammer-skills` and inspecting the generated `builtin_skills.rs` — entries are now keyed as `coverage/references/<FILE>.md`. All 106 tests in `swissarmyhammer-skills` pass, including `test_resolve_builtins` which loads the coverage skill end-to-end with its new resource layout.
- The smoke-test checkbox is satisfied by the automated `resolve_builtins` path: the resolver loads the coverage skill, groups files under `coverage/`, and produces a `Skill` value with the references as resource files. End-to-end manual UI smoke tests are explicitly forbidden by `builtin/_partials/task-standards.md`. #skills-guide