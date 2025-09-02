use crate::error::{ConfigurationError, ConfigurationResult};
use std::path::{Path, PathBuf};
use tracing::debug;

/// Configuration file discovery paths
#[derive(Debug, Clone)]
pub struct DiscoveryPaths {
    /// Global configuration directory (~/.swissarmyhammer/)
    pub global_dir: Option<PathBuf>,
    /// Project configuration directory (./.swissarmyhammer/)
    pub project_dir: Option<PathBuf>,
}

/// Configuration file discovery utility
pub struct ConfigurationDiscovery {
    paths: DiscoveryPaths,
    validate_security: bool,
}

impl ConfigurationDiscovery {
    /// Create a new configuration discovery instance
    pub fn new() -> ConfigurationResult<Self> {
        let global_dir = Self::find_global_config_dir()?;
        let project_dir = Self::find_project_config_dir()?;

        Ok(Self {
            paths: DiscoveryPaths {
                global_dir,
                project_dir,
            },
            validate_security: true,
        })
    }

    /// Create a discovery instance for CLI usage (no security validation)
    pub fn for_cli() -> ConfigurationResult<Self> {
        let mut discovery = Self::new()?;
        discovery.validate_security = false;
        Ok(discovery)
    }

    /// Get the discovered paths
    pub fn paths(&self) -> &DiscoveryPaths {
        &self.paths
    }

    /// Find all configuration files in priority order
    /// Returns paths in ascending precedence order (earlier files are overridden by later ones)
    pub fn discover_config_files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();

        // Global config files (lowest precedence)
        if let Some(ref global_dir) = self.paths.global_dir {
            files.extend(self.find_config_files_in_dir(global_dir));
        }

