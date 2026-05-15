//! MCP notification types and trait

use rmcp::model::{LoggingMessageNotificationParam, ProgressNotificationParam};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use tokio::sync::broadcast;

/// Captured MCP notification (raw MCP types)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum McpNotification {
    /// Progress notification from tool execution
    Progress(ProgressNotificationParam),
    /// Logging message from MCP server
    Log(LoggingMessageNotificationParam),
}

impl McpNotification {
    /// Create a deduplication key for this notification
    ///
    /// Used by RecordingAgent to detect duplicate notifications
    /// from multiple capture sources (server-side + client-side)
    pub fn dedup_key(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        match self {
            McpNotification::Progress(p) => {
                "progress".hash(&mut hasher);
                // Hash the progress token
                if let Some(ref token) = Some(&p.progress_token) {
                    format!("{:?}", token).hash(&mut hasher);
                }
                // Hash progress value (as bits to handle float)
                p.progress.to_bits().hash(&mut hasher);
                // Hash total if present
                if let Some(total) = p.total {
                    total.to_bits().hash(&mut hasher);
                }
                // Hash message if present
                if let Some(ref msg) = p.message {
                    msg.hash(&mut hasher);
                }
            }
            McpNotification::Log(l) => {
                "log".hash(&mut hasher);
                format!("{:?}", l.level).hash(&mut hasher);
                if let Some(ref logger) = l.logger {
                    logger.hash(&mut hasher);
                }
                // Hash data as string
                l.data.to_string().hash(&mut hasher);
            }
        }
        hasher.finish()
    }
}

/// Trait for sources that capture MCP notifications
///
/// Implemented by both `NotifyingServer<H>` (for our servers)
/// and `McpProxy` (for third-party servers).
pub trait McpNotificationSource: Send + Sync {
    /// URL where clients should connect to this MCP server
    fn url(&self) -> &str;

    /// Subscribe to captured MCP notifications
    ///
    /// Returns a broadcast receiver that will receive all notifications
    /// captured by this source. Multiple subscribers can be created.
    fn subscribe(&self) -> broadcast::Receiver<McpNotification>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::{LoggingLevel, NumberOrString, ProgressToken};
    use serde_json::json;

    /// Helper to create a Progress notification with all fields populated.
    fn make_progress(
        token: &str,
        progress: f64,
        total: Option<f64>,
        message: Option<&str>,
    ) -> McpNotification {
        McpNotification::Progress(ProgressNotificationParam {
            progress_token: ProgressToken(NumberOrString::String(token.into())),
            progress,
            total,
            message: message.map(|s| s.to_string()),
        })
    }

    /// Helper to create a Log notification.
    fn make_log(
        level: LoggingLevel,
        logger: Option<&str>,
        data: serde_json::Value,
    ) -> McpNotification {
        McpNotification::Log(LoggingMessageNotificationParam {
            level,
            logger: logger.map(|s| s.to_string()),
            data,
        })
    }

    #[test]
    fn dedup_key_progress_is_deterministic() {
        let a = make_progress("tok1", 50.0, Some(100.0), Some("halfway"));
        let b = make_progress("tok1", 50.0, Some(100.0), Some("halfway"));
        assert_eq!(a.dedup_key(), b.dedup_key());
    }

    #[test]
    fn dedup_key_log_is_deterministic() {
        let a = make_log(LoggingLevel::Info, Some("logger1"), json!("hello"));
        let b = make_log(LoggingLevel::Info, Some("logger1"), json!("hello"));
        assert_eq!(a.dedup_key(), b.dedup_key());
    }

    #[test]
    fn dedup_key_differs_for_different_progress_tokens() {
        let a = make_progress("tok1", 50.0, Some(100.0), None);
        let b = make_progress("tok2", 50.0, Some(100.0), None);
        assert_ne!(a.dedup_key(), b.dedup_key());
    }

    #[test]
    fn dedup_key_differs_for_different_progress_values() {
        let a = make_progress("tok1", 25.0, Some(100.0), None);
        let b = make_progress("tok1", 75.0, Some(100.0), None);
        assert_ne!(a.dedup_key(), b.dedup_key());
    }

    #[test]
    fn dedup_key_differs_for_different_totals() {
        let a = make_progress("tok1", 50.0, Some(100.0), None);
        let b = make_progress("tok1", 50.0, Some(200.0), None);
        assert_ne!(a.dedup_key(), b.dedup_key());
    }

