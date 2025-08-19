# SwissArmyHammer Directory Specification

## Problem Statement

SwissArmyHammer currently has inconsistent and problematic directory determination logic that leads to:

1. **Multiple Directory Lookups**: Current logic searches up the directory tree for multiple `.swissarmyhammer` directories, creating confusion about which one takes precedence
2. **Current Working Directory Dependencies**: Many components assume the current working directory is relevant for `.swissarmyhammer` placement, breaking when commands are run from subdirectories
3. **Inconsistent Behavior**: Different components (file loading, memoranda, search, todos, issues) use different strategies for finding the `.swissarmyhammer` directory
4. **No Git Repository Requirement**: `sah doctor` and other commands don't enforce that operations happen within a Git repository context

There needs to be a singular method to determine the current repository .swissarmyhammer directory

## Current Implementation Analysis

### Existing Directory Logic

The codebase currently has these patterns:

1. **Multi-directory upward search** (`directory_utils.rs:25-72`):
   ```rust
   find_swissarmyhammer_dirs_upward(start_path: &Path, exclude_home: bool) -> Vec<PathBuf>
   ```
   - Walks up directory tree finding ALL `.swissarmyhammer` directories
   - Returns ordered list from root to current
   - Used by file loader and search systems

2. **Repository-aware fallback** (`directory_utils.rs:137-163`):
   ```rust
   find_repository_or_current_directory() -> Result<PathBuf, std::io::Error>
   ```
   - Looks for Git repository root, falls back to current directory
   - Used only by todo system

3. **Current directory only** (memoranda system):
   ```rust
   std::env::current_dir()?.join(".swissarmyhammer").join("memos")
   ```
   - Simple current working directory approach
   - No upward traversal

### Components Using Different Strategies

- **File Loading System**: Uses multiple directory upward search, excludes home
- **Todo System**: Uses Git repository root or current directory
- **Memoranda System**: Uses current working directory only
- **Search System**: Uses deepest `.swissarmyhammer` directory from upward search
- **Issues System**: Uses explicit working directory, doesn't use `.swissarmyhammer` at all
- **Doctor Command**: Checks both home and local current directory

## Proposed Solution

### Core Principles

1. **Git Repository Centric**: `.swissarmyhammer` directories should only exist at Git repository roots (next to `.git` directories)
2. **Single Source of Truth**: Each Git repository has exactly one `.swissarmyhammer` directory
3. **No Current Working Directory Dependencies**: The `.swissarmyhammer` location is determined by Git repository structure, not where commands are executed
4. **Git Repository Required**: All SwissArmyHammer operations must be performed within a Git repository

### New Directory Resolution Algorithm

```rust
/// Find the SwissArmyHammer directory for the current Git repository
/// Returns None if not in a Git repository or if no .swissarmyhammer directory exists
pub fn find_swissarmyhammer_directory() -> Option<PathBuf> {
    let git_root = find_git_repository_root()?;
    let swissarmyhammer_dir = git_root.join(".swissarmyhammer");
    
    if swissarmyhammer_dir.exists() && swissarmyhammer_dir.is_dir() {
        Some(swissarmyhammer_dir)
    } else {
        None
    }
}

/// Find the Git repository root starting from current directory
/// Walks up the directory tree looking for .git directory
pub fn find_git_repository_root() -> Option<PathBuf> {
    let current_dir = std::env::current_dir().ok()?;
    let mut path = current_dir.as_path();
    let mut depth = 0;

    loop {
        if depth >= MAX_DIRECTORY_DEPTH {
            break;
        }

        if path.join(".git").exists() {
            return Some(path.to_path_buf());
        }

        match path.parent() {
            Some(parent) => {
                path = parent;
                depth += 1;
            }
            None => break,
        }
    }

    None
}

/// Get or create the SwissArmyHammer directory for the current Git repository
/// Returns error if not in a Git repository
pub fn get_or_create_swissarmyhammer_directory() -> Result<PathBuf, SwissArmyHammerError> {
    let git_root = find_git_repository_root()
        .ok_or(SwissArmyHammerError::NotInGitRepository)?;
    
    let swissarmyhammer_dir = git_root.join(".swissarmyhammer");
    
    if !swissarmyhammer_dir.exists() {
        std::fs::create_dir_all(&swissarmyhammer_dir)
            .map_err(|e| SwissArmyHammerError::DirectoryCreation(e))?;
    }
    
    Ok(swissarmyhammer_dir)
}
```

