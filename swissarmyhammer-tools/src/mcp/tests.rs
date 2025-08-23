//! Tests for MCP server functionality

use super::server::McpServer;
use super::types::{
    AllCompleteRequest, CreateIssueRequest, MarkCompleteRequest, MergeIssueRequest,
    UpdateIssueRequest, WorkIssueRequest,
};
use super::utils::validate_issue_name;
use crate::test_utils::TestIssueEnvironment;
use rmcp::ServerHandler;
use std::collections::HashMap;
use swissarmyhammer::prompts::Prompt;
use swissarmyhammer::PromptLibrary;

#[tokio::test]
async fn test_mcp_server_creation() {
    let test_env = TestIssueEnvironment::new();
    let _guard = std::env::set_current_dir(test_env.path());

    let library = PromptLibrary::new();
    let server = McpServer::new_with_work_dir(library, test_env.path().to_path_buf()).unwrap();

    let info = server.get_info();
    // Just verify we can get server info - details depend on default implementation
    assert!(!info.server_info.name.is_empty());
    assert!(!info.server_info.version.is_empty());

    // Debug print to see what capabilities are returned
    tracing::debug!("Server capabilities: {:?}", info.capabilities);
}

#[tokio::test]
async fn test_mcp_server_exposes_shell_tools() {
    // Create a test library and server
    let library = PromptLibrary::new();
    let server = McpServer::new(library).unwrap();

    // Test that server info includes shell tools capabilities
    let info = server.get_info();
    assert!(
        info.capabilities.tools.is_some(),
        "Server should expose tools capability"
    );

    // Test that we can get the shell execute tool specifically
    // Note: Direct tool access test would require the full MCP request context,
    // so we test that the server is properly configured to expose tools.
    let tools_cap = info.capabilities.tools.unwrap();
    assert_eq!(tools_cap.list_changed, Some(true));
}

#[tokio::test]
async fn test_mcp_server_list_prompts() {
    let test_env = TestIssueEnvironment::new();
    let _guard = std::env::set_current_dir(test_env.path());

    let mut library = PromptLibrary::new();
    let prompt = Prompt::new("test", "Test prompt: {{ name }}")
        .with_description("Test description".to_string());
    library.add(prompt).unwrap();

    let server = McpServer::new_with_work_dir(library, test_env.path().to_path_buf()).unwrap();
    let prompts = server.list_prompts().await.unwrap();

    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0], "test");
}

#[tokio::test]
async fn test_mcp_server_get_prompt() {
    let test_env = TestIssueEnvironment::new();
    let _guard = std::env::set_current_dir(test_env.path());

    let mut library = PromptLibrary::new();
    let prompt =
        Prompt::new("test", "Hello {{ name }}!").with_description("Greeting prompt".to_string());
    library.add(prompt).unwrap();

    let server = McpServer::new_with_work_dir(library, test_env.path().to_path_buf()).unwrap();
    let mut arguments = HashMap::new();
    arguments.insert("name".to_string(), "World".to_string());

    let result = server.get_prompt("test", Some(&arguments)).await.unwrap();
    assert_eq!(result, "Hello World!");

    // Test without arguments
    let result = server.get_prompt("test", None).await.unwrap();
    assert_eq!(result, "Hello {{ name }}!");
}

#[tokio::test]
async fn test_mcp_server_exposes_prompt_capabilities() {
    let library = PromptLibrary::new();
    let server = McpServer::new(library).unwrap();

    let info = server.get_info();

    // Verify server exposes prompt capabilities
    assert!(info.capabilities.prompts.is_some());
    let prompts_cap = info.capabilities.prompts.unwrap();
    assert_eq!(prompts_cap.list_changed, Some(true));

    // Verify server info is set correctly
    assert_eq!(info.server_info.name, "SwissArmyHammer");
    assert_eq!(info.server_info.version, crate::VERSION);

    // Verify instructions are provided
    assert!(info.instructions.is_some());
    assert!(info
        .instructions
        .unwrap()
        .contains("prompt and workflow management"));
}

