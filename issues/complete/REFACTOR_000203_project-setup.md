# REFACTOR Step 1: Project Setup and Planning

## Overview
Set up the foundation for refactoring MCP tools organization according to the `refactor_tools.md` specification.

## Context
The specification requires:
1. Moving all MCP code into `./mcp/` module folder (currently split between `mcp.rs` and `mcp/`)
2. Organizing tools by noun/verb pattern: `./mcp/tools/memoranda`, `./mcp/tools/issues`, `./mcp/tools/search`
3. Creating individual tool modules with markdown descriptions
4. Building a tool registry to replace the large match statement
5. Making CLI commands use the same tools as MCP server

## Current State Analysis
```
swissarmyhammer/src/
├── mcp.rs                     # Large match statement, needs to move to mcp/mod.rs
├── mcp/                       # Existing submodules
│   ├── tool_handlers.rs       # Needs to be reorganized
│   ├── types.rs              # Issue and memo types
│   ├── memo_types.rs         # Memo-specific types
│   ├── utils.rs              # Utilities
│   └── ...
├── semantic/                  # Search functionality (should be renamed to search)
└── issues/                    # Issue storage logic
```

## Tasks for This Step

### 1. Create New Directory Structure
Create the target directory structure for the refactored tools:

```
swissarmyhammer/src/mcp/
├── mod.rs                     # Main MCP module (move from mcp.rs)
└── tools/
    ├── mod.rs                 # Tool registry
    ├── memoranda/
    │   ├── mod.rs
    │   ├── create/
    │   │   ├── mod.rs
    │   │   └── description.md
    │   ├── get/
    │   ├── update/
    │   ├── delete/
    │   ├── list/
    │   ├── search/
    │   └── get_all_context/
    ├── issues/
    │   ├── mod.rs
    │   ├── create/
    │   ├── mark_complete/
    │   ├── all_complete/
    │   ├── update/
    │   ├── current/
    │   ├── work/
    │   ├── merge/
    │   └── next/
    └── search/
        ├── mod.rs
        ├── index/
        └── query/
```

### 2. Document Current Tool Inventory
Create comprehensive documentation of existing tools:

**Issue Tools:**
- `issue_create` → `issues/create`
- `issue_mark_complete` → `issues/mark_complete` 
- `issue_all_complete` → `issues/all_complete`
- `issue_update` → `issues/update`
- `issue_current` → `issues/current`
- `issue_work` → `issues/work`
- `issue_merge` → `issues/merge`
- `issue_next` → `issues/next`

**Memoranda Tools:**
- `memo_create` → `memoranda/create`
- `memo_get` → `memoranda/get`
- `memo_update` → `memoranda/update`
- `memo_delete` → `memoranda/delete`
- `memo_list` → `memoranda/list`
- `memo_search` → `memoranda/search`
- `memo_get_all_context` → `memoranda/get_all_context`

**Missing Search Tools (from CLI):**
- `search_index` → `search/index` (needs to be added to MCP)
- `search_query` → `search/query` (needs to be added to MCP)

### 3. Research Build Macro Implementation
Research how to implement the build macro pattern similar to builtin prompts for markdown descriptions. This will allow tool descriptions to be maintained as standalone markdown files that get compiled into the binary.

### 4. Set Up Testing Framework
Ensure comprehensive tests exist for all current MCP tools before refactoring begins. This includes:
- Unit tests for each tool handler method
- Integration tests for MCP protocol communication
- CLI integration tests

## Success Criteria
- [x] New directory structure created but empty
- [x] All existing MCP tools documented with mapping to new organization
- [x] Research completed on build macro implementation
- [x] Testing framework verified and any missing tests identified
- [x] No breaking changes - all existing functionality still works

## Next Steps
After completing this setup step, the plan will proceed with:
1. Creating tool registry pattern
2. Moving individual tools to new structure  
3. Implementing build macros for markdown descriptions
4. Updating CLI to use the same tools
5. Cleaning up duplicate code

## Risk Mitigation
- Keep all changes backwards compatible until migration is complete
- Maintain extensive test coverage throughout refactoring
- Use feature flags if needed to switch between old and new implementations

