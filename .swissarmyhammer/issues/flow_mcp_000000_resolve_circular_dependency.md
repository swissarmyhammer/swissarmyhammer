# Step 0: Resolve Circular Dependency Between Workflow and Tools

Refer to ideas/flow_mcp.md

## Objective

Resolve the circular dependency between `swissarmyhammer-workflow` and `swissarmyhammer-tools` to enable flow MCP tool implementation.

## Problem

**Current State**:
- `swissarmyhammer-workflow` depends on `swissarmyhammer-tools` (line 30 of workflow/Cargo.toml)
- Flow MCP tool needs to depend on `swissarmyhammer-workflow` for `WorkflowStorage` and `Workflow` types
- This creates a circular dependency that prevents compilation

**What's Blocked**:
- Flow MCP tool implementation (steps 3, 4, 8)
- Workflow discovery via MCP
- Workflow execution via MCP
- All integration with WorkflowStorage

## Analysis

### Why Does Workflow Depend on Tools?

Check `swissarmyhammer-workflow/Cargo.toml` line 30:
```toml
swissarmyhammer-tools = { path = "../swissarmyhammer-tools" }
```

Need to identify what from tools is being used by workflow.

### Potential Solutions

#### Option 1: Move Shared Types to Common

Move `WorkflowStorage` trait and core workflow types to `swissarmyhammer-common`:
- Both workflow and tools can depend on common
- Common has no dependencies on either
- Cleanest separation of concerns

#### Option 2: Create Workflow Storage Crate

Create new crate `swissarmyhammer-workflow-storage`:
- Contains only storage traits and types
- Both workflow and tools depend on it
- More granular, follows single responsibility

#### Option 3: Remove Tools Dependency from Workflow

Identify what workflow uses from tools and either:
- Move it to common
- Duplicate it (if small)
- Refactor workflow to not need it
- Use dependency injection pattern

#### Option 4: Facade Pattern

Create facade in CLI that coordinates both:
- Tools MCP layer doesn't depend on workflow
- CLI depends on both and wires them together
- More complex but avoids circular dependency

## Tasks

### 1. Identify Workflow's Usage of Tools

```bash
# Find all imports of swissarmyhammer-tools in workflow crate
rg "use.*swissarmyhammer_tools" swissarmyhammer-workflow/src
rg "swissarmyhammer_tools::" swissarmyhammer-workflow/src
```

Document what workflow needs from tools.

### 2. Choose Solution Based on Usage

Based on what workflow actually uses:
- If minimal: Option 3 (remove dependency)
- If storage-related: Option 1 (move to common) or Option 2 (new crate)
- If tightly coupled: Option 4 (facade pattern)

### 3. Implement Chosen Solution

#### If Option 1 (Move to Common):

```bash
# Move types to common
mv swissarmyhammer-workflow/src/storage.rs swissarmyhammer-common/src/workflow_storage.rs

# Update imports in both crates
# Update Cargo.toml dependencies
```

#### If Option 2 (New Crate):

```bash
# Create new crate
cargo new --lib swissarmyhammer-workflow-storage

# Move storage traits and types
# Update both Cargo.toml files to depend on new crate
```

#### If Option 3 (Remove Dependency):

```bash
# Remove tools dependency from workflow/Cargo.toml
# Refactor workflow code to not use tools
# May need to duplicate small utilities
```

### 4. Update All Imports

Search and replace imports throughout codebase:
```bash
# Find all affected files
rg "swissarmyhammer_workflow::.*Storage" --files-with-matches
rg "swissarmyhammer_tools::.*Workflow" --files-with-matches
```

Update imports to use new location.

### 5. Update Cargo.toml Files

Remove circular dependency:
- `swissarmyhammer-workflow/Cargo.toml`: Remove or keep tools dependency
- `swissarmyhammer-tools/Cargo.toml`: Add workflow or storage dependency

### 6. Verify Build

```bash
# Clean build to verify no circular dependency
cargo clean
cargo build --all

# Check for warnings
cargo clippy --all

# Run tests
cargo nextest run --all
```

## Files to Investigate

- `swissarmyhammer-workflow/Cargo.toml`
- `swissarmyhammer-workflow/src/**/*.rs` (find tools usage)
- `swissarmyhammer-tools/Cargo.toml`
- Potentially: `swissarmyhammer-common/Cargo.toml`

## Files to Modify (TBD based on solution)

