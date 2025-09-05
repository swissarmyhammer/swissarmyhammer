//! Configuration management for SwissArmyHammer
//!
//! This module provides centralized configuration management with environment variable support
//! and sensible defaults for all configurable constants throughout the application.

use crate::common::env_loader::EnvLoader;

/// Configuration settings for the SwissArmyHammer application
#[derive(Debug, Clone)]
pub struct Config {
    /// Prefix for issue branches (default: "issue/")
    pub issue_branch_prefix: String,
    /// Maximum number of pending issues to display in summary (default: 5)
    pub max_pending_issues_in_summary: usize,
    /// Maximum content length for issue content (default: 50000)
    pub max_content_length: usize,
    /// Maximum line length for issue content (default: 10000)
    pub max_line_length: usize,
    /// Maximum issue name length (default: 100)
    pub max_issue_name_length: usize,

    /// Issue number width for formatting (default: 6)
    pub issue_number_width: usize,
    /// Minimum issue number (default: 1)
    pub min_issue_number: u32,
    /// Maximum issue number (default: 999_999)
    pub max_issue_number: u32,
    /// Issue number digits (default: 6)
    pub issue_number_digits: usize,
    /// Virtual issue number base (default: 500_000)
    pub virtual_issue_number_base: u32,
    /// Virtual issue number range (default: 500_000)
    pub virtual_issue_number_range: u32,
    /// Default content for new issues (default: "# Issue\n\nDescribe the issue here.")
    pub default_issue_content: String,
}

impl Config {
    const DEFAULT_ISSUE_BRANCH_PREFIX: &'static str = "issue/";
    const DEFAULT_MAX_PENDING_ISSUES_IN_SUMMARY: usize = 5;
    const DEFAULT_MAX_CONTENT_LENGTH: usize = 50000;
    const DEFAULT_MAX_LINE_LENGTH: usize = 10000;
    const DEFAULT_MAX_ISSUE_NAME_LENGTH: usize = 100;
    const DEFAULT_ISSUE_NUMBER_WIDTH: usize = 6;
    const DEFAULT_MIN_ISSUE_NUMBER: u32 = 1;
    const DEFAULT_MAX_ISSUE_NUMBER: u32 = 999_999;
    const DEFAULT_ISSUE_NUMBER_DIGITS: usize = 6;
    const DEFAULT_VIRTUAL_ISSUE_NUMBER_BASE: u32 = 500_000;
    const DEFAULT_VIRTUAL_ISSUE_NUMBER_RANGE: u32 = 500_000;
    const DEFAULT_ISSUE_CONTENT: &'static str = "# Issue\n\nDescribe the issue here.";
}

impl Default for Config {
    fn default() -> Self {
        Self {
            issue_branch_prefix: Config::DEFAULT_ISSUE_BRANCH_PREFIX.to_string(),
            max_pending_issues_in_summary: Config::DEFAULT_MAX_PENDING_ISSUES_IN_SUMMARY,
            max_content_length: Config::DEFAULT_MAX_CONTENT_LENGTH,
            max_line_length: Config::DEFAULT_MAX_LINE_LENGTH,
            max_issue_name_length: Config::DEFAULT_MAX_ISSUE_NAME_LENGTH,

            issue_number_width: Config::DEFAULT_ISSUE_NUMBER_WIDTH,
            min_issue_number: Config::DEFAULT_MIN_ISSUE_NUMBER,
            max_issue_number: Config::DEFAULT_MAX_ISSUE_NUMBER,
            issue_number_digits: Config::DEFAULT_ISSUE_NUMBER_DIGITS,
            virtual_issue_number_base: Config::DEFAULT_VIRTUAL_ISSUE_NUMBER_BASE,
            virtual_issue_number_range: Config::DEFAULT_VIRTUAL_ISSUE_NUMBER_RANGE,
            default_issue_content: Config::DEFAULT_ISSUE_CONTENT.to_string(),
        }
    }
}

