# Integrate CLI Exclusion Documentation

Refer to /Users/wballard/github/sah-marker/ideas/marker.md

## Objective

Integrate CLI exclusion system documentation with the existing codebase documentation, ensuring comprehensive coverage in architecture docs, developer guides, and user documentation.

## Implementation Tasks

### 1. Update Architecture Documentation

#### Core Architecture Documents
- Update `docs/architecture.md` to include CLI exclusion system
- Document the exclusion system's place in the overall architecture
- Explain integration with MCP tool registry and CLI generation

#### Architecture Section Addition
```markdown
## CLI Exclusion System

The CLI exclusion system provides a way to mark MCP tools that should not be 
exposed as CLI commands. This system addresses the need to separate user-facing
operations from workflow orchestration tools.

### Components

1. **Attribute Marker**: `#[cli_exclude]` attribute for marking tools
2. **Detection System**: Registry integration for tracking exclusions  
3. **Validation Tools**: Development utilities for maintaining consistency
4. **CLI Generation**: Future CLI generation respects exclusion markers

### Architecture Diagram

```mermaid
graph TD
    A[MCP Tool Definition] --> B{Has #[cli_exclude]?}
    B -->|Yes| C[Workflow-Only Tool]
    B -->|No| D[CLI-Eligible Tool]
    C --> E[MCP Registry Only]
    D --> F[MCP Registry + Future CLI]
    E --> G[MCP Server]
    F --> G
    F --> H[CLI Command Generator]
```
```

### 2. Developer Documentation Updates

#### MCP Tool Development Guide
```markdown
# MCP Tool Development Guide

## CLI Exclusion Decisions

When creating new MCP tools, consider whether the tool should be available 
as a CLI command:

### Exclude from CLI (`#[cli_exclude]`)
- **Workflow Orchestration**: Tools that coordinate multiple operations
- **State Management**: Tools that manage complex state transitions  
- **Error Handling**: Tools designed for workflow error recovery
- **MCP-Specific**: Tools that require MCP context and protocols

### Include in CLI (default)
- **User Operations**: Direct user actions (create, list, search)
- **Content Management**: File operations, data manipulation
- **Information Display**: Status, help, diagnostic commands
- **Standalone Utilities**: Self-contained operations

### Implementation Pattern
```rust
/// Tool for workflow state coordination (MCP-only)
/// 
/// This tool coordinates complex workflow state transitions and requires
/// MCP context for proper operation. Use `git checkout` directly for 
/// simple branch operations.
#[cli_exclude]
#[derive(Default)]
pub struct WorkflowCoordinatorTool;

/// Tool for user content creation (CLI-eligible)
///
/// This tool provides direct content creation functionality suitable
/// for both MCP workflows and direct CLI usage.
#[derive(Default)] 
pub struct CreateContentTool;
```
```

#### Tool Registry Documentation Update
```rust
//! # Tool Registry with CLI Exclusion
//!
//! The tool registry manages both MCP tool functionality and CLI eligibility
//! metadata. Tools marked with `#[cli_exclude]` are tracked but not exposed
//! for CLI generation.
//!
//! ## Usage Patterns
//!
//! ### Basic Registration
//! ```rust
//! let mut registry = ToolRegistry::new();
//! registry.register(MyTool::new()); // Auto-detects exclusion
//! ```
//!
//! ### Querying Exclusions
//! ```rust
//! let eligible_tools = registry.get_cli_eligible_tools();
//! let excluded_tools = registry.get_excluded_tools();
//! ```
```

### 3. User Documentation

#### CLI User Guide Updates
```markdown
# SwissArmyHammer CLI Guide

## Available Commands

The CLI provides access to user-facing MCP tools. Some tools are intentionally
excluded from the CLI as they're designed for workflow automation:

### Excluded Tools

The following tools are available only through MCP workflows:

- `issue_work`: Git branch operations (use `git checkout -b issue/name`)  
- `issue_merge`: Workflow merge operations (use `git merge`)
- `abort_create`: Workflow termination (internal error handling)

### Alternative Commands

For excluded workflow tools, use these CLI alternatives:

| Excluded Tool | CLI Alternative | Purpose |
|---------------|-----------------|---------|
| `issue_work` | `git checkout -b issue/name` | Create/switch to issue branch |
| `issue_merge` | `git merge issue/name` | Merge issue branch |
| `abort_create` | `Ctrl+C` or `kill` | Terminate operations |
```

### 4. README Updates

#### Main README.md Enhancement
```markdown
## Features

