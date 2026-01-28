//! Generic directory stacking for configuration directories.
//!
//! This crate provides a reusable pattern for loading files from multiple directories
//! with precedence: builtin → user home → project root. It supports different
//! directory configurations (e.g., `.swissarmyhammer`, `.avp`) through the
//! `DirectoryConfig` trait.
//!
//! # Overview
//!
//! The crate provides three main components:
//!
//! - [`DirectoryConfig`] - A trait for defining directory configurations
//! - [`ManagedDirectory`] - A struct for managing directories at different locations
//! - [`VirtualFileSystem`] - A struct for loading files with precedence handling
//!
//! # Example
//!
//! ```no_run
//! use swissarmyhammer_directory::{
//!     DirectoryConfig, ManagedDirectory, VirtualFileSystem,
//!     SwissarmyhammerConfig, FileSource,
//! };
//!
//! // Use the built-in SwissarmyhammerConfig
//! let dir = ManagedDirectory::<SwissarmyhammerConfig>::from_git_root()?;
//! println!("Directory at: {}", dir.root().display());
//!
//! // Load files with precedence
//! let mut vfs = VirtualFileSystem::<SwissarmyhammerConfig>::new("prompts");
//! vfs.add_builtin("default-prompt", "# Default prompt content");
//! vfs.load_all()?;
//!
//! for file in vfs.list() {
//!     println!("{}: {} ({})", file.name, file.source, file.path.display());
//! }
//! # Ok::<(), swissarmyhammer_directory::DirectoryError>(())
//! ```
//!
//! # Custom Configuration
//!
//! You can define your own directory configuration by implementing `DirectoryConfig`:
//!
//! ```rust
//! use swissarmyhammer_directory::{DirectoryConfig, ManagedDirectory};
//!
//! pub struct MyToolConfig;
//!
//! impl DirectoryConfig for MyToolConfig {
//!     const DIR_NAME: &'static str = ".mytool";
//!     const GITIGNORE_CONTENT: &'static str = "*.log\ntmp/\n";
//!
//!     fn init_subdirs() -> &'static [&'static str] {
//!         &["cache"]
//!     }
//! }
//!
//! // Now use it
//! // let dir = ManagedDirectory::<MyToolConfig>::from_git_root()?;
//! ```

mod config;
mod directory;
mod error;
mod file_loader;
mod yaml_expander;

// Re-export main types
pub use config::{AvpConfig, DirectoryConfig, SwissarmyhammerConfig};
pub use directory::{find_git_repository_root, DirectoryRootType, ManagedDirectory};
pub use error::{DirectoryError, Result};
pub use file_loader::{FileEntry, FileSource, VirtualFileSystem};
pub use yaml_expander::YamlExpander;
