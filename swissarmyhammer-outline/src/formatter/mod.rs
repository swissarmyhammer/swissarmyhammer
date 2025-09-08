//! Output formatters for code outlines
//!
//! This module provides various formatters to convert outline structures
//! into different output formats like YAML, JSON, etc.

/// YAML formatter for outline structures
pub struct YamlFormatter {
    config: FormatterConfig,
}

/// Configuration for formatting options
#[derive(Debug, Clone)]
pub struct FormatterConfig {
    /// Number of spaces for indentation
    pub indent_size: usize,
    /// Whether to include line numbers
    pub include_line_numbers: bool,
    /// Whether to include signatures
    pub include_signatures: bool,
}

impl Default for FormatterConfig {
    fn default() -> Self {
        Self {
            indent_size: 2,
            include_line_numbers: true,
            include_signatures: true,
        }
    }
}

impl YamlFormatter {
    /// Create a new YAML formatter with default configuration
    pub fn with_defaults() -> Self {
        Self {
            config: FormatterConfig::default(),
        }
    }
    
    /// Create a new YAML formatter with custom configuration
    pub fn new(config: FormatterConfig) -> Self {
        Self { config }
    }
}