        // Project config files from all discovered directories (higher precedence)
        // Find all .swissarmyhammer directories in the path hierarchy
        let project_dirs = Self::find_all_project_config_dirs().unwrap_or_default();
        for project_dir in project_dirs {
            files.extend(self.find_config_files_in_dir(&project_dir));
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

    /// Find global config directory (~/.swissarmyhammer/)
    fn find_global_config_dir() -> ConfigurationResult<Option<PathBuf>> {
        // First try to get HOME environment variable (respects test environment)
        let home = std::env::var("HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(dirs::home_dir); // Fallback to dirs::home_dir() if HOME not set

        match home {
            Some(home) => {
                let config_dir = home.join(".swissarmyhammer");
                if config_dir.is_dir() {
                    debug!("Found global config directory: {}", config_dir.display());
                    Ok(Some(config_dir))
                } else {
                    debug!(
                        "Global config directory not found: {}",
                        config_dir.display()
                    );
                    Ok(None)
                }
            }
            None => {
                debug!("Could not determine home directory");
                Ok(None)
            }
        }
    }

    /// Find project config directory (./.swissarmyhammer/ or walk up to find repo root)
    fn find_project_config_dir() -> ConfigurationResult<Option<PathBuf>> {
        let all_dirs = Self::find_all_project_config_dirs()?;
        // Return the closest (most specific) directory if any found
        Ok(all_dirs.last().cloned())
    }

    /// Find ALL project config directories by walking up the directory tree
    /// Returns directories in ascending precedence order (parent directories first)
    fn find_all_project_config_dirs() -> ConfigurationResult<Vec<PathBuf>> {
        let current_dir = match std::env::current_dir() {
            Ok(dir) => dir,
            Err(e) => {
                debug!(
                    "Could not get current directory ({}), no project config directories found",
                    e
                );
                return Ok(Vec::new());
            }
        };

        let mut config_dirs = Vec::new();
        let mut dir = Some(current_dir.as_path());

        while let Some(current) = dir {
            let config_dir = current.join(".swissarmyhammer");
            if config_dir.is_dir() {
                debug!("Found project config directory: {}", config_dir.display());
                config_dirs.push(config_dir);
            }

            // Stop at git repository root
            if current.join(".git").exists() {
                debug!("Reached git repository root");
                break;
            }

            dir = current.parent();
        }

        // Reverse to get parent directories first (lower precedence)
        config_dirs.reverse();

        if config_dirs.is_empty() {
            debug!("No project config directories found");
        } else {
            debug!("Found {} project config directories", config_dirs.len());
        }

        Ok(config_dirs)
    }

    /// Find all configuration files in a directory
    fn find_config_files_in_dir(&self, dir: &Path) -> Vec<PathBuf> {
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

    /// Validate file security (only when validation is enabled)
    fn validate_file_security(&self, path: &Path) -> ConfigurationResult<()> {
        // Get file metadata
        let metadata = path.metadata().map_err(|e| {
            ConfigurationError::discovery(format!(
                "Could not read metadata for {}: {}",
                path.display(),
                e
            ))
        })?;

        // Check if file is readable
        if metadata.permissions().readonly() {
            return Err(ConfigurationError::discovery(format!(
                "Configuration file is not readable: {}",
                path.display()
            )));
        }

        // On Unix systems, check file permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = metadata.permissions().mode();

            // Check if file is world-readable (others can read)
            if mode & 0o004 != 0 {
                debug!(
                    "Warning: Configuration file is world-readable: {}",
                    path.display()
                );
            }

            // Check if file is world-writable (others can write)
            if mode & 0o002 != 0 {
                return Err(ConfigurationError::discovery(format!(
                    "Configuration file is world-writable (security risk): {}",
                    path.display()
                )));
            }
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

        let discovery = ConfigurationDiscovery::for_cli().unwrap();
        let files = discovery.find_config_files_in_dir(dir_path);

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

        let discovery = ConfigurationDiscovery::for_cli().unwrap();
        let files = discovery.find_config_files_in_dir(dir_path);

        assert_eq!(files.len(), 0);
    }

    #[test]
    fn test_validate_file_security() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.toml");
        fs::write(&file_path, "[test]\nkey = \"value\"\n").unwrap();

        let discovery = ConfigurationDiscovery::new().unwrap();

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
    fn test_find_all_project_config_dirs_nested() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_dir = temp_dir.path().join("workspace");
        let project_dir = workspace_dir.join("my-project");
        let nested_dir = project_dir.join("src").join("components");

        // Create nested directory structure
        fs::create_dir_all(&nested_dir).unwrap();

        // Create workspace config directory
        let workspace_config_dir = workspace_dir.join(".swissarmyhammer");
        fs::create_dir_all(&workspace_config_dir).unwrap();
        fs::write(
            workspace_config_dir.join("sah.toml"),
            "[workspace]\nname = \"test-workspace\"\n",
        )
        .unwrap();

        // Create project config directory
        let project_config_dir = project_dir.join(".swissarmyhammer");
        fs::create_dir_all(&project_config_dir).unwrap();
        fs::write(
            project_config_dir.join("sah.toml"),
            "[project]\nname = \"test-project\"\n",
        )
        .unwrap();

        // Create a fake .git directory to limit the search scope
        fs::create_dir(workspace_dir.join(".git")).unwrap();

        // Change to nested directory (save original to restore later)
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&nested_dir).unwrap();

        // Test discovery finds both config directories
        let result = ConfigurationDiscovery::find_all_project_config_dirs();

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();

        let config_dirs = result.unwrap();
        println!("Found {} config directories:", config_dirs.len());
        for (i, dir) in config_dirs.iter().enumerate() {
            println!("  {}: {}", i, dir.display());
        }

        // Should find exactly our 2 directories (workspace + project)
        assert_eq!(
            config_dirs.len(),
            2,
            "Should find exactly workspace and project config directories"
        );

        // Verify our directories are found and in correct order
        assert!(
            config_dirs[0].ends_with("workspace/.swissarmyhammer"),
            "First should be workspace config"
        );
        assert!(
            config_dirs[1].ends_with("my-project/.swissarmyhammer"),
            "Second should be project config"
        );
    }
}
