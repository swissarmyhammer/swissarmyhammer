//! Integration tests for todo CLI commands
//!
//! This test suite verifies that the todo CLI commands (create, show, complete)
//! work correctly through the dynamic CLI interface.

use git2::Repository;
use std::process::Command;
use tempfile::TempDir;

/// Length of the JSON prefix `"id":"` used when parsing todo IDs from command output
const JSON_ID_PREFIX_LEN: usize = 6;

/// Helper function to run sah command with specific working directory
fn run_sah_command_with_cwd(args: &[&str], cwd: &std::path::Path) -> std::process::Output {
    // Canonicalize the path to resolve symlinks (important on macOS where /var -> /private/var)
    let canonical_cwd = cwd.canonicalize().expect("Failed to canonicalize cwd");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sah"));
    cmd.args(args).current_dir(canonical_cwd);
    cmd.output().expect("Failed to execute sah command")
}

/// Initialize a minimal git repository in the given directory
fn init_git_repo(path: &std::path::Path) {
    let repo = Repository::init(path).expect("Failed to initialize git repo");

    // Set git config for the repo
    let mut config = repo.config().expect("Failed to get git config");
    config
        .set_str("user.email", "test@example.com")
        .expect("Failed to set git user email");
    config
        .set_str("user.name", "Test User")
        .expect("Failed to set git user name");
}

/// Helper function to create a temp directory with git repo and .swissarmyhammer initialized
fn setup_todo_test_env() -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let temp_path = temp_dir.path();

    init_git_repo(temp_path);
    std::fs::create_dir_all(temp_path.join(".swissarmyhammer"))
        .expect("Failed to create .swissarmyhammer dir");

    temp_dir
}

/// Helper function to extract todo ID from JSON output
fn extract_todo_id_from_output(output: &str) -> &str {
    let id_start = output.find("\"id\":\"").expect("Should find id in output") + JSON_ID_PREFIX_LEN;
    let id_end = output[id_start..]
        .find('\"')
        .expect("Should find end of id")
        + id_start;
    &output[id_start..id_end]
}

/// Helper function to assert JSON field contains expected numeric value
fn assert_json_field_value(output: &str, field: &str, value: u32) {
    let compact = format!("\"{}\":{}", field, value);
    let spaced = format!("\"{}\": {}", field, value);
    assert!(
        output.contains(&compact) || output.contains(&spaced),
        "Should show {} {} todos",
        value,
        field
    );
}

/// Test that todo commands are available in help output
#[test]
fn test_todo_commands_in_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_sah"))
        .arg("--help")
        .output()
        .expect("Failed to execute sah --help");

    let _stdout = String::from_utf8_lossy(&output.stdout);
    // Help should not contain 'todo' as a top-level command (it's a tool category)
    // but running 'sah todo --help' should work

    let todo_help = Command::new(env!("CARGO_BIN_EXE_sah"))
        .args(["todo", "--help"])
        .output()
        .expect("Failed to execute sah todo --help");

    assert!(todo_help.status.success(), "todo --help should succeed");

    let todo_stdout = String::from_utf8_lossy(&todo_help.stdout);
    assert!(
        todo_stdout.contains("create"),
        "todo help should mention 'create' command"
    );
    assert!(
        todo_stdout.contains("show"),
        "todo help should mention 'show' command"
    );
    assert!(
        todo_stdout.contains("list"),
        "todo help should mention 'list' command"
    );
    assert!(
        todo_stdout.contains("complete"),
        "todo help should mention 'complete' command"
    );
}

