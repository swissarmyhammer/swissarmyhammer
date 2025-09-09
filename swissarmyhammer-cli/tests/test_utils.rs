//! Test utilities for SwissArmyHammer CLI tests
//!
//! This module extends the test utilities from the main crate with CLI-specific helpers.

#![allow(dead_code)]

use anyhow::Result;

use std::path::{Path, PathBuf};
use tempfile::TempDir;

// Re-export the ProcessGuard from the main crate's test_utils
#[allow(unused_imports)]
pub use swissarmyhammer::test_utils::ProcessGuard;

// Re-export commonly used test utilities from the main crate
#[allow(unused_imports)]
pub use swissarmyhammer::test_utils::{
    create_simple_test_prompt, create_test_home_guard, create_test_prompt_library,
    create_test_prompts, get_test_home, get_test_swissarmyhammer_dir, TestHomeGuard,
};










/// Create a temporary directory for testing
///
/// Returns a TempDir that will be automatically cleaned up when dropped.
#[allow(dead_code)]
pub fn create_temp_dir() -> Result<TempDir> {
    TempDir::new().map_err(|e| anyhow::anyhow!("Failed to create temporary directory: {}", e))
}

/// Create test prompt files in a directory
///
/// Creates YAML files for each test prompt in the specified directory.
#[allow(dead_code)]
pub fn create_test_prompt_files(dir: &Path) -> Result<()> {
    let prompts = create_test_prompts();
    for prompt in prompts {
        let file_path = dir.join(format!("{}.yaml", prompt.name));
        let content = serde_yaml::to_string(&prompt)
            .map_err(|e| anyhow::anyhow!("Failed to serialize prompt: {}", e))?;
        std::fs::write(&file_path, content)?;
    }
    Ok(())
}

/// Semantic test guard for isolating search database during tests
#[allow(dead_code)]
pub struct SemanticTestGuard {
    _database_file: Option<tempfile::NamedTempFile>,
}

impl SemanticTestGuard {
    /// Create a new semantic test guard with isolated environment
    #[allow(dead_code)]
    pub fn new() -> Self {
        // Create a temporary database file for semantic search testing
        let database_file = tempfile::NamedTempFile::new()
            .expect("Failed to create temporary database file for semantic testing");

        Self {
            _database_file: Some(database_file),
        }
    }
}

impl Drop for SemanticTestGuard {
    fn drop(&mut self) {
        // Clean up is automatic via NamedTempFile drop
    }
}

/// Create a semantic test guard for isolated testing
///
/// Returns a guard that automatically cleans up semantic search database files
/// when dropped, ensuring tests don't interfere with each other.
#[allow(dead_code)]
pub fn create_semantic_test_guard() -> SemanticTestGuard {
    SemanticTestGuard::new()
}

/// Create a test environment with temp directory and prompts directory
///
/// Returns a tuple of (temp_dir, prompts_dir) for testing CLI functionality.
#[allow(dead_code)]
pub fn create_test_environment() -> Result<(TempDir, PathBuf)> {
    let temp_dir = create_temp_dir()?;
    let prompts_dir = temp_dir.path().join("prompts");
    std::fs::create_dir_all(&prompts_dir)?;
    
    // Create test prompt files in the directory
    create_test_prompt_files(&prompts_dir)?;
    
    Ok((temp_dir, prompts_dir))
}

/// Setup a git repository in the given directory using libgit2
///
/// Creates a basic git repository with initial commit for testing
/// git-related CLI functionality.
#[allow(dead_code)]
pub fn setup_git_repo(dir: &Path) -> Result<()> {
    use git2::{Repository, Signature};

    // Initialize git repository
    let repo = Repository::init(dir)
        .map_err(|e| anyhow::anyhow!("Failed to initialize git repository: {}", e))?;

    // Configure git user
    let mut config = repo
        .config()
        .map_err(|e| anyhow::anyhow!("Failed to get repository config: {}", e))?;

    config
        .set_str("user.name", "Test User")
        .map_err(|e| anyhow::anyhow!("Failed to set user.name: {}", e))?;

    config
        .set_str("user.email", "test@example.com")
        .map_err(|e| anyhow::anyhow!("Failed to set user.email: {}", e))?;

    // Create initial commit
    std::fs::write(
        dir.join("README.md"),
        "# Test Repository\n\nThis is a test repository for CLI testing.",
    )?;

    // Add file to index
    let mut index = repo
        .index()
        .map_err(|e| anyhow::anyhow!("Failed to get repository index: {}", e))?;

    index
        .add_path(std::path::Path::new("README.md"))
        .map_err(|e| anyhow::anyhow!("Failed to add README.md to index: {}", e))?;

    index
        .write()
        .map_err(|e| anyhow::anyhow!("Failed to write index: {}", e))?;

    // Create tree and commit
    let tree_oid = index
        .write_tree()
        .map_err(|e| anyhow::anyhow!("Failed to write tree: {}", e))?;

    let tree = repo
        .find_tree(tree_oid)
        .map_err(|e| anyhow::anyhow!("Failed to find tree: {}", e))?;

    let signature = Signature::now("Test User", "test@example.com")
        .map_err(|e| anyhow::anyhow!("Failed to create signature: {}", e))?;

    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Initial commit",
        &tree,
        &[], // No parent commits for initial commit
    )
    .map_err(|e| anyhow::anyhow!("Failed to create initial commit: {}", e))?;

    Ok(())
}

