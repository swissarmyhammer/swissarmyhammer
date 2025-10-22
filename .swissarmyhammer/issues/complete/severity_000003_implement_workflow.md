# Step 3: Implement Severity for Workflow Errors

**Refer to ideas/severity.md**

## Goal

Implement the `Severity` trait for all workflow-related error types in swissarmyhammer-workflow crate.

## Context

The workflow crate contains several error types that need severity implementations:
- WorkflowError
- ExecutorError  
- GraphError
- StateError
- ActionError
- ParseError

## Tasks

### 1. Add swissarmyhammer-common Dependency

Ensure `swissarmyhammer-workflow/Cargo.toml` depends on swissarmyhammer-common:

```toml
[dependencies]
swissarmyhammer-common = { path = "../swissarmyhammer-common" }
```

### 2. Implement Severity for WorkflowError

In `swissarmyhammer-workflow/src/error.rs`:

```rust
use swissarmyhammer_common::{ErrorSeverity, Severity};

impl Severity for WorkflowError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            WorkflowError::InvalidState { .. } => ErrorSeverity::Critical,
            WorkflowError::TransitionFailed { .. } => ErrorSeverity::Critical,
            // Add other variants with appropriate severity
            _ => ErrorSeverity::Error,
        }
    }
}
```

### 3. Implement Severity for ExecutorError

In `swissarmyhammer-workflow/src/executor/mod.rs`:

```rust
impl Severity for ExecutorError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            ExecutorError::WorkflowFailed { .. } => ErrorSeverity::Critical,
            ExecutorError::ActionFailed { .. } => ErrorSeverity::Error,
            // Add other variants
            _ => ErrorSeverity::Error,
        }
    }
}
```

### 4. Implement Severity for Other Error Types

Implement for:
- `GraphError` in `swissarmyhammer-workflow/src/graph.rs`
- `StateError` in `swissarmyhammer-workflow/src/state.rs`
- `ActionError` in `swissarmyhammer-workflow/src/actions.rs`
- `ParseError` in `swissarmyhammer-workflow/src/parser.rs`

### 5. Add Tests

For each implementation, add unit tests verifying severity assignments:

```rust
#[cfg(test)]
mod severity_tests {
    use super::*;
    use swissarmyhammer_common::{ErrorSeverity, Severity};

    #[test]
    fn test_workflow_error_severity() {
        let error = WorkflowError::InvalidState { /* fields */ };
        assert_eq!(error.severity(), ErrorSeverity::Critical);
        
        // Test other variants
    }
}
```

## Severity Guidelines for Workflow Errors

**Critical**:
- Workflow execution failures
- Invalid state transitions
- Data corruption
- Unable to recover workflow state

**Error**:
- Action failures (can retry)
- Invalid configurations
- Missing resources

**Warning**:
- Deprecated workflow features
- Performance issues
- Optional features unavailable

## Acceptance Criteria

- [ ] All 6 workflow error types implement Severity
- [ ] Each error variant has appropriate severity
- [ ] Unit tests for each implementation
- [ ] Tests pass: `cargo test -p swissarmyhammer-workflow`
- [ ] Code compiles: `cargo build -p swissarmyhammer-workflow`
- [ ] Clippy clean: `cargo clippy -p swissarmyhammer-workflow`

## Files to Modify

- `swissarmyhammer-workflow/Cargo.toml` (add dependency)
- `swissarmyhammer-workflow/src/error.rs`
- `swissarmyhammer-workflow/src/executor/mod.rs`
- `swissarmyhammer-workflow/src/graph.rs`
- `swissarmyhammer-workflow/src/state.rs`
- `swissarmyhammer-workflow/src/actions.rs`
- `swissarmyhammer-workflow/src/parser.rs`

## Estimated Changes

~150 lines of code (6 implementations + tests)

## Next Step

Step 4: Implement Severity for CLI errors

## Proposed Solution

Based on my analysis of the codebase, here's my implementation plan:

### Phase 1: Understand Error Variants
I've reviewed all 6 error types:

1. **WorkflowError** (7 variants):
   - NotFound, Invalid, CircularDependency, StateNotFound, InvalidTransition, ExecutionFailed, Timeout

2. **ExecutorError** (9 variants):
   - StateNotFound, InvalidTransition, ValidationFailed, TransitionLimitExceeded, ExecutionFailed, ExpressionError, ActionError, ManualInterventionRequired, Abort

3. **GraphError** (2 variants):
   - CycleDetected, StateNotFound

4. **StateError** (1 variant):
   - EmptyStateId

5. **ActionError** (9 variants):
   - ClaudeError, VariableError, ParseError, ExecutionError, IoError, JsonError, RateLimit, ShellSecurityError

6. **ParseError** (5 variants):
   - MermaidError, WrongDiagramType, NoInitialState, NoTerminalStates, InvalidStructure

### Phase 2: Severity Classification

Following the pattern from SwissArmyHammerError and the guidelines from ideas/severity.md:

**Critical** (System cannot continue):
- WorkflowError: CircularDependency, ExecutionFailed
- ExecutorError: ValidationFailed, TransitionLimitExceeded, ExecutionFailed, Abort
- GraphError: CycleDetected
- ParseError: NoInitialState, NoTerminalStates

**Error** (Operation failed, system can continue):
- WorkflowError: NotFound, Invalid, StateNotFound, InvalidTransition, Timeout
- ExecutorError: StateNotFound, InvalidTransition, ExpressionError, ActionError (from variant), ManualInterventionRequired
- GraphError: StateNotFound
- StateError: EmptyStateId
- ActionError: ClaudeError, VariableError, ParseError, ExecutionError, IoError, JsonError, RateLimit, ShellSecurityError
- ParseError: MermaidError, WrongDiagramType, InvalidStructure

**Warning** (Operation can proceed):
- None identified in these error types

### Phase 3: Implementation Strategy

1. Add imports to each file
2. Implement Severity trait with comprehensive match statements
3. Add unit tests in existing test modules or create new test modules
4. Follow TDD: write tests first, then implementation
5. Run tests incrementally after each implementation

### Phase 4: Testing Strategy

For each error type:
1. Create at least one test for each severity level represented
2. Use existing error construction patterns
3. Verify correct severity assignment with assert_eq!
4. Group tests in `severity_tests` module marked with `#[cfg(test)]`

### Implementation Order:
1. StateError (simplest, 1 variant)
2. GraphError (2 variants)
3. ParseError (5 variants)
4. ActionError (9 variants)
5. WorkflowError (7 variants)
6. ExecutorError (9 variants, depends on ActionError)

This bottom-up approach ensures dependencies are handled correctly and allows for incremental testing.
# Step 3: Implement Severity for Workflow Errors

**Refer to ideas/severity.md**

## Goal

Implement the `Severity` trait for all workflow-related error types in swissarmyhammer-workflow crate.

## Context

The workflow crate contains several error types that need severity implementations:
- WorkflowError
- ExecutorError  
- GraphError
- StateError
- ActionError
- ParseError

## Tasks

### 1. Add swissarmyhammer-common Dependency

Ensure `swissarmyhammer-workflow/Cargo.toml` depends on swissarmyhammer-common:

```toml
[dependencies]
swissarmyhammer-common = { path = "../swissarmyhammer-common" }
```

### 2. Implement Severity for WorkflowError

In `swissarmyhammer-workflow/src/error.rs`:

```rust
use swissarmyhammer_common::{ErrorSeverity, Severity};

impl Severity for WorkflowError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            WorkflowError::InvalidState { .. } => ErrorSeverity::Critical,
            WorkflowError::TransitionFailed { .. } => ErrorSeverity::Critical,
            // Add other variants with appropriate severity
            _ => ErrorSeverity::Error,
        }
    }
}
```

### 3. Implement Severity for ExecutorError

In `swissarmyhammer-workflow/src/executor/mod.rs`:

