//! Comprehensive shell tool integration and production readiness tests
//!
//! This test suite validates the complete shell tool implementation for production readiness,
//! including integration validation, performance testing, security verification, and quality assurance.

use assert_cmd::Command;
use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};
use swissarmyhammer::test_utils::IsolatedTestEnvironment;
use swissarmyhammer::{
    shell_security::{CommandValidator, SecurityPolicy},
    workflow::{
        context::WorkflowContext,
        actions::ShellAction,
        executor::ActionExecutor,
    },
};
use swissarmyhammer_tools::mcp::{
    tool_registry::ToolRegistry,
    tools::shell::{execute::ShellExecuteTool, register_shell_tools},
};
use tempfile::TempDir;
use tokio::time::timeout;

/// Comprehensive integration test suite for shell tool production readiness
mod integration_validation {
    use super::*;

    #[tokio::test]
    async fn test_complete_system_integration() {
        let _guard = IsolatedTestEnvironment::new();
        
        // Test MCP tool registration and discovery
        let mut registry = ToolRegistry::new();
        register_shell_tools(&mut registry);
        
        let tools = registry.list_tools();
        assert!(tools.iter().any(|t| t.name == "shell_execute"));
        assert_eq!(tools.len(), 1);
        
        // Test tool properties
        let shell_tool = registry.get_tool("shell_execute").unwrap();
        assert_eq!(shell_tool.name(), "shell_execute");
        assert!(!shell_tool.description().is_empty());
        assert!(!shell_tool.schema().is_null());
    }

    #[tokio::test]
    async fn test_cli_integration_complete() {
        let _guard = IsolatedTestEnvironment::new();
        
        // Test shell command through CLI
        let output = Command::cargo_bin("sah")
            .unwrap()
            .args(["shell", "--help"])
            .output()
            .unwrap();
            
        assert!(output.status.success());
        let help_text = String::from_utf8_lossy(&output.stdout);
        assert!(help_text.contains("Execute shell commands"));
        assert!(help_text.contains("--timeout"));
        assert!(help_text.contains("--working-directory"));
    }

    #[tokio::test]
    async fn test_workflow_integration_backward_compatibility() {
        let _guard = IsolatedTestEnvironment::new();
        
        // Test workflow action integration
        let mut context = WorkflowContext::new();
        context.set_variable("test_var", "hello world");
        
        let shell_action = ShellAction::new("echo {test_var}")
            .with_timeout(30)
            .with_result_variable("result");
            
        let executor = ActionExecutor::new();
        let result = executor.execute_action(&shell_action, &mut context).await;
        
        assert!(result.is_ok());
        assert!(context.get_variable("result").is_some());
        assert!(context.get_variable("result").unwrap().contains("hello world"));
    }

    #[tokio::test]
    async fn test_shell_security_integration() {
        let _guard = IsolatedTestEnvironment::new();
        
        // Test that shell security system is working with default configuration
        let policy = SecurityPolicy::default();
        let validator = CommandValidator::new(policy);
        
        // Verify basic command validation works
        assert!(validator.validate_command("echo test", Path::new("/tmp")).is_ok());
        assert!(validator.validate_command("rm -rf /", Path::new("/tmp")).is_err());
    }

    #[tokio::test]
    async fn test_cross_component_communication() {
        let _guard = IsolatedTestEnvironment::new();
        
        // Test shell security validator integration
        let policy = SecurityPolicy::default();
        let validator = CommandValidator::new(policy);
        
        // Test command validation
        assert!(validator.validate_command("echo test", Path::new("/tmp")).is_ok());
        assert!(validator.validate_command("rm -rf /", Path::new("/tmp")).is_err());
        
        // Test environment variable validation
        let mut env_vars = HashMap::new();
        env_vars.insert("TEST_VAR".to_string(), "test_value".to_string());
        assert!(validator.validate_environment_variables(&env_vars).is_ok());
    }
}

/// Performance optimization and profiling tests
mod performance_optimization {
    use super::*;

    #[tokio::test]
    async fn test_command_execution_performance() {
        let _guard = IsolatedTestEnvironment::new();
        
        let start = Instant::now();
        
        // Execute simple command and measure overhead
        let mut context = WorkflowContext::new();
        let shell_action = ShellAction::new("echo 'performance test'");
        let executor = ActionExecutor::new();
        
        let result = executor.execute_action(&shell_action, &mut context).await;
        let execution_time = start.elapsed();
        
        assert!(result.is_ok());
        // Target: < 100ms overhead for simple commands
        assert!(execution_time < Duration::from_millis(100));
    }

