---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kyvvjzeb4sztq6p1nx3whggp
  text: |-
    Resolved the other way: the user confirmed the include was removed from `finish` on purpose, so the test was wrong, not the skill.

    `finish` is an orchestrator — it drives `/implement` and `/review` and never works a card itself, so the comment trail belongs to the skills it delegates to. Narrowed the roster in `crates/swissarmyhammer-skills/tests/skill_comment_guidance.rs` to `implement` and `kanban`, both of which still carry `{% include "_partials/record-progress" %}`, and recorded the exclusion reason in the module doc so nobody "restores" it later.

    The roster stays non-empty, so the assertions still run — the test did not go vacuous.

    126/126 in swissarmyhammer-skills, up from 125/126. Committed as 302edf4fe.
  timestamp: 2026-07-31T10:30:14.731292+00:00
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffffe80
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