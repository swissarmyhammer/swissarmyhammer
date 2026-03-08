//! Built-in LSP server registry
//!
//! Maps project types to LSP server specifications. No configuration files —
//! servers are detected automatically based on project type.

use std::time::Duration;
use swissarmyhammer_project_detection::ProjectType;

use crate::types::LspServerSpec;

/// Built-in server registry — all known LSP servers
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
pub fn servers_for_project(project_type: ProjectType) -> Vec<&'static LspServerSpec> {
    SERVERS
        .iter()
        .filter(|spec| spec.project_types.contains(&project_type))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_server_found() {
        let servers = servers_for_project(ProjectType::Rust);
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].command, "rust-analyzer");
        assert_eq!(servers[0].language_ids, &["rust"]);
        assert_eq!(servers[0].file_extensions, &["rs"]);
    }

    #[test]
    fn test_no_server_for_unregistered_type() {
        let servers = servers_for_project(ProjectType::Flutter);
        assert!(servers.is_empty());
    }

    #[test]
    fn test_server_spec_fields() {
        let spec = &SERVERS[0];
        assert_eq!(spec.startup_timeout, Duration::from_secs(30));
        assert_eq!(spec.health_check_interval, Duration::from_secs(60));
        assert!(spec.initialization_options.is_none());
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
}
