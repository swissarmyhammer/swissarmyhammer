# Git2-rs Reflog Analysis Migration

Refer to /Users/wballard/github/sah-skipped/ideas/git.md

## Objective

Migrate reflog analysis operations from shell commands to git2-rs for intelligent merge target detection, replacing shell-based reflog parsing with native git object access.

## Context

The current system uses reflog analysis to determine where issue branches should be merged back to. This is critical for the smart merge target detection that avoids falling back to main branch.

## Current Shell Commands to Migrate

```bash
# Reflog analysis for merge target detection
git reflog --date=local

# Reflog parsing to find branch creation points
grep "checkout: moving from .* to issue/branch_name" (implied in reflog parsing)
```

## Tasks

### 1. Migrate Reflog Analysis

Replace `find_merge_target_branch_using_reflog()` method to use git2:

```rust
// Before (shell)
let output = Command::new("git")
    .args(["reflog", "--date=local"])
    .output()?;

// After (git2)
fn find_merge_target_branch_using_reflog(&self, issue_name: &str) -> Result<String> {
    let repo = self.open_git2_repository()?;
    let branch_name = format!("issue/{}", issue_name);
    
    // Get reflog for HEAD
    let reflog = repo.reflog("HEAD")
        .map_err(|e| SwissArmyHammerError::git2_operation_failed("get HEAD reflog", e))?;
    
    // Iterate through reflog entries looking for branch creation
    for i in 0..reflog.len() {
        if let Some(entry) = reflog.entry(i) {
            if let Some(message) = entry.message() {
                // Look for checkout messages indicating branch creation
                if let Some(target_branch) = self.parse_checkout_message(message, &branch_name)? {
                    // Verify the target branch still exists and is valid
                    if self.branch_exists(&target_branch)? && !self.is_issue_branch(&target_branch) {
                        tracing::debug!(
                            "Found merge target '{}' for issue '{}' via reflog at entry {}",
                            target_branch, issue_name, i
                        );
                        return Ok(target_branch);
                    }
                }
            }
        }
    }
    
    // If no valid target found, create abort file
    create_abort_file(&self.work_dir, &format!(
        "Cannot determine merge target for issue '{}'. No reflog entry found showing where this issue branch was created from.",
        issue_name
    ))?;
    
    Err(SwissArmyHammerError::git2_operation_failed(
        "determine merge target",
        git2::Error::from_str(&format!("no reflog entry found for issue branch '{}'", branch_name))
    ))
}
```

### 2. Implement Reflog Message Parsing

Create helper for parsing checkout messages:

```rust
fn parse_checkout_message(&self, message: &str, target_branch: &str) -> Result<Option<String>> {
    // Parse messages like "checkout: moving from source_branch to target_branch"
    if let Some(checkout_part) = message.strip_prefix("checkout: moving from ") {
        if let Some((from_branch, to_branch)) = checkout_part.split_once(" to ") {
            let to_branch = to_branch.trim();
            let from_branch = from_branch.trim();
            
            // If we moved TO our target branch, the FROM branch is our source
            if to_branch == target_branch {
                return Ok(Some(from_branch.to_string()));
            }
        }
    }
    
    Ok(None)
}
```

### 3. Add Enhanced Reflog Operations

Implement additional reflog functionality for diagnostics:

```rust
pub fn get_recent_branch_operations(&self, limit: usize) -> Result<Vec<ReflogEntry>> {
    let repo = self.open_git2_repository()?;
    let reflog = repo.reflog("HEAD")
        .map_err(|e| SwissArmyHammerError::git2_operation_failed("get HEAD reflog", e))?;
    
    let mut entries = Vec::new();
    let count = std::cmp::min(limit, reflog.len());
    
    for i in 0..count {
        if let Some(entry) = reflog.entry(i) {
            let reflog_entry = ReflogEntry {
                old_oid: entry.id_old().to_string(),
                new_oid: entry.id_new().to_string(),
                committer: entry.committer().name().unwrap_or("unknown").to_string(),
                message: entry.message().unwrap_or("").to_string(),
                time: entry.committer().when().seconds(),
            };
            entries.push(reflog_entry);
        }
    }
    
    Ok(entries)
}

#[derive(Debug, Clone)]
pub struct ReflogEntry {
    pub old_oid: String,
    pub new_oid: String,
    pub committer: String,
    pub message: String,
    pub time: i64,
}
```

