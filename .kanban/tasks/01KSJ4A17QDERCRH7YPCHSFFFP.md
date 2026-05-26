---
assignees:
- claude-code
position_column: todo
position_ordinal: '8780'
title: Decide + document ProjectStructure user-scope behavior
---
## What

`ProjectStructure` (`apps/swissarmyhammer-cli/src/commands/install/components/mod.rs`) creates `.sah/` and `.prompts/` under the git root in `Project` / `Local` scope and is skipped entirely in `User` scope (its `is_applicable` matches only `Project | Local`). That is plausibly correct — sah's runtime state is project-local — but the rationale is undocumented, and a future reader will reasonably wonder "shouldn't this also create `~/.sah/` for user-mode runtime state?"

Either:
1. **Document the decision.** Add a doc comment on `ProjectStructure` (and a one-line note in `commands::registry`'s `register_all` priority table) explaining that user scope intentionally does NOT create a global `~/.sah/`/`~/.prompts/` because user-mode is purely a per-agent config install (skills, agents, preamble, settings) with no shared runtime artifacts; runtime state belongs in the project tree.
2. **Or, if there is a genuine global runtime need** — e.g. a global prompts override directory — add a `GlobalUserStructure` component (priority 40, applicable to User) that creates the relevant `~/.…` dirs.

Pick option 1 unless someone can point at concrete consumers of a `~/.sah/` directory.

## Acceptance Criteria
- [ ] `ProjectStructure`'s doc comment explains the User-skip rationale, OR a new User-scope structure component is added with tests.
- [ ] The registry priority table in `commands::registry::register_all` mentions the same rationale.
- [ ] No behavior change unless option 2 is taken; if option 2, an isolated-HOME test asserts the global dirs are created.

## Tests
- [ ] If option 1: a doc-style assertion is sufficient (no new runtime tests). If option 2: `#[serial_test::serial(home_env)]` test asserting the global dirs are created.

## Workflow
- Investigate first (grep for any callers expecting `~/.sah/` or `~/.prompts/`); pick option 1 or 2 based on the evidence. #init-doctor