# Create swissarmyhammer-prompts Domain Crate

## Overview
Extract prompt functionality from the main `swissarmyhammer` crate into a dedicated domain crate `swissarmyhammer-prompts`, following the pattern established by other domain crates. The prompt system should depend on `swissarmyhammer-templating` for template processing.

## Current State
The prompt functionality currently exists in:
- `swissarmyhammer/src/prompts.rs` - Core prompt system (~67k lines)
- `swissarmyhammer/src/prompt_resolver.rs` - Prompt resolution logic (~12k lines)
- `swissarmyhammer/src/prompt_filter.rs` - Prompt filtering (~9.5k lines)
- Used extensively by `swissarmyhammer-tools` for MCP prompt operations

## Evidence of Current Dependencies in swissarmyhammer-tools
```rust
// These imports should be ELIMINATED:
use swissarmyhammer::{PromptLibrary, PromptResolver};
use swissarmyhammer::prompts::Prompt;

// Found in these specific locations:
- src/mcp/error_handling.rs:4 (PromptLibrary, PromptResolver)
- src/mcp/tests.rs:12 (prompts::Prompt)
- src/mcp/tests.rs:13 (PromptLibrary)
- src/mcp/file_watcher.rs:7 (PromptResolver)
- src/mcp/server.rs:16 (PromptLibrary, PromptResolver)
- src/lib.rs:26 (documentation comment)
```

## Goals
1. Create new `swissarmyhammer-prompts` domain crate with clean boundaries
2. Move all prompt-related code from main crate to the new domain crate
3. Use `swissarmyhammer-templating` for all template processing
4. Update `swissarmyhammer-tools` to depend on prompt domain crate instead of main crate
5. Remove prompt code from main crate when complete
6. Reduce dependencies of `swissarmyhammer-tools` on main `swissarmyhammer` crate

## Implementation Plan

### Phase 1: Create New Crate Structure
- [ ] Create `swissarmyhammer-prompts/` directory
- [ ] Set up `Cargo.toml` with appropriate dependencies:
  - `swissarmyhammer-common` for error types
  - `swissarmyhammer-templating` for template processing
  - Other necessary dependencies
- [ ] Create initial crate structure (`src/lib.rs`, etc.)

### Phase 2: Move Core Prompt Functionality
- [ ] Move `PromptLibrary` from `swissarmyhammer/src/prompts.rs`
- [ ] Move `Prompt` types and implementations
- [ ] Move `PromptResolver` from `swissarmyhammer/src/prompt_resolver.rs`
- [ ] Move prompt filtering from `swissarmyhammer/src/prompt_filter.rs`
- [ ] Move prompt loading and caching logic
- [ ] Move prompt validation and error handling

### Phase 3: Integrate with swissarmyhammer-templating
- [ ] Replace internal template processing with swissarmyhammer-templating
- [ ] Update prompt rendering to use templating domain crate
- [ ] Ensure template context and variable substitution works
- [ ] Remove any duplicate templating logic from prompt code
- [ ] Test template integration thoroughly

### Phase 4: Handle Dependencies and Errors
- [ ] Move prompt-specific error types to new domain crate
- [ ] Ensure proper conversion to common error types
- [ ] Set up dependency chain: `swissarmyhammer-prompts` → `swissarmyhammer-templating` → `swissarmyhammer-common`
- [ ] Avoid circular dependencies with main crate

### Phase 5: Update swissarmyhammer-tools Dependencies
- [ ] Add `swissarmyhammer-prompts` dependency to `swissarmyhammer-tools/Cargo.toml`
- [ ] Update imports in swissarmyhammer-tools:
  ```rust
  // FROM:
  use swissarmyhammer::{PromptLibrary, PromptResolver};
  use swissarmyhammer::prompts::Prompt;
  
  // TO:
  use swissarmyhammer_prompts::{PromptLibrary, PromptResolver, Prompt};
  ```
- [ ] Update all affected files:
  - `src/mcp/error_handling.rs:4`
  - `src/mcp/tests.rs:12, 13`
  - `src/mcp/file_watcher.rs:7`
  - `src/mcp/server.rs:16`
  - `src/lib.rs:26` (documentation)
- [ ] Verify all prompt-related functionality still works

### Phase 6: Clean Up Main Crate
- [ ] Remove `swissarmyhammer/src/prompts.rs` 
- [ ] Remove `swissarmyhammer/src/prompt_resolver.rs`
- [ ] Remove `swissarmyhammer/src/prompt_filter.rs`
- [ ] Update `swissarmyhammer/src/lib.rs` to remove prompt module exports
- [ ] Remove any prompt-related dependencies from main crate if no longer needed
- [ ] Update any remaining references in main crate

