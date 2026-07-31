---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kyw0t8strkt73fe3xxee0xg4
  text: |-
    Adversarial review of ^6xjxebg recommends a stronger option than "one shared entry point":

    The CLI does not need its own registry at all. `CliToolContext` already owns the live `McpServer` it executes against (`resolve_server()`), and `McpServer::get_tool_registry()` is public. Hand that registry to `CliBuilder` and:

    - Drift becomes structurally impossible. `create_tool_registry` and its parity test both disappear.
    - The duplicate `SkillLibrary` / `AgentLibrary` load per invocation goes away.
    - The `tools.yaml` enable/disable pass stops being a thing the CLI can forget.

    Check first whether `McpServerHandle::server()` returns `Some` on every path the CLI uses. `unified_server.rs` builds one variant with `server: None`.

    `collect_all_health_checks` in `health_registry.rs` still holds the third copy and still has no guard, whichever option wins.
  timestamp: 2026-07-31T12:01:36.570518+00:00
position_column: todo
position_ordinal: c480
title: Three copies of the tool registration list (server, CLI, health_registry)
---
The list of `register_*_tools` calls exists in three places. Two of them already drifted once (see ^6xjxebg).

## The three copies

1. `crates/swissarmyhammer-tools/src/mcp/server.rs` — `McpServer::register_all_tools`. The reference list.
2. `apps/swissarmyhammer-cli/src/mcp_integration.rs` — `CliToolContext::create_tool_registry`. Held 8 of 12 entries until ^6xjxebg.
3. `crates/swissarmyhammer-tools/src/health_registry.rs` — `collect_all_health_checks`. Builds its own default skill, agent and prompt libraries, the same way the CLI now does.

Copies 2 and 3 also duplicate the default library construction (`SkillLibrary::new()` + `load_defaults()`, `AgentLibrary::new()` + `load_defaults()`, `TemplateLibrary::default()`).

## Required change

Make one public entry point in `swissarmyhammer-tools` that registers every tool family, plus one helper that builds the default libraries for callers that own no server. Have the CLI and `collect_all_health_checks` call it. `register_all_tools` keeps its shared-library parameters, because the server passes its own libraries.

A parity test now guards copy 2 (`test_cli_tool_registry_matches_server_registry`). Copy 3 has no guard.

## Acceptance

- One list of `register_*_tools` calls in the workspace.
- The parity test still passes.
- `sah doctor` still reports a health check for every tool.

Found while implementing ^6xjxebg. #refactor #tools