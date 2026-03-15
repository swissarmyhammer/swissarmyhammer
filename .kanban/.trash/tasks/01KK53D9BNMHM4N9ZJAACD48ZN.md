---
position_column: todo
position_ordinal: e7
title: code-context skill + remove treesitter tool
---
## What
Create the `code-context` skill (`builtin/skills/code-context/SKILL.md`) that teaches the AI agent when and how to use `code_context` and `git get diff` instead of grep. Remove the old `treesitter` tool registration once `code_context` is stable.

Files: `builtin/skills/code-context/SKILL.md` (new), `swissarmyhammer-tools/src/mcp/tool_registry.rs`, `swissarmyhammer-tools/src/mcp/tools/treesitter/mod.rs`

Spec: `ideas/code-context-architecture.md` — "code-context skill" + "Migration" step 8.

## Acceptance Criteria
- [ ] `SKILL.md` includes: when to trigger, use-case table (scenario → instead of → use), workflow guidance
- [ ] Covers all operations: grep code, search code, get symbol, find/search/list symbol, get callgraph, get blastradius, get status, git get diff
- [ ] Teaches agent to check `get status` first, prefer structured queries, combine operations
- [ ] Old `treesitter` tool registration removed from `ToolRegistry`
- [ ] No references to `treesitter` tool remain in skills or agent configs

## Tests
- [ ] `SKILL.md` parses as valid markdown
- [ ] `treesitter` tool no longer in registry after removal
- [ ] `code_context` tool handles all operations that `treesitter` used to handle
- [ ] `cargo test -p swissarmyhammer-tools`