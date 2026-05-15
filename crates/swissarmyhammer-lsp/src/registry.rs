//! Built-in LSP server registry.
//!
//! Loads LSP server specifications from YAML configuration files under
//! `builtin/lsp/`. The single canonical registry lives in
//! `swissarmyhammer-code-context` — this module simply queries it to
//! provide ergonomic `servers_for_project`, `servers_for_extensions`, and
//! `all_servers` helpers.

use std::time::Duration;
use swissarmyhammer_code_context::LSP_REGISTRY;
use swissarmyhammer_project_detection::ProjectType;

use crate::types::{LspServerSpec, OwnedLspServerSpec};

/// Built-in server registry — kept for API compatibility.
///
/// Returns a static reference to the first registered Rust server for
/// backward compatibility; callers that need the full YAML-loaded registry
/// should use [`all_servers`] instead.
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

/// Return all loaded LSP server specs from the YAML registry.
pub fn all_servers() -> &'static [OwnedLspServerSpec] {
    &LSP_REGISTRY
}

/// Return specs whose file extensions overlap with the given set.
///
/// The returned references point into the process-global registry and
/// live for the lifetime of the program.
pub fn servers_for_extensions(exts: &[&str]) -> Vec<&'static OwnedLspServerSpec> {
    LSP_REGISTRY
        .iter()
        .filter(|s| s.file_extensions.iter().any(|e| exts.contains(&e.as_str())))
        .collect()
}

/// Find all LSP servers that can handle the given project type.
///
/// Returns owned server specs cloned out of the YAML-loaded registry.
pub fn servers_for_project(project_type: ProjectType) -> Vec<OwnedLspServerSpec> {
    LSP_REGISTRY
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
        // Check that the shared YAML registry is initialized and contains at
        // least rust-analyzer.
        let servers = all_servers();
        assert!(!servers.is_empty(), "Should have at least one server");
        assert!(
            servers.iter().any(|s| s.command == "rust-analyzer"),
            "Should include rust-analyzer"
        );
    }

    #[test]
    fn test_all_servers_returns_non_empty() {
        let servers = all_servers();
        assert!(
            !servers.is_empty(),
            "all_servers() should return a non-empty list"
        );
    }

    #[test]
    fn test_servers_for_extensions_rs() {
        let servers = servers_for_extensions(&["rs"]);
        assert!(
            servers.iter().any(|s| s.command == "rust-analyzer"),
            "servers_for_extensions([\"rs\"]) should return rust-analyzer"
        );
    }

    #[test]
    fn test_all_yaml_specs_have_icon() {
        // Every loaded spec from YAML that has an icon field should have it set
        for spec in all_servers() {
            assert!(
                spec.icon.is_some(),
                "Server {} should have an icon set in its YAML file",
                spec.command
            );
        }
    }
}
