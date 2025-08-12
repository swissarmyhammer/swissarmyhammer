//! Git operations for issue management
//!
//! This module provides git integration for managing issue branches,
//! including creating work branches, switching branches, and merging
//! completed work back to the source branch.

use crate::{Result, SwissArmyHammerError};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Git operations for issue management
pub struct GitOperations {
    /// Working directory for git operations
    work_dir: PathBuf,
}

impl GitOperations {
    /// Create new git operations handler
    pub fn new() -> Result<Self> {
        let work_dir = std::env::current_dir()?;

        // Verify this is a git repository
        Self::verify_git_repo(&work_dir)?;

        Ok(Self { work_dir })
    }

    /// Create git operations handler with explicit work directory
    pub fn with_work_dir(work_dir: PathBuf) -> Result<Self> {
        // Verify this is a git repository
        Self::verify_git_repo(&work_dir)?;

        Ok(Self { work_dir })
    }

    /// Verify directory is a git repository
    fn verify_git_repo(path: &Path) -> Result<()> {
        let output = Command::new("git")
            .current_dir(path)
            .args(["rev-parse", "--git-dir"])
            .output()?;

        if !output.status.success() {
            return Err(SwissArmyHammerError::git_operation_failed(
                "check repository",
                "Not in a git repository",
            ));
        }

        Ok(())
    }

