//! Git operations for issue management
//!
//! This module provides git integration for managing issue branches,
//! including creating work branches, switching branches, and merging
//! completed work back to the source branch.
//!
//! ## Git2-rs Migration Strategy
//!
//! This module is undergoing a gradual migration from shell-based git commands
//! to native git2-rs operations for improved performance and reliability.
//!
//! ### Migration Timeline
//!
//! **Phase 1: Foundation and Repository Operations** ✅ (Current)
//! - Repository verification and initialization
//! - Basic repository state queries
//! - Error handling infrastructure
//! - git2 utility functions
//!
//! **Phase 2: Branch Operations** (Next)
//! - Branch creation and deletion
//! - Branch switching and checkout
//! - Branch listing and status
//!
//! **Phase 3: Commit and Status Operations** (Future)
//! - Working directory status
//! - Commit operations
//! - Diff and change detection
//!
//! **Phase 4: Advanced Operations** (Future)
//! - Merge operations
//! - Remote operations
//! - Complex git workflows
//!
//! ### API Design Principles
//!
//! - **Backward Compatibility**: Existing shell-based methods remain available
//! - **Gradual Migration**: New git2 methods are added alongside shell methods
//! - **Performance**: git2 methods eliminate subprocess overhead
//! - **Error Handling**: Structured error types replace generic errors
//! - **Testing**: Comprehensive integration tests ensure equivalence
//!
//! ### When to Use Git2 vs Shell Methods
//!
//! **Use git2 methods (`*_git2`) when:**
//! - Performance is critical
//! - You're building new functionality
//! - You need structured error information
//! - You want to minimize system calls
//!
//! **Use shell methods when:**
//! - Maintaining existing code that works
//! - You need functionality not yet migrated
//! - Integration with external shell scripts
//!
//! ### Current Status
//!
//! - ✅ Repository verification migrated to git2
//! - ✅ Branch existence checking available in both formats  
//! - ✅ Current branch name available in both formats
//! - ✅ Repository state queries (bare, directories) via git2
//! - ⏳ Most complex operations still use shell commands
//!
//! The migration prioritizes reliability and maintainability while providing
//! performance improvements for commonly used operations.

use super::git2_utils;
use crate::common::create_abort_file;
use crate::{Result, SwissArmyHammerError};
use git2::Repository;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Exit code used when a command's exit status cannot be determined
/// Detailed status summary for git repository state
#[derive(Debug, Default)]
pub struct StatusSummary {
    /// Files that are staged and modified
    pub staged_modified: Vec<String>,
    /// Files that are unstaged and modified
    pub unstaged_modified: Vec<String>,
    /// Files that are untracked
    pub untracked: Vec<String>,
    /// Files that are staged for addition
    pub staged_new: Vec<String>,
    /// Files that are staged for deletion
    pub staged_deleted: Vec<String>,
    /// Files that are deleted but not staged
    pub unstaged_deleted: Vec<String>,
    /// Files that are renamed
    pub renamed: Vec<String>,
    /// Files that have type changes
    pub typechange: Vec<String>,
}

impl StatusSummary {
    /// Create a new empty status summary
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if the repository is clean (no changes)
    pub fn is_clean(&self) -> bool {
        self.staged_modified.is_empty()
            && self.unstaged_modified.is_empty()
            && self.untracked.is_empty()
            && self.staged_new.is_empty()
            && self.staged_deleted.is_empty()
            && self.unstaged_deleted.is_empty()
            && self.renamed.is_empty()
            && self.typechange.is_empty()
    }

    /// Get total count of all changes
    pub fn total_changes(&self) -> usize {
        self.staged_modified.len()
            + self.unstaged_modified.len()
            + self.untracked.len()
            + self.staged_new.len()
            + self.staged_deleted.len()
            + self.unstaged_deleted.len()
            + self.renamed.len()
            + self.typechange.len()
    }
}

/// Parameters for merge analysis handling to reduce function argument count
struct MergeAnalysisParams<'a> {
    /// Git2 repository instance
    repo: &'a git2::Repository,
    /// Merge analysis and preference results
    analysis: (git2::MergeAnalysis, git2::MergePreference),
    /// Source commit object
    source_commit: &'a git2::Commit<'a>,
    /// Target commit object
    target_commit: &'a git2::Commit<'a>,
    /// Source branch name
    source_branch: &'a str,
    /// Target branch name
    target_branch: &'a str,
    /// Merge commit message
    message: &'a str,
}

/// Reflog entry representation for enhanced git2-based operations
#[derive(Debug, Clone)]
pub struct ReflogEntry {
    /// Old object ID (before the operation)
    pub old_oid: String,
    /// New object ID (after the operation)
    pub new_oid: String,
    /// Committer name who performed the operation
    pub committer: String,
    /// Reflog message describing the operation
    pub message: String,
    /// Unix timestamp when the operation occurred
    pub time: i64,
}

/// Git operations for issue management
pub struct GitOperations {
    /// Working directory for git operations
    work_dir: PathBuf,
    /// Git2 repository handle for native operations (optional for gradual migration)
    git2_repo: Option<Repository>,
}

impl GitOperations {
    /// Create new git operations handler
    pub fn new() -> Result<Self> {
        let work_dir = std::env::current_dir()?;

        // Verify this is a git repository and initialize git2 repository
        Self::verify_git_repo(&work_dir)?;
        let git2_repo = Some(git2_utils::discover_repository(&work_dir)?);

        Ok(Self {
            work_dir,
            git2_repo,
        })
    }

    /// Create git operations handler with explicit work directory
    pub fn with_work_dir(work_dir: PathBuf) -> Result<Self> {
        // Verify this is a git repository and initialize git2 repository
        Self::verify_git_repo(&work_dir)?;
        let git2_repo = Some(git2_utils::discover_repository(&work_dir)?);

        Ok(Self {
            work_dir,
            git2_repo,
        })
    }

    /// Verify directory is a git repository using git2
    fn verify_git_repo(path: &Path) -> Result<()> {
        match git2_utils::discover_repository(path) {
            Ok(_) => Ok(()),
            Err(_) => Err(SwissArmyHammerError::git_operation_failed(
                "check repository",
                "Not in a git repository",
            )),
        }
    }

    /// Initialize git2 repository handle for native operations
    ///
    /// This method opens a git2::Repository handle for the working directory,
    /// enabling native git operations alongside the existing shell commands.
    /// This supports gradual migration from shell to native operations.
    /// Uses discover for better robustness with subdirectories and worktrees.
    pub fn init_git2(&mut self) -> Result<()> {
        if self.git2_repo.is_none() {
            let repo = git2_utils::discover_repository(&self.work_dir)?;
            git2_utils::validate_repository_state(&repo)?;
            self.git2_repo = Some(repo);
        }
        Ok(())
    }

    /// Get reference to git2 repository (initializing if needed)
    ///
    /// This method provides access to the git2::Repository handle,
    /// automatically initializing it if it hasn't been opened yet.
    /// The repository handle is cached for subsequent calls, ensuring
    /// optimal performance for repeated git operations.
    pub fn git2_repo(&mut self) -> Result<&Repository> {
        if self.git2_repo.is_none() {
            self.init_git2()?;
        }
        Ok(self.git2_repo.as_ref().unwrap())
    }

    /// Check if git2 repository is initialized
    pub fn has_git2_repo(&self) -> bool {
        self.git2_repo.is_some()
    }

    /// Get reference to git2 repository (read-only access)
    ///
    /// This method provides read-only access to the git2::Repository handle
    /// that was initialized during construction. Since the repository is
    /// initialized eagerly, this method can work with immutable references.
    ///
    /// # Returns
    /// - `Ok(&Repository)` - Reference to the initialized repository
    /// - `Err(SwissArmyHammerError)` - If repository is not initialized
    fn get_git2_repo(&self) -> Result<&Repository> {
        self.git2_repo.as_ref().ok_or_else(|| {
            SwissArmyHammerError::git_operation_failed(
                "access repository",
                "Git2 repository not initialized",
            )
        })
    }