impl Config {
    /// Create a new configuration instance with values from environment variables
    /// or defaults if environment variables are not set
    pub fn new() -> Self {
        let loader = EnvLoader::new("SWISSARMYHAMMER");

        Self {
            issue_branch_prefix: loader
                .load_string("ISSUE_BRANCH_PREFIX", Self::DEFAULT_ISSUE_BRANCH_PREFIX),
            max_pending_issues_in_summary: loader.load_parsed(
                "MAX_PENDING_ISSUES_IN_SUMMARY",
                Self::DEFAULT_MAX_PENDING_ISSUES_IN_SUMMARY,
            ),
            max_content_length: loader
                .load_parsed("MAX_CONTENT_LENGTH", Self::DEFAULT_MAX_CONTENT_LENGTH),
            max_line_length: loader.load_parsed("MAX_LINE_LENGTH", Self::DEFAULT_MAX_LINE_LENGTH),
            max_issue_name_length: loader
                .load_parsed("MAX_ISSUE_NAME_LENGTH", Self::DEFAULT_MAX_ISSUE_NAME_LENGTH),

            issue_number_width: loader
                .load_parsed("ISSUE_NUMBER_WIDTH", Self::DEFAULT_ISSUE_NUMBER_WIDTH),
            min_issue_number: loader
                .load_parsed("MIN_ISSUE_NUMBER", Self::DEFAULT_MIN_ISSUE_NUMBER),
            max_issue_number: loader
                .load_parsed("MAX_ISSUE_NUMBER", Self::DEFAULT_MAX_ISSUE_NUMBER),
            issue_number_digits: loader
                .load_parsed("ISSUE_NUMBER_DIGITS", Self::DEFAULT_ISSUE_NUMBER_DIGITS),
            virtual_issue_number_base: loader.load_parsed(
                "VIRTUAL_ISSUE_NUMBER_BASE",
                Self::DEFAULT_VIRTUAL_ISSUE_NUMBER_BASE,
            ),
            virtual_issue_number_range: loader.load_parsed(
                "VIRTUAL_ISSUE_NUMBER_RANGE",
                Self::DEFAULT_VIRTUAL_ISSUE_NUMBER_RANGE,
            ),
            default_issue_content: loader
                .load_string("DEFAULT_ISSUE_CONTENT", Self::DEFAULT_ISSUE_CONTENT),
        }
    }

