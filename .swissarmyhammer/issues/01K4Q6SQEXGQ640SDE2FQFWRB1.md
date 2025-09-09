# Complete swissarmyhammer-workflow Domain Crate Migration Cleanup

## Problem
The most massive incomplete migration has been confirmed. The `swissarmyhammer-workflow` domain crate exists with complete workflow functionality, but **hundreds of thousands of lines** of duplicate code were never removed from the main `swissarmyhammer` crate.

## Evidence of Incomplete Migration

### **Duplicate Workflow Code Found:**

#### **swissarmyhammer/src/workflow/** (51+ files - Should be removed)
**Core Files:**
- `actions.rs` - **131k lines** of workflow actions
- `parser.rs` - **52k lines** of workflow parsing  
- `storage.rs` - **47k lines** of workflow storage
- `action_parser.rs` - **45k lines** of action parsing
- `template_context.rs` - **32k lines** of template context
- `metrics.rs` - **23k lines** of metrics
- `graph_tests.rs` - **21k lines** of graph tests
- `run.rs` - **20k lines** of workflow execution
- `visualization.rs` - **17k lines** of visualization
- `visualization_tests.rs` - **20k lines** of visualization tests
- `test_liquid_rendering.rs` - **17k lines** of template tests
- `examples_tests.rs` - **15k lines** of example tests
- `executor_utils.rs` - **15k lines** of executor utilities
- `error_utils.rs` - **14k lines** of error utilities
- `definition.rs` - **14k lines** of workflow definitions
- `graph.rs` - **12k lines** of graph logic
- `mcp_integration.rs` - **12k lines** of MCP integration
- `metrics_tests.rs` - **10k lines** of metrics tests
- Plus many more files

**Subdirectories:**
- `actions_tests/` - Multiple test files
- `agents/` - Agent implementations  
- `executor/` - Executor implementations

**Total**: **51+ files with hundreds of thousands of lines**

#### **swissarmyhammer-workflow/src/** (Domain crate)
- Complete workflow functionality in organized domain crate
- Equivalent/enhanced versions of main crate workflow code

## Current Problematic State
1. **✅ swissarmyhammer-workflow domain crate** exists and is functional
2. **❌ swissarmyhammer/src/workflow/** still exists with **massive duplicate code**
3. **❌ swissarmyhammer-tools still has 1 workflow import**:
   ```rust
   use swissarmyhammer::workflow::{
   ```
4. **❌ Hundreds of thousands of lines** of duplicate code

## This is the LARGEST Incomplete Migration
- **Scale**: 51+ files, hundreds of thousands of lines
- **Impact**: Main crate is massively bloated with duplicate workflow code
- **Blocking**: Prevents clean workflow domain separation

## Implementation Plan

### Phase 1: Verify Domain Crate Completeness
- [ ] Review `swissarmyhammer-workflow` to ensure it has all functionality from `swissarmyhammer/src/workflow/`
- [ ] Compare massive workflow codebase in main crate to domain crate
- [ ] Identify any missing functionality that needs to be preserved
- [ ] Ensure API compatibility between old and new versions

### Phase 2: Update swissarmyhammer-tools to Use Domain Crate  
- [ ] Update import in `swissarmyhammer-tools/src/mcp/server.rs:12`:
   ```rust
   // FROM: use swissarmyhammer::workflow::{
   // TO:   use swissarmyhammer_workflow::{
   ```
- [ ] Add `swissarmyhammer-workflow` dependency to `swissarmyhammer-tools/Cargo.toml`
- [ ] Verify workflow functionality still works through MCP tools

### Phase 3: Update Main Crate to Use Domain Crate
- [ ] Add `swissarmyhammer-workflow` dependency to main crate `Cargo.toml` 
- [ ] Update any internal workflow usage to use domain crate
- [ ] Re-export workflow types from main crate for backward compatibility if needed

### Phase 4: Remove MASSIVE Duplicate Workflow Code
- [ ] Delete `swissarmyhammer/src/workflow/` directory entirely (**51+ files**):
  - `actions.rs` (131k lines)
  - `parser.rs` (52k lines)
  - `storage.rs` (47k lines)  
  - `action_parser.rs` (45k lines)
  - `template_context.rs` (32k lines)
  - All other workflow files
  - `actions_tests/` subdirectory  
  - `agents/` subdirectory
  - `executor/` subdirectory
- [ ] Update `swissarmyhammer/src/lib.rs` to remove workflow module exports
- [ ] Remove any workflow-related re-exports

### Phase 5: Clean Up Dependencies
- [ ] Remove workflow-related dependencies from main crate if no longer needed
- [ ] Clean up unused imports and dependencies
- [ ] Verify clean dependency chain

### Phase 6: Verification
- [ ] Build entire workspace to ensure no breakage
- [ ] Run all tests to verify workflow functionality still works
- [ ] Test workflow execution through domain crate
- [ ] Ensure no functionality lost in cleanup

## COMPLETION CRITERIA - How to Know This Issue is REALLY Done

**This issue is complete when:**

1. **`swissarmyhammer/src/workflow/` directory no longer exists**
2. **swissarmyhammer-tools import updated:**
   ```bash
   # Should return ZERO results:
   rg "use swissarmyhammer::workflow" swissarmyhammer-tools/
   
   # Directory should not exist:
   ls /Users/wballard/github/sah/swissarmyhammer/src/workflow 2>/dev/null || echo "Directory removed successfully"
   ```

## Expected Impact
- **Eliminate hundreds of thousands of lines** of duplicate workflow code  
- **Massive reduction** in main crate size
- **Complete workflow domain separation**
- **Update dependency count**: 9 → 8 imports (1 workflow import eliminated)

## Notes
This is by far the largest incomplete migration cleanup. The workflow system represents the majority of code in the main crate and should have been completely removed after the domain crate was created.

This cleanup will dramatically reduce the main crate size and complete one of the most significant domain separations.