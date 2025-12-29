# Component Details

This page provides detailed architecture diagrams for each major component in the SwissArmyHammer system.

## CLI Application Architecture

```mermaid
graph TB
    subgraph "CLI Entry Point"
        Main[main.rs]
        DynamicCLI[Dynamic CLI Builder]
    end
    
    subgraph "Command Handlers"
        PromptCmd[Prompt Commands]
        FlowCmd[Flow Commands]
        RulesCmd[Rules Commands]
        TodoCmd[Todo Commands]
        GitCmd[Git Commands]
        DoctorCmd[Doctor Diagnostics]
        ServeCmd[MCP Server]
        AgentCmd[Agent Commands]
    end
    
    subgraph "Core Services"
        WorkflowEngine[Workflow Engine]
        PromptRenderer[Prompt Renderer]
        MCPClient[MCP Client]
        FileDiscovery[File Discovery]
    end
    
    subgraph "Storage"
        FileSystem[File System]
        GitRepo[Git Repository]
    end
    
    Main --> DynamicCLI
    DynamicCLI --> PromptCmd
    DynamicCLI --> FlowCmd
    DynamicCLI --> RulesCmd
    DynamicCLI --> TodoCmd
    DynamicCLI --> GitCmd
    DynamicCLI --> DoctorCmd
    DynamicCLI --> ServeCmd
    DynamicCLI --> AgentCmd
    
    PromptCmd --> PromptRenderer
    FlowCmd --> WorkflowEngine
    RulesCmd --> MCPClient
    TodoCmd --> MCPClient
    GitCmd --> GitRepo
    ServeCmd --> MCPClient
    AgentCmd --> MCPClient
    
    WorkflowEngine --> PromptRenderer
    WorkflowEngine --> MCPClient
    PromptRenderer --> FileDiscovery
    FileDiscovery --> FileSystem
    
    style Main fill:#e1f5ff
    style DynamicCLI fill:#e1f5ff
    style WorkflowEngine fill:#fff4e1
    style PromptRenderer fill:#fff4e1
    style MCPClient fill:#f0e1ff
```

### CLI Features

- **Dynamic Command Generation**: Commands are generated at runtime from discovered workflows
- **Comprehensive Help System**: Auto-generated help text from workflow metadata
- **Shell Completions**: Bash, Zsh, Fish, and PowerShell support
- **Interactive Mode**: Prompt for missing required parameters
- **Dry Run**: Preview workflow execution without making changes

## MCP Server Architecture

```mermaid
graph TB
    subgraph "MCP Server Core"
        Server[MCP Server]
        ToolRegistry[Tool Registry]
        ProgressNotifier[Progress Notification Manager]
    end
    
    subgraph "Tool Categories"
        FilesTools[Files Tools]
        GitTools[Git Tools]
        TodoTools[Todo Tools]
        RulesTools[Rules Tools]
        ShellTools[Shell Tools]
        WebTools[Web Tools]
        FlowTools[Flow Tools]
        QuestionTools[Question Tools]
        AbortTools[Abort Tools]
    end
    
    subgraph "File Tools Detail"
        Read[files_read]
        Write[files_write]
        Edit[files_edit]
        Glob[files_glob]
        Grep[files_grep]
    end
    
    subgraph "Supporting Services"
        PathValidator[Path Security]
        ContentValidator[Content Security]
        FileCache[File Cache]
        NotificationQueue[Notification Queue]
    end
    
    Server --> ToolRegistry
    Server --> ProgressNotifier
    
    ToolRegistry --> FilesTools
    ToolRegistry --> GitTools
    ToolRegistry --> TodoTools
    ToolRegistry --> RulesTools
    ToolRegistry --> ShellTools
    ToolRegistry --> WebTools
    ToolRegistry --> FlowTools
    ToolRegistry --> QuestionTools
    ToolRegistry --> AbortTools
    
    FilesTools --> Read
    FilesTools --> Write
    FilesTools --> Edit
    FilesTools --> Glob
    FilesTools --> Grep
    
    FilesTools --> PathValidator
    FilesTools --> ContentValidator
    FilesTools --> FileCache
    
    ProgressNotifier --> NotificationQueue
    FlowTools --> ProgressNotifier
    
    style Server fill:#f0e1ff
    style ToolRegistry fill:#f0e1ff
    style PathValidator fill:#FFE4E1
    style ContentValidator fill:#FFE4E1
```

### Tool Organization

