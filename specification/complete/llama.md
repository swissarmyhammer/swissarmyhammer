# LlamaAgent Integration Specification

## Overview

This specification outlines the integration of LlamaAgent into the existing SwissArmyHammer workflow system. The goal is to provide an alternative AI agent execution backend that can run locally alongside the current Claude Code shell-out approach.

## Current State Analysis

### Existing Claude Code Integration

The current `PromptAction` implementation in `/swissarmyhammer/src/workflow/actions.rs` executes workflow prompts by:

1. **Prompt Rendering**: Uses `PromptLibrary` and `PromptResolver` to render prompts with context variables
2. **Claude Shell-out**: Spawns a `claude` CLI process with arguments:
   - `--dangerously-skip-permissions`
   - `--print`
   - `--output-format stream-json`
   - `--verbose`
3. **Streaming Response**: Reads streaming JSON responses line-by-line from Claude's stdout
4. **Error Handling**: Comprehensive error handling for timeouts, rate limits, and IO errors

**Key Code Location**: `PromptAction::execute_once_internal()` at lines 373-470

### LlamaAgent Capabilities

From analyzing the llama-agent repository, the system provides:

- **Agent Server**: `AgentServer::initialize(config)` for setting up AI models
- **Session Management**: `create_session()` for managing conversation context
- **Tool Discovery**: `discover_tools()` for MCP tool integration
- **Message Processing**: Adding messages and generating responses
- **Streaming Support**: Async streaming response generation

## Proposed Architecture

### 1. Workflow Context Type

First, define a proper workflow execution context instead of using HashMap mush:

```rust
/// Proper typed workflow execution context
#[derive(Debug, Clone)]
pub struct WorkflowExecutionContext {
    /// Variable storage with type safety
    variables: HashMap<String, Value>,
    /// Workflow metadata
    workflow_name: String,
    workflow_id: String,
    /// Execution state
    current_step: usize,
    /// Agent configuration
    agent_config: AgentConfig,
}

#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Agent executor type -- enumerated, will contain type specific config (ClaudeAgentConfig, LlamaAgentConfig)
    pub executor_type: AgentExecutorType,
    /// Global quiet mode
    pub quiet: bool,
}

impl WorkflowExecutionContext {
    pub fn get_variable(&self, key: &str) -> Option<&Value> {
        self.variables.get(key)
    }
    
    pub fn set_variable(&mut self, key: String, value: Value) {
        self.variables.insert(key, value);
    }
    
    pub fn get_all_variables(&self) -> &HashMap<String, Value> {
        &self.variables
    }
    
    pub fn agent_config(&self) -> &AgentConfig {
        &self.agent_config
    }
}
```

### 2. Agent Execution Types and Trait

Define executor types as a proper enum:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentExecutorType {
    /// Shell out to Claude Code CLI
    ClaudeCode(ClaudeAgentConfig),
    /// Use local LlamaAgent with in-process execution
    LlamaAgent(LlamaAgentConfig),
}

impl AgentExecutorType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentExecutorType::ClaudeCode => "claude-code",
            AgentExecutorType::LlamaAgent => "llama-agent",
        }
    }
}

impl Default for AgentExecutorType {
    fn default() -> Self {
        AgentExecutorType::ClaudeCode
    }
}
```

Create a trait to abstract prompt execution across different AI backends:

```rust
/// Execution context for agent prompt execution
/// Uses the proper typed workflow execution context
#[derive(Debug)]
pub struct AgentExecutionContext<'a> {
    /// Reference to the properly typed workflow execution context
    pub workflow_context: &'a WorkflowExecutionContext,
}

impl<'a> AgentExecutionContext<'a> {
    pub fn new(workflow_context: &'a WorkflowExecutionContext) -> Self {
        Self { workflow_context }
    }
    
    /// Get agent configuration from workflow context
    pub fn agent_config(&self) -> &AgentConfig {
        self.workflow_context.agent_config()
    }
    
    /// Get executor type
    pub fn executor_type(&self) -> AgentExecutorType {
        self.agent_config().executor_type
    }
    
