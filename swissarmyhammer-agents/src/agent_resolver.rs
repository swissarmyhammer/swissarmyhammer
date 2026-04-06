//! Agent resolver — discovers agents from builtin → user → local sources
//!
//! Uses [`VirtualFileSystem`] for search-path resolution so that precedence
//! is handled consistently across all resolver types:
//!
//!   builtin  <  user (`$XDG_DATA_HOME/sah/agents`)  <  local (`{git_root}/.agents`)
//!
//! The VFS discovers `AGENT.md` files; each file's parent directory is then
//! loaded as a full agent (with resource files) via [`load_agent_from_dir`].

use crate::agent::{Agent, AgentSource};
use crate::agent_loader::{load_agent_from_builtin, load_agent_from_dir};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use swissarmyhammer_common::file_loader::{FileSource, VirtualFileSystem};

// Include the generated builtin agents
include!(concat!(env!("OUT_DIR"), "/builtin_agents.rs"));

/// Map a VFS [`FileSource`] to an [`AgentSource`].
fn file_source_to_agent_source(fs: &FileSource) -> AgentSource {
    match fs {
        FileSource::Builtin | FileSource::Dynamic => AgentSource::Builtin,
        FileSource::User => AgentSource::User,
        FileSource::Local => AgentSource::Local,
    }
}

/// Resolves agents from all sources with proper override precedence.
///
/// Internally delegates path discovery to a [`VirtualFileSystem`] configured
/// with `use_dot_directory_paths()`, giving the standard three-tier precedence:
///
/// 1. **Builtin** — agents embedded in the binary (lowest)
/// 2. **User** — `$XDG_DATA_HOME/sah/agents`
/// 3. **Local** — `{git_root}/.agents` (highest)
pub struct AgentResolver {
    /// VFS used for search-path resolution
    vfs: VirtualFileSystem,
}

impl AgentResolver {
    /// Create a new AgentResolver backed by a VirtualFileSystem.
    pub fn new() -> Self {
        let mut vfs = VirtualFileSystem::new("agents");
        vfs.use_dot_directory_paths();
        Self { vfs }
    }

    /// Add an extra search path for agents.
    ///
    /// Delegates to the VFS so that extra paths appear in
    /// `get_search_paths()` alongside user and local directories.
    /// Files found in extra paths are loaded with `Local` precedence.
    pub fn add_search_path(&mut self, path: PathBuf) {
        self.vfs.add_search_path(path, FileSource::Local);
    }

    /// Resolve all agents from all sources.
    ///
    /// Returns agents keyed by name. Later sources override earlier ones.
    /// Precedence: builtin < user < local (including extra paths added via
    /// [`add_search_path`]).
    pub fn resolve_all(&self) -> HashMap<String, Agent> {
        let mut agents = HashMap::new();

        // 1. Load builtins (lowest precedence)
        self.load_builtins(&mut agents);

        // 2. Load from VFS-resolved directories (user, local, and extra paths)
        self.load_from_vfs_directories(&mut agents);

        agents
    }

    /// Resolve only builtin agents (no user/local overrides).
    pub fn resolve_builtins(&self) -> HashMap<String, Agent> {
        let mut agents = HashMap::new();
        self.load_builtins(&mut agents);
        agents
    }

    /// Load builtin agents embedded in the binary.
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

    /// Load agents from VFS-resolved search paths.
    ///
    /// Uses `get_search_paths()` which returns paths with their [`FileSource`]
    /// metadata, so source classification comes from the VFS rather than
    /// string-matching on directory names.
    fn load_from_vfs_directories(&self, agents: &mut HashMap<String, Agent>) {
        for sp in self.vfs.get_search_paths() {
            let source = file_source_to_agent_source(&sp.source);
            load_agents_from_directory(&sp.path, source, agents);
        }
    }
}

