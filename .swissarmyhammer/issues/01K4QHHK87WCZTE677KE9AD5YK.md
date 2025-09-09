# Consolidate All Test Utilities in swissarmyhammer-common

## Problem
Test utilities are **scattered across multiple domain crates** instead of being centralized in `swissarmyhammer-common`. This violates DRY principles, creates maintenance overhead, and leads to inconsistent test infrastructure across the codebase.

## Evidence of Scattered Test Utilities

### **swissarmyhammer-tools - Multiple Test Utilities**
- `src/test_utils.rs:20` - `create_test_rate_limiter()` (duplicate)
- Various `create_test_context()` functions
- `TestIssueEnvironment` type
- Multiple test configuration functions

### **swissarmyhammer-search - Test Utilities Module**
- `src/test_utils.rs` - **Entire test utilities module**
- `test_utils.rs:18: pub use swissarmyhammer::test_utils::IsolatedTestHome;` - Re-exports from main crate
- `acquire_semantic_db_lock()` function - Database test coordination
- Used by embedding tests and type tests

### **swissarmyhammer-workflow - Test Utilities Scattered**
- `src/actions_tests/common.rs:8: pub fn create_test_context() -> HashMap<String, Value>`
- `src/actions_tests/mod.rs:24: pub fn create_test_context() -> WorkflowTemplateContext`
- `src/actions.rs:40: pub fn set_test_storage(storage: Arc<WorkflowStorage>)`
- `src/actions.rs:48: pub fn clear_test_storage()`
- `src/template_context.rs:63: pub fn with_vars_for_test(vars: HashMap<String, Value>)`
- `src/executor/core.rs:59: pub fn with_test_storage(...)`
- **Multiple domain-specific test utilities**

### **swissarmyhammer-config - Test Configuration**
- `src/agent.rs:226: pub fn for_testing() -> Self`
- Configuration-specific test utilities

### **swissarmyhammer-git - Test Utilities**
- `src/repository.rs:260: fn test_utils_functions()`
- Git-specific test utilities

## Problems with Current Scattered Approach
- ❌ **Code Duplication**: Same utilities implemented multiple times
- ❌ **Inconsistent APIs**: Different naming and interfaces across crates
- ❌ **Maintenance Overhead**: Changes need to be made in multiple places
- ❌ **Import Confusion**: Unclear where to find test utilities
- ❌ **Domain Coupling**: Domain crates importing test utils from main crate

## Proposed Solution
**Consolidate ALL shared test utilities in `swissarmyhammer-common/src/test_utils.rs`** while keeping domain-specific test utilities in their respective crates.

## Implementation Plan

### Phase 1: Identify Shared vs Domain-Specific Test Utilities

#### **Move to swissarmyhammer-common (Shared Infrastructure):**
- [ ] `create_test_rate_limiter()` - Rate limiting for tests
- [ ] `acquire_semantic_db_lock()` - Database coordination
- [ ] `IsolatedTestHome` - File system isolation (already there)
- [ ] `create_isolated_test_home()` - Test environment setup (already there)
- [ ] Any other utilities used by multiple crates

#### **Keep in Domain Crates (Domain-Specific):**
- [ ] Workflow-specific test context creation
- [ ] Git-specific test utilities  
- [ ] Config-specific test utilities
- [ ] Domain-specific test setup functions

### Phase 2: Move Shared Utilities to Common Crate

#### **From swissarmyhammer-search:**
- [ ] Move `acquire_semantic_db_lock()` to `swissarmyhammer-common/src/test_utils.rs`
- [ ] Remove `swissarmyhammer-search/src/test_utils.rs` if it only re-exports
- [ ] Update imports in search crate to use common version

#### **From swissarmyhammer-tools:**
- [ ] Move `create_test_rate_limiter()` to `swissarmyhammer-common/src/test_utils.rs` 
- [ ] Move any other shared test utilities from `swissarmyhammer-tools/src/test_utils.rs`
- [ ] Keep tool-specific utilities in tools crate

### Phase 3: Remove Duplications

