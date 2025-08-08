//! Outline generation tools for MCP operations
//!
//! This module provides tools for generating structured code overviews using Tree-sitter parsing.
//! The outline tools help developers understand code structure, generate documentation, and
//! perform code analysis tasks.
//!
//! ## Outline Generation Workflow
//!
//! The outline tools process source code files through several steps:
//!
//! 1. **File Discovery**: Use glob patterns to find matching files
//! 2. **Language Detection**: Determine appropriate Tree-sitter parser
//! 3. **Parsing**: Parse source code into abstract syntax trees
//! 4. **Symbol Extraction**: Extract meaningful symbols and their metadata
//! 5. **Hierarchy Building**: Organize symbols into nested structures
//! 6. **Formatting**: Output results in requested format (YAML/JSON)
//!
//! ## Tool Implementation Pattern
//!
//! Each tool follows the standard MCP pattern:
//! ```ignore
//! use async_trait::async_trait;
//! use swissarmyhammer::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
//!
//! #[derive(Default)]
//! pub struct ExampleOutlineTool;
//!
//! impl ExampleOutlineTool {
//!     pub fn new() -> Self { Self }
//! }
//!
//! #[async_trait]
//! impl McpTool for ExampleOutlineTool {
//!     fn name(&self) -> &'static str {
//!         "outline_example"
//!     }
//!     
//!     fn description(&self) -> &'static str {
//!         include_str!("generate/description.md")
//!     }
//!     
//!     fn schema(&self) -> serde_json::Value {
//!         serde_json::json!({
//!             "type": "object",
//!             "properties": {
//!                 "patterns": {
//!                     "type": "array",
//!                     "items": {"type": "string"},
//!                     "description": "Glob patterns to match files"
//!                 }
//!             },
//!             "required": ["patterns"]
//!         })
//!     }
//!     
//!     async fn execute(
//!         &self,
//!         arguments: serde_json::Map<String, serde_json::Value>,
//!         context: &ToolContext,
//!     ) -> std::result::Result<rmcp::model::CallToolResult, rmcp::Error> {
//!         let request: generate::OutlineRequest = BaseToolImpl::parse_arguments(arguments)?;
//!         // Tool implementation here
//!         Ok(BaseToolImpl::create_success_response("Success!"))
//!     }
//! }
//! ```
//!
//! ## Available Tools
//!
//! - **generate**: Generate structured code outlines from source files
//!
//! ## Supported Languages
//!
//! The outline tools support multiple programming languages through Tree-sitter:
//!
//! - **Rust**: Comprehensive support for structs, enums, functions, traits, modules
//! - **Python**: Classes, functions, methods, properties, imports
//! - **TypeScript/JavaScript**: Classes, interfaces, functions, methods, types
//! - **Dart**: Classes, functions, methods, constructors, properties
//!
//! ## Output Formats
//!
//! All outline tools support multiple output formats:
//!
//! - **YAML**: Human-readable format suitable for documentation
//! - **JSON**: Machine-readable format for programmatic processing
//!
//! ## Performance Considerations
//!
//! - Tree-sitter parsing is performed concurrently for multiple files
//! - Memory usage scales linearly with the number of symbols
//! - Large codebases are processed in batches to prevent memory exhaustion
//! - Parsing errors are handled gracefully with fallback to basic text processing

pub mod generate;

use crate::mcp::tool_registry::ToolRegistry;

/// Register all outline-related tools with the registry
pub fn register_outline_tools(registry: &mut ToolRegistry) {
    registry.register(generate::OutlineGenerateTool::new());
}