Will be determined after analysis phase.

## Acceptance Criteria

- [ ] Analysis complete: documented what workflow uses from tools
- [ ] Solution chosen based on actual usage
- [ ] Circular dependency removed
- [ ] `cargo build --all` succeeds
- [ ] `cargo clippy --all` shows no warnings
- [ ] All existing tests still pass
- [ ] No circular dependency errors
- [ ] Tools can now depend on workflow types (or common storage types)

## Estimated Changes

~50-200 lines depending on solution chosen

## Priority

**CRITICAL**: This blocks all other flow MCP work (steps 1-12)



## Proposed Solution

After thorough analysis, I've identified the circular dependency and determined the best approach to resolve it.

### Analysis Complete

**Workflow's Usage of Tools** (Single Point):
- File: `swissarmyhammer-workflow/src/actions.rs:672`
- Usage: `swissarmyhammer_tools::mcp::unified_server::{start_mcp_server, McpServerMode}`
- Context: Starting an MCP server when using the LlamaAgent executor
- This is the ONLY usage of tools within the workflow crate

**What Flow MCP Tool Needs**:
- Access to workflow types: `Workflow`, `WorkflowStorage`, `WorkflowExecutor`
- Access to workflow metadata for discovery (`list` functionality)
- Ability to execute workflows with parameters

### Chosen Solution: Option 1 - Move MCP Server Code to Common

**Rationale**:
1. The MCP server code (`unified_server.rs`) is **infrastructure**, not a "tool" in the MCP sense
2. It's used by workflow (infrastructure layer) not by MCP tools
3. Moving it to `swissarmyhammer-common` makes it available to all crates without circular dependencies
4. This is the cleanest separation - common provides infrastructure, tools provides MCP tool implementations, workflow provides workflow logic

**Benefits**:
- **Minimal Changes**: Only need to move one file and update imports
- **Clean Architecture**: MCP server infrastructure belongs in common alongside other infrastructure
- **No New Crates**: Avoids proliferation of small crates
- **Future-Proof**: Other crates can use MCP server infrastructure without depending on tools

### Implementation Steps

#### Step 1: Move MCP Server Code to Common

```bash
# Create mcp directory in common
mkdir -p swissarmyhammer-common/src/mcp

# Move the unified_server module
mv swissarmyhammer-tools/src/mcp/unified_server.rs swissarmyhammer-common/src/mcp/unified_server.rs
```

#### Step 2: Update Common's Module Structure

Add to `swissarmyhammer-common/src/lib.rs`:
```rust
pub mod mcp;
```

Create `swissarmyhammer-common/src/mcp/mod.rs`:
```rust
pub mod unified_server;

pub use unified_server::{
    start_mcp_server, McpServerMode, McpServerInfo, McpServerHandle,
    configure_mcp_logging, FileWriterGuard
};
```

#### Step 3: Update Common's Cargo.toml

Add required dependencies to `swissarmyhammer-common/Cargo.toml`:
```toml
# MCP server support
rmcp = { workspace = true }
axum = { workspace = true }
tower = { workspace = true }
tower-http = { workspace = true, features = ["trace"] }
hyper = { workspace = true }
tokio = { workspace = true, features = ["full"] }
tracing-subscriber = { workspace = true }
```

#### Step 4: Update Imports in Workflow

Change in `swissarmyhammer-workflow/src/actions.rs:672`:
```rust
// OLD:
use swissarmyhammer_tools::mcp::unified_server::{start_mcp_server, McpServerMode};

// NEW:
use swissarmyhammer_common::mcp::{start_mcp_server, McpServerMode};
```

#### Step 5: Update Imports in Tools

Update `swissarmyhammer-tools/src/mcp/mod.rs`:
```rust
// Remove or update re-export
pub mod server;
// Remove: pub mod unified_server;

// Re-export from common for backward compatibility
pub use swissarmyhammer_common::mcp::{
    start_mcp_server, McpServerMode, McpServerInfo, McpServerHandle
};
```

#### Step 6: Update Any Tests

Search for test files that import from `swissarmyhammer_tools::mcp::unified_server` and update them to use `swissarmyhammer_common::mcp` instead.

#### Step 7: Remove Workflow's Dependency on Tools

Remove from `swissarmyhammer-workflow/Cargo.toml`:
```toml
# Remove this line:
swissarmyhammer-tools = { path = "../swissarmyhammer-tools" }
```