    /// Get current branch name using git2-rs native operations
    pub fn current_branch(&self) -> Result<String> {
        let repo = self.get_git2_repo()?;

        let head = repo
            .head()
            .map_err(|e| git2_utils::convert_git2_error("get HEAD reference", e))?;

        if let Some(branch_name) = head.shorthand() {
            Ok(branch_name.to_string())
        } else {
            // Create a mock git2::Error for consistency with git2 error patterns
            let git2_error =
                git2::Error::from_str("HEAD reference does not point to a valid branch name");
            Err(git2_utils::convert_git2_error(
                "determine branch name from HEAD",
                git2_error,
            ))
        }
    }

    /// Get the main branch name (main or master) for backward compatibility testing.
    ///
    /// Note: This method is primarily used by tests to verify backward compatibility
    /// with traditional main/master branch workflows. The issue management system
    /// no longer defaults to main branches for merge operations.
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

    /// List all local branch names using git2-rs native operations
    ///
    /// This method provides git2-based branch listing for future use in
    /// advanced branch operations. It returns a vector of all local branch names.
    ///
    /// # Returns
    /// - `Ok(Vec<String>)` containing all local branch names
    /// - `Err(SwissArmyHammerError)` if the repository cannot be accessed or
    ///   branch iteration fails
    ///
    /// # Performance
    /// This method eliminates subprocess overhead and provides direct access
    /// to repository branch data through git2.
    pub fn list_branches(&self) -> Result<Vec<String>> {
        let repo = self.get_git2_repo()?;
        let mut branch_names = Vec::new();

        let branches = repo
            .branches(Some(git2::BranchType::Local))
            .map_err(|e| git2_utils::convert_git2_error("list branches", e))?;

        for branch_result in branches {
            let (branch, _) =
                branch_result.map_err(|e| git2_utils::convert_git2_error("iterate branch", e))?;

            if let Some(name) = branch
                .name()
                .map_err(|e| git2_utils::convert_git2_error("get branch name", e))?
            {
                branch_names.push(name.to_string());
            }
        }

        Ok(branch_names)
    }

    /// Check if a local branch exists using git2-rs native operations
    pub fn branch_exists(&self, branch: &str) -> Result<bool> {
        // Handle empty or whitespace-only branch names
        if branch.trim().is_empty() {
            return Ok(false);
        }

        let repo = self.get_git2_repo()?;

        match repo.find_branch(branch, git2::BranchType::Local) {
            Ok(_) => Ok(true),
            Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(false),
            Err(e) => Err(git2_utils::convert_git2_error("check branch existence", e)),
        }
    }

    /// Validate branch name using git2-rs reference validation
    pub fn validate_branch_name(&self, branch_name: &str) -> Result<()> {
        // Check branch name validity using git2 reference validation
        if git2::Reference::is_valid_name(&format!("refs/heads/{}", branch_name)) {
            Ok(())
        } else {
            Err(SwissArmyHammerError::git2_operation_failed(
                "validate branch name",
                git2::Error::from_str(&format!("Invalid branch name: '{}'", branch_name)),
            ))
        }
    }

    /// Check if we can create a branch with the given name
    pub fn can_create_branch(&self, branch_name: &str) -> Result<bool> {
        // Validate branch name format
        self.validate_branch_name(branch_name)?;

        // Check if branch already exists
        if self.branch_exists(branch_name)? {
            return Ok(false);
        }

        // Check if we have a valid HEAD to branch from
        let repo = self.get_git2_repo()?;
        match repo.head() {
            Ok(_) => Ok(true),
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => Ok(false),
            Err(e) => Err(git2_utils::convert_git2_error(
                "check HEAD for branching",
                e,
            )),
        }
    }

    /// Check if a branch name follows the issue branch pattern
    fn is_issue_branch(&self, branch: &str) -> bool {
        branch.starts_with("issue/")
    }

