//! Coverage tests: Tool type conversions and serialization.

use claude_agent::tool_types::*;
use serde_json::json;

// ============================================================================
// ToolType tests (additional coverage)
// ============================================================================

#[test]
fn test_tool_kind_serialization_all_variants() {
    let kinds = vec![
        ToolKind::Read,
        ToolKind::Edit,
        ToolKind::Delete,
        ToolKind::Move,
        ToolKind::Search,
        ToolKind::Execute,
        ToolKind::Think,
        ToolKind::Fetch,
        ToolKind::Other,
    ];
    for kind in kinds {
        let json = serde_json::to_string(&kind).unwrap();
        let deserialized: ToolKind = serde_json::from_str(&json).unwrap();
        assert_eq!(kind, deserialized);
    }
}

#[test]
fn test_tool_kind_to_acp_kind_all_variants() {
    assert!(matches!(
        ToolKind::Read.to_acp_kind(),
        agent_client_protocol::schema::ToolKind::Read
    ));
    assert!(matches!(
        ToolKind::Edit.to_acp_kind(),
        agent_client_protocol::schema::ToolKind::Edit
    ));
    assert!(matches!(
        ToolKind::Delete.to_acp_kind(),
        agent_client_protocol::schema::ToolKind::Delete
    ));
    assert!(matches!(
        ToolKind::Move.to_acp_kind(),
        agent_client_protocol::schema::ToolKind::Move
    ));
    assert!(matches!(
        ToolKind::Search.to_acp_kind(),
        agent_client_protocol::schema::ToolKind::Search
    ));
    assert!(matches!(
        ToolKind::Execute.to_acp_kind(),
        agent_client_protocol::schema::ToolKind::Execute
    ));
    assert!(matches!(
        ToolKind::Think.to_acp_kind(),
        agent_client_protocol::schema::ToolKind::Think
    ));
    assert!(matches!(
        ToolKind::Fetch.to_acp_kind(),
        agent_client_protocol::schema::ToolKind::Fetch
    ));
    assert!(matches!(
        ToolKind::Other.to_acp_kind(),
        agent_client_protocol::schema::ToolKind::Other
    ));
}

#[test]
fn test_tool_call_status_serialization() {
    let statuses = vec![
        ToolCallStatus::Pending,
        ToolCallStatus::InProgress,
        ToolCallStatus::Completed,
        ToolCallStatus::Failed,
        ToolCallStatus::Cancelled,
    ];
    for status in statuses {
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: ToolCallStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, deserialized);
    }
}

#[test]
fn test_tool_call_status_to_acp_all_variants() {
    assert!(matches!(
        ToolCallStatus::Pending.to_acp_status(),
        agent_client_protocol::schema::ToolCallStatus::Pending
    ));
    assert!(matches!(
        ToolCallStatus::InProgress.to_acp_status(),
        agent_client_protocol::schema::ToolCallStatus::InProgress
    ));
    assert!(matches!(
        ToolCallStatus::Completed.to_acp_status(),
        agent_client_protocol::schema::ToolCallStatus::Completed
    ));
    assert!(matches!(
        ToolCallStatus::Failed.to_acp_status(),
        agent_client_protocol::schema::ToolCallStatus::Failed
    ));
    // Cancelled maps to Failed
    assert!(matches!(
        ToolCallStatus::Cancelled.to_acp_status(),
        agent_client_protocol::schema::ToolCallStatus::Failed
    ));
}

#[test]
fn test_tool_call_report_new() {
    let report = ToolCallReport::new(
        "tc_1".to_string(),
        "Reading file".to_string(),
        ToolKind::Read,
        "Read".to_string(),
    );
    assert_eq!(report.tool_call_id, "tc_1");
    assert_eq!(report.title, "Reading file");
    assert_eq!(report.kind, ToolKind::Read);
    assert_eq!(report.status, ToolCallStatus::Pending);
    assert!(report.content.is_empty());
    assert!(report.locations.is_empty());
    assert!(report.raw_input.is_none());
    assert!(report.raw_output.is_none());
}

#[test]
fn test_tool_call_report_update_status() {
    let mut report = ToolCallReport::new(
        "tc_1".to_string(),
        "Test".to_string(),
        ToolKind::Read,
        "Read".to_string(),
    );
    report.update_status(ToolCallStatus::InProgress);
    assert_eq!(report.status, ToolCallStatus::InProgress);
    report.update_status(ToolCallStatus::Completed);
    assert_eq!(report.status, ToolCallStatus::Completed);
}

#[test]
fn test_tool_call_report_add_location() {
    let mut report = ToolCallReport::new(
        "tc_1".to_string(),
        "Test".to_string(),
        ToolKind::Read,
        "Read".to_string(),
    );
    report.add_location(ToolCallLocation {
        path: "/tmp/test.txt".to_string(),
        line: Some(10),
    });
    assert_eq!(report.locations.len(), 1);
}

#[test]
fn test_tool_call_report_set_raw_io() {
    let mut report = ToolCallReport::new(
        "tc_1".to_string(),
        "Test".to_string(),
        ToolKind::Read,
        "Read".to_string(),
    );
    report.set_raw_input(json!({"path": "/tmp/test.txt"}));
    report.set_raw_output(json!("file contents here"));
    assert_eq!(report.raw_input, Some(json!({"path": "/tmp/test.txt"})));
    assert_eq!(report.raw_output, Some(json!("file contents here")));
}