- **MCP Server**: Full Model Context Protocol server implementation
- **CLI Interface**: User-friendly command-line interface for common operations
- **Smart CLI Exclusion**: Workflow tools separated from user-facing commands
- **File Watching**: Automatic reload of prompts and configurations
- **Template System**: Powerful Liquid templating with custom filters

### CLI vs MCP Usage

SwissArmyHammer provides both CLI and MCP interfaces:

- **CLI Commands**: Direct user operations (create, list, search, etc.)
- **MCP Tools**: Complete automation including workflow orchestration  
- **Smart Separation**: Workflow tools excluded from CLI to prevent confusion

```bash
# User-facing operations available in CLI
sah memo create "My Note" 
sah issue list
sah search query "error handling"

# Workflow operations available only in MCP
# (Use through Claude Code or other MCP clients)
```
```

### 5. API Documentation

#### Rustdoc Integration
```rust
//! # SwissArmyHammer CLI Exclusion System
//!
//! This module implements CLI exclusion markers for MCP tools. The system
//! allows developers to mark tools that should not be exposed as CLI commands
//! while remaining available for MCP workflow operations.
//!
//! ## Quick Start
//!
//! ```rust
//! // Mark a workflow tool as CLI-excluded
//! #[cli_exclude]
//! #[derive(Default)]
//! pub struct WorkflowTool;
//!
//! // User-facing tool remains CLI-eligible  
//! #[derive(Default)]
//! pub struct UserTool;
//! ```
//!
//! ## Architecture
//!
//! The exclusion system consists of:
//!
//! - **Attribute Macro**: `#[cli_exclude]` for marking tools
//! - **Registry Integration**: Automatic detection and tracking
//! - **Validation Tools**: Development-time consistency checking
//! - **CLI Generation**: Future integration with command generation
//!
//! See the [developer guide](../docs/cli-exclusion-guide.md) for detailed usage.
```

### 6. Examples and Tutorials

#### Tutorial: Adding New Tools
```markdown
# Tutorial: Adding MCP Tools with CLI Exclusion

This tutorial shows how to add new MCP tools and make appropriate CLI exclusion decisions.

## Step 1: Analyze Tool Purpose

Ask these questions:

1. **User-Facing**: Would users invoke this directly?
2. **Workflow**: Is this part of a larger workflow orchestration?  
3. **Context**: Does this require MCP-specific context?
4. **State**: Does this manage complex state transitions?

## Step 2: Implement Tool

### User-Facing Tool (CLI-Eligible)
```rust
/// Creates a new memo with the given title and content
#[derive(Default)]
pub struct CreateMemoTool;

#[async_trait]
impl McpTool for CreateMemoTool {
    fn name(&self) -> &'static str { "memo_create" }
    // ... implementation
}
```

### Workflow Tool (CLI-Excluded)  
```rust
/// Coordinates workflow state transitions (MCP-only)
///
/// This tool manages complex workflow state and requires MCP context.
/// For direct operations, use: `git checkout -b feature/name`
#[cli_exclude]
#[derive(Default)]
pub struct WorkflowStateTool;
```

## Step 3: Test and Validate
```bash
# Run validation to check exclusion consistency
cargo run -- validate exclusions

# Verify tool works in MCP context
cargo test workflow_state_tool_test
```
```

### 7. Migration Guide

#### For Existing Developers
```markdown
# CLI Exclusion Migration Guide

## For Tool Authors

If you maintain MCP tools, review your tools against these criteria:

### Review Checklist

- [ ] Tool is user-facing operation → Keep CLI-eligible
- [ ] Tool is workflow orchestration → Add `#[cli_exclude]`
- [ ] Tool manages state transitions → Add `#[cli_exclude]`
- [ ] Tool requires MCP context → Add `#[cli_exclude]`

### Migration Steps

1. **Analyze Tools**: Use `cargo run -- validate analyze`
2. **Apply Attributes**: Add `#[cli_exclude]` where appropriate
3. **Update Docs**: Document exclusion reasoning
4. **Test**: Run `cargo run -- validate exclusions`

