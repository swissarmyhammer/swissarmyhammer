# Agent Client Protocol (ACP) Integration Plan

## Executive Summary

This document outlines the plan to integrate Agent Client Protocol (ACP) support into swissarmyhammer's llama-agent, enabling any ACP-compatible code editor or IDE to interact with our local LLaMA-based agents. This integration leverages our existing claude-agent implementation as a reference and builds upon our current MCP (Model Context Protocol) infrastructure.

## Background

### What is ACP?

Agent Client Protocol standardizes communication between code editors/IDEs and coding agents. Key characteristics:

- **JSON-RPC 2.0** based protocol over stdio
- **Editor-initiated**: User is in their editor, reaches out to use agents
- **Agents as subprocesses**: Run under the editor's control
- **MCP-friendly**: Reuses MCP types where possible (ContentBlock, etc.)
- **UX-first**: Designed for clear agent-to-editor interaction patterns
- **Trusted environment**: Assumes user trusts the model, editor provides access to files and MCP servers

### Current State

**llama-agent**:
- Mature MCP client implementation with both stdio and HTTP transport
- Session management with intelligent compaction
- Streaming generation with llama.cpp backend
- Tool call support via MCP servers
- No ACP protocol support

**claude-agent** (reference implementation):
- Full ACP server implementation using `agent-client-protocol` crate
- Wraps Claude Code as ACP-compatible agent
- Session management, permission handling, content validation
- Terminal management, file operations
- JSON-RPC server with stdio transport

## Architecture Overview

### High-Level Design

```
┌─────────────────────────────────────────────────────────────┐
│                    ACP-Compatible Editor                     │
│                      (Zed, JetBrains, etc.)                  │
└────────────────────────┬────────────────────────────────────┘
                         │ ACP Protocol
                         │ (JSON-RPC over stdio)
                         │
┌────────────────────────▼────────────────────────────────────┐
│                      llama-agent                             │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  acp::server (new module)                            │   │
│  │  - Implements Agent trait                             │   │
│  │  - JSON-RPC request/response handling                 │   │
│  │  - Session lifecycle (initialize, new, prompt, etc.)  │   │
│  │  - Notification streaming (session/update)            │   │
│  └──────────────────────┬───────────────────────────────┘   │
│                         │                                    │
│  ┌──────────────────────▼───────────────────────────────┐   │
│  │  acp::translation (new module)                        │   │
│  │  - Map ACP sessions to AgentServer sessions           │   │
│  │  - Translate ACP content to llama messages            │   │
│  │  - Stream chunks as ACP notifications                 │   │
│  │  - Handle tool call permissions                       │   │
│  └──────────────────────┬───────────────────────────────┘   │
│                         │                                    │
│  ┌──────────────────────▼───────────────────────────────┐   │
│  │  Core Agent (existing)                                │   │
│  │  - AgentServer                                        │   │
│  │  - Session management with compaction                 │   │
│  │  - LLaMA model inference                              │   │
│  │  - MCP client                                          │   │
│  │  - Streaming generation                               │   │
│  └──────────────────────┬───────────────────────────────┘   │
└─────────────────────────┼───────────────────────────────────┘
                          │
                          ├─── MCP Servers (stdio)
                          └─── MCP Servers (HTTP)
```

### Module Structure

Following claude-agent's pattern, ACP support is integrated as a module within llama-agent:

```
llama-agent/
├── src/
│   ├── acp/              # NEW: ACP protocol implementation
│   │   ├── mod.rs        # Public API and re-exports
│   │   ├── server.rs     # ACP server implementing Agent trait
│   │   ├── session.rs    # ACP session state management
│   │   ├── translation.rs # Type translation between ACP and agent
│   │   ├── filesystem.rs  # File read/write operations
│   │   ├── terminal.rs    # Terminal management
│   │   ├── permissions.rs # Permission policy engine
│   │   ├── plan.rs        # Agent plan protocol
│   │   └── commands.rs    # Slash command registry
│   ├── agent.rs          # Existing agent implementation
│   ├── session.rs        # Existing session management
│   ├── mcp.rs            # Existing MCP client
│   └── ...               # Other existing modules
└── Cargo.toml            # Add agent-client-protocol dependency
```

### Component Responsibilities

**acp::server** (`llama-agent/src/acp/server.rs`):
- Implements `Agent` trait from `agent-client-protocol` crate
- Handles JSON-RPC protocol over stdin/stdout
- Manages concurrent request and notification channels
- Routes methods to appropriate handlers
- Wraps existing `AgentServer`

**acp::translation** (`llama-agent/src/acp/translation.rs`):
- Bidirectional mapping between ACP and llama-agent types
- Session ID correlation
- Content block transformation
- Streaming chunk conversion
- Permission request handling

**acp::session** (`llama-agent/src/acp/session.rs`):
- ACP session state management
- Maps ACP SessionId to llama SessionId
- Tracks permissions and capabilities
- Manages session modes

**acp::filesystem** (`llama-agent/src/acp/filesystem.rs`):
- File read/write operations
- Path validation and security
- Integration with ACP client capabilities

**acp::terminal** (`llama-agent/src/acp/terminal.rs`):
- Terminal process management
- Output buffering and streaming
- Exit status tracking

**Core Agent** (existing modules, minimal changes):
- Add callback hooks for ACP notifications
- Expose streaming interface for ACP
- Tool call permission integration points

## Detailed Integration Plan

### Phase 1: Foundation

#### 1.1 Add agent-client-protocol Dependency

**Location**: `llama-agent/Cargo.toml`

```toml
[dependencies]
agent-client-protocol = "0.1" # Check latest version
```

