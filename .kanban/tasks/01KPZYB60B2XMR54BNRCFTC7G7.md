---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffec80
project: skills-guide-review
title: Remove duplicate generic coding advice from `detected-projects` body
---
## What

`builtin/skills/detected-projects/SKILL.md` ends with a generic `## Approach` section that repeats rules already lived in `thoughtful/SKILL.md` ("Think before acting", "Prefer editing over rewriting", etc.). Per the guide, SKILL.md should stay focused on the skill's core instructions; generic approach rules belong once, globally.

This also bloats the context loaded when the skill triggers.

## Acceptance Criteria

- [x] Remove the `## Approach` section from `detected-projects/SKILL.md`.
- [x] Leave the actual process (`detect projects` call + guidance) intact.

## Tests

- [x] Skill still loads and Claude still calls `detect projects` correctly when triggered.

## Reference

Anthropic guide, Chapter 5 — "Instructions not followed / Instructions too verbose" — keep instructions concise. #skills-guide