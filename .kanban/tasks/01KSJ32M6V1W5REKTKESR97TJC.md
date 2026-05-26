---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8680
title: 'Fix `sah init user` permissions + statusline: write to per-scope settings file, agent-agnostic'
---
## What

Despite the preamble fix landing, `sah doctor` still shows `Claude Code · user · Permissions ⚠ missing at /Users/wballard/.claude/settings.json` after `sah init user`. Root causes (same pattern as the prior preamble bug, missed in the first pass):

1. **`DenyBash` component** (`apps/swissarmyhammer-cli/src/commands/install/components/mod.rs`) — `is_applicable` returns `Project | Local` only, so the component is skipped in user scope. Even if it ran, it calls `settings::claude_settings_path()` which is hard-wired to `.claude/settings.json` (project), not the scope's path.
2. **Statusline install** — lives only inline in `init.rs` / `deinit.rs` as `install_statusline` / `uninstall_statusline`, gated on `Project | Local`, also writing to the project path. There is no `Initializable` Statusline component, and user scope never gets a statusline.
3. **Duplication in `init.rs`** — `install_deny_bash` is also called inline after the registry runs, doing the same project-path write the `DenyBash` component does. Belt-and-suspenders cruft.

Fix it the way the preamble was fixed — agent-agnostic, driven by data:

- **Generalize `DenyBash`**: `is_applicable` returns `true` for all three scopes. `init`/`deinit` load detected agents (`mirdan::agents::load_agents_config` + `get_detected_agents`) and, for each, resolve the settings file from `AgentDef` via the existing accessors:
  - `User` → `agent_global_settings_file(def)`
  - `Project`/`Local` → `agent_project_settings_file(def)` (resolved against the git root, like the preamble)
  - Agents whose `settings_path` is `None` for the scope are skipped (not applicable). Today only `claude-code` has settings paths, so this naturally targets `~/.claude/settings.json` for user scope and `.claude/settings.json` for project, with future agents joining by data.
  - Reuse `settings::merge_deny_bash` / `settings::remove_deny_bash` / `settings::read_settings` / `settings::write_settings`, but pass the resolved per-agent settings path.
- **Add a `Statusline` `Initializable` component** in `components/mod.rs` (priority 16, right after `DenyBash`), built the same way: iterate detected agents, resolve `settings_path`/`global_settings_path` per scope, call `settings::merge_statusline` / `settings::remove_statusline`. Register it in `components::register_all`.
- **Remove the inline `install_statusline` / `install_deny_bash` from `init.rs`** and `uninstall_statusline` / `uninstall_deny_bash` from `deinit.rs` (and their target-scope gates), since the registered components now own this end-to-end. Drop the `#[allow(deprecated)]` calls to `settings::claude_settings_path()` from those files entirely.

Per the architecture: all setup/teardown lives in `Initializable` components driven by `InitRegistry`, and the agent path knowledge stays in `AgentDef` / `agents_default.yaml` — not in scope-conditional inline code in `init.rs`.

## Acceptance Criteria
- [x] `sah init user` writes `permissions.deny: ["Bash"]` to `~/.claude/settings.json` (creating parent dirs if needed); first run installs, second is idempotent.
- [x] `sah init user` writes `statusLine.type = "command"`, `statusLine.command = "sah statusline"` to `~/.claude/settings.json`.
- [x] `sah init` (project) still writes both to `<git-root>/.claude/settings.json` — no regression.
- [x] `sah deinit user` removes both entries from `~/.claude/settings.json`.
- [x] `sah doctor` shows `Claude Code · user · Permissions` Ok after `sah init user`.
- [x] No inline `install_*` / `uninstall_*` for these two concerns remain in `init.rs` / `deinit.rs`.
- [x] `cargo build -p swissarmyhammer-cli` is green; clippy clean with `-D warnings`.

## Tests
- [x] Add a `#[serial_test::serial(home_env)]` test (using `IsolatedTestEnvironment`) running the `DenyBash` component's `init` with `InitScope::User` and asserting `~/.claude/settings.json` contains `Bash` in `permissions.deny`; then `deinit` and assert it is gone. Add a parallel test for the new `Statusline` component asserting the statusLine object is present then removed.
- [x] Add a regression test that `DenyBash::is_applicable(&InitScope::User)` and `Statusline::is_applicable(&InitScope::User)` both return `true`.
- [x] Keep the existing project-scope DenyBash tests green (refactor if needed to drive through the component's `init` rather than the inline functions).
- [x] `cargo test -p swissarmyhammer-cli` is green.

## Workflow
- Use `/tdd` — write the `init user` settings-file test first (fails today because the components don't run in user scope), then generalize the components and delete the inline duplicates. #init-doctor