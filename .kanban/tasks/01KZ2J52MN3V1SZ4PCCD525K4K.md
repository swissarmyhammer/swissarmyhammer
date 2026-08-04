---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz6yv65wnvcfke1rz2v8zs72
  text: |-
    ### implement — changed
    - evidence: Investigated whether `.skills/` is read at runtime before deciding (per instructions): `SkillResolver::new()` (crates/swissarmyhammer-skills/src/skill_resolver.rs) resolves project-local skills only from `{git_root}/.skills`; `git_root` for `apps/kanban-cli/` and `apps/code-context-cli/` is this repo's root, never those subdirectories. Confirmed via `git ls-files -s` that the three named `.skills/*/SKILL.md` files are mode 100644 real files, plus six more tracked mode-120000 symlinks (`apps/kanban-cli/.claude/skills/kanban`, `apps/kanban-cli/.zed/skills/kanban`, `apps/code-context-cli/.claude/skills/{code-context,lsp}`, `apps/code-context-cli/.zed/skills/{code-context,lsp}`) all pointing at `../../.skills/<name>` — the full output of someone running `kanban init`/`code-context skill` directly inside those source dirs and committing the result. Decided: untrack (not regenerate+guard), since these are dead artifacts with zero runtime purpose, not just drifted ones.
      - `git rm` all 9 paths (3 files + 6 symlinks).
      - `.gitignore`: widened `/.skills/`, `/.agents/`, `/.claude/skills/`, `/.claude/agents/`, `/.zed/skills/` (repo-root-anchored) to `.skills/`, `.agents/`, `**/.claude/skills/`, `**/.claude/agents/`, `**/.zed/skills/` (recursive), matching the existing `**/.validators/` precedent already in the file, so a local deploy run inside any `apps/*-cli/` dir is ignored instead of accidentally `git add`-able.
      - Added `crates/swissarmyhammer-skills/tests/no_committed_skill_deploy_artifacts.rs`: greps `git ls-files apps` for these path shapes. Verified RED (test failed listing all 9 paths) against the pre-removal tree, then GREEN after `git rm`.
      - Left a comment on ^qg2h0ta (the related "regenerate the llama-agent wording" card) noting it's superseded by this decision.
      - `cargo nextest run -p swissarmyhammer-skills`: 126 passed, 0 skipped.
      - `cargo nextest run -p kanban-cli -p code-context-cli -p mirdan`: 570 passed, 0 skipped.
      - `cargo nextest run --workspace`: 13499 passed, 0 skipped.
      - `cargo clippy -p swissarmyhammer-skills --tests --all-targets`: clean.
    - next: ready for review
    task: ^d525k4k
  timestamp: 2026-08-04T17:58:48.252460+00:00
