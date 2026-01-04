//! Git2 utility functions compatible with the original implementation
//!
//! This module provides utility functions that match the original git2_utils
//! interface for backward compatibility with existing test code.

use crate::error::{convert_git2_error, GitResult};
use git2::Repository;
use std::path::Path;
use swissarmyhammer_common::Pretty;
use tracing::debug;

/// Add files to the index (matches original git2_utils interface)
///
/// # Arguments
/// * `repo` - Git repository reference
/// * `paths` - Array of file paths to add
///
/// # Returns
/// * `Ok(())` - Files added successfully
/// * `Err(GitError)` - Add operation failed
pub fn add_files(repo: &Repository, paths: &[&str]) -> GitResult<()> {
    debug!("Adding files to index: {}", Pretty(&paths));

    let mut index = repo
        .index()
        .map_err(|e| convert_git2_error("get_index", e))?;

    for path in paths {
        index
            .add_path(Path::new(path))
            .map_err(|e| convert_git2_error("add_path", e))?;
    }

    index
        .write()
        .map_err(|e| convert_git2_error("write_index", e))?;

    Ok(())
}

/// Create a commit (matches original git2_utils interface)
///
/// # Arguments
/// * `repo` - Git repository reference
/// * `message` - Commit message
/// * `author_name` - Author name (None uses config)
/// * `author_email` - Author email (None uses config)
///
/// # Returns
/// * `Ok(String)` - Commit hash
/// * `Err(GitError)` - Commit creation failed
pub fn create_commit(
    repo: &Repository,
    message: &str,
    author_name: Option<&str>,
    author_email: Option<&str>,
) -> GitResult<String> {
    debug!("Creating commit with message: {}", message);

    // Get signature (author and committer)
    let signature = if let (Some(name), Some(email)) = (author_name, author_email) {
        git2::Signature::now(name, email).map_err(|e| convert_git2_error("create_signature", e))?
    } else {
        // Use repository config
        repo.signature()
            .map_err(|e| convert_git2_error("get_signature", e))?
    };

    // Get the index and write tree
    let mut index = repo
        .index()
        .map_err(|e| convert_git2_error("get_index", e))?;
    let tree_oid = index
        .write_tree()
        .map_err(|e| convert_git2_error("write_tree", e))?;
    let tree = repo
        .find_tree(tree_oid)
        .map_err(|e| convert_git2_error("find_tree", e))?;

    // Get parent commit(s)
    let parents: Vec<git2::Commit> = match repo.head() {
        Ok(head) => {
            let commit = head
                .peel_to_commit()
                .map_err(|e| convert_git2_error("peel_to_commit", e))?;
            vec![commit]
        }
        Err(_) => Vec::new(), // Initial commit
    };

    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

    // Create the commit
    let commit_oid = repo
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parent_refs,
        )
        .map_err(|e| convert_git2_error("create_commit", e))?;

    Ok(commit_oid.to_string())
}
