//! Standardized logging configuration and utilities

use serde::Serialize;
use std::fmt::Debug;
use tracing::Level;
use tracing_subscriber::{
    filter::LevelFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

/// Wrapper for pretty-printing types in logs as YAML
/// Use this in tracing statements: info!("Config: {}", Pretty(&config));
/// Types must implement Serialize + Debug. Debug is fallback if YAML fails.
pub struct Pretty<T>(pub T);

impl<T: Serialize + Debug> std::fmt::Display for Pretty<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match serde_yaml_ng::to_string(&self.0) {
            Ok(yaml) => write!(f, "\n{}", yaml),
            Err(_) => write!(f, "\n{:#?}", self.0),
        }
    }
}

impl<T: Serialize + Debug> std::fmt::Debug for Pretty<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match serde_yaml_ng::to_string(&self.0) {
            Ok(yaml) => write!(f, "\n{}", yaml),
            Err(_) => write!(f, "\n{:#?}", self.0),
        }
    }
}

/// Standard log levels used across the workspace
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    /// Error conditions that require immediate attention
    Error,
    /// Warning conditions that may need attention
    Warn,
    /// General informational messages
    Info,
    /// Debug information for development
    Debug,
    /// Verbose trace information
    Trace,
}

impl From<LogLevel> for Level {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Error => Level::ERROR,
            LogLevel::Warn => Level::WARN,
            LogLevel::Info => Level::INFO,
            LogLevel::Debug => Level::DEBUG,
            LogLevel::Trace => Level::TRACE,
        }
    }
}

impl From<LogLevel> for LevelFilter {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Error => LevelFilter::ERROR,
            LogLevel::Warn => LevelFilter::WARN,
            LogLevel::Info => LevelFilter::INFO,
            LogLevel::Debug => LevelFilter::DEBUG,
            LogLevel::Trace => LevelFilter::TRACE,
        }
    }
}

/// Configuration for standardized logging setup
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    /// Base log level for the application
    pub level: LogLevel,
    /// Enable pretty formatting (colors, etc.)
    pub pretty: bool,
    /// Enable JSON formatting for structured logging
    pub json: bool,
    /// Module-specific log levels (e.g., "llama_agent::model=debug")
    pub module_filters: Vec<String>,
    /// Include timestamps in log output
    pub with_timestamps: bool,
    /// Include file/line information
    pub with_location: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            pretty: true,
            json: false,
            module_filters: Vec::new(),
            with_timestamps: true,
            with_location: false,
        }
    }
}

impl LoggingConfig {
    /// Create a new logging configuration
    pub fn new(level: LogLevel) -> Self {
        Self {
            level,
            ..Default::default()
        }
    }

    /// Enable pretty formatting
    pub fn with_pretty(mut self, pretty: bool) -> Self {
        self.pretty = pretty;
        self
    }

    /// Enable JSON formatting
    pub fn with_json(mut self, json: bool) -> Self {
        self.json = json;
        self
    }

    /// Add module-specific filter
    pub fn with_module_filter<S: Into<String>>(mut self, filter: S) -> Self {
        self.module_filters.push(filter.into());
        self
    }

    /// Enable/disable timestamps
    pub fn with_timestamps(mut self, timestamps: bool) -> Self {
        self.with_timestamps = timestamps;
        self
    }

    /// Enable/disable location information
    pub fn with_location(mut self, location: bool) -> Self {
        self.with_location = location;
        self
    }

    /// Initialize the global subscriber with this configuration
    pub fn init(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let base_filter = LevelFilter::from(self.level);

        // Build environment filter with module-specific rules
        let mut env_filter = EnvFilter::from_default_env().add_directive(base_filter.into());

        for filter in &self.module_filters {
            env_filter = env_filter.add_directive(filter.parse()?);
        }

        match (self.json, self.with_timestamps) {
            (true, true) => {
                // JSON formatting with timestamps
                tracing_subscriber::registry()
                    .with(env_filter)
                    .with(
                        fmt::layer()
                            .json()
                            .with_timer(fmt::time::SystemTime)
                            .with_file(self.with_location)
                            .with_line_number(self.with_location),
                    )
                    .try_init()?;
            }
            (true, false) => {
                // JSON formatting without timestamps
                tracing_subscriber::registry()
                    .with(env_filter)
                    .with(
                        fmt::layer()
                            .json()
                            .without_time()
                            .with_file(self.with_location)
                            .with_line_number(self.with_location),
                    )
                    .try_init()?;
            }
            (false, true) => {
                // Human-readable formatting with timestamps (always pretty)
                tracing_subscriber::registry()
                    .with(env_filter)
                    .with(
                        fmt::layer()
                            .pretty() // enables multi-line, indented output
                            .with_timer(fmt::time::SystemTime)
                            .with_file(self.with_location)
                            .with_line_number(self.with_location),
                    )
                    .try_init()?;
            }
            (false, false) => {
                // Human-readable formatting without timestamps (always pretty)
                tracing_subscriber::registry()
                    .with(env_filter)
                    .with(
                        fmt::layer()
                            .pretty() // enables multi-line, indented output
                            .without_time()
                            .with_file(self.with_location)
                            .with_line_number(self.with_location),
                    )
                    .try_init()?;
            }
        }

        Ok(())
    }
}

