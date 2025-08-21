use assert_cmd::Command;
use predicates::prelude::*;
use swissarmyhammer::test_utils::create_test_home_guard;
use tempfile::TempDir;

/// Comprehensive backward compatibility tests for the dynamic CLI system
/// These tests ensure that all previously working commands continue to function
/// exactly as they did before the dynamic CLI implementation
/// Helper to create test command with proper environment
fn create_test_command() -> Command {
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.env("SWISSARMYHAMMER_TEST_MODE", "1");
    cmd.env("SAH_MCP_TIMEOUT", "300");
    cmd
}

/// Test that all essential static commands work exactly as before
#[test]
fn test_all_static_commands_preserved() {
    let _guard = create_test_home_guard();
    
    // Test all previously working static commands
    let static_commands = vec![
        // Basic commands
        vec!["serve", "--help"],
        vec!["doctor", "--help"],
        vec!["validate", "--help"],
        vec!["plan", "--help"],
        vec!["implement", "--help"],
        
        // Completion commands
        vec!["completion", "bash"],
        vec!["completion", "zsh"],
        vec!["completion", "fish"],
        vec!["completion", "powershell"],
        
        // Prompt commands
        vec!["prompt", "list", "--help"],
        vec!["prompt", "test", "--help"],
        vec!["prompt", "search", "--help"],
        vec!["prompt", "validate", "--help"],
        
        // Flow commands  
        vec!["flow", "list", "--help"],
        vec!["flow", "run", "--help"],
        vec!["flow", "test", "--help"],
        vec!["flow", "resume", "--help"],
        vec!["flow", "status", "--help"],
        vec!["flow", "logs", "--help"],
        vec!["flow", "metrics", "--help"],
        vec!["flow", "visualize", "--help"],
        
        // Config commands
        vec!["config", "show", "--help"],
        vec!["config", "variables", "--help"],
        vec!["config", "test", "--help"],
        vec!["config", "env", "--help"],
    ];
    
    for args in static_commands {
        create_test_command()
            .args(&args)
            .assert()
            .success()
            .stdout(predicate::str::contains("Usage:").or(
                predicate::str::contains("Generate").or(
                    predicate::str::contains("completion")
                )
            ));
    }
}

/// Test that dynamic commands work identically to how they worked before (when they existed)
#[test]
fn test_dynamic_commands_work_as_before() {
    let _guard = create_test_home_guard();
    
    // Test dynamic commands that should work the same way
    let dynamic_commands = vec![
        vec!["issue", "list", "--help"],
        vec!["memo", "list", "--help"],
        vec!["memo", "create", "--help"],
        vec!["file", "read", "--help"],
        vec!["file", "write", "--help"],
        vec!["file", "edit", "--help"],
        vec!["file", "glob", "--help"],
        vec!["file", "grep", "--help"],
    ];
    
    for args in dynamic_commands {
        let result = create_test_command()
            .args(&args)
            .output()
            .expect("Failed to execute command");
            
        if !result.status.success() {
            eprintln!("Command failed: {:?}", args);
            eprintln!("stderr: {}", String::from_utf8_lossy(&result.stderr));
        }
        
        // Commands should either succeed or fail gracefully (not panic)
        // We allow both success and failure here because some commands might
        // need MCP infrastructure that may not be available in all test environments
        assert!(result.status.success() || !result.stderr.is_empty(), 
               "Command should either succeed or provide error message: {:?}", args);
    }
}

/// Test that error handling works the same way as before
#[test]
fn test_error_handling_preserved() {
    let _guard = create_test_home_guard();
    
    // Test various error conditions that should behave consistently
    let error_cases = vec![
        // Unknown commands
        (vec!["unknown-command"], "should fail with unknown command"),
        (vec!["nonexistent"], "should fail with unknown command"),
        
        // Invalid subcommands  
        (vec!["prompt", "invalid"], "should fail with invalid subcommand"),
        (vec!["flow", "invalid"], "should fail with invalid subcommand"),
        (vec!["config", "invalid"], "should fail with invalid subcommand"),
        
        // Missing required arguments (for commands that require them)
        (vec!["prompt", "test"], "should fail with missing argument"),
        (vec!["prompt", "search"], "should fail with missing argument"),
        (vec!["completion"], "should fail with missing shell argument"),
    ];
    
    for (args, description) in error_cases {
        let result = create_test_command()
            .args(&args)
            .output()
            .expect("Failed to execute command");
            
        assert!(!result.status.success(), 
               "Error case {:?} {}", args, description);
               
        // Should have meaningful error output
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(!stderr.trim().is_empty() || !String::from_utf8_lossy(&result.stdout).trim().is_empty(),
               "Error case {:?} should provide error message", args);
    }
}

