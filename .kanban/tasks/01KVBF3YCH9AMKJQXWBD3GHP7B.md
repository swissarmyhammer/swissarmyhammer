---
assignees:
- claude-code
comments:
- actor: wballard
  id: 01kvbfbtv2bcae6fmdaasz5jmd
  text: 'Picked up. TDD: added crates/swissarmyhammer-skills/tests/finish_commit_guidance.rs mirroring skill_comment_guidance.rs (reuses shared `mod common;` rendered_builtin_instructions). Watched it FAIL (RED): "finish skill must delegate committing to the `/commit` skill". Then edited builtin/skills/finish/SKILL.md: added /commit to the "Orchestrator only … Delegates to" line; added single-task loop step 6 "Commit the rollback point" between Verify-done and the guardrail (local commit only, NEVER push, nothing-to-commit no-op, only after done-confirmed); renumbered guardrail→7, clear-ralph→8; updated scoped-batch "steps 2-7" reference so batch reuses the loop (one commit per task automatically); added /commit to the Delegation constraints section; updated both Examples. GREEN: finish_commit_guidance passes. Verification: `cargo test -p swissarmyhammer-skills` → 115+2+1+2+2 all green; `cargo test -p swissarmyhammer-templating` → all_skills_render_test 1 passed, skills_rendering_test 3 passed.'
  timestamp: 2026-06-17T19:01:41.346503+00:00
- actor: wballard
  id: 01kvbg1qb4bpez2nyfdax5ajtb
  text: |-
    Worked the 2026-06-17 14:09 review findings (2 warnings + 1 nit, 0 blockers).

    SKILL.md fixes (verified whole-file step numbering is now internally consistent: Verify-done=5, Commit=6, Guardrail=7, Clear-ralph=8):
    - Warning (Constraints → Delegation): "the step 6 guardrail handles it" → "the step 7 guardrail handles it" (guardrail is step 7 post-renumber).
    - Nit (Scoped-batch step 2): "(steps 2–7 above)" → "(steps 2–8 above)" so batch loop-reuse covers the commit (6), guardrail (7), and clear-ralph/report (8) tail.
    - Re-grepped all "step N" refs: lines for step 2 (Test/findings/not-done → re-implement), Step 5 (clean → done), "step 5 confirms done", step 1 (Pick next) all already correct; no other stale refs.

    Test fix (finish_commit_guidance.rs):
    - Warning: replaced the 5-way OR chain of .contains() with `const NO_PUSH_PHRASES: &[&str] = &["never push","not push","no push","don't push","does not push"]` + `assert!(NO_PUSH_PHRASES.iter().any(|p| lower.contains(p)), ...)`. Same 5 phrases, same coverage, assertion still fails if none present.

    Only the two task-owned files touched; unrelated working-tree edits (mirdan/src/status.rs, validators/rust/rules) left alone.

    Verification (fresh):
    - `cargo test -p swissarmyhammer-skills` → exit 0; finish_commit_guidance 1 passed; lib 115 passed; all suites green.
    - `cargo test -p swissarmyhammer-templating --test all_skills_render_test` → test_all_builtin_skills_render_without_errors 1 passed (finish still renders).
    - Full `cargo test -p swissarmyhammer-templating` → all green (21 doctests + 3 skills_rendering).

    All three findings checked off. Moving back to review.
  timestamp: 2026-06-17T19:13:38.660065+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffbc80
title: 'finish: commit each task locally when it reaches done (rollback points)'
---
## What

The `finish` skill (`builtin/skills/finish/SKILL.md`) drives tasks to `done` but never commits — so a multi-task `/finish` run leaves one big uncommitted working tree with no per-task rollback points. Add a step so finish creates a **local commit after each task reaches `done`**, giving a clean rollback point at each known-good state (green tests + clean review).

**Decisions (from the user):**
- **Staging:** delegate to the existing `/commit` skill (stages all modified source + `.kanban`). Simpler and reuses the skill; the user accepts that this can bundle unrelated working-tree edits into the task's commit.
- **Local only — do NOT push.** Rollback points are local commits; the user pushes explicitly when ready. Pushing per task would spam CI in batch mode.

### Approach (scope: two files)

