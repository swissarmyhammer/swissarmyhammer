# Properly Extract Workflow Processing - Delete and Redo swissarmyhammer-workflow

## Problem
The existing `swissarmyhammer-workflow` crate is **total shit** and was not a faithful extraction. Instead of moving working code, it was badly rewritten with broken implementations, destroying working functionality. The entire crate needs to be thrown away and redone properly.

## Evidence of Catastrophic Rewrite Instead of Migration

### **What Should Have Been Done:**
- **MOVE** working code from `swissarmyhammer/src/workflow/` to domain crate
- **PRESERVE** all functionality exactly as it was
- **ONLY UPDATE** imports and module structure

### **What Was Actually Done (Disaster):**
- **REWROTE** working chumsky-based action parser with broken regex stub
- **REPLACED** sophisticated parsing with "TEMPORARY IMPLEMENTATION"
- **DESTROYED** `execute prompt` functionality and other working features
- **BROKE** workflow execution that was working before
- **ADDED** "TODO: Restore" comments to previously working code

### **Evidence from swissarmyhammer-workflow:**
- Broken action parser with "simplified stub" implementation
- Missing sophisticated parsing logic
- Broken `execute prompt` handling
- Comments about "temporary implementation"
- Non-functional workflow execution

## Required Solution
**DELETE the entire swissarmyhammer-workflow crate and start over** with proper extraction that preserves working functionality.

## Implementation Plan

### Phase 1: Delete Broken Implementation
- [ ] **DELETE** `swissarmyhammer-workflow/` directory entirely
- [ ] Remove from workspace `Cargo.toml` members
- [ ] Remove any references to the broken workflow crate
- [ ] Clean up any dependencies on the broken implementation

### Phase 2: Verify Original Code Still Works
- [ ] **MANDATORY**: Test that `sah implement` works correctly with original code:
  ```bash
  cargo run -- flow run greeting --var person_name=Bob  # Must work without errors
  ```
- [ ] Test that we can run hello-world and greeting workflows with a unit test
- [ ] Test that `execute prompt "are_issues_complete"` parses correctly
- [ ] Test that workflow state transitions work
- [ ] Test that all workflow functionality is intact in main crate
- [ ] Document what currently works so we know what to preserve

### Phase 3: Create Comprehensive Pre-Extraction Tests
**BEFORE touching any code, create exhaustive tests for ALL workflow functionality:**

- [ ] **Test workflow parsing**: Every action type, every syntax pattern
- [ ] **Test workflow execution**: Complete workflows from start to finish
- [ ] **Test action types**: `log`, `shell`, `prompt`, `execute prompt`, `wait`, `set`, etc.
- [ ] **Test state transitions**: CEL expressions, condition evaluation
- [ ] **Test template context**: Variable substitution, context passing
- [ ] **Test error handling**: Proper error propagation and reporting
- [ ] **Test workflow storage**: Both memory and filesystem backends
- [ ] **Test CLI integration**: All `sah` commands that use workflows

### Phase 4: Faithful Code Extraction (NO REWRITING)
- [ ] **CREATE** new clean `swissarmyhammer-workflow/` crate structure
- [ ] **COPY** every file from `swissarmyhammer/src/workflow/` exactly as-is
- [ ] **PRESERVE** all algorithms, parsing logic, and implementations
- [ ] **MAINTAIN** all comments, TODOs, and code structure
- [ ] **ONLY CHANGE** imports to use appropriate domain crates:
  ```rust
  // ONLY changes allowed:
  use crate::common::thing → use swissarmyhammer_common::thing
  use crate::error → use swissarmyhammer_common::error
  ```

### Phase 5: Minimal Import Updates Only
- [ ] Update imports to use `swissarmyhammer-common` for shared utilities
- [ ] Update imports to use `swissarmyhammer-templating` for template processing
- [ ] **DO NOT** change any logic, algorithms, or implementations
- [ ] **DO NOT** simplify, optimize, or rewrite anything
- [ ] **PRESERVE** exact functionality

### Phase 6: Post-Extraction Verification (MANDATORY)
**ALL pre-extraction tests MUST still pass:**

- [ ] **Test `sah implement` works identically**:
  ```bash
  sah implement  # Must work exactly as before extraction
  ```
- [ ] **ALL workflow parsing tests pass**
- [ ] **ALL workflow execution tests pass**
- [ ] **ALL action type tests pass**
- [ ] **ALL state transition tests pass**
- [ ] **ALL CLI integration tests pass**
- [ ] **ZERO regressions allowed**

### Phase 7: Update swissarmyhammer-tools Integration
- [ ] Update swissarmyhammer-tools to use clean workflow domain crate
- [ ] **ONLY** update imports, no functionality changes
- [ ] Verify MCP server workflow integration works
- [ ] Test that tools can use workflows correctly

### Phase 8: Remove Original Workflow Code (After 100% Verification)
- [ ] **ONLY AFTER** all tests pass and functionality is verified
- [ ] Remove `swissarmyhammer/src/workflow/` from main crate
- [ ] Update main crate to use workflow domain crate
- [ ] **IMMEDIATE ROLLBACK** if anything breaks

## CRITICAL SUCCESS CRITERIA

### **This extraction is ONLY successful if:**

1. **ZERO functionality is lost or changed**
2. **ALL existing workflow features work identically**
3. **ALL CLI commands work exactly as before**
4. **NO "temporary implementations" or broken stubs**
5. **NO behavioral changes whatsoever**

### **MANDATORY Tests Before Declaring Success:**
```bash
# These MUST all work identically to before extraction:
sah implement                           # Core workflow
sah plan                               # Planning workflow
sah flow any-existing-workflow         # Flow command
cargo nextest run                      # All tests pass

# Workflow parsing MUST work:
echo 'execute prompt "test"' | # Should parse correctly

# NO broken implementations allowed:
rg "TEMPORARY IMPLEMENTATION|TODO.*Restore|simplified stub" swissarmyhammer-workflow/
# Should return ZERO results
```

## FAILURE CONDITIONS - IMMEDIATE ROLLBACK

### 🚨 ROLLBACK TRIGGERS (Zero Tolerance):
- **ANY workflow command fails** that worked before
- **ANY test failure** related to workflows
- **ANY parsing errors** for working syntax
- **ANY "temporary implementation" comments**
- **ANY simplified/stub implementations**
- **ANY behavioral changes**

## APPROACH: PRESERVATION NOT REWRITING

### ✅ ALLOWED:
- Copying files exactly as they are
- Updating import statements only
- Adding domain crate exports
- Module structure organization

### ❌ FORBIDDEN (Causes of Previous Disaster):
- Rewriting action parsers
- Simplifying complex logic
- Replacing working code with stubs
- "Improving" algorithms during migration
- Any changes to core functionality
- Any "temporary" implementations

## Verification Strategy

### **Before Starting:**
1. **Document current working state** - what workflows work now
2. **Create comprehensive test suite** covering all functionality
3. **Verify tests pass** with current implementation

### **During Migration:**
1. **Copy, don't rewrite** - preserve exact working code
2. **Test after each file move** to catch breakage immediately
3. **NO LOGIC CHANGES** of any kind

### **After Migration:**
1. **ALL original tests must pass**
2. **ALL functionality must work identically**
3. **IMMEDIATE ROLLBACK** if anything is broken

## Notes
The previous attempt was a **catastrophic failure** because working code was rewritten with broken implementations. This extraction is ONLY about moving working code to the right location while preserving ALL functionality exactly.

**PRINCIPLE**: Preserve working systems above all else. Never break working functionality in the name of "improvement" or "simplification".
