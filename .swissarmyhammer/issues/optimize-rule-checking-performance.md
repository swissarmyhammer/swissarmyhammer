# Optimize Rule Checking Performance

## Problem

Rule checking is extremely slow (~3-5 minutes for 14 rules on 1 file) due to session management overhead:

- Each rule check creates a new llama-agent session
- Each new session discovers 30 tools and sends all descriptions to LLM in system prompt
- Tool discovery takes ~15-20 seconds per rule
- Generating "PASS" (1 token) takes 15-20 seconds because of massive system prompt

## Root Cause

`swissarmyhammer-agent-executor/src/llama/executor.rs:662-671` - `execute_with_real_agent()` creates a new session every time:

```rust
// Create a new session  
let mut session = agent_server.create_session().await?;

// Discover available tools (sends 30 tool descriptions to LLM)
agent_server.discover_tools(&mut session).await?;
```

## Solutions

### Option 1: Session Pooling (Best)
- Create a session pool in `RuleChecker`
- Reuse sessions across multiple rule checks
- Clear session history between checks to avoid context pollution
- Keep tools discovered once per session

### Option 2: Skip Tool Discovery for Rules
- Add parameter to `create_session()` to skip tool discovery
- Rule checking doesn't need tools - just text generation
- Would require llama-agent API changes

### Option 3: Parallel Execution
- Change `checker.rs:621` from `.then()` to `.buffer_unordered(4)`
- Run 4 rules in parallel
- Still slow but 4x faster

## Recommended Approach

1. **Immediate**: Implement parallel execution (Option 3) - 5 minute fix
2. **Short-term**: Add session reuse to RuleChecker (Option 1) - 30 minute fix  
3. **Long-term**: Add no-tools mode to llama-agent (Option 2) - requires upstream changes

## Expected Improvements

- Current: 14 rules × 20s = 280s (4.6 minutes)
- With parallel (4 concurrent): 280s / 4 = 70s (1.2 minutes)
- With session reuse: 14 rules × 2s = 28s (sub-minute)
- With no-tools mode: 14 rules × 0.5s = 7s (optimal)

## Additional Investigation

### Session Creation Overhead

Timing analysis from logs shows:
- Session creation: ~5ms (fast)
- Tool discovery: ~5ms (fast)  
- **LLM inference with tool-heavy system prompt: 15-20 seconds (SLOW)**

The bottleneck is NOT session creation itself, but the massive system prompt containing 30 tool definitions that must be processed for every "PASS" response.

### Potential Solution: Session Cloning

If llama-agent Session struct is Clone, we could:
1. Create ONE "template session" with tools discovered
2. Clone it for each rule check (preserving tool discovery)
3. Clear message history but keep tool context
4. Avoid re-sending 30 tool definitions to LLM

This would reduce per-rule overhead from 15-20s to <1s.

### Alternative: Skip Tools Entirely

Rule checking doesn't need MCP tools - it's simple text classification (PASS/VIOLATION).
Could create sessions WITHOUT tool discovery for massive speedup.