    /// Get the global configuration instance
    pub fn global() -> &'static Self {
        static CONFIG: std::sync::OnceLock<Config> = std::sync::OnceLock::new();
        CONFIG.get_or_init(Config::new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Global mutex to serialize environment variable tests
    /// This prevents race conditions when multiple tests modify environment variables
    static ENV_VAR_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.issue_branch_prefix, "issue/");
        assert_eq!(config.issue_number_width, 6);
        assert_eq!(config.max_pending_issues_in_summary, 5);
        assert_eq!(config.min_issue_number, 1);
        assert_eq!(config.max_issue_number, 999_999);
        assert_eq!(config.issue_number_digits, 6);
        assert_eq!(config.max_content_length, 50000);
        assert_eq!(config.max_line_length, 10000);
        assert_eq!(config.max_issue_name_length, 100);
        assert_eq!(config.virtual_issue_number_base, 500_000);
        assert_eq!(config.virtual_issue_number_range, 500_000);
        assert_eq!(
            config.default_issue_content,
            "# Issue\n\nDescribe the issue here."
        );
    }

    #[test]
    fn test_config_new() {
        let _guard = crate::test_utils::IsolatedTestEnvironment::new().unwrap();

        // Clean up any environment variables that could affect this test
        std::env::remove_var("SWISSARMYHAMMER_ISSUE_BRANCH_PREFIX");
        std::env::remove_var("SWISSARMYHAMMER_ISSUE_NUMBER_WIDTH");
        std::env::remove_var("SWISSARMYHAMMER_MAX_PENDING_ISSUES_IN_SUMMARY");
        std::env::remove_var("SWISSARMYHAMMER_MAX_ISSUE_NUMBER");
        std::env::remove_var("SWISSARMYHAMMER_ISSUE_NUMBER_DIGITS");
        std::env::remove_var("SWISSARMYHAMMER_MAX_CONTENT_LENGTH");
        std::env::remove_var("SWISSARMYHAMMER_MAX_LINE_LENGTH");
        std::env::remove_var("SWISSARMYHAMMER_MAX_ISSUE_NAME_LENGTH");
        std::env::remove_var("SWISSARMYHAMMER_VIRTUAL_ISSUE_NUMBER_BASE");
        std::env::remove_var("SWISSARMYHAMMER_VIRTUAL_ISSUE_NUMBER_RANGE");
        std::env::remove_var("SWISSARMYHAMMER_DEFAULT_ISSUE_CONTENT");

        let config = Config::new();
        // Should use defaults when environment variables are not set
        assert_eq!(config.issue_branch_prefix, "issue/");
        assert_eq!(config.issue_number_width, 6);
        assert_eq!(config.max_pending_issues_in_summary, 5);
        assert_eq!(config.min_issue_number, 1);
        assert_eq!(config.max_issue_number, 999_999);
        assert_eq!(config.issue_number_digits, 6);
        assert_eq!(config.max_content_length, 50000);
        assert_eq!(config.max_line_length, 10000);
        assert_eq!(config.max_issue_name_length, 100);
        assert_eq!(config.virtual_issue_number_base, 500_000);
        assert_eq!(config.virtual_issue_number_range, 500_000);
        assert_eq!(
            config.default_issue_content,
            "# Issue\n\nDescribe the issue here."
        );
    }

    #[test]
    fn test_config_with_env_vars() {
        // Acquire the global environment variable test lock to prevent race conditions
        let _lock_guard = ENV_VAR_TEST_LOCK.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("Environment variable test lock was poisoned, recovering");
            poisoned.into_inner()
        });

        // DO NOT use IsolatedTestEnvironment here as it can trigger Config::global() initialization
        // during HOME env var manipulation, which would contaminate the global config

        // Clean up any environment variables first
        std::env::remove_var("SWISSARMYHAMMER_ISSUE_BRANCH_PREFIX");
        std::env::remove_var("SWISSARMYHAMMER_ISSUE_NUMBER_WIDTH");
        std::env::remove_var("SWISSARMYHAMMER_MAX_PENDING_ISSUES_IN_SUMMARY");
        std::env::remove_var("SWISSARMYHAMMER_MAX_ISSUE_NUMBER");
        std::env::remove_var("SWISSARMYHAMMER_ISSUE_NUMBER_DIGITS");
        std::env::remove_var("SWISSARMYHAMMER_VIRTUAL_ISSUE_NUMBER_BASE");
        std::env::remove_var("SWISSARMYHAMMER_VIRTUAL_ISSUE_NUMBER_RANGE");
        std::env::remove_var("SWISSARMYHAMMER_DEFAULT_ISSUE_CONTENT");

        // Set test values
        std::env::set_var("SWISSARMYHAMMER_ISSUE_BRANCH_PREFIX", "feature/");
        std::env::set_var("SWISSARMYHAMMER_ISSUE_NUMBER_WIDTH", "8");
        std::env::set_var("SWISSARMYHAMMER_MAX_PENDING_ISSUES_IN_SUMMARY", "10");
        std::env::set_var("SWISSARMYHAMMER_MAX_ISSUE_NUMBER", "9999999");
        std::env::set_var("SWISSARMYHAMMER_ISSUE_NUMBER_DIGITS", "7");
        std::env::set_var("SWISSARMYHAMMER_VIRTUAL_ISSUE_NUMBER_BASE", "600000");
        std::env::set_var("SWISSARMYHAMMER_VIRTUAL_ISSUE_NUMBER_RANGE", "400000");
        std::env::set_var(
            "SWISSARMYHAMMER_DEFAULT_ISSUE_CONTENT",
            "# Test Issue\n\nTest content here.",
        );

        // CRITICAL: Use Config::new() instead of Config::global() to avoid contaminating
        // the global singleton. This test specifically verifies environment variable loading.
        let config = Config::new();
        assert_eq!(config.issue_branch_prefix, "feature/");
        assert_eq!(config.issue_number_width, 8);
        assert_eq!(config.max_pending_issues_in_summary, 10);
        assert_eq!(config.min_issue_number, 1);
        assert_eq!(config.max_issue_number, 9_999_999);
        assert_eq!(config.issue_number_digits, 7);
        assert_eq!(config.virtual_issue_number_base, 600_000);
        assert_eq!(config.virtual_issue_number_range, 400_000);
        assert_eq!(
            config.default_issue_content,
            "# Test Issue\n\nTest content here."
        );

        // Clean up the environment variables we set
        std::env::remove_var("SWISSARMYHAMMER_ISSUE_BRANCH_PREFIX");
        std::env::remove_var("SWISSARMYHAMMER_ISSUE_NUMBER_WIDTH");
        std::env::remove_var("SWISSARMYHAMMER_MAX_PENDING_ISSUES_IN_SUMMARY");
        std::env::remove_var("SWISSARMYHAMMER_MAX_ISSUE_NUMBER");
        std::env::remove_var("SWISSARMYHAMMER_ISSUE_NUMBER_DIGITS");
        std::env::remove_var("SWISSARMYHAMMER_VIRTUAL_ISSUE_NUMBER_BASE");
        std::env::remove_var("SWISSARMYHAMMER_VIRTUAL_ISSUE_NUMBER_RANGE");
        std::env::remove_var("SWISSARMYHAMMER_DEFAULT_ISSUE_CONTENT");
    }
}
