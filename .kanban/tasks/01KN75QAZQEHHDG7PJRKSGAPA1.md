---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffca80
title: 'Test MCP server: initialization, tool execution, and prompt handling'
---
File: swissarmyhammer-tools/src/mcp/server.rs (53.0%, 224 uncovered lines)

Uncovered functions:
- new_with_work_dir() - alternate constructor
- set_server_port()
- create_validator_server()
- initialize() - full init flow
- list_prompts() / list_tools()
- execute_tool() - the main tool dispatch
- get_prompt() - prompt retrieval
- reload_prompts()
- start_file_watching() / stop_file_watching()

These are the core MCP server operations that need integration tests with mock peers and tool registries. #coverage-gap