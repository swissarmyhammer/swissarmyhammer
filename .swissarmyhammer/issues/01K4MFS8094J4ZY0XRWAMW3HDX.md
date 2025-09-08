# Create swissarmyhammer-outline Domain Crate

## Overview
Extract outline generation functionality from the main `swissarmyhammer` crate into a dedicated domain crate `swissarmyhammer-outline`, following the pattern established by other domain crates like `swissarmyhammer-todo`, `swissarmyhammer-issues`, etc.

## Current State
The outline generation logic currently exists in:
- `swissarmyhammer/src/outline/` - Core outline generation functionality
- Used by `swissarmyhammer-tools/src/mcp/tools/outline/generate/mod.rs`

## Goals
1. Create a new `swissarmyhammer-outline` crate with clean domain boundaries
2. Move all outline-related code from `swissarmyhammer/src/outline/` to the new crate
3. Update `swissarmyhammer-tools` to depend on the new domain crate instead of the main crate
4. Remove `swissarmyhammer/src/outline/` entirely when complete
5. Reduce dependencies of `swissarmyhammer-tools` on the main `swissarmyhammer` crate

## Implementation Plan

### Phase 1: Create New Crate Structure
- [ ] Create `swissarmyhammer-outline/` directory
- [ ] Set up `Cargo.toml` with appropriate dependencies
- [ ] Create initial crate structure (`src/lib.rs`, etc.)

### Phase 2: Move Core Functionality  
- [ ] Move `FileDiscovery` from `swissarmyhammer/src/outline/`
- [ ] Move `OutlineParser` and `OutlineParserConfig`
- [ ] Move `HierarchyBuilder` 
- [ ] Move `YamlFormatter` and other formatters
- [ ] Move all outline types (`OutlineNode`, `OutlineNodeType`, etc.)
- [ ] Move any supporting utilities and error types

### Phase 3: Update Dependencies
- [ ] Add `swissarmyhammer-outline` dependency to `swissarmyhammer-tools/Cargo.toml`
- [ ] Update imports in `swissarmyhammer-tools/src/mcp/tools/outline/generate/mod.rs`
- [ ] Remove outline-related imports from main `swissarmyhammer` crate usage

### Phase 4: Clean Up
- [ ] Remove `swissarmyhammer/src/outline/` directory entirely  
- [ ] Update any references in main crate
- [ ] Verify tests pass
- [ ] Update documentation

## Files to Move
Based on the current usage in swissarmyhammer-tools:
- `FileDiscovery::new()` and `FileDiscovery::filter_supported_files()`
- `OutlineParser::new()` and `OutlineParserConfig::default()`
- `HierarchyBuilder::new()`
- `YamlFormatter::with_defaults()`
- `OutlineNode` and `OutlineNodeType` types
- Supporting utilities and configuration

## Success Criteria
- [ ] `swissarmyhammer-outline` crate exists and compiles independently
- [ ] `swissarmyhammer-tools` uses the new domain crate for outline functionality
- [ ] `swissarmyhammer/src/outline/` directory no longer exists
- [ ] All tests pass
- [ ] No functionality is lost in the migration
- [ ] Domain boundaries are clean and well-defined

## Notes
- Follow the same patterns used in `swissarmyhammer-todo`, `swissarmyhammer-issues`, etc.
- Ensure the new crate has minimal external dependencies
- Consider TreeSitter dependencies and how they should be handled
- Make sure error types are properly defined in the new domain