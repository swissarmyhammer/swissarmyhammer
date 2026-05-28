---
assignees:
- claude-code
depends_on:
- 01KSMXH3N2YKCNB3HGDYBF5E6B
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffaf80
title: 'Doctor: restrict install-stack to YAML-declared doctored agents'
---
## What

`mirdan::status::check_all` iterates every agent returned by `agents::get_detected_agents`, which means `sah doctor` and `mirdan doctor` show install-stack rows for every detected coding agent on disk (Claude Code, Cursor, Windsurf, Cline, Roo Code, GitHub Copilot, Continue, Amp, Zed AI, etc.). Per the user, only Claude Code, Zed AI, GitHub Copilot, and Codex are first-class doctored agents. Restrict the install-stack to the agents that opt in via YAML.

Drive the allowlist from `agents_default.yaml`, not a hardcoded Rust array. Add an optional `doctor: bool` field (default `false`) to the `AgentDef` schema:

```yaml
- id: claude-code
  name: Claude Code
  doctor: true
  ...
```

Only agents with `doctor: true` participate in the install-stack. This keeps the registry and the doctor coupled by data, not code — adding a fifth doctored agent in the future is a YAML edit, not a Rust change.

Approach:

1. Extend `crates/mirdan/src/agents.rs::AgentDef` with `#[serde(default)] pub doctor: bool` (a missing field deserializes to `false`).
2. In `crates/mirdan/src/agents_default.yaml`, set `doctor: true` on `claude-code`, `zed-ai`, `copilot`, `codex`. Every other entry stays as-is (the missing field defaults to `false`). The path-expansion edits from card **01KSMXH3N2YKCNB3HGDYBF5E6B** are a precondition for the YAML changes here landing cleanly — coordinate with that card so both edit the same YAML entries once.
3. Add `mirdan::status::check_all_doctored(config, scopes)` that filters by `AgentDef.doctor` before delegating. No `DOCTORED_AGENT_IDS` constant — the YAML is the source of truth.
4. Update both consumers to use the new entry point:
   - `crates/mirdan/src/doctor.rs::MirdanDoctor::check_install_stack`
   - `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs::check_install_stack_with`
5. `mirdan status` (the table command) stays on the broad `check_all` — `mirdan` is "where are the packages" and shows everything.

Files:
- `crates/mirdan/src/agents.rs` — add `doctor: bool` field with `#[serde(default)]`.
- `crates/mirdan/src/agents_default.yaml` — set `doctor: true` on the 4 agents.
- `crates/mirdan/src/status.rs` — add `check_all_doctored`.
- `crates/mirdan/src/doctor.rs` — switch to `check_all_doctored`.
- `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs` — switch to `check_all_doctored`.

## Acceptance Criteria
- [ ] `AgentDef` has a public field `pub doctor: bool` with `#[serde(default)]` so older configs without the field still parse (defaulting to `false`).
- [ ] `agents_default.yaml` sets `doctor: true` on exactly `claude-code`, `zed-ai`, `copilot`, and `codex`; every other entry omits the field (or sets `false`).
- [ ] `check_all_doctored(config, scopes)` returns only rows whose `AgentDef.doctor` is `true`.
- [ ] `sah doctor` and `mirdan doctor` install-stack output contains rows only for those four agents, regardless of what else is detected on disk.
- [ ] `mirdan status` (the table command) still shows every detected agent (no behavior change there).
- [ ] No `DOCTORED_AGENT_IDS` constant or other hardcoded id list anywhere in Rust — the only allowlist is the YAML field.
- [ ] Existing install-stack tests in `crates/mirdan/src/doctor.rs::tests` and `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs::tests` still pass (update assertions if they referenced ids outside the YAML allowlist).

## Tests
- [ ] Add `test_agent_def_doctor_field_defaults_to_false` in `crates/mirdan/src/agents.rs::tests`: parse a YAML entry that omits `doctor`, assert `doctor == false`. Parse one that sets `doctor: true`, assert `doctor == true`.
- [ ] Add `test_agents_default_yaml_doctors_exactly_four_agents` in `crates/mirdan/src/agents.rs::tests`: load `agents_default.yaml`, collect `AgentDef`s where `doctor == true`, assert the set is exactly `{claude-code, zed-ai, copilot, codex}`.
- [ ] Add `test_check_all_doctored_filters_by_doctor_field` in `crates/mirdan/src/status.rs::tests`: build a synthetic `AgentsConfig` with two agents (one `doctor: true`, one `doctor: false`, both detectable), call `check_all_doctored(&config, &[InitScope::Project, InitScope::User])`, assert every returned `ComponentStatus.agent_id` belongs to a `doctor: true` agent.
- [ ] Add `test_check_install_stack_only_emits_doctored_agents` in `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs::tests`: drive `check_install_stack_with` with a synthetic config containing `cursor` (no `doctor` field) and assert no check name contains "Cursor".
- [ ] Same shape in `crates/mirdan/src/doctor.rs::tests`.
- [ ] Test commands: `cargo test -p mirdan agents::tests`, `cargo test -p mirdan status::tests`, `cargo test -p mirdan doctor::tests`, `cargo test -p swissarmyhammer-cli doctor::checks::tests`.

## Workflow
- Use `/tdd` — write the YAML-driven filtering tests first; they should fail because `AgentDef.doctor` does not exist yet. Then add the field, set it on the four agents in YAML, add `check_all_doctored`, and switch consumers.

## Depends on
- 01KSMXH3N2YKCNB3HGDYBF5E6B (agents_default.yaml edits for the 4 agents)

## Review Findings (2026-05-27 11:33)

### Warnings
- [x] `crates/mirdan/src/doctor.rs:280-301` and `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs:78-100` — In `test_check_install_stack_only_emits_doctored_agents`, both `claude-code` (doctored) and `cursor` (undoctored) use `/nonexistent/...` detect paths. Because neither detects, `get_detected_agents` triggers the `claude-code` fallback in `agents.rs:227-235` and `cursor` never enters the input set to `check_all_doctored` — the doctor filter is never actually exercised. The "no check name contains 'Cursor'" assertion would pass even against the unfiltered `check_all`. Fix by creating real temp dirs (as `status.rs::test_check_all_doctored_filters_by_doctor_field` already does) so both agents detect and the filter is what excludes cursor. The `status.rs` test does cover the filter correctly, so the regression coverage is not zero, but these two consumer tests don't add the boundary they claim.

### Nits
- [x] `crates/mirdan/src/status.rs:260-270` — `check_all_doctored` duplicates the scope/component iteration loop with `check_all` (lines 236-245). Three lines of shared body is below the threshold where extraction pays off, but if a third caller appears, fold the loop into a private `check_for_agents(&[DetectedAgent], &[InitScope])` helper that both wrappers delegate to.