---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffee80
project: skills-guide-review
title: Align H1 headings with skill names in `kanban`, `really-done`, `thoughtful`
---
## What

The Anthropic guide's recommended body template (Chapter 2, "Writing the main instructions") leads with an H1 that matches the skill name:

```markdown
# Your Skill Name
## Instructions
```

Three skills deviate:

| Skill | Current H1 | Expected |
|-------|-----------|----------|
| `kanban` | `# Do` | `# Kanban` |
| `really-done` | `# Verification Before Completion` | `# Really Done` (or restructure as H1-less section if that's a deliberate choice — but current is mismatched) |
| `thoughtful` | (no H1 — jumps to `## The Most Important Thinks`) | `# Thoughtful` |

Fixing this makes the skill body discoverable and matches the template in the guide.

## Acceptance Criteria

- [x] `builtin/skills/kanban/SKILL.md` starts with `# Kanban` (not `# Do`).
- [x] `builtin/skills/really-done/SKILL.md` starts with an H1 that matches the skill name.
- [x] `builtin/skills/thoughtful/SKILL.md` has an H1 matching the skill name.

## Tests

- [x] Grep each file to confirm the first `^# ` line matches the skill's `name:` value.

## Reference

Anthropic guide, Chapter 2 — "Writing the main instructions" template. #skills-guide