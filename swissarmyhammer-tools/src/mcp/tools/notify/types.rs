//! Request and response types for notification MCP operations

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Notification level enum for type safety
///
/// # Examples
///
/// Create a notification level:
/// ```ignore
/// let level = NotifyLevel::Warn;
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NotifyLevel {
    /// Informational message (default)
    Info,
    /// Warning message
    Warn,
    /// Error message
    Error,
}

impl Default for NotifyLevel {
    fn default() -> Self {
        Self::Info
    }
}

impl From<&str> for NotifyLevel {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "warn" => Self::Warn,
            "error" => Self::Error,
            _ => Self::Info, // Default to info for invalid values
        }
    }
}

impl From<Option<String>> for NotifyLevel {
    fn from(s: Option<String>) -> Self {
        match s {
            Some(level) => Self::from(level.as_str()),
            None => Self::default(),
        }
    }
}

impl From<NotifyLevel> for &'static str {
    fn from(level: NotifyLevel) -> Self {
        match level {
            NotifyLevel::Info => "info",
            NotifyLevel::Warn => "warn",
            NotifyLevel::Error => "error",
        }
    }
}

/// Request to create a notification
///
/// # Examples
///
/// Create a basic info notification:
/// ```ignore
/// NotifyRequest {
///     message: "Processing large codebase - this may take a few minutes".to_string(),
///     level: None, // Defaults to info
///     context: None,
/// }
/// ```
///
/// Create a warning notification with context:
/// ```ignore
/// NotifyRequest {
///     message: "Found potential security vulnerability in authentication logic".to_string(),
///     level: Some("warn".to_string()),
///     context: Some(serde_json::json!({"stage": "analysis", "safety": "critical"})),
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NotifyRequest {
    /// The message to notify the user about (required, must not be empty)
    pub message: String,

    /// The notification level: "info", "warn", or "error" (default: "info")
    #[serde(default)]
    pub level: Option<String>,

    /// Optional structured JSON data for the notification
    #[serde(default)]
    pub context: Option<JsonValue>,
}

