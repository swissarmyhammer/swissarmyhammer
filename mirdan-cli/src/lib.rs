//! Mirdan - Universal skill and validator package manager for AI coding agents.
//!
//! Mirdan manages two package types across all detected AI coding agents:
//!
//! - **Skills** (agentskills.io spec): Deployed to each agent's skill directory
//! - **Validators** (AVP spec): Deployed to .avp/validators/
//!
//! Package type is auto-detected from contents (SKILL.md vs VALIDATOR.md + rules/).

pub mod agents;
pub mod auth;
pub mod banner;
mod cli;
pub use cli::{Cli, Commands, NewKind};
pub mod doctor;
pub mod git_source;
pub mod info;
pub mod install;
pub mod list;
pub mod lockfile;
pub mod mcp_config;
pub mod new;
pub mod outdated;
pub mod package_type;
pub mod publish;
pub mod registry;
pub mod search;
pub mod store;
pub mod sync;
pub mod table;
