//! Git operations implementation
//!
//! This module provides the main GitOperations struct that handles all git operations
//! using git2-rs for performance and reliability.

use crate::error::{convert_git2_error, GitError, GitResult};
use crate::repository::GitRepository;
use crate::types::{BranchName, CommitInfo, StatusSummary};
use git2::{BranchType, StatusOptions};
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Main struct for performing Git operations
#[derive(Debug)]
pub struct GitOperations {
    /// The underlying git repository
    repo: GitRepository,
    /// Working directory path
    work_dir: PathBuf,
}

impl GitOperations {
    /// Create a new GitOperations instance for the current directory
    pub fn new() -> GitResult<Self> {
        let current_dir = std::env::current_dir()
            .map_err(|e| GitError::from_io("get_current_dir".to_string(), e))?;
        Self::with_work_dir(current_dir)
    }

    /// Create a new GitOperations instance for a specific directory
    pub fn with_work_dir<P: Into<PathBuf>>(work_dir: P) -> GitResult<Self> {
        let work_dir = work_dir.into();
        debug!("Creating GitOperations for directory: {}", work_dir.display());

        let repo = GitRepository::open(&work_dir)?;
        
        Ok(Self { repo, work_dir })
    }

    /// Get the repository instance
    pub fn repository(&self) -> &GitRepository {
        &self.repo
    }

    /// Get the working directory path
    pub fn work_dir(&self) -> &Path {
        &self.work_dir
    }

    /// Check if the current directory is a git repository
    pub fn is_git_repository(&self) -> bool {
        self.repo.is_valid()
    }

    /// Get the current branch name
    pub fn get_current_branch(&self) -> GitResult<Option<BranchName>> {
        let repo = self.repo.inner();
        
        match repo.head() {
            Ok(head_ref) => {
                if let Some(branch_name) = head_ref.shorthand() {
                    Ok(Some(BranchName::new_unchecked(branch_name)))
                } else {
                    Ok(None) // Detached HEAD
                }
            }
            Err(e) => {
                if e.code() == git2::ErrorCode::UnbornBranch {
                    Ok(None) // Empty repository
                } else {
                    Err(convert_git2_error("get_current_branch", e))
                }
            }
        }
    }

    /// List all local branches
    pub fn list_local_branches(&self) -> GitResult<Vec<BranchName>> {
        let repo = self.repo.inner();
        let branches = repo
            .branches(Some(BranchType::Local))
            .map_err(|e| convert_git2_error("list_branches", e))?;

        let mut branch_names = Vec::new();
        for branch_result in branches {
            let (branch, _) = branch_result
                .map_err(|e| convert_git2_error("iterate_branches", e))?;
            
            if let Some(name) = branch.name()
                .map_err(|e| convert_git2_error("get_branch_name", e))? {
                branch_names.push(BranchName::new_unchecked(name));
            }
        }

        Ok(branch_names)
    }

    /// Check if a branch exists
    pub fn branch_exists(&self, branch_name: &BranchName) -> GitResult<bool> {
        let repo = self.repo.inner();
        match repo.find_branch(branch_name.as_str(), BranchType::Local) {
            Ok(_) => Ok(true),
            Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(false),
            Err(e) => Err(convert_git2_error("check_branch_exists", e)),
        }
    }

    /// Create a new branch
    pub fn create_branch(&self, branch_name: &BranchName) -> GitResult<()> {
        debug!("Creating branch: {}", branch_name);
        
        if self.branch_exists(branch_name)? {
            return Err(GitError::branch_already_exists(branch_name.to_string()));
        }

        let repo = self.repo.inner();
        let head_commit = repo
            .head()
            .and_then(|head| head.peel_to_commit())
            .map_err(|e| convert_git2_error("get_head_commit", e))?;

        repo.branch(branch_name.as_str(), &head_commit, false)
            .map_err(|e| convert_git2_error("create_branch", e))?;

        info!("Created branch: {}", branch_name);
        Ok(())
    }

