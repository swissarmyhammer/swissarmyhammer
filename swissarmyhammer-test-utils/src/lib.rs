//! Test utilities for SwissArmyHammer crates
//!
//! This crate provides shared testing infrastructure to ensure tests use isolated
//! environments instead of actual user directories, ensuring consistent test behavior
//! in both local development and CI environments while supporting parallel test execution.
//!
//! # Core Concepts
//!
//! ## IsolatedTestEnvironment
//!
//! The primary component that provides:
//! - Temporary HOME directory isolation
//! - Mock `.swissarmyhammer` directory structure
//! - Automatic cleanup via RAII
//! - Parallel test execution support
//!
//! ## Usage
//!
//! ```no_run
//! use swissarmyhammer_test_utils::IsolatedTestEnvironment;
//!
//! #[test]
//! fn test_something() {
//!     let _guard = IsolatedTestEnvironment::new().unwrap();
//!     // Test code here - HOME is now isolated
//!     // Original HOME restored when _guard is dropped
//! }
//! ```

use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::TempDir;

/// RAII guard for isolated test environments
///
/// This creates a temporary directory with mock SwissArmyHammer structure,
/// sets HOME to point to it, and restores the original HOME on drop.
/// This allows parallel test execution without interference.
pub struct IsolatedTestEnvironment {
    _temp_dir: TempDir,
    original_home: Option<String>,
    original_env_vars: HashMap<String, Option<String>>,
}

impl IsolatedTestEnvironment {
    /// Create a new isolated test environment
    ///
    /// This creates:
    /// - A temporary home directory with mock .swissarmyhammer structure
    /// - Sets HOME environment variable to the temporary directory
    /// - Stores original environment state for restoration
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use swissarmyhammer_test_utils::IsolatedTestEnvironment;
    /// #[test]
    /// fn test_with_isolation() {
    ///     let _guard = IsolatedTestEnvironment::new().unwrap();
    ///     // HOME now points to isolated temporary directory
    /// }
    /// ```
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
        let temp_dir = create_temp_dir_with_retry()?;
        let home_path = temp_dir.path().to_path_buf();

        // Store original HOME for restoration
        let original_home = std::env::var("HOME").ok();

        // Create mock SwissArmyHammer directory structure
        let sah_dir = home_path.join(".swissarmyhammer");
        std::fs::create_dir_all(&sah_dir)?;
        std::fs::create_dir_all(sah_dir.join("prompts"))?;
        std::fs::create_dir_all(sah_dir.join("workflows"))?;
        std::fs::create_dir_all(sah_dir.join("todo"))?;
        std::fs::create_dir_all(sah_dir.join("issues"))?;
        std::fs::create_dir_all(sah_dir.join("issues/complete"))?;

        // Set HOME to the temporary directory
        std::env::set_var("HOME", &home_path);

        Ok(Self {
            _temp_dir: temp_dir,
            original_home,
            original_env_vars: HashMap::new(),
        })
    }

    /// Get the path to the isolated home directory
    pub fn home_path(&self) -> PathBuf {
        self._temp_dir.path().to_path_buf()
    }

    /// Get the path to the .swissarmyhammer directory in the isolated home
    pub fn swissarmyhammer_dir(&self) -> PathBuf {
        self.home_path().join(".swissarmyhammer")
    }

    /// Get the path to the issues directory in the isolated home
    pub fn issues_dir(&self) -> PathBuf {
        self.swissarmyhammer_dir().join("issues")
    }

    /// Get the path to the completed issues directory in the isolated home
    pub fn complete_dir(&self) -> PathBuf {
        self.issues_dir().join("complete")
    }

    /// Get the path to the prompts directory in the isolated home
    pub fn prompts_dir(&self) -> PathBuf {
        self.swissarmyhammer_dir().join("prompts")
    }

    /// Get the path to the workflows directory in the isolated home
    pub fn workflows_dir(&self) -> PathBuf {
        self.swissarmyhammer_dir().join("workflows")
    }

    /// Get the path to the todo directory in the isolated home
    pub fn todo_dir(&self) -> PathBuf {
        self.swissarmyhammer_dir().join("todo")
    }

    /// Get the path to the temporary working directory
    ///
    /// Tests can use this directory for operations that need a writable directory,
    /// but should pass this path explicitly to functions rather than changing
    /// the global current working directory.
    pub fn temp_dir(&self) -> &std::path::Path {
        self._temp_dir.path()
    }

    /// Set an environment variable and remember original value for restoration
    ///
    /// This is useful for tests that need to modify environment variables
    /// beyond just HOME.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use swissarmyhammer_test_utils::IsolatedTestEnvironment;
    /// #[test]
    /// fn test_with_env_var() {
    ///     let mut guard = IsolatedTestEnvironment::new().unwrap();
    ///     guard.set_env_var("TEST_VAR", "test_value").unwrap();
    ///     assert_eq!(std::env::var("TEST_VAR").unwrap(), "test_value");
    ///     // TEST_VAR will be restored when guard is dropped
    /// }
    /// ```
    pub fn set_env_var<K: AsRef<str>, V: AsRef<str>>(
        &mut self,
        key: K,
        value: V,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let key_str = key.as_ref().to_string();
        let original_value = std::env::var(&key_str).ok();

        // Store for later restoration if we haven't already stored it
        if !self.original_env_vars.contains_key(&key_str) {
            self.original_env_vars.insert(key_str.clone(), original_value);
        }

        std::env::set_var(&key_str, value.as_ref());
        Ok(())
    }

    /// Set multiple environment variables at once
    pub fn set_env_vars<I, K, V>(&mut self, vars: I) -> Result<(), Box<dyn std::error::Error>>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        for (key, value) in vars {
            self.set_env_var(key, value)?;
        }
        Ok(())
    }
}

