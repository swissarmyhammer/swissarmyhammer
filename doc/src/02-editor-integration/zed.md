# Zed Editor Integration

SwissArmyHammer integrates with Zed editor through the Agent Client Protocol (ACP), providing a local AI coding assistant that runs directly within your editor.

## Quick Start

### 1. Install SwissArmyHammer

```bash
# Install via Homebrew
brew install swissarmyhammer/tap/swissarmyhammer-cli

# Or build from source with ACP support
cargo install --git https://github.com/swissarmyhammer/swissarmyhammer swissarmyhammer-cli --features acp
```

### 2. Configure Zed

Add the SwissArmyHammer agent to your Zed settings (`~/.config/zed/settings.json`):

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

### 3. Start Coding

1. Open Zed
2. Open the Assistant panel (Cmd+Shift+A on Mac, Ctrl+Shift+A on Linux/Windows)
3. Select "swissarmyhammer" from the agent dropdown
4. Start chatting with your local AI coding assistant!

## Configuration

### Basic Configuration

Create `~/.swissarmyhammer/acp-config.yaml` to customize the agent:

```yaml
protocol_version: "0.1.0"

capabilities:
  supports_session_loading: true
  supports_modes: true
  terminal: true
  filesystem:
    read_text_file: true
    write_text_file: true

permission_policy: AlwaysAsk
```

### Permission Policies

Control how the agent handles file operations and tool execution:

#### AlwaysAsk (Recommended for Beginners)

```yaml
permission_policy: AlwaysAsk
```

The editor prompts for every operation, giving you full control.

**Best for:**
- Learning how the agent works
- Working on sensitive projects
- Maximum security

#### AutoApproveReads (Balanced)

```yaml
permission_policy: AutoApproveReads
```

Automatically approves read operations (reading files, listing directories) but asks before writes and terminal execution.

**Best for:**
- Daily development work
- Trusted projects
- Faster workflow while maintaining safety

#### RuleBased (Advanced)

```yaml
permission_policy:
  RuleBased:
    rules:
      # Allow reading any file
      - pattern:
          tool: "files/read"
        action: Allow
      
      # Allow writes within project directory
      - pattern:
          tool: "files/write"
          path_pattern: "/Users/yourname/projects/**"
        action: Allow
      
      # Ask before terminal execution
      - pattern:
          tool: "terminal/execute"
        action: Ask
      
      # Deny everything else by default
      - pattern: {}
        action: Deny
```

**Best for:**
- Power users
- Custom security policies
- Specific project requirements

### Filesystem Security

Restrict which directories the agent can access:

```yaml
filesystem:
  # Only allow access to these directories
  allowed_paths:
    - /Users/yourname/projects
    - /Users/yourname/workspace
  
  # Block these even if they're in allowed_paths
  blocked_paths:
    - /Users/yourname/.ssh
    - /Users/yourname/.aws
    - /Users/yourname/.gnupg
  
  # Maximum file size to read/write (10MB)
  max_file_size_bytes: 10485760
```

### Resource Limits

Control agent resource usage:

```yaml
resources:
  max_file_operations_per_minute: 100
  terminal_output_buffer_bytes: 1048576  # 1MB
  max_concurrent_terminals: 5
```

## Features

### File Operations

The agent can read and write files with your permission:

```
You: Read the config file and update the port to 8080

Agent: I'll read the configuration file and update the port setting.
[Requests permission to read config.yaml]
[Requests permission to write config.yaml]
Done! Updated port from 3000 to 8080.
```

### Terminal Execution

Run commands and capture output:

```
You: Run the tests and show me the results

Agent: I'll execute the test suite.
[Requests permission to run: cargo test]
All tests passed! 15 tests in 2.3 seconds.
```

### Session Persistence

Resume conversations across editor restarts:

1. Close Zed while working on a task
2. Reopen Zed
3. Load your previous session
4. Continue right where you left off!

### Session Modes

Switch the agent's behavior based on your task:

#### Code Mode (Default)
Normal coding assistance:
```
/mode code
```