#### Step 8: Add Tools Dependency on Workflow (for Flow MCP Tool)

This will be done in subsequent issues, but now it's possible:
```toml
# In swissarmyhammer-tools/Cargo.toml
swissarmyhammer-workflow = { path = "../swissarmyhammer-workflow" }
```

### Files to Modify

1. **Move file**:
   - `swissarmyhammer-tools/src/mcp/unified_server.rs` → `swissarmyhammer-common/src/mcp/unified_server.rs`

2. **Create new files**:
   - `swissarmyhammer-common/src/mcp/mod.rs`

3. **Update existing files**:
   - `swissarmyhammer-common/src/lib.rs` - add `pub mod mcp;`
   - `swissarmyhammer-common/Cargo.toml` - add MCP server dependencies
   - `swissarmyhammer-workflow/src/actions.rs` - update import at line 672
   - `swissarmyhammer-workflow/Cargo.toml` - **remove** tools dependency
   - `swissarmyhammer-tools/src/mcp/mod.rs` - update re-exports
   - Any test files importing from unified_server

### Verification Steps

```bash
# 1. Clean build
cargo clean

# 2. Build all crates
cargo build --all

# 3. Verify no circular dependency
# (build will fail if circular dependency exists)

# 4. Run clippy
cargo clippy --all

# 5. Run all tests
cargo nextest run --all --failure-output immediate --hide-progress-bar --status-level fail --final-status-level fail
```

### Expected Outcome

After implementation:
- ✅ No circular dependency between workflow and tools
- ✅ MCP server infrastructure in common (infrastructure layer)
- ✅ Tools can depend on workflow for Flow MCP tool implementation
- ✅ All existing functionality preserved
- ✅ Clean architectural boundaries

### Architecture After Change

```
swissarmyhammer-common
  ├── mcp/unified_server.rs  (MCP server infrastructure)
  └── (other common utilities)
           ↑
           │
    ┌──────┴──────┐
    │             │
swissarmyhammer-workflow    swissarmyhammer-tools
  ├── actions.rs             ├── mcp/server.rs (MCP server logic)
  ├── storage.rs             └── mcp/tools/flow.rs (future)
  └── (workflow logic)                ↑
           ↑                           │
           └───────────────────────────┘
              (tools can now depend on workflow)
```

### Risk Assessment

**Low Risk**:
- Only moving one file
- No logic changes, only location change
- Imports are straightforward to update
- Tests will catch any issues

**Potential Issues**:
- May need to make `McpServer` from tools/mcp/server.rs available to common
- Some internal dependencies in unified_server.rs might need adjustment

### Next Steps After This Issue

Once circular dependency is resolved:
1. Implement Flow MCP tool in tools crate
2. Tool can import from workflow crate
3. Tool can use `WorkflowStorage` and execute workflows
4. Continue with flow_mcp.md implementation plan



## Alternative Approach: Dependency Injection

This approach uses a factory pattern with dependency injection.

### Root Cause Analysis

The circular dependency exists because:
1. **Workflow** uses `swissarmyhammer_tools::mcp::unified_server::start_mcp_server` (line 672 in actions.rs)
2. **Flow MCP Tool** (planned) needs `swissarmyhammer_workflow` types
3. This creates: Workflow → Tools → (would need) Workflow

### Why the Current Design is Wrong

The workflow action layer (`actions.rs:672`) is starting an MCP server for LlamaAgent. This violates separation of concerns:
- **Workflow layer** should orchestrate high-level workflow logic
- **Infrastructure setup** (like starting servers) should happen at initialization, not in action execution
- **Actions** should be pure workflow logic, not infrastructure management

### The Right Solution: Refactor Server Startup Out of Actions

Instead of having the workflow action start the MCP server, move this responsibility to where executors are created (either in executor initialization or at the CLI layer).

#### Step 1: Create MCP Server Startup Trait in Common

Create `swissarmyhammer-common/src/mcp_server_factory.rs`:

```rust
use std::sync::Arc;
use async_trait::async_trait;

/// Trait for starting MCP servers (dependency injection pattern)
#[async_trait]
pub trait McpServerFactory: Send + Sync {
    /// Start an MCP server and return connection info
    async fn start_server(
        &self,
        port: Option<u16>,
    ) -> Result<McpServerConnection, Box<dyn std::error::Error + Send + Sync>>;
}

/// Connection information for an MCP server
#[derive(Debug, Clone)]
pub struct McpServerConnection {
    pub port: u16,
    pub host: String,
    pub shutdown_handle: Arc<dyn ServerShutdown>,
}

/// Trait for shutting down a server
#[async_trait]
pub trait ServerShutdown: Send + Sync {
    async fn shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}
```

