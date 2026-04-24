---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffe180
project: skills-guide-review
title: Narrow `tdd` skill description — will over-trigger
---
## What

Current description of `builtin/skills/tdd/SKILL.md`:

> Use before writing any code, for any reason. Enforces strict test-driven development — RED, GREEN, REFACTOR.

This will over-trigger on every coding question per the guide's "Skill triggers too often" troubleshooting: the symptom is that the skill loads for unrelated queries. The guide prescribes three fixes: add negative triggers, be more specific, clarify scope.

The description is also missing **specific user trigger phrases** (e.g., "tdd", "test first", "red green refactor", "write a test").

## Acceptance Criteria

- [x] Description is re-worded to [what] + [when] + [specific trigger phrases], per the guide's structure.
- [x] Includes explicit trigger phrases users would say (e.g. "tdd", "test first", "red-green-refactor", "write the test first").
- [x] Adds a negative trigger or scope clarification that prevents loading on unrelated coding questions (e.g. "Do NOT use for reading or exploring code").
- [x] Description stays under 1024 characters and contains no `<` or `>` characters.

## Tests

- [x] Run manual trigger test: ask Claude "help me understand this code" — `tdd` should NOT load.
- [x] Ask Claude "write a failing test first" — `tdd` SHOULD load.
- [x] Ask Claude "fix this bug" — `tdd` should load (bug fix requires a test first in this codebase's conventions).
- [x] Ask Claude "when would you use the tdd skill?" — Claude quotes the new description and the triggers make sense.

## Reference

Anthropic "Complete Guide to Building Skills for Claude", Chapter 2 — Description field structure and Chapter 5 — "Skill triggers too often" troubleshooting.

## Implementation Notes

New description (670 chars, 0 forbidden chars):

> Use before writing or changing production code — enforces strict test-driven development (RED, GREEN, REFACTOR) by writing the failing test first, watching it fail, then writing the code to pass. Use when the user says "tdd", "test first", "write the test first", "red-green-refactor", "write a failing test", or when implementing a new function, fixing a bug, or adding behavior that needs a regression test. Do NOT use for reading, exploring, or explaining existing code — use the explore skill instead. Do NOT use for running an already-written test suite — use the test skill. Do NOT use for pure refactors that add no new behavior and keep the existing tests green.

Structural changes vs. old description:
- [what] keeps "enforces strict test-driven development (RED, GREEN, REFACTOR)" but adds the operational shape ("failing test first, watch it fail, then write the code to pass").
- [when] replaces "before writing any code, for any reason" (the over-trigger) with "before writing or changing production code" and an explicit trigger-phrase list.
- Three negative triggers added: reading/exploring/explaining code (→ explore), running existing tests (→ test), pure refactors with no behavior change.

Verified: `cargo test -p swissarmyhammer-skills --lib` passes all 106 tests — the new YAML frontmatter parses correctly. #skills-guide