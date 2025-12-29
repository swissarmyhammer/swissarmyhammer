//! Test filesystem structure serialization with camelCase field names
//!
//! This test verifies that the agent_client_protocol crate's filesystem
//! structures serialize field names according to the ACP specification (camelCase).

#[cfg(feature = "acp")]
mod acp_filesystem_serialization_tests {
    use agent_client_protocol::{
        ReadTextFileRequest, ReadTextFileResponse, SessionId, WriteTextFileRequest,
        WriteTextFileResponse,
    };

    /// Test ReadTextFileRequest serializes with correct field names
    #[test]
    fn test_read_text_file_request_serialization() {
        let session_id = SessionId::new("test-session-123".to_string());
        let path = "/path/to/file.txt".to_string();

        let request = ReadTextFileRequest::new(session_id.clone(), path.clone());

        // Serialize to JSON
        let json = serde_json::to_value(&request).expect("Failed to serialize request");
        let obj = json.as_object().expect("Should be a JSON object");

        // Verify field names are in camelCase (sessionId, not session_id)
        assert!(
            obj.contains_key("sessionId"),
            "ReadTextFileRequest should serialize sessionId as camelCase. Found keys: {:?}",
            obj.keys().collect::<Vec<_>>()
        );
        assert!(
            !obj.contains_key("session_id"),
            "ReadTextFileRequest should NOT use snake_case session_id"
        );

        // Verify path field exists
        assert!(
            obj.contains_key("path"),
            "ReadTextFileRequest should have path field. Found keys: {:?}",
            obj.keys().collect::<Vec<_>>()
        );

        // Verify values
        assert_eq!(
            obj.get("sessionId").unwrap().as_str().unwrap(),
            "test-session-123"
        );
        assert_eq!(
            obj.get("path").unwrap().as_str().unwrap(),
            "/path/to/file.txt"
        );
    }

    /// Test ReadTextFileResponse serializes with correct field names
    #[test]
    fn test_read_text_file_response_serialization() {
        let content = "Hello, world!\nThis is file content.".to_string();

        let response = ReadTextFileResponse::new(content.clone());

        // Serialize to JSON
        let json = serde_json::to_value(&response).expect("Failed to serialize response");
        let obj = json.as_object().expect("Should be a JSON object");

        // Verify content field exists
        assert!(
            obj.contains_key("content"),
            "ReadTextFileResponse should have content field. Found keys: {:?}",
            obj.keys().collect::<Vec<_>>()
        );

        // Verify value
        assert_eq!(obj.get("content").unwrap().as_str().unwrap(), content);
    }

    /// Test WriteTextFileRequest serializes with correct field names
    #[test]
    fn test_write_text_file_request_serialization() {
        let session_id = SessionId::new("test-session-456".to_string());
        let path = "/path/to/output.txt".to_string();
        let content = "New file content".to_string();

        let request = WriteTextFileRequest::new(session_id.clone(), path.clone(), content.clone());

        // Serialize to JSON
        let json = serde_json::to_value(&request).expect("Failed to serialize request");
        let obj = json.as_object().expect("Should be a JSON object");

        // Verify field names are in camelCase (sessionId, not session_id)
        assert!(
            obj.contains_key("sessionId"),
            "WriteTextFileRequest should serialize sessionId as camelCase. Found keys: {:?}",
            obj.keys().collect::<Vec<_>>()
        );
        assert!(
            !obj.contains_key("session_id"),
            "WriteTextFileRequest should NOT use snake_case session_id"
        );

        // Verify other fields exist
        assert!(
            obj.contains_key("path"),
            "WriteTextFileRequest should have path field. Found keys: {:?}",
            obj.keys().collect::<Vec<_>>()
        );
        assert!(
            obj.contains_key("content"),
            "WriteTextFileRequest should have content field. Found keys: {:?}",
            obj.keys().collect::<Vec<_>>()
        );

        // Verify values
        assert_eq!(
            obj.get("sessionId").unwrap().as_str().unwrap(),
            "test-session-456"
        );
        assert_eq!(
            obj.get("path").unwrap().as_str().unwrap(),
            "/path/to/output.txt"
        );
        assert_eq!(
            obj.get("content").unwrap().as_str().unwrap(),
            "New file content"
        );
    }

    /// Test WriteTextFileResponse serializes correctly
    #[test]
    fn test_write_text_file_response_serialization() {
        let response = WriteTextFileResponse::new();

        // Serialize to JSON
        let json = serde_json::to_value(&response).expect("Failed to serialize response");

        // WriteTextFileResponse is typically empty or minimal
        // Verify it serializes without error
        assert!(
            json.is_object() || json.is_null(),
            "WriteTextFileResponse should serialize to object or null, got: {:?}",
            json
        );
    }

    /// Test round-trip serialization/deserialization with camelCase
    #[test]
    fn test_read_request_round_trip() {
        let session_id = SessionId::new("round-trip-test".to_string());
        let path = "/test/round/trip.txt".to_string();

        let original = ReadTextFileRequest::new(session_id, path);

        // Serialize
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

        // Deserialize
        let deserialized: ReadTextFileRequest =
            serde_json::from_str(&json_string).expect("Failed to deserialize");

        // Verify values match
        assert_eq!(
            deserialized.session_id.0.as_ref(),
            original.session_id.0.as_ref()
        );
        assert_eq!(deserialized.path, original.path);
    }

    /// Test round-trip serialization/deserialization for write request
    #[test]
    fn test_write_request_round_trip() {
        let session_id = SessionId::new("write-round-trip".to_string());
        let path = "/test/write.txt".to_string();
        let content = "Test content".to_string();

        let original = WriteTextFileRequest::new(session_id, path, content);

        // Serialize
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

        // Deserialize
        let deserialized: WriteTextFileRequest =
            serde_json::from_str(&json_string).expect("Failed to deserialize");

        // Verify values match
        assert_eq!(
            deserialized.session_id.0.as_ref(),
            original.session_id.0.as_ref()
        );
        assert_eq!(deserialized.path, original.path);
        assert_eq!(deserialized.content, original.content);
    }
}
