# Workflow System Integration and Enhancement

Refer to /Users/wballard/github/sah-shell/ideas/shell.md

## Overview

Integrate the new MCP shell tool with the existing workflow system, enhancing the current `ShellAction` implementation while maintaining backward compatibility with existing workflows.

## Objective

Update the workflow `ShellAction` to leverage the new MCP shell tool infrastructure while preserving all existing functionality and ensuring seamless operation of current workflows.

## Requirements

### ShellAction Enhancement
- Update existing `ShellAction` implementation to use MCP shell tool
- Maintain all existing functionality and behavior
- Preserve backward compatibility with existing workflows
- Enhance error handling and reporting

### Workflow Integration
- Ensure shell actions work within workflow context
- Support workflow variable substitution
- Maintain proper state management
- Handle workflow-specific error conditions

### Configuration Integration
- Support workflow-level shell configuration
- Allow per-action security and timeout overrides
- Integrate with existing workflow configuration patterns
- Maintain deployment flexibility

### Testing and Validation
- Ensure existing workflows continue to work unchanged
- Test integration with workflow execution engine
- Validate state management and error propagation
- Confirm resource cleanup and management

## Implementation Details

### Enhanced ShellAction Structure
```rust
// In swissarmyhammer/src/workflow/actions.rs

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ShellAction {
    pub command: String,
    
    #[serde(default)]
    pub working_directory: Option<String>,
    
    #[serde(default = "default_shell_timeout")]
    pub timeout: u64,
    
    #[serde(default)]
    pub environment: Option<HashMap<String, String>>,
    
    #[serde(default)]
    pub capture_output: bool,
    
    #[serde(default)]
    pub fail_on_error: bool,
}

fn default_shell_timeout() -> u64 { 300 }
```

### MCP Tool Integration
```rust
impl ShellAction {
    pub async fn execute(
        &self,
        context: &WorkflowContext
    ) -> Result<ActionResult, WorkflowError> {
        // Initialize MCP tool context
        let mcp_context = CliToolContext::new().await
            .map_err(|e| WorkflowError::ActionFailed {
                action: "shell".to_string(),
                details: format!("Failed to initialize MCP context: {}", e),
            })?;
        
        // Substitute workflow variables in command
        let resolved_command = context.substitute_variables(&self.command)?;
        let resolved_working_dir = self.working_directory
            .as_ref()
            .map(|dir| context.substitute_variables(dir))
            .transpose()?;
        
        // Prepare MCP tool arguments
        let args = mcp_context.create_arguments(vec![
            ("command", json!(resolved_command)),
            ("working_directory", json!(resolved_working_dir)),
            ("timeout", json!(self.timeout)),
            ("environment", json!(self.environment)),
        ]);
        
        // Execute via MCP tool
        let result = mcp_context.execute_tool("shell_execute", args).await
            .map_err(|e| WorkflowError::ActionFailed {
                action: "shell".to_string(),
                details: format!("Shell execution failed: {}", e),
            })?;
        
        // Process results and update workflow state
        self.process_shell_result(result, context).await
    }
}
```

### Workflow Result Processing
```rust
impl ShellAction {
    async fn process_shell_result(
        &self,
        result: Value,
        context: &WorkflowContext
    ) -> Result<ActionResult, WorkflowError> {
        let metadata = result["metadata"].as_object()
            .ok_or_else(|| WorkflowError::InvalidResponse("Missing shell metadata".to_string()))?;
        
        let exit_code = metadata["exit_code"].as_i64().unwrap_or(-1);
        let stdout = metadata["stdout"].as_str().unwrap_or("");
        let stderr = metadata["stderr"].as_str().unwrap_or("");
        
        // Handle failure conditions
        if self.fail_on_error && exit_code != 0 {
            return Err(WorkflowError::ActionFailed {
                action: "shell".to_string(),
                details: format!(
                    "Command failed with exit code {}: {}",
                    exit_code,
                    stderr
                ),
            });
        }
        
        // Build action result with output
        let mut action_result = ActionResult::new();
        
        if self.capture_output {
            action_result.set_output("stdout", stdout);
            action_result.set_output("stderr", stderr);
            action_result.set_output("exit_code", exit_code);
        }
        
        // Log execution details
        tracing::info!(
            command = %self.command,
            exit_code = exit_code,
            execution_time_ms = metadata["execution_time_ms"].as_u64(),
            "Shell action completed"
        );
        
        Ok(action_result)
    }
}
```

### Backward Compatibility
```rust
// Ensure existing workflow definitions continue to work
impl From<LegacyShellAction> for ShellAction {
    fn from(legacy: LegacyShellAction) -> Self {
        Self {
            command: legacy.command,
            working_directory: legacy.working_directory,
            timeout: legacy.timeout.unwrap_or(300),
            environment: legacy.environment,
            capture_output: legacy.capture_output.unwrap_or(true),
            fail_on_error: legacy.fail_on_error.unwrap_or(true),
        }
    }
}
```

### Configuration Enhancement
```rust
// Workflow-level shell configuration
#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowShellConfig {
    pub default_timeout: Option<u64>,
    pub default_working_directory: Option<String>,
    pub security_policy: Option<SecurityPolicy>,
    pub output_limits: Option<OutputLimits>,
}

impl WorkflowContext {
    pub fn shell_config(&self) -> Option<&WorkflowShellConfig> {
        self.config.shell.as_ref()
    }
}
```

## Integration Points

### Existing Workflow Actions
- Study current `ShellAction` implementation thoroughly
- Understand existing workflows that use shell actions
- Ensure all existing behavior preserved
- Test with existing workflow test suites

### Workflow Execution Engine
- Integrate with existing workflow executor
- Maintain proper async execution patterns
- Support workflow cancellation and timeout
- Handle resource cleanup properly

### Error Handling Integration
- Use existing workflow error types and patterns
- Integrate with workflow abort system
- Support proper error propagation
- Maintain error context and details

## Testing Strategy

### Backward Compatibility Testing
- Run existing workflow test suite unchanged
- Test all existing shell action workflows
- Verify identical behavior with new implementation
- Ensure no regressions in workflow functionality

### Integration Testing
- Test shell actions within complete workflows
- Verify variable substitution works correctly
- Test error handling and propagation
- Validate state management and output capture

### Performance Testing
- Compare performance with previous implementation
- Ensure no significant performance regression
- Test with various command types and sizes
- Validate resource usage and cleanup

## Acceptance Criteria

- [ ] ShellAction updated to use MCP shell tool
- [ ] All existing workflows continue to work unchanged
- [ ] Backward compatibility maintained completely
- [ ] Error handling improved without breaking changes
- [ ] Performance maintained or improved
- [ ] Configuration integration working
- [ ] Workflow test suite passes without modification
- [ ] Resource cleanup and management working properly

## Migration Notes

### For Existing Workflows
- No changes required to existing workflow definitions
- All existing shell actions continue to work identically
- Enhanced error reporting and logging available
- New configuration options available but optional

### For Workflow Authors
- New timeout and environment options available
- Enhanced security validation available
- Better error messages and debugging information
- Improved resource management and cleanup

## Notes

- This step is critical for maintaining system integrity
- Focus heavily on backward compatibility and regression testing
- The existing workflow system is production-critical
- Any breaking changes would affect users significantly
- Enhanced features should be additive, not replacing existing functionality