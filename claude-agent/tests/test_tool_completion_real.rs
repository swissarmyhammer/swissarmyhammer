//! Integration test proving tool completion notifications work
//!
//! This test uses REAL claude-agent with REAL claude code to execute a file read
//! and verifies that we receive ToolCallUpdate(Completed) notifications.
//!
//! ⚠️ MANUAL VERIFICATION ONLY - Use test_tool_completion_recorded.rs for CI
//!
//! This test is kept for manual verification and re-recording fixtures.
//! It makes real API calls and may fail due to rate limits.
//!
//! To use this test for re-recording:
//! 1. Ensure ANTHROPIC_API_KEY is set
//! 2. Run: cargo test test_read_cargo_toml_gets_completion_notification --ignored -- --nocapture
//! 3. Capture the notification output
//! 4. Update fixtures/tool_completion_response.json

use agent_client_protocol::{
    Agent, ClientCapabilities, FileSystemCapability, InitializeRequest, NewSessionRequest,
    PromptRequest, SessionUpdate,
};
use claude_agent::{AgentConfig, ClaudeAgent};

#[tokio::test]
#[ignore = "Manual verification only - makes real API calls. Use test_tool_completion_recorded.rs for CI"]
async fn test_read_cargo_toml_gets_completion_notification() {
    let local = tokio::task::LocalSet::new();

    local
        .run_until(async {
            // Create real ClaudeAgent with default config
            let config = AgentConfig::default();
            let (agent, mut notification_rx) = match ClaudeAgent::new(config).await {
                Ok(result) => result,
                Err(_) => {
                    eprintln!("⚠ Test requires claude authentication");
                    return;
                }
            };

            // Initialize
            let fs_cap = FileSystemCapability::new()
                .read_text_file(true)
                .write_text_file(true);

            let mut meta = std::collections::HashMap::new();
            meta.insert("streaming".to_string(), serde_json::Value::Bool(true));

            let meta_map: serde_json::Map<String, serde_json::Value> = meta.into_iter().collect();
            let client_capabilities = ClientCapabilities::new()
                .fs(fs_cap)
                .terminal(false)
                .meta(meta_map);

            let init_request = InitializeRequest::new(agent_client_protocol::ProtocolVersion::V1)
                .client_capabilities(client_capabilities);

            if agent.initialize(init_request).await.is_err() {
                eprintln!("⚠ Test requires claude");
                return;
            }

            // Create session
            let cwd = std::env::current_dir().unwrap();
            let session_request = NewSessionRequest::new(cwd);
            let session_response = match agent.new_session(session_request).await {
                Ok(resp) => resp,
                Err(_) => {
                    eprintln!("⚠ Session creation failed");
                    return;
                }
            };

            // Collect notifications
            let (notif_tx, mut notif_rx) = tokio::sync::mpsc::unbounded_channel();
            tokio::task::spawn_local(async move {
                while let Ok(n) = notification_rx.recv().await {
                    let _ = notif_tx.send(n);
                }
            });

            // THE KEY TEST: Ask claude to read Cargo.toml
            eprintln!("\n📤 Sending: 'Read the Cargo.toml file'");
            let text_content = agent_client_protocol::TextContent::new(
                "Read the Cargo.toml file and tell me the package name".to_string(),
            );
            let content_block = agent_client_protocol::ContentBlock::Text(text_content);
            let prompt_request =
                PromptRequest::new(session_response.session_id.clone(), vec![content_block]);
            let prompt_result = agent.prompt(prompt_request).await;

            if prompt_result.is_err() {
                eprintln!("⚠ Prompt failed");
                return;
            }

            // Wait for all notifications
            eprintln!("⏳ Waiting for notifications...");
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;

            // Collect
            let mut notifications = Vec::new();
            while let Ok(n) = notif_rx.try_recv() {
                notifications.push(n);
            }

            eprintln!("\n📊 Received {} notifications total", notifications.len());

            // Analyze
            let mut tool_calls = Vec::new();
            let mut tool_updates = Vec::new();
            let mut tool_completions = Vec::new();

            for (idx, n) in notifications.iter().enumerate() {
                match &n.update {
                    SessionUpdate::ToolCall(tc) => {
                        eprintln!(
                            "[{}] 🔧 ToolCall: '{}' status={:?}",
                            idx, tc.title, tc.status
                        );
                        tool_calls.push(tc.clone());
                    }
                    SessionUpdate::ToolCallUpdate(update) => {
                        let status_str = update
                            .fields
                            .status
                            .as_ref()
                            .map(|s| format!("{:?}", s))
                            .unwrap_or_else(|| "None".to_string());
                        eprintln!(
                            "[{}] 🔄 ToolCallUpdate: {} status={}",
                            idx, update.tool_call_id, status_str
                        );
                        tool_updates.push(update.clone());

                        if let Some(status) = &update.fields.status {
                            if matches!(status, agent_client_protocol::ToolCallStatus::Completed) {
                                eprintln!("  ✅ COMPLETION FOUND");
                                tool_completions.push(update.clone());
                            }
                        }
                    }
                    SessionUpdate::AgentMessageChunk(chunk) => {
                        if let agent_client_protocol::ContentBlock::Text(text) = &chunk.content {
                            eprintln!(
                                "[{}] 💬 Agent: '{}'",
                                idx,
                                text.text.chars().take(60).collect::<String>()
                            );
                        } else {
                            // Ignore non-text content
                        }
                    }
                    _ => {}
                }
            }

            // PROOF
            eprintln!("\n=== RESULTS ===");
            eprintln!("ToolCall notifications: {}", tool_calls.len());
            eprintln!("ToolCallUpdate notifications: {}", tool_updates.len());
            eprintln!("ToolCallUpdate(Completed): {}", tool_completions.len());

            // ASSERT
            assert!(!tool_calls.is_empty(), "Should have ToolCall notifications");

            if !tool_completions.is_empty() {
                eprintln!("\n✅ SUCCESS: Tool completion notifications ARE being sent");
                eprintln!("Completed tools:");
                for completion in &tool_completions {
                    eprintln!("  - {}", completion.tool_call_id);
                }
            } else {
                eprintln!("\n❌ FAILURE: NO tool completion notifications");
                eprintln!("This means:");
                eprintln!("  1. Tools were requested (ToolCall sent)");
                eprintln!("  2. Tools may have executed");
                eprintln!("  3. But completion notifications never sent");
                panic!("No ToolCallUpdate(Completed) notifications received");
            }
        })
        .await;
}