    /// Checkout an existing branch
    pub fn checkout_branch(&self, branch_name: &BranchName) -> GitResult<()> {
        debug!("Checking out branch: {}", branch_name);

        if !self.branch_exists(branch_name)? {
            return Err(GitError::branch_not_found(branch_name.to_string()));
        }

        let repo = self.repo.inner();
        
        // Get the branch reference
        let branch_ref_name = format!("refs/heads/{}", branch_name.as_str());
        let obj = repo
            .revparse_single(&branch_ref_name)
            .map_err(|e| convert_git2_error("resolve_branch", e))?;

        // Checkout the branch
        repo.checkout_tree(&obj, None)
            .map_err(|e| convert_git2_error("checkout_tree", e))?;

        // Update HEAD to point to the new branch
        repo.set_head(&branch_ref_name)
            .map_err(|e| convert_git2_error("set_head", e))?;

        info!("Checked out branch: {}", branch_name);
        Ok(())
    }

    /// Create and checkout a new branch in one operation
    pub fn create_and_checkout_branch(&self, branch_name: &BranchName) -> GitResult<()> {
        debug!("Creating and checking out branch: {}", branch_name);
        self.create_branch(branch_name)?;
        self.checkout_branch(branch_name)?;
        Ok(())
    }

    /// Delete a branch
    pub fn delete_branch(&self, branch_name: &BranchName) -> GitResult<()> {
        debug!("Deleting branch: {}", branch_name);

        let repo = self.repo.inner();
        let mut branch = repo
            .find_branch(branch_name.as_str(), BranchType::Local)
            .map_err(|e| {
                if e.code() == git2::ErrorCode::NotFound {
                    GitError::branch_not_found(branch_name.to_string())
                } else {
                    convert_git2_error("find_branch", e)
                }
            })?;

        branch
            .delete()
            .map_err(|e| convert_git2_error("delete_branch", e))?;

        info!("Deleted branch: {}", branch_name);
        Ok(())
    }

    /// Get the repository status
    pub fn get_status(&self) -> GitResult<StatusSummary> {
        let repo = self.repo.inner();
        let mut opts = StatusOptions::new();
        opts.include_untracked(true);
        opts.include_ignored(false);

        let statuses = repo
            .statuses(Some(&mut opts))
            .map_err(|e| convert_git2_error("get_status", e))?;

        let mut summary = StatusSummary::new();

        for entry in statuses.iter() {
            let path = entry.path().unwrap_or("<unknown>").to_string();
            let status = entry.status();

            if status.contains(git2::Status::INDEX_MODIFIED) {
                summary.staged_modified.push(path.clone());
            }
            if status.contains(git2::Status::WT_MODIFIED) {
                summary.unstaged_modified.push(path.clone());
            }
            if status.contains(git2::Status::WT_NEW) {
                summary.untracked.push(path.clone());
            }
            if status.contains(git2::Status::INDEX_NEW) {
                summary.staged_new.push(path.clone());
            }
            if status.contains(git2::Status::INDEX_DELETED) {
                summary.staged_deleted.push(path.clone());
            }
            if status.contains(git2::Status::WT_DELETED) {
                summary.unstaged_deleted.push(path.clone());
            }
            if status.contains(git2::Status::INDEX_RENAMED) {
                summary.renamed.push(path.clone());
            }
            if status.contains(git2::Status::CONFLICTED) {
                summary.conflicted.push(path);
            }
        }

        Ok(summary)
    }

    /// Check if the working directory is clean
    pub fn is_working_directory_clean(&self) -> GitResult<bool> {
        let status = self.get_status()?;
        Ok(status.is_clean())
    }

    /// Get the latest commit information
    pub fn get_latest_commit(&self) -> GitResult<CommitInfo> {
        let repo = self.repo.inner();
        let head = repo
            .head()
            .map_err(|e| convert_git2_error("get_head", e))?;
        
        let commit = head
            .peel_to_commit()
            .map_err(|e| convert_git2_error("peel_to_commit", e))?;

        let hash = commit.id().to_string();
        let message = commit.message().unwrap_or("").to_string();
        let author = commit.author();
        let author_name = author.name().unwrap_or("").to_string();
        let author_email = author.email().unwrap_or("").to_string();
        let timestamp = chrono::DateTime::from_timestamp(author.when().seconds(), 0)
            .unwrap_or_default();

        Ok(CommitInfo::new(
            hash,
            message,
            author_name,
            author_email,
            timestamp,
        ))
    }

