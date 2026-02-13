//! MCP Tools Registry Module
//!
//! This module organizes MCP tools using the modular registry pattern.
//! Each tool category has its own submodule with dedicated implementations.
//!
//! ## Architecture Overview
//!
//! The tool registry pattern provides a clean, modular approach to organizing MCP tools:
//!
//! ### Tool Structure
//! Each tool follows a consistent pattern:
//! - Individual module directory (e.g., `issues/create/`)
//! - `mod.rs` containing the tool implementation with `McpTool` trait
//! - `description.md` containing comprehensive tool documentation
//! - Registration function that adds tools to the global registry
//!
//! ### Registration Workflow
//! 1. Tools are organized by category (issues, memoranda, etc.)
//! 2. Each category module exports a `register_*_tools(registry)` function
//! 3. The main `tool_registry.rs` calls these registration functions
//! 4. Tools are stored in a centralized `ToolRegistry` for MCP operations
//!
//! ### MCP Integration
//! Tools implement the `McpTool` trait which provides:
//! - `name()`: Unique tool identifier for MCP protocol
//! - `description()`: Human-readable documentation from `description.md`
//! - `schema()`: JSON schema defining input parameters
//! - `execute()`: Async implementation handling tool execution
//!
//! ## Architectural Benefits
//!
//! - **Modularity**: Each tool is self-contained with its own module
//! - **Consistency**: All tools follow the same implementation pattern
//! - **Maintainability**: Easy to add, modify, or remove individual tools
//! - **Documentation**: Comprehensive descriptions co-located with implementation
//! - **Type Safety**: Strong typing through schema validation and Rust's type system

pub mod files;
pub mod flow;
pub mod git;
pub mod js;
pub mod kanban;
pub mod questions;
pub mod shell;
pub mod treesitter;
pub mod web;
pub mod web_fetch;
pub mod web_search;
