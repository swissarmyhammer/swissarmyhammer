//! Test that slash_commands from Claude CLI init message are captured
//!
//! ⚠️ MANUAL VERIFICATION ONLY - Use test_slash_commands_from_init_recorded.rs for CI
//!
//! This test makes real API calls and spawns Claude CLI to verify init message parsing.
//! Kept for manual verification and re-recording fixtures.

use agent_client_protocol::{
    Agent, InitializeRequest, NewSessionRequest, SessionNotification, SessionUpdate, V1,
};
use claude_agent::{agent::ClaudeAgent, config::AgentConfig};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

/// Helper to collect notifications with timeout
async fn collect_notifications_with_timeout(
    receiver: &mut broadcast::Receiver<SessionNotification>,
    timeout: Duration,
) -> Vec<SessionNotification> {
    let mut notifications = Vec::new();
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }

        match tokio::time::timeout(remaining, receiver.recv()).await {
            Ok(Ok(notification)) => {
                notifications.push(notification);
            }
            Ok(Err(_)) => break,
            Err(_) => break,
        }
    }

    notifications
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
#[ignore = "Manual verification only - spawns Claude CLI. Use test_slash_commands_from_init_recorded.rs for CI"]
async fn test_slash_commands_from_claude_init_message() {
    let local = tokio::task::LocalSet::new();
    local.run_until(test_inner()).await;
}

async fn test_inner() {
    // Create agent
    let config = AgentConfig::default();
    let (agent, mut notification_receiver) = ClaudeAgent::new(config).await.unwrap();
    let agent = Arc::new(agent);

    // Initialize agent
    let init_request = InitializeRequest {
        protocol_version: V1,
        client_capabilities: agent_client_protocol::ClientCapabilities {
            fs: agent_client_protocol::FileSystemCapability {
                read_text_file: true,
                write_text_file: true,
                meta: None,
            },
            terminal: false,
            meta: None,
        },
        client_info: None,
        meta: None,
    };
    agent.initialize(init_request).await.unwrap();

    // Create session - this should spawn Claude process and read init message
    let cwd = std::env::current_dir().expect("Failed to get current directory");
    let new_session_request = NewSessionRequest {
        cwd,
        mcp_servers: vec![],
        meta: None,
    };

    eprintln!("\n=== Creating session ===");
    let session_response = agent
        .new_session(new_session_request)
        .await
        .expect("Failed to create session");

    eprintln!("Session created: {}", session_response.session_id.0);

    // Collect notifications for a few seconds to capture init message
    eprintln!("=== Collecting notifications ===");
    let notifications =
        collect_notifications_with_timeout(&mut notification_receiver, Duration::from_secs(3))
            .await;

    eprintln!("\n=== Received {} notifications ===", notifications.len());
    for (i, notif) in notifications.iter().enumerate() {
        match &notif.update {
            SessionUpdate::AvailableCommandsUpdate(update) => {
                eprintln!(
                    "{}. AvailableCommandsUpdate: {} commands",
                    i + 1,
                    update.available_commands.len()
                );
                for cmd in &update.available_commands {
                    let source = cmd
                        .meta
                        .as_ref()
                        .and_then(|m| m.get("source"))
                        .and_then(|s| s.as_str())
                        .unwrap_or("unknown");
                    eprintln!("   - {} ({}): {}", cmd.name, source, cmd.description);
                }
            }
            _ => {
                eprintln!("{}. {:?}", i + 1, std::mem::discriminant(&notif.update));
            }
        }
    }

    // Look for AvailableCommandsUpdate with source="claude_cli_init"
    let claude_commands = notifications
        .iter()
        .find(|n| matches!(&n.update, SessionUpdate::AvailableCommandsUpdate { .. }))
        .and_then(|n| {
            if let SessionUpdate::AvailableCommandsUpdate(update) = &n.update {
                let available_commands = &update.available_commands;
                let has_claude_source = available_commands.iter().any(|cmd| {
                    cmd.meta
                        .as_ref()
                        .and_then(|m| m.get("source"))
                        .and_then(|s| s.as_str())
                        == Some("claude_cli")
                });
                if has_claude_source {
                    Some(available_commands.clone())
                } else {
                    None
                }
            } else {
                None
            }
        });

    // Assert we got slash commands from Claude init message
    assert!(
        claude_commands.is_some(),
        "Expected AvailableCommandsUpdate from Claude CLI init message with slash_commands, but got none!\n\
         This means the system/init message was not processed."
    );

    let commands = claude_commands.unwrap();
    eprintln!(
        "\n=== Claude CLI provided {} slash commands ===",
        commands.len()
    );

    // Should have Claude built-in commands
    assert!(
        commands
            .iter()
            .any(|c| c.name == "compact" || c.name == "context"),
        "Expected Claude built-in commands like 'compact' or 'context'"
    );

    // Should have SAH MCP prompts if SAH is connected
    let has_sah_commands = commands.iter().any(|c| c.name.starts_with("mcp__sah__"));
    if has_sah_commands {
        eprintln!("✓ Found SAH MCP commands");
        assert!(
            commands.iter().any(|c| c.name == "mcp__sah__test"),
            "Expected SAH 'test' command"
        );
    } else {
        eprintln!("⚠ No SAH MCP commands found (SAH might not be configured in Claude)");
    }
}
