# GitOperations Integration and API Compatibility

Refer to /Users/wballard/github/sah-skipped/ideas/git.md

## Objective

Integrate all git2-rs operations into the main GitOperations class, ensuring complete API compatibility and seamless migration from shell commands while maintaining backward compatibility.

## Context

This step brings together all the git2 operations implemented in previous steps and integrates them into the GitOperations class as a coherent system. The public API must remain identical to ensure no breaking changes for consumers.

## Tasks

### 1. Update GitOperations Structure

Modify the GitOperations struct to support both backends during transition:

```rust
pub struct GitOperations {
    /// Working directory for git operations
    work_dir: PathBuf,
    
    /// Git2 repository handle (cached after first access)
    git2_repo: Option<Repository>,
    
    /// Migration flag to control which backend to use
    use_git2: bool,
}

impl GitOperations {
    /// Create new git operations handler with git2 backend
    pub fn new() -> Result<Self> {
        let work_dir = std::env::current_dir()?;
        Self::with_work_dir_and_backend(work_dir, true)
    }
    
    /// Create git operations handler with explicit work directory and backend choice
    pub fn with_work_dir_and_backend(work_dir: PathBuf, use_git2: bool) -> Result<Self> {
        // Verify this is a git repository using appropriate backend
        if use_git2 {
            Self::verify_git_repo_git2(&work_dir)?;
        } else {
            Self::verify_git_repo(&work_dir)?;  // Legacy shell method
        }

        Ok(Self { 
            work_dir,
            git2_repo: None,
            use_git2,
        })
    }
    
    /// Verify directory is a git repository using git2
    fn verify_git_repo_git2(path: &Path) -> Result<()> {
        Repository::discover(path)
            .map_err(|e| SwissArmyHammerError::git2_repository_error(
                "Not in a git repository".to_string(), e))?;
        Ok(())
    }
}
```

### 2. Implement Backend Switching Logic

Add methods to handle backend switching for all operations:

```rust
impl GitOperations {
    /// Get current branch using selected backend
    pub fn current_branch(&self) -> Result<String> {
        if self.use_git2 {
            self.current_branch_git2()
        } else {
            self.current_branch_shell()
        }
    }
    
    /// Check if branch exists using selected backend
    pub fn branch_exists(&self, branch: &str) -> Result<bool> {
        if self.use_git2 {
            self.branch_exists_git2(branch)
        } else {
            self.branch_exists_shell(branch)
        }
    }
    
    /// Create work branch using selected backend
    pub fn create_work_branch(&self, issue_name: &str) -> Result<String> {
        if self.use_git2 {
            self.create_work_branch_git2(issue_name)
        } else {
            self.create_work_branch_shell(issue_name)
        }
    }
    
    // ... similar patterns for all operations
}
```

### 3. Implement Fallback Logic

Add robust fallback mechanisms for critical operations:

```rust
impl GitOperations {
    /// Merge with fallback to shell on git2 failures
    pub fn merge_issue_branch_auto(&self, issue_name: &str) -> Result<String> {
        if self.use_git2 {
            match self.merge_issue_branch_auto_git2(issue_name) {
                Ok(target) => Ok(target),
                Err(e) => {
                    tracing::warn!("Git2 merge failed, falling back to shell: {}", e);
                    self.merge_issue_branch_auto_shell(issue_name)
                }
            }
        } else {
            self.merge_issue_branch_auto_shell(issue_name)
        }
    }
    
    /// Repository verification with fallback
    pub fn verify_repository(&self) -> Result<()> {
        if self.use_git2 {
            match Self::verify_git_repo_git2(&self.work_dir) {
                Ok(()) => Ok(()),
                Err(e) => {
                    tracing::warn!("Git2 verification failed, trying shell: {}", e);
                    Self::verify_git_repo(&self.work_dir)
                }
            }
        } else {
            Self::verify_git_repo(&self.work_dir)
        }
    }
}
```

### 4. Add Configuration for Backend Selection

Implement environment-based backend selection:

```rust
impl GitOperations {
    /// Determine which backend to use based on configuration
    fn should_use_git2() -> bool {
        // Check environment variable
        if let Ok(backend) = std::env::var("SAH_GIT_BACKEND") {
            return backend.to_lowercase() == "git2";
        }
        
        // Check if git2 is explicitly disabled
        if std::env::var("SAH_DISABLE_GIT2").is_ok() {
            return false;
        }
        
        // Default to git2 for new installations
        true
    }
    
    /// Create GitOperations with automatic backend selection
    pub fn new_auto() -> Result<Self> {
        let use_git2 = Self::should_use_git2();
        let work_dir = std::env::current_dir()?;
        Self::with_work_dir_and_backend(work_dir, use_git2)
    }
}
```

### 5. Ensure API Compatibility

Preserve all existing public methods with identical signatures:

```rust
impl GitOperations {
    // Existing public API methods - no signature changes
    
    pub fn main_branch(&self) -> Result<String> {
        if self.use_git2 {
            self.main_branch_git2()
        } else {
            self.main_branch_shell()
        }
    }
    
    pub fn checkout_branch(&self, branch: &str) -> Result<()> {
        if self.use_git2 {
            self.checkout_branch_git2(branch)
        } else {
            self.checkout_branch_shell(branch)
        }
    }
    
    pub fn is_working_directory_clean(&self) -> Result<Vec<String>> {
        if self.use_git2 {
            self.is_working_directory_clean_git2()
        } else {
            self.is_working_directory_clean_shell()
        }
    }
    
    pub fn has_uncommitted_changes(&self) -> Result<bool> {
        if self.use_git2 {
            self.has_uncommitted_changes_git2()
        } else {
            self.has_uncommitted_changes_shell()
        }
    }
    
    pub fn get_last_commit_info(&self) -> Result<String> {
        if self.use_git2 {
            self.get_last_commit_info_git2()
        } else {
            self.get_last_commit_info_shell()
        }
    }
    
    // ... all other existing public methods
}
```

### 6. Add Backend Status and Diagnostics

Implement diagnostic methods for backend monitoring:

```rust
impl GitOperations {
    /// Get information about which backend is being used
    pub fn backend_info(&self) -> BackendInfo {
        BackendInfo {
            backend_type: if self.use_git2 { "git2".to_string() } else { "shell".to_string() },
            git2_available: true, // git2 is always available since it's compiled in
            git2_version: git2::version().to_string(),
            shell_available: self.is_shell_git_available(),
            work_dir: self.work_dir.clone(),
            repository_valid: self.verify_repository().is_ok(),
        }
    }
    
    fn is_shell_git_available(&self) -> bool {
        std::process::Command::new("git")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
    
    /// Test both backends for compatibility
    pub fn test_backend_compatibility(&self) -> Result<CompatibilityReport> {
        let mut report = CompatibilityReport::new();
        
        // Test basic operations with both backends
        let operations = vec![
            "current_branch",
            "branch_exists_main", 
            "working_directory_status",
        ];
        
        for op in operations {
            let git2_result = self.test_operation_git2(op);
            let shell_result = self.test_operation_shell(op);
            
            report.add_test(op, git2_result.is_ok(), shell_result.is_ok());
            
            // Compare results if both succeeded
            if let (Ok(git2_val), Ok(shell_val)) = (git2_result, shell_result) {
                report.add_comparison(op, git2_val == shell_val);
            }
        }
        
        Ok(report)
    }
}

#[derive(Debug)]
pub struct BackendInfo {
    pub backend_type: String,
    pub git2_available: bool,
    pub git2_version: String,
    pub shell_available: bool,
    pub work_dir: PathBuf,
    pub repository_valid: bool,
}

#[derive(Debug)]
pub struct CompatibilityReport {
    pub tests: Vec<OperationTest>,
    pub overall_compatible: bool,
}

#[derive(Debug)]
pub struct OperationTest {
    pub operation: String,
    pub git2_success: bool,
    pub shell_success: bool,
    pub results_match: Option<bool>,
}
```

## Implementation Details

```mermaid
graph TD
    A[GitOperations::new] --> B{Backend Selection}
    B -->|git2| C[verify_git_repo_git2]
    B -->|shell| D[verify_git_repo]
    C --> E[Initialize with git2 backend]
    D --> F[Initialize with shell backend]
    
    G[Public Method Call] --> H{use_git2?}
    H -->|Yes| I[Call git2 method]
    H -->|No| J[Call shell method]
    I --> K{Success?}
    K -->|Yes| L[Return result]
    K -->|No| M[Fallback to shell?]
    M -->|Yes| J
    M -->|No| N[Return error]
```

## Acceptance Criteria

- [ ] All existing public API methods preserved with identical signatures
- [ ] Backend selection configurable via environment variables
- [ ] Fallback logic implemented for critical operations
- [ ] No breaking changes to external consumers
- [ ] Both backends can coexist during migration period
- [ ] Diagnostic methods available for backend monitoring
- [ ] Performance metrics available for comparison
- [ ] All existing tests pass without modification

## Testing Requirements

- Test API compatibility with existing code
- Test backend switching and fallback mechanisms
- Test environment variable configuration
- Test diagnostic and monitoring capabilities
- Integration tests with real GitOperations usage
- Performance benchmarks comparing both backends
- Compatibility tests ensuring identical behavior
- Error handling tests for backend failures

## Migration Strategy

1. **Phase 1**: Dual backend support with shell as default
2. **Phase 2**: Git2 as default with shell fallback
3. **Phase 3**: Git2 only with shell fallback for errors
4. **Phase 4**: Git2 only (remove shell code)

## Configuration Options

Environment variables for backend control:
- `SAH_GIT_BACKEND=git2|shell` - Explicit backend selection
- `SAH_DISABLE_GIT2=1` - Disable git2 backend
- `SAH_GIT_FALLBACK=1` - Enable fallback to shell on git2 errors

## Performance Expectations

- Git2 operations should show measurable performance improvements
- Backend switching overhead should be minimal
- Fallback operations should maintain acceptable performance
- Memory usage should not increase significantly

## Dependencies

- All previous git2 migration steps (1-10)
- Existing shell-based GitOperations implementation
- Configuration and error handling infrastructure

## Notes

This step is critical for maintaining backward compatibility while enabling the migration to git2. The dual backend approach allows for gradual rollout and easy rollback if issues are discovered.