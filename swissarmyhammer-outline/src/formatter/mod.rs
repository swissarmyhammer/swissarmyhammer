//! Output formatters for code outlines
//!
//! This module provides various formatters to convert outline structures
//! into different output formats like YAML, JSON, etc.

use crate::types::{FileOutline, OutlineHierarchy, OutlineNode, SymbolVisibility};
use crate::{OutlineError, Result};
use std::fmt::Write;

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
    /// Whether to include private symbols
    pub include_private_symbols: bool,
    /// Maximum length for signatures before truncation
    pub max_signature_length: Option<usize>,
}

impl Default for FormatterConfig {
    fn default() -> Self {
        Self {
            indent_size: 2,
            include_line_numbers: true,
            include_signatures: true,
            include_private_symbols: true,
            max_signature_length: Some(120),
        }
    }
}

impl YamlFormatter {
    /// Create a new YAML formatter with default configuration
    ///
    /// Uses sensible defaults:
    /// - 2-space indentation
    /// - Line numbers included
    /// - Signatures included
    /// - Private symbols included
    /// - Signature length limited to 120 characters
    pub fn with_defaults() -> Self {
        Self {
            config: FormatterConfig::default(),
        }
    }

    /// Create a new YAML formatter with custom configuration
    ///
    /// # Arguments
    ///
    /// * `config` - Custom formatting configuration options
    pub fn new(config: FormatterConfig) -> Self {
        Self { config }
    }

    /// Format a complete hierarchical outline structure into YAML
    ///
    /// Converts an outline hierarchy into a structured YAML format suitable for
    /// code navigation and understanding. The output includes file paths, symbol
    /// names, types, line numbers, signatures, and documentation where available.
    ///
    /// # Arguments
    ///
    /// * `hierarchy` - The outline hierarchy to format
    ///
    /// # Returns
    ///
    /// Returns a YAML string representing the outline structure, or an error
    /// if formatting fails.
    ///
    /// # Example Output
    ///
    /// ```yaml
    /// outline:
    ///   - name: "main.rs"
    ///     kind: "file"
    ///     path: "src/main.rs"
    ///     children:
    ///       - name: "main"
    ///         kind: "function"
    ///         line: 10
    ///         signature: "fn main()"
    ///         children: null
    /// files_processed: 1
    /// symbols_found: 1
    /// ```
    pub fn format_hierarchy(&self, hierarchy: &OutlineHierarchy) -> Result<String> {
        let mut result = String::new();
        writeln!(result, "outline:")
            .map_err(|e| OutlineError::Generation(format!("Format write error: {e}")))?;

        for file in &hierarchy.files {
            self.format_file(file, 1, &mut result)?;
        }

        writeln!(result, "files_processed: {}", hierarchy.total_files())
            .map_err(|e| OutlineError::Generation(format!("Format write error: {e}")))?;
        writeln!(result, "symbols_found: {}", hierarchy.total_symbols())
            .map_err(|e| OutlineError::Generation(format!("Format write error: {e}")))?;

        Ok(result)
    }

    /// Format a file and its symbols
    fn format_file(&self, file: &FileOutline, depth: usize, result: &mut String) -> Result<()> {
        let indent = " ".repeat(depth * self.config.indent_size);
        let file_name = file
            .file_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown");

        writeln!(
            result,
            "{}  - name: {}",
            indent,
            Self::escape_yaml_string(file_name)
        )
        .map_err(|e| OutlineError::Generation(format!("Format write error: {e}")))?;
        writeln!(result, "{}    kind: \"file\"", indent)
            .map_err(|e| OutlineError::Generation(format!("Format write error: {e}")))?;
        writeln!(
            result,
            "{}    path: {}",
            indent,
            Self::escape_yaml_string(&file.file_path.to_string_lossy())
        )
        .map_err(|e| OutlineError::Generation(format!("Format write error: {e}")))?;

        if file.symbols.is_empty() {
            writeln!(result, "{}    children: null", indent)
                .map_err(|e| OutlineError::Generation(format!("Format write error: {e}")))?;
        } else {
            writeln!(result, "{}    children:", indent)
                .map_err(|e| OutlineError::Generation(format!("Format write error: {e}")))?;

            for symbol in &file.symbols {
                if self.should_include_symbol(symbol) {
                    self.format_symbol(symbol, depth + 2, result)?;
                }
            }
        }

        Ok(())
    }

