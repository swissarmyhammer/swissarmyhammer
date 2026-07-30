---
assignees:
- claude-code
position_column: todo
position_ordinal: b780
title: CLI tool registry drifted from register_all_tools (sah tool ralph is missing)
---
The CLI builds its own tool registry and it no longer matches the MCP server registry. Four tool families are absent from the CLI, so their `sah tool <name>` commands do not exist.

## Symptom

```
$ echo '{"session_id":"x"}' | sah tool ralph ralph check --
error: unrecognized subcommand 'ralph'
```

`sah tool --help` lists 8 tools: `code_context` `files` `git` `kanban` `question` `review` `shell` `web`.

This breaks the ralph Stop hook, whose command is `sah tool ralph ralph check --`.

## Cause

`apps/swissarmyhammer-cli/src/mcp_integration.rs:143-152` — `create_tool_registry` makes 8 registrations. Its own doc comment says it "should mirror the registration in `swissarmyhammer_tools::mcp::server::register_all_tools`". It drifted.

`crates/swissarmyhammer-tools/src/mcp/server.rs:968-979` — `register_all_tools` makes 12. The CLI omits:

- `register_ralph_tools`
- `register_agent_tools`
- `register_diagnostics_tools`
- `register_skill_tools`

The `ralph` tool itself is correct. `cli_category()` returns `Some("ralph")` and `cli_name()` returns `"ralph"`, so it becomes a CLI subcommand as soon as it is registered.

## Required change

1. Add the 4 missing registrations to `create_tool_registry`. Match the argument shapes that `register_all_tools` uses — `register_agent_tools` and `register_skill_tools` take library handles.
2. Add a parity test that compares the tool-name set from `create_tool_registry` against the set from `register_all_tools` and fails on any difference. A comment that says "should mirror" did not hold. A test will.

If a tool must stay out of the CLI on purpose, the parity test must name it in an explicit exclusion list with a reason. Silence is what caused this.

## Acceptance

- The parity test fails before the change and passes after it.
- `echo '{"session_id":"probe"}' | sah tool ralph ralph check --` returns the block/allow JSON, not a clap error.
- `sah tool --help` lists `ralph`, `agent`, `diagnostics`, and `skill`.

Note: the hook is also stripped at skill-install time, in a separate card. Both cards are needed before the Stop hook works. #bug #cli #ralph