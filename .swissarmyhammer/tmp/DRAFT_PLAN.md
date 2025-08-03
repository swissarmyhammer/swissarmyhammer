# Todo Tool Implementation Plan

## Overview
This plan implements the Todo Tool specification which provides ephemeral task management capabilities through MCP tools. The todo system is designed exclusively for LLM use during development sessions and is not exposed through the CLI.

## Current State Analysis
- No existing todo tool implementation found in codebase
- Specification exists at `./specification/todo_tool.md`
- Need to implement MCP tools following the established patterns
- Use existing architecture patterns from issues and memoranda tools

## Key Requirements
1. YAML-based file format with ULID identifiers
2. Three core MCP tools: create, show, mark_complete
3. File storage in `./swissarmyhammer/todo/` (gitignored)
4. Sequential ULID generation for task ordering
5. FIFO "next" item retrieval pattern
6. NO CLI integration (LLM-only)

## Implementation Strategy
Following established MCP tool patterns:
- Use `src/mcp/tools/todo/` directory structure
- Implement `create/`, `show/`, `mark_complete/` submodules
- Follow YAML storage patterns similar to other tools
- Leverage existing error handling and validation patterns
- Implement proper test coverage

## Technical Considerations
- ULID generation for sequential ordering
- YAML serialization/deserialization with serde
- File system operations with proper error handling
- Directory creation and gitignore management
- Thread-safe file operations
- Validation for required fields

## Integration Points
- MCP server tool registration
- Error handling through SwissArmyHammerError
- File path validation and security
- ULID generation utilities
- YAML processing utilities

## Testing Strategy
- Unit tests for each tool function
- Integration tests for file operations
- Property-based testing for ULID ordering
- Error condition testing
- File system isolation for tests