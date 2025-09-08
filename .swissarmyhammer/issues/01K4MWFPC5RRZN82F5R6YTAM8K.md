# Create swissarmyhammer-templating Domain Crate

## Overview
Extract templating functionality from the main `swissarmyhammer` crate into a dedicated domain crate `swissarmyhammer-templating`. Both the prompt system and workflow system depend heavily on configured templating, making this a critical foundational dependency for future domain separations.

## Current State
The templating functionality currently exists in:
- `swissarmyhammer/src/template.rs` - Core templating system using Liquid
- Used extensively by prompt system (`PromptLibrary`, `PromptResolver`)
- Used extensively by workflow system for template rendering
- Creates a major coupling point preventing prompt and workflow domain extraction

## Evidence of Templating Dependencies

### Prompt System Dependencies:
```rust
use swissarmyhammer::{PromptLibrary, PromptResolver};
use swissarmyhammer::prompts::Prompt;
```

### Workflow System Dependencies:
```rust
use swissarmyhammer::workflow::{...}
```

Both systems rely heavily on the templating engine for:
- Liquid template processing
- Context variable substitution  
- Template inheritance and includes
- Dynamic content generation

## Goals
1. Create `swissarmyhammer-templating` domain crate as foundational infrastructure
2. Move all templating logic from main crate to the new domain crate
3. Enable prompt and workflow systems to depend on templating domain instead of main crate
4. Provide foundation for future prompt and workflow domain extractions
5. Reduce coupling of swissarmyhammer-tools to main crate

## Implementation Plan

### Phase 1: Create New Crate Structure
- [ ] Create `swissarmyhammer-templating/` directory
- [ ] Set up `Cargo.toml` with appropriate dependencies (liquid, etc.)
- [ ] Create initial crate structure (`src/lib.rs`, etc.)
- [ ] Determine minimal dependencies for templating functionality

### Phase 2: Move Core Templating Functionality
- [ ] Move template processing from `swissarmyhammer/src/template.rs`
- [ ] Move Liquid template engine integration
- [ ] Move template context handling and variable substitution
- [ ] Move template inheritance and include functionality
- [ ] Move template caching and optimization
- [ ] Move template error handling and validation

### Phase 3: Define Clean Domain Interface
- [ ] Create public API for template rendering
- [ ] Define template context types and builders
- [ ] Create template loading and caching interfaces
- [ ] Define error types specific to templating
- [ ] Ensure minimal surface area for domain boundaries

### Phase 4: Handle Dependencies
- [ ] Move templating-specific error types to new domain crate
- [ ] Set up dependency chain: `swissarmyhammer-templating` → `swissarmyhammer-common`
- [ ] Ensure liquid and template dependencies are properly contained
- [ ] Avoid circular dependencies with main crate

### Phase 5: Update Main Crate to Use Domain Crate
- [ ] Add `swissarmyhammer-templating` dependency to main `Cargo.toml`
- [ ] Update prompt system to use templating domain crate
- [ ] Update workflow system to use templating domain crate  
- [ ] Update imports and API usage throughout main crate
- [ ] Verify templating functionality still works

### Phase 6: Future Foundation for Domain Extractions
- [ ] Design templating API to support future prompt domain extraction
- [ ] Design templating API to support future workflow domain extraction
- [ ] Ensure swissarmyhammer-tools can use templating domain independently
- [ ] Prepare for prompt and workflow systems to be extracted later

### Phase 7: Clean Up Main Crate
- [ ] Remove `swissarmyhammer/src/template.rs` when safe to do so
- [ ] Update main crate exports to remove templating internals
- [ ] Clean up any templating-related dependencies from main crate if no longer needed
- [ ] Verify main crate no longer contains templating implementation

### Phase 8: Verification
- [ ] Build entire workspace to ensure no breakage
- [ ] Run all tests, especially prompt and workflow tests
- [ ] Verify template rendering still works correctly
- [ ] Test template inheritance and includes functionality
- [ ] Ensure no functionality is lost in the migration

## Files to Move

### From swissarmyhammer/src/ to swissarmyhammer-templating/src/
- `template.rs` → Core templating functionality (reorganize as needed)
- Template-related utilities and helpers
- Template context types and builders
- Template error types and handling

### Dependencies to Consider
- `liquid` - Template engine (already used)
- `liquid-core` - Core liquid functionality  
- Template-related serde functionality
- Context building and variable substitution

## Success Criteria
- [ ] `swissarmyhammer-templating` crate exists and compiles independently
- [ ] Prompt system uses templating domain crate instead of main crate
- [ ] Workflow system uses templating domain crate instead of main crate
- [ ] All template functionality preserved and working
- [ ] Clean domain boundaries with minimal API surface
- [ ] Foundation ready for future prompt/workflow domain extractions
- [ ] Reduced coupling between components

## Strategic Importance

### This Crate Enables Future Extractions:
1. **Prompt Domain Extraction**: Prompts can depend on templating domain
2. **Workflow Domain Extraction**: Workflows can depend on templating domain  
3. **Tools Independence**: swissarmyhammer-tools can use templating directly

### Dependency Chain After Extraction:
```
swissarmyhammer-tools → swissarmyhammer-templating
                    ↗
swissarmyhammer-prompts → swissarmyhammer-templating
                    ↗
swissarmyhammer-workflow → swissarmyhammer-templating
```

Instead of everything depending on the main crate.

## Risk Mitigation
- Template functionality is complex - test thoroughly after migration
- Liquid integration needs to be preserved exactly
- Template context and variable substitution is critical
- Ensure template inheritance and includes work correctly
- Test prompt and workflow rendering extensively

## Notes
Templating is foundational infrastructure that both prompts and workflows depend on heavily. Creating this domain crate is essential for future domain separations of prompt and workflow systems.

This extraction will enable:
- Future `swissarmyhammer-prompts` domain crate
- Future `swissarmyhammer-workflow` domain crate  
- Direct templating usage by swissarmyhammer-tools
- Significant reduction in main crate coupling

Template processing is a well-defined domain that can be cleanly separated while providing the foundation for larger domain extractions.
## COMPLETION CRITERIA - How to Know This Issue is REALLY Done

**This issue is complete when the following imports NO LONGER EXIST in swissarmyhammer-tools:**

```rust
// These 4+ imports should be ELIMINATED:
use swissarmyhammer::{PromptLibrary, PromptResolver};
use swissarmyhammer::prompts::Prompt;

// Found in these specific locations:
- src/mcp/error_handling.rs:4 (PromptLibrary, PromptResolver)
- src/mcp/tests.rs:12 (prompts::Prompt)
- src/mcp/tests.rs:13 (PromptLibrary)  
- src/mcp/server.rs:15 (PromptLibrary, PromptResolver)
- src/lib.rs:26 (comment reference)
```

**And replaced with:**
```rust
use swissarmyhammer_templating::{TemplateEngine, TemplateContext};
use swissarmyhammer_prompts::{PromptLibrary, PromptResolver, Prompt};
```

**Verification Command:**
```bash
# Should return ZERO results when done:
rg "use swissarmyhammer::(.*)?PromptLibrary|PromptResolver" swissarmyhammer-tools/
rg "use swissarmyhammer::prompts" swissarmyhammer-tools/

# Should find new imports:
rg "use swissarmyhammer_templating" swissarmyhammer-tools/
rg "use swissarmyhammer_prompts" swissarmyhammer-tools/
```

**Expected Impact:**
- **Current**: 23 imports from main crate
- **After completion**: ~19 imports from main crate (4+ prompt imports eliminated)

**Note**: This depends on both swissarmyhammer-templating and swissarmyhammer-prompts domain crates being created first.