//! Test utilities for SwissArmyHammer CLI tests
//!
//! This module extends the test utilities from the main crate with CLI-specific helpers.

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
/// This is a convenience wrapper that provides consistent error handling
#[allow(dead_code)]
pub fn create_temp_dir() -> Result<TempDir> {
    Ok(TempDir::new()?)
}

/// Create test prompt files in a directory
///
/// This creates actual prompt files on disk for integration testing.
/// Different from the main crate's create_test_prompts which creates Prompt objects.
#[allow(dead_code)]
pub fn create_test_prompt_files(prompts_dir: &Path) -> Result<()> {
    let test_prompts = vec![
        ("simple", "Hello, world!", vec![]),
        (
            "with_args",
            "Hello {{name}}, you are {{age}} years old",
            vec![("name", "User's name", true), ("age", "User's age", true)],
        ),
        (
            "code_review",
            "Review this code: {{ code }}",
            vec![("code", "Code to review", true)],
        ),
        (
            "bug_fix",
            "Fix this bug: {{ error }}",
            vec![("error", "Error message", true)],
        ),
        (
            "test_generation",
            "Generate tests for: {{ function }}",
            vec![("function", "Function to test", true)],
        ),
    ];

    for (name, template, args) in test_prompts {
        let prompt_file = prompts_dir.join(format!("{name}.prompt"));
        let mut yaml_content = String::from("---\n");
        yaml_content.push_str(&format!("name: {name}\n"));
        yaml_content.push_str(&format!("description: Test prompt for {name}\n"));

        if !args.is_empty() {
            yaml_content.push_str("parameters:\n");
            for (arg_name, desc, required) in args {
                yaml_content.push_str(&format!("  - name: {arg_name}\n"));
                yaml_content.push_str(&format!("    description: {desc}\n"));
                yaml_content.push_str(&format!("    required: {required}\n"));
            }
        }

        yaml_content.push_str("---\n");
        yaml_content.push_str(template);

        std::fs::write(&prompt_file, yaml_content)?;
    }

    Ok(())
}

/// Create a temporary test environment with prompts
///
/// Returns a TempDir and the path to the prompts directory
#[allow(dead_code)]
pub fn create_test_environment() -> Result<(TempDir, PathBuf)> {
    let temp_dir = create_temp_dir()?;
    let swissarmyhammer_dir = temp_dir.path().join(".swissarmyhammer");
    let prompts_dir = swissarmyhammer_dir.join("prompts");

    std::fs::create_dir_all(&prompts_dir)?;
    create_test_prompt_files(&prompts_dir)?;

    Ok((temp_dir, prompts_dir))
}

/// Setup environment for MCP tests
///
/// Sets HOME to a temporary directory and creates the necessary structure
#[allow(dead_code)]
pub fn setup_mcp_test_env() -> Result<(TempDir, PathBuf)> {
    let temp_dir = create_temp_dir()?;
    std::env::set_var("HOME", temp_dir.path());

    let swissarmyhammer_dir = temp_dir.path().join(".swissarmyhammer");
    let prompts_dir = swissarmyhammer_dir.join("prompts");

    std::fs::create_dir_all(&prompts_dir)?;
    create_test_prompt_files(&prompts_dir)?;

    Ok((temp_dir, prompts_dir))
}

/// Guard that manages test environment variables for semantic search tests
///
/// This sets up a controlled API key environment for testing semantic search
/// functionality without requiring real API credentials.
pub struct SemanticTestGuard {
    _home_guard: TestHomeGuard,
    _temp_dir: TempDir,
    original_api_key: Option<String>,
    original_db_path: Option<String>,
    test_db_path: std::path::PathBuf,
}

impl SemanticTestGuard {
    /// Create a new semantic test guard with isolated environment
    pub fn new() -> Self {
        let home_guard = create_test_home_guard();
        let temp_dir =
            TempDir::new().expect("Failed to create temporary directory for semantic test");

        let original_api_key = std::env::var("NOMIC_API_KEY").ok();
        let original_db_path = std::env::var("SWISSARMYHAMMER_SEMANTIC_DB_PATH").ok();

        // Create a unique database path for this test instance
        let test_db_path = tempfile::NamedTempFile::new()
            .expect("Failed to create temp database file")
            .path()
            .with_extension("db")
            .to_path_buf();

        // Set a test API key that allows the command to start but will fail gracefully
        std::env::set_var("NOMIC_API_KEY", "test-key-for-cli-integration-testing");

        // Set isolated database path for this test
        std::env::set_var("SWISSARMYHAMMER_SEMANTIC_DB_PATH", &test_db_path);

        Self {
            _home_guard: home_guard,
            _temp_dir: temp_dir,
            original_api_key,
            original_db_path,
            test_db_path,
        }
    }
}

impl Default for SemanticTestGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SemanticTestGuard {
    fn drop(&mut self) {
        // Restore original API key environment variable
        match &self.original_api_key {
            Some(key) => std::env::set_var("NOMIC_API_KEY", key),
            None => std::env::remove_var("NOMIC_API_KEY"),
        }

        // Restore original database path environment variable
        match &self.original_db_path {
            Some(path) => std::env::set_var("SWISSARMYHAMMER_SEMANTIC_DB_PATH", path),
            None => std::env::remove_var("SWISSARMYHAMMER_SEMANTIC_DB_PATH"),
        }

        // Clean up test database file
        if self.test_db_path.exists() {
            if let Err(e) = std::fs::remove_file(&self.test_db_path) {
                eprintln!("Warning: Failed to clean up test database file: {e}");
            }
        }
    }
}

/// Create a semantic test environment guard
///
/// This provides isolated environment setup for semantic search tests
/// with proper cleanup and restoration of environment variables.
#[allow(dead_code)]
pub fn create_semantic_test_guard() -> SemanticTestGuard {
    SemanticTestGuard::new()
}

/// Setup a git repository in the given directory
///
/// Creates a basic git repository with initial commit for testing
/// git-related CLI functionality.
#[allow(dead_code)]
pub fn setup_git_repo(dir: &Path) -> Result<()> {
    use std::process::Command;

    // Initialize git repository
    let output = Command::new("git")
        .args(["init"])
        .current_dir(dir)
        .output()?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("Failed to initialize git repository"));
    }

    // Configure git user
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(dir)
        .output()?;

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(dir)
        .output()?;

    // Create initial commit
    std::fs::write(
        dir.join("README.md"),
        "# Test Repository\n\nThis is a test repository for CLI testing.",
    )?;

    Command::new("git")
        .args(["add", "."])
        .current_dir(dir)
        .output()?;

    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(dir)
        .output()?;

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