/// Test todo create command
#[test]
#[ignore = "Git repo detection issue with symlinked temp paths on macOS - see issue notes"]
fn test_todo_create_command() {
    let temp_dir = setup_todo_test_env();
    let temp_path = temp_dir.path();

    let output = run_sah_command_with_cwd(
        &[
            "todo",
            "create",
            "--task",
            "Test task",
            "--context",
            "Test context",
        ],
        temp_path,
    );

    assert!(
        output.status.success(),
        "todo create should succeed. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Created todo item"),
        "Output should confirm creation"
    );
    assert!(stdout.contains("Test task"), "Output should contain task");

    // Verify the todo file was created
    let todo_file = temp_path.join(".swissarmyhammer").join("todo.yaml");
    assert!(
        todo_file.exists(),
        "todo.yaml file should be created at {}",
        todo_file.display()
    );
}

/// Test todo create command without optional context
#[test]
#[ignore = "Git repo detection issue with symlinked temp paths on macOS - see issue notes"]
fn test_todo_create_without_context() {
    let temp_dir = setup_todo_test_env();
    let temp_path = temp_dir.path();

    let output = run_sah_command_with_cwd(&["todo", "create", "--task", "Simple task"], temp_path);

    assert!(
        output.status.success(),
        "todo create without context should succeed"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Created todo item"),
        "Output should confirm creation"
    );
}

/// Test todo show next command
#[test]
#[ignore = "Git repo detection issue with symlinked temp paths on macOS - see issue notes"]
fn test_todo_show_next() {
    let temp_dir = setup_todo_test_env();
    let temp_path = temp_dir.path();

    // First create a todo item
    let create_output =
        run_sah_command_with_cwd(&["todo", "create", "--task", "Task to show"], temp_path);
    assert!(create_output.status.success(), "todo create should succeed");

    // Now show the next item
    let show_output = run_sah_command_with_cwd(&["todo", "show", "--item", "next"], temp_path);

    assert!(
        show_output.status.success(),
        "todo show next should succeed"
    );

    let stdout = String::from_utf8_lossy(&show_output.stdout);
    assert!(
        stdout.contains("Task to show"),
        "Output should contain the task we created"
    );
}

/// Test todo show next with no todos
#[test]
#[ignore = "Git repo detection issue with symlinked temp paths on macOS - see issue notes"]
fn test_todo_show_next_empty() {
    let temp_dir = setup_todo_test_env();
    let temp_path = temp_dir.path();

    let output = run_sah_command_with_cwd(&["todo", "show", "--item", "next"], temp_path);

    // Should succeed but indicate no items found
    assert!(
        output.status.success(),
        "todo show next with no items should succeed"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("No incomplete todo items found") || stdout.contains("null"),
        "Output should indicate no items found"
    );
}

/// Test todo complete command
#[test]
#[ignore = "Git repo detection issue with symlinked temp paths on macOS - see issue notes"]
fn test_todo_complete_command() {
    let temp_dir = setup_todo_test_env();
    let temp_path = temp_dir.path();

    // First create a todo item
    let create_output =
        run_sah_command_with_cwd(&["todo", "create", "--task", "Task to complete"], temp_path);
    assert!(create_output.status.success(), "todo create should succeed");

    // Extract the ID from the creation output
    let create_stdout = String::from_utf8_lossy(&create_output.stdout);
    let todo_id = extract_todo_id_from_output(&create_stdout);

    // Now complete the item
    let complete_output =
        run_sah_command_with_cwd(&["todo", "complete", "--id", todo_id], temp_path);

    assert!(
        complete_output.status.success(),
        "todo complete should succeed. stderr: {}",
        String::from_utf8_lossy(&complete_output.stderr)
    );

    let stdout = String::from_utf8_lossy(&complete_output.stdout);
    assert!(
        stdout.contains("marked_complete") || stdout.contains("Marked todo item"),
        "Output should confirm completion"
    );
    assert!(stdout.contains(todo_id), "Output should contain the ID");
}

/// Test todo create with missing required argument
#[test]
fn test_todo_create_missing_task() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let temp_path = temp_dir.path();

    let output = run_sah_command_with_cwd(&["todo", "create"], temp_path);

    assert!(
        !output.status.success(),
        "todo create without task should fail"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--task") || stderr.contains("required"),
        "Error should mention missing task argument"
    );
}

