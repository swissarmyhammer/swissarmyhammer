//! AgentContext — wraps AgentLibrary for operation execution

use crate::agent_library::AgentLibrary;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Context for agent operations, providing access to the agent library
pub struct AgentContext {
    pub library: Arc<RwLock<AgentLibrary>>,
}

impl AgentContext {
    /// Create a new context wrapping an agent library
    pub fn new(library: Arc<RwLock<AgentLibrary>>) -> Self {
        Self { library }
    }
}
