---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff8880
title: test skill body fails to render delegate-to-subagent partial (no "tester")
---
#test-failure

RESOLVED 2026-06-07.

Failing tests:
- crates/swissarmyhammer-tools/src/mcp/tools/skill/mod.rs:334 — test_skill_use_renders_test_skill_body
- crates/swissarmyhammer-tools/tests/integration/skill_e2e.rs:289 — test_skill_test_returns_body_content

Both assert the rendered builtin `test` skill body contains "tester" (the delegate-to-subagent paragraph).

Actual root cause (the tester's "include path doesn't resolve" guess was wrong — the include resolves fine, as do the sibling partials): the `delegate-to-subagent` partial body is guarded by `{% if agent %}{{ agent }}{% endif %}`, but the skill-render path (`render_skill_instructions` in skill/use_op.rs) only bound `version` and `arguments` into the template context — never `agent`. With `agent` unbound the block rendered empty, so "tester" never appeared. Also, the `use skill` result JSON (`UseSkill::execute`) didn't carry `agent` at all.

Fix:
- crates/swissarmyhammer-skills/src/operations/use_skill.rs — add `"agent": skill.agent` to the use-skill result value.
- crates/swissarmyhammer-tools/src/mcp/tools/skill/use_op.rs — bind that `agent` into the Liquid template context before rendering.

Verified green: test_skill_use_renders_test_skill_body, test_skill_test_returns_body_content, all use_skill + skill_e2e tests; clippy -D warnings clean on swissarmyhammer-skills + swissarmyhammer-tools.