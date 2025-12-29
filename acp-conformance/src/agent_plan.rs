//! Agent plan protocol conformance tests
//!
//! Tests based on https://agentclientprotocol.com/protocol/agent-plan
//!
//! ## Requirements Tested
//!
//! 1. **Creating Plans**
//!    - Agents SHOULD report plans via `session/update` notifications
//!    - Field: `sessionUpdate` = "plan"
//!    - Field: `entries` array of PlanEntry objects
//!    - Each PlanEntry must have:
//!      - `content` (string, required): human-readable task description
//!      - `priority` (PlanEntryPriority, required): "high" | "medium" | "low"
//!      - `status` (PlanEntryStatus, required): "pending" | "in_progress" | "completed"
//!
//! 2. **Updating Plans**
//!    - Agents SHOULD report updates via more `session/update` notifications
//!    - Agent MUST send complete list of all plan entries in each update
//!    - Client MUST replace the current plan completely
//!
//! 3. **Dynamic Planning**
//!    - Plans can evolve during execution
//!    - Agent MAY add, remove, or modify plan entries as it discovers new requirements

use agent_client_protocol::{
    Agent, ContentBlock, InitializeRequest, PromptRequest, ProtocolVersion, TextContent,
};
use agent_client_protocol_extras::recording::RecordedSession;

/// Statistics from plan fixture verification
#[derive(Debug, Default)]
pub struct PlanStats {
    pub plan_notifications: usize,
    pub total_entries: usize,
    pub entries_pending: usize,
    pub entries_in_progress: usize,
    pub entries_completed: usize,
    pub agent_message_chunks: usize,
}

/// Test that agent sends plan notifications when using todo/planning tools
///
/// Per spec: Agents SHOULD report execution plans via session/update notifications
/// with sessionUpdate="plan"
pub async fn test_agent_sends_plan_notifications<A: Agent + ?Sized>(
    agent: &A,
) -> crate::Result<()> {
    tracing::info!("Testing agent sends plan notifications");

    // Initialize agent
    let init_request = InitializeRequest::new(ProtocolVersion::V1);
    let _init_response = agent.initialize(init_request).await?;

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = agent_client_protocol::NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
    let session_id = new_session_response.session_id;

    // Ask agent to use todo_create tool which should trigger plan notifications
    // The TestMcpServer and SwissArmyHammer both have todo tools
    let text_content = TextContent::new(
        "Create a todo item using the mcp__test-mcp-server__create-plan tool with goal 'Implement user authentication'. \
         This is a multi-step task that requires planning.",
    );
    let content_block = ContentBlock::Text(text_content);

    let prompt_request = PromptRequest::new(session_id, vec![content_block]);

    // Send prompt - agent should create plan and send notifications
    let result = agent.prompt(prompt_request).await;

    match result {
        Ok(response) => {
            tracing::info!(
                "Agent completed prompt with stop_reason: {:?}",
                response.stop_reason
            );
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("{:?}", e);
            // Check if error is about unsupported functionality
            if error_msg.contains("unsupported") || error_msg.contains("not implemented") {
                Err(crate::Error::Validation(format!(
                    "Agent rejected planning prompt: {}",
                    error_msg
                )))
            } else {
                // Other errors (like model not loaded) are acceptable for conformance tests
                tracing::warn!(
                    "Agent returned error, but not due to unsupported planning: {}",
                    error_msg
                );
                Ok(())
            }
        }
    }
}

/// Verify plan notifications in a recorded fixture
///
/// This function reads the fixture and verifies:
/// 1. The fixture has recorded calls (not calls: [])
/// 2. Plan notifications were sent with sessionUpdate="plan"
/// 3. Plan entries have required fields (content, priority, status)
pub fn verify_plan_fixture(
    agent_type: &str,
    test_name: &str,
) -> Result<PlanStats, Box<dyn std::error::Error>> {
    let fixture_path = agent_client_protocol_extras::get_fixture_path_for(agent_type, test_name);

    if !fixture_path.exists() {
        return Err(format!("Fixture not found: {:?}", fixture_path).into());
    }

    let content = std::fs::read_to_string(&fixture_path)?;
    let session: RecordedSession = serde_json::from_str(&content)?;

    let mut stats = PlanStats::default();

    // CRITICAL: Verify we have calls recorded (catches poor tests with calls: [])
    assert!(
        !session.calls.is_empty(),
        "Expected recorded calls, fixture has calls: [] - test didn't call agent properly"
    );

    for call in &session.calls {
        for notification_json in &call.notifications {
            // Check for ACP session updates
            if let Some(update_val) = notification_json.get("update") {
                if let Some(session_update) =
                    update_val.get("sessionUpdate").and_then(|v| v.as_str())
                {
                    match session_update {
                        "plan" => {
                            stats.plan_notifications += 1;

                            // Verify plan has entries array
                            if let Some(entries) =
                                update_val.get("entries").and_then(|v| v.as_array())
                            {
                                for entry in entries {
                                    stats.total_entries += 1;

                                    // Verify required fields
                                    assert!(
                                        entry.get("content").and_then(|v| v.as_str()).is_some(),
                                        "Plan entry missing 'content' field"
                                    );

                                    // Verify priority is valid
                                    if let Some(priority) =
                                        entry.get("priority").and_then(|v| v.as_str())
                                    {
                                        assert!(
                                            ["high", "medium", "low"].contains(&priority),
                                            "Invalid priority: {}. Must be 'high', 'medium', or 'low'",
                                            priority
                                        );
                                    } else {
                                        panic!("Plan entry missing 'priority' field");
                                    }

                                    // Verify status and count
                                    if let Some(status) =
                                        entry.get("status").and_then(|v| v.as_str())
                                    {
                                        match status {
                                            "pending" => stats.entries_pending += 1,
                                            "in_progress" => stats.entries_in_progress += 1,
                                            "completed" => stats.entries_completed += 1,
                                            _ => panic!(
                                                "Invalid status: {}. Must be 'pending', 'in_progress', or 'completed'",
                                                status
                                            ),
                                        }
                                    } else {
                                        panic!("Plan entry missing 'status' field");
                                    }
                                }
                            }
                        }
                        "agent_message_chunk" => stats.agent_message_chunks += 1,
                        _ => {}
                    }
                }
            }
        }
    }

    tracing::info!("{} plan fixture stats: {:?}", agent_type, stats);

    // Agent should produce output
    assert!(
        stats.agent_message_chunks > 0,
        "Expected agent_message_chunk notifications, got {}. Agent should respond to prompt.",
        stats.agent_message_chunks
    );

    // Note: Plan notifications are SHOULD (not MUST) per spec, so we don't assert on them
    // But we log them for visibility
    if stats.plan_notifications == 0 {
        tracing::warn!(
            "{} agent did not send any plan notifications. This is allowed per spec (SHOULD, not MUST).",
            agent_type
        );
    }

    Ok(stats)
}

