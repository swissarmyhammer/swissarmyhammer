//! Configuration file discovery system
//!
//! This module provides automatic discovery of SwissArmyHammer configuration files
//! in both project and global directories. It supports multiple file formats and
//! naming conventions with clear precedence ordering.

use std::path::{Path, PathBuf};
use tracing::{debug, trace, warn};

/// Represents a discovered configuration file with metadata
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigFile {
    /// Full path to the configuration file
    pub path: PathBuf,
    /// Detected format of the file (TOML, YAML, JSON)
    pub format: ConfigFormat,
    /// Scope indicating where the file was found (global vs project)
    pub scope: ConfigScope,
    /// Priority for ordering (higher values take precedence)
    pub priority: u8,
}

impl ConfigFile {
    /// Create a new ConfigFile with the given path, format, and scope
    pub fn new(path: PathBuf, format: ConfigFormat, scope: ConfigScope) -> Self {
        let priority = scope.priority();
        Self {
            path,
            format,
            scope,
            priority,
        }
    }
}

/// Configuration file format detected from file extension
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    /// TOML format (.toml extension)
    Toml,
    /// YAML format (.yaml or .yml extensions)
    Yaml,
    /// JSON format (.json extension)
    Json,
}

impl ConfigFormat {
    /// Detect format from file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "toml" => Some(Self::Toml),
            "yaml" | "yml" => Some(Self::Yaml),
            "json" => Some(Self::Json),
            _ => None,
        }
    }
}

/// Configuration scope indicating where the file was discovered
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigScope {
    /// Global configuration from ~/.swissarmyhammer/
    Global,
    /// Project configuration from ./.swissarmyhammer/
    Project,
}

impl ConfigScope {
    /// Get priority value for this scope (higher values override lower ones)
    pub fn priority(self) -> u8 {
        match self {
            Self::Global => 10,
            Self::Project => 20,
        }
    }
}

/// File discovery service for finding configuration files
pub struct FileDiscovery {
    /// Path to project configuration directory (./.swissarmyhammer/)
    project_dir: Option<PathBuf>,
    /// Path to global configuration directory (~/.swissarmyhammer/)
    global_dir: Option<PathBuf>,
}

impl FileDiscovery {
    /// Create a new FileDiscovery instance
    ///
    /// This will attempt to resolve both project and global directories
    /// at discovery time, allowing for directory changes between creation and use.
    pub fn new() -> Self {
        debug!("FileDiscovery created");

        Self {
            project_dir: None, // Will be resolved lazily
            global_dir: None,  // Will be resolved lazily
        }
    }

    /// Discover all configuration files in priority order
    ///
    /// Returns files sorted by priority (lower priority first, so higher priority
    /// files can override when merged by figment).
    pub fn discover_all(&self) -> Vec<ConfigFile> {
        let mut files = Vec::new();

        // Resolve directories at discovery time for flexibility
        let project_dir = self.project_dir.clone().or_else(Self::resolve_project_dir);
        let global_dir = self.global_dir.clone().or_else(Self::resolve_global_dir);

        debug!("FileDiscovery discovering files");
        if let Some(ref dir) = project_dir {
            debug!("Project directory: {}", dir.display());
        } else {
            debug!("Project directory: not available");
        }
        if let Some(ref dir) = global_dir {
            debug!("Global directory: {}", dir.display());
        } else {
            debug!("Global directory: not available");
        }

        // Search global directory first (lower priority)
        if let Some(ref global_dir) = global_dir {
            files.extend(self.search_directory(global_dir, ConfigScope::Global));
        }

        // Search project directory second (higher priority)
        if let Some(ref project_dir) = project_dir {
            files.extend(self.search_directory(project_dir, ConfigScope::Project));
        }

        // Sort by priority (ascending order for figment merging)
        files.sort_by_key(|f| f.priority);

        debug!("Discovered {} configuration files", files.len());
        for file in &files {
            trace!("Found config: {} ({:?})", file.path.display(), file.format);
        }

        files
    }

    /// Search a single directory for configuration files
    fn search_directory(&self, dir: &Path, scope: ConfigScope) -> Vec<ConfigFile> {
        if !dir.exists() {
            debug!("Directory does not exist: {}", dir.display());
            return Vec::new();
        }

        if !dir.is_dir() {
            warn!("Path exists but is not a directory: {}", dir.display());
            return Vec::new();
        }

        trace!("Searching directory: {}", dir.display());

        let mut files = Vec::new();
        let candidates = self.get_file_candidates(dir);

        for candidate in candidates {
            if candidate.exists() && candidate.is_file() {
                if let Some(config_file) = self.classify_file(&candidate, scope) {
                    files.push(config_file);
                }
            }
        }

        debug!("Found {} files in {}", files.len(), dir.display());
        files
    }

