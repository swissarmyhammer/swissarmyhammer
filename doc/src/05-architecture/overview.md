# Architecture Overview

SwissArmyHammer is designed as a modular system with three primary components that work together to provide a comprehensive AI-assisted development platform.

## System Architecture

```mermaid
graph TB
    subgraph "User Interface Layer"
        CLI[CLI Application<br/>swissarmyhammer-cli]
        Editor[Code Editors<br/>Zed, JetBrains, VSCode]
    end
    
    subgraph "Agent Layer"
        ClaudeAgent[Claude Agent<br/>claude-agent]
        LlamaAgent[LLaMA Agent<br/>llama-agent]
    end
    
    subgraph "Protocol Layer"
        MCP[MCP Server<br/>swissarmyhammer-tools]
        ACP[ACP Protocol<br/>llama-agent/acp]
    end
    
    subgraph "Core Libraries"
        Workflow[Workflow Engine<br/>swissarmyhammer-workflow]
        Todo[Task Management<br/>swissarmyhammer-todo]
        Rules[Code Quality<br/>swissarmyhammer-rules]
        Common[Common Utilities<br/>swissarmyhammer-common]
    end
    
    subgraph "External Services"
        ClaudeAPI[Claude API<br/>Anthropic]
        LlamaModel[Local LLaMA<br/>llama.cpp]
        MCPServers[External MCP Servers]
    end
    
    CLI --> MCP
    CLI --> Workflow
    CLI --> Todo
    CLI --> Rules
    
    Editor --> ACP
    Editor --> MCP
    
    ClaudeAgent --> ClaudeAPI
    ClaudeAgent --> MCP
    
    LlamaAgent --> LlamaModel
    LlamaAgent --> ACP
    LlamaAgent --> MCP
    
    MCP --> MCPServers
    MCP --> Common
    
    Workflow --> Common
    Todo --> Common
    Rules --> Common
    
    ACP --> LlamaAgent
    
    style CLI fill:#e1f5ff
    style Editor fill:#e1f5ff
    style ClaudeAgent fill:#fff4e1
    style LlamaAgent fill:#fff4e1
    style MCP fill:#f0e1ff
    style ACP fill:#f0e1ff
    style Workflow fill:#e1ffe1
    style Todo fill:#e1ffe1
    style Rules fill:#e1ffe1
    style Common fill:#e1ffe1
```

## Component Overview

### User Interface Layer

#### CLI Application
The command-line interface provides direct access to all SwissArmyHammer features:
- Execute prompts and workflows
- Manage tasks and rules
- Run diagnostics and tests
- Interact with MCP servers

#### Code Editors
Integration with popular code editors through standardized protocols:
- **Zed Editor**: Native ACP support
- **JetBrains IDEs**: ACP plugin support
- **VSCode**: MCP extension support

### Agent Layer

#### Claude Agent
High-performance agent implementation using Anthropic's Claude API:
- Streaming conversation management
- Tool call orchestration
- Permission management
- Session persistence
- Real-time notifications

#### LLaMA Agent
Local inference agent for privacy-focused development:
- Local model execution via llama.cpp
- Session compaction and KV cache optimization
- Optional ACP support for editor integration
- MCP client for tool access

### Protocol Layer

#### MCP Server
Comprehensive Model Context Protocol implementation:
- 25+ professional development tools
- File operations (read, write, edit, search)
- Git integration
- Web operations (search, fetch)
- Task and rule management
- Shell execution
- Progress notifications

#### ACP Protocol
Agent Client Protocol support for editor integration:
- Streaming token generation
- Permission-based tool execution
- File system operations
- Terminal management
- Session persistence and loading
- Slash commands

### Core Libraries

#### Workflow Engine
State machine-based workflow execution:
- Mermaid diagram definitions
- Parallel and sequential actions
- Error handling and recovery
- Dynamic state transitions

#### Task Management
Ephemeral todo list system:
- ULID-based identification
- Rich context support
- Completion tracking
- Automatic garbage collection

#### Code Quality Rules
LLM-based code validation:
- Markdown rule definitions
- Severity levels (error, warning, info, hint)
- Category organization
- Batch checking with concurrency

#### Common Utilities
Shared infrastructure across all components:
- Error handling with Severity trait
- Path validation and security
- File system utilities
- Test helpers

## Data Flow

### Prompt Execution Flow

