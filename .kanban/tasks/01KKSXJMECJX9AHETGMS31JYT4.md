---
assignees:
- claude-code
depends_on:
- 01KKSXJ97Z89AG2XDP55MKYK5N
position_column: done
position_ordinal: ffffffff8e80
title: Update validator prompt templates to mention available tools
---
## What\nUpdate the validator prompt rendering to inform the LLM that it has `code_context` and `validator_files` tools available. Without this, the validator agent won't know it can read files or search code.\n\n**Files:**\n- `avp-common/src/validator/executor.rs` — `RuleSetSessionContext`, `RulePromptContext`\n- `.system/validator` prompt template (or wherever the validator system prompt lives)\n\n**Approach:**\nAdd a section to the validator session init prompt that describes the available tools:\n- `validator_files` — read file contents, glob for file patterns\n- `code_context` — search symbols, grep code, get blast radius, call graphs\n\nInclude brief usage examples so the LLM knows the tool schemas.\n\n## Acceptance Criteria\n- [ ] Validator session init prompt mentions both tools with usage examples\n- [ ] Prompt is only added when MCP tools are available (not when running without MCP)\n- [ ] Existing validator behavior unchanged when tools are not configured\n\n## Tests\n- [ ] Unit test: rendered prompt includes tool descriptions when MCP is configured\n- [ ] Unit test: rendered prompt omits tool descriptions when MCP is not configured\n- [ ] `cargo test -p avp-common`