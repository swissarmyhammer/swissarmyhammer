//! Common validation helpers for ACP conformance tests.
//!
//! This module provides reusable validation functions that can be used across
//! different test modules to reduce code duplication and ensure consistent
//! validation behavior.

use crate::{Error, Result};
use serde_json::Value;

/// Validates that a capability is supported before proceeding with a test.
///
/// If the capability is not supported, logs an info message and returns Ok(false).
/// This allows tests to gracefully skip when features are not available.
///
/// # Arguments
///
/// * `capability` - Whether the capability is supported
/// * `capability_name` - Name of the capability for logging
///
/// # Returns
///
/// Ok(true) if capability is supported, Ok(false) if not supported
pub fn validate_capability_supported(capability: bool, capability_name: &str) -> Result<bool> {
    if !capability {
        tracing::info!(
            "{} capability not supported - test may be skipped",
            capability_name
        );
        return Ok(false);
    }
    Ok(true)
}

/// Validates that an agent supports a required capability before executing a test.
///
/// This helper is designed to be used at the start of conformance tests that depend
/// on specific agent capabilities. If the capability is not supported, it returns
/// Ok(None), indicating the test should be skipped gracefully.
///
/// # Arguments
///
/// * `capability_check` - A closure that extracts the boolean capability from capabilities struct
/// * `capability_name` - Human-readable name of the capability for logging
///
/// # Returns
///
/// * Ok(Some(())) if capability is supported and test should proceed
/// * Ok(None) if capability is not supported and test should be skipped
/// * Err if there's a validation error
///
/// # Example
///
/// ```ignore
/// use acp_conformance::validation::require_capability;
///
/// async fn test_file_read(agent: &impl Agent) -> Result<()> {
///     let init = agent.initialize(...).await?;
///
///     // Skip test if agent doesn't support file system operations
///     if require_capability(
///         init.agent_capabilities.ext_capabilities.file_system,
///         "file_system"
///     )?.is_none() {
///         return Ok(());
///     }
///
///     // Continue with test...
/// }
/// ```
pub fn require_capability(capability_supported: bool, capability_name: &str) -> Result<Option<()>> {
    if !capability_supported {
        tracing::info!(
            "{} capability not supported - skipping test",
            capability_name
        );
        return Ok(None);
    }
    Ok(Some(()))
}

/// Validates that a session ID is not empty.
///
/// # Arguments
///
/// * `session_id` - The session ID to validate
///
/// # Returns
///
/// Ok(()) if valid, Error::Validation if empty
pub fn validate_session_id(session_id: &str) -> Result<()> {
    if session_id.is_empty() {
        return Err(Error::Validation(
            "Session ID must not be empty".to_string(),
        ));
    }
    Ok(())
}

/// Extracts a required field from a JSON value.
///
/// # Arguments
///
/// * `value` - The JSON value to extract from
/// * `field` - The field name to extract
///
/// # Returns
///
/// Ok(&Value) if field exists, Error::Validation if missing
pub fn require_field<'a>(value: &'a Value, field: &str) -> Result<&'a Value> {
    value.get(field).ok_or_else(|| {
        Error::Validation(format!("Required field '{}' missing from response", field))
    })
}

/// Extracts a required string field from a JSON value.
///
/// # Arguments
///
/// * `value` - The JSON value to extract from
/// * `field` - The field name to extract
///
/// # Returns
///
/// Ok(&str) if field exists and is a string, Error::Validation otherwise
pub fn require_string_field<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    require_field(value, field)?
        .as_str()
        .ok_or_else(|| Error::Validation(format!("Field '{}' must be a string", field)))
}

/// Extracts a required boolean field from a JSON value.
///
/// # Arguments
///
/// * `value` - The JSON value to extract from
/// * `field` - The field name to extract
///
/// # Returns
///
/// Ok(bool) if field exists and is a boolean, Error::Validation otherwise
pub fn require_bool_field(value: &Value, field: &str) -> Result<bool> {
    require_field(value, field)?
        .as_bool()
        .ok_or_else(|| Error::Validation(format!("Field '{}' must be a boolean", field)))
}

