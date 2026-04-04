use crate::error::{ConfigurationError, ConfigurationResult};
use std::path::{Path, PathBuf};
use swissarmyhammer_common::directory::{DirectoryConfig, SwissarmyhammerConfig};
use swissarmyhammer_common::utils::find_git_repository_root;
use tracing::{debug, trace};

/// Configuration file discovery paths
#[derive(Debug, Clone)]
pub struct DiscoveryPaths {
    /// Global configuration directory (~/.sah/)
    pub global_dir: Option<PathBuf>,
    /// Project configuration directories in ascending precedence order.
    ///
    /// Walking from git root down to CWD, every `.sah/` directory found along
    /// the way is included.  Parent directories come first (lowest precedence),
    /// CWD-level last (highest precedence).
    pub project_dirs: Vec<PathBuf>,
}

/// Configuration file discovery utility.
///
/// Resolves global (user) and project (local) configuration directories using
/// direct filesystem checks, then scans those directories for config files with
/// the expected name patterns (`sah.{toml,yaml,yml,json}` and
/// `swissarmyhammer.{toml,yaml,yml,json}`).
///
/// Figment handles the deep key-value merging downstream.
pub struct ConfigurationDiscovery {
    paths: DiscoveryPaths,
    validate_security: bool,
}

impl ConfigurationDiscovery {
    /// Create a new configuration discovery instance.
    ///
    /// Resolves global and project config directories:
    /// - Global: `~/.sah/` (User source)
    /// - Project: `{git_root}/.sah/` down to `{cwd}/.sah/` (Local sources)
    pub fn new() -> ConfigurationResult<Self> {
        let (global_dir, project_dirs) = Self::resolve_directories();

        Ok(Self {
            paths: DiscoveryPaths {
                global_dir,
                project_dirs,
            },
            validate_security: true,
        })
    }

    /// Create a discovery instance for CLI usage (no security validation).
    pub fn for_cli() -> ConfigurationResult<Self> {
        let mut discovery = Self::new()?;
        discovery.validate_security = false;
        Ok(discovery)
    }

    /// Get the discovered paths.
    pub fn paths(&self) -> &DiscoveryPaths {
        &self.paths
    }

    /// Find all configuration files in priority order.
    ///
    /// Returns paths in ascending precedence order (earlier files are overridden
    /// by later ones in Figment merging):
    /// 1. Global config files (`~/.sah/sah.*`) -- User/global
    /// 2. Project config files from git root down to CWD (`{ancestor}/.sah/sah.*`)
    pub fn discover_config_files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();

        // Global config files (lowest precedence among file sources)
        if let Some(ref global_dir) = self.paths.global_dir {
            files.extend(Self::find_config_files_in_dir(global_dir));
        }

        // Project config files in ascending precedence (git root first, CWD last)
        for project_dir in &self.paths.project_dirs {
            files.extend(Self::find_config_files_in_dir(project_dir));
        }

