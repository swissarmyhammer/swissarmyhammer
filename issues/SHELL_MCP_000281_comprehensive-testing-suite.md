# Comprehensive Testing Suite Implementation

Refer to /Users/wballard/github/sah-shell/ideas/shell.md

## Overview

Develop a comprehensive test suite for the shell MCP tool covering unit testing, integration testing, security testing, performance testing, and cross-platform compatibility.

## Objective

Create thorough test coverage that validates all aspects of the shell tool functionality, ensures reliability across different environments, and provides confidence for production deployment.

## Requirements

### Unit Testing Coverage
- Core shell execution engine tests
- Parameter validation and parsing tests  
- Output handling and formatting tests
- Error condition and edge case tests
- Configuration loading and validation tests

### Integration Testing
- MCP tool protocol integration tests
- CLI command integration tests
- Workflow system integration tests
- Configuration system integration tests
- Cross-component interaction tests

### Security Testing
- Command injection prevention tests
- Directory access control tests
- Input validation and sanitization tests
- Security policy enforcement tests
- Audit logging verification tests

### Performance and Resource Testing
- Timeout behavior and process cleanup tests
- Memory usage and resource leak tests
- Large output handling tests
- Concurrent execution tests
- Resource limit enforcement tests

### Cross-Platform Compatibility
- Unix/Linux platform tests
- Windows platform tests (if applicable)
- Process management across platforms
- Path handling and resolution tests
- Environment variable handling tests

## Implementation Details

### Unit Test Organization
```rust
// swissarmyhammer-tools/src/mcp/tools/shell/execute/tests.rs

#[cfg(test)]
mod unit_tests {
    use super::*;
    use swissarmyhammer::test_utils::IsolatedTestEnvironment;
    
    #[tokio::test]
    async fn test_basic_command_execution() {
        let _guard = IsolatedTestEnvironment::new();
        
        let result = execute_shell_command(
            "echo 'Hello, World!'".to_string(),
            None,
            300,
            None,
        ).await.unwrap();
        
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("Hello, World!"));
        assert!(result.stderr.is_empty());
    }
    
    #[tokio::test]
    async fn test_command_failure_handling() {
        let _guard = IsolatedTestEnvironment::new();
        
        let result = execute_shell_command(
            "exit 1".to_string(),
            None,
            300,
            None,
        ).await.unwrap();
        
        assert_eq!(result.exit_code, 1);
        assert!(result.stdout.is_empty());
    }
    
    #[tokio::test]
    async fn test_working_directory_support() {
        let _guard = IsolatedTestEnvironment::new();
        let temp_dir = tempfile::tempdir().unwrap();
        
        let result = execute_shell_command(
            "pwd".to_string(),
            Some(temp_dir.path().to_path_buf()),
            300,
            None,
        ).await.unwrap();
        
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains(temp_dir.path().to_str().unwrap()));
    }
    
    #[tokio::test]
    async fn test_timeout_enforcement() {
        let _guard = IsolatedTestEnvironment::new();
        
        let start = std::time::Instant::now();
        let result = execute_shell_command(
            "sleep 10".to_string(),
            None,
            2, // 2 second timeout
            None,
        ).await;
        
        assert!(start.elapsed().as_secs() < 5); // Should timeout well before 10 seconds
        assert!(result.is_err());
        // Verify timeout error type
    }
}
```