    /// Get LlamaAgent config if configured
    pub fn llama_config(&self) -> Option<&LlamaAgentConfig> {
        self.agent_config().llama_config.as_ref()
    }
    
    /// Check if quiet mode is enabled
    pub fn quiet(&self) -> bool {
        self.agent_config().quiet
    }
}

#[async_trait::async_trait]
pub trait AgentExecutor: Send + Sync {
    /// Execute a rendered prompt and return the response
    async fn execute_prompt(
        &self,
        system_prompt: String,
        rendered_prompt: String,
        context: &AgentExecutionContext,
        timeout: Duration,
    ) -> ActionResult<Value>;
    
    /// Get the executor type enum
    fn executor_type(&self) -> AgentExecutorType;
    
    /// Initialize the executor with configuration
    async fn initialize(&self, config: Option<Value>) -> ActionResult<()>;
    
    /// Shutdown the executor and cleanup resources
    async fn shutdown(&self) -> ActionResult<()>;
}
```

### 2. Claude Code Implementation

Refactor the existing Claude Code integration into a trait implementation:

```rust
pub struct ClaudeCodeExecutor {
    claude_path: PathBuf,
}

#[async_trait::async_trait]
impl AgentExecutor for ClaudeCodeExecutor {
    async fn execute_prompt(
        &self,
        system_prompt: String,
        rendered_prompt: String,
        context: &AgentExecutionContext,
        timeout: Duration,
    ) -> ActionResult<Value> {
        // Move existing execute_once_internal logic here
        // Access quiet mode via context.quiet
        // Use context.variables for variable substitution
        // Maintain current Claude CLI spawning and streaming logic
    }
    
    fn executor_type(&self) -> AgentExecutorType {
        AgentExecutorType::ClaudeCode
    }
    
    async fn initialize(&self, _config: Option<Value>) -> ActionResult<()> {
        // Verify claude CLI is available in PATH
        self.claude_path = which::which("claude")?;
        Ok(())
    }
    
    async fn shutdown(&self) -> ActionResult<()> {
        // No cleanup needed for CLI approach
        Ok(())
    }
}
```

### 3. LlamaAgent Implementation

Create a new LlamaAgent executor with lazy initialization and session-per-prompt-action:

```rust
use std::sync::Arc;
use tokio::sync::OnceCell;

pub struct LlamaAgentExecutor {
    // Lazy-initialized global agent server (shared across all prompts)
    agent_server: Arc<OnceCell<AgentServer>>,
    // Config for this executor instance
    config: LlamaAgentConfig,
}

#[async_trait::async_trait]
impl AgentExecutor for LlamaAgentExecutor {
    async fn execute_prompt(
        &self,
        system_prompt: String,
        rendered_prompt: String,
        context: &AgentExecutionContext,
        timeout: Duration,
    ) -> ActionResult<Value> {
        // 1. Get or lazy-initialize the global agent server
        let agent = self.get_or_init_agent().await?;
        
        // 2. Create a NEW session for this prompt execution
        // This ensures clean state per prompt while reusing the loaded model
        let mut session = agent.create_session().await
            .map_err(|e| ActionError::ExecutionError(format!("Failed to create session: {}", e)))?;
            
        // 3. Discover tools for this session (MCP server is already running)
        agent.discover_tools(&mut session).await
            .map_err(|e| ActionError::ExecutionError(format!("Failed to discover tools: {}", e)))?;

        // 3.5. Add system prompt ...
        
        // 4. Add user message to this session
        let message = Message {
            role: MessageRole::User,
            content: rendered_prompt,
        };
        agent.add_message(&session.id, message).await
            .map_err(|e| ActionError::ExecutionError(format!("Failed to add message: {}", e)))?;
        
        // 5. Generate response with timeout
        // Model uses its own configured generation parameters
        let request = GenerationRequest::new(session.id.clone())
            .with_timeout(timeout);
            
        let response = agent.generate(request).await
            .map_err(|e| ActionError::ExecutionError(format!("Failed to generate response: {}", e)))?;
        
        // 6. Session automatically cleaned up when it goes out of scope
        
        // 7. Convert response to SwissArmyHammer format
        Ok(Value::String(response.content))
    }
    
