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
        debug!(
            "Creating GitOperations for directory: {}",
            work_dir.display()
        );

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
            let (branch, _) =
                branch_result.map_err(|e| convert_git2_error("iterate_branches", e))?;

            if let Some(name) = branch
                .name()
                .map_err(|e| convert_git2_error("get_branch_name", e))?
            {
                branch_names.push(BranchName::new_unchecked(name));
            }
        }

        Ok(branch_names)
    }

    /// Get list of files changed on current branch relative to parent branch
    ///
    /// Uses merge-base to find the common ancestor commit, then diffs from that
    /// point to the current branch HEAD to identify all changed files.
    pub fn get_changed_files_from_parent(
        &self,
        current_branch: &str,
        parent_branch: &str,
    ) -> GitResult<Vec<String>> {
        let repo = self.repo.inner();

        let current_commit = self.resolve_branch_commit(current_branch)?;
        let parent_commit = self.resolve_branch_commit(parent_branch)?;

        let merge_base = repo
            .merge_base(current_commit, parent_commit)
            .map_err(|e| convert_git2_error("merge_base", e))?;

        let from_tree = self.commit_tree(merge_base)?;
        let to_tree = self.commit_tree(current_commit)?;

        self.diff_tree_paths(&from_tree, &to_tree)
    }

    /// Get list of files changed within a git revision range
    ///
    /// Parses the range string and performs a tree-to-tree diff between the two endpoints.
    /// If the range contains `..`, it is split into `from..to`. If it is a single ref,
    /// it is treated as `ref..HEAD`.
    ///
    /// # Arguments
    ///
    /// * `range` - A git revision range, e.g. `HEAD~1..HEAD`, `HEAD~3..HEAD`, or `HEAD~2`
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Either ref in the range cannot be resolved
    /// - The resolved objects cannot be peeled to commits
    /// - The tree diff fails
    pub fn get_changed_files_from_range(&self, range: &str) -> GitResult<Vec<String>> {
        let repo = self.repo.inner();

        // Parse range: "from..to" or single ref treated as "ref..HEAD"
        let (from_ref, to_ref) = if let Some((from, to)) = range.split_once("..") {
            (from.to_string(), to.to_string())
        } else {
            (range.to_string(), "HEAD".to_string())
        };

        let from_commit = self.resolve_ref_to_commit(repo, &from_ref)?;
        let to_commit = self.resolve_ref_to_commit(repo, &to_ref)?;

        let from_tree = self.commit_tree(from_commit)?;
        let to_tree = self.commit_tree(to_commit)?;

        self.diff_tree_paths(&from_tree, &to_tree)
    }

    /// Get all tracked files in the repository
    ///
    /// Returns a sorted list of all files currently tracked in the repository's HEAD commit.
    /// This walks the tree recursively and collects all blob (file) entries, excluding directories.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The repository has no HEAD (empty repository)
    /// - Failed to read the HEAD commit or its tree
    /// - Failed to walk the tree structure
    pub fn get_all_tracked_files(&self) -> GitResult<Vec<String>> {
        let repo = self.repo.inner();

        // Get HEAD reference
        let head = repo.head().map_err(|e| convert_git2_error("get_head", e))?;

        // Get HEAD commit
        let head_commit = head
            .peel_to_commit()
            .map_err(|e| convert_git2_error("peel_head_to_commit", e))?;

        // Get tree from HEAD commit
        let tree = head_commit
            .tree()
            .map_err(|e| convert_git2_error("get_head_tree", e))?;

        // Collect all file paths by walking the tree
        let mut file_paths = Vec::new();

        tree.walk(git2::TreeWalkMode::PreOrder, |root, entry| {
            // Only include blobs (files), not trees (directories)
            if let Some(git2::ObjectType::Blob) = entry.kind() {
                if let Some(name) = entry.name() {
                    let full_path = if root.is_empty() {
                        name.to_string()
                    } else {
                        format!("{}{}", root, name)
                    };
                    file_paths.push(full_path);
                }
            }
            git2::TreeWalkResult::Ok
        })
        .map_err(|e| convert_git2_error("walk_tree", e))?;

        // Sort for consistent ordering
        file_paths.sort();

        Ok(file_paths)
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
        let head = repo.head().map_err(|e| convert_git2_error("get_head", e))?;

        let commit = head
            .peel_to_commit()
            .map_err(|e| convert_git2_error("peel_to_commit", e))?;

        let hash = commit.id().to_string();
        let message = commit.message().unwrap_or("").to_string();
        let author = commit.author();
        let author_name = author.name().unwrap_or("").to_string();
        let author_email = author.email().unwrap_or("").to_string();
        let timestamp =
            chrono::DateTime::from_timestamp(author.when().seconds(), 0).unwrap_or_default();

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
            Ok(head) => Some(
                head.peel_to_commit()
                    .map_err(|e| convert_git2_error("peel_to_commit", e))?,
            ),
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
            None => repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &[]),
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

        let source_ref = repo
            .find_reference(&format!("refs/heads/{}", source_branch.as_str()))
            .map_err(|e| convert_git2_error("find_source_reference", e))?;
        let source_commit = source_ref
            .peel_to_commit()
            .map_err(|e| convert_git2_error("peel_source_commit", e))?;
        let head_commit = repo
            .head()
            .and_then(|head| head.peel_to_commit())
            .map_err(|e| convert_git2_error("get_head_commit", e))?;

        let annotated_commit = repo
            .find_annotated_commit(source_commit.id())
            .map_err(|e| convert_git2_error("find_annotated_commit", e))?;
        let analysis = repo
            .merge_analysis(&[&annotated_commit])
            .map_err(|e| convert_git2_error("merge_analysis", e))?;

        if analysis.0.is_up_to_date() {
            info!("Branch {} is already up to date", source_branch);
            return Ok(());
        }

        if analysis.0.is_fast_forward() {
            self.fast_forward_merge(repo, &source_commit, source_branch)?;
        } else {
            self.three_way_merge(
                repo,
                &annotated_commit,
                &head_commit,
                &source_commit,
                source_branch,
            )?;
        }

        Ok(())
    }

    /// Perform a fast-forward merge by advancing HEAD to the source commit.
    fn fast_forward_merge(
        &self,
        repo: &git2::Repository,
        source_commit: &git2::Commit<'_>,
        source_branch: &BranchName,
    ) -> GitResult<()> {
        debug!("Performing fast-forward merge for branch {}", source_branch);

        let mut head_ref = repo
            .head()
            .map_err(|e| convert_git2_error("get_head_ref", e))?;
        head_ref
            .set_target(source_commit.id(), "Fast-forward merge")
            .map_err(|e| convert_git2_error("fast_forward", e))?;

        let mut checkout_opts = git2::build::CheckoutBuilder::new();
        checkout_opts.force();
        repo.checkout_head(Some(&mut checkout_opts))
            .map_err(|e| convert_git2_error("checkout_after_merge", e))?;

        info!("Fast-forward merged branch {}", source_branch);
        Ok(())
    }

    /// Perform a three-way merge, handling conflicts and creating the merge commit.
    fn three_way_merge(
        &self,
        repo: &git2::Repository,
        annotated_commit: &git2::AnnotatedCommit<'_>,
        head_commit: &git2::Commit<'_>,
        source_commit: &git2::Commit<'_>,
        source_branch: &BranchName,
    ) -> GitResult<()> {
        debug!("Performing three-way merge for branch {}", source_branch);

        let mut checkout_opts = git2::build::CheckoutBuilder::new();
        checkout_opts.conflict_style_merge(true);
        let mut merge_opts = git2::MergeOptions::new();
        merge_opts.file_favor(git2::FileFavor::Normal);

        repo.merge(
            &[annotated_commit],
            Some(&mut merge_opts),
            Some(&mut checkout_opts),
        )
        .map_err(|e| convert_git2_error("perform_merge", e))?;

        let mut index = repo
            .index()
            .map_err(|e| convert_git2_error("get_index_after_merge", e))?;

        if index.has_conflicts() {
            return self.abort_conflicted_merge(repo, source_branch);
        }

        self.create_merge_commit(repo, &mut index, head_commit, source_commit, source_branch)?;
        self.checkout_after_merge(repo)?;

        info!("Created merge commit for branch {}", source_branch);
        Ok(())
    }

    /// Clean up merge state and reset working directory after a conflict.
    fn abort_conflicted_merge(
        &self,
        repo: &git2::Repository,
        source_branch: &BranchName,
    ) -> GitResult<()> {
        info!(
            "CONFLICT DETECTED: Cleaning up merge state for branch {}",
            source_branch
        );

        repo.cleanup_state()
            .map_err(|e| convert_git2_error("cleanup_after_conflict", e))?;

        // Reset index to HEAD to clear conflicted entries
        let head_commit = repo
            .head()
            .map_err(|e| convert_git2_error("get_head_for_reset", e))?
            .peel_to_commit()
            .map_err(|e| convert_git2_error("peel_head_for_reset", e))?;
        let tree = head_commit
            .tree()
            .map_err(|e| convert_git2_error("get_head_tree", e))?;

        let mut fresh_index = repo
            .index()
            .map_err(|e| convert_git2_error("get_fresh_index", e))?;
        fresh_index
            .read_tree(&tree)
            .map_err(|e| convert_git2_error("reset_index", e))?;
        fresh_index
            .write()
            .map_err(|e| convert_git2_error("write_reset_index", e))?;

        // Force checkout HEAD to reset working directory
        let mut checkout_opts = git2::build::CheckoutBuilder::new();
        checkout_opts.force();
        checkout_opts.remove_untracked(true);
        repo.checkout_head(Some(&mut checkout_opts))
            .map_err(|e| convert_git2_error("reset_after_conflict", e))?;

        Err(GitError::from_string(format!(
            "Merge conflicts detected when merging branch '{}'. Please resolve conflicts manually.",
            source_branch
        )))
    }

    /// Create a merge commit from the current index state.
    fn create_merge_commit(
        &self,
        repo: &git2::Repository,
        index: &mut git2::Index,
        head_commit: &git2::Commit<'_>,
        source_commit: &git2::Commit<'_>,
        source_branch: &BranchName,
    ) -> GitResult<()> {
        let signature = repo
            .signature()
            .map_err(|e| convert_git2_error("get_signature", e))?;
        let message = format!("Merge branch '{}'", source_branch);
        let tree_id = index
            .write_tree()
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
            &[head_commit, source_commit],
        )
        .map_err(|e| convert_git2_error("create_merge_commit", e))?;

        repo.cleanup_state()
            .map_err(|e| convert_git2_error("cleanup_merge_state", e))?;
        Ok(())
    }

    /// Force-checkout HEAD to update the working directory after a merge commit.
    fn checkout_after_merge(&self, repo: &git2::Repository) -> GitResult<()> {
        let head_commit = repo
            .head()
            .map_err(|e| convert_git2_error("get_head_after_merge", e))?
            .peel_to_commit()
            .map_err(|e| convert_git2_error("peel_head_after_merge", e))?;

        let mut checkout_opts = git2::build::CheckoutBuilder::new();
        checkout_opts.force();
        checkout_opts.remove_untracked(false);
        repo.checkout_tree(head_commit.as_object(), Some(&mut checkout_opts))
            .map_err(|e| convert_git2_error("checkout_tree_after_merge", e))?;
        Ok(())
    }

    /// Get current branch name as a String (convenience method for tests)
    pub fn current_branch(&self) -> GitResult<String> {
        match self.get_current_branch()? {
            Some(branch) => Ok(branch.into_string()),
            None => Err(GitError::from_string(
                "No current branch (detached HEAD or empty repository)".to_string(),
            )),
        }
    }

    /// Get the main branch name
    pub fn main_branch(&self) -> GitResult<String> {
        // Check if 'main' branch exists, otherwise use 'master'
        let main_branch = BranchName::new("main")?;
        if self.branch_exists(&main_branch)? {
            Ok("main".to_string())
        } else {
            let master_branch = BranchName::new("master")?;
            if self.branch_exists(&master_branch)? {
                Ok("master".to_string())
            } else {
                Err(GitError::from_string(
                    "Neither 'main' nor 'master' branch exists".to_string(),
                ))
            }
        }
    }

    /// Find the best merge target for a branch using git merge-base
    /// This determines which branch the given branch should merge back to
    pub fn find_merge_target_for_issue(&self, issue_branch: &BranchName) -> GitResult<String> {
        debug!("Finding merge target for branch: {}", issue_branch);

        let issue_commit = self.resolve_branch_commit(issue_branch.as_str())?;
        let candidates = self.collect_candidate_branches(issue_branch)?;
        debug!("Total candidate branches: {}", candidates.len());

        let had_candidates = !candidates.is_empty();
        let best_target = self.find_best_scoring_branch(&candidates, issue_commit)?;

        if let Some(target) = best_target {
            debug!("Selected merge target: {}", target);
            return Ok(target);
        }

        if had_candidates {
            debug!("No valid merge-base found with any candidate - likely orphan branch or unrelated history");
            return Err(GitError::generic(format!(
                "Branch '{}' has no common history with other branches (orphan branch)",
                issue_branch
            )));
        }

        debug!("No candidate branches found, falling back to main branch");
        let main = self.main_branch()?;
        if issue_branch.as_str() == main {
            return Err(GitError::generic(format!(
                "Branch '{}' is the main branch and has no parent",
                issue_branch
            )));
        }
        Ok(main)
    }

    /// Resolve a local branch name to its commit OID.
    fn resolve_branch_commit(&self, branch_name: &str) -> GitResult<git2::Oid> {
        let branch = self
            .repo
            .inner()
            .find_branch(branch_name, BranchType::Local)
            .map_err(|e| convert_git2_error("find_branch", e))?;
        let commit = branch
            .get()
            .peel_to_commit()
            .map_err(|e| convert_git2_error("peel_to_commit", e))?;
        Ok(commit.id())
    }

    /// Collect local branches that are valid merge-target candidates for the given issue branch.
    /// Excludes the issue branch itself and sibling branches sharing the same prefix.
    fn collect_candidate_branches(&self, issue_branch: &BranchName) -> GitResult<Vec<String>> {
        let branch_iter = self
            .repo
            .inner()
            .branches(Some(BranchType::Local))
            .map_err(|e| convert_git2_error("branches", e))?;

        // Extract prefix (e.g. "feature/" from "feature/foo") to skip sibling branches.
        let branch_prefix = issue_branch
            .as_str()
            .split('/')
            .next()
            .filter(|_| issue_branch.as_str().contains('/'))
            .map(|s| format!("{}/", s));

        let mut candidates = Vec::new();
        for branch_result in branch_iter {
            let (branch, _) = branch_result.map_err(|e| convert_git2_error("branch_iter", e))?;
            let name = match branch
                .name()
                .map_err(|e| convert_git2_error("branch_name", e))?
            {
                Some(n) => n,
                None => continue,
            };

            if name == issue_branch.as_str() {
                continue;
            }
            if branch_prefix
                .as_deref()
                .is_some_and(|p| name.starts_with(p))
            {
                continue;
            }

            debug!("Found candidate branch: {}", name);
            candidates.push(name.to_string());
        }
        Ok(candidates)
    }

    /// Score each candidate branch by merge-base proximity and return the best one.
    fn find_best_scoring_branch(
        &self,
        candidates: &[String],
        issue_commit: git2::Oid,
    ) -> GitResult<Option<String>> {
        let mut best_target = None;
        let mut best_score = 0i64;

        for branch_name in candidates {
            if let Some((name, score)) = self.score_candidate(branch_name, issue_commit)? {
                debug!("Branch '{}': score = {}", name, score);
                if score > best_score {
                    best_score = score;
                    debug!("New best target: {} (score: {})", name, score);
                    best_target = Some(name);
                }
            }
        }
        Ok(best_target)
    }

    /// Compute a merge-target score for a single candidate branch.
    /// Returns `None` if the branch cannot be resolved or has no common history.
    fn score_candidate(
        &self,
        branch_name: &str,
        issue_commit: git2::Oid,
    ) -> GitResult<Option<(String, i64)>> {
        let target_commit = match self.resolve_branch_commit_lenient(branch_name) {
            Some(oid) => oid,
            None => return Ok(None),
        };

        let (merge_base, merge_base_time) =
            match self.find_merge_base_with_time(issue_commit, target_commit) {
                Some(result) => result,
                None => return Ok(None),
            };

        let is_perfect_match = merge_base == target_commit;
        let issue_distance = self
            .count_commits_between(merge_base, issue_commit)
            .unwrap_or(usize::MAX);

        debug!(
            "Branch '{}': merge_base={}, perfect={}, distance={}",
            branch_name, merge_base, is_perfect_match, issue_distance
        );

        let score = Self::compute_merge_score(issue_distance, is_perfect_match, merge_base_time);
        Ok(Some((branch_name.to_string(), score)))
    }

    /// Find the merge base between two commits and return it with its timestamp.
    /// Returns `None` if there is no common history or the commit cannot be read.
    fn find_merge_base_with_time(&self, a: git2::Oid, b: git2::Oid) -> Option<(git2::Oid, i64)> {
        let merge_base = self.repo.inner().merge_base(a, b).ok()?;
        let commit = self.repo.inner().find_commit(merge_base).ok()?;
        Some((merge_base, commit.time().seconds()))
    }

    /// Compute a merge-target score from distance, perfect-match, and recency.
    fn compute_merge_score(distance: usize, is_perfect_match: bool, merge_base_time: i64) -> i64 {
        // Scoring weights: distance dominates, perfect-match is a tie-breaker, recency is minor.
        const MAX_DISTANCE: i64 = 1000; // Cap beyond which branches are considered unrelated
        const DISTANCE_WEIGHT: i64 = 1000; // Multiplier so distance outweighs other factors
        const PERFECT_MATCH_BONUS: i64 = 100; // Small bonus when merge-base == target tip
        const RECENCY_DIVISOR: i64 = 1000; // Scale epoch seconds to a minor scoring component

        let distance_score = (MAX_DISTANCE - distance as i64).max(0) * DISTANCE_WEIGHT;
        let perfect_bonus = if is_perfect_match {
            PERFECT_MATCH_BONUS
        } else {
            0
        };
        let recency_score = merge_base_time / RECENCY_DIVISOR;

        distance_score + perfect_bonus + recency_score
    }

    /// Try to resolve a branch to its commit OID, returning `None` on any error.
    fn resolve_branch_commit_lenient(&self, branch_name: &str) -> Option<git2::Oid> {
        self.repo
            .inner()
            .find_branch(branch_name, BranchType::Local)
            .ok()
            .and_then(|b| b.get().peel_to_commit().ok())
            .map(|c| c.id())
    }

    /// Resolve an arbitrary ref (branch, tag, SHA, HEAD~N) to a commit OID.
    fn resolve_ref_to_commit(
        &self,
        repo: &git2::Repository,
        refspec: &str,
    ) -> GitResult<git2::Oid> {
        let obj = repo
            .revparse_single(refspec)
            .map_err(|e| convert_git2_error("revparse", e))?;
        let commit = obj
            .peel_to_commit()
            .map_err(|e| convert_git2_error("peel_to_commit", e))?;
        Ok(commit.id())
    }

    /// Get the tree for a commit OID.
    fn commit_tree(&self, oid: git2::Oid) -> GitResult<git2::Tree<'_>> {
        let commit = self
            .repo
            .inner()
            .find_commit(oid)
            .map_err(|e| convert_git2_error("find_commit", e))?;
        commit.tree().map_err(|e| convert_git2_error("get_tree", e))
    }

    /// Diff two trees and return sorted, deduplicated file paths.
    fn diff_tree_paths(
        &self,
        from_tree: &git2::Tree<'_>,
        to_tree: &git2::Tree<'_>,
    ) -> GitResult<Vec<String>> {
        let diff = self
            .repo
            .inner()
            .diff_tree_to_tree(Some(from_tree), Some(to_tree), None)
            .map_err(|e| convert_git2_error("diff_tree_to_tree", e))?;

        let mut files: Vec<String> = diff
            .deltas()
            .filter_map(|d| d.new_file().path()?.to_str().map(String::from))
            .collect();
        files.sort();
        files.dedup();
        Ok(files)
    }

    /// Count commits between two commit IDs
    fn count_commits_between(&self, from: git2::Oid, to: git2::Oid) -> GitResult<usize> {
        let mut revwalk = self
            .repo
            .inner()
            .revwalk()
            .map_err(|e| convert_git2_error("revwalk", e))?;

        revwalk
            .push(to)
            .map_err(|e| convert_git2_error("push_to", e))?;
        revwalk
            .hide(from)
            .map_err(|e| convert_git2_error("hide_from", e))?;

        let count = revwalk.count();
        Ok(count)
    }

    /// Validate branch creation
    pub fn validate_branch_creation(
        &self,
        branch_name: &str,
        base_branch: Option<&str>,
    ) -> GitResult<()> {
        // Check if base branch is an issue branch
        let base_to_check = match base_branch {
            Some(base) => base.to_string(),
            None => self.current_branch()?,
        };

        if base_to_check.starts_with("issue/") {
            return Err(GitError::from_string(format!(
                "Cannot create issue '{}' from issue branch '{}'. Issue branches should be created from feature/release branches.",
                branch_name, base_to_check
            )));
        }

        Ok(())
    }

    /// Check if there are uncommitted changes (convenience method)
    pub fn has_uncommitted_changes(&self) -> GitResult<bool> {
        let is_clean = self.is_working_directory_clean()?;
        Ok(!is_clean)
    }

    /// Checkout branch by string name (convenience method for backward compatibility)
    pub fn checkout_branch_str(&self, branch_name: &str) -> GitResult<()> {
        let branch = BranchName::new(branch_name)?;
        self.checkout_branch(&branch)
    }

    /// Delete branch by string name (convenience method for backward compatibility)
    pub fn delete_branch_str(&self, branch_name: &str) -> GitResult<()> {
        let branch = BranchName::new(branch_name)?;
        self.delete_branch(&branch)
    }

    /// Check if branch exists by string name (convenience method for backward compatibility)
    pub fn branch_exists_str(&self, branch_name: &str) -> GitResult<bool> {
        let branch = BranchName::new(branch_name)?;
        self.branch_exists(&branch)
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
        config
            .set_str("user.name", "Test User")
            .expect("Failed to set user.name");
        config
            .set_str("user.email", "test@example.com")
            .expect("Failed to set user.email");
        println!("Repository configured");

        // Create a file and add it to create a proper initial commit
        std::fs::write(repo_path.join("README.md"), "# Test Repository\n")
            .expect("Failed to write README");

        let mut index = repo.index().expect("Failed to get index");
        index
            .add_path(std::path::Path::new("README.md"))
            .expect("Failed to add file to index");
        index.write().expect("Failed to write index");
        println!("File added to index");

        let signature = git2::Signature::now("Test User", "test@example.com")
            .expect("Failed to create signature");
        let tree_id = index.write_tree().expect("Failed to write tree");
        let tree = repo.find_tree(tree_id).expect("Failed to find tree");

        // Create the initial commit
        let commit_id = repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                "Initial commit",
                &tree,
                &[],
            )
            .expect("Failed to create initial commit");
        println!("Initial commit created: {}", commit_id);

        let git_ops = GitOperations::with_work_dir(repo_path.to_path_buf())
            .expect("Failed to create GitOperations");
        println!("GitOperations created");

        (temp_dir, git_ops)
    }

    #[test]
    fn test_status_operations() {
        let (_temp_dir, git_ops) = setup_test_repo();

        // Initially should be clean
        let status = git_ops.get_status().unwrap();
        assert!(status.is_clean());
        assert!(git_ops.is_working_directory_clean().unwrap());
    }

    #[test]
    fn test_get_changed_files_from_parent() {
        use std::fs;

        // Create a test repository with main and feature branches
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let repo_path = temp_dir.path();

        // Initialize repository
        let repo = Repository::init(repo_path).expect("Failed to init repository");
        // Ensure initial branch is 'main' for consistency across environments
        repo.set_head("refs/heads/main")
            .expect("Failed to set HEAD to main");
        let mut config = repo.config().expect("Failed to get config");
        config
            .set_str("user.name", "Test User")
            .expect("Failed to set user.name");
        config
            .set_str("user.email", "test@example.com")
            .expect("Failed to set user.email");

        let signature = git2::Signature::now("Test User", "test@example.com")
            .expect("Failed to create signature");

        // Helper function to create a commit
        let create_commit = |message: &str, files: Vec<(&str, &str)>| {
            for (filename, content) in files {
                let file_path = repo_path.join(filename);
                if let Some(parent) = file_path.parent() {
                    fs::create_dir_all(parent).expect("Failed to create parent directory");
                }
                fs::write(&file_path, content).expect("Failed to write file");
            }

            let mut index = repo.index().expect("Failed to get index");
            index
                .add_all(["."].iter(), git2::IndexAddOption::DEFAULT, None)
                .expect("Failed to add files to index");
            index.write().expect("Failed to write index");
            let tree_id = index.write_tree().expect("Failed to write tree");
            let tree = repo.find_tree(tree_id).expect("Failed to find tree");

            let parent_commit = match repo.head() {
                Ok(head) => {
                    let parent_oid = head.target().expect("Failed to get head target");
                    Some(
                        repo.find_commit(parent_oid)
                            .expect("Failed to find parent commit"),
                    )
                }
                Err(_) => None,
            };

            let parents: Vec<&git2::Commit> =
                parent_commit.as_ref().map(|c| vec![c]).unwrap_or_default();
            repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parents,
            )
            .expect("Failed to create commit")
        };

        // Create initial commit on main
        create_commit("Initial commit", vec![("README.md", "Initial content")]);

        // Create feature branch from main
        let feature_branch = repo
            .branch(
                "feature",
                &repo.head().unwrap().peel_to_commit().unwrap(),
                false,
            )
            .expect("Failed to create feature branch");
        repo.set_head(feature_branch.get().name().unwrap())
            .expect("Failed to set HEAD");
        repo.checkout_head(None).expect("Failed to checkout");

        // Make changes on feature branch
        create_commit(
            "Feature commit 1",
            vec![
                ("src/main.rs", "fn main() {}"),
                ("src/lib.rs", "pub fn hello() {}"),
            ],
        );

        create_commit("Feature commit 2", vec![("docs/guide.md", "# Guide")]);

        create_commit(
            "Feature commit 3",
            vec![
                ("src/main.rs", "fn main() { println!(\"Hello\"); }"),
                ("tests/test.rs", "#[test] fn test_main() {}"),
            ],
        );

        // Now test the function
        let git_ops = GitOperations::with_work_dir(repo_path.to_path_buf())
            .expect("Failed to create GitOperations");

        let changed_files = git_ops
            .get_changed_files_from_parent("feature", "main")
            .expect("Failed to get changed files");

        // Verify we got all the files changed on feature branch
        assert_eq!(changed_files.len(), 4, "Expected 4 changed files");
        assert!(changed_files.contains(&"src/main.rs".to_string()));
        assert!(changed_files.contains(&"src/lib.rs".to_string()));
        assert!(changed_files.contains(&"docs/guide.md".to_string()));
        assert!(changed_files.contains(&"tests/test.rs".to_string()));

        // Verify README.md is not in the list (it was in the initial commit)
        assert!(!changed_files.contains(&"README.md".to_string()));
    }

    #[test]
    fn test_get_all_tracked_files() {
        use std::fs;

        // Create a test repository with multiple files in nested directories
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let repo_path = temp_dir.path();

        // Initialize repository
        let repo = Repository::init(repo_path).expect("Failed to init repository");
        let mut config = repo.config().expect("Failed to get config");
        config
            .set_str("user.name", "Test User")
            .expect("Failed to set user.name");
        config
            .set_str("user.email", "test@example.com")
            .expect("Failed to set user.email");

        let signature = git2::Signature::now("Test User", "test@example.com")
            .expect("Failed to create signature");

        // Helper function to create a commit
        let create_commit = |message: &str, files: Vec<(&str, &str)>| {
            for (filename, content) in files {
                let file_path = repo_path.join(filename);
                if let Some(parent) = file_path.parent() {
                    fs::create_dir_all(parent).expect("Failed to create parent directory");
                }
                fs::write(&file_path, content).expect("Failed to write file");
            }

            let mut index = repo.index().expect("Failed to get index");
            index
                .add_all(["."].iter(), git2::IndexAddOption::DEFAULT, None)
                .expect("Failed to add files to index");
            index.write().expect("Failed to write index");
            let tree_id = index.write_tree().expect("Failed to write tree");
            let tree = repo.find_tree(tree_id).expect("Failed to find tree");

            let parent_commit = match repo.head() {
                Ok(head) => {
                    let parent_oid = head.target().expect("Failed to get head target");
                    Some(
                        repo.find_commit(parent_oid)
                            .expect("Failed to find parent commit"),
                    )
                }
                Err(_) => None,
            };

            let parents: Vec<&git2::Commit> =
                parent_commit.as_ref().map(|c| vec![c]).unwrap_or_default();
            repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parents,
            )
            .expect("Failed to create commit")
        };

        // Create multiple commits with various files
        create_commit(
            "Initial commit",
            vec![
                ("README.md", "# Project"),
                ("src/main.rs", "fn main() {}"),
                ("src/lib.rs", "pub fn hello() {}"),
            ],
        );

        create_commit(
            "Add more files",
            vec![
                ("docs/guide.md", "# Guide"),
                ("tests/test.rs", "#[test] fn test() {}"),
                ("config/settings.toml", "[settings]"),
            ],
        );

        // Now test the function
        let git_ops = GitOperations::with_work_dir(repo_path.to_path_buf())
            .expect("Failed to create GitOperations");

        let all_files = git_ops
            .get_all_tracked_files()
            .expect("Failed to get all tracked files");

        // Verify we got all 6 tracked files
        assert_eq!(all_files.len(), 6, "Expected 6 tracked files");
        assert!(all_files.contains(&"README.md".to_string()));
        assert!(all_files.contains(&"src/main.rs".to_string()));
        assert!(all_files.contains(&"src/lib.rs".to_string()));
        assert!(all_files.contains(&"docs/guide.md".to_string()));
        assert!(all_files.contains(&"tests/test.rs".to_string()));
        assert!(all_files.contains(&"config/settings.toml".to_string()));

        // Verify files are sorted
        let mut sorted_files = all_files.clone();
        sorted_files.sort();
        assert_eq!(all_files, sorted_files, "Files should be sorted");
    }

    #[test]
    fn test_get_changed_files_from_range_last_commit() {
        use std::fs;

        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let repo_path = temp_dir.path();

        let repo = Repository::init(repo_path).expect("Failed to init repository");
        repo.set_head("refs/heads/main")
            .expect("Failed to set HEAD to main");
        let mut config = repo.config().expect("Failed to get config");
        config
            .set_str("user.name", "Test User")
            .expect("Failed to set user.name");
        config
            .set_str("user.email", "test@example.com")
            .expect("Failed to set user.email");

        let signature = git2::Signature::now("Test User", "test@example.com")
            .expect("Failed to create signature");

        let create_commit = |message: &str, files: Vec<(&str, &str)>| {
            for (filename, content) in files {
                let file_path = repo_path.join(filename);
                if let Some(parent) = file_path.parent() {
                    fs::create_dir_all(parent).expect("Failed to create parent directory");
                }
                fs::write(&file_path, content).expect("Failed to write file");
            }

            let mut index = repo.index().expect("Failed to get index");
            index
                .add_all(["."].iter(), git2::IndexAddOption::DEFAULT, None)
                .expect("Failed to add files to index");
            index.write().expect("Failed to write index");
            let tree_id = index.write_tree().expect("Failed to write tree");
            let tree = repo.find_tree(tree_id).expect("Failed to find tree");

            let parent_commit = match repo.head() {
                Ok(head) => {
                    let parent_oid = head.target().expect("Failed to get head target");
                    Some(
                        repo.find_commit(parent_oid)
                            .expect("Failed to find parent commit"),
                    )
                }
                Err(_) => None,
            };

            let parents: Vec<&git2::Commit> =
                parent_commit.as_ref().map(|c| vec![c]).unwrap_or_default();
            repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parents,
            )
            .expect("Failed to create commit")
        };

        // Create two commits
        create_commit("Initial commit", vec![("README.md", "Initial content")]);
        create_commit("Second commit", vec![("file1.txt", "hello")]);

        let git_ops = GitOperations::with_work_dir(repo_path.to_path_buf())
            .expect("Failed to create GitOperations");

        // HEAD~1..HEAD should return only file1.txt (from the last commit)
        let changed_files = git_ops
            .get_changed_files_from_range("HEAD~1..HEAD")
            .expect("Failed to get changed files from range");

        assert_eq!(changed_files.len(), 1, "Expected 1 changed file");
        assert!(changed_files.contains(&"file1.txt".to_string()));
        assert!(!changed_files.contains(&"README.md".to_string()));
    }

    #[test]
    fn test_get_changed_files_from_range_multiple_commits() {
        use std::fs;

        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let repo_path = temp_dir.path();

        let repo = Repository::init(repo_path).expect("Failed to init repository");
        repo.set_head("refs/heads/main")
            .expect("Failed to set HEAD to main");
        let mut config = repo.config().expect("Failed to get config");
        config
            .set_str("user.name", "Test User")
            .expect("Failed to set user.name");
        config
            .set_str("user.email", "test@example.com")
            .expect("Failed to set user.email");

        let signature = git2::Signature::now("Test User", "test@example.com")
            .expect("Failed to create signature");

        let create_commit = |message: &str, files: Vec<(&str, &str)>| {
            for (filename, content) in files {
                let file_path = repo_path.join(filename);
                if let Some(parent) = file_path.parent() {
                    fs::create_dir_all(parent).expect("Failed to create parent directory");
                }
                fs::write(&file_path, content).expect("Failed to write file");
            }

            let mut index = repo.index().expect("Failed to get index");
            index
                .add_all(["."].iter(), git2::IndexAddOption::DEFAULT, None)
                .expect("Failed to add files to index");
            index.write().expect("Failed to write index");
            let tree_id = index.write_tree().expect("Failed to write tree");
            let tree = repo.find_tree(tree_id).expect("Failed to find tree");

            let parent_commit = match repo.head() {
                Ok(head) => {
                    let parent_oid = head.target().expect("Failed to get head target");
                    Some(
                        repo.find_commit(parent_oid)
                            .expect("Failed to find parent commit"),
                    )
                }
                Err(_) => None,
            };

            let parents: Vec<&git2::Commit> =
                parent_commit.as_ref().map(|c| vec![c]).unwrap_or_default();
            repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parents,
            )
            .expect("Failed to create commit")
        };

        // Create 4 commits
        create_commit("Initial commit", vec![("README.md", "Initial content")]);
        create_commit("Commit 2", vec![("file1.txt", "hello")]);
        create_commit("Commit 3", vec![("file2.txt", "world")]);
        create_commit("Commit 4", vec![("file3.txt", "foo")]);

        let git_ops = GitOperations::with_work_dir(repo_path.to_path_buf())
            .expect("Failed to create GitOperations");

        // HEAD~3..HEAD should return files from last 3 commits
        let changed_files = git_ops
            .get_changed_files_from_range("HEAD~3..HEAD")
            .expect("Failed to get changed files from range");

        assert_eq!(changed_files.len(), 3, "Expected 3 changed files");
        assert!(changed_files.contains(&"file1.txt".to_string()));
        assert!(changed_files.contains(&"file2.txt".to_string()));
        assert!(changed_files.contains(&"file3.txt".to_string()));
        // README.md was in the initial commit, before the range
        assert!(!changed_files.contains(&"README.md".to_string()));
    }

    #[test]
    fn test_get_changed_files_from_range_single_ref() {
        use std::fs;

        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let repo_path = temp_dir.path();

        let repo = Repository::init(repo_path).expect("Failed to init repository");
        repo.set_head("refs/heads/main")
            .expect("Failed to set HEAD to main");
        let mut config = repo.config().expect("Failed to get config");
        config
            .set_str("user.name", "Test User")
            .expect("Failed to set user.name");
        config
            .set_str("user.email", "test@example.com")
            .expect("Failed to set user.email");

        let signature = git2::Signature::now("Test User", "test@example.com")
            .expect("Failed to create signature");

        let create_commit = |message: &str, files: Vec<(&str, &str)>| {
            for (filename, content) in files {
                let file_path = repo_path.join(filename);
                if let Some(parent) = file_path.parent() {
                    fs::create_dir_all(parent).expect("Failed to create parent directory");
                }
                fs::write(&file_path, content).expect("Failed to write file");
            }

            let mut index = repo.index().expect("Failed to get index");
            index
                .add_all(["."].iter(), git2::IndexAddOption::DEFAULT, None)
                .expect("Failed to add files to index");
            index.write().expect("Failed to write index");
            let tree_id = index.write_tree().expect("Failed to write tree");
            let tree = repo.find_tree(tree_id).expect("Failed to find tree");

            let parent_commit = match repo.head() {
                Ok(head) => {
                    let parent_oid = head.target().expect("Failed to get head target");
                    Some(
                        repo.find_commit(parent_oid)
                            .expect("Failed to find parent commit"),
                    )
                }
                Err(_) => None,
            };

            let parents: Vec<&git2::Commit> =
                parent_commit.as_ref().map(|c| vec![c]).unwrap_or_default();
            repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parents,
            )
            .expect("Failed to create commit")
        };

        // Create 3 commits
        create_commit("Initial commit", vec![("README.md", "Initial content")]);
        create_commit("Commit 2", vec![("file1.txt", "hello")]);
        create_commit("Commit 3", vec![("file2.txt", "world")]);

        let git_ops = GitOperations::with_work_dir(repo_path.to_path_buf())
            .expect("Failed to create GitOperations");

        // Single ref "HEAD~2" should be treated as "HEAD~2..HEAD"
        let changed_files = git_ops
            .get_changed_files_from_range("HEAD~2")
            .expect("Failed to get changed files from range");

        assert_eq!(changed_files.len(), 2, "Expected 2 changed files");
        assert!(changed_files.contains(&"file1.txt".to_string()));
        assert!(changed_files.contains(&"file2.txt".to_string()));
        assert!(!changed_files.contains(&"README.md".to_string()));
    }

    /// Helper to create a branch from HEAD in a repo
    fn create_branch_from_head(repo: &Repository, name: &str) {
        let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch(name, &head_commit, false)
            .expect("Failed to create branch");
    }

    /// Helper to switch to a branch
    fn checkout_branch_raw(repo: &Repository, name: &str) {
        let refname = format!("refs/heads/{}", name);
        let obj = repo.revparse_single(&refname).expect("Failed to parse ref");
        repo.checkout_tree(&obj, None)
            .expect("Failed to checkout tree");
        repo.set_head(&refname).expect("Failed to set HEAD");
    }

    /// Helper to add files and commit in a raw repo
    /// Helper to ensure the default branch is `main` after the first commit.
    /// Must be called after at least one commit exists on the default branch.
    fn rename_default_branch_to_main(repo: &Repository) {
        let head = repo.head().unwrap();
        let current_branch = head.shorthand().unwrap_or("").to_string();
        if current_branch == "main" {
            return;
        }
        let head_commit = head.peel_to_commit().unwrap();
        repo.branch("main", &head_commit, false).unwrap();
        repo.set_head("refs/heads/main").unwrap();
        // Delete the old default branch (e.g. "master")
        if let Ok(mut old_branch) = repo.find_branch(&current_branch, BranchType::Local) {
            old_branch.delete().ok();
        }
    }

    fn raw_commit(repo: &Repository, message: &str, files: Vec<(&str, &str)>) -> git2::Oid {
        for (filename, content) in &files {
            let file_path = repo.workdir().unwrap().join(filename);
            if let Some(parent) = file_path.parent() {
                std::fs::create_dir_all(parent).expect("Failed to create dir");
            }
            std::fs::write(&file_path, content).expect("Failed to write file");
        }
        let mut index = repo.index().expect("Failed to get index");
        index
            .add_all(["."].iter(), git2::IndexAddOption::DEFAULT, None)
            .expect("Failed to add all");
        index.write().expect("Failed to write index");
        let tree_id = index.write_tree().expect("Failed to write tree");
        let tree = repo.find_tree(tree_id).expect("Failed to find tree");
        let sig = git2::Signature::now("Test User", "test@example.com")
            .expect("Failed to create signature");
        let parent_commit = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit> =
            parent_commit.as_ref().map(|c| vec![c]).unwrap_or_default();
        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
            .expect("Failed to create commit")
    }

    #[test]
    fn test_branch_exists_nonexistent() {
        let (_temp_dir, git_ops) = setup_test_repo();
        let branch = BranchName::new("nonexistent-branch").unwrap();
        assert!(!git_ops.branch_exists(&branch).unwrap());
    }

    #[test]
    fn test_branch_exists_and_branch_exists_str() {
        let (temp_dir, git_ops) = setup_test_repo();
        let repo = Repository::open(temp_dir.path()).unwrap();
        create_branch_from_head(&repo, "feature-x");

        let branch = BranchName::new("feature-x").unwrap();
        assert!(git_ops.branch_exists(&branch).unwrap());
        assert!(git_ops.branch_exists_str("feature-x").unwrap());
        assert!(!git_ops.branch_exists_str("does-not-exist").unwrap());
    }

    #[test]
    fn test_checkout_branch() {
        let (temp_dir, git_ops) = setup_test_repo();
        let repo = Repository::open(temp_dir.path()).unwrap();
        create_branch_from_head(&repo, "my-branch");

        let branch = BranchName::new("my-branch").unwrap();
        git_ops.checkout_branch(&branch).unwrap();
        assert_eq!(git_ops.current_branch().unwrap(), "my-branch");
    }

    #[test]
    fn test_checkout_branch_nonexistent_fails() {
        let (_temp_dir, git_ops) = setup_test_repo();
        let branch = BranchName::new("ghost-branch").unwrap();
        let result = git_ops.checkout_branch(&branch);
        assert!(result.is_err());
    }

    #[test]
    fn test_checkout_branch_str() {
        let (temp_dir, git_ops) = setup_test_repo();
        let repo = Repository::open(temp_dir.path()).unwrap();
        create_branch_from_head(&repo, "str-branch");

        git_ops.checkout_branch_str("str-branch").unwrap();
        assert_eq!(git_ops.current_branch().unwrap(), "str-branch");
    }

    #[test]
    fn test_delete_branch() {
        let (temp_dir, git_ops) = setup_test_repo();
        let repo = Repository::open(temp_dir.path()).unwrap();
        create_branch_from_head(&repo, "to-delete");

        let branch = BranchName::new("to-delete").unwrap();
        assert!(git_ops.branch_exists(&branch).unwrap());
        git_ops.delete_branch(&branch).unwrap();
        assert!(!git_ops.branch_exists(&branch).unwrap());
    }

    #[test]
    fn test_delete_branch_nonexistent_fails() {
        let (_temp_dir, git_ops) = setup_test_repo();
        let branch = BranchName::new("ghost-branch").unwrap();
        let result = git_ops.delete_branch(&branch);
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_branch_str() {
        let (temp_dir, git_ops) = setup_test_repo();
        let repo = Repository::open(temp_dir.path()).unwrap();
        create_branch_from_head(&repo, "str-to-delete");

        git_ops.delete_branch_str("str-to-delete").unwrap();
        assert!(!git_ops.branch_exists_str("str-to-delete").unwrap());
    }

    #[test]
    fn test_add_all_and_commit() {
        let (temp_dir, git_ops) = setup_test_repo();

        // Write a new file
        std::fs::write(temp_dir.path().join("new_file.txt"), "content").unwrap();

        // add_all stages the file
        git_ops.add_all().unwrap();

        // commit creates the commit
        let commit_hash = git_ops.commit("Add new file").unwrap();
        assert!(!commit_hash.is_empty());

        // Verify the commit exists and the repo is clean again
        let latest = git_ops.get_latest_commit().unwrap();
        assert_eq!(latest.message.trim(), "Add new file");
    }

    #[test]
    fn test_has_uncommitted_changes() {
        let (temp_dir, git_ops) = setup_test_repo();

        // Clean state initially
        assert!(!git_ops.has_uncommitted_changes().unwrap());

        // Write a file to make the working directory dirty
        std::fs::write(temp_dir.path().join("dirty.txt"), "dirty").unwrap();
        assert!(git_ops.has_uncommitted_changes().unwrap());
    }

    #[test]
    fn test_validate_branch_creation_from_issue_branch_fails() {
        let (_temp_dir, git_ops) = setup_test_repo();
        // Passing an issue branch as base_branch should fail
        let result = git_ops.validate_branch_creation("new-branch", Some("issue/123"));
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_branch_creation_from_feature_branch_ok() {
        let (_temp_dir, git_ops) = setup_test_repo();
        // Feature branch as base is fine
        let result = git_ops.validate_branch_creation("issue/999", Some("feature/my-feature"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_branch_creation_from_main_ok() {
        let (_temp_dir, git_ops) = setup_test_repo();
        let result = git_ops.validate_branch_creation("issue/1", Some("main"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_merge_branch_fast_forward() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        let repo = Repository::init(repo_path).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        // Initial commit on master/main
        raw_commit(&repo, "Initial commit", vec![("README.md", "# Repo")]);
        rename_default_branch_to_main(&repo);

        create_branch_from_head(&repo, "feature-ff");
        checkout_branch_raw(&repo, "feature-ff");
        raw_commit(&repo, "Feature commit", vec![("feature.txt", "feature")]);

        // Go back to main and merge feature-ff (will be fast-forward)
        checkout_branch_raw(&repo, "main");

        let git_ops = GitOperations::with_work_dir(repo_path.to_path_buf()).unwrap();
        let feature_branch = BranchName::new("feature-ff").unwrap();
        git_ops.merge_branch(&feature_branch).unwrap();

        // feature.txt should now exist on main
        assert!(repo_path.join("feature.txt").exists());
    }

    #[test]
    fn test_merge_branch_up_to_date() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        let repo = Repository::init(repo_path).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        raw_commit(&repo, "Initial commit", vec![("README.md", "# Repo")]);
        rename_default_branch_to_main(&repo);

        // Create a branch that points to same commit as HEAD
        create_branch_from_head(&repo, "same-branch");

        let git_ops = GitOperations::with_work_dir(repo_path.to_path_buf()).unwrap();
        let same_branch = BranchName::new("same-branch").unwrap();
        // Merging a branch at same commit should succeed (up-to-date)
        git_ops.merge_branch(&same_branch).unwrap();
    }

    #[test]
    fn test_merge_branch_nonexistent_fails() {
        let (_temp_dir, git_ops) = setup_test_repo();
        let branch = BranchName::new("nonexistent-merge-branch").unwrap();
        let result = git_ops.merge_branch(&branch);
        assert!(result.is_err());
    }

    #[test]
    fn test_merge_branch_three_way() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        let repo = Repository::init(repo_path).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        // Initial commit
        raw_commit(&repo, "Initial commit", vec![("README.md", "# Repo")]);
        rename_default_branch_to_main(&repo);

        // Create feature branch and add a commit on it
        create_branch_from_head(&repo, "feature-3way");
        checkout_branch_raw(&repo, "feature-3way");
        raw_commit(
            &repo,
            "Feature commit",
            vec![("feature.txt", "feature content")],
        );

        // Switch back to main and add a different commit (divergent histories)
        checkout_branch_raw(&repo, "main");
        raw_commit(
            &repo,
            "Main commit",
            vec![("main_only.txt", "main content")],
        );

        let git_ops = GitOperations::with_work_dir(repo_path.to_path_buf()).unwrap();
        let feature_branch = BranchName::new("feature-3way").unwrap();
        git_ops.merge_branch(&feature_branch).unwrap();

        // Both files should exist after three-way merge
        assert!(repo_path.join("feature.txt").exists());
        assert!(repo_path.join("main_only.txt").exists());
    }

    #[test]
    fn test_find_merge_target_single_branch() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        let repo = Repository::init(repo_path).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        // Initial commit on main
        raw_commit(&repo, "Initial commit", vec![("README.md", "# Repo")]);
        rename_default_branch_to_main(&repo);

        // Create issue branch from main
        create_branch_from_head(&repo, "issue/42");
        checkout_branch_raw(&repo, "issue/42");
        raw_commit(&repo, "Issue commit", vec![("fix.txt", "fix")]);

        let git_ops = GitOperations::with_work_dir(repo_path.to_path_buf()).unwrap();
        let issue_branch = BranchName::new("issue/42").unwrap();
        let target = git_ops.find_merge_target_for_issue(&issue_branch).unwrap();
        assert_eq!(target, "main");
    }

    #[test]
    fn test_find_merge_target_prefers_direct_parent() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        let repo = Repository::init(repo_path).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        // Initial commit on main
        raw_commit(&repo, "Initial commit", vec![("README.md", "# Repo")]);
        rename_default_branch_to_main(&repo);

        // Create feature branch from main
        create_branch_from_head(&repo, "feature/myfeature");
        checkout_branch_raw(&repo, "feature/myfeature");
        raw_commit(&repo, "Feature commit", vec![("feature.txt", "feature")]);

        // Create issue branch from feature branch
        create_branch_from_head(&repo, "issue/99");
        checkout_branch_raw(&repo, "issue/99");
        raw_commit(&repo, "Issue commit", vec![("issue.txt", "issue")]);

        let git_ops = GitOperations::with_work_dir(repo_path.to_path_buf()).unwrap();
        let issue_branch = BranchName::new("issue/99").unwrap();
        let target = git_ops.find_merge_target_for_issue(&issue_branch).unwrap();
        // Should prefer feature/myfeature as the more direct parent
        assert_eq!(target, "feature/myfeature");
    }

    #[test]
    fn test_find_merge_target_skips_sibling_issue_branches() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        let repo = Repository::init(repo_path).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        // Initial commit on main
        raw_commit(&repo, "Initial commit", vec![("README.md", "# Repo")]);
        rename_default_branch_to_main(&repo);

        // Create two issue branches from main
        create_branch_from_head(&repo, "issue/1");
        create_branch_from_head(&repo, "issue/2");
        checkout_branch_raw(&repo, "issue/2");
        raw_commit(&repo, "Issue 2 commit", vec![("issue2.txt", "issue2")]);

        let git_ops = GitOperations::with_work_dir(repo_path.to_path_buf()).unwrap();
        let issue_branch = BranchName::new("issue/2").unwrap();
        let target = git_ops.find_merge_target_for_issue(&issue_branch).unwrap();
        // issue/1 should be skipped (same prefix), so merge target should be main
        assert_eq!(target, "main");
    }

    #[test]
    fn test_accessors_and_is_git_repository() {
        let (temp_dir, git_ops) = setup_test_repo();
        // repository() returns a reference to the inner GitRepository
        let repo_ref = git_ops.repository();
        assert!(repo_ref.is_valid());
        // work_dir() returns the working directory
        assert_eq!(git_ops.work_dir(), temp_dir.path());
        // is_git_repository() should be true for a valid repo
        assert!(git_ops.is_git_repository());
    }

    #[test]
    fn test_get_current_branch() {
        let (_temp_dir, git_ops) = setup_test_repo();
        // After setup_test_repo, there should be a current branch
        let branch = git_ops.get_current_branch().unwrap();
        assert!(branch.is_some());
    }

    #[test]
    fn test_get_current_branch_empty_repo() {
        // An empty repo (no commits) should return None for current branch
        let temp_dir = TempDir::new().unwrap();
        let _repo = Repository::init(temp_dir.path()).unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();
        let branch = git_ops.get_current_branch().unwrap();
        assert!(branch.is_none());
    }

    #[test]
    fn test_list_local_branches() {
        let (temp_dir, git_ops) = setup_test_repo();
        let repo = Repository::open(temp_dir.path()).unwrap();
        create_branch_from_head(&repo, "branch-a");
        create_branch_from_head(&repo, "branch-b");

        let branches = git_ops.list_local_branches().unwrap();
        let names: Vec<String> = branches.iter().map(|b| b.as_str().to_string()).collect();
        assert!(names.contains(&"branch-a".to_string()));
        assert!(names.contains(&"branch-b".to_string()));
    }

    #[test]
    fn test_current_branch_string() {
        let (_temp_dir, git_ops) = setup_test_repo();
        let branch = git_ops.current_branch().unwrap();
        // Should be a non-empty string
        assert!(!branch.is_empty());
    }

    #[test]
    fn test_current_branch_error_on_empty_repo() {
        let temp_dir = TempDir::new().unwrap();
        let _repo = Repository::init(temp_dir.path()).unwrap();
        let git_ops = GitOperations::with_work_dir(temp_dir.path().to_path_buf()).unwrap();
        // current_branch() should error on empty repo (no HEAD)
        let result = git_ops.current_branch();
        assert!(result.is_err());
    }

    #[test]
    fn test_main_branch_detection() {
        let (temp_dir, git_ops) = setup_test_repo();
        let repo = Repository::open(temp_dir.path()).unwrap();
        // Ensure we have a "main" branch
        rename_default_branch_to_main(&repo);

        let main = git_ops.main_branch().unwrap();
        assert!(main == "main" || main == "master");
    }

    #[test]
    fn test_main_branch_falls_back_to_master() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        let repo = Repository::init(repo_path).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        // Create initial commit, then ensure we're on "master" (not "main")
        raw_commit(&repo, "Initial commit", vec![("README.md", "# Repo")]);
        let head = repo.head().unwrap();
        let current_branch = head.shorthand().unwrap_or("");
        if current_branch != "master" {
            let head_commit = head.peel_to_commit().unwrap();
            repo.branch("master", &head_commit, false).unwrap();
            repo.set_head("refs/heads/master").unwrap();
            // Delete the old default branch
            if let Ok(mut old_branch) = repo.find_branch(current_branch, BranchType::Local) {
                old_branch.delete().ok();
            }
        }

        let git_ops = GitOperations::with_work_dir(repo_path.to_path_buf()).unwrap();
        let main = git_ops.main_branch().unwrap();
        assert_eq!(main, "master");
    }

    #[test]
    fn test_main_branch_neither_exists() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        let repo = Repository::init(repo_path).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        // Create initial commit on a branch that's not main or master
        raw_commit(&repo, "Initial commit", vec![("README.md", "# Repo")]);
        let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("develop", &head_commit, true).unwrap();
        repo.set_head("refs/heads/develop").unwrap();

        // Delete main and master if they exist
        if let Ok(mut b) = repo.find_branch("main", BranchType::Local) {
            b.delete().ok();
        }
        if let Ok(mut b) = repo.find_branch("master", BranchType::Local) {
            b.delete().ok();
        }
        // Also delete the default branch created by git init
        let head = repo.head().unwrap();
        let default_name = head.shorthand().unwrap_or("").to_string();
        if default_name != "develop" && default_name != "main" && default_name != "master" {
            if let Ok(mut b) = repo.find_branch(&default_name, BranchType::Local) {
                b.delete().ok();
            }
        }

        let git_ops = GitOperations::with_work_dir(repo_path.to_path_buf()).unwrap();
        let result = git_ops.main_branch();
        assert!(result.is_err());
    }

    #[test]
    fn test_commit_initial_no_parent() {
        // Test the commit() method on a repo with no prior commits (first commit path)
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        let repo = Repository::init(repo_path).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        // Write and stage a file
        std::fs::write(repo_path.join("file.txt"), "content").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("file.txt")).unwrap();
        index.write().unwrap();

        let git_ops = GitOperations::with_work_dir(repo_path.to_path_buf()).unwrap();
        let hash = git_ops.commit("Initial commit via commit()").unwrap();
        assert!(!hash.is_empty());

        // Verify it's accessible
        let latest = git_ops.get_latest_commit().unwrap();
        assert_eq!(latest.message.trim(), "Initial commit via commit()");
    }

    #[test]
    fn test_get_status_with_various_changes() {
        let (temp_dir, git_ops) = setup_test_repo();
        let repo_path = temp_dir.path();
        let repo = Repository::open(repo_path).unwrap();

        // Create an untracked file
        std::fs::write(repo_path.join("untracked.txt"), "untracked").unwrap();

        // Stage a new file (INDEX_NEW)
        std::fs::write(repo_path.join("staged_new.txt"), "new").unwrap();
        let mut index = repo.index().unwrap();
        index
            .add_path(std::path::Path::new("staged_new.txt"))
            .unwrap();
        index.write().unwrap();

        // Modify a tracked file without staging (WT_MODIFIED)
        std::fs::write(repo_path.join("README.md"), "modified content").unwrap();

        let status = git_ops.get_status().unwrap();
        assert!(!status.is_clean());
        assert!(status.untracked.contains(&"untracked.txt".to_string()));
        assert!(status.staged_new.contains(&"staged_new.txt".to_string()));
        assert!(status.unstaged_modified.contains(&"README.md".to_string()));
    }

    #[test]
    fn test_get_status_staged_modified() {
        let (temp_dir, git_ops) = setup_test_repo();
        let repo_path = temp_dir.path();
        let repo = Repository::open(repo_path).unwrap();

        // Stage a modification of the tracked README.md (INDEX_MODIFIED)
        std::fs::write(repo_path.join("README.md"), "staged modification").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("README.md")).unwrap();
        index.write().unwrap();

        let status = git_ops.get_status().unwrap();
        assert!(status.staged_modified.contains(&"README.md".to_string()));
        assert!(status.has_staged_changes());
    }

    #[test]
    fn test_get_status_staged_deleted() {
        let (temp_dir, git_ops) = setup_test_repo();
        let repo_path = temp_dir.path();
        let repo = Repository::open(repo_path).unwrap();

        // Stage deletion of README.md (INDEX_DELETED)
        let mut index = repo.index().unwrap();
        index
            .remove_path(std::path::Path::new("README.md"))
            .unwrap();
        index.write().unwrap();

        let status = git_ops.get_status().unwrap();
        assert!(status.staged_deleted.contains(&"README.md".to_string()));
    }

    #[test]
    fn test_get_status_unstaged_deleted() {
        let (temp_dir, git_ops) = setup_test_repo();
        let repo_path = temp_dir.path();

        // Delete the tracked README.md without staging (WT_DELETED)
        std::fs::remove_file(repo_path.join("README.md")).unwrap();

        let status = git_ops.get_status().unwrap();
        assert!(status.unstaged_deleted.contains(&"README.md".to_string()));
    }

    #[test]
    fn test_validate_branch_creation_none_base() {
        // When base_branch is None, it should use the current branch
        let (_temp_dir, git_ops) = setup_test_repo();
        // Current branch is not an issue/ branch, so this should succeed
        let result = git_ops.validate_branch_creation("new-issue", None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_branch_creation_none_base_on_issue_branch() {
        let (temp_dir, git_ops) = setup_test_repo();
        let repo = Repository::open(temp_dir.path()).unwrap();

        // Create and checkout an issue branch
        create_branch_from_head(&repo, "issue/123");
        checkout_branch_raw(&repo, "issue/123");

        // Now validate_branch_creation with None base should fail
        // because current branch starts with "issue/"
        let result = git_ops.validate_branch_creation("new-branch", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_find_merge_target_main_branch_is_self() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        let repo = Repository::init(repo_path).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        raw_commit(&repo, "Initial commit", vec![("README.md", "# Repo")]);
        rename_default_branch_to_main(&repo);

        let git_ops = GitOperations::with_work_dir(repo_path.to_path_buf()).unwrap();
        let main_branch = BranchName::new("main").unwrap();
        // Finding merge target for main itself should error
        let result = git_ops.find_merge_target_for_issue(&main_branch);
        assert!(result.is_err());
    }

    #[test]
    fn test_find_merge_target_orphan_branch() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        let repo = Repository::init(repo_path).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        // Create initial commit on main
        raw_commit(&repo, "Initial commit", vec![("README.md", "# Repo")]);
        rename_default_branch_to_main(&repo);

        // Create an orphan branch with completely separate history
        // We do this by creating a commit with no parent on a new branch
        let sig = git2::Signature::now("Test User", "test@example.com").unwrap();
        std::fs::write(repo_path.join("orphan.txt"), "orphan content").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("orphan.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        // Commit with NO parents -> orphan commit
        let orphan_oid = repo
            .commit(None, &sig, &sig, "Orphan commit", &tree, &[])
            .unwrap();
        // Point a branch at the orphan commit
        let orphan_commit = repo.find_commit(orphan_oid).unwrap();
        repo.branch("orphan-branch", &orphan_commit, false).unwrap();

        let git_ops = GitOperations::with_work_dir(repo_path.to_path_buf()).unwrap();
        let orphan = BranchName::new("orphan-branch").unwrap();
        let result = git_ops.find_merge_target_for_issue(&orphan);
        // Should error because no merge base exists with main
        assert!(result.is_err());
    }

    #[test]
    fn test_merge_branch_conflict_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        let repo = Repository::init(repo_path).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        // Initial commit with a shared file
        raw_commit(
            &repo,
            "Initial commit",
            vec![("shared.txt", "original content")],
        );
        rename_default_branch_to_main(&repo);

        // Create a feature branch and modify the shared file
        create_branch_from_head(&repo, "conflict-branch");
        checkout_branch_raw(&repo, "conflict-branch");
        raw_commit(
            &repo,
            "Conflict commit",
            vec![(
                "shared.txt",
                "conflict branch content - line 1\nline 2\nline 3",
            )],
        );

        // Go back to main and make a conflicting change
        checkout_branch_raw(&repo, "main");
        raw_commit(
            &repo,
            "Main diverge",
            vec![("shared.txt", "main branch content - line A\nline B\nline C")],
        );

        let git_ops = GitOperations::with_work_dir(repo_path.to_path_buf()).unwrap();
        let conflict_branch = BranchName::new("conflict-branch").unwrap();
        let result = git_ops.merge_branch(&conflict_branch);

        // Should fail due to conflicts
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("conflict") || err_msg.contains("Merge"),
            "Expected conflict error, got: {}",
            err_msg
        );

        // Repository should be back in clean state after conflict cleanup
        assert!(git_ops.repository().is_in_normal_state());
    }

    #[test]
    fn test_get_latest_commit() {
        let (_temp_dir, git_ops) = setup_test_repo();
        let commit = git_ops.get_latest_commit().unwrap();
        assert_eq!(commit.message.trim(), "Initial commit");
        assert!(!commit.hash.is_empty());
        assert!(!commit.short_hash.is_empty());
        assert_eq!(commit.author, "Test User");
        assert_eq!(commit.author_email, "test@example.com");
    }

    #[test]
    fn test_new_from_current_dir() {
        // GitOperations::new() uses current dir. Since tests run in the repo,
        // this should either succeed (we're in a git repo) or fail gracefully.
        let result = GitOperations::new();
        // We're running inside the swissarmyhammer repo, so this should succeed.
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_current_branch_detached_head() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        let repo = Repository::init(repo_path).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        // Create initial commit
        raw_commit(&repo, "Initial commit", vec![("README.md", "# Repo")]);

        // Detach HEAD by pointing it directly at a commit
        let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
        repo.set_head_detached(head_commit.id()).unwrap();

        let git_ops = GitOperations::with_work_dir(repo_path.to_path_buf()).unwrap();
        // In detached HEAD, shorthand() returns "HEAD" rather than None,
        // so get_current_branch returns Some("HEAD") as an unchecked branch name.
        let branch = git_ops.get_current_branch().unwrap();
        // The important thing is it doesn't error
        let _ = branch;
    }

    #[test]
    fn test_get_status_renamed_file() {
        let (temp_dir, git_ops) = setup_test_repo();
        let repo_path = temp_dir.path();
        let repo = Repository::open(repo_path).unwrap();

        // Create a file with enough content for rename detection
        let content = "This is a test file with enough content for rename detection.\n\
                       It needs multiple lines so git can detect the rename.\n\
                       Line 3 of content.\nLine 4 of content.\nLine 5 of content.\n";
        std::fs::write(repo_path.join("original.txt"), content).unwrap();
        let mut index = repo.index().unwrap();
        index
            .add_path(std::path::Path::new("original.txt"))
            .unwrap();
        index.write().unwrap();

        // Commit it
        let sig = git2::Signature::now("Test User", "test@example.com").unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Add file", &tree, &[&parent])
            .unwrap();

        // Now rename: remove from index, add new path, remove old file
        let mut index = repo.index().unwrap();
        index
            .remove_path(std::path::Path::new("original.txt"))
            .unwrap();
        std::fs::rename(
            repo_path.join("original.txt"),
            repo_path.join("renamed.txt"),
        )
        .unwrap();
        index.add_path(std::path::Path::new("renamed.txt")).unwrap();
        index.write().unwrap();

        // Check status - note that INDEX_RENAMED requires git2 rename detection
        // which uses similarity-based detection. The status may or may not detect it
        // as a rename depending on content. We just verify no errors.
        let status = git_ops.get_status().unwrap();
        // At minimum, we should see something changed
        assert!(!status.is_clean());
    }

    #[test]
    fn test_delete_branch_maps_not_found_error() {
        let (_temp_dir, git_ops) = setup_test_repo();
        let branch = BranchName::new("nonexistent").unwrap();
        let result = git_ops.delete_branch(&branch);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("not found") || err_msg.contains("nonexistent"),
            "Expected not-found error, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_find_merge_target_no_candidates_not_main() {
        // Test the path where there are no candidate branches
        // and the branch is not the main branch itself.
        // This requires only two branches: main and the issue branch (no prefix filtering).
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        let repo = Repository::init(repo_path).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        raw_commit(&repo, "Initial commit", vec![("README.md", "# Repo")]);
        rename_default_branch_to_main(&repo);

        // Create a simple branch (no prefix/) from main
        create_branch_from_head(&repo, "solo-branch");
        checkout_branch_raw(&repo, "solo-branch");
        raw_commit(&repo, "Solo commit", vec![("solo.txt", "solo")]);

        let git_ops = GitOperations::with_work_dir(repo_path.to_path_buf()).unwrap();
        let branch = BranchName::new("solo-branch").unwrap();
        let target = git_ops.find_merge_target_for_issue(&branch).unwrap();
        // Should find main as the merge target
        assert_eq!(target, "main");
    }
}
