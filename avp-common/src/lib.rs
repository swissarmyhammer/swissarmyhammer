//! AVP Common - Agent Validator Protocol core library.
//!
//! This crate provides the core types, chain of responsibility pattern,
//! and hook dispatching logic for the Agent Validator Protocol.
//!
//! Includes code-quality validators that run on PostToolUse (Edit/Write)
//! and code-duplication validator that runs on the Stop hook.

pub mod builtin;
pub mod chain;
pub mod context;
pub mod error;
pub mod hooks;
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
