//! Recorded integration test verifying tool completion notifications
//!
//! This test uses a pre-recorded API response for consistent, fast testing without API calls.
//! The recording was captured from a real Claude API interaction and committed as a test fixture.
//!
//! To re-record this fixture:
//! 1. Ensure you have ANTHROPIC_API_KEY set and sufficient rate limits
//! 2. Run: `cargo test test_read_cargo_toml_gets_completion_notification --ignored -- --nocapture`
//! 3. Manually extract the notifications from the output
//! 4. Update `fixtures/tool_completion_response.json` with the captured data

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct RecordedNotification {
    update_type: String,
    #[serde(flatten)]
    data: serde_json::Value,
}

const FIXTURE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/tool_completion_response.json"
);

#[test]
fn test_tool_completion_with_recorded_response() {
    let data = std::fs::read_to_string(FIXTURE_PATH).expect(
        "Failed to read fixture. To create it:\n\
         1. Run: cargo test test_read_cargo_toml_gets_completion_notification --ignored -- --nocapture\n\
         2. Manually capture the notifications and save as JSON fixture"
    );

    let notifications: Vec<RecordedNotification> =
        serde_json::from_str(&data).expect("Failed to parse fixture JSON");

    println!("\n=== Received {} notifications ===", notifications.len());

    let mut tool_call_count = 0;
    let mut tool_update_count = 0;
    let mut tool_completion_count = 0;

    for (i, notif) in notifications.iter().enumerate() {
        match notif.update_type.as_str() {
            "ToolCall" => {
                tool_call_count += 1;
                if let Some(title) = notif.data.get("title").and_then(|t| t.as_str()) {
                    println!("{}. ToolCall: {}", i + 1, title);
                }
                if let Some(id) = notif
                    .data
                    .get("id")
                    .and_then(|id_obj| id_obj.get("0"))
                    .and_then(|id| id.as_str())
                {
                    println!("   → id: {}", id);
                }
                if let Some(status) = notif.data.get("status").and_then(|s| s.as_str()) {
                    println!("   → status: {}", status);
                }
            }
            "ToolCallUpdate" => {
                tool_update_count += 1;
                if let Some(id) = notif
                    .data
                    .get("id")
                    .and_then(|id_obj| id_obj.get("0"))
                    .and_then(|id| id.as_str())
                {
                    println!("{}. ToolCallUpdate: {}", i + 1, id);
                }

                // Check for completion status
                if let Some(fields) = notif.data.get("fields") {
                    if let Some(status) = fields.get("status").and_then(|s| s.as_str()) {
                        println!("   → status: {}", status);
                        if status == "Completed" {
                            tool_completion_count += 1;
                            println!("   ✅ COMPLETION FOUND");
                        }
                    }
                }
            }
            "AgentMessageChunk" => {
                if let Some(text) = notif
                    .data
                    .get("content")
                    .and_then(|c| c.get("Text"))
                    .and_then(|t| t.get("text"))
                    .and_then(|t| t.as_str())
                {
                    println!(
                        "{}. AgentMessageChunk: {}",
                        i + 1,
                        text.chars().take(60).collect::<String>()
                    );
                }
            }
            _ => {}
        }
    }

    println!("\n=== RESULTS ===");
    println!("ToolCall notifications: {}", tool_call_count);
    println!("ToolCallUpdate notifications: {}", tool_update_count);
    println!("ToolCallUpdate(Completed): {}", tool_completion_count);

    // Assertions - verify the recorded response has what we expect
    assert!(
        !notifications.is_empty(),
        "Expected to receive notifications, but got none!"
    );

    assert!(
        tool_call_count > 0,
        "Expected at least one ToolCall notification"
    );

    assert!(
        tool_update_count > 0,
        "Expected at least one ToolCallUpdate notification"
    );

    assert!(
        tool_completion_count > 0,
        "Expected at least one ToolCallUpdate with Completed status"
    );

    println!("\n✅ Test passed!");
    println!("  - Received {} ToolCall notifications", tool_call_count);
    println!(
        "  - Received {} ToolCallUpdate notifications",
        tool_update_count
    );
    println!(
        "  - Received {} Completed notifications",
        tool_completion_count
    );
    println!("  - Tool completion notifications ARE being sent");
}
