# Write Comprehensive Unit Tests for Shell Actions

Refer to ./specification/shell.md

## Overview

Implement a comprehensive test suite for the shell action functionality, covering all features, edge cases, security scenarios, and error conditions. This ensures the shell action implementation is robust and reliable.

## Objective

Create thorough unit tests that validate all aspects of shell action functionality, following the testing patterns established in the codebase and ensuring complete coverage of the specification requirements.

## Tasks

### 1. Basic Functionality Tests

Write tests for core shell action functionality:

```rust
#[cfg(test)]
mod shell_action_tests {
    use super::*;
    use crate::workflow::test_helpers::*;
    use tokio::time::Duration;
    
    #[test]
    fn test_shell_action_creation() {
        let action = ShellAction::new("echo hello".to_string());
        assert_eq!(action.command, "echo hello");
        assert_eq!(action.timeout, None);
        assert_eq!(action.result_variable, None);
        assert_eq!(action.working_dir, None);
        assert!(action.environment.is_empty());
    }
    
    #[test]
    fn test_shell_action_builder_pattern() {
        let action = ShellAction::new("echo test".to_string())
            .with_timeout(Duration::from_secs(30))
            .with_result_variable("output".to_string())
            .with_working_dir("/tmp".to_string());
            
        assert_eq!(action.timeout, Some(Duration::from_secs(30)));
        assert_eq!(action.result_variable, Some("output".to_string()));
        assert_eq!(action.working_dir, Some("/tmp".to_string()));
    }
    
    #[tokio::test]
    async fn test_shell_action_basic_execution() {
        let action = ShellAction::new("echo 'hello world'".to_string());
        let mut context = HashMap::new();
        
        let result = action.execute(&mut context).await.unwrap();
        
        // Check return value
        assert!(result.as_str().unwrap().contains("hello world"));
        
        // Check context variables
        assert_eq!(context.get("success"), Some(&Value::Bool(true)));
        assert_eq!(context.get("failure"), Some(&Value::Bool(false)));
        assert_eq!(context.get("exit_code"), Some(&Value::Number(0.into())));
        assert!(context.get("stdout").unwrap().as_str().unwrap().contains("hello world"));
        assert_eq!(context.get("stderr"), Some(&Value::String("".to_string())));
        assert!(context.get("duration_ms").unwrap().as_u64().unwrap() > 0);
    }
}
```

### 2. Parser Tests

Write comprehensive tests for shell action parsing:

```rust
#[cfg(test)]
mod parser_tests {
    use super::*;
    use crate::workflow::action_parser::ActionParser;
    
    #[test]
    fn test_parse_basic_shell_action() {
        let parser = ActionParser::new().unwrap();
        let action = parser.parse_shell_action("Shell \"echo hello\"").unwrap().unwrap();
        
        assert_eq!(action.command, "echo hello");
        assert_eq!(action.timeout, None);
    }
    
    #[test]
    fn test_parse_case_insensitive() {
        let parser = ActionParser::new().unwrap();
        let action = parser.parse_shell_action("shell \"echo hello\"").unwrap().unwrap();
        
        assert_eq!(action.command, "echo hello");
    }
    
    #[test]
    fn test_parse_with_timeout() {
        let parser = ActionParser::new().unwrap();
        let action = parser.parse_shell_action("Shell \"echo hello\" with timeout=30").unwrap().unwrap();
        
        assert_eq!(action.command, "echo hello");
        assert_eq!(action.timeout, Some(Duration::from_secs(30)));
    }
    
    #[test]
    fn test_parse_with_result_variable() {
        let parser = ActionParser::new().unwrap();
        let action = parser.parse_shell_action("Shell \"echo hello\" with result=\"output\"").unwrap().unwrap();
        
        assert_eq!(action.command, "echo hello");
        assert_eq!(action.result_variable, Some("output".to_string()));
    }
    
    #[test]
    fn test_parse_combined_parameters() {
        let parser = ActionParser::new().unwrap();
        let action = parser.parse_shell_action(
            "Shell \"echo hello\" with timeout=60 result=\"output\" working_dir=\"/tmp\""
        ).unwrap().unwrap();
        
        assert_eq!(action.command, "echo hello");
        assert_eq!(action.timeout, Some(Duration::from_secs(60)));
        assert_eq!(action.result_variable, Some("output".to_string()));
        assert_eq!(action.working_dir, Some("/tmp".to_string()));
    }
    
    #[test]
    fn test_parse_with_environment() {
        let parser = ActionParser::new().unwrap();
        let action = parser.parse_shell_action(
            r#"Shell "echo $TEST" with env={"TEST": "value", "DEBUG": "1"}"#
        ).unwrap().unwrap();
        
        assert_eq!(action.command, "echo $TEST");
        assert_eq!(action.environment.get("TEST"), Some(&"value".to_string()));
        assert_eq!(action.environment.get("DEBUG"), Some(&"1".to_string()));
    }
    
    #[test]
    fn test_parse_invalid_syntax() {
        let parser = ActionParser::new().unwrap();
        
        // Missing quotes
        let result = parser.parse_shell_action("Shell echo hello").unwrap();
        assert!(result.is_none());
        
        // Invalid parameter
        let result = parser.parse_shell_action("Shell \"echo\" with invalid=value").unwrap();
        assert!(result.is_none());
    }
}
```

