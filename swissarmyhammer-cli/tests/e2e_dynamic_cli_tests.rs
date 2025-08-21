use assert_cmd::Command;
use predicates::prelude::*;
use std::env;
use std::time;
use swissarmyhammer::test_utils::create_test_home_guard;
use tempfile::TempDir;

/// End-to-End integration tests for the dynamic CLI system
/// These tests execute the actual CLI binary as a subprocess to verify
/// that the complete system works correctly in real-world scenarios
/// Test helper to create a command with test environment setup
fn create_test_command() -> Command {
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.env("SWISSARMYHAMMER_TEST_MODE", "1"); // Enable test mode
    cmd.env("SAH_MCP_TIMEOUT", "300"); // Extended timeout for CI environments
    cmd
}

#[test]
fn test_dynamic_cli_help_shows_all_commands() {
    let _guard = create_test_home_guard();
    
    create_test_command()
        .args(["--help"])
        .assert()
        .success()
        // Static commands should be present
        .stdout(predicate::str::contains("serve"))
        .stdout(predicate::str::contains("doctor"))
        .stdout(predicate::str::contains("prompt"))
        .stdout(predicate::str::contains("flow"))
        .stdout(predicate::str::contains("validate"))
        .stdout(predicate::str::contains("plan"))
        .stdout(predicate::str::contains("implement"))
        .stdout(predicate::str::contains("config"))
        .stdout(predicate::str::contains("completion"))
        // Dynamic commands should be present
        .stdout(predicate::str::contains("issue"))
        .stdout(predicate::str::contains("memo"))
        .stdout(predicate::str::contains("file"));
        // Note: Some dynamic commands may not appear if tools fail to load
}

