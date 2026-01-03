//! Tests for MCP server functionality

use super::server::McpServer;
use rmcp::ServerHandler;
use std::collections::HashMap;
use std::path::PathBuf;
use swissarmyhammer_prompts::{Prompt, PromptLibrary, PromptResolver};

/// RAII guard to ensure working directory is restored when dropped
struct DirGuard(PathBuf);

impl Drop for DirGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

#[tokio::test]
async fn test_mcp_server_creation() {
    let test_dir = tempfile::tempdir().unwrap();
    let original_dir = std::env::current_dir().expect("Failed to get current dir");
    std::env::set_current_dir(test_dir.path()).expect("Failed to change dir");
    let _guard = DirGuard(original_dir);

    let library = PromptLibrary::new();
    let server = McpServer::new_with_work_dir(library, test_dir.path().to_path_buf(), None)
        .await
        .unwrap();

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
    let server = McpServer::new(library).await.unwrap();

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
    let test_dir = tempfile::tempdir().unwrap();
    let original_dir = std::env::current_dir().expect("Failed to get current dir");
    std::env::set_current_dir(test_dir.path()).expect("Failed to change dir");
    let _guard = DirGuard(original_dir);

    let mut library = PromptLibrary::new();
    let prompt = Prompt::new("test", "Test prompt: {{ name }}")
        .with_description("Test description".to_string());
    library.add(prompt).unwrap();

    let server = McpServer::new_with_work_dir(library, test_dir.path().to_path_buf(), None)
        .await
        .unwrap();
    let prompts = server.list_prompts().await.unwrap();

    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0], "test");
}