## Proposed Solution

After analyzing the current codebase structure, I propose the following implementation approach:

### Analysis of Current State
Current MCP tools are implemented in two main locations:
1. `mcp.rs` (4268+ lines) - Contains issue-related tools with large match statement
2. `mcp/tool_handlers.rs` - Contains memoranda tools in separate handler struct

**Current Tool Distribution:**
- Issue tools: `issue_create`, `issue_mark_complete`, `issue_all_complete`, `issue_update`, `issue_current`, `issue_work`, `issue_merge`, `issue_next`
- Memoranda tools: `memo_create`, `memo_get`, `memo_update`, `memo_delete`, `memo_list`, `memo_search`, `memo_get_all_context`
- Search tools: Currently CLI-only in `search.rs` and `semantic/` modules

### Implementation Steps

#### Step 1: Create Directory Structure (Non-Breaking)
- Create empty directory structure in `swissarmyhammer/src/mcp/tools/`
- Add placeholder `mod.rs` files but don't expose them yet
- Maintain current imports and functionality

#### Step 2: Document Tool Inventory 
- Create comprehensive mapping document
- Identify all current tool signatures and behaviors  
- Document CLI vs MCP tool differences for search functionality

#### Step 3: Research Build Macro Pattern
- Examine existing `build.rs` usage for builtin prompts
- Research `include_str!` and proc macro approaches for markdown descriptions
- Design tool description loading pattern

#### Step 4: Validate Test Coverage
- Run existing test suite to establish baseline
- Identify gaps in MCP tool test coverage
- Ensure all tools have unit and integration tests

### Expected Outcomes
- Zero breaking changes during this step
- Complete understanding of current tool architecture
- Foundation laid for subsequent refactoring steps
- Comprehensive test coverage baseline established

### Technical Approach
1. Use Test-Driven Development approach
2. Create directory structure without modifying existing code paths
3. Document findings for next refactoring steps
4. Maintain all existing functionality and tests

## Implementation Results

### Completed Work
All planned tasks for this setup step have been successfully completed:

1. **Directory Structure ✅**: The complete target directory structure has been created in `swissarmyhammer/src/mcp/tools/` with all necessary subdirectories for memoranda, issues, and search tools.

2. **Tool Inventory Documentation ✅**: Comprehensive documentation created in `TOOL_INVENTORY.md`:
   - 8 Issue tools mapped from `mcp.rs` 
   - 7 Memoranda tools mapped from `mcp/tool_handlers.rs`
   - 2 Search tools identified for future MCP integration
   - Complete implementation notes and risk assessment

3. **Build Macro Research ✅**: Detailed research completed in `BUILD_MACRO_RESEARCH.md`:
   - Analysis of existing `build.rs` pattern for builtin prompts
   - Proposed extension for tool description embedding
   - Implementation approach using compile-time string embedding
   - Integration strategy with tool registry pattern

4. **Test Coverage Validation ✅**: Comprehensive assessment in `TESTING_FRAMEWORK_ASSESSMENT.md`:
   - **Test Baseline**: 972 tests running successfully (100% pass rate)
   - **Coverage Analysis**: Excellent coverage for issue tools (~50+ tests), good coverage for memoranda tools
   - **Risk Assessment**: Low risk for infrastructure, medium risk for unit tests, high risk for direct handler tests
   - **Migration Strategy**: Incremental approach with TDD methodology

### Zero Breaking Changes
All existing functionality maintained:
- Original MCP server continues to function normally
- All 972 tests pass without modification
- No changes to existing imports or module structure
- Directory structure created without exposing new modules

### Foundation Established
The refactoring foundation is now complete and ready for subsequent steps:
- **REFACTOR_000204**: Tool registry pattern implementation
- **REFACTOR_000205**: Issue tools migration
- **REFACTOR_000206**: Memoranda tools migration
- **REFACTOR_000207**: Search tools MCP integration
- **REFACTOR_000208**: Build macro implementation for descriptions

This step successfully established a solid foundation for the MCP tools refactoring with comprehensive planning, risk mitigation, and zero breaking changes.