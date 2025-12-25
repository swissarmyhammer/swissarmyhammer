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

pub mod recording;
pub mod playback;
pub mod test_mcp_server;

pub use recording::RecordingAgent;
pub use playback::PlaybackAgent;
pub use test_mcp_server::TestMcpServer;

/// Wrap agent with fixture (recording or playback)
///
/// Returns PlaybackAgent if fixture exists, RecordingAgent if not.
pub fn with_fixture<A: AgentWithFixture + 'static>(
    agent: A,
    test_name: &str,
) -> Box<dyn AgentWithFixture> {
    let path = get_fixture_path_for(agent.agent_type(), test_name);

    if path.exists() {
        tracing::info!("Fixture exists, using playback: {:?}", path);
        Box::new(PlaybackAgent::new(path, agent.agent_type()))
    } else {
        tracing::info!("Fixture missing, using recording: {:?}", path);
        Box::new(RecordingAgent::new(agent, path))
    }
}

/// Marker trait for agents used in conformance testing
///
/// Identifies which agent type for fixture organization.
pub trait AgentWithFixture: Agent {
    /// Agent type identifier for fixture organization (e.g., "claude", "llama")
    fn agent_type(&self) -> &'static str;
}

/// Get fixture path for a test
///
/// Constructs path as: `.fixtures/<agent_type>/<test_name>.json`
///
/// # Example
///
/// ```
/// use agent_client_protocol_extras::get_fixture_path_for;
///
/// let path = get_fixture_path_for("claude", "test_basic_prompt");
/// // Returns: .fixtures/claude/test_basic_prompt.json
/// ```
pub fn get_fixture_path_for(agent_type: &str, test_name: &str) -> PathBuf {
    PathBuf::from(".fixtures")
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

/// Resolve fixture path to mode (internal helper for trait)
fn resolve_fixture_path(fixture_path: Option<PathBuf>) -> FixtureMode {
    match fixture_path {
        Some(path) => {
            if path.exists() {
                tracing::info!("Fixture exists, using playback: {:?}", path);
                FixtureMode::Playback { path }
            } else {
                tracing::info!("Fixture missing, using record: {:?}", path);
                FixtureMode::Record { path }
            }
        }
        None => {
            tracing::info!("No fixture path provided, using normal mode");
            FixtureMode::Normal
        }
    }
}

/// Extract test name from current thread name
///
/// Parses thread names like "test_basic_prompt_response::case_1_llama_agent"
/// and returns "test_basic_prompt_response"
pub fn get_test_name_from_thread() -> String {
    std::thread::current()
        .name()
        .and_then(|name| name.split("::").next())
        .unwrap_or("unknown_test")
        .to_string()
}