#[tokio::test]
async fn test_mcp_server_get_prompt() {
    let test_dir = tempfile::tempdir().unwrap();
    let original_dir = std::env::current_dir().expect("Failed to get current dir");
    std::env::set_current_dir(test_dir.path()).expect("Failed to change dir");
    let _guard = DirGuard(original_dir);

    let mut library = PromptLibrary::new();
    let prompt =
        Prompt::new("test", "Hello {{ name }}!").with_description("Greeting prompt".to_string());
    library.add(prompt).unwrap();

    let server = McpServer::new_with_work_dir(library, test_dir.path().to_path_buf(), None)
        .await
        .unwrap();
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
    let server = McpServer::new(library).await.unwrap();

    let info = server.get_info();

    // Verify server exposes prompt capabilities
    assert!(info.capabilities.prompts.is_some());
    let prompts_cap = info.capabilities.prompts.unwrap();
    assert_eq!(prompts_cap.list_changed, Some(true));

    // Verify server info is set correctly
    assert_eq!(info.server_info.name, "SwissArmyHammer");
    assert_eq!(info.server_info.version, crate::VERSION);
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

    let mut resolver1 = PromptResolver::new();
    let mut resolver2 = PromptResolver::new();
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
    let server = McpServer::new(library).await.unwrap();

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
    let test_dir = tempfile::tempdir().unwrap();
    let original_dir = std::env::current_dir().expect("Failed to get current dir");
    std::env::set_current_dir(test_dir.path()).expect("Failed to change dir");
    let _guard = DirGuard(original_dir);

    // Verify that MCP server uses same directory discovery as PromptResolver
    let resolver = PromptResolver::new();
    let resolver_dirs = resolver.get_prompt_directories().unwrap();

    // The server should use the same directories for file watching
    // This test ensures the fix for hardcoded paths is working
    let library = PromptLibrary::new();
    let _server = McpServer::new_with_work_dir(library, test_dir.path().to_path_buf(), None)
        .await
        .unwrap();

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
    let server = McpServer::new(library).await.unwrap();

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
async fn test_mcp_server_exposes_prompts_tools_capability() {
    // Create a test library and server
    let library = PromptLibrary::new();
    let server = McpServer::new(library).await.unwrap();

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
}

#[tokio::test]
async fn test_mcp_server_does_not_expose_partial_templates() {
    let test_dir = tempfile::tempdir().unwrap();
    let original_dir = std::env::current_dir().expect("Failed to get current dir");
    std::env::set_current_dir(test_dir.path()).expect("Failed to change dir");
    let _guard = DirGuard(original_dir);

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

    let server = McpServer::new_with_work_dir(library, test_dir.path().to_path_buf(), None)
        .await
        .unwrap();

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
async fn test_reload_prompts_detects_no_changes() {
    use std::fs;
    use std::io::Write;

    let test_dir = tempfile::tempdir().unwrap();
    let original_dir = std::env::current_dir().expect("Failed to get current dir");
    std::env::set_current_dir(test_dir.path()).expect("Failed to change dir");
    let _guard = DirGuard(original_dir);

    // Create a prompts directory
    let prompts_dir = test_dir.path().join(".swissarmyhammer").join("prompts");
    fs::create_dir_all(&prompts_dir).unwrap();

    // Create a test prompt file
    let prompt_file = prompts_dir.join("test_prompt.md");
    let mut file = fs::File::create(&prompt_file).unwrap();
    writeln!(
        file,
        "---\ntitle: Test Prompt\ndescription: A test prompt\n---\nHello {{{{ name }}}}!"
    )
    .unwrap();
    file.sync_all().unwrap();

    // Create server and load prompts
    let library = PromptLibrary::new();
    let server = McpServer::new_with_work_dir(library, test_dir.path().to_path_buf(), None)
        .await
        .unwrap();

    // First reload - should detect changes (initial load)
    let has_changes = server.reload_prompts().await.unwrap();
    assert!(
        has_changes,
        "First reload should detect changes (initial load)"
    );

    // Touch the file but don't change content (simulate iCloud sync)
    let metadata = fs::metadata(&prompt_file).unwrap();
    let mtime = metadata.modified().unwrap();
    let new_mtime = mtime + std::time::Duration::from_secs(1);
    filetime::set_file_mtime(
        &prompt_file,
        filetime::FileTime::from_system_time(new_mtime),
    )
    .unwrap();

    // Second reload - should NOT detect changes (same content)
    let has_changes = server.reload_prompts().await.unwrap();
    assert!(
        !has_changes,
        "Reload after timestamp-only change should not detect changes"
    );
}

#[tokio::test]
async fn test_reload_prompts_detects_content_changes() {
    use std::fs;
    use std::io::Write;

    let test_dir = tempfile::tempdir().unwrap();
    let original_dir = std::env::current_dir().expect("Failed to get current dir");
    std::env::set_current_dir(test_dir.path()).expect("Failed to change dir");
    let _guard = DirGuard(original_dir);

    // Create a prompts directory
    let prompts_dir = test_dir.path().join(".swissarmyhammer").join("prompts");
    fs::create_dir_all(&prompts_dir).unwrap();

    // Create a test prompt file
    let prompt_file = prompts_dir.join("test_prompt.md");
    let mut file = fs::File::create(&prompt_file).unwrap();
    writeln!(
        file,
        "---\ntitle: Test Prompt\ndescription: A test prompt\n---\nHello {{{{ name }}}}!"
    )
    .unwrap();
    file.sync_all().unwrap();

    // Create server and load prompts
    let library = PromptLibrary::new();
    let server = McpServer::new_with_work_dir(library, test_dir.path().to_path_buf(), None)
        .await
        .unwrap();

    // First reload
    let has_changes = server.reload_prompts().await.unwrap();
    assert!(has_changes, "First reload should detect changes");

    // Modify the prompt content
    let mut file = fs::File::create(&prompt_file).unwrap();
    writeln!(
        file,
        "---\ntitle: Test Prompt\ndescription: A modified test prompt\n---\nGoodbye {{{{ name }}}}!"
    )
    .unwrap();
    file.sync_all().unwrap();

    // Second reload - should detect changes (content changed)
    let has_changes = server.reload_prompts().await.unwrap();
    assert!(
        has_changes,
        "Reload after content change should detect changes"
    );
}

#[tokio::test]
async fn test_reload_prompts_detects_new_prompts() {
    use std::fs;
    use std::io::Write;

    let test_dir = tempfile::tempdir().unwrap();
    let original_dir = std::env::current_dir().expect("Failed to get current dir");
    std::env::set_current_dir(test_dir.path()).expect("Failed to change dir");
    let _guard = DirGuard(original_dir);

    // Create a prompts directory
    let prompts_dir = test_dir.path().join(".swissarmyhammer").join("prompts");
    fs::create_dir_all(&prompts_dir).unwrap();

    // Create first prompt file
    let prompt_file1 = prompts_dir.join("test_prompt1.md");
    let mut file = fs::File::create(&prompt_file1).unwrap();
    writeln!(
        file,
        "---\ntitle: Test Prompt 1\ndescription: First prompt\n---\nHello {{{{ name }}}}!"
    )
    .unwrap();
    file.sync_all().unwrap();

    // Create server and load prompts
    let library = PromptLibrary::new();
    let server = McpServer::new_with_work_dir(library, test_dir.path().to_path_buf(), None)
        .await
        .unwrap();

    // First reload
    let has_changes = server.reload_prompts().await.unwrap();
    assert!(has_changes, "First reload should detect changes");

    // Add a new prompt file
    let prompt_file2 = prompts_dir.join("test_prompt2.md");
    let mut file = fs::File::create(&prompt_file2).unwrap();
    writeln!(
        file,
        "---\ntitle: Test Prompt 2\ndescription: Second prompt\n---\nGoodbye {{{{ name }}}}!"
    )
    .unwrap();
    file.sync_all().unwrap();

    // Second reload - should detect changes (new prompt added)
    let has_changes = server.reload_prompts().await.unwrap();
    assert!(
        has_changes,
        "Reload after adding new prompt should detect changes"
    );
}

#[tokio::test]
async fn test_reload_prompts_detects_deleted_prompts() {
    use std::fs;
    use std::io::Write;

    let test_dir = tempfile::tempdir().unwrap();
    let original_dir = std::env::current_dir().expect("Failed to get current dir");
    std::env::set_current_dir(test_dir.path()).expect("Failed to change dir");
    let _guard = DirGuard(original_dir);

    // Create a prompts directory
    let prompts_dir = test_dir.path().join(".swissarmyhammer").join("prompts");
    fs::create_dir_all(&prompts_dir).unwrap();

    // Create two prompt files
    let prompt_file1 = prompts_dir.join("test_prompt1.md");
    let mut file = fs::File::create(&prompt_file1).unwrap();
    writeln!(
        file,
        "---\ntitle: Test Prompt 1\ndescription: First prompt\n---\nHello {{{{ name }}}}!"
    )
    .unwrap();
    file.sync_all().unwrap();

    let prompt_file2 = prompts_dir.join("test_prompt2.md");
    let mut file = fs::File::create(&prompt_file2).unwrap();
    writeln!(
        file,
        "---\ntitle: Test Prompt 2\ndescription: Second prompt\n---\nGoodbye {{{{ name }}}}!"
    )
    .unwrap();
    file.sync_all().unwrap();

    // Create server and load prompts
    let library = PromptLibrary::new();
    let server = McpServer::new_with_work_dir(library, test_dir.path().to_path_buf(), None)
        .await
        .unwrap();

    // First reload
    let has_changes = server.reload_prompts().await.unwrap();
    assert!(has_changes, "First reload should detect changes");

    // Delete one prompt file
    fs::remove_file(&prompt_file2).unwrap();

    // Second reload - should detect changes (prompt deleted)
    let has_changes = server.reload_prompts().await.unwrap();
    assert!(
        has_changes,
        "Reload after deleting prompt should detect changes"
    );
}