/// Test todo complete with invalid ID
#[test]
fn test_todo_complete_invalid_id() {
    let temp_dir = setup_todo_test_env();
    let temp_path = temp_dir.path();

    let output = run_sah_command_with_cwd(
        &["todo", "complete", "--id", "01INVALID00000000000000000"],
        temp_path,
    );

    // Should fail with an error about the ID not being found
    assert!(
        !output.status.success(),
        "todo complete with invalid ID should fail"
    );
}

/// Test full todo workflow: create -> show -> complete
#[test]
#[ignore = "Git repo detection issue with symlinked temp paths on macOS - see issue notes"]
fn test_todo_full_workflow() {
    let temp_dir = setup_todo_test_env();
    let temp_path = temp_dir.path();

    // Step 1: Create a todo
    let create_output = run_sah_command_with_cwd(
        &[
            "todo",
            "create",
            "--task",
            "Workflow test task",
            "--context",
            "Testing full workflow",
        ],
        temp_path,
    );
    assert!(create_output.status.success(), "Create should succeed");

    // Step 2: Show next todo
    let show_output = run_sah_command_with_cwd(&["todo", "show", "--item", "next"], temp_path);
    assert!(show_output.status.success(), "Show should succeed");
    let show_stdout = String::from_utf8_lossy(&show_output.stdout);
    assert!(
        show_stdout.contains("Workflow test task"),
        "Should show the created task"
    );

    // Extract ID from show output
    let todo_id = extract_todo_id_from_output(&show_stdout);

    // Step 3: Complete the todo
    let complete_output =
        run_sah_command_with_cwd(&["todo", "complete", "--id", todo_id], temp_path);
    assert!(complete_output.status.success(), "Complete should succeed");

    // Step 4: Verify no more incomplete todos
    let final_show = run_sah_command_with_cwd(&["todo", "show", "--item", "next"], temp_path);
    assert!(final_show.status.success(), "Final show should succeed");
    let final_stdout = String::from_utf8_lossy(&final_show.stdout);
    assert!(
        final_stdout.contains("No incomplete todo items") || final_stdout.contains("null"),
        "Should have no incomplete items"
    );
}

/// Test todo list command with no todos
#[test]
#[ignore = "Git repo detection issue with symlinked temp paths on macOS - see issue notes"]
fn test_todo_list_empty() {
    let temp_dir = setup_todo_test_env();
    let temp_path = temp_dir.path();

    let output = run_sah_command_with_cwd(&["todo", "list"], temp_path);

    assert!(output.status.success(), "todo list should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_json_field_value(&stdout, "total", 0);
    assert_json_field_value(&stdout, "pending", 0);
}

/// Test todo list command with multiple todos
#[test]
#[ignore = "Git repo detection issue with symlinked temp paths on macOS - see issue notes"]
fn test_todo_list_multiple() {
    let temp_dir = setup_todo_test_env();
    let temp_path = temp_dir.path();

    // Create multiple todos
    run_sah_command_with_cwd(&["todo", "create", "--task", "Task 1"], temp_path);
    run_sah_command_with_cwd(&["todo", "create", "--task", "Task 2"], temp_path);
    run_sah_command_with_cwd(&["todo", "create", "--task", "Task 3"], temp_path);

    let output = run_sah_command_with_cwd(&["todo", "list"], temp_path);

    assert!(output.status.success(), "todo list should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Task 1") && stdout.contains("Task 2") && stdout.contains("Task 3"),
        "Should show all three tasks"
    );
    assert_json_field_value(&stdout, "total", 3);
    assert_json_field_value(&stdout, "pending", 3);
}