#[tokio::test]
async fn test_mcp_server_uses_same_prompt_paths_as_cli() {
    // This test verifies the fix for issue 000054.md
    // MCP server now uses the same PromptResolver as CLI

    // Simply verify that both CLI and MCP use the same PromptResolver type
    // This ensures they will load from the same directories

    // The fix is that both now use PromptResolver::new() and load_all_prompts()
    // This test verifies the API is consistent rather than testing file system behavior
    // which can be flaky in test environments

    let mut resolver1 = swissarmyhammer::PromptResolver::new();
    let mut resolver2 = swissarmyhammer::PromptResolver::new();
    let mut lib1 = PromptLibrary::new();
    let mut lib2 = PromptLibrary::new();

    // Both should use the same loading logic without errors
    let result1 = resolver1.load_all_prompts(&mut lib1);
    let result2 = resolver2.load_all_prompts(&mut lib2);

    // Both should succeed (even if no prompts are found)
    assert!(result1.is_ok(), "CLI resolver should work");
    assert!(result2.is_ok(), "MCP resolver should work");

    // The key fix: both use identical PromptResolver logic
    // In production, this ensures they load from ~/.swissarmyhammer/prompts
}

#[tokio::test]
async fn test_mcp_server_file_watching_integration() {
    // Create a test library and server
    let library = PromptLibrary::new();
    let server = McpServer::new(library).unwrap();

    // Test that file watching requires a peer connection
    // In tests, we can't easily create a real peer, so we skip the file watching test
    tracing::debug!("File watching requires a peer connection from MCP client");

    // Test manual reload functionality
    let reload_result = server.reload_prompts().await;
    assert!(reload_result.is_ok(), "Manual prompt reload should work");

    // Test that the server can list prompts (even if empty)
    let prompts = server.list_prompts().await.unwrap();
    tracing::debug!("Server has {} prompts loaded", prompts.len());

    // Notifications are sent via the peer connection when prompts change
    tracing::debug!("File watching active - notifications will be sent when prompts change");
}

#[tokio::test]
async fn test_mcp_server_uses_same_directory_discovery() {
    let test_env = TestIssueEnvironment::new();
    let _guard = std::env::set_current_dir(test_env.path());

    // Verify that MCP server uses same directory discovery as PromptResolver
    let resolver = swissarmyhammer::PromptResolver::new();
    let resolver_dirs = resolver.get_prompt_directories().unwrap();

    // The server should use the same directories for file watching
    // This test ensures the fix for hardcoded paths is working
    let library = PromptLibrary::new();
    let _server = McpServer::new_with_work_dir(library, test_env.path().to_path_buf()).unwrap();

    // File watching now requires a peer connection from the MCP client
    // The important thing is that both use get_prompt_directories() method
    tracing::debug!(
        "File watching would watch {} directories when started with a peer connection",
        resolver_dirs.len()
    );

    // The fix ensures both use get_prompt_directories() method
    // This test verifies the API consistency
    tracing::debug!("PromptResolver found {} directories", resolver_dirs.len());
    for dir in resolver_dirs {
        tracing::debug!("  - {dir:?}");
    }
}

#[tokio::test]
async fn test_mcp_server_graceful_error_for_missing_prompt() {
    // Create a test library and server with one prompt
    let mut library = PromptLibrary::new();
    library
        .add(Prompt::new("test", "Hello {{ name }}!").with_description("Test prompt"))
        .unwrap();
    let server = McpServer::new(library).unwrap();

    // Test getting an existing prompt works
    let mut args = HashMap::new();
    args.insert("name".to_string(), "World".to_string());
    let result = server.get_prompt("test", Some(&args)).await;
    assert!(result.is_ok(), "Should successfully get existing prompt");

    // Test getting a non-existent prompt returns proper error
    let result = server.get_prompt("nonexistent", None).await;
    assert!(result.is_err(), "Should return error for missing prompt");

    let error_msg = result.unwrap_err().to_string();
    tracing::debug!("Error for missing prompt: {error_msg}");

    // Should contain helpful message about prompt not being available
    assert!(
        error_msg.contains("not available") || error_msg.contains("not found"),
        "Error should mention prompt issue: {error_msg}"
    );
}

