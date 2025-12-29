//! ACP Protocol Serialization Conformance Tests
//!
//! This test module verifies that agent_client_protocol crate structures
//! serialize field names and enum values according to the ACP specification:
//!
//! - Field names use camelCase (e.g., `sessionId`, not `session_id`)
//! - Enum values use snake_case (e.g., `in_progress`, `pending`, `completed`)
//!
//! These tests ensure interoperability between different ACP implementations.

// =============================================================================
// Command Structure Serialization Tests
// =============================================================================

mod command_serialization {
    use agent_client_protocol::{
        AvailableCommand, AvailableCommandInput, UnstructuredCommandInput,
    };

    #[test]
    fn test_available_command_serialization() {
        let command = AvailableCommand::new("/test", "Test command description");

        let json = serde_json::to_value(&command).expect("Failed to serialize command");
        let obj = json.as_object().expect("Should be a JSON object");

        assert!(
            obj.contains_key("name"),
            "AvailableCommand should have name field. Found keys: {:?}",
            obj.keys().collect::<Vec<_>>()
        );
        assert!(
            obj.contains_key("description"),
            "AvailableCommand should have description field"
        );

        assert_eq!(obj.get("name").unwrap().as_str().unwrap(), "/test");
        assert_eq!(
            obj.get("description").unwrap().as_str().unwrap(),
            "Test command description"
        );
    }

    #[test]
    fn test_available_command_with_input_serialization() {
        let input = UnstructuredCommandInput::new("<arg1> [arg2]");
        let command = AvailableCommand::new("/test", "Test command")
            .input(AvailableCommandInput::Unstructured(input));

        let json = serde_json::to_value(&command).expect("Failed to serialize command");
        let obj = json.as_object().expect("Should be a JSON object");

        assert!(obj.contains_key("name"));
        assert!(obj.contains_key("description"));
        assert!(
            obj.contains_key("input"),
            "AvailableCommand should have input field when specified"
        );
    }

    #[test]
    fn test_unstructured_command_input_serialization() {
        let input = UnstructuredCommandInput::new("<required> [optional]");

        let json = serde_json::to_value(&input).expect("Failed to serialize input");
        let obj = json.as_object().expect("Should be a JSON object");

        assert!(
            obj.contains_key("hint"),
            "UnstructuredCommandInput should have hint field. Found keys: {:?}",
            obj.keys().collect::<Vec<_>>()
        );
        assert_eq!(
            obj.get("hint").unwrap().as_str().unwrap(),
            "<required> [optional]"
        );
    }

    #[test]
    fn test_command_round_trip() {
        let original = AvailableCommand::new("/round-trip", "Round trip test");

        let json_string = serde_json::to_string(&original).expect("Failed to serialize");
        let deserialized: AvailableCommand =
            serde_json::from_str(&json_string).expect("Failed to deserialize");

        assert_eq!(deserialized.name, original.name);
        assert_eq!(deserialized.description, original.description);
    }

    #[test]
    fn test_no_snake_case_in_command_field_names() {
        let command = AvailableCommand::new("/test", "Test");

        let json_string = serde_json::to_string(&command).expect("Failed to serialize");

        // Field names should not use snake_case patterns
        assert!(
            !json_string.contains("\"_id\""),
            "Serialized JSON should not contain snake_case field patterns like _id"
        );
        assert!(
            !json_string.contains("\"_name\""),
            "Serialized JSON should not contain snake_case field patterns like _name"
        );
    }
}

// =============================================================================
// Filesystem Structure Serialization Tests
// =============================================================================

mod filesystem_serialization {
    use agent_client_protocol::{
        ReadTextFileRequest, ReadTextFileResponse, SessionId, WriteTextFileRequest,
        WriteTextFileResponse,
    };

    #[test]
    fn test_read_text_file_request_uses_camel_case() {
        let session_id = SessionId::new("test-session-123".to_string());
        let request = ReadTextFileRequest::new(session_id, "/path/to/file.txt".to_string());

        let json = serde_json::to_value(&request).expect("Failed to serialize request");
        let obj = json.as_object().expect("Should be a JSON object");

        // Field name must be camelCase
        assert!(
            obj.contains_key("sessionId"),
            "ReadTextFileRequest should serialize sessionId as camelCase. Found keys: {:?}",
            obj.keys().collect::<Vec<_>>()
        );
        assert!(
            !obj.contains_key("session_id"),
            "ReadTextFileRequest should NOT use snake_case session_id"
        );

        assert!(obj.contains_key("path"));
        assert_eq!(
            obj.get("sessionId").unwrap().as_str().unwrap(),
            "test-session-123"
        );
        assert_eq!(
            obj.get("path").unwrap().as_str().unwrap(),
            "/path/to/file.txt"
        );
    }

