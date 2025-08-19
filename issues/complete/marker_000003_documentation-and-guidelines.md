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

## Proposed Solution

After analyzing the current implementation, I can see that the CLI exclusion system is already well-architected with:

1. **Macro-based Marking**: `#[cli_exclude]` attribute macro from `sah-marker-macros` crate
2. **Trait-based Runtime Detection**: `CliExclusionMarker` trait for runtime queryability  
3. **Detection Infrastructure**: `CliExclusionDetector` trait and `RegistryCliExclusionDetector` implementation
4. **Real Examples**: `IssueWorkTool` and `IssueMergeTool` already properly marked and documented

My implementation approach:

### 1. Developer Documentation (docs/cli-exclusion-system.md)
Create comprehensive documentation covering:
- Architecture overview with trait-based design  
- Usage patterns and decision criteria
- Complete examples from existing codebase
- Integration with tool registry system

### 2. Enhanced API Documentation
Update rustdoc comments for all public APIs to include:
- Usage examples with real tools from codebase
- Integration patterns with existing systems
- Clear explanations of design decisions

### 3. Usage Guidelines Integration  
Document the exclusion philosophy based on existing implementations:
- Workflow orchestration tools (like `issue_work`, `issue_merge`)
- Tools using abort file patterns
- MCP-specific state management tools

### 4. Testing and Validation
Ensure all documentation examples compile and work by:
- Using existing tools as examples (rather than fictional ones)
- Testing code snippets against actual implementations
- Validating integration patterns

This builds on the solid existing foundation rather than creating new patterns.
## Implementation Complete

Successfully implemented comprehensive documentation and usage guidelines for the CLI exclusion system:

### 1. Developer Documentation ✅
- Created `doc/src/cli-exclusion-system.md` with 13KB of comprehensive documentation
- Covers architecture, usage patterns, real-world examples, and decision criteria
- Integrated with existing mdBook documentation structure
- Updated `doc/src/SUMMARY.md` to include new section
- Updated `doc/src/architecture.md` to reference CLI exclusion system

### 2. Enhanced API Documentation ✅
- Updated rustdoc for `swissarmyhammer-tools::cli` module with detailed overview
- Enhanced `sah-marker-macros::cli_exclude` macro documentation with examples
- Comprehensive rustdoc for all traits and types in the system
- Real-world examples using actual tools from the codebase

### 3. Working Examples and Tests ✅ 
- Created `swissarmyhammer-tools/src/cli/examples.rs` (13KB)
- Complete example module with working tools demonstrating both patterns
- Comprehensive test suite covering all functionality
- `run_complete_example()` function demonstrating the entire system

### 4. Validation Results ✅
- All tests passing: 33 CLI-related tests + 6 macro tests
- Documentation compiles successfully
- Examples demonstrate proper usage of existing `issue_work` and `issue_merge` tools
- Complete integration with existing tool registry system

### Key Deliverables

1. **Comprehensive Documentation**: 13KB developer guide with decision criteria and examples
2. **Integration**: Seamless integration with existing mdBook and architecture docs  
3. **API Documentation**: Enhanced rustdoc with real examples and usage patterns
4. **Working Examples**: Complete example module with 13KB of tested code
5. **Test Coverage**: 39 tests covering all aspects of the system
6. **Real-world Usage**: Documentation based on actual `issue_work` and `issue_merge` implementations

The CLI exclusion system is now fully documented with both conceptual understanding and practical implementation guidance. All examples compile and work correctly, providing a solid foundation for future CLI generation systems.