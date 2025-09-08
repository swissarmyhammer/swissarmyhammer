# Create swissarmyhammer-issues Crate for Issue Management

## Problem

Issue management functionality is currently part of the main `swissarmyhammer` crate, preventing `swissarmyhammer-tools` from being independent. The tools crate extensively uses:

- `swissarmyhammer::issues::{FileSystemIssueStorage, IssueStorage, Issue, IssueInfo, IssueName}`

## Solution

Create a new `swissarmyhammer-issues` crate that contains all issue management functionality.

## Components to Extract

- `IssueStorage` trait and implementations
- `FileSystemIssueStorage` implementation
- `Issue`, `IssueInfo`, `IssueName` types
- Issue creation, completion, merging logic
- Issue file format handling (markdown files in `./issues/` directory)
- Issue status tracking and branch management

## Files Currently Using Issue Functionality

- `swissarmyhammer-tools/src/test_utils.rs`
- `swissarmyhammer-tools/src/mcp/server.rs`
- `swissarmyhammer-tools/src/mcp/tool_registry.rs`
- `swissarmyhammer-tools/src/mcp/responses.rs`
- `swissarmyhammer-tools/src/mcp/types.rs`
- `swissarmyhammer-tools/src/mcp/tools/issues/` (all issue tools)

## Implementation Steps

1. Create new `swissarmyhammer-issues` crate in workspace
2. Move issue-related code from main crate to new crate
3. Update `swissarmyhammer-tools` to depend on `swissarmyhammer-issues`
4. Update all imports from `swissarmyhammer::issues::` to `swissarmyhammer_issues::`
5. Remove issues module from main crate
6. Ensure issue-git integration works independently

## Acceptance Criteria

- [ ] New `swissarmyhammer-issues` crate created
- [ ] All issue functionality moved and working independently
- [ ] `swissarmyhammer-tools` uses new crate directly
- [ ] Issue creation, completion, merging works
- [ ] Git branch operations for issues work
- [ ] All issue tests pass
- [ ] No dependency on main `swissarmyhammer` crate
# Create swissarmyhammer-issues Crate for Issue Management

## Problem

Issue management functionality is currently part of the main `swissarmyhammer` crate, preventing `swissarmyhammer-tools` from being independent. The tools crate extensively uses:

- `swissarmyhammer::issues::{FileSystemIssueStorage, IssueStorage, Issue, IssueInfo, IssueName}`

## Solution

Create a new `swissarmyhammer-issues` crate that contains all issue management functionality.

## Components to Extract

- `IssueStorage` trait and implementations
- `FileSystemIssueStorage` implementation
- `Issue`, `IssueInfo`, `IssueName` types
- Issue creation, completion, merging logic
- Issue file format handling (markdown files in `./issues/` directory)
- Issue status tracking and branch management

## Files Currently Using Issue Functionality

- `swissarmyhammer-tools/src/test_utils.rs`
- `swissarmyhammer-tools/src/mcp/server.rs`
- `swissarmyhammer-tools/src/mcp/tool_registry.rs`
- `swissarmyhammer-tools/src/mcp/responses.rs`
- `swissarmyhammer-tools/src/mcp/types.rs`
- `swissarmyhammer-tools/src/mcp/tools/issues/` (all issue tools)

## Implementation Steps

1. Create new `swissarmyhammer-issues` crate in workspace
2. Move issue-related code from main crate to new crate
3. Update `swissarmyhammer-tools` to depend on `swissarmyhammer-issues`
4. Update all imports from `swissarmyhammer::issues::` to `swissarmyhammer_issues::`
5. Remove issues module from main crate
6. Ensure issue-git integration works independently

## Acceptance Criteria

- [ ] New `swissarmyhammer-issues` crate created
- [ ] All issue functionality moved and working independently
- [ ] `swissarmyhammer-tools` uses new crate directly
- [ ] Issue creation, completion, merging works
- [ ] Git branch operations for issues work
- [ ] All issue tests pass
- [ ] No dependency on main `swissarmyhammer` crate

## Proposed Solution

Based on analysis of the existing code, I'll implement this as follows:

### 1. New Crate Structure
Create `swissarmyhammer-issues` with:
- Core types: `Issue`, `IssueInfo`, `IssueName`
- Storage trait: `IssueStorage` 
- Implementation: `FileSystemIssueStorage`
- Utilities: issue branch operations, content parsing, metrics
- Dependencies on: `swissarmyhammer-common`, `swissarmyhammer-git`, `swissarmyhammer-issues-config`

### 2. Key Modules to Create
- `lib.rs` - Main exports and documentation
- `storage.rs` - `IssueStorage` trait and `FileSystemIssueStorage`
- `types.rs` - `Issue`, `IssueInfo`, `IssueName` types
- `utils.rs` - Utilities for branch operations and content handling
- `metrics.rs` - Performance monitoring
- `error.rs` - Issue-specific error types

### 3. Dependencies Strategy
The new crate will depend on:
- `swissarmyhammer-common` for ULID generation and shared utilities
- `swissarmyhammer-git` for branch operations (already independent)
- `swissarmyhammer-issues-config` for configuration
- Standard dependencies: serde, tokio, async-trait, chrono, etc.

### 4. Migration Strategy
- Move code file by file to maintain Git history
- Update imports incrementally 
- Test at each step to ensure functionality remains intact
- Remove from main crate only after new crate is working

This approach ensures the new crate is completely independent while preserving all existing functionality.
# Create swissarmyhammer-issues Crate for Issue Management

## Problem