**Rationale**: Use the official ACP Rust crate for protocol compliance.

#### 1.2 Create ACP Module Structure

**Location**: `llama-agent/src/acp/mod.rs`

Create the module hierarchy and public API:

```rust
//! Agent Client Protocol support for llama-agent
//!
//! This module implements the ACP protocol, enabling llama-agent to work
//! with ACP-compatible code editors like Zed and JetBrains IDEs.

pub mod server;
pub mod session;
pub mod translation;
pub mod filesystem;
pub mod terminal;
pub mod permissions;
pub mod plan;
pub mod commands;

// Re-export main types
pub use server::AcpServer;
pub use session::{AcpSessionState, SessionMode};
pub use permissions::{PermissionPolicy, PermissionStorage};
```

**Location**: `llama-agent/src/lib.rs`

Add ACP module to library:

```rust
#[cfg(feature = "acp")]
pub mod acp;

// Re-export ACP functionality when feature enabled
#[cfg(feature = "acp")]
pub use acp::{AcpServer, AcpSessionState};
```

**Location**: `llama-agent/Cargo.toml`

Add feature flag:

```toml
[features]
default = []
acp = ["agent-client-protocol"]
```

#### 1.3 Create ACP Server

**Location**: `llama-agent/src/acp/server.rs`

Based on `claude-agent/src/server.rs`:

```rust
use crate::agent::AgentServer;
use agent_client_protocol::{Agent, ...};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

pub struct AcpServer {
    agent_server: Arc<AgentServer>,
    sessions: Arc<RwLock<HashMap<SessionId, AcpSessionState>>>,
    notification_tx: broadcast::Sender<SessionNotification>,
}

impl Agent for AcpServer {
    async fn initialize(&self, req: InitializeRequest) -> Result<InitializeResponse> {
        // Negotiate version, advertise capabilities
    }

    async fn new_session(&self, req: NewSessionRequest) -> Result<NewSessionResponse> {
        // Create AgentServer session, map to ACP session ID
    }

    async fn prompt(&self, req: PromptRequest) -> Result<PromptResponse> {
        // Translate ACP content to llama messages
        // Stream responses as session/update notifications
        // Return final stop reason
    }

    // ... other methods
}
```

**Key Design Points**:
- One `AcpServer` manages multiple concurrent ACP sessions
- Each ACP session maps to one AgentServer session
- Notifications use broadcast channel (like claude-agent)
- Session state tracks permissions, file operations, terminals

#### 1.4 Session State Management

**Location**: `llama-agent/src/acp/session.rs`

```rust
use agent_client_protocol::{SessionId, ClientCapabilities};
use crate::types::SessionId as LlamaSessionId;

pub struct AcpSessionState {
    pub session_id: SessionId,
    pub llama_session_id: LlamaSessionId,
    pub mode: SessionMode,
    pub permissions: PermissionStorage,
    pub capabilities: ClientCapabilities,
    pub created_at: SystemTime,
}

pub enum SessionMode {
    Code,
    Plan,
    Test,
    Custom(String),
}
```

Tracks:
- ACP session ID to llama session ID mapping
- Current mode (if multiple modes supported)
- Granted permissions for tool calls
- Client capabilities (fs, terminal, etc.)

### Phase 2: Type Translation

#### 2.1 Content Block Mapping

**Location**: `llama-agent/src/acp/translation.rs`

ACP uses MCP ContentBlock types. Map to llama-agent Messages:

```rust
pub fn acp_to_llama_messages(
    content: Vec<ContentBlock>
) -> Result<Vec<llama_agent::Message>> {
    // ContentBlock::Text -> Message with role "user"
    // ContentBlock::Image -> Message with image data
    // ContentBlock::Resource -> Fetch and include content
}

pub fn llama_to_acp_content(
    messages: Vec<llama_agent::Message>
) -> Vec<ContentBlock> {
    // Reverse mapping for responses
}
```

**Considerations**:
- ACP supports text, images, resources
- llama-agent primarily text-based (for now)
- Image support may require multimodal model
- Resources may need MCP resource fetching

#### 2.2 Tool Call Translation

**Location**: `llama-agent/src/acp/translation.rs` (continued)

```rust
pub async fn handle_tool_call(
    tool_call: llama_agent::ToolCall,
    session: &AcpSessionState,
    client: &dyn RequestHandler, // ACP client handle
) -> Result<ToolResult> {
    // Check if permission needed
    if needs_permission(&tool_call) {
        let permission = request_permission(client, &tool_call).await?;
        if !permission.granted {
            return Err(PermissionDenied);
        }
    }

    // Execute via MCP
    let result = execute_mcp_tool(tool_call).await?;
    Ok(result)
}
```

**Permission Flow**:
1. Agent wants to call tool
2. Check permission policy
3. If needed, send `session/request_permission` to editor
4. Wait for user approval
5. Execute or abort based on response

#### 2.3 Streaming Chunk Mapping

**Location**: `llama-agent/src/acp/translation.rs` (continued)

```rust
pub fn llama_chunk_to_acp_notification(
    session_id: SessionId,
    chunk: llama_agent::StreamChunk,
) -> SessionNotification {
    SessionNotification {
        session_id,
        update: match chunk {
            StreamChunk::Text(text) => {
                SessionUpdate::AgentMessageChunk(ContentChunk {
                    content: ContentBlock::Text(TextContent {
                        text,
                        annotations: None,
                        meta: None,
                    }),
                    meta: None,
                })
            },
            StreamChunk::ToolCall(call) => {
                SessionUpdate::ToolCall(/* map to ACP ToolCall */)
            },
            // ... other chunk types
        },
        meta: None,
    }
}
```

