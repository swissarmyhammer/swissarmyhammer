//! Unit tests for the tool registry, grouped by concern.
//!
//! - `fixtures`: tool doubles shared by several test modules.
//! - `registry_basics`: registration, lookup, execution, categories.
//! - `disabled_tools`: enable and disable behaviour and tool listing order.
//! - `tool_context`: `ToolContext` builders, `call_tool` and MCP logging.
//! - `cli`: CLI category, name, about text and hidden-tool behaviour.
//! - `validation`: tool validation, schema validation and error types.

mod cli;
mod disabled_tools;
mod fixtures;
mod registry_basics;
mod tool_context;
mod validation;