- actor: claude-code
  id: 01kz6z172xc84adp63gmf03a1w
  text: |-
    ### test — green
    - evidence: `cargo nextest run --workspace` — 13499 tests run: 13499 passed, 0 skipped (114.079s). `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean, no warnings.
    - confirmed: new regression test `swissarmyhammer-skills::no_committed_skill_deploy_artifacts no_generated_skill_deploy_artifacts_tracked_under_apps` ran and passed.
    - next: none. Build is clean.
  timestamp: 2026-08-04T18:02:05.789932+00:00
position_column: doing
position_ordinal: '8380'
title: Three committed .skills/ snapshots have drifted from builtin/skills/
---
## What

Three generated `.skills/` files are tracked in git and have drifted far from
their `builtin/skills/` sources:

- `apps/kanban-cli/.skills/kanban/SKILL.md`
- `apps/code-context-cli/.skills/code-context/SKILL.md`
- `apps/code-context-cli/.skills/lsp/SKILL.md`

`diff builtin/skills/kanban/SKILL.md apps/kanban-cli/.skills/kanban/SKILL.md`
reports dozens of differing paragraphs. Neither `apps/kanban-cli/build.rs` nor
`apps/code-context-cli/build.rs` writes them, so nothing regenerates them on a
build. They are a runtime deploy artifact that was committed once and left.

Found while card ^3y5n9g6 deleted the `llama-agent` crate:
`apps/kanban-cli/.skills/kanban/SKILL.md:19` still tells users the kanban board
is "the single source of truth across Claude Code and llama-agent sessions".
The `builtin/skills/kanban/SKILL.md` source was corrected there; the snapshot
could not be, because `.skills/` must never be hand-edited.

Decide and act:

- Regenerate all three through the real deploy path
  (`swissarmyhammer-skills::deploy`), which re-renders each SKILL.md from
  `parse_skill_md` -> `format_skill_md` with template variables resolved — a
  raw copy is NOT equivalent, and

- add a test that fails when a committed snapshot drifts from its source, or

- stop tracking `.skills/` and add it to `.gitignore`.

Pick one. A snapshot that nothing regenerates and nothing checks will drift
again.

## Decision: untrack

Verified before deciding: `SkillResolver` (`crates/swissarmyhammer-skills/src/skill_resolver.rs`)
only resolves project-local skills from `{git_root}/.skills`, and `{git_root}`
for every crate under `apps/*-cli/` in this workspace is the repository root
(`/Users/wballard/github/swissarmyhammer/swissarmyhammer-main`), never the
`apps/kanban-cli/` or `apps/code-context-cli/` subdirectory. So these three
files (plus their `.claude/skills/`, `.zed/skills/` symlink siblings — found
during investigation, six more tracked paths beyond the three named above) are
never read by any running binary. They are pure leftovers from someone running
`kanban init` / `code-context skill` directly inside those source
directories, whose output then got `git add`-ed because the repo-root
`.gitignore` rules (`/.skills/`, `/.claude/skills/`, `/.zed/skills/`, ...) were
anchored to the repo root only and did not cover nested `apps/*` directories.

Untracking is the correct fix, not regeneration: regeneration would keep
committing a runtime deploy artifact with zero runtime purpose, forever
needing a guard test to catch drift. Untracking removes the artifact and the
class of bug at once.

### Subtasks
- [x] Decide: regenerate + guard, or untrack. -> untrack.
- [x] Apply the decision to all three files (plus the six symlink siblings
      found during investigation: `apps/kanban-cli/.claude/skills/kanban`,
      `apps/kanban-cli/.zed/skills/kanban`,
      `apps/code-context-cli/.claude/skills/code-context`,
      `apps/code-context-cli/.claude/skills/lsp`,
      `apps/code-context-cli/.zed/skills/code-context`,
      `apps/code-context-cli/.zed/skills/lsp`).

## Acceptance Criteria
- [x] Either every committed `.skills/*/SKILL.md` matches what the deploy path
      produces from its `builtin/skills/` source, or no `.skills/` file is
      tracked. -> no `.skills/`/`.claude/skills/`/`.zed/skills/` file is
      tracked under `apps/kanban-cli/` or `apps/code-context-cli/` anymore.
- [x] If they stay tracked, a test fails when one drifts. -> N/A (untracked),
      but added `crates/swissarmyhammer-skills/tests/no_committed_skill_deploy_artifacts.rs`,
      which fails if any such artifact is ever re-committed under `apps/`
      (via `git ls-files`, RED verified against the pre-removal tree then
      GREEN after removal). Also widened `.gitignore` patterns
      (`.skills/`, `.agents/`, `**/.claude/skills/`, `**/.claude/agents/`,
      `**/.zed/skills/`) from repo-root-anchored to recursive, so a future
      local deploy run inside any `apps/*-cli/` directory is ignored instead
      of `git add`-able by accident.

## Tests
- [x] Run `cargo nextest run -p swissarmyhammer-skills`. -> 126 passed, 0 skipped.
- [x] Run `cargo nextest run --workspace`. -> 13499 passed, 0 skipped.

Related: ^qg2h0ta ("Regenerate the deployed .skills/ copies so they lose the
llama-agent wording") assumed the regenerate path; superseded by this card's
untrack decision — see comment left there. #bug #cleanup #docs #skills