#### Plan Mode
Create structured implementation plans:
```
/mode plan
You: Plan out how to add authentication to the API

Agent: I'll create a detailed implementation plan:
1. Add user model with password hashing
2. Implement JWT token generation
3. Create login/logout endpoints
...
```

#### Test Mode
Focus on test generation and validation:
```
/mode test
You: Generate tests for the user service

Agent: I'll create comprehensive tests:
- Unit tests for UserService methods
- Integration tests for API endpoints
- Edge cases and error handling
...
```

### Slash Commands

Execute workflows directly from Zed:

```
/test    - Run tests and fix failures
/review  - Review code changes
/plan    - Create an implementation plan
/commit  - Generate commit message
```

These integrate with SwissArmyHammer's workflow system.

## Example Workflows

### Debugging a Test Failure

```
You: /test

Agent: Running test suite...
[Executes: cargo test]

Found 1 failing test: test_user_validation

Let me analyze the failure...
[Reads test file]
[Reads implementation file]

The issue is in user.rs line 45 - the email validation regex is incorrect.

Would you like me to fix it?

You: Yes

Agent: [Writes corrected code]
Running tests again to verify...
[Executes: cargo test]

All tests now pass! ✓
```

### Implementing a New Feature

```
You: Add a rate limiting middleware to the API

Agent: I'll implement rate limiting. Here's my plan:
1. Add rate limiting dependencies
2. Create middleware module
3. Configure limits in config
4. Add tests

Let me start by updating Cargo.toml...
[Requests permission to write Cargo.toml]
[Requests permission to create src/middleware/rate_limit.rs]
[Requests permission to update src/main.rs]

Done! Rate limiting is now active with:
- 100 requests per minute per IP
- Configurable via RATE_LIMIT_RPM environment variable
- Proper error responses
- Unit tests included

Would you like me to run the tests?
```

### Refactoring Code

```
You: Refactor the database module to use async/await

Agent: I'll refactor the database module to use async/await patterns.
[Reads src/database/mod.rs]
[Reads src/database/connection.rs]
[Reads src/database/queries.rs]

I'll need to:
1. Update function signatures to async fn
2. Replace blocking calls with .await
3. Update error handling
4. Fix all call sites

Starting with the connection module...
[Makes changes with your approval]

All done! The database module now uses async/await consistently.
Running tests to verify...
[Executes: cargo test database]

All tests pass! ✓
```

## Troubleshooting

### Agent Not Appearing in Zed

1. **Verify installation:**
   ```bash
   sah agent acp --version
   ```

2. **Check Zed configuration:**
   - Open `~/.config/zed/settings.json`
   - Verify the command path is correct
   - Try using full path: `/usr/local/bin/sah`

3. **Check Zed logs:**
   - View → Toggle Developer Tools → Console
   - Look for agent initialization errors

4. **Ensure ACP feature is enabled:**
   ```bash
   cargo build --features acp
   ```

### Permission Requests Not Working

1. **Check your policy:**
   ```bash
   cat ~/.swissarmyhammer/acp-config.yaml
   ```

2. **Verify paths are absolute:**
   ACP requires absolute paths, not relative ones.

3. **Check logs:**
   ```bash
   tail -f ~/.swissarmyhammer/logs/acp.log
   ```

### Slow Performance

1. **Reduce streaming buffer size** for lower latency:
   ```yaml
   streaming:
     chunk_buffer_size: 4
     flush_interval_ms: 30
   ```

2. **Check your model:**
   Smaller models respond faster. Try a quantized model.

3. **Monitor resource usage:**
   ```bash
   top -p $(pgrep -f "sah agent")
   ```

### Sessions Not Loading

1. **Verify session storage exists:**
   ```bash
   ls ~/.swissarmyhammer/sessions/
   ```

2. **Check capability is enabled:**
   ```yaml
   capabilities:
     supports_session_loading: true
   ```

3. **Look for errors:**
   ```bash
   grep -i "session" ~/.swissarmyhammer/logs/acp.log
   ```

