---
assignees:
- claude-code
position_column: todo
position_ordinal: '9680'
title: test skill body include partials render empty, dropping "tester" reference
---
PRE-EXISTING, unrelated to the avp-cli/doctor/Doctorable teardown changes.

Two tests in swissarmyhammer-tools fail deterministically:
- crates/swissarmyhammer-tools/tests/integration/skill_e2e.rs:289 (test_skill_test_returns_body_content)
- crates/swissarmyhammer-tools/src/mcp/tools/skill/mod.rs:334 (test_skill_use_renders_test_skill_body)

Both assert the rendered `test` skill body contains "tester".

Root cause: builtin/skills/test/SKILL.md body has `{% include "_partials/delegate-to-subagent" %}` (the only source of "tester" in the body) and `{% include "_partials/coding-standards" %}`. When rendered through the MCP `use skill` pipeline in these tests, both Liquid `{% include %}` partials render to EMPTY — the rendered instructions go straight from "...build is clean or it's broken.\n\n\n\n\n## Guidelines\n\n## Validator Feedback" with no delegate/subagent/tester content. So the assertions fail.

The `tester` reference does still exist in frontmatter (`agent: tester`), but the tests check the rendered body text.

Likely a partial-resolution/registration gap in the skill render path used by the test harness (the `_partials/*` includes are not being found/expanded). Not caused by this branch — branch `review` only adds .kanban/tasks/* vs main.

What I tried: re-ran both tests in isolation (cargo nextest, command 32) - both fail reproducibly. Confirmed source SKILL.md contains the include and `agent: tester` frontmatter. #test-failure