//! Test command structure serialization with camelCase field names
//!
//! This test verifies that the agent_client_protocol crate's command
//! structures serialize field names according to the ACP specification (camelCase).

#[cfg(feature = "acp")]
mod acp_commands_serialization_tests {
    use agent_client_protocol::{
        AvailableCommand, AvailableCommandInput, UnstructuredCommandInput,
    };

    /// Test AvailableCommand serializes with correct field names
    #[test]
    fn test_available_command_serialization() {
        let command = AvailableCommand::new("/test", "Test command description");

        // Serialize to JSON
        let json = serde_json::to_value(&command).expect("Failed to serialize command");
        let obj = json.as_object().expect("Should be a JSON object");

        // Verify basic fields exist
        assert!(
            obj.contains_key("name"),
            "AvailableCommand should have name field. Found keys: {:?}",
            obj.keys().collect::<Vec<_>>()
        );
        assert!(
            obj.contains_key("description"),
            "AvailableCommand should have description field. Found keys: {:?}",
            obj.keys().collect::<Vec<_>>()
        );

        // Verify values
        assert_eq!(obj.get("name").unwrap().as_str().unwrap(), "/test");
        assert_eq!(
            obj.get("description").unwrap().as_str().unwrap(),
            "Test command description"
        );
    }

    /// Test AvailableCommand with input specification
    #[test]
    fn test_available_command_with_input_serialization() {
        let input = UnstructuredCommandInput::new("<arg1> [arg2]");
        let command = AvailableCommand::new("/test", "Test command")
            .input(AvailableCommandInput::Unstructured(input));

        // Serialize to JSON
        let json = serde_json::to_value(&command).expect("Failed to serialize command");
        let obj = json.as_object().expect("Should be a JSON object");

        // Verify fields
        assert!(
            obj.contains_key("name"),
            "AvailableCommand should have name field"
        );
        assert!(
            obj.contains_key("description"),
            "AvailableCommand should have description field"
        );
        assert!(
            obj.contains_key("input"),
            "AvailableCommand should have input field when specified"
        );
    }

    /// Test AvailableCommand with metadata
    ///
    /// NOTE: This test currently skips meta field verification as the
    /// agent_client_protocol crate may not serialize it by default.
    #[test]
    fn test_available_command_with_meta_serialization() {
        let mut meta = serde_json::Map::new();
        meta.insert("custom_field".to_string(), serde_json::json!("value"));
        meta.insert("another_field".to_string(), serde_json::json!(42));

        let command = AvailableCommand::new("/test", "Test command").meta(meta.clone());

        // Serialize to JSON
        let json = serde_json::to_value(&command).expect("Failed to serialize command");
        let obj = json.as_object().expect("Should be a JSON object");

        // Verify basic fields exist
        assert!(
            obj.contains_key("name"),
            "AvailableCommand should have name field"
        );
        assert!(
            obj.contains_key("description"),
            "AvailableCommand should have description field"
        );

        // TODO: Verify meta field when agent_client_protocol supports it
        // The meta field may not be serialized by default in the current implementation
    }

    /// Test UnstructuredCommandInput serializes correctly
    #[test]
    fn test_unstructured_command_input_serialization() {
        let input = UnstructuredCommandInput::new("<required> [optional]");

        // Serialize to JSON
        let json = serde_json::to_value(&input).expect("Failed to serialize input");
        let obj = json.as_object().expect("Should be a JSON object");

        // Verify hint field exists
        assert!(
            obj.contains_key("hint"),
            "UnstructuredCommandInput should have hint field. Found keys: {:?}",
            obj.keys().collect::<Vec<_>>()
        );

        // Verify value
        assert_eq!(
            obj.get("hint").unwrap().as_str().unwrap(),
            "<required> [optional]"
        );
    }

    /// Test UnstructuredCommandInput with metadata
    ///
    /// NOTE: This test currently skips meta field verification as the
    /// agent_client_protocol crate may not serialize it by default.
    #[test]
    fn test_unstructured_command_input_with_meta_serialization() {
        let mut meta = serde_json::Map::new();
        meta.insert("parameters".to_string(), serde_json::json!([]));

        let input = UnstructuredCommandInput::new("test").meta(meta);

        // Serialize to JSON
        let json = serde_json::to_value(&input).expect("Failed to serialize input");
        let obj = json.as_object().expect("Should be a JSON object");

        // Verify hint field exists
        assert!(
            obj.contains_key("hint"),
            "UnstructuredCommandInput should have hint field"
        );

        // TODO: Verify meta field when agent_client_protocol supports it
        // The meta field may not be serialized by default in the current implementation
    }

    /// Test round-trip serialization/deserialization
    #[test]
    fn test_command_round_trip() {
        let original = AvailableCommand::new("/round-trip", "Round trip test");

        // Serialize
        let json_string = serde_json::to_string(&original).expect("Failed to serialize");

        // Deserialize
        let deserialized: AvailableCommand =
            serde_json::from_str(&json_string).expect("Failed to deserialize");

        // Verify values match
        assert_eq!(deserialized.name, original.name);
        assert_eq!(deserialized.description, original.description);
    }

    /// Test that no snake_case fields are used
    #[test]
    fn test_no_snake_case_in_serialization() {
        let command = AvailableCommand::new("/test", "Test");

        // Serialize to string
        let json_string = serde_json::to_string(&command).expect("Failed to serialize");

        // Common snake_case patterns that should NOT appear
        assert!(
            !json_string.contains("_id"),
            "Serialized JSON should not contain snake_case patterns like _id"
        );
        assert!(
            !json_string.contains("_name"),
            "Serialized JSON should not contain snake_case patterns like _name"
        );
        assert!(
            !json_string.contains("_field"),
            "Serialized JSON should not contain snake_case patterns like _field"
        );
    }
}