### 3. Variable Substitution Tests

Test variable substitution functionality:

```rust
#[cfg(test)]
mod variable_substitution_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_command_variable_substitution() {
        let action = ShellAction::new("echo ${message}".to_string());
        let mut context = HashMap::new();
        context.insert("message".to_string(), Value::String("hello world".to_string()));
        
        let result = action.execute(&mut context).await.unwrap();
        assert!(result.as_str().unwrap().contains("hello world"));
    }
    
    #[tokio::test]
    async fn test_working_dir_variable_substitution() {
        let action = ShellAction::new("pwd".to_string())
            .with_working_dir("${base_dir}".to_string());
        let mut context = HashMap::new();
        context.insert("base_dir".to_string(), Value::String("/tmp".to_string()));
        
        let result = action.execute(&mut context).await.unwrap();
        assert!(result.as_str().unwrap().contains("/tmp"));
    }
    
    #[tokio::test]
    async fn test_environment_variable_substitution() {
        let mut env = HashMap::new();
        env.insert("TEST_VAR".to_string(), "${test_value}".to_string());
        
        let action = ShellAction::new("echo $TEST_VAR".to_string())
            .with_environment(env);
        let mut context = HashMap::new();
        context.insert("test_value".to_string(), Value::String("substituted".to_string()));
        
        let result = action.execute(&mut context).await.unwrap();
        assert!(result.as_str().unwrap().contains("substituted"));
    }
}
```

### 4. Timeout and Process Management Tests

Test timeout handling and process cleanup:

```rust
#[cfg(test)]
mod timeout_tests {
    use super::*;
    use tokio::time::Duration;
    
    #[tokio::test]
    async fn test_successful_command_within_timeout() {
        let action = ShellAction::new("echo hello".to_string())
            .with_timeout(Duration::from_secs(5));
        let mut context = HashMap::new();
        
        let result = action.execute(&mut context).await.unwrap();
        
        assert_eq!(context.get("success"), Some(&Value::Bool(true)));
        assert!(context.get("duration_ms").unwrap().as_u64().unwrap() < 5000);
    }
    
    #[tokio::test]
    async fn test_command_timeout() {
        // Use a command that will definitely timeout
        let action = ShellAction::new("sleep 10".to_string())
            .with_timeout(Duration::from_secs(1));
        let mut context = HashMap::new();
        
        let result = action.execute(&mut context).await.unwrap();
        
        // Should indicate failure due to timeout
        assert_eq!(context.get("success"), Some(&Value::Bool(false)));
        assert_eq!(context.get("failure"), Some(&Value::Bool(true)));
        assert!(context.get("stderr").unwrap().as_str().unwrap().contains("timed out"));
        
        // Duration should be close to timeout value
        let duration = context.get("duration_ms").unwrap().as_u64().unwrap();
        assert!(duration >= 1000 && duration < 2000);
    }
    
    #[tokio::test]
    async fn test_default_timeout() {
        let action = ShellAction::new("echo hello".to_string()); // No explicit timeout
        let mut context = HashMap::new();
        
        let result = action.execute(&mut context).await.unwrap();
        assert_eq!(context.get("success"), Some(&Value::Bool(true)));
    }
}
```

