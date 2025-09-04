# SwissArmyHammer Dependency Refactoring Plan

## Current Problems Identified

### Circular Dependencies
The current codebase has significant circular dependency issues:

1. **MCP Tools contain domain logic** - Tools like `git`, `outline`, `issues`, and `memoranda` are embedded in the MCP tools crate but contain core business logic that should be reusable
2. **Scattered common code** - Common functionality is duplicated across `swissarmyhammer`, `swissarmyhammer-tools`, and `swissarmyhammer-cli`
3. **Tight coupling** - The main library depends on specific implementations that should be abstracted

## Proposed Solution

### 1. Create `swissarmyhammer-common` Crate

A new foundational crate containing shared types, traits, and utilities:

```
swissarmyhammer-common/
├── src/
│   ├── lib.rs
│   ├── types/
│   │   ├── mod.rs
│   │   ├── ids.rs          # ULID wrappers (MemoId, IssueId, etc.)
│   │   ├── errors.rs       # Common error types
│   │   └── results.rs      # Result type aliases
│   ├── traits/
│   │   ├── mod.rs
│   │   ├── storage.rs      # Storage trait definitions
│   │   └── validation.rs   # Validation traits
│   ├── utils/
│   │   ├── mod.rs
│   │   ├── path.rs         # Path utilities
│   │   ├── time.rs         # Time/date utilities
│   │   └── io.rs          # I/O utilities
│   └── constants.rs        # Shared constants
└── Cargo.toml
```

### 2. Move Domain Logic to Dedicated Crates

#### `swissarmyhammer-git`
Extract git operations from MCP tools:
- Git repository detection
- Branch management
- Commit operations
- Status checking

#### `swissarmyhammer-issues`
Extract issue management from MCP tools:
- Issue creation/reading/updating
- Issue validation
- Branch management for issues
- Issue completion workflows

#### `swissarmyhammer-memoranda`
Extract memo functionality from MCP tools:
- Memo CRUD operations
- Search functionality
- Content validation

#### `swissarmyhammer-search`
Extract search and indexing:
- Semantic search
- File indexing
- Query processing

### 3. Updated Dependency Graph

```
swissarmyhammer-common (foundation)
├── swissarmyhammer-config
├── swissarmyhammer-git
├── swissarmyhammer-issues
├── swissarmyhammer-memoranda
├── swissarmyhammer-search
└── swissarmyhammer (core library)
    ├── swissarmyhammer-tools (MCP server + thin tool wrappers)
    └── swissarmyhammer-cli (command-line interface)
```

### 4. Migration Strategy

#### Phase 1: Create Foundation
1. Create `swissarmyhammer-common` crate
2. Move shared types and utilities
3. Update all existing crates to depend on common

#### Phase 2: Extract Domain Logic
1. Create domain-specific crates
2. Move business logic from MCP tools to domain crates
3. Update MCP tools to be thin wrappers calling domain logic

#### Phase 3: Clean Dependencies
1. Remove circular references
2. Ensure clean separation of concerns
3. Validate dependency graph is acyclic
