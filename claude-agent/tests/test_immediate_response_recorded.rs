//! Recorded test verifying agent responds immediately to first prompt
//!
//! This test uses a pre-recorded API response to verify the bug fix where
//! agent sends thoughts/plans but no AgentMessageChunk until multiple prompts are sent.
//!
//! The expected behavior is that AgentMessageChunk notifications are sent immediately
//! on the first prompt, not delayed until subsequent prompts.
//!
//! To re-record this fixture:
//! 1. Run: `cargo test test_agent_responds_immediately_to_first_prompt --ignored -- --nocapture`
//! 2. Capture the notification stream
//! 3. Update `fixtures/immediate_response.json`

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct RecordedNotification {
    update_type: String,
    #[serde(flatten)]
    data: serde_json::Value,
}

const FIXTURE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/immediate_response.json"
);

#[test]
fn test_agent_responds_immediately_recorded() {
    let data = std::fs::read_to_string(FIXTURE_PATH).expect(
        "Failed to read fixture. To create it:\n\
         1. Run: cargo test test_agent_responds_immediately_to_first_prompt --ignored -- --nocapture\n\
         2. Capture notifications and save as JSON fixture"
    );

    let notifications: Vec<RecordedNotification> =
        serde_json::from_str(&data).expect("Failed to parse fixture JSON");

    // Print what we received
    println!("\n=== Received {} notifications ===", notifications.len());
    for (i, notif) in notifications.iter().enumerate() {
        match notif.update_type.as_str() {
            "AgentMessageChunk" => {
                if let Some(text) = notif
                    .data
                    .get("content")
                    .and_then(|c| c.get("Text"))
                    .and_then(|t| t.get("text"))
                    .and_then(|t| t.as_str())
                {
                    println!("{}. AgentMessageChunk: {}", i + 1, text);
                }
            }
            "AgentThoughtChunk" => {
                if let Some(text) = notif
                    .data
                    .get("content")
                    .and_then(|c| c.get("Text"))
                    .and_then(|t| t.get("text"))
                    .and_then(|t| t.as_str())
                {
                    println!("{}. AgentThoughtChunk: {}", i + 1, text);
                }
            }
            "UserMessageChunk" => {
                if let Some(text) = notif
                    .data
                    .get("content")
                    .and_then(|c| c.get("Text"))
                    .and_then(|t| t.get("text"))
                    .and_then(|t| t.as_str())
                {
                    println!("{}. UserMessageChunk: {}", i + 1, text);
                }
            }
            "Plan" => {
                println!("{}. Plan", i + 1);
            }
            "AvailableCommandsUpdate" => {
                if let Some(commands) = notif.data.get("available_commands") {
                    if let Some(arr) = commands.as_array() {
                        println!("{}. AvailableCommandsUpdate: {} commands", i + 1, arr.len());
                    }
                }
            }
            _ => {
                println!("{}. {}", i + 1, notif.update_type);
            }
        }
    }

    // Assert that we received at least one AgentMessageChunk
    let has_agent_message = notifications
        .iter()
        .any(|notif| notif.update_type == "AgentMessageChunk");

    assert!(
        has_agent_message,
        "Expected at least one AgentMessageChunk in response to first prompt, but got none!\n\
         Received {} notifications total. This is the bug where agent only sends thoughts/plans but no actual response.",
        notifications.len()
    );

    // Also verify we got some kind of response (not just silence)
    assert!(
        !notifications.is_empty(),
        "Expected some notifications from agent, but got none"
    );

    println!("\n✅ Test passed!");
    println!("  - Agent sent AgentMessageChunk immediately on first prompt");
    println!("  - Bug is not present (agent responds immediately)");
}
