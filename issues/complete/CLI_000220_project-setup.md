# CLI MCP Integration: Project Setup

## Overview

Set up the foundational infrastructure for CLI-MCP integration by creating a shared tool context system that allows CLI commands to call MCP tools directly, eliminating code duplication.

## Problem Statement

The CLI currently implements its own business logic for operations that already exist as MCP tools, creating significant code duplication and maintenance burden. Commands like `issue.rs`, `memo.rs`, and `search.rs` duplicate functionality from their corresponding MCP tools.

## Goals

1. Create a shared `CliToolContext` that can instantiate MCP tools
2. Establish a pattern for CLI commands to call MCP tools directly
3. Set up proper error handling and response formatting for CLI contexts
4. Create utility functions for converting MCP responses to CLI-friendly output

## Tasks

### 1. Create CLI-MCP Integration Module

Create `swissarmyhammer-cli/src/mcp_integration.rs`:

```rust
//! Integration layer for calling MCP tools from CLI commands
//!
//! This module provides utilities for CLI commands to call MCP tools directly,
//! eliminating code duplication between CLI and MCP implementations.

use serde_json::Map;
use swissarmyhammer::mcp::tool_registry::{McpTool, ToolContext};
use swissarmyhammer::mcp::{McpServer, ToolRegistry};
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;

/// CLI-specific tool context that can create and execute MCP tools
pub struct CliToolContext {
    tool_context: ToolContext,
    tool_registry: ToolRegistry,
}

impl CliToolContext {
    /// Create a new CLI tool context with all necessary storage backends
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Implementation details
    }

    /// Execute an MCP tool with the given arguments
    pub async fn execute_tool(
        &self,
        tool_name: &str,
        arguments: Map<String, serde_json::Value>,
    ) -> Result<CallToolResult, McpError> {
        // Implementation details
    }

    /// Helper to convert CLI arguments to MCP tool arguments
    pub fn create_arguments(&self, pairs: Vec<(&str, serde_json::Value)>) -> Map<String, serde_json::Value> {
        // Implementation details
    }
}

/// Utilities for formatting MCP responses for CLI display
pub mod response_formatting {
    use rmcp::model::CallToolResult;
    use colored::*;

    /// Extract and format success message from MCP response
    pub fn format_success_response(result: &CallToolResult) -> String {
        // Implementation details
    }

    /// Extract and format error message from MCP response
    pub fn format_error_response(result: &CallToolResult) -> String {
        // Implementation details
    }

    /// Format structured data as CLI table
    pub fn format_as_table(data: &serde_json::Value) -> String {
        // Implementation details
    }
}
```

### 2. Update CLI Module Structure

Update `swissarmyhammer-cli/src/lib.rs` to include the new integration module:

```rust
pub mod mcp_integration;
```

### 3. Create Error Conversion Utilities

Add to `swissarmyhammer-cli/src/error.rs`:

```rust
impl From<rmcp::Error> for CliError {
    fn from(error: rmcp::Error) -> Self {
        CliError::new(format!("MCP error: {}", error), 1)
    }
}
```

### 4. Create Integration Tests

Create `swissarmyhammer-cli/tests/mcp_integration_test.rs`:

```rust
//! Integration tests for CLI-MCP tool integration

use swissarmyhammer_cli::mcp_integration::CliToolContext;
use serde_json::json;

#[tokio::test]
async fn test_cli_can_call_mcp_tools() {
    let context = CliToolContext::new().await.unwrap();
    
    // Test calling issue_create tool
    let args = context.create_arguments(vec![
        ("name", json!("test_issue")),
        ("content", json!("Test content")),
    ]);
    
    let result = context.execute_tool("issue_create", args).await;
    assert!(result.is_ok());
}
```

## Implementation Plan

1. **Create the integration module structure** - Set up the basic files and module declarations
2. **Implement CliToolContext** - Core functionality for executing MCP tools from CLI
3. **Add response formatting utilities** - Convert MCP responses to CLI-friendly output
4. **Create error handling integration** - Proper error conversion between MCP and CLI layers
5. **Add comprehensive tests** - Ensure the integration works correctly
6. **Update build system** - Ensure new dependencies are properly configured

## Acceptance Criteria

- [ ] `CliToolContext` can successfully instantiate and call MCP tools
- [ ] Response formatting utilities produce appropriate CLI output
- [ ] Error handling properly converts MCP errors to CLI errors
- [ ] Integration tests demonstrate successful CLI-MCP communication
- [ ] No regression in existing CLI functionality
- [ ] Code follows established patterns and coding standards

## Dependencies

- Must run after all existing MCP tools are stable
- Requires access to `swissarmyhammer::mcp` module functionality
- Dependencies on `rmcp`, `serde_json`, `colored` crates

## Expected Changes

- New file: `swissarmyhammer-cli/src/mcp_integration.rs` (~300 lines)
- Modified: `swissarmyhammer-cli/src/lib.rs` (1 line addition)
- Modified: `swissarmyhammer-cli/src/error.rs` (~10 lines)
- New file: `swissarmyhammer-cli/tests/mcp_integration_test.rs` (~100 lines)
- Modified: `swissarmyhammer-cli/Cargo.toml` (dependency updates if needed)

## Follow-up Issues

This foundational work enables the subsequent refactoring of individual CLI command modules (issues, memo, search) to use MCP tools directly.

## Proposed Solution

Based on the existing MCP infrastructure and codebase patterns, I'll implement a clean CLI-MCP integration layer that allows CLI commands to call MCP tools directly.

### Implementation Approach

1. **CliToolContext Structure**: Create a CLI-specific version of the existing ToolContext that can instantiate all necessary storage backends and MCP tools without requiring server initialization.

2. **Direct Tool Registry Access**: Leverage the existing ToolRegistry pattern to allow CLI commands to call tools directly without going through the MCP protocol layer.

3. **Response Formatting**: Create utilities to convert MCP CallToolResult responses into CLI-friendly output with proper formatting and error handling.

4. **Error Integration**: Extend the existing CLI error handling to properly convert MCP errors using the established patterns.

### Key Benefits

- **Zero Duplication**: CLI commands will use identical business logic as MCP tools
- **Consistent Behavior**: Same validation, error handling, and responses across interfaces
- **Maintainable**: Single source of truth for all operations
- **Testable**: Both CLI and MCP functionality tested through same code paths

### Implementation Steps

1. Create `mcp_integration.rs` module with CliToolContext
2. Implement response formatting utilities with colored output
3. Add MCP error conversion to CLI error system
4. Create comprehensive integration tests
5. Update build system and module exports

This approach maintains the existing architecture patterns while enabling direct CLI-to-MCP tool communication.