/// Load all agents from a directory (each subdirectory is an agent).
fn load_agents_from_directory(
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

impl Default for AgentResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_resolve_builtins() {
        let resolver = AgentResolver::new();
        let agents = resolver.resolve_builtins();

        assert!(agents.contains_key("default"), "should have default agent");
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

        let tester = agents.get("tester").unwrap();
        assert_eq!(tester.name.as_str(), "tester");
        assert!(!tester.description.is_empty());
        assert!(!tester.instructions.is_empty());
        assert_eq!(tester.source, AgentSource::Builtin);
    }

    /// Helper to create an agent directory with an AGENT.md file.
    fn create_agent_dir(base: &Path, name: &str, description: &str) {
        let agent_dir = base.join(name);
        fs::create_dir_all(&agent_dir).unwrap();
        let content = format!(
            "---\nname: {}\ndescription: {}\n---\n\nInstructions for {}.\n",
            name, description, name
        );
        fs::write(agent_dir.join("AGENT.md"), content).unwrap();
    }

    #[test]
    fn test_extra_path_overrides_builtin() {
        let temp_dir = TempDir::new().unwrap();

        // Create an agent that shadows a builtin
        create_agent_dir(temp_dir.path(), "tester", "Custom tester from extra path");

        let mut resolver = AgentResolver::new();
        resolver.add_search_path(temp_dir.path().to_path_buf());

        let agents = resolver.resolve_all();
        let tester = agents.get("tester").unwrap();

        assert_eq!(tester.source, AgentSource::Local);
        assert_eq!(tester.description, "Custom tester from extra path");
    }

    #[test]
    fn test_local_overrides_user_overrides_builtin() {
        let user_dir = TempDir::new().unwrap();
        let local_dir = TempDir::new().unwrap();

        // Create the same agent in both directories
        create_agent_dir(user_dir.path(), "my-agent", "User version");
        create_agent_dir(local_dir.path(), "my-agent", "Local version");

        let mut agents = HashMap::new();

        // Simulate precedence: builtin < user < local
        load_agents_from_directory(user_dir.path(), AgentSource::User, &mut agents);
        load_agents_from_directory(local_dir.path(), AgentSource::Local, &mut agents);

        let agent = agents.get("my-agent").unwrap();
        assert_eq!(agent.source, AgentSource::Local);
        assert_eq!(agent.description, "Local version");
    }

    #[test]
    fn test_user_overrides_builtin() {
        let user_dir = TempDir::new().unwrap();

        // Create an agent that shadows a builtin
        create_agent_dir(user_dir.path(), "tester", "User tester override");

        let mut agents = HashMap::new();

        // Simulate: builtin first, then user overrides
        let resolver = AgentResolver::new();
        resolver.load_builtins(&mut agents);

        let builtin_tester = agents.get("tester").unwrap();
        assert_eq!(builtin_tester.source, AgentSource::Builtin);

        // Now user overrides
        load_agents_from_directory(user_dir.path(), AgentSource::User, &mut agents);

        let user_tester = agents.get("tester").unwrap();
        assert_eq!(user_tester.source, AgentSource::User);
        assert_eq!(user_tester.description, "User tester override");
    }

    #[test]
    fn test_file_source_to_agent_source_mapping() {
        assert_eq!(
            file_source_to_agent_source(&FileSource::Builtin),
            AgentSource::Builtin
        );
        assert_eq!(
            file_source_to_agent_source(&FileSource::Dynamic),
            AgentSource::Builtin
        );
        assert_eq!(
            file_source_to_agent_source(&FileSource::User),
            AgentSource::User
        );
        assert_eq!(
            file_source_to_agent_source(&FileSource::Local),
            AgentSource::Local
        );
    }

    #[test]
    fn test_resolver_default_is_same_as_new() {
        let resolver_new = AgentResolver::new();
        let resolver_default = AgentResolver::default();

        let agents_new = resolver_new.resolve_builtins();
        let agents_default = resolver_default.resolve_builtins();

        assert_eq!(agents_new.len(), agents_default.len());
        for key in agents_new.keys() {
            assert!(agents_default.contains_key(key));
        }
    }

    #[test]
    fn test_resolve_all_includes_builtins() {
        let resolver = AgentResolver::new();
        let agents = resolver.resolve_all();

        // resolve_all should include all the same builtins
        assert!(agents.contains_key("default"));
        assert!(agents.contains_key("tester"));
    }

    #[test]
    fn test_load_agents_from_directory_empty_dir() {
        let temp_dir = TempDir::new().unwrap();
        let mut agents = HashMap::new();

        // An empty directory should not load any agents
        load_agents_from_directory(temp_dir.path(), AgentSource::Local, &mut agents);
        assert!(agents.is_empty());
    }

    #[test]
    fn test_load_agents_from_nonexistent_directory() {
        let mut agents = HashMap::new();

        // A non-existent path should be silently skipped
        load_agents_from_directory(
            std::path::Path::new("/tmp/no_such_dir_xyz_12345"),
            AgentSource::Local,
            &mut agents,
        );
        assert!(agents.is_empty());
    }

    #[test]
    fn test_load_agents_from_directory_skips_files() {
        let temp_dir = TempDir::new().unwrap();

        // Place a plain file (not a directory) in the agents dir
        fs::write(temp_dir.path().join("not-an-agent.txt"), "hello").unwrap();

        let mut agents = HashMap::new();
        load_agents_from_directory(temp_dir.path(), AgentSource::Local, &mut agents);
        // The file should be skipped, no agents loaded
        assert!(agents.is_empty());
    }

    #[test]
    fn test_load_agents_from_directory_skips_dir_without_agent_md() {
        let temp_dir = TempDir::new().unwrap();

        // Create a subdirectory with no AGENT.md
        let sub_dir = temp_dir.path().join("not-an-agent");
        fs::create_dir_all(&sub_dir).unwrap();
        fs::write(sub_dir.join("README.md"), "readme").unwrap();

        let mut agents = HashMap::new();
        load_agents_from_directory(temp_dir.path(), AgentSource::Local, &mut agents);
        // Should not load anything since no AGENT.md
        assert!(agents.is_empty());
    }

    #[test]
    fn test_load_agents_from_directory_skips_malformed_agent() {
        // A subdirectory with a malformed AGENT.md should be skipped (warn branch)
        let temp_dir = TempDir::new().unwrap();
        let bad_agent = temp_dir.path().join("bad-agent");
        fs::create_dir_all(&bad_agent).unwrap();
        // Write an AGENT.md with invalid frontmatter (missing closing ---)
        fs::write(bad_agent.join("AGENT.md"), "---\nname: bad\n# no closing").unwrap();

        let mut agents = HashMap::new();
        load_agents_from_directory(temp_dir.path(), AgentSource::Local, &mut agents);
        // The malformed agent should be skipped, not loaded
        assert!(
            !agents.contains_key("bad-agent"),
            "malformed agent should not be loaded"
        );
    }

    #[test]
    fn test_load_agents_from_directory_loads_valid_alongside_invalid() {
        let temp_dir = TempDir::new().unwrap();

        // Create one valid agent
        create_agent_dir(temp_dir.path(), "good-agent", "A valid agent");

        // Create one malformed agent (missing description)
        let bad_dir = temp_dir.path().join("bad-agent");
        fs::create_dir_all(&bad_dir).unwrap();
        fs::write(
            bad_dir.join("AGENT.md"),
            "---\nname: bad-agent\n---\nInstructions.\n",
        )
        .unwrap();

        let mut agents = HashMap::new();
        load_agents_from_directory(temp_dir.path(), AgentSource::Local, &mut agents);

        assert!(
            agents.contains_key("good-agent"),
            "valid agent should be loaded"
        );
        assert!(
            !agents.contains_key("bad-agent"),
            "invalid agent should be skipped"
        );
    }

    #[test]
    fn test_add_search_path_is_used() {
        let temp_dir = TempDir::new().unwrap();

        // Create a new agent in the extra path
        create_agent_dir(temp_dir.path(), "custom-agent", "My custom agent");

        let mut resolver = AgentResolver::new();
        resolver.add_search_path(temp_dir.path().to_path_buf());

        let agents = resolver.resolve_all();
        assert!(
            agents.contains_key("custom-agent"),
            "custom agent from extra path should be loaded"
        );
        let agent = agents.get("custom-agent").unwrap();
        assert_eq!(agent.source, AgentSource::Local);
    }
}
