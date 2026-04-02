//! Virtual file system for loading files from managed directories.
//!
//! This module provides a unified way to load files from the hierarchical
//! directory structure, handling precedence and overrides.
//!
//! # Search Path Modes
//!
//! The VirtualFileSystem supports two modes for resolving directories:
//!
//! - **Managed directory mode** (default): Uses `ManagedDirectory<C>` to resolve
//!   XDG data paths (e.g., `$XDG_DATA_HOME/sah/{subdirectory}`) for user-level files
//!   and `{git_root}/.sah/{subdirectory}` for project-local files.
//!
//! - **Custom search paths mode**: When search paths are configured via
//!   [`VirtualFileSystem::add_search_path`] or [`VirtualFileSystem::use_dot_directory_paths`],
//!   the VFS loads directly from those paths, bypassing `ManagedDirectory`.

use crate::config::DirectoryConfig;
use crate::directory::{find_git_repository_root, xdg_base_dir, ManagedDirectory};
use crate::error::Result;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Maximum file size to load (10MB).
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Source of a file (builtin, user, local, or dynamic).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub enum FileSource {
    /// Builtin files embedded in the binary.
    Builtin,
    /// User files from XDG data directory (e.g., $XDG_DATA_HOME/sah/).
    User,
    /// Local files from project directory (e.g., ./.sah).
    Local,
    /// Dynamically generated files.
    Dynamic,
}

impl FileSource {
    /// Get emoji-based display string for the file source.
    pub fn display_emoji(&self) -> &'static str {
        match self {
            FileSource::Builtin | FileSource::Dynamic => "📦 Built-in",
            FileSource::Local => "📁 Project",
            FileSource::User => "👤 User",
        }
    }
}

impl std::fmt::Display for FileSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileSource::Builtin => write!(f, "builtin"),
            FileSource::User => write!(f, "user"),
            FileSource::Local => write!(f, "local"),
            FileSource::Dynamic => write!(f, "dynamic"),
        }
    }
}

/// A directory to search for files, paired with a precedence source.
///
/// Search paths allow explicit control over where the VirtualFileSystem
/// looks for files, bypassing the default `ManagedDirectory` resolution.
///
/// # Example
///
/// ```rust
/// use swissarmyhammer_directory::{SearchPath, FileSource};
/// use std::path::PathBuf;
///
/// let path = SearchPath {
///     path: PathBuf::from("/home/user/.prompts"),
///     source: FileSource::User,
/// };
/// ```
#[derive(Debug, Clone)]
pub struct SearchPath {
    /// The absolute directory path to search for files.
    pub path: PathBuf,
    /// The precedence source for files found in this path.
    pub source: FileSource,
}

/// Represents a file with its metadata.
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// The logical name of the file (without extension).
    pub name: String,
    /// The full path to the file.
    pub path: PathBuf,
    /// The file content.
    pub content: String,
    /// Where this file came from.
    pub source: FileSource,
}

impl FileEntry {
    /// Create a new FileEntry with explicit name.
    pub fn new(
        name: impl Into<String>,
        path: PathBuf,
        content: String,
        source: FileSource,
    ) -> Self {
        Self {
            name: name.into(),
            path,
            content,
            source,
        }
    }

    /// Create a FileEntry from path and content, deriving name from the path.
    pub fn from_path_and_content(path: PathBuf, content: String, source: FileSource) -> Self {
        let stem = Self::remove_compound_extensions(&path);
        let name = Self::extract_name_from_path(&path, stem);

        Self {
            name,
            path,
            content,
            source,
        }
    }

    /// Remove compound extensions from a filename.
    fn remove_compound_extensions(path: &Path) -> &str {
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();

        // List of supported compound extensions (sorted by length descending)
        let extensions = [
            ".md.liquid",
            ".markdown.liquid",
            ".liquid.md",
            ".md",
            ".markdown",
            ".liquid",
        ];

        for ext in &extensions {
            if let Some(stem) = filename.strip_suffix(ext) {
                return stem;
            }
        }

        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
    }

    /// Extract the name from a path using proper path operations.
    fn extract_name_from_path(path: &Path, stem: &str) -> String {
        let mut components: Vec<String> = Vec::new();
        let mut found_subdirectory = false;

        // Known subdirectory names that mark the start of the logical name.
        // Includes both bare names (for managed directory paths like .sah/prompts)
        // and dot-prefixed names (for dot-directory paths like .prompts).
        let known_subdirs = [
            "prompts",
            "workflows",
            "rules",
            "validators",
            "modes",
            "docs",
            ".prompts",
            ".workflows",
            ".rules",
            ".validators",
            ".modes",
            ".docs",
        ];

        for component in path.components() {
            if let std::path::Component::Normal(os_str) = component {
                if let Some(s) = os_str.to_str() {
                    if found_subdirectory {
                        components.push(s.to_string());
                    } else if known_subdirs.contains(&s) {
                        found_subdirectory = true;
                    }
                }
            }
        }

        if found_subdirectory && !components.is_empty() {
            components.pop(); // Remove filename with extension
            if !components.is_empty() {
                components.push(stem.to_string());
                components.join("/")
            } else {
                stem.to_string()
            }
        } else {
            stem.to_string()
        }
    }
}

/// Virtual file system that manages files from multiple sources.
///
/// The VirtualFileSystem provides a unified interface for loading and managing
/// files from different sources (builtin, user, local, dynamic) with proper
/// precedence handling.
///
/// # Type Parameters
///
/// * `C` - A type implementing `DirectoryConfig` that specifies the directory
///   configuration.
///
/// # Precedence
///
/// Files are loaded with the following precedence (later sources override earlier):
/// 1. Builtin files (embedded in the binary)
/// 2. User files (from home directory)
/// 3. Local files (from project directory)
/// 4. Dynamic files (programmatically added)
pub struct VirtualFileSystem<C: DirectoryConfig> {
    /// The subdirectory to look for (e.g., "prompts" or "validators").
    pub subdirectory: String,
    /// Map of file names to file entries (highest-precedence version wins).
    pub files: HashMap<String, FileEntry>,
    /// Track sources for each file.
    pub file_sources: HashMap<String, FileSource>,
    /// Full stack of all file versions, keyed by name, ordered by load order
    /// (lowest precedence first). Use [`get_stack`](Self::get_stack) to access.
    file_stacks: HashMap<String, Vec<FileEntry>>,
    /// Explicit search paths. When non-empty, `load_all` uses these instead of ManagedDirectory.
    search_paths: Vec<SearchPath>,
    /// When true, `load_all` resolves dot-directory paths lazily (e.g., `~/.prompts`, `{git_root}/.prompts`).
    use_dot_dirs: bool,
    /// Phantom data for the configuration type.
    _phantom: PhantomData<C>,
}

impl<C: DirectoryConfig> VirtualFileSystem<C> {
    /// Create a new virtual file system for a specific subdirectory.
    pub fn new(subdirectory: impl Into<String>) -> Self {
        Self {
            subdirectory: subdirectory.into(),
            files: HashMap::new(),
            file_sources: HashMap::new(),
            file_stacks: HashMap::new(),
            search_paths: Vec::new(),
            use_dot_dirs: false,
            _phantom: PhantomData,
        }
    }