    #[test]
    fn test_read_text_file_response_serialization() {
        let response =
            ReadTextFileResponse::new("Hello, world!\nThis is file content.".to_string());

        let json = serde_json::to_value(&response).expect("Failed to serialize response");
        let obj = json.as_object().expect("Should be a JSON object");

        assert!(obj.contains_key("content"));
        assert_eq!(
            obj.get("content").unwrap().as_str().unwrap(),
            "Hello, world!\nThis is file content."
        );
    }

    #[test]
    fn test_write_text_file_request_uses_camel_case() {
        let session_id = SessionId::new("test-session-456".to_string());
        let request = WriteTextFileRequest::new(
            session_id,
            "/path/to/output.txt".to_string(),
            "New file content".to_string(),
        );

        let json = serde_json::to_value(&request).expect("Failed to serialize request");
        let obj = json.as_object().expect("Should be a JSON object");

        // Field name must be camelCase
        assert!(
            obj.contains_key("sessionId"),
            "WriteTextFileRequest should serialize sessionId as camelCase. Found keys: {:?}",
            obj.keys().collect::<Vec<_>>()
        );
        assert!(
            !obj.contains_key("session_id"),
            "WriteTextFileRequest should NOT use snake_case session_id"
        );

        assert!(obj.contains_key("path"));
        assert!(obj.contains_key("content"));
    }

    #[test]
    fn test_write_text_file_response_serialization() {
        let response = WriteTextFileResponse::new();

        let json = serde_json::to_value(&response).expect("Failed to serialize response");

        assert!(
            json.is_object() || json.is_null(),
            "WriteTextFileResponse should serialize to object or null"
        );
    }

    #[test]
    fn test_read_request_round_trip_preserves_camel_case() {
        let session_id = SessionId::new("round-trip-test".to_string());
        let original = ReadTextFileRequest::new(session_id, "/test/round/trip.txt".to_string());

        let json_string = serde_json::to_string(&original).expect("Failed to serialize");

        // Verify JSON uses camelCase
        assert!(
            json_string.contains("sessionId"),
            "Serialized JSON should use camelCase sessionId"
        );
        assert!(
            !json_string.contains("session_id"),
            "Serialized JSON should NOT use snake_case session_id"
        );

        let deserialized: ReadTextFileRequest =
            serde_json::from_str(&json_string).expect("Failed to deserialize");

        assert_eq!(
            deserialized.session_id.0.as_ref(),
            original.session_id.0.as_ref()
        );
        assert_eq!(deserialized.path, original.path);
    }

    #[test]
    fn test_write_request_round_trip_preserves_camel_case() {
        let session_id = SessionId::new("write-round-trip".to_string());
        let original = WriteTextFileRequest::new(
            session_id,
            "/test/write.txt".to_string(),
            "Test content".to_string(),
        );

        let json_string = serde_json::to_string(&original).expect("Failed to serialize");

        assert!(json_string.contains("sessionId"));
        assert!(!json_string.contains("session_id"));

        let deserialized: WriteTextFileRequest =
            serde_json::from_str(&json_string).expect("Failed to deserialize");

        assert_eq!(
            deserialized.session_id.0.as_ref(),
            original.session_id.0.as_ref()
        );
        assert_eq!(deserialized.path, original.path);
        assert_eq!(deserialized.content, original.content);
    }
}

// =============================================================================
// Plan Structure Serialization Tests
// =============================================================================

mod plan_serialization {
    use agent_client_protocol::{Plan, PlanEntry, PlanEntryPriority, PlanEntryStatus};

    #[test]
    fn test_plan_entry_basic_serialization() {
        let entry = PlanEntry::new(
            "Test task".to_string(),
            PlanEntryPriority::Medium,
            PlanEntryStatus::Pending,
        );

        let json = serde_json::to_value(&entry).expect("Failed to serialize entry");
        let obj = json.as_object().expect("Should be a JSON object");

        assert!(obj.contains_key("content"));
        assert!(obj.contains_key("priority"));
        assert!(obj.contains_key("status"));

        assert_eq!(obj.get("content").unwrap().as_str().unwrap(), "Test task");
        assert_eq!(obj.get("priority").unwrap().as_str().unwrap(), "medium");
        assert_eq!(obj.get("status").unwrap().as_str().unwrap(), "pending");
    }

