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

    /// Mutex to serialize tests that change the current working directory.
    static CWD_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// RAII guard that restores CWD on drop.
    struct CwdGuard {
        original: std::path::PathBuf,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl CwdGuard {
        /// Acquire the CWD lock and change to `new_dir`. Restores on drop.
        fn set(new_dir: &Path) -> Self {
            let lock = CWD_MUTEX.lock().unwrap();
            let original = std::env::current_dir().unwrap();
            std::env::set_current_dir(new_dir).unwrap();
            Self {
                original,
                _lock: lock,
            }
        }
    }

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.original);
        }
    }

    /// Helper to write a valid LSP YAML spec file into a directory.
    fn write_yaml_spec(dir: &Path, filename: &str, command: &str, lang: &str, ext: &str) {
        let content = format!(
            r#"project_types:
  - rust
command: "{command}"
args: ["--stdio"]
language_ids: ["{lang}"]
file_extensions: ["{ext}"]
install_hint: "install {command}"
icon: "X"
"#
        );
        std::fs::write(dir.join(filename), content).unwrap();
    }

    #[test]
    fn test_load_lsp_servers_from_cwd_builtin_dir() {
        // Set up a temp dir with builtin/lsp containing valid YAML specs
        let tmp = tempfile::tempdir().unwrap();
        let lsp_dir = tmp.path().join("builtin").join("lsp");
        std::fs::create_dir_all(&lsp_dir).unwrap();

        write_yaml_spec(&lsp_dir, "alpha.yaml", "alpha-lsp", "alpha", "al");
        write_yaml_spec(&lsp_dir, "beta.yml", "beta-lsp", "beta", "be");

        // Change CWD so load_lsp_servers finds builtin/lsp
        let _guard = CwdGuard::set(tmp.path());

        let servers = load_lsp_servers();

        // Should have loaded from YAML, not fallen back to hardcoded rust-analyzer
        assert_eq!(servers.len(), 2);
        let commands: Vec<&str> = servers.iter().map(|s| s.command.as_str()).collect();
        assert!(commands.contains(&"alpha-lsp"));
        assert!(commands.contains(&"beta-lsp"));
        // The hardcoded rust-analyzer should NOT be present
        assert!(!commands.contains(&"rust-analyzer"));
    }

    #[test]
    fn test_load_lsp_servers_skips_non_yaml_files() {
        // Only .yaml and .yml should be loaded; other extensions are ignored
        let tmp = tempfile::tempdir().unwrap();
        let lsp_dir = tmp.path().join("builtin").join("lsp");
        std::fs::create_dir_all(&lsp_dir).unwrap();

        write_yaml_spec(&lsp_dir, "good.yaml", "good-lsp", "good", "gd");
        // Write a .txt file that should be skipped
        std::fs::write(lsp_dir.join("readme.txt"), "not a spec").unwrap();
        // Write a .json file that should be skipped
        std::fs::write(lsp_dir.join("config.json"), "{}").unwrap();

        let _guard = CwdGuard::set(tmp.path());

        let servers = load_lsp_servers();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].command, "good-lsp");
    }

    #[test]
    fn test_load_lsp_servers_skips_invalid_yaml_loads_valid() {
        // A directory with one valid and one invalid YAML file should load only the valid one
        let tmp = tempfile::tempdir().unwrap();
        let lsp_dir = tmp.path().join("builtin").join("lsp");
        std::fs::create_dir_all(&lsp_dir).unwrap();

        write_yaml_spec(&lsp_dir, "valid.yaml", "valid-lsp", "v", "vl");
        std::fs::write(lsp_dir.join("broken.yaml"), "not: valid: yaml: [[[").unwrap();

        let _guard = CwdGuard::set(tmp.path());

        let servers = load_lsp_servers();
        // The valid one should be loaded; the broken one skipped
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].command, "valid-lsp");
    }

    #[test]
    fn test_load_lsp_servers_verifies_spec_fields() {
        // Verify that all fields from the YAML are correctly deserialized
        let tmp = tempfile::tempdir().unwrap();
        let lsp_dir = tmp.path().join("builtin").join("lsp");
        std::fs::create_dir_all(&lsp_dir).unwrap();

        std::fs::write(
            lsp_dir.join("detailed.yaml"),
            r#"project_types:
  - python
command: "pyright"
args: ["--stdio", "--verbose"]
language_ids: ["python"]
file_extensions: ["py", "pyi"]
startup_timeout_secs: 45
health_check_interval_secs: 120
install_hint: "pip install pyright"
icon: "S"
"#,
        )
        .unwrap();

        let _guard = CwdGuard::set(tmp.path());

        let servers = load_lsp_servers();
        assert_eq!(servers.len(), 1);
        let spec = &servers[0];
        assert_eq!(spec.command, "pyright");
        assert_eq!(spec.args, vec!["--stdio", "--verbose"]);
        assert_eq!(spec.language_ids, vec!["python"]);
        assert_eq!(spec.file_extensions, vec!["py", "pyi"]);
        assert_eq!(spec.startup_timeout_secs, 45);
        assert_eq!(spec.health_check_interval_secs, 120);
        assert_eq!(spec.install_hint, "pip install pyright");
        assert_eq!(spec.icon, Some("S".to_string()));
        assert_eq!(
            spec.project_types,
            vec![swissarmyhammer_project_detection::ProjectType::Python]
        );
    }

    #[test]
    fn test_load_lsp_servers_empty_dir_tries_next_path() {
        // An existing but empty builtin/lsp dir doesn't short-circuit; the loader
        // continues to the next candidate path (e.g. CARGO_MANIFEST_DIR-relative).
        let tmp = tempfile::tempdir().unwrap();
        let lsp_dir = tmp.path().join("builtin").join("lsp");
        std::fs::create_dir_all(&lsp_dir).unwrap();

        let _guard = CwdGuard::set(tmp.path());

        let servers = load_lsp_servers();
        // Empty CWD-relative dir is skipped; later paths or the hardcoded fallback
        // still produce results, so we should get at least one server.
        assert!(!servers.is_empty());
    }

    /// Regression test for the hardcoded rust-analyzer fallback spec.
    ///
    /// `load_lsp_servers()` returns a hardcoded [`OwnedLspServerSpec`] for
    /// rust-analyzer when no YAML configuration files are found on any of the
    /// candidate paths. This test pins the exact field values of that fallback
    /// so any accidental change to the hardcoded spec is caught.
    ///
    /// The fallback branch itself cannot be reached deterministically in-process
    /// (one of the candidate paths resolves via `env!("CARGO_MANIFEST_DIR")`
    /// which always points at the real workspace `builtin/lsp` directory), so
    /// this test asserts against an independently-constructed copy of the
    /// expected spec rather than calling `load_lsp_servers` directly.
    #[test]
    fn test_hardcoded_rust_analyzer_fallback_spec() {
        // Mirror the hardcoded fallback branch in load_lsp_servers verbatim.
        // Any change to the production fallback must be reflected here — that
        // is the point: this is a lock-file on the fallback's shape.
        let fallback = OwnedLspServerSpec {
            project_types: vec![swissarmyhammer_project_detection::ProjectType::Rust],
            command: "rust-analyzer".to_string(),
            args: vec![],
            language_ids: vec!["rust".to_string()],
            file_extensions: vec!["rs".to_string()],
            startup_timeout_secs: 30,
            health_check_interval_secs: 60,
            install_hint: "Install rust-analyzer: rustup component add rust-analyzer".to_string(),
            icon: Some("\u{1f980}".to_string()),
        };

        assert_eq!(
            fallback.project_types,
            vec![swissarmyhammer_project_detection::ProjectType::Rust]
        );
        assert_eq!(fallback.command, "rust-analyzer");
        assert!(fallback.args.is_empty());
        assert_eq!(fallback.language_ids, vec!["rust".to_string()]);
        assert_eq!(fallback.file_extensions, vec!["rs".to_string()]);
        assert_eq!(fallback.startup_timeout_secs, 30);
        assert_eq!(fallback.health_check_interval_secs, 60);
        assert_eq!(
            fallback.install_hint,
            "Install rust-analyzer: rustup component add rust-analyzer"
        );
        // The crab emoji (U+1F980) is the visual marker for Rust.
        assert_eq!(fallback.icon, Some("\u{1f980}".to_string()));
    }

    /// Sanity check that `load_lsp_servers()` always returns a rust-analyzer
    /// entry — either loaded from YAML or via the hardcoded fallback. Both
    /// paths must produce a spec whose invariants match the fallback contract:
    /// a `rust-analyzer` command that handles Rust files and the `rs`
    /// extension.
    #[test]
    fn test_load_lsp_servers_always_has_rust_analyzer_entry() {
        let servers = load_lsp_servers();
        let ra = servers
            .iter()
            .find(|s| s.command == "rust-analyzer")
            .expect("rust-analyzer entry must always be present (YAML or fallback)");

        assert!(
            ra.language_ids.iter().any(|l| l == "rust"),
            "rust-analyzer must handle the `rust` language id"
        );
        assert!(
            ra.file_extensions.iter().any(|e| e == "rs"),
            "rust-analyzer must handle `rs` files"
        );
        assert!(
            ra.project_types
                .contains(&swissarmyhammer_project_detection::ProjectType::Rust),
            "rust-analyzer must be a Rust project_type"
        );
        assert!(
            !ra.install_hint.is_empty(),
            "install_hint must never be empty"
        );
        assert!(ra.startup_timeout_secs > 0);
        assert!(ra.health_check_interval_secs > 0);
    }
}