    /// Add a builtin file.
    pub fn add_builtin(&mut self, name: impl Into<String>, content: impl Into<String>) {
        let name = name.into();
        let entry = FileEntry::new(
            name.clone(),
            PathBuf::from(format!("builtin:/{}/{}", self.subdirectory, name)),
            content.into(),
            FileSource::Builtin,
        );
        self.add_file(entry);
    }

    /// Add a file entry.
    ///
    /// The entry replaces any existing file with the same name (highest-precedence wins
    /// in [`get`](Self::get)). All versions are preserved in the stack, accessible
    /// via [`get_stack`](Self::get_stack).
    pub fn add_file(&mut self, entry: FileEntry) {
        self.file_sources
            .insert(entry.name.clone(), entry.source.clone());
        self.file_stacks
            .entry(entry.name.clone())
            .or_default()
            .push(entry.clone());
        self.files.insert(entry.name.clone(), entry);
    }

    /// Get a file by name (highest-precedence version).
    pub fn get(&self, name: &str) -> Option<&FileEntry> {
        self.files.get(name)
    }

    /// Get the full stack of all versions of a file, ordered by load order
    /// (lowest precedence first, highest precedence last).
    ///
    /// This is useful when you need to merge content from all layers rather
    /// than just taking the highest-precedence version.
    pub fn get_stack(&self, name: &str) -> Option<&[FileEntry]> {
        self.file_stacks.get(name).map(|v| v.as_slice())
    }

    /// Get the source of a file.
    pub fn get_source(&self, name: &str) -> Option<&FileSource> {
        self.file_sources.get(name)
    }

    /// List all files.
    pub fn list(&self) -> Vec<&FileEntry> {
        self.files.values().collect()
    }

    /// Add an explicit search path for file loading.
    ///
    /// When search paths are configured, [`load_all`](Self::load_all) uses them
    /// instead of the default `ManagedDirectory<C>` resolution. Search paths are
    /// loaded in order, with later paths taking precedence over earlier ones.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use swissarmyhammer_directory::{VirtualFileSystem, SwissarmyhammerConfig, FileSource};
    /// use std::path::PathBuf;
    ///
    /// let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
    /// vfs.add_search_path(PathBuf::from("/home/user/.prompts"), FileSource::User);
    /// vfs.add_search_path(PathBuf::from("/project/.prompts"), FileSource::Local);
    /// vfs.load_all().unwrap();
    /// ```
    pub fn add_search_path(&mut self, path: PathBuf, source: FileSource) {
        self.search_paths.push(SearchPath { path, source });
    }

    /// Configure dot-directory resolution for user (XDG) and project (git root) paths.
    ///
    /// This is a convenience method that tells [`load_all`](Self::load_all) to use
    /// XDG data paths for user-level files and dot-directories for project-local files.
    /// Paths are resolved lazily at load time.
    ///
    /// For example, with subdirectory "prompts" and `SwissarmyhammerConfig`, `load_all` will search:
    /// 1. `$XDG_DATA_HOME/sah/prompts` (User source)
    /// 2. `{git_root}/.prompts` (Local source) — falls back to current directory if not in a git repo
    pub fn use_dot_directory_paths(&mut self) {
        self.use_dot_dirs = true;
    }

    /// Load files from a base directory, joining with the subdirectory name.
    ///
    /// Looks for files in `{base_path}/{subdirectory}/` and loads them with
    /// the given source precedence.
    pub fn load_directory(&mut self, base_path: &Path, source: FileSource) -> Result<()> {
        let target_dir = base_path.join(&self.subdirectory);
        self.load_files_from_dir(&target_dir, source)
    }

