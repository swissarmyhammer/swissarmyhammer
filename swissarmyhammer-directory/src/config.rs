//! Configuration trait and implementations for managed directories.
//!
//! This module provides the `DirectoryConfig` trait which defines the configuration
//! for different managed directory types (e.g., `.swissarmyhammer`, `.avp`).

/// Configuration trait for different directory types.
///
/// Implement this trait to define a new managed directory type. The trait provides
/// the directory name, gitignore content, and initialization settings.
///
/// # Example
///
/// ```rust
/// use swissarmyhammer_directory::DirectoryConfig;
///
/// pub struct MyToolConfig;
///
/// impl DirectoryConfig for MyToolConfig {
///     const DIR_NAME: &'static str = ".mytool";
///     const GITIGNORE_CONTENT: &'static str = "# MyTool files\n*.log\n";
///
///     fn init_subdirs() -> &'static [&'static str] {
///         &["cache", "tmp"]
///     }
/// }
/// ```
pub trait DirectoryConfig: Send + Sync {
    /// The directory name (e.g., ".swissarmyhammer" or ".avp").
    ///
    /// This is the name of the directory that will be created at the root location.
    const DIR_NAME: &'static str;

    /// Content for .gitignore file created in the directory.
    ///
    /// This content is written to a `.gitignore` file in the managed directory
    /// to exclude temporary files, logs, and other artifacts from version control.
    const GITIGNORE_CONTENT: &'static str;

    /// Subdirectories to create on initialization.
    ///
    /// These directories will be automatically created when the managed directory
    /// is initialized. Override this method to specify subdirectories.
    ///
    /// Default: No subdirectories.
    fn init_subdirs() -> &'static [&'static str] {
        &[]
    }
}

/// Configuration for `.swissarmyhammer` directories.
///
/// SwissArmyHammer uses this configuration for managing prompts,
/// workflows, and other project configuration.
#[derive(Debug, Clone, Copy)]
pub struct SwissarmyhammerConfig;

impl DirectoryConfig for SwissarmyhammerConfig {
    const DIR_NAME: &'static str = ".swissarmyhammer";
    const GITIGNORE_CONTENT: &'static str = r#"# SwissArmyHammer temporary files and logs
# This file is automatically created by swissarmyhammer-directory

# Temporary files
tmp/
*.tmp

# Todo tracking (ephemeral development session tracking)
todo/

# Abort signals (workflow control files)
.abort

# Logs
*.log
mcp.log

# Workflow execution state
workflow-runs/

# Transcripts (conversation history)
transcripts/

# Question/Answer cache
questions/

# Keep these directories (they should be committed):
# - docs/       Project documentation
"#;

    fn init_subdirs() -> &'static [&'static str] {
        &["tmp"]
    }
}

/// Configuration for `.avp` directories.
///
/// AVP (Agent Validator Protocol) uses this configuration for managing
/// validators and hook logs.
#[derive(Debug, Clone, Copy)]
pub struct AvpConfig;

impl DirectoryConfig for AvpConfig {
    const DIR_NAME: &'static str = ".avp";
    const GITIGNORE_CONTENT: &'static str = r#"# AVP logs and state
# This file is automatically created by swissarmyhammer-directory

# Log files
*.log

# Keep validators/ directory (should be committed)
"#;

    fn init_subdirs() -> &'static [&'static str] {
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swissarmyhammer_config() {
        assert_eq!(SwissarmyhammerConfig::DIR_NAME, ".swissarmyhammer");
        assert!(SwissarmyhammerConfig::GITIGNORE_CONTENT.contains("tmp/"));
        assert_eq!(SwissarmyhammerConfig::init_subdirs(), &["tmp"]);
    }

    #[test]
    fn test_avp_config() {
        assert_eq!(AvpConfig::DIR_NAME, ".avp");
        assert!(AvpConfig::GITIGNORE_CONTENT.contains("*.log"));
        assert!(AvpConfig::init_subdirs().is_empty());
    }
}
