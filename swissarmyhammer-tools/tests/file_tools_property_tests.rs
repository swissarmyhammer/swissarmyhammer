//! Simplified property-based tests for file tools
//!
//! This module contains property-based tests using proptest to verify
//! that file tools behave correctly across a range of inputs.

use proptest::prelude::*;
use proptest::test_runner::Config as ProptestConfig;
use serde_json::json;

use std::sync::Arc;


use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_git::GitOperations;
use swissarmyhammer_issues::{FileSystemIssueStorage, IssueStorage};
use swissarmyhammer_memoranda::{MarkdownMemoStorage, MemoStorage};
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::{ToolContext, ToolRegistry};
use swissarmyhammer_tools::mcp::tools::files;
use tempfile::TempDir;

/// Create a test context for property testing
async fn create_property_test_context() -> ToolContext {
    let issue_storage: Arc<tokio::sync::RwLock<Box<dyn IssueStorage>>> =
        Arc::new(tokio::sync::RwLock::new(Box::new(
            FileSystemIssueStorage::new(tempfile::tempdir().unwrap().path().to_path_buf()).unwrap(),
        )));
    let git_ops: Arc<tokio::sync::Mutex<Option<GitOperations>>> =
        Arc::new(tokio::sync::Mutex::new(None));
    // Create temporary directory for memo storage in tests
    let temp_dir = tempfile::tempdir().unwrap();
    let memo_temp_dir = temp_dir.path().join("memos");
    let memo_storage: Arc<tokio::sync::RwLock<Box<dyn MemoStorage>>> = Arc::new(
        tokio::sync::RwLock::new(Box::new(MarkdownMemoStorage::new(memo_temp_dir))),
    );

    let tool_handlers = Arc::new(ToolHandlers::new(memo_storage.clone()));

    ToolContext::new(
        tool_handlers,
        issue_storage,
        git_ops,
        memo_storage,
    )
}