1. `builtin/skills/finish/SKILL.md` — in **single-task mode**, add a commit step between step 5 (\"Verify done\") and step 7 (\"Clear ralph and report\"): once `get task` confirms the task is in `done`, invoke the `/commit` skill to create a local commit of the verified-good state. Because **scoped-batch mode reuses the single-task loop** (its step 2 says \"Run the single-task mode loop on `<TASK_ID>`\"), this automatically produces one commit per finished task in batch mode — the per-task rollback point — before the next task is picked.
   - State it as orchestration-by-delegation, consistent with the skill's existing \"Orchestrator only — delegates to `/implement`, `/review`, `/test`\" framing: add `/commit` to that delegation list and the Delegation constraints section.
   - Explicitly: **commit only, never push** (call out that pushing is the user's explicit step, to avoid per-task CI runs in batch mode).
   - If a finished task produced no changes, `/commit` is a no-op (it reviews `git status`); note that finish does not treat \"nothing to commit\" as an error.
   - The commit happens only after the task is confirmed in `done` (post clean review + green tests), so every finish-created commit is a verified-good rollback point.

2. `crates/swissarmyhammer-skills/tests/finish_commit_guidance.rs` (new) — a guidance test mirroring the existing `crates/swissarmyhammer-skills/tests/skill_comment_guidance.rs`: resolve the builtin `finish` skill via `SkillResolver`, and assert its instructions instruct committing via `/commit` when a task reaches `done`, and that it says local-only / does not push.

### Out of scope
- Per-task **file-scoped** staging (committing only the task's own files) — the user chose `/commit` (all changes). Do not implement selective staging.
- Any push behavior.

## Acceptance Criteria
- [x] `builtin/skills/finish/SKILL.md` instructs invoking `/commit` once a task is confirmed in `done`, in the single-task loop (so batch mode commits per task via loop reuse).
- [x] The instruction is explicitly **local commit only — no push** (pushing is the user's separate step).
- [x] `/commit` is added to the skill's Delegation list / constraints alongside `/implement`, `/review`, `/test`.
- [x] The skill notes \"nothing to commit\" is a no-op, not an error.
- [x] The commit step is positioned after done-verification, so only verified-good state is committed.

## Tests
- [x] Add `crates/swissarmyhammer-skills/tests/finish_commit_guidance.rs` (mirroring `skill_comment_guidance.rs`): resolve the builtin `finish` skill and assert its rendered instructions contain the commit-on-done guidance (mentions `/commit`, `done`, and local-only / no-push).
- [x] `cargo test -p swissarmyhammer-skills` → green (new test passes).
- [x] `cargo test -p swissarmyhammer-templating` → green (the `all_skills_render_test` still renders the `finish` skill).

## Workflow
- Use `/tdd` — write the `finish_commit_guidance.rs` assertions first (they fail because the SKILL.md has no commit step yet), then edit `builtin/skills/finish/SKILL.md` to make them pass.

## Review Findings (2026-06-17 14:09)

### Warnings
- [x] `builtin/skills/finish/SKILL.md` (Constraints → Delegation) — Stale step cross-reference from the renumber. Inserting the new commit step as **step 6** pushed the guardrail to **step 7** and clear-ralph to **step 8** in the single-task loop, but the Delegation bullet still reads \"Stuck task → the **step 6** guardrail handles it\". The guardrail is now step 7. Update the reference to \"the step 7 guardrail\" (or refer to it by name) so the cross-reference is coherent post-renumber.
- [x] `crates/swissarmyhammer-skills/tests/finish_commit_guidance.rs` — The five alternative phrasings of \"no push\" are expressed as an OR chain of `.contains()` calls rather than as a data table. This scatters the variation across the code; adding a sixth variant requires editing the assertion logic instead of adding to a list. Extract the five phrases into a `const` array or `Vec` and check with `.iter().any()`: `const NO_PUSH_PHRASES: &[&str] = &[\"never push\", \"not push\", \"no push\", \"don't push\", \"does not push\"]; assert!(NO_PUSH_PHRASES.iter().any(|p| lower.contains(p)), \"...\");`.

### Nits
- [x] `builtin/skills/finish/SKILL.md` (Scoped-batch step 2) — Minor: the loop-reuse reference reads \"Run the single-task mode loop (steps 2–7 above)\", but the loop now spans steps 2–8 (step 8 is clear-ralph/report and the commit is step 6, guardrail step 7). The \"2–7\" range silently drops the new commit-and-report tail. Consider \"steps 2–8 above\" (or \"the steps above\") so batch mode unambiguously reuses the commit step it relies on for the per-task rollback point.