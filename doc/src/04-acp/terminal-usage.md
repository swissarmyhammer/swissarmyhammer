# Terminal Usage

SwissArmyHammer implements the Agent Client Protocol (ACP) terminal specification, providing comprehensive terminal session management for command execution and process control.

## Overview

Terminal sessions allow agents to execute commands, monitor output, and manage long-running processes. Each terminal is associated with a session and has a unique lifecycle with proper resource management.

## Terminal Lifecycle

Terminals go through several states:

1. **Created** - Terminal initialized but process not started
2. **Running** - Process is actively executing
3. **Finished** - Process completed with exit status
4. **Killed** - Process terminated by signal
5. **Released** - Resources cleaned up (output/status still queryable)

## Creating Terminals

### Basic Terminal Creation

Create a terminal with a command and arguments:

```rust
use claude_agent::terminal_manager::{TerminalManager, TerminalCreateParams};

let manager = TerminalManager::new();
let session_manager = SessionManager::new();

// Create a session first
let session_id = session_manager.create_session(
    std::path::PathBuf::from("/workspace"),
    None
)?;

// Create terminal with command
let params = TerminalCreateParams {
    session_id: session_id.to_string(),
    command: "cargo".to_string(),
    args: Some(vec!["build".to_string(), "--release".to_string()]),
    env: None,
    cwd: None,
    output_byte_limit: None,
};

let terminal_id = manager.create_terminal_with_command(
    &session_manager,
    params
).await?;
```

### Custom Environment Variables

Set environment variables for the terminal:

```rust
use claude_agent::terminal_manager::EnvVariable;

let params = TerminalCreateParams {
    session_id: session_id.to_string(),
    command: "npm".to_string(),
    args: Some(vec!["start".to_string()]),
    env: Some(vec![
        EnvVariable {
            name: "NODE_ENV".to_string(),
            value: "production".to_string(),
        },
        EnvVariable {
            name: "PORT".to_string(),
            value: "3000".to_string(),
        },
    ]),
    cwd: None,
    output_byte_limit: None,
};

let terminal_id = manager.create_terminal_with_command(
    &session_manager,
    params
).await?;
```

### Custom Working Directory

Specify a working directory (must be absolute):

```rust
let params = TerminalCreateParams {
    session_id: session_id.to_string(),
    command: "git".to_string(),
    args: Some(vec!["status".to_string()]),
    env: None,
    cwd: Some("/path/to/repository".to_string()),
    output_byte_limit: None,
};

let terminal_id = manager.create_terminal_with_command(
    &session_manager,
    params
).await?;
```

### Output Buffer Limits

Control output buffer size (default: 1MB):

```rust
let params = TerminalCreateParams {
    session_id: session_id.to_string(),
    command: "cat".to_string(),
    args: Some(vec!["large-file.log".to_string()]),
    env: None,
    cwd: None,
    output_byte_limit: Some(10_485_760), // 10MB
};

let terminal_id = manager.create_terminal_with_command(
    &session_manager,
    params
).await?;
```

## Retrieving Output

### Get Terminal Output

Query output at any time, even after the terminal is released:

```rust
use claude_agent::terminal_manager::TerminalOutputParams;

let params = TerminalOutputParams {
    session_id: session_id.to_string(),
    terminal_id: terminal_id.clone(),
};

let response = manager.get_output(&session_manager, params).await?;

println!("Output: {}", response.output);
println!("Truncated: {}", response.truncated);

if let Some(exit_status) = response.exit_status {
    if let Some(code) = exit_status.exit_code {
        println!("Exit code: {}", code);
    }
    if let Some(signal) = exit_status.signal {
        println!("Signal: {}", signal);
    }
}
```

### UTF-8 Handling

Output buffers preserve UTF-8 character boundaries when truncating:

```rust
// Output with multibyte characters is handled correctly
let params = TerminalCreateParams {
    session_id: session_id.to_string(),
    command: "echo".to_string(),
    args: Some(vec!["Hello 世界 👋".to_string()]),
    env: None,
    cwd: None,
    output_byte_limit: Some(100),
};

let terminal_id = manager.create_terminal_with_command(
    &session_manager,
    params
).await?;

// Output will be truncated at valid UTF-8 boundaries
// Characters will never be split
```

## Process Management

### Wait for Exit

Block until the process completes:

```rust
let params = TerminalOutputParams {
    session_id: session_id.to_string(),
    terminal_id: terminal_id.clone(),
};

let exit_status = manager.wait_for_exit(&session_manager, params).await?;

if let Some(code) = exit_status.exit_code {
    println!("Process exited with code: {}", code);
}
```

### Kill Process

Terminate a running process with graceful shutdown:

```rust
let params = TerminalOutputParams {
    session_id: session_id.to_string(),
    terminal_id: terminal_id.clone(),
};

manager.kill_terminal(&session_manager, params).await?;
```

#### Signal Handling (Unix)

On Unix systems, killing a process follows a two-step approach:

1. **SIGTERM** - Graceful shutdown signal sent first
2. **Grace Period** - Wait for configurable timeout (default: 5 seconds)
3. **SIGKILL** - Force kill if process doesn't exit

```rust
use claude_agent::terminal_manager::{TimeoutConfig, GracefulShutdownTimeout};
use std::time::Duration;

// Configure custom timeout
let timeout_config = TimeoutConfig {
    graceful_shutdown_timeout: GracefulShutdownTimeout::new(
        Duration::from_secs(10)
    ),
};
```

#### Windows Behavior

On Windows, the process is terminated immediately using `TerminateProcess`.

### Release Terminal

Release terminal resources while preserving output and exit status:

```rust
use claude_agent::terminal_manager::TerminalReleaseParams;

let params = TerminalReleaseParams {
    session_id: session_id.to_string(),
    terminal_id: terminal_id.clone(),
};

manager.release_terminal(&session_manager, params).await?;

// Output and exit status remain queryable after release
let output_params = TerminalOutputParams {
    session_id: session_id.to_string(),
    terminal_id: terminal_id.clone(),
};

let response = manager.get_output(&session_manager, output_params).await?;
// Still works!
```

## Session Cleanup

When closing a session, all associated terminals are automatically cleaned up:

```rust
// Clean up all terminals for a session
let cleanup_count = manager.cleanup_session_terminals(
    &session_id.to_string()
).await?;

println!("Cleaned up {} terminals", cleanup_count);
```

This:
- Kills any running processes
- Releases all resources
- Removes terminals from storage

## Error Handling

### Common Errors

```rust
use claude_agent::AgentError;

match manager.create_terminal_with_command(&session_manager, params).await {
    Ok(terminal_id) => println!("Created: {}", terminal_id),
    Err(AgentError::Protocol(msg)) => {
        // Invalid session ID, missing parameters, etc.
        eprintln!("Protocol error: {}", msg);
    }
    Err(AgentError::ToolExecution(msg)) => {
        // Command execution failed
        eprintln!("Execution error: {}", msg);
    }
    Err(e) => {
        eprintln!("Unexpected error: {}", e);
    }
}
```

### Validation Errors

- **Invalid Session ID** - Session doesn't exist or malformed ULID
- **Terminal Not Found** - Terminal ID doesn't exist
- **Terminal Released** - Operation not allowed on released terminal
- **Empty Environment Variable** - Environment variable name cannot be empty
- **Relative Working Directory** - Working directory must be absolute path

## Best Practices

### 1. Always Release Terminals

Release terminals when done to free resources:

```rust
// Create and use terminal
let terminal_id = manager.create_terminal_with_command(
    &session_manager,
    params
).await?;

// ... use terminal ...

// Always release
let release_params = TerminalReleaseParams {
    session_id: session_id.to_string(),
    terminal_id: terminal_id.clone(),
};
manager.release_terminal(&session_manager, release_params).await?;
```

### 2. Set Appropriate Buffer Limits

Choose buffer limits based on expected output:

```rust
// Small output (commands like 'ls')
output_byte_limit: Some(1_048_576), // 1MB

// Medium output (build logs)
output_byte_limit: Some(10_485_760), // 10MB

// Large output (data processing)
output_byte_limit: Some(104_857_600), // 100MB
```

### 3. Handle Exit Status

Always check exit status for command success:

```rust
let exit_status = manager.wait_for_exit(&session_manager, params).await?;

match exit_status.exit_code {
    Some(0) => println!("Success!"),
    Some(code) => eprintln!("Failed with exit code: {}", code),
    None => {
        if let Some(signal) = exit_status.signal {
            eprintln!("Killed by signal: {}", signal);
        }
    }
}
```

### 4. Use Absolute Paths

Always use absolute paths for working directories:

```rust
use std::path::PathBuf;

// Convert relative to absolute
let cwd = std::env::current_dir()?
    .join("subdir")
    .canonicalize()?;

let params = TerminalCreateParams {
    session_id: session_id.to_string(),
    command: "ls".to_string(),
    args: None,
    env: None,
    cwd: Some(cwd.to_string_lossy().to_string()),
    output_byte_limit: None,
};
```

### 5. Concurrent Operations

The terminal manager is thread-safe and supports concurrent operations:

```rust
use tokio::task::JoinSet;

let mut set = JoinSet::new();

// Create multiple terminals concurrently
for i in 0..5 {
    let manager_clone = manager.clone();
    let session_manager_clone = session_manager.clone();
    let session_id_clone = session_id.to_string();
    
    set.spawn(async move {
        let params = TerminalCreateParams {
            session_id: session_id_clone,
            command: "echo".to_string(),
            args: Some(vec![format!("Task {}", i)]),
            env: None,
            cwd: None,
            output_byte_limit: None,
        };
        
        manager_clone.create_terminal_with_command(
            &session_manager_clone,
            params
        ).await
    });
}

// Wait for all to complete
while let Some(result) = set.join_next().await {
    match result {
        Ok(Ok(terminal_id)) => println!("Created: {}", terminal_id),
        Ok(Err(e)) => eprintln!("Error: {}", e),
        Err(e) => eprintln!("Join error: {}", e),
    }
}
```