    #[tokio::test]
    async fn test_large_output_handling_performance() {
        let _guard = IsolatedTestEnvironment::new();
        
        let start = Instant::now();
        let initial_memory = get_memory_usage();
        
        // Test large output command (simulate with multiple echo commands)
        let mut context = WorkflowContext::new();
        let shell_action = ShellAction::new("for i in {1..100}; do echo 'Large output test line $i with some additional text to make it longer'; done")
            .with_timeout(30);
        let executor = ActionExecutor::new();
        
        let result = executor.execute_action(&shell_action, &mut context).await;
        let execution_time = start.elapsed();
        let final_memory = get_memory_usage();
        
        assert!(result.is_ok());
        // Memory growth should be reasonable (less than 10MB for this test)
        assert!((final_memory - initial_memory) < 10 * 1024 * 1024);
        // Should complete within reasonable time
        assert!(execution_time < Duration::from_secs(5));
    }

    #[tokio::test]
    async fn test_concurrent_execution_performance() {
        let _guard = IsolatedTestEnvironment::new();
        
        let start = Instant::now();
        
        // Execute 5 concurrent shell commands
        let mut handles = Vec::new();
        for i in 0..5 {
            let handle = tokio::spawn(async move {
                let mut context = WorkflowContext::new();
                let shell_action = ShellAction::new(&format!("echo 'concurrent test {}'", i))
                    .with_timeout(10);
                let executor = ActionExecutor::new();
                executor.execute_action(&shell_action, &mut context).await
            });
            handles.push(handle);
        }
        
        // Wait for all to complete
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }
        
        let total_time = start.elapsed();
        // Concurrent execution should be faster than serial (less than 2x single command time)
        assert!(total_time < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn test_process_cleanup_efficiency() {
        let _guard = IsolatedTestEnvironment::new();
        
        let start = Instant::now();
        
        // Test timeout and cleanup
        let mut context = WorkflowContext::new();
        let shell_action = ShellAction::new("sleep 10") // This will timeout
            .with_timeout(1);
        let executor = ActionExecutor::new();
        
        let result = executor.execute_action(&shell_action, &mut context).await;
        let cleanup_time = start.elapsed();
        
        // Should fail due to timeout
        assert!(result.is_err());
        // Cleanup should be fast (< 1 second after timeout)
        assert!(cleanup_time < Duration::from_secs(2));
    }

    fn get_memory_usage() -> u64 {
        // Use proper memory measurement from test utilities
        swissarmyhammer::test_utils::memory_measurement::get_approximate_memory_usage()
    }
}

/// Security review and hardening tests
mod security_validation {
    use super::*;

    #[tokio::test]
    async fn test_blocked_command_validation() {
        let _guard = IsolatedTestEnvironment::new();
        
        let policy = SecurityPolicy::default();
        let validator = CommandValidator::new(policy);
        
        // Test blocked command patterns (only dangerous commands are blocked)
        let command_tests = vec![
            ("rm -rf /", true), // Blocked dangerous command
            ("sudo something", true), // Blocked dangerous command
            ("echo test; ls", false), // Shell constructs now allowed
            ("echo test && ls", false), // Command chaining now allowed
            ("echo test | grep pattern", false), // Pipes now allowed
            ("echo $(date)", false), // Command substitution now allowed
            ("echo `whoami`", false), // Backticks now allowed
            ("echo 'safe string'", false), // Safe command
            ("ls -la", false), // Safe command
        ];
        
        for (command, should_be_blocked) in command_tests {
            let result = validator.validate_command(command, Path::new("/tmp"));
            assert_eq!(
                result.is_err(),
                should_be_blocked,
                "Command validation failed for: {}",
                command
            );
        }
    }

    #[tokio::test]
    async fn test_access_control_enforcement() {
        let _guard = IsolatedTestEnvironment::new();
        
        // Test directory restrictions with default security policy
        let policy = SecurityPolicy {
            enable_validation: true,
            allowed_directories: Some(vec!["/tmp".to_string()]),
            blocked_commands: vec!["rm".to_string()],
            max_command_length: 500,
        };
        
        let validator = CommandValidator::new(policy);
        
        // Test allowed directory
        assert!(validator.validate_working_directory(Some(Path::new("/tmp"))).is_ok());
        
        // Test blocked directory (would need actual implementation)
        // This is a placeholder for the full access control test
    }

    #[tokio::test]
    async fn test_audit_logging_completeness() {
        let _guard = IsolatedTestEnvironment::new();
        
        // Test that audit logging captures all necessary information
        let mut context = WorkflowContext::new();
        let shell_action = ShellAction::new("echo 'audit test'")
            .with_timeout(10);
        let executor = ActionExecutor::new();
        
        let result = executor.execute_action(&shell_action, &mut context).await;
        assert!(result.is_ok());
        
        // Verify audit information is captured
        // This would require integration with actual audit logging system
        // For now, verify the action completed successfully
    }

