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
    /// Cache TTL in seconds (default: 300, i.e., 5 minutes)
    pub cache_ttl_seconds: u64,
    /// Maximum cache size (default: 1000)
    pub cache_max_size: usize,
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

impl Default for Config {
    fn default() -> Self {
        Self {
            issue_branch_prefix: "issue/".to_string(),
            max_pending_issues_in_summary: 5,
            max_content_length: 50000,
            max_line_length: 10000,
            max_issue_name_length: 100,
            cache_ttl_seconds: 300,
            cache_max_size: 1000,
            issue_number_width: 6,
            min_issue_number: 1,
            max_issue_number: 999_999,
            issue_number_digits: 6,
            virtual_issue_number_base: 500_000,
            virtual_issue_number_range: 500_000,
            default_issue_content: "# Issue\n\nDescribe the issue here.".to_string(),
        }
    }
}

impl Config {
    /// Create a new configuration instance with values from environment variables
    /// or defaults if environment variables are not set
    pub fn new() -> Self {
        let loader = EnvLoader::new("SWISSARMYHAMMER");

        Self {
            issue_branch_prefix: loader.load_string("ISSUE_BRANCH_PREFIX", "issue/"),
            max_pending_issues_in_summary: loader.load_parsed("MAX_PENDING_ISSUES_IN_SUMMARY", 5),
            max_content_length: loader.load_parsed("MAX_CONTENT_LENGTH", 50000),
            max_line_length: loader.load_parsed("MAX_LINE_LENGTH", 10000),
            max_issue_name_length: loader.load_parsed("MAX_ISSUE_NAME_LENGTH", 100),
            cache_ttl_seconds: loader.load_parsed("CACHE_TTL_SECONDS", 300),
            cache_max_size: loader.load_parsed("CACHE_MAX_SIZE", 1000),
            issue_number_width: loader.load_parsed("ISSUE_NUMBER_WIDTH", 6),
            min_issue_number: loader.load_parsed("MIN_ISSUE_NUMBER", 1),
            max_issue_number: loader.load_parsed("MAX_ISSUE_NUMBER", 999_999),
            issue_number_digits: loader.load_parsed("ISSUE_NUMBER_DIGITS", 6),
            virtual_issue_number_base: loader.load_parsed("VIRTUAL_ISSUE_NUMBER_BASE", 500_000),
            virtual_issue_number_range: loader.load_parsed("VIRTUAL_ISSUE_NUMBER_RANGE", 500_000),
            default_issue_content: loader.load_string(
                "DEFAULT_ISSUE_CONTENT",
                "# Issue\n\nDescribe the issue here.",
            ),
        }
    }

    /// Get the global configuration instance
    pub fn global() -> &'static Self {
        static CONFIG: std::sync::OnceLock<Config> = std::sync::OnceLock::new();
        CONFIG.get_or_init(Config::new)
    }

    /// Reset the global configuration (for testing purposes)
    #[cfg(test)]
    pub fn reset_global() {
        // This is a workaround since OnceLock doesn't have a reset method
        // We can't actually reset the global config in tests due to OnceLock's design
        // Tests should use Config::new() directly instead of global() for testing env vars
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    #[serial_test::serial]
    fn test_config_new() {
        // Clean up any environment variables from other tests
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
    #[serial_test::serial]
    fn test_config_with_env_vars() {
        // Save original env vars if they exist
        let orig_prefix = std::env::var("SWISSARMYHAMMER_ISSUE_BRANCH_PREFIX").ok();
        let orig_width = std::env::var("SWISSARMYHAMMER_ISSUE_NUMBER_WIDTH").ok();
        let orig_max_pending = std::env::var("SWISSARMYHAMMER_MAX_PENDING_ISSUES_IN_SUMMARY").ok();
        let orig_max_number = std::env::var("SWISSARMYHAMMER_MAX_ISSUE_NUMBER").ok();
        let orig_digits = std::env::var("SWISSARMYHAMMER_ISSUE_NUMBER_DIGITS").ok();
        let orig_virtual_base = std::env::var("SWISSARMYHAMMER_VIRTUAL_ISSUE_NUMBER_BASE").ok();
        let orig_virtual_range = std::env::var("SWISSARMYHAMMER_VIRTUAL_ISSUE_NUMBER_RANGE").ok();
        let orig_default_content = std::env::var("SWISSARMYHAMMER_DEFAULT_ISSUE_CONTENT").ok();

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

        // Restore original env vars or remove if they didn't exist
        match orig_prefix {
            Some(val) => std::env::set_var("SWISSARMYHAMMER_ISSUE_BRANCH_PREFIX", val),
            None => std::env::remove_var("SWISSARMYHAMMER_ISSUE_BRANCH_PREFIX"),
        }
        match orig_width {
            Some(val) => std::env::set_var("SWISSARMYHAMMER_ISSUE_NUMBER_WIDTH", val),
            None => std::env::remove_var("SWISSARMYHAMMER_ISSUE_NUMBER_WIDTH"),
        }
        match orig_max_pending {
            Some(val) => std::env::set_var("SWISSARMYHAMMER_MAX_PENDING_ISSUES_IN_SUMMARY", val),
            None => std::env::remove_var("SWISSARMYHAMMER_MAX_PENDING_ISSUES_IN_SUMMARY"),
        }
        match orig_max_number {
            Some(val) => std::env::set_var("SWISSARMYHAMMER_MAX_ISSUE_NUMBER", val),
            None => std::env::remove_var("SWISSARMYHAMMER_MAX_ISSUE_NUMBER"),
        }
        match orig_digits {
            Some(val) => std::env::set_var("SWISSARMYHAMMER_ISSUE_NUMBER_DIGITS", val),
            None => std::env::remove_var("SWISSARMYHAMMER_ISSUE_NUMBER_DIGITS"),
        }
        match orig_virtual_base {
            Some(val) => std::env::set_var("SWISSARMYHAMMER_VIRTUAL_ISSUE_NUMBER_BASE", val),
            None => std::env::remove_var("SWISSARMYHAMMER_VIRTUAL_ISSUE_NUMBER_BASE"),
        }
        match orig_virtual_range {
            Some(val) => std::env::set_var("SWISSARMYHAMMER_VIRTUAL_ISSUE_NUMBER_RANGE", val),
            None => std::env::remove_var("SWISSARMYHAMMER_VIRTUAL_ISSUE_NUMBER_RANGE"),
        }
        match orig_default_content {
            Some(val) => std::env::set_var("SWISSARMYHAMMER_DEFAULT_ISSUE_CONTENT", val),
            None => std::env::remove_var("SWISSARMYHAMMER_DEFAULT_ISSUE_CONTENT"),
        }
    }
}