        // Filter out non-existent files and apply security validation
        files
            .into_iter()
            .filter(|path| {
                if !path.exists() {
                    return false;
                }

                if self.validate_security {
                    if let Err(e) = self.validate_file_security(path) {
                        debug!(
                            "Skipping config file due to security validation: {}: {}",
                            path.display(),
                            e
                        );
                        return false;
                    }
                }

                true
            })
            .collect()
    }

    /// Resolve global and project directories using direct filesystem checks.
    ///
    /// Returns only directories that actually exist on disk.  Project directories
    /// are in ascending precedence order (git root first, CWD-level last).
    fn resolve_directories() -> (Option<PathBuf>, Vec<PathBuf>) {
        let global_dir = Self::resolve_global_dir().filter(|d| d.is_dir());
        let project_dirs: Vec<PathBuf> = Self::resolve_project_dirs()
            .into_iter()
            .filter(|d| d.is_dir())
            .collect();

        if global_dir.is_some() || !project_dirs.is_empty() {
            debug!(
                "Resolved directories: global={}, project=[{}]",
                global_dir
                    .as_ref()
                    .map_or("none".to_string(), |d| d.display().to_string()),
                project_dirs
                    .iter()
                    .map(|d| d.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        } else {
            trace!("No configuration directories found");
        }

        (global_dir, project_dirs)
    }

    /// Resolve the global configuration directory path.
    ///
    /// Uses `$HOME/.sah/`.
    fn resolve_global_dir() -> Option<PathBuf> {
        // Respect HOME env var first (important for test isolation), then fall back
        // to dirs::home_dir()
        let home = std::env::var("HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(dirs::home_dir)?;

        let config_dir = home.join(SwissarmyhammerConfig::DIR_NAME);
        Some(config_dir)
    }

    /// Resolve all project configuration directory paths by walking from git root to CWD.
    ///
    /// Walks up from CWD to the git repository root (or filesystem root when not
    /// inside a git repo), collecting every `{ancestor}/.sah/` path.  The returned
    /// vec is in ascending precedence order: the git-root-level directory comes
    /// first and the CWD-level directory comes last.
    fn resolve_project_dirs() -> Vec<PathBuf> {
        let cwd = match std::env::current_dir() {
            Ok(d) => d,
            Err(_) => return Vec::new(),
        };

        let stop_at = find_git_repository_root().unwrap_or_else(|| cwd.clone());

        // Collect directories from CWD up to (and including) the stop point.
        // We walk upward, then reverse so that the outermost (git root) directory
        // is first and the CWD-level directory is last.
        let mut dirs = Vec::new();
        let mut current = cwd.as_path();
        loop {
            let config_dir = current.join(SwissarmyhammerConfig::DIR_NAME);
            dirs.push(config_dir);

            // Stop once we've processed the git root (or fallback root)
            if current == stop_at {
                break;
            }

            match current.parent() {
                Some(parent) => current = parent,
                None => break,
            }
        }

        // Reverse: git-root first (lowest precedence), CWD last (highest)
        dirs.reverse();
        dirs
    }

    /// Find all configuration files in a directory.
    ///
    /// Scans for both short (`sah.*`) and long (`swissarmyhammer.*`) form names
    /// in the supported formats (toml, yaml, yml, json).
    fn find_config_files_in_dir(dir: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();

        // Both short and long form names
        let base_names = ["sah", "swissarmyhammer"];
        // Supported extensions in preference order
        let extensions = ["toml", "yaml", "yml", "json"];

        for base_name in &base_names {
            for extension in &extensions {
                let filename = format!("{}.{}", base_name, extension);
                let file_path = dir.join(&filename);
                if file_path.is_file() {
                    debug!("Found config file: {}", file_path.display());
                    files.push(file_path);
                }
            }
        }

        files
    }

    /// Validate file security (only when validation is enabled).
    fn validate_file_security(&self, path: &Path) -> ConfigurationResult<()> {
        let metadata = path.metadata().map_err(|e| {
            ConfigurationError::discovery(format!(
                "Could not read metadata for {}: {}",
                path.display(),
                e
            ))
        })?;

        if metadata.permissions().readonly() {
            return Err(ConfigurationError::discovery(format!(
                "Configuration file is not readable: {}",
                path.display()
            )));
        }

        Ok(())
    }
}

impl Default for ConfigurationDiscovery {
    fn default() -> Self {
        Self::new().expect("Failed to create default ConfigurationDiscovery")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_find_config_files_in_dir() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path();

        // Create test config files
        fs::write(dir_path.join("sah.toml"), "[test]\nkey = \"value\"\n").unwrap();
        fs::write(
            dir_path.join("swissarmyhammer.yaml"),
            "test:\n  key: value\n",
        )
        .unwrap();
        fs::write(dir_path.join("sah.json"), r#"{"test": {"key": "value"}}"#).unwrap();

        let files = ConfigurationDiscovery::find_config_files_in_dir(dir_path);

        // Should find all three files
        assert_eq!(files.len(), 3);

        // Verify the files are the ones we created
        let file_names: Vec<String> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();

        assert!(file_names.contains(&"sah.toml".to_string()));
        assert!(file_names.contains(&"swissarmyhammer.yaml".to_string()));
        assert!(file_names.contains(&"sah.json".to_string()));
    }

    #[test]
    fn test_discovery_with_no_config_files() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path();

        let files = ConfigurationDiscovery::find_config_files_in_dir(dir_path);

        assert_eq!(files.len(), 0);
    }

    #[test]
    fn test_validate_file_security() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.toml");
        fs::write(&file_path, "[test]\nkey = \"value\"\n").unwrap();

        let discovery = ConfigurationDiscovery {
            paths: DiscoveryPaths {
                global_dir: None,
                project_dirs: vec![],
            },
            validate_security: true,
        };

        // Should pass security validation for a normal file
        assert!(discovery.validate_file_security(&file_path).is_ok());
    }

    #[test]
    fn test_cli_discovery_skips_security_validation() {
        // CLI discovery should not perform security validation
        let discovery = ConfigurationDiscovery::for_cli().unwrap();
        assert!(!discovery.validate_security);
    }

    #[test]
    #[serial_test::serial]
    fn test_resolve_global_dir_uses_home() {
        // When HOME is set, resolve_global_dir should return $HOME/.sah
        let original_home = std::env::var("HOME").ok();
        let temp_dir = TempDir::new().unwrap();

        // Use a panic-safe guard so HOME is always restored, even on assertion failure.
        let result = std::panic::catch_unwind(|| {
            std::env::set_var("HOME", temp_dir.path());
            ConfigurationDiscovery::resolve_global_dir()
        });

        // Restore HOME before inspecting the result (runs even if the closure panicked).
        match &original_home {
            Some(home) => std::env::set_var("HOME", home),
            None => std::env::remove_var("HOME"),
        }

        let result = result.expect("resolve_global_dir panicked");
        assert!(result.is_some());
        let dir = result.unwrap();
        assert!(dir.ends_with(".sah"));
        assert!(dir.starts_with(temp_dir.path()));
    }

    #[test]
    fn test_discover_config_files_returns_existing_only() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join(".sah");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(config_dir.join("sah.toml"), "[test]\nkey = \"val\"\n").unwrap();

        // Create a discovery with known paths
        let discovery = ConfigurationDiscovery {
            paths: DiscoveryPaths {
                global_dir: None,
                project_dirs: vec![config_dir],
            },
            validate_security: false,
        };

        let files = discovery.discover_config_files();
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("sah.toml"));
    }

    #[test]
    fn test_discover_config_files_from_multiple_project_dirs() {
        let temp_dir = TempDir::new().unwrap();

        // Simulate nested project structure:
        //   root/.sah/sah.toml            (git root level)
        //   root/workspace/.sah/sah.toml  (intermediate level)
        let root_config = temp_dir.path().join(".sah");
        let workspace_config = temp_dir.path().join("workspace").join(".sah");
        fs::create_dir_all(&root_config).unwrap();
        fs::create_dir_all(&workspace_config).unwrap();
        fs::write(root_config.join("sah.toml"), "source = \"root\"").unwrap();
        fs::write(workspace_config.join("sah.toml"), "source = \"workspace\"").unwrap();

        // Build discovery with both dirs in ascending precedence
        let discovery = ConfigurationDiscovery {
            paths: DiscoveryPaths {
                global_dir: None,
                project_dirs: vec![root_config.clone(), workspace_config.clone()],
            },
            validate_security: false,
        };

        let files = discovery.discover_config_files();
        assert_eq!(files.len(), 2);
        // Root comes first (lower precedence), workspace second (higher precedence)
        assert_eq!(files[0], root_config.join("sah.toml"));
        assert_eq!(files[1], workspace_config.join("sah.toml"));
    }

    #[test]
    fn test_for_cli_returns_valid_discovery_with_paths() {
        // for_cli() should return Ok and expose paths via the accessor
        let discovery = ConfigurationDiscovery::for_cli().unwrap();
        // The returned discovery should have security validation disabled
        assert!(!discovery.validate_security);
        // paths() accessor should work and return the same DiscoveryPaths
        let paths = discovery.paths();
        // global_dir is either Some or None depending on the environment,
        // but the accessor must not panic
        let _ = paths.global_dir.as_ref();
        let _ = paths.project_dirs.len();
    }

    #[test]
    fn test_paths_accessor_returns_configured_paths() {
        let global = PathBuf::from("/tmp/fake-global/.sah");
        let project = PathBuf::from("/tmp/fake-project/.sah");
        let discovery = ConfigurationDiscovery {
            paths: DiscoveryPaths {
                global_dir: Some(global.clone()),
                project_dirs: vec![project.clone()],
            },
            validate_security: false,
        };

        let paths = discovery.paths();
        assert_eq!(paths.global_dir.as_ref().unwrap(), &global);
        assert_eq!(paths.project_dirs.len(), 1);
        assert_eq!(paths.project_dirs[0], project);
    }

    #[test]
    fn test_validate_file_security_rejects_readonly_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("readonly.toml");
        fs::write(&file_path, "[test]\nkey = \"value\"\n").unwrap();

        // Make the file readonly
        let mut perms = fs::metadata(&file_path).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&file_path, perms).unwrap();

        let discovery = ConfigurationDiscovery {
            paths: DiscoveryPaths {
                global_dir: None,
                project_dirs: vec![],
            },
            validate_security: true,
        };

        let result = discovery.validate_file_security(&file_path);
        assert!(
            result.is_err(),
            "readonly file should fail security validation"
        );

        // Restore writable so tempdir cleanup succeeds
        let mut perms = fs::metadata(&file_path).unwrap().permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        fs::set_permissions(&file_path, perms).unwrap();
    }

    #[test]
    fn test_validate_file_security_errors_on_nonexistent_file() {
        let discovery = ConfigurationDiscovery {
            paths: DiscoveryPaths {
                global_dir: None,
                project_dirs: vec![],
            },
            validate_security: true,
        };

        let result = discovery.validate_file_security(Path::new("/nonexistent/path/file.toml"));
        assert!(
            result.is_err(),
            "nonexistent file should fail metadata read"
        );
    }

    #[test]
    fn test_discover_config_files_filters_readonly_when_security_enabled() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join(".sah");
        fs::create_dir_all(&config_dir).unwrap();

        // Create two config files -- one normal, one readonly
        let normal_file = config_dir.join("sah.toml");
        let readonly_file = config_dir.join("sah.yaml");
        fs::write(&normal_file, "key = \"val\"").unwrap();
        fs::write(&readonly_file, "key: val").unwrap();

        let mut perms = fs::metadata(&readonly_file).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&readonly_file, perms).unwrap();

        // With security validation ON, the readonly file should be filtered out
        let discovery = ConfigurationDiscovery {
            paths: DiscoveryPaths {
                global_dir: None,
                project_dirs: vec![config_dir.clone()],
            },
            validate_security: true,
        };

        let files = discovery.discover_config_files();
        assert_eq!(files.len(), 1, "only the writable file should remain");
        assert!(files[0].ends_with("sah.toml"));

        // With security validation OFF, both files should appear
        let discovery_no_sec = ConfigurationDiscovery {
            paths: DiscoveryPaths {
                global_dir: None,
                project_dirs: vec![config_dir],
            },
            validate_security: false,
        };

        let files = discovery_no_sec.discover_config_files();
        assert_eq!(
            files.len(),
            2,
            "both files should appear without security validation"
        );

        // Cleanup: restore writable
        let mut perms = fs::metadata(&readonly_file).unwrap().permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        fs::set_permissions(&readonly_file, perms).unwrap();
    }

    #[test]
    #[serial_test::serial]
    fn test_resolve_directories_debug_branch_with_both_dirs() {
        // Exercise the debug logging branch where both global_dir and project_dirs exist.
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path().canonicalize().unwrap();

        // Create a fake git root and global dir
        fs::create_dir(base.join(".git")).unwrap();
        let sah_dir = base.join(".sah");
        fs::create_dir_all(&sah_dir).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        let original_home = std::env::var("HOME").ok();

        std::env::set_current_dir(&base).unwrap();
        std::env::set_var("HOME", base.as_os_str());

        let (global_dir, project_dirs) = ConfigurationDiscovery::resolve_directories();

        // Restore environment
        std::env::set_current_dir(&original_dir).unwrap();
        match &original_home {
            Some(home) => std::env::set_var("HOME", home),
            None => std::env::remove_var("HOME"),
        }

        // Both should be populated, exercising the debug! branch at line 117
        assert!(global_dir.is_some(), "global_dir should be Some");
        assert!(!project_dirs.is_empty(), "project_dirs should not be empty");
    }

    #[test]
    #[serial_test::serial]
    fn test_resolve_project_dirs_walks_up_to_git_root() {
        let temp_dir = TempDir::new().unwrap();
        // Canonicalize to resolve symlinks (e.g. /var -> /private/var on macOS)
        // so paths match what current_dir() and find_git_repository_root() return.
        let base = temp_dir.path().canonicalize().unwrap();

        // Create a fake git root
        fs::create_dir(base.join(".git")).unwrap();

        // Create nested directories with .sah/ at multiple levels
        let workspace = base.join("workspace");
        let project = workspace.join("project");
        fs::create_dir_all(&project).unwrap();

        let root_sah = base.join(".sah");
        let workspace_sah = workspace.join(".sah");
        // project/.sah/ intentionally absent
        fs::create_dir_all(&root_sah).unwrap();
        fs::create_dir_all(&workspace_sah).unwrap();

        // Set CWD to the innermost directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&project).unwrap();

        let dirs = ConfigurationDiscovery::resolve_project_dirs();

        // Restore CWD before any assertions
        std::env::set_current_dir(&original_dir).unwrap();

        // Should include .sah/ dirs for git root, workspace, and project
        // (even though project/.sah/ does not exist on disk yet -- resolve_project_dirs
        // returns candidate paths; VFS filtering removes non-existent ones later)
        assert!(
            dirs.len() >= 3,
            "Expected at least 3 candidate dirs (root, workspace, project), got: {:?}",
            dirs
        );

        // Verify ascending precedence: git root first, CWD-level (project) last
        assert_eq!(dirs[0], root_sah, "First entry should be git root .sah/");
        assert_eq!(
            dirs[1], workspace_sah,
            "Second entry should be workspace .sah/"
        );
        let project_sah = project.join(".sah");
        assert_eq!(
            *dirs.last().unwrap(),
            project_sah,
            "Last entry should be CWD-level .sah/"
        );
    }

    #[test]
    fn test_default_impl() {
        // Exercises the `Default` impl for `ConfigurationDiscovery`.
        let discovery = ConfigurationDiscovery::default();
        // Should not panic and should have paths
        let _paths = discovery.paths();
    }

    #[test]
    fn test_discover_config_files_with_global_dir() {
        // Exercises the global_dir branch in `discover_config_files`.
        let temp_dir = TempDir::new().unwrap();
        let global_dir = temp_dir.path().join("global-sah");
        fs::create_dir_all(&global_dir).unwrap();
        fs::write(global_dir.join("sah.toml"), "key = \"val\"").unwrap();

        let discovery = ConfigurationDiscovery {
            paths: DiscoveryPaths {
                global_dir: Some(global_dir),
                project_dirs: vec![],
            },
            validate_security: false,
        };

        let files = discovery.discover_config_files();
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_find_config_files_yml_extension() {
        // Exercises the `.yml` extension detection.
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path();

        fs::write(dir_path.join("sah.yml"), "key: value\n").unwrap();

        let files = ConfigurationDiscovery::find_config_files_in_dir(dir_path);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("sah.yml"));
    }
}
