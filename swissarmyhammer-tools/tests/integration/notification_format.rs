//! Tests for MCP notification format compliance
//!
//! This test suite verifies that progress notifications are properly formatted
//! according to the MCP protocol specification for notifications/progress.

use serde_json::json;
use swissarmyhammer_tools::mcp::progress_notifications::{
    complete_notification, generate_progress_token, start_notification, ProgressNotification,
};

/// Test that ProgressNotification serializes to correct JSON structure
#[test]
fn test_progress_notification_json_structure() {
    let notification = ProgressNotification {
        progress_token: "test_token_123".to_string(),
        progress: Some(50),
        message: "Processing files".to_string(),
        metadata: None,
    };

    let json = serde_json::to_value(&notification).unwrap();

    // Verify required fields are present
    assert!(json.get("progress_token").is_some());
    assert!(json.get("message").is_some());

    // Verify field values
    assert_eq!(json["progress_token"], "test_token_123");
    assert_eq!(json["message"], "Processing files");
    assert_eq!(json["progress"], 50);
}

/// Test that ProgressNotification without progress serializes correctly
#[test]
fn test_progress_notification_indeterminate_format() {
    let notification = ProgressNotification {
        progress_token: "test_token_123".to_string(),
        progress: None, // Indeterminate progress
        message: "Working...".to_string(),
        metadata: None,
    };

    let json = serde_json::to_value(&notification).unwrap();

    // When progress is None, the field should not be present in JSON
    assert!(json.get("progress").is_none());
    assert_eq!(json["progress_token"], "test_token_123");
    assert_eq!(json["message"], "Working...");
}

/// Test that metadata is flattened into the top level of the JSON
#[test]
fn test_progress_notification_metadata_flattening() {
    let metadata = json!({
        "files_processed": 42,
        "total_files": 100,
        "current_file": "src/main.rs"
    });

    let notification = ProgressNotification {
        progress_token: "test_token_123".to_string(),
        progress: Some(42),
        message: "Processing files".to_string(),
        metadata: Some(metadata.clone()),
    };

    let json = serde_json::to_value(&notification).unwrap();

    // Verify metadata fields are at the top level, not nested
    assert_eq!(json["files_processed"], 42);
    assert_eq!(json["total_files"], 100);
    assert_eq!(json["current_file"], "src/main.rs");

    // Verify there's no separate "metadata" field
    assert!(json.get("metadata").is_none());
}

/// Test that progress values are within valid range (0-100)
#[test]
fn test_progress_notification_progress_range() {
    // Test minimum
    let notification = ProgressNotification {
        progress_token: "test".to_string(),
        progress: Some(0),
        message: "Starting".to_string(),
        metadata: None,
    };
    let json = serde_json::to_value(&notification).unwrap();
    assert_eq!(json["progress"], 0);

    // Test maximum
    let notification = ProgressNotification {
        progress_token: "test".to_string(),
        progress: Some(100),
        message: "Complete".to_string(),
        metadata: None,
    };
    let json = serde_json::to_value(&notification).unwrap();
    assert_eq!(json["progress"], 100);

    // Test middle value
    let notification = ProgressNotification {
        progress_token: "test".to_string(),
        progress: Some(50),
        message: "Halfway".to_string(),
        metadata: None,
    };
    let json = serde_json::to_value(&notification).unwrap();
    assert_eq!(json["progress"], 50);
}

/// Test that progress_token field name is correct (snake_case)
#[test]
fn test_progress_notification_token_field_name() {
    let notification = ProgressNotification {
        progress_token: "test_token_123".to_string(),
        progress: Some(50),
        message: "Test".to_string(),
        metadata: None,
    };

    let json = serde_json::to_value(&notification).unwrap();

    // Must be "progress_token" not "progressToken"
    assert!(json.get("progress_token").is_some());
    assert!(json.get("progressToken").is_none());
}

/// Test that all field types are correct
#[test]
fn test_progress_notification_field_types() {
    let notification = ProgressNotification {
        progress_token: "test_token_123".to_string(),
        progress: Some(50),
        message: "Processing".to_string(),
        metadata: Some(json!({"key": "value"})),
    };

    let json = serde_json::to_value(&notification).unwrap();

    // progress_token should be string
    assert!(json["progress_token"].is_string());

    // progress should be number
    assert!(json["progress"].is_number());

    // message should be string
    assert!(json["message"].is_string());

    // Flattened metadata fields should be strings
    assert!(json["key"].is_string());
}

