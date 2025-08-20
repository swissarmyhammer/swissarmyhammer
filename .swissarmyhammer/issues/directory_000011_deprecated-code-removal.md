# Deprecated Code Removal and Cleanup

Refer to /Users/wballard/github/sah-directory/ideas/directory.md

## Overview
Remove deprecated functions and code paths that are no longer needed after the directory migration, ensuring a clean codebase without legacy patterns.

## Code Removal Tasks

### Remove Deprecated Functions
```rust
// Remove from directory_utils.rs:
pub fn find_swissarmyhammer_dirs_upward(start_path: &Path, exclude_home: bool) -> Vec<PathBuf>
pub fn find_repository_or_current_directory() -> Result<PathBuf, std::io::Error>
```

### Update Documentation
- Remove references to multiple directory support in code comments
- Update function documentation to reflect Git repository requirements  
- Remove examples showing old directory patterns

### Clean Up Imports
Remove unused imports across codebase:
```rust
// Remove these where no longer needed:
use crate::directory_utils::{find_swissarmyhammer_dirs_upward, find_repository_or_current_directory};
```

## Affected Files Analysis
Based on grep results, these files need updates:

1. **`swissarmyhammer/src/file_loader.rs`**:
   - Remove import of `find_swissarmyhammer_dirs_upward`
   - Verify new implementation doesn't reference old functions

2. **`swissarmyhammer/src/todo/mod.rs`**:
   - Remove usage of `find_repository_or_current_directory`  
   - Verify migration to new Git-centric approach

3. **`swissarmyhammer/src/search/types.rs`**:
   - Remove usage of `find_swissarmyhammer_dirs_upward`
   - Verify migration to single directory approach

### Directory Utils Cleanup
```rust
// directory_utils.rs after cleanup should only contain:
pub fn find_git_repository_root() -> Option<PathBuf>
pub fn find_swissarmyhammer_directory() -> Option<PathBuf> 
pub fn get_or_create_swissarmyhammer_directory() -> Result<PathBuf, SwissArmyHammerError>
pub fn walk_files_with_extensions<'a>(/* ... */) -> impl Iterator<Item = PathBuf> + 'a

// Remove these deprecated functions:
// ❌ find_swissarmyhammer_dirs_upward
// ❌ find_repository_or_current_directory
```

## Test Cleanup  
Remove or update tests that tested deprecated functionality:
```rust
// Remove from directory_utils.rs tests:
#[test]
fn test_find_swissarmyhammer_dirs_upward() { /* ... */ }

// Keep and update tests for new functionality:
#[test]  
fn test_find_git_repository_root() { /* ... */ }
#[test]
fn test_find_swissarmyhammer_directory() { /* ... */ }
```

## Compilation Validation
Ensure no code still references deprecated functions:
```bash
# Should return no results after cleanup:
rg "find_swissarmyhammer_dirs_upward|find_repository_or_current_directory"
```

## Breaking Change Documentation
Update documentation to clearly indicate:
- Which functions were removed and when
- Migration path for users who might be using these functions  
- Clear explanation of new Git repository requirements

## Tasks
1. Remove deprecated functions from `directory_utils.rs`
2. Clean up imports in affected files:
   - `file_loader.rs`  
   - `todo/mod.rs`
   - `search/types.rs`
3. Remove deprecated tests and update test documentation
4. Update code documentation and comments
5. Run comprehensive compilation tests to ensure nothing broken
6. Update public API documentation
7. Add deprecation notes to CHANGELOG.md
8. Verify no dead code warnings after cleanup

## Validation Steps
```bash
# Compilation check
cargo build --all-targets

# Test check  
cargo test

# Linting check
cargo clippy -- -D warnings

# Documentation check
cargo doc --all --no-deps
```

## Dependencies  
- Depends on: All migration steps (000001-000010) must be complete
- This should be the final step in the migration process

## Success Criteria
- All deprecated functions completely removed from codebase
- No compilation errors or warnings  
- All tests pass after cleanup
- No dead code warnings from clippy
- Clean, maintainable codebase with only new Git-centric functions
- Documentation accurately reflects current functionality  
- CHANGELOG documents breaking changes appropriately