---
comments:
- actor: wballard
  id: 01kva1fpzzv8rgc8dpnxv28zp6
  text: 'Picked up. Read precedent: skill_comment_guidance.rs (test pattern), record-progress.md (partial precedent), task-standards.md + task/SKILL.md (modifying). Wrote crates/swissarmyhammer-skills/tests/task_double_check_guidance.rs mirroring skill_comment_guidance.rs. RED confirmed: 2 tests fail (no partial yet, no code_context guidance in rendered body). Canonical sentence chosen: "Do NOT spawn the diff-oriented `double-check` agent to verify a task card". Next: create builtin/_partials/task-double-check.md + add include to task/SKILL.md.'
  timestamp: 2026-06-17T05:39:53.983751+00:00
- actor: wballard
  id: 01kva1rf1hegyj9s4tr38fbjwy
  text: 'GREEN. Created builtin/_partials/task-double-check.md (partial: true) as single source of truth; added {% include \"_partials/task-double-check\" %} to builtin/skills/task/SKILL.md after the task-standards include. Verification (run fresh, full output read):\n- cargo test -p swissarmyhammer-skills: 115 + 2 + 2 + 2 passed, 0 failed (new task_double_check_guidance.rs: 2 passed).\n- cargo test -p swissarmyhammer-templating: 241 unit + all_skills_render_test 1 + skills_rendering_test 3 + 21 doc passed, 0 failed. Task skill still renders with the new include.\nNote: crates/swissarmyhammer-mcp-proxy/.kanban is pre-existing (committed Apr in 375d20b16), not a stray I created. Moving to review.'
  timestamp: 2026-06-17T05:44:40.753499+00:00
- actor: claude-code
  id: 01kva2h379ctqh96w5crvxg04f
  text: |-
    Addressed both review findings.

    Finding 1 (helper duplication): Extracted the four helpers (builtin_dir, strip_frontmatter, expand_partials, rendered_builtin_instructions) into a shared `crates/swissarmyhammer-skills/tests/common/mod.rs` as `pub` fns, following the repo's existing tests/common/mod.rs convention (claude-agent, acp-conformance use the same `mod common;` pattern). Both task_double_check_guidance.rs and skill_comment_guidance.rs now do `mod common;` + `use common::{builtin_dir, rendered_builtin_instructions};`. One source of truth; no duplication.

    Finding 2 (data-driven assertions): Converted the single-marker `assert!(body.contains(...))` calls in task_double_check_guidance.rs into a `required_markers: &[(&str, &str)]` table (marker, requirement) looped over with one assertion, mirroring the sym_op/section loops. Sizing limits split into two rows (5 files / 5 subtasks) so coverage is identical. The manual-verification check stays a distinct assertion because it is an OR (either "manually verify" OR "smoke test" suffices) — folding it into the AND-table would wrongly require both and weaken nothing of value while risking a false failure. No assertion weakened; all markers + canonical sentence still covered.

    Verification (run fresh, full output read):
    - cargo test -p swissarmyhammer-skills: 115 + 2 + 2 + 2 passed, 0 failed; no warnings. skill_comment_guidance (2) and task_double_check_guidance (2) both green with the shared module.
    - cargo test -p swissarmyhammer-templating: 241 unit + all_skills_render 1 + skills_rendering 3 + 21 doc passed, 0 failed.

    Both findings checked off. Moving back to review.
  timestamp: 2026-06-17T05:58:07.849315+00:00
- actor: claude-code
  id: 01kva3mxry3brgje48hdf3767x
  text: |-
    Addressed review finding (2026-06-17 00:58): single-source-of-truth test duplication.

    Extracted a parameterized helper `assert_guidance_single_source(canonical_text: &str, expected_rel_path: &Path)` into crates/swissarmyhammer-skills/tests/common/mod.rs. It walks builtin/, collects every .md file containing canonical_text, and asserts hits == exactly [expected_rel_path], with a panic message listing offending files on failure (semantics preserved, no coverage weakened). Both single-source tests now call it:
    - task_double_check_guidance_has_single_source_of_truth -> assert_guidance_single_source(CANONICAL_GUIDANCE, "_partials/task-double-check.md")
    - record_progress_guidance_has_single_source_of_truth -> assert_guidance_single_source(CANONICAL_GUIDANCE, "_partials/record-progress.md")
    Both files now import `assert_guidance_single_source` from common and dropped the now-unused `builtin_dir`/PathBuf imports (switched to `std::path::Path`).

    Verification (run fresh, full output read):
    - cargo test -p swissarmyhammer-skills: 115 + 2 + 2 + 2 passed, 0 failed, 0 warnings. skill_comment_guidance (2) and task_double_check_guidance (2) both green via the shared helper.
    - cargo test -p swissarmyhammer-templating: 241 unit + all_skills_render 1 + skills_rendering 3 + 21 doc passed, 0 failed.

    Finding checked off. Moving back to review.
  timestamp: 2026-06-17T06:17:41.918950+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffb680
title: Add a double-check verification step to the `task` skill
---
## What

The `task` skill (`builtin/skills/task/SKILL.md`) creates a kanban card but never verifies its own output before reporting done, so it misses things — stale/hallucinated file paths, missing template sections, vague acceptance criteria, manual-verification tests, oversized scope. Add a **double-check / self-verification step** that runs after the card is drafted and persisted, before the skill reports back.

