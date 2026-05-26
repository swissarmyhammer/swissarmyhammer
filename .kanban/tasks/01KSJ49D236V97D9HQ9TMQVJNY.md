---
assignees:
- claude-code
position_column: todo
position_ordinal: '8580'
title: Consolidate settings-JSON helpers in mirdan; shrink/delete CLI install/settings.rs
---
## What

`apps/swissarmyhammer-cli/src/commands/install/settings.rs` carries generic JSON-settings plumbing that duplicates (and predates) `mirdan::mcp_config`:

- Generic: `read_settings`, `write_settings` (parent-dir-creating idempotent JSON I/O).
- MCP-specific: `merge_mcp_server`, `remove_mcp_server`, `is_sah_server`, `sah_mcp_server_config`, `mcp_json_path`, `claude_json_path`, `project_key`, `ensure_project_entry`.
- Claude Code settings-file specific: `merge_deny_bash`, `remove_deny_bash`, `merge_statusline`, `remove_statusline`.

The MCP-specific helpers are mirrored by `mirdan::mcp_config::{register_mcp_server, unregister_mcp_server}` which the registered `McpRegistration` component already uses. The remaining settings-file helpers (`merge_deny_bash`, etc.) are called from the agent-iterating `DenyBash`/`Statusline` components — they are agent-agnostic in their callers but the helpers themselves are still in the CLI.

Goal: one home for JSON-settings primitives — `mirdan` — so adding a new install component or a new agent doesn't fork yet another helper module.

Concrete moves:

- Add `mirdan::settings` (new module): `read_json(path) -> Value`, `write_json(path, &Value)` (the generic idempotent I/O — replaces CLI `read_settings`/`write_settings`).
- Add idempotent JSON primitives in `mirdan::settings`: `ensure_array_contains(root, pointer, value) -> bool` and `remove_from_array(root, pointer, value) -> bool` (the generalized form of `merge_deny_bash` / `remove_deny_bash` — caller passes `/permissions/deny` and `"Bash"`); `set_object(root, key, value) -> bool` and `remove_key(root, key) -> bool` (the generalized form of statusline merge/remove).
- Rewrite `DenyBash` and `Statusline` to call the generic primitives with the Claude-conventional pointer/value pairs they already encode.
- Move the MCP local-scope helpers `ensure_project_entry`, `project_key`, `claude_json_path` to `mirdan` (they are claude-specific but live with the rest of MCP config). `ClaudeLocalScope` is then a thin call site.
- Delete `apps/swissarmyhammer-cli/src/commands/install/settings.rs` once empty. If a thin claude-code-local-scope wrapper genuinely needs to stay in CLI (because it derives `project_key` from cwd/git), keep only that.

This card is the natural follow-up to the path-safety move card and to the existing `01KSFZ3EHCAGT2TNP3FWSKMDX6` (share install-detection predicates) — together they leave the CLI install components as thin orchestrators over mirdan primitives.

## Acceptance Criteria
- [ ] `mirdan::settings` exists with `read_json`/`write_json`/`ensure_array_contains`/`remove_from_array`/`set_object`/`remove_key`, all documented.
- [ ] `DenyBash` and `Statusline` call mirdan primitives only; no claude-code-specific JSON shapes in CLI install code beyond the constants `"permissions/deny"` / `"Bash"` / `"statusLine"`.
- [ ] `apps/swissarmyhammer-cli/src/commands/install/settings.rs` is either deleted or reduced to the smallest claude-code-local-scope wrapper that genuinely cannot move.
- [ ] No regression in `sah init [user|local]` behavior; deny-Bash, statusline, MCP register, claude-local-scope MCP all still install/uninstall as today.
- [ ] `cargo build -p mirdan -p swissarmyhammer-cli` green; clippy clean.

## Tests
- [ ] Add `mirdan::settings` unit tests for each primitive (idempotency, missing parents, removal returning the change flag).
- [ ] Keep the existing `DenyBash`/`Statusline`/`ClaudeLocalScope` install tests green; refactor them to drive through the moved code path.
- [ ] `cargo test -p mirdan -p swissarmyhammer-cli` green.

## Workflow
- Use `/tdd` — write mirdan settings primitive tests first. #init-doctor