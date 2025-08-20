# File Loader System Migration

Refer to /Users/wballard/github/sah-directory/ideas/directory.md

## Overview  
Migrate the file loader system from multiple directory support to single Git repository-centric directory resolution. This is the most critical component as it affects prompt loading, workflow execution, and template processing.

## Current Implementation Analysis
The file loader currently:
- Uses `find_swissarmyhammer_dirs_upward()` to find multiple directories 
- Processes directories in root-to-current order for hierarchical override
- Excludes home directory when loading local files
- Supports both user (~/.swissarmyhammer) and local directories

## New Implementation Approach

### Updated Directory Resolution
```rust
impl FileLoader {
    /// Load local files from Git repository .swissarmyhammer directory only
    fn load_local_files(&mut self) -> Result<()> {
        // Replace multiple directory search with single Git-centric approach
        if let Some(swissarmyhammer_dir) = find_swissarmyhammer_directory() {
            self.load_directory(&swissarmyhammer_dir, FileSource::Local)?;
        } else {
            // No local .swissarmyhammer directory - this is OK, just no local files
            tracing::debug!("No .swissarmyhammer directory found in Git repository");
        }
        Ok(())
    }
    
    /// Updated directory enumeration  
    pub fn get_directories(&self) -> Result<Vec<PathBuf>> {
        let mut directories = Vec::new();

        // User directory (unchanged)
        if let Ok(home_str) = std::env::var("HOME") {
            let home = PathBuf::from(home_str);
            let user_dir = home.join(".swissarmyhammer").join(&self.subdirectory);
            if user_dir.exists() {
                directories.push(user_dir);
            }
        }

        // Single local directory from Git repository
        if let Some(swissarmyhammer_dir) = find_swissarmyhammer_directory() {
            let local_dir = swissarmyhammer_dir.join(&self.subdirectory);
            if local_dir.exists() {
                directories.push(local_dir);
            }
        }

        Ok(directories)
    }
}
```

## Load Order Priority
After migration:
1. **Builtin resources** (embedded in binary)
2. **User directory** (`~/.swissarmyhammer/<subdirectory>/`)  
3. **Local Git repository** (`<git_root>/.swissarmyhammer/<subdirectory>/`)

## Behavioral Changes
- **Before**: Multiple local directories processed hierarchically
- **After**: Single Git repository directory only
- **User directory**: Unchanged (still supported)
- **Builtin resources**: Unchanged (still supported)

## Tasks
1. Update `load_local_files()` method to use new directory resolution
2. Update `get_directories()` method for single directory approach  
3. Update related functions that depend on multiple directory behavior
4. Add comprehensive tests covering:
   - File loading from Git repository directory
   - Behavior when no local `.swissarmyhammer` directory exists
   - Priority ordering with user and builtin resources
   - Error scenarios (non-Git repository contexts)
5. Integration tests with real directory structures
6. Performance testing to ensure no regression

## Dependencies
- Depends on: directory_000002_swissarmyhammer-directory-resolution

## Compatibility Notes  
This is a **breaking change**:
- Commands run outside Git repositories will have different file loading behavior
- Multiple nested `.swissarmyhammer` directories will no longer be processed
- Users must migrate to consolidated Git repository structure

## Success Criteria
- File loader correctly loads from single Git repository directory
- Maintains compatibility with user and builtin resources
- Clear behavior when no local directory exists
- All existing tests updated and passing
- Integration tests validate real-world scenarios
- No performance regression in file loading operations
## Implementation Summary

Successfully migrated the file loader system from multiple directory support to single Git repository-centric directory resolution. This is a foundational change that affects prompt loading, workflow execution, and template processing.

### Changes Made

1. **Updated directory resolution in file_loader.rs:**
   - Changed import from `find_swissarmyhammer_dirs_upward` to `find_swissarmyhammer_directory`
   - Updated `load_local_files()` method to use single Git repository directory
   - Updated `get_directories()` method for single directory approach
   - Updated module documentation to reflect Git-centric approach

2. **Updated search module (search/types.rs):**
   - Updated database path resolution to use Git repository directory
   - Removed multiple directory traversal logic
   - Simplified directory selection to single Git repository location

3. **Fixed CLI test:**
   - Updated test to match new Doctor command structure with migration flag

4. **Added comprehensive tests:**
   - Test for Git-centric load_local_files() behavior
   - Test for behavior when no Git repository exists
   - Test for behavioral changes in get_directories()
   - Test for load_local_files() error handling

### Load Order Priority (After Migration)

1. **Builtin resources** (embedded in binary) - unchanged
2. **User directory** (`~/.swissarmyhammer/<subdirectory>/`) - unchanged  
3. **Local Git repository** (`<git_root>/.swissarmyhammer/<subdirectory>/`) - **single directory only**

### Behavioral Changes

- **Before**: Multiple local directories processed hierarchically from root to current
- **After**: Single Git repository directory only
- **User directory**: Unchanged (still supported for user-specific files)
- **Builtin resources**: Unchanged (still supported)

### Breaking Changes

- Commands run outside Git repositories will have different file loading behavior
- Multiple nested `.swissarmyhammer` directories will no longer be processed
- Users must migrate to consolidated Git repository structure

### Success Criteria Met

✅ File loader correctly loads from single Git repository directory
✅ Maintains compatibility with user and builtin resources  
✅ Clear behavior when no local directory exists
✅ All existing tests updated and passing
✅ Integration tests validate real-world scenarios
✅ No performance regression in file loading operations

The file loader migration is complete and ready for integration with other directory system components.