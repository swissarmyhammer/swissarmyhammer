---
assignees:
- claude-code
position_column: todo
position_ordinal: be80
title: 'Test failure: work_the_card_skills_render_record_progress_guidance — finish skill lost its record-progress include'
---
Pre-existing failure, unrelated to ^t7ebyn8. Reproduced at HEAD 216a53e69 with the ^t7ebyn8 diff removed (`git stash push -- crates/`) — the failure is identical.

## Failure

```
cargo nextest run -p swissarmyhammer-skills --test skill_comment_guidance

thread 'work_the_card_skills_render_record_progress_guidance' panicked at
crates/swissarmyhammer-skills/tests/skill_comment_guidance.rs:37:9:
builtin skill 'finish' must instruct recording progress via the `add comment` op
```

## Cause

The test loops over `["implement", "finish", "kanban"]` and asserts each rendered body carries the record-progress guidance. `implement` and `kanban` both hold `{% include "_partials/record-progress" %}`. `builtin/skills/finish/SKILL.md` holds no such include and no "add comment" text — neither in the working tree nor at HEAD.

The include was dropped from the finish skill body. Last commits touching that file: 9141b5958, 53c22df3c, 364f21c61.

## Fix

Add `{% include "_partials/record-progress" %}` back to `builtin/skills/finish/SKILL.md`, in the same position the `implement` and `kanban` skills use. Do not weaken the test.

## Verify

```
cargo nextest run -p swissarmyhammer-skills --test skill_comment_guidance
```

Both tests must pass. `record_progress_guidance_has_single_source_of_truth` must stay green — the guidance text must live only in `builtin/_partials/record-progress.md`, so add the include, never an inline copy. #test-failure