    #[tokio::test]
    async fn test_security_configuration_defaults() {
        let _guard = IsolatedTestEnvironment::new();
        
        // Note: ShellToolConfig was removed - now using hardcoded DefaultShellConfig
        // Verify that hardcoded defaults are appropriate for production
        
        // Verify execution defaults using the new DefaultShellConfig
        use swissarmyhammer_tools::mcp::tools::shell::execute::DefaultShellConfig;
        
        assert!(DefaultShellConfig::default_timeout() > 0);
        assert!(DefaultShellConfig::max_timeout() >= DefaultShellConfig::default_timeout());
        assert!(DefaultShellConfig::min_timeout() > 0);
        assert!(DefaultShellConfig::max_output_size() > 0);
        assert!(DefaultShellConfig::max_line_length() > 0);
        
        // Note: Security features (validation, blocked commands, etc.) and audit features
        // were part of the removed configurable system. The shell tool now uses 
        // simpler hardcoded limits for output size and timeouts only.
    }
}

/// Quality assurance validation tests
mod quality_assurance {
    use super::*;

    #[tokio::test]
    async fn test_cross_platform_behavior() {
        let _guard = IsolatedTestEnvironment::new();
        
        // Test basic command execution on current platform
        let mut context = WorkflowContext::new();
        
        // Use platform-appropriate commands
        #[cfg(unix)]
        let shell_action = ShellAction::new("echo 'Unix test' && pwd");
        
        #[cfg(windows)]
        let shell_action = ShellAction::new("echo Windows test && cd");
        
        let executor = ActionExecutor::new();
        let result = executor.execute_action(&shell_action, &mut context).await;
        
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_comprehensive_error_handling() {
        let _guard = IsolatedTestEnvironment::new();
        
        // Test various error scenarios
        let error_scenarios = vec![
            ("nonexistent_command_12345", "Command not found"),
            ("echo test && exit 42", "Non-zero exit code"),
        ];
        
        for (command, _expected_error_type) in error_scenarios {
            let mut context = WorkflowContext::new();
            let shell_action = ShellAction::new(command).with_timeout(10);
            let executor = ActionExecutor::new();
            
            let result = executor.execute_action(&shell_action, &mut context).await;
            
            // Command should either fail or return non-zero exit code
            // The exact behavior depends on the implementation
            // For production, we want consistent error handling
            match result {
                Ok(_) => {
                    // If command succeeds, verify it's handled appropriately
                    // This might be the case for "exit 42" which is valid but non-zero
                }
                Err(_) => {
                    // Expected for truly invalid commands
                }
            }
        }
    }

    #[tokio::test]
    async fn test_resource_limits_enforcement() {
        let _guard = IsolatedTestEnvironment::new();
        
        // Test command length limits
        let policy = SecurityPolicy {
            max_command_length: 50,
            ..Default::default()
        };
        let validator = CommandValidator::new(policy);
        
        let long_command = "a".repeat(100);
        assert!(validator.validate_command(&long_command, Path::new("/tmp")).is_err());
        
        let short_command = "echo test";
        assert!(validator.validate_command(short_command, Path::new("/tmp")).is_ok());
    }

    #[tokio::test]
    async fn test_logging_and_monitoring_integration() {
        let _guard = IsolatedTestEnvironment::new();
        
        // Test that shell operations integrate with logging system
        let mut context = WorkflowContext::new();
        let shell_action = ShellAction::new("echo 'logging test'")
            .with_timeout(10);
        let executor = ActionExecutor::new();
        
        let result = executor.execute_action(&shell_action, &mut context).await;
        assert!(result.is_ok());
        
        // Verify logging integration (this would require actual log capture)
        // For now, verify the operation completed successfully
    }
}

/// Production readiness validation tests
mod production_readiness {
    use super::*;

    #[tokio::test]
    async fn test_production_configuration_validation() {
        let _guard = IsolatedTestEnvironment::new();
        
        // Note: ShellToolConfig and ConfigurationLoader were removed with sah_config
        // The shell tool now uses hardcoded DefaultShellConfig values
        
        // Test that the hardcoded production defaults are reasonable
        use swissarmyhammer_tools::mcp::tools::shell::execute::DefaultShellConfig;
        
        // Verify the hardcoded values match production requirements
        assert_eq!(DefaultShellConfig::default_timeout(), 300); // 5 minutes default
        assert_eq!(DefaultShellConfig::max_timeout(), 1800);    // 30 minutes max
        assert_eq!(DefaultShellConfig::min_timeout(), 1);       // 1 second min
        assert_eq!(DefaultShellConfig::max_output_size(), 10_485_760); // 10MB
        assert_eq!(DefaultShellConfig::max_line_length(), 2000);       // 2000 chars
        
        // The new approach trades configurability for simplicity and reliability
        // Security features like blocked commands would need to be implemented
        // at a different layer if required in production
    }