### 4. Add Branch History Tracking

Implement branch history tracking for better merge target detection:

```rust
pub fn find_branch_creation_point(&self, branch_name: &str) -> Result<Option<(String, String)>> {
    let repo = self.open_git2_repository()?;
    
    // First try to find in reflog
    if let Ok(target) = self.find_merge_target_branch_using_reflog_internal(branch_name) {
        return Ok(Some((target, "reflog".to_string())));
    }
    
    // Fall back to configuration if available
    if let Some(issue_name) = branch_name.strip_prefix("issue/") {
        if let Ok(Some(source)) = self.get_issue_source_branch(issue_name) {
            if self.branch_exists(&source)? {
                return Ok(Some((source, "config".to_string())));
            }
        }
    }
    
    Ok(None)
}

fn find_merge_target_branch_using_reflog_internal(&self, branch_name: &str) -> Result<String> {
    // Extract issue name from branch name
    let issue_name = branch_name.strip_prefix("issue/")
        .ok_or_else(|| SwissArmyHammerError::git2_operation_failed(
            "extract issue name",
            git2::Error::from_str("Branch name does not match issue pattern")))?;
    
    self.find_merge_target_branch_using_reflog(issue_name)
}
```

## Implementation Details

```mermaid
graph TD
    A[find_merge_target_branch_using_reflog] --> B[repo.reflog HEAD]
    B --> C[iterate reflog entries]
    C --> D[parse_checkout_message]
    D --> E{found target?}
    E -->|Yes| F[validate target branch]
    F --> G{branch valid?}
    G -->|Yes| H[return target]
    G -->|No| C
    E -->|No| C
    C --> I{more entries?}
    I -->|No| J[create abort file]
    J --> K[return error]
```

## Acceptance Criteria

- [ ] `find_merge_target_branch_using_reflog()` uses git2 instead of shell commands
- [ ] Reflog parsing implemented with native git2 reflog access
- [ ] Branch creation point detection accurate and reliable
- [ ] Reflog message parsing handles all checkout message formats
- [ ] Enhanced reflog operations available for diagnostics
- [ ] Performance significantly improved over shell commands
- [ ] All existing behavior preserved exactly

## Testing Requirements

- Test reflog analysis with various branch creation scenarios
- Test reflog parsing with different checkout message formats
- Test reflog analysis in repositories with limited reflog history
- Test branch creation point detection accuracy
- Test performance with large reflog histories
- Test edge cases (corrupted reflog, missing entries)
- Compatibility tests ensuring identical merge target detection

## Error Handling

- Handle missing or corrupted reflog gracefully
- Handle reflog entries with malformed messages
- Handle case where target branch was deleted after issue creation
- Provide informative error messages for debugging
- Create appropriate abort files for unrecoverable situations

## Performance Expectations

- Eliminate subprocess overhead and text parsing for reflog operations
- Faster reflog analysis, especially for large histories
- Direct access to reflog data structures
- Better memory efficiency for reflog processing

## Reflog Message Formats

The system must handle these reflog message patterns:
- `"checkout: moving from main to issue/branch-name"`
- `"checkout: moving from feature/xyz to issue/branch-name"`
- `"checkout: moving from HEAD~1 to issue/branch-name"` (edge case)
- Custom checkout messages from various git workflows

## Dependencies

- Branch creation operations from step 6
- Configuration management from step 5
- Branch detection from step 3
- Repository operations from step 2

## Notes

Reflog analysis is critical for the intelligent merge target detection that distinguishes SwissArmyHammer from simpler git workflow tools. This step must maintain the exact logic while providing better performance and reliability.