## Advanced Configuration

### Custom Model Path

```yaml
model:
  path: /path/to/your/model.gguf
  context_size: 8192
  gpu_layers: 35
```

### Streaming Tuning

```yaml
streaming:
  chunk_buffer_size: 8     # Buffer more for higher throughput
  flush_interval_ms: 50    # Or flush faster for lower latency
```

### Session Management

```yaml
sessions:
  max_concurrent: 10
  compaction_threshold: 100
  max_tokens_per_session: 100000
  storage_path: ~/.swissarmyhammer/sessions
```

### Audit Logging

```yaml
audit:
  enabled: true
  log_file: ~/.swissarmyhammer/logs/acp-audit.log
  log_level: info
```

## Security Best Practices

1. **Use AlwaysAsk** until you understand the agent's behavior
2. **Restrict allowed_paths** to only your project directories
3. **Block sensitive directories** like `.ssh`, `.aws`, `.gnupg`
4. **Set reasonable file size limits** (5-10MB is usually sufficient)
5. **Enable audit logging** to track all agent actions
6. **Review permissions regularly** and adjust as needed

### Example Secure Configuration

```yaml
protocol_version: "0.1.0"

capabilities:
  supports_session_loading: true
  supports_modes: true
  terminal: true
  filesystem:
    read_text_file: true
    write_text_file: true

permission_policy: AlwaysAsk

filesystem:
  allowed_paths:
    - /Users/yourname/projects
  blocked_paths:
    - /Users/yourname/.ssh
    - /Users/yourname/.aws
    - /Users/yourname/.gnupg
    - /etc
  max_file_size_bytes: 5242880  # 5MB

resources:
  max_file_operations_per_minute: 50
  max_concurrent_terminals: 2
  terminal_timeout_seconds: 300

audit:
  enabled: true
  log_file: /Users/yourname/.swissarmyhammer/logs/acp-audit.log
  log_level: info
```

## Integration with SwissArmyHammer Ecosystem

When using the agent in Zed, you automatically get access to:

- **Rules System**: Code quality checks run on file writes
- **Workflows**: Slash commands trigger SwissArmyHammer workflows
- **MCP Tools**: All 25+ built-in tools available
- **Todo System**: Agent plans sync with todo lists
- **Session Storage**: Conversations persist in `.swissarmyhammer/sessions/`

## Tips and Tricks

### 1. Use Session Modes Strategically

Switch modes based on your task:
- Start with `/mode plan` to understand what needs to be done
- Switch to `/mode code` to implement
- Use `/mode test` to verify everything works

### 2. Leverage Session Persistence

Save your progress frequently:
- Sessions auto-save as you work
- Close and reopen Zed without losing context
- Resume complex multi-step tasks across days

### 3. Customize Slash Commands

Add your own workflows in `~/.swissarmyhammer/workflows/`:
```bash
# Create custom workflow
cat > ~/.swissarmyhammer/workflows/deploy.md << 'EOF'
---
name: deploy
title: Deploy Application
description: Build and deploy the application
---

# Deployment Workflow

- Build: Execute "cargo build --release"
- Test: Execute "cargo test --release"
- Deploy: Execute "rsync -avz target/release/myapp server:/opt/"
EOF

# Now use /deploy in Zed!
```

### 4. Use AutoApproveReads for Speed

Once comfortable, switch to `AutoApproveReads` to speed up your workflow while maintaining safety on writes.

### 5. Monitor with Audit Logs

Keep audit logging enabled and review periodically to understand what the agent is doing and optimize your workflow.

## Next Steps

- [Quick Start Guide](../01-getting-started/quick-start.md)
- [Workflow System](../03-workflows/introduction.md)
- [MCP Tools Reference](../04-tools/overview.md)
- [Configuration Reference](../05-configuration/reference.md)

## Support

- GitHub Issues: https://github.com/swissarmyhammer/swissarmyhammer/issues
- Documentation: https://swissarmyhammer.github.io/swissarmyhammer
- Community: [Join our Discord](#)
