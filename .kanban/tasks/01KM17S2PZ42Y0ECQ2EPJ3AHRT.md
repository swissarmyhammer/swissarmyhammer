---
assignees:
- claude-code
position_column: todo
position_ordinal: '8980'
title: '[WARNING] PLANNING_GUIDE.md partial include position inserts card-standards mid-section'
---
## What

In `builtin/skills/plan/PLANNING_GUIDE.md`, the `{% include "_partials/card-standards" %}` tag is appended at the end of the "Research thoroughly before creating cards" section (after the bullet list), rather than at a natural section boundary. The card-standards content begins with `### Every card must be actionable` — a heading at the same depth as `### Board naming`, `### Ordering`, etc. — but in the rendered file it appears mid-way through the Constraints section between research bullets and the Board naming rule.

In `builtin/skills/plan/SKILL.md` the same pattern applies: the include is appended to the "Research before cards" paragraph, causing the same ordering issue.

In the original files, "Every card must be actionable" appeared as a sibling section after the research section. The partial should be included as a top-level sibling inside Constraints, not appended to the preceding section paragraph.

Specifically: move `{% include "_partials/card-standards" %}` to its own line between the "Research before cards" section and the "Board naming" section, with a blank line above and below, rather than as a continuation of the research section's last paragraph.

Files:
- `builtin/skills/plan/SKILL.md` (line 33)
- `builtin/skills/plan/PLANNING_GUIDE.md` (line 27)

## Acceptance Criteria
- [ ] In both files the include tag stands on its own blank-line-separated block between the research section and the board naming section.
- [ ] Rendered output of both files has `### Every card must be actionable` at the same structural level as `### Board naming`.

## Tests
- [ ] Visual inspection of rendered skill output via `skill { op: "use skill", name: "plan" }` confirms correct heading hierarchy. #review-finding #warning