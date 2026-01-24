//! AVP Common - Agent Validator Protocol core library.
//!
//! This crate provides the core types, chain of responsibility pattern,
//! and hook dispatching logic for the Agent Validator Protocol.

pub mod builtin;
pub mod chain;
pub mod context;
pub mod error;
pub mod hooks;
pub mod strategy;
pub mod types;
pub mod validator;

pub use builtin::load_builtins;
pub use context::{AvpContext, Decision, HookEvent};
pub use error::{AvpError, ChainError, ValidationError};
pub use strategy::HookDispatcher;
pub use types::{HookInput, HookOutput, HookType, LinkOutput};
pub use validator::{MatchContext, Severity, Validator, ValidatorLoader, ValidatorResult};
