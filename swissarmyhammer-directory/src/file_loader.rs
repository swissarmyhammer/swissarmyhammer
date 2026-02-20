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
//!   paths like `~/.swissarmyhammer/{subdirectory}` and `{git_root}/.swissarmyhammer/{subdirectory}`.
//!
//! - **Custom search paths mode**: When search paths are configured via
//!   [`VirtualFileSystem::add_search_path`] or [`VirtualFileSystem::use_dot_directory_paths`],
//!   the VFS loads directly from those paths, bypassing `ManagedDirectory`.
//!   This enables patterns like `~/.prompts` and `{git_root}/.prompts`.

use crate::config::DirectoryConfig;
use crate::directory::{find_git_repository_root, ManagedDirectory};
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
    /// User files from home directory (e.g., ~/.swissarmyhammer).
    User,
    /// Local files from project directory (e.g., ./.swissarmyhammer).
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
        // Includes both bare names (for managed directory paths like .swissarmyhammer/prompts)
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
    /// Map of file names to file entries.
    pub files: HashMap<String, FileEntry>,
    /// Track sources for each file.
    pub file_sources: HashMap<String, FileSource>,
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
    pub fn add_file(&mut self, entry: FileEntry) {
        self.file_sources
            .insert(entry.name.clone(), entry.source.clone());
        self.files.insert(entry.name.clone(), entry);
    }

    /// Get a file by name.
    pub fn get(&self, name: &str) -> Option<&FileEntry> {
        self.files.get(name)
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

    /// Configure dot-directory resolution: `~/.{subdirectory}` and `{git_root}/.{subdirectory}`.
    ///
    /// This is a convenience method that tells [`load_all`](Self::load_all) to use
    /// top-level dot-directories instead of subdirectories under a managed directory.
    /// Paths are resolved lazily at load time, so the git root and current directory
    /// are determined when `load_all` is called, not when this method is called.
    ///
    /// For example, with subdirectory "prompts", `load_all` will search:
    /// 1. `~/.prompts` (User source)
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
    ///    Loads from `ManagedDirectory<C>` paths (e.g., `~/.swissarmyhammer/{subdirectory}`).
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
            // Default: use ManagedDirectory-based loading
            if let Ok(dir) = ManagedDirectory::<C>::from_user_home() {
                self.load_directory(dir.root(), FileSource::User)?;
            }
            self.load_local_files_managed()?;
        }

        Ok(())
    }

    /// Load files from dot-directory paths, resolved lazily.
    fn load_dot_directory_files(&mut self) -> Result<()> {
        let dot_name = format!(".{}", self.subdirectory);

        // User home directory
        if let Some(home) = dirs::home_dir() {
            self.load_files_from_dir(&home.join(&dot_name), FileSource::User)?;
        }

        // Git root or current directory fallback
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
            // Resolve dot-directory paths lazily
            let dot_name = format!(".{}", self.subdirectory);
            let mut directories = Vec::new();

            if let Some(home) = dirs::home_dir() {
                let user_dir = home.join(&dot_name);
                if user_dir.exists() && user_dir.is_dir() {
                    directories.push(user_dir);
                }
            }

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

        // Default: use ManagedDirectory-based resolution
        let mut directories = Vec::new();

        // User directory
        if let Ok(dir) = ManagedDirectory::<C>::from_user_home() {
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
            PathBuf::from("/home/user/.swissarmyhammer/prompts/test.md"),
            "user content".to_string(),
            FileSource::User,
        );
        vfs.add_file(entry);

        let file = vfs.get("test").unwrap();
        assert_eq!(file.content, "user content");
        assert_eq!(file.source, FileSource::User);
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
}
