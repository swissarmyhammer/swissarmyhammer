/// Test utilities for SwissArmyHammer tests
///
/// This module provides shared testing infrastructure to ensure tests use a
/// preconfigured test home directory instead of the actual ~/.swissarmyhammer directory,
/// ensuring consistent test behavior in both local development and CI environments.
///
/// # Architecture
///
/// The test home system works by temporarily overriding the HOME environment variable
/// to point to a preconfigured test directory at `<workspace_root>/tests/test-home`.
/// This directory contains test fixtures including prompts and workflows that tests
/// can reliably depend on.
///
/// # Why HOME Override?
///
/// SwissArmyHammer reads user configuration from `~/.swissarmyhammer`. In tests, we need:
/// - Consistent, predictable content regardless of the developer's actual home directory
/// - Isolation from the developer's real SwissArmyHammer configuration
/// - The ability to test with specific prompt and workflow fixtures
///
/// # Usage Patterns
///
/// ## For Unit Tests
/// ```no_run
/// use swissarmyhammer::test_utils::create_test_home_guard;
///
/// #[test]
/// fn test_something() {
///     let _guard = create_test_home_guard();
///     // Test code here - HOME is now set to test directory
/// }
/// ```
///
/// ## For Integration Tests
/// Integration tests that create their own temporary directories (like MCP tests)
/// don't need to use this module. They can set HOME directly to their temp directory.
///
/// # Thread Safety
///
/// The module uses a global mutex to ensure thread-safe modification of the HOME
/// environment variable. This means tests using TestHomeGuard will serialize access
/// to HOME, which may impact parallel test execution performance.
use crate::{Prompt, PromptLibrary};
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;

// #[cfg(test)]
// use swissarmyhammer_tools::mcp::{ToolHandlers, ToolContext};

use tempfile::TempDir;

/// Helper struct to ensure process cleanup in tests
///
/// This guard automatically kills and waits for a child process when dropped,
/// ensuring test processes don't leak even if a test fails or panics.
///
/// # Example
///
/// ```no_run
/// use std::process::Command;
/// use swissarmyhammer::test_utils::ProcessGuard;
///
/// let child = Command::new("some_program").spawn().unwrap();
/// let _guard = ProcessGuard(child);
/// // Process will be killed when _guard goes out of scope
/// ```
pub struct ProcessGuard(pub std::process::Child);

impl ProcessGuard {
    /// Create a new ProcessGuard from a child process
    pub fn new(child: std::process::Child) -> Self {
        Self(child)
    }

    /// Check if the process is still running
    pub fn is_running(&mut self) -> bool {
        match self.0.try_wait() {
            Ok(None) => true,     // Process is still running
            Ok(Some(_)) => false, // Process has exited
            Err(_) => false,      // Error occurred, assume process is dead
        }
    }

    /// Attempt to gracefully terminate the process with a timeout
    pub fn terminate_gracefully(
        &mut self,
        timeout: std::time::Duration,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::time::Instant;

        // For now, we'll use a simple approach - just wait a bit then force kill
        // This could be enhanced later with proper signal handling if needed
        let start = Instant::now();
        while start.elapsed() < timeout {
            match self.0.try_wait() {
                Ok(Some(_)) => return Ok(()), // Process exited
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(10)),
                Err(e) => return Err(e.into()),
            }
        }

        // If the process didn't exit gracefully, force kill it
        self.0.kill()?;
        self.0.wait()?;
        Ok(())
    }

    /// Force kill the process immediately
    pub fn force_kill(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.0.kill()?;
        self.0.wait()?;
        Ok(())
    }
}

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        // First try graceful termination with a short timeout
        let _ = self.terminate_gracefully(std::time::Duration::from_millis(500));

        // If graceful termination failed, try force kill as fallback
        if self.is_running() {
            let _ = self.force_kill();
        }
    }
}

