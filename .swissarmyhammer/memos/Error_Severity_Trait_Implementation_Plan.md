# Error Severity Trait Implementation Plan

## Current State Analysis

### Existing ErrorSeverity Enum
- Located in `swissarmyhammer-common/src/error.rs`
- Simple enum with Warning, Error, Critical levels
- Currently NOT a trait - just a type
- Used by `PlanCommandError` and `ValidationError` only

### Current Error Types in Codebase
Found 37+ error enums across the codebase:
- **swissarmyhammer-common**: SwissArmyHammerError, ParameterError, ConditionError
- **swissarmyhammer-cli**: ValidationError, ConversionError
- **swissarmyhammer-workflow**: WorkflowError, ExecutorError, GraphError, StateError, ParseError, ActionError
- **swissarmyhammer-rules**: RuleError
- **swissarmyhammer-git**: GitError
- **swissarmyhammer-search**: SearchError
- **swissarmyhammer-issues**: (likely IssuesError)
- **swissarmyhammer-memoranda**: MemorandaError
- **swissarmyhammer-todo**: TodoError
- **swissarmyhammer-templating**: TemplatingError
- **swissarmyhammer-outline**: OutlineError
- **swissarmyhammer-agent-executor**: ActionError
- **swissarmyhammer-shell**: ShellSecurityError, ShellError
- **swissarmyhammer-tools**: Many tool-specific errors
- **swissarmyhammer-config**: ConfigurationError, AgentError
- **swissarmyhammer** (main): PlanCommandError

### Spec Requirements
1. Create a **shared Severity trait** in a new **swissarmyhammer-utils crate**
2. All error types should implement this trait
3. Trait should provide a `severity()` method returning ErrorSeverity

### Implementation Approach

#### Option 1: New swissarmyhammer-utils Crate (Spec Requirement)
- Create new crate for shared utilities
- Move ErrorSeverity enum to this crate
- Define Severity trait in this crate
- Add swissarmyhammer-utils as dependency to all crates with errors
- ~40 files to modify across ~15 crates

#### Option 2: Use Existing swissarmyhammer-common Crate (Simpler)
- ErrorSeverity already lives here
- swissarmyhammer-common is already a workspace-level utility crate
- All crates already depend on it or can easily add it
- No new crate needed
- Aligns with existing architecture

### Recommendation
**Use swissarmyhammer-common** instead of creating swissarmyhammer-utils:
- swissarmyhammer-common IS the shared utilities crate
- Less churn (no new crate, no Cargo.toml updates)
- ErrorSeverity already lives there
- More maintainable

### Trait Design

```rust
/// Trait for error types that have severity levels
pub trait Severity {
    /// Get the severity level of this error
    fn severity(&self) -> ErrorSeverity;
}
```

### Implementation Strategy

1. **Phase 1: Foundation** (swissarmyhammer-common)
   - Define Severity trait
   - Document usage guidelines
   - Implement for SwissArmyHammerError

2. **Phase 2: Domain Crates** (by domain)
   - Implement for each crate's error types
   - One crate at a time to keep changes small
   - Update tests

3. **Phase 3: Verification**
   - Ensure all error types implement Severity
   - Add integration tests
   - Update documentation

### Small, Incremental Steps

Each step should be < 250 lines of code changed:
1. Create Severity trait + implement for SwissArmyHammerError (50 lines)
2. Implement for workflow errors (50 lines)
3. Implement for CLI errors (50 lines)
4. Implement for tools errors (100 lines)
5. Implement for remaining domain errors (50 lines each)

### Testing Strategy
- Each implementation should include unit tests
- Test that severity() returns expected values
- Add tests for new trait implementations
