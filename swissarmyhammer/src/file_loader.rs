//! Virtual file system for loading files from .swissarmyhammer directories
//!
//! This module provides a unified way to load files from the hierarchical
//! .swissarmyhammer directory structure, handling precedence and overrides.
//!
//! # Error Handling
//!
//! The module follows these error handling principles:
//!
//! - **File loading errors**: Individual file loading failures are logged but don't
//!   stop the loading process. This ensures that one corrupt file doesn't prevent
//!   loading other valid files.
//!
//! - **Directory access errors**: If a directory doesn't exist or can't be accessed,
//!   the error is silently ignored and loading continues with other directories.
//!
//! - **Security violations**: Files that fail security checks (path traversal,
//!   file size limits) are logged and skipped, but don't cause the overall
//!   operation to fail.
//!
//! - **Critical errors**: Only errors that prevent the entire operation from
//!   functioning (like current directory access) are propagated up.
//!
//! All skipped files and errors are logged using the `tracing` framework at
//! appropriate levels (warn for security issues, debug for missing directories).

use crate::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use swissarmyhammer_common::utils::find_swissarmyhammer_directory;
use walkdir::WalkDir;

/// Maximum file size to load (10MB)
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;


/// Source of a file (builtin, user, local, or dynamic)
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub enum FileSource {
    /// Builtin files embedded in the binary
    Builtin,
    /// User files from ~/.swissarmyhammer
    User,
    /// Local files from .swissarmyhammer directories
    Local,
    /// Dynamically generated files
    Dynamic,
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

/// Represents a file with its metadata
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// The logical name of the file (without extension)
    pub name: String,
    /// The full path to the file
    pub path: PathBuf,
    /// The file content
    pub content: String,
    /// Where this file came from
    pub source: FileSource,
}

impl FileEntry {
    /// Create a new FileEntry with explicit name
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

    /// Create a FileEntry from path and content, deriving name from the path
    pub fn from_path_and_content(path: PathBuf, content: String, source: FileSource) -> Self {
        // Extract the name from the path
        // For a path like /path/to/prompts/category/subcategory/test.md
        // We want to extract "category/subcategory/test"

        // Get the stem by removing compound extensions
        let stem = Self::remove_compound_extensions(&path);

        // Extract name using proper path operations
        let name = Self::extract_name_from_path(&path, stem);

        Self {
            name,
            path,
            content,
            source,
        }
    }

    /// Remove compound extensions from a filename
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

        // Check for compound extensions first
        for ext in &extensions {
            if let Some(stem) = filename.strip_suffix(ext) {
                return stem;
            }
        }

