//! Tool registry for MCP operations
//!
//! This module provides a registry pattern for managing MCP tools, replacing
//! the large match statement with a flexible, extensible system.
//!
//! # Architecture Overview
//!
//! The tool registry pattern enables a modular, extensible approach to MCP tool management:
//!
//! 1. **McpTool Trait**: Defines the interface that all tools must implement
//! 2. **ToolRegistry**: Central registry that stores and manages tool instances
//! 3. **ToolContext**: Shared context providing access to storage and services
//! 4. **BaseToolImpl**: Common utility methods for tool implementations
//! 5. **Validation Framework**: Comprehensive schema validation and error handling
//!
//! # Validation and Error Handling Framework
//!
//! The tool registry includes a comprehensive validation system that ensures all MCP tools
//! are compatible with CLI generation and provides robust error handling:
//!
//! ## Schema Validation
//!
//! All tools undergo schema validation to ensure CLI compatibility:
//!
//! - **Supported Types**: string, integer, number, boolean, array
//! - **Unsupported Types**: object, null (with helpful error messages)
//! - **Parameter Names**: Must be valid CLI parameter names (alphanumeric, hyphens, underscores)
//! - **Required Fields**: Validated against actual properties
//! - **CLI Categories**: Tools must have valid categories for CLI integration
//!
//! ## Graceful Degradation
//!
//! The system handles invalid tools gracefully:
//!
//! - **Startup Warnings**: Invalid tools are reported but don't crash the application
//! - **Selective Skipping**: Only problematic tools are excluded from CLI generation
//! - **Detailed Feedback**: Users receive specific guidance on fixing schema issues
//! - **Validation Statistics**: Summary reports show overall health of tool ecosystem
//!
//! ## Error Handling Patterns
//!
//! The framework provides multiple error handling approaches:
//!
//! ```rust,ignore
//! // Strict validation (fails fast)
//! registry.validate_cli_tools()?; // Returns Result<(), Vec<ToolValidationError>>
//!
//! // Graceful validation with warnings
//! let warnings = registry.get_tool_validation_warnings(); // Returns Vec<String>
//! for warning in warnings {
//!     tracing::warn!("{}", warning);
//! }
//!
//! // Validation statistics for monitoring
//! let stats = registry.get_validation_stats(); // Returns CliValidationStats
//! println!("Success rate: {:.1}%", stats.success_rate());
//! ```
//!
//! ## CLI Integration Features
//!
//! Tools are automatically categorized and integrated into dynamic CLI:
//!
//! - **Automatic Categorization**: Tools named `category_action` are grouped by category
//! - **Dynamic Command Generation**: CLI commands generated from tool schemas
//! - **Help Integration**: Tool descriptions become CLI help text
//! - **Validation Flags**: `--validate-tools` provides comprehensive validation reports
//!
//! ## Usage Examples
//!
//! ### Basic Tool Registration with Validation
//!
//! ```rust,ignore
//! use crate::mcp::tool_registry::{ToolRegistry, McpTool, ToolValidationError};
//!
//! let mut registry = ToolRegistry::new();
//! registry.register(MyTool::new());
//!
//! // Validate all tools before use
//! match registry.validate_cli_tools() {
//!     Ok(()) => println!("All tools are valid!"),
//!     Err(errors) => {
//!         eprintln!("Found {} validation errors:", errors.len());
//!         for error in errors {
//!             eprintln!("- {}", error);
//!             if let Some(suggestion) = error.suggestion() {
//!                 eprintln!("  💡 {}", suggestion);
//!             }
//!         }
//!     }
//! }
//! ```
//!
//! ### Graceful Error Handling in Production
//!
//! ```rust,ignore
//! let registry = build_tool_registry(); // Your registry setup
//!
//! // Get validation statistics for monitoring
//! let stats = registry.get_validation_stats();
//! if !stats.is_all_valid() {
//!     tracing::warn!("Tool validation issues: {}", stats.summary());
//!
//!     // Log warnings but continue operation
//!     let warnings = registry.get_tool_validation_warnings();
//!     for warning in warnings.iter().take(5) {
//!         tracing::warn!("Validation issue: {}", warning);
//!     }
//! }
//!
//! // Build CLI with graceful degradation
//! let cli_builder = CliBuilder::new(registry);
//! let cli = cli_builder.build_cli_with_warnings(); // Skips invalid tools
//! ```
//!
//! ### Tool Development Best Practices
//!
//! When creating new tools, follow these patterns for robust validation:
//!
//! ```rust,ignore
//! #[derive(Default)]
//! pub struct WellDesignedTool;
//!
//! #[async_trait]
//! impl McpTool for WellDesignedTool {
//!     fn name(&self) -> &'static str {
//!         "memo_create" // category_action pattern
//!     }
//!
//!     fn schema(&self) -> serde_json::Value {
//!         serde_json::json!({
//!             "type": "object",
//!             "properties": {
//!                 // Use supported types only
//!                 "title": {
//!                     "type": "string",
//!                     "description": "Clear description for CLI help",
//!                     "minLength": 1
//!                 },
//!                 "priority": {
//!                     "type": "integer",
//!                     "description": "Priority level (1-5)",
//!                     "minimum": 1,
//!                     "maximum": 5,
//!                     "default": 3
//!                 },
//!                 "tags": {
//!                     "type": "array",
//!                     "description": "Optional tags for categorization",
//!                     "items": {"type": "string"}
//!                 }
//!             },
//!             "required": ["title"] // Match actual requirements
//!         })
//!     }
//!
//!     // CLI integration handled automatically via naming convention
//!     // cli_category() returns "memo", cli_name() returns "create"
//! }
//! ```
//!
//! ## Migration from Legacy System
//!
//! This registry pattern replaces the previous delegation-based approach where all
//! tools were routed through `ToolHandlers` with a large match statement. The new
//! pattern offers:
//!
//! - **Modularity**: Each tool is self-contained in its own module
//! - **Extensibility**: New tools can be added without modifying existing code
//! - **Testability**: Tools can be unit tested independently
//! - **Performance**: Direct access to storage eliminates delegation overhead
//! - **Reliability**: Comprehensive validation prevents runtime CLI failures
//!
//! # Creating New Tools
//!
//! To create a new MCP tool:
//!
//! 1. Create a struct implementing the `McpTool` trait
//! 2. Define the tool's schema using JSON Schema
//! 3. Implement the execute method with your business logic
//! 4. Register the tool with the appropriate registry function
//!
//! ```rust,ignore
//! use async_trait::async_trait;
//! use crate::mcp::tool_registry::{McpTool, ToolContext, BaseToolImpl};
//!
//! #[derive(Default)]
//! pub struct MyTool;
//!
//! #[async_trait]
//! impl McpTool for MyTool {
//!     fn name(&self) -> &'static str {
//!         "my_tool_name"
//!     }
//!
//!     fn description(&self) -> &'static str {
//!         include_str!("description.md")
//!     }
//!
//!     fn schema(&self) -> serde_json::Value {
//!         serde_json::json!({
//!             "type": "object",
//!             "properties": {
//!                 "param": {"type": "string", "description": "Parameter description"}
//!             },
//!             "required": ["param"]
//!         })
//!     }
//!
//!     async fn execute(
//!         &self,
//!         arguments: serde_json::Map<String, serde_json::Value>,
//!         context: &ToolContext,
//!     ) -> std::result::Result<CallToolResult, McpError> {
//!         let request: MyRequest = BaseToolImpl::parse_arguments(arguments)?;
//!         // Tool implementation here
//!         Ok(BaseToolImpl::create_success_response("Success!"))
//!     }
//! }
//! ```

mod context;
mod newtypes;
mod registration;
mod registry;
mod tool;
mod validation;

#[cfg(test)]
mod tests;

pub use context::ToolContext;
pub use registration::{
    create_fully_registered_tool_registry, register_code_context_tools, register_diagnostics_tools,
    register_git_tools, register_kanban_tools, register_ralph_tools, register_review_tools,
    register_web_tools, send_mcp_log, LoggerName,
};
pub use registry::{CliCategory, CliName, ToolRegistry};
pub use tool::{McpTool, ToolCategory};
pub use validation::{
    BaseToolImpl, ToolValidationError, ToolValidationReport, ToolValidationSeverity,
    ToolValidationWarning, ValidationError,
};
