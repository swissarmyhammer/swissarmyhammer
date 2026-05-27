---
assignees:
- claude-code
depends_on:
- 01KSMXJBVVH06V6EDHYCFCRBHS
position_column: todo
position_ordinal: '8380'
title: 'Doctor: scope-pair policy — demote project-missing to Ok when user-installed'
---
## What

`mirdan::status::to_check` in `crates/mirdan/src/status.rs` maps a single `ComponentStatus` to a single `Check`: any `ComponentState::Missing` becomes `CheckStatus::Warning`, regardless of the other scope's state. That means when an agent + component is installed at user scope (e.g. `~/.claude/CLAUDE.md` exists) but missing at project scope (`./CLAUDE.md` does not), the doctor still emits a Warning for the project row — which is noise: the agent works fine at user scope, and project-scope is opt-in customization.

Change the policy: when the same (agent, component) has one scope `Installed` and another scope `Missing`, demote the missing-scope row to `CheckStatus::Ok` with a message that names where it was found (e.g. `"missing at .mcp.json; installed at user scope (/Users/.../.claude.json)"`). When both scopes are missing, both rows remain `Warning`. When both are installed, both stay `Ok`. `NotApplicable` rows continue to be filtered out by callers.

This is a behavior change to the install-stack consumer. The single-status `to_check` is too narrow to express the pair rule, so introduce a new function:

```rust
/// Convert a slice of ComponentStatus into Checks, applying the scope-pair
/// policy: when one scope has an installed component and another scope is
/// missing it, the missing-scope row is demoted to Ok with a message that
/// names the installed scope.
pub fn statuses_to_checks(statuses: &[ComponentStatus]) -> Vec<Check> { ... }
```

Group by `(agent_id, component)` and, for each group, decide the per-scope Check based on the group's contents. Keep `to_check` available for callers that genuinely want per-status conversion (or deprecate/remove it if nothing else uses it after the migration — check `get references`).

Update consumers:
- `crates/mirdan/src/doctor.rs::MirdanDoctor::check_install_stack` — replace the per-status loop with `statuses_to_checks`.
- `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs::check_install_stack_with` — same.

Files:
- `crates/mirdan/src/status.rs`
- `crates/mirdan/src/doctor.rs`
- `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs`

## Acceptance Criteria
- [ ] When user-scope is `Installed` and project-scope is `Missing` for the same (agent, component): project-scope check is `CheckStatus::Ok`, fix is `None`, message mentions the user-scope path.
- [ ] Same rule symmetric: project-scope `Installed`, user-scope `Missing` → user-scope check is `Ok` with project-scope reference in message.
- [ ] When both scopes are `Missing`: both checks are `Warning` with their original `sah init` / `sah init user` fix hints.
- [ ] When both scopes are `Installed`: both checks are `Ok`, unchanged from today.
- [ ] `NotApplicable` rows still produce no `Check` (filter remains in the caller, or in `statuses_to_checks` — either is fine; document the choice in the function doc).
- [ ] The current user's `sah doctor` output on this machine no longer warns about `Claude Code · project · MCP server`, `Claude Code · project · Preamble`, or `Claude Code · project · Permissions` (because the user-scope rows are installed).

## Tests
- [ ] Add `test_statuses_to_checks_demotes_project_missing_when_user_installed` in `crates/mirdan/src/status.rs::tests`: build two `ComponentStatus` for the same agent + component (one user `Installed`, one project `Missing`), assert the project check is `Ok` and references the user path.
- [ ] Add `test_statuses_to_checks_demotes_user_missing_when_project_installed` — symmetric.
- [ ] Add `test_statuses_to_checks_both_missing_stays_warning` — both `Missing`, both `Warning` with fix hints.
- [ ] Add `test_statuses_to_checks_both_installed_stays_ok` — both `Installed`, both `Ok`.
- [ ] Add `test_statuses_to_checks_filters_not_applicable` — `NotApplicable` produces no check.
- [ ] Update `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs::tests::test_check_install_stack_user_scope_rows`: when only user-scope preamble is installed, project-scope preamble check must now be `Ok` (not `Warning`).
- [ ] Test commands: `cargo test -p mirdan status::tests`, `cargo test -p swissarmyhammer-cli doctor::checks`.

## Workflow
- Use `/tdd` — write the five new `statuses_to_checks` tests first; they should fail because the function does not exist. Then implement `statuses_to_checks` and migrate the two consumers.

## Depends on
- 01KSMXJBVVH06V6EDHYCFCRBHS (allowlist restriction lands first so we're only reshaping the 4-agent flow) #init-doctor