### Phase 3: File System Operations

#### 3.1 File Read/Write Support

**Location**: `llama-agent/src/acp/filesystem.rs`

Implement ACP file system methods:

```rust
impl RequestHandler for AcpServer {
    async fn read_text_file(&self, req: ReadTextFileRequest) -> Result<ReadTextFileResponse> {
        // Validate absolute path
        // Check permissions
        // Read file content
        // Return as ACP response
    }

    async fn write_text_file(&self, req: WriteTextFileRequest) -> Result<WriteTextFileResponse> {
        // Validate absolute path
        // Check permissions
        // Write content atomically
        // Return success/failure
    }
}
```

**Security Considerations**:
- Path validation (must be absolute, no traversal)
- Permission checks before operations
- Atomic writes to prevent corruption
- Error handling for IO failures

#### 3.2 Integration with llama-agent Tools

**Location**: `llama-agent/src/tools/filesystem.rs`

Add filesystem tools that use ACP client capabilities:

```rust
pub struct FilesystemTools {
    client: Arc<dyn AcpClient>,
}

impl FilesystemTools {
    pub async fn read_file(&self, path: &str) -> Result<String> {
        // Call client.read_text_file
    }

    pub async fn write_file(&self, path: &str, content: &str) -> Result<()> {
        // Call client.write_text_file
    }
}
```

Register as MCP-style tools for the agent to use.

### Phase 4: Terminal Management

#### 4.1 Terminal Operations

**Location**: `llama-agent/src/acp/terminal.rs`

Based on `claude-agent/src/terminal_manager.rs`:

```rust
pub struct TerminalManager {
    terminals: HashMap<TerminalId, TerminalState>,
}

struct TerminalState {
    id: TerminalId,
    process: Child,
    output_buffer: Vec<u8>,
    exit_status: Option<i32>,
}

impl RequestHandler for AcpServer {
    async fn create_terminal(&self, req: CreateTerminalRequest) -> Result<CreateTerminalResponse> {
        // Spawn process with command
        // Return terminal ID
    }

    async fn terminal_output(&self, req: TerminalOutputRequest) -> Result<TerminalOutputResponse> {
        // Read buffered output
        // Return since last read
    }

    async fn wait_for_exit(&self, req: WaitForExitRequest) -> Result<WaitForExitResponse> {
        // Block until process exits
        // Return exit status
    }
}
```

**Process Management**:
- Async process spawning
- Output buffering and streaming
- Exit status tracking
- Kill/cleanup on session close

### Phase 5: Session Modes

#### 5.1 Mode Support

**Location**: `llama-agent/src/acp/session.rs` (add mode support)

ACP supports multiple session modes (e.g., "code", "plan", "test"):

```rust
pub enum SessionMode {
    Code,
    Plan,
    Test,
    Custom(String),
}

impl AcpServer {
    async fn set_session_mode(
        &self,
        req: SetSessionModeRequest
    ) -> Result<SetSessionModeResponse> {
        let session = self.get_session(&req.session_id)?;

        // Update mode
        session.mode = parse_mode(&req.mode_id)?;

        // Possibly change system prompt or parameters
        self.apply_mode_configuration(&session).await?;

        // Notify via session/update
        self.notify_mode_change(&req.session_id, &session.mode).await?;

        Ok(SetSessionModeResponse {})
    }
}
```

**Mode Behaviors**:
- **Code**: Normal agent interaction
- **Plan**: Output structured plans (integrate with swissarmyhammer planning)
- **Test**: Focus on test generation and validation

### Phase 6: Agent Plan Protocol

#### 6.1 Plan Structure

**Location**: `llama-agent/src/acp/plan.rs`

ACP defines a plan protocol for agents to communicate their strategy:

```rust
pub struct AgentPlan {
    entries: Vec<PlanEntry>,
}

pub struct PlanEntry {
    id: String,
    content: String,
    status: PlanEntryStatus,
    active_form: Option<String>,
}

pub enum PlanEntryStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}
```

Integrate with swissarmyhammer's todo/planning system:

```rust
pub fn swissarmyhammer_plan_to_acp(
    todos: Vec<TodoItem>
) -> AgentPlan {
    AgentPlan {
        entries: todos.into_iter().map(|todo| {
            PlanEntry {
                id: todo.id.to_string(),
                content: todo.task,
                status: match todo.done {
                    true => PlanEntryStatus::Completed,
                    false => PlanEntryStatus::Pending,
                },
                active_form: None,
            }
        }).collect()
    }
}
```

Send plan updates via `SessionUpdate::AgentPlan`.

### Phase 7: Slash Commands

#### 7.1 Command Advertisement

**Location**: `llama-agent/src/acp/commands.rs`

ACP allows agents to advertise available slash commands:

```rust
pub struct SlashCommandRegistry {
    commands: Vec<SlashCommand>,
}

pub struct SlashCommand {
    name: String,
    description: String,
    parameters: Vec<CommandParameter>,
}

impl AcpServer {
    fn get_available_commands(&self) -> Vec<SlashCommand> {
        vec![
            SlashCommand {
                name: "/test".to_string(),
                description: "Run tests and fix failures".to_string(),
                parameters: vec![],
            },
            SlashCommand {
                name: "/review".to_string(),
                description: "Review code changes".to_string(),
                parameters: vec![],
            },
            // Integrate with swissarmyhammer's workflow system
        ]
    }
}
```

Advertise via `SessionUpdate::AvailableCommandsChanged` notification.

### Phase 8: Testing Strategy

#### 8.1 Unit Tests

**Location**: `llama-agent/src/acp/` (module tests)

Test each component in isolation:

```rust
// Protocol compliance tests
#[tokio::test]
async fn test_initialize_negotiates_version() { }

#[tokio::test]
async fn test_new_session_creates_llama_session() { }

#[tokio::test]
async fn test_prompt_streams_notifications() { }

// Translation tests
#[test]
fn test_acp_content_to_llama_message() { }

#[test]
fn test_llama_chunk_to_acp_notification() { }

// Permission tests
#[tokio::test]
async fn test_permission_request_flow() { }
```

#### 8.2 Integration Tests

**Location**: `llama-agent/tests/acp_integration.rs`

End-to-end tests using the ACP protocol:

```rust
#[tokio::test]
async fn test_full_conversation_flow() {
    // Create mock editor client
    let client = MockAcpClient::new();

    // Initialize
    let init_response = client.initialize(...).await.unwrap();

    // Create session
    let session = client.new_session(...).await.unwrap();

    // Send prompt
    let prompt_response = client.prompt(...).await.unwrap();

    // Verify notifications received
    assert_eq!(client.notifications.len(), > 0);

    // Verify final response
    assert!(matches!(prompt_response.stop_reason, StopReason::EndTurn));
}
```

Reference claude-agent's test patterns.

#### 8.3 Compatibility Tests

**Location**: `llama-agent/tests/acp_compatibility.rs`

Test with actual ACP-compatible editors:

```rust
// Use ACP test harness (if available)
// Or create mock editor that exercises protocol

#[tokio::test]
async fn test_zed_compatibility() {
    // Spawn server
    // Simulate Zed editor interactions
    // Verify protocol compliance
}
```

### Phase 9: Configuration

#### 9.1 Server Configuration

**Location**: `llama-agent/src/acp/config.rs`

```rust
pub struct AcpConfig {
    /// Underlying llama-agent configuration
    pub agent_config: llama_agent::AgentConfig,

    /// ACP-specific settings
    pub acp: AcpSettings,
}

pub struct AcpSettings {
    /// Protocol version to advertise
    pub protocol_version: String,

    /// Capabilities to advertise
    pub capabilities: AcpCapabilities,

    /// Permission policy configuration
    pub permission_policy: PermissionPolicy,

    /// File system access restrictions
    pub filesystem: FilesystemSettings,
}

pub struct AcpCapabilities {
    pub supports_session_loading: bool,
    pub supports_modes: bool,
    pub supports_plans: bool,
    pub supports_slash_commands: bool,
    pub filesystem: FilesystemCapabilities,
    pub terminal: bool,
}

pub struct FilesystemSettings {
    /// Allowed paths (absolute paths or patterns)
    pub allowed_paths: Vec<PathBuf>,

    /// Blocked paths
    pub blocked_paths: Vec<PathBuf>,

    /// Maximum file size for read operations
    pub max_file_size_bytes: u64,
}
```

#### 9.2 Permission Policies

**Location**: `llama-agent/src/acp/permissions.rs`

Based on claude-agent's permission system:

```rust
pub enum PermissionPolicy {
    /// Always ask user for permission
    AlwaysAsk,

    /// Auto-approve read operations, ask for writes
    AutoApproveReads,

    /// Auto-approve based on rules
    RuleBased(Vec<PermissionRule>),
}

pub struct PermissionRule {
    pub pattern: ToolPattern,
    pub action: PermissionAction,
}

pub enum PermissionAction {
    Allow,
    Deny,
    Ask,
}
```

### Phase 10: Documentation

#### 10.1 User Documentation

**Location**: `llama-agent/README.md` (add ACP section)

```markdown
# Swiss Army Hammer ACP Server

Run local LLaMA agents from any ACP-compatible code editor.

## Quick Start

### With Zed Editor

1. Add to Zed configuration:
   ```json
   {
     "agents": {
       "swissarmyhammer": {
         "command": "sah",
         "args": ["agent", "acp"],
         "environment": {}
       }
     }
   }
   ```

2. Open Zed, start a conversation with the agent

### Configuration

See `config.example.yaml` for full configuration options.

## Features

- Local LLaMA model inference
- MCP server integration
- File system operations
- Terminal execution
- Session management with compaction

## Supported Editors

- Zed
- JetBrains IDEs (with ACP plugin)
- Any editor implementing ACP

## Architecture

[Diagrams and explanations]
```

#### 10.2 API Documentation

Generate from code with `cargo doc`:

