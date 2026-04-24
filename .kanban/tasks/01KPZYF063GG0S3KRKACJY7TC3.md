---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffff080
project: skills-guide-review
title: Add Examples section to user-facing skills (commit, review, explore, plan, task, finish)
---
## What

The Anthropic guide's recommended SKILL.md structure (Chapter 2, "Writing the main instructions") includes an `## Examples` section:

```markdown
## Examples

### Example 1: [common scenario]
User says: "..."
Actions: ...
Result: ...
```

None of the builtin skills carry this section. Examples make it easier for Claude to decide "this trigger matches X scenario", improving triggering precision.

Highest-leverage skills for examples:

- `commit` — examples of good/bad conventional commit messages and when to split commits.
- `review` — task-mode vs range-mode invocations.
- `explore` — how an exploration walk concludes in a test specification.
- `plan` — "I want to add auth" → resulting task breakdown.
- `task` — "track this bug" → resulting well-formed kanban task.
- `finish` — single-task vs scoped-batch invocations.

## Acceptance Criteria

- [x] Each of the six listed skills has an `## Examples` section with at least one concrete User says / Actions / Result example.
- [x] Examples reflect real invocations the user would actually type.

## Tests

- [x] Read each example aloud and verify it maps to an actual command path in the skill body.

## Reference

Anthropic guide, Chapter 2 — "Writing the main instructions" template (Examples subsection). #skills-guide