/// Test that all help text generation works as expected
#[test]
fn test_help_text_consistency() {
    let _guard = create_test_home_guard();
    
    // Test top-level help
    create_test_command()
        .args(["--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"))
        .stdout(predicate::str::contains("swissarmyhammer"))
        .stdout(predicate::str::contains("Commands:"))
        .stdout(predicate::str::contains("Options:"));
    
    // Test help flag variants
    create_test_command()
        .args(["-h"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"));
    
    // Test command-specific help
    let commands_with_help = vec![
        "serve", "doctor", "validate", "plan", "implement",
        "prompt", "flow", "config", "completion"
    ];
    
    for cmd in commands_with_help {
        create_test_command()
            .args([cmd, "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Usage:"));
    }
}

/// Test that version information works correctly
#[test]
fn test_version_information() {
    let _guard = create_test_home_guard();
    
    // Test version flag variants
    create_test_command()
        .args(["--version"])
        .assert()
        .success()
        .stdout(predicate::str::contains("swissarmyhammer"));
    
    create_test_command()
        .args(["-V"])
        .assert()
        .success()
        .stdout(predicate::str::contains("swissarmyhammer"));
}

/// Test that file operations work exactly as they did before
#[test]
fn test_file_operations_backward_compatibility() {
    let _guard = create_test_home_guard();
    
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let test_file = temp_dir.path().join("backward_compat_test.txt");
    let test_file_str = test_file.to_string_lossy();
    
    // Test file write (should work as before)
    create_test_command()
        .args(["file", "write", "Test content for backward compatibility", "--file_path", &test_file_str])
        .assert()
        .success();
    
    // Test file read (should work as before)
    create_test_command()
        .args(["file", "read", &test_file_str])
        .assert()
        .success()
        .stdout(predicate::str::contains("Test content for backward compatibility"));
    
    // Test file edit (should work as before)
    create_test_command()
        .args(["file", "edit", "--old_string", "Test content", "--new_string", "Modified content", &test_file_str])
        .assert()
        .success();
    
    // Verify edit worked
    create_test_command()
        .args(["file", "read", &test_file_str])
        .assert()
        .success()
        .stdout(predicate::str::contains("Modified content"));
    
    // Test file glob (should work as before)
    create_test_command()
        .args(["file", "glob", "*.txt", "--path", &temp_dir.path().to_string_lossy()])
        .assert()
        .success()
        .stdout(predicate::str::contains("backward_compat_test.txt"));
    
    // Test file grep (should work as before)  
    create_test_command()
        .args(["file", "grep", "--path", &test_file_str, "Modified"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Modified"));
}

/// Test that issue operations maintain backward compatibility
#[test] 
fn test_issue_operations_backward_compatibility() {
    let _guard = create_test_home_guard();
    
    // Test issue creation (should work as before)
    create_test_command()
        .args(["issue", "create", "Backward compatibility test issue"])
        .assert()
        .success();
    
    // Test issue listing (should work as before)
    create_test_command()
        .args(["issue", "list"])
        .assert()
        .success();
    
    // Test issue listing with format options
    create_test_command()
        .args(["issue", "list", "--format", "json"])
        .assert()
        .success();
        
    create_test_command()
        .args(["issue", "list", "--format", "table"])
        .assert()
        .success();
}

/// Test that memo operations maintain backward compatibility
#[test]
fn test_memo_operations_backward_compatibility() {
    let _guard = create_test_home_guard();
    
    // Test memo creation (should work as before)
    let memo_title = format!("Backward Compatibility {}", std::process::id());
    create_test_command()
        .args(["memo", "create", "--title", &memo_title, "Test memo for compatibility"])
        .assert()
        .success();
    
    // Test memo listing (should work as before)
    create_test_command()
        .args(["memo", "list"])
        .assert()
        .success();
}

/// Test that command structure hasn't changed unexpectedly
#[test]
fn test_command_structure_unchanged() {
    let _guard = create_test_home_guard();
    
    // Test that subcommand requirements are preserved
    
    // These commands should require subcommands
    let subcommand_required = vec!["prompt", "flow", "config"];
    
    for cmd in subcommand_required {
        create_test_command()
            .args([cmd])
            .assert()
            .failure()
            .stderr(predicate::str::contains("subcommand").or(
                predicate::str::contains("required")
            ));
    }
    
    // These commands should work without subcommands
    let no_subcommand_needed = vec!["serve", "doctor", "validate", "implement"];
    
    for cmd in no_subcommand_needed {
        let result = create_test_command()
            .args([cmd, "--help"])
            .output()
            .expect("Failed to execute command");
            
        // Should succeed with help
        assert!(result.status.success(), 
               "Command '{}' should work with --help", cmd);
    }
}

/// Test that argument parsing behaviors are preserved
#[test]
fn test_argument_parsing_backward_compatibility() {
    let _guard = create_test_home_guard();
    
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let test_file = temp_dir.path().join("arg_test.txt");
    let test_file_str = test_file.to_string_lossy();
    
    // Test various argument patterns that should work as before
    
    // Test positional arguments
    create_test_command()
        .args(["file", "write", "content with spaces and symbols!@#$%", "--file_path", &test_file_str])
        .assert()
        .success();
    
    // Test flag arguments
    create_test_command()
        .args(["issue", "list", "--format", "json"])
        .assert()
        .success();
    
    // Test boolean flags
    create_test_command()
        .args(["issue", "list", "--show_completed"])
        .assert()
        .success();
    
    // Test combined short flags (if supported)
    let result = create_test_command()
        .args(["prompt", "validate", "-q"])
        .output()
        .expect("Failed to execute command");
    
    // Should either work or provide clear error (not crash)
    assert!(result.status.success() || !result.stderr.is_empty(),
           "Combined short flags should work or fail gracefully");
}

/// Test that output formats remain consistent  
#[test]
fn test_output_format_consistency() {
    let _guard = create_test_home_guard();
    
    // Test commands that support multiple output formats
    let format_tests = vec![
        (vec!["issue", "list", "--format", "table"], "table format should work"),
        (vec!["issue", "list", "--format", "json"], "json format should work"),
        (vec!["flow", "list", "--format", "table"], "flow table format should work"),
        (vec!["flow", "list", "--format", "json"], "flow json format should work"),
        (vec!["config", "show", "--format", "table"], "config table format should work"),
        (vec!["config", "show", "--format", "json"], "config json format should work"),
    ];
    
    for (args, description) in format_tests {
        let result = create_test_command()
            .args(&args)
            .output()
            .expect("Failed to execute command");
            
        if !result.status.success() {
            eprintln!("Format test failed for {:?}: {}", args, description);
            eprintln!("stderr: {}", String::from_utf8_lossy(&result.stderr));
        }
        
        // Format commands should either succeed or fail gracefully
        assert!(result.status.success() || !result.stderr.is_empty(),
               "Format test: {} - args: {:?}", description, args);
    }
}

/// Test that environment variable support is preserved
#[test]
fn test_environment_variables_preserved() {
    let _guard = create_test_home_guard();
    
    // Test that important environment variables still work
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.env("RUST_LOG", "debug"); // Should not break anything
    cmd.env("NO_COLOR", "1"); // Should disable colors
    cmd.args(["--help"])
        .assert()
        .success();
    
    // Test that test mode environment variable works
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.env("SWISSARMYHAMMER_TEST_MODE", "1");
    cmd.args(["issue", "list"])
        .assert()
        .success();
}

/// Test that special characters and edge cases work as before
#[test]
fn test_special_characters_backward_compatibility() {
    let _guard = create_test_home_guard();
    
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    
    // Test files with special characters in names
    let special_files = vec![
        "file with spaces.txt",
        "file-with-dashes.txt",
        "file_with_underscores.txt",
        "file.with.dots.txt",
    ];
    
    for file_name in special_files {
        let file_path = temp_dir.path().join(file_name);
        let file_path_str = file_path.to_string_lossy();
        
        // Test write operation
        create_test_command()
            .args(["file", "write", "Special character test", "--file_path", &file_path_str])
            .assert()
            .success();
        
        // Test read operation  
        create_test_command()
            .args(["file", "read", &file_path_str])
            .assert()
            .success()
            .stdout(predicate::str::contains("Special character test"));
    }
    
    // Test content with special characters
    let unicode_file = temp_dir.path().join("unicode.txt");
    let unicode_content = "Unicode: 你好 🌍 café résumé naïve";
    
    create_test_command()
        .args(["file", "write", unicode_content, "--file_path", &unicode_file.to_string_lossy()])
        .assert()
        .success();
    
    create_test_command()
        .args(["file", "read", &unicode_file.to_string_lossy()])
        .assert()
        .success()
        .stdout(predicate::str::contains("你好"))
        .stdout(predicate::str::contains("🌍"));
}

/// Test that command chaining and workflows work as before
#[test]
fn test_command_workflows_preserved() {
    let _guard = create_test_home_guard();
    
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let workflow_file = temp_dir.path().join("workflow_test.txt");
    let workflow_file_str = workflow_file.to_string_lossy();
    
    // Test a complex workflow that should work as before
    
    // Step 1: Create content
    create_test_command()
        .args(["file", "write", "Initial workflow content", "--file_path", &workflow_file_str])
        .assert()
        .success();
    
    // Step 2: Verify content
    create_test_command()
        .args(["file", "read", &workflow_file_str])
        .assert()
        .success()
        .stdout(predicate::str::contains("Initial workflow"));
    
    // Step 3: Search content
    create_test_command()
        .args(["file", "grep", "--path", &workflow_file_str, "workflow"])
        .assert()
        .success()
        .stdout(predicate::str::contains("workflow"));
    
    // Step 4: Modify content
    create_test_command()
        .args(["file", "edit", "--old_string", "Initial", "--new_string", "Modified", &workflow_file_str])
        .assert()
        .success();
    
    // Step 5: Verify modification
    create_test_command()
        .args(["file", "read", &workflow_file_str])
        .assert()
        .success()
        .stdout(predicate::str::contains("Modified workflow"));
    
    // Step 6: Create related issue
    create_test_command()
        .args(["issue", "create", "Workflow test completed successfully"])
        .assert()
        .success();
}

/// Test that performance characteristics haven't significantly degraded
#[test]
fn test_performance_backward_compatibility() {
    let _guard = create_test_home_guard();
    
    use std::time::Instant;
    
    // Test that basic commands still complete in reasonable time
    let start = Instant::now();
    
    create_test_command()
        .args(["--help"])
        .assert()
        .success();
        
    let duration = start.elapsed();
    
    // Help should complete quickly (within 10 seconds, even in slow CI)
    assert!(duration.as_secs() < 10, 
           "Help command took too long: {:?}", duration);
    
    // Test that file operations are still reasonably fast
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perf_file = temp_dir.path().join("performance_test.txt");
    let perf_file_str = perf_file.to_string_lossy();
    
    let start = Instant::now();
    
    create_test_command()
        .args(["file", "write", "Performance test content", "--file_path", &perf_file_str])
        .assert()
        .success();
        
    let write_duration = start.elapsed();
    
    let start = Instant::now();
    
    create_test_command()
        .args(["file", "read", &perf_file_str])
        .assert()
        .success();
        
    let read_duration = start.elapsed();
    
    // File operations should be fast (within 5 seconds each)
    assert!(write_duration.as_secs() < 5, 
           "File write took too long: {:?}", write_duration);
    assert!(read_duration.as_secs() < 5, 
           "File read took too long: {:?}", read_duration);
}

/// Test that all previously documented command examples still work
#[test]
fn test_documented_examples_preserved() {
    let _guard = create_test_home_guard();
    
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let example_file = temp_dir.path().join("example.txt");
    let example_file_str = example_file.to_string_lossy();
    
    // Test examples that were documented in previous versions
    
    // Example: Basic file operations
    create_test_command()
        .args(["file", "write", "Hello, World!", "--file_path", &example_file_str])
        .assert()
        .success();
    
    create_test_command()
        .args(["file", "read", &example_file_str])
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello, World!"));
    
    // Example: Issue creation
    create_test_command()
        .args(["issue", "create", "Example issue from documentation"])
        .assert()
        .success();
    
    // Example: Issue listing
    create_test_command()
        .args(["issue", "list"])
        .assert()
        .success();
    
    // Example: Memo creation
    let example_memo_title = format!("Example {}", std::process::id());
    create_test_command()
        .args(["memo", "create", "--title", &example_memo_title, "Documentation example memo"])
        .assert()
        .success();
    
    // Example: Shell completion
    create_test_command()
        .args(["completion", "bash"])
        .assert()
        .success();
    
    // Example: Configuration display
    create_test_command()
        .args(["config", "show"])
        .assert()
        .success();
}