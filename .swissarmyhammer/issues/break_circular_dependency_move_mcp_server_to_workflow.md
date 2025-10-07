# Break Circular Dependency: Move MCP Server Startup to Workflow Layer

## Problem

There is a circular dependency that prevents clean architecture:

```
swissarmyhammer-workflow
    ↓ depends on
swissarmyhammer-agent-executor
    ↓ depends on (via start_in_process_mcp_server)
swissarmyhammer-tools
    ↓ depends on
swissarmyhammer-agent-executor
```

This circular dependency exists because:
- `swissarmyhammer-agent-executor/src/llama/executor.rs:77` imports and calls `swissarmyhammer_tools::mcp::unified_server::start_mcp_server`
- The LlamaAgentExecutor's `initialize()` method is responsible for starting the MCP server
- This violates separation of concerns: an executor should execute, not manage infrastructure

## Root Cause

The LlamaAgentExecutor is doing too much:
- It starts the MCP server in its `initialize()` method (line 367)
- It manages MCP server lifecycle
- It couples agent execution to infrastructure management

This prevents `swissarmyhammer-tools` MCP implementations (like `rules_check`) from directly calling core crates like `swissarmyhammer-rules`, because that would create a cycle.

## Proposed Solution

Move MCP server lifecycle management to the **application layer** (workflow), where it belongs.

### New Architecture (No Circular Dependency)

```
swissarmyhammer-workflow
    ↓ depends on both
    ├─→ swissarmyhammer-tools (for MCP server)
    └─→ swissarmyhammer-agent-executor (for execution)

swissarmyhammer-agent-executor
    ↓ NO dependency on tools
    (pure execution logic)
```

### Key Insight

The user's observation is correct: **`create_executor` should create the MCP server and hand it to the actual agents**.

In `swissarmyhammer-workflow/src/actions.rs:242`, the `AgentExecutorFactory::create_executor()` method is the perfect place to:
1. Start the MCP server using `swissarmyhammer_tools::mcp::unified_server::start_mcp_server()`
2. Pass the MCP server handle to the executor

## Implementation Plan

### 1. Modify LlamaAgentExecutor Constructor

**File**: `swissarmyhammer-agent-executor/src/llama/executor.rs`

Change the constructor to accept an optional pre-started MCP server:

```rust
pub fn new(config: LlamaAgentConfig, mcp_server: Option<McpServerHandle>) -> Self {
    Self {
        config,
        mcp_server,  // Use provided server instead of None
        agent_server: None,
        initialized: false,
    }
}
```

### 2. Update initialize() Method

**File**: `swissarmyhammer-agent-executor/src/llama/executor.rs:360-405`

Remove MCP server startup logic:

```rust
pub async fn initialize(&mut self) -> Result<(), ActionError> {
    if self.initialized {
        return Ok(());
    }

    // Validate that MCP server was provided
    if self.mcp_server.is_none() {
        return Err(ActionError::ExecutionError(
            "MCP server must be provided before initialization".to_string()
        ));
    }

    // MCP server is already running - just use it
    let mcp_handle = self.mcp_server.as_ref().unwrap();
    
    tracing::info!(
        "Using pre-started HTTP MCP server on port {} (URL: {})",
        mcp_handle.port(),
        mcp_handle.url()
    );

    // Convert config to llama-agent format and initialize agent server
    let agent_config = self.to_llama_agent_config()?;
    let agent_server = AgentServer::initialize(agent_config).await?;
    
    self.agent_server = Some(Arc::new(agent_server));
    self.initialized = true;
    
    Ok(())
}
```

### 3. Remove MCP Server Functions

**File**: `swissarmyhammer-agent-executor/src/llama/executor.rs`

Delete these functions entirely:
- `start_in_process_mcp_server()` (lines 72-120)
- `start_http_mcp_server()` (lines 145-178)

### 4. Remove swissarmyhammer-tools Dependency

**File**: `swissarmyhammer-agent-executor/Cargo.toml`

Remove the dependency on `swissarmyhammer-tools`:
```toml
# DELETE THIS LINE:
swissarmyhammer-tools = { path = "../swissarmyhammer-tools" }
```

### 5. Update AgentExecutorFactory

**File**: `swissarmyhammer-workflow/src/actions.rs:242-270`

Start MCP server before creating executor:

