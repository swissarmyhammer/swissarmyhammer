---
assignees:
- claude-code
position_column: todo
position_ordinal: '8680'
title: Spawned claude CLI ignores board cwd's .claude/settings.json permission deny rules
---
## What

The kanban-app's in-process AI agent spawns the `claude` CLI in headless `--print` mode, which does **not** load project/local `.claude/settings.json` unless `--setting-sources` is passed. As a result the board's local `permissions.deny` rules (e.g. `deny: ["Bash"]`) are never loaded, so the agent runs tools that should be denied. Observed: with `swissarmyhammer-kanban/.claude/settings.json` containing `{"permissions": {"deny": ["Bash"]}}`, the agent still ran the `Bash` tool.

**Verified root cause** (tested against `claude` CLI 2.1.146 on 2026-05-21):

- `--dangerously-skip-permissions` is NOT the cause. `permissions.deny` is a hard rule enforced even under that flag — which is why running `claude --dangerously-skip-permissions` interactively still blocks denied tools. Leave the flag alone.
- The cause is settings *loading*. In `-p`/`--print` mode, project and local settings are not loaded by default. Decisive experiment — a temp dir with `.claude/settings.json` = `{"permissions":{"deny":["Read"]}}` and a `foo.txt`:
  - `claude -p "read foo.txt" --strict-mcp-config` → read succeeded (deny ignored).
  - `claude -p "read foo.txt" --strict-mcp-config --setting-sources project` → read blocked (deny enforced).
- The CLI exposes `--setting-sources <user,project,local>` (comma-separated) to control this. The interactive alias works because interactive mode loads user+project+local by default.

The spawn args live in `crates/claude-agent/src/claude_process.rs`, in the `CLAUDE_CLI_ARGS` const consumed by `ClaudeProcess::build_base_command`. They currently include `--verbose --print --input-format stream-json --output-format stream-json --dangerously-skip-permissions --include-partial-messages --no-session-persistence --replay-user-messages` and no `--setting-sources`.

**Approach** — Make the spawned headless CLI load the board's filesystem settings so its `permissions.deny`/`allow` are honored:

- [ ] In `crates/claude-agent/src/claude_process.rs`, add `--setting-sources` with the value `user,project,local` to the spawned command (extend `CLAUDE_CLI_ARGS`, or add the two args in `build_base_command`). This matches the setting sources interactive Claude loads by default, so the kanban agent behaves like the user's normal `claude`.
- [ ] Confirm the value covers the reported case: board-local `.claude/settings.json` (`project`) and `.claude/settings.local.json` (`local`). `user` is included to match default interactive behavior; if there is a deliberate reason to isolate the kanban agent from `~/.claude/settings.json`, use `project,local` instead and document why in a code comment.
- [ ] Verify project-settings resolution from the agent's actual cwd. The agent's session cwd is the board path (the `.kanban` subdirectory), while the deny file sits at the git-root `.claude/`. Confirm `--setting-sources project` resolves project settings from the repo/project root (not just the literal cwd) so a board opened at `<repo>/.kanban` still picks up `<repo>/.claude/settings.json`. If it does not, the agent must be spawned with cwd at the project root (or pass `--add-dir`) — capture whichever is needed.

Keep the change minimal and focused on the spawn args; do not refactor the surrounding process pipeline or the ACP permission engine.

## Acceptance Criteria
- [ ] The spawned `claude` command includes `--setting-sources` with `project` and `local` (and `user` unless deliberately excluded with a documented reason).
- [ ] With a board whose `.claude/settings.json` contains `permissions.deny: ["Bash"]`, the agent refuses to run the `Bash` tool (matches interactive `claude` behavior in the same directory).
- [ ] `permissions.allow` and `.claude/settings.local.json` from the board are likewise honored by the agent.
- [ ] No regression to existing spawn behavior: `--print`, stream-json I/O, MCP config args, and `--dangerously-skip-permissions` are unchanged.

## Tests
- [ ] In `crates/claude-agent/src/claude_process.rs` tests, add a unit test that builds the spawn command (mirroring the existing `get_args()` inspection tests around `--mcp-config`) and asserts the args contain `--setting-sources` followed by a value that includes `project` and `local`.
- [ ] Regression unit test asserting the existing required args are still present (`--print`, `--input-format stream-json`, `--output-format stream-json`).
- [ ] `cargo nextest run -p claude-agent` passes.

## Workflow
- Use `/tdd` — write the failing arg-inspection test first, then add the `--setting-sources` args to make it pass. #bug #kanban-app
