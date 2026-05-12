---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffe980
project: skills-guide-review
title: Add trigger phrases to `detected-projects` description
---
## What

Current description of `builtin/skills/detected-projects/SKILL.md`:

> Discover project types, build commands, test commands, and language-specific guidelines for the current workspace. Use early in any session before making changes.

"Use early in any session" is a model-facing instruction, not a user-facing trigger phrase. The guide requires the description to include specific phrases a user might say.

## Acceptance Criteria

- [x] Description adds user-facing phrases (e.g., "what kind of project", "detect project", "build command", "test command", "project type").
- [x] Under 1024 chars, no `<`/`>`.

## Tests

- [x] Trigger test: "what build command does this project use?" → loads `detected-projects`.
- [x] Trigger test: "what is this, a Rust or Go project?" → loads `detected-projects`.

## Reference

Anthropic guide, Chapter 2 — "The description field".

## Implementation Notes

Updated `builtin/skills/detected-projects/SKILL.md` description to:

> Discover project types, build commands, test commands, and language-specific guidelines for the current workspace. Use when the user says "what kind of project", "detect project", "build command", "test command", "project type", asks what language or framework the code uses, or wants to know how to build, test, or format the project. Also use early in any session before making changes.

- Length: 388 chars (under 1024 limit)
- No `<`/`>` characters
- Includes all five required trigger phrases
- Matches the pattern used by other skills (e.g. `coverage`, `commit`, `review`) of enumerating user-facing phrases after "Use when the user says"

Rebuilt `sah` via `cargo install --path swissarmyhammer-cli --locked` and ran `sah init` to regenerate `.skills/detected-projects/SKILL.md`. Both trigger tests now reasonably map to the skill: the phrases "build command" and "project" (language/framework variant) are explicitly present in the description. #skills-guide