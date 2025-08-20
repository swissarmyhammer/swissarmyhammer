# Doctor Command Git Repository Requirement Update

Refer to /Users/wballard/github/sah-directory/ideas/directory.md

## Overview  
Update the `sah doctor` command to enforce Git repository requirements and focus validation on the single repository-centric `.swissarmyhammer` directory.

## Current Implementation Analysis
The doctor command currently:
- Checks both home directory (`~/.swissarmyhammer`) and current directory  
- Uses multiple directory detection approaches
- Provides general system health validation

## New Implementation Approach

### Git Repository Requirement
```rust
pub fn run_doctor() -> Result<(), SwissArmyHammerError> {
    // First, ensure we're in a Git repository
    let git_root = find_git_repository_root()
        .ok_or(SwissArmyHammerError::NotInGitRepository)?;
    
    println!("✅ Git repository detected at: {}", git_root.display());
    
    // Check .swissarmyhammer directory
    let swissarmyhammer_dir = git_root.join(".swissarmyhammer");
    validate_swissarmyhammer_directory(&swissarmyhammer_dir)?;
    
    // Continue with other health checks...
    Ok(())
}
```

### Enhanced Validation Areas
1. **Git Repository Validation**:
   - Confirm `.git` directory exists and is valid
   - Check Git repository health (not bare repository, etc.)
   - Verify working directory is clean for sensitive operations

2. **SwissArmyHammer Directory Validation**:  
   - Validate `.swissarmyhammer` directory structure
   - Check permissions and accessibility
   - Verify subdirectory structure (`memos/`, `todo/`, etc.)

3. **Component Health Checks**:
   - Database file accessibility (`semantic.db`)
   - Memo storage directory and permissions
   - Todo system file permissions  
   - Workflow run storage

## Diagnostic Features
```rust
fn validate_swissarmyhammer_directory(dir: &Path) -> Result<(), SwissArmyHammerError> {
    if !dir.exists() {
        println!("⚠️  .swissarmyhammer directory does not exist (will be created when needed)");
        return Ok(());
    }
    
    println!("✅ .swissarmyhammer directory found: {}", dir.display());
    
    // Check subdirectories
    let subdirs = ["memos", "todo", "runs", "workflows"];
    for subdir in &subdirs {
        let subdir_path = dir.join(subdir);
        if subdir_path.exists() {
            println!("  ✅ {}/", subdir);
        } else {
            println!("  ⚠️  {}/ (will be created when needed)", subdir);
        }
    }
    
    // Check important files
    let semantic_db = dir.join("semantic.db");
    if semantic_db.exists() {
        println!("  ✅ semantic.db");
    } else {
        println!("  ⚠️  semantic.db (will be created when needed)");
    }
    
    Ok(())
}
```

## Error Messaging
Update error messages to guide users:
```rust
SwissArmyHammerError::NotInGitRepository => {
    eprintln!("❌ SwissArmyHammer requires a Git repository");
    eprintln!("");
    eprintln!("Please run this command from within a Git repository.");
    eprintln!("You can create a Git repository with: git init");
    ExitCode::ERROR
}
```

## Tasks  
1. Update `doctor` command to require Git repository
2. Remove multiple directory checking logic
3. Add Git repository health validation
4. Enhance `.swissarmyhammer` directory validation
5. Update error messages and user guidance
6. Add comprehensive tests covering:
   - Doctor command within Git repository
   - Error handling outside Git repository
   - Validation of directory structure and permissions
   - Missing subdirectory scenarios
7. Update CLI help and documentation

## New Features
Add `--migration` flag to doctor command:
```bash
sah doctor --migration  # Scan and validate migration readiness
```

## Dependencies
- Depends on: directory_000003_migration-validation-tools

## Success Criteria
- Doctor command provides clear Git repository requirement feedback
- Comprehensive validation of `.swissarmyhammer` directory structure
- Helpful diagnostic information for troubleshooting
- Clear guidance for users outside Git repository context
- Migration validation feature works correctly
- All tests pass including error scenarios