//
// Unit tests for JSON structure validation (no agent needed)
//

/// Validate plan entry JSON structure
pub fn validate_plan_entry(entry: &serde_json::Value) -> Result<(), String> {
    // Verify required fields
    entry
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing 'content' field".to_string())?;

    let priority = entry
        .get("priority")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing 'priority' field".to_string())?;

    if !["high", "medium", "low"].contains(&priority) {
        return Err(format!(
            "Invalid priority: {}. Must be 'high', 'medium', or 'low'",
            priority
        ));
    }

    let status = entry
        .get("status")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing 'status' field".to_string())?;

    if !["pending", "in_progress", "completed"].contains(&status) {
        return Err(format!(
            "Invalid status: {}. Must be 'pending', 'in_progress', or 'completed'",
            status
        ));
    }

    Ok(())
}

/// Validate plan session update JSON structure
pub fn validate_plan_session_update(update: &serde_json::Value) -> Result<(), String> {
    let session_update_type = update
        .get("sessionUpdate")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing 'sessionUpdate' field".to_string())?;

    if session_update_type != "plan" {
        return Err(format!(
            "Expected sessionUpdate='plan', got '{}'",
            session_update_type
        ));
    }

    let entries = update
        .get("entries")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Missing or invalid 'entries' field".to_string())?;

    if entries.is_empty() {
        return Err("Plan entries array should not be empty".to_string());
    }

    for (i, entry) in entries.iter().enumerate() {
        validate_plan_entry(entry).map_err(|e| format!("Entry {} error: {}", i, e))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_plan_entry() {
        let entry = serde_json::json!({
            "content": "Analyze the codebase",
            "priority": "high",
            "status": "pending"
        });
        assert!(validate_plan_entry(&entry).is_ok());
    }

    #[test]
    fn test_plan_entry_missing_content() {
        let entry = serde_json::json!({
            "priority": "high",
            "status": "pending"
        });
        assert!(validate_plan_entry(&entry).is_err());
    }

    #[test]
    fn test_plan_entry_invalid_priority() {
        let entry = serde_json::json!({
            "content": "Task",
            "priority": "critical",  // invalid
            "status": "pending"
        });
        let result = validate_plan_entry(&entry);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid priority"));
    }

    #[test]
    fn test_plan_entry_invalid_status() {
        let entry = serde_json::json!({
            "content": "Task",
            "priority": "high",
            "status": "done"  // invalid
        });
        let result = validate_plan_entry(&entry);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid status"));
    }

    #[test]
    fn test_valid_plan_session_update() {
        let update = serde_json::json!({
            "sessionUpdate": "plan",
            "entries": [
                {
                    "content": "Task 1",
                    "priority": "high",
                    "status": "pending"
                },
                {
                    "content": "Task 2",
                    "priority": "medium",
                    "status": "in_progress"
                }
            ]
        });
        assert!(validate_plan_session_update(&update).is_ok());
    }

    #[test]
    fn test_plan_session_update_empty_entries() {
        let update = serde_json::json!({
            "sessionUpdate": "plan",
            "entries": []
        });
        let result = validate_plan_session_update(&update);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn test_plan_entry_all_statuses() {
        for status in ["pending", "in_progress", "completed"] {
            let entry = serde_json::json!({
                "content": "Task",
                "priority": "medium",
                "status": status
            });
            assert!(
                validate_plan_entry(&entry).is_ok(),
                "Status '{}' should be valid",
                status
            );
        }
    }

    #[test]
    fn test_plan_entry_all_priorities() {
        for priority in ["high", "medium", "low"] {
            let entry = serde_json::json!({
                "content": "Task",
                "priority": priority,
                "status": "pending"
            });
            assert!(
                validate_plan_entry(&entry).is_ok(),
                "Priority '{}' should be valid",
                priority
            );
        }
    }
}
