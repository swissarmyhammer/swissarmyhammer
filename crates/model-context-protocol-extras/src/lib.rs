//! model-context-protocol-extras - MCP notification capture for recording/playback
//!
//! Provides infrastructure to capture MCP notifications from:
//! - Our own ServerHandlers via `NotifyingServer<H>`
//! - Third-party MCP servers via `McpProxy`
//!
//! Both implement `McpNotificationSource` for uniform access to notification streams.

mod notification;
mod notifying_server;
mod proxy;

pub use notification::{McpNotification, McpNotificationSource};
pub use notifying_server::{start_notifying_server, NotifyingServer};
pub use proxy::{start_proxy, McpProxy};
