# Support ClaudeCode Executor for Rule Checking

## Dependencies

⚠️ **BLOCKED BY**: 
1. `move-claudecode-executor-to-agent-executor` - Must be completed first
2. `add-agent-config-to-toolcontext` - Must be completed second

The ClaudeCodeExecutor needs to be moved to the agent-executor crate, and then ToolContext needs to include agent configuration so MCP tools can respect the user's executor choice.

---

## Problem

ClaudeCode executor is currently disabled for rule checking with a fallback to LlamaAgent:

```
⚠️  ClaudeCode executor cannot be used for rule checking (would create circular dependency)
   Automatically falling back to LlamaAgent for programmatic rule checking.
```

**This is not acceptable. ClaudeCode MUST be supported for rule checking.**

## Current Behavior

When running:
```bash
cargo run -- rule check --rule code-quality/cognitive-complexity /path/to/file.rs
```

The system warns about circular dependency and falls back to LlamaAgent, preventing ClaudeCode from being used.

## Required Behavior

ClaudeCode executor MUST work for rule checking without any fallback or restrictions.

## Technical Challenge

The warning mentions a "circular dependency" concern. This needs to be resolved, not worked around with a fallback.

## Implementation Requirements

- [ ] Wait for `move-claudecode-executor-to-agent-executor` to be completed
- [ ] Wait for `add-agent-config-to-toolcontext` to be completed
- [ ] Enable ClaudeCode executor for rule checking
- [ ] Remove the automatic fallback to LlamaAgent
- [ ] Remove warning messages about ClaudeCode not being supported
- [ ] Ensure ClaudeCode can perform rule checking operations
- [ ] Test rule checking with ClaudeCode executor

## Acceptance Criteria

- ClaudeCode executor works for `rule check` command
- No warnings or fallbacks occur
- No circular dependency issues remain
- All existing rule checking functionality works with ClaudeCode



## Critical Requirement

**IMPORTANT**: Keeping the fallback to LlamaAgent due to "circular dependencies" is **NOT AN ACCEPTABLE OUTCOME**.

The circular dependencies MUST be resolved, not ignored or worked around with fallbacks. The architecture must support ClaudeCode executor for rule checking without any compromises.

Fallbacks and warnings are temporary measures only - the final implementation must fully support ClaudeCode.