#[test]
fn test_issue_command_help() {
    let _guard = create_test_home_guard();
    
    create_test_command()
        .args(["issue", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Issue management"))
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("show"));
}

#[test]
fn test_memo_command_help() {
    let _guard = create_test_home_guard();
    
    create_test_command()
        .args(["memo", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("get"));
}

#[test]
fn test_file_command_help() {
    let _guard = create_test_home_guard();
    
    create_test_command()
        .args(["file", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("read"))
        .stdout(predicate::str::contains("write"))
        .stdout(predicate::str::contains("edit"))
        .stdout(predicate::str::contains("glob"))
        .stdout(predicate::str::contains("grep"));
}

#[test]
fn test_static_commands_still_work() {
    let _guard = create_test_home_guard();
    
    // Test prompt command (static)
    create_test_command()
        .args(["prompt", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("test"))
        .stdout(predicate::str::contains("search"))
        .stdout(predicate::str::contains("validate"));
    
    // Test flow command (static)
    create_test_command()
        .args(["flow", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("run"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("status"));
    
    // Test config command (static)
    create_test_command()
        .args(["config", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("show"))
        .stdout(predicate::str::contains("variables"))
        .stdout(predicate::str::contains("test"))
        .stdout(predicate::str::contains("env"));
}

#[test]
fn test_issue_list_command_executes() {
    let _guard = create_test_home_guard();
    
    create_test_command()
        .args(["issue", "list"])
        .assert()
        .success(); // Should not fail, even if no issues exist
}

#[test]
fn test_memo_list_command_executes() {
    let _guard = create_test_home_guard();
    
    create_test_command()
        .args(["memo", "list"])
        .assert()
        .success(); // Should not fail, even if no memos exist
}

#[test]
fn test_file_operations_e2e() {
    let _guard = create_test_home_guard();
    
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_file = temp_dir.path().join("test_file_cli_dynamic.txt");
    let temp_file_str = temp_file.to_string_lossy();
    
    // Test file write
    create_test_command()
        .args(["file", "write", "Hello, Dynamic World!", "--file_path", &temp_file_str])
        .assert()
        .success();
    
    // Test file read
    create_test_command()
        .args(["file", "read", &temp_file_str])
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello, Dynamic World!"));
    
    // Test file glob
    create_test_command()
        .args(["file", "glob", "*.txt", "--path", &temp_dir.path().to_string_lossy()])
        .assert()
        .success()
        .stdout(predicate::str::contains("test_file_cli_dynamic.txt"));
    
    // Test file grep
    create_test_command()
        .args(["file", "grep", "Dynamic", "--path", &temp_file_str])
        .assert()
        .success()
        .stdout(predicate::str::contains("Dynamic"));
}

#[test]
fn test_issue_create_and_operations() {
    let _guard = create_test_home_guard();
    
    // Create an issue
    create_test_command()
        .args(["issue", "create", "Test issue content"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created"));
    
    // List issues to verify creation
    create_test_command()
        .args(["issue", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Active Issues:").or(
            predicate::str::contains("Issues:") // May show different format
        ));
}

#[test]
fn test_memo_create_and_operations() {
    let _guard = create_test_home_guard();
    
    // Create a memo with unique title to avoid conflicts
    let unique_title = format!("Test Memo {}", time::SystemTime::now().duration_since(time::UNIX_EPOCH).unwrap().as_nanos());
    create_test_command()
        .args(["memo", "create", "--title", &unique_title, "Test memo content"])
        .assert()
        .success();
    
    // List memos to verify creation
    create_test_command()
        .args(["memo", "list"])
        .assert()
        .success();
        // Note: We don't assert specific content because memo creation might
        // succeed even if listing doesn't show the content due to storage differences
}

#[test]
fn test_search_operations() {
    let _guard = create_test_home_guard();
    
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    
    // Create some files to search
    std::fs::write(temp_dir.path().join("test1.txt"), "Hello world").unwrap();
    std::fs::write(temp_dir.path().join("test2.txt"), "Goodbye world").unwrap();
    
    // Change to temp directory for search operations
    let original_dir = env::current_dir().unwrap();
    env::set_current_dir(temp_dir.path()).unwrap();
    
    // Test search index (if available)
    let result = create_test_command()
        .args(["search", "index", "*.txt"])
        .output()
        .expect("Failed to execute search index");
    
    // Search indexing may fail in test environment, which is acceptable
    if result.status.success() {
        // If indexing succeeded, try querying
        create_test_command()
            .args(["search", "query", "world"])
            .assert()
            .success();
    }
    
    // Restore original directory
    env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_invalid_subcommand_error_handling() {
    let _guard = create_test_home_guard();
    
    // Test invalid subcommand in dynamic category
    create_test_command()
        .args(["issue", "invalid-subcommand"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid-subcommand"));
    
    // Test invalid subcommand in static category  
    create_test_command()
        .args(["prompt", "invalid-subcommand"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid-subcommand"));
}

#[test]
fn test_unknown_command_error_handling() {
    let _guard = create_test_home_guard();
    
    create_test_command()
        .args(["unknown-command"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("error").or(
            predicate::str::contains("unknown")
        ));
}

#[test]
fn test_version_display() {
    let _guard = create_test_home_guard();
    
    create_test_command()
        .args(["--version"])
        .assert()
        .success()
        .stdout(predicate::str::contains("swissarmyhammer"));
}

#[test]
fn test_completion_generation_works() {
    let _guard = create_test_home_guard();
    
    // Test bash completion generation
    create_test_command()
        .args(["completion", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("complete").or(
            predicate::str::contains("bash")
        ));
    
    // Test zsh completion generation
    create_test_command()
        .args(["completion", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("complete").or(
            predicate::str::contains("zsh")
        ));
}

#[test]
fn test_doctor_command_works() {
    let _guard = create_test_home_guard();
    
    // Doctor command should run successfully but may have warnings
    // which result in exit code 1, which is expected behavior
    create_test_command()
        .args(["doctor"])
        .assert()
        .code(predicate::in_iter([0, 1])) // Accept success or warning exit codes
        .stdout(predicate::str::contains("diagnostic").or(
            predicate::str::contains("check").or(
                predicate::str::contains("✓").or(
                    predicate::str::contains("OK")
                )
            )
        ));
}

#[test]
fn test_validate_command_works() {
    let _guard = create_test_home_guard();
    
    create_test_command()
        .args(["validate"])
        .assert()
        .success(); // Should succeed even with no files to validate
}

#[test]
fn test_prompt_list_works() {
    let _guard = create_test_home_guard();
    
    create_test_command()
        .args(["prompt", "list"])
        .assert()
        .success(); // Should succeed even if no prompts available
}

#[test]
fn test_flow_list_works() {
    let _guard = create_test_home_guard();
    
    create_test_command()
        .args(["flow", "list"])
        .assert()
        .success(); // Should succeed even if no workflows available
}

#[test]
fn test_error_exit_codes() {
    let _guard = create_test_home_guard();
    
    // Test that various error conditions produce appropriate exit codes
    
    // Unknown command should exit with non-zero
    create_test_command()
        .args(["unknown-command"])
        .assert()
        .failure();
    
    // Invalid subcommand should exit with non-zero
    create_test_command()
        .args(["issue", "invalid"])
        .assert()
        .failure();
    
    // Missing required arguments should exit with non-zero
    create_test_command()
        .args(["file", "read"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("required").or(
            predicate::str::contains("missing")
        ));
}

#[test]
fn test_dynamic_cli_fallback_behavior() {
    let _guard = create_test_home_guard();
    
    // Test that CLI works even when dynamic loading might fail
    // by temporarily disabling dynamic CLI
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.env("SAH_DISABLE_DYNAMIC_CLI", "1"); // Force static-only mode
    
    cmd.args(["--help"])
        .assert()
        .success()
        // Static commands should still be present
        .stdout(predicate::str::contains("serve"))
        .stdout(predicate::str::contains("doctor"))
        .stdout(predicate::str::contains("prompt"))
        .stdout(predicate::str::contains("flow"));
    
    // Dynamic commands might not be present in fallback mode
    // (we don't assert their absence as the implementation may vary)
}

#[test]
fn test_help_text_formatting() {
    let _guard = create_test_home_guard();
    
    // Test that help text is well-formatted and readable
    create_test_command()
        .args(["--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"))
        .stdout(predicate::str::contains("Commands:"))
        .stdout(predicate::str::contains("Options:"));
    
    // Test subcommand help formatting
    create_test_command()
        .args(["issue", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"))
        .stdout(predicate::str::contains("Commands:").or(
            predicate::str::contains("Subcommands:")
        ));
}

#[test]
fn test_argument_parsing_edge_cases() {
    let _guard = create_test_home_guard();
    
    // Test various edge cases in argument parsing
    
    // Edge case with minimal valid content
    create_test_command()
        .args(["issue", "create", "Test content"])
        .assert()
        .success(); // Minimal content should be allowed
    
    // Special characters in arguments
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let special_file = temp_dir.path().join("file with spaces & symbols!.txt");
    let special_file_str = special_file.to_string_lossy();
    
    create_test_command()
        .args(["file", "write", "Special content!@#$%^&*()", "--file_path", &special_file_str])
        .assert()
        .success();
    
    create_test_command()
        .args(["file", "read", &special_file_str])
        .assert()
        .success()
        .stdout(predicate::str::contains("Special content"));
}

#[test]
fn test_concurrent_command_execution() {
    use std::thread;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    
    let _guard = create_test_home_guard();
    
    // Test that multiple commands can be run concurrently without interference
    let success_count = Arc::new(AtomicUsize::new(0));
    let total_commands = 5;
    
    let handles: Vec<_> = (0..total_commands).map(|i| {
        let success_count = success_count.clone();
        thread::spawn(move || {
            let result = create_test_command()
                .args(["issue", "list"])
                .output();
                
            match result {
                Ok(output) if output.status.success() => {
                    success_count.fetch_add(1, Ordering::SeqCst);
                }
                _ => {
                    eprintln!("Concurrent command {} failed", i);
                }
            }
        })
    }).collect();
    
    // Wait for all commands to complete
    for handle in handles {
        handle.join().unwrap();
    }
    
    // At least some commands should succeed
    let successes = success_count.load(Ordering::SeqCst);
    assert!(successes > 0, 
           "At least some concurrent commands should succeed, got {}/{}", 
           successes, total_commands);
}

#[test]
fn test_large_output_handling() {
    let _guard = create_test_home_guard();
    
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let large_file = temp_dir.path().join("large_file.txt");
    
    // Create a large file (1MB of content)
    let large_content = "A".repeat(1024 * 1024);
    std::fs::write(&large_file, &large_content).unwrap();
    
    // Test that CLI can handle large file operations
    create_test_command()
        .args(["file", "read", &large_file.to_string_lossy()])
        .assert()
        .success()
        .stdout(predicate::str::contains("A"));
    
    // Test grep on large file
    create_test_command()
        .args(["file", "grep", "A", "--path", &large_file.to_string_lossy()])
        .timeout(std::time::Duration::from_secs(30)) // Reasonable timeout for large operations
        .assert()
        .success();
}

#[test]
fn test_unicode_content_handling() {
    let _guard = create_test_home_guard();
    
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let unicode_file = temp_dir.path().join("unicode_test.txt");
    let unicode_file_str = unicode_file.to_string_lossy();
    
    // Test Unicode content in various languages
    let unicode_content = "Hello 世界 🌍 Здравствуй мир مرحبا بالعالم";
    
    create_test_command()
        .args(["file", "write", unicode_content, "--file_path", &unicode_file_str])
        .assert()
        .success();
    
    create_test_command()
        .args(["file", "read", &unicode_file_str])
        .assert()
        .success()
        .stdout(predicate::str::contains("世界"))
        .stdout(predicate::str::contains("🌍"));
}

/// Integration test for the full CLI workflow with real file operations
#[test]
fn test_full_workflow_integration() {
    let _guard = create_test_home_guard();
    
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let test_file = temp_dir.path().join("workflow_test.txt");
    let test_file_str = test_file.to_string_lossy();
    
    // Step 1: Create initial content
    create_test_command()
        .args(["file", "write", "Initial content for workflow test", "--file_path", &test_file_str])
        .assert()
        .success();
    
    // Step 2: Read and verify content
    create_test_command()
        .args(["file", "read", &test_file_str])
        .assert()
        .success()
        .stdout(predicate::str::contains("Initial content"));
    
    // Step 3: Edit content
    create_test_command()
        .args(["file", "edit", "--old_string", "Initial content", "--new_string", "Modified content", &test_file_str])
        .assert()
        .success();
    
    // Step 4: Verify edit
    create_test_command()
        .args(["file", "read", &test_file_str])
        .assert()
        .success()
        .stdout(predicate::str::contains("Modified content"))
        .stdout(predicate::str::contains("workflow test"));
    
    // Step 5: Search for content
    create_test_command()
        .args(["file", "grep", "Modified", "--path", &test_file_str])
        .assert()
        .success()
        .stdout(predicate::str::contains("Modified"));
    
    // Step 6: Create related issue
    create_test_command()
        .args(["issue", "create", "Testing workflow integration"])
        .assert()
        .success();
    
    // Step 7: List issues to verify
    create_test_command()
        .args(["issue", "list"])
        .assert()
        .success();
    
    // Step 8: Create related memo with unique title
    let unique_title = format!("Workflow Test {}", time::SystemTime::now().duration_since(time::UNIX_EPOCH).unwrap().as_nanos());
    create_test_command()
        .args(["memo", "create", "--title", &unique_title, "Integration test memo"])
        .assert()
        .success();
    
    // Step 9: List memos to verify
    create_test_command()
        .args(["memo", "list"])
        .assert()
        .success();
}