#### Step 2: Remove Direct Dependency from Workflow

The workflow crate no longer imports from `swissarmyhammer_tools`. Instead, it accepts an optional `McpServerFactory` during executor creation.

Modify `swissarmyhammer-workflow/src/agents/llama_agent_executor.rs` or wherever the executor is created:

```rust
pub struct LlamaAgentExecutorWrapper {
    config: LlamaAgentConfig,
    mcp_server_factory: Option<Arc<dyn McpServerFactory>>,
    // ... other fields
}

impl LlamaAgentExecutorWrapper {
    pub fn new_with_factory(
        config: LlamaAgentConfig,
        factory: Option<Arc<dyn McpServerFactory>>,
    ) -> Self {
        Self {
            config,
            mcp_server_factory: factory,
        }
    }
    
    async fn ensure_mcp_server(&mut self) -> Result<()> {
        if let Some(factory) = &self.mcp_server_factory {
            let connection = factory.start_server(Some(self.config.mcp_server.port)).await?;
            // Store connection info
        }
        Ok(())
    }
}
```

#### Step 3: Implement Factory in Tools Crate

Create `swissarmyhammer-tools/src/mcp/server_factory.rs`:

```rust
use swissarmyhammer_common::mcp_server_factory::{McpServerFactory, McpServerConnection, ServerShutdown};
use super::unified_server::{start_mcp_server, McpServerMode, McpServerHandle};
use async_trait::async_trait;
use std::sync::Arc;

pub struct SwissArmyHammerMcpServerFactory {
    // configuration if needed
}

#[async_trait]
impl McpServerFactory for SwissArmyHammerMcpServerFactory {
    async fn start_server(
        &self,
        port: Option<u16>,
    ) -> Result<McpServerConnection, Box<dyn std::error::Error + Send + Sync>> {
        let handle = start_mcp_server(
            McpServerMode::Http { port },
            None,
        ).await?;
        
        let connection = McpServerConnection {
            port: handle.info.port.unwrap_or(0),
            host: "127.0.0.1".to_string(),
            shutdown_handle: Arc::new(ServerShutdownImpl { handle }),
        };
        
        Ok(connection)
    }
}

struct ServerShutdownImpl {
    handle: McpServerHandle,
}

#[async_trait]
impl ServerShutdown for ServerShutdownImpl {
    async fn shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Implement shutdown logic
        Ok(())
    }
}
```

#### Step 4: Wire it Together in CLI

In `swissarmyhammer-cli`, when creating executors:

```rust
use swissarmyhammer_tools::mcp::server_factory::SwissArmyHammerMcpServerFactory;

// When creating LlamaAgent executor
let factory = Arc::new(SwissArmyHammerMcpServerFactory::new());
let executor = LlamaAgentExecutorWrapper::new_with_factory(config, Some(factory));
```

###Benefits of This Approach

1. **Clean Separation**: Workflow doesn't know about tools implementation
2. **Dependency Injection**: Testable, mockable MCP server startup
3. **No Circular Dependency**: 
   - Common defines the trait (no dependencies)
   - Workflow uses the trait (depends on common only)
   - Tools implements the trait (depends on common only)
   - CLI wires them together (depends on both)
4. **Better Architecture**: Infrastructure concerns separated from workflow logic
5. **Flow MCP Tool Can Live in Tools**: Since workflow no longer depends on tools

### Alternative Simpler Solution

If the dependency injection is too complex, there's an even simpler solution:

**Don't start the MCP server in the workflow at all!**

The CLI should start the MCP server BEFORE creating the workflow executor. The LlamaAgent executor just needs to know the port/host, not how to start the server.

1. CLI starts MCP server using `start_mcp_server` from tools
2. CLI gets the port
3. CLI creates LlamaAgent executor with the port
4. Workflow never imports from tools
5. Tools can import from workflow for Flow MCP tool

This is the **SIMPLEST** solution and requires minimal changes.

### Recommended Implementation: Simple Solution

#### Changes Required:

