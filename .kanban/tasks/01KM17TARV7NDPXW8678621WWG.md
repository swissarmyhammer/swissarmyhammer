---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffe080
title: '[NIT] card/SKILL.md: "detected-projects" partial included but card skill may run outside a coding context'
---
## What

`builtin/skills/card/SKILL.md` includes three partials at the top:

```
{% include "_partials/detected-projects" %}
{% include "_partials/coding-standards" %}
{% include "_partials/test-driven-development" %}
```

These are appropriate when the card describes coding work, but `/card` is described as general-purpose ("track an idea, or capture work"). Including coding standards unconditionally means non-coding cards (e.g. "write a blog post", "schedule a meeting") will have irrelevant boilerplate prepended.

Compare with how `/plan` and `/implement` handle this — they also include these partials, so the pattern is consistent. If the intent is that `/card` is coding-only, the description should reflect that. If it is truly general-purpose, the partials should be conditional or omitted.

File:
- `builtin/skills/card/SKILL.md` (lines 9–11)

## Acceptance Criteria
- [ ] Either: the description is updated to "Create a single, well-researched kanban card for a coding task" to match the partials, OR the coding partials are made conditional on project detection, OR a comment explains why unconditional inclusion is acceptable.

## Tests
- [ ] No automated test needed; this is a documentation/semantics alignment issue. #review-finding #nit