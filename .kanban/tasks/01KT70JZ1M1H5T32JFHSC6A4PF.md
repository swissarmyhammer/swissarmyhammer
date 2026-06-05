---
assignees:
- claude-code
depends_on:
- 01KT57DTV0A34V64FJ53KW826G
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffd980
project: agent-builtins
title: sah deinit must NOT clean up the Bash deny (remove AllowBashCleanup)
---
Reverses the cleanup half of card #7 (`01KT57DTV0A34V64FJ53KW826G`). The serve-time Bash deny is sticky; `sah deinit` must NOT re-allow Bash.

## Why
The Bash deny is applied at serve-time (card #6) so the SAH `shell` tool replaces Claude's native Bash. That replacement should persist; `sah deinit` (which unregisters the MCP server) must not silently re-enable Bash. The deny's lifecycle is owned by the serve path, not by init/deinit.

## Change
- In `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs`: card #7 turned `DenyBash` into the deinit-only `AllowBashCleanup` (init no-op, `deinit()` → `mirdan::install::allow_tool(scope, "Bash")`). With deinit no longer cleaning up, this component now does NOTHING on either init or deinit — **remove it entirely** rather than leave a vestigial no-op component.
  - Delete the `AllowBashCleanup` struct + its `Initializable` impl.
  - Remove it from `register_all` (component count drops from 9 to 8).
  - `apps/swissarmyhammer-cli/src/commands/registry.rs`: drop its priority-table doc row; verify nothing else references it.
- Remove/replace the card-#7 tests that assert deinit removes a serve-applied deny (`test_allow_bash_cleanup_deinit_removes_serve_applied_deny`, etc.). After this, neither `sah init` nor `sah deinit` touches the Bash deny at all — add/keep a test asserting deinit does NOT re-allow Bash (i.e. a pre-existing `permissions.deny: ["Bash"]` survives `sah deinit`).

## Out of scope
- The serve-time deny itself (#6) stays.
- shelltool-cli's standalone install/deinit Bash handling — leave it; this card is only about sah's init/deinit component set.

## Done when
- No `AllowBashCleanup`/`DenyBash` component in sah's init set; nothing in `sah init`/`sah deinit` denies OR re-allows Bash.
- A test proves a Bash deny survives `sah deinit` (deinit does not clean it up).
- `cargo build --workspace` green; clippy clean; tests pass.

## Review Findings (2026-06-03 15:42)

### Warnings
- [x] `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:136` — Stale doc comment on `Statusline::priority`: "Component priority: 30 (runs after `Permissions`, before project workspace setup)." The `Permissions` it names was the display label of the now-removed `DenyBash`/`AllowBashCleanup` component (priority 20). With that component gone, the component before Statusline in the pipeline is `McpRegistration` (priority 10). This card explicitly set out to fix stale doc comments referencing the removed Bash component but missed this one. Suggested fix: reword to "runs after MCP registration, before project workspace setup" (or drop the parenthetical). Docs-only, no behavior impact. → FIXED: reworded to "runs after `McpRegistration`".

### Notes (verified clean — no action)
- Removal completeness: no live `AllowBashCleanup`/`DenyBash`/`deny-bash`/`allow-bash-cleanup` references remain; the only hits (registry.rs:89, 169) are intentional "was removed" explanatory comments.
- Component count 8 is correct: McpRegistration, Statusline, ProjectStructure, ClaudeMd, AgentDeployment, LockfileCleanup (6) + KanbanTool (7) + SkillDeployment (8).
- `test_deinit_does_not_reallow_bash` is sound: HOME-isolated (`IsolatedTestEnvironment` + `#[serial(home_env)]`), seeds `permissions.deny:["Bash"]`, runs the full `register_all` → `run_all_deinit` at User scope, and asserts Bash survives — it would fail if any component re-allowed Bash.
- Kept test `test_install_deny_bash_agrees_with_status_detector` still valid: exercises `ClaudeCodeStrategy.deny_tool` (the serve-time deny writer), not the removed component; comment updated accordingly.
- `lifecycle.rs:102` keeps `"Permissions"` only as a generic illustrative example of a display label (not a claim that a Permissions component exists); the slug example was correctly swapped `deny-bash` → `statusline`. Fine as-is.
- Verified: `cargo build` + `cargo clippy` clean (3 crates); registry + components tests pass (32 tests, incl. the new guard).