/// Create sample issues for testing
///
/// Creates a set of sample issues in the issues directory for testing
/// issue-related CLI commands.
#[allow(dead_code)]
pub fn create_sample_issues(issues_dir: &Path) -> Result<Vec<String>> {
    let issues = vec![
        ("SAMPLE_001_feature_request", "# Feature Request\n\nImplement new search functionality.\n\n## Details\n- Priority: High\n- Estimated effort: 2 days"),
        ("SAMPLE_002_bug_fix", "# Bug Fix\n\nFix issue with memo deletion.\n\n## Details\n- Priority: Critical\n- Affected component: Memo management"),
        ("SAMPLE_003_documentation", "# Documentation Update\n\nUpdate CLI help documentation.\n\n## Details\n- Priority: Medium\n- Type: Documentation"),
        ("SAMPLE_004_refactoring", "# Code Refactoring\n\nRefactor MCP integration layer.\n\n## Details\n- Priority: Medium\n- Technical debt reduction"),
        ("SAMPLE_005_testing", "# Testing Improvements\n\nAdd more comprehensive test coverage.\n\n## Details\n- Priority: High\n- Type: Quality improvement"),
    ];

    let mut created_issues = vec![];

    for (name, content) in issues {
        let issue_file = issues_dir.join(format!("{name}.md"));
        std::fs::write(&issue_file, content)?;
        created_issues.push(name.to_string());
    }

    Ok(created_issues)
}

/// Create sample source files for search testing
///
/// Creates a set of sample source files for testing search indexing
/// and querying functionality.
#[allow(dead_code)]
pub fn create_sample_source_files(src_dir: &Path) -> Result<Vec<String>> {
    let source_files = vec![
        (
            "main.rs",
            r#"
//! Main application entry point

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("Hello, SwissArmyHammer!");
    
    let config = load_configuration()?;
    let app = initialize_application(config)?;
    
    app.run()?;
    
    Ok(())
}

/// Load application configuration
fn load_configuration() -> Result<Config, ConfigError> {
    Config::from_env()
}

/// Initialize the application with configuration
fn initialize_application(config: Config) -> Result<Application, InitError> {
    Application::new(config)
}
"#,
        ),
        (
            "lib.rs",
            r#"
//! SwissArmyHammer library

pub mod config;
pub mod application;
pub mod error_handling;
pub mod utils;

pub use config::Config;
pub use application::Application;
pub use error_handling::{ErrorHandler, ErrorType};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Initialize library logging
pub fn init_logging() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    Ok(())
}
"#,
        ),
        (
            "config.rs",
            r#"
//! Configuration management

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub database_url: String,
    pub log_level: String,
    pub cache_dir: PathBuf,
    pub max_connections: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Missing required environment variable: {0}")]
    MissingEnvVar(String),
    #[error("Invalid configuration value: {0}")]
    InvalidValue(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            database_url: std::env::var("DATABASE_URL")
                .map_err(|_| ConfigError::MissingEnvVar("DATABASE_URL".to_string()))?,
            log_level: std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
            cache_dir: std::env::var("CACHE_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("/tmp/cache")),
            max_connections: std::env::var("MAX_CONNECTIONS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .map_err(|_| ConfigError::InvalidValue("MAX_CONNECTIONS".to_string()))?,
        })
    }
}
"#,
        ),
        (
            "error_handling.rs",
            r#"
//! Error handling utilities

use std::fmt;

#[derive(Debug, Clone)]
pub enum ErrorType {
    Configuration,
    Database,
    Network,
    Validation,
    Internal,
}

pub struct ErrorHandler {
    error_type: ErrorType,
    message: String,
    context: Option<String>,
}

impl ErrorHandler {
    pub fn new(error_type: ErrorType, message: impl Into<String>) -> Self {
        Self {
            error_type,
            message: message.into(),
            context: None,
        }
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    pub fn handle_error(&self) -> Result<(), Box<dyn std::error::Error>> {
        match self.error_type {
            ErrorType::Configuration => {
                eprintln!("Configuration error: {}", self.message);
            }
            ErrorType::Database => {
                eprintln!("Database error: {}", self.message);
            }
            ErrorType::Network => {
                eprintln!("Network error: {}", self.message);
            }
            ErrorType::Validation => {
                eprintln!("Validation error: {}", self.message);
            }
            ErrorType::Internal => {
                eprintln!("Internal error: {}", self.message);
            }
        }

        if let Some(context) = &self.context {
            eprintln!("Context: {}", context);
        }

        Ok(())
    }
}

impl fmt::Display for ErrorHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.error_type, self.message)
    }
}