        // Fallback to file_stem behavior
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
    }

    /// Extract the name from a path using proper path operations
    fn extract_name_from_path(path: &Path, stem: &str) -> String {
        let mut components: Vec<String> = Vec::new();
        let mut found_subdirectory = false;

        // Convert path to components and find prompts or workflows directory
        for component in path.components() {
            if let std::path::Component::Normal(os_str) = component {
                if let Some(s) = os_str.to_str() {
                    if found_subdirectory {
                        // Collect all components after prompts or workflows directory
                        components.push(s.to_string());
                    } else if s == "prompts" || s == "workflows" {
                        found_subdirectory = true;
                    }
                }
            }
        }

        if found_subdirectory && !components.is_empty() {
            // Remove the last component (filename with extension) and use stem instead
            components.pop();
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

/// Virtual file system that manages files from multiple sources
///
/// The VirtualFileSystem provides a unified interface for loading and managing
/// files from different sources (builtin, user, local, dynamic) with proper
/// precedence handling. Files are loaded from the hierarchical .swissarmyhammer
/// directory structure.
///
/// # Example
///
/// ```no_run
/// use swissarmyhammer::file_loader::{VirtualFileSystem, FileSource};
///
/// let mut vfs = VirtualFileSystem::new("prompts");
///
/// // Add a builtin file
/// vfs.add_builtin("example", "This is a builtin prompt");
///
/// // Load all files following standard precedence
/// vfs.load_all().unwrap();
///
/// // Get a file by name
/// if let Some(file) = vfs.get("example") {
///     println!("Content: {}", file.content);
///     println!("Source: {:?}", file.source);
/// }
///
/// // List all loaded files
/// for file in vfs.list() {
///     println!("File: {} from {:?}", file.name, file.source);
/// }
/// ```
///
/// # Precedence
///
/// Files are loaded with the following precedence (later sources override earlier):
/// 1. Builtin files (embedded in the binary)
/// 2. User files (from ~/.swissarmyhammer)
/// 3. Local files (from .swissarmyhammer directories in parent paths)
/// 4. Dynamic files (programmatically added)
pub struct VirtualFileSystem {
    /// The subdirectory to look for (e.g., "prompts" or "workflows")
    pub subdirectory: String,
    /// Map of file names to file entries
    pub files: HashMap<String, FileEntry>,
    /// Track sources for each file
    pub file_sources: HashMap<String, FileSource>,
}

impl VirtualFileSystem {
    /// Create a new virtual file system for a specific subdirectory
    pub fn new(subdirectory: impl Into<String>) -> Self {
        Self {
            subdirectory: subdirectory.into(),
            files: HashMap::new(),
            file_sources: HashMap::new(),
        }
    }

    /// Add a builtin file
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

    /// Add a file entry
    pub fn add_file(&mut self, entry: FileEntry) {
        self.file_sources
            .insert(entry.name.clone(), entry.source.clone());
        self.files.insert(entry.name.clone(), entry);
    }

    /// Get a file by name
    pub fn get(&self, name: &str) -> Option<&FileEntry> {
        self.files.get(name)
    }

    /// Get the source of a file
    pub fn get_source(&self, name: &str) -> Option<&FileSource> {
        self.file_sources.get(name)
    }

    /// List all files
    pub fn list(&self) -> Vec<&FileEntry> {
        self.files.values().collect()
    }

    /// Load files from a directory
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
                            // Check if any part of the compound extension matches our filter
                            let parts: Vec<&str> =
                                compound_ext.trim_start_matches('.').split('.').collect();
                            if parts.iter().any(|part| ["md", "mermaid"].contains(part)) {
                                return Some(path.to_path_buf());
                            }
                        }
                    }

                    // Fallback to single extension check
                    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                        if ["md", "mermaid"].contains(&ext) {
                            return Some(path.to_path_buf());
                        }
                    }
                }
                None
            });

        for path in file_paths {
            // Check file size before loading
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

                    // Validate path is within expected directory
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

    /// Load all files following the standard precedence
    pub fn load_all(&mut self) -> Result<()> {
        // Load builtin files (least precedence)
        // Note: Builtin files are typically added via add_builtin method

        // Load user files from home directory
        if let Ok(home_str) = std::env::var("HOME") {
            let home = PathBuf::from(home_str);
            let user_dir = home.join(".swissarmyhammer");
            self.load_directory(&user_dir, FileSource::User)?;
        } else if let Some(home) = dirs::home_dir() {
            let user_dir = home.join(".swissarmyhammer");
            self.load_directory(&user_dir, FileSource::User)?;
        }

        // Load local files by walking up the directory tree
        self.load_local_files()?;

        Ok(())
    }

    /// Load local files from the Git repository .swissarmyhammer directory
    fn load_local_files(&mut self) -> Result<()> {
        // Primary approach: Find the Git repository .swissarmyhammer directory
        if let Some(swissarmyhammer_dir) = find_swissarmyhammer_directory() {
            self.load_directory(&swissarmyhammer_dir, FileSource::Local)?;
            return Ok(());
        }

        // Fallback approach: If Git-centric approach fails (e.g., in tests),
        // look for .swissarmyhammer in the current directory
        if let Ok(current_dir) = std::env::current_dir() {
            let current_swissarmyhammer = current_dir.join(".swissarmyhammer");
            if current_swissarmyhammer.exists() && current_swissarmyhammer.is_dir() {
                tracing::debug!(
                    "Using fallback directory detection: {}",
                    current_swissarmyhammer.display()
                );
                self.load_directory(&current_swissarmyhammer, FileSource::Local)?;
            }
        }

        Ok(())
    }

    /// Get all directories that are being monitored
    pub fn get_directories(&self) -> Result<Vec<PathBuf>> {
        let mut directories = Vec::new();

        // User directory
        if let Ok(home_str) = std::env::var("HOME") {
            let home = PathBuf::from(home_str);
            let user_dir = home.join(".swissarmyhammer").join(&self.subdirectory);
            if user_dir.exists() {
                directories.push(user_dir);
            }
        } else if let Some(home) = dirs::home_dir() {
            let user_dir = home.join(".swissarmyhammer").join(&self.subdirectory);
            if user_dir.exists() {
                directories.push(user_dir);
            }
        }

        // Local Git repository directory
        if let Some(swissarmyhammer_dir) = find_swissarmyhammer_directory() {
            let subdir = swissarmyhammer_dir.join(&self.subdirectory);
            if subdir.exists() && subdir.is_dir() {
                directories.push(subdir);
            }
        } else {
            // Fallback: Look for .swissarmyhammer in current directory
            if let Ok(current_dir) = std::env::current_dir() {
                let current_swissarmyhammer = current_dir.join(".swissarmyhammer");
                let subdir = current_swissarmyhammer.join(&self.subdirectory);
                if subdir.exists() && subdir.is_dir() {
                    directories.push(subdir);
                }
            }
        }

        Ok(directories)
    }

    /// Validate that a path is safe and within the expected directory
    fn is_path_safe(path: &Path, base_dir: &Path) -> bool {
        // Try to canonicalize both paths
        match (path.canonicalize(), base_dir.canonicalize()) {
            (Ok(canonical_path), Ok(canonical_base)) => {
                // Ensure the path is within the base directory
                canonical_path.starts_with(&canonical_base)
            }
            _ => {
                // If we can't canonicalize, at least check for suspicious patterns
                let path_str = path.to_string_lossy();
                !path_str.contains("..") && !path_str.contains("~")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_source_display() {
        assert_eq!(FileSource::Builtin.to_string(), "builtin");
        assert_eq!(FileSource::User.to_string(), "user");
        assert_eq!(FileSource::Local.to_string(), "local");
        assert_eq!(FileSource::Dynamic.to_string(), "dynamic");
    }

    #[test]
    fn test_file_source_equality() {
        assert_eq!(FileSource::Builtin, FileSource::Builtin);
        assert_ne!(FileSource::Builtin, FileSource::User);
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
    fn test_file_entry_name_from_path() {
        let entry = FileEntry::from_path_and_content(
            PathBuf::from("/path/to/test.md"),
            "content".to_string(),
            FileSource::User,
        );
        assert_eq!(entry.name, "test");
        assert_eq!(entry.content, "content");
        assert_eq!(entry.source, FileSource::User);
    }

    #[test]
    fn test_file_entry_nested_name() {
        let entry = FileEntry::from_path_and_content(
            PathBuf::from("/path/to/prompts/category/subcategory/test.md"),
            "content".to_string(),
            FileSource::Builtin,
        );
        assert_eq!(entry.name, "category/subcategory/test");
    }

    #[test]
    fn test_virtual_file_system_new() {
        let vfs = VirtualFileSystem::new("prompts");
        assert_eq!(vfs.subdirectory, "prompts");
        assert!(vfs.files.is_empty());
    }

    #[test]
    fn test_virtual_file_system_add_builtin() {
        let mut vfs = VirtualFileSystem::new("prompts");
        vfs.add_builtin("test", "content");

        let file = vfs.get("test").unwrap();
        assert_eq!(file.name, "test");
        assert_eq!(file.content, "content");
        assert_eq!(file.source, FileSource::Builtin);
    }

    #[test]
    fn test_virtual_file_system_load_directory() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let prompts_dir = temp_dir.path().join("prompts");
        fs::create_dir_all(&prompts_dir).unwrap();

        // Create a test file
        let test_file = prompts_dir.join("test.md");
        fs::write(&test_file, "test content").unwrap();

        let mut vfs = VirtualFileSystem::new("prompts");
        vfs.load_directory(temp_dir.path(), FileSource::Local)
            .unwrap();

        let file = vfs.get("test").unwrap();
        assert_eq!(file.name, "test");
        assert_eq!(file.content, "test content");
        assert_eq!(file.source, FileSource::Local);
    }

    #[test]
    fn test_virtual_file_system_precedence() {
        let mut vfs = VirtualFileSystem::new("prompts");

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

        // The user version should have overridden the builtin
        let file = vfs.get("test").unwrap();
        assert_eq!(file.content, "user content");
        assert_eq!(file.source, FileSource::User);
    }

    #[test]
    fn test_virtual_file_system_list() {
        let mut vfs = VirtualFileSystem::new("prompts");

        vfs.add_builtin("test1", "content1");
        vfs.add_builtin("test2", "content2");

        let files = vfs.list();
        assert_eq!(files.len(), 2);

        let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"test1"));
        assert!(names.contains(&"test2"));
    }

    #[test]
    fn test_virtual_file_system_get_source() {
        let mut vfs = VirtualFileSystem::new("prompts");

        vfs.add_builtin("test", "content");
        assert_eq!(vfs.get_source("test"), Some(&FileSource::Builtin));
        assert_eq!(vfs.get_source("nonexistent"), None);
    }
}
