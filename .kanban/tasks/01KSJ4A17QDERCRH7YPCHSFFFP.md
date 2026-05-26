---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8880
title: Decide + document ProjectStructure user-scope behavior
---
## What

`ProjectStructure` (`apps/swissarmyhammer-cli/src/commands/install/components/mod.rs`) creates `.sah/` and `.prompts/` under the git root in `Project` / `Local` scope and is skipped entirely in `User` scope (its `is_applicable` matches only `Project | Local`). That is plausibly correct — sah's runtime state is project-local — but the rationale is undocumented, and a future reader will reasonably wonder "shouldn't this also create `~/.sah/` for user-mode runtime state?"

Either:
1. **Document the decision.** Add a doc comment on `ProjectStructure` (and a one-line note in `commands::registry`'s `register_all` priority table) explaining that user scope intentionally does NOT create a global `~/.sah/`/`~/.prompts/` because user-mode is purely a per-agent config install (skills, agents, preamble, settings) with no shared runtime artifacts; runtime state belongs in the project tree.
2. **Or, if there is a genuine global runtime need** — e.g. a global prompts override directory — add a `GlobalUserStructure` component (priority 40, applicable to User) that creates the relevant `~/.…` dirs.

Pick option 1 unless someone can point at concrete consumers of a `~/.sah/` directory.

## Decision: Option 1 (documentation only)

Investigated by grepping the workspace for callers expecting `~/.sah/` or `~/.prompts/`. Found readers in `swissarmyhammer-config/src/discovery.rs`, `swissarmyhammer-tools/src/health_registry.rs`, `swissarmyhammer-tools/src/mcp/tool_config.rs`, and `swissarmyhammer-statusline/src/config.rs` — but every one treats those paths as optional, lazy fallbacks (missing-is-fine). Other User-scope install components (`Statusline`, `AgentDeployment`) already create the `~/.sah/` subdirs they need on demand. No consumer requires an empty `~/.sah/` or `~/.prompts/` to be pre-created, so option 1 (document-and-stay-put) is correct.

## Acceptance Criteria
- [x] `ProjectStructure`'s doc comment explains the User-skip rationale, OR a new User-scope structure component is added with tests.
- [x] The registry priority table in `commands::registry::register_all` mentions the same rationale.
- [x] No behavior change unless option 2 is taken; if option 2, an isolated-HOME test asserts the global dirs are created.

## Tests
- [x] If option 1: a doc-style assertion is sufficient (no new runtime tests). If option 2: `#[serial_test::serial(home_env)]` test asserting the global dirs are created.

## Workflow
- Investigate first (grep for any callers expecting `~/.sah/` or `~/.prompts/`); pick option 1 or 2 based on the evidence. #init-doctor

## Review Findings (2026-05-26 13:05)

### Warnings
- [x] `apps/swissarmyhammer-cli/src/commands/registry.rs:18-31` — Acceptance criterion 2 says "The registry priority table in `commands::registry::register_all` mentions the same rationale," but the priority table in `commands::registry::register_all` is unchanged. The user-skip rationale was added to a NEW priority table in `components::register_all` (`apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:17-47`), not in the function the criterion names. Either update the `commands::registry::register_all` doc to add a one-line note like `priority 20: ProjectStructure — Project/Local only (skipped in User scope; see ProjectStructure docs for rationale)`, or amend the acceptance criterion to point at the new table's actual home.
  - Resolved: Moved the full priority table (now with a `User` column) and the "Why ProjectStructure skips User scope" section into the doc block above `commands::registry::register_all`. Reduced the table in `components::register_all` to a one-paragraph pointer to the canonical table, making the source of truth unambiguous.

### Nits
- [x] `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:23-32` — The new Markdown priority table lists 8 components but omits `KanbanTool` (which the existing `commands::registry` table flags as `default`). A reader cross-referencing the two tables will wonder why one has a `KanbanTool` row and the other does not. Either add a `default | KanbanTool | y | tool lifecycle, no-op for init/deinit` row, or add a one-line note below the table saying "Also registered: `KanbanTool` (no-op lifecycle)".
  - Resolved: The canonical table now lives on `commands::registry::register_all` and includes a `default | KanbanTool | y | Tool lifecycle, no-op for init/deinit` row. The duplicate table in `components::register_all` was removed, so there is no longer a divergence to reconcile.
- [x] `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:23-32` — Priority numbers jump from 22 (ClaudeMd) to 31 (AgentDeployment) with no mention that priority 30 (`SkillDeployment`) is registered separately in `commands::registry::register_all`. Add a footnote like "(priority 30 `SkillDeployment` is registered by `commands::registry::register_all`)" so the gap is not surprising.
  - Resolved: The canonical table on `commands::registry::register_all` now includes a `30 | SkillDeployment | y | Builtin skill deployment via mirdan` row and a paragraph explaining that `SkillDeployment` is registered directly by `commands::registry::register_all`, while priorities 10–32 (except 30) plus `KanbanTool` come from `super::install::components::register_all`. The reduced doc on `components::register_all` makes the same point inline.