    #[test]
    fn dedup_key_differs_with_and_without_total() {
        let a = make_progress("tok1", 50.0, Some(100.0), None);
        let b = make_progress("tok1", 50.0, None, None);
        assert_ne!(a.dedup_key(), b.dedup_key());
    }

    #[test]
    fn dedup_key_differs_for_different_messages() {
        let a = make_progress("tok1", 50.0, None, Some("msg1"));
        let b = make_progress("tok1", 50.0, None, Some("msg2"));
        assert_ne!(a.dedup_key(), b.dedup_key());
    }

    #[test]
    fn dedup_key_differs_with_and_without_message() {
        let a = make_progress("tok1", 50.0, None, Some("msg"));
        let b = make_progress("tok1", 50.0, None, None);
        assert_ne!(a.dedup_key(), b.dedup_key());
    }

    #[test]
    fn dedup_key_differs_for_different_log_levels() {
        let a = make_log(LoggingLevel::Info, None, json!("data"));
        let b = make_log(LoggingLevel::Error, None, json!("data"));
        assert_ne!(a.dedup_key(), b.dedup_key());
    }

    #[test]
    fn dedup_key_differs_for_different_loggers() {
        let a = make_log(LoggingLevel::Info, Some("logger1"), json!("data"));
        let b = make_log(LoggingLevel::Info, Some("logger2"), json!("data"));
        assert_ne!(a.dedup_key(), b.dedup_key());
    }

    #[test]
    fn dedup_key_differs_with_and_without_logger() {
        let a = make_log(LoggingLevel::Info, Some("logger1"), json!("data"));
        let b = make_log(LoggingLevel::Info, None, json!("data"));
        assert_ne!(a.dedup_key(), b.dedup_key());
    }

    #[test]
    fn dedup_key_differs_for_different_log_data() {
        let a = make_log(LoggingLevel::Info, None, json!("data1"));
        let b = make_log(LoggingLevel::Info, None, json!("data2"));
        assert_ne!(a.dedup_key(), b.dedup_key());
    }

    #[test]
    fn dedup_key_differs_between_progress_and_log() {
        let a = make_progress("test", 0.0, None, None);
        let b = make_log(LoggingLevel::Info, None, json!("test"));
        assert_ne!(a.dedup_key(), b.dedup_key());
    }

    #[test]
    fn progress_notification_serializes_roundtrip() {
        let notif = make_progress("tok", 50.0, Some(100.0), Some("halfway"));
        let json = serde_json::to_string(&notif).expect("serialize");
        let back: McpNotification = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(notif.dedup_key(), back.dedup_key());
    }

    #[test]
    fn log_notification_serializes_roundtrip() {
        let notif = make_log(
            LoggingLevel::Warning,
            Some("mylogger"),
            json!({"key": "val"}),
        );
        let json = serde_json::to_string(&notif).expect("serialize");
        let back: McpNotification = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(notif.dedup_key(), back.dedup_key());
    }

    #[test]
    fn progress_notification_debug_format() {
        let notif = make_progress("tok", 50.0, None, None);
        let debug = format!("{:?}", notif);
        assert!(debug.contains("Progress"));
    }

    #[test]
    fn log_notification_debug_format() {
        let notif = make_log(LoggingLevel::Debug, None, json!("msg"));
        let debug = format!("{:?}", notif);
        assert!(debug.contains("Log"));
    }

    #[test]
    fn progress_notification_clone() {
        let notif = make_progress("tok", 50.0, Some(100.0), Some("msg"));
        let cloned = notif.clone();
        assert_eq!(notif.dedup_key(), cloned.dedup_key());
    }

    #[test]
    fn log_notification_clone() {
        let notif = make_log(LoggingLevel::Error, Some("l"), json!("d"));
        let cloned = notif.clone();
        assert_eq!(notif.dedup_key(), cloned.dedup_key());
    }

    #[test]
    fn dedup_key_progress_with_numeric_token() {
        let notif = McpNotification::Progress(ProgressNotificationParam {
            progress_token: ProgressToken(NumberOrString::Number(42)),
            progress: 10.0,
            total: None,
            message: None,
        });
        // Should not panic, and should produce a stable key
        let key1 = notif.dedup_key();
        let key2 = notif.clone().dedup_key();
        assert_eq!(key1, key2);
    }

    #[test]
    fn dedup_key_log_with_complex_data() {
        let notif = make_log(
            LoggingLevel::Info,
            None,
            json!({"nested": {"key": [1, 2, 3]}}),
        );
        let key1 = notif.dedup_key();
        let key2 = notif.clone().dedup_key();
        assert_eq!(key1, key2);
    }
}
