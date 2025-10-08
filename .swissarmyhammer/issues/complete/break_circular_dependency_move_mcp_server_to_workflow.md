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



## Test Migration

Tests in `swissarmyhammer-agent-executor/src/llama/executor.rs` that call `initialize()` need to be updated to provide an MCP server handle.

### Option 1: Use External Echo MCP Server

For tests, use the public echo MCP server at `https://echo.mcp.inevitable.fyi/mcp`:

```rust
async fn create_test_mcp_handle() -> McpServerHandle {
    // Use public echo MCP server for testing
    let (dummy_tx, _dummy_rx) = tokio::sync::oneshot::channel();
    McpServerHandle::new(
        443,  // HTTPS port
        "echo.mcp.inevitable.fyi".to_string(),
        dummy_tx,
    )
}

#[test_log::test(tokio::test)]
async fn test_llama_agent_executor_initialization() {
    let config = LlamaAgentConfig::for_testing();
    let mcp_handle = create_test_mcp_handle().await;
    let mut executor = LlamaAgentExecutor::new(config, Some(mcp_handle));
    
    executor.initialize().await.expect("Initialization must succeed");
    // ... rest of test
}
```

### Option 2: Mock MCP Server Handle

For unit tests that don't need real MCP functionality:

```rust
#[cfg(test)]
fn create_mock_mcp_handle() -> McpServerHandle {
    let (dummy_tx, _dummy_rx) = tokio::sync::oneshot::channel();
    McpServerHandle::new(8080, "127.0.0.1".to_string(), dummy_tx)
}
```

### Tests That Need Updating

1. `test_llama_agent_executor_initialization()` - needs MCP handle
2. `test_llama_agent_executor_initialization_with_validation()` - needs MCP handle
3. `test_llama_agent_executor_initialization_with_invalid_config()` - should fail before MCP check
4. `test_llama_agent_executor_global_management()` - needs MCP handle
5. `test_llama_agent_executor_execute_with_init()` - needs MCP handle
6. `test_llama_agent_executor_random_port()` - needs TWO MCP handles
7. `test_llama_agent_executor_drop_cleanup()` - needs MCP handle



## Proposed Solution - Implementation Approach

After analyzing the code, here's the exact approach I'll take:

### Key Findings from Code Analysis

1. **Current MCP Server Startup Location**: 
   - `swissarmyhammer-agent-executor/src/llama/executor.rs:72-120` - `start_in_process_mcp_server()`
   - `swissarmyhammer-agent-executor/src/llama/executor.rs:145-178` - `start_http_mcp_server()`
   - Called from `initialize_agent_server_real()` at line 360

2. **McpServerHandle Type**:
   - Defined in agent-executor at line 31-65
   - Contains: port, url, shutdown_tx
   - Already has proper constructor and methods

3. **Current Constructor**:
   - `LlamaAgentExecutor::new()` takes only `LlamaAgentConfig`
   - Sets `mcp_server: None` initially
   - MCP server started during `initialize()`

4. **Wrapper Layer**:
   - `swissarmyhammer-workflow/src/agents/llama_agent_executor.rs`
   - Contains thin adapter wrappers
   - `LlamaAgentExecutorWrapper::new()` needs new constructor variant

### Implementation Steps

#### Step 1: Modify LlamaAgentExecutor Constructor
Change signature to accept optional pre-started MCP server:
```rust
pub fn new(config: LlamaAgentConfig, mcp_server: Option<McpServerHandle>) -> Self {
    Self {
        config,
        initialized: false,
        mcp_server,  // Use provided server instead of always None
        agent_server: None,
    }
}
```

#### Step 2: Refactor initialize() Method
Replace MCP server startup with validation:
```rust
async fn initialize_agent_server_real(&mut self) -> ActionResult<()> {
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

    // Rest of initialization remains the same...
}
```

#### Step 3: Delete MCP Server Functions
Remove these functions entirely:
- Lines 72-120: `start_in_process_mcp_server()`
- Lines 145-178: `start_http_mcp_server()`

#### Step 4: Remove Circular Dependency
Delete from `swissarmyhammer-agent-executor/Cargo.toml`:
```toml
swissarmyhammer-tools = { path = "../swissarmyhammer-tools" }
```

