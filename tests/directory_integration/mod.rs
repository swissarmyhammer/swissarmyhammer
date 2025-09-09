//! Directory Integration Test Utilities
//!
//! This module provides comprehensive test utilities for testing SwissArmyHammer's 
//! directory system integration, particularly focusing on the Git repository-centric
//! approach where .swissarmyhammer directories must exist at Git repository roots.

use git2::{Repository, RepositoryInitOptions, Signature};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tempfile::TempDir;

/// RAII guard for creating isolated Git repository test environments
///
/// This guard creates a temporary directory with a proper Git repository structure
/// and optionally creates a .swissarmyhammer directory with all expected subdirectories.
/// It automatically restores the original working directory when dropped.
///
/// # Features
///
/// - Creates genuine Git repository with proper .git structure
/// - Optionally creates .swissarmyhammer directory with standard layout
/// - Supports creating realistic project structures (src/, docs/, etc.)
/// - Automatically changes to repository directory during test
/// - Restores original working directory on drop
/// - Thread-safe for parallel test execution
///
/// # Example
///
/// ```rust
/// use tests::directory_integration::GitRepositoryTestGuard;
///
/// #[test]
/// fn test_git_directory_operations() {
///     let guard = GitRepositoryTestGuard::new().with_swissarmyhammer();
///     
///     // Now in isolated Git repository with .swissarmyhammer directory
///     assert!(Path::new(".git").exists());
///     assert!(Path::new(".swissarmyhammer").exists());
/// }
/// ```
pub struct GitRepositoryTestGuard {
    temp_dir: TempDir,
    git_repo: Repository,
    original_cwd: PathBuf,
    created_swissarmyhammer: bool,
}

impl GitRepositoryTestGuard {
    /// Create a new Git repository test guard with minimal setup
    ///
    /// Creates a temporary directory with a Git repository but no .swissarmyhammer
    /// directory. Use `with_swissarmyhammer()` to add the SwissArmyHammer directory
    /// structure.
    pub fn new() -> Self {
        let original_cwd = env::current_dir().expect("Failed to get current working directory");
        let temp_dir = TempDir::new().expect("Failed to create temporary directory");
        let repo_path = temp_dir.path();

        // Initialize Git repository with default branch
        let mut init_opts = RepositoryInitOptions::new();
        init_opts.initial_head("main");
        
        let git_repo = Repository::init_opts(repo_path, &init_opts)
            .expect("Failed to initialize Git repository");

        // Create initial commit to establish the repository
        Self::create_initial_commit(&git_repo).expect("Failed to create initial commit");

        // Change to repository directory for test execution
        env::set_current_dir(repo_path).expect("Failed to change to repository directory");

        Self {
            temp_dir,
            git_repo,
            original_cwd,
            created_swissarmyhammer: false,
        }
    }

    /// Create a Git repository test guard with .swissarmyhammer directory
    ///
    /// Convenience method that creates a Git repository and immediately adds
    /// the .swissarmyhammer directory with standard subdirectory structure.
    pub fn new_with_swissarmyhammer() -> Self {
        Self::new().with_swissarmyhammer()
    }

    /// Add .swissarmyhammer directory with standard subdirectory structure
    ///
    /// Creates the .swissarmyhammer directory and all expected subdirectories:
    /// - memos/ for memoranda storage
    /// - todo/ for todo lists  
    /// - issues/ and issues/complete/ for issue tracking
    /// - workflows/ for local workflow storage
    ///
    /// Returns self for method chaining.
    pub fn with_swissarmyhammer(mut self) -> Self {
        let swissarmyhammer_dir = self.temp_dir.path().join(".swissarmyhammer");
        
        // Create .swissarmyhammer directory and standard subdirectories
        fs::create_dir_all(&swissarmyhammer_dir)
            .expect("Failed to create .swissarmyhammer directory");
        
        fs::create_dir_all(swissarmyhammer_dir.join("memos"))
            .expect("Failed to create memos directory");
        
        fs::create_dir_all(swissarmyhammer_dir.join("todo"))
            .expect("Failed to create todo directory");
        
        fs::create_dir_all(swissarmyhammer_dir.join("issues"))
            .expect("Failed to create issues directory");
            
        fs::create_dir_all(swissarmyhammer_dir.join("issues").join("complete"))
            .expect("Failed to create issues/complete directory");
        
        fs::create_dir_all(swissarmyhammer_dir.join("workflows"))
            .expect("Failed to create workflows directory");

        self.created_swissarmyhammer = true;
        self
    }

