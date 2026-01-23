//! AVP Common - Agent Validator Protocol core library.
//!
//! This crate provides the core types, chain of responsibility pattern,
//! and hook dispatching logic for the Agent Validator Protocol.

pub mod chain;
pub mod context;
pub mod error;
pub mod hooks;
pub mod strategy;
pub mod types;

pub use context::{AvpContext, Decision, HookEvent};
pub use error::{AvpError, ChainError, ValidationError};
pub use strategy::HookDispatcher;
pub use types::{HookInput, HookOutput, HookType, LinkOutput};
