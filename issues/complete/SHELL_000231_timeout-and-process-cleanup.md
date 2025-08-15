# Implement Timeout Handling and Process Cleanup

Refer to ./specification/shell.md

## Overview

Enhance the shell action execution with timeout handling and proper process cleanup. This ensures that long-running commands can be terminated gracefully and system resources are properly managed.

## Objective

Add timeout functionality to shell command execution with proper process termination and cleanup, following the specification requirements and security best practices.

## Tasks

### 1. Implement Timeout Mechanism

Modify the `ShellAction::execute` method to support timeouts using `tokio::time::timeout`:

```rust
async fn execute(&self, context: &mut HashMap<String, Value>) -> ActionResult<Value> {
    let command = self.substitute_string(&self.command, context);
    let start_time = std::time::Instant::now();
    
    // Create the command
    let mut cmd = create_command(&command);
    cmd.stdout(std::process::Stdio::piped())
       .stderr(std::process::Stdio::piped());
    
    // Spawn the child process
    let mut child = cmd.spawn().map_err(|e| {
        ActionError::ExecutionError(format!("Failed to spawn command: {}", e))
    })?;
    
    // Apply timeout if specified
    let timeout_duration = self.timeout.unwrap_or(Duration::from_secs(300)); // Default 5 minutes
    
    let result = tokio::time::timeout(timeout_duration, child.wait_with_output()).await;
    
    match result {
        Ok(Ok(output)) => {
            // Command completed within timeout
            let duration_ms = start_time.elapsed().as_millis() as u64;
            self.process_command_output(output, duration_ms, context)
        }
        Ok(Err(e)) => {
            Err(ActionError::ExecutionError(format!("Command execution failed: {}", e)))
        }
        Err(_) => {
            // Timeout occurred - kill the process
            tracing::warn!("Command timed out after {:?}, terminating process", timeout_duration);
            let _ = child.kill().await;
            
            let duration_ms = start_time.elapsed().as_millis() as u64;
            self.handle_timeout(context, duration_ms)
        }
    }
}
```

### 2. Implement Graceful Process Termination

Add proper process cleanup for timeout scenarios:
- First attempt graceful termination with SIGTERM (Unix) or TerminateProcess (Windows)
- If process doesn't terminate, use SIGKILL (Unix) or force termination (Windows)
- Provide a grace period between termination attempts

```rust
async fn terminate_process_gracefully(child: &mut tokio::process::Child) -> ActionResult<()> {
    // Try graceful termination first
    if let Err(e) = child.kill() {
        tracing::warn!("Failed to terminate process gracefully: {}", e);
    }
    
    // Give process time to clean up
    let grace_period = Duration::from_secs(2);
    let result = tokio::time::timeout(grace_period, child.wait()).await;
    
    if result.is_err() {
        tracing::warn!("Process did not terminate within grace period, force killing");
        // Process is likely already dead at this point due to kill() above
        // tokio::process::Child::kill() is already forceful
    }
    
    Ok(())
}
```

### 3. Add Duration Tracking

Implement execution duration tracking as specified:
- Track total execution time in milliseconds
- Set `duration_ms` context variable
- Log execution timing information

```rust
fn process_command_output(
    &self,
    output: std::process::Output,
    duration_ms: u64,
    context: &mut HashMap<String, Value>
) -> ActionResult<Value> {
    let success = output.status.success();
    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    
    // Set all required context variables
    context.insert("success".to_string(), Value::Bool(success));
    context.insert("failure".to_string(), Value::Bool(!success));
    context.insert("exit_code".to_string(), Value::Number(exit_code.into()));
    context.insert("stdout".to_string(), Value::String(stdout.clone()));
    context.insert("stderr".to_string(), Value::String(stderr));
    context.insert("duration_ms".to_string(), Value::Number(duration_ms.into()));
    
    // Set result variable if specified
    if let Some(result_var) = &self.result_variable {
        context.insert(result_var.clone(), Value::String(stdout.clone()));
    }
    
    tracing::info!(
        "Shell command completed in {}ms with exit code {}", 
        duration_ms, exit_code
    );
    
    Ok(if success { 
        Value::String(stdout) 
    } else { 
        Value::Bool(false) 
    })
}
```