This breaks the cycle:
```
swissarmyhammer-workflow
    ↓ depends on both
    ├─→ swissarmyhammer-tools (for MCP server)
    └─→ swissarmyhammer-agent-executor (for execution)

swissarmyhammer-agent-executor
    ✓ NO dependency on tools (cycle broken!)
```

#### Step 5: Add Workflow MCP Startup
In `swissarmyhammer-workflow/src/actions.rs` around line 257:
```rust
AgentExecutorType::LlamaAgent => {
    tracing::info!("Using LlamaAgent with singleton pattern");
    let agent_config = context.agent_config();
    let llama_config = match agent_config.executor {
        AgentExecutorConfig::LlamaAgent(config) => config,
        _ => return Err(ActionError::ExecutionError(
            "Expected LlamaAgent configuration".to_string(),
        ))
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
    
    // Convert tools McpServerHandle to agent-executor McpServerHandle
    let agent_mcp_handle = convert_mcp_handle(mcp_handle);
    
    // Pass MCP server to wrapper
    let mut executor = crate::agents::LlamaAgentExecutorWrapper::new_with_mcp(
        llama_config, 
        Some(agent_mcp_handle)
    );
    executor.initialize().await?;
    Ok(Box::new(executor))
}
```

#### Step 6: Handle McpServerHandle Type Conversion
The challenge: Two different `McpServerHandle` types:
- `swissarmyhammer_tools::mcp::unified_server::ServerHandle`
- `swissarmyhammer_agent_executor::llama::McpServerHandle`

**Solution**: Extract data from tools handle and create agent-executor handle:
```rust
fn convert_mcp_handle(
    tools_handle: swissarmyhammer_tools::mcp::unified_server::ServerHandle
) -> swissarmyhammer_agent_executor::llama::McpServerHandle {
    let port = tools_handle.info.port.unwrap_or(0);
    let (dummy_tx, _dummy_rx) = tokio::sync::oneshot::channel();
    swissarmyhammer_agent_executor::llama::McpServerHandle::new(
        port,
        "127.0.0.1".to_string(),
        dummy_tx,
    )
}
```

Note: This creates a dummy shutdown channel because the real shutdown is managed by tools' ServerHandle.

#### Step 7: Update Wrapper Constructors
In `swissarmyhammer-workflow/src/agents/llama_agent_executor.rs`:
```rust
impl LlamaAgentExecutorWrapper {
    pub fn new(config: LlamaAgentConfig) -> Self {
        Self {
            inner: AgentExecutorLlamaAgentExecutorWrapper::new(config, None),
        }
    }
    
    pub fn new_with_mcp(
        config: LlamaAgentConfig, 
        mcp_server: Option<swissarmyhammer_agent_executor::llama::McpServerHandle>
    ) -> Self {
        Self {
            inner: AgentExecutorLlamaAgentExecutorWrapper::new(config, mcp_server),
        }
    }
}
```

#### Step 8: Update All Tests
Tests need to provide MCP server handles. Two approaches:

**Approach A: Start real MCP server in tests**
```rust
async fn create_test_mcp_handle() -> McpServerHandle {
    use swissarmyhammer_tools::mcp::unified_server::{start_mcp_server, McpServerMode};
    let handle = start_mcp_server(McpServerMode::Http { port: None }, None)
        .await
        .expect("Failed to start test MCP server");
    
    let port = handle.info.port.unwrap_or(0);
    let (dummy_tx, _dummy_rx) = tokio::sync::oneshot::channel();
    McpServerHandle::new(port, "127.0.0.1".to_string(), dummy_tx)
}
```

**Approach B: Use dummy handle for unit tests**
```rust
fn create_mock_mcp_handle() -> McpServerHandle {
    let (dummy_tx, _dummy_rx) = tokio::sync::oneshot::channel();
    McpServerHandle::new(8080, "127.0.0.1".to_string(), dummy_tx)
}
```

Tests to update:
- `test_llama_agent_executor_initialization`
- `test_llama_agent_executor_global_management`
- `test_llama_agent_executor_execute_with_init`
- `test_llama_agent_executor_drop_cleanup`
- Integration tests in `swissarmyhammer/tests/llama_mcp_e2e_test.rs`

### Benefits of This Approach

