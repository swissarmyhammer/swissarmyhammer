# OBSOLETE: Issue Directory Migration Specification

**Status**: This specification is obsolete. The issue system has been removed.

**Note**: The `.swissarmyhammer/` directory pattern is still used for todos, rules, and other project metadata.

---

# Original Specification

_The content below describes a planned migration that was never completed before the issue system was removed._

## Overview

This specification outlines the migration of issues storage from the hardcoded root-level `./issues` directory to the standardized `.swissarmyhammer/issues` directory, i.e. the local project .swissarmyhammer directory.

This change improves organization by moving project-specific artifacts into the dedicated project namespace directory.

## Current State Analysis

### Current Directory Structure
```
./
├── issues/                           # Current location (ROOT LEVEL)
│   ├── PLAN_000001_cli-structure.md  
│   ├── PLAN_000002_workflow.md       
│   └── complete/                     # Completed issues
│       ├── 000001_step.md
│       └── ...
├── .swissarmyhammer/                  # Target namespace
│   └── issues/                       # NEW TARGET LOCATION
│       └── complete/
```

### Current Code References

#### 1. Core Storage Implementation
- **File**: `swissarmyhammer/src/issues/filesystem.rs:185-189`
- **Method**: `FileSystemIssueStorage::new_default()`
- **Current Logic**: `current_dir().join("issues")`
- **Impact**: Primary default directory resolution

#### 2. CLI Integration
- **File**: `swissarmyhammer-cli/src/mcp_integration.rs:60-64`
- **Method**: `CliToolContext::create_issue_storage()`
- **Current Logic**: `current_dir.join("issues")`
- **Impact**: CLI commands use this for issue operations

#### 3. MCP Server Integration 
- **File**: `swissarmyhammer-tools/src/mcp/server.rs:118`
- **Current Logic**: `work_dir.join("issues")`
- **Impact**: MCP tools use this directory

#### 4. Test Utilities
- **File**: `swissarmyhammer-tools/src/test_utils.rs:19`
- **Current Logic**: `PathBuf::from("./test_issues")`
- **Impact**: Test isolation uses separate directory

#### 5. Documentation Examples
- **File**: `doc/src/rust-api.md`
- **References**: Multiple examples showing `"./issues"` usage
- **Impact**: Developer documentation and examples

## Target State

### New Directory Structure
```
./
├── .swissarmyhammer/
│   ├── issues/                       # NEW PRIMARY LOCATION
│   │   ├── PLAN_000001_cli-structure.md
│   │   ├── PLAN_000002_workflow.md
│   │   └── complete/                 # Completed issues
│   │       ├── 000001_step.md
│   │       └── ...
```

### New Default Behavior
- **Root project context**: Issues stored in `.swissarmyhammer/issues/`
- **Backward compatibility**: Detect and migrate existing `./issues` directory

## Implementation Plan

### Phase 1: Core Storage Updates

#### 1.1 Update FileSystemIssueStorage Default
```rust
// swissarmyhammer/src/issues/filesystem.rs:185-189
pub fn new_default() -> Result<Self> {
    let current_dir = std::env::current_dir().map_err(SwissArmyHammerError::Io)?;
    
    // New logic: Look for swissarmyhammer directory
    let issues_dir = if current_dir.join(".swissarmyhammer").exists() {
        current_dir.join(".swissarmyhammer").join("issues")
    } else {
        // Fallback to current behavior for backward compatibility
        current_dir.join("issues")
    };
    
    Self::new(issues_dir)
}
```

#### 1.2 Add Migration Helper
```rust
// New method in FileSystemIssueStorage
pub fn migrate_from_legacy_location() -> Result<bool> {
    // Check if ./issues exists and ./swissarmyhammer/issues doesn't
    // If so, move ./issues -> .swissarmyhammer/issues
    // Return true if migration occurred
}
```

### Phase 2: CLI Integration Updates

#### 2.1 Update CLI Storage Creation
```rust
// swissarmyhammer-cli/src/mcp_integration.rs:60-64
fn create_issue_storage(
    current_dir: &std::path::Path,
) -> Result<IssueStorageArc, Box<dyn std::error::Error>> {
    // Use the updated new_default() method instead of hardcoded path
    Ok(Arc::new(RwLock::new(Box::new(
        swissarmyhammer::issues::FileSystemIssueStorage::new_default()?,
    ))))
}
```

### Phase 3: MCP Tools Updates

#### 3.1 Update MCP Server Initialization
```rust
// swissarmyhammer-tools/src/mcp/server.rs:118
// Replace work_dir.join("issues") with:
let issues_dir = if work_dir.join("swissarmyhammer").exists() {
    work_dir.join("swissarmyhammer").join("issues")
} else {
    work_dir.join("issues") // Backward compatibility
};
```

### Phase 4: Test Infrastructure Updates

