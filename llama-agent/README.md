# llama-agent

A high-performance Rust library for building LLaMA-based coding agents with Model Context Protocol (MCP) support and optional Agent Client Protocol (ACP) integration.

## Features

- **Local LLaMA Model Inference**: Run coding agents with local LLaMA models via llama.cpp backend
- **MCP Client**: Connect to MCP servers over stdio and HTTP transports
- **Session Management**: Intelligent session compaction and persistence
- **Streaming Generation**: Real-time token streaming with async support
- **Tool Call Support**: Execute tools via connected MCP servers
- **ACP Support**: Optional integration with ACP-compatible code editors (Zed, JetBrains, etc.)

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
llama-agent = "0.1"
```

For ACP support:

```toml
[dependencies]
llama-agent = { version = "0.1", features = ["acp"] }
```

## Basic Usage

```rust
use llama_agent::{AgentServer, AgentConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AgentConfig::from_file("config.yaml")?;
    let agent = AgentServer::new(config).await?;
    
    // Create a session and start conversing
    let session_id = agent.new_session().await?;
    let response = agent.prompt(session_id, "Hello, agent!").await?;
    
    println!("Agent: {}", response);
    Ok(())
}
```

## Agent Client Protocol (ACP) Support

llama-agent includes optional ACP support for integration with code editors like Zed and JetBrains IDEs.

### Quick Start with Zed

1. Enable the ACP feature in your project or use the `sah` CLI:

```toml
[dependencies]
llama-agent = { version = "0.1", features = ["acp"] }
```

2. Configure Zed (in `~/.config/zed/settings.json`):

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

3. Start using the agent in Zed!

### Configuration

Create an `acp-config.yaml` file to customize ACP server behavior:

```yaml
protocol_version: "0.1.0"

capabilities:
  supports_session_loading: true
  supports_modes: true
  terminal: true
  filesystem:
    read_text_file: true
    write_text_file: true

filesystem:
  allowed_paths:
    - /home/user/projects
  blocked_paths:
    - /home/user/secrets
  max_file_size_bytes: 10485760  # 10MB limit

permission_policy: AlwaysAsk  # Options: AlwaysAsk, AutoApproveReads, RuleBased
```

### Permission Policies

Control how the agent handles tool execution and file operations:

#### AlwaysAsk (Recommended)
```yaml
permission_policy: AlwaysAsk
```
The editor will prompt for every tool call and file operation.

#### AutoApproveReads
```yaml
permission_policy: AutoApproveReads
```
Automatically approve read operations (file reads, directory listings), but ask for writes and terminal execution.

#### RuleBased
```yaml
permission_policy:
  RuleBased:
    rules:
      - pattern:
          tool: "files/read"
        action: Allow
      - pattern:
          tool: "files/write"
          path_pattern: "/home/user/projects/**"
        action: Allow
      - pattern:
          tool: "terminal/execute"
        action: Ask
      - pattern: {}
        action: Deny  # Default deny everything else
```

### Features

- **Streaming Token Generation**: Real-time streaming of agent responses with low latency
- **Tool Calls with Permission Requests**: User control over file system and terminal operations
- **File Read/Write Operations**: Direct file system access from the editor
- **Terminal Execution**: Run commands and capture output
- **Session Persistence and Loading**: Resume conversations across editor sessions
- **Session Modes**: Switch between different agent behaviors (Code, Plan, Test)
- **Slash Commands**: Execute predefined workflows from the editor
- **Agent Plans**: Track and display agent's execution plan

### Architecture

The ACP integration is implemented as an optional module within llama-agent:

```
┌─────────────────────────────────────────┐
│    ACP-Compatible Editor (Zed, etc.)   │
└──────────────────┬──────────────────────┘
                   │ ACP Protocol (JSON-RPC over stdio)
