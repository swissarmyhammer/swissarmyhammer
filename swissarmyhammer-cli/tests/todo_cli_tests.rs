//! Integration tests for todo CLI commands
//!
//! This test suite verifies that the todo CLI commands (create, show, complete)
//! work correctly through the dynamic CLI interface.
//!
//! ## Test Helper Patterns
//!
//! This module uses several semantic wrapper functions (e.g., `assert_next_todo_contains`,
//! `complete_todo_and_verify`) that wrap more generic helper functions with specific parameters.
//! These wrappers exist to provide clarity at test call sites, making the test intent explicit
//! without requiring readers to understand the underlying generic helpers.

use git2::Repository;
use serde_yaml::Value;
use std::process::Command;
use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;

/// Length of the YAML prefix `id: ` used when parsing todo IDs from command output
const YAML_ID_PREFIX_LEN: usize = 4;

/// Standard test tasks used across multiple test setup functions
const TEST_TASKS: &[(&str, Option<&str>)] = &[("Task 1", None), ("Task 2", None), ("Task 3", None)];

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
///
/// Returns a tuple of (TempDir, Path) to reduce the repetitive pattern of calling
/// `setup_todo_test_env()` followed by `temp_dir.path()` in every test.
fn setup_todo_test_env() -> IsolatedTestEnvironment {
    let env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_path = env.temp_dir();

    init_git_repo(&temp_path);
    std::fs::create_dir_all(temp_path.join(".swissarmyhammer"))
        .expect("Failed to create .swissarmyhammer dir");

    env
}

/// Helper function that returns both TempDir and path for convenient test setup
///
/// This eliminates the common pattern of `let temp_dir = setup_todo_test_env(); let temp_path = temp_dir.path();`
/// Usage: `let (temp_dir, temp_path) = setup_todo_test();`
#[allow(dead_code)]
fn setup_todo_test() -> (IsolatedTestEnvironment, std::path::PathBuf) {
    let env = setup_todo_test_env();
    let temp_path = env.temp_dir();
    (env, temp_path)
}

/// Helper function to extract todo ID from YAML output
fn extract_todo_id_from_output(output: &str) -> &str {
    let id_start = output.find("id: ").expect("Should find id in output") + YAML_ID_PREFIX_LEN;
    let id_end = output[id_start..]
        .find('\n')
        .unwrap_or(output.len() - id_start)
        + id_start;
    output[id_start..id_end].trim()
}

/// Generic helper function to parse YAML and extract a numeric field value
fn parse_yaml_field_u32(output: &str, field: &str) -> Result<u32, String> {
    let parsed: Value = serde_yaml::from_str(output)
        .map_err(|e| format!("Failed to parse YAML output: {}. Error: {}", output, e))?;

    parsed[field]
        .as_u64()
        .ok_or_else(|| format!("Field '{}' not found or not a number in YAML", field))
        .map(|v| v as u32)
}

/// Helper function to parse YAML output and extract a numeric field value (panics on error)
fn get_yaml_field_value(output: &str, field: &str) -> u32 {
    parse_yaml_field_u32(output, field).unwrap_or_else(|e| panic!("{}", e))
}

/// Helper function to assert YAML field contains expected numeric value
fn assert_yaml_field_value(output: &str, field: &str, expected: u32) {
    let actual = get_yaml_field_value(output, field);
    assert_eq!(actual, expected, "Field '{}' should be {}", field, expected);
}

/// Helper function to assert todo counts in YAML output
fn assert_todo_counts(output: &str, total: u32, pending: u32, completed: u32) {
    assert_yaml_field_value(output, "total", total);
    assert_yaml_field_value(output, "pending", pending);
    assert_yaml_field_value(output, "completed", completed);
}

