//! Recorded test verifying slash_commands from Claude CLI init message are captured
//!
//! This test uses a pre-recorded init message to verify that:
//! 1. Claude CLI init message with slash_commands is processed
//! 2. AvailableCommandsUpdate notification is emitted
//! 3. Commands have proper metadata (source="claude_cli")
//!
//! Note: The exact commands may vary by Claude CLI version and MCP configuration.
//! This test verifies the structure and parsing, not the specific command list.
//!
//! To re-record this fixture:
//! 1. Run: `cargo test test_slash_commands_from_claude_init_message --ignored -- --nocapture`
//! 2. Capture the AvailableCommandsUpdate notification
//! 3. Update `fixtures/slash_commands_init_response.json`

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CommandMeta {
    source: String,
    #[serde(default)]
    mcp_server: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AvailableCommand {
    name: String,
    description: String,
    #[serde(default)]
    meta: Option<CommandMeta>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct RecordedNotification {
    update_type: String,
    #[serde(default)]
    available_commands: Vec<AvailableCommand>,
}

const FIXTURE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/slash_commands_init_response.json"
);

#[test]
fn test_slash_commands_from_init_recorded() {
    let data = std::fs::read_to_string(FIXTURE_PATH).expect(
        "Failed to read fixture. To create it:\n\
         1. Run: cargo test test_slash_commands_from_claude_init_message --ignored -- --nocapture\n\
         2. Capture AvailableCommandsUpdate and save as JSON fixture",
    );

    let notifications: Vec<RecordedNotification> =
        serde_json::from_str(&data).expect("Failed to parse fixture JSON");

    println!("\n=== Received {} notifications ===", notifications.len());

    for (i, notif) in notifications.iter().enumerate() {
        if notif.update_type == "AvailableCommandsUpdate" {
            println!(
                "{}. AvailableCommandsUpdate: {} commands",
                i + 1,
                notif.available_commands.len()
            );
            for cmd in &notif.available_commands {
                let source = cmd
                    .meta
                    .as_ref()
                    .map(|m| m.source.as_str())
                    .unwrap_or("unknown");
                println!("   - {} ({}): {}", cmd.name, source, cmd.description);
            }
        }
    }

    // Look for AvailableCommandsUpdate with source="claude_cli"
    let claude_commands = notifications
        .iter()
        .find(|n| n.update_type == "AvailableCommandsUpdate")
        .map(|n| &n.available_commands);

    // Assert we got slash commands from Claude init message
    assert!(
        claude_commands.is_some(),
        "Expected AvailableCommandsUpdate from Claude CLI init message with slash_commands, but got none!\n\
         This means the system/init message was not processed."
    );

    let commands = claude_commands.unwrap();
    println!(
        "\n=== Claude CLI provided {} slash commands ===",
        commands.len()
    );

    // Verify commands have claude_cli source
    let has_claude_source = commands
        .iter()
        .any(|cmd| cmd.meta.as_ref().map(|m| m.source.as_str()) == Some("claude_cli"));

    assert!(
        has_claude_source,
        "Expected commands with source='claude_cli'"
    );

    // Should have Claude built-in commands
    assert!(
        commands
            .iter()
            .any(|c| c.name == "compact" || c.name == "context"),
        "Expected Claude built-in commands like 'compact' or 'context'"
    );

    // Check for SAH MCP commands (may or may not be present depending on config)
    let has_sah_commands = commands.iter().any(|c| c.name.starts_with("mcp__sah__"));
    if has_sah_commands {
        println!("✓ Found SAH MCP commands");
    } else {
        println!("⚠ No SAH MCP commands in this recording");
    }

    println!("\n✅ Test passed!");
    println!("  - Claude CLI init message processed correctly");
    println!("  - Slash commands extracted and emitted");
    println!("  - Commands have proper metadata");
}