    #[test]
    fn test_plan_entry_priority_values() {
        // Priority enum values should be lowercase
        let high = PlanEntry::new(
            "High".to_string(),
            PlanEntryPriority::High,
            PlanEntryStatus::Pending,
        );
        let medium = PlanEntry::new(
            "Medium".to_string(),
            PlanEntryPriority::Medium,
            PlanEntryStatus::Pending,
        );
        let low = PlanEntry::new(
            "Low".to_string(),
            PlanEntryPriority::Low,
            PlanEntryStatus::Pending,
        );

        let high_json = serde_json::to_value(&high).unwrap();
        assert_eq!(high_json.get("priority").unwrap().as_str().unwrap(), "high");

        let medium_json = serde_json::to_value(&medium).unwrap();
        assert_eq!(
            medium_json.get("priority").unwrap().as_str().unwrap(),
            "medium"
        );

        let low_json = serde_json::to_value(&low).unwrap();
        assert_eq!(low_json.get("priority").unwrap().as_str().unwrap(), "low");
    }

    #[test]
    fn test_plan_entry_status_values() {
        // Per ACP spec, status values use snake_case: pending, in_progress, completed
        let pending = PlanEntry::new(
            "Pending".to_string(),
            PlanEntryPriority::Medium,
            PlanEntryStatus::Pending,
        );
        let in_progress = PlanEntry::new(
            "In Progress".to_string(),
            PlanEntryPriority::Medium,
            PlanEntryStatus::InProgress,
        );
        let completed = PlanEntry::new(
            "Completed".to_string(),
            PlanEntryPriority::Medium,
            PlanEntryStatus::Completed,
        );

        let pending_json = serde_json::to_value(&pending).unwrap();
        assert_eq!(
            pending_json.get("status").unwrap().as_str().unwrap(),
            "pending"
        );

        let in_progress_json = serde_json::to_value(&in_progress).unwrap();
        // ACP spec uses snake_case for enum values: "in_progress" (not "inProgress")
        assert_eq!(
            in_progress_json.get("status").unwrap().as_str().unwrap(),
            "in_progress"
        );

        let completed_json = serde_json::to_value(&completed).unwrap();
        assert_eq!(
            completed_json.get("status").unwrap().as_str().unwrap(),
            "completed"
        );
    }

    #[test]
    fn test_plan_serialization() {
        let entries = vec![
            PlanEntry::new(
                "First task".to_string(),
                PlanEntryPriority::High,
                PlanEntryStatus::Pending,
            ),
            PlanEntry::new(
                "Second task".to_string(),
                PlanEntryPriority::Medium,
                PlanEntryStatus::InProgress,
            ),
        ];

        let plan = Plan::new(entries);

        let json = serde_json::to_value(&plan).expect("Failed to serialize plan");
        let obj = json.as_object().expect("Should be a JSON object");

        assert!(obj.contains_key("entries"));

        let entries_array = obj.get("entries").unwrap().as_array().unwrap();
        assert_eq!(entries_array.len(), 2);
    }

    #[test]
    fn test_plan_entry_round_trip() {
        let original = PlanEntry::new(
            "Round trip test".to_string(),
            PlanEntryPriority::High,
            PlanEntryStatus::InProgress,
        );

        let json_string = serde_json::to_string(&original).expect("Failed to serialize");

        // Status should be snake_case per ACP spec
        assert!(
            json_string.contains("in_progress"),
            "Status should serialize as snake_case in_progress"
        );

        let deserialized: PlanEntry =
            serde_json::from_str(&json_string).expect("Failed to deserialize");

        assert_eq!(deserialized.content, original.content);
    }

    #[test]
    fn test_plan_round_trip() {
        let entries = vec![
            PlanEntry::new(
                "Task 1".to_string(),
                PlanEntryPriority::High,
                PlanEntryStatus::Pending,
            ),
            PlanEntry::new(
                "Task 2".to_string(),
                PlanEntryPriority::Low,
                PlanEntryStatus::Completed,
            ),
        ];

        let original = Plan::new(entries);

        let json_string = serde_json::to_string(&original).expect("Failed to serialize");
        let deserialized: Plan = serde_json::from_str(&json_string).expect("Failed to deserialize");

        assert_eq!(deserialized.entries.len(), original.entries.len());
        assert_eq!(deserialized.entries[0].content, original.entries[0].content);
        assert_eq!(deserialized.entries[1].content, original.entries[1].content);
    }

    #[test]
    fn test_plan_no_unexpected_snake_case_fields() {
        let entry = PlanEntry::new(
            "Test".to_string(),
            PlanEntryPriority::Medium,
            PlanEntryStatus::InProgress,
        );
        let plan = Plan::new(vec![entry]);

        let json_string = serde_json::to_string(&plan).expect("Failed to serialize");

        // Field names should use camelCase, not snake_case
        // Note: "in_progress" is an enum VALUE (correct), not a field NAME
        assert!(
            !json_string.contains("\"_status\""),
            "Should not contain snake_case field patterns like _status"
        );
        assert!(
            !json_string.contains("\"_priority\""),
            "Should not contain snake_case field patterns like _priority"
        );
        assert!(
            !json_string.contains("\"_content\""),
            "Should not contain snake_case field patterns like _content"
        );
    }
}