    /// Get all possible configuration file candidates in a directory
    fn get_file_candidates(&self, dir: &Path) -> Vec<PathBuf> {
        let file_names = [
            "sah.toml",
            "sah.yaml",
            "sah.yml",
            "sah.json",
            "swissarmyhammer.toml",
            "swissarmyhammer.yaml",
            "swissarmyhammer.yml",
            "swissarmyhammer.json",
        ];

        file_names.iter().map(|name| dir.join(name)).collect()
    }

    /// Classify a file path to determine if it's a valid config file
    fn classify_file(&self, path: &Path, scope: ConfigScope) -> Option<ConfigFile> {
        let extension = path.extension()?.to_str()?;
        let format = ConfigFormat::from_extension(extension)?;

        // Verify filename matches expected patterns
        let filename = path.file_name()?.to_str()?;
        if !self.is_valid_config_filename(filename) {
            return None;
        }

        Some(ConfigFile::new(path.to_path_buf(), format, scope))
    }

    /// Check if a filename matches valid configuration file patterns
    fn is_valid_config_filename(&self, filename: &str) -> bool {
        let valid_names = [
            "sah.toml",
            "sah.yaml",
            "sah.yml",
            "sah.json",
            "swissarmyhammer.toml",
            "swissarmyhammer.yaml",
            "swissarmyhammer.yml",
            "swissarmyhammer.json",
        ];

        valid_names.contains(&filename)
    }

    /// Resolve project configuration directory (./.swissarmyhammer/)
    fn resolve_project_dir() -> Option<PathBuf> {
        let current_dir = std::env::current_dir().ok()?;
        let sah_dir = current_dir.join(".swissarmyhammer");

        if sah_dir.exists() && sah_dir.is_dir() {
            Some(sah_dir)
        } else {
            None
        }
    }

    /// Resolve global configuration directory (~/.swissarmyhammer/)
    fn resolve_global_dir() -> Option<PathBuf> {
        let home_dir = dirs::home_dir()?;
        let sah_dir = home_dir.join(".swissarmyhammer");

        if sah_dir.exists() && sah_dir.is_dir() {
            Some(sah_dir)
        } else {
            None
        }
    }
}

impl Default for FileDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