**File Operations**
- `files_read`: Read file contents with offset/limit support
- `files_write`: Write complete file contents atomically
- `files_edit`: Precise string replacements
- `files_glob`: Pattern matching with .gitignore support
- `files_grep`: Content search with ripgrep

**Git Operations**
- `git_changes`: List changed files on a branch
- Git integration with automatic parent branch detection

**Task Management**
- `todo_create`: Create new todo items
- `todo_list`: List todos with filtering
- `todo_show`: Get specific or next todo
- `todo_mark_complete`: Complete todo items

**Code Quality**
- `rules_create`: Create validation rules
- `rules_check`: Check code against rules

**Workflow Execution**
- `flow`: Execute workflows with progress notifications

**Web Integration**
- `web_search`: DuckDuckGo search with content fetching
- `web_fetch`: HTML to markdown conversion

**Shell Operations**
- `shell_execute`: Safe command execution with output capture

**Question Management**
- `question_ask`: Interactive user questions via elicitation
- `question_summary`: Retrieve Q&A history

**Emergency Controls**
- `abort_create`: Signal workflow termination

## Claude Agent Architecture

```mermaid
graph TB
    subgraph "Agent Core"
        Agent[Agent Server]
        Session[Session Manager]
        Conversation[Conversation Manager]
    end
    
    subgraph "Communication Layer"
        ClaudeClient[Claude API Client]
        StreamProcessor[Stream Processor]
        ContentBlockProcessor[Content Block Processor]
    end
    
    subgraph "Tool Management"
        ToolRegistry[Tool Registry]
        ToolExecutor[Tool Executor]
        ProtocolTranslator[Protocol Translator]
    end
    
    subgraph "Security & Permissions"
        PermissionEngine[Permission Engine]
        PermissionStorage[Permission Storage]
        PathValidator[Path Validator]
        ContentValidator[Content Security Validator]
        CapabilityValidator[Capability Validator]
    end
    
    subgraph "MCP Integration"
        MCPClient[MCP Client]
        MCPServers[External MCP Servers]
    end
    
    subgraph "Storage"
        SessionStorage[Session Storage]
        ConfigStorage[Configuration]
    end
    
    Agent --> Session
    Agent --> Conversation
    Agent --> ClaudeClient
    
    Session --> SessionStorage
    Session --> ConfigStorage
    
    Conversation --> StreamProcessor
    StreamProcessor --> ContentBlockProcessor
    
    ContentBlockProcessor --> ToolExecutor
    ToolExecutor --> ProtocolTranslator
    ProtocolTranslator --> MCPClient
    MCPClient --> MCPServers
    
    ToolExecutor --> PermissionEngine
    PermissionEngine --> PermissionStorage
    PermissionEngine --> PathValidator
    PermissionEngine --> ContentValidator
    PermissionEngine --> CapabilityValidator
    
    ClaudeClient --> StreamProcessor
    
    style Agent fill:#fff4e1
    style PermissionEngine fill:#FFD700
    style PathValidator fill:#FFE4E1
    style ContentValidator fill:#FFE4E1
    style CapabilityValidator fill:#FFE4E1
```

### Claude Agent Features

**Streaming Conversation Management**
- Real-time token streaming from Claude API
- Content block processing for text and tool calls
- Conversation history persistence
- Automatic session compaction

**Tool Orchestration**
- MCP tool integration
- Protocol translation between Claude and MCP formats
- Tool result formatting and error handling
- Concurrent tool execution support

**Permission Management**
- Three-tier permission system (AlwaysAsk, AutoApproveReads, RuleBased)
- Path security validation
- Content security validation
- Capability-based restrictions
- Persistent permission storage

**Session Management**
- ULID-based session identification
- Automatic session persistence
- Session loading with history replay
- Configuration-based session limits

## LLaMA Agent Architecture