    /// Create a realistic project directory structure
    ///
    /// Creates common project directories like src/, docs/, tests/, examples/
    /// to simulate a real repository structure. This helps test that directory
    /// resolution works correctly from various subdirectories.
    ///
    /// Returns self for method chaining.
    pub fn with_project_structure(self) -> Self {
        let base = self.temp_dir.path();
        
        // Create common project directories
        let directories = [
            "src",
            "src/lib", 
            "src/bin",
            "docs",
            "tests",
            "examples",
            "scripts",
            "assets",
            ".github",
            ".github/workflows",
        ];

        for dir in &directories {
            fs::create_dir_all(base.join(dir))
                .unwrap_or_else(|e| panic!("Failed to create directory {}: {}", dir, e));
        }

        // Create some sample files
        fs::write(base.join("README.md"), "# Test Project\n")
            .expect("Failed to create README.md");
        
        fs::write(base.join("src/main.rs"), "fn main() {\n    println!(\"Hello, world!\");\n}\n")
            .expect("Failed to create src/main.rs");
            
        fs::write(base.join("Cargo.toml"), "[package]\nname = \"test-project\"\nversion = \"0.1.0\"\n")
            .expect("Failed to create Cargo.toml");

        self
    }

    /// Create a nested directory structure for testing deep path resolution
    ///
    /// Creates a deeply nested directory structure to test that directory
    /// resolution works correctly even from deeply nested subdirectories
    /// and respects MAX_DIRECTORY_DEPTH limits.
    ///
    /// # Arguments
    ///
    /// * `depth` - Number of nested directories to create (should be <= MAX_DIRECTORY_DEPTH)
    ///
    /// Returns the path to the deepest directory created.
    pub fn create_deep_structure(&self, depth: usize) -> PathBuf {
        let mut current_path = self.temp_dir.path().to_path_buf();
        
        for i in 0..depth {
            current_path = current_path.join(format!("level{}", i));
            fs::create_dir_all(&current_path)
                .unwrap_or_else(|e| panic!("Failed to create directory level{}: {}", i, e));
        }
        
        current_path
    }

    /// Create a Git worktree scenario where .git is a file, not a directory
    ///
    /// This simulates the Git worktree scenario where the .git "directory"
    /// is actually a file containing a gitdir reference to the actual .git
    /// directory. This tests edge cases in Git repository detection.
    ///
    /// Returns self for method chaining.
    pub fn as_git_worktree(self) -> Self {
        let git_dir = self.temp_dir.path().join(".git");
        let worktree_git_dir = self.temp_dir.path().join(".git_worktree");
        
        // Move actual .git directory
        fs::rename(&git_dir, &worktree_git_dir)
            .expect("Failed to move .git directory");
        
        // Create .git file pointing to moved directory
        let gitdir_content = format!("gitdir: {}", worktree_git_dir.display());
        fs::write(&git_dir, gitdir_content)
            .expect("Failed to create .git file");
        
        self
    }

    /// Create multiple nested Git repositories for testing repository precedence
    ///
    /// Creates nested Git repositories to test that directory resolution
    /// correctly identifies the nearest Git repository root rather than
    /// a parent repository.
    ///
    /// Returns the path to the nested repository directory.
    pub fn with_nested_git_repository(&self) -> PathBuf {
        let nested_dir = self.temp_dir.path().join("nested-project");
        fs::create_dir_all(&nested_dir)
            .expect("Failed to create nested directory");
        
        // Initialize nested Git repository
        let mut init_opts = RepositoryInitOptions::new();
        init_opts.initial_head("main");
        
        let nested_repo = Repository::init_opts(&nested_dir, &init_opts)
            .expect("Failed to initialize nested Git repository");
        
        // Create initial commit in nested repository
        Self::create_initial_commit(&nested_repo)
            .expect("Failed to create initial commit in nested repository");
        
        nested_dir
    }