/// Global test home path, initialized once (used for legacy compatibility)
static TEST_HOME_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Initialize and get the test home directory path
///
/// This calculates the correct path to the test-home directory regardless of
/// whether the test is run from the workspace root or from within a crate.
/// Note: This only calculates the path - validation is done later to avoid deadlocks.
fn get_test_home_path() -> &'static PathBuf {
    TEST_HOME_PATH.get_or_init(|| {
        // Try to find the workspace root by looking for the test-home directory
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        // Check if we're already at the workspace root (has tests/test-home)
        let direct_path = manifest_dir.join("tests").join("test-home");
        if direct_path.exists() {
            return direct_path;
        }

        // Otherwise, we're in a subcrate, go up one level
        let workspace_root = manifest_dir
            .parent()
            .expect("Failed to find parent directory")
            .to_path_buf();
        workspace_root.join("tests").join("test-home")
    })
}

/// Set up the test environment by configuring HOME to use the test directory
///
/// **DEPRECATED**: This function serializes tests. Use `IsolatedTestHome::new()` instead.
///
/// This function should be called at the beginning of each test that needs
/// to access the ~/.swissarmyhammer directory.
///
/// # Example
///
/// ```no_run
/// # use swissarmyhammer::test_utils::setup_test_home;
/// #[test]
/// fn test_something() {
///     setup_test_home();
///     // Your test code here
/// }
/// ```
pub fn setup_test_home() {
    let test_home = get_test_home_path();
    std::env::set_var("HOME", test_home);
}

/// Get the path to the test home directory
///
/// This is useful when you need to access files within the test home directory.
pub fn get_test_home() -> PathBuf {
    get_test_home_path().clone()
}

/// Get the path to the test .swissarmyhammer directory
pub fn get_test_swissarmyhammer_dir() -> PathBuf {
    get_test_home().join(".swissarmyhammer")
}

/// Guard that sets up test environment and restores the original HOME on drop
///
/// **DEPRECATED**: This approach serializes all tests. Use `create_isolated_test_home()` instead.
///
/// This is useful for tests that need to ensure the HOME environment variable
/// is restored after the test completes, preventing test pollution.
pub struct TestHomeGuard {
    original_home: Option<String>,
}

impl TestHomeGuard {
    /// Create a new test home guard (DEPRECATED)
    ///
    /// **Warning**: This method serializes all tests and should not be used.
    /// Use `IsolatedTestHome::new()` instead for parallel-safe testing.
    pub fn new() -> Self {
        let original_home = std::env::var("HOME").ok();
        let test_home = get_test_home_path();
        std::env::set_var("HOME", test_home);
        Self { original_home }
    }
}

impl Default for TestHomeGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TestHomeGuard {
    fn drop(&mut self) {
        // Restore original HOME environment variable
        match &self.original_home {
            Some(home) => std::env::set_var("HOME", home),
            None => std::env::remove_var("HOME"),
        }
    }
}

/// Create a test home guard for scoped test environment setup
///
/// **DEPRECATED**: Use `create_isolated_test_home()` instead for parallel-safe testing.
///
/// # Example
///
/// ```no_run
/// # use swissarmyhammer::test_utils::create_test_home_guard;
/// #[test]
/// fn test_with_guard() {
///     let _guard = create_test_home_guard();
///     // Your test code here
///     // HOME will be automatically restored when _guard goes out of scope
/// }
/// ```
pub fn create_test_home_guard() -> TestHomeGuard {
    TestHomeGuard::new()
}

/// Create an isolated test home directory for parallel-safe testing
///
/// This creates a temporary directory with a mock SwissArmyHammer setup,
/// allowing tests to run in parallel without interfering with each other.
///
/// # Example
///
/// ```no_run
/// # use swissarmyhammer::test_utils::create_isolated_test_home;
/// #[test]
/// fn test_with_isolation() {
///     let (temp_dir, home_path) = create_isolated_test_home();
///     // Use home_path for testing instead of reading from HOME env var
///     // temp_dir is automatically cleaned up when dropped
/// }
/// ```
pub fn create_isolated_test_home() -> (TempDir, PathBuf) {
    let temp_dir = create_temp_dir();
    let home_path = temp_dir.path().to_path_buf();

    // Create mock SwissArmyHammer directory structure
    let sah_dir = home_path.join(".swissarmyhammer");
    std::fs::create_dir_all(&sah_dir).expect("Failed to create .swissarmyhammer directory");
    std::fs::create_dir_all(sah_dir.join("prompts")).expect("Failed to create prompts directory");
    std::fs::create_dir_all(sah_dir.join("workflows"))
        .expect("Failed to create workflows directory");
    std::fs::create_dir_all(sah_dir.join("todo")).expect("Failed to create todo directory");
    std::fs::create_dir_all(sah_dir.join("issues")).expect("Failed to create issues directory");
    std::fs::create_dir_all(sah_dir.join("issues/complete"))
        .expect("Failed to create issues/complete directory");

    (temp_dir, home_path)
}