### 5. Error Handling Tests

Test various error conditions:

```rust
#[cfg(test)]
mod error_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_command_failure() {
        let action = ShellAction::new("false".to_string()); // Command that always fails
        let mut context = HashMap::new();
        
        let result = action.execute(&mut context).await.unwrap();
        
        assert_eq!(context.get("success"), Some(&Value::Bool(false)));
        assert_eq!(context.get("failure"), Some(&Value::Bool(true)));
        assert_eq!(context.get("exit_code"), Some(&Value::Number(1.into())));
    }
    
    #[tokio::test]
    async fn test_invalid_command() {
        let action = ShellAction::new("nonexistent_command_12345".to_string());
        let mut context = HashMap::new();
        
        let result = action.execute(&mut context).await.unwrap();
        
        assert_eq!(context.get("success"), Some(&Value::Bool(false)));
        assert_eq!(context.get("failure"), Some(&Value::Bool(true)));
        assert!(context.get("stderr").unwrap().as_str().unwrap().len() > 0);
    }
    
    #[tokio::test]
    async fn test_invalid_working_directory() {
        let action = ShellAction::new("echo hello".to_string())
            .with_working_dir("/nonexistent/directory/12345".to_string());
        let mut context = HashMap::new();
        
        let result = action.execute(&mut context).await;
        assert!(result.is_err());
    }
}
```

### 6. Security Tests

Test security validation and restrictions:

```rust
#[cfg(test)]
mod security_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_empty_command_rejection() {
        let action = ShellAction::new("".to_string());
        let mut context = HashMap::new();
        
        let result = action.execute(&mut context).await;
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_extremely_long_command_rejection() {
        let long_command = "echo ".repeat(1000); // Very long command
        let action = ShellAction::new(long_command);
        let mut context = HashMap::new();
        
        let result = action.execute(&mut context).await;
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_path_traversal_prevention() {
        let action = ShellAction::new("pwd".to_string())
            .with_working_dir("../../../etc".to_string());
        let mut context = HashMap::new();
        
        let result = action.execute(&mut context).await;
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_dangerous_command_detection() {
        // This test depends on the security policy - might warn vs block
        let action = ShellAction::new("rm -rf /tmp/test".to_string());
        let mut context = HashMap::new();
        
        // Should execute but log warnings (depending on policy)
        let result = action.execute(&mut context).await;
        // Verify appropriate security logging occurred
    }
    
    #[test]
    fn test_environment_variable_validation() {
        let mut invalid_env = HashMap::new();
        invalid_env.insert("123invalid".to_string(), "value".to_string());
        
        let result = validate_environment_variables(&invalid_env);
        assert!(result.is_err());
    }
}
```

### 7. Integration Tests

Test integration with the action system:

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::workflow::actions::parse_action_from_description;
    
    #[test]
    fn test_action_dispatch_integration() {
        let action = parse_action_from_description("Shell \"echo hello\"")
            .unwrap()
            .unwrap();
        
        assert_eq!(action.action_type(), "shell");
        assert_eq!(action.description(), "Execute shell command: echo hello");
    }
    
    #[test]
    fn test_action_type_and_description() {
        let action = ShellAction::new("echo test".to_string())
            .with_timeout(Duration::from_secs(30));
        
        assert_eq!(action.action_type(), "shell");
        assert!(action.description().contains("echo test"));
        assert!(action.description().contains("30s timeout"));
    }
    
    #[tokio::test]
    async fn test_result_variable_capture() {
        let action = ShellAction::new("echo 'captured output'".to_string())
            .with_result_variable("my_result".to_string());
        let mut context = HashMap::new();
        
        let result = action.execute(&mut context).await.unwrap();
        
        assert_eq!(
            context.get("my_result").unwrap().as_str().unwrap().trim(),
            "captured output"
        );
    }
}
```

### 8. Cross-Platform Tests

Add tests for cross-platform compatibility:

```rust
#[cfg(test)]
mod cross_platform_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_cross_platform_echo() {
        let action = ShellAction::new("echo hello".to_string());
        let mut context = HashMap::new();
        