### Security Testing Suite
```rust
#[cfg(test)]
mod security_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_command_injection_prevention() {
        let _guard = IsolatedTestEnvironment::new();
        let validator = CommandValidator::new(SecurityPolicy::default());
        
        // Test various injection patterns
        let dangerous_commands = [
            "echo hello; rm -rf /",
            "echo hello && rm -rf /",
            "echo hello || rm -rf /",
            "echo hello | sh -c 'rm -rf /'",
            "echo hello && $(rm -rf /)",
            "echo hello && `rm -rf /`",
        ];
        
        for cmd in &dangerous_commands {
            let result = validator.validate_command(cmd, Path::new("/"));
            assert!(result.is_err(), "Should reject dangerous command: {}", cmd);
        }
    }
    
    #[tokio::test]
    async fn test_directory_access_controls() {
        let _guard = IsolatedTestEnvironment::new();
        let allowed_dirs = vec![PathBuf::from("/tmp"), PathBuf::from("/project")];
        let policy = SecurityPolicy {
            allowed_directories: Some(allowed_dirs),
            ..SecurityPolicy::default()
        };
        let validator = CommandValidator::new(policy);
        
        // Test allowed directory
        assert!(validator.validate_command("echo test", Path::new("/tmp")).is_ok());
        
        // Test disallowed directory
        assert!(validator.validate_command("echo test", Path::new("/etc")).is_err());
    }
    
    #[tokio::test]
    async fn test_blocked_command_patterns() {
        let _guard = IsolatedTestEnvironment::new();
        let policy = SecurityPolicy {
            blocked_commands: vec!["rm".to_string(), "format".to_string()],
            ..SecurityPolicy::default()
        };
        let validator = CommandValidator::new(policy);
        
        assert!(validator.validate_command("rm file.txt", Path::new("/tmp")).is_err());
        assert!(validator.validate_command("format c:", Path::new("/tmp")).is_err());
        assert!(validator.validate_command("ls -la", Path::new("/tmp")).is_ok());
    }
}
```

### Integration Testing Framework
```rust
// swissarmyhammer-tools/tests/shell_integration_tests.rs

use swissarmyhammer_tools::mcp::server::McpServer;
use swissarmyhammer::test_utils::IsolatedTestEnvironment;

#[tokio::test]
async fn test_mcp_shell_execute_integration() {
    let _guard = IsolatedTestEnvironment::new();
    let server = McpServer::new().await.unwrap();
    
    let request = json!({
        "method": "tools/call",
        "params": {
            "name": "shell_execute",
            "arguments": {
                "command": "echo 'MCP Integration Test'",
                "timeout": 30
            }
        }
    });
    
    let response = server.handle_request(request).await.unwrap();
    
    assert!(response["result"]["metadata"]["exit_code"] == 0);
    assert!(response["result"]["metadata"]["stdout"].as_str().unwrap().contains("MCP Integration Test"));
}

#[tokio::test]
async fn test_cli_shell_command_integration() {
    let _guard = IsolatedTestEnvironment::new();
    
    let output = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .args(["shell", "echo 'CLI Integration Test'"])
        .output()
        .unwrap();
    
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("CLI Integration Test"));
}
```

### Performance Testing Suite
```rust
#[cfg(test)]
mod performance_tests {
    use super::*;
    use criterion::*;
    
    #[tokio::test]
    async fn test_large_output_handling() {
        let _guard = IsolatedTestEnvironment::new();
        
        // Generate large output (1MB)
        let large_command = "head -c 1048576 /dev/zero | base64".to_string();
        
        let start = std::time::Instant::now();
        let result = execute_shell_command(
            large_command,
            None,
            300,
            None,
        ).await.unwrap();
        let duration = start.elapsed();
        
        assert_eq!(result.exit_code, 0);
        assert!(!result.stdout.is_empty());
        assert!(duration.as_secs() < 30); // Should complete within reasonable time
    }
    
    #[tokio::test]
    async fn test_concurrent_execution() {
        let _guard = IsolatedTestEnvironment::new();
        
        let futures: Vec<_> = (0..10).map(|i| {
            execute_shell_command(
                format!("echo 'Concurrent test {}'", i),
                None,
                300,
                None,
            )
        }).collect();
        
        let results = futures::future::join_all(futures).await;
        
        // All should succeed
        for (i, result) in results.into_iter().enumerate() {
            let result = result.unwrap();
            assert_eq!(result.exit_code, 0);
            assert!(result.stdout.contains(&format!("Concurrent test {}", i)));
        }
    }
}
```