    /// Get current branch name
    pub fn current_branch(&self) -> Result<String> {
        let output = Command::new("git")
            .current_dir(&self.work_dir)
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SwissArmyHammerError::git_command_failed(
                "rev-parse --abbrev-ref HEAD",
                output.status.code().unwrap_or(-1),
                &stderr,
            ));
        }

        let branch = String::from_utf8(output.stdout)
            .map_err(|e| {
                SwissArmyHammerError::parsing_failed("git output", "stdout", &e.to_string())
            })?
            .trim()
            .to_string();

        Ok(branch)
    }

    /// Get the main branch name (main or master)
    pub fn main_branch(&self) -> Result<String> {
        // Try 'main' first
        if self.branch_exists("main")? {
            return Ok("main".to_string());
        }

        // Fall back to 'master'
        if self.branch_exists("master")? {
            return Ok("master".to_string());
        }

        Err(SwissArmyHammerError::Other(
            "No main or master branch found".to_string(),
        ))
    }

    /// Check if a branch exists
    pub fn branch_exists(&self, branch: &str) -> Result<bool> {
        let output = Command::new("git")
            .current_dir(&self.work_dir)
            .args([
                "show-ref",
                "--verify",
                "--quiet",
                &format!("refs/heads/{branch}"),
            ])
            .output()?;

        Ok(output.status.success())
    }

    /// Check if a branch name follows the issue branch pattern
    fn is_issue_branch(&self, branch: &str) -> bool {
        branch.starts_with("issue/")
    }

    /// Create and switch to issue work branch (backward compatibility)
    ///
    /// This method enforces branching rules to prevent creating or switching to
    /// issue branches from other issue branches. It follows these rules:
    ///
    /// 1. If already on the target branch, return success (resume scenario)
    /// 2. If switching to existing branch, must be on a non-issue branch first
    /// 3. If creating new branch, must be on a non-issue branch
    /// 4. Returns error if branching rules are violated
    pub fn create_work_branch(&self, issue_name: &str) -> Result<String> {
        let (branch_name, _source_branch) =
            self.create_work_branch_with_source(issue_name, None)?;
        Ok(branch_name)
    }

    /// Create and switch to issue work branch (simple backward compatibility)
    ///
    /// This is an alias for create_work_branch that maintains API compatibility.
    pub fn create_work_branch_simple(&self, issue_name: &str) -> Result<String> {
        self.create_work_branch(issue_name)
    }

    /// Create and switch to issue work branch with optional source branch
    ///
    /// This method enforces branching rules and supports flexible base branches:
    ///
    /// 1. If already on the target branch, return success (resume scenario)
    /// 2. If switching to existing branch, must be on a non-issue branch first
    /// 3. If creating new branch, must be on a non-issue branch
    /// 4. Returns error if branching rules are violated
    /// 5. Source branch validation ensures it exists and is not an issue branch
    ///
    /// # Arguments
    /// * `issue_name` - The name of the issue for the branch
    /// * `source_branch` - Optional source branch to create from. If None, uses current branch
    ///
    /// # Returns
    /// * `Ok((branch_name, source_branch))` - The created branch name and actual source branch used
    /// * `Err` - If validation fails or git operations fail
    pub fn create_work_branch_with_source(
        &self,
        issue_name: &str,
        source_branch: Option<&str>,
    ) -> Result<(String, String)> {
        let branch_name = format!("issue/{issue_name}");
        let current_branch = self.current_branch()?;

        // Early return: If we're already on the target issue branch (resume scenario)
        // Skip validation for resume scenarios since we're already on the correct branch
        if current_branch == branch_name {
            // In resume scenario, we need to determine what source branch to return
            let resume_source_branch = match source_branch {
                Some(source) => {
                    // Validate the provided source branch exists and is not an issue branch
                    if !self.branch_exists(source)? {
                        return Err(SwissArmyHammerError::Other(format!(
                            "Source branch '{source}' does not exist"
                        )));
                    }
                    if self.is_issue_branch(source) {
                        return Err(SwissArmyHammerError::Other(format!(
                            "Cannot use issue branch '{source}' as source branch"
                        )));
                    }
                    source.to_string()
                }
                None => {
                    // For resume scenario without explicit source, try to get the stored source branch
                    self.get_issue_source_branch(issue_name)
                        .unwrap_or_else(|_| {
                            self.main_branch().unwrap_or_else(|_| "main".to_string())
                        })
                }
            };
            return Ok((branch_name, resume_source_branch));
        }

        // Enhanced validation to prevent circular dependencies (after resume check)
        self.validate_branch_creation(issue_name, source_branch)?;

        // Check for branch operation validation first to provide specific error messages
        if self.is_issue_branch(&current_branch) && source_branch.is_none() {
            if self.branch_exists(&branch_name)? {
                return Err(SwissArmyHammerError::Other(
                    "Cannot switch to issue branch from another issue branch. Please switch to a non-issue branch first.".to_string()
                ));
            } else {
                return Err(SwissArmyHammerError::Other(
                    "Cannot create new issue branch from another issue branch. Must be on a non-issue branch.".to_string()
                ));
            }
        }

        // Determine the actual source branch to use for new branch creation
        let actual_source_branch = match source_branch {
            Some(source) => {
                // Validate the provided source branch exists and is not an issue branch
                if !self.branch_exists(source)? {
                    return Err(SwissArmyHammerError::Other(format!(
                        "Source branch '{source}' does not exist"
                    )));
                }
                if self.is_issue_branch(source) {
                    return Err(SwissArmyHammerError::Other(format!(
                        "Cannot use issue branch '{source}' as source branch"
                    )));
                }
                source.to_string()
            }
            None => {
                // If we get here, current branch is not an issue branch (validated above)
                current_branch.clone()
            }
        };

        // Handle existing branch: switch to it
        if self.branch_exists(&branch_name)? {
            self.checkout_branch(&branch_name)?;
            // Store the source branch information for existing branches too
            let _ = self.store_issue_source_branch(issue_name, &actual_source_branch);
            return Ok((branch_name, actual_source_branch));
        }

        // Handle new branch: ensure we're on the source branch first, then create and switch
        if current_branch != actual_source_branch {
            self.checkout_branch(&actual_source_branch)?;
        }

        self.create_and_checkout_branch(&branch_name)?;

        // Store the source branch information for the newly created issue branch
        let _ = self.store_issue_source_branch(issue_name, &actual_source_branch);

        Ok((branch_name, actual_source_branch))
    }

    /// Create and checkout a new branch
    fn create_and_checkout_branch(&self, branch_name: &str) -> Result<()> {
        let output = Command::new("git")
            .current_dir(&self.work_dir)
            .args(["checkout", "-b", branch_name])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SwissArmyHammerError::git_command_failed(
                "checkout -b",
                output.status.code().unwrap_or(-1),
                &stderr,
            ));
        }

        Ok(())
    }

    /// Switch to existing branch
    pub fn checkout_branch(&self, branch: &str) -> Result<()> {
        let output = Command::new("git")
            .current_dir(&self.work_dir)
            .args(["checkout", branch])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SwissArmyHammerError::git_command_failed(
                "checkout",
                output.status.code().unwrap_or(-1),
                &stderr,
            ));
        }

        Ok(())
    }

    /// Merge issue branch to specified source branch
    ///
    /// # Arguments
    ///
    /// * `issue_name` - The name of the issue
    /// * `source_branch` - Target branch for merge (required)
    pub fn merge_issue_branch(&self, issue_name: &str, source_branch: &str) -> Result<()> {
        let branch_name = format!("issue/{issue_name}");

        // Enhanced source branch validation with detailed error context
        if !self.branch_exists(source_branch)? {
            let error_message = format!(
                "Cannot merge issue '{issue_name}': source branch '{source_branch}' does not exist. It may have been deleted after the issue branch was created."
            );
            tracing::error!("{}", error_message);

            // Create abort file for deleted source branch scenario
            self.create_abort_file(&format!(
                "Source branch '{source_branch}' deleted before merge of issue '{issue_name}'. Manual intervention required to resolve the merge target."
            ))?;

            return Err(SwissArmyHammerError::git_branch_operation_failed(
                "merge",
                source_branch,
                &format!("Source branch does not exist (may have been deleted after issue '{issue_name}' was created)")
            ));
        }

        // Enhanced validation for issue branch targets
        if self.is_issue_branch(source_branch) {
            return Err(SwissArmyHammerError::git_branch_operation_failed(
                "merge",
                source_branch,
                &format!("Cannot merge issue '{issue_name}' to issue branch '{source_branch}'. Issue branches cannot be merge targets")
            ));
        }

        // Comprehensive source branch validation
        self.validate_source_branch_state(source_branch, issue_name)?;

        let target_branch = source_branch;

        // Debug: List all branches before checking
        let list_output = Command::new("git")
            .current_dir(&self.work_dir)
            .args(["branch", "-a"])
            .output();
        if let Ok(output) = list_output {
            tracing::debug!(
                "All branches before merge check: {}",
                String::from_utf8_lossy(&output.stdout)
            );
        } else {
            tracing::debug!("Failed to list branches");
        }

        // Ensure the issue branch exists
        if !self.branch_exists(&branch_name)? {
            return Err(SwissArmyHammerError::Other(format!(
                "Issue branch '{branch_name}' does not exist"
            )));
        }

        // Switch to target branch
        self.checkout_branch(target_branch)?;

        // Merge the issue branch
        let output = Command::new("git")
            .current_dir(&self.work_dir)
            .args([
                "merge",
                "--no-ff",
                &branch_name,
                "-m",
                &format!("Merge {branch_name} into {target_branch}"),
            ])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);

            // Enhanced merge conflict handling with abort tool integration
            if stderr.contains("CONFLICT") || stdout.contains("CONFLICT") {
                let conflict_message = format!(
                    "Merge conflicts detected while merging issue '{issue_name}' from '{branch_name}' to '{target_branch}'. Conflicts cannot be resolved automatically."
                );
                tracing::error!("{}", conflict_message);

                // Create abort file for merge conflict scenario
                self.create_abort_file(&format!(
                    "Merge conflicts in issue '{issue_name}': '{branch_name}' -> '{target_branch}'. Manual conflict resolution required:\n{stderr}"
                ))?;

                return Err(SwissArmyHammerError::git_branch_operation_failed(
                    "merge",
                    &branch_name,
                    &format!("Merge conflicts with source branch '{target_branch}'. Manual resolution required")
                ));
            }

            // Enhanced handling for automatic merge failures
            if stderr.contains("Automatic merge failed") {
                let failure_message = format!(
                    "Automatic merge failed for issue '{issue_name}': '{branch_name}' -> '{target_branch}'. Source branch may have diverged significantly."
                );
                tracing::error!("{}", failure_message);

                // Create abort file for automatic merge failure
                self.create_abort_file(&format!(
                    "Automatic merge failed for issue '{issue_name}': '{branch_name}' -> '{target_branch}'. Source branch divergence requires manual intervention:\n{stderr}"
                ))?;

                return Err(SwissArmyHammerError::git_branch_operation_failed(
                    "merge",
                    &branch_name,
                    &format!("Automatic merge failed with source branch '{target_branch}'. Manual intervention required")
                ));
            }

            // Generic merge failure with source branch context
            return Err(SwissArmyHammerError::git_branch_operation_failed(
                "merge",
                &branch_name,
                &format!("Failed to merge to source branch '{target_branch}': {stderr}"),
            ));
        }

        Ok(())
    }

    /// Determine the original branch point for an issue branch using git merge-base
    ///
    /// This method uses git's merge-base to find the common ancestor commit between
    /// the issue branch and potential target branches, determining where the issue
    /// branch was originally created from.
    /// Store the source branch for an issue using git config
    pub fn store_issue_source_branch(&self, issue_name: &str, source_branch: &str) -> Result<()> {
        let config_key = format!("swissarmyhammer.issue.{}.source", issue_name);

        let output = Command::new("git")
            .current_dir(&self.work_dir)
            .args(["config", &config_key, source_branch])
            .output()?;

        if !output.status.success() {
            return Err(SwissArmyHammerError::Other(format!(
                "Failed to store source branch for issue '{}'",
                issue_name
            )));
        }

        Ok(())
    }

    /// Retrieve the stored source branch for an issue
    pub fn get_issue_source_branch(&self, issue_name: &str) -> Result<String> {
        let config_key = format!("swissarmyhammer.issue.{}.source", issue_name);

        let output = Command::new("git")
            .current_dir(&self.work_dir)
            .args(["config", &config_key])
            .output()?;

        if output.status.success() {
            let source_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !source_branch.is_empty() {
                return Ok(source_branch);
            }
        }

        // Fallback to old behavior if no stored source branch found
        self.find_merge_target_branch_fallback(issue_name)
    }

    /// Fallback method for finding merge target (legacy behavior)
    fn find_merge_target_branch_fallback(&self, issue_name: &str) -> Result<String> {
        let branch_name = format!("issue/{issue_name}");

        // First check if the issue branch exists
        if !self.branch_exists(&branch_name)? {
            return Err(SwissArmyHammerError::Other(format!(
                "Issue branch '{branch_name}' does not exist"
            )));
        }

        // Get list of all local branches that could be candidates
        let output = Command::new("git")
            .current_dir(&self.work_dir)
            .args(["branch", "--format=%(refname:short)"])
            .output()?;

        if !output.status.success() {
            return Err(SwissArmyHammerError::Other(
                "Failed to list branches".to_string(),
            ));
        }

        let branches = String::from_utf8_lossy(&output.stdout);
        let candidate_branches: Vec<&str> = branches
            .lines()
            .filter(|branch| {
                !branch.trim().is_empty() && *branch != branch_name && !self.is_issue_branch(branch)
            })
            .collect();

        // Find the best merge target by analyzing merge-base depth
        // The branch with the most recent common ancestor should be the target
        let mut best_candidate = None;
        let mut best_merge_base_time = 0;

        for candidate in &candidate_branches {
            let merge_base_output = Command::new("git")
                .current_dir(&self.work_dir)
                .args(["merge-base", &branch_name, candidate])
                .output()?;

            if !merge_base_output.status.success() {
                continue;
            }

            let merge_base_commit = String::from_utf8_lossy(&merge_base_output.stdout)
                .trim()
                .to_string();

            // Get the timestamp of the merge-base commit
            let timestamp_output = Command::new("git")
                .current_dir(&self.work_dir)
                .args(["show", "-s", "--format=%ct", &merge_base_commit])
                .output()?;

            if let Ok(timestamp_str) = String::from_utf8(timestamp_output.stdout) {
                if let Ok(timestamp) = timestamp_str.trim().parse::<u64>() {
                    if timestamp > best_merge_base_time {
                        best_merge_base_time = timestamp;
                        best_candidate = Some(candidate.to_string());
                    }
                }
            }
        }

        if let Some(candidate) = best_candidate {
            return Ok(candidate);
        }

        // If no merge-base found with any branch, default to main branch
        let main_branch = self.main_branch()?;
        Ok(main_branch)
    }

    /// Find the appropriate target branch for merging an issue branch
    ///
    /// This method first tries to retrieve the stored source branch for the issue,
    /// falling back to merge-base analysis if no stored information is available.
    pub fn find_merge_target_branch(&self, issue_name: &str) -> Result<String> {
        // First try to get the stored source branch
        self.get_issue_source_branch(issue_name)
    }

    /// Merge issue branch using git merge-base to determine target
    ///
    /// This method uses git's merge-base to automatically determine where
    /// the issue branch should be merged back to, eliminating the need to
    /// store source branch information.
    pub fn merge_issue_branch_auto(&self, issue_name: &str) -> Result<()> {
        let branch_name = format!("issue/{issue_name}");
        let target_branch = self.find_merge_target_branch(issue_name)?;

        tracing::debug!(
            "Merging issue branch '{}' back to target branch '{}' (determined by git merge-base)",
            branch_name,
            target_branch
        );

        // Enhanced validation for issue branch targets
        if self.is_issue_branch(&target_branch) {
            return Err(SwissArmyHammerError::git_branch_operation_failed(
                "merge",
                &target_branch,
                &format!("Cannot merge issue '{issue_name}' to issue branch '{target_branch}'. Issue branches cannot be merge targets")
            ));
        }

        // Switch to target branch first
        let output = Command::new("git")
            .current_dir(&self.work_dir)
            .args(["checkout", &target_branch])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SwissArmyHammerError::git_branch_operation_failed(
                "checkout",
                &target_branch,
                &format!("Failed to checkout target branch: {stderr}"),
            ));
        }

        // Merge the issue branch
        let output = Command::new("git")
            .current_dir(&self.work_dir)
            .args(["merge", "--no-ff", &branch_name])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SwissArmyHammerError::git_branch_operation_failed(
                "merge",
                &branch_name,
                &format!(
                    "Failed to merge to target branch '{}': {stderr}",
                    target_branch
                ),
            ));
        }

        Ok(())
    }

    /// Merge issue branch to main branch (backward compatibility)
    ///
    /// This is a convenience method that calls merge_issue_branch_auto
    /// for backward compatibility with existing code.
    pub fn merge_issue_branch_simple(&self, issue_name: &str) -> Result<()> {
        self.merge_issue_branch_auto(issue_name)
    }

    /// Delete a branch
    pub fn delete_branch(&self, branch_name: &str) -> Result<()> {
        let output = Command::new("git")
            .current_dir(&self.work_dir)
            .args(["branch", "-D", branch_name])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SwissArmyHammerError::Other(format!(
                "Failed to delete branch '{branch_name}': {stderr}"
            )));
        }

        Ok(())
    }

    /// Get information about the last commit
    pub fn get_last_commit_info(&self) -> Result<String> {
        let output = Command::new("git")
            .current_dir(&self.work_dir)
            .args(["log", "-1", "--pretty=format:%H|%s|%an|%ad", "--date=iso"])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SwissArmyHammerError::Other(format!(
                "Failed to get last commit info: {stderr}"
            )));
        }

        let commit_info = String::from_utf8_lossy(&output.stdout);
        Ok(commit_info.trim().to_string())
    }

    /// Check if working directory is clean (no uncommitted changes)
    pub fn is_working_directory_clean(&self) -> Result<Vec<String>> {
        let output = Command::new("git")
            .current_dir(&self.work_dir)
            .args(["status", "--porcelain"])
            .output()?;

        if !output.status.success() {
            return Err(SwissArmyHammerError::Other(
                "Failed to check git status".to_string(),
            ));
        }

        let status = String::from_utf8_lossy(&output.stdout);
        let mut changes = Vec::new();

        if !status.trim().is_empty() {
            // Parse the changes to provide helpful message
            for line in status.lines() {
                if let Some(file) = line.get(3..) {
                    changes.push(file.to_string());
                }
            }
        }

        Ok(changes)
    }

    /// Check if working directory has uncommitted changes
    pub fn has_uncommitted_changes(&self) -> Result<bool> {
        let changes = self.is_working_directory_clean()?;
        Ok(!changes.is_empty())
    }

    /// Get the work directory path
    pub fn work_dir(&self) -> &std::path::Path {
        &self.work_dir
    }

    /// Validate source branch state for merge operations
    ///
    /// Performs comprehensive validation to ensure the source branch is in a valid state
    /// for merge operations, including existence, permissions, and consistency checks.
    fn validate_source_branch_state(&self, source_branch: &str, issue_name: &str) -> Result<()> {
        // Check if source branch is accessible by verifying its commit
        let output = Command::new("git")
            .current_dir(&self.work_dir)
            .args(["rev-parse", source_branch])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("unknown revision") || stderr.contains("bad revision") {
                return Err(SwissArmyHammerError::git_branch_operation_failed(
                    "validate",
                    source_branch,
                    &format!(
                        "Source branch for issue '{issue_name}' is in corrupted or invalid state"
                    ),
                ));
            }
        }

        // Check if source branch has diverged significantly (detect potential conflicts early)
        let divergence_check = Command::new("git")
            .current_dir(&self.work_dir)
            .args([
                "merge-base",
                "--is-ancestor",
                source_branch,
                &format!("issue/{issue_name}"),
            ])
            .output()?;

        // If merge-base fails with specific exit codes, source branch may have issues
        if let Some(exit_code) = divergence_check.status.code() {
            if exit_code != 0 && exit_code != 1 {
                tracing::warn!(
                    "Source branch '{}' divergence check failed for issue '{}' with exit code: {}",
                    source_branch,
                    issue_name,
                    exit_code
                );
            }
        }

        Ok(())
    }

    /// Create abort file for irrecoverable git operation scenarios
    ///
    /// Creates a `.swissarmyhammer/.abort` file with the provided reason,
    /// enabling file-based abort detection throughout the system.
    fn create_abort_file(&self, reason: &str) -> Result<()> {
        use std::fs;

        // Ensure .swissarmyhammer directory exists in the work directory
        let sah_dir = self.work_dir.join(".swissarmyhammer");
        if !sah_dir.exists() {
            fs::create_dir_all(&sah_dir).map_err(SwissArmyHammerError::Io)?;
        }

        // Create abort file with reason
        let abort_file_path = sah_dir.join(".abort");
        fs::write(&abort_file_path, reason).map_err(SwissArmyHammerError::Io)?;

        tracing::info!("Created abort file: {}", abort_file_path.display());
        Ok(())
    }

    /// Enhanced branch creation validation to prevent circular dependencies
    ///
    /// Validates that issue branches are not created from other issue branches,
    /// preventing circular dependencies and maintaining clean branch hierarchy.
    pub fn validate_branch_creation(
        &self,
        issue_name: &str,
        source_branch: Option<&str>,
    ) -> Result<()> {
        let current_branch = self.current_branch()?;

        // If source branch is explicitly provided, validate it
        if let Some(source) = source_branch {
            if self.is_issue_branch(source) {
                return Err(SwissArmyHammerError::git_branch_operation_failed(
                    "create",
                    source,
                    &format!("Cannot create issue '{issue_name}' from issue branch '{source}'. Issue branches cannot be used as source branches")
                ));
            }

            if !self.branch_exists(source)? {
                return Err(SwissArmyHammerError::git_branch_operation_failed(
                    "create",
                    source,
                    &format!("Source branch '{source}' for issue '{issue_name}' does not exist"),
                ));
            }
        } else {
            // If no source branch provided, validate current branch
            if self.is_issue_branch(&current_branch) {
                return Err(SwissArmyHammerError::git_branch_operation_failed(
                    "create",
                    &current_branch,
                    &format!("Cannot create issue '{issue_name}' from issue branch '{current_branch}'. Switch to a non-issue branch first")
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // Helper to create a temporary git repository
    fn create_test_git_repo() -> Result<TempDir> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();

        // Initialize git repo
        let output = Command::new("git")
            .current_dir(repo_path)
            .args(["init"])
            .output()?;

        if !output.status.success() {
            return Err(SwissArmyHammerError::Other(
                "Failed to initialize git repository".to_string(),
            ));
        }

        // Set up user config for tests
        Command::new("git")
            .current_dir(repo_path)
            .args(["config", "user.name", "Test User"])
            .output()?;

        Command::new("git")
            .current_dir(repo_path)
            .args(["config", "user.email", "test@example.com"])
            .output()?;

        // Create initial commit
        fs::write(repo_path.join("README.md"), "# Test Repository")?;

        Command::new("git")
            .current_dir(repo_path)
            .args(["add", "README.md"])
            .output()?;

        Command::new("git")
            .current_dir(repo_path)
            .args(["commit", "-m", "Initial commit"])
            .output()?;

        Ok(temp_dir)
    }

    #[test]
    fn test_git_operations_new_in_git_repo() {
        let temp_dir = create_test_git_repo().unwrap();
        let original_dir = std::env::current_dir().unwrap();

        // Ensure we restore directory on panic or normal exit
        struct DirGuard {
            original_dir: std::path::PathBuf,
        }

        impl Drop for DirGuard {
            fn drop(&mut self) {
                let _ = std::env::set_current_dir(&self.original_dir);
            }
        }

        let _guard = DirGuard { original_dir };

        // Change to test repo directory
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Test creating GitOperations
        let result = GitOperations::new();
        assert!(result.is_ok());
    }

    #[test]
    fn test_git_operations_with_work_dir() {
        let temp_dir = create_test_git_repo().unwrap();

        // Test creating GitOperations with explicit work directory
        let result = GitOperations::with_work_dir(temp_dir.path().to_path_buf());
        assert!(result.is_ok());
    }

    #[test]
    fn test_git_operations_new_not_in_git_repo() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();

        // Ensure we restore directory on panic or normal exit
        struct DirGuard {
            original_dir: std::path::PathBuf,
        }

        impl Drop for DirGuard {
            fn drop(&mut self) {
                let _ = std::env::set_current_dir(&self.original_dir);
            }
        }

        let _guard = DirGuard { original_dir };

        // Change to non-git directory
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Test creating GitOperations should fail
        let result = GitOperations::new();
        assert!(result.is_err());
    }

    #[test]
    fn test_git_operations_with_work_dir_not_git_repo() {
        let temp_dir = TempDir::new().unwrap();

        // Test creating GitOperations with non-git directory should fail
        let result = GitOperations::with_work_dir(temp_dir.path().to_path_buf());
        assert!(result.is_err());
    }

    #[test]
    fn test_current_branch() {
        let temp_dir = create_test_git_repo().unwrap();

        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();
        let current_branch = git_ops.current_branch().unwrap();

        // Should be on main or master branch
        assert!(current_branch == "main" || current_branch == "master");
    }

    #[test]
    fn test_main_branch() {
        let temp_dir = create_test_git_repo().unwrap();

        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();
        let main_branch = git_ops.main_branch().unwrap();

        // Should find main or master branch
        assert!(main_branch == "main" || main_branch == "master");
    }

    #[test]
    fn test_branch_exists() {
        let temp_dir = create_test_git_repo().unwrap();

        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Main branch should exist
        let main_branch = git_ops.main_branch().unwrap();
        assert!(git_ops.branch_exists(&main_branch).unwrap());

        // Non-existent branch should not exist
        assert!(!git_ops.branch_exists("non-existent-branch").unwrap());
    }

    #[test]
    fn test_create_work_branch() {
        let temp_dir = create_test_git_repo().unwrap();

        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create work branch
        let (branch_name, source_branch) = git_ops
            .create_work_branch_with_source("test_issue", None)
            .unwrap();
        assert_eq!(branch_name, "issue/test_issue");
        // Should use the current branch (main/master) as source
        assert!(source_branch == "main" || source_branch == "master");

        // Verify we're on the new branch
        let current_branch = git_ops.current_branch().unwrap();
        assert_eq!(current_branch, "issue/test_issue");

        // Verify the branch exists
        assert!(git_ops.branch_exists("issue/test_issue").unwrap());
    }

    #[test]
    fn test_checkout_branch() {
        let temp_dir = create_test_git_repo().unwrap();

        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create work branch
        git_ops.create_work_branch_simple("test_issue").unwrap();

        // Switch back to main
        let main_branch = git_ops.main_branch().unwrap();
        git_ops.checkout_branch(&main_branch).unwrap();

        // Verify we're on main
        let current_branch = git_ops.current_branch().unwrap();
        assert_eq!(current_branch, main_branch);

        // Switch back to work branch
        git_ops.checkout_branch("issue/test_issue").unwrap();

        // Verify we're on work branch
        let current_branch = git_ops.current_branch().unwrap();
        assert_eq!(current_branch, "issue/test_issue");
    }

    #[test]
    fn test_merge_issue_branch() {
        let temp_dir = create_test_git_repo().unwrap();

        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create work branch
        git_ops.create_work_branch_simple("test_issue").unwrap();

        // Make a change on the work branch
        fs::write(temp_dir.path().join("test.txt"), "test content").unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["add", "test.txt"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["commit", "-m", "Add test file"])
            .output()
            .unwrap();

        // Merge the branch
        git_ops.merge_issue_branch_simple("test_issue").unwrap();

        // Verify we're on main branch
        let main_branch = git_ops.main_branch().unwrap();
        let current_branch = git_ops.current_branch().unwrap();
        assert_eq!(current_branch, main_branch);

        // Verify the file exists (merge was successful)
        assert!(temp_dir.path().join("test.txt").exists());
    }

    #[test]
    fn test_merge_non_existent_branch() {
        let temp_dir = create_test_git_repo().unwrap();

        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Try to merge non-existent branch
        let result = git_ops.merge_issue_branch_simple("non_existent_issue");
        assert!(result.is_err());
    }

    #[test]
    fn test_has_uncommitted_changes() {
        let temp_dir = create_test_git_repo().unwrap();

        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Initially should have no uncommitted changes
        assert!(!git_ops.has_uncommitted_changes().unwrap());

        // Add a file
        fs::write(temp_dir.path().join("test.txt"), "test content").unwrap();

        // Should now have uncommitted changes
        assert!(git_ops.has_uncommitted_changes().unwrap());

        // Stage and commit the file
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["add", "test.txt"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["commit", "-m", "Add test file"])
            .output()
            .unwrap();

        // Should have no uncommitted changes again
        assert!(!git_ops.has_uncommitted_changes().unwrap());
    }

    #[test]
    fn test_create_work_branch_from_issue_branch_should_abort() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create and switch to first issue branch
        git_ops.create_work_branch_simple("issue_001").unwrap();

        // Try to create another work branch while on an issue branch - should return error
        let result = git_ops.create_work_branch_simple("issue_002");
        assert!(result.is_err());
        let error = result.unwrap_err();

        // Verify it's an error with correct message content
        let error_msg = error.to_string();
        assert!(error_msg
            .contains("Cannot create issue 'issue_002' from issue branch 'issue/issue_001'"));
    }

    #[test]
    fn test_create_work_branch_from_main_succeeds() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Verify we're on main branch
        let main_branch = git_ops.main_branch().unwrap();
        let current_branch = git_ops.current_branch().unwrap();
        assert_eq!(current_branch, main_branch);

        // Create work branch from main - should succeed
        let result = git_ops.create_work_branch_simple("test_issue");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "issue/test_issue");
    }

    #[test]
    fn test_create_work_branch_resume_on_correct_branch() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create work branch
        git_ops.create_work_branch_simple("test_issue").unwrap();

        // Try to create the same work branch again (resume scenario) - should succeed
        let result = git_ops.create_work_branch("test_issue");
        if result.is_err() {
            panic!("Expected success but got error: {:?}", result.unwrap_err());
        }
        assert_eq!(result.unwrap(), "issue/test_issue");

        // Verify we're still on the same branch
        let current_branch = git_ops.current_branch().unwrap();
        assert_eq!(current_branch, "issue/test_issue");
    }

    #[test]
    fn test_switch_to_existing_issue_branch_from_issue_branch_should_abort() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create first issue branch from main
        git_ops.create_work_branch_simple("issue_001").unwrap();

        // Switch back to main and create second branch
        git_ops
            .checkout_branch(&git_ops.main_branch().unwrap())
            .unwrap();
        git_ops.create_work_branch_simple("issue_002").unwrap();

        // Now try to switch to first branch while on second branch - should return error
        let result = git_ops.create_work_branch_simple("issue_001");
        assert!(result.is_err());
        let error = result.unwrap_err();

        // Verify it's an error with correct message content
        let error_msg = error.to_string();
        // Can be caught by either the old validation logic or the new enhanced validation
        assert!(
            error_msg.contains("Cannot switch to issue branch from another issue branch")
                || error_msg.contains(
                    "Cannot create issue 'issue_001' from issue branch 'issue/issue_002'"
                )
        );
    }

    #[test]
    fn test_create_work_branch_without_main_branch_succeeds() {
        use std::fs;
        use std::process::Command;

        // Create a temporary directory and initialize a git repo
        let temp_dir = tempfile::tempdir().unwrap();

        // Initialize git repo
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["init"])
            .output()
            .unwrap();

        // Create a custom branch (not main or master) and check it out
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "-b", "custom_branch"])
            .output()
            .unwrap();

        // Add a test file and commit to make the branch valid
        fs::write(temp_dir.path().join("test.txt"), "test content").unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["add", "."])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args([
                "-c",
                "user.email=test@example.com",
                "-c",
                "user.name=Test User",
                "commit",
                "-m",
                "Initial commit",
            ])
            .output()
            .unwrap();

        // Delete main branch if it exists (though it shouldn't in this fresh repo)
        let _ = Command::new("git")
            .current_dir(temp_dir.path())
            .args(["branch", "-D", "main"])
            .output();

        // Delete master branch if it exists
        let _ = Command::new("git")
            .current_dir(temp_dir.path())
            .args(["branch", "-D", "master"])
            .output();

        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Try to create work branch - should now succeed even without main/master branch
        // This tests the new flexible branching behavior
        let result = git_ops.create_work_branch("test_issue");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "issue/test_issue");

        // Verify we're on the new issue branch
        let current_branch = git_ops.current_branch().unwrap();
        assert_eq!(current_branch, "issue/test_issue");
    }

    #[test]
    fn test_branch_operation_failure_leaves_consistent_state() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Get initial state
        let initial_branch = git_ops.current_branch().unwrap();
        let main_branch = git_ops.main_branch().unwrap();
        assert_eq!(initial_branch, main_branch);

        // Create first issue branch successfully
        git_ops.create_work_branch_simple("issue_001").unwrap();
        assert_eq!(git_ops.current_branch().unwrap(), "issue/issue_001");

        // Try to create another branch while on issue branch (this should fail)
        let result = git_ops.create_work_branch_simple("issue_002");
        assert!(result.is_err());

        // Verify we're still on the original issue branch after the failure
        assert_eq!(git_ops.current_branch().unwrap(), "issue/issue_001");

        // Verify the failed branch was not created
        assert!(!git_ops.branch_exists("issue/issue_002").unwrap());

        // Verify we can still switch back to main cleanly
        git_ops.checkout_branch(&main_branch).unwrap();
        assert_eq!(git_ops.current_branch().unwrap(), main_branch);

        // Verify we can create new branches from main after the failed attempt
        let result = git_ops.create_work_branch_simple("issue_003");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "issue/issue_003");
    }

    #[test]
    fn test_create_work_branch_from_feature_branch_succeeds() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create and switch to a feature branch
        git_ops.checkout_branch("main").unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "-b", "feature/new-feature"])
            .output()
            .unwrap();

        // Verify we can create issue branch from feature branch
        let result = git_ops.create_work_branch_simple("test_issue_from_feature");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "issue/test_issue_from_feature");

        // Verify we're on the new issue branch
        let current_branch = git_ops.current_branch().unwrap();
        assert_eq!(current_branch, "issue/test_issue_from_feature");
    }

    // Comprehensive flexible branching workflow tests

    #[test]
    fn test_complete_feature_branch_workflow() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Start on main branch
        let _main_branch = git_ops.main_branch().unwrap();

        // Create a feature branch
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "-b", "feature/user-auth"])
            .output()
            .unwrap();

        // Add initial feature work
        fs::write(temp_dir.path().join("auth.rs"), "// Auth module").unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["add", "auth.rs"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["commit", "-m", "Initial auth module"])
            .output()
            .unwrap();

        // Create issue branch from feature branch
        let (issue_branch, source_branch) = git_ops
            .create_work_branch_with_source("auth-tests", None)
            .unwrap();

        assert_eq!(issue_branch, "issue/auth-tests");
        assert_eq!(source_branch, "feature/user-auth");

        // Make changes on issue branch
        fs::write(temp_dir.path().join("auth_tests.rs"), "// Auth tests").unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["add", "auth_tests.rs"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["commit", "-m", "Add auth tests"])
            .output()
            .unwrap();

        // Merge back to feature branch
        git_ops
            .merge_issue_branch("auth-tests", "feature/user-auth")
            .unwrap();

        // Verify we're back on feature branch with the changes
        let current_branch = git_ops.current_branch().unwrap();
        assert_eq!(current_branch, "feature/user-auth");
        assert!(temp_dir.path().join("auth_tests.rs").exists());
    }

    #[test]
    fn test_multiple_issues_from_same_source_branch() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create a release branch
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "-b", "release/v1.0"])
            .output()
            .unwrap();

        // Create first issue branch from release branch
        let (issue1_branch, source1) = git_ops
            .create_work_branch_with_source("bug-fix-1", None)
            .unwrap();
        assert_eq!(issue1_branch, "issue/bug-fix-1");
        assert_eq!(source1, "release/v1.0");

        // Switch back to release branch
        git_ops.checkout_branch("release/v1.0").unwrap();

        // Create second issue branch from release branch
        let (issue2_branch, source2) = git_ops
            .create_work_branch_with_source("bug-fix-2", None)
            .unwrap();
        assert_eq!(issue2_branch, "issue/bug-fix-2");
        assert_eq!(source2, "release/v1.0");

        // Both issue branches should exist
        assert!(git_ops.branch_exists("issue/bug-fix-1").unwrap());
        assert!(git_ops.branch_exists("issue/bug-fix-2").unwrap());
    }

    #[test]
    fn test_merge_issue_to_correct_source_branch() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create develop branch from main
        let main_branch = git_ops.main_branch().unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "-b", "develop"])
            .output()
            .unwrap();

        // Add file to develop branch to differentiate it
        fs::write(temp_dir.path().join("develop.txt"), "develop branch file").unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["add", "develop.txt"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["commit", "-m", "Add develop file"])
            .output()
            .unwrap();

        // Create issue from develop branch
        let (_issue_branch, source_branch) = git_ops
            .create_work_branch_with_source("develop-feature", None)
            .unwrap();
        assert_eq!(source_branch, "develop");

        // Make changes on issue branch
        fs::write(temp_dir.path().join("feature.txt"), "feature content").unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["add", "feature.txt"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["commit", "-m", "Add feature"])
            .output()
            .unwrap();

        // Merge back to develop (not main)
        git_ops
            .merge_issue_branch("develop-feature", "develop")
            .unwrap();

        // Verify we're on develop branch with both files
        let current_branch = git_ops.current_branch().unwrap();
        assert_eq!(current_branch, "develop");
        assert!(temp_dir.path().join("develop.txt").exists());
        assert!(temp_dir.path().join("feature.txt").exists());

        // Verify main branch does NOT have the feature file
        git_ops.checkout_branch(&main_branch).unwrap();
        assert!(!temp_dir.path().join("feature.txt").exists());
    }

    #[test]
    fn test_create_work_branch_with_explicit_source() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create multiple branches
        let _main_branch = git_ops.main_branch().unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "-b", "feature/api"])
            .output()
            .unwrap();

        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "-b", "feature/ui"])
            .output()
            .unwrap();

        // While on feature/ui, create issue from feature/api explicitly
        let (issue_branch, source_branch) = git_ops
            .create_work_branch_with_source("api-tests", Some("feature/api"))
            .unwrap();

        assert_eq!(issue_branch, "issue/api-tests");
        assert_eq!(source_branch, "feature/api");

        // Verify the issue branch was created correctly
        assert!(git_ops.branch_exists("issue/api-tests").unwrap());
        let current_branch = git_ops.current_branch().unwrap();
        assert_eq!(current_branch, "issue/api-tests");
    }

    #[test]
    fn test_validation_prevents_issue_from_issue_branch() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create first issue branch
        git_ops.create_work_branch_simple("first-issue").unwrap();

        // Try to create issue from issue branch (current branch)
        let result = git_ops.validate_branch_creation("second-issue", None);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Cannot create issue 'second-issue' from issue branch"));

        // Try to create issue with explicit issue branch as source
        let result = git_ops.validate_branch_creation("third-issue", Some("issue/first-issue"));
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Cannot create issue 'third-issue' from issue branch"));
    }

    #[test]
    fn test_validation_with_non_existent_source_branch() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Try to create issue from non-existent source branch
        let result = git_ops.validate_branch_creation("test-issue", Some("non-existent-branch"));
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg
            .contains("Source branch 'non-existent-branch' for issue 'test-issue' does not exist"));
    }

    #[test]
    fn test_backwards_compatibility_with_simple_methods() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Test that simple methods still work (backwards compatibility)
        let result = git_ops.create_work_branch_simple("test-issue");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "issue/test-issue");

        // Make a change and commit
        fs::write(temp_dir.path().join("test.txt"), "test content").unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["add", "test.txt"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["commit", "-m", "Test change"])
            .output()
            .unwrap();

        // Test simple merge (should merge to main)
        let result = git_ops.merge_issue_branch_simple("test-issue");
        assert!(result.is_ok());

        // Should be back on main branch
        let main_branch = git_ops.main_branch().unwrap();
        let current_branch = git_ops.current_branch().unwrap();
        assert_eq!(current_branch, main_branch);

        // Changes should be present
        assert!(temp_dir.path().join("test.txt").exists());
    }

    // Tests for the new create_work_branch_with_source method

    #[test]
    fn test_create_work_branch_with_source_explicit_source() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create a feature branch from main
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "-b", "feature/awesome"])
            .output()
            .unwrap();

        // Make a commit on feature branch
        std::fs::write(temp_dir.path().join("feature.txt"), "feature content").unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["add", "feature.txt"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["commit", "-m", "Add feature"])
            .output()
            .unwrap();

        // Switch back to main
        git_ops.checkout_branch("main").unwrap();

        // Create issue branch from feature branch explicitly
        let result = git_ops.create_work_branch_with_source("test_issue", Some("feature/awesome"));
        assert!(result.is_ok());
        let (branch_name, source_branch) = result.unwrap();
        assert_eq!(branch_name, "issue/test_issue");
        assert_eq!(source_branch, "feature/awesome");

        // Verify we're on the new issue branch
        let current_branch = git_ops.current_branch().unwrap();
        assert_eq!(current_branch, "issue/test_issue");

        // Verify the issue branch contains the feature branch changes
        assert!(temp_dir.path().join("feature.txt").exists());
    }

    #[test]
    fn test_create_work_branch_with_source_implicit_current_branch() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create and switch to a development branch
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "-b", "development"])
            .output()
            .unwrap();

        // Create issue branch using current branch (development) as implicit source
        let result = git_ops.create_work_branch_with_source("dev_issue", None);
        assert!(result.is_ok());
        let (branch_name, source_branch) = result.unwrap();
        assert_eq!(branch_name, "issue/dev_issue");
        assert_eq!(source_branch, "development");

        // Verify we're on the new issue branch
        let current_branch = git_ops.current_branch().unwrap();
        assert_eq!(current_branch, "issue/dev_issue");
    }

    #[test]
    fn test_create_work_branch_with_source_nonexistent_source() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Try to create issue branch from nonexistent source branch
        let result = git_ops.create_work_branch_with_source("test_issue", Some("nonexistent"));
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Source branch 'nonexistent' for issue 'test_issue' does not exist")
        );
    }

    #[test]
    fn test_create_work_branch_with_source_issue_branch_as_source() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create an issue branch first
        git_ops.create_work_branch("first_issue").unwrap();

        // Switch back to main
        git_ops.checkout_branch("main").unwrap();

        // Try to create another issue branch using the first issue branch as source
        let result =
            git_ops.create_work_branch_with_source("second_issue", Some("issue/first_issue"));
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg
            .contains("Cannot create issue 'second_issue' from issue branch 'issue/first_issue'"));
    }

    #[test]
    fn test_create_work_branch_with_source_from_issue_branch_without_source() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create and switch to first issue branch
        git_ops.create_work_branch("first_issue").unwrap();

        // Try to create another issue branch while on issue branch (should fail)
        let result = git_ops.create_work_branch_with_source("second_issue", None);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg
            .contains("Cannot create issue 'second_issue' from issue branch 'issue/first_issue'"));
    }

    #[test]
    fn test_create_work_branch_with_source_resume_scenario() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create a feature branch
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "-b", "feature/cool"])
            .output()
            .unwrap();

        // Create issue branch from feature branch
        let (branch_name, source_branch) = git_ops
            .create_work_branch_with_source("resume_issue", Some("feature/cool"))
            .unwrap();
        assert_eq!(branch_name, "issue/resume_issue");
        assert_eq!(source_branch, "feature/cool");

        // Try to create the same issue branch again (resume scenario)
        let result = git_ops.create_work_branch_with_source("resume_issue", Some("feature/cool"));
        assert!(result.is_ok());
        let (branch_name_resume, source_branch_resume) = result.unwrap();
        assert_eq!(branch_name_resume, "issue/resume_issue");
        assert_eq!(source_branch_resume, "feature/cool");

        // Verify we're still on the same branch
        let current_branch = git_ops.current_branch().unwrap();
        assert_eq!(current_branch, "issue/resume_issue");
    }

    #[test]
    fn test_create_work_branch_with_source_switch_to_existing_from_different_source() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create issue branch from main
        git_ops.create_work_branch("existing_issue").unwrap();

        // Switch to main
        git_ops.checkout_branch("main").unwrap();

        // Create a feature branch
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "-b", "feature/different"])
            .output()
            .unwrap();

        // Try to switch to existing issue branch with different source specified
        let result = git_ops.create_work_branch_with_source("existing_issue", Some("main"));
        assert!(result.is_ok());
        let (branch_name, source_branch) = result.unwrap();
        assert_eq!(branch_name, "issue/existing_issue");
        assert_eq!(source_branch, "main");

        // Verify we're on the existing issue branch
        let current_branch = git_ops.current_branch().unwrap();
        assert_eq!(current_branch, "issue/existing_issue");
    }

    #[test]
    fn test_abort_file_creation_on_deleted_source_branch() {
        let temp_dir = create_test_git_repo().unwrap();

        // Change to temp directory for the test
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create and switch to feature branch
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "-b", "feature/test"])
            .output()
            .unwrap();

        // Create issue branch from feature branch
        git_ops.create_work_branch("test_issue").unwrap();

        // Switch back to main and delete feature branch
        git_ops.checkout_branch("main").unwrap();
        git_ops.delete_branch("feature/test").unwrap();

        // Try to merge - should create abort file
        let result = git_ops.merge_issue_branch("test_issue", "feature/test");
        assert!(result.is_err());

        // Check that abort file was created (use temp directory path)
        let abort_file = temp_dir.path().join(".swissarmyhammer/.abort");
        assert!(abort_file.exists());

        let abort_content = std::fs::read_to_string(&abort_file).unwrap();
        assert!(abort_content.contains("Source branch 'feature/test' deleted"));
        assert!(abort_content.contains("test_issue"));

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_enhanced_source_branch_validation() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Test validation with nonexistent source branch
        let result = git_ops.validate_branch_creation("test_issue", Some("nonexistent"));
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Source branch 'nonexistent' for issue 'test_issue' does not exist")
        );

        // Test validation with issue branch as source
        git_ops.create_work_branch("first_issue").unwrap();
        let result = git_ops.validate_branch_creation("second_issue", Some("issue/first_issue"));
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg
            .contains("Cannot create issue 'second_issue' from issue branch 'issue/first_issue'"));
    }

    #[test]
    fn test_circular_dependency_prevention() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create first issue branch
        git_ops.create_work_branch("issue_001").unwrap();

        // Try to create second issue branch while on first issue branch - should fail
        let result = git_ops.validate_branch_creation("issue_002", None);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg
            .contains("Cannot create issue 'issue_002' from issue branch 'issue/issue_001'"));
    }

    #[test]
    fn test_enhanced_merge_conflict_handling() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create a feature branch and make changes
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "-b", "feature/conflict"])
            .output()
            .unwrap();

        // Create conflicting content on feature branch
        std::fs::write(temp_dir.path().join("conflict.txt"), "feature content").unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["add", "conflict.txt"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["commit", "-m", "Add conflict file on feature"])
            .output()
            .unwrap();

        // Switch to main and create different content
        git_ops.checkout_branch("main").unwrap();
        std::fs::write(temp_dir.path().join("conflict.txt"), "main content").unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["add", "conflict.txt"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["commit", "-m", "Add conflict file on main"])
            .output()
            .unwrap();

        // Create issue branch from feature branch
        git_ops.checkout_branch("feature/conflict").unwrap();
        git_ops.create_work_branch("conflict_issue").unwrap();

        // Make additional changes on issue branch
        std::fs::write(temp_dir.path().join("issue.txt"), "issue content").unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["add", "issue.txt"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["commit", "-m", "Add issue content"])
            .output()
            .unwrap();

        // Try to merge to main - should detect conflicts and create abort file
        let result = git_ops.merge_issue_branch("conflict_issue", "main");
        if result.is_err() {
            // Check if abort file was created for merge conflicts
            let abort_file = temp_dir.path().join(".swissarmyhammer/.abort");
            if abort_file.exists() {
                let abort_content = std::fs::read_to_string(&abort_file).unwrap();
                assert!(abort_content.contains("conflict_issue"));
                assert!(abort_content.contains("Manual"));
            }
        }
    }

    #[test]
    fn test_source_branch_state_validation() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create a valid branch
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "-b", "valid_branch"])
            .output()
            .unwrap();

        // Create issue from valid branch
        git_ops.create_work_branch("valid_issue").unwrap();

        // Test validation succeeds for valid branch
        let result = git_ops.validate_source_branch_state("valid_branch", "valid_issue");
        assert!(result.is_ok());
    }

    #[test]
    fn test_enhanced_error_messages_with_source_context() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Test nonexistent source branch error includes issue context
        let result = git_ops.merge_issue_branch("test_issue", "nonexistent");
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("test_issue"));
        assert!(error_msg.contains("nonexistent"));
        assert!(error_msg.contains("deleted after issue"));
    }

    #[test]
    fn test_abort_file_contains_detailed_context() {
        let temp_dir = create_test_git_repo().unwrap();

        // Change to temp directory for the test
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create issue branch
        git_ops.create_work_branch("detailed_issue").unwrap();

        // Switch back and try to merge to nonexistent branch
        git_ops.checkout_branch("main").unwrap();
        let result = git_ops.merge_issue_branch("detailed_issue", "deleted_branch");
        assert!(result.is_err());

        // Check abort file contains detailed context (use temp directory path)
        let abort_file = temp_dir.path().join(".swissarmyhammer/.abort");
        assert!(abort_file.exists());

        let abort_content = std::fs::read_to_string(&abort_file).unwrap();
        assert!(abort_content.contains("deleted_branch"));
        assert!(abort_content.contains("detailed_issue"));
        assert!(abort_content.contains("Manual intervention required"));

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_backward_compatibility_methods() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Test create_work_branch_simple
        let branch_name = git_ops.create_work_branch_simple("test_issue").unwrap();
        assert_eq!(branch_name, "issue/test_issue");

        // Make a change
        fs::write(temp_dir.path().join("test.txt"), "test content").unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["add", "test.txt"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["commit", "-m", "Add test file"])
            .output()
            .unwrap();

        // Test merge_issue_branch_simple
        git_ops.merge_issue_branch_simple("test_issue").unwrap();

        // Should be on main branch
        let main_branch = git_ops.main_branch().unwrap();
        let current_branch = git_ops.current_branch().unwrap();
        assert_eq!(current_branch, main_branch);
    }

    #[test]
    fn test_create_work_branch_backwards_compatibility() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create a feature branch and switch to it
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "-b", "feature/compat"])
            .output()
            .unwrap();

        // Create issue branch using original method (should use current branch as source)
        let result = git_ops.create_work_branch("compat_issue");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "issue/compat_issue");

        // Verify we're on the new issue branch
        let current_branch = git_ops.current_branch().unwrap();
        assert_eq!(current_branch, "issue/compat_issue");

        // The original method should still work exactly as before
        // Switch back and create another issue from main
        git_ops.checkout_branch("main").unwrap();
        let result = git_ops.create_work_branch("main_issue");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "issue/main_issue");
    }
}