#### 4.1 Update Test Utilities
```rust
// swissarmyhammer-tools/src/test_utils.rs:19
pub async fn create_test_context() -> ToolContext {
    let issue_storage: Arc<RwLock<Box<dyn IssueStorage>>> = Arc::new(RwLock::new(Box::new(
        FileSystemIssueStorage::new(PathBuf::from("./swissarmyhammer/test_issues")).unwrap(),
    )));
    // ... rest unchanged
}
```

#### 4.2 Update Test Directory Structure
- Create `./swissarmyhammer/test_issues/` for core tests
- Create `./swissarmyhammer-cli/test_issues/` for CLI tests  
- Create `./swissarmyhammer-tools/test_issues/` for tools tests

### Phase 5: Documentation Updates

#### 5.1 Update Rust API Documentation
```rust
// doc/src/rust-api.md - Update all examples:
// OLD: let storage = IssueStorage::new("./issues")?;
// NEW: let storage = IssueStorage::new("./swissarmyhammer/issues")?;

// OLD: let manager = IssueManager::new("./issues")?
// NEW: let manager = IssueManager::new("./swissarmyhammer/issues")?

// OLD: storage_path: "./issues".to_string(),
// NEW: storage_path: "./swissarmyhammer/issues".to_string(),
```

#### 5.2 Update Integration Test Examples
```rust
// Update test examples in issues/PLAN_000012_final-integration-testing.md:
// OLD: [ -d "./issues" ] || { echo "Issues directory not created"; exit 1; }
// NEW: [ -d "./swissarmyhammer/issues" ] || { echo "Issues directory not created"; exit 1; }

// OLD: ls ./issues/SIMPLE_* 
// NEW: ls ./swissarmyhammer/issues/SIMPLE_*
```

### Phase 6: Migration Implementation

#### 6.1 File System Migration
```rust
// New utility function in filesystem.rs
pub fn perform_legacy_migration() -> Result<bool> {
    let current_dir = std::env::current_dir().map_err(SwissArmyHammerError::Io)?;
    let old_issues = current_dir.join("issues");
    let new_issues_parent = current_dir.join("swissarmyhammer");
    let new_issues = new_issues_parent.join("issues");
    
    if old_issues.exists() && !new_issues.exists() {
        // Create swissarmyhammer directory if it doesn't exist
        fs::create_dir_all(&new_issues_parent)?;
        
        // Move ./issues -> ./swissarmyhammer/issues
        fs::rename(old_issues, new_issues)?;
        
        tracing::info!("Migrated issues from ./issues to ./swissarmyhammer/issues");
        return Ok(true);
    }
    
    Ok(false)
}
```

#### 6.2 CLI Migration Command (Optional)
```bash
# Add to CLI
sah issue migrate --from "./issues" --to "./swissarmyhammer/issues"
```


## Migration Strategy

### Automatic Migration
1. **Detection**: Check if `./issues` exists and `.swissarmyhammer/issues` doesn't
2. **Backup**: Create backup of existing issues before migration  
3. **Migration**: Move `./issues` → `.swissarmyhammer/issues`
4. **Validation**: Verify all files moved successfully
5. **Cleanup**: Remove empty `./issues` directory

### Manual Migration  
1. **CLI Command**: `sah issue migrate` for manual control

## Compatibility Considerations

### Backward Compatibility
- **Migration Warning**: Warn users about automatic migration

### Breaking Changes
- **None Expected**: Migration should be transparent to users
- **API Stability**: All public APIs remain unchanged
- **File Format**: No changes to issue file format or structure

## Testing Strategy

### Unit Tests
- [ ] Test `new_default()` directory resolution logic
- [ ] Test migration function with various directory states
- [ ] Test backward compatibility fallback behavior

### Integration Tests  
- [ ] Test CLI commands work with new directory structure
- [ ] Test MCP tools work with new directory structure
- [ ] Test migration preserves all issue data and metadata

### End-to-End Tests
- [ ] Test complete workflow: create → work → complete → merge
- [ ] Test cross-crate issue operations (CLI ↔ MCP ↔ Core)
- [ ] Test migration scenarios with real issue data

## Risk Assessment

### Low Risk
- **API Compatibility**: No public API changes required
- **Data Safety**: File system operations are atomic
- **Rollback**: Easy to reverse migration if needed

### Mitigation Strategies
None

## Success Criteria

### Functional Requirements
- [ ] All issue operations work with new directory structure
- [ ] Automatic migration preserves all existing issues
- [ ] Backward compatibility maintained for legacy setups
- [ ] All tests pass with new directory structure

### Non-Functional Requirements  
- [ ] Migration completes in <5 seconds for typical repositories
- [ ] No performance regression in issue operations
- [ ] Clear error messages for migration failures
- [ ] Comprehensive logging of migration activities