/// Helper function to convert process output to stdout string
fn get_stdout_string(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Helper function to create a todo and return its ID
fn create_todo_and_get_id(
    task: &str,
    context: Option<&str>,
    temp_path: &std::path::Path,
) -> String {
    let mut args = vec!["todo", "create", "--task", task];
    if let Some(ctx) = context {
        args.extend_from_slice(&["--context", ctx]);
    }

    let output = run_sah_command_with_cwd(&args, temp_path);
    assert!(
        output.status.success(),
        "todo create should succeed. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = get_stdout_string(&output);
    extract_todo_id_from_output(&stdout).to_string()
}

/// Unified helper function to create todos and optionally complete specific ones by indices
///
/// This consolidates the setup logic, eliminating duplication across setup_todos_with_completed,
/// setup_todos_with_one_completed, and create_three_test_todos.
///
/// # Arguments
/// * `tasks` - Slice of (task, context) tuples to create
/// * `temp_path` - Path to the test directory
/// * `complete_indices` - Indices of todos to mark as complete after creation
fn setup_todos_with_completion_pattern(
    tasks: &[(&str, Option<&str>)],
    temp_path: &std::path::Path,
    complete_indices: &[usize],
) -> Vec<String> {
    let ids = create_multiple_todos(tasks, temp_path);

    for &index in complete_indices {
        assert!(
            index < ids.len(),
            "complete_index {} must be within bounds (got {}, max {})",
            index,
            index,
            ids.len()
        );

        let complete_output =
            run_sah_command_with_cwd(&["todo", "complete", "--id", &ids[index]], temp_path);
        assert!(
            complete_output.status.success(),
            "todo complete should succeed"
        );
    }

    ids
}

/// Helper function to create three test todos
fn create_three_test_todos(temp_path: &std::path::Path) -> Vec<String> {
    setup_todos_with_completion_pattern(TEST_TASKS, temp_path, &[])
}

/// Helper function to create three todos and complete specific ones by indices
fn setup_todos_with_completed(
    temp_path: &std::path::Path,
    complete_indices: &[usize],
) -> Vec<String> {
    setup_todos_with_completion_pattern(TEST_TASKS, temp_path, complete_indices)
}

/// Helper function to create three todos and complete one
fn setup_todos_with_one_completed(
    temp_path: &std::path::Path,
    complete_index: usize,
) -> Vec<String> {
    setup_todos_with_completed(temp_path, &[complete_index])
}

/// Helper function to show next todo and assert it contains expected task
fn assert_next_todo_contains(temp_path: &std::path::Path, expected_task: &str) {
    assert_output_contains(
        &["todo", "show", "--item", "next"],
        temp_path,
        true,
        &[expected_task],
        &[],
    );
}

/// Helper function to complete a todo and verify success
fn complete_todo_and_verify(todo_id: &str, temp_path: &std::path::Path) {
    assert_output_contains(
        &["todo", "complete", "--id", todo_id],
        temp_path,
        true,
        &["OK"],
        &[],
    );
}

/// Helper function to run command and assert no incomplete todos state
fn assert_no_incomplete_todos(args: &[&str], temp_path: &std::path::Path) {
    let stdout = run_and_get_output(args, temp_path, true);
    assert!(
        stdout.contains("No incomplete todo items") || stdout.contains("null"),
        "Should indicate no incomplete items: {}",
        stdout
    );
}

/// Helper function to create multiple todos from a slice of (task, context) tuples
fn create_multiple_todos(
    tasks: &[(&str, Option<&str>)],
    temp_path: &std::path::Path,
) -> Vec<String> {
    tasks
        .iter()
        .map(|(task, context)| create_todo_and_get_id(task, *context, temp_path))
        .collect()
}

/// Helper function to run command and get stdout on success or stderr on failure
fn run_and_get_output(args: &[&str], temp_path: &std::path::Path, expect_success: bool) -> String {
    let output = run_sah_command_with_cwd(args, temp_path);
    let status = output.status.success();

    assert_eq!(
        status,
        expect_success,
        "Command {} should {}. stderr: {}",
        args.join(" "),
        if expect_success { "succeed" } else { "fail" },
        String::from_utf8_lossy(&output.stderr)
    );

    if status {
        get_stdout_string(&output)
    } else {
        String::from_utf8_lossy(&output.stderr).to_string()
    }
}

/// Unified helper function to assert command output contains/excludes specific fragments
///
/// This function consolidates the logic for running commands and verifying their output,
/// eliminating duplication across assert_output_contains, assert_command_succeeds_with_output,
/// and assert_todo_list_contains.
fn assert_output_contains(
    args: &[&str],
    temp_path: &std::path::Path,
    expect_success: bool,
    expected_fragments: &[&str],
    unexpected_fragments: &[&str],
) -> String {
    let output = run_and_get_output(args, temp_path, expect_success);

    for fragment in expected_fragments {
        assert!(
            output.contains(fragment),
            "Output should contain '{}'. Output: {}",
            fragment,
            output
        );
    }

    for fragment in unexpected_fragments {
        assert!(
            !output.contains(fragment),
            "Output should not contain '{}'. Output: {}",
            fragment,
            output
        );
    }

    output
}

/// Helper function to test todo list filtering with specific completion status
fn assert_todo_list_filter(
    temp_path: &std::path::Path,
    completed: bool,
    expected_tasks: &[&str],
    unexpected_tasks: &[&str],
    expected_count: u32,
) {
    let completed_arg = if completed { "true" } else { "false" };

    let stdout = assert_output_contains(
        &["todo", "list", "--completed", completed_arg],
        temp_path,
        true,
        expected_tasks,
        unexpected_tasks,
    );
    assert_yaml_field_value(&stdout, "total", expected_count);
}

/// Helper function to assert help text contains all expected commands
fn assert_help_contains_commands(help_text: &str, commands: &[&str]) {
    for command in commands {
        assert!(
            help_text.contains(command),
            "help should mention '{}' command",
            command
        );
    }
}

/// Helper function to assert todo file was created in the .swissarmyhammer directory
fn assert_todo_file_created(temp_path: &std::path::Path) {
    let todo_file = temp_path
        .join(".swissarmyhammer")
        .join("todo")
        .join("todo.yaml");
    assert!(
        todo_file.exists(),
        "todo.yaml file should be created at {}",
        todo_file.display()
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
    assert_help_contains_commands(&todo_stdout, &["create", "show", "list", "complete"]);
}

/// Test todo create command
#[test]
fn test_todo_create_command() {
    let temp_dir = setup_todo_test_env();
    let temp_path = &temp_dir.temp_dir();

    assert_output_contains(
        &[
            "todo",
            "create",
            "--task",
            "Test task",
            "--context",
            "Test context",
        ],
        temp_path,
        true,
        &["Created todo item", "Test task"],
        &[],
    );

    assert_todo_file_created(temp_path);
}

/// Test todo create command without optional context
#[test]
fn test_todo_create_without_context() {
    let temp_dir = setup_todo_test_env();
    let temp_path = &temp_dir.temp_dir();

    assert_output_contains(
        &["todo", "create", "--task", "Simple task"],
        temp_path,
        true,
        &["Created todo item", "Simple task"],
        &[],
    );

    assert_todo_file_created(temp_path);
}

/// Test todo show next command
#[test]
fn test_todo_show_next() {
    let temp_dir = setup_todo_test_env();
    let temp_path = &temp_dir.temp_dir();

    // Create a todo item
    create_todo_and_get_id("Task to show", None, temp_path);

    // Now show the next item
    assert_next_todo_contains(temp_path, "Task to show");
}

/// Test todo show next with no todos
#[test]
fn test_todo_show_next_empty() {
    let temp_dir = setup_todo_test_env();
    let temp_path = &temp_dir.temp_dir();

    assert_no_incomplete_todos(&["todo", "show", "--item", "next"], temp_path);
}

/// Test todo complete command
#[test]
fn test_todo_complete_command() {
    let temp_dir = setup_todo_test_env();
    let temp_path = &temp_dir.temp_dir();

    // Create a todo and get its ID
    let todo_id = create_todo_and_get_id("Task to complete", None, temp_path);

    // Complete the item
    complete_todo_and_verify(&todo_id, temp_path);
}

/// Test todo create with missing required argument
#[test]
fn test_todo_create_missing_task() {
    let temp_dir = setup_todo_test_env();
    let temp_path = &temp_dir.temp_dir();

    let stderr = run_and_get_output(&["todo", "create"], temp_path, false);
    assert!(
        stderr.contains("--task") || stderr.contains("required"),
        "Error should mention missing task argument"
    );
}

/// Test todo complete with invalid ID
#[test]
fn test_todo_complete_invalid_id() {
    let temp_dir = setup_todo_test_env();
    let temp_path = &temp_dir.temp_dir();

    // Should fail with an error about the ID not being found
    run_and_get_output(
        &["todo", "complete", "--id", "01INVALID00000000000000000"],
        temp_path,
        false,
    );
}

/// Test full todo workflow: create -> show -> complete
#[test]
fn test_todo_full_workflow() {
    let temp_dir = setup_todo_test_env();
    let temp_path = &temp_dir.temp_dir();

    // Step 1: Create a todo and get its ID
    let todo_id = create_todo_and_get_id(
        "Workflow test task",
        Some("Testing full workflow"),
        temp_path,
    );

    // Step 2: Show next todo
    assert_next_todo_contains(temp_path, "Workflow test task");

    // Step 3: Complete the todo
    complete_todo_and_verify(&todo_id, temp_path);

    // Step 4: Verify no more incomplete todos
    assert_no_incomplete_todos(&["todo", "show", "--item", "next"], temp_path);
}

/// Test todo list command with no todos
#[test]
fn test_todo_list_empty() {
    let temp_dir = setup_todo_test_env();
    let temp_path = &temp_dir.temp_dir();

    let stdout = run_and_get_output(&["todo", "list"], temp_path, true);
    assert_todo_counts(&stdout, 0, 0, 0);
}

/// Test todo list command with multiple todos
#[test]
fn test_todo_list_multiple() {
    let temp_dir = setup_todo_test_env();
    let temp_path = &temp_dir.temp_dir();

    // Create three todos
    create_three_test_todos(temp_path);

    let stdout = assert_output_contains(
        &["todo", "list"],
        temp_path,
        true,
        &["Task 1", "Task 2", "Task 3"],
        &[],
    );
    assert_todo_counts(&stdout, 3, 3, 0);
}

/// Parameterized test helper for todo list filtering tests
fn test_todo_list_with_filter(
    completed: bool,
    expected_tasks: &[&str],
    unexpected_tasks: &[&str],
    expected_count: u32,
) {
    let temp_dir = setup_todo_test_env();
    let temp_path = &temp_dir.temp_dir();

    // Create three todos with Task 2 completed
    setup_todos_with_one_completed(temp_path, 1);

    // List with specified filter
    assert_todo_list_filter(
        temp_path,
        completed,
        expected_tasks,
        unexpected_tasks,
        expected_count,
    );
}

/// Test todo list with filter for incomplete todos
#[test]
fn test_todo_list_filter_incomplete() {
    test_todo_list_with_filter(false, &["Task 1", "Task 3"], &["Task 2"], 2);
}

/// Test todo list with filter for completed todos
#[test]
fn test_todo_list_filter_completed() {
    test_todo_list_with_filter(true, &["Task 2"], &["Task 1", "Task 3"], 1);
}

/// Test todo list sort order (incomplete first)
#[test]
fn test_todo_list_sort_order() {
    let temp_dir = setup_todo_test_env();
    let temp_path = &temp_dir.temp_dir();

    // Create three todos with Task 1 completed
    setup_todos_with_one_completed(temp_path, 0);

    // List all todos - verify all tasks are present
    let stdout = assert_output_contains(
        &["todo", "list"],
        temp_path,
        true,
        &["Task 1", "Task 2", "Task 3"],
        &[],
    );

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
