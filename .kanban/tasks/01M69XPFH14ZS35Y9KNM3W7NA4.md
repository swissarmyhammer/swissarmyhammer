---
assignees:
- claude-code
depends_on:
- 01M69XPBKCWV1Q2JAP5MBCW1EK
position_column: todo
position_ordinal: ffff8c80
project: remove-files-tool
title: Remove the files MCP tool from swissarmyhammer-tools
---
Delete the MCP `files` tool: ops read, write, edit, glob and grep.

Work:
- Delete `crates/swissarmyhammer-tools/src/mcp/tools/files/` (`read`, `write`, `edit` with args, atomic, cascade and prompts, `glob`, `grep`, `schema.rs`, `shared_utils.rs`, `mod.rs`).
- Remove `register_file_tools` and every use of it. Remove `"files"` from `KNOWN_TOOL_NAMES` in `tool_config.rs` and the `files` arm in `cli_category`. Fix `tool_registry/` (registration.rs, context.rs, mod.rs), `server/mod.rs`, `server/profiles.rs`, `health_registry.rs`, `lib.rs`, `mcp/mod.rs`, `mcp/tools/mod.rs`.
- Delete the file tool integration tests: `tests/integration/file_tools_integrations.rs` and its `file_tools_integrations/` directory, `file_size_limits.rs`, `file_rate_limiting.rs`. Fix `session_root_resolution.rs`, `mcp_server_paritys.rs`, `per_client_tool_composition.rs`, and every test in `tool_registry/tests/` that uses a files tool. Use another tool in those tests.
- Fix `apps/swissarmyhammer-cli` (`mcp_integration.rs`, help text and tests that use `files` as the example: the finished card ^94s8tz3 moved the examples to `files`, so they must move again).
- Remove the `swissarmyhammer-hashline` and `swissarmyhammer-edit-match` dependency edges from `crates/swissarmyhammer-tools/Cargo.toml`. Do not delete those two crates here. A later card does.
- Check that no other tool uses code from `tools/files/` (for example `shared_utils` path checks or the session root helpers). If one does, move that code to the owner or a shared module before you delete.

Depends on the card that stops the native deny.
Done when: `cargo build --workspace` and `cargo nextest run --workspace` pass and no `files` tool is in the tool list. #remove-files