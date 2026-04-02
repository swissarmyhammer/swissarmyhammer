---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffa080
title: 'Test shell tool: dispatch, init/deinit lifecycle, and process management'
---
File: swissarmyhammer-tools/src/mcp/tools/shell/mod.rs (51.4%, 156 uncovered lines)

Uncovered areas:
- ShellTool::execute() dispatch for several operation types
- Initializable::init() and deinit() lifecycle hooks (shell config creation/removal)
- Doctorable health check execution paths
- is_applicable() scope checks

Also shell/process.rs (72.2%, 52 lines) and shell/state.rs (73.8%, 56 lines):
- Process timeout handling
- State serialization edge cases
- search_history semantic search path #coverage-gap