/// RAII guard for isolated test environments
///
/// This creates a temporary directory with mock SwissArmyHammer structure,
/// sets HOME to point to it, and restores the original HOME on drop.
/// Unlike TestHomeGuard, this allows parallel test execution.
///
/// # Example
///
/// ```no_run
/// # use swissarmyhammer::test_utils::IsolatedTestHome;
/// #[test]
/// fn test_with_isolated_home() {
///     let _guard = IsolatedTestHome::new();
///     // HOME now points to an isolated temporary directory
///     // with mock .swissarmyhammer structure
///     // Original HOME is restored when _guard is dropped
/// }
/// ```
pub struct IsolatedTestHome {
    _temp_dir: TempDir,
    original_home: Option<String>,
}

impl Default for IsolatedTestHome {
    fn default() -> Self {
        Self::new()
    }
}

impl IsolatedTestHome {
    /// Create a new isolated test home environment
    pub fn new() -> Self {
        let original_home = std::env::var("HOME").ok();
        let (temp_dir, home_path) = create_isolated_test_home();

        // Set HOME to the temporary directory
        std::env::set_var("HOME", &home_path);

        Self {
            _temp_dir: temp_dir,
            original_home,
        }
    }

    /// Get the path to the isolated home directory
    pub fn home_path(&self) -> PathBuf {
        self._temp_dir.path().to_path_buf()
    }

    /// Get the path to the .swissarmyhammer directory in the isolated home
    pub fn swissarmyhammer_dir(&self) -> PathBuf {
        self.home_path().join(".swissarmyhammer")
    }
}

impl Drop for IsolatedTestHome {
    fn drop(&mut self) {
        // Restore original HOME environment variable
        match &self.original_home {
            Some(home) => std::env::set_var("HOME", home),
            None => std::env::remove_var("HOME"),
        }
    }
}

/// RAII helper that isolates both HOME directory and current working directory for tests
/// This prevents abort file pollution between tests by ensuring each test runs in its own environment
#[cfg(any(test, feature = "test-utils"))]
pub struct IsolatedTestEnvironment {
    _home_guard: IsolatedTestHome,
    _temp_dir: TempDir,
    original_cwd: PathBuf,
}

