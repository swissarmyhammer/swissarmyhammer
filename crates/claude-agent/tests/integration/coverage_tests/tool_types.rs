//! Coverage tests: Tool type conversions and serialization.

use claude_agent::tool_types::*;
use serde_json::json;

/// Tool call id that the report tests use.
const TEST_CALL_ID: &str = "tc_1";

/// Title that the report tests use.
const TEST_TITLE: &str = "Test";

/// Tool name that the report tests use.
const READ_TOOL_NAME: &str = "Read";

/// Title that the ACP conversion test uses.
const READING_FILE_TITLE: &str = "Reading file";

/// Path that the location and raw input tests use.
const TEST_FILE_PATH: &str = "/tmp/test.txt";

/// Path that the location tests use for a source file.
const SOURCE_FILE_PATH: &str = "/test/file.rs";

/// Raw output that the raw IO test uses.
const FILE_CONTENTS_OUTPUT: &str = "file contents here";

/// Path that the ACP conversion test uses.
const TEST_DIR_PATH: &str = "/test";

/// Line number that the location tests use.
const TEST_LINE_NUMBER: u32 = 42;
/// Build a tool input that carries only a path.
fn path_input(path: &str) -> serde_json::Value {
    json!({"path": path})
}

/// Assert that every value survives a JSON serialize and deserialize round trip.
fn assert_serde_round_trip<T>(values: Vec<T>)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    for value in values {
        let json = serde_json::to_string(&value).unwrap();
        let deserialized: T = serde_json::from_str(&json).unwrap();
        assert_eq!(value, deserialized);
    }
}

// ============================================================================
// ToolType tests (additional coverage)
// ============================================================================

#[test]
fn test_tool_kind_serialization_all_variants() {
    assert_serde_round_trip(vec![
        ToolKind::Read,
        ToolKind::Edit,
        ToolKind::Delete,
        ToolKind::Move,
        ToolKind::Search,
        ToolKind::Execute,
        ToolKind::Think,
        ToolKind::Fetch,
        ToolKind::Other,
    ]);
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
    assert_serde_round_trip(vec![
        ToolCallStatus::Pending,
        ToolCallStatus::InProgress,
        ToolCallStatus::Completed,
        ToolCallStatus::Failed,
        ToolCallStatus::Cancelled,
    ]);
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
        TEST_CALL_ID.to_string(),
        READING_FILE_TITLE.to_string(),
        ToolKind::Read,
        READ_TOOL_NAME.to_string(),
    );
    assert_eq!(report.tool_call_id, TEST_CALL_ID);
    assert_eq!(report.title, READING_FILE_TITLE);
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
        TEST_CALL_ID.to_string(),
        TEST_TITLE.to_string(),
        ToolKind::Read,
        READ_TOOL_NAME.to_string(),
    );
    report.update_status(ToolCallStatus::InProgress);
    assert_eq!(report.status, ToolCallStatus::InProgress);
    report.update_status(ToolCallStatus::Completed);
    assert_eq!(report.status, ToolCallStatus::Completed);
}

#[test]
fn test_tool_call_report_add_location() {
    let mut report = ToolCallReport::new(
        TEST_CALL_ID.to_string(),
        TEST_TITLE.to_string(),
        ToolKind::Read,
        READ_TOOL_NAME.to_string(),
    );
    report.add_location(ToolCallLocation {
        path: TEST_FILE_PATH.to_string(),
        line: Some(10),
    });
    assert_eq!(report.locations.len(), 1);
}

#[test]
fn test_tool_call_report_set_raw_io() {
    let mut report = ToolCallReport::new(
        TEST_CALL_ID.to_string(),
        TEST_TITLE.to_string(),
        ToolKind::Read,
        READ_TOOL_NAME.to_string(),
    );
    report.set_raw_input(path_input(TEST_FILE_PATH));
    report.set_raw_output(json!(FILE_CONTENTS_OUTPUT));
    assert_eq!(report.raw_input, Some(path_input(TEST_FILE_PATH)));
    assert_eq!(report.raw_output, Some(json!(FILE_CONTENTS_OUTPUT)));
}

#[test]
fn test_tool_call_report_mark_state_sent() {
    let mut report = ToolCallReport::new(
        TEST_CALL_ID.to_string(),
        TEST_TITLE.to_string(),
        ToolKind::Read,
        READ_TOOL_NAME.to_string(),
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
        TEST_CALL_ID.to_string(),
        READING_FILE_TITLE.to_string(),
        ToolKind::Read,
        READ_TOOL_NAME.to_string(),
    );
    report.set_raw_input(path_input(TEST_DIR_PATH));
    report.add_location(ToolCallLocation {
        path: TEST_DIR_PATH.to_string(),
        line: None,
    });

    let acp_call = report.to_acp_tool_call();
    assert_eq!(acp_call.tool_call_id.to_string(), TEST_CALL_ID);
    assert_eq!(acp_call.title, READING_FILE_TITLE);
}

#[test]
fn test_tool_call_report_to_acp_update_first() {
    let report = ToolCallReport::new(
        TEST_CALL_ID.to_string(),
        TEST_TITLE.to_string(),
        ToolKind::Read,
        READ_TOOL_NAME.to_string(),
    );
    // First update (no previous state) - should include all fields
    let update = report.to_acp_tool_call_update();
    assert_eq!(update.tool_call_id.to_string(), TEST_CALL_ID);
    assert_eq!(update.fields.title.as_deref(), Some(TEST_TITLE));
    assert!(update.fields.status.is_some());
    assert!(update.fields.kind.is_some());
    assert!(update.fields.content.is_some());
    assert!(update.fields.locations.is_some());
}

#[test]
fn test_tool_call_report_partial_update_after_state_sent() {
    let mut report = ToolCallReport::new(
        TEST_CALL_ID.to_string(),
        TEST_TITLE.to_string(),
        ToolKind::Read,
        READ_TOOL_NAME.to_string(),
    );
    report.mark_state_sent();

    // Change status
    report.update_status(ToolCallStatus::InProgress);
    let update = report.to_acp_tool_call_update();
    assert_eq!(update.tool_call_id.to_string(), TEST_CALL_ID);
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
        TEST_CALL_ID.to_string(),
        TEST_TITLE.to_string(),
        ToolKind::Read,
        READ_TOOL_NAME.to_string(),
    );
    report.add_location(ToolCallLocation {
        path: TEST_FILE_PATH.to_string(),
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
    assert_eq!(update.tool_call_id.to_string(), TEST_CALL_ID);
    assert_eq!(update.fields.content.map(|c| c.len()), Some(1));
    assert_eq!(update.fields.locations.map(|l| l.len()), Some(1));
}

#[test]
fn test_tool_call_location_to_acp_with_line() {
    let loc = ToolCallLocation {
        path: SOURCE_FILE_PATH.to_string(),
        line: Some(TEST_LINE_NUMBER.into()),
    };
    let acp = loc.to_acp_location();
    assert_eq!(acp.line, Some(TEST_LINE_NUMBER));
}

#[test]
fn test_tool_call_location_to_acp_without_line() {
    let loc = ToolCallLocation {
        path: SOURCE_FILE_PATH.to_string(),
        line: None,
    };
    let acp = loc.to_acp_location();
    assert!(acp.line.is_none());
}

#[test]
fn test_tool_call_content_diff_to_acp() {
    let content = ToolCallContent::Diff {
        path: SOURCE_FILE_PATH.to_string(),
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