```rust
impl Severity for ExecutorError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            ExecutorError::WorkflowFailed { .. } => ErrorSeverity::Critical,
            ExecutorError::ActionFailed { .. } => ErrorSeverity::Error,
            // Add other variants
            _ => ErrorSeverity::Error,
        }
    }
}
```

### 4. Implement Severity for Other Error Types

Implement for:
- `GraphError` in `swissarmyhammer-workflow/src/graph.rs`
- `StateError` in `swissarmyhammer-workflow/src/state.rs`
- `ActionError` in `swissarmyhammer-workflow/src/actions.rs`
- `ParseError` in `swissarmyhammer-workflow/src/parser.rs`

### 5. Add Tests

For each implementation, add unit tests verifying severity assignments:

```rust
#[cfg(test)]
mod severity_tests {
    use super::*;
    use swissarmyhammer_common::{ErrorSeverity, Severity};

    #[test]
    fn test_workflow_error_severity() {
        let error = WorkflowError::InvalidState { /* fields */ };
        assert_eq!(error.severity(), ErrorSeverity::Critical);
        
        // Test other variants
    }
}
```

## Severity Guidelines for Workflow Errors

**Critical**:
- Workflow execution failures
- Invalid state transitions
- Data corruption
- Unable to recover workflow state

**Error**:
- Action failures (can retry)
- Invalid configurations
- Missing resources

**Warning**:
- Deprecated workflow features
- Performance issues
- Optional features unavailable

## Acceptance Criteria

- [x] All 6 workflow error types implement Severity
- [x] Each error variant has appropriate severity
- [x] Unit tests for each implementation
- [x] Tests pass: `cargo test -p swissarmyhammer-workflow`
- [x] Code compiles: `cargo build -p swissarmyhammer-workflow`
- [x] Clippy clean: `cargo clippy -p swissarmyhammer-workflow`

## Files to Modify

- `swissarmyhammer-workflow/Cargo.toml` (add dependency)
- `swissarmyhammer-workflow/src/error.rs`
- `swissarmyhammer-workflow/src/executor/mod.rs`
- `swissarmyhammer-workflow/src/graph.rs`
- `swissarmyhammer-workflow/src/state.rs`
- `swissarmyhammer-workflow/src/actions.rs`
- `swissarmyhammer-workflow/src/parser.rs`

## Estimated Changes

~150 lines of code (6 implementations + tests)

## Next Step

Step 4: Implement Severity for CLI errors

## Proposed Solution

Based on my analysis of the codebase, here's my implementation plan:

### Phase 1: Understand Error Variants
I've reviewed all 6 error types:

1. **WorkflowError** (7 variants):
   - NotFound, Invalid, CircularDependency, StateNotFound, InvalidTransition, ExecutionFailed, Timeout

2. **ExecutorError** (9 variants):
   - StateNotFound, InvalidTransition, ValidationFailed, TransitionLimitExceeded, ExecutionFailed, ExpressionError, ActionError, ManualInterventionRequired, Abort

3. **GraphError** (2 variants):
   - CycleDetected, StateNotFound

4. **StateError** (1 variant):
   - EmptyStateId

5. **ActionError** (9 variants):
   - ClaudeError, VariableError, ParseError, ExecutionError, IoError, JsonError, RateLimit, ShellSecurityError

6. **ParseError** (5 variants):
   - MermaidError, WrongDiagramType, NoInitialState, NoTerminalStates, InvalidStructure

### Phase 2: Severity Classification

Following the pattern from SwissArmyHammerError and the guidelines from ideas/severity.md:

**Critical** (System cannot continue):
- WorkflowError: CircularDependency, ExecutionFailed
- ExecutorError: ValidationFailed, TransitionLimitExceeded, ExecutionFailed, Abort
- GraphError: CycleDetected
- ParseError: NoInitialState, NoTerminalStates

**Error** (Operation failed, system can continue):
- WorkflowError: NotFound, Invalid, StateNotFound, InvalidTransition, Timeout
- ExecutorError: StateNotFound, InvalidTransition, ExpressionError, ActionError (from variant), ManualInterventionRequired
- GraphError: StateNotFound
- StateError: EmptyStateId
- ActionError: ClaudeError, VariableError, ParseError, ExecutionError, IoError, JsonError, RateLimit, ShellSecurityError
- ParseError: MermaidError, WrongDiagramType, InvalidStructure

