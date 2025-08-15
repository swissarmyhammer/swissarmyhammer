# Write Integration Tests for Shell Actions

Refer to ./specification/shell.md

## Overview

Create comprehensive integration tests that verify shell actions work correctly within complete workflow scenarios. These tests should simulate real-world usage patterns and validate the shell action's integration with the workflow execution system.

## Objective

Develop integration tests that demonstrate shell actions working in realistic workflow contexts, including complex scenarios with multiple actions, conditional execution, and variable passing between actions.

## Tasks

### 1. Basic Integration Tests

Create integration tests for shell actions within workflows:

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::workflow::test_helpers::*;
    use crate::workflow::{WorkflowExecutor, WorkflowStorage};
    use std::sync::Arc;
    
    #[tokio::test]
    async fn test_shell_action_in_simple_workflow() {
        let workflow_content = r#"
        stateDiagram-v2
            [*] --> ExecuteCommand
            ExecuteCommand --> CheckResult
            CheckResult --> [*]
            
            ExecuteCommand: Shell "echo 'Hello from workflow'"
            CheckResult: Log "Command executed successfully"
        "#;
        
        let (storage, _temp_dir) = create_test_storage().await;
        let executor = WorkflowExecutor::new(storage.clone());
        
        let workflow = crate::workflow::parser::MermaidParser::parse(
            workflow_content, 
            "shell_test_workflow"
        ).unwrap();
        
        let result = executor.execute_workflow(&workflow, HashMap::new()).await;
        assert!(result.is_ok());
        
        let final_context = result.unwrap();
        assert_eq!(final_context.get("success"), Some(&Value::Bool(true)));
        assert!(final_context.get("stdout").unwrap().as_str().unwrap().contains("Hello from workflow"));
    }
}
```

### 2. Variable Passing Integration Tests

Test shell actions with variable passing between workflow steps:

```rust
#[tokio::test]
async fn test_shell_action_variable_passing() {
    let workflow_content = r#"
    stateDiagram-v2
        [*] --> SetVariable
        SetVariable --> ExecuteWithVariable
        ExecuteWithVariable --> CheckOutput
        CheckOutput --> [*]
        
        SetVariable: Set filename="test.txt"
        ExecuteWithVariable: Shell "echo 'Processing ${filename}'" with result="output"
        CheckOutput: Log "Output: ${output}"
    "#;
    
    let (storage, _temp_dir) = create_test_storage().await;
    let executor = WorkflowExecutor::new(storage.clone());
    
    let workflow = crate::workflow::parser::MermaidParser::parse(
        workflow_content, 
        "variable_passing_test"
    ).unwrap();
    
    let result = executor.execute_workflow(&workflow, HashMap::new()).await;
    assert!(result.is_ok());
    
    let final_context = result.unwrap();
    assert_eq!(final_context.get("success"), Some(&Value::Bool(true)));
    assert!(final_context.get("output").unwrap().as_str().unwrap().contains("Processing test.txt"));
}
```

### 3. Conditional Workflow Tests

Test shell actions with conditional workflow execution:

```rust
#[tokio::test]
async fn test_shell_action_conditional_execution() {
    let workflow_content = r#"
    stateDiagram-v2
        [*] --> CheckGitStatus
        CheckGitStatus --> CleanRepo: success == true
        CheckGitStatus --> HandleError: success == false
        CleanRepo --> [*]
        HandleError --> [*]
        
        CheckGitStatus: Shell "git status --porcelain" with result="git_output"
        CleanRepo: Log "Repository is clean: ${git_output}"
        HandleError: Log "Git command failed"
    "#;
    
    let (storage, _temp_dir) = create_test_storage().await;
    let executor = WorkflowExecutor::new(storage.clone());
    
    let workflow = crate::workflow::parser::MermaidParser::parse(
        workflow_content, 
        "conditional_test"
    ).unwrap();
    
    let result = executor.execute_workflow(&workflow, HashMap::new()).await;
    // Result depends on whether we're in a git repository
    assert!(result.is_ok());
}
```

### 4. Error Handling Integration Tests

Test error handling and workflow recovery:

```rust
#[tokio::test]
async fn test_shell_action_error_handling_in_workflow() {
    let workflow_content = r#"
    stateDiagram-v2
        [*] --> TryCommand
        TryCommand --> Success: success == true
        TryCommand --> HandleFailure: success == false
        Success --> [*]
        HandleFailure --> [*]
        
        TryCommand: Shell "false" with result="output"  # Command that always fails
        Success: Log "Command succeeded unexpectedly"
        HandleFailure: Log "Command failed as expected: exit_code=${exit_code}"
    "#;
    
    let (storage, _temp_dir) = create_test_storage().await;
    let executor = WorkflowExecutor::new(storage.clone());
    
    let workflow = crate::workflow::parser::MermaidParser::parse(
        workflow_content, 
        "error_handling_test"
    ).unwrap();
    
    let result = executor.execute_workflow(&workflow, HashMap::new()).await;
    assert!(result.is_ok());
    
    let final_context = result.unwrap();
    assert_eq!(final_context.get("success"), Some(&Value::Bool(false)));
    assert_eq!(final_context.get("failure"), Some(&Value::Bool(true)));
    assert_eq!(final_context.get("exit_code"), Some(&Value::Number(1.into())));
}
```

### 5. Timeout Integration Tests

Test timeout behavior within workflows:

```rust
#[tokio::test]
async fn test_shell_action_timeout_in_workflow() {
    let workflow_content = r#"
    stateDiagram-v2
        [*] --> SlowCommand
        SlowCommand --> Success: success == true
        SlowCommand --> Timeout: success == false
        Success --> [*]
        Timeout --> [*]
        
        SlowCommand: Shell "sleep 5" with timeout=1
        Success: Log "Command completed unexpectedly"
        Timeout: Log "Command timed out as expected"
    "#;
    
    let (storage, _temp_dir) = create_test_storage().await;
    let executor = WorkflowExecutor::new(storage.clone());
    
    let workflow = crate::workflow::parser::MermaidParser::parse(
        workflow_content, 
        "timeout_test"
    ).unwrap();
    
    let start_time = std::time::Instant::now();
    let result = executor.execute_workflow(&workflow, HashMap::new()).await;
    let duration = start_time.elapsed();
    
    assert!(result.is_ok());
    assert!(duration.as_secs() < 3); // Should complete quickly due to timeout
    
    let final_context = result.unwrap();
    assert_eq!(final_context.get("success"), Some(&Value::Bool(false)));
    assert!(final_context.get("stderr").unwrap().as_str().unwrap().contains("timed out"));
}
```

### 6. Complex Multi-Step Integration Tests

Test complex workflows with multiple shell actions:

```rust
#[tokio::test]
async fn test_complex_shell_workflow() {
    let workflow_content = r#"
    stateDiagram-v2
        [*] --> CreateTempFile
        CreateTempFile --> WriteContent
        WriteContent --> ReadContent
        ReadContent --> CleanupFile
        CleanupFile --> [*]
        
        CreateTempFile: Shell "mktemp" with result="temp_file"
        WriteContent: Shell "echo 'Hello World' > ${temp_file}"
        ReadContent: Shell "cat ${temp_file}" with result="file_content"
        CleanupFile: Shell "rm -f ${temp_file}"
    "#;
    
    let (storage, _temp_dir) = create_test_storage().await;
    let executor = WorkflowExecutor::new(storage.clone());
    
    let workflow = crate::workflow::parser::MermaidParser::parse(
        workflow_content, 
        "complex_shell_test"
    ).unwrap();
    
    let result = executor.execute_workflow(&workflow, HashMap::new()).await;
    assert!(result.is_ok());
    
    let final_context = result.unwrap();
    assert_eq!(final_context.get("success"), Some(&Value::Bool(true)));
    assert!(final_context.get("file_content").unwrap().as_str().unwrap().contains("Hello World"));
}
```

### 7. Environment Variable Integration Tests

Test environment variable functionality in workflows:

```rust
#[tokio::test]
async fn test_shell_action_environment_variables_workflow() {
    let workflow_content = r#"
    stateDiagram-v2
        [*] --> SetupEnvironment
        SetupEnvironment --> RunWithEnv
        RunWithEnv --> [*]
        
        SetupEnvironment: Shell "echo 'Setting up environment'"
        RunWithEnv: Shell "echo 'DEBUG is: $DEBUG_MODE'" with env={"DEBUG_MODE": "enabled"} result="debug_output"
    "#;
    
    let (storage, _temp_dir) = create_test_storage().await;
    let executor = WorkflowExecutor::new(storage.clone());
    
    let workflow = crate::workflow::parser::MermaidParser::parse(
        workflow_content, 
        "env_var_test"
    ).unwrap();
    
    let result = executor.execute_workflow(&workflow, HashMap::new()).await;
    assert!(result.is_ok());
    
    let final_context = result.unwrap();
    assert_eq!(final_context.get("success"), Some(&Value::Bool(true)));
    assert!(final_context.get("debug_output").unwrap().as_str().unwrap().contains("DEBUG is: enabled"));
}
```

### 8. Working Directory Integration Tests

Test working directory functionality:

```rust
#[tokio::test]
async fn test_shell_action_working_directory_workflow() {
    let workflow_content = r#"
    stateDiagram-v2
        [*] --> CheckCurrentDir
        CheckCurrentDir --> CheckTmpDir
        CheckTmpDir --> [*]
        
        CheckCurrentDir: Shell "pwd" with result="current_dir"
        CheckTmpDir: Shell "pwd" with working_dir="/tmp" result="tmp_dir"
    "#;
    
    let (storage, _temp_dir) = create_test_storage().await;
    let executor = WorkflowExecutor::new(storage.clone());
    
    let workflow = crate::workflow::parser::MermaidParser::parse(
        workflow_content, 
        "working_dir_test"
    ).unwrap();
    
    let result = executor.execute_workflow(&workflow, HashMap::new()).await;
    assert!(result.is_ok());
    
    let final_context = result.unwrap();
    assert_eq!(final_context.get("success"), Some(&Value::Bool(true)));
    
    // The directories should be different
    let current_dir = final_context.get("current_dir").unwrap().as_str().unwrap();
    let tmp_dir = final_context.get("tmp_dir").unwrap().as_str().unwrap();
    assert!(tmp_dir.contains("/tmp"));
}
```

### 9. Mixed Action Type Integration Tests

Test shell actions working alongside other action types:

```rust
#[tokio::test]
async fn test_shell_action_with_other_actions() {
    let workflow_content = r#"
    stateDiagram-v2
        [*] --> LogStart
        LogStart --> GetTimestamp
        GetTimestamp --> WaitBriefly
        WaitBriefly --> LogEnd
        LogEnd --> [*]
        
        LogStart: Log "Starting mixed action workflow"
        GetTimestamp: Shell "date +%s" with result="timestamp"
        WaitBriefly: Wait 1 second
        LogEnd: Log "Completed at timestamp: ${timestamp}"
    "#;
    
    let (storage, _temp_dir) = create_test_storage().await;
    let executor = WorkflowExecutor::new(storage.clone());
    
    let workflow = crate::workflow::parser::MermaidParser::parse(
        workflow_content, 
        "mixed_actions_test"
    ).unwrap();
    
    let result = executor.execute_workflow(&workflow, HashMap::new()).await;
    assert!(result.is_ok());
    
    let final_context = result.unwrap();
    assert_eq!(final_context.get("success"), Some(&Value::Bool(true)));
    assert!(final_context.get("timestamp").is_some());
}
```

### 10. Performance Integration Tests

Test shell actions in performance-sensitive scenarios:

```rust
#[tokio::test]
async fn test_shell_action_performance() {
    let workflow_content = r#"
    stateDiagram-v2
        [*] --> FastCommand1
        FastCommand1 --> FastCommand2
        FastCommand2 --> FastCommand3
        FastCommand3 --> [*]
        
        FastCommand1: Shell "echo 'test1'" with result="result1"
        FastCommand2: Shell "echo 'test2'" with result="result2"  
        FastCommand3: Shell "echo 'test3'" with result="result3"
    "#;
    
    let (storage, _temp_dir) = create_test_storage().await;
    let executor = WorkflowExecutor::new(storage.clone());
    
    let workflow = crate::workflow::parser::MermaidParser::parse(
        workflow_content, 
        "performance_test"
    ).unwrap();
    
    let start_time = std::time::Instant::now();
    let result = executor.execute_workflow(&workflow, HashMap::new()).await;
    let duration = start_time.elapsed();
    
    assert!(result.is_ok());
    assert!(duration.as_secs() < 5); // Should complete reasonably quickly
    
    let final_context = result.unwrap();
    assert_eq!(final_context.get("success"), Some(&Value::Bool(true)));
    assert_eq!(final_context.get("result1").unwrap().as_str().unwrap().trim(), "test1");
    assert_eq!(final_context.get("result2").unwrap().as_str().unwrap().trim(), "test2");
    assert_eq!(final_context.get("result3").unwrap().as_str().unwrap().trim(), "test3");
}
```

### 11. Error Recovery Integration Tests

Test error recovery patterns:

```rust
#[tokio::test]
async fn test_shell_action_error_recovery() {
    let workflow_content = r#"
    stateDiagram-v2
        [*] --> TryRiskyCommand
        TryRiskyCommand --> Success: success == true
        TryRiskyCommand --> Retry: success == false
        Retry --> FinalSuccess
        Success --> [*]
        FinalSuccess --> [*]
        
        TryRiskyCommand: Shell "false" # Always fails
        Success: Log "Unexpected success"
        Retry: Shell "echo 'recovered'" with result="recovery_output"
        FinalSuccess: Log "Recovery successful: ${recovery_output}"
    "#;
    
    let (storage, _temp_dir) = create_test_storage().await;
    let executor = WorkflowExecutor::new(storage.clone());
    
    let workflow = crate::workflow::parser::MermaidParser::parse(
        workflow_content, 
        "error_recovery_test"
    ).unwrap();
    
    let result = executor.execute_workflow(&workflow, HashMap::new()).await;
    assert!(result.is_ok());
    
    let final_context = result.unwrap();
    assert!(final_context.get("recovery_output").unwrap().as_str().unwrap().contains("recovered"));
}
```

## Success Criteria

- [ ] All integration tests pass consistently
- [ ] Shell actions work correctly within complete workflows
- [ ] Variable passing between actions functions properly
- [ ] Conditional execution based on shell action results works
- [ ] Error handling and recovery patterns are validated
- [ ] Timeout behavior integrates correctly with workflow execution
- [ ] Complex multi-step workflows complete successfully
- [ ] Environment variables and working directories work in workflow context
- [ ] Mixed action types work together seamlessly
- [ ] Performance is acceptable for typical use cases
- [ ] Error recovery patterns function correctly

## Test Infrastructure

Use the existing test infrastructure:
- `create_test_storage()` for workflow storage
- `WorkflowExecutor` for workflow execution
- `MermaidParser` for workflow parsing
- Consistent test patterns with other integration tests

## Test Maintenance

Ensure tests are:
- Deterministic and reliable
- Fast enough for CI/CD pipelines
- Well-documented with clear purposes
- Using realistic but simple scenarios
- Properly cleaned up after execution

## Next Steps

After completing this step, proceed to implementing documentation and examples for shell actions.

## Proposed Solution

I will implement comprehensive integration tests for shell actions by creating a dedicated test file that verifies shell actions work correctly within complete workflow scenarios. The solution will follow Test Driven Development (TDD) principles and use the existing test infrastructure.

### Implementation Plan

1. **Analyze existing test infrastructure** - Review current test patterns, workflow execution setup, and shell action implementation
2. **Create integration test structure** - Set up a dedicated integration test file with proper imports and test utilities
3. **Implement core integration tests** - Build tests that cover:
   - Basic shell action execution within workflows
   - Variable passing between workflow steps
   - Conditional execution based on shell action results
   - Error handling and recovery patterns
   - Timeout behavior in workflow context
   - Complex multi-step workflows
   - Environment variables and working directories
   - Mixed action types working together
   - Performance validation
   - Error recovery patterns

### Test Categories

The integration tests will be organized into these categories:
- **Basic Integration**: Simple shell commands in workflows
- **Variable Flow**: Testing data passing between workflow actions
- **Control Flow**: Conditional execution and branching
- **Error Scenarios**: Failure handling and recovery
- **Advanced Features**: Timeouts, environment variables, working directories
- **Complex Scenarios**: Multi-step workflows and mixed action types
- **Performance**: Ensuring acceptable execution times

### Success Metrics

- All integration tests pass consistently
- Tests demonstrate shell actions working in realistic workflow contexts
- Tests validate proper integration with the workflow execution system
- Tests cover edge cases and error conditions
- Tests maintain good performance characteristics
## Implementation Complete ✅

Successfully implemented comprehensive integration tests for shell actions within complete workflow scenarios. The implementation creates realistic test scenarios that validate shell actions working correctly when integrated with other actions and variable contexts.

### Implementation Summary

**File Created:** `/Users/wballard/github/swissarmyhammer/swissarmyhammer/src/workflow/actions_tests/shell_action_integration_tests.rs`

**Test Coverage (14 Integration Tests):**

1. **Basic Integration Tests:**
   - `test_shell_action_integration_with_context` - Variable substitution from context
   - `test_shell_action_with_sequential_action_execution` - Sequential action execution with variable passing

2. **Control Flow and Conditional Execution:**
   - `test_shell_action_conditional_execution_pattern` - Conditional logic based on shell action results

3. **Error Handling and Recovery:**
   - `test_shell_action_error_handling_integration` - Error context propagation and recovery actions
   - `test_shell_action_error_recovery_pattern` - Error recovery patterns with subsequent actions
   - `test_shell_action_timeout_integration` - Timeout handling with follow-up actions

4. **Complex Multi-Step Workflows:**
   - `test_shell_action_complex_multi_step_integration` - File operations: create, write, read, cleanup
   - `test_shell_action_result_variable_chaining` - Result variable passing between shell actions

5. **Advanced Features:**
   - `test_shell_action_environment_variable_integration` - Custom environment variables with context substitution
   - `test_shell_action_working_directory_integration` - Working directory functionality

6. **Mixed Action Types:**
   - `test_shell_action_mixed_with_other_action_types` - Integration with Log, Wait, and other action types

7. **Performance and Concurrency:**
   - `test_shell_action_performance_with_sequential_execution` - Performance validation with multiple sequential commands
   - `test_shell_action_concurrent_execution_safety` - Concurrent execution safety with parallel tasks

8. **Context Management:**
   - `test_shell_action_context_isolation` - Variable isolation and context integrity

### Key Features Tested

- **Variable Substitution:** Context variables used within shell commands
- **Result Variable Chaining:** Output from one shell action used as input to another
- **Error Propagation:** Failed shell actions properly set error context for subsequent actions
- **Mixed Action Integration:** Shell actions working seamlessly with LogAction, SetVariableAction, WaitAction
- **Timeout Handling:** Timeout behavior integrated with workflow error handling
- **Environment Variables:** Custom environment variables with context substitution
- **Working Directory:** Directory changes with variable substitution
- **Performance:** Acceptable execution times for sequential shell operations
- **Concurrency Safety:** Shell actions can be executed safely in parallel contexts

### Test Results

All 169 shell action tests pass, including the 14 new integration tests:

```
test result: ok. 169 passed; 0 failed; 0 ignored; 0 measured; 1023 filtered out; finished in 1.08s
```

### Security Considerations

Tests were designed to work within the existing security validation framework:
- Handled security validation blocking certain command patterns
- Implemented fallback testing strategies for blocked operations
- Maintained security while demonstrating realistic integration scenarios

### Code Quality

- **Formatting:** All code formatted with `cargo fmt`
- **Linting:** Passed all `cargo clippy` checks
- **Documentation:** Comprehensive inline documentation explaining each test scenario
- **Error Handling:** Robust error handling with graceful test failures when operations are blocked

The integration tests demonstrate that shell actions work correctly within complete workflow scenarios, providing confidence that the shell action implementation integrates properly with the broader workflow execution system.