# MCP Tool Directory Pattern

## Overview

The Swiss Army Hammer MCP tools follow a consistent directory organization pattern that improves maintainability and discoverability.

## Directory Structure

```
src/mcp/tools/
├── <noun>/
│   ├── <verb>/
│   │   ├── mod.rs         # Tool implementation
│   │   └── description.md # Tool description
│   └── mod.rs            # Module exports
```

## Pattern Details

### Noun-Based Organization
Tools are grouped by the resource they operate on:

Examples:
- `issues/` - Issue management tools
- `memoranda/` - Memo management tools  
- `search/` - Search functionality tools

### Verb-Based Submodules
Each action on a resource gets its own submodule:

Examples:
- `issues/create/` - Create new issues
- `issues/work/` - Start working on an issue
- `issues/merge/` - Merge issue branches
- `memoranda/get/` - Retrieve memos
- `memoranda/update/` - Update existing memos

### Separated Descriptions
Each tool has a `description.md` file that contains the help text shown to users. This separation:
- Keeps implementation code clean
- Makes descriptions easy to update
- Allows markdown formatting in descriptions
- Centralizes user-facing documentation