    fn executor_type(&self) -> AgentExecutorType {
        AgentExecutorType::LlamaAgent
    }
    
    async fn initialize(&self, _config: Option<Value>) -> ActionResult<()> {
        // No-op - config is passed to constructor, initialization is lazy
        Ok(())
    }
    
    async fn shutdown(&self) -> ActionResult<()> {
        // No cleanup needed
        Ok(())
    }
}

impl LlamaAgentExecutor {
    pub fn new(config: LlamaAgentConfig) -> Self {
        Self {
            agent_server: Arc::new(OnceCell::new()),
            config,
        }
    }

    /// Get or lazy-initialize the global agent server
    /// This ensures the model is loaded only once and reused across all prompts
    async fn get_or_init_agent(&self) -> ActionResult<&AgentServer> {
        self.agent_server.get_or_try_init(|| async {
            // 1. Start global MCP server first (needed for tool discovery)
            let mcp_server = get_or_init_global_mcp_server().await?;
            
            // 2. Initialize agent with MCP tools configuration
            let mut agent_config = self.config.clone();
            
            // 3. Configure agent to use the global MCP server
            agent_config.mcp_config = Some(McpConfig {
                server_url: format!("http://localhost:{}", mcp_server.port()),
                timeout: Duration::from_secs(self.config.mcp_server.timeout_seconds),
            });
            
            // 4. Initialize agent (this loads the model)
            tracing::info!("Initializing LlamaAgent server with model: {:?}", agent_config.model);
            let agent = AgentServer::initialize(agent_config).await
                .map_err(|e| ActionError::ExecutionError(format!("Failed to initialize agent: {}", e)))?;
            
            Ok(agent)
        }).await
    }
}

/// Get or lazy-initialize the global MCP server in HTTP mode
/// This ensures the HTTP MCP server stays running for the entire process lifetime
/// LlamaAgent REQUIRES HTTP mode - stdio mode won't work for this integration
async fn get_or_init_global_mcp_server() -> ActionResult<&'static Arc<McpServerHandle>> {
    GLOBAL_MCP_SERVER.get_or_try_init(|| async {
        tracing::info!("Starting global HTTP MCP server for LlamaAgent integration");
        
        // 1. MUST use HTTP mode - LlamaAgent needs HTTP transport to connect to MCP server
        // This is the SAME server that `sah serve http` would use, not `sah serve` (stdio)
        let server_handle = crate::mcp::start_http_server("127.0.0.1:0").await
            .map_err(|e| ActionError::ExecutionError(format!("Failed to start MCP HTTP server: {}", e)))?;
        
        // 2. Server already has ALL SwissArmyHammer tools registered via existing implementation
        // No need to register anything - it's the same server infrastructure as `sah serve http`
        
        tracing::info!("Global MCP HTTP server ready on port {} for LlamaAgent", server_handle.port());
        Ok(Arc::new(server_handle))
    }).await
}

impl LlamaAgentExecutor {
    async fn start_mcp_server(&self) -> ActionResult<Arc<McpServer>> {
        // Start in-process HTTP server for MCP tools
        // Bind to localhost:0 for random available port
        // Register all SwissArmyHammer MCP tools
        // Return server handle for cleanup
    }
    
    fn parse_config(&self, config: Option<Value>) -> ActionResult<AgentConfig> {
        // Parse configuration with defaults:
        // - Model: Local GGUF model or HuggingFace repo
        // - Temperature: 0.7
        // - Top-p: 0.9
        // - Max tokens: 4096
    }
}
```

### 4. Global Resource Management

Add global resource management to ensure MCP server outlives individual executors:

```rust
use tokio::sync::OnceCell;

// Global MCP server handle - must outlive all executors
// This ensures the HTTP MCP server stays running for the entire process lifetime
static GLOBAL_MCP_SERVER: OnceCell<Arc<McpServerHandle>> = OnceCell::const_new();

