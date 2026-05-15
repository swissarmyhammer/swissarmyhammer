---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffff780
title: 'Test web tools: search execution and fetch operations'
---
Files in swissarmyhammer-tools/src/mcp/tools/web/:

- search.rs (19.0%, 47 uncovered) - execute_search() almost entirely untested; Brave API integration
- fetch.rs (37.5%, 25 uncovered) - URL fetch with privacy/security checks
- mod.rs (77.8%, 8 uncovered) - web tool dispatch

Total: 80 uncovered lines. The search and fetch operations are core functionality that needs at minimum unit tests with mocked HTTP responses. #coverage-gap