//! Shared fixture recording and playback infrastructure for ACP agents
//!
//! # Simple API
//!
//! Agents implement `AgentWithFixture` trait which provides automatic
//! fixture recording and playback:
//!
//! ```ignore
//! use agent_client_protocol_extras::AgentWithFixture;
//!
//! // Just pass test name - auto-detects record vs playback
//! let agent = MyAgent::with_fixture("test_basic_prompt").await?;
//!
//! // Fixture path is auto-constructed as:
//! // .fixtures/<agent_type>/test_basic_prompt.json
//!
//! // If file exists -> playback mode (fast)
//! // If file missing -> record mode (creates fixture)
//! ```

use agent_client_protocol::Agent;
use std::path::PathBuf;
use swissarmyhammer_common::Pretty;

pub mod playback;
pub mod recording;
pub mod test_mcp_server;
pub mod tracing_agent;

pub use playback::PlaybackAgent;
pub use recording::RecordingAgent;
pub use test_mcp_server::{start_test_mcp_server, TestMcpServer};
pub use tracing_agent::{trace_notifications, TracingAgent};

// Re-export MCP notification types for convenience
pub use model_context_protocol_extras::{
    start_proxy, McpNotification, McpNotificationSource, McpProxy,
};

/// Result of starting a test MCP server with notification capture
pub struct TestMcpServerWithCapture {
    /// URL where clients should connect (the proxy URL)
    pub url: String,
    /// The proxy that captures notifications
    pub proxy: McpProxy,
}

impl McpNotificationSource for TestMcpServerWithCapture {
    fn url(&self) -> &str {
        &self.url
    }

    fn subscribe(&self) -> tokio::sync::broadcast::Receiver<McpNotification> {
        self.proxy.subscribe()
    }
}

/// Start a test MCP server with notification capture via proxy
///
/// This starts TestMcpServer and wraps it with McpProxy for notification capture.
/// Use this for recording tests where you need to capture MCP notifications.
///
/// # Returns
/// * `TestMcpServerWithCapture` - provides URL and notification subscription
pub async fn start_test_mcp_server_with_capture(
) -> Result<TestMcpServerWithCapture, Box<dyn std::error::Error + Send + Sync>> {
    // Start the actual test MCP server
    let server_url =
        start_test_mcp_server()
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                Box::new(std::io::Error::other(e.to_string()))
            })?;

    // Wrap with proxy for notification capture
    let proxy = start_proxy(&server_url).await?;
    let url = proxy.url().to_string();

    tracing::info!(
        "TestMcpServer with capture: {} -> proxy at {}",
        server_url,
        url
    );

    Ok(TestMcpServerWithCapture { url, proxy })
}

/// Wrap agent with fixture (recording or playback)
///
/// Returns PlaybackAgent if fixture exists, RecordingAgent if not.
pub fn with_fixture<A: AgentWithFixture + 'static>(
    agent: A,
    test_name: &str,
) -> Box<dyn AgentWithFixture> {
    let path = get_fixture_path_for(agent.agent_type(), test_name);

    if path.exists() {
        tracing::info!("Fixture exists, using playback: {}", Pretty(&path));
        Box::new(PlaybackAgent::new(path, agent.agent_type()))
    } else {
        tracing::info!("Fixture missing, using recording: {}", Pretty(&path));
        Box::new(RecordingAgent::new(agent, path))
    }
}

/// Marker trait for agents used in conformance testing
///
/// Identifies which agent type for fixture organization.
pub trait AgentWithFixture: Agent {
    /// Agent type identifier for fixture organization (e.g., "claude", "llama")
    fn agent_type(&self) -> &'static str;

    /// Returns true if this agent is in playback mode (replaying from fixture)
    ///
    /// When true, tests should skip verification of side effects (like file creation)
    /// since playback mode doesn't actually perform those operations.
    fn is_playback(&self) -> bool {
        false // Default: not playback (real agent or recording)
    }
}

/// Get fixture path for a test
///
/// Constructs path as: `<package_root>/.fixtures/<agent_type>/<test_name>.json`
///
/// Uses CARGO_MANIFEST_DIR to ensure fixtures are always saved to the package
/// directory, regardless of current working directory changes during test execution.
///
/// # Example
///
/// ```
/// use agent_client_protocol_extras::get_fixture_path_for;
///
/// let path = get_fixture_path_for("claude", "test_basic_prompt");
/// // Returns: /path/to/package/.fixtures/claude/test_basic_prompt.json
/// ```
pub fn get_fixture_path_for(agent_type: &str, test_name: &str) -> PathBuf {
    // Use CARGO_MANIFEST_DIR if available (set during tests)
    // Otherwise fall back to current directory
    let base_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    base_dir
        .join(".fixtures")
        .join(agent_type)
        .join(format!("{}.json", test_name))
}

/// Fixture operation mode
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixtureMode {
    /// Normal mode - real agent operations
    Normal,
    /// Record mode - capture interactions to fixture
    Record { path: PathBuf },
    /// Playback mode - replay from fixture
    Playback { path: PathBuf },
}

/// Extract test name from current thread name
///
/// Parses thread names like "test_basic_prompt_response::case_1_llama_agent"
/// or "integration::file_system::test_basic_prompt_response::case_1_llama_agent"
/// and returns "test_basic_prompt_response"
pub fn get_test_name_from_thread() -> String {
    std::thread::current()
        .name()
        .and_then(|name| {
            let parts: Vec<&str> = name.split("::").collect();
            // Find the part before the case (e.g., "case_1_llama")
            // This handles both "test_name::case_x" and "module::test_name::case_x"
            parts
                .iter()
                .rev()
                .skip(1) // Skip the case part (last element)
                .find(|part| part.starts_with("test_"))
                .or_else(|| parts.iter().rev().nth(1)) // Fallback to second-to-last
                .copied()
        })
        .unwrap_or("unknown_test")
        .to_string()
}