// Global singleton for LlamaAgent executor 
// This ensures the model is loaded once per process, not per prompt
static GLOBAL_LLAMA_EXECUTOR: OnceCell<LlamaAgentExecutor> = OnceCell::const_new();
```

### 5. PromptAction Integration

Modify `PromptAction` to use the trait system:

```rust
impl PromptAction {
    async fn execute_once_internal(
        &self,
        workflow_context: &mut WorkflowExecutionContext,
    ) -> ActionResult<Value> {
        // 1. Render prompt using existing logic with proper context
        let rendered_prompt = self.render_prompt_directly(workflow_context).await?;
        
        // 2. Create agent execution context
        let execution_context = AgentExecutionContext::new(workflow_context);
        
        // 3. Get executor from configuration
        let executor = self.get_executor(&execution_context).await?;
        
        // 4. Execute prompt through trait
        executor.execute_prompt(
            rendered_prompt,
            &execution_context,
            self.timeout,
        ).await
    }
    
    async fn get_executor(&self, context: &AgentExecutionContext<'_>) -> ActionResult<Box<dyn AgentExecutor>> {
        match context.executor_type() {
            AgentExecutorType::ClaudeCode => {
                let executor = ClaudeCodeExecutor::new();
                executor.initialize(None).await?;
                Ok(Box::new(executor))
            },
            AgentExecutorType::LlamaAgent => {
                // Get or create the global LlamaAgent executor
                // This ensures the AgentServer is created once per process
                let config = context.llama_config()
                    .cloned()
                    .unwrap_or_else(LlamaAgentConfig::default);
                    
                let executor = GLOBAL_LLAMA_EXECUTOR.get_or_try_init(|| async {
                    let executor = LlamaAgentExecutor::new(config);
                    executor.initialize(None).await?;
                    Ok(executor)
                }).await?;
                Ok(Box::new(executor.clone()))
            },
        }
    }
}
```

### 6. System Prompt

LlamaAgent will need to use the system prompt with MessageRole::System as the very first prompt in each session.

Before rendering the system prompt -- for all models, include a new variable `model` in the prompt rendering context.

This will be the hugging face name of the model for LlamaAgent models, or `claude` for claude code.

## MCP Server Architecture Requirements

### Dual MCP Server Support

SwissArmyHammer needs to support MCP in two modes to enable both CLI usage and LlamaAgent integration:

#### 1. Standalone MCP Server (`sah serve`)

For external clients like Claude Desktop, VS Code, etc:

```bash
# stdio mode (default for Claude Desktop integration)
sah serve

# HTTP mode for web clients and debugging
sah serve http
sah serve http 8080  # specific port
sah serve http --port 8080 --host 0.0.0.0  # bind to all interfaces
```

Note that this will require http!

#### 2. In-Process MCP Server (LlamaAgent)

For internal LlamaAgent integration:
1. **Lazy Server Creation**: MCP server created once when first LlamaAgent executor is used
2. **Global Server Instance**: Shared across all prompt executions to avoid port conflicts  
3. **Tool Registration**: All SwissArmyHammer MCP tools registered during server initialization
4. **Session-Level Tool Discovery**: Each new session discovers tools from the running MCP server

### Critical MCP Tool Registration Timing

With the lazy approach, MCP tools must be registered **before** any sessions are created:

```rust
async fn get_or_init_agent(&self) -> ActionResult<&AgentServer> {
    self.agent_server.get_or_try_init(|| async {
        // 1. FIRST: Start MCP server and register tools
        let mcp_server = self.get_or_init_mcp_server().await?;
        
        // 2. THEN: Initialize agent with MCP server URL
        let agent_config = AgentConfig {
            mcp_config: Some(McpConfig {
                server_url: format!("http://localhost:{}", mcp_server.port()),
            }),
            // ... other config
        };
        
        // 3. FINALLY: Create agent (which can now discover tools)
        AgentServer::initialize(agent_config).await
    }).await
}
```

### Tool Discovery Per Session

Each prompt execution creates a fresh session and discovers tools:

```rust
async fn execute_prompt(&self, ...) -> ActionResult<Value> {
    let agent = self.get_or_init_agent().await?; // Agent with model loaded
    
    let mut session = agent.create_session().await?; // Fresh session
    agent.discover_tools(&mut session).await?; // Discover from MCP server
    
    // Use session for this prompt only
    // Session cleaned up automatically
}
```

### Tool Discovery Benefits

This approach ensures:
- **Clean State**: Each prompt starts with clean session state
- **Tool Isolation**: No cross-prompt tool state pollution  
- **Model Reuse**: Expensive model loading happens only once
- **Server Reuse**: MCP server and registered tools shared efficiently

### Configuration

Define a configuration struct for LlamaAgent:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaAgentConfig {
    /// Model configuration
    pub model: ModelConfig,
    /// MCP server configuration
    pub mcp_server: McpServerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub source: ModelSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelSource {
    HuggingFace{ repo: String, filename: Option<String>},
    Local{filename: String},
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Port for in-process MCP server (0 = random)
    pub port: u16,
    /// Timeout for MCP requests
    pub timeout_seconds: u64,
}

impl Default for LlamaAgentConfig {
    fn default() -> Self {
        Self {
            model: ModelConfig {
                source: ModelSource::HuggingFace{
                repo: "unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF".to_string(),
                filename: Some("Qwen3-Coder-30B-A3B-Instruct-UD-Q6_K_XL.gguf".to_string())
                },
            },
            mcp_server: McpServerConfig {
                port: 0, // Random available port
                timeout_seconds: 30,
            },
        }
    }
}

impl LlamaAgentConfig {
    /// Configuration for unit testing with a small model that supports tool calling
    pub fn for_testing() -> Self {
        Self {
            model: ModelConfig {
                source: ModelSource::HuggingFace{
                repo: "unsloth/Phi-4-mini-instruct-GGUF".to_string(),
                filename: Some("Phi-4-mini-instruct-Q4_K_M.gguf".to_string())
                },
            },
            mcp_server: McpServerConfig {
                port: 0, // Random available port
                timeout_seconds: 10, // Shorter timeout for tests
            },
        }
    }
}
```



