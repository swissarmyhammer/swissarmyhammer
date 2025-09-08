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
## Proposed Solution

After analyzing the current outline code structure, I propose the following implementation approach:

### 1. New Crate Structure
- Create `swissarmyhammer-outline/` with standard domain crate layout
- Minimal dependencies: tree-sitter parsers, serde, thiserror, chrono, ulid
- Clean separation from main crate's complex dependency tree

### 2. Core Components to Move
- **Types**: All outline types from `types.rs` including `OutlineNode`, `OutlineNodeType`, etc.
- **Parser**: Tree-sitter integration and language detection
- **Extractors**: Language-specific symbol extraction (rust, python, typescript, javascript, dart)
- **File Discovery**: Glob pattern matching and file filtering
- **Hierarchy Builder**: Organizing symbols into hierarchical structures  
- **Formatters**: YAML and other output formatting
- **Utilities**: Helper functions and configuration

### 3. Dependencies Strategy
- Tree-sitter parsers: Move to new crate
- Language detection: Extract from search crate or duplicate minimal logic
- Error handling: New domain-specific error types
- File system operations: Standard library + walkdir/ignore crates

### 4. API Design
- Clean public API following other domain crates
- Re-export main types and builders at crate root
- Maintain backward compatibility during transition

### 5. Migration Steps
1. Create new crate structure with Cargo.toml
2. Move and adapt core types and error handling
3. Move file discovery and parser functionality
4. Move language-specific extractors
5. Update swissarmyhammer-tools imports
6. Remove old code from main crate
7. Test and verify functionality

This approach follows the established pattern of other domain crates and creates clear separation of concerns.