```rust
/// ACP server for Swiss Army Hammer.
///
/// Implements the Agent Client Protocol to enable code editors
/// to interact with local LLaMA-based coding agents.
///
/// # Examples
///
/// ```no_run
/// use swissarmyhammer_acp_server::{AcpServer, AcpServerConfig};
///
/// #[tokio::main]
/// async fn main() {
///     let config = AcpServerConfig::from_file("config.yaml").unwrap();
///     let server = AcpServer::new(config).await.unwrap();
///     server.start_stdio().await.unwrap();
/// }
/// ```
pub struct AcpServer {
    // ...
}
```

## Implementation Todo List (Dependency Order)

### Dependencies and Module Setup
- [ ] Add `agent-client-protocol` dependency to `llama-agent/Cargo.toml`
- [ ] Add `acp` feature flag to `llama-agent/Cargo.toml`
- [ ] Create `llama-agent/src/acp/` directory
- [ ] Create `llama-agent/src/acp/mod.rs` with module structure
- [ ] Add `pub mod acp;` to `llama-agent/src/lib.rs` with feature gate
- [ ] Re-export main ACP types in `llama-agent/src/lib.rs`

### Basic Types and Session Management
- [ ] Create `llama-agent/src/acp/session.rs`
- [ ] Define `AcpSessionState` struct with session ID mapping
- [ ] Define `SessionMode` enum (Code, Plan, Test, Custom)
- [ ] Implement session ID conversion utilities (ACP SessionId ↔ llama SessionId)
- [ ] Create `llama-agent/src/acp/config.rs`
- [ ] Define `AcpConfig` struct with protocol version and capabilities
- [ ] Define `AcpCapabilities` struct (filesystem, terminal, modes, plans, commands)
- [ ] Define `FilesystemSettings` struct with path restrictions
- [ ] Implement configuration validation

### Permission System
- [ ] Create `llama-agent/src/acp/permissions.rs`
- [ ] Define `PermissionPolicy` enum (AlwaysAsk, AutoApproveReads, RuleBased)
- [ ] Define `PermissionRule` struct with pattern matching
- [ ] Define `PermissionAction` enum (Allow, Deny, Ask)
- [ ] Implement `PermissionStorage` for tracking granted permissions
- [ ] Implement `PermissionPolicyEngine` for evaluating permission requests
- [ ] Add methods for checking tool call permissions
- [ ] Add methods for storing permission decisions

### Content Translation Layer
- [ ] Create `llama-agent/src/acp/translation.rs`
- [ ] Implement `acp_to_llama_messages` function (ContentBlock → Message)
- [ ] Handle `ContentBlock::Text` translation
- [ ] Handle `ContentBlock::Image` translation (if supported)
- [ ] Handle `ContentBlock::Resource` translation
- [ ] Implement `llama_to_acp_content` function (Message → ContentBlock)
- [ ] Implement `llama_chunk_to_acp_notification` function (StreamChunk → SessionNotification)
- [ ] Handle text chunk translation
- [ ] Handle tool call chunk translation
- [ ] Handle tool result chunk translation
- [ ] Implement error translation (llama errors → ACP errors)

### Tool Call Translation
- [ ] Add tool call permission checking to translation layer
- [ ] Implement `handle_tool_call` function with permission flow
- [ ] Implement `needs_permission` function based on tool type
- [ ] Implement `request_permission` function (calls ACP client)
- [ ] Implement `execute_mcp_tool` function via AgentServer
- [ ] Handle tool call results and errors
- [ ] Translate MCP tool schemas to ACP format

### ACP Server Core
- [ ] Create `llama-agent/src/acp/server.rs`
- [ ] Define `AcpServer` struct with AgentServer, sessions, notification channel
- [ ] Implement `AcpServer::new` constructor
- [ ] Create broadcast channel for notifications
- [ ] Implement session storage (Arc<RwLock<HashMap>>)
- [ ] Implement `start_with_streams` method (based on claude-agent pattern)
- [ ] Setup concurrent request and notification handlers
- [ ] Implement shutdown coordination via broadcast channel
- [ ] Implement JSON-RPC request handler
- [ ] Implement JSON-RPC response sender
- [ ] Implement session/update notification sender
- [ ] Add proper JSON-RPC 2.0 format with camelCase fields

### Agent Trait Implementation - Initialization
- [ ] Implement `Agent::initialize` method
- [ ] Negotiate protocol version
- [ ] Store client capabilities from request for later enforcement
- [ ] Advertise agent capabilities matching claude-agent structure:
  - load_session: true
  - prompt_capabilities: audio=false, embedded_context=false, image=false, meta with streaming=true
  - mcp_capabilities: http=true (llama-agent has both), sse=false
  - meta: include available tools list and streaming flag
- [ ] Return `InitializeResponse` with server info
- [ ] Handle version compatibility checking

### Agent Trait Implementation - Authentication
- [ ] Implement `Agent::authenticate` method
- [ ] Handle authentication if required (or return success if not needed)
- [ ] Store authentication state if needed

### Agent Trait Implementation - Session Management
- [ ] Implement `Agent::new_session` method
- [ ] Create new AgentServer session
- [ ] Generate ACP session ID
- [ ] Create `AcpSessionState` with mapping
- [ ] Store session state
- [ ] Apply initial configuration from request
- [ ] Return `NewSessionResponse` with session ID
- [ ] Implement `Agent::load_session` method (match claude-agent capability)
- [ ] Advertise `load_session: true` capability in initialize
- [ ] Load existing session from SessionManager storage
- [ ] Stream ALL historical messages via session/update notifications
- [ ] Maintain chronological order of conversation history
- [ ] Return `LoadSessionResponse` after history replay completes

### Agent Trait Implementation - Session Modes
- [ ] Implement `Agent::set_session_mode` method
- [ ] Parse mode ID from request
- [ ] Update session mode in state
- [ ] Apply mode-specific configuration
- [ ] Send `SessionUpdate::ModeChanged` notification
- [ ] Return `SetSessionModeResponse`

### Agent Trait Implementation - Prompt Handling
- [ ] Implement `Agent::prompt` method
- [ ] Get session state by ID
- [ ] Translate ACP content to llama messages
- [ ] Get AgentServer session
- [ ] Create streaming callback for notifications
- [ ] Call AgentServer generate/prompt with streaming
- [ ] Convert each stream chunk to ACP notification
- [ ] Send notifications via broadcast channel
- [ ] Handle tool calls with permission checks
- [ ] Wait for tool call permissions if needed
- [ ] Execute approved tool calls
- [ ] Handle errors and convert to ACP format
- [ ] Determine stop reason
- [ ] Return `PromptResponse` with stop reason

### Agent Trait Implementation - Cancellation
- [ ] Implement `Agent::cancel` notification handler
- [ ] Get session by ID
- [ ] Cancel ongoing generation in AgentServer
- [ ] Send cancellation notification
- [ ] Clean up resources

### Agent Trait Implementation - Extension Methods
- [ ] Implement `Agent::ext_method` for custom methods
- [ ] Route extension methods to appropriate handlers
- [ ] Handle unknown methods gracefully

### File System Operations
- [ ] Create `llama-agent/src/acp/filesystem.rs`
- [ ] Implement path validation (absolute paths only, no traversal)
- [ ] Implement path security checks (allowed/blocked lists)
- [ ] Implement `fs/read_text_file` in ext_method handler
- [ ] Check client.fs.read_text_file capability before allowing operation
- [ ] Validate file path
- [ ] Check read permissions
- [ ] Check file size limits
- [ ] Read file content
- [ ] Return `ReadTextFileResponse`
- [ ] Handle errors (file not found, permission denied, etc.)
- [ ] Implement `fs/write_text_file` in ext_method handler
- [ ] Check client.fs.write_text_file capability before allowing operation
- [ ] Validate file path
- [ ] Check write permissions
- [ ] Write content atomically
- [ ] Return `WriteTextFileResponse`
- [ ] Handle errors

### Terminal Management
- [ ] Create `llama-agent/src/acp/terminal.rs`
- [ ] Define `TerminalManager` struct
- [ ] Define `TerminalState` enum (Created, Running, Finished, Killed, Released)
- [ ] Define `TerminalSession` struct (process, output buffer, exit status, state)
- [ ] Implement terminal ID generation
- [ ] Implement `terminal/create` in ext_method handler
- [ ] Check client.terminal capability before allowing operation
- [ ] Validate terminal command
- [ ] Spawn process with tokio::process::Command
- [ ] Setup async output capture
- [ ] Store terminal state in manager
- [ ] Return `CreateTerminalResponse` with terminal ID
- [ ] Implement `terminal/output` in ext_method handler
- [ ] Check client.terminal capability
- [ ] Get terminal by ID
- [ ] Read buffered output since last read
- [ ] Return `TerminalOutputResponse`
- [ ] Implement `terminal/wait_for_exit` in ext_method handler
- [ ] Get terminal by ID
- [ ] Wait for process to exit asynchronously
- [ ] Return exit status in `WaitForExitResponse`
- [ ] Implement `terminal/release` in ext_method handler
- [ ] Get terminal by ID
- [ ] Kill process if still running
- [ ] Remove terminal from storage
- [ ] Return success
- [ ] Implement `terminal/kill` in ext_method handler
- [ ] Get terminal by ID
- [ ] Send kill signal to process
- [ ] Keep terminal in storage for output/status queries
- [ ] Return success
- [ ] Add output buffering with configurable byte limits
- [ ] Handle output truncation when buffer full
- [ ] Handle process cleanup on session close
- [ ] Handle zombie process prevention with proper wait
- [ ] Add graceful shutdown timeout configuration

### Agent Plan Protocol
- [ ] Create `llama-agent/src/acp/plan.rs`
- [ ] Define plan structures matching ACP spec
- [ ] Implement plan entry status tracking
- [ ] Implement `swissarmyhammer_todo_to_acp_plan` converter
- [ ] Convert TodoItem to PlanEntry
- [ ] Map done status to PlanEntryStatus
- [ ] Handle active form for in-progress items
- [ ] Implement plan update notification sender
- [ ] Send `SessionUpdate::AgentPlan` notifications
- [ ] Integrate with AgentServer todo/task system
- [ ] Auto-update plans during execution

### Slash Commands
- [ ] Create `llama-agent/src/acp/commands.rs`
- [ ] Query MCP servers for available prompts (via MCP prompts/list)
- [ ] Map MCP prompts to ACP AvailableCommand entries
- [ ] Add available_commands field to AcpSessionState
- [ ] Track available_commands per session
- [ ] Implement get_available_commands_for_session method
- [ ] Include both core commands and MCP prompt-based commands
- [ ] Send `SessionUpdate::AvailableCommandsChanged` notifications when commands change
- [ ] Implement has_available_commands_changed detection
- [ ] Update available commands when MCP servers connect/disconnect
- [ ] Handle command parameter schemas from MCP prompts

### Integration with Existing Agent
- [ ] No modifications needed - use existing generate_stream() method
- [ ] Consume stream in ACP layer and convert StreamChunks to ACP notifications
- [ ] Add client_capabilities field to AcpSessionState
- [ ] Store client capabilities from initialize request
- [ ] Use client capabilities to gate ext_method operations
- [ ] Add session mode tracking to session (current_mode field)
- [ ] Expose SessionManager for session persistence/loading

### Unit Tests - Translation
- [ ] Test `acp_to_llama_messages` for text content
- [ ] Test `acp_to_llama_messages` for image content
- [ ] Test `acp_to_llama_messages` for resource content
- [ ] Test `llama_to_acp_content` conversion
- [ ] Test `llama_chunk_to_acp_notification` for text chunks
- [ ] Test `llama_chunk_to_acp_notification` for tool calls
- [ ] Test error translation

### Unit Tests - Session Management
- [ ] Test session ID mapping
- [ ] Test session state creation
- [ ] Test session storage and retrieval
- [ ] Test session mode changes
- [ ] Test multiple concurrent sessions

### Unit Tests - Permissions
- [ ] Test `PermissionPolicyEngine` evaluation
- [ ] Test `AlwaysAsk` policy
- [ ] Test `AutoApproveReads` policy
- [ ] Test `RuleBased` policy with rules
- [ ] Test permission storage
- [ ] Test permission decision caching

### Unit Tests - File System
- [ ] Test path validation (absolute paths)
- [ ] Test path traversal prevention
- [ ] Test allowed/blocked path lists
- [ ] Test file read operations
- [ ] Test file write operations
- [ ] Test file size limits
- [ ] Test error handling (not found, permission denied)

### Unit Tests - Terminal
- [ ] Test terminal creation
- [ ] Test terminal output buffering
- [ ] Test terminal wait for exit
- [ ] Test terminal kill
- [ ] Test terminal release
- [ ] Test process cleanup
- [ ] Test concurrent terminals

### Unit Tests - Server
- [ ] Test JSON-RPC request parsing
- [ ] Test JSON-RPC response formatting
- [ ] Test notification streaming
- [ ] Test camelCase field serialization
- [ ] Test concurrent request handling
- [ ] Test notification and request separation
- [ ] Test shutdown coordination

### Unit Tests - Agent Trait
- [ ] Test `initialize` method
- [ ] Test version negotiation
- [ ] Test capability advertisement
- [ ] Test `new_session` method
- [ ] Test session creation
- [ ] Test `load_session` method
- [ ] Test session loading
- [ ] Test `set_session_mode` method
- [ ] Test mode changes
- [ ] Test `prompt` method
- [ ] Test basic text prompt
- [ ] Test streaming notifications
- [ ] Test tool calls with permissions
- [ ] Test `cancel` notification
- [ ] Test cancellation

### Integration Tests - Basic Protocol
- [ ] Test full initialize → new_session → prompt flow
- [ ] Test streaming conversation
- [ ] Test multiple messages in session
- [ ] Test session state preservation
- [ ] Test notification delivery
- [ ] Test concurrent sessions

### Integration Tests - Tool Calls
- [ ] Test tool call with permission request
- [ ] Test user approval flow
- [ ] Test user denial flow
- [ ] Test auto-approved tool calls
- [ ] Test tool execution via MCP
- [ ] Test tool results in conversation

### Integration Tests - File Operations
- [ ] Test read file via ACP
- [ ] Test write file via ACP
- [ ] Test file operations in conversation
- [ ] Test agent using file tools
- [ ] Test path security enforcement

### Integration Tests - Terminal Operations
- [ ] Test create and execute command
- [ ] Test capture command output
- [ ] Test wait for command completion
- [ ] Test command exit status
- [ ] Test kill long-running command
- [ ] Test terminal cleanup

### Integration Tests - Advanced Features
- [ ] Test session mode switching
- [ ] Test plan generation and updates
- [ ] Test slash command advertisement
- [ ] Test slash command execution
- [ ] Test session persistence and loading

### Compatibility Tests
- [ ] Create mock ACP client for testing
- [ ] Test protocol compliance
- [ ] Test JSON-RPC 2.0 compliance
- [ ] Test field naming (camelCase)
- [ ] Test error responses
- [ ] Test notification format
- [ ] Verify compatibility with ACP specification

### Error Handling
- [ ] Add comprehensive error types for ACP operations
- [ ] Implement error conversion from AgentServer errors
- [ ] Implement error conversion from MCP errors
- [ ] Implement error conversion from file system errors
- [ ] Implement error conversion from terminal errors
- [ ] Add error recovery strategies
- [ ] Test error propagation

### Documentation
- [ ] Document ACP module in `llama-agent/src/acp/mod.rs`
- [ ] Add ACP section to `llama-agent/README.md`
- [ ] Document ACP feature flag usage
- [ ] Document configuration options
- [ ] Add examples for Zed editor integration
- [ ] Add examples for JetBrains integration
- [ ] Document permission policies
- [ ] Document security considerations
- [ ] Document file system restrictions
- [ ] Document terminal usage
- [ ] Add architecture diagrams
- [ ] Generate API documentation with `cargo doc`

### CLI Integration
- [ ] Add ACP server subcommand to swissarmyhammer CLI
- [ ] Implement stdio transport for ACP
- [ ] Add configuration file support for ACP
- [ ] Add command-line flags for ACP options
- [ ] Test CLI integration

### Performance Optimization
- [ ] Profile notification streaming latency
- [ ] Optimize session lookup performance
- [ ] Optimize content translation performance
- [ ] Add connection pooling for MCP if needed
- [ ] Optimize file operations
- [ ] Add caching where appropriate

### Security Hardening
- [ ] Audit path validation for vulnerabilities
- [ ] Audit permission checks for bypasses
- [ ] Add rate limiting for file operations
- [ ] Add rate limiting for terminal operations
- [ ] Add resource limits (memory, file size, etc.)
- [ ] Add audit logging for security events
- [ ] Test against common attack vectors

### Final Verification
- [ ] Run full test suite with ACP feature enabled
- [ ] Run full test suite with ACP feature disabled
- [ ] Verify backward compatibility (non-ACP usage)
- [ ] Test with real LLaMA models
- [ ] Test with various model sizes
- [ ] Test with long conversations
- [ ] Test with large file operations
- [ ] Test with multiple concurrent sessions
- [ ] Verify no memory leaks
- [ ] Verify clean shutdown
- [ ] Run clippy with no warnings
- [ ] Run cargo fmt
- [ ] Update CHANGELOG.md

## Key Design Decisions

### 1. Module Integration vs. Separate Crate

**Decision**: Integrate ACP as a module within llama-agent (not separate crate)

**Rationale**:
- Follows claude-agent pattern (single crate with integrated ACP support)
- Maintains symmetry with claude-agent architecture
- Simpler dependency management and versioning
- Easier internal access to agent internals
- Feature flag allows optional compilation
- Clean module boundaries provide separation of concerns

### 2. Translation Layer Location

**Decision**: Put translation in ACP module

**Rationale**:
- llama-agent core remains protocol-agnostic
- ACP-specific logic isolated in acp module
- Easier to maintain protocol compliance
- Feature flag gates ACP dependencies

### 3. Session Mapping Strategy

**Decision**: Maintain separate ACP and llama session IDs with mapping

**Rationale**:
- ACP SessionId is string-based from protocol
- llama SessionId might be different type (ULID)
- Allows flexibility in session management
- Clean boundary between protocols

### 4. Notification Mechanism

**Decision**: Use tokio broadcast channel (like claude-agent)

**Rationale**:
- Proven pattern in claude-agent
- Handles multiple subscribers
- Backpressure handling
- Async-friendly

### 5. Permission Model

**Decision**: Policy-based with callback to editor

**Rationale**:
- Follows ACP specification
- Flexible (can be permissive or strict)
- User maintains control
- Supports auto-approval rules for UX

### 6. File System Access

**Decision**: Whitelist/blacklist paths, validate all operations

**Rationale**:
- Security: prevent path traversal
- Configurability: user controls access
- ACP requirement: absolute paths only
- Defense in depth

## Integration with swissarmyhammer Ecosystem

### Workflows

Integrate with swissarmyhammer's workflow system:

```rust
// When agent receives /test command via ACP
pub async fn handle_test_workflow(
    session: &Session,
    workflow_engine: &WorkflowEngine,
) -> Result<()> {
    // Trigger swissarmyhammer test workflow
    let result = workflow_engine.run("test").await?;

    // Send updates via ACP notifications
    for step in result.steps {
        send_notification(SessionUpdate::AgentThoughtChunk(...));
    }

    Ok(())
}
```

### Rules

Integrate with swissarmyhammer's rule checking:

```rust
// Run rules automatically during file writes
pub async fn on_file_write(
    path: &Path,
    content: &str,
    rules: &RuleEngine,
) -> Result<()> {
    let violations = rules.check_file(path, content).await?;

    if !violations.is_empty() {
        // Send as agent thought or warning
        send_notification(SessionUpdate::AgentThoughtChunk(
            "Found code quality issues...".into()
        ));
    }

    Ok(())
}
```

### MCP Servers

Leverage existing MCP infrastructure:

- ACP provides MCP server config in session setup
- Pass through to llama-agent's MCP client
- Agent can use same MCP servers editor provides
- Seamless integration

## Performance Considerations

### Streaming

- Use async streams for token generation
- Buffer notifications to reduce overhead
- Batch small chunks if high-frequency

### Session Management

- Implement session pooling if needed
- Compaction thresholds tuned for interactive use
- Cache frequently accessed sessions

### MCP Operations

- Connection pooling for HTTP MCP servers
- Stdio process reuse
- Parallel tool execution where possible

### Resource Limits

- Max concurrent sessions
- Per-session token limits
- File size limits for read/write
- Terminal output buffer limits

## Security Considerations

### Input Validation

- Validate all paths (absolute, no traversal)
- Validate content blocks
- Validate tool call arguments
- Validate session IDs

### Resource Protection

- Rate limiting on file operations
- Command execution restrictions
- Process isolation for terminals
- Memory limits for sessions

### Permission Model

- Least privilege by default
- User approval for sensitive operations
- Audit log for security events
- Configurable policies

## Testing Strategy

### Unit Tests

- Protocol compliance
- Type translation
- Permission logic
- Session management

### Integration Tests

- Full conversation flows
- File operations
- Terminal execution
- Tool calls with permissions

### Compatibility Tests

- Test with Zed
- Test with JetBrains
- Protocol conformance suite

### Performance Tests

- Streaming latency
- Concurrent sessions
- Large file operations
- Long conversations

## Open Questions

1. **Multimodal Support**: Should we add image support now or later?
   - **Recommendation**: Start text-only, add images in Phase 7 if needed

2. **Session Persistence**: How to handle session loading?
   - **Recommendation**: Store in swissarmyhammer's session storage, load on demand

3. **Custom Extensions**: Should we add custom ACP methods?
   - **Recommendation**: Use `_meta` fields first, add extensions only if necessary

4. **Error Recovery**: How to handle llama-agent failures?
   - **Recommendation**: Return ACP errors, maintain session state, allow retry

5. **Concurrent Prompts**: Can one session have multiple prompts in flight?
   - **Recommendation**: Follow ACP spec - one prompt at a time per session

## Success Criteria

1. **Functional**:
   - Can initialize and create sessions
   - Can have conversations with streaming
   - Can execute tool calls with permissions
   - Can read/write files
   - Can execute terminal commands

2. **Compatible**:
   - Works with Zed editor
   - Works with JetBrains (if available)
   - Passes ACP conformance tests

3. **Performant**:
   - Token streaming latency < 100ms
   - Session creation < 1s
   - File operations < 500ms
   - Tool calls < 2s

4. **Secure**:
   - No path traversal vulnerabilities
   - Permission checks enforced
   - Resource limits respected
   - Audit logging in place

5. **Maintainable**:
   - Comprehensive test coverage (>80%)
   - Clear documentation
   - Clean separation of concerns
   - Follows swissarmyhammer conventions

## Next Steps

1. Review this plan with stakeholders
2. Clarify open questions
3. Set up development environment
4. Begin Phase 1 implementation
5. Iterate based on feedback

## References

- [Agent Client Protocol Specification](https://agentclientprotocol.com)
- [agent-client-protocol Rust Crate](https://docs.rs/agent-client-protocol)
- claude-agent Implementation (../claude-agent)
- llama-agent Documentation (../llama-agent)
- swissarmyhammer Architecture (../docs)
