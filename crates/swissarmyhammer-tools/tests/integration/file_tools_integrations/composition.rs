//! Two or more file tools in one workflow.
//!
//! Write then read, write then edit, read then edit, glob then grep, and
//! how an error in the middle of a workflow reports.

use super::*;

// ============================================================================
// Tool Composition and Integration Tests
// ============================================================================

#[tokio::test]
async fn test_write_then_read_workflow() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files").unwrap();
    let read_tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("write_read_test.txt");
    let test_content = "Content written by write tool\nSecond line of content\n";

    // Step 1: Write file
    let mut write_args = serde_json::Map::new();
    write_args.insert("op".to_string(), json!("write file"));
    write_args.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    write_args.insert("content".to_string(), json!(test_content));

    let write_result = write_tool.execute(write_args, &context).await;
    assert!(write_result.is_ok(), "Write should succeed");

    let write_call_result = write_result.unwrap();
    assert_eq!(write_call_result.is_error, Some(false));

    // Step 2: Read the same file
    let mut read_args = serde_json::Map::new();
    read_args.insert("op".to_string(), json!("read file"));
    read_args.insert("path".to_string(), json!(test_file.to_string_lossy()));

    let read_result = read_tool.execute(read_args, &context).await;
    assert!(read_result.is_ok(), "Read should succeed");

    let read_call_result = read_result.unwrap();
    assert_eq!(read_call_result.is_error, Some(false));

    // Verify content matches (default read is hashline-tagged; recover plain).
    assert_eq!(read_content(&read_call_result), test_content);
}

#[tokio::test]
async fn test_write_then_edit_workflow() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files").unwrap();
    let edit_tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("write_edit_test.txt");
    let initial_content = "Original content that needs updating";

    // Step 1: Write initial file
    let write_args = write_args(&test_file.to_string_lossy(), initial_content);

    let write_result = write_tool.execute(write_args, &context).await;
    assert!(write_result.is_ok(), "Write should succeed");

    // Step 2: Edit the file
    let mut edit_args = edit_args(&test_file.to_string_lossy(), "Original", "Updated");
    edit_args.insert("replace_all".to_string(), json!(false));

    let edit_result = edit_tool.execute(edit_args, &context).await;
    assert!(edit_result.is_ok(), "Edit should succeed");

    let edit_call_result = edit_result.unwrap();
    assert_eq!(edit_call_result.is_error, Some(false));

    // Verify file was edited correctly
    let final_content = fs::read_to_string(test_file).unwrap();
    assert_eq!(final_content, "Updated content that needs updating");
}

#[tokio::test]
async fn test_read_then_edit_workflow() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let read_tool = registry.get_tool("files").unwrap();
    let edit_tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("read_edit_test.txt");
    let initial_content = "Function calculate_sum() {\n    return a + b;\n}";
    fs::write(test_file, initial_content).unwrap();

    // Step 1: Read the file to analyze content
    let read_args = read_args(&test_file.to_string_lossy());

    let read_result = read_tool.execute(read_args, &context).await;
    assert!(read_result.is_ok(), "Read should succeed");

    let read_call_result = read_result.unwrap();
    let response_text = extract_response_text(&read_call_result);

    // Verify we can read the function name
    assert!(response_text.contains("calculate_sum"));

    // Step 2: Edit the function name based on what we read
    let mut edit_args = edit_args(&test_file.to_string_lossy(), "calculate_sum", "add_numbers");
    edit_args.insert("replace_all".to_string(), json!(false));

    let edit_result = edit_tool.execute(edit_args, &context).await;
    assert!(edit_result.is_ok(), "Edit should succeed");

    // Verify the edit was successful
    let final_content = fs::read_to_string(test_file).unwrap();
    assert_eq!(
        final_content,
        "Function add_numbers() {\n    return a + b;\n}"
    );
}

#[tokio::test]
async fn test_glob_then_grep_workflow() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let glob_tool = registry.get_tool("files").unwrap();
    let grep_tool = registry.get_tool("files").unwrap();

    // Create test directory structure with multiple files
    let (_env, temp_dir) = create_test_dir_with_git();

    let test_files = vec![
        ("src/main.rs", "fn main() {\n    println!(\"Hello, world!\");\n    let result = calculate();\n}"),
        ("src/lib.rs", "pub fn calculate() -> i32 {\n    42\n}\n\npub fn helper() {\n    // Helper function\n}"),
        ("tests/integration.rs", "use mylib;\n\n#[test]\nfn test_calculate() {\n    assert_eq!(mylib::calculate(), 42);\n}"),
        ("README.md", "# My Project\n\nThis project has calculate functions.\n"),
    ];

    for (file_path, content) in test_files {
        let full_path = &temp_dir.join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full_path, content).unwrap();
    }

    // Step 1: Use glob to find all Rust files
    let mut glob_args = glob_args("**/*.rs");
    glob_args.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));

    let glob_result = glob_tool.execute(glob_args, &context).await;
    assert!(glob_result.is_ok(), "Glob should succeed");

    let glob_call_result = glob_result.unwrap();
    assert_eq!(glob_call_result.is_error, Some(false));

    let glob_response = extract_response_text(&glob_call_result);

    // Verify glob found Rust files
    assert!(glob_response.contains("main.rs"));
    assert!(glob_response.contains("lib.rs"));
    assert!(glob_response.contains("integration.rs"));
    assert!(!glob_response.contains("README.md")); // Should not find non-Rust files

    // Step 2: Use grep to search within the files found by glob
    let mut grep_args = grep_args("calculate");
    grep_args.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));
    grep_args.insert("glob".to_string(), json!("*.rs")); // Search within Rust files

    let grep_result = grep_tool.execute(grep_args, &context).await;
    assert!(grep_result.is_ok(), "Grep should succeed");

    let grep_call_result = grep_result.unwrap();
    assert_eq!(grep_call_result.is_error, Some(false));

    let grep_response = extract_response_text(&grep_call_result);

    // Verify grep found "calculate" in Rust files
    assert!(grep_response.contains("calculate") || grep_response.contains("matches"));
}

