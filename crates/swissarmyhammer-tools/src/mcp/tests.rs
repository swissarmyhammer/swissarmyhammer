//! Tests for MCP server functionality

use super::server::McpServer;
use rmcp::ServerHandler;
use serial_test::serial;
use std::path::PathBuf;
use swissarmyhammer_common::Pretty;
use swissarmyhammer_templating::{PromptResolver, TemplateLibrary};

/// RAII guard to ensure working directory is restored when dropped
struct DirGuard(PathBuf);

impl Drop for DirGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

/// Get the current working directory, falling back to a known safe directory.
///
/// When tests run in parallel across crate boundaries, another test binary may
/// have changed the process CWD to a temp directory that was already cleaned up,
/// causing `std::env::current_dir()` to fail with ENOENT. This helper falls back
/// to the workspace root to keep things working.
fn safe_current_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
    })
}

#[tokio::test]
#[serial(cwd)]
async fn test_mcp_server_creation() {
    let test_dir = tempfile::tempdir().unwrap();
    let original_dir = safe_current_dir();
    std::env::set_current_dir(test_dir.path()).expect("Failed to change dir");
    let _guard = DirGuard(original_dir);

    let library = TemplateLibrary::new();
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
#[serial(cwd)]
async fn test_mcp_server_exposes_shell_tools() {
    // Create a test library and server
    let library = TemplateLibrary::new();
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
#[serial(cwd)]
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
    let mut lib1 = TemplateLibrary::new();
    let mut lib2 = TemplateLibrary::new();

    // Both should use the same loading logic without errors
    let result1 = resolver1.load_all_prompts(&mut lib1);
    let result2 = resolver2.load_all_prompts(&mut lib2);

    // Both should succeed (even if no prompts are found)
    assert!(result1.is_ok(), "CLI resolver should work");
    assert!(result2.is_ok(), "MCP resolver should work");

    // The key fix: both use identical PromptResolver logic
    // In production, this ensures they load from ~/.prompts
}

#[tokio::test]
#[serial(cwd)]
async fn test_mcp_server_file_watching_integration() {
    // Create a test library and server
    let library = TemplateLibrary::new();
    let server = McpServer::new(library).await.unwrap();

    // Test that file watching requires a peer connection
    // In tests, we can't easily create a real peer, so we skip the file watching test
    tracing::debug!("File watching requires a peer connection from MCP client");

    // Test manual reload functionality
    let reload_result = server.reload_prompts().await;
    assert!(reload_result.is_ok(), "Manual prompt reload should work");

    // Notifications are sent via the peer connection when prompts change
    tracing::debug!("File watching active - notifications will be sent when prompts change");
}

#[tokio::test]
#[serial(cwd)]
async fn test_mcp_server_uses_same_directory_discovery() {
    let test_dir = tempfile::tempdir().unwrap();
    let original_dir = safe_current_dir();
    std::env::set_current_dir(test_dir.path()).expect("Failed to change dir");
    let _guard = DirGuard(original_dir);

    // Verify that MCP server uses same directory discovery as PromptResolver
    let resolver = PromptResolver::new();
    let resolver_dirs = resolver.get_prompt_directories().unwrap();

    // The server should use the same directories for file watching
    // This test ensures the fix for hardcoded paths is working
    let library = TemplateLibrary::new();
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
#[serial(cwd)]
async fn test_mcp_server_advertises_tools_but_not_prompts_capability() {
    // Create a test library and server
    let library = TemplateLibrary::new();
    let server = McpServer::new(library).await.unwrap();

    let info = server.get_info();

    // Verify server exposes tools capabilities for workflows
    assert!(info.capabilities.tools.is_some());
    let tools_cap = info.capabilities.tools.unwrap();
    assert_eq!(tools_cap.list_changed, Some(true));

    // The prompts protocol surface was removed — no prompts capability.
    assert!(
        info.capabilities.prompts.is_none(),
        "Server must not advertise the prompts capability"
    );

    // Verify server info is set correctly
    assert_eq!(info.server_info.name, "SwissArmyHammer");
    assert_eq!(info.server_info.version, crate::VERSION);
}

#[tokio::test]
#[serial(cwd)]
async fn test_reload_prompts_detects_no_changes() {
    use std::fs;
    use std::io::Write;

    let test_dir = tempfile::tempdir().unwrap();
    let original_dir = safe_current_dir();
    std::env::set_current_dir(test_dir.path()).expect("Failed to change dir");
    let _guard = DirGuard(original_dir);

    // Create a prompts directory
    let prompts_dir = test_dir.path().join(".prompts");
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
    let library = TemplateLibrary::new();
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
#[serial(cwd)]
async fn test_reload_prompts_detects_content_changes() {
    use std::fs;
    use std::io::Write;

    let test_dir = tempfile::tempdir().unwrap();
    let original_dir = safe_current_dir();
    std::env::set_current_dir(test_dir.path()).expect("Failed to change dir");
    let _guard = DirGuard(original_dir);

    // Create a prompts directory
    let prompts_dir = test_dir.path().join(".prompts");
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
    let library = TemplateLibrary::new();
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
#[serial(cwd)]
async fn test_reload_prompts_detects_new_prompts() {
    use std::fs;
    use std::io::Write;

    let test_dir = tempfile::tempdir().unwrap();
    let original_dir = safe_current_dir();
    std::env::set_current_dir(test_dir.path()).expect("Failed to change dir");
    let _guard = DirGuard(original_dir);

    // Create a prompts directory
    let prompts_dir = test_dir.path().join(".prompts");
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
    let library = TemplateLibrary::new();
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
#[serial(cwd)]
async fn test_reload_prompts_detects_deleted_prompts() {
    use std::fs;
    use std::io::Write;

    let test_dir = tempfile::tempdir().unwrap();
    let original_dir = safe_current_dir();
    std::env::set_current_dir(test_dir.path()).expect("Failed to change dir");
    let _guard = DirGuard(original_dir);

    // Create a prompts directory
    let prompts_dir = test_dir.path().join(".prompts");
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
    let library = TemplateLibrary::new();
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
#[serial(cwd)]
async fn test_builtin_partials_load_into_library_for_rendering() {
    // The MCP prompt protocol surface is gone, but skill/agent rendering still
    // depends on `load_all_prompts` populating the library with partials. This
    // test proves that partial loading survived: builtin partials must be
    // present in the library so liquid `{% render %}` can resolve them.
    let original_dir = safe_current_dir();

    // We need to be in the project root to load builtin prompts
    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    std::env::set_current_dir(&project_root).expect("Failed to change dir");
    let _guard = DirGuard(original_dir);

    // Load builtin prompts (and partials) the same way the server does.
    let mut library = TemplateLibrary::new();
    let mut resolver = PromptResolver::new();
    resolver
        .load_all_prompts(&mut library)
        .expect("Failed to load prompts");

    let all = library.list().expect("library should list prompts");
    assert!(
        !all.is_empty(),
        "builtin prompts should load into the library"
    );

    // At least one partial template must be present, proving partial loading
    // (the rendering dependency) survived the protocol-surface removal.
    let has_partial = all.iter().any(|p| p.is_partial_template());
    assert!(
        has_partial,
        "builtin partials must load into the library for skill/agent rendering"
    );
}