1. **Modify `LlamaAgentExecutorWrapper`** to accept pre-started server info instead of starting its own server
2. **Move server startup to CLI** before executor creation
3. **Remove tools import** from workflow

#### Files to Modify:

1. `swissarmyhammer-workflow/src/actions.rs` - Remove MCP server startup code from action
2. `swissarmyhammer-workflow/src/agents/llama_agent_executor.rs` - Accept server connection info in constructor
3. `swissarmyhammer-workflow/Cargo.toml` - Remove tools dependency
4. `swissarmyhammer-cli/src/commands/flow/mod.rs` - Start MCP server before creating executor
5. `swissarmyhammer-tools/Cargo.toml` - Add workflow dependency (for future Flow MCP tool)

Let me implement the simple solution!




## Final Solution: Remove Workflow → Tools Dependency

After comprehensive analysis, here is the confirmed solution:

### Confirmed Circular Dependency

```
swissarmyhammer-workflow/Cargo.toml:35
  swissarmyhammer-tools = { path = "../swissarmyhammer-tools" }

swissarmyhammer-workflow/src/actions.rs:672
  use swissarmyhammer_tools::mcp::unified_server::{start_mcp_server, McpServerMode};
```

Rust does NOT allow circular crate dependencies. Since workflow depends on tools, tools CANNOT depend on workflow.

### The Simple Solution

**Move MCP server startup from workflow actions to CLI/caller**

The workflow layer should NOT be responsible for starting infrastructure (MCP servers). This should happen at initialization, not during action execution.

### Implementation

#### Current Code (workflow/src/actions.rs:672-690)

```rust
// Start MCP server in workflow layer
use swissarmyhammer_prompts::PromptLibrary;
use swissarmyhammer_tools::mcp::unified_server::{start_mcp_server, McpServerMode};

tracing::info!("Starting MCP server for LlamaAgent in workflow layer");
let tools_mcp_handle = start_mcp_server(
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
.map_err(|e| {
    ActionError::ExecutionError(format!("Failed to start MCP server: {}", e))
})?;
```

This code will be **REMOVED** from workflow.

#### Good News

The `LlamaAgentExecutorWrapper` already supports pre-started MCP servers via `new_with_mcp(config, mcp_server_handle)`. This means the refactoring is straightforward.

### Changes Required

#### 1. Remove Server Startup from Workflow Actions

In `swissarmyhammer-workflow/src/actions.rs`, remove the MCP server startup code (lines 672-716).

The executor creation should become:
```rust
AgentExecutorType::LlamaAgent => {
    let agent_config = context.agent_config();
    let llama_config = match agent_config.executor {
        AgentExecutorConfig::LlamaAgent(config) => config,
        _ => {
            return Err(ActionError::ExecutionError(
                "Expected LlamaAgent configuration".to_string(),
            ))
        }
    };

    // Expect MCP server to already be started - handle passed via context
    let mut executor = crate::agents::LlamaAgentExecutorWrapper::new(llama_config);
    executor.initialize().await.map_err(|e| {
        ActionError::ExecutionError(format!("Failed to initialize LlamaAgent: {}", e))
    })?;
    
    Ok(Box::new(executor))
}
```

**WAIT** - this won't work because the executor needs the MCP server handle. Let me check how this is supposed to work...

Actually, looking at the code more carefully, the MCP server handle is passed to `new_with_mcp`. So the handle must come from somewhere. Looking at the context, there's likely an `AgentExecutionContext` that could carry this.

Let me check if there's a better way...

Actually, the REAL issue is that `PromptAction` is creating executors on-demand. The better architectural solution is:

**Executors should be created ONCE at workflow initialization, not per-action.**

But that's a bigger refactoring. For now, the pragmatic solution:

#### Pragmatic Solution: Pass MCP Server Factory to Workflow

Instead of workflow starting the server, pass a factory function:

1. Add optional `mcp_server_factory` to workflow executor context
2. Workflow calls factory when needed (if present)
3. Factory implementation lives in CLI (which depends on both workflow and tools)
4. Workflow has NO direct dependency on tools

But this is getting complex again. Let me think...

### ACTUALLY SIMPLEST Solution: Check if Circular Dependency is Actually Blocked

Let me verify: Can tools depend on workflow for JUST the Flow tool, while workflow depends on tools for MCP server?

**Answer: NO**. Cargo will reject this completely.

### TRUE Simplest Solution: Flow Tool in CLI