### Cross-Platform Testing
```rust
#[cfg(test)]
mod cross_platform_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_path_handling() {
        let _guard = IsolatedTestEnvironment::new();
        
        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "test content").unwrap();
        
        #[cfg(unix)]
        let command = format!("cat '{}'", test_file.display());
        
        #[cfg(windows)]
        let command = format!("type \"{}\"", test_file.display());
        
        let result = execute_shell_command(
            command,
            None,
            300,
            None,
        ).await.unwrap();
        
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("test content"));
    }
    
    #[tokio::test]
    async fn test_environment_variables() {
        let _guard = IsolatedTestEnvironment::new();
        
        let mut env_vars = HashMap::new();
        env_vars.insert("TEST_VAR".to_string(), "test_value".to_string());
        
        #[cfg(unix)]
        let command = "echo $TEST_VAR".to_string();
        
        #[cfg(windows)]
        let command = "echo %TEST_VAR%".to_string();
        
        let result = execute_shell_command(
            command,
            None,
            300,
            Some(env_vars),
        ).await.unwrap();
        
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("test_value"));
    }
}
```

### Property-Based Testing
```rust
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;
    
    proptest! {
        #[test]
        fn test_command_validation_consistency(
            cmd in "[a-zA-Z0-9 ._-]+",
            timeout in 1u64..1800
        ) {
            let _guard = IsolatedTestEnvironment::new();
            let rt = tokio::runtime::Runtime::new().unwrap();
            
            rt.block_on(async {
                // Basic commands should not crash the validator
                let validator = CommandValidator::new(SecurityPolicy::default());
                let result = validator.validate_command(&cmd, Path::new("/tmp"));
                
                // Should either succeed or fail gracefully
                match result {
                    Ok(_) => {
                        // If validation passes, execution should not crash
                        // (though it may fail for other reasons)
                        let _exec_result = execute_shell_command(
                            cmd,
                            None,
                            timeout,
                            None,
                        ).await;
                    },
                    Err(_) => {
                        // Validation failure is acceptable
                    }
                }
            });
        }
    }
}
```

## Test Infrastructure

### Test Utilities and Helpers
```rust
// swissarmyhammer-tools/src/test_utils.rs

pub struct ShellTestUtils;

impl ShellTestUtils {
    pub fn create_test_config() -> ShellToolConfig {
        ShellToolConfig {
            security: ShellSecurityConfig {
                enable_validation: false, // Relaxed for testing
                ..Default::default()
            },
            execution: ShellExecutionConfig {
                default_timeout: 30, // Shorter for tests
                ..Default::default()
            },
            ..Default::default()
        }
    }
    
    pub fn create_secure_config() -> ShellToolConfig {
        ShellToolConfig {
            security: ShellSecurityConfig {
                enable_validation: true,
                blocked_commands: vec!["rm".to_string(), "format".to_string()],
                allowed_directories: Some(vec![PathBuf::from("/tmp")]),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}
```

### Continuous Integration Configuration
- Configure test execution in CI/CD pipelines
- Set up cross-platform test execution
- Configure security and performance test thresholds
- Enable test result reporting and analysis

## Acceptance Criteria

- [ ] Unit tests cover all core functionality
- [ ] Integration tests verify end-to-end functionality
- [ ] Security tests prevent common attack vectors
- [ ] Performance tests validate resource usage
- [ ] Cross-platform tests ensure compatibility
- [ ] Property-based tests verify edge cases
- [ ] Test coverage exceeds 90% for core functionality
- [ ] All tests pass consistently in CI/CD environment

## Testing Infrastructure Requirements

- [ ] Test isolation using `IsolatedTestEnvironment`
- [ ] Temporary directory and file management
- [ ] Mock services for external dependencies
- [ ] Performance benchmarking infrastructure
- [ ] Security test vector database
- [ ] Cross-platform test execution framework

## Notes

- Focus on realistic test scenarios that mirror actual usage
- Security testing is critical given the nature of shell execution
- Performance tests should prevent resource exhaustion issues
- Cross-platform testing ensures broad compatibility
- Property-based testing helps find edge cases and unexpected inputs
- Test infrastructure should be reusable for future enhancements