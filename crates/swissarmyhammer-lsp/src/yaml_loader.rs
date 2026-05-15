//! Re-exports of the builtin LSP YAML registry that lives in
//! `swissarmyhammer-code-context`.
//!
//! The loader, the embedded YAML sources list, and the
//! [`OwnedLspServerSpec`] struct are all owned by the lower-tier
//! `swissarmyhammer-code-context` crate. This module preserves the
//! `swissarmyhammer_lsp::yaml_loader::{builtin_lsp_yaml_sources, load_lsp_servers}`
//! API that existing callers in this crate use — they both delegate directly
//! to the consolidated implementation.

pub use swissarmyhammer_code_context::{builtin_lsp_yaml_sources, load_lsp_servers};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::OwnedLspServerSpec;

    #[test]
    fn test_load_lsp_servers() {
        let servers = load_lsp_servers();
        // All builtin YAMLs should load; rust-analyzer is one of them.
        assert!(!servers.is_empty());
        assert!(servers.iter().any(|s| s.command == "rust-analyzer"));
    }

    #[test]
    fn test_rust_analyzer_has_correct_properties() {
        let servers = load_lsp_servers();
        let rust_analyzer = servers
            .iter()
            .find(|s| s.command == "rust-analyzer")
            .expect("rust-analyzer not found");

        assert!(!rust_analyzer.project_types.is_empty());
        assert!(rust_analyzer.language_ids.contains(&"rust".to_string()));
        assert!(rust_analyzer.file_extensions.contains(&"rs".to_string()));
        assert!(rust_analyzer.startup_timeout_secs > 0);
        assert!(rust_analyzer.health_check_interval_secs > 0);
    }

    #[test]
    fn test_builtin_lsp_yaml_sources_non_empty() {
        // Every embedded YAML source must parse into a valid spec via the lsp
        // crate's re-export of `OwnedLspServerSpec`.
        let sources = builtin_lsp_yaml_sources();
        assert!(
            !sources.is_empty(),
            "Should embed at least one builtin LSP YAML"
        );
        for (name, src) in sources {
            assert!(!src.is_empty(), "YAML for {} should not be empty", name);
            let spec: OwnedLspServerSpec = serde_yaml_ng::from_str(src)
                .unwrap_or_else(|e| panic!("Failed to parse {}: {}", name, e));
            assert!(
                !spec.command.is_empty(),
                "Spec for {} has empty command",
                name
            );
        }
    }

    #[test]
    fn test_load_lsp_servers_returns_one_per_embedded_yaml() {
        // Every embedded YAML source should turn into exactly one loaded spec.
        let expected = builtin_lsp_yaml_sources().len();
        let servers = load_lsp_servers();
        assert_eq!(
            servers.len(),
            expected,
            "Expected one spec per embedded YAML, got {}",
            servers.len()
        );
    }

    #[test]
    fn test_load_lsp_servers_covers_expected_languages() {
        // Sanity check that the builtin registry covers the languages listed
        // in the YAML directory.
        let servers = load_lsp_servers();
        let commands: Vec<&str> = servers.iter().map(|s| s.command.as_str()).collect();
        for expected in [
            "rust-analyzer",
            "gopls",
            "pylsp",
            "typescript-language-server",
            "clangd",
            "jdtls",
            "omnisharp",
            "dart",
            "intelephense",
            "solargraph",
            "sourcekit-lsp",
            "kotlin-language-server",
        ] {
            assert!(
                commands.contains(&expected),
                "Expected {} in registry, got {:?}",
                expected,
                commands
            );
        }
    }

    #[test]
    fn test_load_lsp_servers_populates_file_extensions() {
        // Every loaded spec should have file_extensions from its YAML.
        for spec in load_lsp_servers() {
            assert!(
                !spec.file_extensions.is_empty(),
                "Spec for {} should have file_extensions populated from YAML",
                spec.command
            );
        }
    }

    #[test]
    fn test_load_lsp_servers_install_hints_non_empty() {
        // Every YAML file declares a non-empty install_hint.
        for spec in load_lsp_servers() {
            assert!(
                !spec.install_hint.is_empty(),
                "Spec for {} should have a non-empty install_hint",
                spec.command
            );
        }
    }

    #[test]
    fn test_load_lsp_servers_typescript_handles_multiple_ids() {
        // typescript-language-server must handle both TypeScript and JavaScript.
        let servers = load_lsp_servers();
        let ts = servers
            .iter()
            .find(|s| s.command == "typescript-language-server")
            .expect("typescript-language-server not found");
        assert!(ts.language_ids.contains(&"typescript".to_string()));
        assert!(ts.language_ids.contains(&"javascript".to_string()));
        assert!(ts.file_extensions.contains(&"ts".to_string()));
        assert!(ts.file_extensions.contains(&"tsx".to_string()));
        assert!(ts.file_extensions.contains(&"js".to_string()));
    }
}
