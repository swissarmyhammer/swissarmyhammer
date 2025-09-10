# CLI Integration and Final Cleanup

Refer to /Users/wballard/github/swissarmyhammer/ideas/workflow_move.md

## Objective
Complete the workflow migration by updating the CLI to use the new workflow crate and cleaning up the old implementation.

## Tasks

### Phase 1: CLI Integration
1. **Update CLI Cargo.toml**
   - Add `swissarmyhammer-workflow = { path = "../swissarmyhammer-workflow" }` dependency
   - Remove workflow dependency on main swissarmyhammer if no longer needed

2. **Update CLI Import Paths**
   - Change `use swissarmyhammer::workflow::*` to `use swissarmyhammer_workflow::*`
   - Update all workflow-related imports throughout CLI codebase
   - Test CLI workflow execution functionality

### Phase 2: Main Crate Decision Point
3. **Evaluate Main Crate Dependency**
   - Determine if swissarmyhammer crate needs workflow dependency
   - If yes: Add `swissarmyhammer-workflow` dependency
   - If no: Remove all workflow-related exports

4. **Update Main Crate Imports**
   - If keeping workflow integration: Update re-exports
   - If removing: Clean up lib.rs exports

### Phase 3: Final Cleanup
5. **Remove Original Workflow Directory**
   - Delete `swissarmyhammer/src/workflow/` entirely
   - Remove workflow-specific dependencies from main Cargo.toml

6. **Final System Validation**
   - Build entire workspace: `cargo build`
   - Run all tests: `cargo test`
   - Test CLI workflow functionality end-to-end
   - Verify no broken imports or missing functionality

## Implementation Details

### CLI Import Pattern Changes
```rust
// OLD
use swissarmyhammer::workflow::{
    Workflow, WorkflowExecutor, WorkflowStorage
};

// NEW  
use swissarmyhammer_workflow::{
    Workflow, WorkflowExecutor, WorkflowStorage
};
```

### Success Validation
- CLI can parse workflows
- CLI can execute workflows  
- All workflow commands work
- No import errors
- Complete workspace builds cleanly

## Acceptance Criteria
- [ ] CLI uses new workflow crate dependency
- [ ] All CLI imports updated to new crate
- [ ] CLI workflow functionality works correctly
- [ ] Main crate decision made and implemented
- [ ] Original workflow directory removed
- [ ] Workspace builds and tests pass completely
- [ ] End-to-end workflow execution verified
- [ ] No remaining references to old workflow module

## Migration Complete
This completes the workflow module migration to standalone crate. The workflow functionality now exists as `swissarmyhammer-workflow` with clean separation from the main crate.