### Directory Structure

Under this specification, the directory structure will be:

```
project-root/               # Git repository root
├── .git/                   # Git directory
├── .swissarmyhammer/       # SwissArmyHammer directory (ONLY HERE)
│   ├── memos/             # Memoranda storage
│   ├── todo/              # Todo lists
│   ├── runs/              # Workflow run storage
│   ├── search.db          # Semantic search database
│   └── workflows/         # Local workflows (optional)
├── issues/                 # Issue tracking (separate from .swissarmyhammer)
└── src/                    # Project source code
```

### Migration Strategy

1. **Deprecate Multiple Directory Support**: Remove `find_swissarmyhammer_dirs_upward` function
2. **Update All Components**: Migrate each component to use the new single directory resolution
3. **Add Git Repository Validation**: All commands require being in a Git repository
4. **Migration Tooling**: Provide `sah migrate-directory` command to help users consolidate multiple `.swissarmyhammer` directories

### Component Changes Required

#### 1. File Loading System (`file_loader.rs`)
- **Before**: Searches multiple directories, processes in hierarchical order
- **After**: Uses single Git repository `.swissarmyhammer` directory -- still uses ~/.swissarmyhammer and builtin

#### 2. Memoranda System (`memoranda/storage.rs`)
- **Before**: Uses current working directory
- **After**: Uses Git repository `.swissarmyhammer/memos`
- **Impact**: Consistent location regardless of where commands are run

#### 3. Todo System (`todo/mod.rs`)
- **Before**: Uses repository root OR current directory
- **After**: Uses Git repository `.swissarmyhammer/todo` (repository root required)
- **Impact**: Consistent behavior, no fallback to current directory

#### 4. Search System (`search/types.rs`)
- **Before**: Uses deepest directory from multiple search
- **After**: Uses Git repository `.swissarmyhammer/search.db`
- **Impact**: Single database per repository

#### 5. Doctor Command (`doctor/`)
- **Before**: Checks home and current directory
- **After**: Requires Git repository, checks only repository `.swissarmyhammer`
- **Impact**: Focused validation, Git repository requirement enforced

#### 6. Issues System (no changes needed)
- Issues already use a separate `.swissarmyhammer/issues/` directory at repository root

### Error Handling

New error types to handle Git repository requirements:

```rust
#[derive(Error, Debug)]
pub enum SwissArmyHammerError {
    #[error("SwissArmyHammer must be run from within a Git repository")]
    NotInGitRepository,
    
    #[error("Failed to create .swissarmyhammer directory: {0}")]
    DirectoryCreation(std::io::Error),
    
    #[error("Git repository found but .swissarmyhammer directory is not accessible: {0}")]
    DirectoryAccess(std::io::Error),
}
```

### CLI Integration

Update `sah doctor` to enforce Git repository requirement:

```rust
pub fn run_doctor() -> Result<(), SwissArmyHammerError> {
    // First, ensure we're in a Git repository
    let git_root = find_git_repository_root()
        .ok_or(SwissArmyHammerError::NotInGitRepository)?;
    
    println!("Git repository root: {}", git_root.display());
    
    // Check .swissarmyhammer directory
    let swissarmyhammer_dir = git_root.join(".swissarmyhammer");
    // ... rest of doctor checks
}
```

### Benefits

1. **Consistency**: All components use the same directory resolution logic
2. **Predictability**: `.swissarmyhammer` always at Git repository root
3. **No Path Dependencies**: Commands work the same regardless of current directory
4. **Simplified Logic**: No need to handle multiple directories or precedence rules
5. **Git Integration**: Aligns with Git-centric development workflow
6. **Clear Error Messages**: Users know they need to be in a Git repository

### Breaking Changes

Do not worry about it

### Backward Compatibility

None