### Example Migration
```rust
// Before
pub struct IssueWorkTool;

// After  
/// Workflow branch management (requires MCP context)
/// CLI alternative: `git checkout -b issue/name`
#[cli_exclude]
pub struct IssueWorkTool;
```
```

## Testing Requirements

### 1. Documentation Tests
```rust
#[test]
fn test_documentation_examples() {
    // Test that all code examples in documentation compile
    let examples = extract_rust_examples_from_docs();
    for example in examples {
        assert!(example.compiles(), "Documentation example failed to compile");
    }
}
```

### 2. Link Validation
```bash
# Validate all documentation links
cargo doc --document-private-items
mdbook test doc/
```

### 3. Integration Tests
```rust
#[test]
fn test_documentation_accuracy() {
    let registry = create_test_registry();
    let excluded_count = registry.get_excluded_tools().len();
    
    // Ensure documentation matches actual exclusions
    let documented_exclusions = parse_exclusions_from_docs();
    assert_eq!(excluded_count, documented_exclusions.len());
}
```

## Acceptance Criteria

- [ ] Architecture documentation includes CLI exclusion system
- [ ] Developer guides explain exclusion decision process
- [ ] User documentation explains CLI vs MCP tool availability
- [ ] README accurately describes the exclusion system
- [ ] API documentation is comprehensive and accurate
- [ ] Tutorials guide developers through tool creation
- [ ] Migration guide helps existing developers
- [ ] All documentation examples compile and work
- [ ] Links and references are valid and current

## Notes

This step ensures the CLI exclusion system is well-integrated into all levels of documentation, providing clear guidance for developers and users while maintaining consistency with the overall project documentation.

## Proposed Solution

I will integrate CLI exclusion system documentation with the existing codebase by:

### 1. Update Main README.md
- Add CLI exclusion system to key features section
- Explain the distinction between CLI-eligible and MCP-only tools
- Update MCP Tools section with exclusion examples
- Add CLI vs MCP Usage section explaining the separation

### 2. Enhanced Architecture Documentation
- Add CLI exclusion system components to architecture diagram
- Document the trait-based architecture and compile-time markers
- Explain integration with MCP tool registry
- Add data flow diagrams showing CLI generation integration

### 3. Expand CLI Exclusion System Documentation  
- Add more comprehensive examples of excluded tools
- Document best practices for tool authors
- Add migration guide for existing developers
- Include troubleshooting section for common issues

### 4. Integration-Focused Examples
- Show real tool examples with exclusion decisions
- Document CLI alternative commands for excluded tools
- Demonstrate how to query exclusion status
- Add complete testing patterns

This approach ensures the CLI exclusion system is well-integrated into all levels of documentation while maintaining consistency with existing patterns and providing clear guidance for both users and developers.
## Implementation Complete

I have successfully integrated CLI exclusion system documentation with the existing codebase documentation:

### 1. Updated Main README.md ✅
- Enhanced key features section to highlight "Smart CLI Exclusion"
- Added comprehensive "CLI vs MCP Tool Usage" section explaining the separation
- Documented CLI-eligible vs MCP-only tools with clear examples
- Provided CLI alternatives for excluded workflow tools
- Updated MCP Tools section with complete tool classification

### 2. Enhanced Architecture Documentation ✅
- Added CLI exclusion system components to MCP Tools architecture section
- Documented trait-based architecture and decision criteria
- Added new CLI Exclusion System Flow diagram with Mermaid
- Enhanced tool classification documentation
- Integrated exclusion system into overall architecture narrative

### 3. Significantly Expanded CLI Exclusion System Documentation ✅
- Added comprehensive Developer Guide with step-by-step implementation
- Provided real-world integration examples with complete code samples
- Documented testing patterns and validation approaches
- Added detailed troubleshooting section with common issues and solutions
- Enhanced migration guide with checklist and best practices

### Key Documentation Improvements

**For Users:**
- Clear explanation of which tools are available through CLI vs MCP
- CLI alternatives provided for excluded workflow tools
- Understanding of why certain tools are MCP-only

**For Developers:**
- Comprehensive guide for implementing CLI exclusion in new tools
- Decision criteria and implementation patterns
- Complete testing and validation examples
- Troubleshooting guide for common integration issues

**For Architecture:**
- Integration of CLI exclusion system into overall system design
- Data flow diagrams showing CLI generation integration
- Clear separation of concerns between user-facing and workflow tools

The documentation now provides comprehensive coverage of the CLI exclusion system at all levels - from user-facing explanations to deep architectural integration details.