**Warning** (Operation can proceed):
- None identified in these error types

### Phase 3: Implementation Strategy

1. Add imports to each file
2. Implement Severity trait with comprehensive match statements
3. Add unit tests in existing test modules or create new test modules
4. Follow TDD: write tests first, then implementation
5. Run tests incrementally after each implementation

### Phase 4: Testing Strategy

For each error type:
1. Create at least one test for each severity level represented
2. Use existing error construction patterns
3. Verify correct severity assignment with assert_eq!
4. Group tests in `severity_tests` module marked with `#[cfg(test)]`

### Implementation Order:
1. StateError (simplest, 1 variant)
2. GraphError (2 variants)
3. ParseError (5 variants)
4. ActionError (9 variants)
5. WorkflowError (7 variants)
6. ExecutorError (9 variants, depends on ActionError)

This bottom-up approach ensures dependencies are handled correctly and allows for incremental testing.

## Implementation Notes

### Completed Implementation

All 6 error types have been successfully implemented with Severity trait:

1. **StateError** (swissarmyhammer-workflow/src/state.rs:45-53)
   - EmptyStateId → Error
   - Test: state.rs:270-274

2. **GraphError** (swissarmyhammer-workflow/src/graph.rs:25-35)
   - CycleDetected → Critical (prevents proper execution)
   - StateNotFound → Error
   - Test: graph.rs:399-406

3. **ParseError** (swissarmyhammer-workflow/src/parser.rs:51-64)
   - NoInitialState → Critical (missing required structure)
   - NoTerminalStates → Critical (missing required structure)
   - MermaidError, WrongDiagramType, InvalidStructure → Error
   - Test: parser.rs:1472-1494

4. **ActionError** (swissarmyhammer-workflow/src/actions.rs:146-161)
   - All 8 variants → Error (all are recoverable operation failures)
   - Test: actions.rs:3352-3381

5. **WorkflowError** (swissarmyhammer-workflow/src/error.rs:66-81)
   - CircularDependency, ExecutionFailed → Critical
   - NotFound, Invalid, StateNotFound, InvalidTransition, Timeout → Error
   - Test: error.rs:83-130

6. **ExecutorError** (swissarmyhammer-workflow/src/executor/mod.rs:58-75)
   - ValidationFailed, TransitionLimitExceeded, ExecutionFailed, Abort → Critical
   - StateNotFound, InvalidTransition, ExpressionError, ActionError, ManualInterventionRequired → Error
   - Test: executor/mod.rs:135-172

### Test Results

```
✓ All 501 tests passed (1.543s)
✓ Clippy: No warnings
✓ Build: Success
```

### Key Decisions

1. **ActionError severity**: All variants classified as Error (not Critical) because action failures are recoverable - workflows can retry or handle them gracefully.

2. **ParseError critical variants**: NoInitialState and NoTerminalStates are Critical because they represent fundamental structural problems that make a workflow unusable.

3. **ExecutorError::Abort**: Classified as Critical because workflow abortion requires immediate attention and system cannot continue the workflow.

4. **Comprehensive testing**: Each test covers all variants of its error type to ensure complete coverage and prevent future regressions.

### Files Modified

- swissarmyhammer-workflow/src/state.rs (+9 lines impl, +6 lines test)
- swissarmyhammer-workflow/src/graph.rs (+12 lines impl, +9 lines test)  
- swissarmyhammer-workflow/src/parser.rs (+15 lines impl, +24 lines test)
- swissarmyhammer-workflow/src/actions.rs (+18 lines impl, +32 lines test)
- swissarmyhammer-workflow/src/error.rs (+17 lines impl, +48 lines test)
- swissarmyhammer-workflow/src/executor/mod.rs (+20 lines impl, +38 lines test)

**Total**: ~248 lines added (implementations + comprehensive tests)