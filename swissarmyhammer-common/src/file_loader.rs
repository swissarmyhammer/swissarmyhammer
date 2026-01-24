//! Virtual file system for loading files from .swissarmyhammer directories
//!
//! This module provides a unified way to load files from the hierarchical
//! .swissarmyhammer directory structure, handling precedence and overrides.
//!
//! This module re-exports types from `swissarmyhammer-directory` with the
//! `SwissarmyhammerConfig` configuration for backward compatibility.
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

// Re-export the shared file loader types
pub use swissarmyhammer_directory::{FileEntry, FileSource, SwissarmyhammerConfig};

/// Type alias for backward compatibility.
///
/// `VirtualFileSystem` is now an alias for the generic
/// `swissarmyhammer_directory::VirtualFileSystem<SwissarmyhammerConfig>`.
///
/// # Example
///
/// ```no_run
/// use swissarmyhammer_common::file_loader::{VirtualFileSystem, FileSource};
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
pub type VirtualFileSystem =
    swissarmyhammer_directory::VirtualFileSystem<SwissarmyhammerConfig>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
        assert_eq!(FileSource::Dynamic.display_emoji(), "📦 Built-in");
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
