# Workflow Module Migration Specification

## Overview

This specification outlines the plan to move the workflow logical module from `swissarmyhammer/src/workflow` to a new standalone crate `swissarmyhammer-workflow`. The migration must preserve all existing functionality while maintaining system operability throughout the incremental process.

## Current State Analysis

### Workflow Module Structure
The workflow module currently exists at `swissarmyhammer/src/workflow/` and contains:

**Core Module Files:**
- `mod.rs` - Module entry point with public API exports
- `definition.rs` - Workflow definition types
- `state.rs` - State management
- `transition.rs` - State transition logic
- `graph.rs` - Workflow graph analysis
- `run.rs` - Runtime execution state
- `storage.rs` - Persistence layer

**Execution System:**
- `executor/` - Execution engine (mod.rs, core.rs, validation.rs, fork_join.rs, tests.rs)
- `actions.rs` - Action definitions and implementations
- `action_parser.rs` - Action parsing logic
- `mcp_integration.rs` - MCP protocol integration

**Template and Parsing:**
- `parser.rs` - Mermaid diagram parsing
- `template_context.rs` - Template rendering context
- `template_context_integration_test.rs` - Template integration tests

**Utilities and Support:**
- `agents/` - Agent execution (mod.rs, llama_agent_executor.rs)
- `metrics.rs` - Performance monitoring
- `visualization.rs` - Graph visualization
- `error_utils.rs` - Error handling utilities
- `test_helpers.rs` - Testing utilities

**Test Infrastructure:**
- `actions_tests/` - Comprehensive action testing suite (15+ test files)
- `*_tests.rs` - Various unit test files
- `examples_tests.rs` - Example validation tests

### Dependencies and Usage

The workflow module is currently used by:
1. **Main swissarmyhammer crate** - Core workflow functionality
2. **swissarmyhammer-cli** - CLI workflow execution
3. **Various test suites** - Integration and unit tests

Current dependency chain:
```
swissarmyhammer-cli -> swissarmyhammer -> workflow module
```

Target dependency chain:
```
swissarmyhammer-cli -> swissarmyhammer-workflow
swissarmyhammer -> swissarmyhammer-workflow (if needed)
```

## Migration Strategy

### Phase 1: New Crate Creation
1. **Create `swissarmyhammer-workflow` directory structure**
   - Initialize new Cargo.toml with appropriate dependencies
   - Set up src/lib.rs as module entry point
   - Add to workspace members in root Cargo.toml

2. **Dependency Analysis and Setup**
   - Extract all workflow-specific dependencies from swissarmyhammer/Cargo.toml
   - Add swissarmyhammer-workflow to workspace dependencies
   - Ensure all required SwissArmyHammer internal crates are accessible

### Phase 2: Incremental File Migration

**Migration Order (Dependency-First Approach):**

1. **Utility and Support Files** (No internal dependencies)
   - `error_utils.rs`
   - `test_helpers.rs` 
   - `transition_key.rs`

2. **Core Data Types** (Minimal dependencies)
   - `state.rs`
   - `transition.rs`
   - `definition.rs`
   - `run.rs`

3. **Parsing and Template System**
   - `parser.rs`
   - `template_context.rs`

4. **Action System** 
   - `action_parser.rs`
   - `actions.rs`
   - `agents/` (entire directory)

5. **Storage Layer**
   - `storage.rs`

6. **Graph and Visualization**
   - `graph.rs`
   - `visualization.rs`

7. **Execution System**
   - `executor/` (entire directory)
   - `mcp_integration.rs`
   - `metrics.rs`

8. **Test Infrastructure**
   - `actions_tests/` (entire directory)
   - All `*_tests.rs` files
   - `examples_tests.rs`

### Phase 3: Import Path Updates

For each migrated file, update imports in dependent files:

**From:**
```rust
use crate::workflow::definition::Workflow;
use super::workflow::actions::Action;
```

**To:**
```rust
use swissarmyhammer_workflow::definition::Workflow;
use swissarmyhammer_workflow::actions::Action;
```

### Phase 4: Public API Harmonization

1. **Update swissarmyhammer-workflow/src/lib.rs**
   - Re-export all public APIs that were previously exported from workflow/mod.rs
   - Ensure API compatibility with existing consumers

2. **Update swissarmyhammer/src/lib.rs**
   - Add re-exports for swissarmyhammer-workflow types if backward compatibility needed
   - Or remove workflow-related exports if clean break desired

3. **Update swissarmyhammer-cli**
   - Change imports to use swissarmyhammer-workflow directly
   - Update Cargo.toml dependencies

### Phase 5: Cleanup
1. **Remove original workflow directory**
   - Delete `swissarmyhammer/src/workflow/`
   - Remove workflow-specific dependencies from swissarmyhammer/Cargo.toml

2. **Update documentation and examples**
   - Update import paths in documentation
   - Verify all examples still work

## Technical Implementation Details

