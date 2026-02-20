//! CLI definition for the Mirdan command-line interface.
//!
//! This module is self-contained -- it only depends on `clap` and `std` so that
//! `build.rs` can compile it independently via `#[path = "src/cli.rs"]` to
//! generate documentation, man pages, and shell completions at build time.

use clap::{Parser, Subcommand};

/// Mirdan - Universal skill and validator package manager for AI coding agents.
///
/// Manages skills (agentskills.io spec) and validators (AVP spec) across
/// all detected AI coding agents. Skills are deployed to each agent's skill
/// directory; validators are deployed to .avp/validators/.
///
/// Registry URL defaults to https://registry.agentvalidatorprotocol.com.
/// Override with MIRDAN_REGISTRY_URL env var or ~/.mirdan/config.yaml for
/// local testing.
#[derive(Parser, Debug)]
#[command(name = "mirdan")]
#[command(version)]
#[command(arg_required_else_help = true)]
#[command(about = "Universal skill and validator package manager for AI coding agents")]
#[command(
    long_about = "Mirdan manages skills (agentskills.io spec) and validators (AVP spec) across \
    all detected AI coding agents.\n\n\
    Skills are deployed to each agent's skill directory (e.g. .claude/skills/, .cursor/skills/).\n\
    Validators are deployed to .avp/validators/ (project) or ~/.avp/validators/ (global).\n\n\
    Environment variables:\n  \
    MIRDAN_REGISTRY_URL  Override the registry URL (useful for local testing)\n  \
    MIRDAN_TOKEN         Provide an auth token without logging in\n  \
    MIRDAN_CREDENTIALS_PATH  Override the credentials file location\n  \
    MIRDAN_AGENTS_CONFIG     Override the agents configuration file"
)]
pub struct Cli {
    /// Enable debug output to stderr
    #[arg(short, long, global = true)]
    pub debug: bool,

    /// Skip confirmation prompts (useful for CI/CD)
    #[arg(short = 'y', long, global = true)]
    pub yes: bool,

    /// Limit operations to a single agent (e.g. claude-code, cursor)
    #[arg(long, global = true, value_name = "AGENT_ID")]
    pub agent: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Detect and list installed AI coding agents
    Agents {
        /// Show all known agents, not just detected ones
        #[arg(long)]
        all: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Create a new skill or validator from template
    New {
        #[command(subcommand)]
        kind: NewKind,
    },

    /// Install a skill, validator, or MCP server (type auto-detected from contents)
    Install {
        /// Package name, name@version, ./local-path, owner/repo, or git URL
        package: String,
        /// Install globally (~/.avp/validators/ for validators, agent global dirs for skills)
        #[arg(long)]
        global: bool,
        /// Treat package as a git URL (clone instead of registry lookup)
        #[arg(long)]
        git: bool,
        /// Install a specific skill/validator by name from a multi-package repo
        #[arg(long)]
        skill: Option<String>,
        /// Install as an MCP server instead of a skill/validator
        #[arg(long)]
        mcp: bool,
        /// MCP server command (binary to run). Required when --mcp is set.
        #[arg(long, required_if_eq("mcp", "true"))]
        command: Option<String>,
        /// MCP server arguments
        #[arg(long, num_args = 1.., requires = "mcp")]
        args: Vec<String>,
    },

    /// Remove an installed skill or validator package
    Uninstall {
        /// Package name
        name: String,
        /// Remove from global locations
        #[arg(long)]
        global: bool,
    },

    /// List installed skills and validators
    List {
        /// Show only skills
        #[arg(long)]
        skills: bool,
        /// Show only validators
        #[arg(long)]
        validators: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Search the registry for skills and validators
    ///
    /// With a query argument, performs a single search and prints results.
    /// Without a query, enters interactive fuzzy search mode.
    Search {
        /// Search query (omit for interactive mode)
        query: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show detailed information about a package
    Info {
        /// Package name
        name: String,
    },

    /// Authenticate with the registry
    ///
    /// Opens a browser for OAuth login. The registry URL can be overridden
    /// with MIRDAN_REGISTRY_URL for local testing.
    Login,

    /// Log out from the registry and revoke token
    Logout,

    /// Show current authenticated user
    Whoami,

    /// Publish a skill or validator to the registry (type auto-detected)
    ///
    /// Auto-detects package type from directory contents:
    ///   - SKILL.md present -> publishes as a skill
    ///   - VALIDATOR.md + rules/ present -> publishes as a validator
    Publish {
        /// Path or git URL to the package directory to publish
        #[arg(default_value = ".")]
        source: String,
        /// Validate and show what would be published without uploading
        #[arg(long)]
        dry_run: bool,
    },

    /// Remove a published package version from the registry
    Unpublish {
        /// Package name@version (e.g. my-skill@1.0.0)
        name_version: String,
    },

    /// Check for available package updates
    Outdated,

    /// Update installed packages to latest versions
    Update {
        /// Specific package to update (all if omitted)
        name: Option<String>,
        /// Update global packages
        #[arg(long)]
        global: bool,
    },

    /// Reconcile .skills/ with agent directories and verify lockfile
    Sync {
        /// Sync global locations
        #[arg(long)]
        global: bool,
    },

    /// Diagnose Mirdan setup and configuration
    Doctor {
        /// Show detailed output including fix suggestions
        #[arg(short, long)]
        verbose: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum NewKind {
    /// Scaffold a new skill (agentskills.io spec)
    Skill {
        /// Skill name (kebab-case, 1-64 chars)
        name: String,
        /// Create in agent global skill directories instead of project-level
        #[arg(long)]
        global: bool,
    },
    /// Scaffold a new validator (AVP spec)
    Validator {
        /// Validator name (kebab-case, 1-64 chars)
        name: String,
        /// Create in ~/.avp/validators/ instead of .avp/validators/
        #[arg(long)]
        global: bool,
    },
}