## Advanced Usage

### Long-Running Processes

For long-running processes, periodically check output:

```rust
use tokio::time::{sleep, Duration};

let terminal_id = manager.create_terminal_with_command(
    &session_manager,
    params
).await?;

// Poll output every second
loop {
    let params = TerminalOutputParams {
        session_id: session_id.to_string(),
        terminal_id: terminal_id.clone(),
    };
    
    let response = manager.get_output(&session_manager, params).await?;
    
    println!("Current output:\n{}", response.output);
    
    if response.exit_status.is_some() {
        println!("Process completed");
        break;
    }
    
    sleep(Duration::from_secs(1)).await;
}
```

### Interactive Commands

For commands that require interaction, use non-interactive flags:

```rust
// Bad: Interactive command may hang
let params = TerminalCreateParams {
    session_id: session_id.to_string(),
    command: "npm".to_string(),
    args: Some(vec!["init".to_string()]),
    env: None,
    cwd: None,
    output_byte_limit: None,
};

// Good: Use non-interactive flag
let params = TerminalCreateParams {
    session_id: session_id.to_string(),
    command: "npm".to_string(),
    args: Some(vec!["init".to_string(), "-y".to_string()]),
    env: None,
    cwd: None,
    output_byte_limit: None,
};
```

### Environment Inheritance

Terminals inherit system environment variables by default:

```rust
// System environment is inherited
std::env::set_var("MY_VAR", "value");

let params = TerminalCreateParams {
    session_id: session_id.to_string(),
    command: "printenv".to_string(),
    args: Some(vec!["MY_VAR".to_string()]),
    env: None, // Will see MY_VAR
    cwd: None,
    output_byte_limit: None,
};
```

Override specific variables:

```rust
let params = TerminalCreateParams {
    session_id: session_id.to_string(),
    command: "printenv".to_string(),
    args: Some(vec!["MY_VAR".to_string()]),
    env: Some(vec![
        EnvVariable {
            name: "MY_VAR".to_string(),
            value: "overridden".to_string(),
        },
    ]),
    cwd: None,
    output_byte_limit: None,
};
```

## Security Considerations

### Command Injection

Always validate commands and arguments:

```rust
// Bad: User input directly in command
let user_input = get_user_input();
let params = TerminalCreateParams {
    command: format!("echo {}", user_input), // VULNERABLE
    // ...
};

// Good: Use arguments array
let params = TerminalCreateParams {
    command: "echo".to_string(),
    args: Some(vec![user_input.to_string()]), // Safe
    // ...
};
```

### Path Traversal

Validate working directories:

```rust
use std::path::PathBuf;

fn validate_working_dir(path: &str, workspace: &PathBuf) -> Result<PathBuf, String> {
    let path = PathBuf::from(path).canonicalize()
        .map_err(|_| "Invalid path".to_string())?;
    
    if !path.starts_with(workspace) {
        return Err("Path outside workspace".to_string());
    }
    
    Ok(path)
}
```

### Resource Limits

Set appropriate limits to prevent resource exhaustion:

```rust
// Limit output buffer size
output_byte_limit: Some(10_485_760), // 10MB max

// Configure graceful shutdown timeout
let timeout_config = TimeoutConfig {
    graceful_shutdown_timeout: GracefulShutdownTimeout::new(
        Duration::from_secs(5)
    ),
};
```

## Troubleshooting

### Process Not Starting

Check command exists and is executable:

```rust
use std::process::Command;

// Test command exists
match Command::new("mycommand").spawn() {
    Ok(_) => println!("Command found"),
    Err(e) => eprintln!("Command not found: {}", e),
}
```

### Output Truncated

Increase buffer limit or process output in chunks:

```rust
// Increase limit
output_byte_limit: Some(104_857_600), // 100MB

// Or monitor truncation flag
if response.truncated {
    println!("Warning: Output was truncated");
}
```

### Process Hangs

Ensure commands don't wait for input:

```rust
// Add non-interactive flags
args: Some(vec![
    "install".to_string(),
    "-y".to_string(), // Non-interactive
]),
```

### Cleanup Failures

If cleanup fails, check for:
- Processes that don't respond to signals
- Insufficient permissions
- Resource locks

```rust
match manager.cleanup_session_terminals(&session_id).await {
    Ok(count) => println!("Cleaned up {} terminals", count),
    Err(e) => {
        eprintln!("Cleanup failed: {}", e);
        // May need manual intervention
    }
}
```

## See Also

- [Session Management](session-management.md) - Managing session lifecycle
- [Security Overview](../03-security/overview.md) - Security best practices
- [ACP Specification](https://anthropic.com/acp) - Official protocol documentation
