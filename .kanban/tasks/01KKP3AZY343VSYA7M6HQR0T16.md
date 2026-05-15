---
position_column: done
position_ordinal: '9980'
title: dispatch.rs duplicates MCP tool logic — not wired as single source of truth yet
---
swissarmyhammer-kanban/src/dispatch.rs\n\nThe dispatch module claims to be \"the single source of truth for operation dispatch\" but the MCP tool (swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:383-804) still has its own private `execute_operation` function with identical logic. There are now two copies that can drift.\n\nThe MCP tool's version has additional operations not in dispatch.rs (e.g., attachment operations are in the KANBAN_OPERATIONS list and schema but dispatch.rs has no attachment handling).\n\nSuggestion: Wire the MCP tool to call `swissarmyhammer_kanban::dispatch::execute_operation` instead of its private copy, and add the missing attachment dispatch arms."