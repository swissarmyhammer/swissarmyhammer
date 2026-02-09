//! AVP Common - Agent Validator Protocol core library.
//!
//! This crate provides the core types, chain of responsibility pattern,
//! and hook dispatching logic for the Agent Validator Protocol.
//!
//! Frontmatter in VALIDATOR.md and rule files supports Liquid templating.
//! Use `{{ version }}` to reference the workspace version.
//!
//! Validators can use MCP tools like treesitter for enhanced analysis.

pub mod builtin;
pub mod chain;
pub mod context;
pub mod error;
pub mod hooks;
pub mod lockfile;
pub mod strategy;
pub mod turn;
pub mod types;
pub mod validator;

pub use builtin::load_builtins;
pub use chain::LinkOutput;
pub use context::{AvpContext, Decision, HookEvent};
pub use error::{AvpError, ChainError, ValidationError};
pub use strategy::HookDispatcher;
pub use types::{HookInput, HookOutput, HookType};
pub use validator::{MatchContext, Severity, Validator, ValidatorLoader, ValidatorResult};

/// AVP workspace version, inherited from the workspace Cargo.toml.
///
/// Available in VALIDATOR.md and rule frontmatter as the Liquid variable `{{ version }}`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