        let result = action.execute(&mut context).await.unwrap();
        
        assert_eq!(context.get("success"), Some(&Value::Bool(true)));
        assert!(context.get("stdout").unwrap().as_str().unwrap().contains("hello"));
    }
    
    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn test_windows_specific_commands() {
        let action = ShellAction::new("dir".to_string());
        let mut context = HashMap::new();
        
        let result = action.execute(&mut context).await.unwrap();
        assert_eq!(context.get("success"), Some(&Value::Bool(true)));
    }
    
    #[cfg(not(target_os = "windows"))]
    #[tokio::test]
    async fn test_unix_specific_commands() {
        let action = ShellAction::new("ls".to_string());
        let mut context = HashMap::new();
        
        let result = action.execute(&mut context).await.unwrap();
        assert_eq!(context.get("success"), Some(&Value::Bool(true)));
    }
}
```

## Success Criteria

- [ ] All basic functionality tests pass
- [ ] Parser tests cover all syntax variations
- [ ] Variable substitution tests validate templating
- [ ] Timeout and process management tests verify cleanup
- [ ] Error handling tests cover all failure modes
- [ ] Security tests validate protection mechanisms
- [ ] Integration tests verify action system compatibility
- [ ] Cross-platform tests ensure portability
- [ ] Test coverage is comprehensive (>95%)
- [ ] All tests run reliably and deterministically

## Test Organization

Follow the existing test organization patterns:
- Tests in `#[cfg(test)]` modules within the same file
- Use descriptive test names
- Group related tests in sub-modules
- Use helper functions for common test setup
- Follow AAA pattern (Arrange, Act, Assert)

## Performance Considerations

Add performance tests if needed:
- Test with various command execution times
- Verify timeout precision
- Test resource cleanup efficiency

## Next Steps

After completing this step, proceed to implementing integration tests with real workflow scenarios.

## Proposed Solution

After analyzing the existing shell action test suite in `/Users/wballard/github/swissarmyhammer/swissarmyhammer/src/workflow/actions_tests/shell_action_tests.rs`, I've identified that there's already substantial test coverage but some comprehensive areas can be expanded according to the issue specification.

### Current Test Coverage Analysis
The existing test file has 599 lines and covers:
- Basic functionality (creation, builder pattern, execution)
- Security validation (command injection, environment variables, timeouts)  
- Error handling (invalid commands, directories, environment variables)
- Variable substitution (command, working directory, environment)
- Integration (action system, parsing)

### Missing Test Coverage to Add

1. **Enhanced Parser Tests**: Add comprehensive parser tests for all syntax variations and edge cases
2. **Timeout Process Management**: Add tests for graceful process termination scenarios
3. **Cross-Platform Tests**: Add platform-specific command tests
4. **Integration Tests**: Add more comprehensive action system integration tests
5. **Variable Substitution Edge Cases**: Test complex substitution scenarios
6. **Performance Considerations**: Add tests for various execution times and resource usage

### Implementation Approach

I will extend the existing test file with additional test modules to provide comprehensive coverage as specified in the issue, following the established testing patterns in the codebase and ensuring all test cases are deterministic and reliable.

The tests will be organized in modules matching the issue specification:
- Enhanced basic functionality tests
- Comprehensive parser tests 
- Expanded variable substitution tests
- Timeout and process management tests
- Additional error handling tests
- Extended security tests
- Integration tests
- Cross-platform tests
