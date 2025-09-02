//! Test configuration and utilities for LlamaAgent testing
//!
//! This module provides test configuration infrastructure that adapts to different
//! environments (CI vs local development) and enables or disables specific executor
//! types based on availability of dependencies like Claude CLI or model files.

use std::env;
use std::sync::OnceLock;
use swissarmyhammer_config::{
    agent::{AgentConfig, LlamaAgentConfig, McpServerConfig, ModelConfig, ModelSource},
    DEFAULT_TEST_LLM_MODEL_FILENAME, DEFAULT_TEST_LLM_MODEL_REPO,
};

/// Test configuration for different environments
#[derive(Debug, Clone)]
pub struct TestConfig {
    /// Enable LlamaAgent tests (default: false, requires models)
    pub enable_llama_tests: bool,
    /// Enable Claude tests (default: true, requires claude CLI)
    pub enable_claude_tests: bool,
    /// Test timeout in seconds (shorter in CI)
    pub test_timeout_seconds: u64,
    /// LlamaAgent model repository for testing
    pub llama_model_repo: String,
    /// LlamaAgent model filename for testing
    pub llama_model_filename: String,
    /// Maximum concurrent tests (lower in CI)
    pub max_concurrent_tests: usize,
    /// Whether we're running in CI environment
    pub is_ci: bool,
}

impl TestConfig {
    /// Load test configuration from environment variables
    pub fn from_environment() -> &'static Self {
        static CONFIG: OnceLock<TestConfig> = OnceLock::new();
        CONFIG.get_or_init(|| {
            let is_ci = env::var("CI")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(false)
                || env::var("GITHUB_ACTIONS")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false);

            Self {
                enable_llama_tests: env::var("SAH_TEST_LLAMA")
                    .map(|v| v.to_lowercase() == "true" || v == "1")
                    .unwrap_or(false),
                enable_claude_tests: env::var("SAH_TEST_CLAUDE")
                    .map(|v| v.to_lowercase() == "true" || v == "1")
                    .unwrap_or(true),
                test_timeout_seconds: if is_ci {
                    env::var("SAH_TEST_TIMEOUT")
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(60) // Shorter timeout in CI
                } else {
                    env::var("SAH_TEST_TIMEOUT")
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(120) // Longer timeout for local development
                },
                llama_model_repo: env::var("SAH_TEST_MODEL_REPO")
                    .unwrap_or_else(|_| DEFAULT_TEST_LLM_MODEL_REPO.to_string()),
                llama_model_filename: env::var("SAH_TEST_MODEL_FILENAME")
                    .unwrap_or_else(|_| DEFAULT_TEST_LLM_MODEL_FILENAME.to_string()),
                max_concurrent_tests: if is_ci {
                    env::var("SAH_TEST_MAX_CONCURRENT")
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(3) // Lower concurrency in CI
                } else {
                    env::var("SAH_TEST_MAX_CONCURRENT")
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(5) // Higher concurrency for local development
                },
                is_ci,
            }
        })
    }

    /// Get optimized configuration for development environment
    pub fn development() -> Self {
        Self {
            enable_llama_tests: true,
            enable_claude_tests: true,
            test_timeout_seconds: 120,
            llama_model_repo: DEFAULT_TEST_LLM_MODEL_REPO.to_string(),
            llama_model_filename: DEFAULT_TEST_LLM_MODEL_FILENAME.to_string(),
            max_concurrent_tests: 5,
            is_ci: false,
        }
    }

    /// Get optimized configuration for CI environment
    pub fn ci() -> Self {
        Self {
            enable_llama_tests: false, // Disabled by default in CI (no models)
            enable_claude_tests: true,
            test_timeout_seconds: 60,
            llama_model_repo: DEFAULT_TEST_LLM_MODEL_REPO.to_string(),
            llama_model_filename: DEFAULT_TEST_LLM_MODEL_FILENAME.to_string(),
            max_concurrent_tests: 3,
            is_ci: true,
        }
    }

    /// Create LlamaAgent configuration for testing
    pub fn create_llama_config(&self) -> LlamaAgentConfig {
        LlamaAgentConfig {
            model: ModelConfig {
                source: ModelSource::HuggingFace {
                    repo: self.llama_model_repo.clone(),
                    filename: Some(self.llama_model_filename.clone()),
                },
                batch_size: 256, // Smaller batch size for testing
                use_hf_params: true,
                debug: true, // Enable debug for testing
            },
            mcp_server: McpServerConfig {
                port: 0,                                           // Random available port for testing
                timeout_seconds: if self.is_ci { 10 } else { 30 }, // Shorter timeout in CI
            },

            repetition_detection: Default::default(),
        }
    }

    /// Create Claude Code configuration for testing
    pub fn create_claude_config(&self) -> AgentConfig {
        let mut config = AgentConfig::claude_code();
        config.quiet = true; // Quiet mode for tests
        config
    }

    /// Create LlamaAgent configuration for testing
    pub fn create_llama_agent_config(&self) -> AgentConfig {
        let mut config = AgentConfig::llama_agent(self.create_llama_config());
        config.quiet = true; // Quiet mode for tests
        config
    }
}

