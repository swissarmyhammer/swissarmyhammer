when i cargo run -- rule check security/secure-random, i get Rule command failed: Agent execution failed: Action execution failed: LlamaAgent wrapper not initialized -- but i do not have llama configured, so i expect that the default -- claude will be used. this tells me we are not using the same agent configuration as running the workflow action


## Proposed Solution

### Root Cause
The rule check command hardcodes LlamaAgent initialization instead of using the agent configuration system:

**Current code (check.rs:250-251)**:
```rust
let agent_config = LlamaAgentConfig::for_small_model();
let agent = Arc::new(LlamaAgentExecutorWrapper::new(agent_config));
```

**Workflow action (actions.rs:247)** uses:
```rust
let agent_config = context.agent_config();
```

The workflow's `agent_config()` method:
- Checks `SAH_AGENT_EXECUTOR` environment variable
- Falls back to `AgentConfig::default()` which returns **ClaudeCode**
- Properly respects user configuration

### Solution Steps

1. Import `WorkflowTemplateContext` in the check command
2. Use `WorkflowTemplateContext::load_with_agent_config()` to get agent configuration
3. Create the agent executor based on configuration type (ClaudeCode or LlamaAgent)
4. Initialize the executor before creating RuleChecker
5. Update the test command similarly for consistency

This aligns rule check command with how workflow actions initialize agents, respecting environment variables and defaulting to ClaudeCode.

### Implementation Details

The RuleChecker expects `Arc<dyn AgentExecutor>`, so we need to:
- Check the agent config executor type
- Create appropriate executor (ClaudeCodeExecutor or LlamaAgentExecutorWrapper)
- Initialize it before use
- Pass it to RuleChecker::new()

Same pattern used in actions.rs:234-260.



## Implementation Notes

### Changes Made

1. **RuleChecker (swissarmyhammer-rules/src/checker.rs)**
   - Changed agent field type from `Arc<LlamaAgentExecutorWrapper>` to `Arc<dyn AgentExecutor>`
   - Updated documentation to reflect support for multiple agent types
   - Updated test helpers to use trait object instead of concrete type

2. **Rule Check Command (swissarmyhammer-cli/src/commands/rule/check.rs)**
   - Removed hardcoded `LlamaAgentConfig::for_small_model()` 
   - Added `WorkflowTemplateContext::load_with_agent_config()` to respect environment configuration
   - Use `AgentExecutorFactory::create_executor()` to create the appropriate executor (ClaudeCode or LlamaAgent)
   - Initialize executor before passing to RuleChecker

3. **Rule Test Command (swissarmyhammer-cli/src/commands/rule/test.rs)**
   - Applied same changes as check command for consistency
   - Now respects `SAH_AGENT_EXECUTOR` environment variable

### Configuration Behavior

The agent configuration system works as follows:
- Checks `SAH_AGENT_EXECUTOR` environment variable
  - `"claude-code"` → Uses ClaudeCodeExecutor (default)
  - `"llama-agent"` → Uses LlamaAgentExecutorWrapper with config from env vars
- Falls back to `AgentConfig::default()` which returns ClaudeCode
- No Llama configuration → Uses ClaudeCode as intended

### Test Results

All 3223 tests passed successfully, including:
- Rule checker creation tests
- Command execution tests  
- Integration tests

The fix aligns rule commands with workflow action behavior, resolving the "LlamaAgent wrapper not initialized" error when no Llama configuration exists.



### Code Review Fixes

Updated all documentation examples in `swissarmyhammer-rules/src/checker.rs` to demonstrate proper agent configuration pattern instead of hardcoded LlamaAgent initialization.

**Changes:**
- Lines 28-58: Updated module-level example to show `WorkflowTemplateContext::load_with_agent_config()` pattern
- Lines 89-107: Updated `new()` method example
- Lines 173-195: Updated `check_file()` method example  
- Lines 321-345: Updated `check_all()` method example

All examples now demonstrate:
1. Loading agent configuration via `WorkflowTemplateContext::load_with_agent_config()`
2. Creating agent context with `AgentExecutionContext::new()`
3. Creating executor via `AgentExecutorFactory::create_executor()`
4. Initializing executor before use
5. Wrapping in Arc and passing to RuleChecker

This aligns documentation with the actual implementation changes and shows users the correct configuration approach that respects `SAH_AGENT_EXECUTOR` environment variable and defaults to ClaudeCode.

**Verification:**
- ✅ cargo build: Success
- ✅ cargo nextest: All 3223 tests passed
- ✅ cargo clippy: No warnings
