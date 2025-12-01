//! Test that proves --agent override actually works (or doesn't!)

use std::fs;
use swissarmyhammer_prompts::PromptLibrary;
use swissarmyhammer_tools::mcp::server::McpServer;
use tempfile::TempDir;

#[tokio::test]
async fn test_agent_override_is_passed_to_server() {
    let temp_dir = TempDir::new().unwrap();

    // Test 1: Create server WITHOUT override
    let server1 = McpServer::new_with_work_dir(
        PromptLibrary::new(),
        temp_dir.path().to_path_buf(),
        None, // No override
    )
    .await;

    match server1 {
        Ok(_) => eprintln!("✓ Server created without override"),
        Err(e) => eprintln!("✗ Server creation failed: {}", e),
    }

    // Test 2: Create server WITH override
    let server2 = McpServer::new_with_work_dir(
        PromptLibrary::new(),
        temp_dir.path().to_path_buf(),
        Some("qwen-coder-flash".to_string()), // With override
    )
    .await;

    match server2 {
        Ok(_) => {
            eprintln!("✓ Server created with qwen-coder-flash override");
            eprintln!("  BUT: Did it actually use that agent?");
            eprintln!("  We need to check the use_case_agents map!");
        }
        Err(e) => eprintln!("✗ Server creation with override failed: {}", e),
    }

    // TODO: Actually invoke a rule check and verify which agent was used
    // The server needs to expose its use_case_agents map or we need to
    // call a tool and inspect what agent it uses
}

#[tokio::test]
async fn test_agent_override_with_invalid_agent() {
    let temp_dir = TempDir::new().unwrap();

    // Try to create server with non-existent agent
    let result = McpServer::new_with_work_dir(
        PromptLibrary::new(),
        temp_dir.path().to_path_buf(),
        Some("non-existent-agent".to_string()),
    )
    .await;

    match result {
        Ok(_) => {
            panic!("Should have failed with invalid agent name!");
        }
        Err(e) => {
            eprintln!("✓ Correctly failed with invalid agent: {}", e);
            assert!(e.to_string().contains("non-existent-agent"));
        }
    }
}

#[tokio::test]
async fn test_prove_agent_override_bug() {
    eprintln!("\n=== Testing actual rule check invocation ===\n");

    let temp_dir = TempDir::new().unwrap();
    let sah_dir = temp_dir.path().join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir).unwrap();

    // Create a simple rule
    let rules_dir = sah_dir.join("rules");
    fs::create_dir_all(&rules_dir).unwrap();
    fs::write(
        rules_dir.join("test-rule.md"),
        "---\nseverity: warning\n---\nCheck for the word TODO",
    )
    .unwrap();

    // Create a test file
    let test_file = temp_dir.path().join("test.rs");
    fs::write(&test_file, "// TODO: fix this\n").unwrap();

    // Create server with qwen-coder-flash override
    let server = McpServer::new_with_work_dir(
        PromptLibrary::new(),
        temp_dir.path().to_path_buf(),
        Some("qwen-coder-flash".to_string()),
    )
    .await;

    match server {
        Ok(_server) => {
            eprintln!("✓ Server created with agent override");
            eprintln!("\nNow we need to:");
            eprintln!("1. Get the tool registry from the server");
            eprintln!("2. Call the rules_check tool");
            eprintln!("3. Intercept/inspect which agent executor was created");
            eprintln!("4. Assert it's LlamaAgent (qwen), not ClaudeCode");
            eprintln!("\nBUT: McpServer doesn't expose its internals for testing!");
        }
        Err(e) => {
            panic!("Server creation failed: {}", e);
        }
    }
}