    /// Format a single symbol and its children
    fn format_symbol(&self, symbol: &OutlineNode, depth: usize, result: &mut String) -> Result<()> {
        let indent = " ".repeat(depth * self.config.indent_size);

        writeln!(
            result,
            "{}  - name: {}",
            indent,
            Self::escape_yaml_string(&symbol.name)
        )
        .map_err(|e| OutlineError::Generation(format!("Format write error: {e}")))?;
        writeln!(
            result,
            "{}    kind: \"{}\"",
            indent,
            symbol.node_type.display_name()
        )
        .map_err(|e| OutlineError::Generation(format!("Format write error: {e}")))?;

        if self.config.include_line_numbers {
            writeln!(result, "{}    line: {}", indent, symbol.start_line)
                .map_err(|e| OutlineError::Generation(format!("Format write error: {e}")))?;
        }

        // Optional signature
        if self.config.include_signatures {
            if let Some(ref signature) = symbol.signature {
                let formatted_sig = self.format_signature(signature)?;
                writeln!(
                    result,
                    "{}    signature: {}",
                    indent,
                    Self::escape_yaml_string(&formatted_sig)
                )
                .map_err(|e| OutlineError::Generation(format!("Format write error: {e}")))?;
            }
        }

        // Optional documentation
        if let Some(ref doc) = symbol.documentation {
            let formatted_doc = self.format_documentation(doc)?;
            writeln!(
                result,
                "{}    doc: {}",
                indent,
                Self::escape_yaml_string(&formatted_doc)
            )
            .map_err(|e| OutlineError::Generation(format!("Format write error: {e}")))?;
        }

        // Optional type information (from visibility)
        if let Some(ref visibility) = symbol.visibility {
            writeln!(
                result,
                "{}    type_info: {}",
                indent,
                Self::escape_yaml_string(&format!("{visibility:?}"))
            )
            .map_err(|e| OutlineError::Generation(format!("Format write error: {e}")))?;
        }

        // Optional children
        if symbol.children.is_empty() {
            writeln!(result, "{}    children: null", indent)
                .map_err(|e| OutlineError::Generation(format!("Format write error: {e}")))?;
        } else {
            writeln!(result, "{}    children:", indent)
                .map_err(|e| OutlineError::Generation(format!("Format write error: {e}")))?;

            for child in &symbol.children {
                if self.should_include_symbol(child) {
                    self.format_symbol(child, depth + 2, result)?;
                }
            }
        }

        Ok(())
    }

    /// Format a signature string, applying length limits
    fn format_signature(&self, signature: &str) -> Result<String> {
        if let Some(max_len) = self.config.max_signature_length {
            if signature.len() > max_len {
                // Truncate long signatures with ellipsis
                let truncated = &signature[..max_len.saturating_sub(3)];
                return Ok(format!("{truncated}..."));
            }
        }
        Ok(signature.to_string())
    }

    /// Format documentation text, cleaning and limiting length  
    fn format_documentation(&self, doc: &str) -> Result<String> {
        // Clean up documentation text
        let cleaned = doc
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        // Limit documentation length for readability
        if cleaned.len() > 200 {
            let truncated = &cleaned[..197];
            Ok(format!("{truncated}..."))
        } else {
            Ok(cleaned)
        }
    }

    /// Escape a string for safe YAML output
    fn escape_yaml_string(s: &str) -> String {
        // Handle YAML string escaping
        if s.contains('\n')
            || s.contains('"')
            || s.contains('\\')
            || s.contains(' ')
            || s.is_empty()
        {
            format!(
                "\"{}\"",
                s.replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('\n', "\\n")
            )
        } else {
            s.to_string()
        }
    }

    /// Check if a symbol should be included based on configuration
    fn should_include_symbol(&self, symbol: &OutlineNode) -> bool {
        if !self.config.include_private_symbols {
            if let Some(SymbolVisibility::Private) = symbol.visibility {
                return false;
            }
        }
        true
    }
}
