//! Virtual file system for loading files from managed directories.
//!
//! This module provides a unified way to load files from the hierarchical
//! directory structure, handling precedence and overrides.

use crate::config::DirectoryConfig;
use crate::directory::ManagedDirectory;
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

        // Known subdirectory names that mark the start of the logical name
        let known_subdirs = [
            "prompts",
            "workflows",
            "rules",
            "validators",
            "modes",
            "docs",
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

    /// Load files from a directory.
    pub fn load_directory(&mut self, base_path: &Path, source: FileSource) -> Result<()> {
        let target_dir = base_path.join(&self.subdirectory);
        if !target_dir.exists() {
            return Ok(());
        }

        let file_paths = WalkDir::new(&target_dir)
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

                    if !Self::is_path_safe(&path, &target_dir) {
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
    /// This loads files from:
    /// 1. User home directory (lowest precedence after builtins)
    /// 2. Project directory at git root (highest precedence)
    pub fn load_all(&mut self) -> Result<()> {
        // Load user files from home directory
        if let Ok(dir) = ManagedDirectory::<C>::from_user_home() {
            self.load_directory(dir.root(), FileSource::User)?;
        }

        // Load local files from git root
        self.load_local_files()?;

        Ok(())
    }

    /// Load local files from the Git repository directory.
    fn load_local_files(&mut self) -> Result<()> {
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
            (Ok(canonical_path), Ok(canonical_base)) => {
                canonical_path.starts_with(&canonical_base)
            }
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
}
