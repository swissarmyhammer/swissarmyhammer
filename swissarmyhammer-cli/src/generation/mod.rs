//! # CLI Generation System
//!
//! This module provides the foundational infrastructure for automatically generating
//! CLI commands from MCP tool definitions, respecting CLI exclusion markers and
//! providing flexible configuration options.
//!
//! ## Overview
//!
//! The CLI generation system bridges the gap between MCP tools and CLI commands by:
//! - Parsing MCP tool JSON schemas into CLI argument structures
//! - Respecting CLI exclusion markers from the tool registry
//! - Providing configurable naming strategies and command organization
//! - Creating structured representations suitable for CLI framework integration
//!
//! ## Key Components
//!
//! - [`CliGenerator`]: Main generator that orchestrates the CLI generation process
//! - [`CommandBuilder`]: Schema parser that converts JSON Schema to CLI structures
//! - [`GeneratedCommand`]: Structured representation of a generated CLI command
//! - [`GenerationConfig`]: Configuration system for customizing generation behavior
//!
//! ## Usage Example
//!
//! ```rust,no_run
//! use swissarmyhammer_cli::generation::{CliGenerator, GenerationConfig, NamingStrategy};
//! use swissarmyhammer_tools::ToolRegistry;
//! use std::sync::Arc;
//!
//! // Create a registry with tools
//! let mut registry = ToolRegistry::new();
//! // ... register tools ...
//!
//! // Create generator with custom config
//! let config = GenerationConfig {
//!     naming_strategy: NamingStrategy::GroupByDomain,
//!     use_subcommands: true,
//!     ..Default::default()
//! };
//!
//! let generator = CliGenerator::new(Arc::new(registry)).with_config(config);
//!
//! // Generate CLI commands
//! let commands = generator.generate_commands().unwrap();
//! for command in commands {
//!     println!("Generated command: {}", command.name);
//! }
//! ```
//!
//! ## Integration
//!
//! This module is designed to integrate with existing CLI frameworks like clap.
//! The generated command structures can be converted to framework-specific
//! representations for actual CLI implementation.

pub mod cli_generator;
pub mod command_builder;
pub mod types;

// Re-export key types for convenience
pub use cli_generator::CliGenerator;
pub use command_builder::CommandBuilder;
pub use types::{
    GeneratedCommand, CliArgument, CliOption, GenerationConfig, GenerationError, NamingStrategy, ParseError
};