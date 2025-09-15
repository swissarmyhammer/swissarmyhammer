/// Common test utilities for SwissArmyHammer tests
///
/// This module provides shared testing infrastructure that can be used across
/// all SwissArmyHammer crates without creating circular dependencies. The utilities
/// focus on creating isolated test environments and managing test processes.
///
/// # Architecture
///
/// The test utilities provide:
/// - Isolated HOME directory management through `IsolatedTestHome`
/// - Process cleanup utilities through `ProcessGuard`
/// - Thread-safe environment variable manipulation
/// - Common temporary directory creation patterns
///
/// # Usage
///
/// ```no_run
/// use swissarmyhammer_common::test_utils::IsolatedTestHome;
///
/// #[test]
/// fn test_something() {
///     let _guard = IsolatedTestHome::new();
///     // HOME is now set to an isolated temporary directory
///     // with mock .swissarmyhammer structure
/// }
/// ```
use std::path::PathBuf;
use std::sync::Mutex;
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
/// use swissarmyhammer_common::test_utils::ProcessGuard;
///
/// let child = Command::new("some_program").spawn().unwrap();
/// let _guard = ProcessGuard::new(child);
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

/// Global mutex to serialize access to HOME environment variable manipulation
/// This prevents race conditions when multiple tests run in parallel
static HOME_ENV_LOCK: Mutex<()> = Mutex::new(());

/// Global mutex to serialize access to SWISSARMYHAMMER_SEMANTIC_DB_PATH environment variable manipulation
/// This prevents race conditions when multiple tests run in parallel
static SEMANTIC_DB_ENV_LOCK: Mutex<()> = Mutex::new(());

/// Acquire the global semantic database environment lock
/// This prevents race conditions when multiple tests run in parallel and manipulate SWISSARMYHAMMER_SEMANTIC_DB_PATH
pub fn acquire_semantic_db_lock() -> std::sync::MutexGuard<'static, ()> {
    SEMANTIC_DB_ENV_LOCK.lock().unwrap_or_else(|poisoned| {
        tracing::warn!("Semantic DB environment lock was poisoned, recovering");
        poisoned.into_inner()
    })
}

/// Create an isolated test home directory for parallel-safe testing
///
/// This creates a temporary directory with a mock SwissArmyHammer setup,
/// allowing tests to run in parallel without interfering with each other.
///
/// # Example
///
/// ```no_run
/// use swissarmyhammer_common::test_utils::create_isolated_test_home;
///
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

    // Create symlink to real cache directory for HuggingFace model caching
    let real_home = std::env::var("ORIGINAL_HOME")
        .or_else(|_| std::env::var("REAL_HOME"))
        .unwrap_or_else(|_| "/Users/wballard".to_string());
    let real_cache_dir = format!("{}/.cache", real_home);
    let fake_cache_dir = home_path.join(".cache");

    if std::path::Path::new(&real_cache_dir).exists() && !fake_cache_dir.exists() {
        if let Err(e) = std::os::unix::fs::symlink(&real_cache_dir, &fake_cache_dir) {
            // Don't fail the test if symlink creation fails, just warn
            eprintln!("Warning: Could not create cache symlink: {}", e);
        }
    }

    (temp_dir, home_path)
}

/// RAII guard for isolated HOME environment with race condition protection
///
/// This structure temporarily overrides the HOME environment variable to point
/// to an isolated test directory, then restores the original HOME when dropped.
/// Uses a global mutex to prevent race conditions when multiple tests run in parallel.
///
/// The guard holds a mutex lock for the entire duration of the test to ensure
/// that HOME manipulation is serialized across all tests in the test suite.
pub struct IsolatedTestHome {
    _temp_dir: TempDir,
    original_home: Option<String>,
    _lock_guard: std::sync::MutexGuard<'static, ()>,
}

impl Default for IsolatedTestHome {
    fn default() -> Self {
        Self::new()
    }
}

