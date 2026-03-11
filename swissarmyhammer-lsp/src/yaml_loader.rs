//! Load LSP server specifications from YAML configuration files

use std::path::Path;
use tracing::{debug, warn};

use crate::types::OwnedLspServerSpec;

/// Load all LSP server specifications from the builtin YAML files
pub fn load_lsp_servers() -> Vec<OwnedLspServerSpec> {
    // Find the builtin/lsp directory relative to the binary
    let mut servers = Vec::new();

    // Try multiple possible locations for the builtin directory
    let possible_paths = vec![
        // Relative to workspace root (during development)
        Path::new("builtin/lsp").to_path_buf(),
        // Relative to binary location
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|p| p.join("builtin/lsp")))
            .unwrap_or_default(),
        // Relative to CARGO_MANIFEST_DIR
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap_or(Path::new("."))
            .join("builtin/lsp"),
    ];

    for lsp_dir in possible_paths {
        if !lsp_dir.exists() {
            debug!("LSP config dir not found at {:?}", lsp_dir);
            continue;
        }

        if let Ok(entries) = std::fs::read_dir(&lsp_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path
                    .extension()
                    .map(|e| e == "yaml" || e == "yml")
                    .unwrap_or(false)
                {
                    match load_single_server(&path) {
                        Ok(spec) => {
                            debug!("Loaded LSP server config: {}", spec.command);
                            servers.push(spec);
                        }
                        Err(e) => {
                            warn!("Failed to load LSP server config {:?}: {}", path, e);
                        }
                    }
                }
            }
        }

        // If we found and loaded from a directory, don't try other paths
        if !servers.is_empty() {
            break;
        }
    }

    // Always include the hardcoded rust-analyzer as fallback
    if servers.is_empty() {
        debug!("No YAML LSP configs found, using hardcoded rust-analyzer");
        servers.push(OwnedLspServerSpec {
            project_types: vec![swissarmyhammer_project_detection::ProjectType::Rust],
            command: "rust-analyzer".to_string(),
            args: vec![],
            language_ids: vec!["rust".to_string()],
            file_extensions: vec!["rs".to_string()],
            startup_timeout_secs: 30,
            health_check_interval_secs: 60,
            install_hint: "Install rust-analyzer: rustup component add rust-analyzer".to_string(),
            icon: Some("\u{1f980}".to_string()),
        });
    }

    servers
}

/// Load a single LSP server specification from a YAML file
fn load_single_server(path: &Path) -> Result<OwnedLspServerSpec, Box<dyn std::error::Error>> {
    let contents = std::fs::read_to_string(path)?;
    let spec: OwnedLspServerSpec = serde_yaml::from_str(&contents)?;
    Ok(spec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_lsp_servers() {
        let servers = load_lsp_servers();
        // Should have at least the fallback rust-analyzer
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
}