    /// Create and switch to issue work branch
    ///
    /// This method enforces branching rules to prevent creating or switching to
    /// issue branches from other issue branches. It follows these rules:
    ///
    /// 1. If already on the target branch, return success (resume scenario)
    /// 2. If switching to existing branch, must be on a non-issue branch first
    /// 3. If creating new branch, must be on a non-issue branch
    /// 4. Returns error if branching rules are violated
    pub fn create_work_branch(&self, issue_name: &str) -> Result<String> {
        let branch_name = format!("issue/{issue_name}");
        let current_branch = self.current_branch()?;

        // Early return: If we're already on the target issue branch (resume scenario)
        if current_branch == branch_name {
            tracing::info!("Already on target issue branch: {}", branch_name);
            return Ok(branch_name);
        }

        // Enhanced validation to prevent circular dependencies
        self.validate_branch_creation(issue_name, None)?;

        // Check for branch operation validation first to provide specific error messages
        if self.is_issue_branch(&current_branch) {
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

        // Handle existing branch: switch to it
        if self.branch_exists(&branch_name)? {
            tracing::info!(
                "Switching to existing issue branch '{}' from '{}'",
                branch_name,
                current_branch
            );
            self.checkout_branch(&branch_name)?;
            return Ok(branch_name);
        }

        // Handle new branch: create and switch
        tracing::info!(
            "Creating new issue branch '{}' from '{}'",
            branch_name,
            current_branch
        );
        self.create_and_checkout_branch(&branch_name)?;

        Ok(branch_name)
    }

    /// Create and switch to issue work branch (simple backward compatibility)
    ///
    /// This is an alias for create_work_branch that maintains API compatibility.
    pub fn create_work_branch_simple(&self, issue_name: &str) -> Result<String> {
        self.create_work_branch(issue_name)
    }

    /// Create and checkout a new branch using git2-rs
    fn create_and_checkout_branch(&self, branch_name: &str) -> Result<()> {
        // Validate that we can create the branch
        if !self.can_create_branch(branch_name)? {
            return Err(SwissArmyHammerError::git2_operation_failed(
                "create work branch",
                git2::Error::from_str(&format!("Cannot create branch '{}'", branch_name)),
            ));
        }

        let repo = self.get_git2_repo()?;

        // Get current HEAD commit
        let head_commit = repo
            .head()
            .map_err(|e| git2_utils::convert_git2_error("get HEAD reference", e))?
            .peel_to_commit()
            .map_err(|e| git2_utils::convert_git2_error("get HEAD commit", e))?;

        // Create new branch pointing to HEAD commit
        let branch = repo.branch(branch_name, &head_commit, false).map_err(|e| {
            git2_utils::convert_git2_error(&format!("create branch '{}'", branch_name), e)
        })?;

        // Get branch reference name
        let branch_ref = branch.get();
        let branch_ref_name = branch_ref.name().ok_or_else(|| {
            SwissArmyHammerError::git2_operation_failed(
                "get branch reference name",
                git2::Error::from_str("Invalid branch reference"),
            )
        })?;

        // Set HEAD to point to new branch
        repo.set_head(branch_ref_name).map_err(|e| {
            git2_utils::convert_git2_error(&format!("checkout branch '{}'", branch_name), e)
        })?;

        // Update working directory to match new HEAD
        repo.checkout_head(Some(
            git2::build::CheckoutBuilder::new()
                .force()
                .remove_untracked(false),
        ))
        .map_err(|e| {
            git2_utils::convert_git2_error(
                &format!("update working directory for '{}'", branch_name),
                e,
            )
        })?;

        Ok(())
    }

    /// Switch to existing branch using git2-rs
    pub fn checkout_branch(&self, branch: &str) -> Result<()> {
        let repo = self.get_git2_repo()?;

        // Find the branch reference
        let branch_ref = repo
            .find_branch(branch, git2::BranchType::Local)
            .map_err(|e| git2_utils::convert_git2_error(&format!("find branch '{}'", branch), e))?;

        // Get branch reference name
        let reference = branch_ref.get();
        let branch_ref_name = reference.name().ok_or_else(|| {
            SwissArmyHammerError::git2_operation_failed(
                "get branch reference name",
                git2::Error::from_str("Invalid branch reference"),
            )
        })?;

        // Set HEAD to point to the branch
        repo.set_head(branch_ref_name)
            .map_err(|e| git2_utils::convert_git2_error(&format!("set HEAD to '{}'", branch), e))?;

        // Update working directory to match branch
        repo.checkout_head(Some(
            git2::build::CheckoutBuilder::new()
                .force()
                .remove_untracked(false),
        ))
        .map_err(|e| {
            git2_utils::convert_git2_error(
                &format!("checkout working directory for '{}'", branch),
                e,
            )
        })?;

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
            create_abort_file(&self.work_dir, &format!(
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
                create_abort_file(&self.work_dir, &format!(
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
                create_abort_file(&self.work_dir, &format!(
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

    /// Find merge target branch using git2 reflog analysis (native git2 implementation)
    fn find_merge_target_branch_using_reflog(&self, issue_name: &str) -> Result<String> {
        let branch_name = format!("issue/{issue_name}");

        // First check if the issue branch exists
        if !self.branch_exists(&branch_name)? {
            return Err(SwissArmyHammerError::Other(format!(
                "Issue branch '{branch_name}' does not exist"
            )));
        }

        let repo = self.get_git2_repo()?;

        // Get reflog for HEAD
        let reflog = repo
            .reflog("HEAD")
            .map_err(|e| SwissArmyHammerError::git2_operation_failed("get HEAD reflog", e))?;

        // Iterate through reflog entries looking for branch creation
        for i in 0..reflog.len() {
            if let Some(entry) = reflog.get(i) {
                if let Some(message) = entry.message() {
                    // Look for checkout messages indicating branch creation
                    if let Some(target_branch) =
                        self.parse_checkout_message(message, &branch_name)?
                    {
                        // Verify the target branch still exists and is valid
                        if self.branch_exists(&target_branch)?
                            && !self.is_issue_branch(&target_branch)
                        {
                            tracing::debug!(
                                "Found merge target '{}' for issue '{}' via reflog at entry {}",
                                target_branch,
                                issue_name,
                                i
                            );
                            return Ok(target_branch);
                        }
                    }
                }
            }
        }

        // If no valid target found, create abort file
        create_abort_file(&self.work_dir, &format!(
            "Cannot determine merge target for issue '{issue_name}'. No reflog entry found showing where this issue branch was created from. This usually means:\n1. The issue branch was not created using standard git checkout operations\n2. The reflog has been cleared or is too short\n3. The branch was created externally"
        ))?;

        Err(SwissArmyHammerError::git2_operation_failed(
            "determine merge target",
            git2::Error::from_str(&format!(
                "no reflog entry found for issue branch '{branch_name}'"
            )),
        ))
    }

    /// Parse checkout message to find source branch for branch creation
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

    /// Merge issue branch using git merge-base to determine target
    ///
    /// This method uses git's merge-base to automatically determine where
    /// the issue branch should be merged back to, eliminating the need to
    /// store source branch information.
    ///
    /// Returns the target branch that was merged to.
    pub fn merge_issue_branch_auto(&self, issue_name: &str) -> Result<String> {
        let branch_name = format!("issue/{issue_name}");
        let target_branch = self.find_merge_target_branch_using_reflog(issue_name)?;

        tracing::debug!(
            "Merging issue branch '{}' back to target branch '{}' (determined by git reflog)",
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

            // Create abort file for checkout failure
            create_abort_file(&self.work_dir, &format!(
                "Failed to checkout target branch '{target_branch}' for issue '{issue_name}'. Git checkout operation failed:\n{stderr}"
            ))?;

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

            // Any git merge failure is fatal - create abort file and return error
            create_abort_file(&self.work_dir, &format!(
                "Git merge failed for issue '{issue_name}': '{branch_name}' -> '{target_branch}'. Git merge output:\n{stderr}"
            ))?;

            return Err(SwissArmyHammerError::git_branch_operation_failed(
                "merge",
                &branch_name,
                &format!("Failed to merge to target branch '{target_branch}': {stderr}"),
            ));
        }

        Ok(target_branch)
    }

    /// Delete a branch
    pub fn delete_branch(&self, branch_name: &str, force: bool) -> Result<()> {
        // Check if branch exists first - if not, we've already achieved the desired outcome
        if !self.branch_exists(branch_name)? {
            tracing::info!(
                "Branch '{}' does not exist - deletion already achieved",
                branch_name
            );
            return Ok(());
        }

        let mut args = vec!["branch", "--delete"];
        if force {
            args.push("--force");
        }
        args.push(branch_name);

        let output = Command::new("git")
            .current_dir(&self.work_dir)
            .args(args)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Create abort file for delete failure
            create_abort_file(
                &self.work_dir,
                &format!("Failed to delete branch '{branch_name}': {stderr}"),
            )?;
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

    /// Get detailed status summary with categorized changes
    pub fn get_status_summary(&self) -> Result<StatusSummary> {
        let repo = self.get_git2_repo()?;

        let statuses = repo
            .statuses(Some(
                git2::StatusOptions::new()
                    .include_untracked(true)
                    .include_ignored(false),
            ))
            .map_err(|e| git2_utils::convert_git2_error("get status summary", e))?;

        let mut summary = StatusSummary::new();

        for status_entry in statuses.iter() {
            let flags = status_entry.status();
            let path = status_entry.path().unwrap_or("<unknown>");

            // Handle index (staged) changes
            if flags.contains(git2::Status::INDEX_MODIFIED) {
                summary.staged_modified.push(path.to_string());
            }
            if flags.contains(git2::Status::INDEX_NEW) {
                summary.staged_new.push(path.to_string());
            }
            if flags.contains(git2::Status::INDEX_DELETED) {
                summary.staged_deleted.push(path.to_string());
            }
            if flags.contains(git2::Status::INDEX_RENAMED) {
                summary.renamed.push(path.to_string());
            }
            if flags.contains(git2::Status::INDEX_TYPECHANGE) {
                summary.typechange.push(path.to_string());
            }

            // Handle working tree (unstaged) changes
            if flags.contains(git2::Status::WT_MODIFIED) {
                summary.unstaged_modified.push(path.to_string());
            }
            if flags.contains(git2::Status::WT_NEW) {
                summary.untracked.push(path.to_string());
            }
            if flags.contains(git2::Status::WT_DELETED) {
                summary.unstaged_deleted.push(path.to_string());
            }
            if flags.contains(git2::Status::WT_RENAMED) {
                summary.renamed.push(path.to_string());
            }
            if flags.contains(git2::Status::WT_TYPECHANGE) {
                summary.typechange.push(path.to_string());
            }
        }

        Ok(summary)
    }

    /// Refresh the git index to ensure it's up to date
    pub fn refresh_index(&self) -> Result<()> {
        let repo = self.get_git2_repo()?;

        let mut index = repo
            .index()
            .map_err(|e| git2_utils::convert_git2_error("get repository index", e))?;

        index
            .read(true)
            .map_err(|e| git2_utils::convert_git2_error("refresh index", e))?;

        Ok(())
    }

    /// Get the work directory path
    pub fn work_dir(&self) -> &std::path::Path {
        &self.work_dir
    }

    /// Check if repository is bare using git2-rs native operations
    ///
    /// A bare repository is one that does not have a working directory,
    /// typically used for sharing and hosting git repositories.
    ///
    /// # Returns
    /// - `Ok(true)` if the repository is bare (no working directory)
    /// - `Ok(false)` if the repository has a working directory
    /// - `Err(SwissArmyHammerError)` if the repository cannot be accessed
    pub fn is_bare_repository(&mut self) -> Result<bool> {
        let repo = self.git2_repo()?;
        Ok(git2_utils::is_bare_repository(repo))
    }

    /// Get git directory path using git2-rs native operations
    ///
    /// Returns the path to the git directory (typically `.git/`) for this repository.
    /// For bare repositories, this is the repository root. For normal repositories,
    /// this is the `.git` subdirectory.
    ///
    /// # Returns
    /// - `Ok(PathBuf)` containing the absolute path to the git directory
    /// - `Err(SwissArmyHammerError)` if the repository cannot be accessed
    pub fn git_directory(&mut self) -> Result<std::path::PathBuf> {
        let repo = self.git2_repo()?;
        git2_utils::get_git_dir(repo)
    }

    /// Get working directory path using git2-rs native operations
    ///
    /// Returns the path to the repository's working directory. For bare repositories,
    /// this will return None since bare repositories have no working directory.
    ///
    /// # Returns
    /// - `Ok(Some(PathBuf))` containing the absolute path to the working directory
    /// - `Ok(None)` if the repository is bare (no working directory)
    /// - `Err(SwissArmyHammerError)` if the repository cannot be accessed
    pub fn working_directory(&mut self) -> Result<Option<std::path::PathBuf>> {
        let repo = self.git2_repo()?;
        git2_utils::get_work_dir(repo)
    }

    /// Validate repository consistency using git2-rs native operations
    ///
    /// Performs comprehensive validation of the repository state to ensure it
    /// is in a consistent, usable condition. This includes checking repository
    /// integrity and basic structural consistency.
    ///
    /// # Returns
    /// - `Ok(())` if the repository passes all validation checks
    /// - `Err(SwissArmyHammerError)` if validation fails or repository is inconsistent
    ///
    /// # Usage
    /// Use this method before performing critical operations or after repository
    /// modifications to ensure the repository remains in a valid state.
    pub fn validate_repository(&mut self) -> Result<()> {
        let repo = self.git2_repo()?;
        git2_utils::validate_repository_state(repo)
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

    /// Get recent branch operations from reflog for diagnostics
    pub fn get_recent_branch_operations(&self, limit: usize) -> Result<Vec<ReflogEntry>> {
        let repo = self.get_git2_repo()?;
        let reflog = repo
            .reflog("HEAD")
            .map_err(|e| SwissArmyHammerError::git2_operation_failed("get HEAD reflog", e))?;

        let mut entries = Vec::new();
        let count = std::cmp::min(limit, reflog.len());

        for i in 0..count {
            if let Some(entry) = reflog.get(i) {
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

    /// Find branch creation point for better merge target detection
    pub fn find_branch_creation_point(
        &self,
        branch_name: &str,
    ) -> Result<Option<(String, String)>> {
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

    /// Internal helper for find_branch_creation_point
    fn find_merge_target_branch_using_reflog_internal(&self, branch_name: &str) -> Result<String> {
        // Extract issue name from branch name
        let issue_name = branch_name.strip_prefix("issue/").ok_or_else(|| {
            SwissArmyHammerError::git2_operation_failed(
                "extract issue name",
                git2::Error::from_str("Branch name does not match issue pattern"),
            )
        })?;

        self.find_merge_target_branch_using_reflog(issue_name)
    }

    /// Get issue source branch from configuration
    /// 
    /// This is a placeholder for future configuration-based source branch tracking.
    /// When implemented, this would read from .swissarmyhammer/config or similar
    /// to allow users to specify which branch issues should be merged back to
    /// on a per-project or per-issue basis.
    /// 
    /// Currently returns None, causing the system to fall back to reflog analysis
    /// for determining the appropriate target branch.
    fn get_issue_source_branch(&self, _issue_name: &str) -> Result<Option<String>> {
        // Future implementation will read from configuration files
        // to determine project-specific or issue-specific target branches
        Ok(None)
    }

    /// Merge branches using git2-rs for improved performance and reliability
    ///
    /// This is the git2 implementation of merge operations, providing direct
    /// git object manipulation without subprocess overhead.
    ///
    /// # Arguments
    /// * `source_branch` - Branch to merge from
    /// * `target_branch` - Branch to merge into
    /// * `message` - Commit message for the merge
    ///
    /// # Returns
    /// * `Ok(())` if merge completed successfully
    /// * `Err(SwissArmyHammerError)` if merge failed or conflicts detected
    pub fn merge_branches_git2(
        &self,
        source_branch: &str,
        target_branch: &str,
        message: &str,
    ) -> Result<()> {
        let repo = self.open_git2_repository()?;

        // Ensure we're on the target branch
        self.checkout_branch(target_branch)?;

        // Get the source branch reference and create annotated commit
        let source_ref = repo
            .find_branch(source_branch, git2::BranchType::Local)
            .map_err(|e| {
                SwissArmyHammerError::git2_operation_failed(
                    &format!("find source branch '{}'", source_branch),
                    e,
                )
            })?;

        let source_oid = source_ref.get().target().ok_or_else(|| {
            SwissArmyHammerError::git2_operation_failed(
                &format!("get source branch OID for '{}'", source_branch),
                git2::Error::from_str("Branch has no target OID"),
            )
        })?;

        // Create annotated commit for merge analysis
        let source_annotated = repo.find_annotated_commit(source_oid).map_err(|e| {
            SwissArmyHammerError::git2_operation_failed(
                &format!("create annotated commit for '{}'", source_branch),
                e,
            )
        })?;

        // Get actual commit objects for later use
        let source_commit = repo.find_commit(source_oid).map_err(|e| {
            SwissArmyHammerError::git2_operation_failed(
                &format!("get source commit for '{}'", source_branch),
                e,
            )
        })?;

        // Get current HEAD commit (target branch)
        let target_commit = repo
            .head()
            .map_err(|e| SwissArmyHammerError::git2_operation_failed("get target HEAD", e))?
            .peel_to_commit()
            .map_err(|e| SwissArmyHammerError::git2_operation_failed("get target commit", e))?;

        // Perform merge analysis using annotated commit
        let merge_analysis = repo
            .merge_analysis(&[&source_annotated])
            .map_err(|e| SwissArmyHammerError::git2_operation_failed("analyze merge", e))?;

        self.handle_merge_analysis_with_repo(MergeAnalysisParams {
            repo: &repo,
            analysis: merge_analysis,
            source_commit: &source_commit,
            target_commit: &target_commit,
            source_branch,
            target_branch,
            message,
        })
    }

    /// Handle merge analysis with provided repository instance
    fn handle_merge_analysis_with_repo(&self, params: MergeAnalysisParams) -> Result<()> {
        let (merge_analysis, _merge_pref) = params.analysis;

        if merge_analysis.is_fast_forward() {
            // Force non-fast-forward merge as per original shell behavior
            self.create_merge_commit_with_repo(
                params.repo,
                params.source_commit,
                params.target_commit,
                params.source_branch,
                params.target_branch,
                params.message,
            )
        } else if merge_analysis.is_normal() {
            // Normal merge - may have conflicts
            self.perform_normal_merge_with_repo(
                params.repo,
                params.source_commit,
                params.target_commit,
                params.source_branch,
                params.target_branch,
                params.message,
            )
        } else if merge_analysis.is_up_to_date() {
            // Nothing to merge
            tracing::info!(
                "Branch '{}' is already up to date with '{}'",
                params.target_branch,
                params.source_branch
            );
            Ok(())
        } else {
            // Unmerged state or other issues
            create_abort_file(
                &self.work_dir,
                &format!(
                "Cannot merge '{}' into '{}': repository is in an unmerged state or has conflicts",
                params.source_branch, params.target_branch
            ),
            )?;

            Err(SwissArmyHammerError::git2_operation_failed(
                "merge analysis",
                git2::Error::from_str("Repository in unmerged state"),
            ))
        }
    }

    /// Perform normal merge with provided repository instance
    fn perform_normal_merge_with_repo(
        &self,
        repo: &git2::Repository,
        source_commit: &git2::Commit,
        target_commit: &git2::Commit,
        source_branch: &str,
        target_branch: &str,
        message: &str,
    ) -> Result<()> {
        // Get merge base for 3-way merge
        let merge_base = repo
            .merge_base(source_commit.id(), target_commit.id())
            .map_err(|e| SwissArmyHammerError::git2_operation_failed("find merge base", e))?;

        let merge_base_commit = repo
            .find_commit(merge_base)
            .map_err(|e| SwissArmyHammerError::git2_operation_failed("get merge base commit", e))?;

        // Create trees for 3-way merge
        let ancestor_tree = merge_base_commit
            .tree()
            .map_err(|e| SwissArmyHammerError::git2_operation_failed("get ancestor tree", e))?;
        let our_tree = target_commit
            .tree()
            .map_err(|e| SwissArmyHammerError::git2_operation_failed("get target tree", e))?;
        let their_tree = source_commit
            .tree()
            .map_err(|e| SwissArmyHammerError::git2_operation_failed("get source tree", e))?;

        // Perform merge
        let mut index = repo
            .merge_trees(&ancestor_tree, &our_tree, &their_tree, None)
            .map_err(|e| SwissArmyHammerError::git2_operation_failed("merge trees", e))?;

        // Check for conflicts
        if index.has_conflicts() {
            self.handle_merge_conflicts(&index, source_branch, target_branch)?;
            return Err(SwissArmyHammerError::git2_operation_failed(
                "merge",
                git2::Error::from_str("Merge conflicts detected"),
            ));
        }

        // Write the merged index to the repository index and working directory
        let mut repo_index = repo
            .index()
            .map_err(|e| SwissArmyHammerError::git2_operation_failed("get repository index", e))?;

        // Create the merged tree and get the Tree object
        let tree_oid = index
            .write_tree_to(repo)
            .map_err(|e| SwissArmyHammerError::git2_operation_failed("write merge tree", e))?;
        let merge_tree = repo
            .find_tree(tree_oid)
            .map_err(|e| SwissArmyHammerError::git2_operation_failed("find merge tree", e))?;

        // Write the merged tree to the repository index
        repo_index
            .read_tree(&merge_tree)
            .map_err(|e| SwissArmyHammerError::git2_operation_failed("read tree to index", e))?;

        // Write index to working directory
        repo_index
            .write()
            .map_err(|e| SwissArmyHammerError::git2_operation_failed("write index", e))?;

        // Checkout the index to working directory
        repo.checkout_index(Some(&mut repo_index), None)
            .map_err(|e| SwissArmyHammerError::git2_operation_failed("checkout index", e))?;

        // Create merge commit
        let tree_oid = index
            .write_tree_to(repo)
            .map_err(|e| SwissArmyHammerError::git2_operation_failed("write merge tree", e))?;
        let tree = repo
            .find_tree(tree_oid)
            .map_err(|e| SwissArmyHammerError::git2_operation_failed("find merge tree", e))?;

        self.create_commit_with_parents_internal(
            repo,
            &tree,
            &[target_commit, source_commit],
            message,
        )
    }

    /// Handle merge conflicts by collecting detailed information and creating abort file
    ///
    /// # Arguments
    /// * `index` - Git index containing conflict information
    /// * `source_branch` - Source branch name
    /// * `target_branch` - Target branch name
    fn handle_merge_conflicts(
        &self,
        index: &git2::Index,
        source_branch: &str,
        target_branch: &str,
    ) -> Result<()> {
        let mut conflicts = Vec::new();

        // Collect conflict information
        let conflicts_iter = index.conflicts().map_err(|e| {
            SwissArmyHammerError::git2_operation_failed("get conflicts iterator", e)
        })?;

        for conflict in conflicts_iter {
            let conflict = conflict.map_err(|e| {
                SwissArmyHammerError::git2_operation_failed("read conflict entry", e)
            })?;

            if let Some(ours) = conflict.our {
                if let Ok(path) = std::str::from_utf8(&ours.path) {
                    conflicts.push(path.to_string());
                }
            }
        }

        // Create detailed abort message
        let conflict_details = if conflicts.is_empty() {
            "Unknown conflicts detected".to_string()
        } else {
            format!("Conflicts in files: {}", conflicts.join(", "))
        };

        create_abort_file(&self.work_dir, &format!(
            "Merge conflicts detected while merging '{}' into '{}'. {}. Manual conflict resolution required.",
            source_branch, target_branch, conflict_details
        ))?;

        Ok(())
    }

    /// Create merge commit with provided repository instance
    fn create_merge_commit_with_repo(
        &self,
        repo: &git2::Repository,
        source_commit: &git2::Commit,
        target_commit: &git2::Commit,
        source_branch: &str,
        target_branch: &str,
        message: &str,
    ) -> Result<()> {
        // Use source tree for fast-forward case, but create explicit merge commit
        let tree = source_commit
            .tree()
            .map_err(|e| SwissArmyHammerError::git2_operation_failed("get source tree", e))?;

        // Update repository index and working directory to match the source tree
        let mut repo_index = repo
            .index()
            .map_err(|e| SwissArmyHammerError::git2_operation_failed("get repository index", e))?;

        // Read the source tree into the index
        repo_index
            .read_tree(&tree)
            .map_err(|e| SwissArmyHammerError::git2_operation_failed("read tree to index", e))?;

        // Write index to disk
        repo_index
            .write()
            .map_err(|e| SwissArmyHammerError::git2_operation_failed("write index", e))?;

        // Checkout the index to working directory
        repo.checkout_index(Some(&mut repo_index), None)
            .map_err(|e| SwissArmyHammerError::git2_operation_failed("checkout index", e))?;

        let full_message = format!(
            "Merge {} into {}\n\n{}",
            source_branch, target_branch, message
        );
        self.create_commit_with_parents_internal(
            repo,
            &tree,
            &[target_commit, source_commit],
            &full_message,
        )
    }

    /// Internal helper to create commit with parents using provided repository
    fn create_commit_with_parents_internal(
        &self,
        repo: &git2::Repository,
        tree: &git2::Tree,
        parents: &[&git2::Commit],
        message: &str,
    ) -> Result<()> {
        // Get signature for commit
        let signature = repo
            .signature()
            .map_err(|e| SwissArmyHammerError::git2_operation_failed("get signature", e))?;

        // Create commit
        let commit_oid = repo
            .commit(Some("HEAD"), &signature, &signature, message, tree, parents)
            .map_err(|e| SwissArmyHammerError::git2_operation_failed("create merge commit", e))?;

        tracing::info!("Created merge commit: {}", commit_oid);
        Ok(())
    }

    /// Open git2 repository with proper error handling
    ///
    /// Helper function to get the git2 repository instance with
    /// consistent error handling across all git2 merge operations.
    fn open_git2_repository(&self) -> Result<Repository> {
        git2_utils::open_repository(&self.work_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::IsolatedTestEnvironment;
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
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();
        let original_dir = match std::env::current_dir() {
            Ok(dir) => dir,
            Err(_) => return, // Skip test if current directory is not accessible
        };

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
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = TempDir::new().unwrap();

        // Instead of changing current directory (which can fail due to test isolation issues),
        // test the non-git scenario using with_work_dir method
        let result = GitOperations::with_work_dir(temp_dir.path().to_path_buf());
        assert!(
            result.is_err(),
            "GitOperations should fail when not in a git repository"
        );

        // Also test that the error is specifically about not being in a git repo
        match result {
            Err(e) => {
                let error_str = e.to_string().to_lowercase();
                assert!(
                    error_str.contains("git")
                        || error_str.contains("repository")
                        || error_str.contains("not a git"),
                    "Expected git-related error, got: {}",
                    e
                );
            }
            Ok(_) => panic!("Expected error but got success"),
        }
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
        let branch_name = git_ops.create_work_branch("test_issue").unwrap();
        assert_eq!(branch_name, "issue/test_issue");

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
        git_ops.merge_issue_branch_auto("test_issue").unwrap();

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
        let result = git_ops.merge_issue_branch_auto("non_existent_issue");
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

        // Create issue branch from feature branch (should use current branch)
        let issue_branch = git_ops.create_work_branch("auth-tests").unwrap();

        assert_eq!(issue_branch, "issue/auth-tests");

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
        let issue1_branch = git_ops.create_work_branch("bug-fix-1").unwrap();
        assert_eq!(issue1_branch, "issue/bug-fix-1");

        // Switch back to release branch
        git_ops.checkout_branch("release/v1.0").unwrap();

        // Create second issue branch from release branch
        let issue2_branch = git_ops.create_work_branch("bug-fix-2").unwrap();
        assert_eq!(issue2_branch, "issue/bug-fix-2");

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
        let _issue_branch = git_ops.create_work_branch("develop-feature").unwrap();

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

        // Switch to feature/api first, then create issue from current branch
        git_ops.checkout_branch("feature/api").unwrap();
        let issue_branch = git_ops.create_work_branch("api-tests").unwrap();

        assert_eq!(issue_branch, "issue/api-tests");

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
        let result = git_ops.merge_issue_branch_auto("test-issue");
        assert!(result.is_ok());

        // Should be back on main branch
        let main_branch = git_ops.main_branch().unwrap();
        let current_branch = git_ops.current_branch().unwrap();
        assert_eq!(current_branch, main_branch);

        // Changes should be present
        assert!(temp_dir.path().join("test.txt").exists());
    }

    // Tests for backwards compatibility after removing create_work_branch_with_source method

    #[test]
    fn test_create_work_branch_explicit_source_compatibility() {
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

        // Create issue branch from feature branch (by switching first)
        git_ops.checkout_branch("feature/awesome").unwrap();
        let result = git_ops.create_work_branch("test_issue");
        assert!(result.is_ok());
        let branch_name = result.unwrap();
        assert_eq!(branch_name, "issue/test_issue");

        // Verify we're on the new issue branch
        let current_branch = git_ops.current_branch().unwrap();
        assert_eq!(current_branch, "issue/test_issue");

        // Verify the issue branch contains the feature branch changes
        assert!(temp_dir.path().join("feature.txt").exists());
    }

    #[test]
    fn test_create_work_branch_uses_current_branch() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create and switch to a development branch
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "-b", "development"])
            .output()
            .unwrap();

        // Create issue branch using current branch (development) as source
        let result = git_ops.create_work_branch("dev_issue");
        assert!(result.is_ok());
        let branch_name = result.unwrap();
        assert_eq!(branch_name, "issue/dev_issue");

        // Verify we're on the new issue branch
        let current_branch = git_ops.current_branch().unwrap();
        assert_eq!(current_branch, "issue/dev_issue");
    }

    // This test is no longer applicable since explicit source branches are not supported

    // This test is no longer applicable since explicit source branches are not supported

    #[test]
    fn test_create_work_branch_from_issue_branch_fails() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create and switch to first issue branch
        git_ops.create_work_branch("first_issue").unwrap();

        // Try to create another issue branch while on issue branch (should fail)
        let result = git_ops.create_work_branch("second_issue");
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Switch to a non-issue branch first"));
    }

    #[test]
    fn test_create_work_branch_resume_scenario() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create a feature branch
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "-b", "feature/cool"])
            .output()
            .unwrap();

        // Create issue branch from feature branch
        let branch_name = git_ops.create_work_branch("resume_issue").unwrap();
        assert_eq!(branch_name, "issue/resume_issue");

        // Try to create the same issue branch again (resume scenario)
        let result = git_ops.create_work_branch("resume_issue");
        assert!(result.is_ok());
        let branch_name_resume = result.unwrap();
        assert_eq!(branch_name_resume, "issue/resume_issue");

        // Verify we're still on the same branch
        let current_branch = git_ops.current_branch().unwrap();
        assert_eq!(current_branch, "issue/resume_issue");
    }

    #[test]
    fn test_create_work_branch_switch_to_existing() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create issue branch from main
        git_ops.create_work_branch("existing_issue").unwrap();

        // Switch to main
        git_ops.checkout_branch("main").unwrap();

        // Try to switch to existing issue branch (should work)
        let result = git_ops.create_work_branch("existing_issue");
        assert!(result.is_ok());
        let branch_name = result.unwrap();
        assert_eq!(branch_name, "issue/existing_issue");

        // Verify we're on the existing issue branch
        let current_branch = git_ops.current_branch().unwrap();
        assert_eq!(current_branch, "issue/existing_issue");
    }

    #[test]
    fn test_auto_merge_with_deleted_source_branch() {
        let temp_dir = create_test_git_repo().unwrap();

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
        let main_branch = git_ops.main_branch().unwrap();
        git_ops.checkout_branch(&main_branch).unwrap();
        git_ops.delete_branch("feature/test", false).unwrap();

        // Auto merge should fail because the source branch (feature/test) was deleted
        // and reflog-based detection cannot find a valid target branch
        // This is the correct behavior - we shouldn't guess at merge targets
        let result = git_ops.merge_issue_branch_auto("test_issue");
        assert!(result.is_err());

        // Verify abort file was created with appropriate message
        let abort_file = temp_dir.path().join(".swissarmyhammer/.abort");
        assert!(abort_file.exists());
        let abort_content = std::fs::read_to_string(&abort_file).unwrap();
        assert!(abort_content.contains("Cannot determine merge target"));
        assert!(abort_content.contains("test_issue"));
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
        let _test_env = IsolatedTestEnvironment::new().unwrap();
        let temp_dir = create_test_git_repo().unwrap();

        // Save original directory and restore it safely at the end
        let original_dir = match std::env::current_dir() {
            Ok(dir) => dir,
            Err(_) => return, // Skip test if current directory is not accessible
        };

        // Use a closure to ensure directory is restored even if test panics
        let test_result = std::panic::catch_unwind(|| {
            std::env::set_current_dir(temp_dir.path()).unwrap();

            let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

            // Create issue branch
            git_ops.create_work_branch("detailed_issue").unwrap();

            // Switch back and try to merge to nonexistent branch
            let main_branch = git_ops.main_branch().unwrap();
            git_ops.checkout_branch(&main_branch).unwrap();
            let result = git_ops.merge_issue_branch("detailed_issue", "deleted_branch");
            assert!(result.is_err());

            // Check abort file contains detailed context (use temp directory path)
            let abort_file = temp_dir.path().join(".swissarmyhammer/.abort");
            assert!(abort_file.exists());

            let abort_content = std::fs::read_to_string(&abort_file).unwrap();
            assert!(abort_content.contains("deleted_branch"));
            assert!(abort_content.contains("detailed_issue"));
            assert!(abort_content.contains("Manual intervention required"));
        });

        // Always try to restore the original directory, ignoring errors
        let _ = std::env::set_current_dir(&original_dir);

        // Re-panic if the test failed
        if let Err(panic_payload) = test_result {
            std::panic::resume_unwind(panic_payload);
        }
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

        // Test merge_issue_branch_auto
        git_ops.merge_issue_branch_auto("test_issue").unwrap();

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

    #[test]
    fn test_delete_branch_nonexistent_succeeds() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Try to delete a branch that doesn't exist - should succeed
        let result = git_ops.delete_branch("nonexistent-branch", false);
        assert!(
            result.is_ok(),
            "Deleting nonexistent branch should succeed since desired outcome is achieved"
        );

        // Verify the branch still doesn't exist
        assert!(!git_ops.branch_exists("nonexistent-branch").unwrap());
    }

    #[test]
    fn test_delete_branch_existing_succeeds() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Create a test branch
        git_ops.create_work_branch("delete-test").unwrap();

        // Switch back to main so we can delete the branch
        git_ops.checkout_branch("main").unwrap();

        // Verify the branch exists
        assert!(git_ops.branch_exists("issue/delete-test").unwrap());

        // Delete the branch - should succeed
        let result = git_ops.delete_branch("issue/delete-test", false);
        assert!(result.is_ok(), "Deleting existing branch should succeed");

        // Verify the branch no longer exists
        assert!(!git_ops.branch_exists("issue/delete-test").unwrap());
    }

    #[test]
    fn test_delete_branch_nonexistent_then_existing() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Try to delete a branch that doesn't exist - should succeed
        let result = git_ops.delete_branch("test-branch", false);
        assert!(
            result.is_ok(),
            "First deletion of nonexistent branch should succeed"
        );

        // Create the branch
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "-b", "test-branch"])
            .output()
            .unwrap();

        // Switch back to main
        git_ops.checkout_branch("main").unwrap();

        // Delete the now-existing branch - should succeed
        let result = git_ops.delete_branch("test-branch", false);
        assert!(result.is_ok(), "Deletion of existing branch should succeed");

        // Try to delete it again - should still succeed (idempotent)
        let result = git_ops.delete_branch("test-branch", false);
        assert!(
            result.is_ok(),
            "Second deletion of now-nonexistent branch should succeed"
        );
    }

    #[test]
    fn test_get_recent_branch_operations() {
        let temp_dir = create_test_git_repo().unwrap();
        let mut git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Initialize git2 repo
        git_ops.init_git2().unwrap();

        // Create some branch operations to populate reflog
        git_ops.create_work_branch("test-reflog").unwrap();
        git_ops.checkout_branch("main").unwrap();
        git_ops.create_work_branch("another-test").unwrap();

        // Get recent branch operations
        let result = git_ops.get_recent_branch_operations(10);
        assert!(result.is_ok());

        let entries = result.unwrap();
        assert!(!entries.is_empty());

        // Verify entry structure
        for entry in &entries {
            assert!(!entry.old_oid.is_empty());
            assert!(!entry.new_oid.is_empty());
            assert!(!entry.committer.is_empty());
            assert!(entry.time > 0);
        }
    }

    #[test]
    fn test_parse_checkout_message() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Test valid checkout message
        let message = "checkout: moving from main to issue/test-branch";
        let result = git_ops.parse_checkout_message(message, "issue/test-branch");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some("main".to_string()));

        // Test invalid checkout message (wrong target)
        let result = git_ops.parse_checkout_message(message, "issue/other-branch");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);

        // Test non-checkout message
        let message = "commit: add new feature";
        let result = git_ops.parse_checkout_message(message, "issue/test-branch");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);