```mermaid
graph TB
    subgraph "Agent Core"
        Agent[Agent Server]
        Session[Session Manager]
        Queue[Request Queue]
    end
    
    subgraph "Generation Layer"
        Generator[Generator]
        LlamaCpp[llama.cpp Backend]
        KVCache[KV Cache Manager]
        ChatTemplate[Chat Template]
    end
    
    subgraph "MCP Integration"
        MCPClient[MCP Client]
        ToolCall[Tool Call Parser]
        MCPServers[External MCP Servers]
    end
    
    subgraph "ACP Support (Optional)"
        ACPServer[ACP Server]
        ACPAgent[Agent Trait Impl]
        ACPFilesystem[Filesystem Operations]
        ACPTerminal[Terminal Operations]
        ACPSession[Session Operations]
    end
    
    subgraph "Validation"
        AgentValidator[Agent Validator]
        MCPValidator[MCP Validator]
        QueueValidator[Queue Validator]
        RequestValidator[Request Validator]
    end
    
    subgraph "Storage"
        SessionStorage[Session Storage]
        ConfigStorage[Configuration]
        ModelStorage[Model Cache]
    end
    
    Agent --> Session
    Agent --> Queue
    Agent --> Generator
    
    Session --> SessionStorage
    Session --> KVCache
    
    Queue --> QueueValidator
    Queue --> Generator
    
    Generator --> LlamaCpp
    Generator --> ChatTemplate
    Generator --> KVCache
    
    Generator --> ToolCall
    ToolCall --> MCPClient
    MCPClient --> MCPServers
    
    ACPServer --> ACPAgent
    ACPAgent --> Agent
    ACPAgent --> ACPFilesystem
    ACPAgent --> ACPTerminal
    ACPAgent --> ACPSession
    
    Agent --> AgentValidator
    MCPClient --> MCPValidator
    
    ConfigStorage --> ModelStorage
    
    style Agent fill:#fff4e1
    style Generator fill:#90EE90
    style ACPServer fill:#f0e1ff
    style KVCache fill:#FFD700
```

### LLaMA Agent Features

**Local Inference**
- Integration with llama.cpp for local model execution
- KV cache optimization for session continuity
- Template cache for fast context switching
- Streaming token generation

**Session Compaction**
- Automatic compaction at configurable thresholds
- Preservation of critical conversation context
- KV cache state management across compaction
- Session persistence to disk

**MCP Client**
- Connect to external MCP servers over stdio and HTTP
- Tool call parsing from LLaMA output
- Result formatting and error recovery
- Multiple simultaneous server connections

**ACP Integration (Optional)**
- JSON-RPC 2.0 server over stdio
- Streaming token notifications
- Permission-based file operations
- Terminal execution support
- Session loading and persistence
- Slash command integration
- Mode switching (Code, Plan, Test)

## Workflow Engine Architecture

```mermaid
graph TB
    subgraph "Workflow Definition"
        Markdown[Workflow Markdown]
        Parser[YAML Frontmatter Parser]
        MermaidDiagram[Mermaid State Diagram]
    end
    
    subgraph "Workflow Execution"
        Engine[Workflow Engine]
        StateManager[State Manager]
        ActionExecutor[Action Executor]
    end
    
    subgraph "Action Types"
        LogAction[Log Action]
        PromptAction[Execute Prompt]
        FlowAction[Execute Sub-workflow]
        ShellAction[Shell Command]
    end
    
    subgraph "Context Management"
        Variables[Variable Store]
        Parameters[Parameter Bindings]
        Environment[Environment Context]
    end
    
    subgraph "Integration"
        PromptRenderer[Prompt Renderer]
        MCPTools[MCP Tool Execution]
        ProgressNotifier[Progress Notifications]
    end
    
    Markdown --> Parser
    Parser --> MermaidDiagram
    MermaidDiagram --> Engine
    
    Engine --> StateManager
    Engine --> ActionExecutor
    Engine --> Variables
    
    StateManager --> ActionExecutor
    
    ActionExecutor --> LogAction
    ActionExecutor --> PromptAction
    ActionExecutor --> FlowAction
    ActionExecutor --> ShellAction
    
    Variables --> Parameters
    Parameters --> Environment
    
    PromptAction --> PromptRenderer
    PromptAction --> MCPTools
    
    Engine --> ProgressNotifier
    
    style Engine fill:#e1ffe1
    style StateManager fill:#e1ffe1
    style ProgressNotifier fill:#f0e1ff
```

### Workflow Features

**State Machine Execution**
- Mermaid diagram parsing for state definitions
- Automatic state transition logic
- Error state handling
- Completion detection

**Action Types**
- **Log**: Output messages to console
- **Execute Prompt**: Render and execute prompts with context
- **Execute Sub-workflow**: Nested workflow execution
- **Shell Command**: Safe command execution

**Context Management**
- Parameter binding from CLI arguments
- Variable interpolation in actions
- Environment variable access
- State-based context passing

**Progress Notifications**
- MCP notification support for long-running workflows
- Progress tracking across states
- Cancellation support