#[tokio::test]
async fn test_complex_file_workflow() {
    // Test a complex workflow: glob -> read -> edit -> read (to verify)
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let glob_tool = registry.get_tool("files").unwrap();
    let read_tool = registry.get_tool("files").unwrap();
    let edit_tool = registry.get_tool("files").unwrap();

    // Create test project structure
    let (_env, temp_dir) = create_test_dir_with_git();

    let test_files = vec![
        (
            "src/config.json",
            "{\n  \"version\": \"1.0.0\",\n  \"debug\": true\n}",
        ),
        (
            "config/app.json",
            "{\n  \"version\": \"1.0.0\",\n  \"production\": false\n}",
        ),
        (
            "package.json",
            "{\n  \"name\": \"myapp\",\n  \"version\": \"1.0.0\"\n}",
        ),
    ];

    for (file_path, content) in test_files {
        let full_path = &temp_dir.join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full_path, content).unwrap();
    }

    // Step 1: Find JSON files in src directory (scoped glob, not overly broad **/*)
    // Use respect_git_ignore: false because files are untracked in the fresh git repo
    let mut glob_args = serde_json::Map::new();
    glob_args.insert("op".to_string(), json!("glob files"));
    glob_args.insert("pattern".to_string(), json!("src/**/*.json"));
    glob_args.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));
    glob_args.insert("respect_git_ignore".to_string(), json!(false));

    let glob_result = glob_tool.execute(glob_args, &context).await;
    assert!(glob_result.is_ok(), "Glob should find JSON files in src/");

    // Step 2: Read one of the config files
    let config_file = &temp_dir.join("src/config.json");
    let initial_read_args = read_args(&config_file.to_string_lossy());

    let read_result = read_tool.execute(initial_read_args, &context).await;
    assert!(read_result.is_ok(), "Read should succeed");

    let read_call_result = read_result.unwrap();
    let original_content = extract_response_text(&read_call_result);

    // Verify we can read the version
    assert!(original_content.contains("1.0.0"));
    assert!(original_content.contains("debug"));

    // Step 3: Update the version in the config file
    let mut edit_args = edit_args(&config_file.to_string_lossy(), "1.0.0", "1.1.0");
    edit_args.insert("replace_all".to_string(), json!(false));

    let edit_result = edit_tool.execute(edit_args, &context).await;
    assert!(edit_result.is_ok(), "Edit should succeed");

    // Step 4: Read again to verify the change
    let read_verify_args = read_args(&config_file.to_string_lossy());

    let read_verify_result = read_tool.execute(read_verify_args, &context).await;
    assert!(
        read_verify_result.is_ok(),
        "Read verification should succeed"
    );

    let verify_call_result = read_verify_result.unwrap();
    let updated_content = extract_response_text(&verify_call_result);

    // Verify the version was updated
    assert!(updated_content.contains("1.1.0"));
    assert!(!updated_content.contains("1.0.0")); // Old version should be gone
    assert!(updated_content.contains("debug")); // Other content should remain
}

#[tokio::test]
async fn test_error_handling_in_workflow() {
    // Test error handling when tools fail in a workflow
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let read_tool = registry.get_tool("files").unwrap();
    let edit_tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let nonexistent_file = &temp_dir.join("does_not_exist.txt");

    // Step 1: Try to read non-existent file (should fail)
    let mut read_args = serde_json::Map::new();
    read_args.insert("op".to_string(), json!("read file"));
    read_args.insert(
        "path".to_string(),
        json!(nonexistent_file.to_string_lossy()),
    );

    let read_result = read_tool.execute(read_args, &context).await;
    assert!(
        read_result.is_err(),
        "Read should fail for non-existent file"
    );

    // Step 2: Try to edit the same non-existent file (should also fail)
    let mut edit_args = serde_json::Map::new();
    edit_args.insert("op".to_string(), json!("edit file"));
    edit_args.insert(
        "file_path".to_string(),
        json!(nonexistent_file.to_string_lossy()),
    );
    edit_args.insert("old_string".to_string(), json!("old"));
    edit_args.insert("new_string".to_string(), json!("new"));

    let edit_result = edit_tool.execute(edit_args, &context).await;
    assert!(
        edit_result.is_err(),
        "Edit should fail for non-existent file"
    );

    // Both operations should fail gracefully with clear error messages
    let read_error = format!("{:?}", read_result.unwrap_err());
    let edit_error = format!("{:?}", edit_result.unwrap_err());

    assert!(read_error.contains("does not exist") || read_error.contains("not found"));
    assert!(edit_error.contains("does not exist") || edit_error.contains("not found"));
}