/// Test environment helper for consistent test setup
#[derive(Debug)]
pub struct TestEnvironment {
    config: TestConfig,
}

impl TestEnvironment {
    /// Create a new test environment with configuration from environment
    pub fn new() -> Self {
        Self {
            config: TestConfig::from_environment().clone(),
        }
    }

    /// Create a test environment with specific configuration
    pub fn with_config(config: TestConfig) -> Self {
        Self { config }
    }

    /// Check if LlamaAgent tests should run
    pub fn should_test_llama(&self) -> bool {
        self.config.enable_llama_tests
    }

    /// Check if Claude tests should run
    pub fn should_test_claude(&self) -> bool {
        self.config.enable_claude_tests
    }

    /// Get test timeout duration
    pub fn test_timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.config.test_timeout_seconds)
    }

    /// Get maximum concurrent test count
    pub fn max_concurrent(&self) -> usize {
        self.config.max_concurrent_tests
    }

    /// Get the test configuration
    pub fn config(&self) -> &TestConfig {
        &self.config
    }
}

impl Default for TestEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

/// Skip test if LlamaAgent testing is disabled
pub fn skip_if_llama_disabled() {
    let config = TestConfig::from_environment();
    if !config.enable_llama_tests {
        eprintln!("⚠️ Skipping LlamaAgent test (set SAH_TEST_LLAMA=true to enable)");
        eprintln!("   This test requires model files and may take significant time");
        std::process::exit(0); // Skip test gracefully
    }
}

/// Skip test if Claude testing is disabled
pub fn skip_if_claude_disabled() {
    let config = TestConfig::from_environment();
    if !config.enable_claude_tests {
        eprintln!("⚠️ Skipping Claude test (set SAH_TEST_CLAUDE=false to disable)");
        std::process::exit(0); // Skip test gracefully
    }
}

/// Skip test if both executors are disabled
pub fn skip_if_both_disabled() {
    let config = TestConfig::from_environment();
    if !config.enable_claude_tests && !config.enable_llama_tests {
        eprintln!("⚠️ Skipping test - both Claude and LlamaAgent tests are disabled");
        eprintln!("   Enable with SAH_TEST_CLAUDE=true and/or SAH_TEST_LLAMA=true");
        std::process::exit(0); // Skip test gracefully
    }
}

/// Macro for executor-specific test setup
///
/// This macro generates test functions that only run when the specific
/// executor type is enabled in the test configuration.
///
/// # Example
///
/// ```rust
/// executor_test!(test_claude_only, claude, {
///     let config = AgentConfig::claude_code();
///     // Test Claude executor only
/// });
///
/// executor_test!(test_llama_only, llama, {
///     let config = AgentConfig::llama_agent(LlamaAgentConfig::for_testing());
///     // Test LlamaAgent executor only
/// });
/// ```
#[macro_export]
macro_rules! executor_test {
    ($test_name:ident, claude, $test_body:block) => {
        #[tokio::test]
        async fn $test_name() {
            $crate::llama_test_config::skip_if_claude_disabled();
            $test_body
        }
    };
    ($test_name:ident, llama, $test_body:block) => {
        #[tokio::test]
        async fn $test_name() {
            $crate::llama_test_config::skip_if_llama_disabled();
            $test_body
        }
    };
    ($test_name:ident, both, $test_body:block) => {
        #[tokio::test]
        async fn $test_name() {
            $crate::llama_test_config::skip_if_both_disabled();
            $test_body
        }
    };
}