## Implementation Plan

### Phase 1: Workflow Context Refactor (CRITICAL FIRST)
1. **Replace `HashMap<String, Value>` with `WorkflowExecutionContext` throughout workflow system**
2. Update `Action` trait to use `&mut WorkflowExecutionContext` instead of `&mut HashMap<String, Value>`
3. Update all action implementations (PromptAction, ShellAction, etc.) to use typed context
4. Update workflow executor to create and manage `WorkflowExecutionContext`
5. Migrate variable substitution logic to use typed context
6. Update all tests to use new context type

### Phase 2: Agent Execution Infrastructure
1. Create `AgentExecutor` trait and `AgentExecutionContext`
2. Add global executor management with `OnceCell`
3. Add agent configuration parsing in workflow context
4. Create executor factory methods

### Phase 3: Claude Code Refactor
1. Move existing logic to `ClaudeCodeExecutor`
2. Implement `AgentExecutor` trait methods
3. Update `PromptAction` to use executor trait
4. Maintain backward compatibility during transition

### Phase 4: MCP Server Implementation
1. Implement `sah serve` command with stdio mode (default)
2. Implement `sah serve http [port]` command for HTTP mode
3. Create shared MCP server infrastructure for both modes
4. Add health check endpoints for HTTP mode
5. Add proper error handling and graceful shutdown

### Phase 5: LlamaAgent Implementation
1. Add llama-agent dependency to Cargo.toml
2. Implement `LlamaAgentExecutor` with lazy initialization
3. Create in-process HTTP MCP server using shared infrastructure
4. Implement session-per-prompt pattern
5. Add comprehensive error handling and logging

### Phase 6: Resource Management
1. Implement proper cleanup for global resources
2. Add graceful shutdown for MCP server
3. Handle model loading errors and retries
4. Add memory usage monitoring

### Phase 7: Testing & Documentation
1. Unit tests for both executors
2. Integration tests with real workflows using typed context
3. Load tests for session creation/cleanup
4. Performance benchmarks (model loading time, memory usage)
5. Configuration documentation and examples
6. Migration guide for existing workflows



## Backward Compatibility

The trait-based approach ensures full backward compatibility:
- Existing workflows continue to work unchanged
- Claude Code remains the default executor
- Optional opt-in to LlamaAgent through configuration
- Same prompt rendering and variable substitution
- Identical error handling patterns