#[cfg(any(test, feature = "test-utils"))]
impl IsolatedTestEnvironment {
    /// Creates a new isolated test environment with temporary HOME and current working directory.
    ///
    /// This creates:
    /// - A temporary home directory with mock .swissarmyhammer structure
    /// - A temporary current working directory
    /// - Automatic restoration of original directories on drop
    pub fn new() -> std::io::Result<Self> {
        // Retry up to 3 times in case of temporary filesystem issues during parallel test execution
        for attempt in 1..=3 {
            match Self::try_create() {
                Ok(env) => return Ok(env),
                Err(_e) if attempt < 3 => {
                    // Add small delay before retry to reduce contention
                    std::thread::sleep(std::time::Duration::from_millis(10 * attempt as u64));
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        unreachable!()
    }

    /// Try to create an isolated test environment (single attempt)
    fn try_create() -> std::io::Result<Self> {
        let original_cwd = std::env::current_dir()?;
        let home_guard = IsolatedTestHome::new();
        let temp_dir = TempDir::new()?;

        // Ensure the temporary directory exists and is accessible before changing to it
        let temp_path = temp_dir.path();
        if !temp_path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Temporary directory does not exist: {:?}", temp_path),
            ));
        }

        // Verify we can access the directory
        match std::fs::read_dir(temp_path) {
            Ok(_) => {}
            Err(e) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("Cannot access temporary directory {:?}: {}", temp_path, e),
                ));
            }
        }

        std::env::set_current_dir(temp_path)?;

        Ok(Self {
            _home_guard: home_guard,
            _temp_dir: temp_dir,
            original_cwd,
        })
    }

    /// Get the path to the isolated home directory
    pub fn home_path(&self) -> PathBuf {
        self._home_guard.home_path()
    }

    /// Get the path to the .swissarmyhammer directory in the isolated home
    pub fn swissarmyhammer_dir(&self) -> PathBuf {
        self._home_guard.swissarmyhammer_dir()
    }

    /// Get the path to the issues directory in the isolated home
    pub fn issues_dir(&self) -> PathBuf {
        self.swissarmyhammer_dir().join("issues")
    }

    /// Get the path to the completed issues directory in the isolated home
    pub fn complete_dir(&self) -> PathBuf {
        self.issues_dir().join("complete")
    }
    
    /// Get the path to the temporary working directory
    pub fn temp_dir(&self) -> &std::path::Path {
        self._temp_dir.path()
    }
}

#[cfg(any(test, feature = "test-utils"))]
impl Drop for IsolatedTestEnvironment {
    fn drop(&mut self) {
        // Restore original working directory
        let _ = std::env::set_current_dir(&self.original_cwd);
    }
}

/// Create a temporary directory for testing
///
/// This is a convenience wrapper around tempfile::TempDir::new() that provides
/// better error handling and consistent behavior across tests.
pub fn create_temp_dir() -> TempDir {
    TempDir::new().expect("Failed to create temporary directory for test")
}

/// Create a set of standard test prompts for testing
///
/// Returns a collection of diverse test prompts that can be used across
/// different test scenarios without duplicating prompt creation logic.
pub fn create_test_prompts() -> Vec<Prompt> {
    vec![
        Prompt::new("code-review", "Review this code: {{ code }}")
            .with_description("A prompt for reviewing code")
            .with_category("development")
            .with_tags(vec!["code".to_string(), "review".to_string()]),
        Prompt::new("bug-fix", "Fix this bug: {{ error }}")
            .with_description("A prompt for fixing bugs")
            .with_category("debugging")
            .with_tags(vec!["bug".to_string(), "fix".to_string()]),
        Prompt::new("test-generation", "Generate tests for: {{ function }}")
            .with_description("Generate unit tests")
            .with_category("testing")
            .with_tags(vec!["test".to_string(), "unit".to_string()]),
        Prompt::new("documentation", "Document this function: {{ code }}")
            .with_description("Generate documentation")
            .with_category("docs")
            .with_tags(vec!["docs".to_string(), "documentation".to_string()]),
        Prompt::new("refactor", "Refactor this code: {{ code }}")
            .with_description("Suggest refactoring improvements")
            .with_category("development")
            .with_tags(vec!["refactor".to_string(), "improvement".to_string()]),
    ]
}

/// Create a simple test prompt with minimal setup
///
/// Useful for tests that need a single prompt without all the metadata.
pub fn create_simple_test_prompt(name: &str, template: &str) -> Prompt {
    Prompt::new(name, template).with_description(format!("Test prompt: {name}"))
}

/// Create a test prompt library with standard test prompts
///
/// Returns a PromptLibrary pre-populated with test prompts for consistent
/// testing across different components.
pub fn create_test_prompt_library() -> PromptLibrary {
    let mut library = PromptLibrary::new();
    for prompt in create_test_prompts() {
        let _ = library.add(prompt);
    }
    library
}

/// Create a temporary directory with test prompt files
///
/// Creates a temporary directory and populates it with YAML files containing
/// the standard test prompts. Returns both the TempDir and the path.
#[cfg(test)]
pub fn create_temp_prompt_dir() -> (TempDir, PathBuf) {
    let temp_dir = create_temp_dir();
    let temp_path = temp_dir.path().to_path_buf();

    // Create prompt files
    for prompt in create_test_prompts() {
        let file_path = temp_path.join(format!("{}.yaml", prompt.name));
        let content = serde_yaml::to_string(&prompt).expect("Failed to serialize test prompt");
        std::fs::write(&file_path, content).expect("Failed to write test prompt file");
    }

    (temp_dir, temp_path)
}

