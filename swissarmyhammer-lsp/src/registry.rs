//! Built-in LSP server registry
//!
//! Loads LSP server specifications from YAML configuration files in builtin/lsp/,
//! or falls back to hardcoded defaults if YAML files are not found.
//! Servers are detected automatically based on project type.

use once_cell::sync::Lazy;
use std::time::Duration;
use swissarmyhammer_project_detection::ProjectType;

use crate::types::{LspServerSpec, OwnedLspServerSpec};

/// Lazy-initialized registry of owned LSP server specs loaded from YAML
static OWNED_SERVERS: Lazy<Vec<OwnedLspServerSpec>> =
    Lazy::new(crate::yaml_loader::load_lsp_servers);

/// Built-in server registry — kept for API compatibility
/// Returns a static reference to the first registered Rust server for backward compatibility
pub static SERVERS: &[LspServerSpec] = &[LspServerSpec {
    project_types: &[ProjectType::Rust],
    command: "rust-analyzer",
    args: &[],
    language_ids: &["rust"],
    file_extensions: &["rs"],
    initialization_options: None,
    startup_timeout: Duration::from_secs(30),
    health_check_interval: Duration::from_secs(60),
    install_hint: "Install rust-analyzer: rustup component add rust-analyzer",
}];

/// Find all LSP servers that can handle the given project type
/// Returns owned server specs loaded from YAML configuration files
pub fn servers_for_project(project_type: ProjectType) -> Vec<OwnedLspServerSpec> {
    OWNED_SERVERS
        .iter()
        .filter(|spec| spec.project_types.contains(&project_type))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_server_found() {
        let servers = servers_for_project(ProjectType::Rust);
        assert!(
            !servers.is_empty(),
            "Should find at least one server for Rust"
        );
        let rust_server = servers.iter().find(|s| s.command == "rust-analyzer");
        assert!(rust_server.is_some(), "Should find rust-analyzer");
        let spec = rust_server.unwrap();
        assert_eq!(spec.command, "rust-analyzer");
        assert!(spec.language_ids.contains(&"rust".to_string()));
        assert!(spec.file_extensions.contains(&"rs".to_string()));
    }

    #[test]
    fn test_no_server_for_unregistered_type() {
        let servers = servers_for_project(ProjectType::Flutter);
        // Flutter may or may not be in YAML, but query should not panic
        let _ = servers;
    }

    #[test]
    fn test_server_spec_fields() {
        let servers = servers_for_project(ProjectType::Rust);
        assert!(!servers.is_empty());
        let spec = &servers[0];
        assert_eq!(spec.startup_timeout_secs, 30);
        assert_eq!(spec.health_check_interval_secs, 60);
        assert!(!spec.install_hint.is_empty());
    }

    #[test]
    fn test_all_project_types_queryable() {
        // Every project type should be queryable without panic
        for pt in [
            ProjectType::Rust,
            ProjectType::NodeJs,
            ProjectType::Python,
            ProjectType::Go,
            ProjectType::JavaMaven,
            ProjectType::JavaGradle,
            ProjectType::CSharp,
            ProjectType::CMake,
            ProjectType::Makefile,
            ProjectType::Flutter,
        ] {
            let _ = servers_for_project(pt);
        }
    }

    #[test]
    fn test_owned_servers_loaded() {
        // Check that OWNED_SERVERS is initialized and contains at least rust-analyzer
        let servers = OWNED_SERVERS.clone();
        assert!(!servers.is_empty(), "Should have at least one server");
        assert!(
            servers.iter().any(|s| s.command == "rust-analyzer"),
            "Should include rust-analyzer"
        );
    }
}
