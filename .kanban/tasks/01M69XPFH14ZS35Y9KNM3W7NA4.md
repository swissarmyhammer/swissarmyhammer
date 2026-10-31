---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m6a5xfwzx05feh8nr7b86w6a
  text: |
    ### implement — changed
    - evidence: Deleted crates/swissarmyhammer-tools/src/mcp/tools/files/ and the tests file_tools_integrations.rs (+ dir), file_size_limits.rs, file_rate_limiting.rs. Removed register_file_tools and all uses, "files" from KNOWN_TOOL_NAMES, the "file"/"files" rows in CLI_CATEGORY_TABLE (cli_category), and the hashline, edit-match and (unused) similar, ignore, grep, encoding_rs deps from swissarmyhammer-tools/Cargo.toml (Cargo.lock dropped 3 edges). Validator profile is now code_context only. Agent-tools server no longer mounts files. CLI: mcp_integration.rs, cli.rs/banner.rs/main.rs examples moved to kanban/git, README row removed, tests moved to kanban/git/web.
    - code moves: none. Checked: no other tool used tools/files/ code (shared_utils was used only inside files/). ToolContext::session_root stays in tool_registry/context.rs; it is now used only by a unit test.
    - tests rewritten on other tools: rmcp_integration.rs (large-result log test now uses a git unknown-op error, because multi-line results are dropped by traced_test scope filter and the old tail check passed on the args line), session_root_resolution.rs (git get diff relative paths), per_client_tool_composition.rs (no Replacement tool exists now), disabled_tools.rs/fixtures.rs (new ReplacementMockTool keeps host composition coverage), profiles.rs, unified_server.rs, mcp_server_paritys.rs, tool_descriptions.rs (git/changes), cli tests.
    - checks: cargo check --workspace --tests ok; cargo build --workspace ok; cargo nextest run --override-version-check --workspace --no-fail-fast: 13547 passed, 0 failed (re-run of tools + cli crates after review fixes: 1661 passed); cargo fmt --check ok; cargo clippy --workspace --all-targets -- -D warnings ok.
    - review file on every changed Rust file: findings on changed lines fixed (rmcp_integration tail check scoped to the completion record, helper dedupe; profiles.rs code_context assertion; tool_descriptions.rs guard; test_utils.rs call_tool assertion). Findings on lines this card did not change were left (examples: profiles.rs create_* duplication, tools/mod.rs register_validator_tools, session_root_resolution.rs context_rooted_at helper, per_client_tool_composition.rs assert helpers, mcp_server_paritys.rs parity test, tool_descriptions.rs newtypes).
    - left for later cards: mcp-proxy tests and claude-agent tool_classification still use the string "files"; ToolCategory::Replacement has no real tool now; builtin skill docs; doc/src/concepts/closed-write-surface.md; diagnostics crate watcher text.
    - note: a stray untracked nested copy ./swissarmyhammer/ exists inside the repo root (not touched).
    - next: /review
  timestamp: 2026-10-31T23:49:59.839941+00:00
depends_on:
- 01M69XPBKCWV1Q2JAP5MBCW1EK
position_column: doing
position_ordinal: '8480'
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