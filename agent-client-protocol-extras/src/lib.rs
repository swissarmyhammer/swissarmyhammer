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

/// Trait for agents that support fixture recording and playback
///
/// This trait is called on an agent instance to configure it for fixture mode.
///
/// # Usage
///
/// ```ignore
/// let mut agent = MyAgent::new(...);
/// agent.with_fixture("test_basic_prompt");  // Auto-detects record vs playback
/// ```
pub trait AgentWithFixture: Agent {
    /// Agent type identifier for fixture organization (e.g., "claude", "llama")
    fn agent_type(&self) -> &'static str;

    /// Configure agent to use fixture for a test
    ///
    /// Automatically detects record vs playback mode based on file existence.
    /// Fixture path: `.fixtures/<agent_type>/<test_name>.json`
    ///
    /// # Arguments
    ///
    /// * `test_name` - Test name (e.g., "test_basic_prompt_response")
    ///
    /// # Behavior
    ///
    /// - If fixture exists: Configures agent for playback mode
    /// - If fixture missing: Configures agent for record mode (will create fixture)
    fn with_fixture(&mut self, test_name: &str);

    /// Get the fixture path for this agent and test (helper)
    ///
    /// Returns `.fixtures/<agent_type>/<test_name>.json`
    fn fixture_path(&self, test_name: &str) -> PathBuf {
        get_fixture_path_for(self.agent_type(), test_name)
    }

    /// Determine fixture mode for a test (helper)
    ///
    /// - If fixture exists: Playback mode
    /// - If fixture missing: Record mode
    fn fixture_mode(&self, test_name: &str) -> FixtureMode {
        let path = self.fixture_path(test_name);
        resolve_fixture_path(Some(path))
    }
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