/// Create a test tool registry
fn create_property_test_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    files::register_file_tools(&mut registry);
    registry
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]
    /// Property: Write content to a file, then read it back. The content should be identical.
    #[test]
    fn test_write_read_roundtrip_property(
        content in ".*{0,100}", // Simple content pattern
    ) {
        let result = tokio_test::block_on(async {
            let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
            let registry = create_property_test_registry();
            let context = create_property_test_context().await;
            let write_tool = registry.get_tool("files_write").unwrap();
            let read_tool = registry.get_tool("files_read").unwrap();

            let temp_dir = TempDir::new().unwrap();
            let test_file = temp_dir.path().join("test.txt");

            // Write content
            let mut write_args = serde_json::Map::new();
            write_args.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
            write_args.insert("content".to_string(), json!(content));

            let write_result = write_tool.execute(write_args, &context).await;
            prop_assert!(write_result.is_ok(), "Write should succeed");

            // Read content back
            let mut read_args = serde_json::Map::new();
            read_args.insert("absolute_path".to_string(), json!(test_file.to_string_lossy()));

            let read_result = read_tool.execute(read_args, &context).await;
            prop_assert!(read_result.is_ok(), "Read should succeed");

            let call_result = read_result.unwrap();
            prop_assert_eq!(call_result.is_error, Some(false));

            // Verify content matches
            if let Some(content_item) = call_result.content.first() {
                if let rmcp::model::RawContent::Text(text_content) = &content_item.raw {
                    prop_assert_eq!(&text_content.text, &content);
                }
            }

            Ok(())
        });
        result.unwrap()
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]
    /// Property: Edit operations should be deterministic - same input produces same output
    #[test]
    fn test_edit_deterministic_property(
        content in r"[a-zA-Z ]{10,50}",
        old_string in r"[a-zA-Z]{2,5}",
        new_string in r"[a-zA-Z]{2,5}",
    ) {
        // Skip this test case if old_string equals new_string (not allowed by edit tool)
        prop_assume!(old_string != new_string);
        let result = tokio_test::block_on(async {
            // Create content that definitely contains the old_string
            let test_content = format!("{} {} {}", content, old_string, content);

            let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
            let registry = create_property_test_registry();
            let context = create_property_test_context().await;
            let write_tool = registry.get_tool("files_write").unwrap();
            let edit_tool = registry.get_tool("files_edit").unwrap();
            let read_tool = registry.get_tool("files_read").unwrap();

            let temp_dir = TempDir::new().unwrap();

            // Create two identical files
            let file1 = temp_dir.path().join("file1.txt");
            let file2 = temp_dir.path().join("file2.txt");

            // Write same content to both files
            for file_path in [&file1, &file2] {
                let mut write_args = serde_json::Map::new();
                write_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
                write_args.insert("content".to_string(), json!(test_content));

                let write_result = write_tool.execute(write_args, &context).await;
                prop_assert!(write_result.is_ok());
            }

            // Apply same edit to both files
            for file_path in [&file1, &file2] {
                let mut edit_args = serde_json::Map::new();
                edit_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
                edit_args.insert("old_string".to_string(), json!(old_string));
                edit_args.insert("new_string".to_string(), json!(new_string));
                edit_args.insert("replace_all".to_string(), json!(false));

                let edit_result = edit_tool.execute(edit_args, &context).await;
                prop_assert!(edit_result.is_ok());
            }

            // Read both files and verify they are identical
            let mut file_contents = Vec::new();
            for file_path in [&file1, &file2] {
                let mut read_args = serde_json::Map::new();
                read_args.insert("absolute_path".to_string(), json!(file_path.to_string_lossy()));

                let read_result = read_tool.execute(read_args, &context).await;
                prop_assert!(read_result.is_ok());

                let call_result = read_result.unwrap();
                if let Some(content_item) = call_result.content.first() {
                    if let rmcp::model::RawContent::Text(text_content) = &content_item.raw {
                        file_contents.push(text_content.text.clone());
                    }
                }
            }

            prop_assert_eq!(file_contents.len(), 2);
            prop_assert_eq!(&file_contents[0], &file_contents[1],
                           "Same edit operation should produce identical results");

            Ok(())
        });
        result.unwrap()
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]
    /// Property: Glob patterns should return consistent results
    #[test]
    fn test_glob_consistency_property(
        pattern in r"\*\.(txt|rs|md)",
    ) {
        let result = tokio_test::block_on(async {
            let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
            let registry = create_property_test_registry();
            let context = create_property_test_context().await;
            let write_tool = registry.get_tool("files_write").unwrap();
            let glob_tool = registry.get_tool("files_glob").unwrap();

            let temp_dir = TempDir::new().unwrap();

            // Create test files
            let test_files = ["test1.txt", "test2.rs", "test3.md", "test4.py"];
            for file_name in &test_files {
                let file_path = temp_dir.path().join(file_name);
                let mut write_args = serde_json::Map::new();
                write_args.insert("absolute_path".to_string(), json!(file_path.to_string_lossy()));
                write_args.insert("content".to_string(), json!("test content"));

                let _write_result = write_tool.execute(write_args, &context).await;
            }

            // Run glob search multiple times
            let mut results = Vec::new();
            for _ in 0..2 {
                let mut glob_args = serde_json::Map::new();
                glob_args.insert("pattern".to_string(), json!(pattern));
                glob_args.insert("path".to_string(), json!(temp_dir.path().to_string_lossy()));

                let glob_result = glob_tool.execute(glob_args, &context).await;
                if let Ok(call_result) = glob_result {
                    if let Some(content_item) = call_result.content.first() {
                        if let rmcp::model::RawContent::Text(text_content) = &content_item.raw {
                            results.push(text_content.text.clone());
                        }
                    }
                }
            }

            // Results should be identical
            if results.len() >= 2 {
                prop_assert_eq!(&results[0], &results[1],
                               "Glob results should be consistent across multiple runs");
            }

            Ok(())
        });
        result.unwrap()
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]
    /// Property: File tools should handle valid input gracefully
    #[test]
    fn test_path_validation_property(
        filename in r"[a-zA-Z][a-zA-Z0-9_]{0,20}\.(txt|rs|md)",
    ) {
        let result = tokio_test::block_on(async {
            let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
            let registry = create_property_test_registry();
            let context = create_property_test_context().await;
            let write_tool = registry.get_tool("files_write").unwrap();
            let read_tool = registry.get_tool("files_read").unwrap();

            let temp_dir = TempDir::new().unwrap();
            let test_file = temp_dir.path().join(filename);
            let test_content = "Valid test content";

            // Write file
            let mut write_args = serde_json::Map::new();
            write_args.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
            write_args.insert("content".to_string(), json!(test_content));

            let write_result = write_tool.execute(write_args, &context).await;
            prop_assert!(write_result.is_ok(), "Should handle valid filename");

            // Read file back
            let mut read_args = serde_json::Map::new();
            read_args.insert("absolute_path".to_string(), json!(test_file.to_string_lossy()));

            let read_result = read_tool.execute(read_args, &context).await;
            prop_assert!(read_result.is_ok(), "Should read valid file");

            Ok(())
        });
        result.unwrap()
    }
}