impl std::error::Error for ErrorHandler {}
"#,
        ),
        (
            "utils.rs",
            r#"
//! Utility functions

use std::collections::HashMap;
use std::hash::Hash;

/// Generic cache implementation
pub struct Cache<K, V> 
where 
    K: Hash + Eq + Clone,
    V: Clone,
{
    data: HashMap<K, V>,
    max_size: usize,
}

impl<K, V> Cache<K, V> 
where 
    K: Hash + Eq + Clone,
    V: Clone,
{
    pub fn new(max_size: usize) -> Self {
        Self {
            data: HashMap::new(),
            max_size,
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.data.get(key)
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        if self.data.len() >= self.max_size && !self.data.contains_key(&key) {
            // Simple eviction: remove first item
            if let Some(first_key) = self.data.keys().next().cloned() {
                self.data.remove(&first_key);
            }
        }
        self.data.insert(key, value)
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.data.remove(key)
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Utility function for data processing
pub fn process_batch<T, F, R>(items: Vec<T>, processor: F) -> Vec<R>
where
    F: Fn(T) -> R,
{
    items.into_iter().map(processor).collect()
}

/// Async utility function
pub async fn async_operation_with_retry<F, T, E>(
    operation: F,
    max_retries: usize,
) -> Result<T, E>
where
    F: Fn() -> Result<T, E>,
{
    let mut attempts = 0;
    loop {
        match operation() {
            Ok(result) => return Ok(result),
            Err(e) => {
                attempts += 1;
                if attempts >= max_retries {
                    return Err(e);
                }
                // In a real implementation, we might want to add delay here
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_basic_operations() {
        let mut cache = Cache::new(3);
        
        assert!(cache.is_empty());
        
        cache.insert("key1", "value1");
        cache.insert("key2", "value2");
        
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get(&"key1"), Some(&"value1"));
        assert_eq!(cache.get(&"key2"), Some(&"value2"));
        assert_eq!(cache.get(&"key3"), None);
    }

    #[test]
    fn test_cache_eviction() {
        let mut cache = Cache::new(2);
        
        cache.insert("key1", "value1");
        cache.insert("key2", "value2");
        cache.insert("key3", "value3"); // Should evict key1
        
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get(&"key1"), None);
        assert_eq!(cache.get(&"key2"), Some(&"value2"));
        assert_eq!(cache.get(&"key3"), Some(&"value3"));
    }

    #[test]
    fn test_process_batch() {
        let numbers = vec![1, 2, 3, 4, 5];
        let doubled = process_batch(numbers, |x| x * 2);
        assert_eq!(doubled, vec![2, 4, 6, 8, 10]);
    }
}
"#,
        ),
    ];

    let mut created_files = vec![];

    for (filename, content) in source_files {
        let file_path = src_dir.join(filename);
        std::fs::write(&file_path, content)?;
        created_files.push(filename.to_string());
    }

    Ok(created_files)
}

/// Git2-based test utilities for replacing shell git commands
#[allow(dead_code)]
pub mod git2_test_utils {
    use anyhow::Result;
    use git2::{BranchType, Repository, Signature};
    use std::path::Path;

    /// Initialize a git repository at the specified path with basic configuration
    pub fn init_repo(path: &Path) -> Result<Repository> {
        let repo = Repository::init(path)?;

        // Configure git user
        let mut config = repo.config()?;
        config.set_str("user.name", "Test User")?;
        config.set_str("user.email", "test@example.com")?;

        Ok(repo)
    }

    /// Add all files and create a commit with the given message
    pub fn create_commit(repo: &Repository, message: &str) -> Result<git2::Oid> {
        let mut index = repo.index()?;
        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
        index.write()?;

        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;
        let signature = Signature::now("Test User", "test@example.com")?;

        let commit_id = if let Ok(head) = repo.head() {
            let parent = head.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &[&parent],
            )?
        } else {
            repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &[])?
        };

        Ok(commit_id)
    }

    /// Create a new branch with the given name from HEAD
    pub fn create_branch<'a>(repo: &'a Repository, name: &str) -> Result<git2::Branch<'a>> {
        let head_commit = repo.head()?.peel_to_commit()?;
        let branch = repo.branch(name, &head_commit, false)?;
        Ok(branch)
    }

    /// Checkout to the specified branch
    pub fn checkout_branch(repo: &Repository, branch_name: &str) -> Result<()> {
        let branch = repo.find_branch(branch_name, BranchType::Local)?;
        let branch_ref = branch.get();
        let tree = branch_ref.peel_to_tree()?;

        repo.checkout_tree(tree.as_object(), None)?;
        repo.set_head(&format!("refs/heads/{}", branch_name))?;

        Ok(())
    }

    /// Add specific files to the index
    pub fn add_files(repo: &Repository, paths: &[&str]) -> Result<()> {
        let mut index = repo.index()?;
        for path in paths {
            index.add_path(std::path::Path::new(path))?;
        }
        index.write()?;
        Ok(())
    }

    /// Create a commit with specific parent commits
    pub fn create_commit_with_parents(
        repo: &Repository,
        message: &str,
        parents: &[&git2::Commit],
    ) -> Result<git2::Oid> {
        let mut index = repo.index()?;
        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;
        let signature = Signature::now("Test User", "test@example.com")?;

        let commit_id = repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            parents,
        )?;

        Ok(commit_id)
    }

    /// Get the current branch name
    pub fn current_branch_name(repo: &Repository) -> Result<String> {
        let head = repo.head()?;
        if let Some(name) = head.shorthand() {
            Ok(name.to_string())
        } else {
            Err(anyhow::anyhow!("Unable to determine current branch name"))
        }
    }

    /// Check if repository has uncommitted changes
    pub fn has_uncommitted_changes(repo: &Repository) -> Result<bool> {
        let statuses = repo.statuses(None)?;
        Ok(!statuses.is_empty())
    }

    /// Create multiple commits to simulate development history
    pub fn create_commits_with_history(
        repo: &Repository,
        commits: &[(String, String)],
    ) -> Result<Vec<git2::Oid>> {
        let mut commit_ids = vec![];

        for (filename, content) in commits {
            std::fs::write(repo.workdir().unwrap().join(filename), content)?;
            add_files(repo, &[filename])?;
            let commit_id = create_commit(repo, &format!("Add {}", filename))?;
            commit_ids.push(commit_id);
        }

        Ok(commit_ids)
    }

    /// Create a branch and switch to it (equivalent to git checkout -b)
    pub fn create_and_checkout_branch<'a>(
        repo: &'a Repository,
        name: &str,
    ) -> Result<git2::Branch<'a>> {
        let branch = create_branch(repo, name)?;
        checkout_branch(repo, name)?;
        Ok(branch)
    }

    /// Merge a branch into current branch
    pub fn merge_branch(repo: &Repository, branch_name: &str) -> Result<git2::Oid> {
        let current_head = repo.head()?.target().unwrap();
        let branch = repo.find_branch(branch_name, BranchType::Local)?;
        let branch_commit = branch.get().peel_to_commit()?;

        let merge_base = repo.merge_base(current_head, branch_commit.id())?;
        let current_commit = repo.find_commit(current_head)?;

        if merge_base == branch_commit.id() {
            // Already up to date
            return Ok(current_head);
        }

        if merge_base == current_head {
            // Fast-forward merge
            let branch_ref = branch.get();
            let tree = branch_ref.peel_to_tree()?;
            repo.checkout_tree(tree.as_object(), None)?;
            repo.head()?
                .set_target(branch_commit.id(), "Fast-forward merge")?;
            return Ok(branch_commit.id());
        }

        // Create merge commit
        let signature = Signature::now("Test User", "test@example.com")?;
        let tree_id = repo.index()?.write_tree()?;
        let tree = repo.find_tree(tree_id)?;

        let merge_commit_id = repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            &format!("Merge branch '{}'", branch_name),
            &tree,
            &[&current_commit, &branch_commit],
        )?;

        Ok(merge_commit_id)
    }

    /// Complete setup function that creates a repo with multiple branches for testing
    pub fn setup_git_repo_with_branches(path: &Path) -> Result<Repository> {
        let repo = init_repo(path)?;

        // Create initial commit on main branch
        std::fs::write(
            path.join("README.md"),
            "# Test Project\n\nMain branch content",
        )?;
        add_files(&repo, &["README.md"])?;
        create_commit(&repo, "Initial commit")?;

        // Create feature branch
        create_and_checkout_branch(&repo, "feature/user-authentication")?;

        std::fs::write(path.join("auth.rs"), "// User authentication module")?;
        add_files(&repo, &["auth.rs"])?;
        create_commit(&repo, "Add authentication module")?;

        // Switch back to main
        checkout_branch(&repo, "main")?;

        Ok(repo)
    }

    /// Advanced setup with development branch
    pub fn setup_git_repo_with_dev_branch(path: &Path) -> Result<Repository> {
        let repo = setup_git_repo_with_branches(path)?;

        // Create development branch
        create_and_checkout_branch(&repo, "development")?;

        std::fs::write(path.join("dev_feature.rs"), "// Development feature")?;
        add_files(&repo, &["dev_feature.rs"])?;
        create_commit(&repo, "Add development feature")?;

        // Switch back to main
        checkout_branch(&repo, "main")?;

        Ok(repo)
    }

    /// Setup for performance testing with many commits
    pub fn setup_git_repo_for_performance_testing(
        path: &Path,
        commit_count: usize,
    ) -> Result<Repository> {
        let repo = init_repo(path)?;

        // Create initial commit
        std::fs::write(path.join("README.md"), "# Performance Test Repository")?;
        add_files(&repo, &["README.md"])?;
        create_commit(&repo, "Initial commit")?;

        // Create many commits for performance testing
        for i in 1..=commit_count {
            let filename = format!("file_{:04}.txt", i);
            let content = format!("Content for file {}", i);
            std::fs::write(path.join(&filename), content)?;
            add_files(&repo, &[&filename])?;
            create_commit(&repo, &format!("Add {}", filename))?;
        }

        Ok(repo)
    }

    /// Get repository status (equivalent to git status --porcelain)
    pub fn get_status_porcelain(repo: &Repository) -> Result<String> {
        let statuses = repo.statuses(None)?;
        let mut output = String::new();

        for entry in statuses.iter() {
            let status = entry.status();
            let path = entry.path().unwrap_or("");

            let prefix = if status.contains(git2::Status::INDEX_NEW) {
                "A "
            } else if status.contains(git2::Status::INDEX_MODIFIED) {
                "M "
            } else if status.contains(git2::Status::INDEX_DELETED) {
                "D "
            } else if status.contains(git2::Status::WT_NEW) {
                "??"
            } else if status.contains(git2::Status::WT_MODIFIED) {
                " M"
            } else if status.contains(git2::Status::WT_DELETED) {
                " D"
            } else {
                "  "
            };

            output.push_str(&format!("{} {}\n", prefix, path));
        }

        Ok(output)
    }

    /// Create a tag at the current HEAD
    pub fn create_tag(repo: &Repository, tag_name: &str, message: &str) -> Result<git2::Oid> {
        let head_commit = repo.head()?.peel_to_commit()?;
        let signature = Signature::now("Test User", "test@example.com")?;

        let tag_id = repo.tag(
            tag_name,
            head_commit.as_object(),
            &signature,
            message,
            false,
        )?;

        Ok(tag_id)
    }

    /// List all branches in the repository
    pub fn list_branches(repo: &Repository) -> Result<Vec<String>> {
        let branches = repo.branches(Some(BranchType::Local))?;
        let mut branch_names = vec![];

        for branch_result in branches {
            let (branch, _) = branch_result?;
            if let Some(name) = branch.name()? {
                branch_names.push(name.to_string());
            }
        }

        Ok(branch_names)
    }

    /// Delete a branch
    pub fn delete_branch(repo: &Repository, branch_name: &str) -> Result<()> {
        let mut branch = repo.find_branch(branch_name, BranchType::Local)?;
        branch.delete()?;
        Ok(())
    }

    /// Get commit count on current branch
    pub fn commit_count(repo: &Repository) -> Result<usize> {
        let mut revwalk = repo.revwalk()?;
        revwalk.push_head()?;
        Ok(revwalk.count())
    }

    /// Check if a branch exists
    pub fn branch_exists(repo: &Repository, branch_name: &str) -> bool {
        repo.find_branch(branch_name, BranchType::Local).is_ok()
    }
}