┌──────────────────▼──────────────────────┐
│         llama-agent (with acp feature)  │
│  ┌────────────────────────────────────┐ │
│  │  acp::server                       │ │
│  │  - Agent trait implementation      │ │
│  │  - JSON-RPC request handling       │ │
│  │  - Streaming notifications         │ │
│  └────────────┬───────────────────────┘ │
│               │                          │
│  ┌────────────▼───────────────────────┐ │
│  │  Core Agent                        │ │
│  │  - LLaMA inference                 │ │
│  │  - Session management              │ │
│  │  - MCP client                      │ │
│  └────────────┬───────────────────────┘ │
└───────────────┼─────────────────────────┘
                │
                └─── MCP Servers (stdio/HTTP)
```

### Configuration Options

#### Protocol Version

```yaml
protocol_version: "0.1.0"
```
Specify the ACP protocol version. Must match the version supported by your editor.

#### Capabilities

Advertise what features your agent supports:

```yaml
capabilities:
  supports_session_loading: true    # Can load previous sessions
  supports_modes: true              # Can switch between modes (Code/Plan/Test)
  terminal: true                    # Can execute terminal commands
  filesystem:
    read_text_file: true            # Can read files
    write_text_file: true           # Can write files
```

### Session Loading

Load previous conversation sessions to resume work across editor restarts.

When `supports_session_loading: true` is enabled, the agent can reload conversation history from persistent storage. The session loading process works as follows:

1. **Client requests session load** via `session/load` method
2. **Agent loads session from storage** (typically `.swissarmyhammer/sessions/`)
3. **Historical messages are streamed** via `session/update` notifications in chronological order
4. **All message types are included**: user messages, assistant responses, tool calls, and tool results
5. **LoadSessionResponse is sent** only after the complete history has been replayed

This ensures the editor's UI can reconstruct the full conversation state progressively, providing immediate visual feedback to the user.

#### Example: Loading a Session

Request:
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "session/load",
  "params": {
    "sessionId": "01KC2EXAMPLE123456789"
  }
}
```

Historical messages stream as notifications (in chronological order):
```json
{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"01KC2EXAMPLE123456789","update":{"agentMessageChunk":{"content":{"type":"text","text":"[User's first message]"}}}}}

{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"01KC2EXAMPLE123456789","update":{"agentMessageChunk":{"content":{"type":"text","text":"[Agent's first response]"}}}}}

{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"01KC2EXAMPLE123456789","update":{"toolCall":{"id":"tool_1","type":"function","function":{"name":"files/read","arguments":"{\"path\":\"/path/to/file\"}"}}}}}

{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"01KC2EXAMPLE123456789","update":{"toolResult":{"toolCallId":"tool_1","content":[{"type":"text","text":"File contents..."}]}}}}
```

Final response after history replay:
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "result": {}
}
```

**Key Points:**
- Messages maintain chronological order
- Tool calls and their results are preserved with context
- Notifications allow progressive UI updates
- Response signals completion of history replay

#### Filesystem Security

Control file system access with path restrictions:

```yaml
filesystem:
  allowed_paths:
    - /home/user/projects
    - /opt/workspace
  blocked_paths:
    - /home/user/.ssh
    - /home/user/.aws
  max_file_size_bytes: 10485760  # 10MB
```

**Notes:**
- Paths must be absolute
- `allowed_paths` is a whitelist - agent can only access these directories
- `blocked_paths` takes precedence over `allowed_paths`
- File size limit prevents reading/writing very large files

### Session Modes

Switch agent behavior based on the task:

#### Code Mode (Default)
Normal agent interaction for writing and discussing code.

#### Plan Mode
Agent focuses on creating structured plans and breaking down tasks:
```json
{
  "method": "session/set_mode",
  "params": {
    "sessionId": "...",
    "modeId": "plan"
  }
}
```

#### Test Mode
Agent focuses on test generation and validation:
```json
{
  "method": "session/set_mode",
  "params": {
    "sessionId": "...",
    "modeId": "test"
  }
}
```

### Terminal Operations

Execute commands and capture output:

```rust
// Create terminal
let create_resp = client.create_terminal(CreateTerminalRequest {
    command: "cargo test".to_string(),
    working_directory: Some("/project/path".into()),
    environment: None,
}).await?;

// Read output
let output_resp = client.terminal_output(TerminalOutputRequest {
    terminal_id: create_resp.terminal_id,
}).await?;

