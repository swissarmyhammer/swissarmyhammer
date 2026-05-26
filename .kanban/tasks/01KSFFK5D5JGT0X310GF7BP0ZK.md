---
assignees:
- claude-code
depends_on:
- 01KSFFJCKN0WV1C5D9MQ72VVWY
- 01KSFFHT748VD2DNVVXW1NTYCC
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8580
title: 'Doctor: agent-agnostic install-stack checks (project + user) via mirdan status'
---
## What

Replace the doctor's hand-coded, project-only, Claude-specific config checks with a single generic loop over `mirdan::status::check_all`, so `sah doctor` reports the full install stack — skills, preamble, permissions, MCP, agents — for **both project and user scope**, agent-agnostically. This is the "smart not copy-paste" payoff.

In `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs` add:
- `pub fn check_install_stack(checks: &mut Vec<Check>) -> Result<()>` that:
  - loads `mirdan::agents::load_agents_config()`,
  - calls `mirdan::status::check_all(&config, &[InitScope::Project, InitScope::User])`,
  - for each `ComponentStatus`, skips `ComponentState::NotApplicable`, and pushes `mirdan::status::to_check(&status)` for the rest, with a check name that encodes agent + scope + component (e.g. `"Claude Code · user · Preamble"`) so project and user rows are distinguishable.
  - On config-load error, push a single Error check (mirror `MirdanDoctor::check_agents_detected`).

Wire it into `apps/swissarmyhammer-cli/src/commands/doctor/mod.rs`:
- Call `checks::check_install_stack(&mut self.checks)?` from `run_system_checks` (or a new `run_install_stack_checks`).
- **Remove** the now-redundant bespoke `check_claude_md` / `run_configuration_checks` CLAUDE.md path (the Preamble component covers it for both scopes). Keep `check_claude_config` (the `claude mcp list` runtime probe) since it validates the agent actually loads sah — it is complementary, not duplicative. Delete `check_claude_md`/`check_claude_md_at` and their tests, or repoint them at the stack; do not leave two code paths reporting the preamble.

The stack must run regardless of git repo presence (it relies on the doctor-no-fail card so user-scope checks surface even from `~`).

## Acceptance Criteria
- [x] `sah doctor` output includes one row per applicable (agent, scope, component); for a machine with Claude Code, both `project` and `user` rows appear for Preamble, Permissions, Skills, Agents, MCP.
- [x] NotApplicable combinations are not shown as failures (skipped entirely).
- [x] Missing artifacts render as Warning with an actionable fix (`sah init` / `sah init user`); installed render as Ok.
- [x] No duplicate CLAUDE.md/preamble check remains (old `check_claude_md` removed or repointed).
- [x] `cargo build -p swissarmyhammer-cli` is green.

## Tests
- [x] Add a `#[serial_test::serial(cwd)]` test (isolated HOME + CWD) that writes a fake detectable Claude Code layout into the temp HOME (e.g. create `~/.claude/`), installs some artifacts (e.g. `~/.claude/CLAUDE.md` with the marker) and leaves others missing, runs `check_install_stack`, and asserts: a user-scope Preamble check is `Ok`, a user-scope Permissions check is `Warning`, and both project and user rows are present.
- [x] Assert NotApplicable rows (e.g. an agent with no settings path) produce no check.
- [x] `cargo test -p swissarmyhammer-cli doctor` runs green.

## Workflow
- Use `/tdd` — write the stack-output test first, then wire `check_install_stack` and delete the redundant check. #init-doctor

## Review Findings (2026-05-25 11:20)

Reviewed: `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs`, `apps/swissarmyhammer-cli/src/commands/doctor/mod.rs`, and their tests. Build green (`cargo build -p swissarmyhammer-cli`), clippy clean on the doctor files, and `cargo test -p swissarmyhammer-cli doctor` passes 12/12 (incl. the three install-stack tests). All acceptance criteria verified, old `check_claude_md`/`check_claude_md_at` and tests fully removed (no remaining references). The implementation is clean, well-documented, and the test environment correctly redirects `dirs::home_dir()` into the isolated HOME so it is a genuine end-to-end exercise of `load_agents_config` + `check_all` (not fixture-only).

### Warnings
- [x] `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs:742` — `test_check_install_stack_skips_not_applicable` never calls the production `check_install_stack`; it builds a synthetic bare-agent `AgentsConfig`, then re-implements `check_install_stack`'s skip loop locally (lines 786-792) and asserts against that copy. Because `check_install_stack` loads the real config (no injection seam) and, under the isolated `~/.claude`-only HOME of test 1, only `claude-code` is detected (which defines all 5 components at both scopes), the production function's `if status.state == NotApplicable { continue; }` branch is not exercised by any test that actually invokes it — the test name implies coverage it doesn't provide. Suggest either giving `check_install_stack` a config-injectable inner helper (e.g. `check_install_stack_with(config, checks)`) and asserting the bare-agent NotApplicable rows are absent from its output, or detecting a second skills+mcp-only agent in the isolated HOME so test 1's real call yields NotApplicable rows to assert on.
  - RESOLVED: Extracted a config-injectable inner helper `check_install_stack_with(config, checks)` holding the real skip-and-convert loop; `check_install_stack` now loads the host config and delegates to it. `test_check_install_stack_skips_not_applicable` now invokes `check_install_stack_with(&bare_config, …)` — the actual production NotApplicable branch — and asserts (a) every NotApplicable status's would-be check name is absent, and (b) the emitted count equals the applicable-status count (so the absence is real filtering, not an empty result).

### Nits
- [x] `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs:508` — The config-load-error fix hint says `Check $XDG_CONFIG_HOME/mirdan/ (or ~/.config/mirdan/) for syntax errors`, but `mirdan::agents::load_agents_config` (crates/mirdan/src/agents.rs:114) only reads `MIRDAN_AGENTS_CONFIG` then `~/.mirdan/agents.yaml` then the embedded default — it never consults `$XDG_CONFIG_HOME/mirdan/` or `~/.config/mirdan/`, so the hint points users at a directory that has no effect. This is a faithful mirror of the pre-existing string in `crates/mirdan/src/doctor.rs:167` (as the task instructed), so the root fix belongs upstream; consider hoisting the hint to a shared constant in `mirdan::status` so the corrected path lives in one place rather than two. Pre-existing, non-blocking.
  - RESOLVED (smallest in-scope fix): Corrected the hint in `checks.rs` to `Check $MIRDAN_AGENTS_CONFIG or ~/.mirdan/agents.yaml for syntax errors` — the locations `load_agents_config` actually reads — with an explanatory comment. RATIONALE for not hoisting to a `mirdan::status` shared constant: the task scoped this work to the doctor crate (checks.rs/mod.rs) and instructed preferring the smallest in-scope fix over editing the mirdan crate. The stale `$XDG_CONFIG_HOME/mirdan/` string still lives in `crates/mirdan/src/doctor.rs:167`; correcting that copy and introducing a shared constant is a separate upstream change and should be a follow-up mirdan task. The doctor's own hint is now correct for its own users.