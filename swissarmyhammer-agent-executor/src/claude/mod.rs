//! Claude Code CLI executor module
//!
//! This module provides an executor that shells out to the Claude Code CLI
//! for executing prompts using Claude AI.

pub mod executor;

pub use executor::ClaudeCodeExecutor;