    /// Get the path to the temporary directory (repository root)
    pub fn path(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Get the path to the .swissarmyhammer directory (if created)
    ///
    /// Returns None if .swissarmyhammer directory was not created.
    pub fn swissarmyhammer_dir(&self) -> Option<PathBuf> {
        if self.created_swissarmyhammer {
            Some(self.temp_dir.path().join(".swissarmyhammer"))
        } else {
            None
        }
    }

    /// Get the path to the .git directory or file
    pub fn git_dir(&self) -> PathBuf {
        self.temp_dir.path().join(".git")
    }

    /// Get a reference to the underlying Git repository
    pub fn repository(&self) -> &Repository {
        &self.git_repo
    }

    /// Change working directory to a subdirectory within the repository
    ///
    /// This is useful for testing directory resolution from various subdirectories.
    /// The original working directory will still be restored when the guard is dropped.
    ///
    /// # Arguments
    ///
    /// * `subdir` - Relative path to subdirectory within the repository
    pub fn cd_to_subdir<P: AsRef<Path>>(&self, subdir: P) -> std::io::Result<()> {
        let target_dir = self.temp_dir.path().join(subdir.as_ref());
        env::set_current_dir(target_dir)
    }

    /// Create initial commit to establish the Git repository
    ///
    /// This is necessary because an empty Git repository without any commits
    /// can behave differently than a repository with at least one commit.
    fn create_initial_commit(repo: &Repository) -> Result<(), git2::Error> {
        // Create initial README file
        let readme_path = repo.path().parent().unwrap().join("README.md");
        fs::write(&readme_path, "# Test Repository\n\nCreated for integration testing.\n")
            .map_err(|e| git2::Error::from_str(&format!("Failed to create README: {}", e)))?;

        // Stage the README file
        let mut index = repo.index()?;
        index.add_path(Path::new("README.md"))?;
        index.write()?;

        // Create commit
        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;
        
        // Create signature for commits
        let signature = Signature::now("Test User", "test@example.com")?;
        
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &tree,
            &[],
        )?;

        Ok(())
    }
}

impl Default for GitRepositoryTestGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for GitRepositoryTestGuard {
    fn drop(&mut self) {
        // Restore original working directory
        if let Err(e) = env::set_current_dir(&self.original_cwd) {
            eprintln!("Warning: Failed to restore working directory: {}", e);
        }
    }
}

/// Create a temporary directory with multiple .swissarmyhammer directories
///
/// This utility function creates a directory structure that simulates the
/// legacy behavior where multiple .swissarmyhammer directories could exist
/// in a directory hierarchy. Used for testing migration scenarios.
///
/// # Returns
///
/// A tuple containing:
/// - TempDir guard (must be kept alive)
/// - PathBuf to the deepest directory  
/// - Vec of paths to all .swissarmyhammer directories created
pub fn create_legacy_directory_structure() -> (TempDir, PathBuf, Vec<PathBuf>) {
    let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    let base = temp_dir.path();
    
    // Create nested directory structure
    let level1 = base.join("project");
    let level2 = level1.join("backend");
    let level3 = level2.join("services");
    let level4 = level3.join("auth");
    
    fs::create_dir_all(&level4).expect("Failed to create nested directories");
    
    // Create .swissarmyhammer directories at multiple levels
    let swissarmyhammer_dirs = vec![
        base.join(".swissarmyhammer"),
        level1.join(".swissarmyhammer"),
        level3.join(".swissarmyhammer"),
    ];
    
    for dir in &swissarmyhammer_dirs {
        fs::create_dir_all(dir).expect("Failed to create .swissarmyhammer directory");
        
        // Create some content to make directories non-empty
        fs::create_dir_all(dir.join("memos")).expect("Failed to create memos subdirectory");
        fs::write(dir.join("memos").join("test.md"), "# Test memo\nContent")
            .expect("Failed to create test memo");
    }
    
    (temp_dir, level4, swissarmyhammer_dirs)
}

/// Create a large Git repository for performance testing
///
/// Creates a Git repository with many files, commits, and deep directory
/// structure to test performance characteristics of directory resolution.
///
/// # Arguments
///
/// * `num_commits` - Number of commits to create in the repository
/// * `files_per_commit` - Number of files to create per commit
///
/// # Returns
///
/// GitRepositoryTestGuard with the performance test repository
pub fn create_large_git_repository(num_commits: usize, files_per_commit: usize) -> GitRepositoryTestGuard {
    let mut guard = GitRepositoryTestGuard::new().with_swissarmyhammer().with_project_structure();
    let repo = &guard.git_repo;
    let base_path = guard.temp_dir.path();
    
    // Create signature for commits
    let signature = Signature::now("Perf Test", "perf@test.com")
        .expect("Failed to create signature");
    
    for commit_num in 0..num_commits {
        // Create files for this commit
        for file_num in 0..files_per_commit {
            let file_path = base_path.join("src").join(format!("file_{}_{}.rs", commit_num, file_num));
            let content = format!(
                "//! File {} in commit {}\n\npub fn function_{}() {{\n    println!(\"Hello from file {}\");\n}}\n",
                file_num, commit_num, file_num, file_num
            );
            fs::write(&file_path, content)
                .expect("Failed to write performance test file");
        }
        
        // Stage and commit files
        let mut index = repo.index().expect("Failed to get repository index");
        index.add_all(["src/"].iter(), git2::IndexAddOption::DEFAULT, None)
            .expect("Failed to stage files");
        index.write().expect("Failed to write index");
        
        let tree_id = index.write_tree().expect("Failed to write tree");
        let tree = repo.find_tree(tree_id).expect("Failed to find tree");
        
        // Get parent commit if not first commit
        let parent_commits = if commit_num == 0 {
            vec![]
        } else {
            vec![repo.head().unwrap().peel_to_commit().unwrap()]
        };
        
        let parent_refs: Vec<&git2::Commit> = parent_commits.iter().collect();
        
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            &format!("Add {} files (commit {})", files_per_commit, commit_num + 1),
            &tree,
            &parent_refs,
        ).expect("Failed to create performance test commit");
    }
    
    guard
}