impl FileDiscovery {
    /// Create a FileDiscovery with custom directories for testing
    #[cfg(test)]
    pub fn with_directories(project_dir: Option<PathBuf>, global_dir: Option<PathBuf>) -> Self {
        Self {
            project_dir,
            global_dir,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_config_format_from_extension() {
        assert_eq!(
            ConfigFormat::from_extension("toml"),
            Some(ConfigFormat::Toml)
        );
        assert_eq!(
            ConfigFormat::from_extension("yaml"),
            Some(ConfigFormat::Yaml)
        );
        assert_eq!(
            ConfigFormat::from_extension("yml"),
            Some(ConfigFormat::Yaml)
        );
        assert_eq!(
            ConfigFormat::from_extension("json"),
            Some(ConfigFormat::Json)
        );
        assert_eq!(ConfigFormat::from_extension("txt"), None);
        assert_eq!(ConfigFormat::from_extension(""), None);
    }

    #[test]
    fn test_config_format_case_insensitive() {
        assert_eq!(
            ConfigFormat::from_extension("TOML"),
            Some(ConfigFormat::Toml)
        );
        assert_eq!(
            ConfigFormat::from_extension("Yaml"),
            Some(ConfigFormat::Yaml)
        );
        assert_eq!(
            ConfigFormat::from_extension("JSON"),
            Some(ConfigFormat::Json)
        );
    }

    #[test]
    fn test_config_scope_priority() {
        assert_eq!(ConfigScope::Global.priority(), 10);
        assert_eq!(ConfigScope::Project.priority(), 20);
        assert!(ConfigScope::Project.priority() > ConfigScope::Global.priority());
    }

    #[test]
    fn test_config_file_creation() {
        let path = PathBuf::from("/test/sah.toml");
        let file = ConfigFile::new(path.clone(), ConfigFormat::Toml, ConfigScope::Project);

        assert_eq!(file.path, path);
        assert_eq!(file.format, ConfigFormat::Toml);
        assert_eq!(file.scope, ConfigScope::Project);
        assert_eq!(file.priority, ConfigScope::Project.priority());
    }

    #[test]
    fn test_is_valid_config_filename() {
        let discovery = FileDiscovery::new();

        // Valid short form names
        assert!(discovery.is_valid_config_filename("sah.toml"));
        assert!(discovery.is_valid_config_filename("sah.yaml"));
        assert!(discovery.is_valid_config_filename("sah.yml"));
        assert!(discovery.is_valid_config_filename("sah.json"));

        // Valid long form names
        assert!(discovery.is_valid_config_filename("swissarmyhammer.toml"));
        assert!(discovery.is_valid_config_filename("swissarmyhammer.yaml"));
        assert!(discovery.is_valid_config_filename("swissarmyhammer.yml"));
        assert!(discovery.is_valid_config_filename("swissarmyhammer.json"));

        // Invalid names
        assert!(!discovery.is_valid_config_filename("config.toml"));
        assert!(!discovery.is_valid_config_filename("sah.txt"));
        assert!(!discovery.is_valid_config_filename("something.toml"));
    }

    #[test]
    fn test_get_file_candidates() {
        let temp_dir = TempDir::new().unwrap();
        let discovery = FileDiscovery::new();

        let candidates = discovery.get_file_candidates(temp_dir.path());

        assert_eq!(candidates.len(), 8);
        assert!(candidates.contains(&temp_dir.path().join("sah.toml")));
        assert!(candidates.contains(&temp_dir.path().join("sah.yaml")));
        assert!(candidates.contains(&temp_dir.path().join("sah.yml")));
        assert!(candidates.contains(&temp_dir.path().join("sah.json")));
        assert!(candidates.contains(&temp_dir.path().join("swissarmyhammer.toml")));
        assert!(candidates.contains(&temp_dir.path().join("swissarmyhammer.yaml")));
        assert!(candidates.contains(&temp_dir.path().join("swissarmyhammer.yml")));
        assert!(candidates.contains(&temp_dir.path().join("swissarmyhammer.json")));
    }

    #[test]
    fn test_classify_file() {
        let temp_dir = TempDir::new().unwrap();
        let discovery = FileDiscovery::new();

        let toml_path = temp_dir.path().join("sah.toml");
        fs::write(&toml_path, "test = \"value\"").unwrap();

        let config_file = discovery.classify_file(&toml_path, ConfigScope::Project);

        assert!(config_file.is_some());
        let config_file = config_file.unwrap();
        assert_eq!(config_file.path, toml_path);
        assert_eq!(config_file.format, ConfigFormat::Toml);
        assert_eq!(config_file.scope, ConfigScope::Project);
    }

    #[test]
    fn test_classify_file_invalid_name() {
        let temp_dir = TempDir::new().unwrap();
        let discovery = FileDiscovery::new();

        let invalid_path = temp_dir.path().join("config.toml");
        fs::write(&invalid_path, "test = \"value\"").unwrap();

        let config_file = discovery.classify_file(&invalid_path, ConfigScope::Project);

        assert!(config_file.is_none());
    }

    #[test]
    fn test_search_directory_empty() {
        let temp_dir = TempDir::new().unwrap();
        let discovery = FileDiscovery::new();

        let files = discovery.search_directory(temp_dir.path(), ConfigScope::Project);

        assert!(files.is_empty());
    }

    #[test]
    fn test_search_directory_with_files() {
        let temp_dir = TempDir::new().unwrap();
        let discovery = FileDiscovery::new();

        // Create some config files
        fs::write(temp_dir.path().join("sah.toml"), "test = \"value\"").unwrap();
        fs::write(temp_dir.path().join("sah.yaml"), "test: value").unwrap();
        fs::write(temp_dir.path().join("invalid.toml"), "test = \"value\"").unwrap(); // Should be ignored

        let files = discovery.search_directory(temp_dir.path(), ConfigScope::Project);

        assert_eq!(files.len(), 2);

        // Check that both valid files are found
        let toml_found = files
            .iter()
            .any(|f| f.path.file_name().unwrap() == "sah.toml" && f.format == ConfigFormat::Toml);
        let yaml_found = files
            .iter()
            .any(|f| f.path.file_name().unwrap() == "sah.yaml" && f.format == ConfigFormat::Yaml);

        assert!(toml_found);
        assert!(yaml_found);

        // Check scope
        assert!(files.iter().all(|f| f.scope == ConfigScope::Project));
    }

    #[test]
    fn test_search_directory_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let nonexistent = temp_dir.path().join("nonexistent");
        let discovery = FileDiscovery::new();

        let files = discovery.search_directory(&nonexistent, ConfigScope::Project);

        assert!(files.is_empty());
    }

    #[test]
    fn test_discover_all_empty() {
        // Create a discovery with no directories
        let discovery = FileDiscovery {
            project_dir: None,
            global_dir: None,
        };

        let files = discovery.discover_all();
        assert!(files.is_empty());
    }

    #[test]
    fn test_file_priority_ordering() {
        let temp_dir = TempDir::new().unwrap();

        // Create global and project directories
        let global_dir = temp_dir.path().join("global");
        let project_dir = temp_dir.path().join("project");
        fs::create_dir_all(&global_dir).unwrap();
        fs::create_dir_all(&project_dir).unwrap();

        // Create config files
        fs::write(global_dir.join("sah.toml"), "global = true").unwrap();
        fs::write(project_dir.join("sah.toml"), "project = true").unwrap();

        let discovery = FileDiscovery {
            project_dir: Some(project_dir),
            global_dir: Some(global_dir),
        };

        let files = discovery.discover_all();

        assert_eq!(files.len(), 2);

        // Files should be sorted by priority (global first, project second)
        assert_eq!(files[0].scope, ConfigScope::Global);
        assert_eq!(files[1].scope, ConfigScope::Project);
        assert!(files[0].priority < files[1].priority);
    }
}
