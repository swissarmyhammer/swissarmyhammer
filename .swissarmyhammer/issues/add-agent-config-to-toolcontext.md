# Add Agent Configuration to ToolContext

## Dependencies

⚠️ **BLOCKED BY**: `move-claudecode-executor-to-agent-executor` must be completed first.

⚠️ **BLOCKS**: `support-claudecode-rule-checking` depends on this being completed.

---

## Problem

The MCP `rules_check` tool is hardcoded to use LlamaAgent, ignoring the user's configured agent executor:

**swissarmyhammer-tools/src/mcp/tools/rules/check/mod.rs:64-66**
```rust
let config = LlamaAgentConfig::for_testing();
let agent: Arc<dyn AgentExecutor> =
    Arc::new(LlamaAgentExecutorWrapper::new(config));
```

This happens because `ToolContext` doesn't provide access to agent configuration.

**swissarmyhammer-tools/src/mcp/tool_registry.rs:264**
```rust
pub struct ToolContext {
    pub tool_handlers: Arc<ToolHandlers>,
    pub issue_storage: Arc<RwLock<Box<dyn IssueStorage>>>,
    pub git_ops: Arc<Mutex<Option<GitOperations>>>,
    pub memo_storage: Arc<RwLock<Box<dyn MemoStorage>>>,
    // ❌ NO AGENT CONFIG!
}
```

## Required Changes

### 1. Add Agent Config to ToolContext

```rust
pub struct ToolContext {
    pub tool_handlers: Arc<ToolHandlers>,
    pub issue_storage: Arc<RwLock<Box<dyn IssueStorage>>>,
    pub git_ops: Arc<Mutex<Option<GitOperations>>>,
    pub memo_storage: Arc<RwLock<Box<dyn MemoStorage>>>,
    pub agent_config: Arc<AgentConfig>,  // ✅ ADD THIS
}
```

### 2. Update MCP Server Initialization

Wherever `ToolContext` is created, pass the agent configuration:

- Find all places where `ToolContext::new()` or similar is called
- Pass the user's agent configuration
- Ensure it respects `SAH_AGENT_EXECUTOR` environment variable
- Default to ClaudeCode if not specified

### 3. Update RuleCheckTool to Use ToolContext Agent Config

**swissarmyhammer-tools/src/mcp/tools/rules/check/mod.rs**

Change from:
```rust
async fn get_checker(&self) -> Result<&RuleChecker, McpError> {
    self.checker
        .get_or_try_init(|| async {
            let config = LlamaAgentConfig::for_testing();  // ❌ HARDCODED
            let agent: Arc<dyn AgentExecutor> =
                Arc::new(LlamaAgentExecutorWrapper::new(config));
            // ...
        })
}
```

To:
```rust
async fn get_checker(&self, context: &ToolContext) -> Result<&RuleChecker, McpError> {
    self.checker
        .get_or_try_init(|| async {
            let agent = create_agent_from_config(&context.agent_config).await?;  // ✅ USE CONTEXT
            // ...
        })
}
```

### 4. Implement Agent Factory Function

Create a helper to instantiate agents from configuration:

```rust
async fn create_agent_from_config(config: &AgentConfig) -> Result<Arc<dyn AgentExecutor>, McpError> {
    match &config.executor {
        AgentExecutorConfig::ClaudeCode(claude_config) => {
            let mut executor = ClaudeCodeExecutor::new();
            executor.initialize().await?;
            Ok(Arc::new(executor))
        }
        AgentExecutorConfig::LlamaAgent(llama_config) => {
            let mut executor = LlamaAgentExecutorWrapper::new(llama_config.clone());
            executor.initialize().await?;
            Ok(Arc::new(executor))
        }
    }
}
```

## Implementation Requirements

- [ ] Add `agent_config: Arc<AgentConfig>` to `ToolContext` struct
- [ ] Update all `ToolContext` construction sites to pass agent config
- [ ] Update `RuleCheckTool::get_checker()` signature to accept `&ToolContext`
- [ ] Implement agent factory function to create executors from config
- [ ] Update MCP server startup to load agent config from environment/defaults
- [ ] Test with both ClaudeCode and LlamaAgent configurations
- [ ] Update documentation for ToolContext

## Acceptance Criteria

- ToolContext includes agent configuration
- MCP tools respect user's configured agent executor
- RuleCheckTool uses the configured agent (ClaudeCode or LlamaAgent)
- Environment variable `SAH_AGENT_EXECUTOR` is respected
- No hardcoded executor types in MCP tools
- All existing MCP tools continue to work

## Notes

This is a critical architectural fix that enables MCP tools to respect user preferences and makes the system work as designed. Without this, rule checking will always fall back to LlamaAgent regardless of what the user configured.
