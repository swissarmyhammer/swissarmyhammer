---
position_column: done
position_ordinal: '9580'
title: cli_gen builds all schema args for every verb — args not operation-specific
---
kanban-cli/src/cli_gen.rs:112-123\n\nEvery verb subcommand gets ALL non-op schema properties as arguments. So `kanban board init` shows `--id`, `--column`, `--title`, `--assignee`, etc. — none of which apply to board init. The MCP tool's CliBuilder uses per-operation schemas to scope args to only those relevant to each verb.\n\nThis is cosmetically confusing but functionally harmless — unused args are simply ignored by parse_input. The `sah tool kanban` CLI has per-operation arg scoping.\n\nSuggestion: Use the `x-operation-schemas` array from the schema (if present) to scope args per verb, or accept the trade-off for now and document it."