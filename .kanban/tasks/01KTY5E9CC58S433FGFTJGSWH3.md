---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffa980
project: card-comments
title: Consolidate add-comment skill guidance into a shared partial; record failures and discoveries
---
## What
The comment-guidance card (^xqz8cgx) inlined three near-identical "Record progress" sections into `builtin/skills/{implement,finish,kanban}/SKILL.md`. Consolidate them into ONE shared partial — the repo's existing mechanism for exactly this (`builtin/_partials/` + Liquid `{% include %}`; these same skills already include `_partials/short-ids`, `_partials/review-column`, etc.) — and enrich WHAT gets recorded.

### Content requirement (the point of this card)
The current guidance only says "record what happened" at milestones. The conversation log's value is institutional memory for the NEXT agent on the card, so the partial must explicitly instruct recording:
- **Milestones** — picked up, research done, implementation landed, moved to review (the existing guidance).
- **What did NOT work** — failed approaches, dead ends, reverted attempts, and WHY they failed, so the next agent doesn't burn the same tokens repeating them.
- **Interesting discoveries** — surprising behavior, latent bugs found along the way, non-obvious constraints, useful context that isn't in the card description.
- **Blockers** — what's blocking and what was tried.
Plus: read prior context with `list comments` before starting a card.

### Files
1. NEW `builtin/_partials/record-progress.md` — the canonical guidance: the `{"op": "add comment", "task_id": "<id>", "text": "..."}` call, the record-what list above (milestones / didn't-work / discoveries / blockers), `list comments` for prior context, and the note that the author is attributed automatically to the dispatching actor. Write it VOICE-NEUTRAL so it reads correctly inside all three skills (worker on one task, orchestrator loop, general pick-up-a-card flow). Match the prose style of existing partials (see `_partials/short-ids.md`, `_partials/review-column.md`).
2. `builtin/skills/implement/SKILL.md` — replace the inline "### Record progress" section body with at most a one-line skill-specific lead-in + `{% include "_partials/record-progress" %}`.
3. `builtin/skills/finish/SKILL.md` — same replacement for "### Record progress (both modes)" (keep the orchestrator-specific lead-in line: log each iteration/state transition on the task being driven).
4. `builtin/skills/kanban/SKILL.md` — replace the step-5 inline guidance with the include (or a one-line step that points at it); keep the `list comments` suggestion (it can move into the partial since all three benefit).
5. `crates/swissarmyhammer-skills/tests/skill_comment_guidance.rs` — extend the existing test: the RENDERED bodies (post-include expansion via `SkillResolver::resolve_builtins()`) of all three skills must contain the add-comment call AND distinctive phrases proving the failures + discoveries guidance is present (pick stable canonical phrases from the partial, e.g. "did not work" / "discoveries"). Also assert the guidance text exists exactly once in `builtin/` sources (single source of truth — a small fs-walk assertion that the canonical sentence appears only in `_partials/record-progress.md`).
6. Regenerate deployments after editing sources: `cargo run -p kanban-cli -- init` (NEVER hand-edit `.skills/`, `.claude/skills/`, `.zed/skills/`, `.sah/skills/`, `apps/kanban-cli/.skills/`).

## Acceptance Criteria
- [x] `builtin/_partials/record-progress.md` exists and is the ONLY place in `builtin/` containing the canonical guidance text.
- [x] All three skills include it via `{% include "_partials/record-progress" %}`; their rendered (deployed) bodies contain the expanded guidance.
- [x] The guidance explicitly instructs recording failed approaches/dead ends (with why), interesting discoveries, blockers, and milestones — and reading `list comments` before starting.
- [x] Deployments regenerated from source; no generated dir hand-edited.
- [x] `cargo nextest run -p swissarmyhammer-skills` — green.

## Tests
- [x] Extend `crates/swissarmyhammer-skills/tests/skill_comment_guidance.rs`: rendered implement/finish/kanban bodies contain the add-comment op AND the didn't-work + discoveries phrases; single-source-of-truth assertion on `builtin/`.
- [x] `cargo nextest run -p swissarmyhammer-skills` — all green.

## Workflow
- Use `/tdd` — extend the failing rendered-content assertions first, then create the partial and swap the includes.

## Implementation notes (2026-06-12)
- TDD: rewrote `skill_comment_guidance.rs` first (RED: both tests failed for the expected reasons), then created the partial and swapped the includes (GREEN).
- `SkillResolver::resolve_builtins()` does NOT expand Liquid includes (the skills crate has no templating dep; expansion happens at render/deploy time in prompts/tools). The test expands `{% include "_partials/..." %}` itself by reading the partial sources from disk and stripping frontmatter — the production-path render is separately covered by `all_skills_render_test.rs` (prompts) and `skill_e2e.rs` (tools), both green.
- Canonical single-source sentence asserted: "burn the same tokens repeating them".
- kanban skill: step 3's inline `list comments` text and the Guidelines "Blocked or unclear" add-comment line moved into the partial too; a new `### Record progress` section after the Process list holds the include.
- Verified: 280 tests green across swissarmyhammer-skills + swissarmyhammer-prompts; 12 skill_e2e tests green in swissarmyhammer-tools; deployments regenerated (`cargo run -p kanban-cli -- init`), expanded guidance present in `.skills/`, `.claude/skills/`, `.zed/skills/`, no raw include tags leak.
- Note: this session's kanban MCP server predates comment ops (`list comments` errors as unsupported), so progress was recorded here in the description instead of the comment thread.