### 4. Handle Timeout Scenarios

Implement proper timeout handling that sets appropriate context variables:

```rust
fn handle_timeout(
    &self,
    context: &mut HashMap<String, Value>,
    duration_ms: u64
) -> ActionResult<Value> {
    // Set timeout-specific context variables
    context.insert("success".to_string(), Value::Bool(false));
    context.insert("failure".to_string(), Value::Bool(true));
    context.insert("exit_code".to_string(), Value::Number((-1).into()));
    context.insert("stdout".to_string(), Value::String("".to_string()));
    context.insert("stderr".to_string(), Value::String("Command timed out".to_string()));
    context.insert("duration_ms".to_string(), Value::Number(duration_ms.into()));
    
    // Don't set result variable on timeout
    
    Ok(Value::Bool(false))
}
```

### 5. Add Timeout Configuration

Support timeout configuration:
- Default timeout from specification (no timeout initially, but add reasonable default)
- Respect timeout parameter from action configuration
- Add maximum timeout limits for security

### 6. Resource Cleanup

Ensure proper resource cleanup:
- Close file handles properly
- Clean up child processes
- Handle zombie processes appropriately
- Add cleanup on drop if needed

## Success Criteria

- [ ] Timeout mechanism works correctly
- [ ] Processes are terminated gracefully when they exceed timeout
- [ ] Force termination works if graceful termination fails
- [ ] Duration tracking works and sets `duration_ms` variable
- [ ] Timeout scenarios set appropriate context variables
- [ ] Resource cleanup prevents zombie processes
- [ ] Cross-platform process termination works
- [ ] Logging provides appropriate timeout information

## Testing

Write comprehensive tests for:
- Commands that complete within timeout
- Commands that exceed timeout and are terminated
- Timeout handling with different timeout values
- Duration measurement accuracy
- Process cleanup verification
- Context variable setting in timeout scenarios

Example timeout test:
```rust
#[tokio::test]
async fn test_shell_action_timeout() {
    let action = ShellAction::new("sleep 10".to_string())
        .with_timeout(Duration::from_secs(1));
    let mut context = HashMap::new();
    
    let result = action.execute(&mut context).await.unwrap();
    
    assert_eq!(context.get("success"), Some(&Value::Bool(false)));
    assert_eq!(context.get("failure"), Some(&Value::Bool(true)));
    assert!(context.get("stderr").unwrap().as_str().unwrap().contains("timed out"));
    assert!(context.get("duration_ms").unwrap().as_u64().unwrap() >= 1000);
}
```

## Security Considerations

- Implement reasonable maximum timeout limits (e.g., 5 minutes default, 1 hour maximum)
- Ensure process termination cannot be bypassed
- Log timeout events for security monitoring

## Next Steps

After completing this step, proceed to implementing working directory and environment variable support.
## Proposed Solution

After reviewing the current implementation in `actions.rs:1057-1170`, I can see that a basic shell action exists but lacks proper timeout handling and process cleanup. The current timeout implementation is incomplete and doesn't handle process termination gracefully.

My implementation will:

1. **Enhance timeout mechanism**: Replace the basic timeout with proper process spawning and termination
2. **Implement graceful process cleanup**: Add proper SIGTERM/SIGKILL handling with grace periods
3. **Improve duration tracking**: Ensure accurate timing for both successful and timeout scenarios
4. **Add proper context variable handling**: Set all required variables for timeout scenarios
5. **Test thoroughly**: Add comprehensive tests for timeout scenarios

The key changes will be in the `ShellAction::execute` method to:
- Use `tokio::process::Child` for better process control
- Implement graceful termination with `child.kill()` and cleanup
- Set proper context variables for timeout scenarios
- Ensure resources are cleaned up properly

This follows the existing patterns in the codebase while adding the missing timeout and cleanup functionality.