impl NotifyRequest {
    /// Create a new NotifyRequest with the given message
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            level: None,
            context: None,
        }
    }

    /// Set the notification level
    pub fn with_level(mut self, level: impl Into<String>) -> Self {
        self.level = Some(level.into());
        self
    }

    /// Set the context data
    pub fn with_context(mut self, context: JsonValue) -> Self {
        self.context = Some(context);
        self
    }

    /// Get the notification level as a typed enum
    pub fn get_level(&self) -> NotifyLevel {
        NotifyLevel::from(self.level.clone())
    }

    /// Validate the request
    pub fn validate(&self) -> Result<(), String> {
        if self.message.trim().is_empty() {
            return Err("Message cannot be empty".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_notify_level_default() {
        assert_eq!(NotifyLevel::default(), NotifyLevel::Info);
    }

    #[test]
    fn test_notify_level_from_string() {
        assert_eq!(NotifyLevel::from("info"), NotifyLevel::Info);
        assert_eq!(NotifyLevel::from("warn"), NotifyLevel::Warn);
        assert_eq!(NotifyLevel::from("error"), NotifyLevel::Error);
        assert_eq!(NotifyLevel::from("invalid"), NotifyLevel::Info);
        assert_eq!(NotifyLevel::from(""), NotifyLevel::Info);
    }

    #[test]
    fn test_notify_level_from_option_string() {
        assert_eq!(
            NotifyLevel::from(Some("warn".to_string())),
            NotifyLevel::Warn
        );
        assert_eq!(NotifyLevel::from(None), NotifyLevel::Info);
    }

    #[test]
    fn test_notify_level_to_string() {
        let info: &str = NotifyLevel::Info.into();
        let warn: &str = NotifyLevel::Warn.into();
        let error: &str = NotifyLevel::Error.into();

        assert_eq!(info, "info");
        assert_eq!(warn, "warn");
        assert_eq!(error, "error");
    }

    #[test]
    fn test_notify_level_serialization() {
        let level = NotifyLevel::Warn;
        let json = serde_json::to_string(&level).unwrap();
        assert_eq!(json, "\"warn\"");

        let deserialized: NotifyLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, NotifyLevel::Warn);
    }

    #[test]
    fn test_notify_request_new() {
        let request = NotifyRequest::new("Test message");
        assert_eq!(request.message, "Test message");
        assert_eq!(request.level, None);
        assert_eq!(request.context, None);
    }

    #[test]
    fn test_notify_request_builder_pattern() {
        let request = NotifyRequest::new("Test message")
            .with_level("warn")
            .with_context(json!({"stage": "test"}));

        assert_eq!(request.message, "Test message");
        assert_eq!(request.level, Some("warn".to_string()));
        assert_eq!(request.context, Some(json!({"stage": "test"})));
    }

    #[test]
    fn test_notify_request_get_level() {
        let request1 = NotifyRequest::new("Test").with_level("warn");
        assert_eq!(request1.get_level(), NotifyLevel::Warn);

        let request2 = NotifyRequest::new("Test");
        assert_eq!(request2.get_level(), NotifyLevel::Info);

        let request3 = NotifyRequest::new("Test").with_level("invalid");
        assert_eq!(request3.get_level(), NotifyLevel::Info);
    }

    #[test]
    fn test_notify_request_validation() {
        let valid_request = NotifyRequest::new("Valid message");
        assert!(valid_request.validate().is_ok());

        let empty_request = NotifyRequest::new("");
        assert!(empty_request.validate().is_err());

        let whitespace_request = NotifyRequest::new("   ");
        assert!(whitespace_request.validate().is_err());
    }

    #[test]
    fn test_notify_request_serialization_minimal() {
        let request = NotifyRequest::new("Test notification message");

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: NotifyRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(request.message, deserialized.message);
        assert_eq!(request.level, deserialized.level);
        assert_eq!(request.context, deserialized.context);
    }

    #[test]
    fn test_notify_request_serialization_full() {
        let request = NotifyRequest {
            message: "Test notification message".to_string(),
            level: Some("warn".to_string()),
            context: Some(json!({"stage": "analysis", "file_count": 42})),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: NotifyRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(request.message, deserialized.message);
        assert_eq!(request.level, deserialized.level);
        assert_eq!(request.context, deserialized.context);
    }

    #[test]
    fn test_notify_request_serialization_with_defaults() {
        let json = r#"{"message": "Test message"}"#;
        let deserialized: NotifyRequest = serde_json::from_str(json).unwrap();

        assert_eq!(deserialized.message, "Test message");
        assert_eq!(deserialized.level, None);
        assert_eq!(deserialized.context, None);
    }

    #[test]
    fn test_notify_request_complex_context() {
        let complex_context = json!({
            "nested": {
                "data": "value",
                "numbers": [1, 2, 3],
                "boolean": true
            },
            "array": ["a", "b", "c"]
        });

        let request =
            NotifyRequest::new("Complex notification").with_context(complex_context.clone());

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: NotifyRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.context, Some(complex_context));
        assert!(deserialized.context.unwrap()["nested"]["data"] == "value");
    }

    #[test]
    fn test_notify_request_unicode_message() {
        let unicode_message = "通知消息 🔔 with émojis and ñoñ-ASCII characters";
        let request = NotifyRequest::new(unicode_message);

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: NotifyRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.message, unicode_message);
    }

    #[test]
    fn test_notify_request_long_message() {
        let long_message = "x".repeat(10000);
        let request = NotifyRequest::new(long_message.clone());

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: NotifyRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.message, long_message);
    }

    #[test]
    fn test_case_insensitive_level_parsing() {
        assert_eq!(NotifyLevel::from("INFO"), NotifyLevel::Info);
        assert_eq!(NotifyLevel::from("WARN"), NotifyLevel::Warn);
        assert_eq!(NotifyLevel::from("ERROR"), NotifyLevel::Error);
        assert_eq!(NotifyLevel::from("Warn"), NotifyLevel::Warn);
        assert_eq!(NotifyLevel::from("Error"), NotifyLevel::Error);
    }
}