/// Test that empty message is allowed
#[test]
fn test_progress_notification_empty_message() {
    let notification = ProgressNotification {
        progress_token: "test".to_string(),
        progress: Some(50),
        message: "".to_string(),
        metadata: None,
    };

    let json = serde_json::to_value(&notification).unwrap();
    assert_eq!(json["message"], "");
}

/// Test round-trip serialization/deserialization
#[test]
fn test_progress_notification_round_trip() {
    let original = ProgressNotification {
        progress_token: "test_token_123".to_string(),
        progress: Some(75),
        message: "Almost done".to_string(),
        metadata: Some(json!({
            "files": 75,
            "total": 100
        })),
    };

    // Serialize
    let json_str = serde_json::to_string(&original).unwrap();

    // Deserialize
    let deserialized: ProgressNotification = serde_json::from_str(&json_str).unwrap();

    // Verify all fields match
    assert_eq!(original.progress_token, deserialized.progress_token);
    assert_eq!(original.progress, deserialized.progress);
    assert_eq!(original.message, deserialized.message);
    assert_eq!(original.metadata, deserialized.metadata);
}

/// Test that start_notification helper produces correct format
#[test]
fn test_start_notification_format() {
    let notification = start_notification("token_123", "file processing");

    let json = serde_json::to_value(&notification).unwrap();

    assert_eq!(json["progress_token"], "token_123");
    assert_eq!(json["progress"], 0);
    assert!(json["message"]
        .as_str()
        .unwrap()
        .contains("file processing"));
    assert!(json.get("metadata").is_none());
}

/// Test that complete_notification helper produces correct format
#[test]
fn test_complete_notification_format() {
    let notification = complete_notification("token_123", "file processing");

    let json = serde_json::to_value(&notification).unwrap();

    assert_eq!(json["progress_token"], "token_123");
    assert_eq!(json["progress"], 100);
    assert!(json["message"]
        .as_str()
        .unwrap()
        .contains("file processing"));
    assert!(json.get("metadata").is_none());
}

/// Test notification with complex metadata structure
#[test]
fn test_progress_notification_complex_metadata() {
    let metadata = json!({
        "operation": "file_scan",
        "stats": {
            "files": 100,
            "errors": 2,
            "warnings": 5
        },
        "files": ["file1.rs", "file2.rs", "file3.rs"],
        "duration_ms": 1234
    });

    let notification = ProgressNotification {
        progress_token: "test".to_string(),
        progress: Some(50),
        message: "Scanning files".to_string(),
        metadata: Some(metadata.clone()),
    };

    let json = serde_json::to_value(&notification).unwrap();

    // Verify complex metadata is flattened correctly
    assert_eq!(json["operation"], "file_scan");
    assert_eq!(json["stats"]["files"], 100);
    assert_eq!(json["stats"]["errors"], 2);
    assert_eq!(json["files"][0], "file1.rs");
    assert_eq!(json["duration_ms"], 1234);
}

/// Test notification with very long message
#[test]
fn test_progress_notification_long_message() {
    let long_message = "Processing file: ".to_string() + &"a".repeat(1000);

    let notification = ProgressNotification {
        progress_token: "test".to_string(),
        progress: Some(50),
        message: long_message.clone(),
        metadata: None,
    };

    let json = serde_json::to_value(&notification).unwrap();
    assert_eq!(json["message"], long_message);
}

/// Test notification with special characters in token
#[test]
fn test_progress_notification_special_chars_in_token() {
    let notification = ProgressNotification {
        progress_token: "progress_1234567890abcdef_0123456789abcdef".to_string(),
        progress: Some(50),
        message: "Test".to_string(),
        metadata: None,
    };

    let json = serde_json::to_value(&notification).unwrap();
    assert_eq!(
        json["progress_token"],
        "progress_1234567890abcdef_0123456789abcdef"
    );
}

/// Test notification with unicode characters
#[test]
fn test_progress_notification_unicode() {
    let notification = ProgressNotification {
        progress_token: "test".to_string(),
        progress: Some(50),
        message: "Processing 文件 🚀".to_string(),
        metadata: Some(json!({"file": "テスト.rs"})),
    };

    let json = serde_json::to_value(&notification).unwrap();
    assert_eq!(json["message"], "Processing 文件 🚀");
    assert_eq!(json["file"], "テスト.rs");
}

/// Test that JSON serialization is compact (no pretty printing)
#[test]
fn test_progress_notification_compact_json() {
    let notification = ProgressNotification {
        progress_token: "test".to_string(),
        progress: Some(50),
        message: "Test".to_string(),
        metadata: None,
    };

    let json_str = serde_json::to_string(&notification).unwrap();

    // Should not contain newlines or excessive whitespace
    assert!(!json_str.contains('\n'));
    assert!(!json_str.contains("  ")); // No double spaces
}

