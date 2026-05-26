---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8b80
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
- [x] `mirdan::settings` exists with `read_json`/`write_json`/`ensure_array_contains`/`remove_from_array`/`set_object`/`remove_key`, all documented.
- [x] `DenyBash` and `Statusline` call mirdan primitives only; no claude-code-specific JSON shapes in CLI install code beyond the constants `"permissions/deny"` / `"Bash"` / `"statusLine"`.
- [x] `apps/swissarmyhammer-cli/src/commands/install/settings.rs` is either deleted or reduced to the smallest claude-code-local-scope wrapper that genuinely cannot move. (Deleted; the local-scope MCP helpers moved to `mirdan::mcp_config`; `ClaudeLocalScope` is now a thin caller.)
- [x] No regression in `sah init [user|local]` behavior; deny-Bash, statusline, MCP register, claude-local-scope MCP all still install/uninstall as today.
- [x] `cargo build -p mirdan -p swissarmyhammer-cli` green; clippy clean.

## Tests
- [x] Add `mirdan::settings` unit tests for each primitive (idempotency, missing parents, removal returning the change flag).
- [x] Keep the existing `DenyBash`/`Statusline`/`ClaudeLocalScope` install tests green; refactor them to drive through the moved code path.
- [x] `cargo test -p mirdan -p swissarmyhammer-cli` green.

## Workflow
- Use `/tdd` — write mirdan settings primitive tests first. #init-doctor

## Implementation Notes
- New module `mirdan::settings` with six primitives: `read_json`, `write_json`, `ensure_array_contains`, `remove_from_array`, `set_object`, `remove_key`. 21 unit tests cover them.
- `mirdan::mcp_config` gained the local-scope helpers (`claude_json_path`, `project_key`, `ensure_project_entry`) plus two in-memory primitives (`set_mcp_server_entry`, `remove_mcp_server_entry`) that the file-based `register_mcp_server`/`unregister_mcp_server` now compose. `project_key` returns `RegistryError` (not the bare `String` the old CLI helper returned) to match mirdan conventions.
- `mirdan::mcp_config::register_mcp_server`/`unregister_mcp_server` are now thin callers of `mirdan::settings::read_json`/`write_json`; the previously private `read_json_config`/`write_json_config` helpers are deleted.
- `DenyBash`/`Statusline` install/uninstall paths use only the generic mirdan primitives: `ensure_array_contains(/permissions/deny, "Bash")`, `remove_from_array(/permissions/deny, "Bash")`, `set_object("statusLine", {...})`, `remove_key("statusLine")`. The Claude-specific constants (`"/permissions/deny"`, `"Bash"`, `"statusLine"`) and the statusline value live as `const`s in `components/mod.rs`.
- `ClaudeLocalScope` is now a thin caller: `read_json` → `ensure_project_entry` → `set_mcp_server_entry` / `remove_mcp_server_entry` → `write_json`. The empty-`mcpServers` cleanup remains in `components/mod.rs` because it is local-scope specific (project entries can carry sibling fields like `allowedTools`).
- `apps/swissarmyhammer-cli/src/commands/install/settings.rs` is deleted; `pub mod settings;` removed from `commands/install/mod.rs`. No callers outside that file existed.
- The `mirdan::settings::write_json` writer always emits a trailing newline (matching the existing `mirdan::mcp_config` writer). The legacy CLI writer omitted the trailing newline; this is a one-byte EOF change to `~/.claude.json` and `~/.claude/settings.json` writes, semantically irrelevant (every reader accepts trailing newlines and editors normalize them). All component tests assert JSON values, not bytes.
- All 327 mirdan tests + 1080+ swissarmyhammer-cli tests pass. Clippy clean with `-D warnings`.

## Review Findings (2026-05-26 09:06)

### Nits
- [x] `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:563` — Stale doc-comment on `pub struct Statusline`: it says "then calls `merge_statusline` / `remove_statusline`", but those helpers are deleted by this card. The component now calls `mirdan_settings::set_object` / `mirdan_settings::remove_key`. Update the sentence to reflect the new code path (e.g. "then calls `mirdan::settings::set_object` / `mirdan::settings::remove_key` with the `statusLine` key").