/// Test file system utility for creating mock file structures
///
/// Provides a convenient way to set up temporary file structures
/// for testing file-based operations.
#[cfg(test)]
pub struct TestFileSystem {
    temp_dir: TempDir,
}

#[cfg(test)]
impl Default for TestFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl TestFileSystem {
    /// Create a new test file system
    pub fn new() -> Self {
        Self {
            temp_dir: create_temp_dir(),
        }
    }

    /// Get the root path of the test file system
    pub fn root(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Create a file with the given relative path and content
    pub fn create_file<P: AsRef<Path>>(&self, path: P, content: &str) -> PathBuf {
        let full_path = self.temp_dir.path().join(path);

        // Ensure parent directory exists
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).expect("Failed to create parent directory");
        }

        std::fs::write(&full_path, content).expect("Failed to write test file");

        full_path
    }

    /// Create a directory with the given relative path
    pub fn create_dir<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        let full_path = self.temp_dir.path().join(path);
        std::fs::create_dir_all(&full_path).expect("Failed to create test directory");
        full_path
    }

    /// Create a YAML file with the given object
    pub fn create_yaml_file<P: AsRef<Path>, T: serde::Serialize>(
        &self,
        path: P,
        data: &T,
    ) -> PathBuf {
        let content = serde_yaml::to_string(data).expect("Failed to serialize to YAML");
        self.create_file(path, &content)
    }

    /// Create a JSON file with the given object
    pub fn create_json_file<P: AsRef<Path>, T: serde::Serialize>(
        &self,
        path: P,
        data: &T,
    ) -> PathBuf {
        let content = serde_json::to_string_pretty(data).expect("Failed to serialize to JSON");
        self.create_file(path, &content)
    }
}