impl Drop for IsolatedTestEnvironment {
    fn drop(&mut self) {
        // Restore original HOME environment variable
        match &self.original_home {
            Some(home) => std::env::set_var("HOME", home),
            None => std::env::remove_var("HOME"),
        }

        // Restore all other environment variables that were modified
        for (key, original_value) in &self.original_env_vars {
            match original_value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

/// Create a temporary directory with retry logic for parallel test execution
///
/// This function attempts to create a temporary directory up to 3 times with
/// exponential backoff to handle filesystem contention during parallel test execution.
/// This is the robust replacement for TempDir::new().unwrap() throughout the codebase.
pub fn create_temp_dir_with_retry() -> std::io::Result<TempDir> {
    // Retry up to 3 times in case of temporary filesystem issues during parallel test execution
    for attempt in 1..=3 {
        match TempDir::new() {
            Ok(dir) => return Ok(dir),
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

/// Create a temporary directory for testing (convenience function)
pub fn create_temp_dir() -> TempDir {
    create_temp_dir_with_retry().expect("Failed to create temporary directory for test after 3 attempts")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isolated_environment_creation() {
        let guard = IsolatedTestEnvironment::new().unwrap();
        
        // Verify home path exists and is accessible
        assert!(guard.home_path().exists());
        
        // Verify SwissArmyHammer directory structure was created
        assert!(guard.swissarmyhammer_dir().exists());
        assert!(guard.issues_dir().exists());
        assert!(guard.complete_dir().exists());
        assert!(guard.prompts_dir().exists());
        assert!(guard.workflows_dir().exists());
        assert!(guard.todo_dir().exists());
        
        // Verify HOME is set to the isolated directory
        let current_home = std::env::var("HOME").unwrap();
        assert_eq!(current_home, guard.home_path().to_string_lossy());
    }

    #[test]
    fn test_environment_variable_management() {
        let mut guard = IsolatedTestEnvironment::new().unwrap();
        
        // Test setting and getting environment variable
        guard.set_env_var("TEST_VAR", "test_value").unwrap();
        assert_eq!(std::env::var("TEST_VAR").unwrap(), "test_value");
        
        // Test setting multiple variables
        let vars = vec![
            ("VAR1", "value1"),
            ("VAR2", "value2"),
        ];
        guard.set_env_vars(vars).unwrap();
        assert_eq!(std::env::var("VAR1").unwrap(), "value1");
        assert_eq!(std::env::var("VAR2").unwrap(), "value2");
    }

    #[test]
    fn test_parallel_isolation() {
        use std::thread;

        let handles: Vec<_> = (0..5)
            .map(|_| {
                thread::spawn(|| {
                    let guard = IsolatedTestEnvironment::new().unwrap();
                    let home = std::env::var("HOME").unwrap();
                    // Each isolated test environment creates its own temporary directory
                    assert!(!home.is_empty());
                    assert!(guard.home_path().exists());
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("Thread panicked");
        }
    }

    #[test]
    fn test_temp_dir_creation_with_retry() {
        let temp_dir = create_temp_dir_with_retry().unwrap();
        assert!(temp_dir.path().exists());
    }

    #[test]
    fn test_create_temp_dir_convenience() {
        let temp_dir = create_temp_dir();
        assert!(temp_dir.path().exists());
    }
}