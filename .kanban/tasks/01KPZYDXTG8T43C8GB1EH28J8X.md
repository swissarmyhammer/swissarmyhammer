---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffed80
project: skills-guide-review
title: Add `metadata` block to `thoughtful` and `really-done` skills
---
## What

Every other builtin skill has a consistent metadata block:

```yaml
metadata:
  author: swissarmyhammer
  version: "{{version}}"
```

Two skills are missing it:

- `builtin/skills/thoughtful/SKILL.md`
- `builtin/skills/really-done/SKILL.md`

The Anthropic guide (Reference B, "All optional fields") lists `metadata` with `author`, `version` as suggested. Consistency matters for distribution and discovery.

## Acceptance Criteria

- [x] Both skills have the standard metadata block with `author: swissarmyhammer` and `version: "{{version}}"`.

## Tests

- [x] Grep `builtin/skills/*/SKILL.md` for `metadata:` and confirm all 21 skills have it.

## Reference

Anthropic guide, Reference B — "All optional fields", metadata entries. #skills-guide