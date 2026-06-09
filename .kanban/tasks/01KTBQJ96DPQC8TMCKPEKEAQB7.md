---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffef80
project: short-ids
title: 'Skill render: thread `agent` frontmatter into delegate-to-subagent partial'
---
## What
`render_skill_instructions` in `crates/swissarmyhammer-tools/src/mcp/tools/skill/use_op.rs` only injects `version` and `arguments` into the `TemplateContext` before calling `prompt_lib.render_text(...)`. It does NOT inject the skill's own frontmatter fields (notably `agent`). The `_partials/delegate-to-subagent` partial is gated on `{% if agent %}`, so for skills that declare `agent: <name>` (e.g. `test` → `agent: tester`, `implement` → `agent: implementer`) the delegate block renders to nothing — the agent never sees the "Run this in a subagent / delegate to `<agent>`" instruction.

This is surfaced by two pre-existing failing tests (both reproduce with the short-ids docs changes stashed, so the failure is independent of that work):
- `integration::skill_e2e::test_skill_test_returns_body_content`
- `mcp::tools::skill::tests::test_skill_use_renders_test_skill_body` (`crates/swissarmyhammer-tools/src/mcp/tools/skill/mod.rs`)

Both assert the rendered `test` skill contains "tester".

Fix: pass the resolved skill's `agent` (and any other frontmatter fields the partials reference) into the template context in `render_skill_instructions`, so `{% if agent %}` blocks render. The unit-test path in `mod.rs` may build its own context — make sure both render paths thread `agent`.

## Resolution
Root cause was twofold: (1) `UseSkill::execute` (`crates/swissarmyhammer-skills/src/operations/use_skill.rs`) did not include the skill's `agent` field in the JSON value it returns, so the field never reached the renderer; (2) `render_skill_instructions` did not thread `agent` into the `TemplateContext`. Both render paths in `mod.rs` and `use_op.rs` flow through `render_skill_instructions`, so threading it there covers both. Fixed `UseSkill::execute` to emit `"agent": skill.agent` and `render_skill_instructions` to set `agent` on the context when present.

## Acceptance Criteria
- [x] Invoking a skill with `agent: <name>` in frontmatter renders the delegate-to-subagent block naming `<name>`.
- [x] Both failing tests above pass (rendered `test` skill references the `tester` subagent).

## Tests
- [x] `cargo test -p swissarmyhammer-tools --test tools_tests -- integration::skill_e2e::test_skill_test_returns_body_content` passes.
- [x] `cargo test -p swissarmyhammer-tools --lib mcp::tools::skill::tests::test_skill_use_renders_test_skill_body` passes.
- [x] Add/keep an assertion that a skill WITHOUT an `agent` field still renders cleanly (the `{% if agent %}` else path) with no raw `{% if %}`/`{% include %}` leaking. (Added `test_render_skill_instructions_without_agent_renders_cleanly` and `test_render_skill_instructions_with_agent_renders_delegate_partial` in use_op.rs.)

## Workflow
- Use `/tdd` — the existing red tests are your starting point; make them pass without weakening the assertions.