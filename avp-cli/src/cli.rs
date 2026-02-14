//! CLI definition for the AVP command-line interface.
//!
//! This module is self-contained — it only depends on `clap` and `std` so that
//! `build.rs` can compile it independently via `#[path = "src/cli.rs"]` to
//! generate documentation, man pages, and shell completions at build time.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// Target location for install/uninstall operations.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum InstallTarget {
    /// Project-level settings (.claude/settings.json)
    Project,
    /// Local project settings, not committed (.claude/settings.local.json)
    Local,
    /// User-level settings (~/.claude/settings.json)
    User,
}

impl std::fmt::Display for InstallTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallTarget::Project => write!(f, "project"),
            InstallTarget::Local => write!(f, "local"),
            InstallTarget::User => write!(f, "user"),
        }
    }
}

/// AVP - Agent Validator Protocol
///
/// Claude Code hook processor that validates tool calls, file changes, and more.
#[derive(Parser, Debug)]
#[command(name = "avp")]
#[command(version)]
#[command(about = "Agent Validator Protocol - Claude Code hook processor")]
pub struct Cli {
    /// Enable debug output to stderr
    #[arg(short, long, global = true)]
    pub debug: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Install AVP hooks into Claude Code settings
    Init {
        /// Where to install the hooks
        #[arg(value_enum, default_value_t = InstallTarget::Project)]
        target: InstallTarget,
    },
    /// Remove AVP hooks from Claude Code settings and delete .avp directory
    Deinit {
        /// Where to remove the hooks from
        #[arg(value_enum, default_value_t = InstallTarget::Project)]
        target: InstallTarget,
    },
    /// Diagnose AVP configuration and setup
    Doctor {
        /// Show detailed output including fix suggestions
        #[arg(short, long)]
        verbose: bool,
    },
    /// List all available validators
    List {
        /// Show detailed output including descriptions
        #[arg(short, long)]
        verbose: bool,
        /// Show only global (user-level) validators
        #[arg(long)]
        global: bool,
        /// Show only local (project-level) validators
        #[arg(long)]
        local: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Authenticate with the AVP registry
    Login,
    /// Log out from the AVP registry
    Logout,
    /// Show current authenticated user
    Whoami,
    /// Search the AVP registry for packages
    Search {
        /// Search query
        query: String,
        /// Filter by tag
        #[arg(long)]
        tag: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show detailed information about a package
    Info {
        /// Package name
        name: String,
    },
    /// Install a package from the registry
    Install {
        /// Package name, optionally with @version (e.g. no-secrets@1.2.3)
        package: String,
        /// Install to project (.avp/validators/) [default]
        #[arg(long, visible_alias = "project")]
        local: bool,
        /// Install globally (~/.avp/validators/)
        #[arg(long, visible_alias = "user")]
        global: bool,
    },
    /// Remove an installed package
    Uninstall {
        /// Package name
        name: String,
        /// Remove from project (.avp/validators/) [default]
        #[arg(long, visible_alias = "project")]
        local: bool,
        /// Remove from global (~/.avp/validators/)
        #[arg(long, visible_alias = "user")]
        global: bool,
    },
    /// Create a new Validator from template
    New {
        /// RuleSet name (kebab-case)
        name: String,
        /// Create in project (.avp/validators/) [default]
        #[arg(long, visible_alias = "project")]
        local: bool,
        /// Create in user-level directory (~/.avp/validators/)
        #[arg(long, visible_alias = "user")]
        global: bool,
    },
    /// Publish a package to the registry
    Publish {
        /// Path to the RuleSet directory to publish
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Validate and show what would be published without uploading
        #[arg(long)]
        dry_run: bool,
    },
    /// Remove a published package version from the registry
    Unpublish {
        /// Package name@version (e.g. no-secrets@1.2.3)
        name_version: String,
    },
    /// Check for available package updates
    Outdated,
    /// Update installed packages to latest versions
    Update {
        /// Specific package to update (all if omitted)
        name: Option<String>,
        /// Update project packages [default]
        #[arg(long, visible_alias = "project")]
        local: bool,
        /// Update global (~/.avp/validators/) packages
        #[arg(long, visible_alias = "user")]
        global: bool,
    },
}