/// Macro for cross-executor testing
///
/// This macro generates test functions that test both executor types
/// when they are enabled, providing consistent test structure.
///
/// # Example
///
/// ```rust
/// cross_executor_test!(test_both_executors, {
///     |config: AgentConfig| async move {
///         // Test logic that works with any AgentConfig
///         let context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());
///         // ... test implementation
///     }
/// });
/// ```
#[macro_export]
macro_rules! cross_executor_test {
    ($test_name:ident, $test_fn:expr) => {
        #[tokio::test]
        async fn $test_name() {
            let env = $crate::llama_test_config::TestEnvironment::new();
            let test_fn = $test_fn;

            if env.should_test_claude() {
                println!("🔷 Testing with Claude executor");
                let config = env.config().create_claude_config();
                test_fn(config).await;
            }

            if env.should_test_llama() {
                println!("🔸 Testing with LlamaAgent executor");
                let config = env.config().create_llama_agent_config();
                test_fn(config).await;
            }

            if !env.should_test_claude() && !env.should_test_llama() {
                eprintln!("⚠️ No executors enabled for cross-executor test");
                eprintln!("   Enable with SAH_TEST_CLAUDE=true and/or SAH_TEST_LLAMA=true");
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_environment_defaults() {
        // Clear environment variables
        env::remove_var("SAH_TEST_LLAMA");
        env::remove_var("SAH_TEST_CLAUDE");
        env::remove_var("SAH_TEST_TIMEOUT");
        env::remove_var("CI");
        env::remove_var("GITHUB_ACTIONS");

        let config = TestConfig::from_environment();

        // Defaults should be Claude enabled, LlamaAgent disabled
        assert!(!config.enable_llama_tests);
        assert!(config.enable_claude_tests);
        assert_eq!(config.test_timeout_seconds, 120); // Non-CI default
        assert!(!config.is_ci);
    }

    #[test]
    fn test_config_development_preset() {
        let config = TestConfig::development();

        assert!(config.enable_llama_tests);
        assert!(config.enable_claude_tests);
        assert_eq!(config.test_timeout_seconds, 120);
        assert_eq!(config.max_concurrent_tests, 5);
        assert!(!config.is_ci);
    }

    #[test]
    fn test_config_ci_preset() {
        let config = TestConfig::ci();

        assert!(!config.enable_llama_tests); // Disabled in CI by default
        assert!(config.enable_claude_tests);
        assert_eq!(config.test_timeout_seconds, 60); // Shorter in CI
        assert_eq!(config.max_concurrent_tests, 3); // Lower concurrency in CI
        assert!(config.is_ci);
    }

    #[test]
    fn test_llama_config_creation() {
        let config = TestConfig::development();
        let llama_config = config.create_llama_config();

        match llama_config.model.source {
            ModelSource::HuggingFace { repo, filename } => {
                assert_eq!(repo, DEFAULT_TEST_LLM_MODEL_REPO);
                assert_eq!(filename, Some(DEFAULT_TEST_LLM_MODEL_FILENAME.to_string()));
            }
            ModelSource::Local { .. } => panic!("Should be HuggingFace source"),
        }

        assert_eq!(llama_config.mcp_server.port, 0); // Random port for testing
        assert_eq!(llama_config.mcp_server.timeout_seconds, 30); // Development timeout
    }

    #[test]
    fn test_claude_config_creation() {
        let config = TestConfig::development();
        let agent_config = config.create_claude_config();

        assert!(agent_config.quiet);
        assert!(matches!(
            agent_config.executor,
            swissarmyhammer_config::agent::AgentExecutorConfig::ClaudeCode(_)
        ));
    }

    #[test]
    fn test_llama_agent_config_creation() {
        let config = TestConfig::development();
        let agent_config = config.create_llama_agent_config();

        assert!(agent_config.quiet);
        assert!(matches!(
            agent_config.executor,
            swissarmyhammer_config::agent::AgentExecutorConfig::LlamaAgent(_)
        ));
    }

    #[test]
    fn test_test_environment_new() {
        let env = TestEnvironment::new();
        // Should not panic and should load configuration
        let _ = env.config();
    }

    #[test]
    fn test_test_environment_with_config() {
        let config = TestConfig::ci();
        let env = TestEnvironment::with_config(config.clone());

        assert_eq!(env.should_test_llama(), config.enable_llama_tests);
        assert_eq!(env.should_test_claude(), config.enable_claude_tests);
        assert_eq!(
            env.test_timeout(),
            std::time::Duration::from_secs(config.test_timeout_seconds)
        );
        assert_eq!(env.max_concurrent(), config.max_concurrent_tests);
    }

    #[test]
    fn test_ci_detection() {
        // Test GitHub Actions detection
        env::set_var("GITHUB_ACTIONS", "true");
        let config = TestConfig::from_environment();
        assert!(config.is_ci);
        env::remove_var("GITHUB_ACTIONS");

        // Test generic CI detection
        env::set_var("CI", "true");
        let config = TestConfig::from_environment();
        assert!(config.is_ci);
        env::remove_var("CI");
    }
}