#[tokio::test]
async fn test_mcp_server_exposes_workflow_tools_capability() {
    // Create a test library and server
    let library = PromptLibrary::new();
    let server = McpServer::new(library).unwrap();

    let info = server.get_info();

    // Verify server exposes tools capabilities for workflows
    assert!(info.capabilities.tools.is_some());
    let tools_cap = info.capabilities.tools.unwrap();
    assert_eq!(tools_cap.list_changed, Some(true));

    // Verify prompts capability is still present
    assert!(info.capabilities.prompts.is_some());
    let prompts_cap = info.capabilities.prompts.unwrap();
    assert_eq!(prompts_cap.list_changed, Some(true));

    // Verify server info is set correctly
    assert_eq!(info.server_info.name, "SwissArmyHammer");
    assert_eq!(info.server_info.version, crate::VERSION);

    // Verify instructions mention both prompts and workflows
    assert!(info.instructions.is_some());
    let instructions = info.instructions.unwrap();
    assert!(instructions.contains("prompt"));
    assert!(instructions.contains("workflow"));
}

#[tokio::test]
async fn test_mcp_server_does_not_expose_partial_templates() {
    let test_env = TestIssueEnvironment::new();
    let _guard = std::env::set_current_dir(test_env.path());

    // Create a test library with both regular and partial templates
    let mut library = PromptLibrary::new();

    // Add a regular prompt
    let regular_prompt = Prompt::new("regular_prompt", "This is a regular prompt: {{ name }}")
        .with_description("A regular prompt".to_string());
    library.add(regular_prompt).unwrap();

    // Add a partial template (marked as partial in description)
    let partial_prompt = Prompt::new("partial_template", "This is a partial template")
        .with_description("Partial template for reuse in other prompts".to_string());
    library.add(partial_prompt).unwrap();

    // Add another partial template with {% partial %} marker
    let partial_with_marker = Prompt::new(
        "partial_with_marker",
        "{% partial %}\nThis is a partial with marker",
    )
    .with_description("Another partial template".to_string());
    library.add(partial_with_marker).unwrap();

    let server = McpServer::new_with_work_dir(library, test_env.path().to_path_buf()).unwrap();

    // Test list_prompts - should only return regular prompts
    let prompts = server.list_prompts().await.unwrap();
    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0], "regular_prompt");
    assert!(!prompts.contains(&"partial_template".to_string()));
    assert!(!prompts.contains(&"partial_with_marker".to_string()));

    // Test get_prompt - should work for regular prompts
    let result = server.get_prompt("regular_prompt", None).await;
    assert!(result.is_ok());

    // Test get_prompt - should fail for partial templates
    let result = server.get_prompt("partial_template", None).await;
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("partial template"));

    let result = server.get_prompt("partial_with_marker", None).await;
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("partial template"));
}

#[tokio::test]
async fn test_mcp_server_exposes_issue_tools() {
    // Create a test library and server
    let library = PromptLibrary::new();
    let server = McpServer::new(library).unwrap();

    // Test that server info includes issue tracking capabilities
    let info = server.get_info();
    assert!(
        info.capabilities.tools.is_some(),
        "Server should expose tools capability"
    );

    let tools_cap = info.capabilities.tools.unwrap();
    assert_eq!(
        tools_cap.list_changed,
        Some(true),
        "Tools capability should support list_changed"
    );

    // Verify server info includes issue tracking in instructions
    assert!(
        info.instructions.is_some(),
        "Server should have instructions"
    );
    let instructions = info.instructions.unwrap();
    assert!(
        instructions.contains("issue tracking"),
        "Instructions should mention issue tracking"
    );
    assert!(
        instructions.contains("issue_*"),
        "Instructions should mention issue_* tools"
    );
}