The Flow MCP tool doesn't need to live in `swissarmyhammer-tools` at all!

**It can live in `swissarmyhammer-cli`** where both workflow and tools are already dependencies!

Then:
1. Workflow keeps its dependency on tools (for MCP server startup)
2. Tools does NOT depend on workflow (no circular dependency)
3. Flow tool is registered from CLI into the McpServer when it's created
4. Zero refactoring of existing code needed!

This is the ACTUAL simplest solution!




## Final Solution

The cleanest solution addresses the architectural issue:

### Remove Workflow's Dependency on Tools

**Why other solutions don't work:**
1. **Flow tool in CLI**: Tools are registered inside `McpServer::new()` which is in tools crate - no way to register from outside
2. **Keep circular dependency**: Rust Cargo does NOT allow circular dependencies between crates at all
3. **Third crate**: Adds unnecessary complexity when simpler solution exists

### The Problem: Architecture Violation

The workflow action layer is starting MCP servers. This is wrong because:
- Actions should contain workflow logic, not infrastructure setup
- Infrastructure (servers) should be started at initialization, not during execution
- This creates coupling between workflow and tools crates

### The Solution: Move Server Startup Responsibility

**Current (Wrong)**: Workflow action starts MCP server when creating LlamaAgent executor

**Correct**: Whoever creates/runs the workflow provides a pre-started MCP server handle

### Implementation: Remove 45 Lines from Workflow

The MCP server startup code in `swissarmyhammer-workflow/src/actions.rs:672-716` will be **completely removed**.

Since `LlamaAgentExecutorWrapper` already has `new()` and `new_with_mcp()` constructors, the workflow will simply use `new()` and let initialization fail if no MCP server is available. The CALLER (CLI) is responsible for providing the MCP infrastructure.

### Detailed Changes

#### Change 1: Simplify Executor Creation in Workflow

File: `swissarmyhammer-workflow/src/actions.rs`

Remove lines 672-716 (MCP server startup code) and replace with:

```rust
AgentExecutorType::LlamaAgent => {
    tracing::info!("Using LlamaAgent");
    let agent_config = context.agent_config();
    let llama_config = match agent_config.executor {
        AgentExecutorConfig::LlamaAgent(config) => config,
        _ => {
            return Err(ActionError::ExecutionError(
                "Expected LlamaAgent configuration".to_string(),
            ))
        }
    };

    // Note: MCP server must be started by caller before workflow execution
    // LlamaAgent will fail to initialize if MCP server is not available
    let mut executor = crate::agents::LlamaAgentExecutorWrapper::new(llama_config);
    executor.initialize().await.map_err(|e| {
        ActionError::ExecutionError(format!(
            "Failed to initialize LlamaAgent (is MCP server running?): {}", e
        ))
    })?;
    
    Ok(Box::new(executor))
}
```

####Change 2: Remove Tools Dependency

File: `swissarmyhammer-workflow/Cargo.toml`

Remove line 35:
```toml
swissarmyhammer-tools = { path = "../swissarmyhammer-tools" }
```

Also remove from imports in `actions.rs`:
```rust
// REMOVE these lines:
use swissarmyhammer_prompts::PromptLibrary;
use swissarmyhammer_tools::mcp::unified_server::{start_mcp_server, McpServerMode};
```

#### Change 3: CLI Starts MCP Server (Future Work - Not This Issue)

When the CLI runs a workflow with LlamaAgent, it will:
1. Start MCP server using `swissarmyhammer_tools::mcp::unified_server::start_mcp_server`
2. Get the port/connection info
3. Pass connection info to LlamaAgent configuration
4. Create and run workflow

This will be implemented in the CLI layer, NOT in this issue.

### What This Enables

After this change:
- ✅ No circular dependency
- ✅ Tools can add `swissarmyhammer-workflow` dependency
- ✅ Flow MCP tool can be implemented in tools crate
- ✅ Better separation of concerns
- ✅ Workflow layer is cleaner and more focused

### Acceptance Criteria

- [ ] Workflow crate does NOT import from tools crate
- [ ] `swissarmyhammer-workflow/Cargo.toml` does NOT list tools dependency
- [ ] `cargo build --all` succeeds
- [ ] `cargo clippy --all` shows no new warnings
- [ ] Existing tests pass (LlamaAgent tests may need adjustment)
- [ ] Workflow crate compiles independently