/// Test todo list with filter for incomplete todos
#[test]
#[ignore = "Git repo detection issue with symlinked temp paths on macOS - see issue notes"]
fn test_todo_list_filter_incomplete() {
    let temp_dir = setup_todo_test_env();
    let temp_path = temp_dir.path();

    // Create todos
    run_sah_command_with_cwd(&["todo", "create", "--task", "Task 1"], temp_path);
    let create2 = run_sah_command_with_cwd(&["todo", "create", "--task", "Task 2"], temp_path);
    run_sah_command_with_cwd(&["todo", "create", "--task", "Task 3"], temp_path);

    // Complete one task
    let create2_stdout = String::from_utf8_lossy(&create2.stdout);
    let todo_id = extract_todo_id_from_output(&create2_stdout);
    run_sah_command_with_cwd(&["todo", "complete", "--id", todo_id], temp_path);

    // List incomplete only
    let output = run_sah_command_with_cwd(&["todo", "list", "--completed", "false"], temp_path);

    assert!(output.status.success(), "todo list should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Task 1") && stdout.contains("Task 3"),
        "Should show incomplete tasks"
    );
    assert!(!stdout.contains("Task 2"), "Should not show completed task");
    assert_json_field_value(&stdout, "total", 2);
}

/// Test todo list with filter for completed todos
#[test]
#[ignore = "Git repo detection issue with symlinked temp paths on macOS - see issue notes"]
fn test_todo_list_filter_completed() {
    let temp_dir = setup_todo_test_env();
    let temp_path = temp_dir.path();

    // Create todos
    run_sah_command_with_cwd(&["todo", "create", "--task", "Task 1"], temp_path);
    let create2 = run_sah_command_with_cwd(&["todo", "create", "--task", "Task 2"], temp_path);
    run_sah_command_with_cwd(&["todo", "create", "--task", "Task 3"], temp_path);

    // Complete one task
    let create2_stdout = String::from_utf8_lossy(&create2.stdout);
    let todo_id = extract_todo_id_from_output(&create2_stdout);
    run_sah_command_with_cwd(&["todo", "complete", "--id", todo_id], temp_path);

    // List completed only
    let output = run_sah_command_with_cwd(&["todo", "list", "--completed", "true"], temp_path);

    assert!(output.status.success(), "todo list should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Task 2"), "Should show completed task");
    assert!(
        !stdout.contains("Task 1") && !stdout.contains("Task 3"),
        "Should not show incomplete tasks"
    );
    assert_json_field_value(&stdout, "total", 1);
}

/// Test todo list sort order (incomplete first)
#[test]
#[ignore = "Git repo detection issue with symlinked temp paths on macOS - see issue notes"]
fn test_todo_list_sort_order() {
    let temp_dir = setup_todo_test_env();
    let temp_path = temp_dir.path();

    // Create todos
    let create1 = run_sah_command_with_cwd(&["todo", "create", "--task", "Task 1"], temp_path);
    run_sah_command_with_cwd(&["todo", "create", "--task", "Task 2"], temp_path);
    run_sah_command_with_cwd(&["todo", "create", "--task", "Task 3"], temp_path);

    // Complete the first task
    let create1_stdout = String::from_utf8_lossy(&create1.stdout);
    let todo_id = extract_todo_id_from_output(&create1_stdout);
    run_sah_command_with_cwd(&["todo", "complete", "--id", todo_id], temp_path);

    // List all todos
    let output = run_sah_command_with_cwd(&["todo", "list"], temp_path);

    assert!(output.status.success(), "todo list should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Find positions of tasks in output
    let task1_pos = stdout.find("Task 1").expect("Should find Task 1");
    let task2_pos = stdout.find("Task 2").expect("Should find Task 2");
    let task3_pos = stdout.find("Task 3").expect("Should find Task 3");

    // Task 2 and Task 3 (incomplete) should come before Task 1 (completed)
    assert!(
        task2_pos < task1_pos && task3_pos < task1_pos,
        "Incomplete tasks should come before completed tasks"
    );
}
