//! AgentLibrary — stores and retrieves resolved agents

use crate::agent::Agent;
use crate::agent_resolver::AgentResolver;
use std::collections::HashMap;

/// In-memory agent library populated by the resolver
pub struct AgentLibrary {
    agents: HashMap<String, Agent>,
}

impl AgentLibrary {
    /// Create a new empty library
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
        }
    }

    /// Load all agents using the default resolver
    pub fn load_defaults(&mut self) {
        let resolver = AgentResolver::new();
        self.agents = resolver.resolve_all();
        tracing::debug!("AgentLibrary loaded {} agents", self.agents.len());
    }

    /// Load agents using a custom resolver
    pub fn load_with_resolver(&mut self, resolver: &AgentResolver) {
        self.agents = resolver.resolve_all();
    }

    /// Get an agent by name
    pub fn get(&self, name: &str) -> Option<&Agent> {
        self.agents.get(name)
    }

    /// List all available agents
    pub fn list(&self) -> Vec<&Agent> {
        let mut agents: Vec<_> = self.agents.values().collect();
        agents.sort_by_key(|a| a.name.as_str());
        agents
    }

    /// Get the number of loaded agents
    pub fn len(&self) -> usize {
        self.agents.len()
    }

    /// Check if the library is empty
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }

    /// Get all agent names
    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<_> = self.agents.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }
}

impl Default for AgentLibrary {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_load_defaults() {
        let mut library = AgentLibrary::new();
        library.load_defaults();

        assert!(!library.is_empty());
        assert!(library.get("default").is_some());
        assert!(library.get("test").is_some());
        assert!(library.get("nonexistent").is_none());
    }

    #[test]
    fn test_library_list() {
        let mut library = AgentLibrary::new();
        library.load_defaults();

        let agents = library.list();
        assert!(!agents.is_empty());

        // Should be sorted
        let names: Vec<_> = agents.iter().map(|a| a.name.as_str()).collect();
        let mut sorted_names = names.clone();
        sorted_names.sort();
        assert_eq!(names, sorted_names);
    }
}