```mermaid
sequenceDiagram
    participant User
    participant CLI
    participant Workflow
    participant MCP
    participant Agent
    participant LLM
    
    User->>CLI: sah plan spec.md
    CLI->>Workflow: Load workflow definition
    Workflow->>CLI: Return workflow states
    CLI->>Agent: Create session
    Agent->>LLM: Initialize conversation
    
    loop For each workflow state
        CLI->>Agent: Execute prompt with context
        Agent->>LLM: Send prompt with tools
        LLM->>Agent: Response with tool calls
        Agent->>MCP: Execute tool calls
        MCP->>Agent: Tool results
        Agent->>LLM: Continue with results
        LLM->>Agent: Final response
        Agent->>CLI: Streaming updates
    end
    
    CLI->>User: Workflow complete
```

### Editor Integration Flow

```mermaid
sequenceDiagram
    participant Editor
    participant ACP
    participant Agent
    participant MCP
    participant Tools
    
    Editor->>ACP: Initialize (JSON-RPC)
    ACP->>Agent: Configure capabilities
    Agent->>ACP: Capabilities confirmed
    ACP->>Editor: InitializeResponse
    
    Editor->>ACP: Create session
    ACP->>Agent: New session
    Agent->>Editor: Session created
    
    Editor->>ACP: Send prompt
    ACP->>Agent: Process prompt
    
    loop Streaming generation
        Agent->>Editor: Token chunks (notifications)
    end
    
    Agent->>ACP: Tool call required
    ACP->>Editor: Request permission
    Editor->>ACP: Permission granted
    ACP->>MCP: Execute tool
    MCP->>Tools: Perform operation
    Tools->>MCP: Result
    MCP->>ACP: Tool result
    ACP->>Agent: Continue generation
    
    Agent->>Editor: Generation complete
```

## File System Organization

```mermaid
graph TD
    subgraph "Builtin (Binary)"
        BP[builtin/prompts/]
        BW[builtin/workflows/]
        BR[builtin/rules/]
    end
    
    subgraph "User (~/.swissarmyhammer/)"
        UP[prompts/]
        UW[workflows/]
        UR[rules/]
        UM[models/]
        US[sessions/]
    end
    
    subgraph "Local (./.swissarmyhammer/)"
        LP[prompts/]
        LW[workflows/]
        LR[rules/]
        LM[models/]
        LS[sessions/]
        LT[todos/]
        LQ[questions/]
    end
    
    Search[File Search] --> LP
    Search --> LW
    Search --> LR
    
    Search --> UP
    Search --> UW
    Search --> UR
    
    Search --> BP
    Search --> BW
    Search --> BR
    
    style LP fill:#90EE90
    style LW fill:#90EE90
    style LR fill:#90EE90
    style UP fill:#87CEEB
    style UW fill:#87CEEB
    style UR fill:#87CEEB
    style BP fill:#FFB6C1
    style BW fill:#FFB6C1
    style BR fill:#FFB6C1
```

**Precedence**: Local → User → Builtin (first match wins)

## Security Architecture

```mermaid
graph TB
    subgraph "Request Layer"
        UserRequest[User/Editor Request]
        ToolCall[Tool Call]
    end
    
    subgraph "Validation Layer"
        ContentValidator[Content Security Validator]
        PathValidator[Path Security Validator]
        CapabilityValidator[Content Capability Validator]
    end
    
    subgraph "Permission Layer"
        PermissionEngine[Permission Engine]
        PermissionStorage[Permission Storage]
    end
    
    subgraph "Execution Layer"
        FileOps[File Operations]
        ShellOps[Shell Execution]
        NetOps[Network Operations]
    end
    
    UserRequest --> ToolCall
    ToolCall --> ContentValidator
    ToolCall --> PathValidator
    ToolCall --> CapabilityValidator
    
    ContentValidator --> PermissionEngine
    PathValidator --> PermissionEngine
    CapabilityValidator --> PermissionEngine
    
    PermissionEngine --> PermissionStorage
    PermissionEngine --> FileOps
    PermissionEngine --> ShellOps
    PermissionEngine --> NetOps
    
    style ContentValidator fill:#FFE4E1
    style PathValidator fill:#FFE4E1
    style CapabilityValidator fill:#FFE4E1
    style PermissionEngine fill:#FFD700
    style PermissionStorage fill:#FFD700
```

## Key Design Principles

### 1. File-Based Everything
All configuration, prompts, workflows, and state are stored as files. No databases required.

### 2. Protocol-First Integration
Standard protocols (MCP, ACP) enable integration with any compatible client.

### 3. Security by Default
Multiple validation layers protect against unauthorized access and malicious operations.

### 4. Composable Architecture
Each component can be used independently or as part of the complete system.

### 5. Local-First, Cloud-Optional
Core functionality works entirely offline. Cloud services (Claude API) are optional enhancements.

## Next Steps

- [Component Details](components.md) - Deep dive into each component
- [Data Flow](dataflow.md) - Detailed sequence diagrams
- [Security Model](../03-security/overview.md) - Comprehensive security architecture
