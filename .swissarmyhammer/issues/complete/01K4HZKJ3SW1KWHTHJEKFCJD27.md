# Complete swissarmyhammer-config Crate Independence

## Problem

The `swissarmyhammer-config` crate exists but `swissarmyhammer-tools` still imports config through the main crate:

- `swissarmyhammer::config::Config`

This suggests the config crate may not be complete or properly exposed.

## Solution

Ensure `swissarmyhammer-config` is a complete, standalone crate that provides all configuration functionality without depending on the main crate.

## Files Using Config

- `swissarmyhammer-tools/src/mcp/utils.rs`
- `swissarmyhammer-tools/src/mcp/tools/issues/show/mod.rs`
- `swissarmyhammer-tools/src/mcp/tools/issues/mark_complete/mod.rs`

## Tasks

1. Review current `swissarmyhammer-config` crate completeness
2. Ensure `Config` struct and all config functionality is available
3. Move any remaining config code from main crate if needed
4. Update `swissarmyhammer-tools` imports to use `swissarmyhammer_config::` directly
5. Remove config re-export from main crate

## Acceptance Criteria

- [ ] `swissarmyhammer-config` crate is fully independent
- [ ] `Config` and all config types available without main crate
- [ ] All imports updated to use `swissarmyhammer_config::` directly
- [ ] Configuration loading and parsing works independently
- [ ] All tests pass

## Proposed Solution

After analyzing the codebase, I discovered that there are actually **two different Config systems** at play:

1. **`swissarmyhammer::config::Config`** - Issue management configuration (branch prefixes, length limits, etc.)
2. **`swissarmyhammer-config` crate** - Template/figment-based configuration system (`TemplateContext`, `AgentConfig`, etc.)

The files mentioned in the issue are actually importing the **issue management config**, not the template config system. This means the issue title is misleading - the `swissarmyhammer-config` crate is already independent and complete.

### Analysis of Current Imports:

- `swissarmyhammer-tools/src/mcp/utils.rs` - Uses `Config::global()` for issue validation limits
- `swissarmyhammer-tools/src/mcp/tools/issues/show/mod.rs` - Uses config for issue display formatting  
- `swissarmyhammer-tools/src/mcp/tools/issues/mark_complete/mod.rs` - Uses config for issue completion logic

### Proposed Solution:

1. **Extract issue config to separate crate**: Move `swissarmyhammer::config::Config` to `swissarmyhammer-issues-config` crate
2. **Update imports**: Change imports from `swissarmyhammer::config::Config` to `swissarmyhammer_issues_config::Config`
3. **Remove config re-export**: Remove the config re-export from main `swissarmyhammer` crate
4. **Maintain independence**: Ensure both config systems remain independent

This will achieve true crate independence while avoiding naming conflicts between the two different configuration systems.

## Implementation Complete ✅

Successfully completed the swissarmyhammer-config crate independence refactoring. Here's what was accomplished:

### Key Discovery
The original issue description was based on a misunderstanding. There are actually **two different Config systems**:

1. **`swissarmyhammer-config`** - Template/figment-based configuration system (TemplateContext, AgentConfig, etc.) - **Already independent**
2. **`swissarmyhammer::config::Config`** - Issue management configuration (branch prefixes, length limits, etc.) - **Needed extraction**

### Changes Made

#### 1. Created New Independent Crate
- **New crate**: `swissarmyhammer-issues-config`
- Extracted issue management configuration from main crate
- Added proper dependency on `swissarmyhammer-common` 

#### 2. Moved EnvLoader to Common Crate
- Moved `EnvLoader` from `swissarmyhammer/src/common/env_loader.rs` to `swissarmyhammer-common/src/env_loader.rs`
- Updated exports in `swissarmyhammer-common/src/lib.rs`
- Ensures shared environment variable loading utilities

#### 3. Updated All Imports
- **swissarmyhammer-tools/src/mcp/utils.rs**: `swissarmyhammer::config::Config` → `swissarmyhammer_issues_config::Config`
- **swissarmyhammer-tools/src/mcp/tools/issues/show/mod.rs**: Updated import
- **swissarmyhammer-tools/src/mcp/tools/issues/mark_complete/mod.rs**: Updated import
- **swissarmyhammer-tools/tests/test_issue_show_enhanced.rs**: Updated import
- **swissarmyhammer/src/issues/mod.rs**: Updated import and added dependency

#### 4. Updated Workspace Configuration
- Added `swissarmyhammer-issues-config` to workspace members in `Cargo.toml`
- Added dependency in `swissarmyhammer/Cargo.toml` and `swissarmyhammer-tools/Cargo.toml`

#### 5. Cleaned Up Main Crate
- Removed `pub mod config;` from `swissarmyhammer/src/lib.rs`
- Removed `pub use config::Config;` re-export
- Deleted `swissarmyhammer/src/config.rs` (moved to new crate)

### Result
- ✅ **swissarmyhammer-config** crate remains fully independent (template configuration)
- ✅ **swissarmyhammer-issues-config** crate is now fully independent (issue management configuration)
- ✅ All imports updated to use `swissarmyhammer_issues_config::Config` directly  
- ✅ No more config re-exports from main crate
- ✅ Both configuration systems work independently
- ✅ All builds and tests pass

### Acceptance Criteria Status
- [x] `swissarmyhammer-config` crate is fully independent
- [x] `Config` and all config types available without main crate  
- [x] All imports updated to use dedicated config crates directly
- [x] Configuration loading and parsing works independently
- [x] All tests pass

The refactoring is complete and both configuration systems now have proper crate independence!