1. **Clean Separation**: Workflow layer manages infrastructure, executor just executes
2. **No Circular Dependency**: agent-executor no longer imports tools
3. **Testability**: Can inject mock MCP servers for testing
4. **Future Flexibility**: Easy to add other MCP server sources

### Potential Issues & Solutions

**Issue**: Dummy shutdown channel in conversion
**Solution**: Acceptable because tools' ServerHandle manages the real lifecycle

**Issue**: Two MCP handle types to maintain
**Solution**: Minimal burden, types are simple and unlikely to change

**Issue**: Tests become more complex
**Solution**: Create test utilities to simplify MCP handle creation



## Implementation Complete

Successfully implemented the solution to break the circular dependency by moving MCP server startup to the workflow layer.

### Changes Made

1. **Modified LlamaAgentExecutor Constructor** (swissarmyhammer-agent-executor/src/llama/executor.rs)
   - Changed `new()` to accept `Option<McpServerHandle>`
   - Updated documentation to explain the new architecture

2. **Refactored initialize() Method**
   - Removed MCP server startup logic
   - Added validation to ensure MCP server was provided before initialization
   - MCP server must now be provided by the workflow layer

3. **Deleted MCP Server Startup Functions**
   - Removed `start_in_process_mcp_server()`
   - Removed `start_http_mcp_server()`

4. **Broke Circular Dependency**
   - Removed `swissarmyhammer-tools` from dependencies in agent-executor/Cargo.toml
   - Added it back as dev-dependency for tests only
   - **No circular dependency anymore!**

5. **Updated Workflow Layer** (swissarmyhammer-workflow/src/actions.rs)
   - `AgentExecutorFactory::create_executor()` now starts MCP server before creating executor
   - Converts tools' `McpServerHandle` to agent-executor's `McpServerHandle`
   - Proper separation: workflow manages infrastructure, executor handles execution

6. **Updated Wrapper Constructors** (swissarmyhammer-workflow/src/agents/llama_agent_executor.rs)
   - Added `new_with_mcp()` constructor
   - Updated adapter to pass MCP server through

7. **Updated All Tests**
   - All tests in agent-executor now start MCP server before creating executor
   - Tests use real MCP server from swissarmyhammer-tools
   - All agent-executor tests passing ✅

8. **Updated shutdown() Behavior**
   - Executor no longer shuts down MCP server (owned by workflow layer)
   - MCP server handle remains in executor after shutdown
   - Clean separation of concerns

### Verification

- ✅ `cargo build` succeeds with no circular dependency errors
- ✅ All agent-executor tests pass
- ✅ Integration tests work with new architecture
- ✅ Dependency graph is now acyclic

### Architecture Achieved

```
swissarmyhammer-workflow
    ↓ depends on both
    ├─→ swissarmyhammer-tools (for MCP server)
    └─→ swissarmyhammer-agent-executor (for execution)

swissarmyhammer-agent-executor
    ✓ NO dependency on tools (cycle broken!)
    ↓ only depends on
    swissarmyhammer-config, swissarmyhammer-prompts
```

### Unrelated Test Failures

Note: There are 6 test failures in `swissarmyhammer-cli` rule check tests, but these are pre-existing and unrelated to this circular dependency fix.



## Code Review Completed

### Changes Made

1. **Deleted Dead Code** (swissarmyhammer-agent-executor/src/llama/executor.rs:25)
   - Removed unused `RANDOM_PORT_DISPLAY` constant
   - Removed unused test imports: `sleep` and `Duration as TokioDuration`

2. **Verification Results**
   - ✅ `cargo clippy` - No warnings
   - ✅ `cargo nextest run -p swissarmyhammer-agent-executor` - All 15 tests passed
   - ✅ `cargo nextest run -p swissarmyhammer --test llama_mcp_e2e_test` - All 4 integration tests passed
   
3. **Pre-existing Test Failures**
   - 6 failing tests in `swissarmyhammer-cli` rule check tests (unrelated to this issue)
   - These failures existed before this work and are not caused by the circular dependency fix

### Compliance

All critical issues from code review are resolved:
- Dead code removed
- No clippy warnings
- All relevant tests passing
- Circular dependency successfully broken

The implementation is complete and ready for the next workflow step.
