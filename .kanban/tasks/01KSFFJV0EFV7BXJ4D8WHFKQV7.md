---
assignees:
- claude-code
depends_on:
- 01KSFFHM968X2RXQ4TZQNPVAT1
- 01KSFFJCKN0WV1C5D9MQ72VVWY
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8380
title: 'Fix `sah init user`: install preamble into each agent''s global instructions file'
---
## What

`sah init user` does not create `~/.claude/CLAUDE.md`. Root cause in `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs`: the `ClaudeMd` component's `is_applicable` returns `false` for `InitScope::User`, so it is skipped entirely in user scope; and even when it runs (project/local), it writes to `find_git_repository_root()` unconditionally — there is no global-path branch.

Make the preamble component agent-agnostic and scope-aware, driven by the `AgentDef` instructions paths added in the AgentDef card:
- `is_applicable` returns `true` for `Project`, `Local`, and `User`.
- `init`/`deinit` load the agents config (`mirdan::agents::load_agents_config` + `get_detected_agents`) and, for each detected agent, resolve the instructions file for the scope:
  - `User` → `agent_global_instructions_file` (absolute, e.g. `~/.claude/CLAUDE.md`).
  - `Project`/`Local` → `agent_project_instructions_file` resolved **relative to the git root** (so Claude Code keeps writing `<git-root>/CLAUDE.md` exactly as today). If no git root in project scope, report a Warning as the current code does.
  - Agents whose instructions path is `None` for the scope are skipped (not applicable).
- Reuse the existing `ensure_*_preamble`/`remove_*_preamble` file logic, but generalize them to take a full file `Path` (the instructions file), not a root dir, so they work for both the project `CLAUDE.md` and the absolute global file. Keep using the `CLAUDE_MD_PREAMBLE`/`PREAMBLE_MARKER` constant (now sourced from mirdan per the status-API card).
- Keep the reporter Action/Skipped/Warning messages, now per-agent and per-path.

Note: today only `claude-code` has instructions paths populated, so behavior change is: user scope now writes `~/.claude/CLAUDE.md`, project scope unchanged. The mechanism is generic so future agents Just Work.

## Acceptance Criteria
- [x] `sah init user` writes the preamble to `~/.claude/CLAUDE.md` (creating the file + parent dir if needed); the first non-empty line is the preamble marker.
- [x] `sah init` (project) still ensures `<git-root>/CLAUDE.md` has the preamble — no regression.
- [x] `sah deinit user` removes the preamble from `~/.claude/CLAUDE.md` (deleting the file if it only held the preamble).
- [x] Component is idempotent across scopes (running twice makes no second change).

## Tests
- [x] Add a `#[serial_test::serial(cwd)]` test using `IsolatedTestEnvironment` (isolated HOME) that runs the preamble component's `init` with `InitScope::User` and asserts `~/.claude/CLAUDE.md` exists with the marker as its first non-empty line; then `deinit` and assert it is removed.
- [x] Keep the existing project-scope `ensure_claude_md_preamble`/`remove_claude_md_preamble` unit tests green (refactor them to the path-based signature if the helper signature changes).
- [x] Add a regression test that `is_applicable(&InitScope::User)` is `true`.
- [x] `cargo test -p swissarmyhammer-cli` for the install/components module runs green.

## Workflow
- Use `/tdd` — write the `init user` → `~/.claude/CLAUDE.md` test first (fails today), then generalize the component. #init-doctor

## Implementation Notes
- Renamed the three file helpers to take a full file `Path` instead of a root dir: `claude_md_has_preamble` → `preamble_file_has_preamble`, `ensure_claude_md_preamble` → `ensure_preamble` (now also `create_dir_all`s the parent so it works for `~/.claude/CLAUDE.md`), `remove_claude_md_preamble` → `remove_preamble`.
- `ClaudeMd::init`/`deinit` now delegate to a private `for_each_agent_path` that loads detected agents, resolves the per-scope instructions file (via new `resolve_instructions_file`), and applies a per-agent action (`ensure_preamble_for_agent` / `remove_preamble_for_agent`) that emits per-agent reporter events. Mirrors the existing `register_agent_mcp` agent-iteration structure.
- Project/Local scope joins `agent_project_instructions_file` (CLAUDE.md) onto the git root — identical to prior `<git-root>/CLAUDE.md` behavior; missing git root in those scopes returns an error + Warning as before.
- New tests: `test_claude_md_creates_parent_dirs`, `test_claude_md_is_applicable_user_scope`, `test_claude_md_user_scope_writes_global_file` (uses `IsolatedTestEnvironment` + `NullReporter`, `#[serial_test::serial(home_env)]`). Existing helper tests refactored to the path-based signatures.
- Verified: 50 install lib tests green, 10 doctor lib tests green (no regression), clippy clean, rustfmt clean. Doctor module and mirdan crate were NOT edited per task scope.