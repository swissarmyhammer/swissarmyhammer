//! File editing and manipulation tools for MCP operations
//!
//! This module provides a comprehensive suite of file manipulation and search tools for
//! AI-assisted development environments. The tools are designed to work together as a
//! cohesive system for file management in development workflows.
//!
//! ## Tool Overview
//!
//! The file tools module implements five core file editing tools that provide essential
//! file system operations for code analysis, editing, and search functionality:
//!
//! ### Core File Operations
//! - **read**: Read and return file contents with support for various file types
//! - **write**: Create new files or completely overwrite existing files  
//! - **edit**: Perform precise string replacements in existing files
//!
//! ### File Discovery & Search
//! - **glob**: Fast file pattern matching with advanced filtering and sorting
//! - **grep**: Content-based search using ripgrep for fast text searching
//!
//! ## Tool Composition Patterns
//!
//! These tools are designed to work together in common workflows:
//! - Use **glob** to find relevant files, then **read** to examine contents
//! - Use **grep** to locate specific code, then **edit** to make changes
//! - Use **read** before **edit** to understand context  
//! - Use **write** for new files, **edit** for modifications
//!
//! ## Security & Safety
//!
//! All file tools implement comprehensive security measures:
//! - **Workspace Boundaries**: All file paths are validated to be within workspace
//! - **Path Validation**: Absolute paths are required and validated for safety
//! - **File Permissions**: Proper handling of file permissions and access rights
//! - **Atomic Operations**: Edit operations are atomic to prevent corruption
//! - **Ignore Patterns**: Respect .gitignore and other ignore patterns where appropriate
//!
//! ## Performance Characteristics
//!
//! - **glob** and **grep** are optimized for large codebases using efficient algorithms
//! - **read** supports partial reading for large files via offset/limit parameters
//! - **edit** performs atomic operations to ensure consistency
//! - All tools respect workspace boundaries to limit scope and improve performance
//!
//! ## Tool Implementation Pattern
//!
//! Each file tool follows the standard MCP pattern with shared utilities:
//!
//! - **Tool Structure**: Each tool implements the `McpTool` trait with `name()`, `description()`, `schema()`, and `execute()` methods
//! - **Argument Parsing**: Uses `BaseToolImpl::parse_arguments()` for JSON argument parsing with serde
//! - **Security Validation**: Leverages `shared_utils::validate_file_path()` for path validation and security checks
//! - **Error Handling**: Uses MCP-compatible error types and proper error propagation
//! - **Response Creation**: Returns structured responses using `BaseToolImpl::create_success_response()`
//!
//! See individual tool modules (`read`, `write`, `edit`, `glob`, `grep`) for specific implementations.
//!
//! ## Available Tools
//!
//! - **read**: Read file contents from the local filesystem with support for various file types
//! - **edit**: Perform precise string replacements in existing files  
//! - **write**: Create new files or completely overwrite existing files
//! - **glob**: Fast file pattern matching with advanced filtering and sorting
//! - **grep**: Content-based search using ripgrep for fast and flexible text searching

pub mod edit;
pub mod glob;
pub mod grep;
pub mod read;
pub mod shared_utils;
pub mod write;

use crate::mcp::tool_registry::ToolRegistry;

/// Register all file-related tools with the registry
pub async fn register_file_tools(registry: &mut ToolRegistry) {
    registry.register(read::ReadFileTool::new());
    registry.register(edit::EditFileTool::new());
    registry.register(write::WriteFileTool::new());
    registry.register(glob::GlobFileTool::new());
    registry.register(grep::GrepFileTool::new().await);
}
