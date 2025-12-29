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
