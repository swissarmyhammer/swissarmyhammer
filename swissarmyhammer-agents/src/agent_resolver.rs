//! Agent resolver — discovers agents from builtin → local → user sources
//!
//! Precedence: builtin < local < user (later sources override earlier ones)

use crate::agent::{Agent, AgentSource};
use crate::agent_loader::{load_agent_from_builtin, load_agent_from_dir};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// Include the generated builtin agents
include!(concat!(env!("OUT_DIR"), "/builtin_agents.rs"));

/// Resolves agents from all sources with proper override precedence
pub struct AgentResolver {
    /// Additional search paths beyond defaults
    extra_paths: Vec<PathBuf>,
}

impl AgentResolver {
    pub fn new() -> Self {
        Self {
            extra_paths: Vec::new(),
        }
    }

    /// Add an extra search path for agents
    pub fn add_search_path(&mut self, path: PathBuf) {
        self.extra_paths.push(path);
    }

    /// Resolve all agents from all sources
    ///
    /// Returns agents keyed by name. Later sources override earlier ones.
    /// Precedence: builtin → local → user
    pub fn resolve_all(&self) -> HashMap<String, Agent> {
        let mut agents = HashMap::new();

        // 1. Load builtins (lowest precedence)
        self.load_builtins(&mut agents);

        // 2. Load from local project paths
        self.load_from_local_paths(&mut agents);

        // 3. Load from user-level paths
        self.load_from_user_paths(&mut agents);

        // 4. Load from extra paths
        for path in &self.extra_paths {
            self.load_from_directory(path, AgentSource::Local, &mut agents);
        }

        agents
    }

    /// Resolve only builtin agents (no local/user overrides)
    pub fn resolve_builtins(&self) -> HashMap<String, Agent> {
        let mut agents = HashMap::new();
        self.load_builtins(&mut agents);
        agents
    }

    /// Load builtin agents embedded in the binary
    fn load_builtins(&self, agents: &mut HashMap<String, Agent>) {
        let builtin_files = get_builtin_agents();

        // Group files by agent name (directory prefix)
        let mut agent_groups: HashMap<String, Vec<(&str, &str)>> = HashMap::new();

        for (name, content) in &builtin_files {
            let agent_name = if let Some(pos) = name.find('/') {
                &name[..pos]
            } else {
                name
            };

            agent_groups
                .entry(agent_name.to_string())
                .or_default()
                .push((name, content));
        }

        for (agent_name, files) in &agent_groups {
            match load_agent_from_builtin(agent_name, files) {
                Ok(agent) => {
                    tracing::debug!("Loaded builtin agent: {}", agent.name);
                    agents.insert(agent.name.as_str().to_string(), agent);
                }
                Err(e) => {
                    tracing::warn!("Failed to load builtin agent '{}': {}", agent_name, e);
                }
            }
        }
    }

    /// Load agents from project-local paths
    fn load_from_local_paths(&self, agents: &mut HashMap<String, Agent>) {
        let cwd = std::env::current_dir().unwrap_or_default();

        let dot_agents = cwd.join(".agents");
        self.load_from_directory(&dot_agents, AgentSource::Local, agents);

        let sah_agents = cwd.join(".swissarmyhammer").join("agents");
        self.load_from_directory(&sah_agents, AgentSource::Local, agents);
    }

    /// Load agents from user-level paths
    fn load_from_user_paths(&self, agents: &mut HashMap<String, Agent>) {
        if let Some(home) = dirs::home_dir() {
            let user_agents = home.join(".agents");
            self.load_from_directory(&user_agents, AgentSource::User, agents);

            let user_sah_agents = home.join(".swissarmyhammer").join("agents");
            self.load_from_directory(&user_sah_agents, AgentSource::User, agents);
        }
    }

    /// Load all agents from a directory (each subdirectory is an agent)
    fn load_from_directory(
        &self,
        dir: &Path,
        source: AgentSource,
        agents: &mut HashMap<String, Agent>,
    ) {
        if !dir.is_dir() {
            return;
        }

        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) => {
                tracing::warn!("Failed to read agents directory {}: {}", dir.display(), e);
                return;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                match load_agent_from_dir(&path, source.clone()) {
                    Ok(agent) => {
                        tracing::debug!(
                            "Loaded {} agent: {} from {}",
                            source,
                            agent.name,
                            path.display()
                        );
                        agents.insert(agent.name.as_str().to_string(), agent);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load agent from {}: {}", path.display(), e);
                    }
                }
            }
        }
    }
}

impl Default for AgentResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_builtins() {
        let resolver = AgentResolver::new();
        let agents = resolver.resolve_builtins();

        assert!(agents.contains_key("default"), "should have default agent");
        assert!(agents.contains_key("test"), "should have test agent");
        assert!(agents.contains_key("tester"), "should have tester agent");
        assert!(agents.contains_key("planner"), "should have planner agent");
        assert!(
            agents.contains_key("committer"),
            "should have committer agent"
        );
        assert!(
            agents.contains_key("reviewer"),
            "should have reviewer agent"
        );
        assert!(agents.contains_key("explore"), "should have explore agent");
    }

    #[test]
    fn test_builtin_agent_content() {
        let resolver = AgentResolver::new();
        let agents = resolver.resolve_builtins();

        let test = agents.get("test").unwrap();
        assert_eq!(test.name.as_str(), "test");
        assert!(!test.description.is_empty());
        assert!(!test.instructions.is_empty());
        assert_eq!(test.source, AgentSource::Builtin);
        assert_eq!(test.max_turns, Some(25));
    }
}