/// Extracts a required number field from a JSON value as u64.
///
/// # Arguments
///
/// * `value` - The JSON value to extract from
/// * `field` - The field name to extract
///
/// # Returns
///
/// Ok(u64) if field exists and is a number, Error::Validation otherwise
pub fn require_number_field(value: &Value, field: &str) -> Result<u64> {
    require_field(value, field)?
        .as_u64()
        .ok_or_else(|| Error::Validation(format!("Field '{}' must be a number", field)))
}

/// Extracts a required signed number field from a JSON value as i64.
///
/// This is useful for fields that may contain negative values, such as
/// exit codes from terminal processes.
///
/// # Arguments
///
/// * `value` - The JSON value to extract from
/// * `field` - The field name to extract
///
/// # Returns
///
/// Ok(i64) if field exists and is a number, Error::Validation otherwise
pub fn require_i64_field(value: &Value, field: &str) -> Result<i64> {
    require_field(value, field)?
        .as_i64()
        .ok_or_else(|| Error::Validation(format!("Field '{}' must be a number", field)))
}

/// Validates that a JSON array field exists and is non-empty.
///
/// # Arguments
///
/// * `value` - The JSON value to check
/// * `field` - The array field name to validate
///
/// # Returns
///
/// Ok(&Vec<Value>) if field is a non-empty array, Error::Validation otherwise
pub fn require_non_empty_array<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>> {
    let array = require_field(value, field)?
        .as_array()
        .ok_or_else(|| Error::Validation(format!("Field '{}' must be an array", field)))?;

    if array.is_empty() {
        return Err(Error::Validation(format!(
            "Field '{}' must not be empty",
            field
        )));
    }

    Ok(array)
}

