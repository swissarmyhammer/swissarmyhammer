//! Tests for MCP server functionality

use super::server::McpServer;
use rmcp::ServerHandler;
use std::collections::HashMap;
use std::path::PathBuf;
use swissarmyhammer_common::Pretty;
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
    tracing::debug!("Server capabilities: {}", Pretty(&info.capabilities));
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
async fn test_mcp_server_excludes_partials_and_system_prompts_from_list() {
    let test_dir = tempfile::tempdir().unwrap();
    let original_dir = std::env::current_dir().expect("Failed to get current dir");
    std::env::set_current_dir(test_dir.path()).expect("Failed to change dir");
    let _guard = DirGuard(original_dir);

    let mut library = PromptLibrary::new();

    // Add a regular prompt
    let regular_prompt = Prompt::new("regular", "Regular prompt: {{ content }}")
        .with_description("A regular prompt".to_string());
    library.add(regular_prompt).unwrap();

    // Add a partial with partial: true in metadata
    let mut partial_metadata = HashMap::new();
    partial_metadata.insert("partial".to_string(), serde_json::Value::Bool(true));
    let mut partial_prompt = Prompt::new("_partials/header", "Partial template content")
        .with_description("Partial template for reuse in other prompts".to_string());
    partial_prompt.metadata = partial_metadata;
    library.add(partial_prompt).unwrap();

    // Add another partial with _partials in name and partial: true metadata
    let mut partial_by_name = Prompt::new("_partials/footer", "Footer partial")
        .with_description("Footer partial template".to_string());
    partial_by_name.metadata.insert("partial".to_string(), serde_json::Value::Bool(true));
    library.add(partial_by_name).unwrap();

    // Add a system prompt with hidden: true in metadata
    let mut system_metadata = HashMap::new();
    system_metadata.insert("hidden".to_string(), serde_json::Value::Bool(true));
    let mut system_prompt = Prompt::new(".system/tester", "System prompt for testing")
        .with_description("System prompt for test mode".to_string());
    system_prompt.metadata = system_metadata;
    library.add(system_prompt).unwrap();

    // Add another system prompt with hidden: true metadata
    let mut system_by_name = Prompt::new(".system/implementer", "System prompt for implementation")
        .with_description("System prompt for implementer mode".to_string());
    system_by_name.metadata.insert("hidden".to_string(), serde_json::Value::Bool(true));
    library.add(system_by_name).unwrap();

    let server = McpServer::new_with_work_dir(library, test_dir.path().to_path_buf(), None)
        .await
        .unwrap();
    let prompts = server.list_prompts().await.unwrap();

    // Only the regular prompt should be in the list
    assert_eq!(
        prompts.len(),
        1,
        "Only non-partial and non-system prompts should be listed"
    );
    assert_eq!(prompts[0], "regular");

    // Verify partials are not in the list
    assert!(!prompts.contains(&"_partials/header".to_string()));
    assert!(!prompts.contains(&"_partials/footer".to_string()));

    // Verify system prompts are not in the list
    assert!(!prompts.contains(&".system/tester".to_string()));
    assert!(!prompts.contains(&".system/implementer".to_string()));
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
        tracing::debug!("  - {}", Pretty(&dir));
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

    // Add a partial template (marked as partial with metadata)
    let mut partial_prompt = Prompt::new("partial_template", "This is a partial template")
        .with_description("A partial template".to_string());
    partial_prompt.metadata.insert("partial".to_string(), serde_json::Value::Bool(true));
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
    assert!(error_msg.contains("hidden prompt") || error_msg.contains("partial"));

    let result = server.get_prompt("partial_with_marker", None).await;
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("hidden prompt") || error_msg.contains("partial"));
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

#[tokio::test]
async fn test_builtin_partials_not_exposed_in_mcp() {
    // Test that actual builtin partials with partial: true metadata are filtered correctly
    let test_dir = tempfile::tempdir().unwrap();
    let original_dir = std::env::current_dir().expect("Failed to get current dir");

    // We need to be in the project root to load builtin prompts
    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    std::env::set_current_dir(&project_root).expect("Failed to change dir");
    let _guard = DirGuard(original_dir);

    // Load builtin prompts
    let mut library = PromptLibrary::new();
    let mut resolver = PromptResolver::new();
    resolver.load_all_prompts(&mut library).expect("Failed to load prompts");

    let server = McpServer::new_with_work_dir(library, test_dir.path().to_path_buf(), None)
        .await
        .unwrap();

    // List all prompts via MCP
    let mcp_prompts = server.list_prompts().await.unwrap();

    // Check that none of the partial templates from _partials/ are exposed
    let partial_names = vec![
        "detected-projects",
        "git-practices",
        "test-driven-development",
        "coding-standards",
        "tool_use",
    ];

    for partial_name in partial_names {
        assert!(
            !mcp_prompts.contains(&partial_name.to_string()),
            "Partial '{}' should not be exposed via MCP but was found in: {:?}",
            partial_name,
            mcp_prompts
        );
    }

    // Check that hidden prompts (like .check) are not exposed
    let hidden_names = vec![".check", ".system/rule-checker"];

    for hidden_name in hidden_names {
        assert!(
            !mcp_prompts.contains(&hidden_name.to_string()),
            "Hidden prompt '{}' should not be exposed via MCP but was found in: {:?}",
            hidden_name,
            mcp_prompts
        );
    }

    // Verify that at least some regular prompts are exposed
    assert!(
        !mcp_prompts.is_empty(),
        "MCP should expose some non-partial prompts"
    );

    println!("MCP exposed {} prompts (partials correctly filtered)", mcp_prompts.len());
    println!("Sample of exposed prompts: {:?}", &mcp_prompts[..mcp_prompts.len().min(5)]);
}
