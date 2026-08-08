//! Mirdan - Universal skill and validator package manager for AI coding agents.
//!
//! Mirdan manages four package types across all detected AI coding agents:
//!
//! - **Skills** (agentskills.io spec): Deployed to each agent's skill directory
//! - **Validators** (AVP spec): Deployed to ./.validators/
//! - **Tools** (MCP server definitions): Deployed to .tools/ + agent MCP configs
//! - **Plugins** (Claude Code plugins): Deployed to .claude/plugins/
//!
//! Package type is auto-detected from contents.

/// Agent detection and configuration.
pub mod agents;
/// Authentication against the package registry.
pub mod auth;
/// Branded ASCII banner for the Mirdan CLI.
pub mod banner;
/// Builtin validator assets embedded in the binary for the profile installer.
pub mod builtin_validators;
mod cli;
/// The clap command tree and its argument types.
pub use cli::{Cli, Commands, NewKind};
/// Shell completion script generation for the `mirdan` binary.
pub mod completions;
/// Structured result types for deploy/uninstall operations.
pub mod deploy_result;
/// The deploy/uninstall result type and the action it reports.
pub use deploy_result::{DeployAction, DeployResult};
mod dispatch;
/// Command dispatch shared by the CLI and Tauri app binaries, and the
/// human-readable rendering of the results it returns.
pub use dispatch::{dispatch, format_deploy_results};
/// Diagnostic checks for Mirdan setup and configuration.
pub mod doctor;
/// Shared YAML frontmatter reading for the package manifests mirdan reads.
mod frontmatter;
/// Git-based package installation support.
pub mod git_source;
/// Detailed information about a single package.
pub mod info;
/// Type-aware package deployment: install and uninstall.
pub mod install;
/// Lenient JSONC parsing for user-written configuration files.
pub mod jsonc;
/// Parse JSON that may carry comments and trailing commas.
pub use jsonc::parse_jsonc;
/// Listing of installed packages (skills, validators, tools, plugins).
pub mod list;
/// Lockfile management (`mirdan-lock.json`).
pub mod lockfile;
/// MCP configuration file management for tool deployment.
pub mod mcp_config;
/// Order-preserving, duplicate-free extension of a list.
pub mod merge;
/// Scaffolding for a new skill, validator, tool, or plugin package.
pub mod new;
/// Checking for and applying package updates.
pub mod outdated;
/// Package type detection and shared name validation.
pub mod package_type;
/// Publishing and unpublishing packages.
pub mod publish;
/// Client and types for the Mirdan package registry.
pub mod registry;
/// Shipped-content snapshot of the retired builtin validator sets (the nine
/// single-rule sets merged into `code-security` and `code-hygiene`). Used by
/// the refresh-prune mechanism in [`install`] to remove a retired set from a
/// deployed store only when the user never modified it.
pub mod retired_validators;
/// Registry search for skills and validators.
pub mod search;
/// Generic JSON settings-file primitives shared by install components.
pub mod settings;
/// Agent-agnostic install-status detection.
pub mod status;
/// Central skill store and symlink management.
pub mod store;
/// Per-agent configuration strategies.
pub mod strategy;
/// Reconciliation of `.skills/` with agent directories, and lockfile
/// verification.
pub mod sync;
/// Terminal-aware table utilities.
pub mod table;
/// The shared install lifecycle for single-server tool CLIs.
pub mod tool_install;

/// Public test-support helpers for driving the profile installer in a hermetic
/// environment. Compiled only when the `test-support` feature is enabled.
#[cfg(feature = "test-support")]
pub mod test_support;
