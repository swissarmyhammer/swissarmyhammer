---
assignees:
- claude-code
depends_on:
- 01KSMXH3N2YKCNB3HGDYBF5E6B
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffae80
title: 'Doctor: detect Codex-style TOML MCP config in mirdan::status::mcp_server_installed'
---
## What

`mirdan::status::mcp_server_installed` in `crates/mirdan/src/status.rs` reads the MCP config file as JSON only — it calls `read_json` and probes `mcpServers.sah` / `servers.sah`. Codex's MCP config lives in `~/.codex/config.toml` (TOML, not JSON) with sections like `[mcp_servers.sah]` and a `command = "sah"` key. The current code returns `false` for any TOML file, so the prior task's `codex` MCP rows would always report `missing`.

Extend detection to recognize TOML when the file extension is `.toml` (or the JSON parse fails and the path ends with `.toml`). The rule is the same in both formats: a server entry exists under the configured `servers_key` (or the conventional fallbacks) whose `command` is `sah` or ends with `/sah`.

Keep this isolated to the detector — do not add a TOML writer here. Installing the Codex MCP config (the write path used by `sah init`) is the install layer's concern and is out of scope for this task.

Files:
- `crates/mirdan/src/status.rs` — extend `mcp_server_installed` (and its helpers) to handle TOML. Add `toml` to `crates/mirdan/Cargo.toml` if not already a dep (likely yes — confirm).
- Probably introduce a small `enum McpConfigFormat { Json, Toml }` derived from path extension, or a `read_config_doc` helper that returns a `serde_json::Value` (parse TOML → JSON value) so the downstream `mcpServers.sah.command` probing is unchanged.

## Acceptance Criteria
- [ ] `mcp_server_installed` returns `true` when given a TOML file at any path containing `[mcp_servers.sah]\ncommand = "sah"`.
- [ ] `mcp_server_installed` returns `true` when given a TOML file with `command = "/usr/local/bin/sah"` (absolute path).
- [ ] `mcp_server_installed` returns `false` when the TOML server entry's command is not sah (`command = "node"`).
- [ ] `mcp_server_installed` returns `false` when the TOML file is missing or malformed.
- [ ] Existing JSON behavior is unchanged: every test in `status::tests` that currently passes still passes.
- [ ] The `Component::Mcp` row for the `codex` agent (with `mcp_config.servers_key = mcp_servers` from the prior task) correctly resolves to `Installed` when a fake `~/.codex/config.toml` exists with a sah server.

## Tests
- [ ] Add `test_mcp_installed_toml_basic` in `crates/mirdan/src/status.rs::tests`: write a `.toml` file with `[mcp_servers.sah]\ncommand = "sah"`, call `mcp_server_installed`, assert true.
- [ ] Add `test_mcp_installed_toml_absolute_path` mirroring the JSON `test_mcp_installed_with_absolute_path_command`.
- [ ] Add `test_mcp_installed_toml_wrong_command` mirroring `test_mcp_wrong_command_is_missing`.
- [ ] Add `test_mcp_installed_toml_other_server` mirroring `test_mcp_other_server_is_missing`.
- [ ] Add `test_mcp_installed_toml_malformed_returns_false` that writes invalid TOML.
- [ ] Test command: `cargo test -p mirdan status::tests`.

## Workflow
- Use `/tdd` — write the five new TOML tests first, then implement the TOML branch in `mcp_server_installed`.

## Depends on
- 01KSMXH3N2YKCNB3HGDYBF5E6B (agents_default.yaml needs the `codex.mcp_config` entry before the full-stack test in the previous task makes sense) #init-doctor