/// Validates that multiple required fields exist in a JSON value.
///
/// # Arguments
///
/// * `value` - The JSON value to check
/// * `fields` - Slice of field names that must exist
///
/// # Returns
///
/// Ok(()) if all fields exist, Error::Validation with details of missing fields
pub fn validate_required_fields(value: &Value, fields: &[&str]) -> Result<()> {
    let missing_fields: Vec<&str> = fields
        .iter()
        .filter(|&&field| value.get(field).is_none())
        .copied()
        .collect();

    if !missing_fields.is_empty() {
        return Err(Error::Validation(format!(
            "Missing required fields: {}",
            missing_fields.join(", ")
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_validate_capability_supported() {
        assert!(validate_capability_supported(true, "test").unwrap());
        assert!(!validate_capability_supported(false, "test").unwrap());
    }

    #[test]
    fn test_require_capability() {
        // Test with supported capability - should return Some(())
        let result = require_capability(true, "test_capability");
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());

        // Test with unsupported capability - should return None
        let result = require_capability(false, "test_capability");
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_validate_session_id() {
        assert!(validate_session_id("valid-id").is_ok());
        assert!(validate_session_id("").is_err());
    }

    #[test]
    fn test_require_field() {
        let value = json!({"name": "test", "count": 42});

        assert!(require_field(&value, "name").is_ok());
        assert!(require_field(&value, "missing").is_err());
    }

    #[test]
    fn test_require_string_field() {
        let value = json!({"name": "test", "count": 42});

        assert_eq!(require_string_field(&value, "name").unwrap(), "test");
        assert!(require_string_field(&value, "count").is_err());
        assert!(require_string_field(&value, "missing").is_err());
    }

    #[test]
    fn test_require_bool_field() {
        let value = json!({"enabled": true, "name": "test"});

        assert_eq!(require_bool_field(&value, "enabled").unwrap(), true);
        assert!(require_bool_field(&value, "name").is_err());
        assert!(require_bool_field(&value, "missing").is_err());
    }

    #[test]
    fn test_require_number_field() {
        let value = json!({"count": 42, "name": "test"});

        assert_eq!(require_number_field(&value, "count").unwrap(), 42);
        assert!(require_number_field(&value, "name").is_err());
        assert!(require_number_field(&value, "missing").is_err());
    }

    #[test]
    fn test_require_non_empty_array() {
        let value = json!({
            "items": [1, 2, 3],
            "empty": [],
            "not_array": "string"
        });

        assert_eq!(require_non_empty_array(&value, "items").unwrap().len(), 3);
        assert!(require_non_empty_array(&value, "empty").is_err());
        assert!(require_non_empty_array(&value, "not_array").is_err());
        assert!(require_non_empty_array(&value, "missing").is_err());
    }

    #[test]
    fn test_validate_required_fields() {
        let value = json!({"name": "test", "count": 42});

        assert!(validate_required_fields(&value, &["name", "count"]).is_ok());
        assert!(validate_required_fields(&value, &["name"]).is_ok());
        assert!(validate_required_fields(&value, &["name", "missing"]).is_err());
        assert!(validate_required_fields(&value, &["missing1", "missing2"]).is_err());
    }

    #[test]
    fn test_require_string_field_with_unicode() {
        let value = json!({"text": "Hello 世界 🌍", "emoji": "😀"});

        assert_eq!(
            require_string_field(&value, "text").unwrap(),
            "Hello 世界 🌍"
        );
        assert_eq!(require_string_field(&value, "emoji").unwrap(), "😀");
    }

    #[test]
    fn test_require_string_field_with_empty_string() {
        let value = json!({"empty": "", "whitespace": "   "});

        assert_eq!(require_string_field(&value, "empty").unwrap(), "");
        assert_eq!(require_string_field(&value, "whitespace").unwrap(), "   ");
    }

    #[test]
    fn test_require_number_field_edge_cases() {
        let value = json!({
            "zero": 0,
            "max_u64": u64::MAX,
            "negative": -1,
            "float": 1.23
        });

        assert_eq!(require_number_field(&value, "zero").unwrap(), 0);
        assert_eq!(require_number_field(&value, "max_u64").unwrap(), u64::MAX);
        // Negative numbers and floats should fail for u64
        assert!(require_number_field(&value, "negative").is_err());
        assert!(require_number_field(&value, "float").is_err());
    }

    #[test]
    fn test_require_field_with_null_value() {
        let value = json!({"null_field": null, "valid": "data"});

        // A null value should be considered as existing but will fail type checks
        assert!(require_field(&value, "null_field").is_ok());
        assert!(require_string_field(&value, "null_field").is_err());
        assert!(require_bool_field(&value, "null_field").is_err());
        assert!(require_number_field(&value, "null_field").is_err());
    }

    #[test]
    fn test_require_non_empty_array_with_various_types() {
        let value = json!({
            "numbers": [1, 2, 3],
            "strings": ["a", "b"],
            "mixed": [1, "two", true, null],
            "nested": [[1, 2], [3, 4]]
        });

        assert_eq!(require_non_empty_array(&value, "numbers").unwrap().len(), 3);
        assert_eq!(require_non_empty_array(&value, "strings").unwrap().len(), 2);
        assert_eq!(require_non_empty_array(&value, "mixed").unwrap().len(), 4);
        assert_eq!(require_non_empty_array(&value, "nested").unwrap().len(), 2);
    }

    #[test]
    fn test_validate_session_id_edge_cases() {
        // Valid session IDs
        assert!(validate_session_id("a").is_ok());
        assert!(validate_session_id("session-123").is_ok());
        assert!(validate_session_id("sess_abc123def456").is_ok());
        assert!(validate_session_id("123").is_ok());
        assert!(validate_session_id("session with spaces").is_ok());

        // Invalid session IDs
        assert!(validate_session_id("").is_err());
    }

    #[test]
    fn test_validate_required_fields_empty_list() {
        let value = json!({"name": "test"});

        // Empty field list should succeed
        assert!(validate_required_fields(&value, &[]).is_ok());
    }

    #[test]
    fn test_require_bool_field_with_numbers() {
        let value = json!({"zero": 0, "one": 1, "bool": true});

        // Numbers should not be treated as booleans
        assert!(require_bool_field(&value, "zero").is_err());
        assert!(require_bool_field(&value, "one").is_err());
        assert_eq!(require_bool_field(&value, "bool").unwrap(), true);
    }

    #[test]
    fn test_nested_field_access() {
        let value = json!({
            "nested": {
                "inner": {
                    "value": "deep"
                }
            }
        });

        // require_field returns &Value, so we can chain accesses
        let nested = require_field(&value, "nested").unwrap();
        let inner = require_field(nested, "inner").unwrap();
        let deep_value = require_string_field(inner, "value").unwrap();
        assert_eq!(deep_value, "deep");
    }

    #[test]
    fn test_require_i64_field() {
        let value = json!({
            "positive": 42,
            "negative": -1,
            "zero": 0,
            "large_negative": -128,
            "string": "not a number"
        });

        // Valid i64 values
        assert_eq!(require_i64_field(&value, "positive").unwrap(), 42);
        assert_eq!(require_i64_field(&value, "negative").unwrap(), -1);
        assert_eq!(require_i64_field(&value, "zero").unwrap(), 0);
        assert_eq!(require_i64_field(&value, "large_negative").unwrap(), -128);

        // Invalid cases
        assert!(require_i64_field(&value, "string").is_err());
        assert!(require_i64_field(&value, "missing").is_err());
    }

    #[test]
    fn test_require_i64_field_with_signal_codes() {
        // Test exit codes that would be returned by killed processes
        let value = json!({
            "sigterm": 143,
            "sigkill": 137,
            "sigsegv": 139
        });

        assert_eq!(require_i64_field(&value, "sigterm").unwrap(), 143);
        assert_eq!(require_i64_field(&value, "sigkill").unwrap(), 137);
        assert_eq!(require_i64_field(&value, "sigsegv").unwrap(), 139);
    }

    #[test]
    fn test_validation_error_messages_are_descriptive() {
        let value = json!({"name": "test"});

        // Check error messages contain field names
        let err = require_field(&value, "missing").unwrap_err();
        assert!(err.to_string().contains("missing"));

        let err = require_string_field(&value, "nonexistent").unwrap_err();
        assert!(err.to_string().contains("nonexistent"));

        let err = require_bool_field(&value, "name").unwrap_err();
        assert!(err.to_string().contains("name"));
    }

    #[test]
    fn test_require_capability_logs_correctly() {
        // Test that require_capability returns correct values
        // (tracing output is tested via test_log in integration tests)
        assert_eq!(require_capability(true, "test").unwrap(), Some(()));
        assert_eq!(require_capability(false, "test").unwrap(), None);
    }

    #[test]
    fn test_validate_session_id_with_uuid_format() {
        // Common session ID formats
        assert!(validate_session_id("550e8400-e29b-41d4-a716-446655440000").is_ok());
        assert!(validate_session_id("01HZZZZZZZZZZZZZZZZZZZZZZ").is_ok()); // ULID format
        assert!(validate_session_id("sess_abc123").is_ok()); // Prefixed format
    }

    #[test]
    fn test_require_non_empty_array_with_single_element() {
        let value = json!({
            "single": [1],
            "empty": []
        });

        // Single element should be valid
        let arr = require_non_empty_array(&value, "single").unwrap();
        assert_eq!(arr.len(), 1);

        // Empty should fail
        assert!(require_non_empty_array(&value, "empty").is_err());
    }

    #[test]
    fn test_validate_required_fields_error_lists_all_missing() {
        let value = json!({"a": 1});

        let err = validate_required_fields(&value, &["a", "b", "c"]).unwrap_err();
        let msg = err.to_string();

        // Should list both missing fields
        assert!(msg.contains("b"));
        assert!(msg.contains("c"));
        // Should not list present field
        assert!(!msg.contains("Missing") || !msg.contains(": a,"));
    }
}