## Rules Engine Architecture

```mermaid
graph TB
    subgraph "Rule Definition"
        RuleMarkdown[Rule Markdown Files]
        Frontmatter[YAML Frontmatter]
        RuleContent[Rule Instructions]
    end
    
    subgraph "Rule Discovery"
        FileScanner[File Scanner]
        CategoryBuilder[Category Builder]
        RuleIndex[Rule Index]
    end
    
    subgraph "Rule Execution"
        CheckEngine[Check Engine]
        FileResolver[File Pattern Resolver]
        Scheduler[Concurrent Scheduler]
    end
    
    subgraph "Validation"
        LLMValidator[LLM-based Validator]
        MCPTools[MCP Tool Access]
        ResultAggregator[Result Aggregator]
    end
    
    subgraph "Output"
        ViolationReport[Violation Report]
        TodoCreator[Todo Item Creator]
        ExitCode[Exit Code Handler]
    end
    
    RuleMarkdown --> Frontmatter
    RuleMarkdown --> RuleContent
    
    Frontmatter --> FileScanner
    RuleContent --> FileScanner
    
    FileScanner --> CategoryBuilder
    CategoryBuilder --> RuleIndex
    
    RuleIndex --> CheckEngine
    CheckEngine --> FileResolver
    CheckEngine --> Scheduler
    
    Scheduler --> LLMValidator
    LLMValidator --> MCPTools
    LLMValidator --> ResultAggregator
    
    ResultAggregator --> ViolationReport
    ResultAggregator --> TodoCreator
    ResultAggregator --> ExitCode
    
    style CheckEngine fill:#e1ffe1
    style LLMValidator fill:#fff4e1
    style Scheduler fill:#90EE90
```

### Rules Features

**Rule Organization**
- Markdown-based rule definitions
- Category-based organization via directory structure
- Severity levels: error, warning, info, hint
- Tag-based filtering

**Flexible Checking**
- File pattern matching (glob support)
- Git integration for changed files only
- Specific rule selection
- Category and severity filtering

**Concurrent Execution**
- Configurable concurrency limits
- Parallel rule checking across files
- Result aggregation
- Early termination on max errors

**Integration**
- Automatic todo creation for violations
- LLM-based validation via MCP tools
- Exit code based on severity
- Rich violation reporting

## Todo System Architecture

```mermaid
graph TB
    subgraph "Todo Storage"
        TodoFiles[YAML Todo Files]
        ULIDGenerator[ULID Generator]
        FileStorage[File System Storage]
    end
    
    subgraph "Todo Operations"
        Create[Create Todo]
        List[List Todos]
        Show[Show Todo]
        Complete[Mark Complete]
    end
    
    subgraph "Features"
        Context[Rich Context Support]
        Filtering[Completion Filtering]
        NextItem[Next Item Selector]
        GarbageCollector[Garbage Collection]
    end
    
    subgraph "Integration"
        MCPTools[MCP Tool Exposure]
        RulesIntegration[Rules Integration]
        WorkflowIntegration[Workflow Integration]
    end
    
    Create --> ULIDGenerator
    Create --> Context
    Create --> TodoFiles
    
    List --> Filtering
    List --> TodoFiles
    
    Show --> NextItem
    Show --> TodoFiles
    
    Complete --> TodoFiles
    Complete --> GarbageCollector
    
    TodoFiles --> FileStorage
    
    Create --> MCPTools
    List --> MCPTools
    Show --> MCPTools
    Complete --> MCPTools
    
    RulesIntegration --> Create
    WorkflowIntegration --> Show
    WorkflowIntegration --> Complete
    
    style Create fill:#e1ffe1
    style GarbageCollector fill:#FFD700
```

### Todo Features

**Simple Structure**
- ULID-based unique identification
- Task description with optional context
- Boolean completion status
- YAML file storage

**Operations**
- Create with task and context
- List with completion filtering
- Show specific item or next incomplete
- Mark complete with automatic GC

**Integration Points**
- MCP tool exposure for AI agents
- Rules engine violation tracking
- Workflow task management
- Automatic cleanup on completion

## Next Steps

- [Data Flow Diagrams](dataflow.md) - Detailed interaction sequences
- [Security Architecture](../03-security/overview.md) - Security model details
- [MCP Protocol](mcp-protocol.md) - MCP implementation details
- [ACP Protocol](acp-protocol.md) - ACP implementation details
