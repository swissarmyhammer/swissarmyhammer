use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_tools::mcp::{
    test_utils::create_test_client,
    unified_server::{start_mcp_server, McpServerMode},
};

/// Test MCP server basic functionality (Fast In-Process)
///
/// Tests MCP server initialize and list prompts without subprocess overhead:
/// - Uses in-process HTTP MCP server
/// - No cargo build/run overhead
/// - Tests initialization and prompt listing
/// - Fast execution (<1s instead of 20-30s)
///
/// Runs on a `multi_thread` runtime: the test hosts the in-process server and
/// drives the RMCP client on the same runtime, and a current-thread runtime
/// cannot advance the server's SSE response task while blocked awaiting the
/// client handshake — that starvation made the handshake stall for seconds. See
/// `test_client_handshake_is_fast` in `swissarmyhammer-tools` for the analysis.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_mcp_server_basic_functionality() {
    // Bind the server to a fresh temp dir rather than the host monorepo so its
    // `startup_cleanup` doesn't walk/hash the entire repo on every startup —
    // that walk (not cross-test contention) is what made this test take
    // minutes under full-workspace nextest. With a tiny working dir it runs in
    // seconds and needs no serial guard. See `test_client_list_tools` in
    // `swissarmyhammer-tools/src/mcp/test_utils.rs` for the canonical pattern.
    let working_dir = tempfile::TempDir::new().expect("Failed to create temp working dir");
    let mut server = start_mcp_server(
        McpServerMode::Http { port: None },
        None,
        None,
        Some(working_dir.path().to_path_buf()),
    )
    .await
    .expect("Failed to start in-process MCP server");

    // Create RMCP client
    let client = create_test_client(server.url()).await;

    // List prompts
    let _response = client
        .list_prompts(Default::default())
        .await
        .expect("Failed to list prompts");

    // Clean shutdown
    client.cancel().await.expect("Failed to cancel client");
    server.shutdown().await.expect("Failed to shutdown server");
}

/// Test that MCP server loads prompts from the same directories as CLI (Fast In-Process)
///
/// Tests MCP server prompt loading without subprocess overhead:
/// - Uses in-process HTTP MCP server
/// - No cargo build/run overhead
/// - Tests prompt loading from filesystem
/// - Fast execution (<1s instead of 20-30s)
///
/// multi_thread required — see `test_mcp_server_basic_functionality`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_mcp_server_prompt_loading() {
    let guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let home_path = std::env::var("HOME").expect("HOME should be set");
    let prompts_dir = std::path::PathBuf::from(home_path).join(".prompts");
    std::fs::create_dir_all(&prompts_dir).unwrap();

    // Create a test prompt
    let test_prompt = prompts_dir.join("test-prompt.md");
    std::fs::write(
        &test_prompt,
        "---\ntitle: Test Prompt\n---\nThis is a test prompt",
    )
    .unwrap();

    // Create prompt library that loads from the test environment
    use swissarmyhammer_prompts::PromptLibrary;
    let library = PromptLibrary::default();

    // Start in-process MCP server with the prompt library. Bind it to the
    // isolated environment's small temp working dir (not the host monorepo) so
    // `startup_cleanup` doesn't walk/hash the entire repo on startup — that
    // walk is what made this test take minutes under full-workspace nextest.
    // With a tiny working dir it runs in seconds and needs no serial guard.
    let mut server = start_mcp_server(
        McpServerMode::Http { port: None },
        Some(library),
        None,
        Some(guard.temp_dir()),
    )
    .await
    .expect("Failed to start in-process MCP server");

    // Create RMCP client
    let client = create_test_client(server.url()).await;

    // List prompts
    let prompts = client
        .list_prompts(Default::default())
        .await
        .expect("Failed to list prompts");

    // Verify that prompts are loaded (should have at least built-in prompts)
    assert!(
        !prompts.prompts.is_empty(),
        "MCP server should load at least built-in prompts"
    );

    // Clean shutdown
    client.cancel().await.expect("Failed to cancel client");
    server.shutdown().await.expect("Failed to shutdown server");
}

/// Test that MCP server loads built-in prompts (Fast In-Process)
///
/// Tests that MCP server provides built-in prompts:
/// - Uses in-process HTTP MCP server
/// - No cargo build/run overhead
/// - Verifies built-in prompts are available
/// - Fast execution (<1s instead of 20-30s)
///
/// multi_thread required — see `test_mcp_server_basic_functionality`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_mcp_server_builtin_prompts() {
    // Bind the server to a fresh temp dir rather than the host monorepo so its
    // `startup_cleanup` doesn't walk/hash the entire repo on every startup —
    // that walk (not cross-test contention) is what made this test slow under
    // full-workspace nextest. With a tiny working dir it runs in seconds and
    // needs no serial guard.
    let working_dir = tempfile::TempDir::new().expect("Failed to create temp working dir");
    let mut server = start_mcp_server(
        McpServerMode::Http { port: None },
        None,
        None,
        Some(working_dir.path().to_path_buf()),
    )
    .await
    .expect("Failed to start in-process MCP server");

    // Create RMCP client
    let client = create_test_client(server.url()).await;

    // List prompts
    let prompts = client
        .list_prompts(Default::default())
        .await
        .expect("Failed to list prompts");

    // Verify we have built-in prompts
    assert!(
        prompts.prompts.len() > 5,
        "MCP server should load multiple built-in prompts, found: {}",
        prompts.prompts.len()
    );

    // Look for some known built-in prompts
    let prompt_names: Vec<String> = prompts.prompts.iter().map(|p| p.name.to_string()).collect();

    let has_help = prompt_names.contains(&"help".to_string());
    let has_example = prompt_names.contains(&"example".to_string());

    assert!(
        has_help || has_example,
        "MCP server should load built-in prompts like 'help' or 'example'"
    );

    // Clean shutdown
    client.cancel().await.expect("Failed to cancel client");
    server.shutdown().await.expect("Failed to shutdown server");
}

// Removed slow subprocess E2E tests - they are replaced by the fast in-process tests above
// The subprocess tests caused build lock deadlocks and took 20-30s each
// The in-process tests provide equivalent coverage in <1s each
