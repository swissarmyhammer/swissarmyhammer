---
assignees:
- claude-code
depends_on:
- 01KSMXK4R8Y9A2ZWV7KFC1Y4PT
position_column: todo
position_ordinal: '8480'
title: 'Doctor: delete scope-blind legacy checks superseded by the install stack'
---
## What

Three legacy Claude-only `Doctorable` health checks live inside `swissarmyhammer-tools` and only probe project-scope paths. They produce false warnings whenever sah is installed at user scope, because they don't know `~/.claude` exists. Each is fully covered by `mirdan::status`'s install-stack at both scopes (per the prior tasks), so they are pure duplication.

Delete them, their helper functions, their `HealthCheck` arms, and their tests:

1. **`crates/swissarmyhammer-tools/src/mcp/tools/skill/mod.rs`** — `Skills installation` check around line 166/176 in the skill `Doctorable` impl. Iterates known skill names against `.claude/skills/<name>`. Subsumed by `Claude Code · {project,user} · Skills` from mirdan's install stack.
2. **`crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs`** — `Bash denied` check (~line 277–322, including `load_claude_settings_for_bash_check` and `settings_denies_bash`). Inspects `.claude/settings.json`. Subsumed by `Claude Code · {project,user} · Permissions`.
3. **`crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs`** — `Shell skill deployed` check (~line 344–365). Inspects `.claude/skills/shell`. Subset of `Claude Code · {project,user} · Skills`.

Also remove any tests that exclusively cover these three checks:
- `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs::tests` — `test_bash_*` checks tied to the deleted health check, `test_shell_skill_deployed_*` tests.
- `crates/swissarmyhammer-tools/src/mcp/tools/skill/mod.rs::tests` — any test that asserts on `Skills installation`.

Do NOT delete:
- The shell tool's `Initializable::init` write path that creates `.claude/settings.json` with Bash denied — that's the installer, separate from the doctor's detector.
- `settings_denies_bash` if it's used elsewhere (grep to confirm).
- Any other health checks (only these three are scope-blind duplicates).

Files:
- `crates/swissarmyhammer-tools/src/mcp/tools/skill/mod.rs`
- `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs`

Verify with `code_context` before deleting:

```json
{"op": "get callgraph", "symbol": "load_claude_settings_for_bash_check", "direction": "inbound"}
{"op": "get callgraph", "symbol": "settings_denies_bash", "direction": "inbound"}
```

If a helper has callers outside the deleted check, keep the helper.

## Acceptance Criteria
- [ ] `cargo run --bin sah doctor` no longer emits checks named "Skills installation", "Bash denied", or "Shell skill deployed".
- [ ] All three checks (and their dedicated tests) are removed.
- [ ] No dead helper functions left behind (grep confirms zero callers).
- [ ] The shell tool's `Initializable::init` still writes `.claude/settings.json` with Bash denied — the installer remains functional. Verified by re-running `cargo test -p swissarmyhammer-tools shell::tests::test_init` (or whatever covers the install side today; identify by reading the test list before deleting).
- [ ] `swissarmyhammer_tools::collect_all_health_checks()` no longer returns these three check names; existing other health checks unaffected.

## Tests
- [ ] Add a regression test in `apps/swissarmyhammer-cli/src/commands/doctor/mod.rs::tests` (or a new test alongside `test_run_diagnostics`) that runs the full doctor pipeline and asserts none of the deleted check names appear in `doctor.checks()`.
- [ ] Run the entire `swissarmyhammer-tools` test suite to confirm nothing depended on the deleted helpers: `cargo test -p swissarmyhammer-tools`.
- [ ] Run `cargo test -p swissarmyhammer-cli doctor` and confirm the install side of the shell tool still produces a `.claude/settings.json` with Bash denied (this verifies we didn't accidentally delete the installer).
- [ ] Full workspace check: `cargo check --workspace --all-targets` must pass with no warnings about unused functions/imports.

## Workflow
- Use `/tdd` — write the regression test asserting the deleted check names do NOT appear first (it should fail until the deletions land). Then perform the deletions and confirm green.

## Depends on
- 01KSMXK4R8Y9A2ZWV7KFC1Y4PT (scope-pair policy must be in place so the install-stack rows that supersede these are correct — otherwise we'd swap false-warning legacy checks for false-warning mirdan rows) #init-doctor