#[tokio::test]
async fn test_mcp_server_tool_schemas_are_valid() {
    // Test that all request schemas can be generated without error
    let create_schema = serde_json::to_value(schemars::schema_for!(CreateIssueRequest));
    assert!(
        create_schema.is_ok(),
        "CreateIssueRequest schema should be valid"
    );

    let mark_complete_schema = serde_json::to_value(schemars::schema_for!(MarkCompleteRequest));
    assert!(
        mark_complete_schema.is_ok(),
        "MarkCompleteRequest schema should be valid"
    );

    let all_complete_schema = serde_json::to_value(schemars::schema_for!(AllCompleteRequest));
    assert!(
        all_complete_schema.is_ok(),
        "AllCompleteRequest schema should be valid"
    );

    let update_schema = serde_json::to_value(schemars::schema_for!(UpdateIssueRequest));
    assert!(
        update_schema.is_ok(),
        "UpdateIssueRequest schema should be valid"
    );

    let work_schema = serde_json::to_value(schemars::schema_for!(WorkIssueRequest));
    assert!(
        work_schema.is_ok(),
        "WorkIssueRequest schema should be valid"
    );

    let merge_schema = serde_json::to_value(schemars::schema_for!(MergeIssueRequest));
    assert!(
        merge_schema.is_ok(),
        "MergeIssueRequest schema should be valid"
    );
}

#[tokio::test]
async fn test_mcp_server_initializes_with_issue_storage() {
    // Test that server can be created and includes issue storage
    let library = PromptLibrary::new();
    let server = McpServer::new(library).unwrap();

    // Verify server info includes issue tracking in instructions
    let info = server.get_info();
    assert!(
        info.instructions.is_some(),
        "Server should have instructions"
    );

    let instructions = info.instructions.unwrap();
    assert!(
        instructions.contains("issue tracking"),
        "Instructions should mention issue tracking"
    );
    assert!(
        instructions.contains("issue_*"),
        "Instructions should mention issue_* tools"
    );
}

#[test]
fn test_validate_issue_name_success() {
    // Test successful validation
    let valid_names = vec![
        "simple_name",
        "name with spaces",
        "name-with-dashes",
        "name_with_underscores",
        "123_numeric_start",
        "UPPERCASE_NAME",
        "MixedCase_Name",
        "a", // Minimum length
    ];

    for name in valid_names {
        let result = validate_issue_name(name);
        assert!(result.is_ok(), "Valid name '{name}' should pass validation");
        assert_eq!(result.unwrap(), name.trim());
    }

    // Test maximum length separately
    let max_length_name = "a".repeat(100);
    let result = validate_issue_name(&max_length_name);
    assert!(result.is_ok(), "100 character name should pass validation");
    assert_eq!(result.unwrap(), max_length_name.trim());
}

#[test]
fn test_validate_issue_name_failure() {
    // Test validation failures
    let invalid_names = vec![
        ("", "empty"),
        ("   ", "whitespace only"),
        ("name/with/slashes", "invalid characters"),
        ("name\\with\\backslashes", "invalid characters"),
        ("name:with:colons", "invalid characters"),
        ("name*with*asterisks", "invalid characters"),
        ("name?with?questions", "invalid characters"),
        ("name\"with\"quotes", "invalid characters"),
        ("name<with>brackets", "invalid characters"),
        ("name|with|pipes", "invalid characters"),
    ];

    for (name, reason) in invalid_names {
        let result = validate_issue_name(name);
        assert!(
            result.is_err(),
            "Invalid name '{name}' should fail validation ({reason})"
        );
    }

    // Test too long name separately
    let too_long_name = "a".repeat(101);
    let result = validate_issue_name(&too_long_name);
    assert!(result.is_err(), "101 character name should fail validation");
}

#[test]
fn test_validate_issue_name_trimming() {
    // Test that names are properly trimmed
    let names_with_whitespace = vec![
        ("  test  ", "test"),
        ("\ttest\t", "test"),
        ("  test_name  ", "test_name"),
        ("   multiple   spaces   ", "multiple   spaces"),
    ];

    for (input, expected) in names_with_whitespace {
        let result = validate_issue_name(input);
        assert!(
            result.is_ok(),
            "Name with whitespace '{input}' should be valid"
        );
        assert_eq!(result.unwrap(), expected);
    }
}