impl IsolatedTestHome {
    /// Create a new isolated test home environment
    pub fn new() -> Self {
        // Acquire the global HOME environment lock to prevent race conditions
        // If the lock is poisoned, we can still proceed since the guard data is not corrupted
        let lock_guard = HOME_ENV_LOCK.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("HOME environment lock was poisoned, recovering");
            poisoned.into_inner()
        });

        let original_home = std::env::var("HOME").ok();
        let (temp_dir, home_path) = create_isolated_test_home();

        // Set HOME to the temporary directory
        std::env::set_var("HOME", &home_path);

        Self {
            _temp_dir: temp_dir,
            original_home,
            _lock_guard: lock_guard,
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

/// RAII helper that isolates HOME directory for tests without changing the working directory
/// This prevents test pollution while allowing parallel test execution
pub struct IsolatedTestEnvironment {
    _home_guard: IsolatedTestHome,
    _temp_dir: TempDir,
}

impl IsolatedTestEnvironment {
    /// Creates a new isolated test environment with temporary HOME directory only.
    ///
    /// This creates:
    /// - A temporary home directory with mock .swissarmyhammer structure
    /// - A temporary directory that can be used as working directory if needed
    /// - Does NOT change the current working directory to allow parallel test execution
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
        let home_guard = IsolatedTestHome::new();
        let temp_dir = TempDir::new()?;

        // Ensure the temporary directory exists and is accessible
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



        // NOTE: We do NOT change the current working directory to allow parallel test execution

        Ok(Self {
            _home_guard: home_guard,
            _temp_dir: temp_dir,
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

    /// Get the path to the temporary working directory
    pub fn temp_dir(&self) -> PathBuf {
        self._temp_dir.path().to_path_buf()
    }

    /// Get the path to the issues directory in the isolated home
    pub fn issues_dir(&self) -> PathBuf {
        self.swissarmyhammer_dir().join("issues")
    }

    /// Get the path to the completed issues directory in the isolated home
    pub fn complete_dir(&self) -> PathBuf {
        self.issues_dir().join("complete")
    }
}

/// Create a temporary directory for testing
///
/// This is a convenience wrapper around tempfile::TempDir::new() that provides
/// better error handling and consistent behavior across tests.
pub fn create_temp_dir() -> TempDir {
    // Retry up to 3 times in case of temporary filesystem issues during parallel test execution
    for attempt in 1..=3 {
        match TempDir::new() {
            Ok(temp_dir) => return temp_dir,
            Err(_e) if attempt < 3 => {
                // Add small delay before retry to reduce contention
                std::thread::sleep(std::time::Duration::from_millis(10 * attempt as u64));
                continue;
            }
            Err(e) => panic!(
                "Failed to create temporary directory for test after {} attempts: {}",
                attempt, e
            ),
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isolated_home_basic_functionality() {
        // Simple test that verifies IsolatedTestHome basic functionality without
        // testing restoration behavior which is complex in concurrent environments

        let guard = IsolatedTestHome::new();
        let isolated_home = guard.home_path();

        // Verify the isolated home is accessible
        assert!(isolated_home.exists());
        assert!(isolated_home.is_dir());

        // Verify .swissarmyhammer directory was created
        let sah_dir = guard.swissarmyhammer_dir();
        assert!(sah_dir.exists());
        assert!(sah_dir.is_dir());

        // Verify expected subdirectories exist
        assert!(sah_dir.join("prompts").exists());
        assert!(sah_dir.join("workflows").exists());

        // Verify HOME is set to our temporary directory
        let current_home = std::env::var("HOME").expect("HOME should be set");
        assert_eq!(current_home, isolated_home.to_string_lossy());
    }

    #[test]
    #[serial_test::serial(home_env)]
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
    fn test_create_temp_dir() {
        let temp_dir = create_temp_dir();
        assert!(temp_dir.path().exists());
        assert!(temp_dir.path().is_dir());
    }
}