### New Crate Structure
```
swissarmyhammer-workflow/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── definition.rs
│   ├── state.rs
│   ├── transition.rs
│   ├── graph.rs
│   ├── run.rs
│   ├── storage.rs
│   ├── executor/
│   │   ├── mod.rs
│   │   ├── core.rs
│   │   ├── validation.rs
│   │   ├── fork_join.rs
│   │   └── tests.rs
│   ├── actions.rs
│   ├── action_parser.rs
│   ├── mcp_integration.rs
│   ├── parser.rs
│   ├── template_context.rs
│   ├── agents/
│   │   ├── mod.rs
│   │   └── llama_agent_executor.rs
│   ├── metrics.rs
│   ├── visualization.rs
│   ├── error_utils.rs
│   └── test_helpers.rs
└── tests/
    └── integration_tests.rs (if needed)
```

### Cargo.toml Dependencies

**swissarmyhammer-workflow/Cargo.toml:**
```toml
[package]
name = "swissarmyhammer-workflow"
version.workspace = true
edition.workspace = true
# ... other metadata

[dependencies]
# Internal SwissArmyHammer crates
swissarmyhammer-common = { path = "../swissarmyhammer-common" }
swissarmyhammer-shell = { path = "../swissarmyhammer-shell" }
swissarmyhammer-templating = { path = "../swissarmyhammer-templating" }
swissarmyhammer-git = { path = "../swissarmyhammer-git" }

# Workflow-specific external dependencies
mermaid-parser = { git = "https://github.com/wballard/mermaid_parser" }
cel-interpreter = "0.8"
chumsky = "0.10.1"
llama-agent = { workspace = true }
# ... other workflow-specific deps
```

### API Compatibility

The new crate must export exactly the same public API as the current workflow module:

```rust
// swissarmyhammer-workflow/src/lib.rs
pub use actions::{
    is_valid_env_var_name, parse_action_from_description,
    parse_action_from_description_with_context, validate_command,
    validate_environment_variables_security, validate_working_directory_security, Action,
    ActionError, ActionResult, AgentExecutionContext, AgentExecutor, AgentExecutorFactory,
    LogAction, LogLevel, PromptAction, SetVariableAction, ShellAction, SubWorkflowAction,
    WaitAction,
};
pub use agents::LlamaAgentExecutor;
pub use definition::{Workflow, WorkflowError, WorkflowName, WorkflowResult};
// ... all other current exports
```

## Migration Execution Plan

### Step-by-Step Process

1. **Setup Phase**
   - Create swissarmyhammer-workflow directory
   - Initialize Cargo.toml with basic structure
   - Add to workspace members
   - Create basic src/lib.rs

2. **File Migration Loop** (for each file in dependency order):
   - Copy file to new crate location
   - Update internal imports within the file
   - Add re-export to swissarmyhammer-workflow/src/lib.rs
   - Run `cargo check` to verify compilation
   - Update any external files that import from this module
   - Run full test suite to ensure no regressions
   - Commit changes

3. **Test Migration**
   - Move test files maintaining same structure
   - Update test imports
   - Verify all tests pass in new location

4. **Final Integration**
   - Update CLI crate to use new dependency
   - Remove old workflow directory
   - Clean up old dependencies
   - Final test run

### Validation Criteria

For each migration step:
- [ ] `cargo check` passes for all workspace members
- [ ] `cargo test` passes for all relevant tests
- [ ] No functionality regressions
- [ ] CLI workflows continue to work
- [ ] All import paths resolve correctly

## Risk Mitigation

### Potential Issues and Solutions

1. **Circular Dependencies**
   - **Risk:** New crate might need something from main swissarmyhammer crate
   - **Mitigation:** Careful dependency analysis; move shared code to swissarmyhammer-common

2. **Test Failures**
   - **Risk:** Tests might fail due to import path changes
   - **Mitigation:** Incremental testing at each step; fix imports immediately

3. **CLI Breaking Changes**
   - **Risk:** CLI might break if workflow APIs change
   - **Mitigation:** Maintain exact API compatibility; test CLI at each step

4. **Performance Regressions**
   - **Risk:** Additional crate boundaries might impact performance
   - **Mitigation:** Benchmark critical paths; optimize if needed

## Success Criteria

Migration is complete when:
1. All workflow code exists in swissarmyhammer-workflow crate
2. No workflow code remains in main swissarmyhammer crate
3. All tests pass
4. CLI functionality unchanged
5. Build times are reasonable
6. No circular dependencies exist
7. Documentation is updated

## Timeline Estimate

- **Phase 1 (Setup):** 2-4 hours
- **Phase 2 (File Migration):** 8-12 hours (depending on import complexity)
- **Phase 3 (Import Updates):** 4-6 hours
- **Phase 4 (API Harmonization):** 2-4 hours
- **Phase 5 (Cleanup):** 1-2 hours

**Total Estimated Time:** 17-28 hours

This incremental approach ensures the system remains functional throughout the migration while systematically moving all workflow functionality to its own dedicated crate.