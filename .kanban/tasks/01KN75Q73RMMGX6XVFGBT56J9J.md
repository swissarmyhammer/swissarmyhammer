---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff8c80
title: 'Test code_context tool: operation dispatch and query execution'
---
File: swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs (33.1%, 342 uncovered lines)

Major uncovered areas:
- Most operation handlers in execute(): get_symbol, search_symbol, grep_code, get_callgraph, get_blastradius, list_symbols, detect_projects
- CodeContextState methods for query execution
- Error handling paths for missing/invalid workspace

This is the largest coverage gap in the tools crate. Needs integration tests that set up a code-context workspace and exercise each operation type. #coverage-gap