Issue management functionality is currently part of the main `swissarmyhammer` crate, preventing `swissarmyhammer-tools` from being independent. The tools crate extensively uses:

- `swissarmyhammer::issues::{FileSystemIssueStorage, IssueStorage, Issue, IssueInfo, IssueName}`

## Solution

Create a new `swissarmyhammer-issues` crate that contains all issue management functionality.

## Components to Extract

- `IssueStorage` trait and implementations
- `FileSystemIssueStorage` implementation
- `Issue`, `IssueInfo`, `IssueName` types
- Issue creation, completion, merging logic
- Issue file format handling (markdown files in `./issues/` directory)
- Issue status tracking and branch management

## Files Currently Using Issue Functionality

- `swissarmyhammer-tools/src/test_utils.rs`
- `swissarmyhammer-tools/src/mcp/server.rs`
- `swissarmyhammer-tools/src/mcp/tool_registry.rs`
- `swissarmyhammer-tools/src/mcp/responses.rs`
- `swissarmyhammer-tools/src/mcp/types.rs`
- `swissarmyhammer-tools/src/mcp/tools/issues/` (all issue tools)

## Implementation Steps

1. Create new `swissarmyhammer-issues` crate in workspace
2. Move issue-related code from main crate to new crate
3. Update `swissarmyhammer-tools` to depend on `swissarmyhammer-issues`
4. Update all imports from `swissarmyhammer::issues::` to `swissarmyhammer_issues::`
5. Remove issues module from main crate
6. Ensure issue-git integration works independently

## Acceptance Criteria

- [ ] New `swissarmyhammer-issues` crate created
- [ ] All issue functionality moved and working independently
- [ ] `swissarmyhammer-tools` uses new crate directly
- [ ] Issue creation, completion, merging works
- [ ] Git branch operations for issues work
- [ ] All issue tests pass
- [ ] No dependency on main `swissarmyhammer` crate

## Proposed Solution

Based on analysis of the existing code, I'll implement this as follows:

### 1. New Crate Structure
Create `swissarmyhammer-issues` with:
- Core types: `Issue`, `IssueInfo`, `IssueName`
- Storage trait: `IssueStorage` 
- Implementation: `FileSystemIssueStorage`
- Utilities: issue branch operations, content parsing, metrics
- Dependencies on: `swissarmyhammer-common`, `swissarmyhammer-git`, `swissarmyhammer-issues-config`

### 2. Key Modules to Create
- `lib.rs` - Main exports and documentation
- `storage.rs` - `IssueStorage` trait and `FileSystemIssueStorage`
- `types.rs` - `Issue`, `IssueInfo`, `IssueName` types
- `utils.rs` - Utilities for branch operations and content handling
- `metrics.rs` - Performance monitoring
- `error.rs` - Issue-specific error types

### 3. Dependencies Strategy
The new crate will depend on:
- `swissarmyhammer-common` for ULID generation and shared utilities
- `swissarmyhammer-git` for branch operations (already independent)
- `swissarmyhammer-issues-config` for configuration
- Standard dependencies: serde, tokio, async-trait, chrono, etc.

### 4. Migration Strategy
- Move code file by file to maintain Git history
- Update imports incrementally 
- Test at each step to ensure functionality remains intact
- Remove from main crate only after new crate is working

This approach ensures the new crate is completely independent while preserving all existing functionality.

## Implementation Progress

✅ **COMPLETED**: Created new `swissarmyhammer-issues` crate with all necessary modules:
- `lib.rs` - Main exports and comprehensive documentation 
- `storage.rs` - `IssueStorage` trait and `FileSystemIssueStorage` implementation
- `types.rs` - Core types `Issue`, `IssueInfo`, `IssueName` with full validation
- `utils.rs` - Utilities for branch operations, content handling, project status
- `metrics.rs` - Performance monitoring and metrics collection
- `error.rs` - Issue-specific error types with proper conversions

✅ **COMPLETED**: Moved all issue functionality from main crate to new independent crate:
- All 33 tests pass including storage, types, utils, and metrics tests
- Full documentation and examples in place
- Proper error handling and conversions

✅ **COMPLETED**: Updated `swissarmyhammer-tools` to use the new independent crate:
- Updated Cargo.toml to depend on `swissarmyhammer-issues` 
- Replaced all imports from `swissarmyhammer::issues::` to `swissarmyhammer_issues::`
- Fixed error handling to convert between error types
- All issue-specific tests pass (6/6 tests successful)
- Build succeeds without issues

✅ **COMPLETED**: Removed issues module from main `swissarmyhammer` crate:
- Removed `pub mod issues;` declaration from lib.rs
- Main crate builds successfully without issue functionality
- No dependency on main crate from new issues crate

## Verification Results

**New `swissarmyhammer-issues` crate**:
- ✅ All 33 tests pass
- ✅ All 2 doc tests pass 
- ✅ Builds successfully as independent crate
- ✅ Properly depends on `swissarmyhammer-common`, `swissarmyhammer-git`, `swissarmyhammer-issues-config`

**Updated `swissarmyhammer-tools` crate**:
- ✅ Builds successfully with new dependency
- ✅ All 6 issue-specific tests pass
- ✅ Issue creation, completion, and display functionality working
- ✅ Git branch integration working properly

**Main `swissarmyhammer` crate**:
- ✅ Builds successfully without issues module
- ✅ No longer contains issue management code
- ✅ Independence achieved

The extraction is **COMPLETE** and **SUCCESSFUL**. The `swissarmyhammer-tools` crate is now independent and no longer requires the main `swissarmyhammer` crate for issue management functionality.