/// Utility function to measure execution time of a function
///
/// This is useful for performance testing to measure how long directory
/// resolution operations take.
///
/// # Arguments
///
/// * `f` - Function to measure
///
/// # Returns
///
/// Tuple of (function_result, duration)
pub fn measure_time<F, R>(f: F) -> (R, Duration)
where
    F: FnOnce() -> R,
{
    let start = std::time::Instant::now();
    let result = f();
    let duration = start.elapsed();
    (result, duration)
}

/// Create a corrupt .git directory for testing error handling
///
/// Creates a directory structure that looks like a Git repository but
/// has some corruption to test error handling paths.
///
/// # Returns
///
/// TempDir containing the corrupt Git repository
pub fn create_corrupt_git_repository() -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    let git_dir = temp_dir.path().join(".git");
    
    // Create .git directory structure but with missing/corrupt files
    fs::create_dir_all(&git_dir).expect("Failed to create .git directory");
    fs::create_dir_all(git_dir.join("objects")).expect("Failed to create objects directory");
    fs::create_dir_all(git_dir.join("refs")).expect("Failed to create refs directory");
    
    // Create corrupt HEAD file
    fs::write(git_dir.join("HEAD"), "corrupt content that is not a valid git ref")
        .expect("Failed to create corrupt HEAD file");
    
    temp_dir
}