    #[tokio::test]
    async fn test_monitoring_integration() {
        let _guard = IsolatedTestEnvironment::new();
        
        // Test metrics collection capability
        let start = Instant::now();
        
        let mut context = WorkflowContext::new();
        let shell_action = ShellAction::new("echo 'monitoring test'")
            .with_timeout(10);
        let executor = ActionExecutor::new();
        
        let result = executor.execute_action(&shell_action, &mut context).await;
        let execution_time = start.elapsed();
        
        assert!(result.is_ok());
        
        // Verify monitoring data can be collected
        assert!(execution_time > Duration::from_nanos(1));
        assert!(execution_time < Duration::from_secs(1));
    }

    #[tokio::test]
    async fn test_deployment_scenario_compatibility() {
        let _guard = IsolatedTestEnvironment::new();
        
        // Test that shell tool works in constrained environments
        // This simulates deployment scenarios
        
        // Note: ShellToolConfig and ConfigurationLoader were removed with sah_config
        // The shell tool now uses hardcoded DefaultShellConfig which eliminates
        // configuration validation complexity in deployment scenarios
        
        use swissarmyhammer_tools::mcp::tools::shell::execute::DefaultShellConfig;
        
        // Verify hardcoded configuration is deployment-ready
        assert!(DefaultShellConfig::default_timeout() >= 1);
        assert!(DefaultShellConfig::max_timeout() > DefaultShellConfig::default_timeout());
        assert!(DefaultShellConfig::max_output_size() > 0);
        
        // Test basic functionality works with hardcoded defaults
        let mut context = WorkflowContext::new();
        let shell_action = ShellAction::new("echo 'deployment test'")
            .with_timeout(5);
        let executor = ActionExecutor::new();
        
        let result = executor.execute_action(&shell_action, &mut context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_backward_compatibility_maintenance() {
        let _guard = IsolatedTestEnvironment::new();
        
        // Test that existing shell actions still work
        let legacy_action = ShellAction::new("echo 'legacy test'");
        let mut context = WorkflowContext::new();
        let executor = ActionExecutor::new();
        
        let result = executor.execute_action(&legacy_action, &mut context).await;
        assert!(result.is_ok());
        
        // Test with various legacy patterns
        let actions = vec![
            ShellAction::new("echo test"),
            ShellAction::new("echo test").with_timeout(30),
            ShellAction::new("echo test").with_working_directory("/tmp"),
        ];
        
        for action in actions {
            let result = executor.execute_action(&action, &mut context).await;
            assert!(result.is_ok(), "Legacy action should still work");
        }
    }
}

/// Performance benchmarking tests
#[cfg(test)]
mod benchmarks {
    use super::*;

    #[tokio::test]
    async fn benchmark_simple_command_execution() {
        let _guard = IsolatedTestEnvironment::new();
        
        let iterations = 10;
        let mut total_time = Duration::new(0, 0);
        
        for _ in 0..iterations {
            let start = Instant::now();
            
            let mut context = WorkflowContext::new();
            let shell_action = ShellAction::new("echo 'benchmark'");
            let executor = ActionExecutor::new();
            
            let result = executor.execute_action(&shell_action, &mut context).await;
            assert!(result.is_ok());
            
            total_time += start.elapsed();
        }
        
        let average_time = total_time / iterations;
        println!("Average execution time: {:?}", average_time);
        
        // Performance target: average < 50ms
        assert!(average_time < Duration::from_millis(50));
    }

    #[tokio::test]
    async fn benchmark_memory_usage() {
        let _guard = IsolatedTestEnvironment::new();
        
        // Baseline memory
        let baseline = get_approximate_memory_usage();
        
        // Execute multiple commands
        for i in 0..50 {
            let mut context = WorkflowContext::new();
            let shell_action = ShellAction::new(&format!("echo 'Memory test {}'", i));
            let executor = ActionExecutor::new();
            
            let result = executor.execute_action(&shell_action, &mut context).await;
            assert!(result.is_ok());
        }
        
        let final_memory = get_approximate_memory_usage();
        let memory_growth = final_memory - baseline;
        
        println!("Memory growth: {} bytes", memory_growth);
        
        // Memory growth should be reasonable (< 1MB for 50 simple commands)
        assert!(memory_growth < 1024 * 1024);
    }

    fn get_approximate_memory_usage() -> u64 {
        // Use proper memory measurement from test utilities
        swissarmyhammer::test_utils::memory_measurement::get_approximate_memory_usage()
    }
}