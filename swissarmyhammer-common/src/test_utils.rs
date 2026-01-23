/// Common test utilities for SwissArmyHammer tests
///
/// This module provides shared testing infrastructure that can be used across
/// all SwissArmyHammer crates without creating circular dependencies. The utilities
/// focus on creating isolated test environments and managing test processes.
///
/// ```
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tempfile::TempDir;

/// Global mutex to serialize access to current directory manipulation
/// This prevents race conditions when multiple tests run in parallel
static CURRENT_DIR_LOCK: Mutex<()> = Mutex::new(());

/// RAII guard for temporarily changing the current directory
///
/// This structure changes to a new directory and automatically restores
/// the original directory when dropped, even on panic. Uses a global mutex
/// to serialize all current directory changes across tests.
///
/// # Example
///
/// ```
/// use swissarmyhammer_common::test_utils::CurrentDirGuard;
/// use tempfile::TempDir;
///
/// let temp = TempDir::new().unwrap();
/// {
///     let _guard = CurrentDirGuard::new(temp.path()).unwrap();
///     // Current directory is now temp.path()
///     // Original directory restored when _guard is dropped
/// }
/// ```
pub struct CurrentDirGuard {
    original_dir: PathBuf,
    _lock_guard: std::sync::MutexGuard<'static, ()>,
}

impl CurrentDirGuard {
    /// Create a new CurrentDirGuard that changes to the specified directory
    ///
    /// # Arguments
    /// * `new_dir` - The directory to change to
    ///
    /// # Returns
    /// * `Ok(CurrentDirGuard)` - Successfully changed directory
    /// * `Err` - If current directory cannot be saved or new directory cannot be entered
    ///
    /// # Errors
    /// Returns an error if:
    /// - The current directory cannot be determined
    /// - The target directory does not exist or cannot be entered
    pub fn new<P: AsRef<Path>>(new_dir: P) -> std::io::Result<Self> {
        let new_dir_path = new_dir.as_ref();

        // Verify the new directory exists BEFORE trying to acquire the mutex
        // This catches the case where a TempDir was created but deleted before we get here
        if !new_dir_path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "Target directory does not exist: {}",
                    new_dir_path.display()
                ),
            ));
        }

        let lock_guard = CURRENT_DIR_LOCK.lock().unwrap_or_else(|poisoned| {
            // When a test panics, the mutex is poisoned. We need to clear the poisoning.
            eprintln!("WARNING: Current directory lock was poisoned, recovering");
            poisoned.into_inner()
        });

        // After acquiring the mutex, check if the current directory is valid.
        // If not, reset to a known good directory (the cargo manifest dir).
        let original_dir = match std::env::current_dir() {
            Ok(dir) => dir,
            Err(e) => {
                // Current directory is invalid - try to recover
                let fallback_dir = std::env::var("CARGO_MANIFEST_DIR")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from("/"));
                eprintln!(
                    "WARNING: Current directory invalid ({}), resetting to: {}",
                    e,
                    fallback_dir.display()
                );
                std::env::set_current_dir(&fallback_dir)?;
                fallback_dir
            }
        };

        // Double-check the new directory still exists after acquiring the mutex
        if !new_dir_path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "Target directory was deleted while waiting for lock: {}",
                    new_dir_path.display()
                ),
            ));
        }

        std::env::set_current_dir(new_dir_path)?;

        Ok(Self {
            original_dir,
            _lock_guard: lock_guard,
        })
    }
}

impl Drop for CurrentDirGuard {
    fn drop(&mut self) {
        if let Err(e) = std::env::set_current_dir(&self.original_dir) {
            // Try to recover to a known good directory
            if let Ok(cargo_manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
                let _ = std::env::set_current_dir(&cargo_manifest_dir);
            }
            tracing::error!(
                "Failed to restore current directory to {}: {}",
                self.original_dir.display(),
                e
            );
        }
    }
}

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
        // Use try_wait in a loop instead of blocking wait
        let wait_start = Instant::now();
        while wait_start.elapsed() < std::time::Duration::from_secs(1) {
            match self.0.try_wait() {
                Ok(Some(_)) => return Ok(()),
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(10)),
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }

    /// Force kill the process immediately
    pub fn force_kill(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.0.kill()?;
        // Use try_wait in a loop instead of blocking wait
        use std::time::Instant;
        let start = Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(1) {
            match self.0.try_wait() {
                Ok(Some(_)) => return Ok(()),
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(10)),
                Err(e) => return Err(e.into()),
            }
        }
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
/// ```ignore
/// // This function is private - use IsolatedTestEnvironment instead
/// let (temp_dir, home_path) = create_isolated_test_home();
/// // Use home_path for testing instead of reading from HOME env var
/// // temp_dir is automatically cleaned up when dropped
/// ```
fn create_isolated_test_home() -> (TempDir, PathBuf) {
    let temp_dir = create_temp_dir();
    let home_path = temp_dir.path().to_path_buf();

    // Create mock SwissArmyHammer directory structure using centralized directory management
    let sah_dir = crate::directory::SwissarmyhammerDirectory::from_custom_root(home_path.clone())
        .expect("Failed to create .swissarmyhammer directory");
    sah_dir
        .ensure_subdir("prompts")
        .expect("Failed to create prompts directory");
    sah_dir
        .ensure_subdir("workflows")
        .expect("Failed to create workflows directory");
    sah_dir
        .ensure_subdir("todo")
        .expect("Failed to create todo directory");
    sah_dir
        .ensure_subdir("issues")
        .expect("Failed to create issues directory");
    sah_dir
        .ensure_subdir("issues/complete")
        .expect("Failed to create issues/complete directory");

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
///
/// Used from IsolatedTestEnvironment to provide a complete isolated test setup.
struct IsolatedTestHome {
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
        crate::directory::SwissarmyhammerDirectory::from_custom_root(self.home_path())
            .expect("Failed to get swissarmyhammer directory")
            .root()
            .to_path_buf()
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
        // Capture original HOME before any isolation happens
        let original_home = std::env::var("HOME").ok();
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

        // Create symlink to real cache directory for HuggingFace model caching
        if let Some(original_home_path) = &original_home {
            let real_cache_dir = format!("{}/.cache", original_home_path);
            let fake_cache_dir = home_guard.home_path().join(".cache");

            if std::path::Path::new(&real_cache_dir).exists() && !fake_cache_dir.exists() {
                if let Err(_e) = std::os::unix::fs::symlink(&real_cache_dir, &fake_cache_dir) {
                    // Ignore symlink creation failures
                }
            }
        }

        // NOTE: We do NOT change the current working directory to allow parallel test execution
        // Tests should pass temp_dir() to CliToolContext::new_with_dir() for proper isolation

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

impl Drop for IsolatedTestEnvironment {
    fn drop(&mut self) {
        // Cleanup is handled by the guards (_home_guard and _temp_dir)
        // which automatically restore state and delete temporary directories
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
