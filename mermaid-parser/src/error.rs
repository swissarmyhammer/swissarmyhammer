//! Error types for the Mermaid parser

use std::fmt;

/// Location information for parse errors
#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    pub line: usize,
    pub column: usize,
}

/// Result type alias for parser operations
pub type Result<T> = std::result::Result<T, ParseError>;

/// Errors that can occur during parsing
#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    /// The input is empty or contains only whitespace/comments
    EmptyInput,

    /// Unknown diagram type encountered
    UnknownDiagramType(String),

    /// Diagram type is recognized but not yet supported
    UnsupportedDiagramType(String),

    /// Lexical analysis error
    LexError {
        message: String,
        line: usize,
        column: usize,
    },

    /// Syntax parsing error
    SyntaxError {
        message: String,
        expected: Vec<String>,
        found: String,
        line: usize,
        column: usize,
    },

    /// Enhanced syntax parsing error with context and suggestions
    EnhancedSyntaxError {
        message: String,
        location: Location,
        snippet: Box<String>,
        suggestions: Box<Vec<String>>,
        expected: Box<Vec<String>>,
        found: String,
    },

    /// Semantic error (valid syntax but invalid meaning)
    SemanticError { message: String, context: String },

    /// I/O error when reading input
    IoError(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::EmptyInput => {
                write!(f, "Input is empty or contains no valid diagram content")
            }
            ParseError::UnknownDiagramType(diagram_type) => {
                write!(f, "Unknown diagram type: '{}'", diagram_type)
            }
            ParseError::UnsupportedDiagramType(diagram_type) => {
                write!(f, "Diagram type '{}' is not yet supported", diagram_type)
            }
            ParseError::LexError {
                message,
                line,
                column,
            } => {
                write!(
                    f,
                    "Lexical error at line {}, column {}: {}",
                    line, column, message
                )
            }
            ParseError::SyntaxError {
                message,
                expected,
                found,
                line,
                column,
            } => {
                write!(
                    f,
                    "Syntax error at line {}, column {}: {}. Expected one of: [{}], but found: '{}'",
                    line,
                    column,
                    message,
                    expected.join(", "),
                    found
                )
            }
            ParseError::EnhancedSyntaxError {
                message,
                location,
                snippet,
                suggestions,
                expected,
                found,
            } => {
                writeln!(
                    f,
                    "Syntax error at line {}, column {}: {}",
                    location.line, location.column, message
                )?;
                writeln!(f, "{}", snippet)?;
                if !expected.is_empty() {
                    writeln!(
                        f,
                        "Expected one of: [{}], but found: '{}'",
                        expected.join(", "),
                        found
                    )?;
                }
                if !suggestions.is_empty() {
                    for suggestion in suggestions.iter() {
                        if suggestion.contains("http") || suggestion.starts_with("See ") {
                            writeln!(f, " = help: {}", suggestion)?;
                        } else {
                            writeln!(f, " = note: {}", suggestion)?;
                        }
                    }
                }
                Ok(())
            }
            ParseError::SemanticError { message, context } => {
                write!(f, "Semantic error in {}: {}", context, message)
            }
            ParseError::IoError(message) => {
                write!(f, "I/O error: {}", message)
            }
        }
    }
}

impl std::error::Error for ParseError {}

impl From<std::io::Error> for ParseError {
    fn from(error: std::io::Error) -> Self {
        ParseError::IoError(error.to_string())
    }
}

/// Generate a formatted error snippet showing the error location with context
pub fn format_error_snippet(input: &str, line: usize, column: usize, end_column: usize) -> String {
    let lines: Vec<&str> = input.lines().collect();
    let mut result = String::new();

    // Ensure line is 1-indexed and convert to 0-indexed for array access
    let zero_indexed_line = if line > 0 { line - 1 } else { 0 };

    if zero_indexed_line < lines.len() {
        let problem_line = lines[zero_indexed_line];

        // Show the line with line number
        result.push_str(&format!("{} | {}\n", line, problem_line));

        // Add pointer line showing the error location
        let padding = format!("{} | ", line).len();
        let mut pointer_line = " ".repeat(padding);

        // Add spaces to align with the error column (1-indexed to 0-indexed)
        let start_pos = if column > 0 { column - 1 } else { 0 };
        pointer_line.push_str(&" ".repeat(start_pos));

        // Add carets to highlight the error span
        let span_length = if end_column > column {
            end_column - column
        } else {
            1
        };
        pointer_line.push_str(&"^".repeat(span_length));
        pointer_line.push_str(" expected");

        result.push_str(&pointer_line);
    }

    result
}

// Note: Chumsky error conversion will be implemented later
// when we have actual parsers using the library

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let error = ParseError::UnknownDiagramType("invalid".to_string());
        assert_eq!(error.to_string(), "Unknown diagram type: 'invalid'");

        let error = ParseError::EmptyInput;
        assert_eq!(
            error.to_string(),
            "Input is empty or contains no valid diagram content"
        );

        let error = ParseError::SyntaxError {
            message: "Unexpected token".to_string(),
            expected: vec!["identifier".to_string(), "number".to_string()],
            found: "symbol".to_string(),
            line: 5,
            column: 10,
        };
        assert!(error.to_string().contains("line 5, column 10"));
        assert!(error.to_string().contains("identifier, number"));
    }

    #[test]
    fn test_error_equality() {
        let error1 = ParseError::EmptyInput;
        let error2 = ParseError::EmptyInput;
        assert_eq!(error1, error2);

        let error3 = ParseError::UnknownDiagramType("test".to_string());
        assert_ne!(error1, error3);
    }

    #[test]
    fn test_enhanced_syntax_error_with_context() {
        let _input = "flowchart TD\n    A => B\n    B --> C";
        let error = ParseError::SyntaxError {
            message: "Expected arrow '->' or '-->' after node".to_string(),
            expected: vec!["-->".to_string(), "->".to_string()],
            found: "=>".to_string(),
            line: 2,
            column: 7,
        };

        let error_msg = error.to_string();
        assert!(error_msg.contains("line 2, column 7"));
        assert!(error_msg.contains("-->"));
        assert!(error_msg.contains("=>"));
    }

    #[test]
    fn test_enhanced_error_with_suggestions() {
        // This test expects a new error variant with suggestions
        // Will fail until we implement ParseError::EnhancedSyntaxError
        let input = "flowchart TD\n    A => B";
        let error = ParseError::EnhancedSyntaxError {
            message: "Expected arrow '->' or '-->' after node".to_string(),
            location: Location { line: 2, column: 7 },
            snippet: Box::new(format_error_snippet(input, 2, 7, 9)),
            suggestions: Box::new(vec![
                "Did you mean '-->'?".to_string(),
                "See https://mermaid.js.org/syntax/flowchart.html#links-between-nodes".to_string(),
            ]),
            expected: Box::new(vec!["-->".to_string(), "->".to_string()]),
            found: "=>".to_string(),
        };

        let error_msg = error.to_string();
        assert!(error_msg.contains("2 |     A => B"));
        assert!(error_msg.contains("^^ expected"));
        assert!(error_msg.contains("Did you mean"));
        assert!(error_msg.contains("help:"));
    }
}