// /// Create a shared test context for MCP memoranda tool tests
// ///
// /// This function creates a consistent ToolContext with mock storage backends
// /// that can be used across all memoranda tool tests, reducing code duplication
// /// and ensuring consistent test behavior.
// ///
// /// # Returns
// ///
// /// A `ToolContext` configured with:
// /// - Mock issue storage using a test directory
// /// - No git operations (None)
// /// - Mock memo storage for testing
// /// - Tool handlers with all backends connected
// #[cfg(test)]
// pub async fn create_test_context() -> ToolContext {
//     let issue_storage: Arc<RwLock<Box<dyn IssueStorage>>> = Arc::new(RwLock::new(Box::new(
//         crate::issues::FileSystemIssueStorage::new(PathBuf::from("./test_issues")).unwrap(),
//     )));
//     let git_ops: Arc<TokioMutex<Option<GitOperations>>> = Arc::new(TokioMutex::new(None));
//     let memo_storage: Arc<RwLock<Box<dyn MemoStorage>>> =
//         Arc::new(RwLock::new(Box::new(MockMemoStorage::new())));
//
//     let tool_handlers = Arc::new(ToolHandlers::new(memo_storage.clone()));
//
//     ToolContext::new(
//         tool_handlers,
//         issue_storage,
//         git_ops,
//         memo_storage,
//         Arc::new(MockRateLimiter),
//     )
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[serial_test::serial]
    fn test_setup_test_home() {
        let _guard = IsolatedTestHome::new();

        let home = std::env::var("HOME").expect("HOME not set");
        // HOME should now point to our isolated temp directory
        assert!(PathBuf::from(&home).exists());

        // Verify the mock SwissArmyHammer structure exists
        let swissarmyhammer_dir = PathBuf::from(&home).join(".swissarmyhammer");
        assert!(swissarmyhammer_dir.exists());
        assert!(swissarmyhammer_dir.join("prompts").exists());
        assert!(swissarmyhammer_dir.join("workflows").exists());
    }

    #[test]
    fn test_guard_restores_home() {
        let original_home = std::env::var("HOME").ok();

        {
            let _guard = IsolatedTestHome::new();
            let test_home = std::env::var("HOME").expect("HOME not set");
            // Should be pointing to temporary directory, not original home
            assert_ne!(test_home, original_home.clone().unwrap_or_default());
            assert!(PathBuf::from(&test_home).exists());
        }

        // Check that HOME is restored after guard is dropped
        let restored_home = std::env::var("HOME").ok();
        assert_eq!(original_home, restored_home);
    }

    #[test]
    fn test_concurrent_access() {
        use std::thread;

        let handles: Vec<_> = (0..5)
            .map(|_| {
                thread::spawn(|| {
                    let _guard = IsolatedTestHome::new();
                    let home = std::env::var("HOME").expect("HOME not set");
                    // Each isolated test home creates its own temporary directory
                    // The path should be unique per thread, so we just verify it's set
                    assert!(!home.is_empty());
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("Thread panicked");
        }
    }

    #[test]
    fn test_create_test_prompts() {
        let prompts = create_test_prompts();
        assert_eq!(prompts.len(), 5);

        // Verify each prompt has expected properties
        let code_review = &prompts[0];
        assert_eq!(code_review.name, "code-review");
        assert!(code_review
            .description
            .as_ref()
            .unwrap()
            .contains("reviewing code"));
        assert_eq!(code_review.category.as_ref().unwrap(), "development");
        assert!(code_review.tags.contains(&"code".to_string()));
    }

    #[test]
    fn test_create_simple_test_prompt() {
        let prompt = create_simple_test_prompt("test-name", "Test template: {{ var }}");
        assert_eq!(prompt.name, "test-name");
        assert_eq!(prompt.template, "Test template: {{ var }}");
        assert!(prompt
            .description
            .as_ref()
            .unwrap()
            .contains("Test prompt: test-name"));
    }

    #[test]
    fn test_create_test_prompt_library() {
        let library = create_test_prompt_library();

        // Verify the library contains all test prompts
        assert!(library.get("code-review").is_ok());
        assert!(library.get("bug-fix").is_ok());
        assert!(library.get("test-generation").is_ok());
        assert!(library.get("documentation").is_ok());
        assert!(library.get("refactor").is_ok());

        // Verify non-existent prompt returns error
        assert!(library.get("non-existent").is_err());
    }

    #[test]
    fn test_create_temp_prompt_dir() {
        let (_temp_dir, temp_path) = create_temp_prompt_dir();

        // Verify directory exists
        assert!(temp_path.exists());
        assert!(temp_path.is_dir());

        // Verify prompt files were created
        let code_review_file = temp_path.join("code-review.yaml");
        assert!(code_review_file.exists());

        // Verify file content is valid YAML
        let content = std::fs::read_to_string(&code_review_file).unwrap();
        let prompt: Prompt = serde_yaml::from_str(&content).unwrap();
        assert_eq!(prompt.name, "code-review");
    }

    #[test]
    fn test_test_file_system() {
        let fs = TestFileSystem::new();

        // Test creating a file
        let file_path = fs.create_file("test.txt", "Hello, world!");
        assert!(file_path.exists());
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Hello, world!");

        // Test creating a directory
        let dir_path = fs.create_dir("subdir");
        assert!(dir_path.exists());
        assert!(dir_path.is_dir());

        // Test creating a file in a subdirectory
        let nested_file = fs.create_file("subdir/nested.txt", "Nested content");
        assert!(nested_file.exists());

        // Test creating YAML file
        let test_data = serde_json::json!({
            "name": "test",
            "value": 42
        });
        let yaml_file = fs.create_yaml_file("data.yaml", &test_data);
        assert!(yaml_file.exists());

        // Verify YAML content
        let yaml_content = std::fs::read_to_string(&yaml_file).unwrap();
        let parsed: serde_json::Value = serde_yaml::from_str(&yaml_content).unwrap();
        assert_eq!(parsed["name"], "test");
        assert_eq!(parsed["value"], 42);
    }
}
