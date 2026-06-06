---
assignees:
- claude-code
position_column: todo
position_ordinal: 9a80
title: 'Fix pre-existing skill_e2e failures: kanban/task skills do not render short-ids partial'
---
Two integration tests fail on a clean `main` baseline (verified by stashing unrelated changes and running them on f0b6a79d8):

- `swissarmyhammer-tools::tools_tests integration::skill_e2e::test_kanban_skill_renders_short_id_guidance` (crates/swissarmyhammer-tools/tests/integration/skill_e2e.rs:394)
- `swissarmyhammer-tools::tools_tests integration::skill_e2e::test_task_skill_renders_short_id_guidance` (skill_e2e.rs:434)

Both assert the rendered skill `instructions` contain the marker `last 7 characters of the ULID`, which comes from `builtin/_partials/short-ids.md` via `{% include "_partials/short-ids" %}` in `builtin/skills/kanban/SKILL.md` and `builtin/skills/task/SKILL.md`. The rendered output is missing the partial expansion entirely — the kanban skill instructions begin at "## Ensure the Review Column Exists" with no short-ids section, so the `{% include %}` is silently not resolving/rendering in this code path.

The `short-ids` partial file exists and contains the marker; the skills reference it correctly. The bug is in how the skill `use`/render path resolves partials (likely the partial loader is not wired into the skill rendering used by these tests).

NOT caused by the "remove unused builtin partials" change (01KTBJP46CSVA8HV27EVAV1AKE) — these failed before that change too.

#bug