    /// Create a commit with the given message
    pub fn commit(&self, message: &str) -> GitResult<String> {
        let repo = self.repo.inner();
        
        // Get the current index
        let mut index = repo
            .index()
            .map_err(|e| convert_git2_error("get_index", e))?;
        
        let tree_id = index
            .write_tree()
            .map_err(|e| convert_git2_error("write_tree", e))?;
        
        let tree = repo
            .find_tree(tree_id)
            .map_err(|e| convert_git2_error("find_tree", e))?;

        // Get the current HEAD commit (parent)
        let parent_commit = match repo.head() {
            Ok(head) => Some(head.peel_to_commit()
                .map_err(|e| convert_git2_error("peel_to_commit", e))?),
            Err(_) => None, // First commit in repository
        };

        // Create signature
        let signature = repo
            .signature()
            .map_err(|e| convert_git2_error("get_signature", e))?;

        // Create the commit
        let commit_id = match parent_commit {
            Some(parent) => repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &[&parent],
            ),
            None => repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &[],
            ),
        }
        .map_err(|e| convert_git2_error("create_commit", e))?;

        info!("Created commit: {} - {}", commit_id, message);
        Ok(commit_id.to_string())
    }

    /// Add all files to the index
    pub fn add_all(&self) -> GitResult<()> {
        let repo = self.repo.inner();
        let mut index = repo
            .index()
            .map_err(|e| convert_git2_error("get_index", e))?;

        index
            .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .map_err(|e| convert_git2_error("add_all", e))?;

        index
            .write()
            .map_err(|e| convert_git2_error("write_index", e))?;

        debug!("Added all files to index");
        Ok(())
    }

    /// Merge a branch into the current branch
    pub fn merge_branch(&self, source_branch: &BranchName) -> GitResult<()> {
        debug!("Merging branch {} into current branch", source_branch);

        if !self.branch_exists(source_branch)? {
            return Err(GitError::branch_not_found(source_branch.to_string()));
        }

        let repo = self.repo.inner();
        
        // Get the source branch commit
        let source_ref = repo
            .find_reference(&format!("refs/heads/{}", source_branch.as_str()))
            .map_err(|e| convert_git2_error("find_source_reference", e))?;
        
        let source_commit = source_ref
            .peel_to_commit()
            .map_err(|e| convert_git2_error("peel_source_commit", e))?;

        // Get the current HEAD commit
        let head_commit = repo
            .head()
            .and_then(|head| head.peel_to_commit())
            .map_err(|e| convert_git2_error("get_head_commit", e))?;

        // Create an AnnotatedCommit from the source commit for merge analysis
        let annotated_commit = repo
            .find_annotated_commit(source_commit.id())
            .map_err(|e| convert_git2_error("find_annotated_commit", e))?;

        // Perform the merge analysis
        let analysis = repo
            .merge_analysis(&[&annotated_commit])
            .map_err(|e| convert_git2_error("merge_analysis", e))?;

        if analysis.0.is_up_to_date() {
            info!("Branch {} is already up to date", source_branch);
            return Ok(());
        }

        if analysis.0.is_fast_forward() {
            // Fast-forward merge
            let mut head_ref = repo
                .head()
                .map_err(|e| convert_git2_error("get_head_ref", e))?;
            
            head_ref
                .set_target(source_commit.id(), "Fast-forward merge")
                .map_err(|e| convert_git2_error("fast_forward", e))?;
            
            repo.checkout_head(None)
                .map_err(|e| convert_git2_error("checkout_after_merge", e))?;
            
            info!("Fast-forward merged branch {}", source_branch);
        } else {
            // Create merge commit
            let signature = repo
                .signature()
                .map_err(|e| convert_git2_error("get_signature", e))?;
            
            let message = format!("Merge branch '{}'", source_branch);
            let tree_id = repo
                .index()
                .and_then(|mut idx| idx.write_tree())
                .map_err(|e| convert_git2_error("write_merge_tree", e))?;
            
            let tree = repo
                .find_tree(tree_id)
                .map_err(|e| convert_git2_error("find_merge_tree", e))?;

            repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                &message,
                &tree,
                &[&head_commit, &source_commit],
            )
            .map_err(|e| convert_git2_error("create_merge_commit", e))?;
            
            info!("Created merge commit for branch {}", source_branch);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Repository;
    use tempfile::TempDir;

    fn setup_test_repo() -> (TempDir, GitOperations) {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let repo_path = temp_dir.path();

        println!("Setting up test repo at: {:?}", repo_path);

        // Initialize repository directly
        let repo = Repository::init(repo_path).expect("Failed to init repository");
        println!("Repository initialized");
        
        // Configure git for testing
        let mut config = repo.config().expect("Failed to get config");
        config.set_str("user.name", "Test User").expect("Failed to set user.name");
        config.set_str("user.email", "test@example.com").expect("Failed to set user.email");
        println!("Repository configured");

        // Create a file and add it to create a proper initial commit
        std::fs::write(repo_path.join("README.md"), "# Test Repository\n").expect("Failed to write README");
        
        let mut index = repo.index().expect("Failed to get index");
        index.add_path(std::path::Path::new("README.md")).expect("Failed to add file to index");
        index.write().expect("Failed to write index");
        println!("File added to index");
        
        let signature = git2::Signature::now("Test User", "test@example.com").expect("Failed to create signature");
        let tree_id = index.write_tree().expect("Failed to write tree");
        let tree = repo.find_tree(tree_id).expect("Failed to find tree");
        
        // Create the initial commit
        let commit_id = repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &tree,
            &[]
        ).expect("Failed to create initial commit");
        println!("Initial commit created: {}", commit_id);

        let git_ops = GitOperations::with_work_dir(repo_path.to_path_buf())
            .expect("Failed to create GitOperations");
        println!("GitOperations created");

        (temp_dir, git_ops)
    }

    #[test]
    fn test_branch_operations() {
        // Create a simple temporary repo without using setup_test_repo
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let repo_path = temp_dir.path();
        
        // Initialize and configure repository
        let repo = Repository::init(repo_path).expect("Failed to init repository");
        let mut config = repo.config().expect("Failed to get config");
        config.set_str("user.name", "Test User").expect("Failed to set user.name");
        config.set_str("user.email", "test@example.com").expect("Failed to set user.email");
        
        // Create initial commit directly
        let signature = git2::Signature::now("Test User", "test@example.com").expect("Failed to create signature");
        let tree_id = repo.index().unwrap().write_tree().expect("Failed to write tree");
        let tree = repo.find_tree(tree_id).expect("Failed to find tree");
        repo.commit(Some("HEAD"), &signature, &signature, "Initial commit", &tree, &[])
            .expect("Failed to create initial commit");
        
        // Now create GitOperations
        let git_ops = GitOperations::with_work_dir(repo_path.to_path_buf())
            .expect("Failed to create GitOperations");
            
        let branch_name = BranchName::new("test-branch").expect("Invalid branch name");

        // Test branch creation
        assert!(!git_ops.branch_exists(&branch_name).unwrap());
        git_ops.create_branch(&branch_name).unwrap();
        assert!(git_ops.branch_exists(&branch_name).unwrap());

        // Test branch checkout
        git_ops.checkout_branch(&branch_name).unwrap();
        let current_branch = git_ops.get_current_branch().unwrap();
        assert_eq!(current_branch, Some(branch_name.clone()));
    }

    #[test]
    fn test_status_operations() {
        let (_temp_dir, git_ops) = setup_test_repo();
        
        // Initially should be clean
        let status = git_ops.get_status().unwrap();
        assert!(status.is_clean());
        assert!(git_ops.is_working_directory_clean().unwrap());
    }
}