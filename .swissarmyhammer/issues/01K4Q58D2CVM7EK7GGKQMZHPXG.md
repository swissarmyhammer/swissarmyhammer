# Move file_loader to swissarmyhammer-common for Shared Usage

## Problem
The `file_loader` functionality in `swissarmyhammer/src/file_loader.rs` is a cross-cutting concern used by both prompt and workflow systems, but it's currently in the main crate. This prevents clean domain separation since both future `swissarmyhammer-prompts` and `swissarmyhammer-workflow` domain crates will need file loading capabilities.

## Current State Analysis

### **file_loader Usage Evidence:**
- **Prompts**: `swissarmyhammer/src/prompt_resolver.rs:1: use crate::file_loader::{FileSource, VirtualFileSystem};`
- **Workflows**: `swissarmyhammer/src/workflow/storage.rs:3: use crate::file_loader::{FileSource, VirtualFileSystem};`
- **Workflow Domain Crate**: `swissarmyhammer-workflow/src/storage.rs:3: use swissarmyhammer::file_loader::{FileSource, VirtualFileSystem};`
- **Main Crate Exports**: Re-exports as `FileSource` and `PromptSource`

### **Architecture Problem:**
- `file_loader.rs` (~20k lines) is infrastructure needed by multiple domains
- Prompts need file loading for prompt directory scanning
- Workflows need file loading for workflow file management
- Currently blocks domain extractions since both need this shared functionality

## Proposed Solution
Move `file_loader` to `swissarmyhammer-common` where it can be shared by:
- Future `swissarmyhammer-prompts` domain crate
- `swissarmyhammer-workflow` domain crate  
- Any other components that need file loading infrastructure

## Implementation Plan

### Phase 1: Move file_loader to swissarmyhammer-common
- [ ] Move `swissarmyhammer/src/file_loader.rs` to `swissarmyhammer-common/src/file_loader.rs`
- [ ] Add exports to `swissarmyhammer-common/src/lib.rs`:
  ```rust
  pub mod file_loader;
  pub use file_loader::{FileSource, VirtualFileSystem};
  ```
- [ ] Ensure all file loading functionality is preserved
- [ ] Add any necessary dependencies to swissarmyhammer-common

### Phase 2: Update swissarmyhammer-workflow Domain Crate
- [ ] Update import in `swissarmyhammer-workflow/src/storage.rs:3`:
  ```rust
  // FROM: use swissarmyhammer::file_loader::{FileSource, VirtualFileSystem};
  // TO:   use swissarmyhammer_common::file_loader::{FileSource, VirtualFileSystem};
  ```
- [ ] Add `swissarmyhammer-common` dependency to `swissarmyhammer-workflow/Cargo.toml`
- [ ] Verify workflow storage functionality still works

### Phase 3: Update Main Crate Usage
- [ ] Update `swissarmyhammer/src/prompt_resolver.rs:1`:
  ```rust
  // FROM: use crate::file_loader::{FileSource, VirtualFileSystem};
  // TO:   use swissarmyhammer_common::file_loader::{FileSource, VirtualFileSystem};
  ```
- [ ] Update `swissarmyhammer/src/workflow/storage.rs:3`:
  ```rust
  // FROM: use crate::file_loader::{FileSource, VirtualFileSystem};
  // TO:   use swissarmyhammer_common::file_loader::{FileSource, VirtualFileSystem};
  ```
- [ ] Add `swissarmyhammer-common` dependency to main crate if not already present

### Phase 4: Update Main Crate Exports
- [ ] Update `swissarmyhammer/src/lib.rs` exports:
  ```rust
  // FROM: pub use file_loader::FileSource;
  //       pub use file_loader::FileSource as PromptSource;
  // TO:   pub use swissarmyhammer_common::file_loader::FileSource;
  //       pub use swissarmyhammer_common::file_loader::FileSource as PromptSource;
  ```
- [ ] Or remove re-exports if no longer needed for backward compatibility

### Phase 5: Remove Original file_loader from Main Crate
- [ ] Delete `swissarmyhammer/src/file_loader.rs` (20k lines)
- [ ] Remove file_loader module from main crate
- [ ] Clean up any unused imports

### Phase 6: Enable Future Domain Extractions
- [ ] Verify `swissarmyhammer-common` file_loader can support future prompts domain crate
- [ ] Verify `swissarmyhammer-common` file_loader can support workflow domain crate
- [ ] Ensure clean dependency chain: domains → common (no circular dependencies)

### Phase 7: Verification
- [ ] Build entire workspace to ensure no breakage
- [ ] Run all tests to verify file loading still works
- [ ] Test prompt loading functionality
- [ ] Test workflow file loading functionality
- [ ] Ensure no regressions in file system operations

## COMPLETION CRITERIA - How to Know This Issue is REALLY Done

**This issue is complete when:**

1. **`swissarmyhammer/src/file_loader.rs` no longer exists**
2. **All file_loader imports use swissarmyhammer-common:**
   ```bash
   # Should return ZERO results:
   rg "use.*::file_loader|use crate::file_loader" swissarmyhammer/
   
   # Should find common crate imports:
   rg "use swissarmyhammer_common::file_loader" swissarmyhammer/
   rg "use swissarmyhammer_common::file_loader" swissarmyhammer-workflow/
   ```

## Expected Impact
- **Move 20k lines** of infrastructure code to appropriate common crate
- **Enable future domain extractions** for prompts and workflow
- **Create shared file loading infrastructure**
- **Reduce main crate size** significantly

## Strategic Importance

### This Move Enables:
- **swissarmyhammer-prompts domain crate** - Can use common file loading
- **swissarmyhammer-workflow domain crate** - Can use common file loading
- **Clean domain separation** - No shared infrastructure in main crate

### Dependency Chain After Move:
```
swissarmyhammer-prompts → swissarmyhammer-common (file_loader)
swissarmyhammer-workflow → swissarmyhammer-common (file_loader)  
```

Instead of both needing the main crate for file loading.

## Files to Move/Update

### Move to swissarmyhammer-common
- `swissarmyhammer/src/file_loader.rs` → `swissarmyhammer-common/src/file_loader.rs`

### Update Imports
- `swissarmyhammer/src/prompt_resolver.rs` - Update import
- `swissarmyhammer/src/workflow/storage.rs` - Update import  
- `swissarmyhammer-workflow/src/storage.rs` - Update import
- `swissarmyhammer/src/lib.rs` - Update re-exports

## Success Criteria
- [ ] file_loader moved to swissarmyhammer-common
- [ ] All consumers use file_loader from common crate
- [ ] Prompts and workflows can use shared file loading infrastructure
- [ ] Foundation ready for future domain extractions
- [ ] No duplicate file loading code
- [ ] Workspace builds and tests pass

## Benefits
- **Shared Infrastructure**: File loading available to all domain crates
- **Enables Domain Separation**: Removes blocker for prompts/workflow extraction
- **Cleaner Architecture**: Infrastructure in common crate where it belongs
- **Reduced Duplication**: Single file loading implementation

## Notes
File loading is classic infrastructure that belongs in the common crate. Both prompts and workflows need to load files from directories, making this a perfect shared utility.

Moving file_loader to swissarmyhammer-common is a prerequisite for clean extraction of both prompt and workflow domain crates, since both will need file loading capabilities without depending on the main crate.