# Create Documentation and Usage Guidelines

Refer to /Users/wballard/github/sah-marker/ideas/marker.md

## Objective

Establish comprehensive documentation for the CLI exclusion marker system, including usage guidelines, best practices, and architectural documentation for developers.

## Implementation Tasks

### 1. Developer Documentation
- Create `docs/cli-exclusion-system.md` with comprehensive documentation
- Document the exclusion philosophy and decision criteria
- Provide clear guidelines on when to use `#[cli_exclude]`

### 2. Usage Guidelines Document
```markdown
# CLI Exclusion Guidelines

## When to Use #[cli_exclude]

### Workflow-Specific Tools
Tools designed for MCP workflow orchestration:
- State transition management
- Cross-tool coordination
- Workflow-specific error handling

### Examples
- `issue_work`: Git branch operations within issue workflows
- `issue_merge`: Complex merge logic with abort handling
- Workflow orchestration tools

## When NOT to Use #[cli_exclude]

### User-Facing Operations
Tools that users might want to invoke directly:
- Content creation (memos, issues)
- Search and query operations  
- File operations
- Information display tools
```

### 3. Architecture Documentation
- Update existing architecture docs to include exclusion system
- Document the attribute processing pipeline
- Explain integration with tool registry and future CLI generation

### 4. Code Examples and Patterns
```rust
// GOOD: Workflow-specific tool excluded from CLI
#[cli_exclude]
#[derive(Default)]
pub struct WorkflowOrchestratorTool {
    // Complex state management, abort file handling
}

// GOOD: User-facing tool available in CLI  
#[derive(Default)]
pub struct CreateMemoTool {
    // Direct user operation
}
```

## Testing Requirements

### 1. Documentation Tests
- Verify all code examples in documentation compile correctly
- Test that documented patterns work as expected
- Validate example tool implementations

### 2. Documentation Completeness
- Ensure all public APIs are documented
- Verify documentation covers all usage scenarios
- Test that examples match actual implementation

## Documentation Content Structure

### 1. Overview Section
- Explain the CLI exclusion concept
- Describe the problem it solves
- Outline the implementation approach

### 2. Usage Guide
- Step-by-step instructions for marking tools
- Decision tree for when to exclude tools
- Common patterns and anti-patterns

### 3. Technical Reference
- Complete API documentation
- Attribute processing details
- Integration with registry system

### 4. Examples Section
- Real-world usage examples
- Before/after scenarios
- Common implementation patterns

### 5. Migration Guide
- Guidelines for existing tool authors
- Recommendations for tool categorization
- Future CLI generation preparation

## Rust Documentation

### 1. Module Documentation
```rust
//! # CLI Exclusion System
//!
//! This module provides infrastructure for marking MCP tools that should
//! be excluded from CLI generation. Tools marked with `#[cli_exclude]`
//! are designed for MCP workflow operations and should not be exposed
//! as direct CLI commands.
//!
//! ## Usage
//!
//! ```rust
//! #[cli_exclude]
//! #[derive(Default)]
//! pub struct WorkflowTool;
//! ```
```

### 2. Comprehensive Rustdoc
- Document all public functions with examples
- Include usage scenarios in documentation
- Provide links to related concepts

## Integration with Existing Documentation

### 1. Update MCP Tool Documentation
- Reference CLI exclusion in tool documentation patterns
- Update tool creation guidelines
- Include exclusion considerations in design decisions

### 2. Update CLI Documentation  
- Explain why some tools are not available in CLI
- Document the exclusion system for CLI users
- Provide guidance on MCP vs CLI usage

## Acceptance Criteria

- [ ] Comprehensive developer documentation is created
- [ ] Usage guidelines clearly explain when to use exclusions
- [ ] Architecture documentation integrates exclusion system
- [ ] Code examples compile and work correctly
- [ ] Documentation is integrated with existing docs
- [ ] All public APIs have rustdoc documentation
- [ ] Examples cover common usage patterns

## Notes

This step ensures that the CLI exclusion system is well-documented and easily understood by developers, creating a foundation for consistent usage across the codebase.