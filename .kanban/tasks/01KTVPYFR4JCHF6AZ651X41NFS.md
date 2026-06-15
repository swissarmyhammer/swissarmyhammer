---
assignees:
- claude-code
position_column: todo
position_ordinal: 9d80
project: mirdan-install
title: 'mirdan: TOML-aware MCP config writes (Codex .codex/config.toml)'
---
## What
All mirdan MCP config writers are JSON-only, but Codex's MCP config is TOML (`.codex/config.toml`, servers key `mcp_servers` — `crates/mirdan/src/agents_default.yaml:268-281`). Today registering an MCP server for Codex would write pretty-printed JSON into a `.toml` file. The status *reader* already dispatches on extension (`crates/mirdan/src/status.rs:630-644` parses `.toml` via `toml::from_str` and converts to `serde_json::Value`); the writers do not.

Affected writers (all funnel through `settings::read_json` / `settings::write_json` in `crates/mirdan/src/settings.rs:36-58`, which call `swissarmyhammer-common`'s JSONC `read_json_file`):
- `mcp_config::register_mcp_server` / `mcp_config::unregister_mcp_server` (`crates/mirdan/src/mcp_config.rs:169` / `:184`) — used by the root-explicit path `install.rs::register_mcp_server_at`.
- `generic_register_mcp` / `generic_unregister_mcp` (`crates/mirdan/src/strategy/mod.rs:378` / `:407`) — the `GenericMcpJsonStrategy` path (its doc comment already claims "JSON/TOML" but the impl is JSON-only).

Approach: add extension-dispatched config read/write helpers (e.g. `read_mcp_config` / `write_mcp_config` in `crates/mirdan/src/settings.rs` or `mcp_config.rs`): for `.toml` paths, parse TOML → convert to `serde_json::Value` (reuse/extract the existing toml→json conversion already used by `status.rs`), mutate via the existing `set_mcp_server_entry` / `remove_mcp_server_entry` (`mcp_config.rs:101-157`, unchanged), then convert back and serialize with the `toml` crate. All four writer functions above switch to the new helpers; JSON behavior is byte-identical to today for non-`.toml` paths.

- [ ] Extension-dispatched read/write helpers (TOML ⇄ serde_json::Value), reusing the status.rs toml→json conversion
- [ ] Route `mcp_config::register_mcp_server` / `unregister_mcp_server` through them
- [ ] Route `generic_register_mcp` / `generic_unregister_mcp` through them
- [ ] Unit tests (see Tests)

## Acceptance Criteria
- [ ] Registering `McpServerEntry { command, args: ["serve"], env: {} }` into a temp `<root>/.codex/config.toml` produces valid TOML containing `[mcp_servers.kanban]` with `command` and `args`, not JSON text
- [ ] Unrelated pre-existing TOML keys in the file (e.g. `model = "..."`) are preserved across register/unregister
- [ ] The write round-trips with the existing TOML status reader: `status.rs` MCP-installed detection reports the server as installed after a write
- [ ] Re-registering an identical entry is a no-op (idempotent, mirrors the existing JSON `Ok(false)` behavior)
- [ ] Existing JSON-path tests in `mcp_config.rs` and `strategy/mod.rs` still pass unchanged

## Tests
- [ ] Unit tests in `crates/mirdan/src/mcp_config.rs` `#[cfg(test)]`: register into temp `config.toml` → parse with `toml::from_str`, assert entry shape; unregister removes it; unrelated keys preserved; idempotent re-register returns no-change
- [ ] Unit test in `crates/mirdan/src/strategy/mod.rs` tests: `GenericMcpJsonStrategy::register_mcp` against a synthetic agent whose `mcp_config.project_path` ends in `.toml` writes TOML
- [ ] Round-trip test: write via the new path, assert the `status.rs` TOML reader sees the server installed
- [ ] `cargo test -p mirdan` passes with 0 failures

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.