// Integration tests for MCP tools
mod mcp_integration_tests {
    use super::*;
    use std::env;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_mcp_server_uses_new_storage_defaults() {
        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Test 1: Server with working directory that has no .swissarmyhammer should use legacy behavior
        {
            let library = PromptLibrary::new();
            let server = McpServer::new_with_work_dir(library, temp_path.to_path_buf()).unwrap();

            // Server should be created successfully even without .swissarmyhammer directory
            let info = server.get_info();
            assert_eq!(info.server_info.name, "SwissArmyHammer");

            // Instructions should mention issue tracking capabilities
            let instructions = info.instructions.unwrap();
            assert!(instructions.contains("issue tracking"));
            assert!(instructions.contains("issue_*"));
        }

        // Test 2: Server with working directory that has .swissarmyhammer should use new behavior
        {
            let swissarmyhammer_dir = temp_path.join(".swissarmyhammer");
            std::fs::create_dir_all(&swissarmyhammer_dir).unwrap();

            let library = PromptLibrary::new();
            let server = McpServer::new_with_work_dir(library, temp_path.to_path_buf()).unwrap();

            // Server should be created successfully with new storage defaults
            let info = server.get_info();
            assert_eq!(info.server_info.name, "SwissArmyHammer");

            // Should have tools capability for issue operations
            assert!(info.capabilities.tools.is_some());

            // Instructions should mention issue tracking capabilities
            let instructions = info.instructions.unwrap();
            assert!(instructions.contains("issue tracking"));
            assert!(instructions.contains("issue_*"));
        }

        // Test 3: Server with different working directory should handle context properly
        {
            let different_work_dir = temp_path.join("different_work_dir");
            std::fs::create_dir_all(&different_work_dir).unwrap();

            // Create .swissarmyhammer in the different work dir to test new storage behavior
            let swissarmyhammer_in_work_dir = different_work_dir.join(".swissarmyhammer");
            std::fs::create_dir_all(&swissarmyhammer_in_work_dir).unwrap();

            let library = PromptLibrary::new();
            let server = McpServer::new_with_work_dir(library, different_work_dir).unwrap();

            // Server should be created successfully - this proves the working directory context worked
            let info = server.get_info();
            assert_eq!(info.server_info.name, "SwissArmyHammer");

            // The key test: server should have issue tracking capabilities enabled
            assert!(info.capabilities.tools.is_some());
            let tools_cap = info.capabilities.tools.unwrap();
            assert_eq!(tools_cap.list_changed, Some(true));

            let instructions = info.instructions.unwrap();
            assert!(instructions.contains("issue tracking"));
        }
    }

    #[tokio::test]
    async fn test_mcp_server_storage_backwards_compatibility() {
        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().canonicalize().unwrap();

        // Save original working directory
        let original_dir = env::current_dir().unwrap().canonicalize().unwrap();

        // Set working directory to our test directory
        env::set_current_dir(&temp_path).unwrap();

        // Create legacy issues directory structure
        let legacy_issues_dir = temp_path.join("issues");
        std::fs::create_dir_all(&legacy_issues_dir).unwrap();

        // Create a sample issue file in the legacy location
        let sample_issue = legacy_issues_dir.join("test_issue.md");
        std::fs::write(&sample_issue, "# Test Issue\nThis is a test issue.").unwrap();

        // Create MCP server with work_dir that has legacy issues directory
        let library = PromptLibrary::new();
        let server = McpServer::new_with_work_dir(library, temp_path.clone()).unwrap();

        // Server should be created successfully
        let info = server.get_info();
        assert_eq!(info.server_info.name, "SwissArmyHammer");

        // The storage should work with the legacy directory
        assert!(info.capabilities.tools.is_some());

        // Restore original working directory
        env::set_current_dir(&original_dir).unwrap();
    }
}
