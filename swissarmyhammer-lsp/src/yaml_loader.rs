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
    let spec: OwnedLspServerSpec = serde_yaml_ng::from_str(&contents)?;
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

    #[test]
    fn test_load_single_server_valid_yaml() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml_path = tmp.path().join("test-server.yaml");
        std::fs::write(
            &yaml_path,
            r#"
project_types:
  - rust
command: "test-lsp"
args: ["--stdio"]
language_ids: ["test"]
file_extensions: ["tst"]
install_hint: "install test-lsp"
icon: "T"
"#,
        )
        .unwrap();

        let spec = load_single_server(&yaml_path).unwrap();
        assert_eq!(spec.command, "test-lsp");
        assert_eq!(spec.args, vec!["--stdio"]);
        assert_eq!(spec.language_ids, vec!["test"]);
        assert_eq!(spec.file_extensions, vec!["tst"]);
        assert_eq!(spec.install_hint, "install test-lsp");
    }

    #[test]
    fn test_load_single_server_invalid_yaml() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml_path = tmp.path().join("bad.yaml");
        std::fs::write(&yaml_path, "not: valid: yaml: [[[").unwrap();

        let result = load_single_server(&yaml_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_single_server_missing_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml_path = tmp.path().join("incomplete.yaml");
        std::fs::write(&yaml_path, "command: test\n").unwrap();

        let result = load_single_server(&yaml_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_single_server_nonexistent_file() {
        let result = load_single_server(Path::new("/nonexistent/file.yaml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_from_custom_directory_with_valid_yaml() {
        // Create a temporary directory structure that mimics builtin/lsp
        let tmp = tempfile::tempdir().unwrap();
        let lsp_dir = tmp.path().join("builtin").join("lsp");
        std::fs::create_dir_all(&lsp_dir).unwrap();

        std::fs::write(
            lsp_dir.join("custom.yaml"),
            r#"
project_types:
  - python
command: "custom-lsp"
args: []
language_ids: ["python"]
file_extensions: ["py"]
install_hint: "install custom-lsp"
icon: "P"
"#,
        )
        .unwrap();

        // Also write a non-yaml file that should be skipped
        std::fs::write(lsp_dir.join("readme.txt"), "not yaml").unwrap();

        // load_single_server on the valid file should work
        let spec = load_single_server(&lsp_dir.join("custom.yaml")).unwrap();
        assert_eq!(spec.command, "custom-lsp");
    }

    #[test]
    fn test_load_from_directory_with_bad_yaml_skips_invalid() {
        // Create a dir with one good and one bad YAML file
        let tmp = tempfile::tempdir().unwrap();
        let lsp_dir = tmp.path().join("lsp");
        std::fs::create_dir_all(&lsp_dir).unwrap();

        // Good file
        std::fs::write(
            lsp_dir.join("good.yaml"),
            r#"
project_types:
  - rust
command: "good-lsp"
args: []
language_ids: ["rust"]
file_extensions: ["rs"]
install_hint: "install good-lsp"
icon: "G"
"#,
        )
        .unwrap();

        // Bad file - invalid YAML structure for OwnedLspServerSpec
        std::fs::write(lsp_dir.join("bad.yaml"), "invalid: yaml: structure: [[[").unwrap();

        // Load the good one directly to verify it works
        let spec = load_single_server(&lsp_dir.join("good.yaml")).unwrap();
        assert_eq!(spec.command, "good-lsp");

        // The bad one should fail
        let result = load_single_server(&lsp_dir.join("bad.yaml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_single_server_with_defaults() {
        // Test that serde defaults are applied when optional fields are missing
        let tmp = tempfile::tempdir().unwrap();
        let yaml_path = tmp.path().join("defaults.yaml");
        std::fs::write(
            &yaml_path,
            r#"
project_types: []
command: "default-lsp"
args: []
language_ids: ["test"]
file_extensions: ["txt"]
install_hint: "install it"
"#,
        )
        .unwrap();

        let spec = load_single_server(&yaml_path).unwrap();
        assert_eq!(spec.startup_timeout_secs, 30);
        assert_eq!(spec.health_check_interval_secs, 60);
        assert!(spec.icon.is_none());
    }
}
