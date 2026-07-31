---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kywnr47k8n9xwm0s6qnscsff
  text: |-
    Resolved: the user confirmed `builtin/_partials/skills.md` was deleted on purpose — agent system prompts no longer advertise available skills.

    All three tests in `crates/swissarmyhammer-templating/tests/skills_rendering_test.rs` existed only to assert that partial rendered (`test_skills_partial_renders_with_available_skills`, `test_skills_partial_hidden_when_no_skills`, `test_agent_system_prompt_includes_skills_section`), so the whole file was deleted rather than narrowed — unlike the short-ids and record-progress cases, there was no surviving consumer to keep asserting against.

    Checked before deleting: no dangling references to `skills_rendering_test` anywhere, and the sibling `all_skills_render_test.rs` does not touch the deleted partial, so it stays.

    swissarmyhammer-templating + mirdan: 646/646 pass, clippy clean, fmt clean. Committed as 57eb2ccaf.
  timestamp: 2026-07-31T18:07:26.451948+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffff8380
title: '3 red tests on main: skills_rendering_test wants the deleted _partials/skills.md'
---
Three tests in `crates/swissarmyhammer-templating/tests/skills_rendering_test.rs` fail on main:

- `test_skills_partial_hidden_when_no_skills`
- `test_skills_partial_renders_with_available_skills`
- `test_agent_system_prompt_includes_skills_section`

All three panic the same way:

```
Failed to get skills partial: Other { message: "Prompt '_partials/skills' not found" }
```

## Cause

`builtin/_partials/skills.md` was deleted as part of the partials dedup that landed in `ed28b4d1d refactor(skills): remove profile-based skill selection` (the deletion rode along with that commit; it is not part of the profile removal itself). The tests were never updated, so they still ask the template library for a partial that no longer exists. Surviving partials: `architecture-awareness.md`, `project-types`, `record-progress.md`, `review-column.md`, `task-double-check.md`, `task-standards.md`, `validator-tools.md`.

Found while running `cargo nextest run --workspace` for ^qsr5rdt. Full run: 12578 tests, 12574 passed, these 3 failed, 1 real-model test timed out.

## Decide and act

The dedup was a deliberate product call, so the question is whether an agent system prompt should still advertise available skills at all:

1. **The skills section is still wanted** — restore a `_partials/skills.md` (or point the tests at whichever surviving partial absorbed its content) and keep the three tests asserting the rendered section.
2. **The skills section is gone on purpose** — delete the three tests along with it, and check whether anything else still renders a skills section into an agent system prompt.

Read `ed28b4d1d` and the preceding `00e108145 refactor(skills): compact builtin skills and partials (#51)` to see where the content went before choosing. #test-failure #bug #test-failure-bug