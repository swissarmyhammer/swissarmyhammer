---
assignees:
- claude-code
depends_on:
- 01KT57C9AFYCHVVK6VKK4V7W8A
- 01KT57CGJNA5TWBYBC7J75PVBA
- 01KT57CYY7P8VXA6JXBNJTNRF4
- 01KT57DNAKPKRHSXJ1KH7NQSQJ
- 01KT57D6T3WKRD1YNAKA70BEKC
- 01KT57DTV0A34V64FJ53KW826G
position_column: todo
position_ordinal: '8880'
project: agent-builtins
title: 'tests: real-path coverage of the per-host tool matrix'
---
Production-path tests (not mock-boundary) proving the full serve matrix. Follows the project rule: every user-visible capability needs a real-pipeline test.

## Cases
1. **llama-agent, zero external MCP servers** → agent still lists `files`, `web`, `skill`, `agent`, `shell` (the unconditional in-memory Agent mount). This is the load-bearing invariant.
2. **llama-agent + SAH server** → additionally lists Shared (`kanban`, `git`, `code_context`, `ralph`, `question`); `shell` appears exactly once (no duplicate from SAH).
3. **Claude client on `sah serve`** → `tools/list` returns Shared + `shell`, and NONE of the other Agent tools (files/web/skill/agent). Bash is denied in Claude settings after connect.
4. **Claude client** → native Read/Write/etc. NOT served by SAH (it uses its own); only `shell` crosses as the replacement.
5. **Validator profile** → read-only file tools present; locked-down subset unchanged.

## Notes
- Mirror the reference real-indexer→real-query pattern at `swissarmyhammer-tools/tests/integration/semantic_search_e2e.rs`.
- Drive `tools/list` through the actual rmcp serve/handshake with a clientInfo for Claude vs llama, not by calling internal filters directly.
- Assert the Bash-deny by reading the settings the serve path writes.

## Done when
- All five cases pass against the production serve/mount paths.
- A regression here would catch any reintroduction of `agent_mode` subtraction or a duplicated/ missing shell.