#[test]
fn test_tool_call_report_mark_state_sent() {
    let mut report = ToolCallReport::new(
        "tc_1".to_string(),
        "Test".to_string(),
        ToolKind::Read,
        "Read".to_string(),
    );
    report.mark_state_sent();
    // Nothing changed since the mark, so a partial update carries no fields
    let fields = report.to_acp_tool_call_update().fields;
    assert!(
        fields.title.is_none()
            && fields.status.is_none()
            && fields.kind.is_none()
            && fields.content.is_none()
            && fields.locations.is_none()
    );
}

#[test]
fn test_tool_call_report_to_acp_tool_call() {
    let mut report = ToolCallReport::new(
        "tc_1".to_string(),
        "Reading file".to_string(),
        ToolKind::Read,
        "Read".to_string(),
    );
    report.set_raw_input(json!({"path": "/test"}));
    report.add_location(ToolCallLocation {
        path: "/test".to_string(),
        line: None,
    });

    let acp_call = report.to_acp_tool_call();
    assert_eq!(acp_call.tool_call_id.to_string(), "tc_1");
    assert_eq!(acp_call.title, "Reading file");
}

#[test]
fn test_tool_call_report_to_acp_update_first() {
    let report = ToolCallReport::new(
        "tc_1".to_string(),
        "Test".to_string(),
        ToolKind::Read,
        "Read".to_string(),
    );
    // First update (no previous state) - should include all fields
    let update = report.to_acp_tool_call_update();
    assert_eq!(update.tool_call_id.to_string(), "tc_1");
    assert_eq!(update.fields.title.as_deref(), Some("Test"));
    assert!(update.fields.status.is_some());
    assert!(update.fields.kind.is_some());
    assert!(update.fields.content.is_some());
    assert!(update.fields.locations.is_some());
}

#[test]
fn test_tool_call_report_partial_update_after_state_sent() {
    let mut report = ToolCallReport::new(
        "tc_1".to_string(),
        "Test".to_string(),
        ToolKind::Read,
        "Read".to_string(),
    );
    report.mark_state_sent();

    // Change status
    report.update_status(ToolCallStatus::InProgress);
    let update = report.to_acp_tool_call_update();
    assert_eq!(update.tool_call_id.to_string(), "tc_1");
    assert_eq!(
        update.fields.status,
        Some(ToolCallStatus::InProgress.to_acp_status())
    );
    assert!(
        update.fields.title.is_none()
            && update.fields.kind.is_none()
            && update.fields.content.is_none()
            && update.fields.locations.is_none()
    );
}

#[test]
fn test_tool_call_report_update_with_context() {
    let mut report = ToolCallReport::new(
        "tc_1".to_string(),
        "Test".to_string(),
        ToolKind::Read,
        "Read".to_string(),
    );
    report.add_location(ToolCallLocation {
        path: "/tmp/test.txt".to_string(),
        line: None,
    });
    report.add_content(ToolCallContent::Content {
        content: agent_client_protocol::schema::ContentBlock::Text(
            agent_client_protocol::schema::TextContent::new("output".to_string()),
        ),
    });
    report.mark_state_sent();
    report.update_status(ToolCallStatus::Completed);
    // With context includes content/locations even if unchanged
    let update = report.to_acp_tool_call_update_with_context(true);
    assert_eq!(update.tool_call_id.to_string(), "tc_1");
    assert_eq!(update.fields.content.map(|c| c.len()), Some(1));
    assert_eq!(update.fields.locations.map(|l| l.len()), Some(1));
}

#[test]
fn test_tool_call_location_to_acp_with_line() {
    let loc = ToolCallLocation {
        path: "/test/file.rs".to_string(),
        line: Some(42),
    };
    let acp = loc.to_acp_location();
    assert_eq!(acp.line, Some(42));
}

#[test]
fn test_tool_call_location_to_acp_without_line() {
    let loc = ToolCallLocation {
        path: "/test/file.rs".to_string(),
        line: None,
    };
    let acp = loc.to_acp_location();
    assert!(acp.line.is_none());
}

#[test]
fn test_tool_call_content_diff_to_acp() {
    let content = ToolCallContent::Diff {
        path: "/test/file.rs".to_string(),
        old_text: Some("old".to_string()),
        new_text: "new".to_string(),
    };
    let acp = content.to_acp_content();
    assert!(matches!(
        acp,
        agent_client_protocol::schema::ToolCallContent::Diff(_)
    ));
}

#[test]
fn test_tool_call_content_diff_no_old_text() {
    let content = ToolCallContent::Diff {
        path: "/test/new_file.rs".to_string(),
        old_text: None,
        new_text: "new content".to_string(),
    };
    let acp = content.to_acp_content();
    assert!(matches!(
        acp,
        agent_client_protocol::schema::ToolCallContent::Diff(_)
    ));
}

#[test]
fn test_tool_call_content_terminal_to_acp() {
    let content = ToolCallContent::Terminal {
        terminal_id: "term_123".to_string(),
    };
    let acp = content.to_acp_content();
    assert!(matches!(
        acp,
        agent_client_protocol::schema::ToolCallContent::Terminal(_)
    ));
}