```rust
pub async fn create_executor(
    context: &AgentExecutionContext<'_>,
) -> ActionResult<Box<dyn AgentExecutor>> {
    match context.executor_type() {
        AgentExecutorType::ClaudeCode => {
            tracing::info!("Using ClaudeCode");
            let mut executor = ClaudeCodeExecutor::new();
            executor.initialize().await?;
            Ok(Box::new(executor))
        }
        AgentExecutorType::LlamaAgent => {
            tracing::info!("Using LlamaAgent with singleton pattern");
            let agent_config = context.agent_config();
            let llama_config = match agent_config.executor {
                AgentExecutorConfig::LlamaAgent(config) => config,
                _ => {
                    return Err(ActionError::ExecutionError(
                        "Expected LlamaAgent configuration".to_string(),
                    ))
                }
            };
            
            // Start MCP server in workflow layer
            use swissarmyhammer_prompts::PromptLibrary;
            use swissarmyhammer_tools::mcp::unified_server::{start_mcp_server, McpServerMode};
            
            tracing::info!("Starting MCP server for LlamaAgent");
            let mcp_handle = start_mcp_server(
                McpServerMode::Http {
                    port: if llama_config.mcp_server.port == 0 {
                        None
                    } else {
                        Some(llama_config.mcp_server.port)
                    },
                },
                Some(PromptLibrary::default()),
            )
            .await
            .map_err(|e| ActionError::ExecutionError(format!("Failed to start MCP server: {}", e)))?;
            
            // Convert mcp_handle to the type expected by agent-executor
            let agent_mcp_handle = convert_mcp_handle(mcp_handle);
            
            // Pass MCP server to executor
            let mut executor = crate::agents::LlamaAgentExecutorWrapper::new_with_mcp(
                llama_config, 
                Some(agent_mcp_handle)
            );
            executor.initialize().await?;
            Ok(Box::new(executor))
        }
    }
}
```

### 6. Update LlamaAgentExecutorWrapper

**File**: `swissarmyhammer-workflow/src/agents/llama_agent_executor.rs:81-95`

Add constructor that accepts MCP server:

```rust
impl LlamaAgentExecutorWrapper {
    /// Create a new wrapper instance
    pub fn new(config: LlamaAgentConfig) -> Self {
        Self {
            inner: AgentExecutorLlamaAgentExecutorWrapper::new(config, None),
        }
    }
    
    /// Create a new wrapper instance with pre-started MCP server
    pub fn new_with_mcp(
        config: LlamaAgentConfig, 
        mcp_server: Option<McpServerHandle>
    ) -> Self {
        Self {
            inner: AgentExecutorLlamaAgentExecutorWrapper::new(config, mcp_server),
        }
    }
}
```

### 7. Handle McpServerHandle Type Conversion

Since the `McpServerHandle` type is defined in `swissarmyhammer-agent-executor`, we need to either:
- **Option A**: Move `McpServerHandle` to a shared crate (like `swissarmyhammer-common`)
- **Option B**: Create a conversion function in workflow
- **Option C**: Re-export the type from agent-executor and use it directly

**Recommendation**: Option C - just use the agent-executor's McpServerHandle type directly in workflow.

## Benefits

1. **Breaks Circular Dependency**: agent-executor no longer depends on tools
2. **Enables Direct Library Calls**: MCP tools can now directly call core crates like swissarmyhammer-rules
3. **Proper Separation of Concerns**: Executors execute, application layer manages infrastructure
4. **Better Testing**: Can inject mock MCP servers for testing
5. **Cleaner Architecture**: Each layer has clear responsibilities

## Migration Notes

- Existing tests that use LlamaAgentExecutor will need to provide an MCP server or use a test fixture
- The global singleton pattern can remain, but initialization must provide MCP server
- MCP server lifecycle is now owned by the workflow layer, which can shut it down cleanly

## Success Criteria

- [ ] `swissarmyhammer-agent-executor/Cargo.toml` has no dependency on `swissarmyhammer-tools`
- [ ] `cargo build` succeeds with no circular dependency errors
- [ ] All tests pass
- [ ] MCP tools in swissarmyhammer-tools can directly import and use swissarmyhammer-rules
- [ ] The rules_check MCP tool can be refactored to use RuleChecker directly instead of CLI subprocess