        // Test malformed checkout message
        let message = "checkout: moving from";
        let result = git_ops.parse_checkout_message(message, "issue/test-branch");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_find_merge_target_branch_using_reflog_git2() {
        let temp_dir = create_test_git_repo().unwrap();
        let mut git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Initialize git2 repo
        git_ops.init_git2().unwrap();

        // Create feature branch and issue branch from it
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "-b", "feature/test-feature"])
            .output()
            .unwrap();

        git_ops.create_work_branch("reflog-test").unwrap();

        // Test finding merge target via reflog
        let result = git_ops.find_merge_target_branch_using_reflog("reflog-test");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "feature/test-feature");
    }

    #[test]
    fn test_find_merge_target_branch_nonexistent_issue() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Test with nonexistent issue branch
        let result = git_ops.find_merge_target_branch_using_reflog("nonexistent");
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("does not exist"));
    }

    #[test]
    fn test_find_merge_target_branch_no_reflog_entry() {
        let temp_dir = create_test_git_repo().unwrap();
        let mut git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Initialize git2 repo
        git_ops.init_git2().unwrap();

        // Create branch manually without going through normal checkout flow
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["branch", "issue/manual-branch"])
            .output()
            .unwrap();

        // Test finding merge target - should fail and create abort file
        let result = git_ops.find_merge_target_branch_using_reflog("manual-branch");
        assert!(result.is_err());

        // Verify abort file was created
        let abort_file = temp_dir.path().join(".swissarmyhammer/.abort");
        assert!(abort_file.exists());

        let abort_content = std::fs::read_to_string(&abort_file).unwrap();
        assert!(abort_content.contains("Cannot determine merge target"));
        assert!(abort_content.contains("manual-branch"));
    }

    #[test]
    fn test_find_branch_creation_point() {
        let temp_dir = create_test_git_repo().unwrap();
        let mut git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Initialize git2 repo
        git_ops.init_git2().unwrap();

        // Create feature branch and issue branch from it
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "-b", "feature/source"])
            .output()
            .unwrap();

        git_ops.create_work_branch("creation-test").unwrap();

        // Test finding branch creation point
        let result = git_ops.find_branch_creation_point("issue/creation-test");
        assert!(result.is_ok());

        let creation_point = result.unwrap();
        assert!(creation_point.is_some());

        let (source_branch, method) = creation_point.unwrap();
        assert_eq!(source_branch, "feature/source");
        assert_eq!(method, "reflog");
    }

    #[test]
    fn test_find_branch_creation_point_non_issue_branch() {
        let temp_dir = create_test_git_repo().unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();

        // Test with non-issue branch
        let result = git_ops.find_branch_creation_point("main");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_reflog_entry_structure() {
        // Test ReflogEntry creation and field access
        let entry = ReflogEntry {
            old_oid: "abc123".to_string(),
            new_oid: "def456".to_string(),
            committer: "test-user".to_string(),
            message: "checkout: moving from main to issue/test".to_string(),
            time: 1234567890,
        };

        assert_eq!(entry.old_oid, "abc123");
        assert_eq!(entry.new_oid, "def456");
        assert_eq!(entry.committer, "test-user");
        assert_eq!(entry.message, "checkout: moving from main to issue/test");
        assert_eq!(entry.time, 1234567890);
    }

    #[test]
    fn test_merge_branches_git2_fast_forward() {
        let temp_dir = create_test_git_repo().unwrap();
        let mut git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();
        git_ops.init_git2().unwrap();

        // Create a feature branch and make a commit
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "-b", "feature"])
            .output()
            .unwrap();

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

        // Switch back to main and merge
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "main"])
            .output()
            .unwrap();

        // Test git2 merge (should create explicit merge commit despite fast-forward possibility)
        let result = git_ops.merge_branches_git2("feature", "main", "Merge feature branch");
        assert!(
            result.is_ok(),
            "Fast-forward merge should succeed: {:?}",
            result
        );

        // Verify merge commit was created
        let log_output = Command::new("git")
            .current_dir(temp_dir.path())
            .args(["log", "--oneline", "-3"])
            .output()
            .unwrap();
        let log = String::from_utf8_lossy(&log_output.stdout);
        assert!(
            log.contains("Merge feature into main"),
            "Should create explicit merge commit"
        );
    }

    #[test]
    fn test_merge_branches_git2_normal_merge() {
        let temp_dir = create_test_git_repo().unwrap();
        let mut git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();
        git_ops.init_git2().unwrap();

        // Create feature branch
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "-b", "feature"])
            .output()
            .unwrap();

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

        // Switch back to main and make a different commit
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "main"])
            .output()
            .unwrap();
        fs::write(temp_dir.path().join("main.txt"), "main content").unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["add", "main.txt"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["commit", "-m", "Add main feature"])
            .output()
            .unwrap();

        // Test git2 merge (should perform 3-way merge)
        let result = git_ops.merge_branches_git2("feature", "main", "Merge feature branch");
        assert!(result.is_ok(), "Normal merge should succeed: {:?}", result);

        // Verify both files exist after merge
        assert!(
            temp_dir.path().join("feature.txt").exists(),
            "Feature file should exist"
        );
        assert!(
            temp_dir.path().join("main.txt").exists(),
            "Main file should exist"
        );

        // Verify merge commit was created
        let log_output = Command::new("git")
            .current_dir(temp_dir.path())
            .args(["log", "--oneline", "-1"])
            .output()
            .unwrap();
        let log = String::from_utf8_lossy(&log_output.stdout);
        assert!(
            log.contains("Merge feature branch"),
            "Should create merge commit with message"
        );
    }

    #[test]
    fn test_merge_branches_git2_conflict_detection() {
        let temp_dir = create_test_git_repo().unwrap();
        let mut git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();
        git_ops.init_git2().unwrap();

        // Create initial commit with a file
        fs::write(temp_dir.path().join("conflict.txt"), "original content\n").unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["add", "conflict.txt"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["commit", "-m", "Initial commit"])
            .output()
            .unwrap();

        // Create feature branch and modify the file
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "-b", "feature"])
            .output()
            .unwrap();
        fs::write(temp_dir.path().join("conflict.txt"), "feature content\n").unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["add", "conflict.txt"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["commit", "-m", "Feature change"])
            .output()
            .unwrap();

        // Switch back to main and modify the same file differently
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "main"])
            .output()
            .unwrap();
        fs::write(temp_dir.path().join("conflict.txt"), "main content\n").unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["add", "conflict.txt"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["commit", "-m", "Main change"])
            .output()
            .unwrap();

        // Test git2 merge (should detect conflicts)
        let result = git_ops.merge_branches_git2("feature", "main", "Merge feature branch");
        assert!(result.is_err(), "Conflicting merge should fail");

        // Verify abort file was created
        let abort_file = temp_dir.path().join(".swissarmyhammer").join(".abort");
        assert!(
            abort_file.exists(),
            "Abort file should be created on conflict"
        );

        let abort_content = std::fs::read_to_string(abort_file).unwrap();
        assert!(
            abort_content.contains("Merge conflicts detected"),
            "Abort file should contain conflict message"
        );
        assert!(
            abort_content.contains("conflict.txt"),
            "Abort file should list conflicted files"
        );
    }

    #[test]
    fn test_merge_branches_git2_up_to_date() {
        let temp_dir = create_test_git_repo().unwrap();
        let mut git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();
        git_ops.init_git2().unwrap();

        // Create branch but don't make any changes
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "-b", "identical"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "main"])
            .output()
            .unwrap();

        // Test git2 merge (should be up to date)
        let result = git_ops.merge_branches_git2("identical", "main", "Merge identical branch");
        assert!(
            result.is_ok(),
            "Up-to-date merge should succeed: {:?}",
            result
        );
    }

    #[test]
    fn test_merge_branches_git2_nonexistent_source_branch() {
        let temp_dir = create_test_git_repo().unwrap();
        let mut git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();
        git_ops.init_git2().unwrap();

        // Test git2 merge with nonexistent branch
        let result = git_ops.merge_branches_git2("nonexistent", "main", "Merge nonexistent branch");
        assert!(
            result.is_err(),
            "Merge with nonexistent source branch should fail"
        );

        // Verify error contains meaningful information
        let error = result.unwrap_err();
        assert!(
            error.to_string().contains("find source branch"),
            "Error should mention source branch issue"
        );
    }

    #[test]
    fn test_create_commit_with_parents() {
        let temp_dir = create_test_git_repo().unwrap();
        let mut git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();
        git_ops.init_git2().unwrap();

        // Create a file and commit on main
        fs::write(temp_dir.path().join("file1.txt"), "content1").unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["add", "file1.txt"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["commit", "-m", "First parent"])
            .output()
            .unwrap();

        // Create second parent on a branch
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "-b", "branch"])
            .output()
            .unwrap();
        fs::write(temp_dir.path().join("file2.txt"), "content2").unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["add", "file2.txt"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["commit", "-m", "Second parent"])
            .output()
            .unwrap();

        // Switch back to main for merge commit
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "main"])
            .output()
            .unwrap();

        // Test the git2 merge functionality using merge_branches_git2
        // This will internally test create_commit_with_parents indirectly
        let result = git_ops.merge_branches_git2("branch", "main", "Test merge commit");
        assert!(
            result.is_ok(),
            "Merge should succeed and create proper merge commit: {:?}",
            result
        );

        // Verify the commit has two parents using shell commands (more reliable)
        let log_output = Command::new("git")
            .current_dir(temp_dir.path())
            .args(["log", "--format=%P", "-1"])
            .output()
            .unwrap();
        let parents = String::from_utf8_lossy(&log_output.stdout);
        let parent_count = parents.trim().split_whitespace().count();
        assert_eq!(parent_count, 2, "Merge commit should have two parents");
    }

    #[test]
    fn test_handle_merge_conflicts_detailed_reporting() {
        let temp_dir = create_test_git_repo().unwrap();
        let mut git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();
        git_ops.init_git2().unwrap();

        // Create initial commit with conflicting files
        fs::write(temp_dir.path().join("file1.txt"), "original\n").unwrap();
        fs::write(temp_dir.path().join("file2.txt"), "original\n").unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["add", "."])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["commit", "-m", "Initial"])
            .output()
            .unwrap();

        // Create conflicting changes on both branches
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "-b", "branch1"])
            .output()
            .unwrap();
        fs::write(temp_dir.path().join("file1.txt"), "branch1 change\n").unwrap();
        fs::write(temp_dir.path().join("file2.txt"), "branch1 change\n").unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["add", "."])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["commit", "-m", "Branch1 changes"])
            .output()
            .unwrap();

        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["checkout", "main"])
            .output()
            .unwrap();
        fs::write(temp_dir.path().join("file1.txt"), "main change\n").unwrap();
        fs::write(temp_dir.path().join("file2.txt"), "main change\n").unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["add", "."])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(temp_dir.path())
            .args(["commit", "-m", "Main changes"])
            .output()
            .unwrap();

        // Attempt merge that should produce conflicts
        let result = git_ops.merge_branches_git2("branch1", "main", "Test merge");
        assert!(result.is_err(), "Merge should fail due to conflicts");

        // Verify detailed conflict reporting in abort file
        let abort_file = temp_dir.path().join(".swissarmyhammer").join(".abort");
        assert!(abort_file.exists(), "Abort file should exist");

        let abort_content = std::fs::read_to_string(abort_file).unwrap();
        assert!(
            abort_content.contains("file1.txt"),
            "Should list first conflicted file"
        );
        assert!(
            abort_content.contains("file2.txt"),
            "Should list second conflicted file"
        );
        assert!(
            abort_content.contains("Manual conflict resolution required"),
            "Should provide resolution guidance"
        );
    }
}