**Mechanism (match the record-progress precedent):** put the guidance in ONE shared partial and `{% include %}` it into the task skill, the same way `builtin/_partials/record-progress.md` is pulled into implement/finish/kanban. Create `builtin/_partials/task-double-check.md` and add `{% include "_partials/task-double-check" %}` to `builtin/skills/task/SKILL.md` (after the existing `task-standards` include). Scope it to the `task` skill only for now — `plan` can opt in later.

**What the step must instruct** (adversarial self-review of the freshly created card):
- Re-read the created card with `get task`.
- Verify every file path, function, and type named in the card actually exists — via the `code_context` MCP tool (`search symbol` / `get symbol`) or Glob/Read. Catches stale or invented references.
- Confirm all required sections are present: `## What`, `## Acceptance Criteria`, `## Tests`, `## Workflow`.
- Confirm the `Tests` section is automated — no "manually verify / smoke test / user confirms" language (the partial already forbids this; verify the produced card honors it).
- Confirm sizing limits hold (≤5 files, ≤5 subtasks, one concern).
- Confirm acceptance criteria are observable, not vague.
- Fix-and-re-verify loop: on any failure, `update task` to correct it, then re-check. Report only after it passes.

**Important design constraint:** do NOT reach for the existing code-oriented `double-check` *agent* (the one really-done spawns). That agent verifies a git diff and **skips when there is no diff** — a freshly created task card has no diff, so it is the wrong tool. This is a task-appropriate verification checklist, not the diff critic.

### Source of truth
- `.skills/` is generated — never edit there. Edit `builtin/` only. (See repo memory: skills source of truth is `builtin/skills/<name>/SKILL.md`.)

## Acceptance Criteria
- [x] New partial `builtin/_partials/task-double-check.md` exists (frontmatter `partial: true`) holding the double-check guidance as the single source of truth.
- [x] `builtin/skills/task/SKILL.md` includes it via `{% include "_partials/task-double-check" %}`; the rendered task body carries the guidance.
- [x] Guidance covers: re-read card, verify paths/symbols exist via code_context, all four sections present, tests automated, sizing limits, fix-and-re-verify loop.
- [x] Guidance explicitly says NOT to spawn the diff-oriented `double-check` agent for task verification.
- [x] The canonical guidance sentence appears in exactly one `builtin/` file (the new partial) — no duplication.

## Tests
- [x] Add `crates/swissarmyhammer-skills/tests/task_double_check_guidance.rs`, mirroring the existing `crates/swissarmyhammer-skills/tests/skill_comment_guidance.rs`: resolve the builtin `task` skill, expand partials, and assert the rendered body contains the double-check markers (e.g. mentions re-reading the card, verifying paths exist, and a unique canonical sentence). Add a single-source-of-truth test asserting the canonical sentence lives ONLY in `_partials/task-double-check.md`.
- [x] `cargo test -p swissarmyhammer-skills` → green (new test passes).
- [x] `cargo test -p swissarmyhammer-templating` → green (the `all_skills_render_test` / `skills_rendering_test` still render the task skill with the new include).

## Workflow
- Use `/tdd` — write the `task_double_check_guidance.rs` assertions first (they fail because the include/partial don't exist yet), then add the partial + include to make them pass.

## Review Findings (2026-06-17 00:45)

### Warnings
- [x] `crates/swissarmyhammer-skills/tests/task_double_check_guidance.rs:23` — Four helper functions (`builtin_dir`, `strip_frontmatter`, `expand_partials`, `rendered_builtin_instructions`) are exact duplicates of functions already defined in another test file. Keeping one canonical source prevents divergence when these helpers are refined. Extract the four helper functions to a shared test utility module (e.g., `crates/swissarmyhammer-skills/tests/common/builtin_helpers.rs`), then import them in both `task_double_check_guidance.rs` and `skill_comment_guidance.rs`. This ensures one source of truth for test infrastructure and avoids maintaining two copies.
- [x] `crates/swissarmyhammer-skills/tests/task_double_check_guidance.rs:108` — Hardcoded string literals ("get task", "code_context", "## What", "5 files", etc.) are being checked via repeated `assert!(body.contains(...))` calls. This is a table of requirements that should be data, not parallel assertion code paths. The existing `for sym_op in [...]` loop at line 115 already demonstrates the pattern this test should follow throughout. Define a vec of requirement tuples (string to find, description/purpose) and loop over it with a single assertion, similar to the `sym_op` loop. Group related requirements (e.g., section headers, sizing limits) into logical requirement sets if needed. This eliminates duplication and makes adding/removing requirements a data change, not code change.

## Review Findings (2026-06-17 00:58)

### Warnings
- [x] `crates/swissarmyhammer-skills/tests/task_double_check_guidance.rs:79` — task_double_check_guidance_has_single_source_of_truth duplicates record_progress_guidance_has_single_source_of_truth nearly identically (0.99 similarity). Both walk builtin/, search for a canonical guidance string, and assert it appears in exactly one file. The only variables are the guidance string and expected path — parameterize into a single shared helper. Extract a parameterized helper `assert_guidance_single_source(canonical_text: &str, expected_path: &Path)` in common/mod.rs; call it from both tests with their respective guidance strings and paths.