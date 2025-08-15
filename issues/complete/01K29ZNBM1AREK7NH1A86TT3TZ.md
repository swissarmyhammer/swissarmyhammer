separate mpc from swissarmyhammer into swissarmyhammer-tools crate, and add it to the workspace. preserve all functionality, we're just looking to have a separate crate for tools which will speed up builds.

## Proposed Solution

After analyzing the codebase, I will separate the MCP functionality from the main `swissarmyhammer` crate into a new `swissarmyhammer-tools` crate. This will improve build times by isolating the MCP server and tools functionality.

### Implementation Plan

1. **Create New Crate Structure**
   - Create `swissarmyhammer-tools/` directory with standard Rust crate structure
   - Initialize `Cargo.toml` with appropriate dependencies
   - Set up module structure based on current MCP organization

2. **Move MCP Code**
   - Move entire `src/mcp/` directory to `swissarmyhammer-tools/src/mcp/`
   - Extract MCP-specific functionality while preserving all features
   - Maintain the existing tool directory pattern: `noun/verb/` organization

3. **Dependencies Analysis**
   The MCP module currently depends on:
   - Core library types: `PromptLibrary`, `SwissArmyHammerError`, `Result`
   - Storage backends: `FileSystemIssueStorage`, `MarkdownMemoStorage`
   - Workflow system: `WorkflowStorage`, `WorkflowRunStorage` 
   - Git operations: `GitOperations`
   - Search functionality: Semantic search and outline generation
   - File watching: `FileWatcher`
   - Rate limiting and common utilities

4. **Update Workspace Configuration**
   - Add `swissarmyhammer-tools` to workspace members
   - Move MCP-specific dependencies (`rmcp`, etc.) to new crate
   - Maintain shared dependencies at workspace level

5. **Preserve Functionality**
   - All MCP tools remain functional (issues, memos, search, outline)
   - Server startup and protocol handling unchanged
   - CLI integration maintained through new crate imports

6. **Update Imports**
   - Update `swissarmyhammer-cli` to import MCP server from new crate
   - Remove MCP module from main library's `lib.rs`
   - Update all references throughout the codebase

### Benefits

- **Faster Builds**: Separating MCP tools reduces compilation time for library-only usage
- **Cleaner Architecture**: Clear separation between library core and MCP tools
- **Better Organization**: MCP functionality isolated in dedicated crate
- **Maintained Functionality**: All existing features preserved
- **Easier Testing**: MCP tools can be tested independently

This approach follows the established workspace pattern in the codebase and maintains the existing tool directory organization while achieving the goal of faster builds through better separation of concerns.