    /// Load files directly from an absolute directory path.
    ///
    /// Unlike [`load_directory`](Self::load_directory), this does not join with the
    /// subdirectory name — the given path is used as-is.
    pub fn load_files_from_dir(&mut self, target_dir: &Path, source: FileSource) -> Result<()> {
        if !target_dir.exists() {
            return Ok(());
        }

        let file_paths = WalkDir::new(target_dir)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_file())
            .filter_map(|entry| {
                let path = entry.path();
                if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                    // Check for compound extensions first
                    let compound_extensions = [".md.liquid", ".markdown.liquid", ".liquid.md"];

                    for compound_ext in &compound_extensions {
                        if filename.ends_with(compound_ext) {
                            return Some(path.to_path_buf());
                        }
                    }

                    // Check for supported single extensions
                    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                        if ["md", "markdown", "yaml", "yml", "mermaid"].contains(&ext) {
                            return Some(path.to_path_buf());
                        }
                    }
                }
                None
            });

        for path in file_paths {
            match std::fs::metadata(&path) {
                Ok(metadata) => {
                    if metadata.len() > MAX_FILE_SIZE {
                        tracing::warn!(
                            "Skipping file '{}' - size {} bytes exceeds limit of {} bytes",
                            path.display(),
                            metadata.len(),
                            MAX_FILE_SIZE
                        );
                        continue;
                    }

                    if !Self::is_path_safe(&path, target_dir) {
                        tracing::warn!(
                            "Skipping file '{}' - path validation failed",
                            path.display()
                        );
                        continue;
                    }

                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let file_entry =
                            FileEntry::from_path_and_content(path, content, source.clone());
                        self.add_file(file_entry);
                    } else {
                        tracing::warn!("Failed to read file '{}'", path.display());
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to get metadata for '{}': {}", path.display(), e);
                }
            }
        }

        Ok(())
    }

    /// Load all files following the standard precedence.
    ///
    /// The loading strategy depends on configuration:
    ///
    /// 1. **Dot-directory mode** (via [`use_dot_directory_paths`](Self::use_dot_directory_paths)):
    ///    Loads from `~/.{subdirectory}` and `{git_root}/.{subdirectory}`.
    ///
    /// 2. **Custom search paths** (via [`add_search_path`](Self::add_search_path)):
    ///    Loads from the explicitly configured paths in order.
    ///
    /// 3. **Managed directory mode** (default):
    ///    Loads from XDG data directory for user-level and `ManagedDirectory<C>` for project-local.
    pub fn load_all(&mut self) -> Result<()> {
        if self.use_dot_dirs {
            // Resolve dot-directory paths lazily
            self.load_dot_directory_files()?;
        } else if !self.search_paths.is_empty() {
            // Use explicit search paths
            let paths: Vec<SearchPath> = self.search_paths.clone();
            for sp in &paths {
                self.load_files_from_dir(&sp.path, sp.source.clone())?;
            }
        } else {
            // Default: use ManagedDirectory-based loading with XDG data directory
            if let Ok(dir) = ManagedDirectory::<C>::xdg_data() {
                self.load_directory(dir.root(), FileSource::User)?;
            }
            self.load_local_files_managed()?;
        }

        Ok(())
    }

    /// Load files from dot-directory paths, resolved lazily.
    ///
    /// Uses the XDG data directory for the user-level path:
    /// `$XDG_DATA_HOME/{XDG_NAME}/{subdirectory}` (or `~/.local/share/{XDG_NAME}/{subdirectory}`).
    fn load_dot_directory_files(&mut self) -> Result<()> {
        // User XDG data directory
        if let Ok(base) = xdg_base_dir("XDG_DATA_HOME", ".local/share") {
            let user_dir = base.join(C::XDG_NAME).join(&self.subdirectory);
            self.load_files_from_dir(&user_dir, FileSource::User)?;
        }

        // Git root or current directory fallback (dot-directory style)
        let dot_name = format!(".{}", self.subdirectory);
        if let Some(git_root) = find_git_repository_root() {
            self.load_files_from_dir(&git_root.join(&dot_name), FileSource::Local)?;
        } else if let Ok(current_dir) = std::env::current_dir() {
            self.load_files_from_dir(&current_dir.join(&dot_name), FileSource::Local)?;
        }

        Ok(())
    }

    /// Load local files from the Git repository directory using ManagedDirectory.
    fn load_local_files_managed(&mut self) -> Result<()> {
        if let Ok(dir) = ManagedDirectory::<C>::from_git_root() {
            self.load_directory(dir.root(), FileSource::Local)?;
            return Ok(());
        }

        // Fallback: use current directory
        if let Ok(current_dir) = std::env::current_dir() {
            if let Ok(dir) = ManagedDirectory::<C>::from_custom_root(current_dir) {
                tracing::debug!(
                    "Using fallback directory detection: {}",
                    dir.root().display()
                );
                self.load_directory(dir.root(), FileSource::Local)?;
            }
        }

        Ok(())
    }

    /// Get all directories that are being monitored.
    pub fn get_directories(&self) -> Result<Vec<PathBuf>> {
        if self.use_dot_dirs {
            let dot_name = format!(".{}", self.subdirectory);
            let mut directories = Vec::new();

            // User XDG data directory
            if let Ok(base) = xdg_base_dir("XDG_DATA_HOME", ".local/share") {
                let user_dir = base.join(C::XDG_NAME).join(&self.subdirectory);
                if user_dir.exists() && user_dir.is_dir() {
                    directories.push(user_dir);
                }
            }

            // Git root or current directory fallback (dot-directory style)
            if let Some(git_root) = find_git_repository_root() {
                let local_dir = git_root.join(&dot_name);
                if local_dir.exists() && local_dir.is_dir() {
                    directories.push(local_dir);
                }
            } else if let Ok(current_dir) = std::env::current_dir() {
                let local_dir = current_dir.join(&dot_name);
                if local_dir.exists() && local_dir.is_dir() {
                    directories.push(local_dir);
                }
            }

            return Ok(directories);
        }

        if !self.search_paths.is_empty() {
            // Return configured search paths that exist
            return Ok(self
                .search_paths
                .iter()
                .filter(|sp| sp.path.exists() && sp.path.is_dir())
                .map(|sp| sp.path.clone())
                .collect());
        }

        // Default: use ManagedDirectory-based resolution with XDG data directory
        let mut directories = Vec::new();

        // User XDG data directory
        if let Ok(dir) = ManagedDirectory::<C>::xdg_data() {
            let user_dir = dir.subdir(&self.subdirectory);
            if user_dir.exists() {
                directories.push(user_dir);
            }
        }

        // Local Git repository directory
        if let Ok(dir) = ManagedDirectory::<C>::from_git_root() {
            let subdir = dir.subdir(&self.subdirectory);
            if subdir.exists() && subdir.is_dir() {
                directories.push(subdir);
            }
        } else if let Ok(current_dir) = std::env::current_dir() {
            if let Ok(dir) = ManagedDirectory::<C>::from_custom_root(current_dir) {
                let subdir = dir.subdir(&self.subdirectory);
                if subdir.exists() && subdir.is_dir() {
                    directories.push(subdir);
                }
            }
        }

        Ok(directories)
    }

    /// Get the effective search paths with their source metadata.
    ///
    /// Resolves the same directories as [`load_all`](Self::load_all) / [`get_directories`](Self::get_directories)
    /// but returns [`SearchPath`] entries so callers can pair each directory with its
    /// [`FileSource`].  Only directories that exist on disk are returned.
    ///
    /// This is useful for consumers that need to walk directories themselves (e.g.,
    /// skill resolvers that enumerate subdirectories rather than individual files).
    pub fn get_search_paths(&self) -> Vec<SearchPath> {
        let mut paths = Vec::new();

        if self.use_dot_dirs {
            let dot_name = format!(".{}", self.subdirectory);

            // User XDG data directory
            if let Ok(base) = xdg_base_dir("XDG_DATA_HOME", ".local/share") {
                let user_dir = base.join(C::XDG_NAME).join(&self.subdirectory);
                if user_dir.exists() && user_dir.is_dir() {
                    paths.push(SearchPath {
                        path: user_dir,
                        source: FileSource::User,
                    });
                }
            }

            // Git root or current directory fallback (dot-directory style)
            if let Some(git_root) = find_git_repository_root() {
                let local_dir = git_root.join(&dot_name);
                if local_dir.exists() && local_dir.is_dir() {
                    paths.push(SearchPath {
                        path: local_dir,
                        source: FileSource::Local,
                    });
                }
            } else if let Ok(current_dir) = std::env::current_dir() {
                let local_dir = current_dir.join(&dot_name);
                if local_dir.exists() && local_dir.is_dir() {
                    paths.push(SearchPath {
                        path: local_dir,
                        source: FileSource::Local,
                    });
                }
            }
        } else if self.search_paths.is_empty() {
            // Default: use ManagedDirectory-based resolution with XDG data directory
            if let Ok(dir) = ManagedDirectory::<C>::xdg_data() {
                let user_dir = dir.subdir(&self.subdirectory);
                if user_dir.exists() {
                    paths.push(SearchPath {
                        path: user_dir,
                        source: FileSource::User,
                    });
                }
            }

            if let Ok(dir) = ManagedDirectory::<C>::from_git_root() {
                let subdir = dir.subdir(&self.subdirectory);
                if subdir.exists() && subdir.is_dir() {
                    paths.push(SearchPath {
                        path: subdir,
                        source: FileSource::Local,
                    });
                }
            } else if let Ok(current_dir) = std::env::current_dir() {
                if let Ok(dir) = ManagedDirectory::<C>::from_custom_root(current_dir) {
                    let subdir = dir.subdir(&self.subdirectory);
                    if subdir.exists() && subdir.is_dir() {
                        paths.push(SearchPath {
                            path: subdir,
                            source: FileSource::Local,
                        });
                    }
                }
            }
        }

        // Always append explicit search paths (highest precedence).
        // These are added via add_search_path() and may coexist with
        // dot-directory or managed-directory resolution.
        for sp in &self.search_paths {
            if sp.path.exists() && sp.path.is_dir() {
                paths.push(sp.clone());
            }
        }

        paths
    }

    /// Validate that a path is safe and within the expected directory.
    fn is_path_safe(path: &Path, base_dir: &Path) -> bool {
        match (path.canonicalize(), base_dir.canonicalize()) {
            (Ok(canonical_path), Ok(canonical_base)) => canonical_path.starts_with(&canonical_base),
            _ => {
                let path_str = path.to_string_lossy();
                !path_str.contains("..") && !path_str.contains('~')
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SwissarmyhammerConfig;
    use serial_test::serial;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_file_source_display() {
        assert_eq!(FileSource::Builtin.to_string(), "builtin");
        assert_eq!(FileSource::User.to_string(), "user");
        assert_eq!(FileSource::Local.to_string(), "local");
        assert_eq!(FileSource::Dynamic.to_string(), "dynamic");
    }

    #[test]
    fn test_file_source_display_emoji() {
        assert_eq!(FileSource::Builtin.display_emoji(), "📦 Built-in");
        assert_eq!(FileSource::User.display_emoji(), "👤 User");
        assert_eq!(FileSource::Local.display_emoji(), "📁 Project");
    }

    #[test]
    fn test_file_entry_creation() {
        let entry = FileEntry::new(
            "test_file",
            PathBuf::from("/path/to/file"),
            "content".to_string(),
            FileSource::Local,
        );
        assert_eq!(entry.name, "test_file");
        assert_eq!(entry.path, PathBuf::from("/path/to/file"));
        assert_eq!(entry.content, "content");
        assert_eq!(entry.source, FileSource::Local);
    }

    #[test]
    fn test_virtual_file_system_new() {
        let vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        assert_eq!(vfs.subdirectory, "prompts");
        assert!(vfs.files.is_empty());
    }

    #[test]
    fn test_virtual_file_system_add_builtin() {
        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        vfs.add_builtin("test", "content");

        let file = vfs.get("test").unwrap();
        assert_eq!(file.name, "test");
        assert_eq!(file.content, "content");
        assert_eq!(file.source, FileSource::Builtin);
    }

    #[test]
    fn test_virtual_file_system_load_directory() {
        let temp_dir = TempDir::new().unwrap();
        let prompts_dir = temp_dir.path().join("prompts");
        fs::create_dir_all(&prompts_dir).unwrap();

        let test_file = prompts_dir.join("test.md");
        fs::write(&test_file, "test content").unwrap();

        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        vfs.load_directory(temp_dir.path(), FileSource::Local)
            .unwrap();

        let file = vfs.get("test").unwrap();
        assert_eq!(file.name, "test");
        assert_eq!(file.content, "test content");
        assert_eq!(file.source, FileSource::Local);
    }

    #[test]
    fn test_virtual_file_system_precedence() {
        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");

        // Add builtin first
        vfs.add_builtin("test", "builtin content");

        // Add user version (should override)
        let entry = FileEntry::new(
            "test",
            PathBuf::from("/home/user/.sah/prompts/test.md"),
            "user content".to_string(),
            FileSource::User,
        );
        vfs.add_file(entry);

        let file = vfs.get("test").unwrap();
        assert_eq!(file.content, "user content");
        assert_eq!(file.source, FileSource::User);
    }

    #[test]
    fn test_get_stack_preserves_all_versions() {
        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");

        // Add builtin
        vfs.add_builtin("config", "builtin content");

        // Add user version
        let user_entry = FileEntry::new(
            "config",
            PathBuf::from("/home/user/.shell/config.yaml"),
            "user content".to_string(),
            FileSource::User,
        );
        vfs.add_file(user_entry);

        // Add local version
        let local_entry = FileEntry::new(
            "config",
            PathBuf::from("/project/.shell/config.yaml"),
            "local content".to_string(),
            FileSource::Local,
        );
        vfs.add_file(local_entry);

        // get() returns the winner (last added)
        let winner = vfs.get("config").unwrap();
        assert_eq!(winner.content, "local content");

        // get_stack() returns all three in load order
        let stack = vfs.get_stack("config").unwrap();
        assert_eq!(stack.len(), 3);
        assert_eq!(stack[0].content, "builtin content");
        assert_eq!(stack[0].source, FileSource::Builtin);
        assert_eq!(stack[1].content, "user content");
        assert_eq!(stack[1].source, FileSource::User);
        assert_eq!(stack[2].content, "local content");
        assert_eq!(stack[2].source, FileSource::Local);
    }

    #[test]
    fn test_get_stack_returns_none_for_missing() {
        let vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        assert!(vfs.get_stack("nonexistent").is_none());
    }

    #[test]
    fn test_virtual_file_system_list() {
        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");

        vfs.add_builtin("test1", "content1");
        vfs.add_builtin("test2", "content2");

        let files = vfs.list();
        assert_eq!(files.len(), 2);

        let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"test1"));
        assert!(names.contains(&"test2"));
    }

    #[test]
    fn test_search_path_loading() {
        let temp_dir = TempDir::new().unwrap();

        // Create a dot-directory style path
        let dot_prompts = temp_dir.path().join(".prompts");
        fs::create_dir_all(&dot_prompts).unwrap();
        fs::write(dot_prompts.join("my_prompt.md"), "dot prompt content").unwrap();

        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        vfs.add_search_path(dot_prompts.clone(), FileSource::Local);
        vfs.load_all().unwrap();

        let file = vfs.get("my_prompt").unwrap();
        assert_eq!(file.content, "dot prompt content");
        assert_eq!(file.source, FileSource::Local);
    }

    #[test]
    fn test_search_path_precedence() {
        let temp_dir = TempDir::new().unwrap();

        // Create user-level dot-directory
        let user_dir = temp_dir.path().join("home").join(".prompts");
        fs::create_dir_all(&user_dir).unwrap();
        fs::write(user_dir.join("shared.md"), "user version").unwrap();

        // Create local-level dot-directory
        let local_dir = temp_dir.path().join("project").join(".prompts");
        fs::create_dir_all(&local_dir).unwrap();
        fs::write(local_dir.join("shared.md"), "local version").unwrap();

        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        vfs.add_search_path(user_dir, FileSource::User);
        vfs.add_search_path(local_dir, FileSource::Local);
        vfs.load_all().unwrap();

        // Local should override user
        let file = vfs.get("shared").unwrap();
        assert_eq!(file.content, "local version");
        assert_eq!(file.source, FileSource::Local);
    }

    #[test]
    fn test_search_path_get_directories() {
        let temp_dir = TempDir::new().unwrap();

        // Create one existing and one non-existing path
        let existing = temp_dir.path().join(".prompts");
        fs::create_dir_all(&existing).unwrap();

        let missing = temp_dir.path().join(".nonexistent");

        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        vfs.add_search_path(existing.clone(), FileSource::User);
        vfs.add_search_path(missing, FileSource::Local);

        let dirs = vfs.get_directories().unwrap();
        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0], existing);
    }

    #[test]
    fn test_load_files_from_dir_directly() {
        let temp_dir = TempDir::new().unwrap();

        // Create files directly (no subdirectory join)
        let dir = temp_dir.path().join("direct");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("test.md"), "direct content").unwrap();

        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        vfs.load_files_from_dir(&dir, FileSource::Local).unwrap();

        let file = vfs.get("test").unwrap();
        assert_eq!(file.content, "direct content");
    }

    #[test]
    fn test_dot_directory_name_extraction() {
        // Test that file names are correctly extracted from dot-directory paths
        let entry = FileEntry::from_path_and_content(
            PathBuf::from("/home/user/.prompts/category/test.md"),
            "content".to_string(),
            FileSource::User,
        );
        assert_eq!(entry.name, "category/test");
    }

    #[test]
    fn test_remove_compound_extensions_unrecognized_extension() {
        // A file with an extension not in the supported list hits the fallback
        // branch (lines 153-154) that uses path.file_stem().
        let entry = FileEntry::from_path_and_content(
            PathBuf::from("/some/prompts/file.txt"),
            "content".to_string(),
            FileSource::Local,
        );
        assert_eq!(entry.name, "file");

        // Also verify the raw helper directly for a bare unrecognized extension.
        let stem = FileEntry::remove_compound_extensions(Path::new("/other/notes.toml"));
        assert_eq!(stem, "notes");
    }

    #[test]
    fn test_get_source_existing_and_missing() {
        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        vfs.add_builtin("present", "hello");

        // Existing file returns the correct source.
        let source = vfs.get_source("present");
        assert!(source.is_some());
        assert_eq!(*source.unwrap(), FileSource::Builtin);

        // Missing file returns None.
        assert!(vfs.get_source("absent").is_none());
    }

    #[test]
    fn test_list_returns_correct_count() {
        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        assert_eq!(vfs.list().len(), 0, "empty VFS should list zero files");

        vfs.add_builtin("alpha", "a");
        assert_eq!(vfs.list().len(), 1);

        vfs.add_builtin("beta", "b");
        vfs.add_builtin("gamma", "c");
        assert_eq!(vfs.list().len(), 3);

        // Overwriting an existing name should not increase the count.
        let override_entry = FileEntry::new(
            "alpha",
            PathBuf::from("builtin:/prompts/alpha"),
            "a-override".to_string(),
            FileSource::User,
        );
        vfs.add_file(override_entry);
        assert_eq!(
            vfs.list().len(),
            3,
            "overriding a file should not change the count"
        );
    }

    #[test]
    fn test_load_files_from_dir_compound_extensions() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().join("templates");
        fs::create_dir_all(&dir).unwrap();

        // Create files with compound extensions
        fs::write(dir.join("greeting.md.liquid"), "Hello {{ name }}").unwrap();
        fs::write(dir.join("farewell.markdown.liquid"), "Goodbye {{ name }}").unwrap();
        fs::write(dir.join("mixed.liquid.md"), "Mixed {{ content }}").unwrap();

        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        vfs.load_files_from_dir(&dir, FileSource::Local).unwrap();

        // All three compound-extension files should be loaded
        let files = vfs.list();
        assert_eq!(
            files.len(),
            3,
            "all compound-extension files should be loaded"
        );

        // Verify each file was loaded with correct content
        let greeting = vfs.get("greeting").unwrap();
        assert_eq!(greeting.content, "Hello {{ name }}");

        let farewell = vfs.get("farewell").unwrap();
        assert_eq!(farewell.content, "Goodbye {{ name }}");

        let mixed = vfs.get("mixed").unwrap();
        assert_eq!(mixed.content, "Mixed {{ content }}");
    }

    #[test]
    fn test_load_files_from_dir_supported_single_extensions() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().join("configs");
        fs::create_dir_all(&dir).unwrap();

        // Create files with each supported single extension
        fs::write(dir.join("config.yaml"), "key: value").unwrap();
        fs::write(dir.join("settings.yml"), "setting: true").unwrap();
        fs::write(dir.join("diagram.mermaid"), "graph LR; A-->B").unwrap();
        fs::write(dir.join("readme.md"), "# Title").unwrap();
        fs::write(dir.join("notes.markdown"), "## Notes").unwrap();

        // Also create an unsupported file that should be skipped
        fs::write(dir.join("script.sh"), "#!/bin/bash").unwrap();
        fs::write(dir.join("data.json"), "{}").unwrap();
        fs::write(dir.join("no_extension"), "plain text").unwrap();

        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        vfs.load_files_from_dir(&dir, FileSource::Local).unwrap();

        // Only the 5 supported extensions should be loaded
        let files = vfs.list();
        assert_eq!(files.len(), 5, "only supported extensions should be loaded");

        // Verify each supported file is present
        assert!(vfs.get("config").is_some(), "yaml file should be loaded");
        assert!(vfs.get("settings").is_some(), "yml file should be loaded");
        assert!(
            vfs.get("diagram").is_some(),
            "mermaid file should be loaded"
        );
        assert!(vfs.get("readme").is_some(), "md file should be loaded");
        assert!(vfs.get("notes").is_some(), "markdown file should be loaded");

        // Verify unsupported files are not loaded
        assert!(vfs.get("script").is_none(), "sh file should not be loaded");
        assert!(vfs.get("data").is_none(), "json file should not be loaded");
        assert!(
            vfs.get("no_extension").is_none(),
            "extensionless file should not be loaded"
        );
    }

    #[test]
    fn test_load_files_from_dir_nonexistent_directory() {
        let temp_dir = TempDir::new().unwrap();
        let missing = temp_dir.path().join("does_not_exist");

        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        // Should return Ok(()) without error for missing directories
        let result = vfs.load_files_from_dir(&missing, FileSource::Local);
        assert!(result.is_ok());
        assert!(vfs.list().is_empty());
    }

    #[test]
    fn test_load_files_from_dir_skips_oversized_files() {
        // We test the MAX_FILE_SIZE guard by creating a sparse file just over the limit.
        // On most filesystems this is fast because sparse files don't allocate all blocks.
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().join("big");
        fs::create_dir_all(&dir).unwrap();

        let big_file = dir.join("huge.md");
        let f = fs::File::create(&big_file).unwrap();
        // Set the file length to just over MAX_FILE_SIZE (10MB + 1 byte)
        f.set_len(MAX_FILE_SIZE + 1).unwrap();

        // Also add a normal-sized file to verify it still gets loaded
        fs::write(dir.join("small.md"), "small content").unwrap();

        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        vfs.load_files_from_dir(&dir, FileSource::Local).unwrap();

        // The oversized file should be skipped, but the small one loaded
        assert!(
            vfs.get("huge").is_none(),
            "oversized file should be skipped"
        );
        assert!(vfs.get("small").is_some(), "normal file should be loaded");
    }

    /// Default managed mode: load_all on a fresh VFS with no search paths and
    /// no dot-directory flag. Both XDG data and git root discovery may fail
    /// gracefully (e.g., in a temp dir with no git repo). Verify it returns Ok.
    #[test]
    fn test_load_all_default_managed_mode() {
        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        // A fresh VFS has no search paths and use_dot_dirs is false,
        // so load_all takes the default managed branch. Even if XDG or
        // git root discovery fails, it should succeed gracefully.
        let result = vfs.load_all();
        assert!(
            result.is_ok(),
            "load_all in default managed mode should not error"
        );
    }

    /// Default managed mode with controlled XDG_DATA_HOME: create a temp
    /// directory tree that mimics the XDG data layout so load_all actually
    /// finds and loads user-level files through the managed path.
    #[test]
    #[serial]
    fn test_load_all_managed_mode_with_xdg_data() {
        let temp_dir = TempDir::new().unwrap();

        // Build: $XDG_DATA_HOME/sah/prompts/greeting.md
        let xdg_data = temp_dir.path().join("xdg_data");
        let prompts_dir = xdg_data.join("sah").join("prompts");
        fs::create_dir_all(&prompts_dir).unwrap();
        fs::write(prompts_dir.join("greeting.md"), "hello from xdg").unwrap();

        let original = std::env::var("XDG_DATA_HOME").ok();
        std::env::set_var("XDG_DATA_HOME", &xdg_data);

        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        let result = vfs.load_all();

        // Restore env before assertions so panics don't leave it dirty.
        match original {
            Some(v) => std::env::set_var("XDG_DATA_HOME", v),
            None => std::env::remove_var("XDG_DATA_HOME"),
        }

        assert!(result.is_ok(), "load_all managed mode should succeed");
        let file = vfs.get("greeting");
        assert!(file.is_some(), "should load greeting from XDG data dir");
        assert_eq!(file.unwrap().content, "hello from xdg");
        assert_eq!(file.unwrap().source, FileSource::User);
    }

    /// Search paths mode via load_all: configure multiple search paths with
    /// different sources and verify load_all dispatches through the search
    /// path branch, loading files with correct precedence.
    #[test]
    fn test_load_all_search_paths_mode() {
        let temp_dir = TempDir::new().unwrap();

        // User-level search path
        let user_dir = temp_dir.path().join("user_prompts");
        fs::create_dir_all(&user_dir).unwrap();
        fs::write(user_dir.join("common.md"), "user common").unwrap();
        fs::write(user_dir.join("user_only.md"), "user exclusive").unwrap();

        // Local-level search path (should override user for same name)
        let local_dir = temp_dir.path().join("local_prompts");
        fs::create_dir_all(&local_dir).unwrap();
        fs::write(local_dir.join("common.md"), "local common").unwrap();
        fs::write(local_dir.join("local_only.md"), "local exclusive").unwrap();

        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        vfs.add_search_path(user_dir, FileSource::User);
        vfs.add_search_path(local_dir, FileSource::Local);
        vfs.load_all().unwrap();

        // Local overrides user for "common"
        let common = vfs.get("common").unwrap();
        assert_eq!(common.content, "local common");
        assert_eq!(common.source, FileSource::Local);

        // Each exclusive file is present
        assert_eq!(vfs.get("user_only").unwrap().content, "user exclusive");
        assert_eq!(vfs.get("local_only").unwrap().content, "local exclusive");

        // Total: common + user_only + local_only = 3
        assert_eq!(vfs.list().len(), 3);
    }

    /// Search paths mode via load_all with an empty / nonexistent directory:
    /// verify it does not error and only loads from paths that exist.
    #[test]
    fn test_load_all_search_paths_missing_dir() {
        let temp_dir = TempDir::new().unwrap();

        let existing = temp_dir.path().join("exists");
        fs::create_dir_all(&existing).unwrap();
        fs::write(existing.join("found.md"), "found content").unwrap();

        let missing = temp_dir.path().join("nope");

        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        vfs.add_search_path(existing, FileSource::User);
        vfs.add_search_path(missing, FileSource::Local);

        let result = vfs.load_all();
        assert!(
            result.is_ok(),
            "load_all should tolerate missing search path dirs"
        );
        assert_eq!(vfs.list().len(), 1);
        assert_eq!(vfs.get("found").unwrap().content, "found content");
    }

    /// Dot-directory mode: call use_dot_directory_paths(), then load_all().
    /// Uses a controlled XDG_DATA_HOME to place user files under
    /// `$XDG_DATA_HOME/sah/prompts/` and verifies they load as User source.
    #[test]
    #[serial]
    fn test_load_all_dot_directory_mode_xdg() {
        let temp_dir = TempDir::new().unwrap();

        // Build: $XDG_DATA_HOME/sah/prompts/dot_user.md
        let xdg_data = temp_dir.path().join("xdg_data");
        let user_prompts = xdg_data.join("sah").join("prompts");
        fs::create_dir_all(&user_prompts).unwrap();
        fs::write(user_prompts.join("dot_user.md"), "dot user content").unwrap();

        let original = std::env::var("XDG_DATA_HOME").ok();
        std::env::set_var("XDG_DATA_HOME", &xdg_data);

        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        vfs.use_dot_directory_paths();
        let result = vfs.load_all();

        match original {
            Some(v) => std::env::set_var("XDG_DATA_HOME", v),
            None => std::env::remove_var("XDG_DATA_HOME"),
        }

        assert!(result.is_ok(), "dot-directory load_all should succeed");
        let file = vfs.get("dot_user");
        assert!(file.is_some(), "should load dot_user from XDG data path");
        assert_eq!(file.unwrap().content, "dot user content");
        assert_eq!(file.unwrap().source, FileSource::User);
    }

    /// Dot-directory mode without any valid XDG or git repo:
    /// load_all should still return Ok gracefully.
    #[test]
    #[serial]
    fn test_load_all_dot_directory_mode_no_xdg() {
        let original_data = std::env::var("XDG_DATA_HOME").ok();
        let original_home = std::env::var("HOME").ok();

        // Point XDG_DATA_HOME to a nonexistent path so xdg_base_dir fails
        // for the "XDG_DATA_HOME" lookup. Also set HOME to a temp dir to
        // avoid the fallback finding real user data.
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("XDG_DATA_HOME", temp_dir.path().join("no_such_xdg"));
        std::env::set_var("HOME", temp_dir.path().join("fake_home"));

        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        vfs.use_dot_directory_paths();
        let result = vfs.load_all();

        // Restore env
        match original_data {
            Some(v) => std::env::set_var("XDG_DATA_HOME", v),
            None => std::env::remove_var("XDG_DATA_HOME"),
        }
        match original_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }

        assert!(
            result.is_ok(),
            "dot-directory mode should not error even without valid dirs"
        );
    }

    // ── get_directories tests ────────────────────────────────────────────

    #[test]
    fn test_get_directories_explicit_paths_filters_missing() {
        // Only directories that exist on disk should be returned when
        // explicit search paths are configured.
        let temp_dir = TempDir::new().unwrap();

        let existing_a = temp_dir.path().join("dir_a");
        let existing_b = temp_dir.path().join("dir_b");
        let missing = temp_dir.path().join("does_not_exist");
        fs::create_dir_all(&existing_a).unwrap();
        fs::create_dir_all(&existing_b).unwrap();

        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        vfs.add_search_path(existing_a.clone(), FileSource::User);
        vfs.add_search_path(missing, FileSource::Local);
        vfs.add_search_path(existing_b.clone(), FileSource::Local);

        let dirs = vfs.get_directories().unwrap();
        assert_eq!(dirs.len(), 2);
        assert!(dirs.contains(&existing_a));
        assert!(dirs.contains(&existing_b));
    }

    #[test]
    #[serial]
    fn test_get_directories_dot_dir_mode_includes_xdg() {
        // With dot-directory mode and a custom XDG_DATA_HOME, get_directories
        // should include the XDG-based directory when it exists on disk.
        let temp_dir = TempDir::new().unwrap();
        let xdg_data = temp_dir.path().join("xdg_data_gd");
        // SwissarmyhammerConfig::XDG_NAME = "sah"
        let user_dir = xdg_data.join("sah").join("prompts");
        fs::create_dir_all(&user_dir).unwrap();

        let original = std::env::var("XDG_DATA_HOME").ok();
        std::env::set_var("XDG_DATA_HOME", &xdg_data);

        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        vfs.use_dot_directory_paths();

        let dirs = vfs.get_directories().unwrap();

        match original {
            Some(v) => std::env::set_var("XDG_DATA_HOME", v),
            None => std::env::remove_var("XDG_DATA_HOME"),
        }

        assert!(
            dirs.contains(&user_dir),
            "expected {user_dir:?} in {dirs:?}"
        );
    }

    #[test]
    #[serial]
    fn test_get_directories_dot_dir_mode_skips_missing_xdg() {
        // If the XDG data subdirectory does not exist on disk, it should not
        // appear in the result.
        let temp_dir = TempDir::new().unwrap();
        let xdg_data = temp_dir.path().join("empty_xdg_gd");
        // Create the base but NOT the sah/prompts subdirectory.
        fs::create_dir_all(&xdg_data).unwrap();

        let original = std::env::var("XDG_DATA_HOME").ok();
        std::env::set_var("XDG_DATA_HOME", &xdg_data);

        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        vfs.use_dot_directory_paths();

        let dirs = vfs.get_directories().unwrap();

        match original {
            Some(v) => std::env::set_var("XDG_DATA_HOME", v),
            None => std::env::remove_var("XDG_DATA_HOME"),
        }

        let absent = xdg_data.join("sah").join("prompts");
        assert!(
            !dirs.contains(&absent),
            "missing directory should not appear in {dirs:?}"
        );
    }

    #[test]
    fn test_get_directories_default_mode_returns_ok() {
        // A fresh VFS with no search paths and no dot-dir mode should return
        // Ok (may be empty depending on the host environment).
        let vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        let result = vfs.get_directories();
        assert!(
            result.is_ok(),
            "get_directories should not error: {result:?}"
        );
    }

    // ── get_search_paths tests ─────────────────────────────────────────

    #[test]
    fn test_get_search_paths_explicit_with_source_metadata() {
        // Explicit search paths should be returned with their FileSource
        // metadata, but only when the directory exists on disk.
        let temp_dir = TempDir::new().unwrap();

        let existing = temp_dir.path().join("user_prompts_sp");
        let missing = temp_dir.path().join("ghost_sp");
        fs::create_dir_all(&existing).unwrap();

        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        vfs.add_search_path(existing.clone(), FileSource::User);
        vfs.add_search_path(missing, FileSource::Local);

        let paths = vfs.get_search_paths();
        assert_eq!(paths.len(), 1, "only existing paths should be returned");
        assert_eq!(paths[0].path, existing);
        assert_eq!(paths[0].source, FileSource::User);
    }

    #[test]
    fn test_get_search_paths_multiple_existing() {
        // When multiple explicit search paths exist, all are returned with
        // their correct source.
        let temp_dir = TempDir::new().unwrap();

        let user_dir = temp_dir.path().join("user_sp");
        let local_dir = temp_dir.path().join("local_sp");
        fs::create_dir_all(&user_dir).unwrap();
        fs::create_dir_all(&local_dir).unwrap();

        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        vfs.add_search_path(user_dir.clone(), FileSource::User);
        vfs.add_search_path(local_dir.clone(), FileSource::Local);

        let paths = vfs.get_search_paths();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].path, user_dir);
        assert_eq!(paths[0].source, FileSource::User);
        assert_eq!(paths[1].path, local_dir);
        assert_eq!(paths[1].source, FileSource::Local);
    }

    #[test]
    #[serial]
    fn test_get_search_paths_dot_dir_mode_xdg_source() {
        // In dot-directory mode, get_search_paths should return the XDG
        // directory with FileSource::User when it exists.
        let temp_dir = TempDir::new().unwrap();
        let xdg_data = temp_dir.path().join("xdg_sp_gsp");
        let user_dir = xdg_data.join("sah").join("prompts");
        fs::create_dir_all(&user_dir).unwrap();

        let original = std::env::var("XDG_DATA_HOME").ok();
        std::env::set_var("XDG_DATA_HOME", &xdg_data);

        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        vfs.use_dot_directory_paths();

        let paths = vfs.get_search_paths();

        match original {
            Some(v) => std::env::set_var("XDG_DATA_HOME", v),
            None => std::env::remove_var("XDG_DATA_HOME"),
        }

        let xdg_entry = paths.iter().find(|sp| sp.path == user_dir);
        assert!(
            xdg_entry.is_some(),
            "expected XDG path {user_dir:?} in search paths"
        );
        assert_eq!(xdg_entry.unwrap().source, FileSource::User);
    }

    #[test]
    fn test_get_search_paths_default_mode_returns_vec() {
        // A fresh VFS should return a (possibly empty) Vec without panicking.
        let vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        let paths = vfs.get_search_paths();
        // Cannot assert exact contents (host-dependent), but it must not panic.
        assert!(
            paths.len() < 100,
            "sanity: should not return absurd number of paths"
        );
    }

    #[test]
    fn test_get_search_paths_empty_when_all_missing() {
        // When all explicit search paths point to non-existent directories,
        // the result should be empty.
        let temp_dir = TempDir::new().unwrap();
        let missing_a = temp_dir.path().join("nope_a_sp");
        let missing_b = temp_dir.path().join("nope_b_sp");

        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        vfs.add_search_path(missing_a, FileSource::User);
        vfs.add_search_path(missing_b, FileSource::Local);

        let paths = vfs.get_search_paths();
        assert!(
            paths.is_empty(),
            "all missing paths should yield empty result"
        );
    }

    /// is_path_safe falls back to string-based check when canonicalize() fails
    /// for either argument (e.g. a non-existent path). Paths containing ".."
    /// or "~" should be rejected; clean paths should be accepted.
    #[test]
    fn test_is_path_safe_fallback_string_check() {
        // Non-existent paths → canonicalize() fails → fallback to string check.
        let safe = Path::new("/nonexistent/subdir/file.md");
        let base = Path::new("/nonexistent/subdir");
        assert!(
            VirtualFileSystem::<SwissarmyhammerConfig>::is_path_safe(safe, base),
            "clean path should be safe in fallback mode"
        );

        // Path with ".." should be rejected.
        let traversal = Path::new("/nonexistent/subdir/../../../etc/passwd");
        assert!(
            !VirtualFileSystem::<SwissarmyhammerConfig>::is_path_safe(traversal, base),
            "path with '..' should be rejected in fallback mode"
        );

        // Path with "~" should be rejected.
        let home = Path::new("/nonexistent/~/secret");
        assert!(
            !VirtualFileSystem::<SwissarmyhammerConfig>::is_path_safe(home, base),
            "path with '~' should be rejected in fallback mode"
        );
    }

    /// load_files_from_dir should skip files that fail the path-safety check
    /// (e.g. a symlink that points outside the base directory).
    #[cfg(unix)]
    #[test]
    fn test_load_files_from_dir_skips_symlink_outside_base() {
        use std::os::unix::fs::symlink;

        let temp_dir = tempfile::TempDir::new().unwrap();

        // Create a "secret" file outside the base directory.
        let outside = temp_dir.path().join("secret.md");
        fs::write(&outside, "secret content").unwrap();

        // Create the base directory.
        let base = temp_dir.path().join("base");
        fs::create_dir_all(&base).unwrap();

        // Place a normal file so we can verify good files still load.
        fs::write(base.join("good.md"), "good content").unwrap();

        // Create a symlink inside base that points to the outside file.
        symlink(&outside, base.join("evil.md")).unwrap();

        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        vfs.load_files_from_dir(&base, FileSource::Local).unwrap();

        // The normal file should load.
        assert!(
            vfs.get("good").is_some(),
            "normal file inside base should be loaded"
        );
        // The symlink pointing outside should be skipped.
        assert!(
            vfs.get("evil").is_none(),
            "symlink pointing outside base should be skipped"
        );
    }

    /// from_path_and_content derives the correct name from a file that lives
    /// directly under a known subdirectory (no intermediate subdirectories).
    /// This exercises the `else { stem.to_string() }` branch in
    /// extract_name_from_path when components is empty after the pop.
    #[test]
    fn test_from_path_and_content_flat_under_subdir() {
        // Path is directly under the known subdir (.prompts), no subdirectory prefix.
        let entry = FileEntry::from_path_and_content(
            PathBuf::from("/home/user/.prompts/flat.md"),
            "flat content".to_string(),
            FileSource::User,
        );
        assert_eq!(
            entry.name, "flat",
            "file directly under known subdir should use bare stem"
        );
        assert_eq!(entry.content, "flat content");
        assert_eq!(entry.source, FileSource::User);
    }

    /// from_path_and_content with a path that has no known subdirectory component
    /// falls through to the else branch returning just the stem.
    #[test]
    fn test_from_path_and_content_no_known_subdir() {
        let entry = FileEntry::from_path_and_content(
            PathBuf::from("/arbitrary/path/myfile.md"),
            "content".to_string(),
            FileSource::Builtin,
        );
        assert_eq!(
            entry.name, "myfile",
            "path with no known subdir should use bare stem"
        );
    }

    /// Stacking: multiple versions of the same file loaded via separate
    /// add_search_path calls are all preserved in the stack, and get() returns
    /// the last one added (highest precedence).
    #[test]
    fn test_stacking_multiple_search_paths() {
        let temp_dir = TempDir::new().unwrap();

        let base_dir = temp_dir.path().join("base");
        let user_dir = temp_dir.path().join("user");
        let local_dir = temp_dir.path().join("local");
        fs::create_dir_all(&base_dir).unwrap();
        fs::create_dir_all(&user_dir).unwrap();
        fs::create_dir_all(&local_dir).unwrap();

        fs::write(base_dir.join("config.yaml"), "level: base").unwrap();
        fs::write(user_dir.join("config.yaml"), "level: user").unwrap();
        fs::write(local_dir.join("config.yaml"), "level: local").unwrap();

        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        vfs.add_search_path(base_dir, FileSource::Builtin);
        vfs.add_search_path(user_dir, FileSource::User);
        vfs.add_search_path(local_dir, FileSource::Local);
        vfs.load_all().unwrap();

        // get() returns the highest-precedence (last loaded) version.
        let winner = vfs.get("config").unwrap();
        assert_eq!(winner.content, "level: local");
        assert_eq!(winner.source, FileSource::Local);

        // get_stack() returns all three versions in load order.
        let stack = vfs.get_stack("config").unwrap();
        assert_eq!(stack.len(), 3, "stack should contain all three versions");
        assert_eq!(stack[0].source, FileSource::Builtin);
        assert_eq!(stack[1].source, FileSource::User);
        assert_eq!(stack[2].source, FileSource::Local);
    }

    /// Dot-directory mode: load_all should discover files placed in the
    /// dot-directory (e.g., .prompts) relative to a custom working directory.
    /// This exercises the git-root branch of load_dot_directory_files since
    /// tests run inside the git repository.
    #[test]
    #[serial]
    fn test_dot_directory_loads_from_git_root_dot_dir() {
        // Point XDG_DATA_HOME to a temp dir so no user-level files interfere.
        let temp_dir = TempDir::new().unwrap();
        let empty_xdg = temp_dir.path().join("empty_xdg");
        fs::create_dir_all(&empty_xdg).unwrap();

        // The test suite runs inside the git repository root. Place a
        // .prompts directory at the git root and a test file inside it.
        // We use find_git_repository_root to know where to create the dir.
        let git_root = crate::directory::find_git_repository_root();

        if git_root.is_none() {
            // If not in a git repo, skip this test gracefully.
            return;
        }

        let git_root = git_root.unwrap();
        let dot_prompts = git_root.join(".prompts");
        let test_file = dot_prompts.join("_test_dot_dir_file.md");

        // Clean up in case a previous test run left the file.
        let _ = fs::remove_file(&test_file);

        // Create the directory and file.
        fs::create_dir_all(&dot_prompts).unwrap();
        fs::write(&test_file, "dot dir content").unwrap();

        let old_xdg = std::env::var("XDG_DATA_HOME").ok();
        std::env::set_var("XDG_DATA_HOME", &empty_xdg);

        let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
        vfs.use_dot_directory_paths();
        let result = vfs.load_all();

        // Clean up before assertions so panics don't leave trash.
        let _ = fs::remove_file(&test_file);
        match old_xdg {
            Some(v) => std::env::set_var("XDG_DATA_HOME", v),
            None => std::env::remove_var("XDG_DATA_HOME"),
        }

        assert!(result.is_ok(), "dot-directory load_all should succeed");
        // The test file should have been loaded from .prompts.
        let file = vfs.get("_test_dot_dir_file");
        assert!(
            file.is_some(),
            "should load file from git-root dot-directory .prompts"
        );
        assert_eq!(file.unwrap().source, FileSource::Local);
    }
}