/// Generate unique test data to prevent test interference
///
/// Creates unique content based on current timestamp to ensure tests
/// don't interfere with each other when running in parallel.
///
/// # Returns
///
/// String containing unique test identifier
pub fn generate_test_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_nanos();
    format!("test_{}", timestamp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_common::utils::{find_git_repository_root, find_swissarmyhammer_directory};

    #[test]
    fn test_git_repository_test_guard_basic() {
        let guard = GitRepositoryTestGuard::new();
        
        // Should be in a temporary directory with Git repository
        assert!(Path::new(".git").exists());
        
        // Git repository should be functional
        let git_root = find_git_repository_root();
        assert!(git_root.is_some());
        assert_eq!(git_root.unwrap(), guard.path());
    }

    #[test]
    fn test_git_repository_test_guard_with_swissarmyhammer() {
        let guard = GitRepositoryTestGuard::new_with_swissarmyhammer();
        
        // Should have both Git repository and .swissarmyhammer directory
        assert!(Path::new(".git").exists());
        assert!(Path::new(".swissarmyhammer").exists());
        assert!(Path::new(".swissarmyhammer/memos").exists());
        assert!(Path::new(".swissarmyhammer/todo").exists());
        assert!(Path::new(".swissarmyhammer/issues").exists());
        
        // SwissArmyHammer directory resolution should work
        let swissarmyhammer_dir = find_swissarmyhammer_directory();
        assert!(swissarmyhammer_dir.is_some());
        assert_eq!(swissarmyhammer_dir.unwrap(), guard.swissarmyhammer_dir().unwrap());
    }

    #[test]
    fn test_git_repository_test_guard_with_project_structure() {
        let guard = GitRepositoryTestGuard::new()
            .with_swissarmyhammer()
            .with_project_structure();
        
        // Should have project directories
        assert!(Path::new("src").exists());
        assert!(Path::new("docs").exists());
        assert!(Path::new("README.md").exists());
        assert!(Path::new("Cargo.toml").exists());
        
        // Directory resolution should work from subdirectories
        guard.cd_to_subdir("src/lib").expect("Failed to change to subdirectory");
        
        let git_root = find_git_repository_root();
        assert!(git_root.is_some());
        assert_eq!(git_root.unwrap(), guard.path());
        
        let swissarmyhammer_dir = find_swissarmyhammer_directory();
        assert!(swissarmyhammer_dir.is_some());
    }

    #[test]
    fn test_git_repository_test_guard_deep_structure() {
        let guard = GitRepositoryTestGuard::new().with_swissarmyhammer();
        let deep_path = guard.create_deep_structure(5);
        
        // Should be able to resolve Git repository from deep path
        guard.cd_to_subdir(deep_path.strip_prefix(guard.path()).unwrap())
            .expect("Failed to change to deep directory");
        
        let git_root = find_git_repository_root();
        assert!(git_root.is_some());
        assert_eq!(git_root.unwrap(), guard.path());
    }

    #[test]
    fn test_git_worktree_scenario() {
        let guard = GitRepositoryTestGuard::new()
            .with_swissarmyhammer()
            .as_git_worktree();
        
        // .git should exist but be a file, not directory
        let git_path = Path::new(".git");
        assert!(git_path.exists());
        assert!(git_path.is_file());
        
        // Repository detection should still work
        let git_root = find_git_repository_root();
        assert!(git_root.is_some());
        assert_eq!(git_root.unwrap(), guard.path());
    }

    #[test]
    fn test_nested_git_repositories() {
        let guard = GitRepositoryTestGuard::new().with_swissarmyhammer();
        let nested_path = guard.with_nested_git_repository();
        
        // Change to nested repository
        guard.cd_to_subdir(nested_path.strip_prefix(guard.path()).unwrap())
            .expect("Failed to change to nested repository");
        
        // Should detect nested repository, not parent
        let git_root = find_git_repository_root();
        assert!(git_root.is_some());
        assert_eq!(git_root.unwrap(), nested_path);
        
        // No SwissArmyHammer directory in nested repository
        let swissarmyhammer_dir = find_swissarmyhammer_directory();
        assert!(swissarmyhammer_dir.is_none());
    }

    #[test]
    fn test_legacy_directory_structure() {
        let (_temp_dir, deepest_path, swissarmyhammer_dirs) = create_legacy_directory_structure();
        
        // Should have created multiple .swissarmyhammer directories
        assert_eq!(swissarmyhammer_dirs.len(), 3);
        
        for dir in &swissarmyhammer_dirs {
            assert!(dir.exists());
            assert!(dir.join("memos").exists());
            assert!(dir.join("memos/test.md").exists());
        }
        
        // Deepest path should exist
        assert!(deepest_path.exists());
    }

    #[test]
    fn test_measure_time_utility() {
        let (result, duration) = measure_time(|| {
            std::thread::sleep(Duration::from_millis(10));
            42
        });
        
        assert_eq!(result, 42);
        assert!(duration >= Duration::from_millis(10));
        assert!(duration < Duration::from_millis(100)); // Should be reasonable
    }

    #[test]
    fn test_generate_test_id_uniqueness() {
        let id1 = generate_test_id();
        std::thread::sleep(Duration::from_nanos(1));
        let id2 = generate_test_id();
        
        assert_ne!(id1, id2);
        assert!(id1.starts_with("test_"));
        assert!(id2.starts_with("test_"));
    }

    #[test]
    fn test_concurrent_git_repository_guards() {
        use std::thread;
        
        let handles: Vec<_> = (0..5)
            .map(|i| {
                thread::spawn(move || {
                    let guard = GitRepositoryTestGuard::new().with_swissarmyhammer();
                    
                    // Each guard should have its own isolated environment
                    assert!(Path::new(".git").exists());
                    assert!(Path::new(".swissarmyhammer").exists());
                    
                    let git_root = find_git_repository_root();
                    assert!(git_root.is_some());
                    assert_eq!(git_root.unwrap(), guard.path());
                    
                    // Create unique content to verify isolation
                    let test_file = format!("test_{}.txt", i);
                    fs::write(&test_file, format!("Thread {} test content", i))
                        .expect("Failed to write test file");
                    
                    assert!(Path::new(&test_file).exists());
                    
                    i
                })
            })
            .collect();

        let results: Vec<_> = handles.into_iter()
            .map(|h| h.join().expect("Thread panicked"))
            .collect();
        
        // All threads should have completed successfully
        assert_eq!(results, vec![0, 1, 2, 3, 4]);
    }
}