/// Test that generated progress tokens have consistent format
#[test]
fn test_generated_progress_token_format() {
    let token = generate_progress_token();

    // Should start with "progress_"
    assert!(token.starts_with("progress_"));

    // Should contain underscores separating parts
    let parts: Vec<&str> = token.split('_').collect();
    assert_eq!(parts.len(), 3); // "progress", timestamp, random

    // Should be alphanumeric + underscores
    assert!(token
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-'));
}

/// Test that multiple generated tokens are unique
#[test]
fn test_generated_tokens_unique() {
    let tokens: Vec<String> = (0..100).map(|_| generate_progress_token()).collect();

    // All tokens should be unique
    let unique_tokens: std::collections::HashSet<_> = tokens.iter().collect();
    assert_eq!(unique_tokens.len(), 100);
}

/// Test notification with null metadata value
#[test]
fn test_progress_notification_null_metadata_value() {
    let metadata = json!({
        "optional_field": null
    });

    let notification = ProgressNotification {
        progress_token: "test".to_string(),
        progress: Some(50),
        message: "Test".to_string(),
        metadata: Some(metadata),
    };

    let json = serde_json::to_value(&notification).unwrap();
    assert!(json["optional_field"].is_null());
}

/// Test that notification size is reasonable
#[test]
fn test_progress_notification_size() {
    let notification = ProgressNotification {
        progress_token: "test_token_123".to_string(),
        progress: Some(50),
        message: "Processing".to_string(),
        metadata: None,
    };

    let json_str = serde_json::to_string(&notification).unwrap();

    // Basic notification should be under 200 bytes
    assert!(json_str.len() < 200);
}

/// Test notification with zero progress
#[test]
fn test_progress_notification_zero_progress() {
    let notification = ProgressNotification {
        progress_token: "test".to_string(),
        progress: Some(0),
        message: "Starting".to_string(),
        metadata: None,
    };

    let json = serde_json::to_value(&notification).unwrap();
    assert_eq!(json["progress"], 0);
}

/// Test notification with 100 progress
#[test]
fn test_progress_notification_complete_progress() {
    let notification = ProgressNotification {
        progress_token: "test".to_string(),
        progress: Some(100),
        message: "Complete".to_string(),
        metadata: None,
    };

    let json = serde_json::to_value(&notification).unwrap();
    assert_eq!(json["progress"], 100);
}

/// Test that notification can be deserialized from minimal JSON
#[test]
fn test_progress_notification_minimal_deserialization() {
    let json_str = r#"{"progress_token":"test","message":"Test"}"#;

    let notification: ProgressNotification = serde_json::from_str(json_str).unwrap();

    assert_eq!(notification.progress_token, "test");
    assert_eq!(notification.message, "Test");
    assert!(notification.progress.is_none());
    // Metadata may be empty object due to flatten, which is fine
    if let Some(metadata) = &notification.metadata {
        // If present, should be an empty object
        assert!(metadata.as_object().map_or(true, |o| o.is_empty()));
    }
}

/// Test that notification can be deserialized from complete JSON
#[test]
fn test_progress_notification_complete_deserialization() {
    let json_str = r#"{"progress_token":"test","progress":50,"message":"Test","extra":"value"}"#;

    let notification: ProgressNotification = serde_json::from_str(json_str).unwrap();

    assert_eq!(notification.progress_token, "test");
    assert_eq!(notification.progress, Some(50));
    assert_eq!(notification.message, "Test");

    // Extra field should be captured in metadata
    assert!(notification.metadata.is_some());
    let metadata = notification.metadata.unwrap();
    assert_eq!(metadata["extra"], "value");
}

/// Test notification format consistency across different scenarios
#[test]
fn test_progress_notification_format_consistency() {
    let scenarios = vec![
        (Some(0), "Starting"),
        (Some(25), "25% complete"),
        (Some(50), "Halfway done"),
        (Some(75), "Almost there"),
        (Some(100), "Complete"),
        (None, "Working..."),
    ];

    for (progress, message) in scenarios {
        let notification = ProgressNotification {
            progress_token: "test".to_string(),
            progress,
            message: message.to_string(),
            metadata: None,
        };

        let json = serde_json::to_value(&notification).unwrap();

        // All should have these fields
        assert!(json.get("progress_token").is_some());
        assert!(json.get("message").is_some());

        // Progress field should match
        if progress.is_some() {
            assert_eq!(json["progress"], progress.unwrap());
        } else {
            assert!(json.get("progress").is_none());
        }
    }
}