println!("Output: {}", output_resp.output);

// Wait for completion
let exit_resp = client.wait_for_exit(WaitForExitRequest {
    terminal_id: create_resp.terminal_id,
}).await?;

println!("Exit code: {}", exit_resp.exit_code);
```

### Agent Plans

The agent can communicate its execution plan to the editor:

```json
{
  "method": "session/update",
  "params": {
    "sessionId": "...",
    "update": {
      "agentPlan": {
        "entries": [
          {
            "id": "1",
            "content": "Read the configuration file",
            "status": "completed"
          },
          {
            "id": "2",
            "content": "Validate the settings",
            "status": "inProgress"
          },
          {
            "id": "3",
            "content": "Write the updated config",
            "status": "pending"
          }
        ]
      }
    }
  }
}
```

Plans integrate with swissarmyhammer's todo system and are automatically updated as the agent works.

### Slash Commands

Agents can advertise available slash commands to editors:

```json
{
  "method": "session/update",
  "params": {
    "sessionId": "...",
    "update": {
      "availableCommandsChanged": {
        "commands": [
          {
            "name": "/test",
            "description": "Run tests and fix failures"
          },
          {
            "name": "/review",
            "description": "Review code changes"
          },
          {
            "name": "/plan",
            "description": "Create an implementation plan"
          }
        ]
      }
    }
  }
}
```

Commands integrate with swissarmyhammer's workflow system.

### Supported Editors

- **Zed**: Native ACP support
- **JetBrains IDEs**: Via ACP plugin
- **Any editor implementing ACP**: As long as it follows the protocol specification

### Error Handling

ACP errors follow JSON-RPC 2.0 format:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32603,
    "message": "Permission denied",
    "data": {
      "tool": "files/write",
      "path": "/etc/passwd",
      "reason": "Path not in allowed list"
    }
  }
}
```

Common error codes:
- `-32700`: Parse error
- `-32600`: Invalid request
- `-32601`: Method not found
- `-32602`: Invalid params
- `-32603`: Internal error

### Integration with swissarmyhammer Ecosystem

When used with the `sah` CLI, ACP agents automatically integrate with:

- **Rules**: Code quality checks run on file writes
- **Workflows**: Slash commands trigger swissarmyhammer workflows
- **MCP Servers**: Access to swissarmyhammer's built-in MCP tools
- **Session Storage**: Sessions persist in `.swissarmyhammer/sessions/`
- **Todo System**: Agent plans sync with todo lists

### Example: Full Conversation

```rust
use llama_agent::acp::{AcpServer, AcpConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = AcpConfig::from_file("acp-config.yaml")?;
    
    // Create ACP server
    let server = AcpServer::new(config).await?;
    
    // Start server (reads from stdin, writes to stdout)
    server.start_stdio().await?;
    
    Ok(())
}
```

The editor communicates via JSON-RPC:

```json
// Initialize
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"0.1.0","capabilities":{"filesystem":{"readTextFile":true,"writeTextFile":true},"terminal":true}}}

// Create session
{"jsonrpc":"2.0","id":2,"method":"session/new","params":{}}

// Send prompt
{"jsonrpc":"2.0","id":3,"method":"session/prompt","params":{"sessionId":"01KC...","content":[{"type":"text","text":"Write a function to calculate fibonacci"}]}}

// Receive streaming updates (notifications)
{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"01KC...","update":{"agentMessageChunk":{"content":{"type":"text","text":"I'll"}}}}}
{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"01KC...","update":{"agentMessageChunk":{"content":{"type":"text","text":" create"}}}}}
// ... more chunks ...

// Final response
{"jsonrpc":"2.0","id":3,"result":{"stopReason":"endTurn"}}

// Later: Load the session to resume
{"jsonrpc":"2.0","id":4,"method":"session/load","params":{"sessionId":"01KC..."}}

// Historical messages stream as notifications (chronological order)
{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"01KC...","update":{"agentMessageChunk":{"content":{"type":"text","text":"Write a function to calculate fibonacci"}}}}}
{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"01KC...","update":{"agentMessageChunk":{"content":{"type":"text","text":"I'll create a fibonacci function..."}}}}}
// ... all historical messages ...

// Load complete
{"jsonrpc":"2.0","id":4,"result":{}}
```