#### **Eliminate Duplicate create_test_rate_limiter:**
- [ ] Remove from `swissarmyhammer-tools/src/test_utils.rs:20`
- [ ] Remove from `swissarmyhammer-tools/tests/file_tools_integration_tests.rs:27`
- [ ] Remove from `swissarmyhammer-tools/tests/notify_integration_tests.rs:19`
- [ ] Update all usage to import from common crate

#### **Eliminate Other Duplications:**
- [ ] Check for other duplicate test utilities across crates
- [ ] Centralize shared utilities in common crate
- [ ] Update imports to use centralized versions

### Phase 4: Update Domain Crates to Use Common Test Utils

#### **swissarmyhammer-search:**
- [ ] Update imports from `swissarmyhammer::test_utils` to `swissarmyhammer_common::test_utils`
- [ ] Remove local test_utils.rs if it becomes empty
- [ ] Update test imports throughout search crate

#### **swissarmyhammer-workflow:**
- [ ] Update imports from `swissarmyhammer::test_utils` to `swissarmyhammer_common::test_utils`
- [ ] Keep workflow-specific test utilities in workflow crate
- [ ] Update imports throughout workflow crate

### Phase 5: Clean Up Domain-Specific Test Utilities

#### **Keep Domain-Specific (Don't Move):**
- [ ] **swissarmyhammer-workflow**: `create_test_context()` variations specific to workflows
- [ ] **swissarmyhammer-git**: Git-specific test utilities  
- [ ] **swissarmyhammer-config**: Config-specific test utilities
- [ ] **swissarmyhammer-tools**: Tool-specific test utilities like `TestIssueEnvironment`

### Phase 6: Verification
- [ ] Build entire workspace to ensure no breakage
- [ ] Run all tests to verify test utilities work correctly
- [ ] Test that shared utilities work across all crates
- [ ] Verify domain-specific utilities still work in their crates
- [ ] Ensure no test functionality is lost

## COMPLETION CRITERIA - How to Know This Issue is REALLY Done

**This issue is complete when:**

```bash
# Should return ZERO results (no duplicate test utilities):
rg "fn create_test_rate_limiter" swissarmyhammer-tools/
rg "fn.*test.*util.*fn" swissarmyhammer-search/src/test_utils.rs

# Should find shared utilities in common crate:
rg "create_test_rate_limiter|acquire_semantic_db_lock" swissarmyhammer-common/src/test_utils.rs

# Should find proper imports from common:
rg "use swissarmyhammer_common::test_utils" swissarmyhammer-search/
rg "use swissarmyhammer_common.*test_utils" swissarmyhammer-workflow/
```

## Organizational Principle

### **Shared Test Infrastructure → swissarmyhammer-common**
- Rate limiting for tests
- Database coordination utilities  
- File system isolation
- Cross-cutting test infrastructure

### **Domain-Specific Test Utilities → Domain Crate**
- Workflow test contexts
- Git test utilities
- Tool-specific test environments
- Domain-specific test setup

## Benefits
- **Eliminate Duplication**: Single implementation of shared test utilities
- **Consistent APIs**: Unified interface for common test infrastructure  
- **Easier Maintenance**: Changes in one place for shared utilities
- **Better Organization**: Clear separation between shared and domain-specific
- **Reduced Dependencies**: Domain crates don't import test utils from main crate

## Files to Update

### swissarmyhammer-common/src/test_utils.rs
- Add `create_test_rate_limiter()`
- Add `acquire_semantic_db_lock()`
- Consolidate other shared test utilities

### Remove/Update in Domain Crates
- Remove duplicate implementations
- Update imports to use common crate
- Keep only domain-specific test utilities

## Success Criteria
- [ ] All shared test utilities centralized in swissarmyhammer-common
- [ ] No duplicate test utility implementations
- [ ] Domain crates use common test infrastructure
- [ ] Domain-specific utilities remain in their domains
- [ ] All tests pass with consolidated utilities
- [ ] Clean separation between shared and domain-specific test code

## Notes
This consolidation follows the principle that **shared infrastructure belongs in the common crate** while **domain-specific utilities remain in their domains**.

The current scattered approach creates maintenance overhead and inconsistency. Centralizing shared test utilities in swissarmyhammer-common will provide consistent test infrastructure across all domain crates.