/// Initialize logging with debug level for development
pub fn init_debug_logging() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    LoggingConfig::new(LogLevel::Debug)
        .with_pretty(true)
        .with_location(true)
        .init()
}

/// Initialize logging with info level for production
pub fn init_production_logging() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    LoggingConfig::new(LogLevel::Info)
        .with_json(true)
        .with_pretty(false)
        .init()
}

/// Initialize logging from environment variables and debug flag
pub fn init_from_env(debug: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let level = if debug {
        LogLevel::Debug
    } else {
        LogLevel::Info
    };

    LoggingConfig::new(level)
        .with_pretty(
            !std::env::var("JSON_LOGS")
                .unwrap_or_default()
                .parse()
                .unwrap_or(false),
        )
        .with_json(
            std::env::var("JSON_LOGS")
                .unwrap_or_default()
                .parse()
                .unwrap_or(false),
        )
        .with_location(debug)
        .init()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_conversion() {
        assert_eq!(Level::from(LogLevel::Error), Level::ERROR);
        assert_eq!(Level::from(LogLevel::Info), Level::INFO);
        assert_eq!(Level::from(LogLevel::Debug), Level::DEBUG);
    }

    #[test]
    fn test_logging_config_builder() {
        let config = LoggingConfig::new(LogLevel::Debug)
            .with_pretty(false)
            .with_json(true)
            .with_module_filter("llama_agent=trace")
            .with_timestamps(false)
            .with_location(true);

        assert_eq!(config.level, LogLevel::Debug);
        assert!(!config.pretty);
        assert!(config.json);
        assert!(config
            .module_filters
            .contains(&"llama_agent=trace".to_string()));
        assert!(!config.with_timestamps);
        assert!(config.with_location);
    }

    #[test]
    fn test_default_config() {
        let config = LoggingConfig::default();
        assert_eq!(config.level, LogLevel::Info);
        assert!(config.pretty);
        assert!(!config.json);
        assert!(config.module_filters.is_empty());
        assert!(config.with_timestamps);
        assert!(!config.with_location);
    }

    #[test]
    fn test_log_level_to_level_filter() {
        assert_eq!(LevelFilter::from(LogLevel::Error), LevelFilter::ERROR);
        assert_eq!(LevelFilter::from(LogLevel::Warn), LevelFilter::WARN);
        assert_eq!(LevelFilter::from(LogLevel::Info), LevelFilter::INFO);
        assert_eq!(LevelFilter::from(LogLevel::Debug), LevelFilter::DEBUG);
        assert_eq!(LevelFilter::from(LogLevel::Trace), LevelFilter::TRACE);
    }

    #[test]
    fn test_log_level_all_conversions() {
        // Cover Warn and Trace which were missing
        assert_eq!(Level::from(LogLevel::Warn), Level::WARN);
        assert_eq!(Level::from(LogLevel::Trace), Level::TRACE);
    }

    #[test]
    fn test_log_level_traits() {
        let level = LogLevel::Info;
        let cloned = level;
        assert_eq!(level, cloned);

        let debug = format!("{:?}", LogLevel::Error);
        assert!(debug.contains("Error"));
    }

    #[test]
    fn test_pretty_display_serializable() {
        let data = vec!["hello", "world"];
        let pretty = Pretty(&data);
        let output = format!("{}", pretty);
        assert!(output.contains("hello"));
        assert!(output.contains("world"));
    }

    #[test]
    fn test_pretty_debug_serializable() {
        let data = vec![1, 2, 3];
        let pretty = Pretty(&data);
        let output = format!("{:?}", pretty);
        assert!(output.contains("1"));
        assert!(output.contains("2"));
        assert!(output.contains("3"));
    }

    #[test]
    fn test_logging_config_with_timestamps() {
        let config = LoggingConfig::new(LogLevel::Info).with_timestamps(false);
        assert!(!config.with_timestamps);

        let config = LoggingConfig::new(LogLevel::Info).with_timestamps(true);
        assert!(config.with_timestamps);
    }

    #[test]
    fn test_logging_config_chaining() {
        let config = LoggingConfig::new(LogLevel::Warn)
            .with_pretty(true)
            .with_json(false)
            .with_timestamps(true)
            .with_location(false)
            .with_module_filter("mymod=debug");

        assert_eq!(config.level, LogLevel::Warn);
        assert!(config.pretty);
        assert!(!config.json);
        assert!(config.with_timestamps);
        assert!(!config.with_location);
        assert_eq!(config.module_filters.len(), 1);
    }
}