### Performance Tuning

#### Streaming Latency

Reduce notification overhead:

```yaml
streaming:
  chunk_buffer_size: 8     # Buffer up to 8 chunks
  flush_interval_ms: 50    # Flush every 50ms
```

#### Session Limits

```yaml
sessions:
  max_concurrent: 10
  compaction_threshold: 100  # Compact after 100 messages
  max_tokens_per_session: 100000
```

#### Resource Limits

```yaml
resources:
  max_file_operations_per_minute: 100
  terminal_output_buffer_bytes: 1048576  # 1MB
  max_concurrent_terminals: 5
```

### Security Best Practices

1. **Use AlwaysAsk policy** for untrusted or production environments
2. **Restrict allowed_paths** to only necessary directories
3. **Block sensitive paths** like `.ssh`, `.aws`, `.env` files
4. **Set reasonable file size limits** to prevent memory exhaustion
5. **Enable audit logging** to track agent actions
6. **Use session timeouts** to clean up inactive sessions

Example secure configuration:

```yaml
permission_policy: AlwaysAsk

filesystem:
  allowed_paths:
    - /home/user/projects
  blocked_paths:
    - /home/user/.ssh
    - /home/user/.aws
    - /home/user/.gnupg
    - /etc
  max_file_size_bytes: 5242880  # 5MB

resources:
  max_file_operations_per_minute: 50
  max_concurrent_terminals: 2
  terminal_timeout_seconds: 300

audit:
  enabled: true
  log_file: /home/user/.config/sah/acp-audit.log
  log_level: info
```

### Troubleshooting

#### Agent Not Appearing in Editor

1. Check that `sah agent acp` command works in terminal
2. Verify editor configuration points to correct command
3. Check editor logs for initialization errors
4. Ensure ACP feature is enabled: `cargo build --features acp`

#### Permission Requests Not Working

1. Verify `permission_policy` is set correctly
2. Check that client advertises required capabilities
3. Look for permission errors in agent logs
4. Ensure paths are absolute (ACP requirement)

#### Slow Streaming Performance

1. Reduce `chunk_buffer_size` for lower latency
2. Increase `flush_interval_ms` to batch more chunks
3. Check network latency if using remote MCP servers
4. Profile with `--features profile` to identify bottlenecks

#### Sessions Not Loading

**What successful session loading looks like:**
- `session/load` request returns after all historical messages are streamed
- Historical messages arrive via `session/update` notifications in chronological order
- All message types are included: user messages, assistant responses, tool calls, and tool results
- Editor UI progressively reconstructs the conversation as notifications arrive

**Troubleshooting steps:**

1. Verify `supports_session_loading: true` in capabilities
2. Check session storage path exists and is writable (default: `.swissarmyhammer/sessions/`)
3. Ensure session IDs are valid ULIDs
4. Check logs for notification sequence and message order:
   - Look for "Loading session" log entries
   - Verify notifications are sent in chronological order
   - Confirm LoadSessionResponse sent only after history replay completes
5. If sessions load but history appears incomplete:
   - Check for errors during notification streaming
   - Verify all message types (user, assistant, tool) are being loaded
   - Ensure tool call context and results are preserved
6. Look for session loading errors in logs

### Development

#### Running Tests

```bash
# All tests
cargo test --features acp

# ACP-specific tests
cargo test --features acp --test acp_integration

# With logging
RUST_LOG=debug cargo test --features acp
```

#### Building

```bash
# Without ACP
cargo build

# With ACP
cargo build --features acp

# Release build
cargo build --release --features acp
```

#### Documentation

```bash
# Generate docs
cargo doc --features acp --open
```

## License

[Insert license information]

## Contributing

[Insert contribution guidelines]

## References

- [Agent Client Protocol Specification](https://agentclientprotocol.com)
- [Model Context Protocol](https://modelcontextprotocol.io)
- [swissarmyhammer Documentation](../README.md)