### Phase 7: Update Main Crate Integration (if needed)
- [ ] If main crate still needs prompt functionality, add dependency on prompts domain crate
- [ ] Re-export prompt types from main crate for backward compatibility if needed
- [ ] Ensure clean separation between main crate and prompt domain

### Phase 8: Verification
- [ ] Build entire workspace to ensure no breakage
- [ ] Run all tests, especially prompt-related tests
- [ ] Verify prompt loading and resolution still works
- [ ] Test template integration in prompts
- [ ] Ensure MCP prompt operations work correctly
- [ ] Test prompt filtering and library functionality

## Files to Move

### From swissarmyhammer/src/ to swissarmyhammer-prompts/src/
- `prompts.rs` → Core prompt functionality (~67k lines)
- `prompt_resolver.rs` → Prompt resolution logic (~12k lines)
- `prompt_filter.rs` → Prompt filtering (~9.5k lines)
- Any prompt-related utilities and helpers
- Prompt-related error types and handling

### swissarmyhammer-tools Updates
- `src/mcp/error_handling.rs` - Update prompt imports
- `src/mcp/tests.rs` - Update prompt imports
- `src/mcp/file_watcher.rs` - Update PromptResolver import
- `src/mcp/server.rs` - Update prompt imports
- `src/lib.rs` - Update documentation references

## COMPLETION CRITERIA - How to Know This Issue is REALLY Done

**This issue is complete when the following imports NO LONGER EXIST in swissarmyhammer-tools:**

```rust
// These 5+ imports should be ELIMINATED:
use swissarmyhammer::{PromptLibrary, PromptResolver};
use swissarmyhammer::prompts::Prompt;

// Found in these specific locations:
- src/mcp/error_handling.rs:4
- src/mcp/tests.rs:12  
- src/mcp/tests.rs:13
- src/mcp/file_watcher.rs:7
- src/mcp/server.rs:16
```

**And replaced with:**
```rust
use swissarmyhammer_prompts::{PromptLibrary, PromptResolver, Prompt};
```

**Verification Command:**
```bash
# Should return ZERO results when done:
rg "use swissarmyhammer::(.*)?PromptLibrary|PromptResolver" swissarmyhammer-tools/
rg "use swissarmyhammer::prompts" swissarmyhammer-tools/

# Should find new imports:
rg "use swissarmyhammer_prompts" swissarmyhammer-tools/
```

**Expected Impact:**
- **Current**: 9 imports from main crate
- **After completion**: ~4 imports from main crate (5 prompt imports eliminated)

## Success Criteria
- [ ] `swissarmyhammer-prompts` crate exists and compiles independently
- [ ] Uses `swissarmyhammer-templating` for all template processing
- [ ] `swissarmyhammer-tools` uses the new prompt domain crate
- [ ] Prompt-related code no longer exists in main crate
- [ ] All prompt functionality preserved and working
- [ ] All tests pass
- [ ] Domain boundaries are clean and well-defined
- [ ] Reduced coupling between swissarmyhammer-tools and main crate

## Dependencies

### This Issue Depends On:
- **Templating Domain Crate** (`01K4MWFPC5RRZN82F5R6YTAM8K`) - Prompts need templating functionality

### This Issue Enables:
- Major reduction in swissarmyhammer-tools dependencies on main crate
- Clean separation between prompt and main crate functionality
- Reusable prompt system for other projects

## Risk Mitigation
- Prompt system is complex - test thoroughly after migration
- Template integration is critical - ensure no regressions
- Prompt loading and resolution must work identically
- Keep git commits granular for easy rollback
- Test prompt filtering and library functionality extensively

## Benefits
- **Domain Separation**: Clean boundaries between prompt and main functionality
- **Reduced Coupling**: Tools don't need main crate for prompt operations
- **Better Maintainability**: Prompt domain can be maintained independently
- **Template Integration**: Proper use of templating domain crate
- **Reusability**: Prompt system can be used by other projects

## Notes
This is a major domain extraction that will significantly reduce coupling. The prompt system is large (~88k lines across 3 files) but well-defined, making it a good candidate for domain separation.

Once complete, this will eliminate 5+ import dependencies from swissarmyhammer-tools to the main crate, bringing us much closer to full domain separation.

The integration with swissarmyhammer-templating is crucial - prompts should use the templating domain for all template processing rather than having their own template logic.