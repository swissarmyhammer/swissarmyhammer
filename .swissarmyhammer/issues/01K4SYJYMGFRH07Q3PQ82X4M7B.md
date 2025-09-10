# Complete Prompt System Migration - Add swissarmyhammer-prompts Dependency and Update Imports

## Problem
swissarmyhammer-tools has 4 remaining imports from the main crate that should use the `swissarmyhammer-prompts` domain crate instead. The prompts crate has been properly extracted (faithfully, not rewritten), but swissarmyhammer-tools is missing the dependency.

## Current State - 4 Remaining Dependencies

### **All Remaining Dependencies are Prompt-Related:**

#### **src/mcp/error_handling.rs:4**
```rust
use swissarmyhammer::{PromptLibrary, PromptResolver};
```

#### **src/mcp/tests.rs:12**
```rust
use swissarmyhammer::{Prompt, PromptLibrary, PromptResolver};
```

#### **src/mcp/file_watcher.rs:8**
```rust
use swissarmyhammer::PromptResolver;
```

#### **src/mcp/server.rs:12**
```rust
use swissarmyhammer::{PromptLibrary, PromptResolver};
```

## Root Cause Analysis
- ✅ **swissarmyhammer-prompts domain crate exists** and is properly extracted
- ✅ **Workspace includes prompts crate** in Cargo.toml members
- ❌ **swissarmyhammer-tools missing dependency** on prompts crate
- ❌ **Imports still point to main crate** instead of domain crate

## Implementation Plan

### Phase 1: Add Missing Dependency
- [ ] Add to `swissarmyhammer-tools/Cargo.toml`:
  ```toml
  swissarmyhammer-prompts = { path = "../swissarmyhammer-prompts" }
  ```

### Phase 2: Update All 4 Import Statements

#### **Update src/mcp/error_handling.rs:4**
- [ ] Change:
  ```rust
  // FROM: use swissarmyhammer::{PromptLibrary, PromptResolver};
  // TO:   use swissarmyhammer_prompts::{PromptLibrary, PromptResolver};
  ```

#### **Update src/mcp/tests.rs:12**
- [ ] Change:
  ```rust
  // FROM: use swissarmyhammer::{Prompt, PromptLibrary, PromptResolver};
  // TO:   use swissarmyhammer_prompts::{Prompt, PromptLibrary, PromptResolver};
  ```

#### **Update src/mcp/file_watcher.rs:8**
- [ ] Change:
  ```rust
  // FROM: use swissarmyhammer::PromptResolver;
  // TO:   use swissarmyhammer_prompts::PromptResolver;
  ```

#### **Update src/mcp/server.rs:12**
- [ ] Change:
  ```rust
  // FROM: use swissarmyhammer::{PromptLibrary, PromptResolver};
  // TO:   use swissarmyhammer_prompts::{PromptLibrary, PromptResolver};
  ```

### Phase 3: Build and Test Verification
- [ ] Build swissarmyhammer-tools to ensure no compilation errors
- [ ] Run tests to verify prompt functionality still works identically
- [ ] Test MCP server prompt operations
- [ ] Verify prompt loading, filtering, and resolution work correctly

## COMPLETION CRITERIA - How to Know This Issue is REALLY Done

**This issue is complete when:**

```bash
# Should return ZERO results (complete domain separation):
rg "use swissarmyhammer::" swissarmyhammer-tools/

# Should find new imports from prompts domain crate:
rg "use swissarmyhammer_prompts::" swissarmyhammer-tools/

# Should have prompts dependency in Cargo.toml:
rg "swissarmyhammer-prompts" swissarmyhammer-tools/Cargo.toml
```

**Target**: 0 dependencies from swissarmyhammer-tools to main crate
**Current**: 4 dependencies (all prompt-related)

## Expected Impact
- **Before**: 4 imports from main crate
- **After**: 0 imports from main crate  
- **Achievement**: **100% complete domain separation**

## Files to Update

### Cargo.toml
- `swissarmyhammer-tools/Cargo.toml` - Add swissarmyhammer-prompts dependency

### Import Updates
- `src/mcp/error_handling.rs` - Update prompt imports
- `src/mcp/tests.rs` - Update prompt imports
- `src/mcp/file_watcher.rs` - Update prompt imports  
- `src/mcp/server.rs` - Update prompt imports

## Success Criteria
- [ ] swissarmyhammer-prompts dependency added to tools Cargo.toml
- [ ] All 4 prompt imports updated to use domain crate
- [ ] Build succeeds without compilation errors
- [ ] All tests pass
- [ ] Prompt functionality works identically to before
- [ ] **COMPLETE DOMAIN SEPARATION ACHIEVED**

## Strategic Significance
This represents the **final step** to achieve 100% domain separation between swissarmyhammer-tools and the main crate. Once complete:
- ✅ **Complete architectural independence** 
- ✅ **Zero coupling** between tools and main crate
- ✅ **Clean domain boundaries** across entire system

## Risk Assessment: LOW
- The prompts extraction was done faithfully (not rewritten)
- Domain crate exports all needed types
- Should be simple dependency and import update
- Low risk of breakage since extraction preserved functionality

## Notes
Unlike the workflow disaster, the prompts domain crate was properly extracted with preserved functionality. This should be a straightforward completion of the migration by adding the missing dependency and updating imports.