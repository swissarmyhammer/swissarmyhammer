//! Configuration trait and implementations for managed directories.
//!
//! This module provides the `DirectoryConfig` trait which defines the configuration
//! for different managed directory types (e.g., `.sah`, `.avp`).

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
///     const XDG_NAME: &'static str = "mytool";
///     const GITIGNORE_CONTENT: &'static str = "# MyTool files\n*.log\n";
///
///     fn init_subdirs() -> &'static [&'static str] {
///         &["cache", "tmp"]
///     }
/// }
/// ```
pub trait DirectoryConfig: Send + Sync {
    /// The directory name (e.g., ".sah" or ".avp").
    ///
    /// This is the name of the directory that will be created at the root location
    /// for git-root and user-home modes.
    const DIR_NAME: &'static str;

    /// The XDG name for this configuration (without leading dot).
    ///
    /// This is the name used in XDG Base Directory paths. For example, with
    /// `XDG_NAME = "sah"`, the XDG config path would be `$XDG_CONFIG_HOME/sah/`.
    const XDG_NAME: &'static str;

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

/// Configuration for `.sah` directories.
///
/// SwissArmyHammer uses this configuration for managing prompts,
/// workflows, and other project configuration.
#[derive(Debug, Clone, Copy)]
pub struct SwissarmyhammerConfig;

impl DirectoryConfig for SwissarmyhammerConfig {
    const DIR_NAME: &'static str = ".sah";
    const XDG_NAME: &'static str = "sah";
    const GITIGNORE_CONTENT: &'static str = r#"# SAH temporary files and logs
# This file is automatically created by swissarmyhammer-directory

# Temporary files
tmp/
*.tmp

# Todo tracking (ephemeral development session tracking)
todo/

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
    const XDG_NAME: &'static str = "avp";
    const GITIGNORE_CONTENT: &'static str = r#"# AVP logs and state
# This file is automatically created by swissarmyhammer-directory

# Log files
*.log
log

# Turn state (ephemeral, per-session)
turn_state/

# Session-scoped sidecar diff files (ephemeral, per-turn)
turn_diffs/

# Pre-execution file snapshots (ephemeral, per-turn)
turn_pre/

# Validator agent recordings (one JSON file per AvpContext lifetime)
recordings/

# Keep validators/ directory (should be committed)
"#;

    fn init_subdirs() -> &'static [&'static str] {
        &[]
    }
}

/// Configuration for `.shell` directories.
///
/// Shell security uses this configuration for managing permit/deny
/// pattern configs at user (`~/.shell/`) and project (`./.shell/`) levels.
#[derive(Debug, Clone, Copy)]
pub struct ShellConfig;

impl DirectoryConfig for ShellConfig {
    const DIR_NAME: &'static str = ".shell";
    const XDG_NAME: &'static str = "shell";
    const GITIGNORE_CONTENT: &'static str = r#"# Shell runtime data
# This file is automatically created by swissarmyhammer-directory

# Ignore everything except this gitignore
*
!.gitignore
"#;

    fn init_subdirs() -> &'static [&'static str] {
        &[]
    }
}

/// Configuration for `.code-context` directories.
///
/// Code context uses this configuration for managing the code index database,
/// LSP server state, and stacked YAML config overlays for stderr filtering.
#[derive(Debug, Clone, Copy)]
pub struct CodeContextConfig;

impl DirectoryConfig for CodeContextConfig {
    const DIR_NAME: &'static str = ".code-context";
    const XDG_NAME: &'static str = "code-context";
    const GITIGNORE_CONTENT: &'static str = r#"# Code context index
# This file is automatically created by swissarmyhammer-directory

# Database and temp files
*.db
*.db-wal
*.db-shm
*.log
"#;

    fn init_subdirs() -> &'static [&'static str] {
        &[]
    }
}

/// Configuration for `.ralph` directories.
///
/// Ralph stores per-session agent loop instructions as ephemeral markdown files.
/// All content is ephemeral — the `.gitignore` ignores everything.
#[derive(Debug, Clone, Copy)]
pub struct RalphConfig;

impl DirectoryConfig for RalphConfig {
    const DIR_NAME: &'static str = ".ralph";
    const XDG_NAME: &'static str = "ralph";
    const GITIGNORE_CONTENT: &'static str = r#"# Ralph session state
# All files are ephemeral per-session instructions
*
!.gitignore
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
        assert_eq!(SwissarmyhammerConfig::DIR_NAME, ".sah");
        assert_eq!(SwissarmyhammerConfig::XDG_NAME, "sah");
        assert!(SwissarmyhammerConfig::GITIGNORE_CONTENT.contains("tmp/"));
        assert_eq!(SwissarmyhammerConfig::init_subdirs(), &["tmp"]);
    }

    #[test]
    fn test_avp_config() {
        assert_eq!(AvpConfig::DIR_NAME, ".avp");
        assert_eq!(AvpConfig::XDG_NAME, "avp");
        assert!(AvpConfig::GITIGNORE_CONTENT.contains("*.log"));
        assert!(AvpConfig::GITIGNORE_CONTENT.contains("turn_state/"));
        assert!(AvpConfig::GITIGNORE_CONTENT.contains("turn_diffs/"));
        assert!(AvpConfig::GITIGNORE_CONTENT.contains("turn_pre/"));
        assert!(!AvpConfig::GITIGNORE_CONTENT.contains("turn_state.yaml"));

        // `log` must match as a standalone token (bare filename), not merely as a
        // substring of `*.log`. Check for the line anchored by a newline on each side.
        assert!(AvpConfig::GITIGNORE_CONTENT.contains("\nlog\n"));
        assert!(AvpConfig::init_subdirs().is_empty());
    }

    #[test]
    fn test_shell_config() {
        assert_eq!(ShellConfig::DIR_NAME, ".shell");
        assert_eq!(ShellConfig::XDG_NAME, "shell");
        assert!(ShellConfig::GITIGNORE_CONTENT.contains("!.gitignore"));
        assert!(ShellConfig::init_subdirs().is_empty());
    }

    #[test]
    fn test_code_context_config() {
        assert_eq!(CodeContextConfig::DIR_NAME, ".code-context");
        assert_eq!(CodeContextConfig::XDG_NAME, "code-context");
        assert!(CodeContextConfig::GITIGNORE_CONTENT.contains("*.db"));
        assert!(CodeContextConfig::GITIGNORE_CONTENT.contains("*.log"));
        assert!(CodeContextConfig::init_subdirs().is_empty());
    }

    #[test]
    fn test_ralph_config() {
        assert_eq!(RalphConfig::DIR_NAME, ".ralph");
        assert_eq!(RalphConfig::XDG_NAME, "ralph");
        assert!(RalphConfig::GITIGNORE_CONTENT.contains("*"));
        assert!(RalphConfig::init_subdirs().is_empty());
    }
}
